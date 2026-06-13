use serde::{Deserialize, Serialize};

use crate::project::Semitone;

use super::ADSREnvelope;

macro_rules! add_from_other {
    ($self:ident, $other:ident, $( $effect:ident ),*) => {
        $(
            if $other.$effect.is_some() {
                $self.$effect = $other.$effect.clone();
            }
        )*
    };
}

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
    /// adds effects from another `NoteEffects` struct
    pub fn add_from_other(&mut self, other: &NoteEffects) {
        add_from_other!(
            self, other, vibrato, kill, pitch_bend, slide, envelope, pan, soft_kill
        );
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
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
    pub start_semitone: Option<Semitone>,
    /// attached when effect is added
    pub end_semitone: Option<Semitone>,
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
