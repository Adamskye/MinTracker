use crate::{helpers::to_colour32, keybinds::Keybinds};
use egui::{Context, FontFamily, Stroke, TextStyle};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Preferences {
    pub general: General,
    pub keybinds: Keybinds,
    pub style: Style,
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
            highlighted: [30, 100, 190],
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

            style.visuals.widgets.noninteractive.bg_stroke =
                Stroke::new(1.0_f32, to_colour32(self.button_bg).linear_multiply(1.25));
            style.visuals.widgets.inactive.bg_stroke =
                Stroke::new(1.0_f32, to_colour32(self.button_bg).linear_multiply(1.25));
            style.visuals.widgets.hovered.bg_stroke =
                Stroke::new(1.0_f32, to_colour32(self.button_bg).linear_multiply(2.0));
            style.visuals.widgets.hovered.expansion = 0.0;
            style.visuals.widgets.active.bg_stroke =
                Stroke::new(1.0_f32, to_colour32(self.button_bg).linear_multiply(1.25));

            style.visuals.panel_fill = to_colour32(self.window_bg);
            style.visuals.striped = true;

            style.visuals.selection.bg_fill = to_colour32(self.highlighted);
        });
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Style {
    pub rounded_corners: bool,
    pub window_margin: i8,
    pub colours: Colours,
    pub ui_scale: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            rounded_corners: false,
            colours: Colours::default(),
            ui_scale: 1.5,
            window_margin: 8,
        }
    }
}

impl Style {
    pub fn apply(&self, ctx: &Context) {
        // general styling
        ctx.all_styles_mut(|style| {
            let corner_radius = if self.rounded_corners { 2.0 } else { 0.0 }.into();
            style.visuals.widgets.noninteractive.corner_radius = corner_radius;
            style.visuals.widgets.inactive.corner_radius = corner_radius;
            style.visuals.widgets.hovered.corner_radius = corner_radius;
            style.visuals.widgets.active.corner_radius = corner_radius;
            style.visuals.widgets.open.corner_radius = corner_radius;
            style.visuals.window_corner_radius = corner_radius;
            style.visuals.menu_corner_radius = corner_radius;
        });

        // colours
        self.colours.apply(ctx);

        // ui scale
        let pix_per_point = ctx.native_pixels_per_point().unwrap_or(1.0) * self.ui_scale;
        ctx.set_pixels_per_point(pix_per_point);
    }
}
