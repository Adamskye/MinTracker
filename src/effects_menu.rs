use std::collections::BTreeMap;

use eframe::egui::{Align, Button, DragValue, Layout, Separator, Ui};

use crate::{
    project::{
        EffectPreset, EnvelopeEffect, KillEffect, NoteEffects, PanEffect, PitchBendEffect, Project,
        ProjectEvent, SlideEffect, SoftKillEffect, VibratoEffect,
    },
    widget,
};

#[derive(Clone, PartialEq, Default)]
pub enum EffectMenuSelected {
    #[default]
    None,
    Vibrato,
    Kill,
    SoftKill,
    PitchBend,
    Slide,
    Envelope,
    Pan,
}

#[derive(Clone, PartialEq, Default)]
pub struct EffectsMenu {
    selected: EffectMenuSelected,

    preset_textbox_content: String,
}

impl EffectsMenu {
    pub fn update(&mut self, ui: &mut Ui, effects: &mut NoteEffects, project: &Project) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(80.0);

                self.effect_presets(ui, effects, project);
                self.effect_adder(ui, effects);

                ui.separator();

                self.effect_selector(ui, effects);

                ui.separator();

                ui.text_edit_singleline(&mut self.preset_textbox_content);
                if ui
                    .add_enabled(
                        !self.preset_textbox_content.is_empty(),
                        Button::new("Add Preset"),
                    )
                    .clicked()
                {
                    let name = self.preset_textbox_content.clone();
                    let effects = effects.clone();
                    let id = Project::get_unique_key(project.effect_presets());
                    project.push_event(ProjectEvent::UpdateEffectPreset {
                        id,
                        new_preset: Some(Box::new(EffectPreset { name, effects })),
                    });
                    self.preset_textbox_content = "".to_string();
                }
            });

            self.show_page(ui, effects);
        });
    }

    fn effect_selector(&mut self, ui: &mut Ui, effects: &mut NoteEffects) {
        macro_rules! effect_option {
            ($effect_var:expr,$enum_var:expr,$text:expr) => {
                if $effect_var.is_some()
                    && ui
                        .selectable_label(self.selected == $enum_var, $text)
                        .clicked()
                {
                    self.selected = $enum_var;
                }
            };
        }

        use EffectMenuSelected as ems;
        effect_option!(effects.vibrato, ems::Vibrato, "Vibrato");
        effect_option!(effects.kill, ems::Kill, "Kill");
        effect_option!(effects.soft_kill, ems::SoftKill, "Soft Kill");
        effect_option!(effects.pitch_bend, ems::PitchBend, "Pitch Bend");
        effect_option!(effects.slide, ems::Slide, "Slide");
        effect_option!(effects.envelope, ems::Envelope, "Envelope");
        effect_option!(effects.pan, ems::Pan, "Pan");
    }

    fn effect_presets(&mut self, ui: &mut Ui, effects: &mut NoteEffects, project: &Project) {
        if project.effect_presets().is_empty() {
            return;
        }

        let mut to_delete_key = None;

        let _ = ui.menu_button("Preset", |ui| {
            project.effect_presets().iter().for_each(|(key, preset)| {
                ui.horizontal_centered(|ui| {
                    if ui.small_button("❌").clicked() {
                        to_delete_key = Some(key);
                    }
                    if ui.button(format!("{} - {}", key, preset.name)).clicked() {
                        effects.add_from_other(&preset.effects);
                    }
                });
            });
        });

        if let Some(key) = to_delete_key {
            project.push_event(ProjectEvent::UpdateEffectPreset {
                id: *key,
                new_preset: None,
            });
        }
    }

    fn effect_adder(&mut self, ui: &mut Ui, effects: &mut NoteEffects) {
        macro_rules! add_effect_option {
            ($ui:ident, $effect_var:expr,$name:literal) => {
                if $effect_var.is_none() && $ui.button($name).clicked() {
                    $effect_var = Some(Default::default());
                }
            };
        }

        let _ = ui.menu_button("Add Effect", |ui| {
            add_effect_option!(ui, effects.vibrato, "Vibrato");
            add_effect_option!(ui, effects.kill, "Kill");
            add_effect_option!(ui, effects.soft_kill, "Soft Kill");
            add_effect_option!(ui, effects.pitch_bend, "Pitch Bend");
            add_effect_option!(ui, effects.slide, "Slide");
            add_effect_option!(ui, effects.envelope, "Envelope");
            add_effect_option!(ui, effects.pan, "Pan");
        });
    }

    fn show_page(&mut self, ui: &mut Ui, effects: &mut NoteEffects) {
        macro_rules! show_effects_page {
            ($ui:expr,$effect:expr, $self:ident, $page_func:ident) => {
                if let Some(x) = &mut $effect {
                    $self.$page_func($ui, x);
                } else {
                    return;
                }
            };
        }

        ui.vertical(|ui| {
            use EffectMenuSelected as ems;
            match self.selected {
                ems::None => return,
                ems::Vibrato => {
                    show_effects_page!(ui, effects.vibrato, self, vibrato_page);
                }
                ems::Kill => {
                    show_effects_page!(ui, effects.kill, self, kill_page);
                }
                ems::SoftKill => {
                    show_effects_page!(ui, effects.soft_kill, self, soft_kill_page);
                }
                ems::PitchBend => {
                    show_effects_page!(ui, effects.pitch_bend, self, pitch_bend_page);
                }
                ems::Slide => {
                    show_effects_page!(ui, effects.slide, self, slide_page);
                }
                ems::Envelope => {
                    show_effects_page!(ui, effects.envelope, self, envelope_page);
                }
                ems::Pan => {
                    show_effects_page!(ui, effects.pan, self, pan_page);
                }
            };

            if ui.button("Remove").clicked() {
                self.remove_selection(effects);
            }
        });
    }

    fn remove_selection(&mut self, effects: &mut NoteEffects) {
        use EffectMenuSelected as ems;
        match self.selected {
            ems::None => return,
            ems::Vibrato => effects.vibrato = None,
            ems::Kill => effects.kill = None,
            ems::SoftKill => effects.soft_kill = None,
            ems::PitchBend => effects.pitch_bend = None,
            ems::Slide => effects.slide = None,
            ems::Envelope => effects.envelope = None,
            ems::Pan => effects.pan = None,
        };
        self.selected = EffectMenuSelected::None;
    }

    fn vibrato_page(&mut self, ui: &mut Ui, vibrato_effect: &mut VibratoEffect) {
        ui.horizontal(|ui| {
            ui.label("Amplitude (semitones)");
            ui.add(
                DragValue::new(&mut vibrato_effect.amplitude)
                    .range(0.0..=f32::INFINITY)
                    .speed(0.01),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Speed (ms)");
            ui.add(DragValue::new(&mut vibrato_effect.speed));
        });
    }

    fn kill_page(&mut self, ui: &mut Ui, kill_effect: &mut KillEffect) {
        ui.horizontal(|ui| {
            ui.label("Delay (ticks)");
            ui.add(DragValue::new(&mut kill_effect.delay_ticks).range(0.0..=f32::INFINITY));
        });
    }

    fn soft_kill_page(&mut self, ui: &mut Ui, soft_kill_effect: &mut SoftKillEffect) {
        ui.horizontal(|ui| {
            ui.label("Delay (ticks)");
            ui.add(DragValue::new(&mut soft_kill_effect.delay_ticks).range(0.0..=f32::INFINITY));
        });
    }

    fn pitch_bend_page(&self, ui: &mut Ui, pitch_bend_effect: &mut PitchBendEffect) {
        ui.horizontal(|ui| {
            ui.label("Semitones per tick");
            ui.add(
                DragValue::new(&mut pitch_bend_effect.semitones_per_tick)
                    .range(f32::NEG_INFINITY..=f32::INFINITY),
            );
        });
    }

    fn slide_page(&self, ui: &mut Ui, slide_effect: &mut SlideEffect) {
        ui.horizontal(|ui| {
            ui.label("Time (ticks)");
            ui.add(DragValue::new(&mut slide_effect.time_ticks).range(0.0..=f32::INFINITY));
        });
    }

    fn envelope_page(&self, ui: &mut Ui, envelope_effect: &mut EnvelopeEffect) {
        widget::adsr_graph::adsr_graph(ui, &mut envelope_effect.envelope);
    }

    fn pan_page(&self, ui: &mut Ui, pan_effect: &mut PanEffect) {
        ui.horizontal(|ui| {
            ui.label("Value");
            ui.add(
                DragValue::new(&mut pan_effect.value)
                    .range(-1.0..=1.0)
                    .speed(0.1),
            );
        });
    }
}
