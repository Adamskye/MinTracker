pub mod cell_data;
pub mod event;
pub mod selection;
pub mod state;

use std::marker::PhantomData;

use eframe::egui::{self, Color32, Id, Rect, Sense, Stroke, Ui, UiBuilder};
use egui::{FontId, Response, Vec2};

use crate::{
    AppUIState,
    helpers::to_colour32,
    project::Project,
    widget::cells::{
        cell_data::CellData, event::CellGridEvent, selection::GridSelection, state::CellGridState,
    },
};

/// T is the data stored in each cell
/// G is the data shared across all cells
pub struct CellGridWidget<T, G>
where
    T: Default + Clone + CellData<G>,
{
    pub shared_data: G,
    pub state: CellGridState<T, G>,
    shade_every: Option<usize>,
    start_shade_at: usize,

    _marker: std::marker::PhantomData<G>,
}

impl<T, G> CellGridWidget<T, G>
where
    T: Default + Clone + CellData<G>,
{
    pub fn new(num_rows: usize, num_columns: usize, shared_data: G) -> Self {
        Self {
            shared_data,
            state: CellGridState::new(num_rows, num_columns),
            shade_every: None,
            start_shade_at: 0,
            _marker: PhantomData,
        }
    }

    pub fn shade_every(mut self, every: usize) -> Self {
        self.shade_every = Some(every);
        self
    }

    pub fn start_shade_at(mut self, at: usize) -> Self {
        self.start_shade_at = at;
        self
    }

    /// T is the type of data stored in each cell, and G is the type of data that is shared across all
    /// cells.
    /// `shade_every` allows you to specify how often the row shading should alternate.
    pub fn show(&mut self, ui: &mut Ui, state: &mut AppUIState, project: &Project)
    where
        T: Default + Clone + CellData<G>,
    {
        // each column should be 40.0 wide, and each row should be 20.0 tall
        let cell_size = egui::vec2(40.0, 20.0);
        let desired_width = cell_size.x * self.state.num_columns() as f32;
        let desired_height = cell_size.y * self.state.num_rows() as f32;

        // allocating space
        let (grid_rect, _response) = ui.allocate_exact_size(
            egui::vec2(desired_width, desired_height),
            Sense::hover() | Sense::click(),
        );

        if !ui.is_rect_visible(grid_rect) {
            return;
        }

        // correcting highlighted row and column if they are over a cell that can't be highlighted
        if self
            .state
            .get(self.state.highlighted_row(), self.state.highlighted_col())
            .is_some_and(|cell| !cell.highlightable())
        {
            for column in 0..self.state.num_columns() {
                for row in 0..self.state.num_rows() {
                    if self
                        .state
                        .get(row, column)
                        .is_some_and(cell_data::CellData::highlightable)
                    {
                        self.state.set_highlighted_row(row);
                        self.state.set_highlighted_col(column);
                        break;
                    }
                }
            }
        }

        if self.state.highlighted_row() >= self.state.num_rows() {
            self.state
                .set_highlighted_row(self.state.num_rows().saturating_sub(1));
        }

        // drawing background if `shade_every` is specified
        if let Some(every) = self.shade_every {
            self.draw_alternating_background(ui, every, cell_size, grid_rect);
        }

        // showing all cells
        for row in 0..self.state.num_rows() {
            for column in 0..self.state.num_columns() {
                let Some(cell_data) = self.state.get(row, column).cloned() else {
                    continue;
                };
                let cell_rect = rect_from_cell_pos(row, column, cell_size, grid_rect);

                let cell = VisibleCell {
                    row,
                    column,
                    cell_rect,
                    cell_data,
                    cell_size,
                    grid_rect,
                    marker: PhantomData,
                };

                cell.handle_input_and_draw(
                    ui,
                    &mut self.shared_data,
                    &mut self.state,
                    state,
                    project,
                );
            }
        }

        // global keyboard input (e.g. moving up, down, left, and right)
        self.state.handle_global_keyboard_input(
            ui,
            &mut self.shared_data,
            cell_size,
            grid_rect,
            state,
            project,
        );
    }

    fn draw_alternating_background(
        &self,
        ui: &Ui,
        shade_every: usize,
        cell_size: Vec2,
        grid_rect: Rect,
    ) {
        let painter = ui.painter();
        let c_height = cell_size.y;
        let rows = self.state.num_rows();
        let columns = self.state.num_columns();
        let origin = grid_rect.min;

        let mut position = self.start_shade_at;
        loop {
            if position >= rows {
                // done
                return;
            }

            // shade
            let rows_to_shade = shade_every.min(rows - position) as f32;
            let pos_f32 = position as f32;
            let rect = Rect {
                min: (origin.x, origin.y + (c_height * pos_f32)).into(),
                max: (
                    origin.x + (columns as f32 * cell_size.x),
                    origin.y + ((c_height * pos_f32) + (c_height * rows_to_shade)),
                )
                    .into(),
            };

            painter.rect_filled(rect, 0, Color32::GRAY.gamma_multiply(0.1));

            position += shade_every + shade_every;
        }
    }
}

struct VisibleCell<T, G>
where
    T: Default + Clone + CellData<G>,
{
    row: usize,
    column: usize,
    cell_rect: Rect,
    cell_data: T,
    cell_size: Vec2,
    grid_rect: Rect,

    marker: PhantomData<G>,
}

impl<T, G> VisibleCell<T, G>
where
    T: Default + Clone + CellData<G>,
{
    fn handle_input_and_draw(
        &self,
        ui: &mut Ui,
        shared_data: &mut G,
        env: &mut CellGridState<T, G>,
        state: &mut AppUIState,
        project: &Project,
    ) {
        if !ui.is_rect_visible(self.cell_rect) {
            return;
        }

        let response = self.get_interact_response(ui);
        self.multi_cell_selection(env, ui, &response);
        self.clicking(&response, shared_data, env, state, project);
        self.keyboard_input(env, shared_data, ui, state, project);
        self.context_menu(shared_data, &response, state, project);

        self.draw_cell(ui, shared_data, env, state, project, &response);
    }

    fn is_highlighted(&self, env: &mut CellGridState<T, G>) -> bool {
        self.row == env.highlighted_row() && self.column == env.highlighted_col()
    }

    fn get_interact_response(&self, ui: &mut Ui) -> Response {
        let id = Id::new((ui.id(), self.row, self.column));
        ui.interact(self.cell_rect, id, Sense::click() | Sense::drag())
    }

    fn multi_cell_selection(
        &self,
        env: &mut CellGridState<T, G>,
        ui: &mut Ui,
        response: &Response,
    ) {
        // disabling multiple cell selection
        if response.clicked()
            || response.double_clicked()
            || response.secondary_clicked()
            || response.clicked_elsewhere()
        {
            env.selection = None;
        }

        // drag enables a multiple cell selection
        if response.drag_started() {
            env.selection = Some(GridSelection {
                first: (self.row, self.column),
                last: (self.row, self.column),
            });
        }

        // expanding/shrinking multiple cell selection
        if response.dragged() {
            // get which cell the mouse is currently over
            let mouse_pos = ui.input(|i| i.pointer.interact_pos());

            if let Some(mouse_pos) = mouse_pos {
                let col =
                    ((mouse_pos.x - self.grid_rect.min.x) / self.cell_size.x).floor() as usize;
                let row =
                    ((mouse_pos.y - self.grid_rect.min.y) / self.cell_size.y).floor() as usize;

                let n_rows = env.num_rows();
                let n_cols = env.num_columns();
                if let Some(selection) = &mut env.selection {
                    selection.last = (row.min(n_rows), col.min(n_cols));
                }
            }
        }
    }

    fn context_menu(
        &self,
        shared_data: &mut G,
        response: &Response,
        state: &mut AppUIState,
        project: &Project,
    ) {
        let mut context_menu_opened = false;
        if self.cell_data.has_context_menu() {
            response.context_menu(|ui| {
                context_menu_opened = true;
                self.cell_data.context_menu(ui, shared_data, state, project);
            });
        }
    }

    fn keyboard_input(
        &self,
        env: &mut CellGridState<T, G>,
        shared_data: &mut G,
        ui: &mut Ui,
        state: &mut AppUIState,
        project: &Project,
    ) {
        let highlighted = self.is_highlighted(env);

        let selected = env.selection.as_ref().is_some_and(|selection| {
            let (start_row, start_col) = selection.first;
            let (end_row, end_col) = selection.last;

            self.row >= start_row.min(end_row)
                && self.row <= start_row.max(end_row)
                && self.column >= start_col.min(end_col)
                && self.column <= start_col.max(end_col)
        });

        if ((env.selection.is_none() && highlighted) || env.selection.is_some() && selected)
            && let Some(evt) = ui.input(|i| {
                self.cell_data
                    .on_keyboard_input(i, shared_data, state, project)
            })
        {
            trigger_event(evt, env);
        }
    }

    fn clicking(
        &self,
        response: &Response,
        shared_data: &mut G,
        env: &mut CellGridState<T, G>,
        state: &mut AppUIState,
        project: &Project,
    ) {
        let highlighted = self.is_highlighted(env);

        if response.clicked() {
            self.cell_data.on_click(shared_data, state, project);
        }

        if response.clicked() || response.secondary_clicked() {
            env.set_highlighted_row(self.row);
            env.set_highlighted_col(self.column);
        }

        // clicking and double clicking
        if response.double_clicked() && highlighted {
            // trigger cell action
            self.cell_data.trigger_action(shared_data, state, project);
            env.selection = None;
        }
    }

    fn draw_cell(
        &self,
        ui: &mut Ui,
        shared_data: &mut G,
        env: &mut CellGridState<T, G>,
        state: &mut AppUIState,
        project: &Project,
        response: &Response,
    ) {
        let cell_is_highlighted =
            self.row == env.highlighted_row() && self.column == env.highlighted_col();
        let select_colour = state.preferences().style.colours.highlighted;
        let painter = ui.painter();

        // drawing border
        if cell_is_highlighted {
            // highlighted
            let stroke = Stroke::new(2.0_f32, to_colour32(select_colour));
            painter.rect_stroke(self.cell_rect, 0.0, stroke, egui::StrokeKind::Inside);
        } else if response.hovered() || response.is_pointer_button_down_on() {
            // mouse over
            let stroke = Stroke::new(
                2.0_f32,
                to_colour32(state.preferences().style.colours.button_bg),
            );
            painter.rect_stroke(self.cell_rect, 0.0, stroke, egui::StrokeKind::Inside);
        }

        // multiple cell selection
        if let Some(selection) = &env.selection
            && self.cell_data.multiselectable()
        {
            let (start_row, start_col) = selection.first;
            let (end_row, end_col) = selection.last;

            if self.row >= start_row.min(end_row)
                && self.row <= start_row.max(end_row)
                && self.column >= start_col.min(end_col)
                && self.column <= start_col.max(end_col)
            {
                let colour = to_colour32(select_colour).linear_multiply(0.5);
                let stroke = Stroke::new(2.0_f32, colour);
                painter.rect_stroke(self.cell_rect, 0.0, stroke, egui::StrokeKind::Inside);
            }
        }

        // drawing text (if any)
        let text = self.cell_data.text();
        if let Some(text) = &text {
            painter.text(
                self.cell_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                FontId::default(),
                self.cell_data.text_color(project, state),
            );
        }

        // drawing inner widget (if any)
        if self.cell_data.has_inner_widget(shared_data) {
            let mut ui = ui.new_child(UiBuilder::new().max_rect(self.cell_rect));
            self.cell_data
                .inner_widget(&mut ui, shared_data, state, project);
        }
    }
}

fn trigger_event<T, G>(event: CellGridEvent, env: &mut CellGridState<T, G>)
where
    T: Default + Clone + CellData<G>,
{
    match event {
        CellGridEvent::SetHighlightedPosition(row, col) => {
            env.set_highlighted_row(row);
            env.set_highlighted_col(col);
        }
        CellGridEvent::ConsumeInput => env.input_consumed_marker = true,
        CellGridEvent::Multiple(evts) => {
            for evt in evts {
                trigger_event(evt, env);
            }
        }
    }
}

fn rect_from_cell_pos(row: usize, column: usize, cell_size: Vec2, grid_rect: Rect) -> Rect {
    Rect::from_min_size(
        egui::pos2(column as f32 * cell_size.x, row as f32 * cell_size.y) + grid_rect.min.to_vec2(),
        cell_size,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_correct_bounds_when_first_is_top_left() {
        let selection = GridSelection {
            first: (2, 3),
            last: (5, 7),
        };

        assert_eq!(selection.small_row(), 2);
        assert_eq!(selection.big_row(), 5);
        assert_eq!(selection.small_col(), 3);
        assert_eq!(selection.big_col(), 7);
    }

    #[test]
    fn returns_correct_bounds_when_first_is_bottom_right() {
        let selection = GridSelection {
            first: (5, 7),
            last: (2, 3),
        };

        assert_eq!(selection.small_row(), 2);
        assert_eq!(selection.big_row(), 5);
        assert_eq!(selection.small_col(), 3);
        assert_eq!(selection.big_col(), 7);
    }

    #[test]
    fn returns_same_values_for_single_cell_selection() {
        let selection = GridSelection {
            first: (4, 6),
            last: (4, 6),
        };

        assert_eq!(selection.small_row(), 4);
        assert_eq!(selection.big_row(), 4);
        assert_eq!(selection.small_col(), 6);
        assert_eq!(selection.big_col(), 6);
    }

    #[test]
    fn handles_mixed_row_and_column_order() {
        let selection = GridSelection {
            first: (8, 2),
            last: (3, 10),
        };

        assert_eq!(selection.small_row(), 3);
        assert_eq!(selection.big_row(), 8);
        assert_eq!(selection.small_col(), 2);
        assert_eq!(selection.big_col(), 10);
    }
}
