use std::sync::Arc;

use eframe::egui::{
    self,
    util::undoer::{Settings, Undoer},
    ComboBox, Label, TextWrapMode, Ui,
};

use crate::{
    page::Page,
    project::{Instrument, InstrumentDataTable, Note, Project, ProjectEvent, NUM_SEMITONES},
    widget::{adsr_graph, waveform_graph::WaveformGraph},
    AppUIState,
};

#[derive(PartialEq, Clone)]
struct InstrumentUIState {
    instrument_id: Option<u32>,
    name_textbox: String,

    data_tables: Vec<InstrumentDataTable>,
    graph: WaveformGraph,

    data_table_map: [Option<usize>; NUM_SEMITONES as usize],

    selected_data_table: Option<usize>,
}

impl Default for InstrumentUIState {
    fn default() -> Self {
        Self {
            instrument_id: Default::default(),
            name_textbox: Default::default(),
            data_tables: Default::default(),
            graph: Default::default(),
            data_table_map: [None; NUM_SEMITONES as usize],
            selected_data_table: Default::default(),
        }
    }
}

pub struct InstrumentUI {
    local_state: InstrumentUIState,
    undoer: Undoer<InstrumentUIState>,
}

impl Default for InstrumentUI {
    fn default() -> Self {
        Self {
            local_state: Default::default(),
            undoer: Undoer::with_settings(Settings {
                stable_time: 0.1,
                ..Default::default()
            }),
        }
    }
}

impl Page for InstrumentUI {
    fn update(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project) {
        Self::select_instrument(&mut self.local_state, ui, state, project);
        ui.separator();

        if state.viewed_instrument != self.local_state.instrument_id {
            Self::populate(&mut self.local_state, state, project);
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            Self::show_instrument_ui(&mut self.local_state, ui, state, project);
        });

        ui.allocate_space(ui.available_size());

        self.undoer
            .feed_state(ui.input(|i| i.time), &self.local_state);
    }

    fn heading(&self, state: &AppUIState) -> String {
        format!("Instrument {}", state.viewed_instrument.unwrap_or_default())
    }

    fn draw_side_buttons(&mut self, _ui: &mut Ui, _state: &mut AppUIState, _project: &Project) {}

    fn handle_undo(&mut self, _project: &Project) {
        if !self.undoer.has_undo(&self.local_state) {
            return;
        }
        if let Some(new_state) = self.undoer.undo(&self.local_state) {
            self.local_state = new_state.clone();
        }
    }
}

impl InstrumentUI {
    fn select_instrument(
        s: &mut InstrumentUIState,
        ui: &mut Ui,
        state: &mut AppUIState,
        project: &Project,
    ) {
        let selected_label = state
            .viewed_instrument
            .and_then(|id| {
                project
                    .instruments()
                    .get(&id)
                    .map(|inst| format!("{} - {}", id, inst.name))
            })
            .unwrap_or("None".to_string());

        ui.horizontal(|ui| {
            ui.label("Select Instrument");

            // combobox
            let mut cb_instrument = state.viewed_instrument;
            let inst_before = cb_instrument;
            ComboBox::from_id_salt("instrument_ui_selected_instrument")
                .selected_text(selected_label)
                .show_ui(ui, |ui| {
                    for (id, inst) in project.instruments() {
                        ui.selectable_value(
                            &mut cb_instrument,
                            Some(*id),
                            format!("{} - {}", id, inst.name),
                        );
                    }
                    ui.selectable_value(&mut cb_instrument, None, "None");
                });

            // todo: might be able to simplify this
            if inst_before != cb_instrument {
                state.viewed_instrument = cb_instrument;
            }

            // new instrument
            if ui.button("New Instrument").clicked() {
                let id = Project::get_unique_key(project.instruments());
                project.push_event(ProjectEvent::UpdateInstrument {
                    id,
                    new_instrument: Some(Default::default()),
                });
                state.viewed_instrument = Some(id);
            }

            // delete instrument
            if let Some(id) = s.instrument_id {
                if ui.button("Delete Instrument").clicked() {
                    project.push_event(ProjectEvent::UpdateInstrument {
                        id,
                        new_instrument: None,
                    });
                    state.viewed_instrument = None;
                }
            }
        });
    }

    fn show_instrument_ui(
        s: &mut InstrumentUIState,
        ui: &mut Ui,
        state: &AppUIState,
        project: &Project,
    ) {
        if s.instrument_id.is_none() {
            return;
        }

        // naming instrument
        ui.horizontal(|ui| {
            ui.label("Instrument Name");
            ui.text_edit_singleline(&mut s.name_textbox);
        });

        // wavetables
        ui.horizontal(|ui| {
            if ui.button("Add table").clicked() {
                Self::add_data_table(s, Default::default());
                if !s.data_tables.is_empty() {
                    s.selected_data_table = Some(s.data_tables.len() - 1);
                }
            }

            if let Some((idx, dt)) = s
                .selected_data_table
                .and_then(|idx| s.data_tables.get_mut(idx).map(|dt| (idx, dt)))
            {
                if ui.button("Duplicate").clicked() {
                    let dt = dt.clone();
                    Self::add_data_table(s, dt);
                }

                if ui.button("Delete").clicked() {
                    Self::remove_data_table(s, idx);
                }
            }
        });

        ui.horizontal(|ui| {
            for i in 0..s.data_tables.len() {
                let trimmed_name = s.data_tables[i].name.trim();
                let label = if trimmed_name.is_empty() {
                    i.to_string()
                } else {
                    format!("{} {}", i, trimmed_name)
                };

                if ui
                    .selectable_label(
                        s.selected_data_table.map(|s_dt| s_dt == i).unwrap_or(false),
                        label,
                    )
                    .clicked()
                {
                    s.selected_data_table = Some(i);
                }
            }
        });

        ui.separator();

        if let Some(selected_dt) = s.selected_data_table {
            if let Some(dt) = s.data_tables.get_mut(selected_dt) {
                s.graph.waveform_graph(ui, dt);
                ui.separator();
                adsr_graph::adsr_graph(ui, &mut dt.new_envelope);
            }
        }

        ui.separator();

        egui::ScrollArea::horizontal().show(ui, |ui| {
            Self::show_data_table_mapping(s, ui);
        });

        if let Some(selected_dt) = s.selected_data_table {
            if selected_dt < s.data_table_map.len() && ui.button("Apply to all notes").clicked() {
                for mapping in &mut s.data_table_map {
                    *mapping = Some(selected_dt);
                }
            }
        }

        if ui.button("Apply Changes").clicked() {
            Self::apply(s, state, project)
        }
    }

    fn show_data_table_mapping(s: &mut InstrumentUIState, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing *= 2.0;
            for (semitone, mapping) in s.data_table_map.iter_mut().enumerate() {
                ui.vertical(|ui| {
                    ui.add(
                        Label::new(format!(
                            "{}{}{}",
                            Note::letter_from_semitone(semitone as u8),
                            if Note::sharp_from_semitone(semitone as u8) {
                                "#"
                            } else {
                                ""
                            },
                            Note::octave_from_semitone(semitone as u8)
                        ))
                        .wrap_mode(TextWrapMode::Extend),
                    );

                    let btn_response = ui.small_button(
                        mapping
                            .and_then(|m| {
                                if m < s.data_tables.len() {
                                    Some(m)
                                } else {
                                    None
                                }
                            })
                            .map(|m| m.to_string())
                            .unwrap_or("-".to_string()),
                    );

                    if let Some(selected_dt) = s.selected_data_table {
                        if selected_dt < s.data_tables.len() && btn_response.clicked() {
                            *mapping = Some(selected_dt);
                        }
                    }

                    if btn_response.secondary_clicked() {
                        *mapping = None;
                    }
                });
            }
        });
    }

    fn populate(s: &mut InstrumentUIState, state: &AppUIState, project: &Project) {
        let Some(instrument) = state
            .viewed_instrument
            .and_then(|id| project.instruments().get(&id))
        else {
            s.instrument_id = None;
            return;
        };

        s.instrument_id = state.viewed_instrument;
        s.name_textbox = instrument.name.to_string();
        s.data_tables = instrument
            .data_tables
            .iter()
            .map(|d| d.as_ref().clone())
            .collect();
        s.data_table_map = instrument.data_table_map;
        s.selected_data_table = if instrument.data_tables.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    fn apply(s: &mut InstrumentUIState, state: &AppUIState, project: &Project) {
        let Some(id) = state.viewed_instrument else {
            return;
        };

        let instrument = Instrument {
            name: s.name_textbox.clone(),
            data_tables: s.data_tables.iter().cloned().map(Arc::new).collect(),
            data_table_map: s.data_table_map,
        };

        project.push_event(ProjectEvent::UpdateInstrument {
            id,
            new_instrument: Some(Box::new(instrument)),
        });
    }

    fn add_data_table(s: &mut InstrumentUIState, table: InstrumentDataTable) {
        s.data_tables.push(table.clone());
    }

    fn remove_data_table(s: &mut InstrumentUIState, index: usize) {
        if index >= s.data_tables.len() {
            return;
        }

        s.data_tables.remove(index);

        for mapping_opt in &mut s.data_table_map {
            if let Some(mapping) = mapping_opt {
                match (*mapping).cmp(&index) {
                    std::cmp::Ordering::Greater => *mapping -= 1,
                    std::cmp::Ordering::Equal => *mapping_opt = None,
                    std::cmp::Ordering::Less => (),
                };
            }
        }
    }
}
