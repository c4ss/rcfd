//! Drawing of the Viewport panel (ADR 0132): field, range and time controls, the rendered texture, the legend and the
//! hover readout. State and decisions are in [`crate::viewport`].

use crate::studio::Studio;
use crate::view_toggle::Side;
use crate::viewport::{
    drag_input, slider_words, step_time_text, time_hover, Axis, DragButton, Input, RangeMode, Status, Viewport,
    LEGEND_TICKS, MOUSE_HINT, PLAY_PERIOD,
};
use egui::{Color32, PointerButton, Pos2, Rect, Sense};
use rcfd_render::ColorMap;
use std::time::Instant;

const ERR: Color32 = Color32::from_rgb(200, 60, 60);
const LEGEND_W: f32 = 120.0;

/// Draw the Viewport panel.
pub fn panel(ui: &mut egui::Ui, st: &mut Studio) {
    let dark = st.theme == crate::theme::Theme::Dark;
    toggle(ui, st);
    if let Some(e) = st.viewport.gpu_error() {
        ui.colored_label(ERR, e.to_string());
    }
    if st.viewport.run.is_none() {
        // ADR 0138, ADR 0144: the Mesh side, the mesh of the open case
        crate::preview_ui::panel(ui, st, dark);
        return;
    }
    let vp = &mut st.viewport;
    match vp.status.clone() {
        Status::Idle | Status::Loading => {
            ui.weak("Reading the run's field store...");
            return;
        }
        Status::NoStore(m) => {
            // ADR 0140 D8, D9: why there is nothing to show, and the re-run when it is offered
            if crate::fields_ui::empty_state(ui, vp.empty.as_ref(), &m, true) {
                if let Some(id) = vp.run.clone() {
                    if let Err(e) = st.rerun_with_fields(&id) {
                        st.say(e);
                    }
                }
            }
            return;
        }
        Status::Failed(m) => {
            ui.colored_label(ERR, m);
            return;
        }
        Status::Ready => {}
    }
    if vp.snaps.is_empty() {
        crate::fields_ui::empty_state(ui, vp.empty.as_ref(), "The store has no snapshots yet.", false);
        return;
    }
    if let Some(n) = &vp.store_note {
        ui.weak(n.as_str());
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
        scale_bar_over(ui, vp);
        legend(ui, vp, img.y);
    });
    match &vp.hover {
        Some(h) => ui.monospace(h.text()),
        None => ui.weak(format!("hover over the surface for its value; {MOUSE_HINT}")),
    };
    if vp.playing {
        if vp.tick_play(Instant::now()) {
            ui.ctx().request_repaint();
        }
        ui.ctx().request_repaint_after(PLAY_PERIOD);
    }
}

/// The Mesh / Run toggle (ADR 0144): which side the panel shows; a side with nothing to show is disabled and says why.
fn toggle(ui: &mut egui::Ui, st: &mut Studio) {
    let mut pick: Option<Side> = None;
    ui.horizontal(|ui| {
        for side in Side::BOTH {
            // ADR 0149: the Run side of a case without runs names the case
            let why = st.view_refusal(side);
            let r = ui
                .add_enabled(
                    why.is_none(),
                    egui::Button::selectable(st.view.side == side, side.label()),
                )
                .on_hover_text(side.hover())
                .on_disabled_hover_text(why.clone().unwrap_or_default());
            st.view.rects.insert(side.label(), r.rect);
            st.view.enabled.insert(side.label(), why.is_none());
            if r.clicked() {
                pick = Some(side);
            }
        }
        if let Some(c) = st.view_caption() {
            ui.separator();
            ui.weak(c);
            // ADR 0148: the run's case, the size of its domain and its cells, from the run's own stored mesh
            if let Some(r) = st.run_readout() {
                ui.weak(r);
            }
        }
    });
    if let Some(side) = pick {
        if let Err(m) = st.set_view(side) {
            st.say(m);
        }
        ui.ctx().request_repaint();
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
        ui.separator();
        if ui
            .button("Save PNG...")
            .on_hover_text("the shown image, pixel for pixel (for comparing with a screenshot)")
            .clicked()
        {
            save_png(vp);
        }
    });
}

/// Read the image back, ask where to save it, and write it on a background thread.
fn save_png(vp: &mut Viewport) {
    let img = match vp.snapshot_image() {
        Ok(i) => i,
        Err(e) => {
            vp.error = Some(e);
            return;
        }
    };
    let step = vp.frame.as_ref().map_or(0.0, |f| f.step);
    let name = format!("{}-step{step}.png", vp.run.as_deref().unwrap_or("viewport"));
    let Some(path) = rfd::FileDialog::new()
        .add_filter("PNG", &["png"])
        .set_file_name(name)
        .save_file()
    else {
        return;
    };
    std::thread::spawn(move || {
        let r = img
            .to_png()
            .map_err(|e| e.to_string())
            .and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()));
        if let Err(e) = r {
            eprintln!("rcfd-studio: could not save {}: {e}", path.display());
        }
    });
}

fn time_row(ui: &mut egui::Ui, vp: &mut Viewport) {
    let axis = vp.axis();
    // ADR 0148: every time says what it is (simulated time in seconds; a steady run's iterations)
    let (kind, bare) = (vp.kind(), vp.switches.bare_time);
    ui.horizontal(|ui| {
        if ui.button("|<").on_hover_text("first snapshot").clicked() {
            vp.show_snapshot(0);
        }
        if ui.button("<").on_hover_text("previous snapshot").clicked() {
            vp.step(-1);
        }
        if ui.button(if vp.playing { "pause" } else { "play" }).clicked() {
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
        let mut s = egui::Slider::new(&mut pos, a..=b.max(a)).text(slider_words(axis, kind, bare));
        if axis == Axis::Step {
            s = s.step_by(1.0);
        }
        let mut r = ui.add(s);
        if !bare {
            r = r.on_hover_text(time_hover(kind));
        }
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
        // ADR 0141: follow the newest snapshot of a running run; scrubbing back turns it off
        let mut follow = vp.follow;
        if ui
            .checkbox(&mut follow, "follow latest")
            .on_hover_text("show each new snapshot as the run writes it; scrubbing to an older one turns this off")
            .changed()
        {
            vp.set_follow(follow);
        }
    });
    if let Some(f) = &vp.frame {
        let step = if f.interpolated {
            format!("{:.3}", f.step)
        } else {
            format!("{}", f.step)
        };
        let r = ui.monospace(format!(
            "snapshot {}/{}  {}{}{}",
            f.index + 1,
            vp.snaps.len(),
            step_time_text(kind, &step, f.time, bare),
            if f.interpolated { "  interpolated" } else { "" },
            if vp.pending() { "  (reading...)" } else { "" }
        ));
        if !bare {
            r.on_hover_text(time_hover(kind));
        }
    }
}

/// The rendered texture with mouse input and hover (also used by the case preview, ADR 0138).
pub(crate) fn image(ui: &mut egui::Ui, vp: &mut Viewport, size: egui::Vec2, dark: bool) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    vp.image_rect = Some(rect);
    let ppp = ui.ctx().pixels_per_point();
    let px = |v: f32| ((v * ppp).round() as u32).max(1);
    vp.set_size(px(rect.width()), px(rect.height()), dark);
    let p = ppp as f64;
    let d = resp.drag_delta();
    // ADR 0148: the middle drag orbits, the secondary drag pans, the primary (left) drag does nothing ([`drag_input`])
    let middle_pans = vp.switches.middle_pans;
    for (pb, b) in [
        (PointerButton::Primary, DragButton::Primary),
        (PointerButton::Secondary, DragButton::Secondary),
        (PointerButton::Middle, DragButton::Middle),
    ] {
        if resp.dragged_by(pb) && d != egui::Vec2::ZERO {
            if let Some(i) = drag_input(b, d.x as f64 * p, d.y as f64 * p, middle_pans) {
                vp.input(i);
            }
        }
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

/// The scale bar of a 2D view (ADR 0148) over the image last drawn ([`Viewport::image_rect`]), drawn after anything
/// else over the image (the preview's outlines) so that it stays readable. Nothing for a 3D view.
pub(crate) fn scale_bar_over(ui: &egui::Ui, vp: &Viewport) {
    let Some(rect) = vp.image_rect else { return };
    if let Some(bar) = vp.scale_bar(rect.width(), rect.height()) {
        scale_bar(ui, rect, &bar);
    }
}

/// A scale bar in the image's bottom-left corner: a bar of `bar.points` with end ticks and its length above it, on a
/// translucent plate so it reads on either background.
fn scale_bar(ui: &egui::Ui, image: Rect, bar: &crate::viewport::ScaleBar) {
    let p = ui.painter_at(image);
    let ink = ui.visuals().strong_text_color();
    let x0 = image.min.x + 14.0;
    let y = image.max.y - 12.0;
    let x1 = x0 + bar.points;
    let font = egui::FontId::proportional(12.0);
    let plate = p.add(egui::Shape::Noop);
    let r = p.text(
        Pos2::new(0.5 * (x0 + x1), y - 5.0),
        egui::Align2::CENTER_BOTTOM,
        &bar.label,
        font,
        ink,
    );
    let area = r.union(Rect::from_min_max(Pos2::new(x0, y - 5.0), Pos2::new(x1, y + 5.0)));
    p.set(
        plate,
        egui::Shape::rect_filled(
            area.expand(4.0),
            3.0,
            ui.visuals().extreme_bg_color.gamma_multiply(0.75),
        ),
    );
    let stroke = egui::Stroke::new(2.0, ink);
    p.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], stroke);
    for x in [x0, x1] {
        p.line_segment([Pos2::new(x, y - 5.0), Pos2::new(x, y + 5.0)], stroke);
    }
}

fn legend(ui: &mut egui::Ui, vp: &Viewport, height: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(LEGEND_W - 8.0, height), Sense::hover());
    let Some(l) = vp.legend() else { return };
    let painter = ui.painter_at(rect);
    let bar = Rect::from_min_size(
        rect.min + egui::vec2(4.0, 8.0),
        egui::vec2(16.0, (height - 16.0).max(8.0)),
    );
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
