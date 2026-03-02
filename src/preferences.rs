use crate::{helpers::to_colour32, keybinds::Keybinds};
use egui::{Context, FontFamily, TextStyle};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Preferences {
    pub general: General,
    pub keybinds: Keybinds,
    pub style: Style,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            keybinds: Keybinds::default(),
            style: Style::default(),
            general: General::default(),
        }
    }
}

impl Preferences {
    pub fn load() -> Self {
        confy::load("mintracker", "config").unwrap_or_else(|_| {
            eprintln!("Failed to load config, using defaults");
            Preferences::default()
        })
    }

    pub fn save(&self) {
        confy::store("mintracker", "config", self).unwrap();
    }
}

impl Drop for Preferences {
    fn drop(&mut self) {
        self.save();
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct General {
    pub notification_time: Option<f64>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            notification_time: Some(5.0),
        }
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
    fn apply(&self, ctx: &Context) {
        ctx.all_styles_mut(|style| {
            style.visuals.override_text_color = Some(to_colour32(self.text));
            style.text_styles.insert(
                TextStyle::Heading,
                egui::FontId::new(18.0, FontFamily::Name("Bold".into())),
            );

            style.text_styles.insert(
                TextStyle::Heading,
                egui::FontId::new(18.0, FontFamily::Name("Bold".into())),
            );

            style.visuals.widgets.noninteractive.weak_bg_fill = to_colour32(self.button_bg);
            style.visuals.widgets.inactive.weak_bg_fill = to_colour32(self.button_bg);
            style.visuals.widgets.hovered.weak_bg_fill = to_colour32(self.button_bg);
            style.visuals.widgets.active.weak_bg_fill = to_colour32(self.button_bg);

            style.visuals.panel_fill = to_colour32(self.window_bg);
            style.visuals.striped = true;

            style.visuals.selection.bg_fill = to_colour32(self.highlighted);
        });
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Style {
    pub rounded_corners: bool,
    pub colours: Colours,
    pub ui_scale: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            rounded_corners: false,
            colours: Colours::default(),
            ui_scale: 1.5,
        }
    }
}

impl Style {
    pub fn apply(&self, ctx: &Context) {
        self.colours.apply(ctx);

        ctx.all_styles_mut(|style| {
            subsecond::call(|| {
                let corner_radius = if self.rounded_corners { 4.0 } else { 0.0 }.into();
                style.visuals.widgets.noninteractive.corner_radius = corner_radius;
                style.visuals.widgets.inactive.corner_radius = corner_radius;
                style.visuals.widgets.hovered.corner_radius = corner_radius;
                style.visuals.widgets.active.corner_radius = corner_radius;
                style.visuals.widgets.open.corner_radius = corner_radius;
                style.visuals.window_corner_radius = corner_radius;
                style.visuals.menu_corner_radius = corner_radius;
            });
        });
    }
}
