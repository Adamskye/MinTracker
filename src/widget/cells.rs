use eframe::egui::{
    self, Align, Color32, Direction, Id, Key, Layout, Rect, Sense, Stroke, Ui, UiBuilder,
};

use crate::{project::Project, AppUIState};

pub struct CellGrid<T, G>
where
    T: Default + Clone + CellData<G>,
{
    num_rows: usize,
    num_columns: usize,
    selected_row: usize,
    selected_col: usize,

    grid: Vec<Vec<T>>,

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
            selected_row: 0,
            selected_col: 0,
            grid: vec![vec![T::default(); num_columns]; num_rows],
            _marker: std::marker::PhantomData,
        }
    }

    pub fn num_rows(&self) -> usize {
        self.num_rows
    }

    pub fn num_columns(&self) -> usize {
        self.num_columns
    }

    pub fn selected_row(&self) -> usize {
        self.selected_row
    }

    pub fn selected_col(&self) -> usize {
        self.selected_col
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

    pub fn set_selected_row(&mut self, selected_row: usize) {
        self.selected_row = selected_row.clamp(0, self.num_rows - 1);
    }

    pub fn set_selected_col(&mut self, selected_col: usize) {
        self.selected_col = selected_col.clamp(0, self.num_columns - 1);
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
    fn selected(&self, _grid_state: &mut G, _state: &mut AppUIState, _project: &Project) {}

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
    let (rect, _response) = ui.allocate_exact_size(
        egui::vec2(desired_width, desired_height),
        Sense::hover() | Sense::click(),
    );

    if !ui.is_rect_visible(rect) {
        return;
    }

    // correcting selected row and column if they are over a cell that can't be selected
    if env
        .get(env.selected_row(), env.selected_col())
        .is_some_and(|cell| !cell.selectable())
    {
        for column in 0..env.num_columns() {
            for row in 0..env.num_rows() {
                if env.get(row, column).is_some_and(|cell| cell.selectable()) {
                    env.set_selected_row(row);
                    env.set_selected_col(column);
                    break;
                }
            }
        }
    }

    if env.selected_row() >= env.num_rows() {
        env.set_selected_row(env.num_rows().saturating_sub(1));
    }

    let mut should_scroll_to_selected = false;

    ui.input(|i| {
        let mut new_row = env.selected_row();
        let mut new_col = env.selected_col();
        if i.key_pressed(Key::ArrowRight) {
            new_col = env.selected_col() + 1;
            should_scroll_to_selected = true;
        }
        if i.key_pressed(Key::ArrowLeft) {
            new_col = env.selected_col().saturating_sub(1);
            should_scroll_to_selected = true;
        }
        if i.key_pressed(Key::ArrowDown) {
            new_row = env.selected_row() + 1;
            should_scroll_to_selected = true;
        }
        if i.key_pressed(Key::ArrowUp) {
            new_row = env.selected_row().saturating_sub(1);
            should_scroll_to_selected = true;
        }

        if env
            .get(new_row, new_col)
            .is_some_and(|cell| !cell.selectable())
        {
            return;
        }

        if new_row != env.selected_row() {
            env.set_selected_row(new_row);
        }
        if new_col != env.selected_col() {
            env.set_selected_col(new_col);
        }
    });

    for row in 0..env.num_rows() {
        for column in 0..env.num_columns() {
            let cell_rect = rect_from_cell_pos(row, column, cell_size, rect);

            // interact response
            let id = Id::new((ui.id(), row, column));
            let response = ui.interact(cell_rect, id, Sense::click());

            let stroke = if row == env.selected_row() && column == env.selected_col() {
                Stroke::new(2.0, Color32::YELLOW)
            } else if response.hovered() || response.is_pointer_button_down_on() {
                Stroke::new(2.0, Color32::LIGHT_GRAY)
            } else {
                Stroke::new(1.0, Color32::GRAY)
            };

            let Some(cell_data) = env.get(row, column) else {
                continue;
            };

            // draw cell
            let painter = ui.painter();
            let text = cell_data.text();
            if let Some(text) = &text {
                painter.rect_stroke(cell_rect, 1.0, stroke, egui::StrokeKind::Inside);
                painter.text(
                    cell_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::default(),
                    Color32::WHITE,
                );
            } else {
                ui.scope_builder(
                    UiBuilder::new()
                        .layout(Layout::centered_and_justified(Direction::TopDown))
                        .max_rect(cell_rect),
                    |ui| {
                        cell_data.inner_widget(ui, grid_state, state, project);
                    },
                );
            }

            // scroll to the selected cell if it was changed by keyboard input
            if should_scroll_to_selected
                && row == env.selected_row()
                && column == env.selected_col()
            {
                ui.scroll_to_rect(cell_rect, Some(Align::Center));
            }

            // context menu
            let mut context_menu_opened = false;
            response.context_menu(|ui| {
                context_menu_opened = true;
                cell_data.context_menu(ui, grid_state, state, project);
            });

            // clicking and double clicking
            if response.double_clicked()
                && env.selected_row() == row
                && env.selected_col() == column
            {
                cell_data.selected(grid_state, state, project);
            } else if (response.clicked() || context_menu_opened) && cell_data.selectable() {
                env.set_selected_row(row);
                env.set_selected_col(column);
            }
        }
    }
}
