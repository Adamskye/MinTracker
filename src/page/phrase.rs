use std::sync::mpsc;

use eframe::{
    egui::{
        self,
        util::undoer::{Settings, Undoer},
        Button, Key, Label, Response, RichText, ScrollArea, Sense, Ui,
    },
    epaint::{Color32, Stroke},
};
use egui_phosphor::regular;

use crate::{
    effects_menu::EffectsMenu,
    page::Page,
    project::{
        Note, Phrase, Project, ProjectEvent, ProjectLocation, MID_A_SEMITONE, ROWS_PER_PHRASE,
    },
    selection::{self, SelectionCoords},
    synth::{PlayerCmd, PlayerScope, ROProject},
    AppUIState,
};

type Clipboard = Vec<Vec<Note>>;

#[derive(Default, Clone, PartialEq)]
enum Tool {
    #[default]
    Edit,
    Select(SelectionCoords),
}

#[derive(Clone, PartialEq)]
pub struct PhraseUIState {
    last_note_semitone: u8,
    effects_menu: EffectsMenu,
    phrase: Phrase,
    phrase_id: u32,
}

impl Default for PhraseUIState {
    fn default() -> Self {
        Self {
            last_note_semitone: MID_A_SEMITONE,
            phrase: Phrase::default(),
            phrase_id: Default::default(),
            effects_menu: EffectsMenu::default(),
        }
    }
}

pub struct PhraseUI {
    local_state: PhraseUIState,
    tool: Tool,
    clipboard: Clipboard,
    undoer: Undoer<PhraseUIState>,
}

impl Default for PhraseUI {
    fn default() -> Self {
        Self {
            local_state: Default::default(),
            undoer: Undoer::with_settings(Settings {
                stable_time: 0.1,
                ..Default::default()
            }),
            clipboard: Vec::new(),
            tool: Tool::default(),
        }
    }
}

impl Page for PhraseUI {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        let phrase_opt = state
            .viewed_phrase
            .and_then(|id| project.phrases().get(&id));

        Self::handle_player_buffer(&self.local_state, state, project);

        ScrollArea::both().show(ui, |ui| {
            if let (Some(phrase), Some(id)) = (phrase_opt, state.viewed_phrase) {
                if self.local_state.phrase != *phrase {
                    self.local_state.phrase = phrase.clone();
                }
                self.local_state.phrase_id = id;

                self.handle_keybinds(ui);
                self.show_voices(ui, project, state);

                if self.local_state.phrase != *phrase {
                    project.push_event(ProjectEvent::UpdatePhrase {
                        id,
                        new_phrase: Box::new(self.local_state.phrase.clone()),
                    });
                }
            } else {
                ui.label("No valid phrase selected");
            }

            ui.allocate_space(ui.available_size());

            self.undoer
                .feed_state(ui.input(|i| i.time), &self.local_state);
        });
    }

    fn draw_side_buttons(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        self.up_down_side_buttons(ui, state, project);

        ui.add_sized([40.0, 20.0], egui::Separator::default().horizontal());

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

        if self.local_state.phrase != new_state.phrase {
            project.push_event(ProjectEvent::UpdatePhrase {
                id: new_state.phrase_id,
                new_phrase: Box::new(new_state.phrase.clone()),
            });
        }

        self.local_state = new_state.clone();
    }

    fn play(&self, state: &AppUIState, project: ROProject) {
        let (Some(track_idx), Some(chain_offset), Some(phrase_offset)) = (
            state.viewed_track,
            state.track_selected_row,
            state.chain_selected_row,
        ) else {
            return;
        };

        let note_offset = match self.tool {
            Tool::Select(Some(((_, row1), (_, row2)))) => std::cmp::min(row1, row2),
            _ => 0,
        };

        let scope = PlayerScope {
            first_notes: vec![ProjectLocation {
                track_idx,
                chain_offset,
                phrase_offset,
                note_offset,
            }]
            .into(),
            last_note: Some(ProjectLocation {
                track_idx,
                chain_offset,
                phrase_offset,
                note_offset: ROWS_PER_PHRASE - 1,
            }),
        };

        state.player.play(project, scope);
    }

    fn heading(&self, state: &AppUIState) -> String {
        format!("Phrase {}", state.viewed_phrase.unwrap_or_default())
    }
}

impl PhraseUI {
    fn project_location_to_phrase_id(project: &Project, location: &ProjectLocation) -> Option<u32> {
        project
            .tracks()
            .get(location.track_idx)
            .and_then(|track| track.chains.get(location.chain_offset))
            .and_then(|chain_id| *chain_id)
            .and_then(|chain_id| project.chains().get(&chain_id))
            .and_then(|chain| chain.rows.get(location.phrase_offset))
            .and_then(|row| row.phrase)
    }

    fn handle_player_buffer(s: &PhraseUIState, state: &AppUIState, project: &Project) {
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
        // there is always an end marker when playing just a phrase
        let Some(scope_end) = &scope.last_note else {
            return;
        };

        // abort if not playing just a phrase
        if !(scope_start.track_idx == scope_end.track_idx
            && scope_start.chain_offset == scope_end.chain_offset
            && scope_start.phrase_offset == scope_end.phrase_offset
            && scope_start.note_offset <= scope_end.note_offset)
        {
            return;
        }

        // abort if playing same phrase_id that is being viewed
        if Some(s.phrase_id) == Self::project_location_to_phrase_id(project, scope_start) {
            return;
        }

        // abort if buffer is of the same phrase as is being viewed
        if let Some(first_notes) = buf.map(|buf| buf.first_notes.clone())
            && let Some(buf_start) = first_notes.first()
                && Self::project_location_to_phrase_id(project, buf_start) == Some(s.phrase_id) {
                    return;
                }

        // update player buffer
        let Some(track_idx) = state.viewed_track else {
            return;
        };
        let Some(chain_offset) = state.track_selected_row else {
            return;
        };
        let Some(phrase_offset) = state.chain_selected_row else {
            return;
        };
        let new_buffer = PlayerScope {
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
                phrase_offset,
                note_offset: ROWS_PER_PHRASE - 1,
            }),
        };

        state
            .player
            .send_command(PlayerCmd::UpdateBuffer(new_buffer));
    }

    fn handle_keybinds(&mut self, ui: &mut Ui) {
        if ui.input(|i| i.key_pressed(Key::E)) {
            self.tool = Tool::Edit;
        } else if ui.input(|i| i.key_pressed(Key::S)) {
            self.tool = Tool::Select(None);
        }
    }

    fn show_voices(&mut self, ui: &mut Ui, project: &Project, state: &AppUIState) {
        ui.horizontal(|ui| {
            Self::show_play_pos_indicator(ui, state);
            for i_voice in 0..self.local_state.phrase.voices.len() {
                self.show_voice(ui, i_voice, project);
            }
        });
    }

    fn show_play_pos_indicator(ui: &mut Ui, state: &AppUIState) {
        let (tx, rx) = mpsc::channel();
        state.player.send_command(PlayerCmd::RequestLocation(tx));
        let position_opt = rx.recv().ok();

        ui.vertical(|ui| {
            for row in 0..ROWS_PER_PHRASE {
                let is_playing = position_opt
                    .as_ref()
                    .and_then(|position| {
                        position.iter().filter_map(|it_opt| *it_opt).find(|it| {
                            Some(it.track_idx) == state.viewed_track
                                && Some(it.chain_offset) == state.track_selected_row
                                && Some(it.phrase_offset) == state.chain_selected_row
                                && it.note_offset == row
                        })
                    })
                    .is_some();

                let pos_indicator = RichText::new(">").color(if is_playing {
                    Color32::RED
                } else {
                    Color32::TRANSPARENT
                });

                ui.add_sized([20.0, 20.0], Label::new(pos_indicator));
            }
        });
    }

    fn show_voice(&mut self, ui: &mut Ui, voice_id: usize, project: &Project) {
        let Some(num_notes) = self
            .local_state
            .phrase
            .voices
            .get_mut(voice_id)
            .map(|voice| voice.notes.len())
        else {
            return;
        };

        ui.vertical(|ui| {
            for row in 0..num_notes {
                self.note_controller(ui, row, voice_id, project);
            }
        });
    }

    fn note_controller(&mut self, ui: &mut Ui, row: usize, voice_id: usize, project: &Project) {
        let bg_colour = if (row / 4) % 2 == 1 {
            Color32::from_rgba_unmultiplied(128, 128, 128, 128)
        } else {
            Color32::TRANSPARENT
        };

        let selected = if let Tool::Select(selection) = self.tool {
            selection::widget_in_selection(&selection, row, voice_id)
        } else {
            false
        };

        ui.horizontal(|ui| {
            let Some(note) = self
                .local_state
                .phrase
                .voices
                .get_mut(voice_id)
                .and_then(|voice| voice.notes.get_mut(row))
            else {
                return;
            };

            let label = match note.semitone() {
                Some(semitone) => format!(
                    "{}{}{}",
                    Note::letter_from_semitone(semitone),
                    Note::octave_from_semitone(semitone),
                    if Note::sharp_from_semitone(semitone) {
                        "#"
                    } else {
                        ""
                    }
                ),
                None => "-".to_string(),
            };

            let btn = ui.add_sized(
                [40.0, 20.0],
                if selected {
                    Button::new(label).stroke(Stroke::new(2.0, Color32::LIGHT_BLUE))
                } else {
                    Button::new(label)
                }
                .fill(bg_colour)
                .corner_radius(0.0)
                .sense(Sense::click_and_drag()),
            );

            ui.menu_button(if note.has_effects() { "*+" } else { "+" }, |ui| {
                self.local_state
                    .effects_menu
                    .update(ui, &mut note.effects, project);
            });

            match &mut self.tool {
                Tool::Edit => Self::handle_note_editing(
                    ui,
                    note,
                    &btn,
                    &mut self.local_state.last_note_semitone,
                ),
                Tool::Select(selection) => {
                    selection::handle_widget_selecting(ui, selection, &btn, row, voice_id);
                    self.selection_context_menu(&btn, row, voice_id);
                }
            }
        });
    }

    fn selection_context_menu(&mut self, response: &Response, row: usize, voice_id: usize) {
        let Tool::Select(Some((coord1, coord2))) = self.tool else {
            return;
        };

        let phrase = &mut self.local_state.phrase;

        response.context_menu(|ui| {
            if ui.button("Delete").clicked() {
                ui.close();
                Self::delete_selection(phrase, coord1, coord2);
            }

            if ui.button("Cut").clicked() {
                ui.close();
                Self::copy_selection(phrase, &mut self.clipboard, coord1, coord2);
                Self::delete_selection(phrase, coord1, coord2);
            }

            if ui.button("Copy").clicked() {
                ui.close();
                Self::copy_selection(phrase, &mut self.clipboard, coord1, coord2);
            }

            if ui.button("Paste").clicked() {
                ui.close();
                Self::paste_selection(phrase, &self.clipboard, (voice_id, row));
            }
        });
    }

    fn handle_note_editing(
        ui: &mut Ui,
        note: &mut Note,
        btn: &Response,
        last_note_semitone: &mut u8,
    ) {
        // if can be edited
        if !btn.hovered() {
            return;
        }

        // note adding/deleting
        if btn.secondary_clicked() {
            match note.semitone() {
                Some(semitone) => *last_note_semitone = semitone,
                None => note.set_semitone(Some(*last_note_semitone)),
            }
        }

        if btn.clicked()
            && let Some(semitone) = note.semitone() {
                *last_note_semitone = semitone;
                note.set_semitone(None);
            }

        let Some(semitone) = note.semitone() else {
            return;
        };

        let amount = if ui.input(|i| i.modifiers.shift) {
            12
        } else {
            1
        };

        if ui.input(|i| i.key_pressed(Key::A)) {
            let new_semitone = semitone.saturating_sub(amount);
            *last_note_semitone = new_semitone;
            note.set_semitone(Some(new_semitone));
        }

        if ui.input(|i| i.key_pressed(Key::D)) {
            let new_semitone = semitone.saturating_add(amount);
            *last_note_semitone = new_semitone;
            note.set_semitone(Some(new_semitone));
        }
    }

    fn up_down_side_buttons(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        let Some(chain) = state
            .viewed_chain
            .and_then(|chain_id| project.chains().get(&chain_id))
        else {
            return;
        };

        let Some(current_row) = state.chain_selected_row else {
            return;
        };

        if ui.small_button(regular::ARROW_UP).clicked() {
            for i in (0..current_row).rev() {
                let Some(phrase_id) = chain.rows.get(i).and_then(|row| row.phrase) else {
                    continue;
                };

                state.chain_selected_row = Some(i);
                state.viewed_phrase = Some(phrase_id);
                break;
            }
        }

        if ui.small_button(regular::ARROW_DOWN).clicked() {
            for i in (current_row + 1)..chain.rows.len() {
                let Some(phrase_id) = chain.rows.get(i).and_then(|row| row.phrase) else {
                    continue;
                };

                state.chain_selected_row = Some(i);
                state.viewed_phrase = Some(phrase_id);
                break;
            }
        }
    }

    fn delete_selection(phrase: &mut Phrase, coord1: (usize, usize), coord2: (usize, usize)) {
        let small_x = coord1.0.min(coord2.0);
        let big_x = coord1.0.max(coord2.0);
        let small_y = coord1.1.min(coord2.1);
        let big_y = coord1.1.max(coord2.1);

        phrase
            .voices
            .iter_mut()
            .take(big_x + 1)
            .skip(small_x)
            .for_each(|voice| {
                voice
                    .notes
                    .iter_mut()
                    .take(big_y + 1)
                    .skip(small_y)
                    .for_each(|note| *note = Default::default())
            });
    }

    fn copy_selection(
        phrase: &mut Phrase,
        clipboard: &mut Clipboard,
        coord1: (usize, usize),
        coord2: (usize, usize),
    ) {
        let small_x = coord1.0.min(coord2.0);
        let big_x = coord1.0.max(coord2.0);
        let small_y = coord1.1.min(coord2.1);
        let big_y = coord1.1.max(coord2.1);

        clipboard.clear();

        phrase
            .voices
            .iter()
            .take(big_x + 1)
            .skip(small_x)
            .for_each(|voice| {
                clipboard.push(
                    voice
                        .notes
                        .iter()
                        .take(big_y + 1)
                        .skip(small_y)
                        .cloned()
                        .collect(),
                )
            });
    }

    fn paste_selection(phrase: &mut Phrase, clipboard: &Clipboard, top_left: (usize, usize)) {
        phrase
            .voices
            .iter_mut()
            .skip(top_left.0)
            .take(clipboard.len())
            .zip(clipboard)
            .for_each(|(proj_voice, clip_voice)| {
                proj_voice
                    .notes
                    .iter_mut()
                    .skip(top_left.1)
                    .take(clip_voice.len())
                    .zip(clip_voice)
                    .for_each(|(proj_note, clip_note)| {
                        *proj_note = clip_note.clone();
                    });
            });
    }
}
