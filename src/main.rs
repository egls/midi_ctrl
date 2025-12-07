use anyhow::{Context, Result};
use clap::Parser;
use midir::{MidiOutput, MidiOutputConnection};
use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(feature = "egui")]
mod gui;

mod midi_map;

#[derive(Parser, Debug)]
#[command(author, version, about = "Simple Digitakt MIDI controller - CLI + egui UI")]
struct Args {
    /// MIDI output port index (0-based). If omitted, you'll be prompted.
    #[arg(short, long)]
    port: Option<usize>,

    /// MIDI channel (1-16). Defaults to 1.
    #[arg(short, long, default_value_t = 1)]
    channel: u8,
}

fn list_midi_outputs(midi_out: &MidiOutput) -> Result<Vec<String>> {
    let ports = midi_out.ports();
    let mut names = Vec::new();
    for p in ports.iter() {
        let name = midi_out
            .port_name(p)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "Unknown".into());
        names.push(name);
    }
    Ok(names)
}

// create a local MidiOutput to avoid ownership problems when connecting
fn open_output(port_index: usize) -> Result<MidiOutputConnection> {
    let midi_out = MidiOutput::new("midi_ctrl")?;
    let ports = midi_out.ports();
    let port = ports
        .get(port_index)
        .with_context(|| format!("No MIDI output port at index {}", port_index))?;
    let port_name = midi_out
        .port_name(port)
        .unwrap_or_else(|_| "<unknown>".to_string());
    let conn_out = midi_out
        .connect(port, &format!("midi_ctrl-{}", port_name))
        .with_context(|| format!("Failed to connect to port '{}'", port_name))?;
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

fn send_program_change(conn: &mut MidiOutputConnection, channel: u8, program: u8) -> Result<()> {
    let status = 0xC0 | ((channel - 1) & 0x0F);
    conn.send(&[status, program])?;
    Ok(())
}

fn send_note_on(conn: &mut MidiOutputConnection, channel: u8, note: u8, vel: u8) -> Result<()> {
    let status = 0x90 | ((channel - 1) & 0x0F);
    conn.send(&[status, note, vel])?;
    Ok(())
}

fn send_note_off(conn: &mut MidiOutputConnection, channel: u8, note: u8) -> Result<()> {
    let status = 0x80 | ((channel - 1) & 0x0F);
    conn.send(&[status, note, 0])?;
    Ok(())
}

fn interactive_loop(conn: MidiOutputConnection, channel: u8) -> Result<()> {
    println!("Interactive MIDI controller");
    println!("Type `help` for commands. `exit` or Ctrl+C to quit.");

    let conn = Arc::new(Mutex::new(conn));
    // Spawn a small thread to periodically send a small keepalive if desired (optional)
    let _keepalive_conn = Arc::clone(&conn);
    thread::spawn(move || {
        // no-op now; could poll device state or send heartbeat if needed
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    });

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let s = match line {
            Ok(s) => s.trim().to_string(),
            Err(_) => break,
        };
        let args: Vec<&str> = s.split_whitespace().collect();
        if args.is_empty() {
            continue;
        }
        match args[0].to_lowercase().as_str() {
            "help" => {
                println!("Commands:");
                println!("  cc <controller 0-127> <value 0-127>   Send CC");
                println!("  start                                  Send MIDI Start (0xFA)");
                println!("  stop                                   Send MIDI Stop (0xFC)");
                println!("  continue                               Send MIDI Continue (0xFB)");
                println!("  pc <program 0-127>                     Program change");
                println!("  noteon <note> <vel>                    Note on");
                println!("  noteoff <note>                         Note off");
                println!("  list                                   Show MIDI CC controllers 0-127");
                println!("  exit                                   Quit");
            }
            "cc" => {
                if args.len() < 3 {
                    println!("Usage: cc <controller> <value>");
                    continue;
                }
                if let (Ok(controller), Ok(value)) = (args[1].parse::<u8>(), args[2].parse::<u8>()) {
                    let mut c = conn.lock().unwrap();
                    if let Err(e) = send_cc(&mut *c, channel, controller, value) {
                        eprintln!("Failed to send CC: {:?}", e);
                    }
                } else {
                    println!("controller and value must be integers 0-127");
                }
            }
            "start" => {
                let mut c = conn.lock().unwrap();
                if let Err(e) = send_realtime(&mut *c, 0xFA) {
                    eprintln!("Failed to send Start: {:?}", e);
                } else {
                    // Send a few clock ticks to ensure devices that expect clock see it
                    for _ in 0..6 {
                        if let Err(e) = send_realtime(&mut *c, 0xF8) {
                            eprintln!("Failed to send Clock tick: {:?}", e);
                        }
                        thread::sleep(Duration::from_millis(8));
                    }
                }
            }
            "stop" => {
                let mut c = conn.lock().unwrap();
                if let Err(e) = send_realtime(&mut *c, 0xFC) {
                    eprintln!("Failed to send Stop: {:?}", e);
                }
            }
            "continue" => {
                let mut c = conn.lock().unwrap();
                if let Err(e) = send_realtime(&mut *c, 0xFB) {
                    eprintln!("Failed to send Continue: {:?}", e);
                }
            }
            "pc" => {
                if args.len() < 2 {
                    println!("Usage: pc <program>");
                    continue;
                }
                if let Ok(program) = args[1].parse::<u8>() {
                    let mut c = conn.lock().unwrap();
                    if let Err(e) = send_program_change(&mut *c, channel, program) {
                        eprintln!("Failed to send Program Change: {:?}", e);
                    }
                } else {
                    println!("program must be integer 0-127");
                }
            }
            "noteon" => {
                if args.len() < 3 {
                    println!("Usage: noteon <note> <vel>");
                    continue;
                }
                if let (Ok(note), Ok(vel)) = (args[1].parse::<u8>(), args[2].parse::<u8>()) {
                    let mut c = conn.lock().unwrap();
                    if let Err(e) = send_note_on(&mut *c, channel, note, vel) {
                        eprintln!("Failed to send Note On: {:?}", e);
                    }
                } else {
                    println!("note and vel must be integer 0-127");
                }
            }
            "noteoff" => {
                if args.len() < 2 {
                    println!("Usage: noteoff <note>");
                    continue;
                }
                if let Ok(note) = args[1].parse::<u8>() {
                    let mut c = conn.lock().unwrap();
                    if let Err(e) = send_note_off(&mut *c, channel, note) {
                        eprintln!("Failed to send Note Off: {:?}", e);
                    }
                } else {
                    println!("note must be integer 0-127");
                }
            }
            "list" => {
                println!("Controllers 0..127 are addressable via `cc` command.");
            }
            "exit" => break,
            other => {
                println!("Unknown command: {}", other);
                println!("Type `help` for commands.");
            }
        }
        // small prompt to indicate ready
        print!("> ");
        io::stdout().flush().ok();
    }

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let midi_out = MidiOutput::new("midi_ctrl")?;
    let port_names = list_midi_outputs(&midi_out)?;

    if cfg!(feature = "egui") {
        // If GUI feature enabled, run GUI and exit CLI path.
        #[cfg(feature = "egui")]
        {
            if port_names.is_empty() {
                println!("No MIDI output ports found. Connect your Digitakt or a virtual port and try again.");
                return Ok(());
            }
            println!("Launching GUI...");

            // run the GUI (it spawns the background thread internally)
            gui::run_gui(midi_out, port_names, args.channel)?;
            return Ok(());
        }
    }

    // Default: interactive CLI
    if port_names.is_empty() {
        println!("No MIDI output ports found. Connect your Digitakt or a virtual port and try again.");
        return Ok(());
    }

    println!("MIDI Output Ports:");
    for (i, name) in port_names.iter().enumerate() {
        println!("  {}: {}", i, name);
    }

    let selected = if let Some(idx) = args.port {
        idx
    } else {
        // Prompt user to select a port index
        print!("Select output port index: ");
        io::stdout().flush().ok();
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        line.trim().parse::<usize>().unwrap_or(0)
    };

    // updated call: pass only the index
    let conn = open_output(selected)
        .with_context(|| "Failed to open MIDI output")?;

    interactive_loop(conn, args.channel)?;
    Ok(())
}