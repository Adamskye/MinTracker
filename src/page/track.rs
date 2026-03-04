use std::sync::mpsc;

use eframe::{
    egui::{Button, ComboBox, Context, DragValue, Grid, Key, ScrollArea, Ui, Window},
    epaint::Color32,
};
use egui_phosphor::regular;
use itertools::Itertools;

use crate::{
    AppUIState, ProjectEvent,
    page::{Page, PageID},
    project::{CHAINS_PER_TRACK, Project, ProjectLocation, ProjectTimestamp, Track},
    selection::SelectionCoords,
    synth::{PlayerCmd, PlayerScope, ROProject},
    widget::cells::{self, CellData, CellGrid},
};

type Clipboard = Vec<Vec<Option<u32>>>;

#[derive(Default, PartialEq)]
enum Tool {
    #[default]
    Edit,
    Select(SelectionCoords),
}

#[derive(Clone, Default)]
pub enum TrackCellData {
    #[default]
    Empty,
    ChainButton {
        track_index: usize,
        chain_offset: usize,
        chain_id: Option<u32>,
    },
    TrackOptionsButton {
        track_index: usize,
    },
}

impl CellData<TrackUIGridState> for TrackCellData {
    fn color(&self) -> Color32 {
        Color32::TRANSPARENT
    }

    fn text(&self) -> Option<String> {
        match self {
            TrackCellData::Empty => None,
            TrackCellData::ChainButton { chain_id, .. } => Some(
                chain_id
                    .map(|i| i.to_string())
                    .unwrap_or(regular::MINUS.into()),
            ),
            TrackCellData::TrackOptionsButton { .. } => Some(regular::GEAR_SIX.into()),
        }
    }

    fn inner_widget(
        &self,
        ui: &mut Ui,
        grid_state: &mut TrackUIGridState,
        _state: &mut AppUIState,
        _project: &Project,
    ) {
        if let TrackCellData::ChainButton { .. } = self
            && self.is_playing(grid_state)
        {
            ui.horizontal_centered(|ui| {
                ui.label(regular::CARET_RIGHT);
            });
        }
    }

    fn has_inner_widget(&self, grid_state: &mut TrackUIGridState) -> bool {
        self.is_playing(grid_state)
    }

    fn context_menu(
        &self,
        ui: &mut Ui,
        _grid_state: &mut TrackUIGridState,
        _state: &mut AppUIState,
        project: &Project,
    ) {
        match self {
            TrackCellData::ChainButton {
                track_index,
                chain_offset,
                chain_id,
                ..
            } => {
                Self::context_menu_chain(*track_index, *chain_offset, *chain_id, ui, project);
            }
            _ => {}
        }
    }

    fn trigger_action(
        &self,
        grid_state: &mut TrackUIGridState,
        ui_state: &mut AppUIState,
        project: &Project,
    ) {
        match self {
            TrackCellData::Empty => {}
            TrackCellData::ChainButton {
                track_index,
                chain_offset,
                chain_id,
                ..
            } => {
                if let Some(id) = chain_id {
                    // go to chain screen
                    ui_state.viewed_track = Some(*track_index);
                    ui_state.track_selected_row = Some(*chain_offset);
                    ui_state.viewed_chain = Some(*id);
                    ui_state.current_page = PageID::Chain;
                } else {
                    // put a chain here
                    project.push_event(ProjectEvent::UpdateTrackCell {
                        track_index: *track_index,
                        chain_offset: *chain_offset,
                        new_chain_id: project.chains().iter().next().map(|(id, _)| *id),
                    });
                }
            }
            TrackCellData::TrackOptionsButton { .. } => {
                self.on_click(grid_state, ui_state, project);
            }
        }
    }

    fn on_click(
        &self,
        grid_state: &mut TrackUIGridState,
        _state: &mut AppUIState,
        _project: &Project,
    ) {
        // open track settings
        if let TrackCellData::TrackOptionsButton { track_index } = self {
            grid_state.track_opened_settings =
                if grid_state.track_opened_settings == Some(*track_index) {
                    None
                } else {
                    Some(*track_index)
                };
        }
    }

    fn selectable(&self) -> bool {
        matches!(self, TrackCellData::ChainButton { .. })
    }
}

impl TrackCellData {
    fn is_playing(&self, grid_state: &mut TrackUIGridState) -> bool {
        match self {
            TrackCellData::ChainButton {
                track_index,
                chain_offset,
                ..
            } => grid_state
                .playing
                .get(*track_index)
                .and_then(|opt| *opt)
                .map_or(false, |position| position.chain_offset == *chain_offset),
            _ => false,
        }
    }

    fn context_menu_chain(
        track_index: usize,
        chain_offset: usize,
        chain_id_opt: Option<u32>,
        ui: &mut Ui,
        project: &Project,
    ) {
        if ui.button("Create Chain").clicked() {
            ui.close();
            let id = Project::get_unique_key(project.chains());
            project.push_event(ProjectEvent::UpdateChain {
                id,
                new_chain: Default::default(),
            });
            project.push_event(ProjectEvent::UpdateTrackCell {
                track_index,
                chain_offset,
                new_chain_id: Some(id),
            });
        }
        if let Some(chain_id) = chain_id_opt {
            if ui.button("Delete Chain").clicked() {
                ui.close();
                project.push_event(ProjectEvent::UpdateTrackCell {
                    track_index,
                    chain_offset,
                    new_chain_id: None,
                });
            }

            if ui.button("Shallow Clone").clicked() {
                ui.close();

                let id = shallow_clone(chain_id, project);
                project.push_event(ProjectEvent::UpdateTrackCell {
                    track_index,
                    chain_offset,
                    new_chain_id: Some(id),
                });
            }

            if ui.button("Deep Clone").clicked() {
                ui.close();
                let id = deep_clone(chain_id, project);
                project.push_event(ProjectEvent::UpdateTrackCell {
                    track_index,
                    chain_offset,
                    new_chain_id: Some(id),
                });
            }
        }
    }
}

#[derive(Default)]
struct TrackUIGridState {
    track_opened_settings: Option<usize>,
    playing: Vec<Option<ProjectLocation>>,
}

pub struct TrackUI {
    tool: Tool,
    _clipboard: Clipboard,
    grid_state: TrackUIGridState,

    cells_state: CellGrid<TrackCellData, TrackUIGridState>,
    grid_update_ts: Option<ProjectTimestamp>,
}

impl Default for TrackUI {
    fn default() -> Self {
        Self {
            tool: Tool::default(),
            _clipboard: Default::default(),
            grid_state: TrackUIGridState::default(),
            cells_state: CellGrid::new(CHAINS_PER_TRACK, 0),
            grid_update_ts: None,
        }
    }
}

impl Page for TrackUI {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        ScrollArea::both().show(ui, |ui| {
            self.handle_keybinds(ui);
            ui.horizontal(|ui| {
                self.show_tracks(ui, state, project);
            });

            ui.allocate_space(ui.available_size());
        });
    }

    fn draw_side_buttons(&mut self, _ui: &mut Ui, _state: &mut AppUIState, _project: &Project) {}
    fn handle_undo(&mut self, _project: &Project) {}

    fn play(&self, state: &AppUIState, project: ROProject) {
        let chain_offset = match self.tool {
            Tool::Select(Some((coord1, coord2))) => std::cmp::min(coord1.1, coord2.1),
            _ => 0,
        };

        let scope = PlayerScope {
            first_notes: (0..project.read().unwrap().tracks().len())
                .map(|i| ProjectLocation {
                    track_idx: i,
                    chain_offset,
                    phrase_offset: 0,
                    note_offset: 0,
                })
                .collect(),
            last_note: None,
        };

        state.player.play(project, scope);
    }

    fn heading(&self, _: &AppUIState) -> String {
        "Song".to_string()
    }
}

impl TrackUI {
    fn handle_keybinds(&mut self, ui: &mut Ui) {
        if ui.input(|i| i.key_pressed(Key::E)) {
            self.tool = Tool::Edit;
        } else if ui.input(|i| i.key_pressed(Key::S)) {
            self.tool = Tool::Select(None);
        }
    }

    fn show_tracks(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        // fetch where the player is at
        let (tx, rx) = mpsc::channel();
        state.player.send_command(PlayerCmd::RequestLocation(tx));

        // update playing position
        let position_opt: Option<Vec<Option<ProjectLocation>>> = rx.recv().ok();
        if let Some(position_opt) = &position_opt {
            self.grid_state.playing = position_opt.clone();
        } else {
            self.grid_state.playing = vec![None; project.tracks().len()];
        }

        // update cells state

        // +1 for the option button row
        self.cells_state.set_num_columns(project.tracks().len());
        self.cells_state.set_num_rows(CHAINS_PER_TRACK + 1);

        // add track options buttons
        for track_index in 0..project.tracks().len() {
            self.cells_state.set(
                0,
                track_index,
                TrackCellData::TrackOptionsButton { track_index },
            );
        }

        // populate cells
        if Some(project.timestamp_last_update()) != self.grid_update_ts {
            self.grid_update_ts = Some(project.timestamp_last_update());

            for (track_index, track) in project.tracks().iter().enumerate() {
                for (chain_offset, &chain_id) in track.chains.iter().enumerate() {
                    self.cells_state.set(
                        chain_offset + 1,
                        track_index,
                        TrackCellData::ChainButton {
                            track_index,
                            chain_offset,
                            chain_id,
                        },
                    );
                }
            }
        }

        // show cells
        cells::cells(
            ui,
            &mut self.cells_state,
            &mut self.grid_state,
            state,
            project,
        );

        self.show_track_settings(ui.ctx(), project);

        ui.vertical(|ui| {
            if ui.button(format!("{} New Track", regular::PLUS)).clicked() {
                project.push_event(ProjectEvent::UpdateTrack {
                    index: project.tracks().len(),
                    new_track: Some(Box::new(Track::default())),
                });
            }
        });
    }

    fn show_track_settings(&mut self, context: &Context, project: &Project) {
        let Some(track_num) = self.grid_state.track_opened_settings else {
            return;
        };

        let Some(track) = project.tracks().get(track_num) else {
            return;
        };

        let mut settings = track.settings.clone();

        let mut to_remove = false;
        let mut open = true;
        Window::new(format!("Settings for track {track_num}"))
            .open(&mut open)
            .resizable(false)
            .show(context, |ui| {
                Grid::new(format!("Grid for settings for track {track_num}"))
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        // instrument select
                        ui.label("Instrument");
                        let selected_text = settings
                            .instrument
                            .and_then(|id| {
                                project
                                    .instruments()
                                    .get(&id)
                                    .map(|inst| format!("{} - {}", id, inst.name))
                            })
                            .unwrap_or("No Instrument".to_string());

                        let mut cb_instrument = settings.instrument;
                        ComboBox::from_id_salt(format!("Track instrument {track_num}"))
                            .selected_text(selected_text)
                            .show_ui(ui, |ui| {
                                for (id, inst) in project.instruments() {
                                    ui.selectable_value(
                                        &mut cb_instrument,
                                        Some(*id),
                                        format!("{} - {}", id, inst.name),
                                    );
                                }
                                ui.selectable_value(&mut cb_instrument, None, "None");
                            });

                        if cb_instrument != settings.instrument {
                            settings.instrument = cb_instrument;
                        }

                        ui.end_row();

                        // volume
                        ui.label("Volume");
                        ui.add(
                            DragValue::new(&mut settings.volume)
                                .range(0.0..=1.5)
                                .fixed_decimals(2)
                                .speed(0.05),
                        );

                        ui.end_row();

                        // pan
                        ui.label("Pan");
                        ui.add(
                            DragValue::new(&mut settings.pan)
                                .range(-1.0..=1.0)
                                .fixed_decimals(2)
                                .speed(0.05),
                        );

                        ui.end_row();

                        // transpose
                        ui.label("Transpose");
                        ui.add(
                            DragValue::new(&mut settings.transpose_semitones)
                                .fixed_decimals(2)
                                .speed(0.01),
                        );
                    });

                // mute
                let mute_button = Button::new("Mute");
                if ui
                    .add(if settings.muted {
                        mute_button.fill(Color32::RED)
                    } else {
                        mute_button
                    })
                    .clicked()
                {
                    settings.muted = !settings.muted;
                }

                // remove track
                if ui.button("Remove Track").clicked() {
                    to_remove = true;
                }
            });

        if to_remove && track_num < project.tracks().len() {
            project.push_event(ProjectEvent::UpdateTrack {
                index: track_num,
                new_track: None,
            });
        } else if settings != track.settings {
            project.push_event(ProjectEvent::UpdateTrackSettings {
                index: track_num,
                new_settings: settings,
            });
        }

        if !open {
            self.grid_state.track_opened_settings = None;
        }
    }

    // fn copy_selection(&mut self) {
    //     let Tool::Select(Some((coord1, coord2))) = self.tool else {
    //         return;
    //     };
    //
    //     let small_x = coord1.0.min(coord2.0);
    //     let big_x = coord1.0.max(coord2.0);
    //     let small_y = coord1.1.min(coord2.1);
    //     let big_y = coord1.1.max(coord2.1);
    //
    //     self.clipboard.clear();
    //
    //     self.local_state
    //         .tracks
    //         .iter_mut()
    //         .take(big_x + 1)
    //         .skip(small_x)
    //         .for_each(|track| {
    //             self.clipboard.push(
    //                 track
    //                     .chains
    //                     .iter()
    //                     .cloned()
    //                     .take(big_y + 1)
    //                     .skip(small_y)
    //                     .collect(),
    //             );
    //         });
    // }
    //
    // fn paste_selection(&mut self, row: usize, column: usize) {
    //     self.local_state
    //         .tracks
    //         .iter_mut()
    //         .skip(column)
    //         .take(self.clipboard.len())
    //         .zip(&self.clipboard)
    //         .for_each(|(proj_track, clip_track)| {
    //             proj_track
    //                 .chains
    //                 .iter_mut()
    //                 .skip(row)
    //                 .take(clip_track.len())
    //                 .zip(clip_track)
    //                 .for_each(|(proj_chain, clip_chain)| {
    //                     *proj_chain = *clip_chain;
    //                 });
    //         });
    // }
}

fn shallow_clone(chain_id: u32, project: &Project) -> u32 {
    let Some(chain) = project.chains().get(&chain_id) else {
        return chain_id;
    };

    // clone the thing
    let new_chain = Box::new(chain.clone());
    let id = Project::get_unique_key(project.chains());
    project.push_event(ProjectEvent::UpdateChain { id, new_chain });

    id
}

fn deep_clone(chain_id: u32, project: &Project) -> u32 {
    // shallow clone the chain
    let Some(mut new_chain) = project.chains().get(&chain_id).cloned() else {
        return chain_id;
    };

    // clone all the phrases inside the chain
    new_chain
        .rows
        .iter()
        .filter_map(|row| row.phrase)
        .sorted()
        .dedup()
        .enumerate()
        .for_each(|(n, unique_id)| {
            let new_id = Project::get_nth_unique_key(n, project.phrases());
            let new_phrase = project
                .phrases()
                .get(&unique_id)
                .cloned()
                .map(Box::new)
                .unwrap_or_default();

            project.push_event(ProjectEvent::UpdatePhrase {
                id: new_id,
                new_phrase,
            });

            new_chain
                .rows
                .iter_mut()
                .filter(|row| row.phrase == Some(unique_id))
                .for_each(|row| row.phrase = Some(new_id));
        });

    // add the chain
    let new_chain_id = Project::get_unique_key(project.chains());
    project.push_event(ProjectEvent::UpdateChain {
        id: new_chain_id,
        new_chain: Box::new(new_chain),
    });

    new_chain_id
}
