use std::{
    fs::File,
    sync::{mpsc, Arc, RwLock},
};

use crate::{
    app_ui_state::AppUIState,
    page::{PageID, Pages},
};
use eframe::{
    egui::{self, Button, DragValue, Key, Separator, Ui, ViewportCommand},
    App,
};
use egui::{Align, Layout, RichText, Vec2};
use egui_phosphor::regular;
use egui_toast::ToastKind;
use project::{Project, ProjectEvent, ProjectSettings};
use synth::{PlayerCmd, ROProject};

mod app_preferences;
mod app_ui_state;
mod effects_menu;
mod file_handling;
mod helpers;
mod keybinds;
mod page;
mod project;
mod selection;
mod synth;
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
            fonts.font_data.insert(
                "Lekton-Regular".to_string(),
                egui::FontData::from_static(include_bytes!("../assets/Lekton-Regular.ttf")).into(),
            );
            fonts.font_data.insert(
                "VT323".to_string(),
                egui::FontData::from_static(include_bytes!("../assets/Lekton-Italic.ttf")).into(),
            );
            fonts.font_data.insert(
                "Lekton-Bold".to_string(),
                egui::FontData::from_static(include_bytes!("../assets/Lekton-Bold.ttf")).into(),
            );
            fonts.font_data.insert(
                "Lekton-Italic".to_string(),
                egui::FontData::from_static(include_bytes!("../assets/Lekton-Italic.ttf")).into(),
            );

            fonts.families.insert(
                egui::FontFamily::Proportional,
                vec!["Lekton-Regular".into()],
            );
            fonts.families.insert(
                egui::FontFamily::Name("Bold".into()),
                vec!["Lekton-Bold".into()],
            );
            fonts.families.insert(
                egui::FontFamily::Name("Italic".into()),
                vec!["Lekton-Italic".into()],
            );
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::<MinTracker>::default())
        }),
    )
}

struct MinTracker {
    ui_state: AppUIState,
    project: Arc<RwLock<Project>>,

    pages: Pages,

    show_exit_dialog: bool,
    force_close: bool,
}

impl Default for MinTracker {
    fn default() -> Self {
        Self {
            ui_state: AppUIState::new(),
            project: Default::default(),

            pages: Pages::new(),

            show_exit_dialog: false,
            force_close: false,
        }
    }
}

impl App for MinTracker {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        subsecond::call(|| {
            {
                let (changed, messages) = self.project.write().unwrap().handle_events();
                if changed {
                    self.ui_state.project_dirty = true;
                }

                // for msg in messages {
                //     self.ui_state.toasts.info(msg);
                // }

                for msg in messages {
                    self.ui_state.add_toast(ToastKind::Info, msg);
                }
            }

            let pix_per_point = ctx.native_pixels_per_point().unwrap_or(1.0)
                * self.ui_state.app_preferences().ui_scale;
            ctx.set_pixels_per_point(pix_per_point);

            if self.ui_state.player.is_playing() {
                ctx.request_repaint()
            }

            egui::CentralPanel::default().show(ctx, |ui| {
                self.ui_state
                    .app_preferences()
                    .colours
                    .apply(&self.ui_state, ctx);
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

                self.ui_state.show_toasts(ui);
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
                self.save()
            }
            if i.key_pressed(Key::O) && i.modifiers.ctrl {
                self.load()
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
                        self.save();
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
                self.load()
            }
            if ui
                .button(egui_phosphor::regular::FLOPPY_DISK)
                .on_hover_text("Save")
                .clicked()
            {
                self.save()
            }
            if ui
                .button(egui_phosphor::regular::BROOM)
                .on_hover_text("Clean Project")
                .clicked()
            {
                self.project
                    .read()
                    .unwrap()
                    .push_event(ProjectEvent::CleanUnusedNotes)
            }
        });

        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    self.ui_state.current_page == PageID::Preferences,
                    regular::SLIDERS,
                )
                .on_hover_text("Preferences")
                .clicked()
            {
                self.ui_state.current_page = PageID::Preferences
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
            if player_active
                && !is_paused
                && ui
                    .small_button(regular::PAUSE)
                    .on_hover_text("Pause")
                    .clicked()
            {
                self.ui_state.player.send_command(PlayerCmd::Pause);
            }

            if (!player_active || is_paused)
                && ui
                    .small_button(regular::PLAY)
                    .on_hover_text("Play")
                    .clicked()
            {
                if player_active {
                    self.ui_state.player.send_command(PlayerCmd::Resume);
                } else {
                    self.play_viewed();
                }
            }

            if ui
                .add_enabled(player_active, Button::new(regular::STOP).small())
                .on_hover_text("Stop")
                .clicked()
            {
                self.ui_state.player.send_command(PlayerCmd::Stop);
            }

            let loop_button = Button::selectable(settings.loop_player, regular::REPEAT).small();
            if ui.add(loop_button).clicked() {
                proj.push_event(ProjectEvent::UpdateSettings(ProjectSettings {
                    loop_player: !settings.loop_player,
                    ..settings.clone()
                }));
            };
        });

        ui.add_sized([10.0, 10.0], Separator::default().horizontal());

        self.pages
            .page_mut(self.ui_state.current_page)
            .draw_side_buttons(ui, &mut self.ui_state, &proj);
    }

    fn pages_panel(&mut self, ui: &mut Ui) {
        let pages = [
            ("Tracks", PageID::Track),
            ("Chains", PageID::Chain),
            ("Phrases", PageID::Phrase),
            ("Instruments", PageID::Instrument),
        ];

        for (label, page_id) in pages.into_iter().rev() {
            ui.selectable_value(&mut self.ui_state.current_page, page_id, label);
        }
    }

    fn update_page(&mut self, ui: &mut Ui) {
        let page = self.pages.page_mut(self.ui_state.current_page);

        if ui.input(|i| i.key_pressed(Key::Z) && i.modifiers.ctrl) {
            page.handle_undo(&self.project.read().unwrap());
        }

        ui.heading(
            RichText::new(page.heading(&self.ui_state))
                .family(egui::FontFamily::Name("Bold".into())),
        );
        ui.separator();

        page.update(ui, &mut self.ui_state, &self.project.read().unwrap());
    }

    fn save(&mut self) {
        // if no file is currently loaded
        if self.ui_state.filepath.is_none() {
            let new_filepath = rfd::FileDialog::new()
                .add_filter("cbor", &["cbor"])
                .save_file()
                .map(|mut f| {
                    f.set_extension("cbor");
                    f
                });

            if new_filepath.is_none() {
                self.ui_state.add_toast(ToastKind::Error, "Save cancelled");
                return;
            }

            self.ui_state.filepath = new_filepath;
        }

        match self
            .ui_state
            .filepath
            .as_ref()
            .ok_or("Save cancelled".to_string())
            .and_then(|path| {
                File::create(path.clone()).map_err(|e| format!("Failed to save project: {e}"))
            })
            .and_then(|file| {
                serde_cbor::to_writer(file, &self.project)
                    .map_err(|e| format!("Failed to save project: {e}"))
            }) {
            Ok(_) => {
                self.ui_state.project_dirty = false;
                self.ui_state.add_toast(ToastKind::Success, "Project saved");
            }
            Err(e) => self.ui_state.add_toast(ToastKind::Error, e),
        }
    }

    fn load(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("cbor", &["cbor"])
            .pick_file()
        else {
            self.ui_state.add_toast(ToastKind::Info, "Load cancelled");
            return;
        };

        match File::open(path.clone())
            .map_err(|e| format!("Failed to open file: {e}"))
            .and_then(|file| {
                serde_cbor::from_reader(file).map_err(|e| format!("Failed to parse project: {e}"))
            }) {
            Ok(proj) => {
                self.project = Arc::new(RwLock::new(proj));
                self.ui_state.filepath = Some(path.clone());
                self.ui_state.project_dirty = false;
                self.ui_state
                    .add_toast(ToastKind::Success, "Project loaded");
            }
            Err(e) => {
                self.ui_state.add_toast(ToastKind::Error, e);
            }
        }
    }

    fn set_style(ui: &mut Ui) {
        ui.spacing_mut().item_spacing = Vec2 { x: 2.0, y: 2.0 };

        let style = ui.style_mut();
        style.interaction.selectable_labels = false;
        style.animation_time = 0.0;
    }

    fn play_viewed(&self) {
        let page = self.pages.page(self.ui_state.current_page);
        page.play(&self.ui_state, ROProject::new(self.project.clone()));
    }
}
