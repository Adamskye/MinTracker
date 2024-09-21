use serde::{Deserialize, Serialize};

use crate::helpers;

use super::NoteEffects;

pub const NUM_SEMITONES: u8 = 108;
pub const MID_A_SEMITONE: u8 = 57;
pub const MID_A_FREQUENCY: f32 = 440.0;

#[derive(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    semitone: Option<u8>,

    #[serde(default)]
    pub effects: NoteEffects,
}

impl Note {
    pub fn new(semitone: Option<u8>) -> Self {
        let mut note = Self::default();
        note.set_semitone(semitone);
        note
    }

    pub fn has_effects(&self) -> bool {
        self.effects != NoteEffects::default()
    }

    pub fn set_semitone(&mut self, st: Option<u8>) {
        self.semitone = st.map(|st| st.clamp(0, NUM_SEMITONES - 1));
    }

    pub fn semitone(&self) -> Option<u8> {
        self.semitone
    }

    pub fn letter_from_semitone(semitone: u8) -> &'static str {
        match semitone % 12 {
            0 | 1 => "C",
            2 | 3 => "D",
            4 => "E",
            5 | 6 => "F",
            7 | 8 => "G",
            9 | 10 => "A",
            11 => "B",
            _ => "N/A",
        }
    }

    pub fn sharp_from_semitone(semitone: u8) -> bool {
        matches!(semitone % 12, 1 | 3 | 6 | 8 | 10)
    }

    pub fn octave_from_semitone(semitone: u8) -> u8 {
        semitone / 12
    }
}
