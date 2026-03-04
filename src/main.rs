use std::{
    fs::File,
    sync::{Arc, RwLock, mpsc},
};

use crate::{
    app_ui_state::AppUIState,
    font_styling::FontStylingEx,
    helpers::to_colour32,
    page::{PageID, Pages},
};
use eframe::{
    App,
    egui::{self, Button, DragValue, Key, Separator, Ui, ViewportCommand},
};
use egui::{Align, Color32, Context, Frame, Layout, RichText, Sense, Vec2};
use egui_phosphor::regular;
use egui_toast::ToastKind;
use project::{Project, ProjectEvent, ProjectSettings};
use synth::{PlayerCmd, ROProject};

mod app_ui_state;
mod effects_menu;
mod file_handling;
mod font_styling;
mod helpers;
mod keybinds;
mod page;
mod preferences;
mod project;
mod selection;
mod synth;
mod widget;

fn main() -> eframe::Result {
    dioxus_devtools::connect_subsecond();

    let options = eframe::NativeOptions {
        ..Default::default()
    };

    eframe::run_native(
        "MinTracker",
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "Tiny-ExtraLight".to_string(),
                egui::FontData::from_static(include_bytes!("../assets/Tiny-ExtraLight.ttf")).into(),
            );
            fonts.font_data.insert(
                "Tiny-Bold".to_string(),
                egui::FontData::from_static(include_bytes!("../assets/Tiny-Bold.ttf")).into(),
            );
            fonts.families.insert(
                egui::FontFamily::Proportional,
                vec!["Tiny-ExtraLight".into()],
            );
            fonts.families.insert(
                egui::FontFamily::Name("Bold".into()),
                vec!["Tiny-Bold".into()],
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

                for msg in messages {
                    self.ui_state.add_toast(ToastKind::Info, msg);
                }
            }

            if self.ui_state.player.is_playing() {
                ctx.request_repaint()
            }

            Self::set_style(&ctx);
            self.ui_state.preferences().style.apply(ctx);
            self.handle_global_keybinds(&ctx);

            let window_fill = to_colour32(self.ui_state.preferences().style.colours.window_bg);
            let sidepanel_fill = window_fill.linear_multiply(1.3);
            let toppanel_fill = window_fill.linear_multiply(1.15);

            let window_margin = self.ui_state.preferences().style.window_margin as i8;

            egui::SidePanel::left("side_panel")
                .resizable(false)
                .frame(
                    Frame::default()
                        .inner_margin(window_margin)
                        .fill(sidepanel_fill),
                )
                .show_separator_line(true)
                .show(ctx, |ui| {
                    self.sidepanel(ui);
                });

            egui::TopBottomPanel::top("top_panel")
                .resizable(false)
                .frame(
                    Frame::default()
                        .inner_margin(window_margin)
                        .fill(toppanel_fill),
                )
                .show_separator_line(true)
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui| {
                        let page = self.pages.page_mut(self.ui_state.current_page);
                        ui.heading(RichText::new(page.heading(&self.ui_state)).bold_ex());
                    });
                });

            egui::CentralPanel::default()
                .frame(
                    Frame::default()
                        .inner_margin(window_margin)
                        .fill(window_fill),
                )
                .show(ctx, |ui| {
                    // main area
                    ui.vertical(|ui| {
                        self.update_page(ui);

                        ui.allocate_space(ui.available_size());
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
    fn handle_global_keybinds(&mut self, ctx: &Context) {
        // handle audio
        if ctx.input(|i| i.key_pressed(self.ui_state.preferences().keybinds.play_pause)) {
            if self.ui_state.player.is_playing() {
                self.ui_state.player.send_command(PlayerCmd::Stop);
            } else {
                self.play_viewed();
            }
        }

        // handle page switching
        ctx.input(|ui| {
            let kb = &self.ui_state.preferences().keybinds;
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
        ctx.input(|i| {
            if i.key_pressed(Key::S) && i.modifiers.ctrl {
                self.save()
            }
            if i.key_pressed(Key::O) && i.modifiers.ctrl {
                self.load()
            }
        });
    }

    fn exit_dialog(&mut self, ctx: &egui::Context) {
        // background
        egui::Area::new("exit_dialog_bg".into())
            .order(egui::Order::Foreground)
            .interactable(true)
            .fixed_pos([0.0, 0.0])
            .show(ctx, |ui| {
                let rect = ui.input(|i| i.viewport_rect());
                ui.painter()
                    .rect_filled(rect, 0.0, Color32::from_black_alpha(150));
                ui.allocate_rect(rect, Sense::click());
            });

        egui::Window::new("Save Project Before Exiting?")
            .order(egui::Order::Tooltip)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            // force window to always be on top
            .show(ctx, |ui| {
                ui.label("You have unsaved changes. Do you want to save before exiting?\n");
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            self.show_exit_dialog = false;
                        }

                        if ui.button("Don't Save").clicked() {
                            self.show_exit_dialog = false;
                            self.force_close = true;
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }

                        let button = Button::new("Save").fill(to_colour32(
                            self.ui_state.preferences().style.colours.highlighted,
                        ));

                        if ui.add(button).clicked() {
                            self.save();
                            self.show_exit_dialog = false;
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                    });
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
                .on_hover_text("Open (CTRL+O)")
                .clicked()
            {
                self.load()
            }
            if ui
                .button(egui_phosphor::regular::FLOPPY_DISK)
                .on_hover_text("Save (CTRL+S)")
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

            if ui
                .selectable_label(
                    self.ui_state.current_page == PageID::Preferences,
                    regular::SLIDERS,
                )
                .on_hover_text(format!(
                    "Preferences ({:?})",
                    self.ui_state.preferences().keybinds.show_preferences
                ))
                .clicked()
            {
                self.ui_state.current_page = PageID::Preferences
            }
        });

        ui.add_space(20.0);
        //ui.add_sized([90.0, 10.0], Separator::default().horizontal());

        let proj = self.project.read().unwrap();
        let settings = proj.settings();

        let _ = ui.menu_button(format!("{} Tempo", regular::METRONOME), |ui| {
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

        let _ = ui.menu_button(format!("{} Transpose", regular::PIANO_KEYS), |ui| {
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

        ui.add_space(20.0);
        // ui.add_sized([90.0, 10.0], Separator::default().horizontal());

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
                && ui.button(regular::PAUSE).on_hover_text("Pause").clicked()
            {
                self.ui_state.player.send_command(PlayerCmd::Pause);
            }

            if (!player_active || is_paused)
                && ui.button(regular::PLAY).on_hover_text("Play").clicked()
            {
                if player_active {
                    self.ui_state.player.send_command(PlayerCmd::Resume);
                } else {
                    self.play_viewed();
                }
            }

            if ui
                .add_enabled(player_active, Button::new(regular::STOP))
                .on_hover_text("Stop")
                .clicked()
            {
                self.ui_state.player.send_command(PlayerCmd::Stop);
            }

            let loop_button = Button::selectable(settings.loop_player, regular::REPEAT);
            if ui.add(loop_button).clicked() {
                proj.push_event(ProjectEvent::UpdateSettings(ProjectSettings {
                    loop_player: !settings.loop_player,
                    ..settings.clone()
                }));
            };
        });

        ui.add_space(20.0);
        // ui.add_sized([90.0, 10.0], Separator::default().horizontal());

        self.pages
            .page_mut(self.ui_state.current_page)
            .draw_side_buttons(ui, &mut self.ui_state, &proj);
    }

    fn pages_panel(&mut self, ui: &mut Ui) {
        let kb = &self.ui_state.preferences().keybinds;
        let pages = [
            ("Tracks", PageID::Track, kb.show_tracks),
            ("Chains", PageID::Chain, kb.show_chains),
            ("Phrases", PageID::Phrase, kb.show_phrases),
            ("Instruments", PageID::Instrument, kb.show_instruments),
        ];

        for (label, page_id, keybind) in pages.into_iter().rev() {
            let tooltip = format!("{label} ({keybind:?})");
            ui.selectable_value(&mut self.ui_state.current_page, page_id, label)
                .on_hover_text(tooltip);
        }
    }

    fn update_page(&mut self, ui: &mut Ui) {
        let page = self.pages.page_mut(self.ui_state.current_page);

        if ui.input(|i| i.key_pressed(Key::Z) && i.modifiers.ctrl) {
            page.handle_undo(&self.project.read().unwrap());
        }

        // ui.heading(
        //     RichText::new(page.heading(&self.ui_state))
        //         .family(egui::FontFamily::Name("Bold".into())),
        // );
        // //ui.separator();
        // let window_margin = self.ui_state.preferences().style.window_margin as f32;
        // ui.add(Separator::default().grow(window_margin));

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

    fn set_style(ctx: &Context) {
        ctx.style_mut(|style| {
            style.spacing.item_spacing = Vec2 { x: 2.0, y: 2.0 };
            style.interaction.selectable_labels = false;
            style.animation_time = 0.0;
        });
    }

    fn play_viewed(&self) {
        let page = self.pages.page(self.ui_state.current_page);
        page.play(&self.ui_state, ROProject::new(self.project.clone()));
    }
}
