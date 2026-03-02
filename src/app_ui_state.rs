use std::path::PathBuf;

use egui::{Align2, WidgetText};
use egui_phosphor::regular;
use egui_toast::{Toast, ToastKind, ToastOptions, Toasts};

use crate::{page::PageID, preferences::Preferences, synth::Player};

#[derive(Default)]
pub struct AppUIState {
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

    toasts: Toasts,
    app_preferences: Preferences,
}

impl AppUIState {
    pub fn new() -> Self {
        Self {
            app_preferences: Preferences::load(),
            toasts: Toasts::new()
                .anchor(Align2::RIGHT_TOP, (-10., 10.))
                .order(egui::Order::Tooltip),
            ..Default::default()
        }
    }

    pub fn preferences(&self) -> &Preferences {
        &self.app_preferences
    }

    pub fn modify_preferences(&mut self, f: impl FnOnce(&mut Preferences)) {
        f(&mut self.app_preferences);
        confy::store("mintracker", "config", &self.app_preferences).unwrap();
    }

    pub fn add_toast(&mut self, kind: ToastKind, msg: impl Into<WidgetText>) {
        self.toasts.add(Toast {
            kind,
            text: msg.into(),
            options: ToastOptions::default().duration_in_seconds(5.0),
            style: egui_toast::ToastStyle {
                info_icon: regular::INFO.into(),
                warning_icon: regular::WARNING.into(),
                error_icon: regular::X_CIRCLE.into(),
                success_icon: regular::CHECK.into(),
                close_button_text: regular::X.into(),
            },
        });
    }

    pub fn show_toasts(&mut self, ui: &mut egui::Ui) {
        self.toasts.show(ui.ctx());
    }
}
