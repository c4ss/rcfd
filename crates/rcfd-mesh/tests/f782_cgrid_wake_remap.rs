//! F-782 (ADR 0699): construction S'' of the camber-seam C-grid (`CGridOptions::camber_seam_s_wake_remap_construction`, construction
//! S' of ADR 0696 with the wake-line nodes of the span layers taken from the remapped grid away from the trailing edge) on the
//! reference fan `ComputerFan::rom_reference()` blade 0 and on the ADR 0696 second blade at the ADR 0118 s4 specs: the spanwise
//! faces of the wake stack within skewness 4.0, no folded cell within 85 deg and aspect 1000; the drift of the wake stacks between
//! the thin end-wall layers; the locality of the change; camber_seam_s and camber_seam_s_unstretched unchanged in bits. Limits
//! registered in ADR 0699 before this code ran.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the limits

use rcfd_core::vec3::Vec3;
use rcfd_mesh::fan::*;
use rcfd_mesh::fan_cgrid::*;
use rcfd_mesh::fan_design::*;
use rcfd_mesh::fan_mesh::*;
use rcfd_mesh::mesh::Mesh;
use rcfd_mesh::quality::compute_metrics;
use std::collections::HashMap;
use std::f64::consts::PI;

/// Registered limits (ADR 0699 W2 to W6).
const NON_ORTH_MAX: f64 = 85.0;
const SKEW_MAX: f64 = 4.0;
const ASPECT_MAX: f64 = 1000.0;
const WAKE_SKEW_TIGHT: f64 = 1.0;
const DRIFT_MAX: f64 = 5e-6;

/// Measured values against registered limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        self.holds(name, value <= limit, &format!("{value:.6e} (limit {limit})"));
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// The s4 C-grid smoke spec of `fan_design_w46a.rs` (ADR 0118), copied verbatim.
fn s4() -> CGridSpec {
    CGridSpec {
        n_up: 8,
        n_chord: 24,
        n_down: 8,
        n_side: 10,
        n_nose: 6,
        n_core: 2,
        n_span: 10,
        n_gap: 7,
        n_bl: 2,
        growth: 1.2,
        first_cell: FirstCell::Height(5e-5),
        front_gap: 0.3,
        include_blade: true,
    }
}

/// The reference blade 0 (ADR 0643) in the s4 domain.
fn reference() -> AxialFan {
    ComputerFan::rom_reference().axial_fan(0, 0.03, 0.04)
}

/// The second blade of ADR 0696: the reference with the per-blade deviation stagger -1 deg, chord +20 % on every blade.
fn second_blade() -> AxialFan {
    ComputerFan {
        deviation: Some(Deviation {
            blades: vec![
                BladeDeviation {
                    stagger_deg: -1.0,
                    chord_rel: 0.2,
                    ..Default::default()
                };
                7
            ],
            ..Default::default()
        }),
        ..ComputerFan::rom_reference()
    }
    .axial_fan(0, 0.03, 0.04)
}

fn s_prime() -> CGridOptions {
    CGridOptions::camber_seam_s_unstretched_construction()
}

fn s_second() -> CGridOptions {
    CGridOptions::camber_seam_s_wake_remap_construction()
}

/// The algebraic grid of a construction (the fallback chain's step-5 grid): the same options without the smoothing.
fn algebraic(opts: CGridOptions) -> CGridOptions {
    CGridOptions {
        smoothing: false,
        ..opts
    }
}

// ---------------------------------------------------------------- measures (as f777_cgrid_unstretched.rs)

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn det(a: Vec3, b: Vec3, c: Vec3) -> f64 {
    a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
}

/// The eight corner Jacobians of a hex in the layout order, signed so that a regular hex gives one common sign.
fn corner_jacobians(p: &[Vec3; 8]) -> [f64; 8] {
    let mut out = [0.0; 8];
    for (c, o) in out.iter_mut().enumerate() {
        let sg = |bit: usize| if c & bit == 0 { 1.0 } else { -1.0 };
        *o = det(sub(p[c ^ 1], p[c]), sub(p[c ^ 2], p[c]), sub(p[c ^ 4], p[c])) * sg(1) * sg(2) * sg(4);
    }
    out
}

/// A hex of the C or F block: `(block, a, b, k)`.
type HexTag = (&'static str, usize, usize, usize);

/// (positive, negative, zero) corner Jacobians over every C- and F-block hex, and the first hex with a minority or zero corner.
fn fold_count(m: &CGridMesh) -> (usize, usize, usize, Option<HexTag>) {
    let mut hexes: Vec<(HexTag, [Vec3; 8])> = Vec::new();
    for k in 0..m.n_k {
        for j in 0..m.n_side() {
            for i in 0..m.n_i() {
                let mut p = [[0.0; 3]; 8];
                for (c, pc) in p.iter_mut().enumerate() {
                    *pc = m.grid[m.c_node(i + (c & 1), j + ((c >> 1) & 1), k + (c >> 2))];
                }
                hexes.push((("C", i, j, k), p));
            }
        }
        for b in 0..2 * m.n_nose() {
            for a in 0..m.n_up() {
                let mut p = [[0.0; 3]; 8];
                for (c, pc) in p.iter_mut().enumerate() {
                    *pc = m.grid[m.f_node(a + (c & 1), b + ((c >> 1) & 1), k + (c >> 2))];
                }
                hexes.push((("F", a, b, k), p));
            }
        }
    }
    let js: Vec<[f64; 8]> = hexes.iter().map(|(_, p)| corner_jacobians(p)).collect();
    let pos = js.iter().flatten().filter(|&&d| d > 0.0).count();
    let neg = js.iter().flatten().filter(|&&d| d < 0.0).count();
    let zero = js.len() * 8 - pos - neg;
    let majority_positive = pos >= neg;
    let first = hexes
        .iter()
        .zip(&js)
        .find(|(_, j)| {
            j.iter()
                .any(|&d| if majority_positive { !(d > 0.0) } else { !(d < 0.0) })
        })
        .map(|(h, _)| h.0);
    (pos, neg, zero, first)
}

/// Largest distance between a lower seam node rotated by `2 pi / N` and its upper partner, over every layer [m].
fn periodicity(m: &CGridMesh, n_blades: usize) -> f64 {
    let a = 2.0 * PI / n_blades as f64;
    let mut worst: f64 = 0.0;
    for k in 0..=m.n_k {
        let (lo, hi) = m.seam_nodes(k);
        for (l, h) in lo.iter().zip(&hi) {
            let p = m.grid[*l];
            let q = m.grid[*h];
            let r = [p[0] * a.cos() - p[1] * a.sin(), p[0] * a.sin() + p[1] * a.cos(), p[2]];
            let d = sub(r, q);
            worst = worst.max((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt());
        }
    }
    worst
}

fn tag_name(m: &CGridMesh, c: usize) -> String {
    let (blk, a, b, k) = m.cell_tags[c];
    let name = match blk {
        0 => "C (i, j)",
        1 => "F (a, b)",
        _ => "core (m, t)",
    };
    format!("{name} = ({a}, {b}), layer {k}")
}

/// R1: where the maxima of non-orthogonality, skewness and aspect are.
fn locate(m: &CGridMesh) -> String {
    let mesh = &m.mesh;
    let q = compute_metrics(mesh);
    let arg = |v: &[f64]| {
        v.iter().enumerate().fold(
            (0usize, f64::NEG_INFINITY),
            |a, (i, &x)| if x > a.1 { (i, x) } else { a },
        )
    };
    let face = |f: usize| {
        let o = tag_name(m, mesh.owner()[f]);
        if f < mesh.n_internal_faces() {
            format!("face {f} between {o} and {}", tag_name(m, mesh.neighbour()[f]))
        } else {
            let p = mesh.patches().iter().find(|p| f >= p.start && f < p.start + p.size);
            format!("boundary face {f} ({}) of {o}", p.map_or("?", |p| p.name.as_str()))
        }
    };
    let (fn_, vn) = arg(&q.non_orthogonality);
    let (fs, vs) = arg(&q.skewness);
    let (ca, va) = arg(&q.aspect_ratio);
    format!(
        "max non-orthogonality {vn:.3} deg at {}; max skewness {vs:.4} at {}; max aspect {va:.1} at cell {}",
        face(fn_),
        face(fs),
        tag_name(m, ca)
    )
}

fn fnv(h: &mut u64, x: u64) {
    for b in x.to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}

/// FNV-1a over the node coordinates, face node lists, owner/neighbour and patch names and ranges (as ADR 0079 B1, ADR 0692 B2,
/// ADR 0696 BS).
fn mesh_hash(m: &Mesh) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for p in m.points() {
        for v in p {
            fnv(&mut h, v.to_bits());
        }
    }
    for f in 0..m.n_faces() {
        fnv(&mut h, m.face_points(f).len() as u64);
        for &n in m.face_points(f) {
            fnv(&mut h, n as u64);
        }
    }
    for _ in 0..m.n_cells() {
        fnv(&mut h, m.owner().len() as u64);
    }
    for f in 0..m.n_faces() {
        fnv(&mut h, m.owner()[f] as u64);
    }
    for f in 0..m.n_internal_faces() {
        fnv(&mut h, m.neighbour()[f] as u64);
    }
    for p in m.patches() {
        for b in p.name.bytes() {
            fnv(&mut h, b as u64);
        }
        fnv(&mut h, p.start as u64);
        fnv(&mut h, p.size as u64);
    }
    h
}

// ---------------------------------------------------------------- the ADR 0699 measures

/// The wake index `p` of C line `i` (0 on the trailing-edge lines), `None` on the blade lines between them.
fn wake_index(m: &CGridMesh, i: usize) -> Option<usize> {
    let (n_i, nd) = (m.n_i(), m.n_down());
    if i <= nd {
        Some(nd - i)
    } else if i >= n_i - nd {
        Some(i - (n_i - nd))
    } else {
        None
    }
}

/// W5 measure: the skewness of every spanwise face of the wake-stack cells (C cells `(i, j, k)`, `i < n_down` or `i >= n_i -
/// n_down`, `j < n_bl`, the face to `(i, j, k + 1)`), largest first: `(skewness, i, j, k)`.
fn wake_spanwise_skewness(m: &CGridMesh) -> Vec<(f64, usize, usize, usize)> {
    let mesh = &m.mesh;
    let q = compute_metrics(mesh);
    let mut id: HashMap<(usize, usize, usize), usize> = HashMap::new();
    for (c, t) in m.cell_tags.iter().enumerate() {
        if t.0 == 0 {
            id.insert((t.1, t.2, t.3), c);
        }
    }
    let (n_i, nd) = (m.n_i(), m.n_down());
    let mut out = Vec::new();
    for k in 0..m.n_k - 1 {
        for j in 0..m.n_bl {
            for i in (0..nd).chain(n_i - nd..n_i) {
                let (a, b) = (id[&(i, j, k)], id[&(i, j, k + 1)]);
                let f = mesh
                    .cell_faces(a)
                    .iter()
                    .copied()
                    .find(|&f| {
                        f < mesh.n_internal_faces()
                            && ((mesh.owner()[f] == a && mesh.neighbour()[f] == b)
                                || (mesh.owner()[f] == b && mesh.neighbour()[f] == a))
                    })
                    .expect("the spanwise face of two wake-stack cells");
                out.push((q.skewness[f], i, j, k));
            }
        }
    }
    out.sort_by(|x, y| y.0.total_cmp(&x.0));
    out
}

/// W6 measure: over every thin span cell `k` (thinner than the mean span cell), every full-weight wake line (`p >= max(n_down / 4,
/// 1)`) and every stack row `j <= n_bl`, the largest `max(|dz|, r_{k+1} |dtheta|)` between node layers `k` and `k + 1`, and where
/// `(drift, k, i, j)`.
fn thin_layer_drift(m: &CGridMesh) -> (f64, usize, usize, usize) {
    let r = &m.radii;
    let mean = (r[m.n_span] - r[0]) / m.n_span as f64;
    let m_full = (m.n_down() / 4).max(1);
    let mut worst = (0.0f64, 0, 0, 0);
    for k in 0..m.n_span {
        if !(r[k + 1] - r[k] < mean) {
            continue;
        }
        for i in 0..=m.n_i() {
            match wake_index(m, i) {
                Some(p) if p >= m_full => {}
                _ => continue,
            }
            for j in 0..=m.n_bl {
                let (a, b) = (m.grid[m.c_node(i, j, k)], m.grid[m.c_node(i, j, k + 1)]);
                let mut dth = b[1].atan2(b[0]) - a[1].atan2(a[0]);
                dth -= 2.0 * PI * (dth / (2.0 * PI)).round();
                let d = (b[2] - a[2]).abs().max(r[k + 1] * dth.abs());
                if !(d <= worst.0) {
                    worst = (d, k, i, j);
                }
            }
        }
    }
    worst
}

/// W1a to W6 (B2a to B2h and B2-W6 for the second blade) and R1 for construction S'' on `fan` at s4.
fn w1_w6(tag: &str, fan: &AxialFan) {
    let mut ch = Checks::new();
    let m = match build_cgrid_mesh_with(fan, &s4(), &s_second()) {
        Ok(m) => m,
        Err(e) => {
            ch.holds(&format!("{tag} W1a S'' builds"), false, &format!("{e:?}"));
            ch.finish();
            unreachable!()
        }
    };
    ch.holds(
        &format!("{tag} W1a S'' builds"),
        m.mesh.n_cells() > 0,
        &format!("{} cells, construction step {}", m.mesh.n_cells(), m.construction_step),
    );
    let q = m.quality();
    let (pos, neg, zero, first) = fold_count(&m);
    ch.holds(
        &format!("{tag} W1b no folded cell (corner Jacobians of one sign, min volume > 0)"),
        (pos == 0 || neg == 0) && zero == 0 && q.min_volume > 0.0,
        &format!(
            "positive {pos}, negative {neg}, zero {zero}, first minority {first:?}, min volume {:.4e}",
            q.min_volume
        ),
    );
    ch.at_most(&format!("{tag} W1c periodicity [m]"), periodicity(&m, 7), 1e-12);
    ch.at_most(
        &format!("{tag} W2 max non-orthogonality [deg]"),
        q.max_non_orthogonality,
        NON_ORTH_MAX,
    );
    ch.at_most(&format!("{tag} W3 max skewness"), q.max_skewness, SKEW_MAX);
    ch.at_most(&format!("{tag} W4 max aspect ratio"), q.max_aspect_ratio, ASPECT_MAX);
    let ws = wake_spanwise_skewness(&m);
    println!(
        "{tag} R1 largest wake-stack spanwise skewness (skewness, i, j, k): {:?}",
        &ws[..5]
    );
    ch.at_most(&format!("{tag} W5a wake-stack spanwise skewness"), ws[0].0, SKEW_MAX);
    ch.at_most(
        &format!("{tag} W5b wake-stack spanwise skewness"),
        ws[0].0,
        WAKE_SKEW_TIGHT,
    );
    let (d, k, i, j) = thin_layer_drift(&m);
    ch.at_most(
        &format!("{tag} W6 thin-layer wake-stack drift [m] (cell {k}, line {i}, row {j})"),
        d,
        DRIFT_MAX,
    );
    println!("{tag} R1 S'': {q:?}");
    println!("{tag} R1 {}", locate(&m));
    ch.finish();
}

// ---------------------------------------------------------------- the registered checks

/// W1a to W6 (ADR 0699, c2, c3, c5): S'' on the reference at s4.
#[test]
fn w1_w6_construction_s_second_meshes_the_reference() {
    w1_w6("REF", &reference());
}

/// B2a to B2h and B2-W6 (ADR 0699, c2, c4, c5): S'' on the ADR 0696 second blade (outer stagger 76.6 deg) at s4.
#[test]
fn b2_construction_s_second_meshes_the_second_blade() {
    let st = second_blade().stations;
    assert!(st.last().unwrap().stagger.to_degrees() >= 75.0);
    w1_w6("B2", &second_blade());
}

/// NC-W5 and NC-W6 (ADR 0699): the W5a measure on S' reproduces the ADR 0696 numbers at C (4, 1) between layers 8 and 9, and the
/// W6 measure on S' sees the drift of the true sections.
#[test]
fn nc_w5_w6_the_measures_see_construction_s_prime() {
    let mut ch = Checks::new();
    for (tag, fan, want) in [("REF", reference(), 3.2404), ("B2", second_blade(), 4.0112)] {
        let m = build_cgrid_mesh_with(&fan, &s4(), &s_prime()).expect("S' builds");
        let ws = wake_spanwise_skewness(&m);
        println!("NC-W5 {tag} S' largest (skewness, i, j, k): {:?}", &ws[..5]);
        ch.holds(
            &format!("NC-W5 {tag} S' W5a"),
            (ws[0].0 - want).abs() <= 1e-4 && (ws[0].1, ws[0].2, ws[0].3) == (4, 1, 8),
            &format!(
                "{:.6} at {:?} (ADR 0696: {want} at (4, 1, 8))",
                ws[0].0,
                (ws[0].1, ws[0].2, ws[0].3)
            ),
        );
        if tag == "REF" {
            let (d, k, i, j) = thin_layer_drift(&m);
            ch.holds(
                "NC-W6 REF S' drift",
                d > 1e-5,
                &format!("{d:.4e} m (cell {k}, line {i}, row {j}; limit > 1e-5)"),
            );
        }
    }
    ch.finish();
}

/// W7 and NC-W7 (ADR 0699, c2): on the algebraic grids of S'' and S' of the reference, every node outside the C wake lines
/// `p >= 1` of the node layers 1 to `n_span - 1` is equal in bits, and some node of those lines differs.
#[test]
fn w7_the_change_is_local_to_the_wake_lines_of_the_span_layers() {
    let fan = reference();
    let a = build_cgrid_mesh_with(&fan, &s4(), &algebraic(s_second())).expect("the algebraic grid of S'' builds");
    let b = build_cgrid_mesh_with(&fan, &s4(), &algebraic(s_prime())).expect("the algebraic grid of S' builds");
    assert_eq!(a.grid.len(), b.grid.len());
    let mut changed = vec![false; a.grid.len()];
    for k in 1..a.n_span {
        for i in 0..=a.n_i() {
            if matches!(wake_index(&a, i), Some(p) if p >= 1) {
                for j in 0..=a.n_side() {
                    changed[a.c_node(i, j, k)] = true;
                }
            }
        }
    }
    let bits = |p: Vec3| p.map(f64::to_bits);
    let outside = (0..a.grid.len())
        .filter(|&n| !changed[n] && bits(a.grid[n]) != bits(b.grid[n]))
        .collect::<Vec<_>>();
    let inside = (0..a.grid.len())
        .filter(|&n| changed[n] && bits(a.grid[n]) != bits(b.grid[n]))
        .count();
    println!(
        "W7: steps {} / {}; nodes outside the changed set that differ: {} (first {:?}); inside that differ: {inside}",
        a.construction_step,
        b.construction_step,
        outside.len(),
        outside.first()
    );
    assert!(outside.is_empty(), "W7 missed");
    assert!(inside > 0, "NC-W7 missed");
}

/// HS (ADR 0699, c6): construction S on the reference at s4 keeps the ADR 0696 BS hash (Windows; other platforms print it).
#[test]
fn hs_construction_s_is_bit_identical() {
    let m = build_cgrid_mesh_with(&reference(), &s4(), &CGridOptions::camber_seam_s_construction()).unwrap();
    let got = mesh_hash(&m.mesh);
    println!("HS camber_seam_s reference s4: {got:#018x}");
    if cfg!(windows) {
        assert_eq!(got, 0x0105a2fcca924d23);
    } else {
        eprintln!("HS: no recorded hash for this platform; hash {got:#018x}");
    }
}

/// HS' (ADR 0699, c6): construction S' on the reference and on the second blade at s4 keeps the hashes computed on a `git
/// archive` copy of 2acf29d9 (`f782-scratch/base_hs.log`, Windows; ADR 0699 amendment 1). Other platforms print them.
#[test]
fn hs_prime_construction_s_prime_is_bit_identical() {
    for (tag, fan, want) in [("REF", reference(), HS_PRIME_REF), ("B2", second_blade(), HS_PRIME_B2)] {
        let m = build_cgrid_mesh_with(&fan, &s4(), &s_prime()).unwrap();
        let got = mesh_hash(&m.mesh);
        println!("HS' camber_seam_s_unstretched {tag} s4: {got:#018x}");
        if cfg!(windows) {
            assert_eq!(got, want, "HS' {tag}");
        } else {
            eprintln!("HS': no recorded hash for this platform; hash {got:#018x}");
        }
    }
}

/// The hashes of HS' on the base copy (filled from `f782-scratch/base_hs.log` before the registered run).
const HS_PRIME_REF: u64 = 0x0d257b2fcee093ca;
const HS_PRIME_B2: u64 = 0xefc53b8c413a845b;
