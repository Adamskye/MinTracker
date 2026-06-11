use serde::{Deserialize, Serialize};

pub const ROWS_PER_CHAIN: usize = 16;

#[derive(Default, Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct ChainRow {
    pub phrase: Option<u32>,
    pub transpose: i32,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct Chain {
    #[serde(default)]
    pub rows: Vec<ChainRow>,
}

impl Default for Chain {
    fn default() -> Self {
        Self {
            rows: vec![ChainRow::default(); ROWS_PER_CHAIN],
        }
    }
}
