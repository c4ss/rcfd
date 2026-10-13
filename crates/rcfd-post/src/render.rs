//! Headless renderer: PNG contour and vector previews of cell fields (F-073).
//!
//! Meshes are drawn in the plane normal to `axis` (default: the thinnest
//! direction of the bounding box, which is z for the usual 2D case).
//! 3D meshes are sliced: only cells whose extent along `axis` contains the
//! slice coordinate are drawn. Each cell is filled with the colour of its
//! value (flat shading), so what you see is exactly the cell data.

use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;

/// RGBA raster image.
#[derive(Debug, Clone)]
pub struct Image {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Pixels, row-major from the top, RGBA bytes.
    pub rgba: Vec<u8>,
}

impl Image {
    fn new(width: usize, height: usize, bg: [u8; 4]) -> Self {
        let mut rgba = Vec::with_capacity(width * height * 4);
        for _ in 0..width * height {
            rgba.extend_from_slice(&bg);
        }
        Image { width, height, rgba }
    }

    /// Pixel colour.
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let i = 4 * (y * self.width + x);
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }

    fn put(&mut self, x: i64, y: i64, c: [u8; 4]) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            let i = 4 * (y as usize * self.width + x as usize);
            self.rgba[i..i + 4].copy_from_slice(&c);
        }
    }

    /// Encode as PNG.
    pub fn to_png(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.width as u32, self.height as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().map_err(|e| Error::Io(format!("png: {e}")))?;
            w.write_image_data(&self.rgba)
                .map_err(|e| Error::Io(format!("png: {e}")))?;
        }
        Ok(out)
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

/// Rendering options.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Image width in pixels (the height follows the aspect ratio unless `height` is set).
    pub width: usize,
    /// Explicit height; 0 = derive from the aspect ratio.
    pub height: usize,
    /// Axis normal to the view plane (0, 1, 2); `None` picks the thinnest direction.
    pub axis: Option<usize>,
    /// Slice coordinate along `axis` (`None` = middle of the bounding box).
    pub slice: Option<f64>,
    /// Colour range; `None` takes min/max of the drawn data.
    pub range: Option<(f64, f64)>,
    /// Draw the colour bar with min/max labels.
    pub colorbar: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            width: 640,
            height: 0,
            axis: None,
            slice: None,
            range: None,
            colorbar: true,
        }
    }
}

/// Image plus the data range that was mapped to the colour bar.
#[derive(Debug, Clone)]
pub struct Rendered {
    /// The image.
    pub image: Image,
    /// Value mapped to the bottom of the colour map.
    pub vmin: f64,
    /// Value mapped to the top of the colour map.
    pub vmax: f64,
    /// Number of cells drawn.
    pub cells_drawn: usize,
}

struct View {
    u: usize,
    v: usize,
    axis: usize,
    slice: f64,
    bounds: ([f64; 2], [f64; 2]),
}

fn view(mesh: &Mesh, opt: &RenderOptions) -> View {
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

fn convex_hull(mut p: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
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

fn fill_polygon(img: &mut Image, poly: &[[f64; 2]], color: [u8; 4]) {
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
                img.put(x, y, color);
            }
        }
    }
}

fn line(img: &mut Image, a: (f64, f64), b: (f64, f64), color: [u8; 4]) {
    let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as usize).max(1);
    for i in 0..=n {
        let t = i as f64 / n as f64;
        img.put(
            (a.0 + t * (b.0 - a.0)).round() as i64,
            (a.1 + t * (b.1 - a.1)).round() as i64,
            color,
        );
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
                            img.put(x + (i as i64 * 4 + col) * s + dx, y + row as i64 * s + dy, color);
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

struct Frame {
    w: usize,
    h: usize,
    margin: f64,
    sx: f64,
    x0: f64,
    y1: f64,
}

fn frame(v: &View, opt: &RenderOptions) -> Frame {
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

fn map_point(f: &Frame, v: &View, p: Vec3) -> [f64; 2] {
    [f.margin + (p[v.u] - f.x0) * f.sx, f.margin + (f.y1 - p[v.v]) * f.sx]
}

fn cell_polygon(mesh: &Mesh, c: usize, v: &View, f: &Frame) -> Option<Vec<[f64; 2]>> {
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

/// Render a cell scalar field as a flat-shaded contour image.
pub fn render_scalar(mesh: &Mesh, values: &[f64], opt: &RenderOptions) -> Result<Rendered> {
    if values.len() != mesh.n_cells() {
        return Err(Error::InvalidValue {
            path: "values".into(),
            reason: format!("{} values for {} cells", values.len(), mesh.n_cells()),
        });
    }
    let v = view(mesh, opt);
    let fr = frame(&v, opt);
    let mut img = Image::new(fr.w, fr.h, [255, 255, 255, 255]);
    let polys: Vec<(usize, Vec<[f64; 2]>)> = (0..mesh.n_cells())
        .filter_map(|c| cell_polygon(mesh, c, &v, &fr).map(|p| (c, p)))
        .collect();
    let finite = polys.iter().map(|(c, _)| values[*c]).filter(|x| x.is_finite());
    let (dmin, dmax) = finite.fold((f64::MAX, f64::MIN), |a, x| (a.0.min(x), a.1.max(x)));
    let (vmin, vmax) = opt
        .range
        .unwrap_or(if dmin <= dmax { (dmin, dmax) } else { (0.0, 1.0) });
    let span = if vmax > vmin { vmax - vmin } else { 1.0 };
    for (c, poly) in &polys {
        let [r, g, b] = colormap((values[*c] - vmin) / span);
        fill_polygon(&mut img, poly, [r, g, b, 255]);
    }
    if opt.colorbar {
        let x0 = fr.w as i64 - 62;
        let (top, bot) = (fr.margin as i64 + 8, fr.h as i64 - fr.margin as i64 - 8);
        for y in top..=bot {
            let t = 1.0 - (y - top) as f64 / (bot - top).max(1) as f64;
            let [r, g, b] = colormap(t);
            for x in x0..x0 + 12 {
                img.put(x, y, [r, g, b, 255]);
            }
        }
        draw_text(&mut img, x0 - 2, top - 8, &label(vmax), 1, [0, 0, 0, 255]);
        draw_text(&mut img, x0 - 2, bot + 3, &label(vmin), 1, [0, 0, 0, 255]);
    }
    Ok(Rendered {
        image: img,
        vmin,
        vmax,
        cells_drawn: polys.len(),
    })
}

/// Draw velocity arrows (in-plane components) on top of an image from [`render_scalar`].
///
/// Roughly `density` x `density` arrows are drawn on the cells nearest to a regular lattice.
pub fn overlay_vectors(
    mesh: &Mesh,
    vectors: &[Vec3],
    opt: &RenderOptions,
    density: usize,
    image: &mut Image,
) -> Result<()> {
    if vectors.len() != mesh.n_cells() {
        return Err(Error::InvalidValue {
            path: "vectors".into(),
            reason: format!("{} vectors for {} cells", vectors.len(), mesh.n_cells()),
        });
    }
    let v = view(mesh, opt);
    let fr = frame(&v, opt);
    let cc = mesh.cell_centres();
    let inplane = |c: usize| (vectors[c][v.u].powi(2) + vectors[c][v.v].powi(2)).sqrt();
    let vmax = (0..mesh.n_cells()).map(inplane).fold(0.0, f64::max).max(1e-300);
    let cells: Vec<usize> = (0..mesh.n_cells())
        .filter(|&c| cell_polygon(mesh, c, &v, &fr).is_some())
        .collect();
    let density = density.max(2);
    let (bu, bv) = v.bounds;
    let mut taken = std::collections::HashSet::new();
    let arrow = (fr.w as f64 / density as f64 * 0.8).max(6.0);
    for c in cells {
        let gi = (((cc[c][v.u] - bu[0]) / (bu[1] - bu[0]).max(1e-300)) * density as f64) as usize;
        let gj = (((cc[c][v.v] - bv[0]) / (bv[1] - bv[0]).max(1e-300)) * density as f64) as usize;
        if !taken.insert((gi, gj)) {
            continue;
        }
        let a = map_point(&fr, &v, cc[c]);
        let s = arrow * inplane(c) / vmax;
        if s < 0.5 {
            continue;
        }
        let (dx, dy) = (vectors[c][v.u] / inplane(c), -vectors[c][v.v] / inplane(c));
        let b = (a[0] + dx * s, a[1] + dy * s);
        line(image, (a[0], a[1]), b, [0, 0, 0, 255]);
        let (hx, hy) = (-dx * 0.3 * s, -dy * 0.3 * s);
        line(image, b, (b.0 + hx - hy * 0.5, b.1 + hy + hx * 0.5), [0, 0, 0, 255]);
        line(image, b, (b.0 + hx + hy * 0.5, b.1 + hy - hx * 0.5), [0, 0, 0, 255]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::gen::{box_mesh, cavity_2d, Grading};

    fn luminance(c: [u8; 3]) -> f64 {
        0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64
    }

    #[test]
    fn colormap_is_monotone_in_luminance_and_clamps() {
        let mut last = -1.0;
        for i in 0..=50 {
            let l = luminance(colormap(i as f64 / 50.0));
            assert!(l >= last - 1e-9, "{i}");
            last = l;
        }
        assert_eq!(colormap(-3.0), colormap(0.0));
        assert_eq!(colormap(7.0), colormap(1.0));
        assert_eq!(colormap(f64::NAN), colormap(0.0));
    }

    #[test]
    fn linear_field_renders_a_left_to_right_gradient() {
        let m = cavity_2d(16, 1.0).unwrap();
        let vals: Vec<f64> = m.cell_centres().iter().map(|c| c[0]).collect();
        let opt = RenderOptions {
            width: 400,
            colorbar: false,
            ..Default::default()
        };
        let r = render_scalar(&m, &vals, &opt).unwrap();
        assert_eq!(r.cells_drawn, 256);
        assert!((r.vmin - 0.5 / 16.0).abs() < 1e-12 && (r.vmax - 15.5 / 16.0).abs() < 1e-12);
        let img = &r.image;
        let y = img.height / 2;
        let row: Vec<f64> = (10..img.width - 10)
            .map(|x| luminance([img.pixel(x, y)[0], img.pixel(x, y)[1], img.pixel(x, y)[2]]))
            .collect();
        assert!(row.windows(2).all(|w| w[1] >= w[0] - 1e-9), "luminance not monotone");
        // squares: the image is (about) square
        assert!((img.width as i64 - img.height as i64).abs() <= 2);
        // the first column is the lowest colour, the last the highest
        assert_eq!(&img.pixel(12, y)[..3], &colormap(0.0)[..]);
        assert_eq!(&img.pixel(img.width - 12, y)[..3], &colormap(1.0)[..]);
    }

    #[test]
    fn png_round_trips_through_a_decoder() {
        let m = cavity_2d(8, 1.0).unwrap();
        let vals: Vec<f64> = (0..64).map(|i| i as f64).collect();
        let r = render_scalar(&m, &vals, &RenderOptions::default()).unwrap();
        let bytes = r.image.to_png().unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let dec = png::Decoder::new(std::io::Cursor::new(&bytes));
        let mut reader = dec.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!(
            (info.width as usize, info.height as usize),
            (r.image.width, r.image.height)
        );
        assert_eq!(&buf[..info.buffer_size()], &r.image.rgba[..]);
    }

    #[test]
    fn slicing_a_3d_box_selects_the_cells_at_the_plane() {
        let m = box_mesh([4, 4, 4], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap();
        let vals: Vec<f64> = m.cell_centres().iter().map(|c| c[2]).collect();
        let opt = RenderOptions {
            axis: Some(2),
            slice: Some(0.3),
            colorbar: false,
            ..Default::default()
        };
        let r = render_scalar(&m, &vals, &opt).unwrap();
        assert_eq!(r.cells_drawn, 16); // one layer of 4x4 cells
        assert!((r.vmin - 0.375).abs() < 1e-12 && (r.vmax - 0.375).abs() < 1e-12);
        assert!(render_scalar(&m, &vals[1..], &opt).is_err());
    }

    #[test]
    fn colorbar_and_labels_are_drawn_and_vectors_overlay() {
        let m = cavity_2d(10, 1.0).unwrap();
        let vals: Vec<f64> = m.cell_centres().iter().map(|c| c[1]).collect();
        let opt = RenderOptions::default();
        let mut r = render_scalar(&m, &vals, &opt).unwrap();
        let x = r.image.width - 62 + 5;
        let y_top = r.image.height / 2;
        assert_ne!(r.image.pixel(x, y_top), [255, 255, 255, 255]);
        let blank = r.image.clone();
        let vecs: Vec<Vec3> = m.cell_centres().iter().map(|c| [c[1] - 0.5, 0.5 - c[0], 0.0]).collect();
        overlay_vectors(&m, &vecs, &opt, 8, &mut r.image).unwrap();
        let changed = r.image.rgba.iter().zip(&blank.rgba).filter(|(a, b)| a != b).count();
        assert!(changed > 200, "{changed}");
        assert!(overlay_vectors(&m, &vecs[1..], &opt, 8, &mut r.image).is_err());
    }
}
