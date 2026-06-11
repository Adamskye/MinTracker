use egui::{Color32, Ui};

use crate::{
    app_ui_state::AppUIState, helpers, project::Project, widget::cells::event::CellGridEvent,
};

pub trait CellData<G> {
    /// String to be displayed within cell.
    fn text(&self) -> Option<String>;

    fn text_color(&self, _project: &Project, state: &mut AppUIState) -> Color32 {
        helpers::to_colour32(state.preferences().style.colours.text)
    }

    /// Inner widget of cell. Only called if has_inner_widget returns true.
    fn inner_widget(
        &self,
        _ui: &mut Ui,
        _shared_data: &mut G,
        _state: &mut AppUIState,
        _project: &Project,
    ) {
    }

    /// Whether this cell has an inner widget.
    fn has_inner_widget(&self, _shared_data: &mut G) -> bool {
        false
    }

    /// Background colour of cell.
    fn color(&self, _shared_data: &G) -> Color32 {
        Color32::TRANSPARENT
    }

    /// Whether this cell has a context menu when right-clicked.
    fn has_context_menu(&self) -> bool {
        false
    }

    /// Context menu when right-clicked. Only shows if has_context_menu() returns true.
    fn context_menu(
        &self,
        _ui: &mut Ui,
        _shared_data: &mut G,
        _state: &mut AppUIState,
        _project: &Project,
    ) {
    }

    /// Extra action to perform when double clicked or trigger button is pressed when this cell is
    /// highlighted.
    /// By default, will call self.on_click(...)
    fn trigger_action(&self, shared_data: &mut G, state: &mut AppUIState, project: &Project) {
        self.on_click(shared_data, state, project);
    }

    /// Extra action to perform when single-clicked
    fn on_click(&self, _shared_data: &mut G, _state: &mut AppUIState, _project: &Project) {}

    /// If there is no selection, then this is run if the cell is highlighted.
    /// If there is a selection, then this is run for all cells in the selection.
    fn on_keyboard_input(
        &self,
        _input: &egui::InputState,
        _shared_data: &mut G,
        _state: &mut AppUIState,
        _project: &Project,
    ) -> Option<CellGridEvent> {
        None
    }

    /// Whether cursor can highlight this cell.
    fn highlightable(&self) -> bool {
        true
    }

    /// Whether this cell can be part of a multi-cell selection.
    fn multiselectable(&self) -> bool {
        true
    }
}
