use eframe::egui::{self, Ui};
use egui::{Key, RichText};

use crate::{
    app_preferences::Colours, app_ui_state::AppUIState, keybinds::Keybinds, project::Project, Page,
};

type RebindFunc = Box<dyn Fn(Keybinds, Key) -> Keybinds>;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum Tab {
    #[default]
    General,
    Keybinds,
    Colours,
}

#[derive(Default)]
pub struct PreferencesUI {
    current_tab: Tab,
    recording_key: Option<RebindFunc>,
}

impl Page for PreferencesUI {
    fn update(&mut self, ui: &mut Ui, ui_state: &mut AppUIState, _project: &Project) {
        if self.recording_key.is_some() {
            self.recording_keybind(ui, ui_state);
            return;
        }

        // use selectable labels to simulate tabs
        let tabs = [Tab::General, Tab::Keybinds, Tab::Colours];
        ui.horizontal(|ui| {
            for tab in tabs {
                if ui
                    .selectable_label(self.current_tab == tab, format!("{tab:?}"))
                    .clicked()
                {
                    self.current_tab = tab
                }
            }
        });

        // the following scrollarea should take all the remaining space

        egui::ScrollArea::both().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| match self.current_tab {
                    Tab::General => self.general_tab(ui, ui_state),
                    Tab::Keybinds => self.keybinds_tab(ui, ui_state),
                    Tab::Colours => self.colours_tab(ui, ui_state),
                });
            });
            ui.allocate_space(ui.available_size());
        });
    }

    fn heading(&self, _: &AppUIState) -> String {
        "Preferences".into()
    }

    fn draw_side_buttons(&mut self, _ui: &mut Ui, _state: &mut AppUIState, _project: &Project) {}
    fn handle_undo(&mut self, _project: &Project) {}
}

impl PreferencesUI {
    fn general_tab(&mut self, ui: &mut Ui, ui_state: &mut AppUIState) {
        let mut ui_scale = ui_state.app_preferences().ui_scale;
        egui::ComboBox::from_label("UI Scale")
            .selected_text(format!("{:.2}", ui_state.app_preferences().ui_scale))
            .show_ui(ui, |ui| {
                for scale in [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 3.0] {
                    ui.selectable_value(&mut ui_scale, scale, format!("{:.2}", scale));
                }
            });

        if ui_scale != ui_state.app_preferences().ui_scale {
            ui_state.modify_preferences(|prefs| prefs.ui_scale = ui_scale);
        }
    }

    fn keybinds_tab(&mut self, ui: &mut Ui, ui_state: &mut AppUIState) {
        egui::Grid::new("keybinds_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(RichText::new("Action").heading());
                ui.label(RichText::new("Key").heading());
                ui.end_row();

                let kb = &ui_state.app_preferences().keybinds;
                let rows: [(&str, Key, RebindFunc); _] = [
                    ("Up", kb.up, Box::new(|kb, up| Keybinds { up, ..kb })),
                    (
                        "Down",
                        kb.down,
                        Box::new(|kb, down| Keybinds { down, ..kb }),
                    ),
                    (
                        "Left",
                        kb.left,
                        Box::new(|kb, left| Keybinds { left, ..kb }),
                    ),
                    (
                        "Right",
                        kb.right,
                        Box::new(|kb, right| Keybinds { right, ..kb }),
                    ),
                    (
                        "Play/Pause",
                        kb.play_pause,
                        Box::new(|kb, play_pause| Keybinds { play_pause, ..kb }),
                    ),
                    (
                        "Show Tracks",
                        kb.show_tracks,
                        Box::new(|kb, show_tracks| Keybinds { show_tracks, ..kb }),
                    ),
                    (
                        "Show Chains",
                        kb.show_chains,
                        Box::new(|kb, show_chains| Keybinds { show_chains, ..kb }),
                    ),
                    (
                        "Show Phrases",
                        kb.show_phrases,
                        Box::new(|kb, show_phrases| Keybinds { show_phrases, ..kb }),
                    ),
                    (
                        "Show Instruments",
                        kb.show_instruments,
                        Box::new(|kb, show_instruments| Keybinds {
                            show_instruments,
                            ..kb
                        }),
                    ),
                    (
                        "Show Preferences",
                        kb.show_preferences,
                        Box::new(|kb, show_preferences| Keybinds {
                            show_preferences,
                            ..kb
                        }),
                    ),
                    (
                        "Trigger Cell",
                        kb.trigger_cell,
                        Box::new(|kb, trigger_cell| Keybinds { trigger_cell, ..kb }),
                    ),
                ];

                for (action_name, key, rebind_func) in rows {
                    self.keybind_row(ui, action_name, key, rebind_func);
                }
            });

        if ui.button("Reset to Default").clicked() {
            ui_state.modify_preferences(|prefs| prefs.keybinds = Keybinds::default())
        }
    }

    fn keybind_row(
        &mut self,
        ui: &mut Ui,
        action_name: &str,
        key: eframe::egui::Key,
        rebind_func: RebindFunc,
    ) {
        ui.label(action_name);
        if ui.small_button(format!("{key:?}")).clicked() {
            self.recording_key = Some(rebind_func)
        }
        ui.end_row();
    }

    fn recording_keybind(&mut self, ui: &mut Ui, ui_state: &mut AppUIState) {
        ui.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Key { key, pressed, .. } if *pressed => {
                        let Some(ref rebind_func) = self.recording_key else {
                            return;
                        };

                        ui_state.modify_preferences(|prefs| {
                            prefs.keybinds = rebind_func(prefs.keybinds.clone(), *key)
                        });

                        self.recording_key = None;
                    }
                    _ => {}
                }
            }
        });

        ui.label(RichText::new("Press a key...").italics());
        if ui.button("Cancel").clicked() {
            self.recording_key = None;
        }
    }

    fn colours_tab(&mut self, ui: &mut Ui, ui_state: &mut AppUIState) {
        egui::Grid::new("colours_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                let mut colours = ui_state.app_preferences().colours.clone();

                ui.label(RichText::new("Element").heading());
                ui.label(RichText::new("Colour").heading());
                ui.end_row();

                // text
                ui.label("Text");
                ui.color_edit_button_srgb(&mut colours.text);
                ui.end_row();

                // button
                ui.label("Button BG");
                ui.color_edit_button_srgb(&mut colours.button_bg);
                ui.end_row();

                // window
                ui.label("Window BG");
                ui.color_edit_button_srgb(&mut colours.window_bg);
                ui.end_row();

                // highlight
                ui.label("Highlight");
                ui.color_edit_button_srgb(&mut colours.highlighted);
                ui.end_row();

                if colours != ui_state.app_preferences().colours {
                    ui_state.modify_preferences(|prefs| prefs.colours = colours);
                }
            });

        if ui.button("Reset to Default").clicked() {
            ui_state.modify_preferences(|prefs| prefs.colours = Colours::default())
        }
    }
}
