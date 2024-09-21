use std::{
    ops::{Deref, Div, Mul, MulAssign},
    sync::Arc,
};

use super::NUM_SEMITONES;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

const DEFAULT_WAVETABLE_SIZE: usize = 64;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ADSREnvelope {
    #[serde(default)]
    pub volume: f32,

    pub attack_ms: u32,
    pub decay_ms: u32,
    pub sustain_vol: f32,
    pub release_ms: u32,
}

impl Default for ADSREnvelope {
    fn default() -> Self {
        Self {
            volume: 1.0,

            attack_ms: 0,
            decay_ms: 0,
            sustain_vol: 1.0,
            release_ms: 0,
        }
    }
}

impl ADSREnvelope {
    pub fn volume_at_time(&self, time_ms: f32) -> f32 {
        self.volume
            * if time_ms < self.attack_ms as f32 {
                // attack
                time_ms / self.attack_ms as f32
            } else if time_ms < self.decay_ms as f32 {
                // decay
                1.0 + (((self.sustain_vol - 1.0) / self.decay_ms as f32)
                    * (time_ms - self.attack_ms as f32))
            } else {
                // sustain
                self.sustain_vol
            }
    }

    pub fn volume_at_time_stopped(&self, time_ms: f32, time_when_stopped: f32) -> Option<f32> {
        // release
        let time_since_stopped = time_ms - time_when_stopped;
        let vol_when_stopped = self.volume_at_time(time_when_stopped);

        if self.release_ms == 0 {
            return None;
        }

        let gradient = -vol_when_stopped / self.release_ms as f32;
        let return_val = (vol_when_stopped + (gradient * time_since_stopped as f32)) * self.volume;

        if return_val <= 0.0 {
            None
        } else {
            Some(return_val)
        }
    }
}

#[derive(Default, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum InstrumentVariant {
    #[default]
    Normal,
    OneShot,
    OneShotPitched(f32),
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct SamplePoint(f32);
impl SamplePoint {
    pub fn value(&self) -> f32 {
        self.0
    }
}

impl Into<f32> for SamplePoint {
    fn into(self) -> f32 {
        self.0
    }
}

impl From<f32> for SamplePoint {
    fn from(value: f32) -> Self {
        Self(value.clamp(-1.0, 1.0))
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentDataTable {
    pub data: Vec<SamplePoint>,
    pub variant: InstrumentVariant,

    // todo: change this to just `envelope`
    #[serde(default)]
    pub new_envelope: ADSREnvelope,

    #[serde(default)]
    pub name: String,
}

impl Default for InstrumentDataTable {
    fn default() -> Self {
        Self {
            data: vec![0.0.into(); DEFAULT_WAVETABLE_SIZE],
            variant: Default::default(),
            new_envelope: Default::default(),
            name: String::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Instrument {
    pub name: String,
    pub data_tables: Vec<Arc<InstrumentDataTable>>,
    #[serde(with = "BigArray")]
    pub data_table_map: [Option<usize>; NUM_SEMITONES as usize],
}

impl Default for Instrument {
    fn default() -> Self {
        Self {
            data_tables: Default::default(),
            name: "Untitled Instrument".to_string(),
            data_table_map: [None; NUM_SEMITONES as usize],
        }
    }
}
