//! CPU reference rasteriser for the render-core checks (ADR 0125 G2, P3): the same scene description drawn in f64
//! with edge functions, the top-left fill rule at pixel centres, perspective-correct barycentrics and a z-buffer
//! (depth test `Less`). It shares only the camera matrices (checked on their own by C1-C3), the fan triangulation,
//! the point normals and the colour map (checked against rcfd-post) with the GPU path.

use rcfd_core::vec3::{add, dot, norm, scale, Vec3};
use rcfd_render::camera::mat_vec;
use rcfd_render::{Field, Image, PartStyle, Shading, SurfaceMesh, View};

pub struct Ref {
    pub image: Image,
    /// Surface face + 1 per pixel (0 = background).
    pub ids: Vec<u32>,
    /// Field value per pixel (0 where none).
    pub values: Vec<f64>,
}

pub fn render(s: &SurfaceMesh, field: Field, view: &View) -> Ref {
    let (w, h) = (view.width as usize, view.height as usize);
    let (lo, hi) = view.range.or(s.field_range(field).unwrap()).unwrap_or((0.0, 1.0));
    let span = if hi > lo { hi - lo } else { 1.0 };
    let vp = view.camera.view_proj(w as f64 / h as f64);
    let light = view.camera.basis()[2];
    let pn = s.point_normals();
    let mut out = Ref {
        image: Image {
            width: w,
            height: h,
            rgba: view.background.iter().copied().cycle().take(4 * w * h).collect(),
        },
        ids: vec![0; w * h],
        values: vec![0.0; w * h],
    };
    let mut zbuf = vec![1.0f64; w * h];
    for f in 0..s.n_faces() {
        let style = view.parts.get(s.face_part[f]).copied().unwrap_or(PartStyle::Field);
        if style == PartStyle::Hidden {
            continue;
        }
        let a = s.face_area_vector(f);
        let nf = scale(a, 1.0 / norm(a));
        let l = s.face(f);
        for k in 1..l.len() - 1 {
            let tri = [l[0], l[k], l[k + 1]];
            let c = tri.map(|p| {
                let q = s.points[p];
                mat_vec(&vp, [q[0], q[1], q[2], 1.0])
            });
            assert!(
                c.iter().all(|c| c[3] > 0.0),
                "reference needs every vertex in front of the eye"
            );
            let sc = c.map(|c| {
                [
                    0.5 * (c[0] / c[3] + 1.0) * w as f64,
                    0.5 * (1.0 - c[1] / c[3]) * h as f64,
                    c[2] / c[3],
                ]
            });
            let mut ord = [0usize, 1, 2];
            let area = edge(sc[0], sc[1], sc[2]);
            if area == 0.0 {
                continue;
            }
            if area < 0.0 {
                ord = [0, 2, 1];
            }
            let v = ord.map(|i| sc[i]);
            let area = area.abs();
            let x0 = v.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min).floor().max(0.0) as usize;
            let x1 = (v.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max).ceil() as usize).min(w);
            let y0 = v.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min).floor().max(0.0) as usize;
            let y1 = (v.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max).ceil() as usize).min(h);
            for py in y0..y1 {
                for px in x0..x1 {
                    let p = [px as f64 + 0.5, py as f64 + 0.5, 0.0];
                    // e[i] is the edge opposite vertex i
                    let e = [edge(v[1], v[2], p), edge(v[2], v[0], p), edge(v[0], v[1], p)];
                    let inside =
                        (0..3).all(|i| e[i] > 0.0 || (e[i] == 0.0 && top_left(v[(i + 1) % 3], v[(i + 2) % 3])));
                    if !inside {
                        continue;
                    }
                    let lb = e.map(|x| x / area);
                    let z = (0..3).map(|i| lb[i] * v[i][2]).sum::<f64>();
                    let idx = py * w + px;
                    if !(0.0..=1.0).contains(&z) || z >= zbuf[idx] {
                        continue;
                    }
                    // perspective-correct weights in the original vertex order
                    let mut b = [0.0; 3];
                    for i in 0..3 {
                        b[ord[i]] = lb[i] / c[ord[i]][3];
                    }
                    let sum: f64 = b.iter().sum();
                    let b = b.map(|x| x / sum);
                    let world = (0..3).fold([0.0; 3], |acc, i| add(acc, scale(s.points[tri[i]], b[i])));
                    if let Some((n, d)) = view.clip {
                        if dot(n, world) > d {
                            continue;
                        }
                    }
                    zbuf[idx] = z;
                    let value = match field {
                        Field::None => 0.0,
                        Field::Face(v) => v[f],
                        Field::Vertex(v) => (0..3).map(|i| b[i] * v[tri[i]]).sum(),
                    };
                    let base: [f64; 3] = match style {
                        PartStyle::Solid(c) => c.map(|x| x as f64),
                        _ if matches!(field, Field::None) => [200.0; 3],
                        _ => view.colormap.eval((value - lo) / span).map(|x| x as f64),
                    };
                    let n: Vec3 = match view.shading {
                        Shading::Unlit => [0.0; 3],
                        Shading::Flat => nf,
                        Shading::Smooth => (0..3).fold([0.0; 3], |acc, i| add(acc, scale(pn[tri[i]], b[i]))),
                    };
                    let k = if view.shading == Shading::Unlit {
                        1.0
                    } else {
                        0.25 + 0.75 * (dot(n, light) / norm(n)).abs()
                    };
                    let rgb = base.map(|x| (x * k).round() as u8);
                    out.image.rgba[4 * idx..4 * idx + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                    out.ids[idx] = f as u32 + 1;
                    out.values[idx] = value;
                }
            }
        }
    }
    out
}

fn edge(a: Vec3, b: Vec3, p: Vec3) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Top-left rule with y down and the interior where the edge function is positive: a left edge has the interior
/// at +x (dy < 0), a top edge is horizontal with the interior below (dy = 0, dx > 0).
fn top_left(a: Vec3, b: Vec3) -> bool {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    dy < 0.0 || (dy == 0.0 && dx > 0.0)
}

/// Comparison statistics of an image against the reference.
#[derive(Debug, Clone, Copy)]
pub struct Diff {
    /// Pixels whose 3x3 neighbourhood in the reference id buffer is not uniform.
    pub edge_pixels: usize,
    /// Largest channel difference on the other (interior) pixels.
    pub interior_max: u8,
    /// Edge pixels whose largest channel difference exceeds the interior limit.
    pub edge_over: usize,
    /// Mean absolute channel difference over all pixels and the three colour channels.
    pub mean: f64,
}

pub fn edge_mask(r: &Ref) -> Vec<bool> {
    let (w, h) = (r.image.width as i64, r.image.height as i64);
    let mut m = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let id = r.ids[(y * w + x) as usize];
            'n: for dy in -1..=1 {
                for dx in -1..=1 {
                    let (u, v) = (x + dx, y + dy);
                    if u >= 0 && v >= 0 && u < w && v < h && r.ids[(v * w + u) as usize] != id {
                        m[(y * w + x) as usize] = true;
                        break 'n;
                    }
                }
            }
        }
    }
    m
}

pub fn diff(img: &Image, r: &Ref, interior_limit: u8) -> Diff {
    let m = edge_mask(r);
    let mut d = Diff {
        edge_pixels: m.iter().filter(|x| **x).count(),
        interior_max: 0,
        edge_over: 0,
        mean: 0.0,
    };
    let mut sum = 0u64;
    for i in 0..m.len() {
        let mx = (0..3)
            .map(|k| img.rgba[4 * i + k].abs_diff(r.image.rgba[4 * i + k]))
            .max()
            .unwrap();
        sum += (0..3)
            .map(|k| img.rgba[4 * i + k].abs_diff(r.image.rgba[4 * i + k]) as u64)
            .sum::<u64>();
        if m[i] {
            d.edge_over += (mx > interior_limit) as usize;
        } else {
            d.interior_max = d.interior_max.max(mx);
        }
    }
    d.mean = sum as f64 / (3 * m.len()) as f64;
    d
}
