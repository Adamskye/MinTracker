use egui::Key;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Keybinds {
    pub up: Key,
    pub down: Key,
    pub left: Key,
    pub right: Key,

    pub increase: Key,
    pub decrease: Key,

    pub play_pause: Key,

    pub show_tracks: Key,
    pub show_chains: Key,
    pub show_phrases: Key,
    pub show_instruments: Key,
    pub show_preferences: Key,

    pub trigger_cell: Key,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            up: Key::W,
            down: Key::S,
            left: Key::A,
            right: Key::D,

            increase: Key::Equals,
            decrease: Key::Minus,

            play_pause: Key::Space,

            show_tracks: Key::F1,
            show_chains: Key::F2,
            show_phrases: Key::F3,
            show_instruments: Key::F4,
            show_preferences: Key::P,

            trigger_cell: Key::Enter,
        }
    }
}
