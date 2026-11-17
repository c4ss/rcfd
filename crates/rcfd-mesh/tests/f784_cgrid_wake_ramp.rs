//! F-784 (ADR 0702): construction S''' of the camber-seam C-grid (`CGridOptions::camber_seam_s_wake_ramp_construction`,
//! construction S'' of ADR 0699 with the ramp of the wake-stack remap set over the length `L = 1.5 D / tan(1 deg)` along the wake)
//! on the reference fan `ComputerFan::rom_reference()` blade 0 and on the ADR 0696 second blade at the ADR 0118 s4 specs: no folded
//! cell within 84.5 deg, skewness 4.0 and aspect 1000; the wake-stack spanwise skewness within 1.0; the first ramp face; the
//! locality of the change; a front_gap / n_nose scan; camber_seam_s, camber_seam_s_unstretched and camber_seam_s_wake_remap
//! unchanged in bits. Limits registered in ADR 0702 before this code ran.

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

/// Registered limits (ADR 0702 R2 to R6).
const NON_ORTH_MAX: f64 = 84.5;
const SKEW_MAX: f64 = 4.0;
const ASPECT_MAX: f64 = 1000.0;
const WAKE_SKEW_MAX: f64 = 1.0;
const DRIFT_MAX: f64 = 5e-6;
const FIRST_FACE_RISE_MAX: f64 = 0.03;

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

fn s_third() -> CGridOptions {
    CGridOptions::camber_seam_s_wake_ramp_construction()
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

// ---------------------------------------------------------------- the ADR 0699 measures (as f782_cgrid_wake_remap.rs)

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

/// The C cell ids by `(i, j, k)`.
fn c_ids(m: &CGridMesh) -> HashMap<(usize, usize, usize), usize> {
    let mut id = HashMap::new();
    for (c, t) in m.cell_tags.iter().enumerate() {
        if t.0 == 0 {
            id.insert((t.1, t.2, t.3), c);
        }
    }
    id
}

/// The internal face between two cells.
fn face_between(mesh: &Mesh, a: usize, b: usize) -> usize {
    mesh.cell_faces(a)
        .iter()
        .copied()
        .find(|&f| {
            f < mesh.n_internal_faces()
                && ((mesh.owner()[f] == a && mesh.neighbour()[f] == b)
                    || (mesh.owner()[f] == b && mesh.neighbour()[f] == a))
        })
        .expect("the face between two neighbouring cells")
}

/// The non-orthogonality of the first ramp face: between C (n_i - n_down, 2) and (n_i - n_down, 3) at layer 3 (ADR 0702).
fn first_ramp_face(m: &CGridMesh) -> f64 {
    let id = c_ids(m);
    let i = m.n_i() - m.n_down();
    let f = face_between(&m.mesh, id[&(i, 2, 3)], id[&(i, 3, 3)]);
    compute_metrics(&m.mesh).non_orthogonality[f]
}

/// R2w measure: the largest non-orthogonality over the internal faces with an owner or neighbour among the C wake cells, and where.
fn wake_cell_non_orthogonality(m: &CGridMesh) -> (f64, String) {
    let mesh = &m.mesh;
    let q = compute_metrics(mesh);
    let (n_i, nd) = (m.n_i(), m.n_down());
    let wake = |c: usize| {
        let t = m.cell_tags[c];
        t.0 == 0 && (t.1 < nd || t.1 >= n_i - nd)
    };
    let mut best = (f64::NEG_INFINITY, String::new());
    for f in 0..mesh.n_internal_faces() {
        let (o, n) = (mesh.owner()[f], mesh.neighbour()[f]);
        if (wake(o) || wake(n)) && !(q.non_orthogonality[f] <= best.0) {
            best = (
                q.non_orthogonality[f],
                format!("{} / {}", tag_name(m, o), tag_name(m, n)),
            );
        }
    }
    best
}

/// The ADR 0699 W6 drift restricted to the wake lines `p` in `ps`: over every thin span cell `k`, every stack row `j <= n_bl`, the
/// largest `max(|dz|, r_{k+1} |dtheta|)` between node layers `k` and `k + 1`: `(drift, k, i, j)`.
fn thin_layer_drift(m: &CGridMesh, ps: std::ops::RangeInclusive<usize>) -> (f64, usize, usize, usize) {
    let r = &m.radii;
    let mean = (r[m.n_span] - r[0]) / m.n_span as f64;
    let mut worst = (0.0f64, 0, 0, 0);
    for k in 0..m.n_span {
        if !(r[k + 1] - r[k] < mean) {
            continue;
        }
        for i in 0..=m.n_i() {
            match wake_index(m, i) {
                Some(p) if ps.contains(&p) => {}
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

fn dist(a: Vec3, b: Vec3) -> f64 {
    let d = sub(a, b);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// R1a to R8 for construction S''' on `fan` at s4 (the global R2 only with `global_limit`; the REF record has its own test).
fn r_checks(tag: &str, fan: &AxialFan, global_limit: bool) {
    let mut ch = Checks::new();
    let m = match build_cgrid_mesh_with(fan, &s4(), &s_third()) {
        Ok(m) => m,
        Err(e) => {
            ch.holds(&format!("{tag} R1a S''' builds"), false, &format!("{e:?}"));
            ch.finish();
            unreachable!()
        }
    };
    let m1 = build_cgrid_mesh_with(fan, &s4(), &s_prime()).expect("S' builds");
    let m2 = build_cgrid_mesh_with(fan, &s4(), &s_second()).expect("S'' builds");
    ch.holds(
        &format!("{tag} R1a S''' builds"),
        m.mesh.n_cells() > 0,
        &format!("{} cells, construction step {}", m.mesh.n_cells(), m.construction_step),
    );
    let q = m.quality();
    let (pos, neg, zero, first) = fold_count(&m);
    ch.holds(
        &format!("{tag} R1b no folded cell (corner Jacobians of one sign, min volume > 0)"),
        (pos == 0 || neg == 0) && zero == 0 && q.min_volume > 0.0,
        &format!(
            "positive {pos}, negative {neg}, zero {zero}, first minority {first:?}, min volume {:.4e}",
            q.min_volume
        ),
    );
    ch.at_most(&format!("{tag} R1c periodicity [m]"), periodicity(&m, 7), 1e-12);
    if global_limit {
        ch.at_most(
            &format!("{tag} R2 max non-orthogonality (all faces) [deg]"),
            q.max_non_orthogonality,
            NON_ORTH_MAX,
        );
    } else {
        println!(
            "{tag} R2 max non-orthogonality (all faces, reported here; the check is the REF record): {:.4} deg",
            q.max_non_orthogonality
        );
    }
    let (wn, wat) = wake_cell_non_orthogonality(&m);
    ch.at_most(
        &format!("{tag} R2w max non-orthogonality of the wake-cell faces [deg] at {wat}"),
        wn,
        NON_ORTH_MAX,
    );
    let (f3, f1, f2) = (first_ramp_face(&m), first_ramp_face(&m1), first_ramp_face(&m2));
    println!("{tag} R2t first ramp face: S' {f1:.4}, S'' {f2:.4}, S''' {f3:.4} deg");
    ch.at_most(
        &format!("{tag} R2t first ramp face S''' - S' [deg]"),
        f3 - f1,
        FIRST_FACE_RISE_MAX,
    );
    ch.at_most(&format!("{tag} R3 max skewness"), q.max_skewness, SKEW_MAX);
    ch.at_most(&format!("{tag} R4 max aspect ratio"), q.max_aspect_ratio, ASPECT_MAX);
    let ws = wake_spanwise_skewness(&m);
    println!(
        "{tag} Rep largest wake-stack spanwise skewness (skewness, i, j, k): {:?}",
        &ws[..5]
    );
    ch.at_most(
        &format!("{tag} R5 wake-stack spanwise skewness"),
        ws[0].0,
        WAKE_SKEW_MAX,
    );
    let (d, k, i, j) = thin_layer_drift(&m, 3..=m.n_down());
    ch.at_most(
        &format!("{tag} R6 thin-layer wake-stack drift, lines p >= 3 [m] (cell {k}, line {i}, row {j})"),
        d,
        DRIFT_MAX,
    );
    for p in 1..=2 {
        let (d, k, i, j) = thin_layer_drift(&m, p..=p);
        let (d1, ..) = thin_layer_drift(&m1, p..=p);
        println!(
            "{tag} Rep thin-layer drift on line p = {p}: S''' {d:.3e} m (cell {k}, line {i}, row {j}), S' {d1:.3e} m"
        );
    }
    // omega_p from the node shifts on the upper line, stack row 1: S'' has omega 0.5 at p = 1 and 1 from p = 2 on
    for k in [1usize, 3, 4, 7, 9] {
        let row: Vec<String> = (1..=3)
            .map(|p| {
                let n = m.c_node(m.n_i() - m.n_down() + p, 1, k);
                let full = dist(m2.grid[n], m1.grid[n]) / if p == 1 { 0.5 } else { 1.0 };
                format!("p {p}: {:.4}", dist(m.grid[n], m1.grid[n]) / full)
            })
            .collect();
        println!("{tag} Rep omega at node layer {k}: {row:?}");
    }
    let l = m.wake_ramp_length;
    let (lo, hi) = if tag == "REF" {
        (6.5e-3, 7.5e-3)
    } else {
        (7.2e-3, 8.2e-3)
    };
    ch.holds(
        &format!("{tag} R8 wake_ramp_length"),
        matches!(l, Some(x) if x >= lo && x <= hi),
        &format!("{l:?} (limit [{lo}, {hi}] m)"),
    );
    println!("{tag} Rep S''': {q:?}");
    println!("{tag} Rep {}", locate(&m));
    ch.finish();
}

// ---------------------------------------------------------------- the registered checks

/// R1a to R8 except the global R2 (ADR 0702, c2 to c6): S''' on the reference at s4.
#[test]
fn r_construction_s_third_meshes_the_reference() {
    r_checks("REF", &reference(), false);
}

/// R2-REF (ADR 0702, c4): the global non-orthogonality of S''' on the reference within 84.5 deg. Predicted to miss in the
/// registration at the blade-line face C (45, 2) / (45, 3), layer 3 (84.688 deg on S' and S'', not a wake face), so written as an
/// ignored record from the start and run once.
#[test]
#[ignore = "ADR 0702 R2-REF, predicted miss (registered), measured 84.688 deg at the blade-line face C (45, 2) / (45, 3), layer 3 (limit 84.5): cargo test -p rcfd-mesh --release --test f784_cgrid_wake_ramp r2_ref -- --ignored --nocapture"]
fn r2_ref_global_non_orthogonality_known_miss() {
    let m = build_cgrid_mesh_with(&reference(), &s4(), &s_third()).expect("S''' builds");
    let q = m.quality();
    println!(
        "R2-REF: {:.4} deg (limit {NON_ORTH_MAX}); {}",
        q.max_non_orthogonality,
        locate(&m)
    );
    assert!(q.max_non_orthogonality <= NON_ORTH_MAX, "R2-REF missed");
}

/// R1a to R8 with the global R2 (ADR 0702, c2 to c6): S''' on the ADR 0696 second blade (outer stagger 76.6 deg) at s4.
#[test]
fn b2_construction_s_third_meshes_the_second_blade() {
    let st = second_blade().stations;
    assert!(st.last().unwrap().stagger.to_degrees() >= 75.0);
    r_checks("B2", &second_blade(), true);
}

/// NC-R2t, NC-R5 and NC-R8 (ADR 0702): the first-ramp-face measure sees the ADR 0699 rise on S'', the R5 measure sees the S'
/// shear, and S'' carries no ramp length.
#[test]
fn nc_the_measures_see_s_second_and_s_prime() {
    let mut ch = Checks::new();
    for (tag, fan, want2, want5) in [
        ("REF", reference(), 84.953, 3.2404),
        ("B2", second_blade(), 84.472, 4.0112),
    ] {
        let m2 = build_cgrid_mesh_with(&fan, &s4(), &s_second()).expect("S'' builds");
        let m1 = build_cgrid_mesh_with(&fan, &s4(), &s_prime()).expect("S' builds");
        let f = first_ramp_face(&m2);
        ch.holds(
            &format!("NC-R2t {tag} S'' first ramp face"),
            (f - want2).abs() <= 1e-3,
            &format!("{f:.5} deg (ADR 0699: {want2})"),
        );
        let ws = wake_spanwise_skewness(&m1);
        ch.holds(
            &format!("NC-R5 {tag} S' wake-stack spanwise skewness"),
            (ws[0].0 - want5).abs() <= 1e-4,
            &format!(
                "{:.6} at {:?} (ADR 0696: {want5})",
                ws[0].0,
                (ws[0].1, ws[0].2, ws[0].3)
            ),
        );
        ch.holds(
            &format!("NC-R8 {tag} S'' wake_ramp_length"),
            m2.wake_ramp_length.is_none(),
            &format!("{:?}", m2.wake_ramp_length),
        );
    }
    ch.finish();
}

/// R7 and NC-R7 (ADR 0702, c2): on the algebraic grids of S''' and S'' of both blades, every node outside the C wake lines `p = 1,
/// 2, 3` of the node layers 1 to `n_span - 1` is equal in bits, and some node of the lines `p = 1, 2` differs; the nodes of `p = 3`
/// that differ are reported.
#[test]
fn r7_the_change_is_local_to_the_first_wake_lines() {
    for (tag, fan) in [("REF", reference()), ("B2", second_blade())] {
        let a = build_cgrid_mesh_with(&fan, &s4(), &algebraic(s_third())).expect("the algebraic grid of S''' builds");
        let b = build_cgrid_mesh_with(&fan, &s4(), &algebraic(s_second())).expect("the algebraic grid of S'' builds");
        assert_eq!(a.grid.len(), b.grid.len());
        let mut set = vec![0usize; a.grid.len()];
        for k in 1..a.n_span {
            for i in 0..=a.n_i() {
                if let Some(p) = wake_index(&a, i) {
                    if (1..=3).contains(&p) {
                        for j in 0..=a.n_side() {
                            set[a.c_node(i, j, k)] = p;
                        }
                    }
                }
            }
        }
        let bits = |p: Vec3| p.map(f64::to_bits);
        let differs = |n: usize| bits(a.grid[n]) != bits(b.grid[n]);
        let outside: Vec<usize> = (0..a.grid.len()).filter(|&n| set[n] == 0 && differs(n)).collect();
        let in12 = (0..a.grid.len())
            .filter(|&n| (set[n] == 1 || set[n] == 2) && differs(n))
            .count();
        let in3 = (0..a.grid.len()).filter(|&n| set[n] == 3 && differs(n)).count();
        println!(
            "R7 {tag}: steps {} / {}; outside the set differ {} (first {:?}); lines p = 1, 2 differ {in12}; line p = 3 differ {in3}",
            a.construction_step,
            b.construction_step,
            outside.len(),
            outside.first()
        );
        assert!(outside.is_empty(), "R7 {tag} missed");
        assert!(in12 > 0, "NC-R7 {tag} missed");
    }
}

/// HS (ADR 0702, c8): construction S on the reference at s4 keeps the ADR 0696 BS hash (Windows; other platforms print it).
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

/// HS' and HS'' (ADR 0702, c8): constructions S' and S'' on both blades at s4 keep the hashes of the base (S': ADR 0699 amendment
/// 1; S'': the ADR 0702 base probe on a `git archive` copy of 6642beb3, `f784-scratch/probe.log`). Windows; other platforms print
/// them.
#[test]
fn hs_prime_and_second_are_bit_identical() {
    for (name, opts, want_ref, want_b2) in [
        ("S'", s_prime(), 0x0d257b2fcee093cau64, 0xefc53b8c413a845bu64),
        ("S''", s_second(), 0x63011aa8288ac9c0, 0x769e2e330b0cf295),
    ] {
        for (tag, fan, want) in [("REF", reference(), want_ref), ("B2", second_blade(), want_b2)] {
            let m = build_cgrid_mesh_with(&fan, &s4(), &opts).unwrap();
            let got = mesh_hash(&m.mesh);
            println!("{name} {tag} s4: {got:#018x}");
            if cfg!(windows) {
                assert_eq!(got, want, "{name} {tag}");
            } else {
                eprintln!("{name}: no recorded hash for this platform; hash {got:#018x}");
            }
        }
    }
}

/// The registered scan of ADR 0702 (SC, as ADR 0696): per blade, `front_gap` x `n_nose`, the other s4 values.
fn scan_list() -> Vec<(String, AxialFan, CGridSpec)> {
    let mut out = Vec::new();
    for (name, fan) in [("REF", reference()), ("B2", second_blade())] {
        for fg in [0.15, 0.3, 0.6, 1.0] {
            for nn in [4, 6, 9] {
                out.push((
                    format!("{name} front_gap {fg} n_nose {nn}"),
                    fan.clone(),
                    CGridSpec {
                        front_gap: fg,
                        n_nose: nn,
                        ..s4()
                    },
                ));
            }
        }
    }
    out
}

/// SC0 (ADR 0702, c7): every scan combination is inside the camber_seam range.
#[test]
fn sc0_the_scan_is_inside_the_camber_seam_range() {
    let list = scan_list();
    let inside = list
        .iter()
        .filter(|(_, _, sp)| control_range_problems(sp).is_empty())
        .count();
    println!("SC0: {inside} of {} inside", list.len());
    assert_eq!(inside, 24);
    assert_eq!(list.len(), 24);
}

/// SC (ADR 0702, c7): S''' builds at least 9 of the 12 scan combinations of each blade fold-free and periodic (R1a to R1c); quality
/// and the ramp length reported. Heavy (24 builds on three threads): a record, run once; the s4 point is covered by the fast tests
/// above.
#[test]
#[ignore = "heavy record of ADR 0702 SC (24 builds): cargo test -p rcfd-mesh --release --test f784_cgrid_wake_ramp sc_the_scan -- --ignored --nocapture"]
fn sc_the_scan_heavy() {
    let list = scan_list();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..3 {
            sc.spawn(|| loop {
                let n = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n >= list.len() {
                    break;
                }
                let (label, fan, spec) = &list[n];
                let line = match build_cgrid_mesh_with(fan, spec, &s_third()) {
                    Ok(m) => {
                        let q = m.quality();
                        let (pos, neg, zero, _) = fold_count(&m);
                        let per = periodicity(&m, 7);
                        let built = m.mesh.n_cells() > 0
                            && (pos == 0 || neg == 0)
                            && zero == 0
                            && q.min_volume > 0.0
                            && per <= 1e-12;
                        let ws = wake_spanwise_skewness(&m);
                        let (wn, _) = wake_cell_non_orthogonality(&m);
                        (
                            n,
                            label.clone(),
                            built,
                            format!(
                                "step {}, {} cells, folds {} / {} / {}, periodicity {per:.2e}, {:.3} deg (wake cells {wn:.3}), {:.4}, {:.1}, wake spanwise skew {:.4}, L {:?}",
                                m.construction_step,
                                m.mesh.n_cells(),
                                pos,
                                neg,
                                zero,
                                q.max_non_orthogonality,
                                q.max_skewness,
                                q.max_aspect_ratio,
                                ws[0].0,
                                m.wake_ramp_length
                            ),
                        )
                    }
                    Err(e) => (n, label.clone(), false, format!("refused: {e:?}")),
                };
                results.lock().unwrap().push(line);
            });
        }
    });
    let mut res = results.into_inner().unwrap();
    res.sort_by_key(|r| r.0);
    for r in &res {
        println!("SC {}: built {}; {}", r.1, r.2, r.3);
    }
    let mut ch = Checks::new();
    for name in ["REF", "B2"] {
        let mine: Vec<_> = res.iter().filter(|r| r.1.starts_with(&format!("{name} "))).collect();
        let built = mine.iter().filter(|r| r.2).count();
        ch.holds(
            &format!("SC {name} builds fold-free and periodic"),
            mine.len() == 12 && built >= 9,
            &format!("{built} of {} (limit 9 of 12)", mine.len()),
        );
    }
    ch.finish();
}
