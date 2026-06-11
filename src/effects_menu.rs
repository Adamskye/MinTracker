use eframe::egui::{Button, DragValue, Ui};
use egui::{Align, Layout, ScrollArea};
use egui_phosphor::regular::{PLUS, SLIDERS_HORIZONTAL, TRASH};

use crate::{
    project::{
        EffectPreset, EffectPresetCmd, EnvelopeEffect, KillEffect, NoteEffects, PanEffect,
        PitchBendEffect, Project, SlideEffect, SoftKillEffect, VibratoEffect,
    },
    widget,
};

#[derive(Clone, PartialEq, Default)]
pub enum EffectMenuSelected {
    #[default]
    AddEffect,
    Presets,
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
        ui.with_layout(
            Layout::left_to_right(Align::Min).with_main_justify(true),
            |ui| {
                // left
                ui.vertical(|ui| {
                    ui.set_width(120.0);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.horizontal(|ui| {
                            use EffectMenuSelected as ems;
                            ui.selectable_label(self.selected == ems::AddEffect, PLUS)
                                .on_hover_text("Add Effects")
                                .clicked()
                                .then(|| self.selected = ems::AddEffect);

                            ui.selectable_label(self.selected == ems::Presets, SLIDERS_HORIZONTAL)
                                .on_hover_text("Presets")
                                .clicked()
                                .then(|| self.selected = ems::Presets);
                        });

                        // show list of effects
                        self.effect_selector(ui, effects);
                    });

                    // adding a preset
                    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                        if ui
                            .add_enabled(
                                !self.preset_textbox_content.is_empty(),
                                Button::new("Add Preset"),
                            )
                            .clicked()
                        {
                            let id = Project::get_unique_key(project.effect_presets());
                            project.push_cmd(EffectPresetCmd::Update {
                                id,
                                new_preset: Some(Box::new(EffectPreset {
                                    name: self.preset_textbox_content.clone(),
                                    effects: effects.clone(),
                                })),
                            });
                        }

                        ui.text_edit_singleline(&mut self.preset_textbox_content)
                    });
                });

                ui.add_space(10.0);

                // right
                ScrollArea::vertical().show(ui, |ui| {
                    self.show_page(ui, effects, project);
                    ui.allocate_space(ui.available_size());
                });
            },
        );
    }

    fn effect_selector(&mut self, ui: &mut Ui, effects: &mut NoteEffects) {
        use EffectMenuSelected as ems;
        macro_rules! effect_option {
            ($effect_var:ident,$enum_var:expr,$text:expr) => {
                if effects.$effect_var.is_some()
                    && ui
                        .selectable_label(self.selected == $enum_var, $text)
                        .clicked()
                {
                    self.selected = $enum_var;
                }
            };
        }
        effect_option!(vibrato, ems::Vibrato, "Vibrato");
        effect_option!(kill, ems::Kill, "Kill");
        effect_option!(soft_kill, ems::SoftKill, "Soft Kill");
        effect_option!(pitch_bend, ems::PitchBend, "Pitch Bend");
        effect_option!(slide, ems::Slide, "Slide");
        effect_option!(envelope, ems::Envelope, "Envelope");
        effect_option!(pan, ems::Pan, "Pan");
    }

    fn effect_adder(&mut self, ui: &mut Ui, effects: &mut NoteEffects) {
        macro_rules! add_effect_option {
            ($effect_var:expr,$name:literal) => {
                if $effect_var.is_none() && ui.button($name).clicked() {
                    $effect_var = Some(Default::default());
                }
            };
        }
        add_effect_option!(effects.vibrato, "Vibrato");
        add_effect_option!(effects.kill, "Kill");
        add_effect_option!(effects.soft_kill, "Soft Kill");
        add_effect_option!(effects.pitch_bend, "Pitch Bend");
        add_effect_option!(effects.slide, "Slide");
        add_effect_option!(effects.envelope, "Envelope");
        add_effect_option!(effects.pan, "Pan");
    }

    fn show_page(&mut self, ui: &mut Ui, effects: &mut NoteEffects, project: &Project) {
        ui.vertical(|ui| {
            macro_rules! page {
                ($effect:ident, $page_func:ident) => {
                    if let Some(x) = &mut effects.$effect {
                        self.$page_func(ui, x);
                    } else {
                        return;
                    }
                };
            }

            use EffectMenuSelected as ems;
            match self.selected {
                ems::AddEffect => {
                    self.effect_adder(ui, effects);
                    return;
                }
                ems::Presets => {
                    self.preset_selector(ui, effects, project);
                }
                ems::Vibrato => page!(vibrato, vibrato_page),
                ems::Kill => page!(kill, kill_page),
                ems::SoftKill => page!(soft_kill, soft_kill_page),
                ems::PitchBend => page!(pitch_bend, pitch_bend_page),
                ems::Slide => page!(slide, slide_page),
                ems::Envelope => page!(envelope, envelope_page),
                ems::Pan => page!(pan, pan_page),
            };

            if ui.button("Remove").clicked() {
                self.remove_selection(effects);
            }
        });
    }

    fn preset_selector(&mut self, ui: &mut Ui, effects: &mut NoteEffects, project: &Project) {
        for (id, preset) in project.effect_presets() {
            ui.horizontal(|ui| {
                if ui.button(format!("{} - {}", id, preset.name)).clicked() {
                    *effects = preset.effects.clone();
                }

                if ui.button(TRASH).clicked() {
                    project.push_cmd(EffectPresetCmd::Update {
                        id: *id,
                        new_preset: None,
                    });
                }
            });
        }
    }

    fn remove_selection(&mut self, effects: &mut NoteEffects) {
        use EffectMenuSelected as ems;
        match self.selected {
            ems::AddEffect => return,
            ems::Presets => return,
            ems::Vibrato => effects.vibrato = None,
            ems::Kill => effects.kill = None,
            ems::SoftKill => effects.soft_kill = None,
            ems::PitchBend => effects.pitch_bend = None,
            ems::Slide => effects.slide = None,
            ems::Envelope => effects.envelope = None,
            ems::Pan => effects.pan = None,
        };
        self.selected = EffectMenuSelected::AddEffect;
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
