use anyhow::Result;
use clap::Parser;
use midir::MidiOutput;

mod gui;
mod midi_map;

#[derive(Parser, Debug)]
#[command(author, version, about = "Digitakt MIDI controller")]
struct Args {
    /// MIDI channel (1-16). Defaults to 1.
    #[arg(short, long, default_value_t = 1)]
    channel: u8,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let midi_out = MidiOutput::new("midi_ctrl")?;
    
    // List available MIDI ports
    let ports = midi_out.ports();
    let mut port_names = Vec::new();
    for p in ports.iter() {
        let name = midi_out
            .port_name(p)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "Unknown".to_string());
        port_names.push(name);
    }

    // Launch GUI
    gui::run_gui(midi_out, port_names, args.channel)?;
    
    Ok(())
}