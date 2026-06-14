use egui::{Align2, Button, Color32, Event, Key, ScrollArea, Sense, Ui};
use egui_phosphor::regular::{self, FUNCTION, MINUS};

use crate::{
    app_ui_state::AppUIState,
    effects_menu::EffectsMenu,
    helpers,
    page::Page,
    project::{
        Note, NoteEffects, Phrase, PhraseCmd, PlayScope, Project, ProjectLocation, ROWS_PER_PHRASE,
        Semitone, VOICES_PER_TRACK,
    },
    synth::ROProject,
    widget::cells::{CellGridWidget, cell_data::CellData, event::CellGridEvent},
};

#[derive(Default)]
enum FXMenuState {
    #[default]
    Closed,
    Open {
        voice_idx: usize,
        note_idx: usize,
        fx_menu: EffectsMenu,
    },
}

#[derive(Default)]
struct GridState {
    playing_row: Option<usize>,
    last_semitone: Semitone,
    fx_menu: FXMenuState,
}

#[derive(Default, Clone)]
enum CellState {
    #[default]
    Empty,
    Note {
        note: Note,
        row: usize,
        voice: usize,
    },
    AddEffect {
        row: usize,
        voice: usize,
    },
}

impl CellData<GridState> for CellState {
    fn text(&self) -> Option<String> {
        match self {
            CellState::Empty => None,
            CellState::Note { note, .. } => {
                Some(note.semitone.map_or(MINUS.into(), |s| s.to_string()))
            }
            CellState::AddEffect { .. } => Some(FUNCTION.into()),
        }
    }

    fn text_color(&self, project: &Project, state: &mut AppUIState) -> Color32 {
        let default = helpers::to_colour32(state.preferences().style.colours.text);
        match self {
            CellState::AddEffect { row, voice } => {
                let Some(viewed_phrase) = state.viewed_phrase else {
                    return default;
                };

                // highlight if an effect has been added or not
                project
                    .phrases()
                    .get(&viewed_phrase)
                    .and_then(|phrase| phrase.voices.get(*voice))
                    .and_then(|v| v.notes.get(*row))
                    .filter(|n| !n.effects.is_empty())
                    .map_or(default, |_| {
                        helpers::to_colour32(state.preferences().style.colours.highlighted)
                    })
            }
            _ => default,
        }
    }

    fn inner_widget(&self, ui: &mut Ui, _: &mut GridState, _: &mut AppUIState, _: &Project) {
        // player indicator
        ui.horizontal_centered(|ui| {
            ui.label(regular::CARET_RIGHT);
        });
    }

    fn has_inner_widget(&self, grid_state: &mut GridState) -> bool {
        matches!(self, CellState::Note { row, .. } if Some(*row) == grid_state.playing_row)
    }

    fn trigger_action(
        &self,
        grid_state: &mut GridState,
        state: &mut AppUIState,
        project: &Project,
    ) {
        let Some(phrase_id) = state.viewed_phrase else {
            return;
        };

        match self {
            CellState::Empty => {}
            CellState::Note { note, row, voice } => {
                if note.semitone.is_none() {
                    project.push_cmd(PhraseCmd::UpdateNote {
                        id: phrase_id,
                        voice_index: *voice,
                        note_index: *row,
                        new_note: Note {
                            semitone: Some(grid_state.last_semitone),
                            effects: NoteEffects::default(),
                        },
                    });
                }
            }
            CellState::AddEffect { .. } => {
                self.on_click(grid_state, state, project);
            }
        }
    }

    fn on_click(&self, grid_state: &mut GridState, _state: &mut AppUIState, _project: &Project) {
        if let CellState::AddEffect { row, voice } = self {
            grid_state.fx_menu = FXMenuState::Open {
                voice_idx: *voice,
                note_idx: *row,
                fx_menu: EffectsMenu::default(),
            };
        }
    }

    fn on_keyboard_input(
        &self,
        input: &egui::InputState,
        grid_state: &mut GridState,
        state: &mut AppUIState,
        project: &Project,
    ) -> Option<CellGridEvent> {
        let evt = self.keyboard_input_move_between_phrases(input, state, project);
        if evt.is_some() {
            return evt;
        }

        let Self::Note { note, row, voice } = self else {
            return None;
        };
        let mut new_note = note.clone();

        // get the currently viewed phrase
        let phrase_id = state.viewed_phrase?;

        // handling transpose
        let transpose_amount = input.events.iter().find_map(|evt| {
            let Event::Key {
                physical_key,
                pressed: true,
                modifiers,
                ..
            } = evt
            else {
                return None;
            };
            let key = (*physical_key)?;

            let transpose_amount = if key == state.preferences().keybinds.increase {
                1
            } else if key == state.preferences().keybinds.decrease {
                -1
            } else {
                return None;
            } * if modifiers.shift { 12 } else { 1 };

            Some(transpose_amount)
        });

        if let Some(ta) = transpose_amount {
            new_note.semitone = new_note
                .semitone
                .map(|s| s.transposed_by(ta))
                .or(Some(grid_state.last_semitone));
        }

        // handling delete
        if input.key_pressed(state.preferences().keybinds.delete) {
            new_note.semitone = None;
        }

        if let Some(s) = new_note.semitone {
            grid_state.last_semitone = s;
        }

        if new_note != *note {
            project.push_cmd(PhraseCmd::UpdateNote {
                id: phrase_id,
                voice_index: *voice,
                note_index: *row,
                new_note,
            });
        }

        None
    }

    fn highlightable(&self) -> bool {
        true
    }

    fn multiselectable(&self) -> bool {
        matches!(self, CellState::Note { .. })
    }
}

impl CellState {
    fn keyboard_input_move_between_phrases(
        &self,
        input: &egui::InputState,
        state: &mut AppUIState,
        project: &Project,
    ) -> Option<CellGridEvent> {
        let (row, voice) = match self {
            CellState::Note { row, voice, .. } => (row, voice * 2),
            CellState::AddEffect { row, voice } => (row, (voice * 2) + 1),
            _ => return None,
        };

        if input.key_pressed(state.preferences().keybinds.down) && *row >= ROWS_PER_PHRASE - 1 {
            get_next_row_and_phrase(state, project).map(|(row, phrase)| {
                state.chain_selected_row = Some(row);
                state.viewed_phrase = Some(phrase);
                CellGridEvent::Multiple(vec![
                    CellGridEvent::ConsumeInput,
                    CellGridEvent::SetHighlightedPosition(0, voice),
                ])
            })
        } else if input.key_pressed(state.preferences().keybinds.up) && *row == 0 {
            get_previous_row_and_phrase(state, project).map(|(row, phrase)| {
                state.chain_selected_row = Some(row);
                state.viewed_phrase = Some(phrase);
                CellGridEvent::Multiple(vec![
                    CellGridEvent::ConsumeInput,
                    CellGridEvent::SetHighlightedPosition(ROWS_PER_PHRASE - 1, voice),
                ])
            })
        } else {
            None
        }
    }
}

pub struct PhraseUI {
    cell_grid: CellGridWidget<CellState, GridState>,

    // used for buffering player
    last_seen_phrase_id: Option<u32>,
}

impl Default for PhraseUI {
    fn default() -> Self {
        Self {
            cell_grid: CellGridWidget::new(
                ROWS_PER_PHRASE,
                VOICES_PER_TRACK * 2,
                GridState::default(),
            )
            .shade_every(4),
            last_seen_phrase_id: None,
        }
    }
}

impl Page for PhraseUI {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        self.handle_keybinds(ui, state, project);

        if self.last_seen_phrase_id != state.viewed_phrase {
            self.handle_player_buffering(state);
        }
        self.last_seen_phrase_id = state.viewed_phrase;

        // set up cell grid
        let Some(phrase) = state
            .viewed_phrase
            .and_then(|id| project.phrases().get(&id))
        else {
            ui.label("No valid phrase selected");
            ui.allocate_space(ui.available_size());
            return;
        };

        ScrollArea::both().show(ui, |ui| {
            self.show_notes(phrase, ui, state, project);
            ui.allocate_space(ui.available_size());
        });

        if matches!(self.cell_grid.shared_data.fx_menu, FXMenuState::Open { .. }) {
            self.show_fx_menu(ui, state, project);
        }
    }

    fn draw_side_buttons(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        let up_button = Button::new(regular::ARROW_UP).small();
        if let Some((row, phrase)) = get_previous_row_and_phrase(state, project) {
            if ui.add(up_button).clicked() {
                state.chain_selected_row = Some(row);
                state.viewed_phrase = Some(phrase);
            }
        } else {
            ui.add_enabled(false, up_button);
        }

        let down_button = Button::new(regular::ARROW_DOWN).small();
        if let Some((row, phrase)) = get_next_row_and_phrase(state, project) {
            if ui.add(down_button).clicked() {
                state.chain_selected_row = Some(row);
                state.viewed_phrase = Some(phrase);
            }
        } else {
            ui.add_enabled(false, down_button);
        }
    }

    fn play(&self, state: &AppUIState, project: ROProject) {
        if let Some(start) = self.play_start_location(state) {
            state.player.play(project, vec![start], PlayScope::Phrase);
        }
    }

    fn heading(&self, state: &AppUIState) -> String {
        format!("Phrase {}", state.viewed_phrase.unwrap_or_default())
    }
}

impl PhraseUI {
    fn show_notes(
        &mut self,
        phrase: &Phrase,
        ui: &mut Ui,
        state: &mut AppUIState,
        project: &Project,
    ) {
        // set up cell grid
        for (voice_idx, voice) in phrase.voices.iter().enumerate() {
            for (note_idx, note) in voice.notes.iter().enumerate() {
                // showing note
                self.cell_grid.state.set(
                    note_idx,
                    voice_idx * 2,
                    CellState::Note {
                        note: note.clone(),
                        row: note_idx,
                        voice: voice_idx,
                    },
                );

                // button to select effects
                self.cell_grid.state.set(
                    note_idx,
                    (voice_idx * 2) + 1,
                    CellState::AddEffect {
                        row: note_idx,
                        voice: voice_idx,
                    },
                );
            }
        }

        // determine which row is playing
        self.cell_grid.shared_data.playing_row = self.playing_row(state);
        self.cell_grid.show(ui, state, project);
    }

    fn playing_row(&self, state: &AppUIState) -> Option<usize> {
        let track_idx = state.viewed_track?;
        let positions = state.player.request_locations()?;
        let pos = positions.iter().find(|pos| {
            pos.track_idx == track_idx
                && Some(pos.row_in_track) == state.track_selected_row
                && Some(pos.row_in_chain) == state.chain_selected_row
        })?;

        Some(pos.row_in_phrase)
    }

    fn show_fx_menu(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        let Some(phrase_id) = state.viewed_phrase else {
            return;
        };

        let FXMenuState::Open {
            voice_idx,
            note_idx,
            fx_menu,
        } = &mut self.cell_grid.shared_data.fx_menu
        else {
            return;
        };

        let Some(mut note) = Self::get_note(*note_idx, *voice_idx, state, project) else {
            self.cell_grid.shared_data.fx_menu = FXMenuState::Closed;
            return;
        };

        let original_note = note.clone();

        let viewport_rect = ui.input(egui::InputState::viewport_rect);
        let area_response = egui::Area::new("fx_dialog_bg".into())
            .order(egui::Order::Foreground)
            .interactable(true)
            .fixed_pos([0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.painter()
                    .rect_filled(viewport_rect, 0.0, Color32::from_black_alpha(150));
                ui.allocate_rect(viewport_rect, Sense::click());
            })
            .response;

        if area_response.clicked() {
            self.cell_grid.shared_data.fx_menu = FXMenuState::Closed;
            return;
        }

        let mut open = true;

        let diag_width = (viewport_rect.width() * 3.0) / 4.0;
        let diag_height = (viewport_rect.height() * 3.0) / 4.0;
        egui::Window::new("Effects")
            .id(egui::Id::new("fx_dialog"))
            .order(egui::Order::Tooltip)
            .collapsible(false)
            .resizable(false)
            .fixed_size((diag_width, diag_height))
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                // table of effects that could be added
                fx_menu.update(ui, &mut note.effects, project);
                if note != original_note {
                    project.push_cmd(PhraseCmd::UpdateNote {
                        id: phrase_id,
                        voice_index: *voice_idx,
                        note_index: *note_idx,
                        new_note: Note {
                            effects: note.effects.clone(),
                            ..original_note
                        },
                    });
                }
            });

        if !open || ui.input(|i| i.key_pressed(Key::Escape)) {
            self.cell_grid.shared_data.fx_menu = FXMenuState::Closed;
        }
    }

    fn get_note(
        note_idx: usize,
        voice_idx: usize,
        state: &AppUIState,
        project: &Project,
    ) -> Option<Note> {
        let phrase_id = state.viewed_phrase?;
        let phrase = project.phrases().get(&phrase_id)?;
        Some(phrase.voices.get(voice_idx)?.notes.get(note_idx)?.clone())
    }

    fn play_start_location(&self, state: &AppUIState) -> Option<ProjectLocation> {
        Some(ProjectLocation {
            track_idx: state.viewed_track?,
            row_in_track: state.track_selected_row?,
            row_in_chain: state.chain_selected_row?,
            row_in_phrase: self.cell_grid.state.highlighted_row(),
        })
    }

    fn handle_player_buffering(&self, state: &mut AppUIState) {
        // if not playing just a phrase, then skip
        if !matches!(state.player.request_scope(), Some(PlayScope::Phrase)) {
            return;
        }

        let Some(mut starting_position) = self.play_start_location(state) else {
            return;
        };

        starting_position.row_in_phrase = 0;

        state
            .player
            .play_buffered(vec![starting_position], PlayScope::Phrase);
    }

    fn handle_keybinds(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        ui.input(|i| {
            if i.key_pressed(state.preferences().keybinds.up_screen) {
                if let Some((row, phrase)) = get_previous_row_and_phrase(state, project) {
                    state.chain_selected_row = Some(row);
                    state.viewed_phrase = Some(phrase);
                }
            } else if i.key_pressed(state.preferences().keybinds.down_screen) {
                if let Some((row, phrase)) = get_next_row_and_phrase(state, project) {
                    state.chain_selected_row = Some(row);
                    state.viewed_phrase = Some(phrase);
                }
            }
        });
    }
}

fn get_previous_row_and_phrase(state: &mut AppUIState, project: &Project) -> Option<(usize, u32)> {
    let chain = project.chains().get(&state.viewed_chain?)?;
    let current_row = state.chain_selected_row?;
    chain
        .rows
        .iter()
        .enumerate()
        .take(current_row)
        .rev()
        .filter_map(|(idx, row)| row.phrase.map(|p| (idx, p)))
        .next()
}

fn get_next_row_and_phrase(state: &mut AppUIState, project: &Project) -> Option<(usize, u32)> {
    let chain = project.chains().get(&state.viewed_chain?)?;
    let current_row = state.chain_selected_row?;
    chain
        .rows
        .iter()
        .enumerate()
        .skip(current_row + 1)
        .filter_map(|(idx, row)| row.phrase.map(|p| (idx, p)))
        .next()
}
