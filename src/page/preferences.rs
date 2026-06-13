use eframe::egui::{self, Ui};
use egui::{Key, RichText};

use crate::{
    app_ui_state::AppUIState,
    font_styling::FontStylingEx as _,
    keybinds::Keybinds,
    page::Page,
    preferences::{Colours, General},
    project::Project,
};

type RebindFunc = Box<dyn Fn(Keybinds, Key) -> Keybinds>;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum Tab {
    #[default]
    General,
    Keybinds,
    Styling,
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
        let tabs = [Tab::General, Tab::Keybinds, Tab::Styling];
        ui.horizontal(|ui| {
            for tab in tabs {
                if ui
                    .selectable_label(self.current_tab == tab, format!("{tab:?}"))
                    .clicked()
                {
                    self.current_tab = tab;
                }
            }
        });

        ui.separator();

        egui::ScrollArea::both().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| match self.current_tab {
                    Tab::General => self.general_tab(ui, ui_state),
                    Tab::Keybinds => self.keybinds_tab(ui, ui_state),
                    Tab::Styling => self.style_tab(ui, ui_state),
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
        egui::Grid::new("general_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(RichText::new("Setting").bold_ex());
                ui.label(RichText::new("Value").bold_ex());
                ui.end_row();

                // notification time
                let mut notifications_show_unlimited =
                    ui_state.preferences().general.notification_time.is_none();

                ui.label("Notification Time (s)");
                ui.checkbox(&mut notifications_show_unlimited, "Unlimited");

                if notifications_show_unlimited
                    != ui_state.preferences().general.notification_time.is_none()
                {
                    ui_state.modify_preferences(|prefs| {
                        prefs.general.notification_time = if notifications_show_unlimited {
                            None
                        } else {
                            Some(5.0)
                        }
                    });
                }

                if let Some(time) = ui_state.preferences().general.notification_time {
                    let mut new_time = time;
                    ui.add(
                        egui::DragValue::new(&mut new_time)
                            .speed(0.01)
                            .range(0.1..=60.0),
                    );

                    if time != new_time {
                        ui_state.modify_preferences(|prefs| {
                            prefs.general.notification_time = Some(new_time);
                        });
                    }
                }
                ui.end_row();
            });

        if ui.button("Reset to Default").clicked() {
            ui_state.modify_preferences(|prefs| prefs.general = General::default());
        }
    }
    fn keybinds_tab(&mut self, ui: &mut Ui, ui_state: &mut AppUIState) {
        egui::Grid::new("keybinds_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(RichText::new("Action").bold_ex());
                ui.label(RichText::new("Key").bold_ex());
                ui.end_row();

                let kb = &ui_state.preferences().keybinds;
                macro_rules! keybind_row {
                    ($name:literal,$key:ident) => {
                        let rebind_func = Box::new(|kb, new_key| Keybinds {
                            $key: new_key,
                            ..kb
                        });

                        self.keybind_row(ui, $name, kb.$key, rebind_func);
                    };
                }

                keybind_row!("Up", up);
                keybind_row!("Down", down);
                keybind_row!("Left", left);
                keybind_row!("Right", right);
                keybind_row!("Increase", increase);
                keybind_row!("Decrease", decrease);
                keybind_row!("Play/Pause", play_pause);
                keybind_row!("Show Tracks", show_tracks);
                keybind_row!("Show Chains", show_chains);
                keybind_row!("Show Phrases", show_phrases);
                keybind_row!("Show Instruments", show_instruments);
                keybind_row!("Show Preferences", show_preferences);
                keybind_row!("Trigger Cell", trigger_cell);
            });

        if ui.button("Reset to Default").clicked() {
            ui_state.modify_preferences(|prefs| prefs.keybinds = Keybinds::default());
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
            self.recording_key = Some(rebind_func);
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
                            prefs.keybinds = rebind_func(prefs.keybinds.clone(), *key);
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

    fn style_tab(&mut self, ui: &mut Ui, ui_state: &mut AppUIState) {
        self.general_styling(ui, ui_state);

        ui.separator();

        self.colours(ui, ui_state);
    }

    fn general_styling(&mut self, ui: &mut Ui, ui_state: &mut AppUIState) {
        ui.label(RichText::new("General Styling").subheading_ex());
        let mut style = ui_state.preferences().style.clone();

        egui::Grid::new("style_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label(RichText::new("Setting").bold_ex());
                ui.label(RichText::new("Value").bold_ex());
                ui.end_row();

                // ui scale
                ui.label("UI Scale");
                egui::ComboBox::from_id_salt("UI Scale")
                    .selected_text(format!("{:.2}", style.ui_scale))
                    .show_ui(ui, |ui| {
                        for scale in [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 3.0] {
                            ui.selectable_value(
                                &mut style.ui_scale,
                                scale,
                                format!("{scale:.2}"),
                            );
                        }
                    });
                ui.end_row();

                // rounded corners
                ui.label("Rounded Corners");
                ui.checkbox(&mut style.rounded_corners, "Enabled");
                ui.end_row();
            });

        if style != ui_state.preferences().style {
            ui_state.modify_preferences(|prefs| prefs.style = style);
        }
    }

    fn colours(&mut self, ui: &mut Ui, ui_state: &mut AppUIState) {
        ui.label(RichText::new("Colours").subheading_ex());
        egui::Grid::new("colours_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                let mut colours = ui_state.preferences().style.colours.clone();

                ui.label(RichText::new("Element").bold_ex());
                ui.label(RichText::new("Colour").bold_ex());
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

                if colours != ui_state.preferences().style.colours {
                    ui_state.modify_preferences(|prefs| prefs.style.colours = colours);
                }
            });

        if ui.button("Reset to Default").clicked() {
            ui_state.modify_preferences(|prefs| prefs.style.colours = Colours::default());
        }
    }
}
