use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct MidiParameter {
    pub name: String,
    pub cc: u8,
    pub category: String,
}

pub struct MidiMap {
    params_by_cc: HashMap<u8, MidiParameter>,
}

impl MidiMap {
    pub fn new() -> Self {
        let mut params_by_cc = HashMap::new();

        // Track parameters
        let track_params = vec![
            (93, "Solo"),
            (94, "Global Mute"),
            (110, "Pattern Mute"),
            (95, "Track Level"),
        ];
        for (cc, name) in track_params {
            params_by_cc.insert(cc, MidiParameter {
                name: name.to_string(),
                cc,
                category: "Track".to_string(),
            });
        }

        // Trig parameters
        let trig_params = vec![
            (3, "Trig Note"),
            (4, "Trig Velocity"),
            (5, "Trig Length"),
            (13, "Filter Trig"),
            (14, "LFO Trig"),
        ];
        for (cc, name) in trig_params {
            params_by_cc.insert(cc, MidiParameter {
                name: name.to_string(),
                cc,
                category: "Trig".to_string(),
            });
        }

        // Source parameters
        let source_params = vec![
            (16, "Source Tune"),
            (17, "Source Play Mode"),
            (18, "Source Bit Reduction"),
            (19, "Source Sample Slot"),
            (20, "Source Start"),
            (21, "Source Length"),
            (22, "Source Loop Position"),
            (23, "Source Sample Level"),
        ];
        for (cc, name) in source_params {
            params_by_cc.insert(cc, MidiParameter {
                name: name.to_string(),
                cc,
                category: "Source".to_string(),
            });
        }

        // Filter parameters
        let filter_params = vec![
            (74, "Filter Frequency"),
            (75, "Resonance"),
            (76, "Filter Type"),
            (70, "Filter Attack Time"),
            (71, "Filter Decay Time"),
            (72, "Filter Sustain Level"),
            (73, "Filter Release Time"),
            (77, "Filter Env Depth"),
        ];
        for (cc, name) in filter_params {
            params_by_cc.insert(cc, MidiParameter {
                name: name.to_string(),
                cc,
                category: "Filter".to_string(),
            });
        }

        // Amp parameters
        let amp_params = vec![
            (78, "Amp Attack Time"),
            (79, "Amp Hold Time"),
            (80, "Amp Decay Time"),
            (81, "Amp Overdrive"),
            (82, "Amp Delay Send"),
            (83, "Amp Reverb Send"),
            (10, "Amp Pan"),
            (7, "Amp Volume"),
        ];
        for (cc, name) in amp_params {
            params_by_cc.insert(cc, MidiParameter {
                name: name.to_string(),
                cc,
                category: "Amp".to_string(),
            });
        }

        // LFO parameters
        let lfo_params = vec![
            (102, "LFO Speed"),
            (103, "LFO Multiplier"),
            (104, "LFO Fade In/Out"),
            (105, "LFO Destination"),
            (106, "LFO Waveform"),
            (107, "LFO Start Phase"),
            (108, "LFO Trig Mode"),
            (109, "LFO Depth"),
        ];
        for (cc, name) in lfo_params {
            params_by_cc.insert(cc, MidiParameter {
                name: name.to_string(),
                cc,
                category: "LFO".to_string(),
            });
        }

        // FX Delay parameters
        let fx_delay_params = vec![
            (85, "FX Delay Time"),
            (86, "FX Pingpong"),
            (87, "FX Stereo Width"),
            (88, "FX Feedback"),
            (89, "FX Highpass Filter"),
            (90, "FX Lowpass Filter"),
            (91, "FX Reverb Send"),
            (92, "FX Mix Volume"),
        ];
        for (cc, name) in fx_delay_params {
            params_by_cc.insert(cc, MidiParameter {
                name: name.to_string(),
                cc,
                category: "FX Delay".to_string(),
            });
        }

        // FX Reverb parameters
        let fx_reverb_params = vec![
            (24, "FX Reverb Predelay"),
            (25, "FX Reverb Decay Time"),
            (2, "FX Reverb Shelving Freq"),
            (27, "FX Reverb Shelving Gain"),
            (28, "FX Reverb Highpass Filter"),
            (29, "FX Reverb Lowpass Filter"),
            (31, "FX Reverb Mix Volume"),
        ];
        for (cc, name) in fx_reverb_params {
            params_by_cc.insert(cc, MidiParameter {
                name: name.to_string(),
                cc,
                category: "FX Reverb".to_string(),
            });
        }

        MidiMap { params_by_cc }
    }

    pub fn get_parameter(&self, cc: u8) -> Option<MidiParameter> {
        self.params_by_cc.get(&cc).cloned()
    }

    pub fn get_name(&self, cc: u8) -> String {
        self.params_by_cc
            .get(&cc)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("CC {}", cc))
    }

    pub fn get_all_parameters(&self) -> Vec<MidiParameter> {
        let mut params: Vec<_> = self.params_by_cc.values().cloned().collect();
        params.sort_by_key(|p| p.cc);
        params
    }
}