use eframe::{
    egui::{self, DragValue, Stroke, Ui},
    epaint::{pos2, Color32, Pos2, Rect},
};

use crate::project::ADSREnvelope;

fn draw_adsr_controls(ui: &mut Ui, env: &mut ADSREnvelope) {
    ui.horizontal(|ui| {
        ui.label("Volume");
        ui.add(
            DragValue::new(&mut env.volume)
                .range(0.0..=1.0)
                .fixed_decimals(2)
                .speed(0.01),
        );
    });

    ui.columns(4, |ui| {
        ui[0].label("Attack");
        ui[1].label("Decay");
        ui[2].label("Sustain");
        ui[3].label("Release");

        ui[0].add(DragValue::new(&mut env.attack_ms).range(0..=20000));
        ui[1].add(DragValue::new(&mut env.decay_ms).range(0..=20000));
        ui[2].add(
            DragValue::new(&mut env.sustain_vol)
                .range(0.0..=1.0)
                .fixed_decimals(2)
                .speed(0.01),
        );
        ui[3].add(DragValue::new(&mut env.release_ms).range(0..=20000));

        ui[0].label("ms");
        ui[1].label("ms");
        ui[2].label("");
        ui[3].label("ms");
    });
}

fn draw_adsr_graph(ui: &mut Ui, env: &mut ADSREnvelope) {
    // determining size
    //let desired_width = f32::min(ui.available_width(), 400.0);
    let desired_width = 400.0;
    let desired_size = egui::vec2(desired_width, desired_width * 0.6);

    // allocating space
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::drag());

    // handle mouse editing
    if let Some(_mouse_pos) = response.interact_pointer_pos() {}

    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Other,
            ui.is_enabled(),
            response.hovered(),
            "",
        )
    });

    // paint widget
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, Color32::from_rgb(0, 0, 0));

        // draw graph
        let width_multiplier = f32::min(
            1.0,
            rect.width() / (env.attack_ms + env.decay_ms + env.release_ms) as f32,
        );

        // main points
        let mut points = [pos2(0.0, 0.0); 4];
        points[1] = pos2(env.attack_ms as f32, 1.0);
        points[2] = pos2(points[1].x + env.decay_ms as f32, env.sustain_vol);
        points[3] = pos2(points[2].x + env.release_ms as f32, 0.0);

        for point in points.iter_mut() {
            point.x *= width_multiplier;
            point.x += rect.left();

            point.y = 1.0 - point.y.clamp(0.0, 1.0);
            point.y *= rect.height();
            point.y += rect.top();
        }

        let stroke = Stroke {
            width: 1.0,
            color: Color32::from_rgb(255, 255, 255),
        };

        // draw lines between main points
        painter.line_segment([points[0], points[1]], stroke);
        painter.line_segment([points[1], points[2]], stroke);
        painter.line_segment([points[2], points[3]], stroke);

        // update main points
        if let Some(p) = adsr_graph_point(ui, points[1]) {
            points[1] = p;
            env.attack_ms = point_to_value(p, &rect, width_multiplier).0;
        }
        if let Some(p) = adsr_graph_point(ui, points[2]) {
            points[2] = p;
            (env.decay_ms, env.sustain_vol) = point_to_value(p, &rect, width_multiplier);
            env.decay_ms = env.decay_ms.saturating_sub(env.attack_ms);
        }
        if let Some(p) = adsr_graph_point(ui, points[3]) {
            //points[3] = p;
            env.release_ms = point_to_value(p, &rect, width_multiplier).0;
            env.release_ms = env.release_ms.saturating_sub(env.decay_ms + env.attack_ms);
        }
    }
}

pub fn adsr_graph(ui: &mut Ui, env: &mut ADSREnvelope) {
    ui.vertical(|ui| {
        draw_adsr_controls(ui, env);
        draw_adsr_graph(ui, env);
    });
}

fn point_to_value(pos: Pos2, rect: &Rect, width_multiplier: f32) -> (u32, f32) {
    let x = ((pos.x - rect.left()) / width_multiplier) as u32;
    let y = 1.0 - ((pos.y - rect.top()) / rect.height());

    (x, y)
}

fn adsr_graph_point(ui: &mut Ui, point: Pos2) -> Option<Pos2> {
    let mut color = Color32::from_rgb(255, 0, 0);

    let rect = Rect::from_two_pos(
        pos2(point.x - 2.5, point.y - 2.5),
        pos2(point.x + 2.5, point.y + 2.5),
    );
    let response = ui.allocate_rect(rect, egui::Sense::drag());

    let interact_pointer_pos = response.interact_pointer_pos();
    if interact_pointer_pos.is_some() {
        color = Color32::from_rgb(255, 255, 255);
    }

    ui.painter().rect_filled(rect, 0.0, color);

    interact_pointer_pos
}
