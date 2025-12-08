use anyhow::Result;
use eframe::{egui, NativeOptions};
use midir::{MidiOutput, MidiOutputConnection};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use crate::midi_map::MidiMap;

#[derive(Debug, Clone)]
pub enum MidiCommand {
    Connect(Option<usize>, u8),
    Disconnect,
    SendCC { channel: u8, controller: u8, value: u8 },
    Start,
    Stop,
    Continue,
    QueryDevice,
    SetBpm(f32),
    Quit,
}

#[derive(Debug, Clone)]
pub enum DeviceState {
    Artist(String),
    Bpm(f32),
}

fn open_output(port_index: usize) -> Result<MidiOutputConnection> {
    let midi_out = MidiOutput::new("midi_ctrl")?;
    let ports = midi_out.ports();
    let port = ports.get(port_index).ok_or_else(|| {
        anyhow::anyhow!("No MIDI output port at index {}", port_index)
    })?;
    let port_name = midi_out
        .port_name(port)
        .unwrap_or_else(|_| "<unknown>".to_string());
    let conn_out = midi_out
        .connect(port, &format!("midi_ctrl-{}", port_name))?;
    Ok(conn_out)
}

fn send_realtime(conn: &mut MidiOutputConnection, byte: u8) -> Result<()> {
    conn.send(&[byte])?;
    Ok(())
}

fn send_cc(conn: &mut MidiOutputConnection, channel: u8, controller: u8, value: u8) -> Result<()> {
    let status = 0xB0 | ((channel - 1) & 0x0F);
    conn.send(&[status, controller, value])?;
    Ok(())
}

fn send_timing_clock(conn: &mut MidiOutputConnection, bpm: f32, ticks: u32) -> Result<()> {
    // Send timing clock pulses at the given BPM
    // MIDI clock = 24 pulses per quarter note
    // Time between pulses = 60 / (BPM * 24) seconds
    let ms_per_tick = (60.0 / (bpm * 24.0)) * 1000.0;
    
    for _ in 0..ticks {
        send_realtime(conn, 0xF8)?; // Clock (0xF8)
        let duration = std::time::Duration::from_millis(ms_per_tick as u64);
        thread::sleep(duration);
    }
    Ok(())
}

pub fn run_gui(_midi_out: MidiOutput, port_names: Vec<String>, initial_channel: u8) -> Result<()> {
    let (tx, rx) = mpsc::channel::<MidiCommand>();
    let (state_tx, state_rx) = mpsc::channel::<DeviceState>();

    // Background thread owns the MidiOutputConnection and performs sends.
    thread::spawn(move || {
        let mut conn: Option<MidiOutputConnection> = None;
        let mut _current_port: Option<usize> = None;
        let mut _current_channel: u8 = initial_channel;
        let mut current_bpm: f32 = 120.0;

        for cmd in rx {
            match cmd {
                MidiCommand::Connect(maybe_idx, ch) => {
                    _current_channel = ch;
                    if let Some(idx) = maybe_idx {
                        match open_output(idx) {
                            Ok(c) => {
                                conn = Some(c);
                                _current_port = Some(idx);
                                eprintln!("✓ Connected to port {}", idx);
                                // Broadcast device state on connect
                                let _ = state_tx.send(DeviceState::Artist("Digitakt".to_string()));
                                let _ = state_tx.send(DeviceState::Bpm(current_bpm));
                            }
                            Err(e) => eprintln!("✗ Failed to connect: {:?}", e),
                        }
                    }
                }
                MidiCommand::Disconnect => {
                    conn = None;
                    _current_port = None;
                    eprintln!("✓ Disconnected");
                }
                MidiCommand::SendCC { channel, controller, value } => {
                    if let Some(ref mut c) = conn {
                        if let Err(e) = send_cc(c, channel, controller, value) {
                            eprintln!("✗ Failed to send CC {}: {:?}", controller, e);
                        } else {
                            eprintln!("→ CC {} = {} (ch {})", controller, value, channel);
                        }
                    }
                }
                MidiCommand::Start => {
                    if let Some(ref mut c) = conn {
                        if let Err(e) = send_realtime(c, 0xFA) {
                            eprintln!("✗ Failed to send Start: {:?}", e);
                        } else {
                            eprintln!("► Start");
                            for _ in 0..6 {
                                if let Err(e) = send_realtime(c, 0xF8) {
                                    eprintln!("✗ Failed to send Clock tick: {:?}", e);
                                }
                                std::thread::sleep(std::time::Duration::from_millis(8));
                            }
                        }
                    }
                }
                MidiCommand::Stop => {
                    if let Some(ref mut c) = conn {
                        if let Err(e) = send_realtime(c, 0xFC) {
                            eprintln!("✗ Failed to send Stop: {:?}", e);
                        } else {
                            eprintln!("⏹ Stop");
                        }
                    }
                }
                MidiCommand::Continue => {
                    if let Some(ref mut c) = conn {
                        if let Err(e) = send_realtime(c, 0xFB) {
                            eprintln!("✗ Failed to send Continue: {:?}", e);
                        } else {
                            eprintln!("→ Continue");
                        }
                    }
                }
                MidiCommand::QueryDevice => {
                    // Broadcast current device state
                    let _ = state_tx.send(DeviceState::Artist("Digitakt".to_string()));
                    let _ = state_tx.send(DeviceState::Bpm(current_bpm));
                }
                MidiCommand::SetBpm(bpm) => {
                    current_bpm = bpm;
                    eprintln!("⏱ BPM set to {}", bpm);
                    let _ = state_tx.send(DeviceState::Bpm(bpm));
                }
                MidiCommand::Quit => {
                    break;
                }
            }
        }
    });

    let app = MidiGuiApp::new(port_names, tx, state_rx, initial_channel);
    let native_options = NativeOptions::default();
    eframe::run_native(
        "midi_ctrl - Digitakt MIDI controller",
        native_options,
        Box::new(|_cc| Box::new(app)),
    );

    Ok(())
}

struct MidiGuiApp {
    port_names: Vec<String>,
    tx: Sender<MidiCommand>,
    state_rx: Receiver<DeviceState>,
    selected_port: Option<usize>,
    channel: u8,
    cc_values: Vec<i32>,
    connected: bool,
    last_sent_cc: Option<(u8, u8)>,
    last_sent_time: Option<std::time::Instant>,
    midi_map: MidiMap,
    device_artist: String,
    device_bpm: f32,
}

impl MidiGuiApp {
    fn new(port_names: Vec<String>, tx: Sender<MidiCommand>, state_rx: Receiver<DeviceState>, initial_channel: u8) -> Self {
        Self {
            port_names,
            tx,
            state_rx,
            selected_port: None,
            channel: initial_channel,
            cc_values: vec![0i32; 128],
            connected: false,
            last_sent_cc: None,
            last_sent_time: None,
            midi_map: MidiMap::new(),
            device_artist: "Unknown".to_string(),
            device_bpm: 120.0,
        }
    }

    fn update_device_state(&mut self) {
        // Drain all pending device state updates
        while let Ok(state) = self.state_rx.try_recv() {
            match state {
                DeviceState::Artist(artist) => {
                    self.device_artist = artist;
                }
                DeviceState::Bpm(bpm) => {
                    self.device_bpm = bpm;
                }
            }
        }
    }
}

impl eframe::App for MidiGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update device state from background thread
        self.update_device_state();

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("MIDI Port:");
                if self.port_names.is_empty() {
                    ui.label("No ports available");
                } else {
                    let mut selected_label = "None".to_string();
                    if let Some(idx) = self.selected_port {
                        if let Some(n) = self.port_names.get(idx) {
                            selected_label = format!("{} (#{})", n, idx);
                        }
                    }
                    egui::ComboBox::from_label("")
                        .selected_text(selected_label)
                        .show_ui(ui, |ui| {
                            for (i, name) in self.port_names.iter().enumerate() {
                                let label = format!("{} (#{})", name, i);
                                if ui.selectable_value(&mut self.selected_port, Some(i), label).clicked() {
                                }
                            }
                            if ui.selectable_value(&mut self.selected_port, None, "None").clicked() {
                            }
                        });
                }

                ui.label("Channel:");
                ui.add(egui::DragValue::new(&mut self.channel).clamp_range(1..=16));

                if !self.connected {
                    if ui.button("Connect").clicked() {
                        let _ = self.tx.send(MidiCommand::Connect(self.selected_port, self.channel));
                        self.connected = true;
                    }
                } else {
                    ui.colored_label(egui::Color32::GREEN, "✓ Connected");
                    if ui.button("Disconnect").clicked() {
                        let _ = self.tx.send(MidiCommand::Disconnect);
                        self.connected = false;
                    }
                    
                    // Show device info
                    ui.separator();
                    ui.label(format!("Artist: {}", self.device_artist));
                    
                    // BPM control
                    ui.label("BPM:");
                    let mut bpm_value = self.device_bpm;
                    if ui.add(egui::Slider::new(&mut bpm_value, 20.0..=300.0).show_value(true)).changed() {
                        let _ = self.tx.send(MidiCommand::SetBpm(bpm_value));
                    }
                }

                ui.separator();

                if ui.button("▶ Start").clicked() {
                    let _ = self.tx.send(MidiCommand::Start);
                }
                if ui.button("⏹ Stop").clicked() {
                    let _ = self.tx.send(MidiCommand::Stop);
                }
                if ui.button("→ Continue").clicked() {
                    let _ = self.tx.send(MidiCommand::Continue);
                }

                if let Some((cc, val)) = self.last_sent_cc {
                    if let Some(time) = self.last_sent_time {
                        let elapsed = time.elapsed().as_secs_f32();
                        if elapsed < 2.0 {
                            let param_name = self.midi_map.get_name(cc);
                            ui.label(format!("Last: {} = {}", param_name, val));
                        }
                    }
                }
            });
        });

               egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Digitakt Parameters");
            ui.label("Move sliders to send CC values to your Digitakt");
            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                let mut categories: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
                
                for cc in 0..128u8 {
                    if let Some(param) = self.midi_map.get_parameter(cc) {
                        categories.entry(param.category.clone())
                            .or_insert_with(Vec::new)
                            .push(cc);
                    }
                }

                let mut sorted_categories: Vec<_> = categories.into_iter().collect();
                sorted_categories.sort_by(|a, b| a.0.cmp(&b.0));

                ui.horizontal(|ui| {
                    // Left column
                    ui.vertical(|ui| {
                        let half = (sorted_categories.len() + 1) / 2;
                        for (category, mut ccs) in sorted_categories.iter().take(half).cloned() {
                            ccs.sort();
                            
                            ui.group(|ui| {
                                ui.heading(&category);
                                
                                let cols = 2;
                                for row in 0..((ccs.len() + cols - 1) / cols) {
                                    ui.horizontal(|ui| {
                                        for col in 0..cols {
                                            let idx = row * cols + col;
                                            if idx >= ccs.len() {
                                                break;
                                            }
                                            
                                            let cc = ccs[idx];
                                            let param_name = self.midi_map.get_name(cc);
                                            
                                            ui.vertical(|ui| {
                                                ui.label(&param_name);
                                                
                                                let slider_response = ui.add(
                                                    egui::Slider::new(&mut self.cc_values[cc as usize], 0..=127)
                                                        .show_value(true)
                                                );
                                                
                                                if slider_response.changed() {
                                                    let new_val = self.cc_values[cc as usize] as u8;
                                                    let _ = self.tx.send(MidiCommand::SendCC {
                                                        channel: self.channel,
                                                        controller: cc,
                                                        value: new_val,
                                                    });
                                                    self.last_sent_cc = Some((cc, new_val));
                                                    self.last_sent_time = Some(std::time::Instant::now());
                                                }
                                                
                                                ui.label(format!("Value: {}", self.cc_values[cc as usize]));
                                            });
                                            
                                            ui.separator();
                                        }
                                    });
                                }
                            });
                        }
                    });

                    // Right column
                    ui.vertical(|ui| {
                        let half = (sorted_categories.len() + 1) / 2;
                        for (category, mut ccs) in sorted_categories.iter().skip(half).cloned() {
                            ccs.sort();
                            
                            ui.group(|ui| {
                                ui.heading(&category);
                                
                                let cols = 2;
                                for row in 0..((ccs.len() + cols - 1) / cols) {
                                    ui.horizontal(|ui| {
                                        for col in 0..cols {
                                            let idx = row * cols + col;
                                            if idx >= ccs.len() {
                                                break;
                                            }
                                            
                                            let cc = ccs[idx];
                                            let param_name = self.midi_map.get_name(cc);
                                            
                                            ui.vertical(|ui| {
                                                ui.label(&param_name);
                                                
                                                let slider_response = ui.add(
                                                    egui::Slider::new(&mut self.cc_values[cc as usize], 0..=127)
                                                        .show_value(true)
                                                );
                                                
                                                if slider_response.changed() {
                                                    let new_val = self.cc_values[cc as usize] as u8;
                                                    let _ = self.tx.send(MidiCommand::SendCC {
                                                        channel: self.channel,
                                                        controller: cc,
                                                        value: new_val,
                                                    });
                                                    self.last_sent_cc = Some((cc, new_val));
                                                    self.last_sent_time = Some(std::time::Instant::now());
                                                }
                                                
                                                ui.label(format!("Value: {}", self.cc_values[cc as usize]));
                                            });
                                            
                                            ui.separator();
                                        }
                                    });
                                }
                            });
                        }
                    });
                });
            });
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
                        let _ = self.tx.send(MidiCommand::Quit);
                        std::process::exit(0);
                    }
                });
            });
        });
    }
}