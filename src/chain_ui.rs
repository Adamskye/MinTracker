use std::sync::mpsc;

use eframe::{
    egui::{
        util::undoer::{Settings, Undoer},
        Button, DragValue, Frame, Grid, Key, Response, RichText, ScrollArea, Sense, Separator, Ui,
    },
    epaint::{Color32, Stroke},
};

use crate::{
    project::{Chain, ChainRow, Project, ProjectEvent, ProjectLocation, ROWS_PER_PHRASE},
    selection::{self, SelectionCoords},
    synth::{PlayerCmd, PlayerScope, ROProject},
    AppUIState, Page, PageID,
};

type Clipboard = Vec<ChainRow>;

#[derive(Default, PartialEq)]
enum Tool {
    #[default]
    Edit,
    Select(SelectionCoords),
}

#[derive(Clone, PartialEq, Default)]
pub struct ChainUIState {
    chain: Chain,
    chain_id: u32,
}

pub struct ChainUI {
    local_state: ChainUIState,
    tool: Tool,
    clipboard: Clipboard,
    undoer: Undoer<ChainUIState>,
}

impl Default for ChainUI {
    fn default() -> Self {
        Self {
            local_state: Default::default(),
            undoer: Undoer::with_settings(Settings {
                stable_time: 0.1,
                ..Default::default()
            }),
            tool: Default::default(),
            clipboard: Default::default(),
        }
    }
}

impl Page for ChainUI {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        let chain_opt = state.viewed_chain.and_then(|id| project.chains().get(&id));

        Self::handle_player_buffer(&self.local_state, state, project);
        ScrollArea::vertical().show(ui, |ui| {
            if let (Some(chain), Some(id)) = (chain_opt, state.viewed_chain) {
                if self.local_state.chain != *chain {
                    self.local_state.chain = chain.clone();
                }
                self.local_state.chain_id = id;

                Self::handle_keybinds(ui, &mut self.tool);
                self.show_phrase_list(ui, state, project);

                if self.local_state.chain != *chain {
                    project.push_event(ProjectEvent::UpdateChain {
                        id: self.local_state.chain_id,
                        new_chain: Box::new(self.local_state.chain.clone()),
                    })
                }
            } else {
                ui.label("No valid chain selected");
            }

            ui.allocate_space(ui.available_size());

            self.undoer
                .feed_state(ui.input(|i| i.time), &self.local_state);
        });
    }

    fn draw_side_buttons(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        if let Some(viewed_chain) = state.viewed_chain {
            if !project.chains().contains_key(&viewed_chain) {
                return;
            }
        }

        self.up_down_side_buttons(ui, state, project);
        ui.add_sized([40.0, 20.0], Separator::default().horizontal());

        // tool select
        let selection = if let Tool::Select(selection) = self.tool {
            selection
        } else {
            None
        };

        ui.selectable_value(&mut self.tool, Tool::Edit, "Edit");
        ui.selectable_value(&mut self.tool, Tool::Select(selection), "Select");
    }

    fn handle_undo(&mut self, project: &Project) {
        let Some(new_state) = self.undoer.undo(&self.local_state) else {
            return;
        };

        if self.local_state.chain != new_state.chain {
            project.push_event(ProjectEvent::UpdateChain {
                id: new_state.chain_id,
                new_chain: Box::new(new_state.chain.clone()),
            });
        }

        self.local_state = new_state.clone();
    }

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

        let phrase_offset = match self.tool {
            Tool::Select(Some(((_, row1), (_, row2)))) => std::cmp::min(row1, row2),
            _ => 0,
        };

        let scope = PlayerScope {
            first_notes: vec![ProjectLocation {
                track_idx,
                chain_offset,
                phrase_offset,
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
        if let Some(first_notes) = buf.map(|buf| buf.first_notes.clone()) {
            if let Some(buf_start) = first_notes.first() {
                if Self::project_location_to_chain_id(project, buf_start) == Some(s.chain_id) {
                    return;
                }
            }
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

    fn handle_keybinds(ui: &mut Ui, tool: &mut Tool) {
        if ui.input(|i| i.key_pressed(Key::E)) {
            *tool = Tool::Edit;
        } else if ui.input(|i| i.key_pressed(Key::S)) {
            *tool = Tool::Select(None);
        }
    }

    fn show_phrase_list(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        let (tx, rx) = mpsc::channel();
        state.player.send_command(PlayerCmd::RequestLocation(tx));
        let position_opt: Option<Vec<Option<ProjectLocation>>> = rx.recv().ok();

        Grid::new("chain_ui_phrase_grid")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.label("");
                ui.label("Phrases");
                ui.label("Transpose");
                ui.end_row();

                for row in 0..self.local_state.chain.rows.len() {
                    Self::position_indicator(ui, &position_opt, state, row);
                    let btn_response = self.phrase_button_no_interaction(ui, row);
                    self.transpose(ui, row);

                    ui.end_row();

                    let Some(btn_response) = btn_response else {
                        continue;
                    };
                    match &mut self.tool {
                        Tool::Edit => Self::phrase_button_interaction(
                            ui,
                            &mut self.local_state,
                            state,
                            project,
                            btn_response,
                            row,
                        ),
                        Tool::Select(selection) => {
                            selection::handle_widget_selecting(
                                ui,
                                selection,
                                &btn_response,
                                row,
                                0,
                            );
                            self.selection_context_menu(&btn_response, row);
                        }
                    };
                }
            });
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

    fn phrase_button_no_interaction(&self, ui: &mut Ui, row: usize) -> Option<Response> {
        let phrase = self.local_state.chain.rows.get(row).map(|row| row.phrase)?;

        let label = match phrase {
            Some(p) => p.to_string(),
            None => "-".to_string(),
        };

        let selected = if let Tool::Select(selection) = &self.tool {
            selection::widget_in_selection(selection, row, 0)
        } else {
            false
        };

        let btn = if selected {
            Button::new(label).stroke(Stroke::new(2.0, Color32::LIGHT_BLUE))
        } else {
            Button::new(label)
        }
        .corner_radius(0.0)
        .fill(Color32::TRANSPARENT)
        .sense(Sense::click_and_drag());

        Some(ui.add_sized([40.0, 20.0], btn))
    }

    fn transpose(&mut self, ui: &mut Ui, row: usize) {
        let Some(chain_row) = self.local_state.chain.rows.get_mut(row) else {
            return;
        };

        let selected = if let Tool::Select(selection) = &self.tool {
            selection::widget_in_selection(selection, row, 0)
        } else {
            false
        };

        if selected {
            Frame::new().stroke(Stroke::new(2.0, Color32::LIGHT_BLUE))
        } else {
            Frame::new()
        }
        .show(ui, |ui| {
            ui.add_sized(
                [40.0, 20.0],
                DragValue::new(&mut chain_row.transpose).speed(1.0),
            );
        });
    }

    fn phrase_button_interaction(
        ui: &mut Ui,
        s: &mut ChainUIState,
        state: &mut AppUIState,
        project: &Project,
        response: Response,
        row: usize,
    ) {
        {
            let Some(phrase) = s.chain.rows.get_mut(row).map(|row| &mut row.phrase) else {
                return;
            };

            if response.clicked() && phrase.is_some() {
                state.chain_selected_row = Some(row);
                state.viewed_phrase = *phrase;
                state.current_page = PageID::Phrase;
            }
            if response.clicked() && phrase.is_none() {
                *phrase = project.phrases().iter().next().map(|p| *p.0);
            }
        }

        Self::button_context_menu(s, project, &response, row);

        if !response.hovered() {
            return;
        }

        let Some(phrase) = s
            .chain
            .rows
            .get_mut(row)
            .and_then(|row| row.phrase.as_mut())
        else {
            return;
        };

        if ui.input(|i| i.key_pressed(Key::A)) {
            for i in (0..*phrase).rev() {
                if project.phrases().get(&i).is_some() {
                    *phrase = i;
                    break;
                }
            }
        } else if ui.input(|i| i.key_pressed(Key::D)) {
            if let Some((max_key, _)) = project.phrases().iter().next_back() {
                for i in (*phrase + 1)..=*max_key {
                    if project.phrases().get(&i).is_some() {
                        *phrase = i;
                        break;
                    }
                }
            }
        }
    }

    fn button_context_menu(
        s: &mut ChainUIState,
        project: &Project,
        response: &Response,
        row: usize,
    ) {
        response.context_menu(|ui| {
            let Some(phrase) = s
                .chain
                .rows
                .get_mut(row)
                .map(|chain_row| &mut chain_row.phrase)
            else {
                return;
            };

            if ui.button("Create Phrase").clicked() {
                ui.close();
                let id = Project::get_unique_key(project.phrases());
                project.push_event(ProjectEvent::UpdatePhrase {
                    id,
                    new_phrase: Default::default(),
                });
                *phrase = Some(id);
            }

            if ui.button("Delete").clicked() {
                ui.close();
                *phrase = None;
            }

            if ui.button("Clone").clicked() {
                ui.close();
                Self::clone_phrase(phrase, project);
            }
        });
    }

    fn selection_context_menu(&mut self, response: &Response, row: usize) {
        let Tool::Select(Some((coord1, coord2))) = self.tool else {
            return;
        };

        response.context_menu(|ui| {
            if ui.button("Delete").clicked() {
                ui.close();

                Self::delete_selection(&mut self.local_state.chain, coord1.1, coord2.1)
            }

            if ui.button("Cut").clicked() {
                ui.close();
                Self::copy_selection(
                    &mut self.local_state.chain,
                    &mut self.clipboard,
                    coord1.1,
                    coord2.1,
                );
                Self::delete_selection(&mut self.local_state.chain, coord1.1, coord2.1);
            }

            if ui.button("Copy").clicked() {
                ui.close();
                Self::copy_selection(
                    &mut self.local_state.chain,
                    &mut self.clipboard,
                    coord1.1,
                    coord2.1,
                );
            }

            if ui.button("Paste").clicked() {
                ui.close();
                Self::paste_selection(&mut self.local_state.chain, &self.clipboard, row);
            }
        });
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

        if ui.small_button("⬆").clicked() {
            for i in (0..current_row).rev() {
                let Some(chain_id) = track.chains.get(i) else {
                    continue;
                };

                state.track_selected_row = Some(i);
                state.viewed_chain = *chain_id;
                break;
            }
        }

        if ui.small_button("⬇").clicked() {
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
            .map(Box::new)
        else {
            return;
        };

        let id = Project::get_unique_key(project.phrases());
        project.push_event(ProjectEvent::UpdatePhrase { id, new_phrase });
        *phrase_id_opt = Some(id);

        //let Some(phrase_id) = phrase_id_opt else {
        //    return;
        //};

        //let phrase =
    }
}
