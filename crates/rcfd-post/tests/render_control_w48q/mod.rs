//! ADR 0151 (job 48Q): a verbatim copy of the base renderer, `render_scalar` and the private functions it calls in
//! `crates/rcfd-post/src/render.rs` at main 6867e40. It is the oracle of the byte-identity checks and the control of the
//! timing checks.
//!
//! The only changes from the base code:
//! * `Image::new` and `Image::put` are free functions;
//! * `mark` is called between the phases (a timer for the profile; it does not touch the data);
//! * the hulls are dropped explicitly before the return, so that the profile can time it.
//!
//! Do not edit the arithmetic: the checks compare the product against this copy.

#![allow(dead_code)]

use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;
use rcfd_post::render::{Image, RenderOptions, Rendered};

pub fn image_new(width: usize, height: usize, bg: [u8; 4]) -> Image {
    let mut rgba = Vec::with_capacity(width * height * 4);
    for _ in 0..width * height {
        rgba.extend_from_slice(&bg);
    }
    Image { width, height, rgba }
}

pub fn put(img: &mut Image, x: i64, y: i64, c: [u8; 4]) {
    if x >= 0 && y >= 0 && (x as usize) < img.width && (y as usize) < img.height {
        let i = 4 * (y as usize * img.width + x as usize);
        img.rgba[i..i + 4].copy_from_slice(&c);
    }
}

/// Viridis colour map (9-stop piecewise-linear approximation), `t` in [0, 1].
pub fn colormap(t: f64) -> [u8; 3] {
    const STOPS: [[f64; 3]; 9] = [
        [68.0, 1.0, 84.0],
        [72.0, 36.0, 117.0],
        [65.0, 68.0, 135.0],
        [53.0, 95.0, 141.0],
        [42.0, 120.0, 142.0],
        [34.0, 168.0, 132.0],
        [68.0, 191.0, 112.0],
        [122.0, 209.0, 81.0],
        [253.0, 231.0, 37.0],
    ];
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    let x = t * (STOPS.len() - 1) as f64;
    let i = (x.floor() as usize).min(STOPS.len() - 2);
    let f = x - i as f64;
    [0, 1, 2].map(|k| (STOPS[i][k] * (1.0 - f) + STOPS[i + 1][k] * f).round() as u8)
}

pub struct View {
    pub u: usize,
    pub v: usize,
    pub axis: usize,
    pub slice: f64,
    pub bounds: ([f64; 2], [f64; 2]),
}

pub fn view(mesh: &Mesh, opt: &RenderOptions) -> View {
    let pts = mesh.points();
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in pts {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let axis = opt.axis.unwrap_or_else(|| {
        (0..3)
            .min_by(|&a, &b| (hi[a] - lo[a]).total_cmp(&(hi[b] - lo[b])))
            .unwrap_or(2)
    });
    let (u, v) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    View {
        u,
        v,
        axis,
        slice: opt.slice.unwrap_or(0.5 * (lo[axis] + hi[axis])),
        bounds: ([lo[u], hi[u]], [lo[v], hi[v]]),
    }
}

pub fn convex_hull(mut p: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-14 && (a[1] - b[1]).abs() < 1e-14);
    if p.len() < 3 {
        return p;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut h: Vec<[f64; 2]> = Vec::new();
    for pass in 0..2 {
        let start = h.len();
        let it: Box<dyn Iterator<Item = &[f64; 2]>> = if pass == 0 {
            Box::new(p.iter())
        } else {
            Box::new(p.iter().rev())
        };
        for &q in it {
            while h.len() >= start + 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
                h.pop();
            }
            h.push(q);
        }
        h.pop();
    }
    h
}

pub fn fill_polygon(img: &mut Image, poly: &[[f64; 2]], color: [u8; 4]) {
    if poly.len() < 3 {
        return;
    }
    let (ymin, ymax) = poly
        .iter()
        .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
    let y0 = (ymin - 0.5).ceil().max(0.0) as i64;
    let y1 = (ymax - 0.5).floor().min(img.height as f64 - 1.0) as i64;
    for y in y0..=y1 {
        let yc = y as f64 + 0.5;
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            if (a[1] <= yc && b[1] > yc) || (b[1] <= yc && a[1] > yc) {
                xs.push(a[0] + (yc - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks_exact(2) {
            let xa = (pair[0] - 0.5).ceil().max(0.0) as i64;
            let xb = (pair[1] - 0.5).floor().min(img.width as f64 - 1.0) as i64;
            for x in xa..=xb {
                put(img, x, y, color);
            }
        }
    }
}

/// 3x5 glyphs for number labels (rows top to bottom, 3 bits each, MSB left).
fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [7, 1, 7, 4, 7],
        '3' => [7, 1, 7, 1, 7],
        '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 7, 1, 7],
        '6' => [7, 4, 7, 5, 7],
        '7' => [7, 1, 1, 1, 1],
        '8' => [7, 5, 7, 5, 7],
        '9' => [7, 5, 7, 1, 7],
        '-' => [0, 0, 7, 0, 0],
        '+' => [0, 2, 7, 2, 0],
        '.' => [0, 0, 0, 0, 2],
        'e' => [0, 7, 7, 4, 7],
        _ => [0; 5],
    }
}

/// Draw `text` with the built-in 3x5 font at integer scale `s`.
pub fn draw_text(img: &mut Image, x: i64, y: i64, text: &str, s: i64, color: [u8; 4]) {
    for (i, ch) in text.chars().enumerate() {
        let g = glyph(ch);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..3 {
                if bits >> (2 - col) & 1 == 1 {
                    for dy in 0..s {
                        for dx in 0..s {
                            put(img, x + (i as i64 * 4 + col) * s + dx, y + row as i64 * s + dy, color);
                        }
                    }
                }
            }
        }
    }
}

fn label(v: f64) -> String {
    if v != 0.0 && (v.abs() < 1e-2 || v.abs() >= 1e4) {
        format!("{v:.1e}").replace("e0", "e").replace("e-0", "e-")
    } else {
        format!("{v:.3}")
    }
}

pub struct Frame {
    pub w: usize,
    pub h: usize,
    pub margin: f64,
    pub sx: f64,
    pub x0: f64,
    pub y1: f64,
}

pub fn frame(v: &View, opt: &RenderOptions) -> Frame {
    let bar = if opt.colorbar { 70.0 } else { 0.0 };
    let margin = 8.0;
    let (dx, dy) = (
        (v.bounds.0[1] - v.bounds.0[0]).max(1e-300),
        (v.bounds.1[1] - v.bounds.1[0]).max(1e-300),
    );
    let w = opt.width.max(64);
    let draw_w = w as f64 - 2.0 * margin - bar;
    let h = if opt.height > 0 {
        opt.height
    } else {
        (draw_w * dy / dx + 2.0 * margin).ceil() as usize
    }
    .max(32);
    let sx = (draw_w / dx).min((h as f64 - 2.0 * margin) / dy);
    Frame {
        w,
        h,
        margin,
        sx,
        x0: v.bounds.0[0],
        y1: v.bounds.1[1],
    }
}

pub fn map_point(f: &Frame, v: &View, p: Vec3) -> [f64; 2] {
    [f.margin + (p[v.u] - f.x0) * f.sx, f.margin + (f.y1 - p[v.v]) * f.sx]
}

pub fn cell_polygon(mesh: &Mesh, c: usize, v: &View, f: &Frame) -> Option<Vec<[f64; 2]>> {
    let pts = mesh.points();
    let mut proj = Vec::new();
    let (mut zlo, mut zhi) = (f64::MAX, f64::MIN);
    for &face in mesh.cell_faces(c) {
        for &pi in mesh.face_points(face) {
            let p = pts[pi];
            zlo = zlo.min(p[v.axis]);
            zhi = zhi.max(p[v.axis]);
            proj.push(map_point(f, v, p));
        }
    }
    if v.slice < zlo - 1e-12 || v.slice > zhi + 1e-12 {
        return None;
    }
    Some(convex_hull(proj))
}

/// The base `render_scalar`.
pub fn render_scalar(mesh: &Mesh, values: &[f64], opt: &RenderOptions) -> Result<Rendered> {
    render_scalar_marked(mesh, values, opt, &mut |_| {})
}

/// The base `render_scalar` with a call of `mark` after each phase.
pub fn render_scalar_marked(
    mesh: &Mesh,
    values: &[f64],
    opt: &RenderOptions,
    mark: &mut dyn FnMut(&'static str),
) -> Result<Rendered> {
    if values.len() != mesh.n_cells() {
        return Err(Error::InvalidValue {
            path: "values".into(),
            reason: format!("{} values for {} cells", values.len(), mesh.n_cells()),
        });
    }
    let v = view(mesh, opt);
    mark("view");
    let fr = frame(&v, opt);
    let mut img = image_new(fr.w, fr.h, [255, 255, 255, 255]);
    mark("frame_image");
    let polys: Vec<(usize, Vec<[f64; 2]>)> = (0..mesh.n_cells())
        .filter_map(|c| cell_polygon(mesh, c, &v, &fr).map(|p| (c, p)))
        .collect();
    mark("polygons");
    let finite = polys.iter().map(|(c, _)| values[*c]).filter(|x| x.is_finite());
    let (dmin, dmax) = finite.fold((f64::MAX, f64::MIN), |a, x| (a.0.min(x), a.1.max(x)));
    let (vmin, vmax) = opt
        .range
        .unwrap_or(if dmin <= dmax { (dmin, dmax) } else { (0.0, 1.0) });
    let span = if vmax > vmin { vmax - vmin } else { 1.0 };
    mark("range");
    for (c, poly) in &polys {
        let [r, g, b] = colormap((values[*c] - vmin) / span);
        fill_polygon(&mut img, poly, [r, g, b, 255]);
    }
    mark("fill");
    if opt.colorbar {
        let x0 = fr.w as i64 - 62;
        let (top, bot) = (fr.margin as i64 + 8, fr.h as i64 - fr.margin as i64 - 8);
        for y in top..=bot {
            let t = 1.0 - (y - top) as f64 / (bot - top).max(1) as f64;
            let [r, g, b] = colormap(t);
            for x in x0..x0 + 12 {
                put(&mut img, x, y, [r, g, b, 255]);
            }
        }
        draw_text(&mut img, x0 - 2, top - 8, &label(vmax), 1, [0, 0, 0, 255]);
        draw_text(&mut img, x0 - 2, bot + 3, &label(vmin), 1, [0, 0, 0, 255]);
    }
    mark("colorbar");
    let cells_drawn = polys.len();
    drop(polys);
    mark("free");
    Ok(Rendered {
        image: img,
        vmin,
        vmax,
        cells_drawn,
    })
}
