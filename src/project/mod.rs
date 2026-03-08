pub mod chain;
pub use chain::*;

pub mod instrument;
pub use instrument::*;

pub mod note;
pub use note::*;

pub mod phrase;
pub use phrase::*;

pub mod track;
pub use track::*;

pub mod effect;
pub use effect::*;

use std::collections::{BTreeMap, LinkedList};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

#[derive(Default, Copy, Clone, PartialEq)]
pub struct ProjectLocation {
    pub track_idx: usize,
    pub chain_offset: usize,
    pub phrase_offset: usize,
    pub note_offset: usize,
}

pub trait Cmd: Send + Sync {
    /// Returns a command that can undo this command, if applicable.
    fn apply(&self, project: &mut Project) -> Option<Box<dyn Cmd>>;
}

#[derive(Clone)]
pub enum ChainCmd {
    Update {
        id: u32,
        new_chain: Chain,
    },
    UpdatePhrase {
        id: u32,
        row_index: usize,
        new_phrase_id: Option<u32>,
    },
    UpdateTranspose {
        id: u32,
        row_index: usize,
        new_transpose: f32,
    },
}

impl Cmd for ChainCmd {
    fn apply(&self, project: &mut Project) -> Option<Box<dyn Cmd>> {
        match self {
            Self::Update { id, new_chain } => {
                let undo = if let Some(new_chain) = project.chains.get(id).cloned() {
                    Some(Box::new(ChainCmd::Update { id: *id, new_chain }) as Box<dyn Cmd>)
                } else {
                    None
                };

                project.chains.insert(*id, new_chain.clone());
                undo
            }
            Self::UpdatePhrase {
                id,
                row_index,
                new_phrase_id,
            } => {
                let row = project.chains.get_mut(id)?.rows.get_mut(*row_index)?;
                let undo = Some(Box::new(ChainCmd::UpdatePhrase {
                    id: *id,
                    row_index: *row_index,
                    new_phrase_id: row.phrase,
                }) as Box<dyn Cmd>);

                row.phrase = *new_phrase_id;
                undo
            }
            Self::UpdateTranspose {
                id,
                row_index,
                new_transpose,
            } => {
                let row = project.chains.get_mut(id)?.rows.get_mut(*row_index)?;
                let undo = Some(Box::new(ChainCmd::UpdateTranspose {
                    id: *id,
                    row_index: *row_index,
                    new_transpose: row.transpose,
                }) as Box<dyn Cmd>);

                row.transpose = *new_transpose;
                undo
            }
        }
    }
}

#[derive(Clone)]
pub enum PhraseCmd {
    Update {
        id: u32,
        new_phrase: Phrase,
    },
    UpdateNote {
        id: u32,
        voice_index: usize,
        note_index: usize,
        new_note: Note,
    },
}

impl Cmd for PhraseCmd {
    fn apply(&self, project: &mut Project) -> Option<Box<dyn Cmd>> {
        match self {
            Self::Update { id, new_phrase } => {
                let phrase = project.phrases.get_mut(id)?;
                let undo = Some(Box::new(PhraseCmd::Update {
                    id: *id,
                    new_phrase: phrase.clone(),
                }) as Box<dyn Cmd>);

                *phrase = new_phrase.clone();
                undo
            }
            Self::UpdateNote {
                id,
                voice_index,
                note_index,
                new_note,
            } => {
                let voice = project.phrases.get_mut(id)?.voices.get_mut(*voice_index)?;
                let current_note = voice.notes.get(*note_index)?.clone();
                let note = voice.notes.get_mut(*note_index)?;
                let undo = Some(Box::new(PhraseCmd::UpdateNote {
                    id: *id,
                    voice_index: *voice_index,
                    note_index: *note_index,
                    new_note: current_note,
                }) as Box<dyn Cmd>);

                *note = new_note.clone();
                undo
            }
        }
    }
}

#[derive(Clone)]
pub enum InstrumentCmd {
    Update {
        id: u32,
        new_instrument: Option<Box<Instrument>>,
    },
}

impl Cmd for InstrumentCmd {
    fn apply(&self, project: &mut Project) -> Option<Box<dyn Cmd>> {
        match self {
            Self::Update { id, new_instrument } => {
                let undo = project.instruments.get(id).cloned().map(|old_instrument| {
                    Box::new(InstrumentCmd::Update {
                        id: *id,
                        new_instrument: Some(Box::new(old_instrument)),
                    }) as Box<dyn Cmd>
                });

                match new_instrument {
                    Some(instrument) => project.instruments.insert(*id, *instrument.clone()),
                    None => project.instruments.remove(id),
                };

                undo
            }
        }
    }
}

#[derive(Clone)]
pub enum EffectPresetCmd {
    Update {
        id: u32,
        new_preset: Option<Box<EffectPreset>>,
    },
}

impl Cmd for EffectPresetCmd {
    fn apply(&self, project: &mut Project) -> Option<Box<dyn Cmd>> {
        match self {
            Self::Update { id, new_preset } => {
                let undo = project.effect_presets.get(id).cloned().map(|old_preset| {
                    Box::new(EffectPresetCmd::Update {
                        id: *id,
                        new_preset: Some(Box::new(old_preset)),
                    }) as Box<dyn Cmd>
                });

                match new_preset {
                    Some(preset) => project.effect_presets.insert(*id, *preset.clone()),
                    None => project.effect_presets.remove(id),
                };

                undo
            }
        }
    }
}

#[derive(Clone)]
pub enum TracksCmd {
    Update {
        new_tracks: Vec<Track>,
    },
    UpdateTrack {
        index: usize,
        new_track: Option<Box<Track>>,
    },
    UpdateTrackSettings {
        index: usize,
        new_settings: TrackSettings,
    },
    UpdateTrackCell {
        track_index: usize,
        chain_offset: usize,
        new_chain_id: Option<u32>,
    },
}

impl Cmd for TracksCmd {
    fn apply(&self, project: &mut Project) -> Option<Box<dyn Cmd>> {
        match self {
            Self::Update { new_tracks } => {
                let undo = Some(Box::new(TracksCmd::Update {
                    new_tracks: project.tracks.clone(),
                }) as Box<dyn Cmd>);
                project.tracks = new_tracks.clone();
                undo
            }
            Self::UpdateTrack { index, new_track } => {
                let undo = if let Some(track) = project.tracks.get(*index) {
                    Some(Box::new(TracksCmd::UpdateTrack {
                        index: *index,
                        new_track: Some(Box::new(track.clone())),
                    }) as Box<dyn Cmd>)
                } else {
                    None
                };

                match project.tracks.get_mut(*index) {
                    Some(track) => {
                        if let Some(new_track) = new_track {
                            // update track
                            *track = *new_track.clone();
                        } else {
                            // remove track
                            project.tracks.remove(*index);
                        }
                    }
                    None => {
                        // add a track (only if new_track is Some, otherwise it's a no-op)
                        project.tracks.push(*new_track.clone()?);
                    }
                }

                undo
            }
            Self::UpdateTrackSettings {
                index,
                new_settings,
            } => {
                let track = project.tracks.get_mut(*index)?;
                let undo = Some(Box::new(TracksCmd::UpdateTrackSettings {
                    index: *index,
                    new_settings: track.settings.clone(),
                }) as Box<dyn Cmd>);
                track.settings = new_settings.clone();
                undo
            }
            Self::UpdateTrackCell {
                track_index,
                chain_offset,
                new_chain_id,
            } => {
                let track = project.tracks.get_mut(*track_index)?;
                if *chain_offset >= track.chains.len() {
                    return None;
                }

                let undo = Some(Box::new(TracksCmd::UpdateTrackCell {
                    track_index: *track_index,
                    chain_offset: *chain_offset,
                    new_chain_id: track.chains[*chain_offset],
                }) as Box<dyn Cmd>);

                track.chains[*chain_offset] = *new_chain_id;
                undo
            }
        }
    }
}

#[derive(Clone)]
pub enum ProjectCmd {
    UpdateSettings(ProjectSettings),
    CleanUnusedNotes,
    Undo,
}

impl Cmd for ProjectCmd {
    fn apply(&self, project: &mut Project) -> Option<Box<dyn Cmd>> {
        match self {
            Self::UpdateSettings(new_settings) => {
                let undo = Some(
                    Box::new(ProjectCmd::UpdateSettings(project.settings.clone())) as Box<dyn Cmd>,
                );
                project.settings = new_settings.clone();
                undo
            }
            Self::CleanUnusedNotes => {
                // delete undo history
                project.reverse_undo_stack.clear();

                let mut log_messages = Vec::new();
                project.clean_unused_notes(&mut log_messages);
                None
            }
            Self::Undo => {
                if let Some(undo_event) = project.reverse_undo_stack.pop() {
                    undo_event.apply(project);
                    //project.handle_event(undo_event, &mut Vec::new());
                }
                None
            }
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub tempo: f32,
    pub transpose: i8,

    #[serde(default)]
    pub loop_player: bool,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            tempo: 120.0,
            transpose: 0,
            loop_player: false,
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub struct ProjectTimestamp(std::time::Instant);

impl ProjectTimestamp {
    pub fn now() -> Self {
        Self(std::time::Instant::now())
    }
}

impl Default for ProjectTimestamp {
    fn default() -> Self {
        Self::now()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Project {
    tracks: Vec<Track>,
    chains: BTreeMap<u32, Chain>,
    phrases: BTreeMap<u32, Phrase>,
    instruments: BTreeMap<u32, Instrument>,
    settings: ProjectSettings,
    effect_presets: BTreeMap<u32, EffectPreset>,

    #[serde(skip)]
    cmds: Arc<Mutex<LinkedList<Arc<dyn Cmd>>>>,

    #[serde(skip)]
    timestamp_last_update: ProjectTimestamp,

    #[serde(skip)]
    reverse_undo_stack: Vec<Arc<dyn Cmd>>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            tracks: vec![Track::default(); 4],
            chains: BTreeMap::new(),
            phrases: BTreeMap::new(),
            instruments: BTreeMap::new(),
            settings: Default::default(),
            effect_presets: BTreeMap::default(),

            //event: Default::default(),
            cmds: Default::default(),
            timestamp_last_update: ProjectTimestamp::default(),
            reverse_undo_stack: Vec::new(),
        }
    }
}

impl Project {
    pub fn tracks(&self) -> &Vec<Track> {
        &self.tracks
    }

    pub fn chains(&self) -> &BTreeMap<u32, Chain> {
        &self.chains
    }

    pub fn phrases(&self) -> &BTreeMap<u32, Phrase> {
        &self.phrases
    }

    pub fn instruments(&self) -> &BTreeMap<u32, Instrument> {
        &self.instruments
    }

    pub fn settings(&self) -> &ProjectSettings {
        &self.settings
    }

    pub fn effect_presets(&self) -> &BTreeMap<u32, EffectPreset> {
        &self.effect_presets
    }

    pub fn timestamp_last_update(&self) -> ProjectTimestamp {
        self.timestamp_last_update
    }

    pub fn push_cmd(&self, cmd: impl Cmd + 'static) {
        self.cmds
            .lock()
            .unwrap()
            .push_back(Arc::new(cmd) as Arc<dyn Cmd>);
    }

    pub fn handle_cmds(&mut self) -> (bool, Vec<String>) {
        let mut changed = false;
        let log_messages = Vec::new();

        while let Some(cmd) = { self.cmds.lock().unwrap().pop_front() } {
            changed = true;
            if let Some(undo_cmd) = cmd.apply(self) {
                self.reverse_undo_stack.push(undo_cmd.into());
            }
        }

        self.timestamp_last_update = ProjectTimestamp::now();
        (changed, log_messages)
    }

    fn clean_unused_notes(&mut self, log_messages: &mut Vec<String>) {
        // delete undo history
        self.reverse_undo_stack.clear();

        // clean chains
        let mut chains_to_delete = Vec::new();
        for chain_id in self.chains().keys() {
            // if chain is ever used, move on
            if self
                .tracks()
                .iter()
                .flat_map(|track| &track.chains)
                .any(|it| it.as_ref() == Some(chain_id))
            {
                continue;
            }

            chains_to_delete.push(*chain_id);
        }

        let num_chains = chains_to_delete.len();
        chains_to_delete.into_iter().for_each(|id| {
            self.chains.remove(&id);
        });

        // clean phrases
        let mut phrases_to_delete = Vec::new();
        for phrase_id in self.phrases().keys() {
            // if phrase is ever used, move on
            if self
                .chains()
                .values()
                .any(|chain| chain.rows.iter().any(|row| row.phrase == Some(*phrase_id)))
            {
                continue;
            }

            phrases_to_delete.push(*phrase_id);
        }

        let num_phrases = phrases_to_delete.len();
        phrases_to_delete.into_iter().for_each(|id| {
            self.phrases.remove(&id);
        });

        log_messages.push(format!(
            "Deleted {num_phrases} unused phrases and {num_chains} unused chains"
        ));
    }

    pub fn increment_project_location(&self, location: ProjectLocation) -> Option<ProjectLocation> {
        self.get_notes_at_location(location)?;
        let mut new_location = location;

        new_location.note_offset += 1;
        if self.get_notes_at_location(new_location).is_some() {
            return Some(new_location);
        }

        new_location.note_offset = 0;
        new_location.phrase_offset += 1;
        if self.get_notes_at_location(new_location).is_some() {
            return Some(new_location);
        }

        new_location.phrase_offset = 0;
        new_location.chain_offset += 1;
        if self.get_notes_at_location(new_location).is_some() {
            return Some(new_location);
        }

        None
    }

    pub fn get_notes_at_location(&self, location: ProjectLocation) -> Option<Arc<[&Note]>> {
        self.tracks()
            .get(location.track_idx)
            .and_then(|track| track.chains.get(location.chain_offset).cloned())
            .flatten()
            .and_then(|chain_id| self.chains().get(&chain_id))
            .and_then(|chain| chain.rows.get(location.phrase_offset).cloned())
            .and_then(|row| row.phrase)
            .and_then(|phrase_id| self.phrases().get(&phrase_id))
            .and_then(|phrase| {
                let notes = phrase
                    .voices
                    .iter()
                    .filter_map(|voice| voice.notes.get(location.note_offset))
                    .collect::<Vec<&Note>>();
                if notes.is_empty() {
                    return None;
                }
                Some(notes.into())
            })
    }

    pub fn get_unique_key<T>(map: &BTreeMap<u32, T>) -> u32 {
        for potential_key in 0.. {
            if !map.contains_key(&potential_key) {
                return potential_key;
            }
        }

        // backup: should never reach here
        match map.keys().last() {
            Some(last_key) => last_key + 1,
            None => 0,
        }
    }

    pub fn get_nth_unique_key<T>(n: usize, map: &BTreeMap<u32, T>) -> u32 {
        let mut countdown = n;

        for potential_key in 0.. {
            if !map.contains_key(&potential_key) {
                if countdown == 0 {
                    return potential_key;
                } else {
                    countdown -= 1;
                }
            }
        }

        // backup: should never reach here
        map.keys().last().unwrap_or(&0) + 1 + n as u32
    }

    pub fn get_note_transpose_semitones(&self, location: ProjectLocation) -> Option<f32> {
        // All the places that notes can be transposed:
        // - project-wide
        // - track
        // - a phrase can be transposed inside a chain
        // - an instrument??? (todo)

        let project_trans = self.settings().transpose as f32;
        let track = self
            .tracks()
            .get(location.track_idx)?
            .settings
            .transpose_semitones;
        let inside_chain = self
            .tracks()
            .get(location.track_idx)
            .and_then(|track| track.chains.get(location.chain_offset).cloned())
            .and_then(|chain_id_opt| chain_id_opt)
            .and_then(|chain_id| self.chains().get(&chain_id))
            .and_then(|chain| chain.rows.get(location.phrase_offset))
            .map(|row| row.transpose)
            .unwrap_or(0.0);

        Some(project_trans + track + inside_chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: deal with this later

    #[test]
    fn test_handle_event_update_tracks() {}

    #[test]
    fn test_handle_event_update_track() {}

    #[test]
    fn test_handle_event_update_track_settings() {}

    #[test]
    fn test_handle_event_update_track_cell() {}

    #[test]
    fn test_handle_event_update_chain() {}

    #[test]
    fn test_handle_event_update_phrase() {}

    #[test]
    fn test_handle_event_update_instrument() {}

    #[test]
    fn test_handle_event_update_effect_preset() {}

    #[test]
    fn test_handle_event_update_settings() {}

    #[test]
    fn test_handle_event_clean_unused_notes() {}

    #[test]
    fn test_handle_event_undo() {}
}
