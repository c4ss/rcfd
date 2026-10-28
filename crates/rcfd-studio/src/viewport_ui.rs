//! Drawing of the Viewport panel (ADR 0132): field, range and time controls, the rendered texture, the legend and the
//! hover readout. State and decisions are in [`crate::viewport`].

use crate::studio::Studio;
use crate::viewport::{Axis, Input, RangeMode, Status, Viewport, LEGEND_TICKS, PLAY_PERIOD};
use egui::{Color32, PointerButton, Pos2, Rect, Sense};
use rcfd_render::ColorMap;
use std::time::Instant;

const ERR: Color32 = Color32::from_rgb(200, 60, 60);
const LEGEND_W: f32 = 120.0;

/// Draw the Viewport panel.
pub fn panel(ui: &mut egui::Ui, st: &mut Studio) {
    let dark = st.theme == crate::theme::Theme::Dark;
    let vp = &mut st.viewport;
    if let Some(e) = vp.gpu_error() {
        ui.colored_label(ERR, e.to_string());
    }
    if vp.run.is_none() {
        ui.weak("Select a run in the Runs panel to see its stored fields.");
        if vp.gpu.is_none() {
            ui.weak("(no GPU attached: headless)");
        }
        return;
    }
    match vp.status.clone() {
        Status::Idle | Status::Loading => {
            ui.weak("Reading the run's field store...");
            return;
        }
        Status::NoStore(m) => {
            ui.weak(m);
            return;
        }
        Status::Failed(m) => {
            ui.colored_label(ERR, m);
            return;
        }
        Status::Ready => {}
    }
    if vp.snaps.is_empty() {
        ui.weak("The store has no snapshots yet.");
        return;
    }
    controls(ui, vp);
    time_row(ui, vp);
    if let Some(e) = &vp.error {
        ui.colored_label(ERR, e.as_str());
    }
    let avail = ui.available_size();
    let img = egui::vec2((avail.x - LEGEND_W).max(16.0), (avail.y - 22.0).max(16.0));
    ui.horizontal(|ui| {
        image(ui, vp, img, dark);
        legend(ui, vp, img.y);
    });
    match &vp.hover {
        Some(h) => ui.monospace(h.text()),
        None => ui.weak("hover over the surface for its value"),
    };
    if vp.playing {
        if vp.tick_play(Instant::now()) {
            ui.ctx().request_repaint();
        }
        ui.ctx().request_repaint_after(PLAY_PERIOD);
    }
}

fn controls(ui: &mut egui::Ui, vp: &mut Viewport) {
    ui.horizontal(|ui| {
        let current = vp.field.clone().unwrap_or_default();
        let mut pick: Option<String> = None;
        egui::ComboBox::from_label("field")
            .selected_text(current.as_str())
            .show_ui(ui, |ui| {
                for f in &vp.fields {
                    if ui.selectable_label(*f == current, f).clicked() && *f != current {
                        pick = Some(f.clone());
                    }
                }
            });
        if let Some(f) = pick {
            vp.set_field(&f);
        }
        ui.separator();
        ui.label("range");
        let mode = vp.range_mode;
        let mut next = mode;
        if ui.radio(mode == RangeMode::Snapshot, "snapshot").clicked() {
            next = RangeMode::Snapshot;
        }
        if ui.radio(mode == RangeMode::Run, "whole run").clicked() {
            next = RangeMode::Run;
        }
        let fixed = matches!(mode, RangeMode::Fixed(..));
        if ui.radio(fixed, "fixed").clicked() && !fixed {
            let (a, b) = vp.drawn_range().unwrap_or((0.0, 1.0));
            next = RangeMode::Fixed(a, if b > a { b } else { a + 1.0 });
        }
        if let RangeMode::Fixed(mut a, mut b) = next {
            let ra = ui.add(egui::DragValue::new(&mut a).speed(0.0).prefix("min "));
            let rb = ui.add(egui::DragValue::new(&mut b).speed(0.0).prefix("max "));
            if ra.changed() || rb.changed() {
                next = RangeMode::Fixed(a, b);
            }
        }
        if next != mode {
            if let Err(e) = vp.set_range_mode(next) {
                vp.error = Some(e);
            }
        }
    });
}

fn time_row(ui: &mut egui::Ui, vp: &mut Viewport) {
    let axis = vp.axis();
    ui.horizontal(|ui| {
        if ui.button("|<").on_hover_text("first snapshot").clicked() {
            vp.show_snapshot(0);
        }
        if ui.button("<").on_hover_text("previous snapshot").clicked() {
            vp.step(-1);
        }
        if ui
            .button(if vp.playing { "pause" } else { "play" })
            .clicked()
        {
            vp.toggle_play();
        }
        if ui.button(">").on_hover_text("next snapshot").clicked() {
            vp.step(1);
        }
        if ui.button(">|").on_hover_text("last snapshot").clicked() {
            vp.show_snapshot(vp.snaps.len() - 1);
        }
        let (a, b) = vp.bounds();
        let mut pos = vp.pos;
        let mut s = egui::Slider::new(&mut pos, a..=b.max(a)).text(match axis {
            Axis::Time => "time",
            Axis::Step => "step",
        });
        if axis == Axis::Step {
            s = s.step_by(1.0);
        }
        let r = ui.add(s);
        if r.changed() {
            vp.set_pos(pos);
        } else if !r.dragged() && !vp.linear {
            // nearest: the slider snaps to the stored snapshot shown
            if let Some(f) = &vp.frame {
                vp.pos = vp.position_of(f.index);
            }
        }
        let mut lin = vp.linear;
        if ui
            .add_enabled(axis == Axis::Time, egui::Checkbox::new(&mut lin, "linear"))
            .on_disabled_hover_text("linear in time needs distinct snapshot times (a steady run stores time 0)")
            .changed()
        {
            vp.set_linear(lin);
        }
    });
    if let Some(f) = &vp.frame {
        ui.monospace(format!(
            "snapshot {}/{}  step {}  t = {}{}{}",
            f.index + 1,
            vp.snaps.len(),
            if f.interpolated {
                format!("{:.3}", f.step)
            } else {
                format!("{}", f.step)
            },
            f.time,
            if f.interpolated { "  interpolated" } else { "" },
            if vp.pending() { "  (reading...)" } else { "" }
        ));
    }
}

fn image(ui: &mut egui::Ui, vp: &mut Viewport, size: egui::Vec2, dark: bool) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    vp.image_rect = Some(rect);
    let ppp = ui.ctx().pixels_per_point();
    let px = |v: f32| ((v * ppp).round() as u32).max(1);
    vp.set_size(px(rect.width()), px(rect.height()), dark);
    let p = ppp as f64;
    let d = resp.drag_delta();
    if resp.dragged_by(PointerButton::Primary) && d != egui::Vec2::ZERO {
        vp.input(Input::Orbit(d.x as f64 * p, d.y as f64 * p));
    }
    if (resp.dragged_by(PointerButton::Secondary) || resp.dragged_by(PointerButton::Middle)) && d != egui::Vec2::ZERO {
        vp.input(Input::Pan(d.x as f64 * p, d.y as f64 * p));
    }
    if resp.hovered() {
        let y = ui.input(|i| i.smooth_scroll_delta.y);
        if y != 0.0 {
            let speed = ui.ctx().options(|o| o.input_options.line_scroll_speed).max(1e-3);
            vp.input(Input::Dolly((y / speed) as f64));
        }
    }
    if resp.double_clicked() {
        vp.input(Input::Fit);
    }
    vp.prepare();
    let tex = vp.gpu().and_then(|g| g.tex_id);
    match tex {
        Some(id) => {
            ui.painter().image(
                id,
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        None => {
            ui.painter().rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
            let text = if vp.gpu().is_some() {
                "rendered offscreen (no egui renderer attached)"
            } else {
                "no GPU attached"
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(13.0),
                ui.visuals().weak_text_color(),
            );
        }
    }
    let dragging = resp.dragged();
    match resp.hover_pos() {
        Some(pos) if !dragging => {
            let v = (pos - rect.min) * ppp;
            if v.x >= 0.0 && v.y >= 0.0 {
                vp.hover = vp.hover_at(v.x.floor() as u32, v.y.floor() as u32);
            }
        }
        _ => vp.hover = None,
    }
}

fn legend(ui: &mut egui::Ui, vp: &Viewport, height: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(LEGEND_W - 8.0, height), Sense::hover());
    let Some(l) = vp.legend() else { return };
    let painter = ui.painter_at(rect);
    let bar = Rect::from_min_size(rect.min + egui::vec2(4.0, 8.0), egui::vec2(16.0, (height - 16.0).max(8.0)));
    let map = ColorMap::viridis();
    let n = 64;
    for k in 0..n {
        // bottom = low end
        let t = (k as f64 + 0.5) / n as f64;
        let c = map.eval(t);
        let y1 = bar.max.y - bar.height() * k as f32 / n as f32;
        let y0 = bar.max.y - bar.height() * (k + 1) as f32 / n as f32;
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(bar.min.x, y0), Pos2::new(bar.max.x, y1)),
            0.0,
            Color32::from_rgb(c[0], c[1], c[2]),
        );
    }
    let font = egui::FontId::monospace(11.0);
    for (k, label) in l.labels.iter().enumerate() {
        let y = bar.max.y - bar.height() * k as f32 / (LEGEND_TICKS - 1) as f32;
        painter.line_segment(
            [Pos2::new(bar.max.x, y), Pos2::new(bar.max.x + 4.0, y)],
            egui::Stroke::new(1.0, ui.visuals().text_color()),
        );
        painter.text(
            Pos2::new(bar.max.x + 6.0, y),
            egui::Align2::LEFT_CENTER,
            label,
            font.clone(),
            ui.visuals().text_color(),
        );
    }
    if let Some(f) = &vp.field {
        painter.text(
            Pos2::new(bar.min.x, rect.min.y),
            egui::Align2::LEFT_TOP,
            f,
            font,
            ui.visuals().strong_text_color(),
        );
    }
}
