use std::{f32::consts::PI, fs::File, io::BufReader};

use eframe::egui::{
    pos2, vec2, Color32, ComboBox, DragValue, PointerButton, Sense, Stroke, Ui, WidgetInfo,
    WidgetType, Window,
};
use rodio::Decoder;

use crate::{
    helpers,
    project::{InstrumentDataTable, InstrumentVariant, SamplePoint, MID_A_FREQUENCY},
};

#[derive(Clone, PartialEq)]
struct ResizeWindow {
    pub new_size: usize,
}

#[derive(Clone, PartialEq)]
struct MultiplyWindow {
    pub multiplier: f32,
}

#[derive(Default, Clone, PartialEq)]
pub struct WaveformGraph {
    resize_window: Option<ResizeWindow>,
    multiply_window: Option<MultiplyWindow>,
}

impl WaveformGraph {
    pub fn waveform_graph(&mut self, ui: &mut Ui, data_table: &mut InstrumentDataTable) {
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                self.draw_controls(ui, data_table);
            });
            Self::draw_waveform_graph(ui, data_table);
            self.draw_windows(ui, data_table);
            Self::draw_info(ui, &*data_table);
        });
    }

    fn set_sine_wave(data_table: &mut InstrumentDataTable) {
        let max_i = (data_table.data.len() - 1) as f32;
        data_table.data.iter_mut().enumerate().for_each(|(i, val)| {
            *val = (f32::sin((2.0 * PI * i as f32) / max_i)).into();
        });
    }

    fn set_sawtooth_wave(data_table: &mut InstrumentDataTable) {
        let max_i = (data_table.data.len() - 1) as f32;
        for (i, val) in data_table.data.iter_mut().enumerate() {
            *val = (i as f32 * (2.0 / max_i) - 1.0).into();
        }
    }

    fn set_triangle_wave(data_table: &mut InstrumentDataTable) {
        let max_i = (data_table.data.len() - 1) as f32;
        let first_quartile = max_i / 4.0;
        let third_quartile = 3.0 * (max_i / 4.0);

        data_table.data.iter_mut().enumerate().for_each(|(i, val)| {
            let i = i as f32;

            let gradient = 1.0 / first_quartile;
            if i < first_quartile {
                *val = (gradient * i).into();
            } else if i < third_quartile {
                *val = (1.0 + (-gradient * (i - first_quartile))).into();
            } else {
                *val = ((gradient * (i - third_quartile)) - 1.0).into();
            }
        });
    }

    fn draw_controls(&mut self, ui: &mut Ui, data_table: &mut InstrumentDataTable) {
        ui.label("Wavetable Name");
        ui.text_edit_singleline(&mut data_table.name);

        ui.end_row();

        ui.menu_button("Presets", |ui| {
            if ui.button("Sine Wave").clicked() {
                ui.close();
                Self::set_sine_wave(data_table);
            }
            if ui.button("Sawtooth Wave").clicked() {
                ui.close();
                Self::set_sawtooth_wave(data_table);
            }
            if ui.button("Triangle Wave").clicked() {
                ui.close();
                Self::set_triangle_wave(data_table);
            }
        });

        ui.menu_button("Transform", |ui| {
            if ui.button("Resize").clicked() {
                ui.close();
                self.resize_window = Some(ResizeWindow {
                    new_size: data_table.data.len(),
                });
            }
            if ui.button("Multiply").clicked() {
                ui.close();
                self.multiply_window = Some(MultiplyWindow { multiplier: 1.0 });
            }
            if ui.button("Normalise").clicked() {
                ui.close();
                let multiplier = data_table.data.iter().fold(f32::INFINITY, |acc, &point| {
                    acc.min(1.0 / point.value().abs())
                });

                data_table
                    .data
                    .iter_mut()
                    .for_each(|point| *point = (point.value() * multiplier).into());
            }
        });

        let selected_label = match data_table.variant {
            InstrumentVariant::Normal => "Normal",
            InstrumentVariant::OneShot => "OneShot",
            InstrumentVariant::OneShotPitched(_) => "OneShot (Pitched)",
        };

        ComboBox::from_id_salt("instrument_variant_combobox")
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                let variant = &mut data_table.variant;
                ui.selectable_value(variant, InstrumentVariant::Normal, "Normal");
                ui.selectable_value(variant, InstrumentVariant::OneShot, "OneShot");
                ui.selectable_value(
                    variant,
                    InstrumentVariant::OneShotPitched(MID_A_FREQUENCY),
                    "OneShot (Pitched)",
                );
            });

        if let InstrumentVariant::OneShotPitched(frequency) = &mut data_table.variant {
            ui.horizontal(|ui| {
                ui.label("Frequency");
                ui.add(DragValue::new(frequency));
            });
        }

        if ui.button("Load from WAV file").clicked()
            && let Some(dt) = Self::load_wav_file() {
                *data_table = dt;
            }
    }

    fn draw_windows(&mut self, ui: &mut Ui, data_table: &mut InstrumentDataTable) {
        // resize window
        if let Some(resize_window) = &mut self.resize_window {
            let mut should_show = true;
            Window::new("Resize").show(ui.ctx(), |ui| {
                ui.add(
                    DragValue::new(&mut resize_window.new_size)
                        .range(0_usize..=(u16::MAX as usize))
                        .speed(1),
                );

                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        should_show = false;
                        if resize_window.new_size != data_table.data.len() {
                            let step = data_table.data.len() as f32 / resize_window.new_size as f32;
                            data_table.data = (0..resize_window.new_size)
                                .map(|i| {
                                    helpers::linear_interpolate(&data_table.data, i as f32 * step)
                                })
                                .collect::<Vec<SamplePoint>>();
                        }
                    }

                    if ui.button("Cancel").clicked() {
                        should_show = false;
                    }
                });
            });

            if !should_show {
                self.resize_window = None;
            }
        }

        // multiply window
        if let Some(multiply_window) = &mut self.multiply_window {
            let mut should_show = true;
            Window::new("Multiply").show(ui.ctx(), |ui| {
                ui.add(
                    DragValue::new(&mut multiply_window.multiplier)
                        .range(0.0..=f32::INFINITY)
                        .speed(0.01),
                );

                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        should_show = false;
                        data_table.data.iter_mut().for_each(|point| {
                            *point = (point.value() * multiply_window.multiplier).into();
                        });
                    }

                    if ui.button("Cancel").clicked() {
                        should_show = false;
                    }
                });
            });

            if !should_show {
                self.multiply_window = None;
            }
        }
    }

    fn draw_waveform_graph(ui: &mut Ui, data_table: &mut InstrumentDataTable) {
        // determining size
        let desired_width = f32::min(ui.available_width(), 400.0);
        let desired_size = vec2(desired_width, desired_width * 0.6);

        // allocating space
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::drag());

        // handle interactions
        let point_spacing = match data_table.data.len() {
            0 | 1 => 0.0,
            _ => rect.width() / (data_table.data.len() - 1) as f32,
        };

        // handle mouse editing
        if data_table.data.len() > 1
            && data_table.variant == InstrumentVariant::Normal
            && response.dragged_by(PointerButton::Primary)
            && let Some(mouse_pos) = response.interact_pointer_pos() {
                // edit graph
                let point_idx = ((mouse_pos.x - rect.left()) / point_spacing).round() as usize;
                let value =
                    ((rect.center().y - mouse_pos.y) / rect.height() * 2.0).clamp(-1.0, 1.0);

                if let Some(point) = data_table.data.get_mut(point_idx) {
                    *point = value.into();
                }
            }

        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::Other, ui.is_enabled(), response.hovered(), "")
        });

        // paint widget
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, Color32::from_rgb(0, 0, 0));

            if data_table.data.is_empty() {
                return;
            }

            if data_table.data.len() == 1 {
                painter.circle_filled(rect.center(), 2.0, Color32::from_rgb(255, 255, 255));
                return;
            }

            // mark middle (horizontal line)
            painter.line_segment(
                [rect.left_center(), rect.right_center()],
                Stroke {
                    width: 1.0,
                    color: Color32::from_rgb(128, 128, 128),
                },
            );

            // mark middle (vertical line)
            painter.line_segment(
                [rect.center_top(), rect.center_bottom()],
                Stroke {
                    width: 1.0,
                    color: Color32::from_rgb(128, 128, 128),
                },
            );

            // draw graph
            let mut prev_pos = pos2(0.0, 0.0);

            let step = match data_table.variant {
                InstrumentVariant::Normal => 1,
                InstrumentVariant::OneShot => std::cmp::max(data_table.data.len() / 500, 1),
                InstrumentVariant::OneShotPitched(_) => {
                    std::cmp::max(data_table.data.len() / 500, 1)
                }
            };

            for i in (0..data_table.data.len()).step_by(step) {
                let x_pos = (i as f32 * point_spacing).clamp(0.0, rect.width()) + rect.left();
                let y_pos = rect.center().y - (data_table.data[i].value() * rect.height() / 2.0);

                let current_pos = pos2(x_pos, y_pos);

                if i == 0 {
                    prev_pos = pos2(x_pos, y_pos);
                } else {
                    painter.line_segment(
                        [prev_pos, current_pos],
                        Stroke {
                            width: 1.0,
                            color: Color32::from_rgb(255, 255, 255),
                        },
                    );
                    prev_pos = current_pos;
                }
            }
        }
    }

    fn load_wav_file() -> Option<InstrumentDataTable> {
        let path = rfd::FileDialog::new()
            .add_filter("wav", &["wav"])
            .pick_file()
            .map(|mut f| {
                f.set_extension("wav");
                f
            })?;

        let file = BufReader::new(File::open(path).ok()?);
        let data = Decoder::new(file)
            .ok()?
            .map(Into::into)
            .collect::<Vec<SamplePoint>>();

        Some(InstrumentDataTable {
            data,
            variant: InstrumentVariant::OneShotPitched(MID_A_FREQUENCY),
            ..Default::default()
        })
    }

    fn draw_info(ui: &mut Ui, data_table: &InstrumentDataTable) {
        ui.horizontal(|ui| ui.label(format!("Num. Samples: {}", data_table.data.len())));
    }
}
