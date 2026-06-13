use std::collections::HashMap;

use crate::project::{Note, Project, ProjectLocation, ROWS_PER_PHRASE, VOICES_PER_TRACK, phrase};

// TODO: error checking with NoteLocation
#[derive(Clone)]
pub struct NoteLocation {
    pub track_idx: usize,
    pub row_in_track: usize,
    pub row_in_chain: usize,
    pub voice: usize,
    pub row_in_voice: usize,
}

pub struct PlayScope(Box<dyn PlayScopeType>);

impl PlayScope {
    pub fn new(scope_type: Box<dyn PlayScopeType>) -> Self {
        Self(scope_type)
    }

    pub fn get_notes(&self, project: &Project) -> Vec<NoteLocation> {
        self.0.get_notes(project).unwrap_or_else(Vec::new)
    }

    pub fn increment(mut self, project: &Project) -> Option<PlayScope> {
        match self.0.increment(project) {
            IncrementState::Continued => Some(self),
            IncrementState::Finished => None,
        }
    }
}

impl Clone for PlayScope {
    fn clone(&self) -> Self {
        Self(self.0.box_clone())
    }
}

enum IncrementState {
    /// Continued successfully
    Continued,
    /// Could not increment because end of scope was reached
    Finished,
}

/// This is needed because if a note is played, and then another note is played on the same voice
/// and same track, then the previous note should be released
#[derive(PartialEq, Eq, Hash)]
pub struct NotePlayInfo {
    pub track_idx: usize,
    pub voice_idx: usize,
}

pub trait PlayScopeType: Send + Sync {
    fn get_notes(&self, project: &Project) -> Option<Vec<NoteLocation>>;
    fn increment(&mut self, project: &Project) -> IncrementState;
    fn box_clone(&self) -> Box<dyn PlayScopeType>;
}

#[derive(Clone)]
pub struct PhraseScope {
    location: NoteLocation,
}

impl PhraseScope {
    pub fn new(_project: &Project, start_location: NoteLocation) -> Option<Self> {
        Some(Self {
            location: start_location,
        })
    }
}

impl PlayScopeType for PhraseScope {
    fn get_notes(&self, _project: &Project) -> Option<Vec<NoteLocation>> {
        let notes = (0..VOICES_PER_TRACK)
            .map(|voice_idx| NoteLocation {
                voice: voice_idx,
                ..self.location
            })
            .collect::<Vec<_>>();
        (!notes.is_empty()).then_some(notes)
    }

    fn increment(&mut self, _: &Project) -> IncrementState {
        self.location.row_in_voice += 1;
        if self.location.row_in_voice >= ROWS_PER_PHRASE {
            self.location.row_in_voice -= 1;
            IncrementState::Finished
        } else {
            IncrementState::Continued
        }
    }

    fn box_clone(&self) -> Box<dyn PlayScopeType> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
pub struct ChainScope {
    location: NoteLocation,
    // track_idx: usize,
    // chain_id: u32,
    // row: usize,
    // pos_in_phrase: PhraseScope,
}

impl ChainScope {
    pub fn new(_project: &Project, location: NoteLocation) -> Option<Self> {
        Some(Self { location })
    }
}

impl PlayScopeType for ChainScope {
    fn get_notes(&self, project: &Project) -> Option<Vec<NoteLocation>> {
        let phrase = PhraseScope::new(project, self.location.clone())?;
        phrase.get_notes(project)
    }

    fn increment(&mut self, project: &Project) -> IncrementState {
        let phrase = PhraseScope::new(project, self.location.clone());

        // increment position in phrase if possible, otherwise move onto next phrase
        match self.pos_in_phrase.increment(project) {
            IncrementState::Continued => IncrementState::Continued,
            IncrementState::Finished => {
                match Self::new(project, self.track_idx, self.chain_id, self.row + 1) {
                    Some(new_self) => {
                        *self = new_self;
                        IncrementState::Continued
                    }
                    None => IncrementState::Finished,
                }
            }
        }
    }

    fn box_clone(&self) -> Box<dyn PlayScopeType> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
pub struct TrackScope {
    track_idx: usize,
    row: usize,
    pos_in_chain: ChainScope,
}

impl TrackScope {
    pub fn new(
        project: &Project,
        track_idx: usize,
        start_row: usize,
        start_row_in_chain: usize,
    ) -> Option<Self> {
        let track = project.tracks().get(track_idx)?;
        let chain_id = (*track.chains.get(start_row)?)?;
        Some(Self {
            track_idx,
            row: start_row,
            pos_in_chain: ChainScope::new(project, track_idx, chain_id, start_row_in_chain)?,
        })
    }
}

impl PlayScopeType for TrackScope {
    fn get_notes(&self, project: &Project) -> Option<Vec<NoteLocation>> {
        self.pos_in_chain.get_notes(project)
    }

    fn increment(&mut self, project: &Project) -> IncrementState {
        // increment position in chain if possible, otherwise move onto next chain
        match self.pos_in_chain.increment(project) {
            IncrementState::Continued => IncrementState::Continued,
            IncrementState::Finished => match Self::new(project, self.track_idx, self.row + 1, 0) {
                Some(new_self) => {
                    *self = new_self;
                    IncrementState::Continued
                }
                None => IncrementState::Finished,
            },
        }
    }

    fn box_clone(&self) -> Box<dyn PlayScopeType> {
        Box::new(self.clone())
    }
}

pub struct MultiScope {
    scopes: Vec<Box<dyn PlayScopeType>>,
}

impl MultiScope {
    pub fn new(scopes: Vec<Box<dyn PlayScopeType>>) -> Self {
        Self { scopes }
    }
}

impl PlayScopeType for MultiScope {
    fn get_notes(&self, project: &Project) -> Option<Vec<NoteLocation>> {
        let mut notes: Vec<NoteLocation> = Vec::default();
        for new_notes in self.scopes.iter().filter_map(|s| s.get_notes(project)) {
            notes.extend(new_notes);
        }
        (!notes.is_empty()).then_some(notes.into())
    }

    fn increment(&mut self, project: &Project) -> IncrementState
    where
        Self: Sized,
    {
        self.scopes
            .retain_mut(|scope| matches!(scope.increment(project), IncrementState::Continued));
        match self.scopes.is_empty() {
            true => IncrementState::Finished,
            false => IncrementState::Continued,
        }
    }

    fn box_clone(&self) -> Box<dyn PlayScopeType> {
        Box::new(MultiScope {
            scopes: self.scopes.iter().map(|s| s.box_clone()).collect(),
        })
    }
}
