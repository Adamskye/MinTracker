use std::sync::mpsc;

use eframe::{
    egui::{ScrollArea, Ui},
    epaint::Color32,
};
use egui::Event;
use egui_phosphor::regular;

use crate::{
    AppUIState,
    page::{Page, PageID},
    project::{
        ChainCmd, ChainRow, PhraseCmd, Project, ProjectLocation, ROWS_PER_CHAIN, ROWS_PER_PHRASE,
    },
    synth::{PlayerCmd, PlayerScope, ROProject},
    widget::cells::{CellGridWidget, cell_data::CellData, event::CellGridEvent},
};

#[derive(Default, Clone)]
enum Clipboard {
    #[default]
    Empty,
    FullRows(Vec<ChainRow>),
    OnlyPhrases(Vec<Option<u32>>),
    OnlyTransposes(Vec<i32>),
}

#[derive(Default)]
struct GridState {
    playing_row: Option<usize>,
}

#[derive(Clone)]
enum CellState {
    Phrase { row: usize, id: Option<u32> },
    Transpose { row: usize, transpose: i32 },
}

impl Default for CellState {
    fn default() -> Self {
        Self::Phrase { row: 0, id: None }
    }
}

impl CellData<GridState> for CellState {
    fn text(&self) -> Option<String> {
        match self {
            CellState::Phrase { id: None, .. } => Some(regular::MINUS.into()),
            CellState::Phrase { id: Some(idx), .. } => Some(idx.to_string()),
            CellState::Transpose { transpose, .. } => Some(transpose.to_string()),
        }
    }

    fn color(&self, _grid_state: &GridState) -> Color32 {
        Color32::TRANSPARENT
    }

    fn has_inner_widget(&self, grid_state: &mut GridState) -> bool {
        match self {
            CellState::Phrase { row, .. } => Some(*row) == grid_state.playing_row,
            _ => false,
        }
    }

    fn inner_widget(&self, ui: &mut Ui, _: &mut GridState, _: &mut AppUIState, _: &Project) {
        ui.horizontal_centered(|ui| {
            ui.label(regular::CARET_RIGHT);
        });
    }

    fn has_context_menu(&self) -> bool {
        matches!(self, CellState::Phrase { .. })
    }

    fn context_menu(
        &self,
        ui: &mut Ui,
        _grid_state: &mut GridState,
        state: &mut AppUIState,
        project: &Project,
    ) {
        match self {
            CellState::Phrase { row, .. } => {
                Self::context_menu_phrase(*row, ui, state, project);
            }
            CellState::Transpose { .. } => {}
        }
    }

    fn trigger_action(
        &self,
        _grid_state: &mut GridState,
        state: &mut AppUIState,
        project: &Project,
    ) {
        match self {
            CellState::Phrase { row, id: Some(id) } => {
                // go to phrase screen
                state.chain_selected_row = Some(*row);
                state.viewed_phrase = Some(*id);
                state.current_page = PageID::Phrase;
            }
            CellState::Phrase { row, id: None } => {
                // create phrase in chain
                let Some((&id, _)) = project.phrases().iter().next() else {
                    return;
                };

                let Some(chain_id) = state.viewed_chain else {
                    return;
                };

                project.push_cmd(ChainCmd::UpdatePhrase {
                    id: chain_id,
                    row_index: *row,
                    new_phrase_id: Some(id),
                });
            }
            _ => {}
        }
    }

    fn on_keyboard_input(
        &self,
        input: &egui::InputState,
        _grid_state: &mut GridState,
        state: &mut AppUIState,
        project: &Project,
    ) -> Option<CellGridEvent> {
        match self {
            CellState::Phrase { row, id: Some(id) } => {
                Self::keyboard_input_phrase(*row, *id, input, _grid_state, state, project)
            }
            CellState::Transpose { row, .. } => {
                Self::keyboard_input_transpose(*row, input, _grid_state, state, project)
            }
            _ => {}
        }
        None
    }
}

impl CellState {
    fn context_menu_phrase(row: usize, ui: &mut Ui, ui_state: &AppUIState, project: &Project) {
        // get viewed chain
        // get current chain
        let Some((viewed_chain, chain)) = ui_state
            .viewed_chain
            .and_then(|id| project.chains().get(&id).map(|c| (id, c)))
        else {
            return;
        };

        if ui.button("Create Phrase").clicked() {
            ui.close();
            let id = Project::get_unique_key(project.phrases());
            let mut chain = chain.clone();
            if let Some(row) = chain.rows.get_mut(row) {
                row.phrase = Some(id);
            }

            // create phrase
            project.push_cmd(PhraseCmd::Update {
                id,
                new_phrase: Default::default(),
            });

            // add it to chain
            project.push_cmd(ChainCmd::UpdatePhrase {
                id: viewed_chain,
                row_index: row,
                new_phrase_id: Some(id),
            });
        }

        if let Some(phrase) = chain.rows.get(row).and_then(|r| r.phrase) {
            if ui.button("Delete").clicked() {
                ui.close();
                let mut chain = chain.clone();
                if let Some(row) = chain.rows.get_mut(row) {
                    row.phrase = None;
                }

                project.push_cmd(ChainCmd::UpdatePhrase {
                    id: viewed_chain,
                    row_index: row,
                    new_phrase_id: None,
                });
            }

            if ui.button("Clone").clicked() {
                ui.close();
                let mut chain = chain.clone();

                // clone phrase
                let new_id = Project::get_unique_key(project.phrases());
                if let Some(row) = chain.rows.get_mut(row) {
                    let new_phrase = project.phrases().get(&phrase).cloned().map(Box::new);
                    if let Some(new_phrase) = new_phrase {
                        project.push_cmd(PhraseCmd::Update {
                            id: new_id,
                            new_phrase,
                        });
                        row.phrase = Some(new_id);
                    }
                }

                // update chain to include new phrase
                project.push_cmd(ChainCmd::UpdatePhrase {
                    id: viewed_chain,
                    row_index: row,
                    new_phrase_id: Some(new_id),
                });
            }
        }
    }

    fn keyboard_input_phrase(
        row: usize,
        mut id: u32,
        input: &egui::InputState,
        _grid_state: &mut GridState,
        state: &mut AppUIState,
        project: &Project,
    ) {
        let inc_keybind = state.preferences().keybinds.increase;
        let dec_keybind = state.preferences().keybinds.decrease;
        let del_keybind = state.preferences().keybinds.delete;
        let original_id = id;

        if input.key_pressed(dec_keybind) {
            for i in (0..id).rev() {
                if project.phrases().get(&i).is_some() {
                    id = i;
                    break;
                }
            }
        } else if input.key_pressed(inc_keybind)
            && let Some((max_key, _)) = project.phrases().iter().next_back()
        {
            for i in (id + 1)..=*max_key {
                if project.phrases().get(&i).is_some() {
                    id = i;
                    break;
                }
            }
        } else if input.key_pressed(del_keybind) {
            project.push_cmd(ChainCmd::UpdatePhrase {
                id,
                row_index: row,
                new_phrase_id: None,
            });
        }

        if id != original_id {
            // if id of phrase was changed, update the chain
            project.push_cmd(ChainCmd::UpdatePhrase {
                id: state.viewed_chain.unwrap(),
                row_index: row,
                new_phrase_id: Some(id),
            });
        }
    }

    fn keyboard_input_transpose(
        row: usize,
        input: &egui::InputState,
        _grid_state: &mut GridState,
        state: &mut AppUIState,
        project: &Project,
    ) {
        let inc_keybind = state.preferences().keybinds.increase;
        let dec_keybind = state.preferences().keybinds.decrease;
        let change = if input.key_pressed(inc_keybind) {
            1
        } else if input.key_pressed(dec_keybind) {
            -1
        } else {
            return;
        };

        let viewed_chain = match state.viewed_chain {
            Some(id) => id,
            None => return,
        };

        let Some(current_transpose) = project
            .chains()
            .get(&viewed_chain)
            .and_then(|chain| chain.rows.get(row))
            .map(|row| row.transpose)
        else {
            return;
        };

        project.push_cmd(ChainCmd::UpdateTranspose {
            id: viewed_chain,
            row_index: row,
            new_transpose: current_transpose + change,
        });
    }
}

pub struct ChainUI {
    clipboard: Clipboard,
    cell_grid: CellGridWidget<CellState, GridState>,
}

impl Default for ChainUI {
    fn default() -> Self {
        Self {
            clipboard: Default::default(),
            cell_grid: CellGridWidget::new(ROWS_PER_CHAIN, 2, GridState::default()),
        }
    }
}

impl Page for ChainUI {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        let chain_opt = state.viewed_chain.and_then(|id| project.chains().get(&id));

        self.handle_keybinds(ui, state, project);

        ScrollArea::vertical().show(ui, |ui| {
            if chain_opt.is_some() {
                self.show_phrase_list(ui, state, project);
            } else {
                ui.label("No valid chain selected");
            }

            ui.allocate_space(ui.available_size());
        });
    }

    fn draw_side_buttons(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        if let Some(viewed_chain) = state.viewed_chain
            && !project.chains().contains_key(&viewed_chain)
        {
            return;
        }

        self.up_down_side_buttons(ui, state, project);
    }

    fn handle_undo(&mut self, _project: &Project) {}

    fn play(&self, state: &AppUIState, project: ROProject) {
        let (Some(track_idx), Some(chain_offset)) = (state.viewed_track, state.track_selected_row)
        else {
            return;
        };

        let Some(last_phrase_offset) =
            Self::get_last_row_to_play(track_idx, chain_offset, &project.read().unwrap())
        else {
            return;
        };

        let scope = PlayerScope {
            first_notes: vec![ProjectLocation {
                track_idx,
                chain_offset,
                phrase_offset: self.cell_grid.state.highlighted_row(),
                note_offset: 0,
            }]
            .into(),
            last_note: Some(ProjectLocation {
                track_idx,
                chain_offset,
                phrase_offset: last_phrase_offset,
                note_offset: ROWS_PER_PHRASE - 1,
            }),
        };

        state.player.play(project, scope);
    }

    fn play_global(&self, state: &AppUIState, project: ROProject) {
        let Some(chain_offset) = state.track_selected_row else {
            return;
        };

        let num_tracks = project.read().unwrap().tracks().len();

        let scope = PlayerScope {
            first_notes: (0..num_tracks)
                .map(|track_idx| ProjectLocation {
                    track_idx,
                    chain_offset,
                    phrase_offset: self.cell_grid.state.highlighted_row(),
                    note_offset: 0,
                })
                .collect(),
            last_note: None,
        };

        state.player.play(project, scope);
    }

    fn heading(&self, state: &AppUIState) -> String {
        format!("Chain {}", state.viewed_chain.unwrap_or_default())
    }
}

impl ChainUI {
    fn handle_keybinds(&mut self, ui: &mut Ui, state: &AppUIState, project: &Project) {
        ui.input(|i| {
            if i.events.iter().any(|e| matches!(e, Event::Copy)) {
                self.copy_selection(project, state);
            } else if i.modifiers.command && i.events.iter().any(|e| matches!(e, Event::Paste(_))) {
                self.paste_clipboard(project, state);
            }
        });
    }

    fn get_last_row_to_play(
        track_idx: usize,
        chain_offset: usize,
        project: &Project,
    ) -> Option<usize> {
        // find end of chain
        let first_none_offset = project
            .tracks()
            .get(track_idx)
            .and_then(|track| track.chains.get(chain_offset))
            .and_then(|chain_id_opt| *chain_id_opt)
            .and_then(|chain_id| project.chains().get(&chain_id))
            .map(|chain| &chain.rows)
            .map(|rows| {
                rows.iter()
                    .position(|row| row.phrase.is_none())
                    .unwrap_or(rows.len())
            })?;

        // if the first slot in the chain doesn't contain a phrase id
        if first_none_offset == 0 {
            None
        } else {
            Some(first_none_offset.saturating_sub(1))
        }
    }

    fn playing_row(&self, state: &AppUIState) -> Option<usize> {
        let (tx, rx) = mpsc::channel();
        state.player.send_command(PlayerCmd::RequestLocation(tx));
        let positions = rx.recv().ok()?;

        positions.iter().flatten().find(|pos| {
            Some(pos.track_idx) == state.viewed_track
                && Some(pos.chain_offset) == state.track_selected_row
        }).map(|pos| pos.phrase_offset)
    }

    fn show_phrase_list(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        self.cell_grid.shared_data.playing_row = self.playing_row(state);

        let Some(chain) = state
            .viewed_chain
            .and_then(|chain_id| project.chains().get(&chain_id))
        else {
            return;
        };

        for (row_idx, row) in chain.rows.iter().enumerate() {
            self.cell_grid.state.set(
                row_idx,
                0,
                CellState::Phrase {
                    row: row_idx,
                    id: row.phrase,
                },
            );
            self.cell_grid.state.set(
                row_idx,
                1,
                CellState::Transpose {
                    row: row_idx,
                    transpose: row.transpose,
                },
            );
        }

        self.cell_grid.show(ui, state, project);
    }

    fn up_down_side_buttons(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        let Some(current_row) = state.track_selected_row else {
            return;
        };

        let Some(track) = state
            .viewed_track
            .and_then(|track_idx| project.tracks().get(track_idx))
        else {
            return;
        };

        if ui.small_button(regular::ARROW_UP).clicked() {
            for i in (0..current_row).rev() {
                let Some(chain_id) = track.chains.get(i) else {
                    continue;
                };

                state.track_selected_row = Some(i);
                state.viewed_chain = *chain_id;
                break;
            }
        }

        if ui.small_button(regular::ARROW_DOWN).clicked() {
            for i in (current_row + 1)..track.chains.len() {
                let Some(Some(chain_id)) = track.chains.get(i) else {
                    continue;
                };

                state.track_selected_row = Some(i);
                state.viewed_chain = Some(*chain_id);
                break;
            }
        }
    }

    fn copy_selection(&mut self, project: &Project, state: &AppUIState) {
        let Some(this_chain) = state.viewed_chain.and_then(|vc| project.chains().get(&vc)) else {
            return;
        };

        let Some(selection) = &self.cell_grid.state.selection else {
            return;
        };

        let small_col = selection.small_col();
        let big_col = selection.big_col();

        let small_row = selection.small_row();
        let big_row = selection.big_row();

        let selected_rows = this_chain
            .rows
            .iter()
            .skip(small_row)
            .take(big_row - small_row + 1)
            .cloned()
            .collect::<Vec<ChainRow>>();

        if small_col == 0 && big_col == 1 {
            // selecting full rows
            self.clipboard = Clipboard::FullRows(selected_rows);
        } else if small_col == 0 {
            // selecting only phrases
            self.clipboard =
                Clipboard::OnlyPhrases(selected_rows.into_iter().map(|row| row.phrase).collect());
        } else {
            // selecting only transposes
            self.clipboard = Clipboard::OnlyTransposes(
                selected_rows.into_iter().map(|row| row.transpose).collect(),
            );
        }
    }

    fn paste_clipboard(&mut self, project: &Project, state: &AppUIState) {
        let Some(mut chain) = state
            .viewed_chain
            .and_then(|vc| project.chains().get(&vc))
            .cloned()
        else {
            return;
        };

        let start_row = self
            .cell_grid
            .state
            .selection
            .as_ref()
            .map(|s| s.small_row())
            .unwrap_or_else(|| self.cell_grid.state.highlighted_position().0);

        // paste from clipboard, starting at row
        match &self.clipboard {
            Clipboard::Empty => return,
            Clipboard::FullRows(chain_rows) => {
                for (this_row, clipboard_row) in
                    chain.rows.iter_mut().skip(start_row).zip(chain_rows.iter())
                {
                    *this_row = clipboard_row.clone();
                }
            }
            Clipboard::OnlyPhrases(items) => {
                for (row, phrase_id_opt) in chain.rows.iter_mut().skip(start_row).zip(items.iter())
                {
                    row.phrase = *phrase_id_opt;
                }
            }
            Clipboard::OnlyTransposes(items) => {
                for (row, transpose) in chain.rows.iter_mut().skip(start_row).zip(items.iter()) {
                    row.transpose = *transpose;
                }
            }
        }

        // commit row
        project.push_cmd(ChainCmd::Update {
            id: state.viewed_chain.unwrap(),
            new_chain: chain,
        });
    }
}
