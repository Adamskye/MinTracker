use std::sync::mpsc;

use eframe::{
    egui::{
        util::undoer::{Settings, Undoer},
        Button, Grid, Key, Response, RichText, ScrollArea, Sense, Ui,
    },
    epaint::{Color32, Stroke},
};

use crate::{
    project::{Chain, Project, ProjectEvent, ProjectLocation, ROWS_PER_PHRASE},
    selection::{self, SelectionCoords},
    synth::{PlayerCmd, ROProject},
    AppUIState, Page, PageID,
};

type Clipboard = Vec<Option<u32>>;

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

        // find end of chain
        let project_read = project.read().unwrap();
        let Some(first_none_offset) = project_read
            .tracks()
            .get(track_idx)
            .and_then(|track| track.chains.get(chain_offset))
            .and_then(|chain_id_opt| *chain_id_opt)
            .and_then(|chain_id| project_read.chains().get(&chain_id))
            .map(|chain| &chain.phrases)
            .map(|phrase_list| {
                phrase_list
                    .iter()
                    .position(|phrase_opt| phrase_opt.is_none())
                    .unwrap_or(phrase_list.len())
            })
        else {
            return;
        };

        // if the first slot in the chain doesn't contain a phrase id
        if first_none_offset == 0 {
            return;
        }

        let phrase_offset = match self.tool {
            Tool::Select(Some(((_, row1), (_, row2)))) => std::cmp::min(row1, row2),
            _ => 0,
        };

        let starts = vec![ProjectLocation {
            track_idx,
            chain_offset,
            phrase_offset,
            note_offset: 0,
        }];

        let end = Some(ProjectLocation {
            track_idx,
            chain_offset,
            phrase_offset: first_none_offset.saturating_sub(1),
            note_offset: ROWS_PER_PHRASE - 1,
        });

        drop(project_read);
        state.player.play(project, starts.into(), end);
    }
}

impl ChainUI {
    fn handle_keybinds(ui: &mut Ui, tool: &mut Tool) {
        if ui.input(|i| i.key_pressed(Key::E)) {
            *tool = Tool::Edit;
        } else if ui.input(|i| i.key_pressed(Key::S)) {
            *tool = Tool::Select(None);
        }

        //if let Tool::SelectOld { event, .. } = tool {
        //    if ui.input(|i| i.key_pressed(Key::Delete)) {
        //        *event = SelectionEvent::Delete;
        //    }

        //    // relevant: https://github.com/emilk/egui/issues/4065
        //    ui.input(|i| {
        //        i.events.iter().for_each(|ev| match ev {
        //            Event::Copy => {
        //                *event = SelectionEvent::Copy;
        //            }
        //            Event::Paste(_) => {
        //                *event = SelectionEvent::Paste;
        //            }
        //            Event::Cut => {
        //                *event = SelectionEvent::Cut;
        //            }
        //            _ => (),
        //        })
        //    });
        //}
    }

    fn show_phrase_list(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        let (tx, rx) = mpsc::channel();
        state.player.send_command(PlayerCmd::RequestLocation(tx));
        let position_opt: Option<Vec<Option<ProjectLocation>>> = rx.recv().ok();

        for row in 0..self.local_state.chain.phrases.len() {
            // playing position indicator
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

            // phrase
            let phrase = &self.local_state.chain.phrases[row];
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
            .rounding(0.0)
            .fill(Color32::TRANSPARENT)
            .sense(Sense::click_and_drag());

            ui.horizontal(|ui| {
                ui.label(pos_indicator);
                let response = ui.add_sized([40.0, 20.0], btn);

                match &mut self.tool {
                    Tool::Edit => Self::handle_button_interaction(
                        ui,
                        &mut self.local_state,
                        state,
                        project,
                        response,
                        row,
                    ),
                    Tool::Select(selection) => {
                        selection::handle_widget_selecting(ui, selection, &response, row, 0);
                        self.selection_context_menu(&response, row);
                    }
                };
            });
        }
    }

    fn handle_button_interaction(
        ui: &mut Ui,
        s: &mut ChainUIState,
        state: &mut AppUIState,
        project: &Project,
        response: Response,
        row: usize,
    ) {
        {
            let Some(phrase) = s.chain.phrases.get_mut(row) else {
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

        let Some(Some(phrase)) = s.chain.phrases.get_mut(row) else {
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
            let Some(phrase) = s.chain.phrases.get_mut(row) else {
                return;
            };

            if ui.button("Create").clicked() {
                ui.close_menu();
                *phrase = Some(Project::get_unique_key(project.phrases()));
            }

            if ui.button("Delete").clicked() {
                ui.close_menu();
                *phrase = None;
            }
        });
    }

    fn selection_context_menu(&mut self, response: &Response, row: usize) {
        let Tool::Select(Some((coord1, coord2))) = self.tool else {
            return;
        };

        response.context_menu(|ui| {
            if ui.button("Delete").clicked() {
                ui.close_menu();

                Self::delete_selection(&mut self.local_state.chain, coord1.1, coord2.1)
            }

            if ui.button("Cut").clicked() {
                ui.close_menu();
                Self::copy_selection(
                    &mut self.local_state.chain,
                    &mut self.clipboard,
                    coord1.1,
                    coord2.1,
                );
                Self::delete_selection(&mut self.local_state.chain, coord1.1, coord2.1);
            }

            if ui.button("Copy").clicked() {
                ui.close_menu();
                Self::copy_selection(
                    &mut self.local_state.chain,
                    &mut self.clipboard,
                    coord1.1,
                    coord2.1,
                );
            }

            if ui.button("Paste").clicked() {
                ui.close_menu();
                Self::paste_selection(&mut self.local_state.chain, &mut self.clipboard, row);
            }
        });
    }

    fn delete_selection(chain: &mut Chain, row1: usize, row2: usize) {
        for phrase in chain
            .phrases
            .iter_mut()
            .take(row1.max(row2) + 1)
            .skip(row1.min(row2))
        {
            *phrase = None;
        }
    }

    fn copy_selection(chain: &mut Chain, clipboard: &mut Clipboard, row1: usize, row2: usize) {
        *clipboard = chain
            .phrases
            .iter()
            .cloned()
            .take(row1.max(row2) + 1)
            .skip(row1.min(row2))
            .collect::<Vec<Option<u32>>>();
    }

    fn paste_selection(chain: &mut Chain, clipboard: &Clipboard, row: usize) {
        for (proj_phrase, clipboard_phrase) in chain.phrases.iter_mut().skip(row).zip(clipboard) {
            *proj_phrase = *clipboard_phrase;
        }
    }
}
