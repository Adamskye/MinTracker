use eframe::egui::{self, Align, Color32, Id, Rect, Sense, Stroke, Ui, UiBuilder};
use egui::FontId;

use crate::{AppUIState, helpers::to_colour32, project::Project};

#[derive(Clone, Debug)]
pub struct GridSelection {
    // (row, column)
    pub first: (usize, usize),
    pub last: (usize, usize),
}

pub struct CellGrid<T, G>
where
    T: Default + Clone + CellData<G>,
{
    num_rows: usize,
    num_columns: usize,
    highlighted_row: usize,
    highlighted_col: usize,

    grid: Vec<Vec<T>>,

    selection: Option<GridSelection>,

    _marker: std::marker::PhantomData<G>,
}

impl<T, G> CellGrid<T, G>
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

    pub fn get_selection(&self) -> Option<GridSelection> {
        self.selection.clone()
    }

    pub fn get_highlighted_position(&self) -> (usize, usize) {
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
}

pub trait CellData<G> {
    fn text(&self) -> Option<String>;

    fn inner_widget(
        &self,
        _ui: &mut Ui,
        _grid_state: &mut G,
        _state: &mut AppUIState,
        _project: &Project,
    ) {
    }

    fn has_inner_widget(&self, _grid_state: &mut G) -> bool {
        false
    }

    #[allow(unused)]
    fn color(&self) -> Color32;
    fn context_menu(
        &self,
        _ui: &mut Ui,
        _grid_state: &mut G,
        _state: &mut AppUIState,
        _project: &Project,
    ) {
    }

    /// Extra action to perform when double clicked or trigger button is pressed when this cell is
    /// highlighted.
    /// By default, will call self.on_click(...)
    fn trigger_action(&self, grid_state: &mut G, state: &mut AppUIState, project: &Project) {
        self.on_click(grid_state, state, project);
    }

    /// Extra action to perform when single-clicked
    fn on_click(&self, _grid_state: &mut G, _state: &mut AppUIState, _project: &Project) {}

    /// If there is no selection, then this is run if the cell is highlighted.
    /// If there is a selection, then this is run for all cells in the selection.
    fn on_keyboard_input(
        &self,
        _input: &egui::InputState,
        _grid_state: &mut G,
        _state: &mut AppUIState,
        _project: &Project,
    ) {
    }

    fn highlightable(&self) -> bool {
        true
    }

    fn selectable(&self) -> bool {
        true
    }
}

fn rect_from_cell_pos(row: usize, column: usize, cell_size: egui::Vec2, grid_rect: Rect) -> Rect {
    Rect::from_min_size(
        egui::pos2(column as f32 * cell_size.x, row as f32 * cell_size.y) + grid_rect.min.to_vec2(),
        cell_size,
    )
}

pub fn cells<T, G>(
    ui: &mut Ui,
    env: &mut CellGrid<T, G>,
    grid_state: &mut G,
    state: &mut AppUIState,
    project: &Project,
) where
    T: Default + Clone + CellData<G>,
{
    // each column should be 40.0 wide, and each row should be 20.0 tall
    let cell_size = egui::vec2(40.0, 20.0);
    let desired_width = cell_size.x * env.num_columns() as f32;
    let desired_height = cell_size.y * env.num_rows() as f32;

    // allocating space
    let (grid_rect, _response) = ui.allocate_exact_size(
        egui::vec2(desired_width, desired_height),
        Sense::hover() | Sense::click(),
    );

    if !ui.is_rect_visible(grid_rect) {
        return;
    }

    // correcting highlighted row and column if they are over a cell that can't be highlighted
    if env
        .get(env.highlighted_row(), env.highlighted_col())
        .is_some_and(|cell| !cell.highlightable())
    {
        for column in 0..env.num_columns() {
            for row in 0..env.num_rows() {
                if env
                    .get(row, column)
                    .is_some_and(|cell| cell.highlightable())
                {
                    env.set_highlighted_row(row);
                    env.set_highlighted_col(column);
                    break;
                }
            }
        }
    }

    if env.highlighted_row() >= env.num_rows() {
        env.set_highlighted_row(env.num_rows().saturating_sub(1));
    }

    let mut should_scroll_to_highlighted = false;

    // handle keyboard input
    ui.input(|i| {
        if i.key_pressed(state.preferences().keybinds.trigger_cell)
            && let Some(cell) = env.get(env.highlighted_row(), env.highlighted_col())
        {
            cell.trigger_action(grid_state, state, project);
            return;
        }

        let mut new_row = env.highlighted_row();
        let mut new_col = env.highlighted_col();
        let prefs = state.preferences();
        if i.key_pressed(prefs.keybinds.right) {
            new_col = env.highlighted_col() + 1;
            should_scroll_to_highlighted = true;
        }
        if i.key_pressed(prefs.keybinds.left) {
            new_col = env.highlighted_col().saturating_sub(1);
            should_scroll_to_highlighted = true;
        }
        if i.key_pressed(prefs.keybinds.down) {
            new_row = env.highlighted_row() + 1;
            should_scroll_to_highlighted = true;
        }
        if i.key_pressed(prefs.keybinds.up) {
            new_row = env.highlighted_row().saturating_sub(1);
            should_scroll_to_highlighted = true;
        }

        if new_row != env.highlighted_row() || new_col != env.highlighted_col() {
            // get cell at this new position
            let Some(cell) = env.get(new_row, new_col) else {
                return;
            };

            if cell.highlightable() {
                env.set_highlighted_row(new_row);
                env.set_highlighted_col(new_col);
            }
        }
    });

    for row in 0..env.num_rows() {
        for column in 0..env.num_columns() {
            let cell_rect = rect_from_cell_pos(row, column, cell_size, grid_rect);
            let cell_is_highlighted =
                row == env.highlighted_row() && column == env.highlighted_col();

            let cell_is_selected = env.selection.as_ref().map_or(false, |selection| {
                let (start_row, start_col) = selection.first;
                let (end_row, end_col) = selection.last;

                row >= start_row.min(end_row)
                    && row <= start_row.max(end_row)
                    && column >= start_col.min(end_col)
                    && column <= start_col.max(end_col)
            });

            if !ui.is_rect_visible(cell_rect) {
                continue;
            }

            // interact response
            let id = Id::new((ui.id(), row, column));
            let response = ui.interact(cell_rect, id, Sense::click() | Sense::drag());

            let sel_col = state.preferences().style.colours.highlighted;

            // disabling multiple cell selection
            if response.clicked()
                || response.double_clicked()
                || response.secondary_clicked()
                || response.clicked_elsewhere()
            {
                env.selection = None;
            }

            let Some(cell_data) = env.get(row, column) else {
                continue;
            };

            // draw cell
            let painter = ui.painter();
            let text = cell_data.text();
            if let Some(text) = &text {
                // drawing border
                if cell_is_highlighted {
                    // highlighted
                    let stroke = Stroke::new(2.0, to_colour32(sel_col));
                    painter.rect_stroke(cell_rect, 0.0, stroke, egui::StrokeKind::Inside);
                } else if response.hovered() || response.is_pointer_button_down_on() {
                    // mouse over
                    let stroke = Stroke::new(
                        2.0,
                        to_colour32(state.preferences().style.colours.button_bg),
                    );
                    painter.rect_stroke(cell_rect, 0.0, stroke, egui::StrokeKind::Inside);
                }

                // multiple cell selection
                if let Some(selection) = &env.selection
                    && cell_data.selectable()
                {
                    let (start_row, start_col) = selection.first;
                    let (end_row, end_col) = selection.last;

                    if row >= start_row.min(end_row)
                        && row <= start_row.max(end_row)
                        && column >= start_col.min(end_col)
                        && column <= start_col.max(end_col)
                    {
                        let colour = to_colour32(sel_col).linear_multiply(0.5);
                        let stroke = Stroke::new(2.0, colour);
                        painter.rect_stroke(cell_rect, 0.0, stroke, egui::StrokeKind::Inside);
                    }
                }

                painter.text(
                    cell_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    FontId::default(),
                    Color32::WHITE,
                );
            }

            if cell_data.has_inner_widget(grid_state) {
                let mut ui = ui.new_child(UiBuilder::new().max_rect(cell_rect));
                cell_data.inner_widget(&mut ui, grid_state, state, project);
            }

            // scroll to the highlighted cell if it was changed by keyboard input
            if should_scroll_to_highlighted && cell_is_highlighted {
                ui.scroll_to_rect(cell_rect, Some(Align::Center));
            }

            // keyboard input
            if (env.selection.is_none() && cell_is_highlighted)
                || env.selection.is_some() && cell_is_selected
            {
                ui.input(|i| {
                    cell_data.on_keyboard_input(i, grid_state, state, project);
                });
            }

            // context menu
            let mut context_menu_opened = false;
            response.context_menu(|ui| {
                context_menu_opened = true;
                cell_data.context_menu(ui, grid_state, state, project);
            });

            if response.clicked() {
                cell_data.on_click(grid_state, state, project);
            }

            // clicking and double clicking
            if response.double_clicked() && cell_is_highlighted {
                // trigger cell action
                cell_data.trigger_action(grid_state, state, project);
                env.selection = None;
            } else if (response.clicked() || context_menu_opened) && cell_data.highlightable() {
                // highlighting due to click
                env.set_highlighted_row(row);
                env.set_highlighted_col(column);
            }

            // drag enables a multiple cell selection
            if response.drag_started() {
                env.selection = Some(GridSelection {
                    first: (row, column),
                    last: (row, column),
                });
            }

            if response.dragged() {
                // get which cell the mouse is currently over
                let mouse_pos = ui.input(|i| i.pointer.interact_pos());

                if let Some(mouse_pos) = mouse_pos {
                    let col = ((mouse_pos.x - grid_rect.min.x) / cell_size.x).floor() as usize;
                    let row = ((mouse_pos.y - grid_rect.min.y) / cell_size.y).floor() as usize;

                    let n_rows = env.num_rows();
                    let n_cols = env.num_columns();
                    if let Some(selection) = &mut env.selection {
                        selection.last = (row.min(n_rows), col.min(n_cols));
                    }
                }
            }
        }
    }
}
