use eframe::{
    egui::{
        util::undoer::{Settings, Undoer},
        Button, ComboBox, Context, DragValue, Grid, Key, Response, RichText, ScrollArea, Sense, Ui,
        Window,
    },
    epaint::{Color32, Stroke},
};
use itertools::Itertools;

use std::sync::mpsc;

use crate::{
    project::{Project, ProjectLocation, Track, CHAINS_PER_TRACK},
    selection::{self, SelectionCoords},
    synth::{PlayerCmd, ROProject},
    AppUIState, Page, PageID, ProjectEvent,
};

type Clipboard = Vec<Vec<Option<u32>>>;

#[derive(Default, PartialEq)]
enum Tool {
    #[default]
    Edit,
    Select(SelectionCoords),
}

#[derive(Clone, PartialEq, Default)]
struct TrackUIState {
    tracks: Vec<Track>,
}

pub struct TrackUI {
    local_state: TrackUIState,
    tool: Tool,
    clipboard: Clipboard,
    track_opened_settings: Option<usize>,

    undoer: Undoer<TrackUIState>,
}

impl Default for TrackUI {
    fn default() -> Self {
        Self {
            local_state: Default::default(),
            undoer: Undoer::with_settings(Settings {
                stable_time: 0.1,
                ..Default::default()
            }),
            tool: Tool::default(),
            clipboard: Default::default(),
            track_opened_settings: None,
        }
    }
}

impl Page for TrackUI {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        ScrollArea::both().show(ui, |ui| {
            if self.local_state.tracks.len() != project.tracks().len() {
                self.local_state
                    .tracks
                    .resize(project.tracks().len(), Default::default());
            }

            self.handle_keybinds(ui);
            Self::update_local_state(&mut self.local_state, project);
            ui.horizontal(|ui| {
                self.show_tracks(ui, state, project);
            });
            Self::update_project(&mut self.local_state, project);

            ui.allocate_space(ui.available_size());
        });

        self.undoer
            .feed_state(ui.input(|i| i.time), &self.local_state);
    }

    fn draw_side_buttons(&mut self, ui: &mut Ui, _state: &mut AppUIState, _project: &Project) {
        let tool_selection = if let Tool::Select(sel) = self.tool {
            sel
        } else {
            None
        };

        ui.selectable_value(&mut self.tool, Tool::Edit, "Edit");
        ui.selectable_value(&mut self.tool, Tool::Select(tool_selection), "Select");
    }

    fn handle_undo(&mut self, project: &Project) {
        let Some(new_state) = self.undoer.undo(&self.local_state) else {
            return;
        };

        self.local_state = new_state.clone();

        Self::update_project(&mut self.local_state, project);
    }

    fn play(&self, state: &AppUIState, project: ROProject) {
        let chain_offset = match self.tool {
            Tool::Select(Some((coord1, coord2))) => std::cmp::min(coord1.1, coord2.1),
            _ => 0,
        };

        let start_locations = (0..project.read().unwrap().tracks().len())
            .map(|i| ProjectLocation {
                track_idx: i,
                chain_offset,
                phrase_offset: 0,
                note_offset: 0,
            })
            .collect();

        state.player.play(project, start_locations, None);
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
        let (tx, rx) = mpsc::channel();
        state.player.send_command(PlayerCmd::RequestLocation(tx));

        let position_opt: Option<Vec<Option<ProjectLocation>>> = rx.recv().ok();

        Grid::new("track_grid_layout").show(ui, |ui| {
            self.show_track_settings_select(ui);
            ui.end_row();

            for row in 0..CHAINS_PER_TRACK {
                self.show_chain_row(ui, row, project, state, position_opt.as_ref());
                ui.end_row();
            }
        });

        self.show_track_settings(ui.ctx(), project);

        ui.vertical(|ui| {
            if ui.button("Add Track").clicked() {
                self.local_state.tracks.push(Default::default());
            }
        });
    }

    fn show_track_settings_select(&mut self, ui: &mut Ui) {
        // select track
        for track_num in 0..self.local_state.tracks.len() {
            ui.horizontal(|ui| {
                // todo: determine if this is needed in either light or dark mode
                //let fill_color = if self.track_opened_settings == Some(track_num) {
                //    Color32::LIGHT_BLUE
                //} else {
                //    Color32::LIGHT_GRAY
                //};

                let button = Button::new("⛭")/*.fill(fill_color)*/;
                ui.vertical_centered(|ui| {
                    if ui.add(button).clicked() {
                        self.track_opened_settings =
                            if self.track_opened_settings == Some(track_num) {
                                None
                            } else {
                                Some(track_num)
                            };
                    }
                });
            });
        }
    }

    fn show_track_settings(&mut self, context: &Context, project: &Project) {
        let Some(track_num) = self.track_opened_settings else {
            return;
        };

        let Some(track) = self.local_state.tracks.get_mut(track_num) else {
            return;
        };

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
                        let selected_text = track
                            .settings
                            .instrument
                            .and_then(|id| {
                                project
                                    .instruments()
                                    .get(&id)
                                    .map(|inst| format!("{} - {}", id, inst.name))
                            })
                            .unwrap_or("No Instrument".to_string());

                        let mut cb_instrument = track.settings.instrument;
                        ComboBox::from_id_source(format!("Track instrument {track_num}"))
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

                        if cb_instrument != track.settings.instrument {
                            track.settings.instrument = cb_instrument;
                        }

                        ui.end_row();

                        // volume
                        ui.label("Volume");
                        ui.add(
                            DragValue::new(&mut track.settings.volume)
                                .range(0.0..=1.5)
                                .fixed_decimals(2)
                                .speed(0.05),
                        );

                        ui.end_row();

                        // pan
                        ui.label("Pan");
                        ui.add(
                            DragValue::new(&mut track.settings.pan)
                                .range(-1.0..=1.0)
                                .fixed_decimals(2)
                                .speed(0.05),
                        );

                        ui.end_row();

                        // transpose
                        ui.label("Transpose");
                        ui.add(
                            DragValue::new(&mut track.settings.transpose_semitones)
                                .fixed_decimals(2)
                                .speed(0.01),
                        );
                    });

                // mute
                let mute_button = Button::new("Mute");
                if ui
                    .add(if track.settings.muted {
                        mute_button.fill(Color32::RED)
                    } else {
                        mute_button
                    })
                    .clicked()
                {
                    track.settings.muted = !track.settings.muted;
                }

                // remove track
                if ui.button("Remove Track").clicked() {
                    to_remove = true;
                }
            });

        if to_remove && track_num < self.local_state.tracks.len() {
            self.local_state.tracks.remove(track_num);
        }

        if !open {
            self.track_opened_settings = None;
        }
    }

    fn show_chain_row(
        &mut self,
        ui: &mut Ui,
        row: usize,
        project: &Project,
        state: &mut AppUIState,
        playing_positions: Option<&Vec<Option<ProjectLocation>>>,
    ) {
        for track_num in 0..self.local_state.tracks.len() {
            let is_playing = playing_positions
                .as_ref()
                .and_then(|position| {
                    position
                        .iter()
                        .filter_map(|it_opt| *it_opt)
                        .find(|it| it.track_idx == track_num && it.chain_offset == row)
                })
                .is_some();

            self.show_chain_button(ui, state, project, row, track_num, is_playing);
        }
    }

    fn show_chain_button(
        &mut self,
        ui: &mut Ui,
        state: &mut AppUIState,
        project: &Project,
        row: usize,
        track_num: usize,
        playing: bool,
    ) {
        let Some(chain) = self
            .local_state
            .tracks
            .get_mut(track_num)
            .and_then(|track| track.chains.get_mut(row))
        else {
            return;
        };

        let label = match chain {
            Some(c) => c.to_string(),
            None => "-".to_owned(),
        };

        let selected = if let Tool::Select(coords) = &self.tool {
            selection::widget_in_selection(coords, row, track_num)
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

        let response = ui
            .vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    // playing position indicator
                    let pos_indicator = RichText::new(">").color(if playing && chain.is_some() {
                        Color32::RED
                    } else {
                        Color32::TRANSPARENT
                    });
                    ui.label(pos_indicator);

                    // button
                    ui.add_sized([40.0, 20.0], btn)
                })
                .inner
            })
            .inner;

        match &mut self.tool {
            Tool::Edit => {
                Self::chain_button_interaction(ui, project, state, &response, chain, track_num, row)
            }
            Tool::Select(coords) => {
                selection::handle_widget_selecting(ui, coords, &response, row, track_num);
                self.selection_context_menu(&response, row, track_num);
            }
        }
    }

    fn chain_button_interaction(
        ui: &mut Ui,
        project: &Project,
        state: &mut AppUIState,
        response: &Response,
        chain: &mut Option<u32>,
        track_num: usize,
        row: usize,
    ) {
        if response.clicked() && chain.is_some() {
            state.viewed_track = Some(track_num);
            state.track_selected_row = Some(row);
            state.viewed_chain = *chain;
            state.current_page = PageID::Chain;
        }

        if response.clicked() && chain.is_none() {
            *chain = project.chains().iter().next().map(|c| *c.0);
        }

        Self::chain_context_menu(project, response, chain);

        if !response.hovered() {
            return;
        }

        let Some(chain) = chain else {
            return;
        };

        if ui.input(|i| i.key_pressed(Key::A)) {
            for i in (0..*chain).rev() {
                if project.chains().get(&i).is_some() {
                    *chain = i;
                    break;
                }
            }
        } else if ui.input(|i| i.key_pressed(Key::D)) {
            if let Some((max_key, _)) = project.chains().iter().next_back() {
                for i in (*chain + 1)..=*max_key {
                    if project.chains().get(&i).is_some() {
                        *chain = i;
                        break;
                    }
                }
            }
        }
    }

    fn chain_context_menu(project: &Project, response: &Response, chain: &mut Option<u32>) {
        response.context_menu(|ui| {
            if ui.button("Create Chain").clicked() {
                ui.close_menu();
                *chain = Some(Project::get_unique_key(project.chains()));
            }
            if ui.button("Delete Chain").clicked() {
                ui.close_menu();
                *chain = None;
            }
            if ui.button("Rename Chain").clicked() {
                ui.close_menu();
            }

            if ui.button("Shallow Clone").clicked() {
                ui.close_menu();
                Self::shallow_clone(chain, project);
            }

            if ui.button("Deep Clone").clicked() {
                ui.close_menu();
                Self::deep_clone(chain, project);
            }
        });
    }

    fn selection_context_menu(&mut self, response: &Response, row: usize, track_num: usize) {
        response.context_menu(|ui| {
            if ui.button("Delete").clicked() {
                ui.close_menu();
                self.delete_selection();
            }

            if ui.button("Cut").clicked() {
                ui.close_menu();
                self.copy_selection();
                self.delete_selection();
            }

            if ui.button("Copy").clicked() {
                ui.close_menu();
                self.copy_selection();
            }

            if ui.button("Paste").clicked() {
                ui.close_menu();
                self.paste_selection(row, track_num);
            }
        });
    }

    fn delete_selection(&mut self) {
        let Tool::Select(Some((coord1, coord2))) = self.tool else {
            return;
        };

        let small_x = coord1.0.min(coord2.0);
        let big_x = coord1.0.max(coord2.0);
        let small_y = coord1.1.min(coord2.1);
        let big_y = coord1.1.max(coord2.1);

        self.local_state
            .tracks
            .iter_mut()
            .take(big_x + 1)
            .skip(small_x)
            .for_each(|track| {
                track
                    .chains
                    .iter_mut()
                    .take(big_y + 1)
                    .skip(small_y)
                    .for_each(|chain| *chain = None);
            });
    }

    fn copy_selection(&mut self) {
        let Tool::Select(Some((coord1, coord2))) = self.tool else {
            return;
        };

        let small_x = coord1.0.min(coord2.0);
        let big_x = coord1.0.max(coord2.0);
        let small_y = coord1.1.min(coord2.1);
        let big_y = coord1.1.max(coord2.1);

        self.clipboard.clear();

        self.local_state
            .tracks
            .iter_mut()
            .take(big_x + 1)
            .skip(small_x)
            .for_each(|track| {
                self.clipboard.push(
                    track
                        .chains
                        .iter()
                        .cloned()
                        .take(big_y + 1)
                        .skip(small_y)
                        .collect(),
                );
            });
    }

    fn paste_selection(&mut self, row: usize, column: usize) {
        self.local_state
            .tracks
            .iter_mut()
            .skip(column)
            .take(self.clipboard.len())
            .zip(&self.clipboard)
            .for_each(|(proj_track, clip_track)| {
                proj_track
                    .chains
                    .iter_mut()
                    .skip(row)
                    .take(clip_track.len())
                    .zip(clip_track)
                    .for_each(|(proj_chain, clip_chain)| {
                        *proj_chain = *clip_chain;
                    });
            });
    }

    fn update_local_state(s: &mut TrackUIState, project: &Project) {
        for track_index in 0..project.tracks().len() {
            let Some(track) = s.tracks.get_mut(track_index) else {
                break;
            };

            let Some(project_track) = project.tracks().get(track_index) else {
                break;
            };

            if track != project_track {
                *track = project_track.clone();
            }
        }
    }

    fn update_project(s: &mut TrackUIState, project: &Project) {
        for (track_index, track) in s.tracks.iter().enumerate() {
            if let Some(project_track) = project.tracks().get(track_index) {
                if track != project_track {
                    project.push_event(ProjectEvent::UpdateTrack {
                        index: track_index,
                        new_track: Some(Box::new(track.clone())),
                    })
                }
            } else {
                project.push_event(ProjectEvent::UpdateTrack {
                    index: track_index,
                    new_track: Some(Box::new(track.clone())),
                })
            }
        }

        for track_index in s.tracks.len()..project.tracks().len() {
            project.push_event(ProjectEvent::UpdateTrack {
                index: track_index,
                new_track: None,
            })
        }
    }

    fn shallow_clone(old_chain_id: &mut Option<u32>, project: &Project) {
        // get the chain
        let Some(old_chain_id) = old_chain_id else {
            return;
        };

        let Some(chain) = project.chains().get(&old_chain_id) else {
            return;
        };

        // clone the thing
        let new_chain = Box::new(chain.clone());
        let id = Project::get_unique_key(project.chains());
        project.push_event(ProjectEvent::UpdateChain { id, new_chain });

        // put the cloned thing there
        *old_chain_id = id;
    }

    fn deep_clone(old_chain_id_opt: &mut Option<u32>, project: &Project) {
        // get the chain
        let Some(old_chain_id) = old_chain_id_opt else {
            return;
        };

        // shallow clone the chain
        let Some(mut new_chain) = project.chains().get(&old_chain_id).cloned() else {
            return;
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

        *old_chain_id_opt = Some(new_chain_id);
    }
}
