use std::{
    error::Error,
    fs::File,
    sync::{mpsc, Arc, RwLock},
};

use crate::{
    app_ui_state::{AppUIState, PageID},
    preferences_ui::PreferencesUI,
};
use chain_ui::ChainUI;
use eframe::{
    egui::{self, Button, Color32, DragValue, Key, Separator, Ui, ViewportCommand},
    App,
};
use egui::{Align, Layout, Vec2};
use instrument_ui::InstrumentUI;
use phrase_ui::PhraseUI;
use project::{Project, ProjectEvent, ProjectSettings};
use synth::{PlayerCmd, ROProject};
use track_ui::TrackUI;

mod app_preferences;
mod app_ui_state;
mod chain_ui;
mod effects_menu;
mod file_handling;
mod helpers;
mod instrument_ui;
mod phrase_ui;
mod preferences_ui;
mod project;
mod selection;
mod synth;
mod track_ui;
mod widget;

fn main() -> eframe::Result {
    dioxus_devtools::connect_subsecond();
    //env_logger::init();

    let options = eframe::NativeOptions {
        ..Default::default()
    };

    eframe::run_native(
        "MinTracker",
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::<MinTracker>::default())
        }),
    )
}

trait Page {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project);
    fn draw_side_buttons(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project);
    fn handle_undo(&mut self, project: &Project);
    fn play(&self, _state: &AppUIState, _project: ROProject) {}
}

struct MinTracker {
    ui_state: AppUIState,
    project: Arc<RwLock<Project>>,

    track_ui: Box<dyn Page>,
    chain_ui: Box<dyn Page>,
    phrase_ui: Box<dyn Page>,
    instrument_ui: Box<dyn Page>,

    preferences_ui: Box<dyn Page>,

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
            PageID::Preferences => &mut $mintracker.preferences_ui,
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
            PageID::Preferences => &$mintracker.preferences_ui,
        }
    };
}

impl Default for MinTracker {
    fn default() -> Self {
        Self {
            ui_state: AppUIState::new(),
            project: Default::default(),

            track_ui: Box::<TrackUI>::default(),
            chain_ui: Box::<ChainUI>::default(),
            phrase_ui: Box::<PhraseUI>::default(),
            instrument_ui: Box::<InstrumentUI>::default(),
            preferences_ui: Box::<PreferencesUI>::default(),

            show_exit_dialog: false,
            force_close: false,
        }
    }
}

impl App for MinTracker {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        subsecond::call(|| {
            {
                let changed = self.project.write().unwrap().handle_events();
                if changed {
                    self.ui_state.project_dirty = true;
                }
            }

            let pix_per_point = ctx.native_pixels_per_point().unwrap_or(1.0)
                * self.ui_state.app_preferences().ui_scale;
            ctx.set_pixels_per_point(pix_per_point);

            if self.ui_state.player.is_playing() {
                ctx.request_repaint();
            }

            egui::CentralPanel::default().show(ctx, |ui| {
                Self::set_style(ui);
                self.handle_global_keybinds(ui);

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
                && self.ui_state.project_dirty
                && !self.force_close
            {
                self.show_exit_dialog = true;
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }

            if self.show_exit_dialog {
                self.exit_dialog(ctx);
            }
        });
    }
}

impl MinTracker {
    fn handle_global_keybinds(&mut self, ui: &mut Ui) {
        // handle audio
        if ui.input(|i| i.key_pressed(self.ui_state.app_preferences().keybinds.play_pause)) {
            if self.ui_state.player.is_playing() {
                self.ui_state.player.send_command(PlayerCmd::Stop);
            } else {
                self.play_viewed();
            }
        }

        // handle page switching
        ui.input(|ui| {
            let kb = &self.ui_state.app_preferences().keybinds;
            let mut new = None;
            ui.key_pressed(kb.show_tracks)
                .then(|| new = Some(PageID::Track));
            ui.key_pressed(kb.show_chains)
                .then(|| new = Some(PageID::Chain));
            ui.key_pressed(kb.show_phrases)
                .then(|| new = Some(PageID::Phrase));
            ui.key_pressed(kb.show_instruments)
                .then(|| new = Some(PageID::Instrument));
            ui.key_pressed(kb.show_preferences)
                .then(|| new = Some(PageID::Preferences));

            if let Some(new_page) = new {
                self.ui_state.current_page = new_page;
            }
        });

        // handle save and load,
        ui.input(|i| {
            if i.key_pressed(Key::S) && i.modifiers.ctrl {
                let _ = self.save();
            }
            if i.key_pressed(Key::O) && i.modifiers.ctrl {
                let _ = self.load();
            }
        });
    }

    fn exit_dialog(&mut self, ctx: &egui::Context) {
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

    fn sidepanel(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.with_layout(Layout::top_down(Align::Min), |ui| self.button_panel(ui));
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| self.pages_panel(ui));
        });
    }

    fn button_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui
                .button(egui_phosphor::regular::FOLDER)
                .on_hover_text("Open")
                .clicked()
            {
                loop {
                    let res = self.load();
                    if let Err(e) = res {
                        eprintln!("Error: {}", e);
                    } else {
                        break;
                    }
                }
            }
            if ui
                .button(egui_phosphor::regular::FLOPPY_DISK)
                .on_hover_text("Save")
                .clicked()
            {
                loop {
                    let res = self.save();
                    if let Err(e) = res {
                        eprintln!("Error: {}", e);
                    } else {
                        break;
                    }
                }
            }

            if ui
                .button(egui_phosphor::regular::BROOM)
                .on_hover_text("Clean Project")
                .clicked()
            {
                self.project
                    .read()
                    .unwrap()
                    .push_event(ProjectEvent::CleanUnusedNotes);
            }
        });

        ui.add_sized([10.0, 10.0], Separator::default().horizontal());

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

        let player_active = self.ui_state.player.is_playing();
        let is_paused = {
            let (tx, rx) = mpsc::channel();
            self.ui_state
                .player
                .send_command(PlayerCmd::RequestIsPaused(tx));

            rx.recv().unwrap_or(false)
        };

        ui.horizontal(|ui| {
            if player_active && !is_paused && ui.small_button("⏸").clicked() {
                self.ui_state.player.send_command(PlayerCmd::Pause);
            }

            if (!player_active || is_paused) && ui.small_button("⏵").clicked() {
                if player_active {
                    self.ui_state.player.send_command(PlayerCmd::Resume);
                } else {
                    self.play_viewed();
                }
            }

            if ui.small_button("⏹").clicked() {
                self.ui_state.player.send_command(PlayerCmd::Stop);
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

        get_page_box_mut!(self, self.ui_state.current_page).draw_side_buttons(
            ui,
            &mut self.ui_state,
            &proj,
        );
    }

    fn pages_panel(&mut self, ui: &mut Ui) {
        let pages = [
            ("Tracks", PageID::Track),
            ("Chains", PageID::Chain),
            ("Phrases", PageID::Phrase),
            ("Instruments", PageID::Instrument),
        ];
        let pages2 = [("Preferences", PageID::Preferences)];

        for (label, page_id) in pages2.into_iter().rev() {
            ui.selectable_value(&mut self.ui_state.current_page, page_id, label);
        }
        ui.add_sized([10.0, 10.0], Separator::default().horizontal());
        for (label, page_id) in pages.into_iter().rev() {
            ui.selectable_value(&mut self.ui_state.current_page, page_id, label);
        }
    }

    fn update_page(&mut self, ui: &mut Ui) {
        let page = get_page_box_mut!(self, self.ui_state.current_page);

        if ui.input(|i| i.key_pressed(Key::Z) && i.modifiers.ctrl) {
            page.handle_undo(&self.project.read().unwrap());
        }

        ui.heading(format!(
            "{} {}",
            self.ui_state.current_page.str(),
            match self.ui_state.current_page {
                PageID::Track => "".to_string(),
                PageID::Chain => self
                    .ui_state
                    .viewed_chain
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                PageID::Phrase => self
                    .ui_state
                    .viewed_phrase
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                PageID::Instrument => self
                    .ui_state
                    .viewed_instrument
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                PageID::Preferences => {
                    "".to_string()
                }
            }
        ));
        ui.separator();

        page.update(ui, &mut self.ui_state, &self.project.read().unwrap());
    }

    fn save(&mut self) -> Result<(), Box<dyn Error>> {
        if self.ui_state.filepath.is_none() {
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

            self.ui_state.filepath = new_filepath;
        }

        let Some(path) = &self.ui_state.filepath else {
            return Ok(());
        };

        let file = File::create(path)?;

        serde_cbor::to_writer(file, &self.project)?;
        self.ui_state.project_dirty = false;

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
        self.ui_state.filepath = Some(path);
        self.ui_state.project_dirty = false;
        Ok(())
    }

    fn set_style(ui: &mut Ui) {
        ui.spacing_mut().item_spacing = Vec2 { x: 2.0, y: 2.0 };

        let style = ui.style_mut();
        style.interaction.selectable_labels = false;
        style.animation_time = 0.0;
    }

    fn play_viewed(&self) {
        let page = get_page_box!(self, self.ui_state.current_page);
        page.play(&self.ui_state, ROProject::new(self.project.clone()));
    }
}
