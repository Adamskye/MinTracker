use super::note::Note;
use super::phrase::Phrase;
use super::{instrument::Instrument, track::Track};
use super::{Chain, EffectPreset};
use std::collections::{BTreeMap, LinkedList};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

// todo: get rid of copy
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

    // todo: allow UpdateInstrument to delete instruments like UpdateTrack can so a different event
    // type isn't needed
    DeleteInstrument(u32),
    UpdateSettings(ProjectSettings),
    CleanUnusedNotes,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub tempo: f32,
    pub transpose: i8,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            tempo: 120.0,
            transpose: 0,
        }
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

    pub fn handle_events(&mut self) -> bool {
        let mut changed = false;

        while let Some(event) = {
            let mut ev = self.event.lock().unwrap();
            ev.pop_front()
        } {
            changed = true;
            match event {
                ProjectEvent::UpdateTrack { index, new_track } => {
                    self.update_track(index, new_track);
                }
                ProjectEvent::UpdateChain {
                    id: chain_id,
                    new_chain,
                } => self.update_chain(chain_id, *new_chain),
                ProjectEvent::UpdatePhrase {
                    id: phrase_id,
                    new_phrase,
                } => {
                    if let Some(old_phrase) = self.phrases.get_mut(&phrase_id) {
                        *old_phrase = *new_phrase;
                    }
                }
                ProjectEvent::UpdateInstrument { id, new_instrument } => {
                    self.update_instrument(id, new_instrument)
                }
                ProjectEvent::UpdateSettings(settings) => self.settings = settings,
                ProjectEvent::UpdateEffectPreset { id, new_preset } => {
                    self.update_effect_preset(id, new_preset);
                }
                ProjectEvent::DeleteInstrument(id) => {
                    self.delete_instrument(id);
                }
                ProjectEvent::CleanUnusedNotes => {
                    self.clean_unused_notes();
                }
            }
        }

        changed
    }

    pub fn push_event(&self, event: ProjectEvent) {
        self.event.lock().unwrap().push_back(event);
    }

    fn clean_unused_notes(&mut self) {
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
        println!("Deleted {num_chains} chains");

        // clean phrases
        let mut phrases_to_delete = Vec::new();
        for phrase_id in self.phrases().keys() {
            // if phrase is ever used, move on
            if self
                .chains()
                .iter()
                .flat_map(|(_, chain)| &chain.phrases)
                .any(|it| it.as_ref() == Some(phrase_id))
            {
                continue;
            }

            phrases_to_delete.push(*phrase_id);
        }

        let num_phrases = phrases_to_delete.len();
        phrases_to_delete.into_iter().for_each(|id| {
            self.phrases.remove(&id);
        });
        println!("Deleted {num_phrases} phrases");
    }

    pub fn get_notes_at_location(&self, location: ProjectLocation) -> Option<Arc<[&Note]>> {
        self.tracks()
            .get(location.track_idx)
            .and_then(|track| track.chains.get(location.chain_offset).cloned())
            .flatten()
            .and_then(|chain_id| self.chains().get(&chain_id))
            .and_then(|chain| chain.phrases.get(location.phrase_offset).cloned())
            .flatten()
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

    pub fn update_track(&mut self, index: usize, new_track: Option<Box<Track>>) {
        let Some(new_track) = new_track else {
            if index < self.tracks.len() {
                self.tracks.remove(index);
            }
            return;
        };

        let track = match self.tracks.get_mut(index) {
            Some(t) => t,
            None => {
                self.tracks.push(Default::default());
                match self.tracks.last_mut() {
                    Some(t) => t,
                    None => return,
                }
            }
        };

        for new_chain in new_track.chains.iter().filter_map(|c| *c) {
            self.chains.entry(new_chain).or_default();
        }

        *track = *new_track;
    }

    pub fn update_chain(&mut self, chain_id: u32, new_chain: Chain) {
        let Some(chain) = self.chains.get_mut(&chain_id) else {
            return;
        };

        for new_id in new_chain.phrases.iter().filter_map(|p| *p) {
            self.phrases.entry(new_id).or_default();
        }

        *chain = new_chain;
    }

    pub fn update_instrument(&mut self, id: u32, instrument: Option<Box<Instrument>>) {
        match instrument {
            Some(instrument) => self.instruments.insert(id, *instrument),
            None => self.instruments.remove(&id),
        };
    }

    pub fn update_effect_preset(&mut self, id: u32, effect_preset: Option<Box<EffectPreset>>) {
        match effect_preset {
            Some(effect_preset) => self.effect_presets.insert(id, *effect_preset),
            None => self.effect_presets.remove(&id),
        };
    }

    pub fn delete_instrument(&mut self, id: u32) {
        self.instruments.remove(&id);
        self.tracks.iter_mut().for_each(|track| {
            if Some(id) == track.settings.instrument {
                track.settings.instrument = None;
            }
        });
    }

    pub fn get_unique_key<T>(map: &BTreeMap<u32, T>) -> u32 {
        for (potential_key, key) in (0..).zip(map.keys()) {
            if potential_key != *key {
                return potential_key;
            }
        }

        map.keys().last().unwrap_or(&0) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency() {
        let mut note = Note::default();

        note.set_semitone(Some(57));
        assert!((note.frequency() - 440.0).abs() < 0.1);

        note.set_semitone(Some(0));
        assert!((note.frequency() - 16.35).abs() < 0.1);

        note.set_semitone(Some(69));
        assert!((note.frequency() - 880.0).abs() < 0.1);

        note.set_semitone(Some(27));
        assert!((note.frequency() - 77.78).abs() < 0.1);

        note.set_semitone(Some(107));
        assert!((note.frequency() - 7902.13).abs() < 0.1);
    }
}
