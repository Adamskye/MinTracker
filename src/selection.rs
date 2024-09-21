use eframe::egui::{Response, Ui};

/**
 * This file contains helpers for handling selections as well as tracking when the user cuts,
 * copies, pastes, or deletes a selection.
 */

pub type SelectionCoords = Option<((usize, usize), (usize, usize))>;

pub fn handle_widget_selecting(
    ui: &mut Ui,
    coords: &mut SelectionCoords,
    widget_response: &Response,
    row: usize,
    column: usize,
) {
    if !widget_response.contains_pointer() {
        return;
    }

    if ui.input(|i| i.pointer.primary_pressed() && !i.modifiers.shift) {
        *coords = Some(((column, row), (column, row)));
        return;
    }

    let Some((_, coord2)) = coords else {
        return;
    };

    if ui.input(|i| i.pointer.primary_down()) {
        *coord2 = (column, row);
    }
}

pub fn widget_in_selection(coords: &SelectionCoords, row: usize, column: usize) -> bool {
    matches!(*coords,
        Some(((x1, y1), (x2, y2)))
        if (usize::min(x1, x2)..=usize::max(x1, x2)).contains(&column)
        && (usize::min(y1, y2)..=usize::max(y1, y2)).contains(&row)
    )
}
