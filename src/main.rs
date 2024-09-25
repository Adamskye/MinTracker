use std::{
    borrow::Cow,
    error::Error,
    fs::File,
    path::PathBuf,
    sync::{mpsc, Arc, RwLock},
};

use chain_ui::ChainUI;
use eframe::{
    egui::{self, Button, Color32, DragValue, Key, Separator, Slider, Ui, ViewportCommand},
    App,
};
use egui::{Align, Layout, Vec2};
use instrument_ui::InstrumentUI;
use phrase_ui::PhraseUI;
use project::{Project, ProjectEvent, ProjectSettings};
use synth::{Player, PlayerCmd, ROProject};
use track_ui::TrackUI;

mod chain_ui;
mod effects_menu;
mod file_handling;
mod helpers;
mod instrument_ui;
mod phrase_ui;
mod project;
mod selection;
mod synth;
mod track_ui;
mod widget;

fn main() -> eframe::Result {
    //env_logger::init();

    let options = eframe::NativeOptions {
        ..Default::default()
    };

    eframe::run_native(
        "MinTracker",
        options,
        Box::new(|_cc| Ok(Box::<MinTracker>::default())),
    )
}

trait Page {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project);
    fn draw_side_buttons(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project);
    fn handle_undo(&mut self, project: &Project);
    fn play(&self, state: &AppUIState, project: ROProject) {}
}

#[derive(Clone, Copy, Default, PartialEq)]
enum PageID {
    #[default]
    Track,
    Chain,
    Phrase,
    Instrument,
}

impl PageID {
    fn str(&self) -> &str {
        match self {
            PageID::Track => "Project",
            PageID::Chain => "Chain",
            PageID::Phrase => "Phrase",
            PageID::Instrument => "Instrument",
        }
    }
}

#[derive(Default)]
struct AppUIState {
    pub current_page: PageID,
    pub filepath: Option<PathBuf>,
    pub player: Player,
    pub project_dirty: bool,

    pub viewed_track: Option<usize>,
    pub viewed_chain: Option<u32>,
    pub viewed_phrase: Option<u32>,
    pub viewed_instrument: Option<u32>,

    pub track_selected_row: Option<usize>,
    pub chain_selected_row: Option<usize>,
}

struct MinTracker {
    state: AppUIState,
    project: Arc<RwLock<Project>>,

    // todo: move this to AppState
    ui_scale: f32,

    track_ui: Box<dyn Page>,
    chain_ui: Box<dyn Page>,
    phrase_ui: Box<dyn Page>,
    instrument_ui: Box<dyn Page>,

    show_exit_dialog: bool,
    force_close: bool,
}

macro_rules! get_page_box_mut {
    ($mintracker:ident, $page_id:expr) => {
        match $page_id {
            PageID::Track => &mut $mintracker.track_ui,
            PageID::Chain => &mut $mintracker.chain_ui,
            PageID::Phrase => &mut $mintracker.phrase_ui,
            PageID::Instrument => &mut $mintracker.instrument_ui,
        }
    };
}

macro_rules! get_page_box {
    ($mintracker:ident, $page_id:expr) => {
        match $page_id {
            PageID::Track => &$mintracker.track_ui,
            PageID::Chain => &$mintracker.chain_ui,
            PageID::Phrase => &$mintracker.phrase_ui,
            PageID::Instrument => &$mintracker.instrument_ui,
        }
    };
}

impl Default for MinTracker {
    fn default() -> Self {
        Self {
            state: AppUIState::default(),
            project: Default::default(),
            ui_scale: 1.4,

            track_ui: Box::<TrackUI>::default(),
            chain_ui: Box::<ChainUI>::default(),
            phrase_ui: Box::<PhraseUI>::default(),
            instrument_ui: Box::<InstrumentUI>::default(),

            show_exit_dialog: false,
            force_close: false,
        }
    }
}

impl App for MinTracker {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        {
            let changed = self.project.write().unwrap().handle_events();
            if changed {
                self.state.project_dirty = true;
            }
        }

        let pix_per_point = ctx.native_pixels_per_point().unwrap_or(1.0) * self.ui_scale;
        ctx.set_pixels_per_point(pix_per_point);

        if self.state.player.is_playing() {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            Self::set_style(ui);

            // handle audio
            if ui.input(|i| i.key_pressed(Key::Space)) {
                if self.state.player.is_playing() {
                    self.state.player.send_command(PlayerCmd::Stop);
                } else {
                    self.play_viewed();
                }
            }
            self.menubar(ui);

            // main area
            ui.with_layout(egui::Layout::left_to_right(Align::Min), |ui| {
                self.sidepanel(ui);
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_width(ui.available_size().x);
                    self.update_page(ui);

                    ui.allocate_space(ui.available_size());
                });
            });
        });

        if ctx.input(|i| i.viewport().close_requested())
            && self.state.project_dirty
            && !self.force_close
        {
            self.show_exit_dialog = true;
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        }

        if self.show_exit_dialog {
            egui::Window::new("Do you want to save your project?")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            let _ = self.save();
                            self.show_exit_dialog = false;
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }

                        if ui.button("Don't Save").clicked() {
                            self.show_exit_dialog = false;
                            self.force_close = true;
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }

                        if ui.button("Cancel").clicked() {
                            self.show_exit_dialog = false;
                        }
                    });
                });
        }
    }
}

impl MinTracker {
    fn sidepanel(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.with_layout(Layout::top_down(Align::Min), |ui| self.button_panel(ui));
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| self.map_panel(ui));
        });
    }

    fn button_panel(&mut self, ui: &mut Ui) {
        let proj = self.project.read().unwrap();
        let settings = proj.settings();

        let _ = ui.menu_button("Tempo", |ui| {
            let mut tempo_value = settings.tempo;
            ui.add(DragValue::new(&mut tempo_value).range(1.0..=1000.0));

            if tempo_value != settings.tempo {
                let new_settings = ProjectSettings {
                    tempo: tempo_value,
                    ..settings.clone()
                };
                proj.push_event(ProjectEvent::UpdateSettings(new_settings));
            }
        });

        let _ = ui.menu_button("Transpose", |ui| {
            let mut transpose_value = settings.transpose;
            ui.add(DragValue::new(&mut transpose_value));

            if transpose_value != settings.transpose {
                let new_settings = ProjectSettings {
                    transpose: transpose_value,
                    ..settings.clone()
                };
                proj.push_event(ProjectEvent::UpdateSettings(new_settings));
            }
        });

        ui.add_sized([10.0, 10.0], Separator::default().horizontal());

        let player_active = self.state.player.is_playing();
        let is_paused = {
            let (tx, rx) = mpsc::channel();
            self.state
                .player
                .send_command(PlayerCmd::RequestIsPaused(tx));

            rx.recv().unwrap_or(false)
        };

        ui.label("Project Player");
        ui.horizontal(|ui| {
            if player_active && !is_paused && ui.small_button("⏸").clicked() {
                self.state.player.send_command(PlayerCmd::Pause);
            }

            if (!player_active || is_paused) && ui.small_button("⏵").clicked() {
                if player_active {
                    self.state.player.send_command(PlayerCmd::Resume);
                } else {
                    self.play_viewed();
                }
            }

            if ui.small_button("⏹").clicked() {
                self.state.player.send_command(PlayerCmd::Stop);
            }

            let loop_button = Button::new("🔁").small();
            if ui
                .add(if settings.loop_player {
                    loop_button.fill(Color32::LIGHT_BLUE)
                } else {
                    loop_button
                })
                .clicked()
            {
                proj.push_event(ProjectEvent::UpdateSettings(ProjectSettings {
                    loop_player: !settings.loop_player,
                    ..settings.clone()
                }));
            };
        });

        ui.add_sized([10.0, 10.0], Separator::default().horizontal());

        get_page_box_mut!(self, self.state.current_page).draw_side_buttons(
            ui,
            &mut self.state,
            &proj,
        );
    }

    fn map_panel(&mut self, ui: &mut Ui) {
        if ui.button("Instrument").clicked() {
            self.state.current_page = PageID::Instrument;
        }
        if ui.button("Phrase").clicked() {
            self.state.current_page = PageID::Phrase;
        }
        if ui.button("Chain").clicked() {
            self.state.current_page = PageID::Chain;
        }
        if ui.button("Project").clicked() {
            self.state.current_page = PageID::Track;
        }
    }

    fn update_page(&mut self, ui: &mut Ui) {
        let page = get_page_box_mut!(self, self.state.current_page);

        if ui.input(|i| i.key_pressed(Key::Z) && i.modifiers.ctrl) {
            page.handle_undo(&self.project.read().unwrap());
        }

        ui.heading(format!(
            "{} {}",
            self.state.current_page.str(),
            match self.state.current_page {
                PageID::Track => "".to_string(),
                PageID::Chain => self
                    .state
                    .viewed_chain
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                PageID::Phrase => self
                    .state
                    .viewed_phrase
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                PageID::Instrument => self
                    .state
                    .viewed_instrument
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
            }
        ));
        ui.separator();

        page.update(ui, &mut self.state, &self.project.read().unwrap());
    }

    fn menubar(&mut self, ui: &mut Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {
                    ui.close_menu();
                    loop {
                        let res = self.load();
                        if let Err(e) = res {
                            eprintln!("Error: {}", e);
                        } else {
                            break;
                        }
                    }
                }
                if ui.button("Save").clicked() {
                    ui.close_menu();
                    loop {
                        let res = self.save();
                        if let Err(e) = res {
                            eprintln!("Error: {}", e);
                        } else {
                            break;
                        }
                    }
                }
                if ui.button("Clean Project").clicked() {
                    ui.close_menu();
                    self.project
                        .read()
                        .unwrap()
                        .push_event(ProjectEvent::CleanUnusedNotes);
                }
            });

            ui.menu_button("View", |ui| {
                ui.menu_button("UI Scale", |ui| {
                    ui.add(
                        Slider::new(&mut self.ui_scale, 0.5..=2.0)
                            .fixed_decimals(1)
                            .step_by(0.1),
                    );
                });
            });
        });
    }

    fn save(&mut self) -> Result<(), Box<dyn Error>> {
        if self.state.filepath.is_none() {
            let new_filepath = rfd::FileDialog::new()
                .add_filter("cbor", &["cbor"])
                .save_file()
                .map(|mut f| {
                    f.set_extension("cbor");
                    f
                });

            if new_filepath.is_none() {
                return Ok(());
            }

            self.state.filepath = new_filepath;
        }

        let Some(path) = &self.state.filepath else {
            return Ok(());
        };

        let file = File::create(path)?;
        serde_cbor::to_writer(file, &self.project)?;
        self.state.project_dirty = false;

        Ok(())
    }

    fn load(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("cbor", &["cbor"])
            .pick_file()
        else {
            return Ok(());
        };

        let file = File::open(path.clone())?;

        self.project = Arc::new(RwLock::new(serde_cbor::from_reader(file)?));
        self.state.filepath = Some(path);
        self.state.project_dirty = false;
        Ok(())
    }

    fn set_style(ui: &mut Ui) {
        ui.spacing_mut().item_spacing = Vec2 { x: 2.0, y: 2.0 };

        let style = ui.style_mut();
        style.interaction.selectable_labels = false;
        style.animation_time = 0.0;
    }

    fn play_viewed(&self) {
        let page = get_page_box!(self, self.state.current_page);
        page.play(&self.state, ROProject::new(self.project.clone()));
    }
}
