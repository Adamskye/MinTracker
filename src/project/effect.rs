use serde::{Deserialize, Serialize};

use super::ADSREnvelope;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteEffects {
    #[serde(default)]
    pub vibrato: Option<VibratoEffect>,
    #[serde(default)]
    pub kill: Option<KillEffect>,
    #[serde(default)]
    pub pitch_bend: Option<PitchBendEffect>,
    #[serde(default)]
    pub slide: Option<SlideEffect>,
    #[serde(default)]
    pub envelope: Option<EnvelopeEffect>,
    #[serde(default)]
    pub pan: Option<PanEffect>,
    #[serde(default)]
    pub soft_kill: Option<SoftKillEffect>,
    // remember to implement add_from_other when adding new effects
}

impl NoteEffects {
    /// adds effects from another NoteEffects struct
    pub fn add_from_other(&mut self, other: &NoteEffects) {
        macro_rules! add_effect {
            ($effect:ident) => {
                if other.$effect.is_some() {
                    self.$effect = other.$effect.clone();
                }
            };
        }
        add_effect!(vibrato);
        add_effect!(kill);
        add_effect!(pitch_bend);
        add_effect!(slide);
        add_effect!(envelope);
        add_effect!(pan);
        add_effect!(soft_kill);
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct EffectPreset {
    pub name: String,
    pub effects: NoteEffects,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VibratoEffect {
    /// in semitones
    pub amplitude: f32,
    /// milliseconds per cycle
    pub speed: u32,
}

impl Default for VibratoEffect {
    fn default() -> Self {
        Self {
            amplitude: 0.2,
            speed: 200,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct KillEffect {
    pub delay_ticks: f32,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct PitchBendEffect {
    pub semitones_per_tick: f32,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlideEffect {
    /// attached when effect is added
    pub start_semitone: Option<u8>,
    /// attached when effect is added
    pub end_semitone: Option<u8>,
    pub time_ticks: f32,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeEffect {
    pub envelope: ADSREnvelope,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanEffect {
    pub value: f32,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoftKillEffect {
    pub delay_ticks: f32,
}
