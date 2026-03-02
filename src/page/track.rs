use eframe::{
    egui::{Button, ComboBox, Context, DragValue, Grid, Key, ScrollArea, Ui, Window},
    epaint::Color32,
};
use egui_phosphor::regular;
use itertools::Itertools;

use crate::{
    page::{Page, PageID},
    project::{Project, ProjectLocation, Track, CHAINS_PER_TRACK},
    selection::SelectionCoords,
    synth::{PlayerScope, ROProject},
    widget::cells::{self, CellData, CellGrid},
    AppUIState, ProjectEvent,
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
            TrackCellData::ChainButton { chain_id, .. } => Some(
                chain_id
                    .map(|i| i.to_string())
                    .unwrap_or(regular::MINUS.into()),
            ),
            TrackCellData::TrackOptionsButton { .. } => Some(regular::GEAR_SIX.into()),
            _ => None,
        }
    }

    fn inner_widget(
        &self,
        _ui: &mut Ui,
        _grid_state: &mut TrackUIGridState,
        _state: &mut AppUIState,
        _project: &Project,
    ) {
    }

    fn context_menu(
        &self,
        ui: &mut Ui,
        _grid_state: &mut TrackUIGridState,
        _state: &mut AppUIState,
        project: &Project,
    ) {
        match self {
            TrackCellData::Empty => {}
            TrackCellData::TrackOptionsButton { .. } => {}
            TrackCellData::ChainButton {
                track_index,
                chain_offset,
                chain_id: id,
            } => {
                Self::context_menu_chain(*track_index, *chain_offset, *id, ui, project);
            }
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
}

impl TrackCellData {
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
}

pub struct TrackUI {
    tool: Tool,
    _clipboard: Clipboard,
    grid_state: TrackUIGridState,
    cells_state: CellGrid<TrackCellData, TrackUIGridState>,
}

impl Default for TrackUI {
    fn default() -> Self {
        Self {
            tool: Tool::default(),
            _clipboard: Default::default(),
            grid_state: TrackUIGridState::default(),
            cells_state: CellGrid::new(CHAINS_PER_TRACK, 0),
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
        // fetch what is playing
        //let (tx, rx) = mpsc::channel();
        //state.player.send_command(PlayerCmd::RequestLocation(tx));

        //let position_opt: Option<Vec<Option<ProjectLocation>>> = rx.recv().ok();

        // update cells state
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
            if ui.button(regular::PLUS).clicked() {
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

    // fn show_chain_row(
    //     &mut self,
    //     ui: &mut Ui,
    //     row: usize,
    //     project: &Project,
    //     state: &mut AppUIState,
    //     playing_positions: Option<&Vec<Option<ProjectLocation>>>,
    // ) {
    //     for track_num in 0..self.local_state.tracks.len() {
    //         let is_playing = playing_positions
    //             .as_ref()
    //             .and_then(|position| {
    //                 position
    //                     .iter()
    //                     .filter_map(|it_opt| *it_opt)
    //                     .find(|it| it.track_idx == track_num && it.chain_offset == row)
    //             })
    //             .is_some();
    //
    //         self.show_chain_button(ui, state, project, row, track_num, is_playing);
    //     }
    // }

    // fn show_chain_button(
    //     &mut self,
    //     ui: &mut Ui,
    //     state: &mut AppUIState,
    //     project: &Project,
    //     row: usize,
    //     track_num: usize,
    //     playing: bool,
    // ) {
    //     let Some(chain) = self
    //         .local_state
    //         .tracks
    //         .get_mut(track_num)
    //         .and_then(|track| track.chains.get_mut(row))
    //     else {
    //         return;
    //     };
    //
    //     let label = match chain {
    //         Some(c) => c.to_string(),
    //         None => "-".to_owned(),
    //     };
    //
    //     let selected = if let Tool::Select(coords) = &self.tool {
    //         selection::widget_in_selection(coords, row, track_num)
    //     } else {
    //         false
    //     };
    //
    //     let btn = if selected {
    //         Button::new(label).stroke(Stroke::new(2.0, Color32::LIGHT_BLUE))
    //     } else {
    //         Button::new(label)
    //     }
    //     .corner_radius(0.0)
    //     .fill(Color32::TRANSPARENT)
    //     .sense(Sense::click_and_drag());
    //
    //     let response = ui
    //         .vertical_centered(|ui| {
    //             ui.horizontal(|ui| {
    //                 // playing position indicator
    //                 let pos_indicator = RichText::new(">").color(if playing && chain.is_some() {
    //                     Color32::RED
    //                 } else {
    //                     Color32::TRANSPARENT
    //                 });
    //                 ui.label(pos_indicator);
    //
    //                 // button
    //                 ui.add_sized([40.0, 20.0], btn)
    //             })
    //             .inner
    //         })
    //         .inner;
    //
    //     match &mut self.tool {
    //         Tool::Edit => {
    //             Self::chain_button_interaction(ui, project, state, &response, chain, track_num, row)
    //         }
    //         Tool::Select(coords) => {
    //             selection::handle_widget_selecting(ui, coords, &response, row, track_num);
    //             self.selection_context_menu(&response, row, track_num);
    //         }
    //     }
    // }

    // fn chain_button_interaction(
    //     ui: &mut Ui,
    //     project: &Project,
    //     state: &mut AppUIState,
    //     response: &Response,
    //     chain: &mut Option<u32>,
    //     track_num: usize,
    //     row: usize,
    // ) {
    //     if response.clicked() && chain.is_some() {
    //         state.viewed_track = Some(track_num);
    //         state.track_selected_row = Some(row);
    //         state.viewed_chain = *chain;
    //         state.current_page = PageID::Chain;
    //     }
    //
    //     if response.clicked() && chain.is_none() {
    //         *chain = project.chains().iter().next().map(|c| *c.0);
    //     }
    //
    //     Self::chain_context_menu(project, response, chain);
    //
    //     if !response.hovered() {
    //         return;
    //     }
    //
    //     let Some(chain) = chain else {
    //         return;
    //     };
    //
    //     if ui.input(|i| i.key_pressed(Key::A)) {
    //         for i in (0..*chain).rev() {
    //             if project.chains().get(&i).is_some() {
    //                 *chain = i;
    //                 break;
    //             }
    //         }
    //     } else if ui.input(|i| i.key_pressed(Key::D)) {
    //         if let Some((max_key, _)) = project.chains().iter().next_back() {
    //             for i in (*chain + 1)..=*max_key {
    //                 if project.chains().get(&i).is_some() {
    //                     *chain = i;
    //                     break;
    //                 }
    //             }
    //         }
    //     }
    // }

    // fn chain_context_menu(project: &Project, response: &Response, chain: &mut Option<u32>) {
    //     response.context_menu(|ui| {
    //         if ui.button("Create Chain").clicked() {
    //             ui.close();
    //             let id = Project::get_unique_key(project.chains());
    //             project.push_event(ProjectEvent::UpdateChain {
    //                 id,
    //                 new_chain: Default::default(),
    //             });
    //             *chain = Some(id);
    //         }
    //         if ui.button("Delete Chain").clicked() {
    //             ui.close();
    //             *chain = None;
    //         }
    //         if ui.button("Rename Chain").clicked() {
    //             ui.close();
    //         }
    //
    //         if ui.button("Shallow Clone").clicked() {
    //             ui.close();
    //             Self::shallow_clone(chain, project);
    //         }
    //
    //         if ui.button("Deep Clone").clicked() {
    //             ui.close();
    //             Self::deep_clone(chain, project);
    //         }
    //     });
    // }

    // fn selection_context_menu(&mut self, response: &Response, row: usize, track_num: usize) {
    //     response.context_menu(|ui| {
    //         if ui.button("Delete").clicked() {
    //             ui.close();
    //             self.delete_selection();
    //         }
    //
    //         if ui.button("Cut").clicked() {
    //             ui.close();
    //             self.copy_selection();
    //             self.delete_selection();
    //         }
    //
    //         if ui.button("Copy").clicked() {
    //             ui.close();
    //             self.copy_selection();
    //         }
    //
    //         if ui.button("Paste").clicked() {
    //             ui.close();
    //             self.paste_selection(row, track_num);
    //         }
    //     });
    // }

    // fn delete_selection(&mut self) {
    //     let Tool::Select(Some((coord1, coord2))) = self.tool else {
    //         return;
    //     };
    //
    //     let small_x = coord1.0.min(coord2.0);
    //     let big_x = coord1.0.max(coord2.0);
    //     let small_y = coord1.1.min(coord2.1);
    //     let big_y = coord1.1.max(coord2.1);
    //
    //     self.local_state
    //         .tracks
    //         .iter_mut()
    //         .take(big_x + 1)
    //         .skip(small_x)
    //         .for_each(|track| {
    //             track
    //                 .chains
    //                 .iter_mut()
    //                 .take(big_y + 1)
    //                 .skip(small_y)
    //                 .for_each(|chain| *chain = None);
    //         });
    // }
    //
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
