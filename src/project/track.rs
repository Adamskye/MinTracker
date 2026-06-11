use serde::{Deserialize, Serialize};

pub const CHAINS_PER_TRACK: usize = 50;

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct TrackSettings {
    pub instrument: Option<u32>,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    #[serde(default)]
    pub transpose_semitones: i32,
}

impl Default for TrackSettings {
    fn default() -> Self {
        Self {
            instrument: None,
            volume: 1.0,
            pan: 0.0,
            muted: false,
            transpose_semitones: 0,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct Track {
    pub chains: Vec<Option<u32>>,
    pub settings: TrackSettings,
}

impl Default for Track {
    fn default() -> Self {
        Self {
            chains: vec![None; CHAINS_PER_TRACK],
            settings: Default::default(),
        }
    }
}
