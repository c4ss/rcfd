//! F-791 (ADR 0713): construction S'''' of the camber-seam C-grid (`CGridOptions::camber_seam_s_free_rows_construction`,
//! construction S''' of ADR 0702 with the free cells of every C line grown by the hub layer's ratio of that line in every layer and
//! only the cap solved per layer) on the reference fan `ComputerFan::rom_reference()` blade 0 and on the ADR 0696 second blade at the
//! ADR 0118 s4 specs: no folded cell within 84.5 deg, skewness 4.0 and aspect 1000; the wake-stack spanwise skewness within 1.0;
//! the first-free-row faces of the thick span layers; the locality of the change; a front_gap / n_nose scan; camber_seam_s,
//! camber_seam_s_unstretched, camber_seam_s_wake_remap and camber_seam_s_wake_ramp unchanged in bits. Limits registered in ADR
//! 0713 before this code ran.

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

/// Registered limits (ADR 0713 R2 to R5, R2f, R2m, Rrow, Rfirst, R8).
const NON_ORTH_MAX: f64 = 84.5;
const FIRST_ROW_MAX: f64 = 84.0;
const RADIAL_D_MAX: f64 = 0.60;
const SKEW_MAX: f64 = 4.0;
const ASPECT_MAX: f64 = 1000.0;
const WAKE_SKEW_MAX: f64 = 1.0;
const ROW_RATIO_BAND: (f64, f64) = (0.95, 1.05);
const FIRST_CELL_REL: f64 = 1e-6;
const RATIO_BAND: (f64, f64) = (3.1, 3.3);

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

fn s_fourth() -> CGridOptions {
    CGridOptions::camber_seam_s_free_rows_construction()
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

fn dist(a: Vec3, b: Vec3) -> f64 {
    let d = sub(a, b);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// The face between the first and second free cells C (i, n_bl) / (i, n_bl + 1) at cell layer `k`.
fn first_row_face(m: &CGridMesh, id: &HashMap<(usize, usize, usize), usize>, i: usize, k: usize) -> usize {
    face_between(&m.mesh, id[&(i, m.n_bl, k)], id[&(i, m.n_bl + 1, k)])
}

/// R2f measure: the largest non-orthogonality of the first-free-row faces of every blade line `n_down <= i < n_i - n_down` at the
/// cell layers 3 to 6, and where.
fn first_row_non_orthogonality(m: &CGridMesh) -> (f64, usize, usize) {
    let q = compute_metrics(&m.mesh);
    let id = c_ids(m);
    let mut best = (f64::NEG_INFINITY, 0, 0);
    for k in 3..=6 {
        for i in m.n_down()..m.n_i() - m.n_down() {
            let v = q.non_orthogonality[first_row_face(m, &id, i, k)];
            if !(v <= best.0) {
                best = (v, i, k);
            }
        }
    }
    best
}

/// R2m measure: the radial component of the unit vector between the cell centroids of C (45, 2) and C (45, 3) at layer 3, at the
/// face centre.
fn radial_centroid_component(m: &CGridMesh) -> f64 {
    let id = c_ids(m);
    let (o, n) = (id[&(45, 2, 3)], id[&(45, 3, 3)]);
    let f = face_between(&m.mesh, o, n);
    let cc = m.mesh.cell_centres();
    let x = m.mesh.face_centres()[f];
    let d = sub(cc[n], cc[o]);
    let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
    (d[0] * x[0] / r + d[1] * x[1] / r) / dist(cc[n], cc[o])
}

/// The step along C line `i` from row `j` to row `j + 1` at node layer `k` (final grid).
fn step(m: &CGridMesh, i: usize, j: usize, k: usize) -> f64 {
    dist(m.grid[m.c_node(i, j, k)], m.grid[m.c_node(i, j + 1, k)])
}

/// Rrow measure: REF line 45, the step from row 3 to row 4 at node layer 4 over that at node layer 3.
fn row_ratio(m: &CGridMesh) -> f64 {
    step(m, 45, 3, 4) / step(m, 45, 3, 3)
}

/// R1a to R5, R2f, R2w, R8 for construction S'''' on `fan` at s4.
fn r_checks(tag: &str, fan: &AxialFan) {
    let mut ch = Checks::new();
    let m = match build_cgrid_mesh_with(fan, &s4(), &s_fourth()) {
        Ok(m) => m,
        Err(e) => {
            ch.holds(&format!("{tag} R1a S'''' builds"), false, &format!("{e:?}"));
            ch.finish();
            unreachable!()
        }
    };
    let m3 = build_cgrid_mesh_with(fan, &s4(), &s_third()).expect("S''' builds");
    ch.holds(
        &format!("{tag} R1a S'''' builds"),
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
    ch.at_most(
        &format!("{tag} R2 max non-orthogonality (all faces) [deg]"),
        q.max_non_orthogonality,
        NON_ORTH_MAX,
    );
    let (fv, fi, fk) = first_row_non_orthogonality(&m);
    let (fv3, fi3, fk3) = first_row_non_orthogonality(&m3);
    ch.at_most(
        &format!(
            "{tag} R2f max non-orthogonality of the first-free-row faces [deg] at line {fi}, layer {fk} (S''' {fv3:.4} at line {fi3}, layer {fk3})"
        ),
        fv,
        FIRST_ROW_MAX,
    );
    let (wn, wat) = wake_cell_non_orthogonality(&m);
    ch.at_most(
        &format!("{tag} R2w max non-orthogonality of the wake-cell faces [deg] at {wat}"),
        wn,
        NON_ORTH_MAX,
    );
    ch.at_most(&format!("{tag} R3 max skewness"), q.max_skewness, SKEW_MAX);
    ch.at_most(&format!("{tag} R4 max aspect ratio"), q.max_aspect_ratio, ASPECT_MAX);
    let ws = wake_spanwise_skewness(&m);
    let ws3 = wake_spanwise_skewness(&m3);
    println!(
        "{tag} Rep largest wake-stack spanwise skewness (skewness, i, j, k): {:?}; S''' {:?}",
        &ws[..3],
        &ws3[..1]
    );
    ch.at_most(
        &format!("{tag} R5 wake-stack spanwise skewness"),
        ws[0].0,
        WAKE_SKEW_MAX,
    );
    let ratios = m.free_row_ratios.clone().unwrap_or_default();
    if tag == "REF" {
        let r45 = ratios.get(45).copied().unwrap_or(f64::NAN);
        ch.holds(
            "REF R8 free_row_ratios line 45",
            r45 >= RATIO_BAND.0 && r45 <= RATIO_BAND.1,
            &format!("{r45:.5} (limit [{}, {}])", RATIO_BAND.0, RATIO_BAND.1),
        );
    }
    // reported
    let qm = compute_metrics(&m.mesh);
    let qm3 = compute_metrics(&m3.mesh);
    let id = c_ids(&m);
    let id3 = c_ids(&m3);
    let row: Vec<String> = (0..m.n_span)
        .map(|k| {
            format!(
                "{k}: {:.3} ({:.3})",
                qm.non_orthogonality[first_row_face(&m, &id, 45, k)],
                qm3.non_orthogonality[first_row_face(&m3, &id3, 45, k)]
            )
        })
        .collect();
    println!("{tag} Rep first-free-row face of line 45 per layer, S'''' (S'''): {row:?}");
    for i in [44usize, 46, 47, 48] {
        let row: Vec<String> = [3usize, 4, 5, 6]
            .iter()
            .map(|&k| format!("{k}: {:.3}", qm.non_orthogonality[first_row_face(&m, &id, i, k)]))
            .collect();
        println!("{tag} Rep first-free-row face of line {i}: {row:?}");
    }
    let mut order: Vec<usize> = (0..m.mesh.n_internal_faces()).collect();
    order.sort_by(|a, b| qm.non_orthogonality[*b].total_cmp(&qm.non_orthogonality[*a]));
    for &f in order.iter().take(5) {
        println!(
            "{tag} Rep large face: {:.4} deg {} / {}",
            qm.non_orthogonality[f],
            tag_name(&m, m.mesh.owner()[f]),
            tag_name(&m, m.mesh.neighbour()[f])
        );
    }
    println!(
        "{tag} Rep first ramp face: S'''' {:.4}, S''' {:.4} deg; wake_ramp_length {:?} (S''' {:?})",
        first_ramp_face(&m),
        first_ramp_face(&m3),
        m.wake_ramp_length,
        m3.wake_ramp_length
    );
    let rr: Vec<String> = [44usize, 45, 46, 47, 48, 56]
        .iter()
        .map(|&i| format!("{i}: {:.4}", ratios.get(i).copied().unwrap_or(f64::NAN)))
        .collect();
    println!("{tag} Rep free_row_ratios {rr:?}");
    println!("{tag} Rep S'''': {q:?}");
    println!("{tag} Rep {}", locate(&m));
    ch.finish();
}

// ---------------------------------------------------------------- the registered checks

/// R1a to R5, R2f, R2w and R8 (ADR 0713, c2 to c6): S'''' on the reference at s4.
#[test]
fn r_construction_s_fourth_meshes_the_reference() {
    r_checks("REF", &reference());
}

/// R1a to R5, R2f and R2w (ADR 0713, c2 to c6): S'''' on the ADR 0696 second blade (outer stagger 76.6 deg) at s4.
#[test]
fn b2_construction_s_fourth_meshes_the_second_blade() {
    let st = second_blade().stations;
    assert!(st.last().unwrap().stagger.to_degrees() >= 75.0);
    r_checks("B2", &second_blade());
}

/// R2m, Rrow and Rfirst (ADR 0713, c2): the radial part of the centroid vector of the worst S''' face, the layer-to-layer ratio of
/// the second free cell of line 45, and the first free cell unchanged against S'''.
#[test]
fn r2m_rrow_rfirst_the_near_wall_rows() {
    let mut ch = Checks::new();
    let m = build_cgrid_mesh_with(&reference(), &s4(), &s_fourth()).expect("S'''' builds");
    ch.at_most(
        "REF R2m radial component of the centroid vector, C (45, 2) / (45, 3), layer 3",
        radial_centroid_component(&m),
        RADIAL_D_MAX,
    );
    let r = row_ratio(&m);
    ch.holds(
        "REF Rrow step row 3 -> 4, node layer 4 over node layer 3",
        r >= ROW_RATIO_BAND.0 && r <= ROW_RATIO_BAND.1,
        &format!("{r:.5} (limit [{}, {}])", ROW_RATIO_BAND.0, ROW_RATIO_BAND.1),
    );
    for (tag, fan) in [("REF", reference()), ("B2", second_blade())] {
        let a = build_cgrid_mesh_with(&fan, &s4(), &s_fourth()).expect("S'''' builds");
        let b = build_cgrid_mesh_with(&fan, &s4(), &s_third()).expect("S''' builds");
        let worst = (0..=a.n_k)
            .map(|k| ((step(&a, 45, 2, k) - step(&b, 45, 2, k)) / step(&b, 45, 2, k)).abs())
            .fold(0.0f64, f64::max);
        ch.at_most(
            &format!("{tag} Rfirst first free cell of line 45 against S''' (relative)"),
            worst,
            FIRST_CELL_REL,
        );
    }
    ch.finish();
}

/// NC-R2, NC-R2m, NC-Rrow and NC-R8 (ADR 0713): the measures see the S''' miss, its radial centroid offset and its varying second
/// free cell, and S''' carries no hub ratios.
#[test]
fn nc_the_measures_see_s_third() {
    let mut ch = Checks::new();
    let m = build_cgrid_mesh_with(&reference(), &s4(), &s_third()).expect("S''' builds");
    let q = m.quality();
    let id = c_ids(&m);
    let v = compute_metrics(&m.mesh).non_orthogonality[first_row_face(&m, &id, 45, 3)];
    ch.holds(
        "NC-R2 S''' REF max non-orthogonality at C (45, 2) / (45, 3), layer 3",
        (q.max_non_orthogonality - 84.688).abs() <= 1e-3 && v == q.max_non_orthogonality,
        &format!("{:.5} deg, the face {v:.5} (ADR 0702: 84.688)", q.max_non_orthogonality),
    );
    let d = radial_centroid_component(&m);
    ch.holds(
        "NC-R2m S''' radial component",
        (d - 0.710).abs() <= 0.005,
        &format!("{d:.5} (probe: 0.710)"),
    );
    let r = row_ratio(&m);
    ch.holds(
        "NC-Rrow S''' row ratio",
        (r - 1.174).abs() <= 0.005,
        &format!("{r:.5} (probe: 1.174)"),
    );
    for (tag, fan) in [("REF", reference()), ("B2", second_blade())] {
        let m = build_cgrid_mesh_with(&fan, &s4(), &s_third()).expect("S''' builds");
        ch.holds(
            &format!("NC-R8 {tag} S''' free_row_ratios"),
            m.free_row_ratios.is_none(),
            &format!("{:?}", m.free_row_ratios.as_ref().map(|v| v.len())),
        );
    }
    ch.finish();
}

/// R7 and NC-R7 (ADR 0713, c2): on the algebraic grids of S'''' and S''' of both blades, every grid node except the free C nodes
/// (`stack_cells[k][i] < j < n_side`) is equal in bits, and some free node differs.
#[test]
fn r7_the_change_is_local_to_the_free_rows() {
    for (tag, fan) in [("REF", reference()), ("B2", second_blade())] {
        let a = build_cgrid_mesh_with(&fan, &s4(), &algebraic(s_fourth())).expect("the algebraic grid of S'''' builds");
        let b = build_cgrid_mesh_with(&fan, &s4(), &algebraic(s_third())).expect("the algebraic grid of S''' builds");
        assert_eq!(a.grid.len(), b.grid.len());
        let mut free = vec![false; a.grid.len()];
        for k in 0..=a.n_k {
            for i in 0..=a.n_i() {
                for j in a.stack_cells[k][i] + 1..a.n_side() {
                    free[a.c_node(i, j, k)] = true;
                }
            }
        }
        let bits = |p: Vec3| p.map(f64::to_bits);
        let differs = |n: usize| bits(a.grid[n]) != bits(b.grid[n]);
        let outside: Vec<usize> = (0..a.grid.len()).filter(|&n| !free[n] && differs(n)).collect();
        let inside = (0..a.grid.len()).filter(|&n| free[n] && differs(n)).count();
        println!(
            "R7 {tag}: steps {} / {}; outside the free nodes differ {} (first {:?}); free nodes differ {inside}",
            a.construction_step,
            b.construction_step,
            outside.len(),
            outside.first()
        );
        assert!(outside.is_empty(), "R7 {tag} missed");
        assert!(inside > 0, "NC-R7 {tag} missed");
    }
}

/// HS (ADR 0713, c8): construction S on the reference at s4 keeps the ADR 0696 BS hash (Windows; other platforms print it).
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

/// HS', HS'' and HS''' (ADR 0713, c8): constructions S', S'' and S''' on both blades at s4 keep the hashes of the base (S''': the
/// ADR 0713 base probe on a `git archive` copy of ad931721, `f791-scratch/probe.log`). Windows; other platforms print them.
#[test]
fn hs_prime_second_and_third_are_bit_identical() {
    for (name, opts, want_ref, want_b2) in [
        ("S'", s_prime(), 0x0d257b2fcee093cau64, 0xefc53b8c413a845bu64),
        ("S''", s_second(), 0x63011aa8288ac9c0, 0x769e2e330b0cf295),
        ("S'''", s_third(), 0xba8d86a18ca34784, 0x64432ac2c53ab55a),
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

/// The registered scan of ADR 0713 (SC, as ADR 0702): per blade, `front_gap` x `n_nose`, the other s4 values.
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

/// SC0 (ADR 0713, c7): every scan combination is inside the camber_seam range.
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

/// SC (ADR 0713, c7): S'''' builds at least 9 of the 12 scan combinations of each blade fold-free and periodic (R1a to R1c);
/// quality reported. Heavy (24 builds on three threads): a record, run once; the s4 point is covered by the fast tests above.
#[test]
#[ignore = "heavy record of ADR 0713 SC (24 builds): cargo test -p rcfd-mesh --release --test f791_cgrid_free_rows sc_the_scan -- --ignored --nocapture"]
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
                let line = match build_cgrid_mesh_with(fan, spec, &s_fourth()) {
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
                        let (fv, fi, fk) = first_row_non_orthogonality(&m);
                        (
                            n,
                            label.clone(),
                            built,
                            format!(
                                "step {}, {} cells, folds {} / {} / {}, periodicity {per:.2e}, {:.3} deg (first free rows {fv:.3} at line {fi}, layer {fk}), {:.4}, {:.1}, wake spanwise skew {:.4}",
                                m.construction_step,
                                m.mesh.n_cells(),
                                pos,
                                neg,
                                zero,
                                q.max_non_orthogonality,
                                q.max_skewness,
                                q.max_aspect_ratio,
                                ws[0].0,
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
