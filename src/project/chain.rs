use serde::{Deserialize, Serialize};

pub const PHRASES_PER_CHAIN: usize = 16;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Chain {
    pub phrases: Vec<Option<u32>>,
}

impl Default for Chain {
    fn default() -> Self {
        Self {
            phrases: vec![None; PHRASES_PER_CHAIN],
        }
    }
}
