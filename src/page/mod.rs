use std::collections::HashMap;

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
    fn draw_side_buttons(&mut self, _ui: &mut Ui, _state: &mut AppUIState, _project: &Project) {}
    fn handle_undo(&mut self, _project: &Project) {}
    fn handle_redo(&mut self, _project: &Project) {}
    fn play(&self, _state: &AppUIState, _project: ROProject) {}
    fn play_global(&self, state: &AppUIState, project: ROProject) {
        self.play(state, project);
    }
    fn heading(&self, _state: &AppUIState) -> String;
}

pub struct Pages {
    pages: HashMap<PageID, Box<dyn Page>>,
}

impl Pages {
    pub fn new() -> Self {
        macro_rules! page {
            ($pid:expr,$page:ident) => {
                ($pid, Box::<$page>::default() as Box<dyn Page>)
            };
        }

        let pages = vec![
            page!(PageID::Track, TrackUI),
            page!(PageID::Chain, ChainUI),
            page!(PageID::Phrase, PhraseUI),
            page!(PageID::Instrument, InstrumentUI),
            page!(PageID::Preferences, PreferencesUI),
        ]
        .into_iter()
        .collect();

        Self { pages }
    }

    pub fn page_mut(&mut self, page_id: PageID) -> &mut dyn Page {
        self.pages
            .get_mut(&page_id)
            .expect("Page not defined!")
            .as_mut()
    }

    pub fn page(&self, page_id: PageID) -> &dyn Page {
        self.pages
            .get(&page_id)
            .expect("Page not defined!")
            .as_ref()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PageID {
    #[default]
    Track,
    Chain,
    Phrase,
    Instrument,
    Preferences,
}
