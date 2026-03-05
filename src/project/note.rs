use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use crate::helpers;

    use super::*;

    #[test]
    fn frequency() {
        let mut note = Note::default();
        macro_rules! freq {
            ($note:ident) => {
                helpers::frequency_from_semitone($note.semitone().unwrap() as f32)
            };
        }

        note.set_semitone(Some(57));
        assert!((freq!(note) - 440.0).abs() < 0.1);

        note.set_semitone(Some(0));
        assert!((freq!(note) - 16.35).abs() < 0.1);

        note.set_semitone(Some(69));
        assert!((freq!(note) - 880.0).abs() < 0.1);

        note.set_semitone(Some(27));
        assert!((freq!(note) - 77.78).abs() < 0.1);

        note.set_semitone(Some(107));
        assert!((freq!(note) - 7902.13).abs() < 0.1);
    }
}
