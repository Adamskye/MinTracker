use std::sync::mpsc;

use egui::{Align2, Color32, Key, ScrollArea, Sense, Ui, Vec2, viewport};
use egui_phosphor::regular::{self, FUNCTION, MINUS};

use crate::{
    app_ui_state::AppUIState,
    effects_menu::EffectsMenu,
    helpers,
    page::Page,
    project::{
        Note, NoteEffects, Phrase, PhraseCmd, Project, ProjectLocation, ROWS_PER_PHRASE, Semitone,
        VOICES_PER_TRACK,
    },
    synth::{PlayerCmd, PlayerScope, ROProject},
    widget::cells::{CellData, CellGrid, CellGridEvent, cells},
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
                Some(note.semitone.map(|s| s.to_string()).unwrap_or(MINUS.into()))
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
                    .map(|_| helpers::to_colour32(state.preferences().style.colours.highlighted))
                    .unwrap_or(default)
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
        match self {
            CellState::AddEffect { row, voice } => {
                grid_state.fx_menu = FXMenuState::Open {
                    voice_idx: *voice,
                    note_idx: *row,
                    fx_menu: EffectsMenu::default(),
                };
            }
            _ => {}
        }
    }

    fn on_keyboard_input(
        &self,
        input: &egui::InputState,
        grid_state: &mut GridState,
        state: &mut AppUIState,
        project: &Project,
    ) -> Option<CellGridEvent> {
        let Self::Note { note, row, voice } = self else {
            return None;
        };
        let mut note = note.clone();

        // get the currently viewed phrase
        let phrase_id = state.viewed_phrase?;

        let mut changed = false;
        if input.key_pressed(state.preferences().keybinds.increase) {
            note.semitone = note.semitone.map(|s| s.transposed_by(1));
            if note.semitone.is_none() {
                note.semitone = Some(grid_state.last_semitone);
            }
            changed = true;
        } else if input.key_pressed(state.preferences().keybinds.decrease) {
            note.semitone = note.semitone.map(|s| s.transposed_by(-1));
            if note.semitone.is_none() {
                note.semitone = Some(grid_state.last_semitone);
            }
            changed = true;
        } else if input.key_pressed(state.preferences().keybinds.delete) {
            note.semitone = None;
            changed = true;
        } else if input.key_pressed(state.preferences().keybinds.down)
            && *row >= ROWS_PER_PHRASE - 1
        {
            return if go_to_next(state, project) {
                Some(CellGridEvent::SetHighlightedPosition(0, *voice))
            } else {
                None
            };
        } else if input.key_pressed(state.preferences().keybinds.up) && *row == 0 {
            return if go_to_previous(state, project) {
                Some(CellGridEvent::SetHighlightedPosition(
                    ROWS_PER_PHRASE - 1,
                    *voice,
                ))
            } else {
                None
            };
        }

        if !changed {
            return None;
        }

        if let Some(s) = note.semitone {
            grid_state.last_semitone = s;
        }

        project.push_cmd(PhraseCmd::UpdateNote {
            id: phrase_id,
            voice_index: *voice,
            note_index: *row,
            new_note: note,
        });

        None
    }

    fn highlightable(&self) -> bool {
        true
    }

    fn multiselectable(&self) -> bool {
        matches!(self, CellState::Note { .. })
    }
}

pub struct PhraseUI {
    cell_grid: CellGrid<CellState, GridState>,
    grid_state: GridState,
}

impl Default for PhraseUI {
    fn default() -> Self {
        Self {
            cell_grid: CellGrid::new(ROWS_PER_PHRASE, VOICES_PER_TRACK * 2),
            grid_state: GridState::default(),
        }
    }
}

impl Page for PhraseUI {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
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

        if matches!(self.grid_state.fx_menu, FXMenuState::Open { .. }) {
            self.show_fx_menu(ui, state, project);
        }
    }

    fn draw_side_buttons(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        if ui.small_button(regular::ARROW_UP).clicked() {
            go_to_previous(state, project);
        }

        if ui.small_button(regular::ARROW_DOWN).clicked() {
            go_to_next(state, project);
        }
    }

    fn play(&self, state: &AppUIState, project: ROProject) {
        let (Some(track_idx), Some(chain_offset), Some(phrase_offset)) = (
            state.viewed_track,
            state.track_selected_row,
            state.chain_selected_row,
        ) else {
            return;
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
                self.cell_grid.set(
                    note_idx,
                    voice_idx * 2,
                    CellState::Note {
                        note: note.clone(),
                        row: note_idx,
                        voice: voice_idx,
                    },
                );

                // button to select effects
                self.cell_grid.set(
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
        self.grid_state.playing_row = self.playing_row(state);

        cells(
            ui,
            &mut self.cell_grid,
            &mut self.grid_state,
            state,
            project,
        );
    }

    fn playing_row(&self, state: &AppUIState) -> Option<usize> {
        let (tx, rx) = mpsc::channel();
        state.player.send_command(PlayerCmd::RequestLocation(tx));
        let positions = rx.recv().ok()?;
        let track_idx = state.viewed_track?;

        let pos = positions.iter().flatten().find(|pos| {
            pos.track_idx == track_idx
                && Some(pos.chain_offset) == state.track_selected_row
                && Some(pos.phrase_offset) == state.chain_selected_row
        })?;

        Some(pos.note_offset)
    }

    fn show_fx_menu(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        let Some(phrase_id) = state.viewed_phrase else {
            return;
        };

        let FXMenuState::Open {
            voice_idx,
            note_idx,
            fx_menu,
        } = &mut self.grid_state.fx_menu
        else {
            return;
        };

        let Some(mut note) = Self::get_note(*note_idx, *voice_idx, state, project) else {
            self.grid_state.fx_menu = FXMenuState::Closed;
            return;
        };

        let original_note = note.clone();

        let viewport_rect = ui.input(|i| i.viewport_rect());
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
            self.grid_state.fx_menu = FXMenuState::Closed;
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

        if open == false || ui.input(|i| i.key_pressed(Key::Escape)) {
            self.grid_state.fx_menu = FXMenuState::Closed;
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
}

fn go_to_previous(state: &mut AppUIState, project: &Project) -> bool {
    let Some(chain) = state
        .viewed_chain
        .and_then(|chain_idx| project.chains().get(&chain_idx))
    else {
        return false;
    };

    let Some(current_row) = state.chain_selected_row else {
        return false;
    };

    if let Some((new_idx, new_row)) = chain
        .rows
        .iter()
        .enumerate()
        .take(current_row)
        .rev()
        .find(|(_, row)| row.phrase.is_some())
    {
        state.chain_selected_row = Some(new_idx);
        state.viewed_phrase = new_row.phrase;
        true
    } else {
        false
    }
}

fn go_to_next(state: &mut AppUIState, project: &Project) -> bool {
    let Some(chain) = state
        .viewed_chain
        .and_then(|chain_idx| project.chains().get(&chain_idx))
    else {
        return false;
    };

    let Some(current_row) = state.chain_selected_row else {
        return false;
    };

    if let Some((new_idx, new_row)) = chain
        .rows
        .iter()
        .enumerate()
        .skip(current_row + 1)
        .find(|(_, row)| row.phrase.is_some())
    {
        state.chain_selected_row = Some(new_idx);
        state.viewed_phrase = new_row.phrase;
        true
    } else {
        false
    }
}
