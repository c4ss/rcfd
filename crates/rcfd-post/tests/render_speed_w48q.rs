//! ADR 0151 (job 48Q): the CPU contour render against the base renderer.
//! * Q1: byte identity over a matrix of meshes and options. Q1's negative control is the base renderer with the cell order
//!   reversed, which must differ on the cases where cell order decides a pixel.
//! * Q2: r1's and r3's images are as before (SHA-256 recorded from the base code).
//! * Q3: the 1M-cell image of r2 is as before. Ignored: run by hand in release; the mesh takes 2.7 GB.
//! * Q0: the profile of the base renderer on r2's workload. Ignored: run by hand in release.
//! * Q4: the margin proxy (ADR 0151 part 2): the base copy and the product with 1, 2 and the default threads on r2's
//!   workload, interleaved in one process. Ignored: run by hand in release under the registered conditions.
//!
//! The base renderer is the verbatim copy in `render_control_w48q`.

mod render_control_w48q;

use rcfd_io::datapack::sha256_hex;
use rcfd_mesh::gen::{
    annulus_3d, backward_step, box_mesh, cavity_2d, cavity_2d_rect, cylinder_ogrid, warp, Grading, StepSpec,
};
use rcfd_mesh::Mesh;
use rcfd_post::render::{render_scalar, Image, RenderOptions, Rendered};
use render_control_w48q as control;
use std::time::{Duration, Instant};

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*s >> 11) as f64 / (1u64 << 53) as f64
}

fn random_values(n: usize, seed: u64) -> Vec<f64> {
    let mut s = seed;
    (0..n).map(|_| lcg(&mut s)).collect()
}

struct Case {
    name: String,
    mesh: Mesh,
    values: Vec<f64>,
    opt: RenderOptions,
    /// Cell order decides at least one pixel (checked by the negative control).
    order_matters: bool,
}

fn case(name: &str, mesh: &Mesh, values: Vec<f64>, opt: RenderOptions, order_matters: bool) -> Case {
    Case {
        name: name.into(),
        mesh: mesh.clone(),
        values,
        opt,
        order_matters,
    }
}

fn no_bar(width: usize) -> RenderOptions {
    RenderOptions {
        width,
        colorbar: false,
        ..Default::default()
    }
}

/// The registered matrix of Q1 (ADR 0151).
fn matrix() -> Vec<Case> {
    let mut m = Vec::new();
    // 1: the colour bar and its labels
    let c8 = cavity_2d(8, 1.0).unwrap();
    m.push(case(
        "1 cavity 8, default options",
        &c8,
        random_values(64, 1),
        RenderOptions::default(),
        false,
    ));
    // 2: edges at 8 + 1.5 i px; every other edge passes through pixel centres, drawn by both cells
    let c32 = cavity_2d_rect(32, 32, 1.0, 1.0).unwrap();
    m.push(case(
        "2 cavity 32, width 64",
        &c32,
        random_values(1024, 2),
        no_bar(64),
        true,
    ));
    // 3: coarse cells spanning hundreds of rows
    let coarse = cavity_2d_rect(3, 5, 1.0, 2.0).unwrap();
    m.push(case(
        "3 coarse 3 x 5, width 2048",
        &coarse,
        random_values(15, 3),
        no_bar(2048),
        false,
    ));
    // 4: skewed quads, explicit height
    let base = cavity_2d_rect(17, 13, 1.0, 1.0).unwrap();
    let skew = warp(&base, |p| {
        [p[0] + 0.3 * p[1] * p[1], p[1] + 0.2 * (3.0 * p[0]).sin(), p[2]]
    })
    .unwrap();
    let opt4 = RenderOptions {
        width: 777,
        height: 500,
        ..Default::default()
    };
    m.push(case(
        "4 warped 17 x 13, 777 x 500",
        &skew,
        random_values(17 * 13, 4),
        opt4,
        false,
    ));
    // 5: curved cells around a hole
    let ogrid = cylinder_ogrid(24, 8, 0.5, 2.0, Grading::Expansion(3.0), 0.1).unwrap();
    m.push(case(
        "5 O-grid 24 x 8, width 513",
        &ogrid,
        random_values(24 * 8, 5),
        no_bar(513),
        false,
    ));
    // 6: a non-rectangular domain
    let step = backward_step(StepSpec {
        l_in: 1.0,
        l_out: 4.0,
        h_in: 1.0,
        step: 0.5,
        depth: 0.1,
        n: [6, 20, 8, 4],
    })
    .unwrap();
    let ns = step.n_cells();
    m.push(case(
        "6 backward step, width 300",
        &step,
        random_values(ns, 6),
        RenderOptions {
            width: 300,
            ..Default::default()
        },
        false,
    ));
    // 7: 3D slices of a graded box, on every axis: inside a layer, on a layer boundary (two layers drawn), outside
    let bx = box_mesh(
        [4, 5, 6],
        [1.0, 1.2, 1.5],
        [
            Grading::Expansion(2.0),
            Grading::Symmetric(3.0),
            Grading::Expansion(0.5),
        ],
    )
    .unwrap();
    let nb = bx.n_cells();
    for axis in 0..3 {
        let mut coords: Vec<f64> = bx.points().iter().map(|p| p[axis]).collect();
        coords.sort_by(f64::total_cmp);
        coords.dedup();
        let boundary = coords[2]; // an inner layer boundary
        let inside = 0.5 * (coords[1] + coords[2]);
        for (what, slice, order) in [
            ("inside", inside, false),
            ("boundary", boundary, true),
            ("outside", 9.0, false),
        ] {
            let opt = RenderOptions {
                width: 400,
                axis: Some(axis),
                slice: Some(slice),
                ..Default::default()
            };
            let name = format!("7 box 4 x 5 x 6 graded, axis {axis}, slice {what}");
            m.push(case(&name, &bx, random_values(nb, 70 + axis as u64), opt, order));
        }
    }
    // 8: non-finite values
    let mut nonfinite = random_values(1024, 8);
    for (i, v) in nonfinite.iter_mut().enumerate() {
        match i % 7 {
            0 => *v = f64::NAN,
            3 => *v = f64::INFINITY,
            5 => *v = f64::NEG_INFINITY,
            _ => {}
        }
    }
    m.push(case(
        "8 cavity 32, NaN and inf",
        &c32,
        nonfinite,
        RenderOptions {
            width: 200,
            ..Default::default()
        },
        false,
    ));
    // 9: the range option: increasing, equal ends, reversed
    for (what, range) in [
        ("increasing", (0.2, 0.7)),
        ("equal", (0.4, 0.4)),
        ("reversed", (0.9, 0.1)),
    ] {
        let opt = RenderOptions {
            width: 200,
            range: Some(range),
            ..Default::default()
        };
        m.push(case(
            &format!("9 cavity 32, range {what}"),
            &c32,
            random_values(1024, 9),
            opt,
            false,
        ));
    }
    // 10: all values equal
    m.push(case(
        "10 cavity 32, all equal",
        &c32,
        vec![0.25; 1024],
        RenderOptions {
            width: 200,
            ..Default::default()
        },
        false,
    ));
    // 11: the clamps of width and height
    let opt11 = RenderOptions {
        width: 10,
        height: 5,
        ..Default::default()
    };
    m.push(case(
        "11 cavity 32, width 10, height 5",
        &c32,
        random_values(1024, 11),
        opt11,
        false,
    ));
    // 12: degenerate hulls: not filled, still counted and in the range
    let flat = box_mesh([3, 3, 3], [1.0, 1.0, 1e-20], [Grading::Uniform; 3]).unwrap();
    let mut fv = random_values(27, 12);
    let opt12 = RenderOptions {
        axis: Some(0),
        ..Default::default()
    };
    // the extremes go on two of the drawn cells (the middle layer along x), so the range depends on degenerate hulls
    let (v12, f12) = (
        control::view(&flat, &opt12),
        control::frame(&control::view(&flat, &opt12), &opt12),
    );
    let drawn: Vec<usize> = (0..27)
        .filter(|&c| control::cell_polygon(&flat, c, &v12, &f12).is_some())
        .collect();
    fv[drawn[1]] = -50.0;
    fv[drawn[4]] = 75.0;
    m.push(case("12 flat box, degenerate hulls", &flat, fv, opt12, false));
    // 13: a curved 3D mesh, default axis and slice
    let ann = annulus_3d(16, 3, 4, 0.5, 1.0, 0.8, 0.0, Grading::Uniform).unwrap();
    let na = ann.n_cells();
    m.push(case(
        "13 annulus 3D",
        &ann,
        random_values(na, 13),
        RenderOptions {
            width: 300,
            ..Default::default()
        },
        false,
    ));
    // 14: a medium mesh at r2's width and field
    let med = cavity_2d_rect(256, 256, 1.0, 1.0).unwrap();
    let vals: Vec<f64> = med.cell_centres().iter().map(|c| (6.0 * c[0]).sin() * c[1]).collect();
    m.push(case(
        "14 cavity 256, r2's field, width 1024",
        &med,
        vals,
        RenderOptions {
            width: 1024,
            ..Default::default()
        },
        false,
    ));
    // 15 (ADR 0151 part 2): non-finite projections, the full-face-point path of `cell_hulls`
    match nan_mesh() {
        Ok(nan) => m.push(case(
            "15 cavity 12, NaN points",
            &nan,
            random_values(144, 15),
            no_bar(300),
            false,
        )),
        Err(e) => {
            eprintln!("Q1 row 15 dropped: the constructor refuses the mesh ({e}); the non-finite path is not reachable")
        }
    }
    m
}

/// Row 15's mesh: `cavity_2d_rect(12, 12)` with the points on x = 1 moved to x = NaN and those on y = 1 to y = NaN.
fn nan_mesh() -> rcfd_core::Result<Mesh> {
    let base = cavity_2d_rect(12, 12, 1.0, 1.0)?;
    warp(&base, |p| {
        let x = if p[0] == 1.0 { f64::NAN } else { p[0] };
        let y = if p[1] == 1.0 { f64::NAN } else { p[1] };
        [x, y, p[2]]
    })
}

/// The options each case is rendered with by the product: the default threads and 1, 2, 3, 5, 8 and 16 threads.
fn variants(opt: &RenderOptions) -> Vec<(String, RenderOptions)> {
    [0, 1, 2, 3, 5, 8, 16]
        .into_iter()
        .map(|threads| {
            let label = if threads == 0 {
                "default threads".to_string()
            } else {
                format!("{threads} threads")
            };
            (label, RenderOptions { threads, ..opt.clone() })
        })
        .collect()
}

/// Differences of `a` from `b`; the PNG bytes are compared when `png` is set and nothing else differs.
fn differences(a: &Rendered, b: &Rendered, png: bool) -> Vec<String> {
    let mut d = Vec::new();
    if (a.image.width, a.image.height) != (b.image.width, b.image.height) {
        d.push(format!(
            "size {} x {} vs {} x {}",
            a.image.width, a.image.height, b.image.width, b.image.height
        ));
    }
    if a.image.rgba != b.image.rgba {
        let n = a
            .image
            .rgba
            .chunks(4)
            .zip(b.image.rgba.chunks(4))
            .filter(|(x, y)| x != y)
            .count();
        d.push(format!("{n} pixels differ"));
    }
    if a.vmin.to_bits() != b.vmin.to_bits() || a.vmax.to_bits() != b.vmax.to_bits() {
        d.push(format!(
            "range {:?}..{:?} vs {:?}..{:?}",
            a.vmin, a.vmax, b.vmin, b.vmax
        ));
    }
    if a.cells_drawn != b.cells_drawn {
        d.push(format!("cells drawn {} vs {}", a.cells_drawn, b.cells_drawn));
    }
    if png && d.is_empty() && a.image.to_png().unwrap() != b.image.to_png().unwrap() {
        d.push("PNG bytes differ".into());
    }
    d
}

fn painted(img: &Image) -> usize {
    img.rgba.chunks(4).filter(|p| *p != [255, 255, 255, 255]).count()
}

#[test]
fn q1_matches_the_base_renderer_byte_for_byte() {
    let mut fails = Vec::new();
    let mut checked = 0;
    for c in matrix() {
        let want = control::render_scalar(&c.mesh, &c.values, &c.opt).unwrap();
        for (k, (label, opt)) in variants(&c.opt).into_iter().enumerate() {
            let got = render_scalar(&c.mesh, &c.values, &opt).unwrap();
            // the PNG bytes are a function of the size and the RGBA bytes; compared once per case
            let d = differences(&got, &want, k == 0);
            checked += 1;
            if !d.is_empty() {
                fails.push(format!("{} ({label}): {}", c.name, d.join("; ")));
            }
        }
        eprintln!(
            "Q1 {}: {} x {}, cells drawn {} of {}, range {:?}..{:?}, {} pixels painted",
            c.name,
            want.image.width,
            want.image.height,
            want.cells_drawn,
            c.mesh.n_cells(),
            want.vmin,
            want.vmax,
            painted(&want.image)
        );
    }
    eprintln!("Q1: {checked} renders compared, {} differ", fails.len());
    assert!(fails.is_empty(), "{fails:#?}");
}

/// The base renderer with the cells drawn in reverse order (Q1's negative control).
fn reversed_order(mesh: &Mesh, values: &[f64], opt: &RenderOptions) -> Vec<u8> {
    let v = control::view(mesh, opt);
    let fr = control::frame(&v, opt);
    let mut img = control::image_new(fr.w, fr.h, [255, 255, 255, 255]);
    let polys: Vec<(usize, Vec<[f64; 2]>)> = (0..mesh.n_cells())
        .filter_map(|c| control::cell_polygon(mesh, c, &v, &fr).map(|p| (c, p)))
        .collect();
    let base = control::render_scalar(mesh, values, opt).unwrap();
    let span = if base.vmax > base.vmin {
        base.vmax - base.vmin
    } else {
        1.0
    };
    for (c, poly) in polys.iter().rev() {
        let [r, g, b] = control::colormap((values[*c] - base.vmin) / span);
        control::fill_polygon(&mut img, poly, [r, g, b, 255]);
    }
    img.rgba
}

#[test]
fn q1_negative_control_reversed_cell_order_is_detected() {
    let mut detected = Vec::new();
    for c in matrix().into_iter().filter(|c| c.order_matters) {
        let mut opt = c.opt.clone();
        opt.colorbar = false;
        let base = control::render_scalar(&c.mesh, &c.values, &opt).unwrap();
        let rev = reversed_order(&c.mesh, &c.values, &opt);
        let n = base
            .image
            .rgba
            .chunks(4)
            .zip(rev.chunks(4))
            .filter(|(a, b)| a != b)
            .count();
        eprintln!(
            "Q1 negative control {}: {n} pixels differ with the cell order reversed",
            c.name
        );
        detected.push((c.name, n));
    }
    assert_eq!(detected.len(), 4, "{detected:?}");
    assert!(detected.iter().all(|(_, n)| *n > 0), "{detected:?}");
}

// Recorded from the base code (main 6867e40), ADR 0151.
const R1_RGBA_SHA256: &str = "8902a5bd06d0a563a8c8046330ccd2ffa63b6b09d05ab503d9be96683c1ea415";
const R1_PNG_SHA256: &str = "26185d0854af3cadf9226ccb391a689489a81ad5b3c620f53a694463c839ec4b";
const R3_RGBA_SHA256: &str = "a9cc025acfecf67490a370c03a338916e9c704b540d29008cc211e218c9477c0";
const R3_PNG_SHA256: &str = "1cbd86702c2cde0805db44c2c10ac981638d2928dbd1963e562d4c6c0226839e";

#[test]
fn q2_r1_and_r3_images_are_as_before() {
    // r1's set-up (render_pixels.rs)
    let n = 64usize;
    let mesh = cavity_2d_rect(n, n, 1.0, 1.0).unwrap();
    let mut s = 5u64;
    let vals: Vec<f64> = (0..mesh.n_cells()).map(|_| lcg(&mut s)).collect();
    let opt = RenderOptions {
        width: 640,
        colorbar: false,
        ..Default::default()
    };
    let r1 = render_scalar(&mesh, &vals, &opt).unwrap();
    // r3's set-up
    let mesh = cavity_2d_rect(32, 32, 1.0, 1.0).unwrap();
    let vals: Vec<f64> = mesh.cell_centres().iter().map(|c| c[0] * 3.0 - c[1]).collect();
    let r3 = render_scalar(&mesh, &vals, &RenderOptions::default()).unwrap();
    let got = [
        sha256_hex(&r1.image.rgba),
        sha256_hex(&r1.image.to_png().unwrap()),
        sha256_hex(&r3.image.rgba),
        sha256_hex(&r3.image.to_png().unwrap()),
    ];
    eprintln!(
        "Q2: r1 {} x {} RGBA {} PNG {}; r3 {} x {} RGBA {} PNG {}",
        r1.image.width, r1.image.height, got[0], got[1], r3.image.width, r3.image.height, got[2], got[3]
    );
    assert_eq!(got, [R1_RGBA_SHA256, R1_PNG_SHA256, R3_RGBA_SHA256, R3_PNG_SHA256]);
}

// Recorded from the base code (main 6867e40), ADR 0151.
const R2_RGBA_SHA256: &str = "9ac74c0801a555bd5a16cfa724abedc4b6c522911f2d77b97c3d6284210bf027";
const R2_PNG_SHA256: &str = "59638fce7adee79af86dbd727324e8db0724b9ca2202fb3b951eb1f79278c8b4";
const R2_RANGE_BITS: (u64, u64) = (0xbfeffbe708600597, 0x3feffbe40acae0c8);

fn r2_workload() -> (Mesh, Vec<f64>, RenderOptions) {
    let mesh = cavity_2d_rect(1000, 1000, 1.0, 1.0).unwrap();
    let vals: Vec<f64> = mesh.cell_centres().iter().map(|c| (6.0 * c[0]).sin() * c[1]).collect();
    let opt = RenderOptions {
        width: 1024,
        ..Default::default()
    };
    (mesh, vals, opt)
}

#[test]
#[ignore = "ADR 0151 Q3: run by hand in release; the 1M-cell mesh takes 2.7 GB"]
fn q3_one_million_cell_image_is_as_before() {
    let (mesh, vals, opt) = r2_workload();
    let r = render_scalar(&mesh, &vals, &opt).unwrap();
    let png = r.image.to_png().unwrap();
    let (rgba_hash, png_hash) = (sha256_hex(&r.image.rgba), sha256_hex(&png));
    eprintln!(
        "Q3: {} x {}, cells drawn {}, range {:?}..{:?} (bits {:016x}..{:016x}), PNG {} bytes, RGBA {rgba_hash}, PNG {png_hash}",
        r.image.width,
        r.image.height,
        r.cells_drawn,
        r.vmin,
        r.vmax,
        r.vmin.to_bits(),
        r.vmax.to_bits(),
        png.len()
    );
    let base = control::render_scalar(&mesh, &vals, &opt).unwrap();
    let d = differences(&r, &base, true);
    assert!(d.is_empty(), "against the base copy: {d:?}");
    assert_eq!(r.cells_drawn, 1_000_000);
    assert_eq!((r.vmin.to_bits(), r.vmax.to_bits()), R2_RANGE_BITS);
    assert_eq!([rgba_hash.as_str(), png_hash.as_str()], [R2_RGBA_SHA256, R2_PNG_SHA256]);
}

// ---------------------------------------------------------------------------------------------------------------------
// Q0: profile of the base renderer (ADR 0151 C-profile).

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn stats(xs: &[f64]) -> (f64, f64, f64) {
    let mut s = xs.to_vec();
    s.sort_by(f64::total_cmp);
    (s[s.len() / 2], s[0], s[s.len() - 1])
}

/// The monotone chain of `control::convex_hull` on a reused output buffer, without the boxed iterators.
fn hull_into(p: &mut Vec<[f64; 2]>, h: &mut Vec<[f64; 2]>) {
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-14 && (a[1] - b[1]).abs() < 1e-14);
    h.clear();
    if p.len() < 3 {
        h.extend_from_slice(p);
        return;
    }
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    for &q in p.iter() {
        while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
        h.push(q);
    }
    h.pop();
    let start = h.len();
    for &q in p.iter().rev() {
        while h.len() >= start + 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
        h.push(q);
    }
    h.pop();
}

/// Flat hulls: drawn cells, hull offsets, hull points.
struct Flat {
    cells: Vec<usize>,
    starts: Vec<usize>,
    pts: Vec<[f64; 2]>,
}

impl Flat {
    fn new(n: usize) -> Self {
        Flat {
            cells: Vec::with_capacity(n),
            starts: {
                let mut s = Vec::with_capacity(n + 1);
                s.push(0);
                s
            },
            pts: Vec::with_capacity(4 * n),
        }
    }
    fn hull(&self, k: usize) -> &[[f64; 2]] {
        &self.pts[self.starts[k]..self.starts[k + 1]]
    }
}

/// Probe A: the polygon phase with reused buffers (all 24 projected points sorted, as the base).
fn probe_reuse(mesh: &Mesh, v: &control::View, f: &control::Frame) -> Flat {
    let pts = mesh.points();
    let mut out = Flat::new(mesh.n_cells());
    let (mut proj, mut h) = (Vec::new(), Vec::new());
    for c in 0..mesh.n_cells() {
        proj.clear();
        let (mut zlo, mut zhi) = (f64::MAX, f64::MIN);
        for &face in mesh.cell_faces(c) {
            for &pi in mesh.face_points(face) {
                let p = pts[pi];
                zlo = zlo.min(p[v.axis]);
                zhi = zhi.max(p[v.axis]);
                proj.push(control::map_point(f, v, p));
            }
        }
        if v.slice < zlo - 1e-12 || v.slice > zhi + 1e-12 {
            continue;
        }
        hull_into(&mut proj, &mut h);
        out.cells.push(c);
        out.pts.extend_from_slice(&h);
        out.starts.push(out.pts.len());
    }
    out
}

/// Probe B: as A, with the cell's point indices deduplicated before the projection and the sort.
fn probe_dedup(mesh: &Mesh, v: &control::View, f: &control::Frame) -> Flat {
    let pts = mesh.points();
    let mut out = Flat::new(mesh.n_cells());
    let (mut idx, mut proj, mut h) = (Vec::new(), Vec::new(), Vec::new());
    for c in 0..mesh.n_cells() {
        idx.clear();
        for &face in mesh.cell_faces(c) {
            for &pi in mesh.face_points(face) {
                if !idx.contains(&pi) {
                    idx.push(pi);
                }
            }
        }
        proj.clear();
        let (mut zlo, mut zhi) = (f64::MAX, f64::MIN);
        for &pi in &idx {
            let p = pts[pi];
            zlo = zlo.min(p[v.axis]);
            zhi = zhi.max(p[v.axis]);
            proj.push(control::map_point(f, v, p));
        }
        if v.slice < zlo - 1e-12 || v.slice > zhi + 1e-12 {
            continue;
        }
        hull_into(&mut proj, &mut h);
        out.cells.push(c);
        out.pts.extend_from_slice(&h);
        out.starts.push(out.pts.len());
    }
    out
}

/// Probe C: colour and fill from flat hulls with a reused crossing buffer (the arithmetic of `control::fill_polygon`).
fn probe_fill(flat: &Flat, values: &[f64], vmin: f64, span: f64, img: &mut Image) {
    let mut xs: Vec<f64> = Vec::new();
    for (k, &c) in flat.cells.iter().enumerate() {
        let poly = flat.hull(k);
        let [r, g, b] = control::colormap((values[c] - vmin) / span);
        let color = [r, g, b, 255];
        if poly.len() < 3 {
            continue;
        }
        let (ymin, ymax) = poly
            .iter()
            .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
        let y0 = (ymin - 0.5).ceil().max(0.0) as i64;
        let y1 = (ymax - 0.5).floor().min(img.height as f64 - 1.0) as i64;
        for y in y0..=y1 {
            let yc = y as f64 + 0.5;
            xs.clear();
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
                    control::put(img, x, y, color);
                }
            }
        }
    }
}

fn same_hulls(a: &Flat, polys: &[(usize, Vec<[f64; 2]>)]) -> bool {
    a.cells.len() == polys.len()
        && polys.iter().enumerate().all(|(k, (c, p))| {
            a.cells[k] == *c
                && a.hull(k).len() == p.len()
                && a.hull(k)
                    .iter()
                    .zip(p)
                    .all(|(x, y)| x[0].to_bits() == y[0].to_bits() && x[1].to_bits() == y[1].to_bits())
        })
}

#[test]
#[ignore = "ADR 0151 Q0: profile, run by hand in release with --nocapture"]
fn q0_profile_control_phases() {
    let reps: usize = std::env::var("RCFD_W48Q_REPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let t = Instant::now();
    let (mesh, vals, opt) = r2_workload();
    eprintln!(
        "Q0: workload built in {:.1} s ({} cells, {} points)",
        t.elapsed().as_secs_f64(),
        mesh.n_cells(),
        mesh.points().len()
    );
    let names = [
        "view",
        "frame_image",
        "polygons",
        "range",
        "fill",
        "colorbar",
        "free",
        "render total",
        "png encode",
        "A polygons, reused buffers",
        "B polygons, index dedup",
        "C colour + fill, reused crossings",
        "D colormap alone",
        "E polygons, base cell_polygon alone",
        "F hulls freed (E's)",
        "G colour + fill, base fill_polygon (E's hulls)",
    ];
    let mut rows: Vec<Vec<f64>> = vec![Vec::new(); names.len()];
    for rep in 0..reps {
        // the base renderer with phase marks
        let mut phases: Vec<(&'static str, Duration)> = Vec::new();
        let mut last = Instant::now();
        let t0 = last;
        let r = control::render_scalar_marked(&mesh, &vals, &opt, &mut |name| {
            let now = Instant::now();
            phases.push((name, now - last));
            last = now;
        })
        .unwrap();
        let total = t0.elapsed();
        for (name, d) in &phases {
            let i = names.iter().position(|n| n == name).unwrap();
            rows[i].push(ms(*d));
        }
        rows[7].push(ms(total));
        let t = Instant::now();
        let png = r.image.to_png().unwrap();
        rows[8].push(ms(t.elapsed()));
        // probes on the same data
        let v = control::view(&mesh, &opt);
        let fr = control::frame(&v, &opt);
        let t = Instant::now();
        let polys: Vec<(usize, Vec<[f64; 2]>)> = (0..mesh.n_cells())
            .filter_map(|c| control::cell_polygon(&mesh, c, &v, &fr).map(|p| (c, p)))
            .collect();
        rows[13].push(ms(t.elapsed()));
        let t = Instant::now();
        let a = probe_reuse(&mesh, &v, &fr);
        rows[9].push(ms(t.elapsed()));
        let t = Instant::now();
        let b = probe_dedup(&mesh, &v, &fr);
        rows[10].push(ms(t.elapsed()));
        assert!(same_hulls(&a, &polys), "probe A's hulls differ from the base");
        assert!(same_hulls(&b, &polys), "probe B's hulls differ from the base");
        let vmin = r.vmin;
        let span = if r.vmax > vmin { r.vmax - vmin } else { 1.0 };
        let mut base_fill = control::image_new(fr.w, fr.h, [255, 255, 255, 255]);
        let t = Instant::now();
        for (c, poly) in &polys {
            let [cr, cg, cb] = control::colormap((vals[*c] - vmin) / span);
            control::fill_polygon(&mut base_fill, poly, [cr, cg, cb, 255]);
        }
        rows[15].push(ms(t.elapsed()));
        let t = Instant::now();
        drop(polys);
        rows[14].push(ms(t.elapsed()));
        let mut img = control::image_new(fr.w, fr.h, [255, 255, 255, 255]);
        let t = Instant::now();
        probe_fill(&a, &vals, vmin, span, &mut img);
        rows[11].push(ms(t.elapsed()));
        assert!(base_fill.rgba == img.rgba, "probe C's image differs from the base fill");
        let t = Instant::now();
        let mut acc = 0u64;
        for &c in &a.cells {
            let [cr, cg, cb] = control::colormap((vals[c] - vmin) / span);
            acc = acc.wrapping_add(u64::from(cr) + u64::from(cg) + u64::from(cb));
        }
        rows[12].push(ms(t.elapsed()));
        eprintln!(
            "Q0 rep {rep}: render {:.1} ms, png {:.1} ms ({} bytes), checksum {acc}",
            ms(total),
            rows[8][rep],
            png.len()
        );
    }
    eprintln!("Q0 profile of the base renderer, r2 workload, {reps} repetitions (ms: median, min, max)");
    for (name, xs) in names.iter().zip(&rows) {
        let (med, lo, hi) = stats(xs);
        eprintln!("Q0 {name:<38} {med:>9.1} {lo:>9.1} {hi:>9.1}");
    }
}

/// Q1 row 15's probe (reported, no limit): would row 15 tell the product's non-finite path apart from deduplicated points
/// throughout? The image from probe B's hulls (no fallback) is compared with the base image.
#[test]
fn q1_row15_probe_without_the_non_finite_path() {
    let Ok(mesh) = nan_mesh() else {
        eprintln!("Q1 row 15 probe: the mesh is refused; nothing to probe");
        return;
    };
    let (values, opt) = (random_values(144, 15), no_bar(300));
    let base = control::render_scalar(&mesh, &values, &opt).unwrap();
    let v = control::view(&mesh, &opt);
    let fr = control::frame(&v, &opt);
    let flat = probe_dedup(&mesh, &v, &fr);
    let span = if base.vmax > base.vmin {
        base.vmax - base.vmin
    } else {
        1.0
    };
    let mut img = control::image_new(fr.w, fr.h, [255, 255, 255, 255]);
    probe_fill(&flat, &values, base.vmin, span, &mut img);
    let n = img
        .rgba
        .chunks(4)
        .zip(base.image.rgba.chunks(4))
        .filter(|(a, b)| a != b)
        .count();
    let nan_cells = (0..mesh.n_cells())
        .filter(|&c| {
            mesh.cell_faces(c)
                .iter()
                .flat_map(|&f| mesh.face_points(f))
                .any(|&p| !mesh.points()[p][0].is_finite() || !mesh.points()[p][1].is_finite())
        })
        .count();
    eprintln!(
        "Q1 row 15 probe: {nan_cells} cells with NaN points, {} drawn; without the non-finite path {n} pixels differ from the base",
        base.cells_drawn
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Q4: the margin proxy (ADR 0151 part 2).

/// Q4-r1 and Q4-r2: the product's median over the base copy's, with 1 and with 2 threads.
const Q4_RATIO: f64 = 0.43;
/// Q4-a1 and Q4-a2 (S2L only): the product's median with 1 and with 2 threads, seconds.
const Q4_ABSOLUTE_S: f64 = 1.0;
/// Q4-v (S2L only): the base copy's median that makes an S2L run valid, seconds.
const Q4_VALID_BASE_S: f64 = 1.6;

#[test]
#[ignore = "ADR 0151 Q4: run by hand in release under the registered conditions (RCFD_W48Q_COND, RCFD_W48Q_REPS)"]
fn q4_margin_proxy_render_times() {
    let reps: usize = std::env::var("RCFD_W48Q_REPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);
    let cond = std::env::var("RCFD_W48Q_COND").unwrap_or_else(|_| "U".into());
    let (mesh, vals, opt) = r2_workload();
    let reference = control::render_scalar(&mesh, &vals, &opt).unwrap();
    let variants: [(&str, Option<usize>); 4] = [
        ("base copy", None),
        ("product, 1 thread", Some(1)),
        ("product, 2 threads", Some(2)),
        ("product, default threads", Some(0)),
    ];
    let mut secs = vec![Vec::new(); variants.len()];
    for rep in 0..reps {
        // the order rotates every round, so that a drift in the load spreads over all variants
        for j in 0..variants.len() {
            let k = (j + rep) % variants.len();
            let t = Instant::now();
            let r = match variants[k].1 {
                None => control::render_scalar(&mesh, &vals, &opt).unwrap(),
                Some(threads) => render_scalar(&mesh, &vals, &RenderOptions { threads, ..opt.clone() }).unwrap(),
            };
            secs[k].push(t.elapsed().as_secs_f64());
            assert!(
                r.image.rgba == reference.image.rgba && r.vmin.to_bits() == reference.vmin.to_bits(),
                "{}: the image differs from the base",
                variants[k].0
            );
        }
    }
    eprintln!("Q4 r2 workload, condition {cond}, {reps} rounds (s: median, min, max)");
    let mut med = Vec::new();
    for ((name, _), xs) in variants.iter().zip(&secs) {
        let (m, lo, hi) = stats(xs);
        eprintln!("Q4 {name:<26} {m:>7.3} {lo:>7.3} {hi:>7.3}");
        med.push(m);
    }
    let (r1, r2) = (med[1] / med[0], med[2] / med[0]);
    eprintln!(
        "Q4 ratios to the base copy: 1 thread {r1:.3}, 2 threads {r2:.3}, default {:.3}",
        med[3] / med[0]
    );
    let mut checks = vec![
        ("Q4-r1 ratio, 1 thread", r1, Q4_RATIO, r1 <= Q4_RATIO),
        ("Q4-r2 ratio, 2 threads", r2, Q4_RATIO, r2 <= Q4_RATIO),
    ];
    let mut void = false;
    if cond == "S2L" {
        void = med[0] < Q4_VALID_BASE_S;
        eprintln!(
            "Q4-v validity: base copy median {:.3} s, needs >= {Q4_VALID_BASE_S} s: {}",
            med[0],
            if void { "VOID run" } else { "valid" }
        );
        if !void {
            checks.push((
                "Q4-a1 median, 1 thread (s)",
                med[1],
                Q4_ABSOLUTE_S,
                med[1] <= Q4_ABSOLUTE_S,
            ));
            checks.push((
                "Q4-a2 median, 2 threads (s)",
                med[2],
                Q4_ABSOLUTE_S,
                med[2] <= Q4_ABSOLUTE_S,
            ));
        }
    }
    for (name, got, limit, ok) in &checks {
        eprintln!(
            "Q4 {name}: {got:.3} against <= {limit}: {}",
            if *ok { "pass" } else { "MISS" }
        );
    }
    let missed: Vec<_> = checks.iter().filter(|c| !c.3).map(|c| c.0).collect();
    assert!(missed.is_empty(), "missed: {missed:?}");
    assert!(
        !void,
        "Q4-v: the S2L run is void (base copy median {:.3} s < {Q4_VALID_BASE_S} s)",
        med[0]
    );
}
