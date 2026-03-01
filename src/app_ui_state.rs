use std::path::PathBuf;

use crate::{app_preferences::AppPreferences, synth::Player};

#[derive(Clone, Copy, Default, PartialEq)]
pub enum PageID {
    #[default]
    Track,
    Chain,
    Phrase,
    Instrument,
    Preferences,
}

impl PageID {
    pub fn str(&self) -> &str {
        match self {
            PageID::Track => "Project",
            PageID::Chain => "Chain",
            PageID::Phrase => "Phrase",
            PageID::Instrument => "Instrument",
            PageID::Preferences => "Preferences",
        }
    }
}

#[derive(Default)]
pub struct AppUIState {
    pub current_page: PageID,
    pub filepath: Option<PathBuf>,
    pub player: Player,
    pub project_dirty: bool,

    pub viewed_track: Option<usize>,
    pub viewed_chain: Option<u32>,
    pub viewed_phrase: Option<u32>,
    pub viewed_instrument: Option<u32>,

    pub track_selected_row: Option<usize>,
    pub chain_selected_row: Option<usize>,

    app_preferences: AppPreferences,
}

impl AppUIState {
    pub fn new() -> Self {
        Self {
            app_preferences: AppPreferences::load(),
            ..Default::default()
        }
    }

    pub fn app_preferences(&self) -> &AppPreferences {
        &self.app_preferences
    }

    pub fn modify_preferences(&mut self, f: impl FnOnce(&mut AppPreferences)) {
        f(&mut self.app_preferences);
        confy::store("mintracker", "config", &self.app_preferences).unwrap();
    }
}
