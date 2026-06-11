use egui::{Align, Rect, Ui, Vec2};

use crate::{
    app_ui_state::AppUIState,
    project::Project,
    widget::cells::{cell_data::CellData, rect_from_cell_pos, selection::GridSelection},
};

pub struct CellGridState<T, G> {
    num_rows: usize,
    num_columns: usize,
    highlighted_row: usize,
    highlighted_col: usize,

    grid: Vec<Vec<T>>,

    pub selection: Option<GridSelection>,

    _marker: std::marker::PhantomData<G>,
}

impl<T, G> CellGridState<T, G>
where
    T: Default + Clone + CellData<G>,
{
    pub fn new(num_rows: usize, num_columns: usize) -> Self {
        Self {
            num_rows,
            num_columns,
            highlighted_row: 0,
            highlighted_col: 0,
            grid: vec![vec![T::default(); num_columns]; num_rows],
            _marker: std::marker::PhantomData,
            selection: None,
        }
    }

    pub fn highlighted_position(&self) -> (usize, usize) {
        (self.highlighted_row, self.highlighted_col)
    }

    pub fn num_rows(&self) -> usize {
        self.num_rows
    }

    pub fn num_columns(&self) -> usize {
        self.num_columns
    }

    pub fn highlighted_row(&self) -> usize {
        self.highlighted_row
    }

    pub fn highlighted_col(&self) -> usize {
        self.highlighted_col
    }

    pub fn set_num_rows(&mut self, num_rows: usize) {
        self.num_rows = num_rows;
        self.grid
            .resize(num_rows, vec![T::default(); self.num_columns]);
    }

    pub fn set_num_columns(&mut self, num_columns: usize) {
        self.num_columns = num_columns;
        for row in &mut self.grid {
            row.resize(num_columns, T::default());
        }
    }

    pub fn set_highlighted_row(&mut self, selected_row: usize) {
        self.highlighted_row = selected_row.clamp(0, self.num_rows - 1);
    }

    pub fn set_highlighted_col(&mut self, selected_col: usize) {
        self.highlighted_col = selected_col.clamp(0, self.num_columns - 1);
    }

    pub fn get(&self, row: usize, column: usize) -> Option<&T> {
        self.grid.get(row)?.get(column)
    }

    pub fn set(&mut self, row: usize, column: usize, value: T) {
        if self.grid.len() <= row || self.grid[row].len() <= column {
            return;
        }

        while self.grid[row].len() <= column {
            self.grid[row].push(T::default());
        }

        self.grid[row][column] = value;
    }

    pub(super) fn handle_global_keyboard_input(
        &mut self,
        ui: &mut Ui,
        shared_data: &mut G,
        cell_size: Vec2,
        grid_rect: Rect,
        state: &mut AppUIState,
        project: &Project,
    ) {
        // handle keyboard input
        let mut should_scroll_to_highlighted = false;
        ui.input(|i| {
            if i.key_pressed(state.preferences().keybinds.trigger_cell)
                && let Some(cell) = self.get(self.highlighted_row(), self.highlighted_col())
            {
                cell.trigger_action(shared_data, state, project);
                return;
            }

            let mut new_row = self.highlighted_row();
            let mut new_col = self.highlighted_col();
            let prefs = state.preferences();
            if i.key_pressed(prefs.keybinds.right) {
                new_col = self.highlighted_col() + 1;
                should_scroll_to_highlighted = true;
            }
            if i.key_pressed(prefs.keybinds.left) {
                new_col = self.highlighted_col().saturating_sub(1);
                should_scroll_to_highlighted = true;
            }
            if i.key_pressed(prefs.keybinds.down) {
                new_row = self.highlighted_row() + 1;
                should_scroll_to_highlighted = true;
            }
            if i.key_pressed(prefs.keybinds.up) {
                new_row = self.highlighted_row().saturating_sub(1);
                should_scroll_to_highlighted = true;
            }

            if new_row != self.highlighted_row() || new_col != self.highlighted_col() {
                // get cell at this new position
                let Some(cell) = self.get(new_row, new_col) else {
                    return;
                };

                if cell.highlightable() {
                    self.set_highlighted_row(new_row);
                    self.set_highlighted_col(new_col);
                }
            }
        });

        if should_scroll_to_highlighted {
            self.scroll_to_highlighted(ui, cell_size, grid_rect);
        }
    }

    fn scroll_to_highlighted(&mut self, ui: &mut Ui, cell_size: Vec2, grid_rect: Rect)
    where
        T: Default + Clone + CellData<G>,
    {
        // scroll to the highlighted cell if it was changed by keyboard input
        let (row, col) = (self.highlighted_row(), self.highlighted_col());
        let cell_rect = rect_from_cell_pos(row, col, cell_size, grid_rect);
        ui.scroll_to_rect(cell_rect, Some(Align::Center));
    }
}
