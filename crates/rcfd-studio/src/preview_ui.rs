//! Drawing of the case preview (ADR 0138; live from the case editor, ADR 0144) on the Viewport panel's Mesh side: status,
//! counts and controls, the image through the run viewport's own [`crate::viewport_ui::image`] (texture, mouse, pick),
//! the 2D overlay of cell edges and boundary patches, the patch legend and the hover readout. State and decisions are
//! in [`crate::preview`].

use crate::preview::{self, kind_name, Built, Colouring, Status, FRONT_2D, SINGLE};
use crate::studio::Studio;
use crate::viewport::MOUSE_HINT;
use egui::{Color32, Pos2, Rect, Sense};
use std::sync::Arc;
use std::time::Duration;

const ERR: Color32 = Color32::from_rgb(200, 60, 60);
const WARN: Color32 = Color32::from_rgb(200, 140, 0);
const LEGEND_W: f32 = 190.0;
/// Width of a 2D boundary outline, points.
pub const OUTLINE_W: f32 = 3.0;
/// Width of a 2D cell edge, points.
pub const EDGE_W: f32 = 1.0;
/// Hover distance to a 2D boundary outline, points.
pub const HOVER_PT: f32 = 6.0;
/// Colour of the 2D cell edges.
pub const EDGE_COLOUR: Color32 = Color32::from_rgb(90, 90, 90);
/// Lines of a failure message shown before the rest is folded away.
const SHOWN_LINES: usize = 4;
/// Source line of a mesh built from the case file (ADR 0144).
pub const FROM_DISK: &str = "(built from the case file on disk; the solver is not run)";
/// Source line of a mesh built from the case editor's unsaved text (ADR 0144).
pub const FROM_EDITOR: &str = "(built from the unsaved edits in the case editor; the solver is not run)";
/// Shown while the case editor holds unsaved edits of the previewed case (ADR 0144; Run still refuses them).
pub const UNSAVED: &str =
    "The mesh follows the unsaved edits in the case editor; a run uses the file on disk, so save the case before Run.";
/// Start of the line shown while the picture is stale (ADR 0144); the reason follows.
pub const STALE: &str = "Stale: the picture is the last mesh that built";
/// The badge painted on a stale picture (ADR 0144).
pub const STALE_BADGE: &str = "stale";

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Draw the preview part of the Viewport panel (the Mesh side).
pub fn panel(ui: &mut egui::Ui, st: &mut Studio, dark: bool) {
    let Some(case) = st.preview.case.clone() else {
        ui.weak(
            "Select a run in the Runs panel to see its stored fields, or a case in the Project panel to see its mesh.",
        );
        if st.viewport.gpu.is_none() {
            ui.weak("(no GPU attached: headless)");
        }
        return;
    };
    let shown = st.preview.displayed().cloned();
    ui.horizontal(|ui| {
        ui.strong(format!("Mesh of {case}"));
        ui.weak(if shown.as_ref().is_some_and(|b| b.from_editor) {
            FROM_EDITOR
        } else {
            FROM_DISK
        });
        if ui
            .add_enabled(!st.preview.building(), egui::Button::new("Rebuild"))
            .on_hover_text(
                "read the case (or the editor's unsaved edits) and its mesh file again (a mesh file outside the project \
                 is not watched)",
            )
            .clicked()
        {
            st.preview.rebuild();
        }
    });
    if st
        .editor
        .as_ref()
        .is_some_and(|e| e.rel == case && (e.draft.is_some() || e.is_dirty()))
    {
        ui.colored_label(WARN, UNSAVED);
    }
    if let Some(why) = st.preview.stale_reason() {
        ui.colored_label(WARN, format!("{STALE}: {why}."));
    }
    match st.preview.status.clone() {
        Status::Idle => {
            ui.weak("Waiting for the project...");
            return;
        }
        Status::Building { since } => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("Building the mesh... {:.1} s", since.elapsed().as_secs_f64()));
            });
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        Status::TooLarge { message, rebuild } => {
            ui.colored_label(WARN, message);
            if ui
                .button(if rebuild { "Build anyway" } else { "Show anyway" })
                .clicked()
            {
                st.preview.anyway();
            }
        }
        Status::Failed(m) => failure(ui, &m),
        Status::Ready => {}
    }
    // the last mesh that built stays on screen (stale) while a newer text builds, fails or is over a limit
    let Some(b) = shown else { return };
    if !st.viewport.switches.no_scale {
        resolution(ui, st, &b);
    }
    ui.monospace(counts(&b));
    controls(ui, st, &b);
    let avail = ui.available_size();
    let img = egui::vec2((avail.x - LEGEND_W).max(16.0), (avail.y - 22.0).max(16.0));
    let mut outline: Option<usize> = None;
    ui.horizontal(|ui| {
        crate::viewport_ui::image(ui, &mut st.viewport, img, dark);
        if b.two_d {
            outline = overlay(ui, st, &b);
        }
        // ADR 0148: the scale of a 2D view, over the outlines
        crate::viewport_ui::scale_bar_over(ui, &st.viewport);
        if st.preview.stale() {
            if let Some(r) = st.viewport.image_rect {
                badge(ui, r);
            }
        }
        legend(ui, st, &b, img.y);
    });
    let face = st.viewport.hover.as_ref().map(|h| (h.face, h.cell));
    match preview::readout(&b, face, outline) {
        Some(t) => ui.monospace(t),
        None => ui.weak(if b.two_d {
            "hover over a coloured outline for its boundary patch, or over the mesh for its cell"
        } else {
            "hover over the surface for its patch"
        }),
    };
}

/// The "stale" badge in the image's top-left corner (ADR 0144).
fn badge(ui: &mut egui::Ui, image: Rect) {
    let p = ui.painter_at(image);
    let back = p.add(egui::Shape::Noop); // the background goes under the text, sized by it
    let r = p.text(
        image.min + egui::vec2(10.0, 8.0),
        egui::Align2::LEFT_TOP,
        STALE_BADGE,
        egui::FontId::proportional(13.0),
        Color32::BLACK,
    );
    p.set(
        back,
        egui::Shape::rect_filled(r.expand2(egui::vec2(5.0, 2.0)), 3.0, WARN),
    );
}

/// The label of the Mesh side's refine button (ADR 0148).
pub const REFINE: &str = "Refine x2";

/// The resolution line of the Mesh side (ADR 0148): the domain and the cell counts ([`preview::resolution_text`]),
/// with "Refine x2" for a generator mesh (its hover text gives the counts after, the cells and the runner's memory
/// estimate, and the preview's cell limit when the finer mesh is over it). Greyed out, with the reason, while the case
/// editor does not hold the case as shown ([`preview::refine_blocked`]).
fn resolution(ui: &mut egui::Ui, st: &mut Studio, b: &Built) {
    let mut go = false;
    ui.horizontal(|ui| {
        ui.monospace(preview::resolution_text(b));
        match preview::refine_offer(b, &st.preview.limits) {
            preview::RefineOffer::Refine(r) => {
                let why = preview::refine_blocked(st);
                let label = if r.over_limit.is_some() {
                    egui::RichText::new(REFINE).color(WARN)
                } else {
                    egui::RichText::new(REFINE)
                };
                let resp = ui
                    .add_enabled(why.is_none(), egui::Button::new(label))
                    .on_hover_text(preview::refine_hover(&r))
                    .on_disabled_hover_text(why.clone().unwrap_or_default());
                st.preview.rects.insert(REFINE, resp.rect);
                go = resp.clicked();
            }
            preview::RefineOffer::MeshFile(_) => {
                st.preview.rects.remove(REFINE);
            }
            preview::RefineOffer::NotOffered(why) => {
                st.preview.rects.remove(REFINE);
                ui.weak("(no Refine x2)").on_hover_text(why);
            }
        }
    });
    if go {
        // the message goes to the status bar either way
        if let Err(m) = preview::refine(st) {
            st.say(m);
        }
    }
}

/// Counts of the built mesh.
pub fn counts(b: &Built) -> String {
    format!(
        "{} cells, {} faces, {} patches; drawn surface: {} faces, {} triangles; {}; built in {:.2} s",
        b.cells,
        b.faces,
        b.patches.len(),
        b.surface.n_faces(),
        b.triangles,
        if b.two_d { "2D, front view" } else { "3D" },
        b.build_s
    )
}

fn failure(ui: &mut egui::Ui, m: &str) {
    let lines: Vec<&str> = m.lines().collect();
    for l in lines.iter().take(SHOWN_LINES) {
        ui.colored_label(ERR, *l);
    }
    if lines.len() > SHOWN_LINES {
        egui::CollapsingHeader::new(format!("and {} more", lines.len() - SHOWN_LINES))
            .id_salt("preview-failure")
            .show(ui, |ui| {
                for l in &lines[SHOWN_LINES..] {
                    ui.colored_label(ERR, *l);
                }
            });
    }
}

fn controls(ui: &mut egui::Ui, st: &mut Studio, b: &Built) {
    ui.horizontal(|ui| {
        if b.two_d {
            let kept = !b.edges.is_empty();
            let mut on = st.preview.cell_edges && kept;
            let r = ui.add_enabled(kept, egui::Checkbox::new(&mut on, "cell edges"));
            if kept {
                if r.changed() {
                    st.preview.cell_edges = on;
                }
            } else {
                r.on_disabled_hover_text(format!(
                    "{} edges, more than the overlay limit of {}",
                    b.n_edges, st.preview.limits.overlay_edges
                ));
            }
            ui.weak(format!(
                "face-on orthographic view; the boundary patches are the coloured outlines; {MOUSE_HINT}"
            ));
        } else {
            let c = st.preview.colouring;
            if ui.radio(c == Colouring::Patches, "patch colours").clicked() {
                st.preview.colouring = Colouring::Patches;
            }
            if ui.radio(c == Colouring::Single, "single colour").clicked() {
                st.preview.colouring = Colouring::Single;
            }
            // ADR 0148: the middle drag orbits too
            ui.weak(MOUSE_HINT);
        }
    });
    // the colours follow the controls at once (the model hands them to the viewport in the next drain too)
    let styles = st.preview.part_styles();
    st.viewport.set_part_styles(styles);
}

fn quad(m: &mut egui::Mesh, a: Pos2, b: Pos2, w: f32, c: Color32) {
    let d = b - a;
    let len = d.length();
    if len.is_nan() || len <= 1e-6 {
        return; // a point (or a non-finite projection) draws nothing
    }
    let n = egui::vec2(-d.y, d.x) * (0.5 * w / len);
    let i = m.vertices.len() as u32;
    m.colored_vertex(a + n, c);
    m.colored_vertex(a - n, c);
    m.colored_vertex(b + n, c);
    m.colored_vertex(b - n, c);
    m.add_triangle(i, i + 1, i + 2);
    m.add_triangle(i + 1, i + 3, i + 2);
}

/// The overlay mesh (points) of a 2D preview: cell edges (when `edges`), then the boundary outlines.
pub fn overlay_mesh(b: &Built, cam: &rcfd_render::Camera, size: [u32; 2], rect: Rect, edges: bool) -> egui::Mesh {
    let (w, h) = (size[0].max(1) as f64, size[1].max(1) as f64);
    let to_pt = |p: [f64; 2]| {
        Pos2::new(
            rect.min.x + (p[0] / w) as f32 * rect.width(),
            rect.min.y + (p[1] / h) as f32 * rect.height(),
        )
    };
    let mut m = egui::Mesh::default();
    let n = if edges { b.edges.len() } else { 0 } + b.outlines.len();
    m.reserve_vertices(4 * n);
    m.reserve_triangles(2 * n);
    if edges {
        let px = preview::project_all(cam, size, b.edges.iter().flat_map(|e| [e[0], e[1]]));
        for e in px.chunks(2) {
            if let (Some(a), Some(c)) = (e[0], e[1]) {
                quad(&mut m, to_pt(a), to_pt(c), EDGE_W, EDGE_COLOUR);
            }
        }
    }
    let px = preview::project_all(cam, size, b.outlines.iter().flat_map(|(_, s)| [s[0], s[1]]));
    for ((k, _), e) in b.outlines.iter().zip(px.chunks(2)) {
        if let (Some(a), Some(c)) = (e[0], e[1]) {
            quad(&mut m, to_pt(a), to_pt(c), OUTLINE_W, rgb(b.patches[*k].colour));
        }
    }
    m
}

#[derive(Clone)]
struct Cached {
    key: Vec<u64>,
    mesh: Arc<egui::Mesh>,
}

/// Draw the 2D overlay over the image (cached until the camera, size or content changes); the outline under the
/// pointer.
fn overlay(ui: &mut egui::Ui, st: &Studio, b: &Arc<Built>) -> Option<usize> {
    let vp = &st.viewport;
    let (Some(rect), Some(cam)) = (vp.image_rect, vp.camera) else {
        return None;
    };
    let edges = st.preview.cell_edges && !b.edges.is_empty();
    let c = cam.cam;
    let proj = match c.projection {
        rcfd_render::Projection::Perspective { fovy } => [0.0, fovy],
        rcfd_render::Projection::Orthographic { half_height } => [1.0, half_height],
    };
    let mut key: Vec<u64> = vec![Arc::as_ptr(b) as usize as u64, edges as u64];
    key.extend(
        [
            c.target[0],
            c.target[1],
            c.target[2],
            c.distance,
            c.yaw,
            c.pitch,
            c.near,
            c.far,
            proj[0],
            proj[1],
        ]
        .map(f64::to_bits),
    );
    key.extend([vp.size[0] as u64, vp.size[1] as u64]);
    key.extend([rect.min.x, rect.min.y, rect.max.x, rect.max.y].map(|v| v.to_bits() as u64));
    let id = egui::Id::new("rcfd-studio-preview-overlay");
    let cached: Option<Cached> = ui.ctx().data(|d| d.get_temp(id));
    let mesh = match cached {
        Some(cc) if cc.key == key => cc.mesh,
        _ => {
            let mesh = Arc::new(overlay_mesh(b, &c, vp.size, rect, edges));
            ui.ctx().data_mut(|d| {
                d.insert_temp(
                    id,
                    Cached {
                        key,
                        mesh: mesh.clone(),
                    },
                )
            });
            mesh
        }
    };
    ui.painter_at(rect).add(egui::Shape::Mesh(mesh));
    let pos = ui.input(|i| i.pointer.hover_pos())?;
    if !rect.contains(pos) {
        return None;
    }
    let sx = vp.size[0] as f32 / rect.width().max(1e-3);
    let sy = vp.size[1] as f32 / rect.height().max(1e-3);
    let px = [((pos.x - rect.min.x) * sx) as f64, ((pos.y - rect.min.y) * sy) as f64];
    preview::outline_at(b, &c, vp.size, px, (HOVER_PT * sx) as f64)
}

fn legend(ui: &mut egui::Ui, st: &mut Studio, b: &Built, height: f32) {
    let mut toggle: Option<usize> = None;
    ui.vertical(|ui| {
        ui.set_width(LEGEND_W - 8.0);
        ui.strong("Patches");
        egui::ScrollArea::vertical()
            .id_salt("preview-legend")
            .max_height(height.max(40.0))
            .show(ui, |ui| {
                for (k, p) in b.patches.iter().enumerate() {
                    let colour = if b.two_d && p.drawn {
                        FRONT_2D
                    } else if !b.two_d && st.preview.colouring == Colouring::Single {
                        SINGLE
                    } else {
                        p.colour
                    };
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
                        ui.painter().rect_filled(r, 2.0, rgb(colour));
                        if !b.two_d {
                            let mut shown = !st.preview.hidden.contains(&k);
                            if ui
                                .checkbox(&mut shown, "")
                                .on_hover_text("show or hide this patch")
                                .changed()
                            {
                                toggle = Some(k);
                            }
                        }
                        ui.label(&p.name);
                    });
                    let role = match (b.two_d, p.drawn) {
                        (true, true) => ", front (drawn)",
                        (true, false) => ", outline",
                        _ => "",
                    };
                    ui.weak(format!("  {}, {} faces{role}", kind_name(&p.kind), p.faces));
                }
            });
    });
    if let Some(k) = toggle {
        if !st.preview.hidden.remove(&k) {
            st.preview.hidden.insert(k);
        }
        let styles = st.preview.part_styles();
        st.viewport.set_part_styles(styles);
    }
}
