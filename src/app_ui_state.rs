use std::path::PathBuf;

use egui::{Align2, WidgetText};
use egui_phosphor::regular;
use egui_toast::{Toast, ToastKind, ToastOptions, Toasts};

use crate::{cache::Cache, page::PageID, preferences::Preferences, synth::Player};

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
    preferences: Preferences,
    cache: Cache,
}

impl AppUIState {
    pub fn new() -> Self {
        Self {
            preferences: Preferences::load(),
            cache: Cache::load(),
            toasts: Toasts::new()
                .anchor(Align2::RIGHT_TOP, (-10., 10.))
                .order(egui::Order::Tooltip),
            ..Default::default()
        }
    }

    pub fn preferences(&self) -> &Preferences {
        &self.preferences
    }

    pub fn modify_preferences(&mut self, f: impl FnOnce(&mut Preferences)) {
        f(&mut self.preferences);
        self.preferences.save();
    }

    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    pub fn modify_cache(&mut self, f: impl FnOnce(&mut Cache)) {
        f(&mut self.cache);
        self.cache.save();
    }

    pub fn add_toast(&mut self, kind: ToastKind, msg: impl Into<WidgetText>) {
        let mut options = ToastOptions::default();
        if let Some(notification_time) = self.preferences.general.notification_time {
            options = options.duration_in_seconds(notification_time);
        }

        self.toasts.add(Toast {
            kind,
            text: msg.into(),
            options,
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
