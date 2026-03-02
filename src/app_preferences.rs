use egui::Context;
use serde::{Deserialize, Serialize};

use crate::{app_ui_state::AppUIState, helpers::to_colour32, keybinds::Keybinds};

#[derive(Serialize, Deserialize, Clone)]
pub struct AppPreferences {
    pub ui_scale: f32,
    pub keybinds: Keybinds,
    pub colours: Colours,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            ui_scale: 1.5,
            keybinds: Keybinds::default(),
            colours: Colours::default(),
        }
    }
}

impl AppPreferences {
    pub fn load() -> Self {
        confy::load("mintracker", "config").unwrap_or_else(|_| {
            eprintln!("Failed to load config, using defaults");
            AppPreferences::default()
        })
    }

    pub fn save(&self) {
        confy::store("mintracker", "config", self).unwrap();
    }
}

impl Drop for AppPreferences {
    fn drop(&mut self) {
        self.save();
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Colours {
    pub text: [u8; 3],
    pub button_bg: [u8; 3],
    pub window_bg: [u8; 3],
    pub highlighted: [u8; 3],
}

impl Default for Colours {
    fn default() -> Self {
        Self {
            text: [255, 255, 255],
            button_bg: [50, 50, 50],
            window_bg: [30, 30, 30],
            highlighted: [50, 150, 250],
        }
    }
}

impl Colours {
    pub fn apply(&self, ui_state: &AppUIState, ctx: &Context) {
        let colours = ui_state.app_preferences().colours.clone();

        ctx.all_styles_mut(|style| {
            subsecond::call(|| {
                style.visuals.override_text_color = Some(to_colour32(colours.text));

                style.visuals.widgets.noninteractive.weak_bg_fill = to_colour32(colours.button_bg);
                style.visuals.widgets.inactive.weak_bg_fill = to_colour32(colours.button_bg);
                style.visuals.widgets.hovered.weak_bg_fill = to_colour32(colours.button_bg);
                style.visuals.widgets.active.weak_bg_fill = to_colour32(colours.button_bg);
                style.visuals.panel_fill = to_colour32(colours.window_bg);
                style.visuals.striped = true;

                style.visuals.selection.bg_fill = to_colour32(colours.highlighted);
            });
        });
    }
}
