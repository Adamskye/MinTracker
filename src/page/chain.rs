use std::sync::mpsc;

use eframe::{
    egui::{
        RichText, ScrollArea, Ui,
        util::undoer::{Settings, Undoer},
    },
    epaint::Color32,
};
use egui_phosphor::regular;

use crate::{
    AppUIState,
    page::{Page, PageID},
    project::{
        Chain, ChainCmd, ChainRow, PhraseCmd, Project, ProjectLocation, ROWS_PER_CHAIN,
        ROWS_PER_PHRASE,
    },
    selection::SelectionCoords,
    synth::{PlayerCmd, PlayerScope, ROProject},
    widget::cells::{self, CellData, CellGrid},
};

type Clipboard = Vec<ChainRow>;

struct CellSharedState {}

#[derive(Clone)]
enum ChainCell {
    Phrase { row: usize, id: Option<u32> },
    Transpose { row: usize, transpose: f32 },
}

impl Default for ChainCell {
    fn default() -> Self {
        Self::Phrase { row: 0, id: None }
    }
}

impl CellData<CellSharedState> for ChainCell {
    fn text(&self) -> Option<String> {
        match self {
            ChainCell::Phrase { id: None, .. } => Some(regular::MINUS.into()),
            ChainCell::Phrase { id: Some(idx), .. } => Some(idx.to_string()),
            ChainCell::Transpose { transpose, .. } => Some(transpose.to_string()),
        }
    }

    fn color(&self) -> Color32 {
        Color32::TRANSPARENT
    }

    fn context_menu(
        &self,
        ui: &mut Ui,
        _grid_state: &mut CellSharedState,
        state: &mut AppUIState,
        project: &Project,
    ) {
        match self {
            ChainCell::Phrase { row, .. } => {
                Self::context_menu_phrase(*row, ui, state, project);
            }
            ChainCell::Transpose { .. } => {}
        }
    }

    fn trigger_action(
        &self,
        _grid_state: &mut CellSharedState,
        state: &mut AppUIState,
        project: &Project,
    ) {
        match self {
            ChainCell::Phrase { row, id: Some(id) } => {
                // go to phrase screen
                state.chain_selected_row = Some(*row);
                state.viewed_phrase = Some(*id);
                state.current_page = PageID::Phrase;
            }
            ChainCell::Phrase { row, id: None } => {
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
        _grid_state: &mut CellSharedState,
        state: &mut AppUIState,
        project: &Project,
    ) {
        match self {
            ChainCell::Phrase { row, id: Some(id) } => {
                Self::keyboard_input_phrase(*row, *id, input, _grid_state, state, project)
            }
            ChainCell::Transpose { row, .. } => {
                println!("{row}");
                Self::keyboard_input_transpose(*row, input, _grid_state, state, project)
            }
            _ => {}
        }
    }
}

impl ChainCell {
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
                            new_phrase: *new_phrase,
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
        _grid_state: &mut CellSharedState,
        state: &mut AppUIState,
        project: &Project,
    ) {
        let inc_keybind = state.preferences().keybinds.increase;
        let dec_keybind = state.preferences().keybinds.decrease;
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
        _grid_state: &mut CellSharedState,
        state: &mut AppUIState,
        project: &Project,
    ) {
        let inc_keybind = state.preferences().keybinds.increase;
        let dec_keybind = state.preferences().keybinds.decrease;
        let change = if input.key_pressed(inc_keybind) {
            1.0
        } else if input.key_pressed(dec_keybind) {
            -1.0
        } else {
            return;
        };

        let viewed_chain = match state.viewed_chain {
            Some(id) => id,
            None => return,
        };

        project.push_cmd(ChainCmd::UpdateTranspose {
            id: viewed_chain,
            row_index: row,
            new_transpose: change,
        });

        // project.push_event(ProjectEvent::UpdateChainNew {
        //     id: viewed_chain,
        //     new_chain: Arc::new(move |mut chain| {
        //         if let Some(row) = chain.rows.get_mut(row) {
        //             row.transpose += change;
        //         }
        //         chain
        //     }),
        // });
    }
}

#[derive(Clone, PartialEq, Default)]
pub struct ChainUIState {
    chain: Chain,
    chain_id: u32,
}

pub struct ChainUI {
    local_state: ChainUIState,
    clipboard: Clipboard,
    undoer: Undoer<ChainUIState>,
    cell_grid: CellGrid<ChainCell, CellSharedState>,
}

impl Default for ChainUI {
    fn default() -> Self {
        Self {
            local_state: Default::default(),
            undoer: Undoer::with_settings(Settings {
                stable_time: 0.1,
                ..Default::default()
            }),
            clipboard: Default::default(),
            cell_grid: CellGrid::new(ROWS_PER_CHAIN, 2),
        }
    }
}

impl Page for ChainUI {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        let chain_opt = state.viewed_chain.and_then(|id| project.chains().get(&id));

        //Self::handle_player_buffer(&self.local_state, state, project);
        ScrollArea::vertical().show(ui, |ui| {
            if chain_opt.is_some() {
                self.show_phrase_list(ui, state, project);
            } else {
                ui.label("No valid chain selected");
            }

            ui.allocate_space(ui.available_size());

            self.undoer
                .feed_state(ui.input(|i| i.time), &self.local_state);
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

    fn handle_undo(&mut self, project: &Project) {}

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
                phrase_offset: self.cell_grid.highlighted_row(),
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

    fn heading(&self, state: &AppUIState) -> String {
        format!("Chain {}", state.viewed_chain.unwrap_or_default())
    }
}

impl ChainUI {
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

    fn project_location_to_chain_id(project: &Project, location: &ProjectLocation) -> Option<u32> {
        project
            .tracks()
            .get(location.track_idx)
            .and_then(|track| track.chains.get(location.chain_offset))
            .and_then(|chain_id| *chain_id)
    }

    fn handle_player_buffer(s: &ChainUIState, state: &AppUIState, project: &Project) {
        let (scope_tx, scope_rx) = mpsc::channel();
        let (buf_tx, buf_rx) = mpsc::channel();
        state.player.send_command(PlayerCmd::RequestScope(scope_tx));
        state.player.send_command(PlayerCmd::RequestBuffer(buf_tx));

        let (Ok(scope), Ok(buf)) = (scope_rx.recv(), buf_rx.recv()) else {
            return;
        };

        // if playing multiple (or no) tracks, abort function
        if scope.first_notes.len() != 1 {
            return;
        }

        let Some(scope_start) = &scope.first_notes.first() else {
            return;
        };
        // there is always an end marker when playing just a chain
        let Some(scope_end) = &scope.last_note else {
            return;
        };

        // abort if not playing just a chain
        // TODO: simplify this
        if !(scope_start.phrase_offset < scope_end.phrase_offset
            || scope_start.track_idx == scope_end.track_idx
                && scope_start.chain_offset == scope_end.chain_offset
            || scope_start.phrase_offset == scope_end.phrase_offset
                && scope_start.note_offset <= scope_end.note_offset)
        {
            return;
        }

        // abort if playing same phrase_id that is being viewed
        if Some(s.chain_id) == Self::project_location_to_chain_id(project, scope_start) {
            return;
        }

        // abort if buffer is of the same phrase as is being viewed
        if let Some(first_notes) = buf.map(|buf| buf.first_notes.clone())
            && let Some(buf_start) = first_notes.first()
            && Self::project_location_to_chain_id(project, buf_start) == Some(s.chain_id)
        {
            return;
        }

        // update player buffer
        let Some(track_idx) = state.viewed_track else {
            return;
        };
        let Some(chain_offset) = state.track_selected_row else {
            return;
        };
        let Some(last_phrase_offset) = Self::get_last_row_to_play(track_idx, chain_offset, project)
        else {
            return;
        };
        let new_buffer = PlayerScope {
            first_notes: vec![ProjectLocation {
                track_idx,
                chain_offset,
                phrase_offset: 0,
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

        state
            .player
            .send_command(PlayerCmd::UpdateBuffer(new_buffer));
    }

    fn show_phrase_list(&mut self, ui: &mut Ui, ui_state: &mut AppUIState, project: &Project) {
        let (tx, rx) = mpsc::channel();
        ui_state.player.send_command(PlayerCmd::RequestLocation(tx));
        let position_opt: Option<Vec<Option<ProjectLocation>>> = rx.recv().ok();

        // add phrases to cell grid

        let Some(chain) = ui_state
            .viewed_chain
            .and_then(|chain_id| project.chains().get(&chain_id))
        else {
            return;
        };

        for (row_idx, row) in chain.rows.iter().enumerate() {
            self.cell_grid.set(
                row_idx,
                0,
                ChainCell::Phrase {
                    row: row_idx,
                    id: row.phrase,
                },
            );
            self.cell_grid.set(
                row_idx,
                1,
                ChainCell::Transpose {
                    row: row_idx,
                    transpose: row.transpose,
                },
            );
        }

        cells::cells(
            ui,
            &mut self.cell_grid,
            &mut CellSharedState {},
            ui_state,
            project,
        );
    }

    fn position_indicator(
        ui: &mut Ui,
        position_opt: &Option<Vec<Option<ProjectLocation>>>,
        state: &AppUIState,
        row: usize,
    ) {
        let is_playing = position_opt
            .as_ref()
            .and_then(|position| {
                position.iter().filter_map(|it_opt| *it_opt).find(|it| {
                    Some(it.track_idx) == state.viewed_track
                        && Some(it.chain_offset) == state.track_selected_row
                        && it.phrase_offset == row
                })
            })
            .is_some();

        let pos_indicator = RichText::new(">").color(if is_playing {
            Color32::RED
        } else {
            Color32::TRANSPARENT
        });

        ui.label(pos_indicator);
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

    fn delete_selection(chain: &mut Chain, row1: usize, row2: usize) {
        for row in chain
            .rows
            .iter_mut()
            .take(row1.max(row2) + 1)
            .skip(row1.min(row2))
        {
            *row = ChainRow::default();
        }
    }

    fn copy_selection(chain: &mut Chain, clipboard: &mut Clipboard, row1: usize, row2: usize) {
        *clipboard = chain
            .rows
            .iter()
            .take(row1.max(row2) + 1)
            .skip(row1.min(row2))
            .cloned()
            .collect::<Clipboard>();
    }

    fn paste_selection(chain: &mut Chain, clipboard: &Clipboard, row: usize) {
        for (proj_row, clipboard_row) in chain.rows.iter_mut().skip(row).zip(clipboard) {
            *proj_row = clipboard_row.clone();
        }
    }

    fn clone_phrase(phrase_id_opt: &mut Option<u32>, project: &Project) {
        let Some(new_phrase) = phrase_id_opt
            .and_then(|phrase_id| project.phrases().get(&phrase_id))
            .cloned()
        else {
            return;
        };

        let id = Project::get_unique_key(project.phrases());
        //project.push_event(ProjectEvent::UpdatePhrase { id, new_phrase });
        project.push_cmd(PhraseCmd::Update { id, new_phrase });

        *phrase_id_opt = Some(id);
    }
}
