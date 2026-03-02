use egui::Ui;

use crate::{
    app_ui_state::AppUIState,
    page::{
        chain::ChainUI, instrument::InstrumentUI, phrase::PhraseUI, preferences::PreferencesUI,
        track::TrackUI,
    },
    project::Project,
    synth::ROProject,
};

mod chain;
mod instrument;
mod phrase;
mod preferences;
mod track;

pub trait Page {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project);
    fn draw_side_buttons(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project);
    fn handle_undo(&mut self, project: &Project);
    fn play(&self, _state: &AppUIState, _project: ROProject) {}
    fn heading(&self, _state: &AppUIState) -> String;
}

pub struct Pages {
    track: Box<dyn Page>,
    chain: Box<dyn Page>,
    phrase: Box<dyn Page>,
    instrument: Box<dyn Page>,
    preferences: Box<dyn Page>,
}

impl Pages {
    pub fn new() -> Self {
        Self {
            track: Box::<TrackUI>::default(),
            chain: Box::<ChainUI>::default(),
            phrase: Box::<PhraseUI>::default(),
            instrument: Box::<InstrumentUI>::default(),
            preferences: Box::<PreferencesUI>::default(),
        }
    }

    pub fn page_mut(&mut self, page_id: PageID) -> &mut dyn Page {
        match page_id {
            PageID::Track => &mut *self.track,
            PageID::Chain => &mut *self.chain,
            PageID::Phrase => &mut *self.phrase,
            PageID::Instrument => &mut *self.instrument,
            PageID::Preferences => &mut *self.preferences,
        }
    }

    pub fn page(&self, page_id: PageID) -> &dyn Page {
        match page_id {
            PageID::Track => &*self.track,
            PageID::Chain => &*self.chain,
            PageID::Phrase => &*self.phrase,
            PageID::Instrument => &*self.instrument,
            PageID::Preferences => &*self.preferences,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum PageID {
    #[default]
    Track,
    Chain,
    Phrase,
    Instrument,
    Preferences,
}
