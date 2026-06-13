use std::fmt::Display;

use serde::{Deserialize, Serialize};

use super::NoteEffects;

pub const NUM_SEMITONES: u8 = 108;
pub const MID_A_SEMITONE: u8 = 57;
pub const MID_A_FREQUENCY: f32 = 440.0;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Semitone<T = u8>(T);

impl Default for Semitone {
    fn default() -> Self {
        Self::from(MID_A_SEMITONE)
    }
}

impl Display for Semitone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.letter(),
            if self.is_sharp() { "#" } else { "" },
            self.octave()
        )
    }
}

impl From<u8> for Semitone<u8> {
    fn from(value: u8) -> Self {
        Self(value.clamp(0, NUM_SEMITONES - 1))
    }
}

impl From<f32> for Semitone<f32> {
    fn from(value: f32) -> Self {
        Self(value.clamp(0.0, f32::from(NUM_SEMITONES - 1)))
    }
}

impl From<Semitone> for usize {
    fn from(value: Semitone) -> Self {
        value.0 as usize
    }
}

impl Semitone {
    pub fn from_frequency(frequency: f32) -> Self {
        Self::from((12.0 * f32::log2(frequency / MID_A_FREQUENCY) + 57.0) as u8)
    }

    pub fn transposed_by(&self, amount: i32) -> Semitone {
        Semitone::from((i32::from(self.0).saturating_add(amount)) as u8)
    }
}

impl<T> Semitone<T>
where
    T: Clone + Copy + Into<f32>,
{
    pub fn value(&self) -> T {
        self.0
    }

    pub fn frequency(&self) -> f32 {
        let semitone = self.0.into();
        let diff = semitone - 57.0;
        let ratio = 2.0_f32.powf(1.0 / 12.0);
        MID_A_FREQUENCY * ratio.powf(diff)
    }
}

impl Semitone<u8> {
    pub fn letter(&self) -> &'static str {
        match self.0 % 12 {
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

    pub fn is_sharp(&self) -> bool {
        matches!(self.0 % 12, 1 | 3 | 6 | 8 | 10)
    }

    pub fn octave(&self) -> u8 {
        self.0 / 12
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub semitone: Option<Semitone>,
    #[serde(default)]
    pub effects: NoteEffects,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency() {
        let mut note = Note::default();
        macro_rules! freq {
            ($note:ident) => {
                $note.semitone.unwrap().frequency()
            };
        }

        note.semitone = Some(57.into());
        assert!((freq!(note) - 440.0).abs() < 0.1);

        note.semitone = Some(0.into());
        assert!((freq!(note) - 16.35).abs() < 0.1);

        note.semitone = Some(69.into());
        assert!((freq!(note) - 880.0).abs() < 0.1);

        note.semitone = Some(27.into());
        assert!((freq!(note) - 77.78).abs() < 0.1);

        note.semitone = Some(107.into());
        assert!((freq!(note) - 7902.13).abs() < 0.1);
    }
}
