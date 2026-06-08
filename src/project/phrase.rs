use super::note::Note;
use serde::{Deserialize, Serialize};

pub const VOICES_PER_TRACK: usize = 8;
pub const ROWS_PER_PHRASE: usize = 16;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Phrase {
    pub voices: [PhraseVoice; VOICES_PER_TRACK],
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhraseVoice {
    pub notes: [Note; ROWS_PER_PHRASE],
}
