use crate::{send_cc, send_note_off, send_note_on, send_program_change, send_realtime, open_output};
use anyhow::Result;
use eframe::{egui, NativeOptions};
use midir::{MidiOutput, MidiOutputConnection};
use std::sync::mpsc::{self, Sender};
use std::thread;

#[derive(Debug)]
pub enum MidiCommand {
    Connect(Option<usize>, u8), // port index, channel
    Disconnect,
    SendCC { channel: u8, controller: u8, value: u8 },
    Start,
    Stop,
    Continue,
    ProgramChange { channel: u8, program: u8 },
    NoteOn { channel: u8, note: u8, vel: u8 },
    NoteOff { channel: u8, note: u8 },
    Quit,
}

pub fn run_gui(_midi_out: MidiOutput, port_names: Vec<String>, initial_channel: u8) -> Result<()> {
    let (tx, rx) = mpsc::channel::<MidiCommand>();

    // Background thread owns the MidiOutputConnection and performs sends.
    thread::spawn(move || {
        let mut conn: Option<MidiOutputConnection> = None;
        let mut _current_port: Option<usize> = None;
        let mut _current_channel: u8 = initial_channel;

        for cmd in rx {
            match cmd {
                MidiCommand::Connect(maybe_idx, ch) => {
                    _current_channel = ch;
                    if let Some(idx) = maybe_idx {
                        match open_output(idx) {
                            Ok(c) => {
                                conn = Some(c);
                                _current_port = Some(idx);
                                eprintln!("Connected to port {}", idx);
                            }
                            Err(e) => eprintln!("Failed to connect: {:?}", e),
                        }
                    }
                }
                MidiCommand::Disconnect => {
                    conn = None;
                    _current_port = None;
                    eprintln!("Disconnected");
                }
                MidiCommand::SendCC { channel, controller, value } => {
                    if let Some(ref mut c) = conn {
                        if let Err(e) = send_cc(c, channel, controller, value) {
                            eprintln!("Failed to send CC: {:?}", e);
                        }
                    } else {
                        eprintln!("Not connected: cannot send CC");
                    }
                }
                MidiCommand::Start => {
                    if let Some(ref mut c) = conn {
                        if let Err(e) = send_realtime(c, 0xFA) {
                            eprintln!("Failed to send Start: {:?}", e);
                        } else {
                            for _ in 0..6 {
                                if let Err(e) = send_realtime(c, 0xF8) {
                                    eprintln!("Failed to send Clock tick: {:?}", e);
                                }
                                std::thread::sleep(std::time::Duration::from_millis(8));
                            }
                        }
                    }
                }
                MidiCommand::Stop => {
                    if let Some(ref mut c) = conn {
                        if let Err(e) = send_realtime(c, 0xFC) {
                            eprintln!("Failed to send Stop: {:?}", e);
                        }
                    }
                }
                MidiCommand::Continue => {
                    if let Some(ref mut c) = conn {
                        if let Err(e) = send_realtime(c, 0xFB) {
                            eprintln!("Failed to send Continue: {:?}", e);
                        }
                    }
                }
                MidiCommand::ProgramChange { channel, program } => {
                    if let Some(ref mut c) = conn {
                        if let Err(e) = send_program_change(c, channel, program) {
                            eprintln!("Failed to send PC: {:?}", e);
                        }
                    }
                }
                MidiCommand::NoteOn { channel, note, vel } => {
                    if let Some(ref mut c) = conn {
                        if let Err(e) = send_note_on(c, channel, note, vel) {
                            eprintln!("Failed to send NoteOn: {:?}", e);
                        }
                    }
                }
                MidiCommand::NoteOff { channel, note } => {
                    if let Some(ref mut c) = conn {
                        if let Err(e) = send_note_off(c, channel, note) {
                            eprintln!("Failed to send NoteOff: {:?}", e);
                        }
                    }
                }
                MidiCommand::Quit => {
                    break;
                }
            }
        }
    });

    // Build and run the eframe app
    let app = MidiGuiApp::new(port_names, tx, initial_channel);
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
    selected_port: Option<usize>,
    channel: u8,
    cc_values: Vec<u8>,
    connected: bool,
}

impl MidiGuiApp {
    fn new(port_names: Vec<String>, tx: Sender<MidiCommand>, initial_channel: u8) -> Self {
        Self {
            port_names,
            tx,
            selected_port: None,
            channel: initial_channel,
            cc_values: vec![0u8; 128],
            connected: false,
        }
    }
}

impl eframe::App for MidiGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                                    // selection changed
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
                    if ui.button("Disconnect").clicked() {
                        let _ = self.tx.send(MidiCommand::Disconnect);
                        self.connected = false;
                    }
                }

                if ui.button("Start").clicked() {
                    let _ = self.tx.send(MidiCommand::Start);
                }
                if ui.button("Stop").clicked() {
                    let _ = self.tx.send(MidiCommand::Stop);
                }
                if ui.button("Continue").clicked() {
                    let _ = self.tx.send(MidiCommand::Continue);
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Controllers (CC 0..127)");
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Show sliders in rows of 4 to save vertical space
                let cols = 4;
                for row in 0..((128 + cols - 1) / cols) {
                    ui.horizontal(|ui| {
                        for col in 0..cols {
                            let idx = row * cols + col;
                            if idx >= 128 {
                                break;
                            }
                            // slider text like "CC 0: 64"
                            let mut v = self.cc_values[idx] as i32;
                            if ui.vertical(|ui| {
                                ui.label(format!("CC {}", idx));
                                let slider = egui::Slider::new(&mut v, 0..=127).show_value(false);
                                ui.add(slider)
                            }).response.changed() {
                                // changed
                                let new_v = v as u8;
                                self.cc_values[idx] = new_v;
                                let _ = self.tx.send(MidiCommand::SendCC {
                                    channel: self.channel,
                                    controller: idx as u8,
                                    value: new_v,
                                });
                            }
                            // small spacer
                            ui.separator();
                        }
                    });
                }
            });
        });

        // Add a small close button in the bottom-right
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Align right-to-left; supply vertical Align argument (Center works well here).
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
                        let _ = self.tx.send(MidiCommand::Quit);
                        // close the native window by exiting the process.
                        std::process::exit(0);
                    }
                });
            });
        });
    }
}