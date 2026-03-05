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

#[derive(Clone)]
pub enum ProjectEvent {
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
    UpdateChain {
        id: u32,
        new_chain: Box<Chain>,
    },
    UpdatePhrase {
        id: u32,
        new_phrase: Box<Phrase>,
    },
    UpdateInstrument {
        id: u32,
        new_instrument: Option<Box<Instrument>>,
    },
    UpdateEffectPreset {
        id: u32,
        new_preset: Option<Box<EffectPreset>>,
    },
    UpdateSettings(ProjectSettings),
    CleanUnusedNotes,
    Undo,
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
    event: Arc<Mutex<LinkedList<ProjectEvent>>>,

    #[serde(skip)]
    timestamp_last_update: ProjectTimestamp,

    #[serde(skip)]
    reverse_undo_stack: Vec<ProjectEvent>,
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

            event: Default::default(),
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

    /// Returns event to undo the given event, if applicable.
    fn handle_event(
        &mut self,
        event: ProjectEvent,
        log_messages: &mut Vec<String>,
    ) -> Option<ProjectEvent> {
        use ProjectEvent as PE;
        self.timestamp_last_update = ProjectTimestamp::now();
        match event {
            PE::UpdateTrack { index, new_track } => self.update_track(index, new_track),
            PE::UpdateTrackSettings {
                index,
                new_settings,
            } => self.update_track_settings(index, new_settings),
            PE::UpdateTrackCell {
                track_index,
                chain_offset,
                new_chain_id,
            } => self.update_track_cell(track_index, chain_offset, new_chain_id),
            PE::UpdateChain { id, new_chain } => self.update_chain(id, *new_chain),
            PE::UpdatePhrase { id, new_phrase } => self.update_phrase(id, *new_phrase),
            PE::UpdateInstrument { id, new_instrument } => {
                self.update_instrument(id, new_instrument)
            }
            PE::UpdateSettings(settings) => self.update_settings(settings),
            PE::UpdateEffectPreset { id, new_preset } => self.update_effect_preset(id, new_preset),
            PE::CleanUnusedNotes => {
                self.clean_unused_notes(log_messages);
                None
            }
            PE::Undo => {
                if let Some(undo_event) = self.reverse_undo_stack.pop() {
                    self.handle_event(undo_event, log_messages);
                } else {
                    log_messages.push("Nothing to undo".to_string());
                }
                None
            }
        }
    }

    pub fn handle_events(&mut self) -> (bool, Vec<String>) {
        let mut changed = false;
        let mut log_messages = Vec::new();

        while let Some(event) = {
            let mut ev = self.event.lock().unwrap();
            ev.pop_front()
        } {
            changed = true;
            if let Some(undo_event) = self.handle_event(event, &mut log_messages) {
                self.reverse_undo_stack.push(undo_event);
            }
        }

        (changed, log_messages)
    }

    pub fn push_event(&self, event: ProjectEvent) {
        self.event.lock().unwrap().push_back(event);
    }

    fn update_settings(&mut self, new_settings: ProjectSettings) -> Option<ProjectEvent> {
        let undo = Some(ProjectEvent::UpdateSettings(self.settings.clone()));
        self.settings = new_settings;
        undo
    }

    fn update_track_settings(
        &mut self,
        index: usize,
        new_settings: TrackSettings,
    ) -> Option<ProjectEvent> {
        let track = self.tracks.get_mut(index)?;
        let undo = Some(ProjectEvent::UpdateTrackSettings {
            index,
            new_settings: track.settings.clone(),
        });
        track.settings = new_settings;
        undo
    }

    fn update_track_cell(
        &mut self,
        track_index: usize,
        chain_offset: usize,
        new_chain_id: Option<u32>,
    ) -> Option<ProjectEvent> {
        let track = self.tracks.get_mut(track_index)?;
        if chain_offset >= track.chains.len() {
            return None;
        }

        let undo = Some(ProjectEvent::UpdateTrackCell {
            track_index,
            chain_offset,
            new_chain_id: track.chains[chain_offset],
        });
        track.chains[chain_offset] = new_chain_id;
        undo
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

    pub fn update_track(
        &mut self,
        index: usize,
        new_track: Option<Box<Track>>,
    ) -> Option<ProjectEvent> {
        let Some(new_track) = new_track else {
            // removing track
            if index < self.tracks.len() {
                let old_track = self.tracks[index].clone();
                self.tracks.remove(index);

                // undo event
                return Some(ProjectEvent::UpdateTrack {
                    index,
                    new_track: Some(Box::new(old_track)),
                });
            } else {
                // can't do anything since new_track does not exist and index is out of bounds
                return None;
            }
        };

        match self.tracks.get_mut(index) {
            Some(track) => {
                let undo = Some(ProjectEvent::UpdateTrack {
                    index,
                    new_track: Some(Box::new(track.clone())), // undo event
                });
                *track = *new_track;
                undo
            }
            None => {
                self.tracks.push(*new_track);
                Some(ProjectEvent::UpdateTrack {
                    index: self.tracks.len() - 1,
                    new_track: None,
                })
            }
        }
    }

    pub fn update_chain(&mut self, id: u32, new_chain: Chain) -> Option<ProjectEvent> {
        let undo = self
            .chains
            .get(&id)
            .cloned()
            .map(|old_chain| ProjectEvent::UpdateChain {
                id,
                new_chain: Box::new(old_chain),
            });
        self.chains.insert(id, new_chain);
        undo
    }

    pub fn update_phrase(&mut self, id: u32, new_phrase: Phrase) -> Option<ProjectEvent> {
        let undo = self
            .phrases
            .get(&id)
            .cloned()
            .map(|old_phrase| ProjectEvent::UpdatePhrase {
                id,
                new_phrase: Box::new(old_phrase),
            });
        self.phrases.insert(id, new_phrase);
        undo
    }

    pub fn update_instrument(
        &mut self,
        id: u32,
        instrument: Option<Box<Instrument>>,
    ) -> Option<ProjectEvent> {
        let undo = self.instruments.get(&id).cloned().map(|old_instrument| {
            ProjectEvent::UpdateInstrument {
                id,
                new_instrument: Some(Box::new(old_instrument)),
            }
        });

        match instrument {
            Some(instrument) => {
                self.instruments.insert(id, *instrument);
            }
            None => {
                self.instruments.remove(&id);
            }
        }

        undo
    }

    pub fn update_effect_preset(
        &mut self,
        id: u32,
        effect_preset: Option<Box<EffectPreset>>,
    ) -> Option<ProjectEvent> {
        let undo = self
            .effect_presets
            .get(&id)
            .cloned()
            .map(|old_effect_preset| ProjectEvent::UpdateEffectPreset {
                id,
                new_preset: Some(Box::new(old_effect_preset)),
            });
        match effect_preset {
            Some(effect_preset) => self.effect_presets.insert(id, *effect_preset),
            None => self.effect_presets.remove(&id),
        };
        undo
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
