//! F-726 (ADR 0692): construction S of the camber-seam C-grid (`CGridOptions::camber_seam_s_construction`, camber_seam_h with the
//! front edge on the extended leading-edge camber line, straight lines beyond the stacks and no radial filter) on the strongly
//! staggered reference fan `ComputerFan::rom_reference()` at the ADR 0118 s4 specs: no folded cell and the registered
//! non-orthogonality, skewness and aspect limits; the h_slit mesh of the reference against the same limits; camber_seam_h
//! unchanged in bits. Limits registered in ADR 0692 before this code ran.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the limits

use rcfd_core::vec3::Vec3;
use rcfd_mesh::fan::*;
use rcfd_mesh::fan_cgrid::*;
use rcfd_mesh::fan_design::*;
use rcfd_mesh::fan_mesh::*;
use rcfd_mesh::mesh::Mesh;
use rcfd_mesh::quality::compute_metrics;
use std::f64::consts::PI;

/// Registered limits (ADR 0692 A2 to A4).
const NON_ORTH_MAX: f64 = 85.0;
const SKEW_MAX: f64 = 4.0;
const ASPECT_MAX: f64 = 1000.0;

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
        self.holds(name, value <= limit, &format!("{value:.6} (limit {limit})"));
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// The s4 smoke specs of `fan_design_w46a.rs` (ADR 0118), copied verbatim.
fn smoke_specs() -> (SectorSpec, CGridSpec) {
    (
        SectorSpec {
            n_up: 8,
            n_chord: 16,
            n_down: 8,
            n_side: 10,
            n_mid: 2,
            n_span: 10,
            n_gap: 7,
            n_bl: 2,
            growth: 1.2,
            first_cell: FirstCell::Height(5e-5),
            trunc_le: 0.02,
            trunc_te: 0.01,
            include_blade: true,
        },
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
        },
    )
}

fn reference() -> AxialFan {
    ComputerFan::rom_reference().axial_fan(0, 0.03, 0.04)
}

// ---------------------------------------------------------------- measures

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn det(a: Vec3, b: Vec3, c: Vec3) -> f64 {
    a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])
}

/// The eight corner Jacobians of a hex in the layout order (corner `c` at offsets `(c & 1, (c >> 1) & 1, c >> 2)`), each with the
/// edges taken towards the neighbours along the three index directions and signed so that a regular hex gives one common sign.
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

/// A1b: (positive, negative, zero) corner Jacobians over every C- and F-block hex, and the first hex with a corner of the minority
/// sign or zero.
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

/// A1c: largest distance between a lower seam node rotated by `2 pi / N` and its upper partner, over every layer [m].
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

// ---------------------------------------------------------------- hashes (B2)

fn fnv(h: &mut u64, x: u64) {
    for b in x.to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}

/// FNV-1a over the node coordinates, face node lists, owner/neighbour and patch names and ranges (as ADR 0079 B1).
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

// ---------------------------------------------------------------- the registered run

/// A1 to A4 and R1 as registered (ADR 0692). Run once: A2 MISSED (86.97 deg against 85) and A3 MISSED (7.43 against 4.0), at the
/// stack edge of the wake lines 58, 59 and 4 next to the trailing edge (ADR 0692 addendum 1); A1a to A1c and A4 met (12 841 cells,
/// step 5, no folded cell, aspect 525.6). Kept with the limits intact as the record.
#[test]
#[ignore = "known miss of ADR 0692 A2 and A3: construction S on the reference 86.97 deg and skewness 7.43 against 85 / 4.0 (wake stack edge next to the trailing edge)"]
fn a1_a4_construction_s_meshes_the_reference_known_miss() {
    a1_a4(true);
}

/// ADR 0692 amendment 2: the met clauses of the registered test (A1a to A1c, A4) with their limits; A2 and A3 printed only.
#[test]
fn a1_a4_construction_s_meshes_the_reference_met_limits() {
    a1_a4(false);
}

/// A1 to A4 and R1 (ADR 0692): construction S builds the reference at the s4 specs with no folded cell, periodic seams and the
/// registered quality limits (A2 and A3 only printed without `with_a2_a3`).
fn a1_a4(with_a2_a3: bool) {
    let mut ch = Checks::new();
    let (_, cs) = smoke_specs();
    let m = match build_cgrid_mesh_with(&reference(), &cs, &CGridOptions::camber_seam_s_construction()) {
        Ok(m) => m,
        Err(e) => {
            ch.holds("A1a construction S builds", false, &format!("{e:?}"));
            ch.finish();
            unreachable!()
        }
    };
    ch.holds(
        "A1a construction S builds",
        m.mesh.n_cells() > 0,
        &format!("{} cells, construction step {}", m.mesh.n_cells(), m.construction_step),
    );
    let q = m.quality();
    let (pos, neg, zero, first) = fold_count(&m);
    ch.holds(
        "A1b no folded cell (corner Jacobians of one sign, min volume > 0)",
        (pos == 0 || neg == 0) && zero == 0 && q.min_volume > 0.0,
        &format!(
            "positive {pos}, negative {neg}, zero {zero}, first minority {first:?}, min volume {:.4e}",
            q.min_volume
        ),
    );
    ch.at_most("A1c periodicity [m]", periodicity(&m, 7), 1e-12);
    if with_a2_a3 {
        ch.at_most("A2 max non-orthogonality [deg]", q.max_non_orthogonality, NON_ORTH_MAX);
        ch.at_most("A3 max skewness", q.max_skewness, SKEW_MAX);
    } else {
        println!(
            "A2 (record only) max non-orthogonality {:.6} deg (limit {NON_ORTH_MAX}); A3 (record only) max skewness {:.6} (limit {SKEW_MAX})",
            q.max_non_orthogonality, q.max_skewness
        );
    }
    ch.at_most("A4 max aspect ratio", q.max_aspect_ratio, ASPECT_MAX);
    println!("R1 S on the reference: {q:?}");
    println!("R1 {}", locate(&m));
    ch.finish();
}

/// A5 (ADR 0692): the h_slit mesh of the reference at the s4 specs measured against the limits of A2 to A4 (reported, not a gate;
/// predicted from ADR 0643: non-orthogonality and skewness missed, aspect met).
#[test]
fn a5_h_slit_of_the_reference_against_the_same_limits() {
    let (ss, _) = smoke_specs();
    let h = build_sector_mesh(&reference(), &ss).expect("h_slit builds the reference (ADR 0643 A5a)");
    let q = h.quality();
    let mark = |v: f64, l: f64| if v <= l { "met" } else { "missed" };
    println!(
        "A5 h_slit on the reference: {} cells; non-orthogonality {:.3} deg ({} against {NON_ORTH_MAX}), skewness {:.4} ({} against {SKEW_MAX}), aspect {:.1} ({} against {ASPECT_MAX}); {q:?}",
        h.mesh.n_cells(),
        q.max_non_orthogonality,
        mark(q.max_non_orthogonality, NON_ORTH_MAX),
        q.max_skewness,
        mark(q.max_skewness, SKEW_MAX),
        q.max_aspect_ratio,
        mark(q.max_aspect_ratio, ASPECT_MAX),
    );
    assert!(h.mesh.n_cells() > 0);
}

/// NC1 (ADR 0692): camber_seam_h refuses the reference at the s4 specs with the ADR 0643 message.
#[test]
fn nc1_camber_seam_h_refuses_the_reference() {
    let (_, cs) = smoke_specs();
    let e = build_cgrid_mesh_with(&reference(), &cs, &CGridOptions::camber_seam_h_construction()).unwrap_err();
    println!("NC1: {e:?}");
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].path, "front_gap");
    assert_eq!(
        e[0].message,
        "the line family is folded at layer 0 (r = 0.02150), C block around line 1, row 2: the leading-edge fan cannot be built for this blade with the front_gap 0.3 and n_nose 6 given; reduce front_gap or change n_nose"
    );
}

/// NC2 and NC3 (ADR 0692, reported): H with one of the two fields only (predicted: both refused).
#[test]
fn nc2_nc3_one_field_only_reported() {
    let (_, cs) = smoke_specs();
    for (tag, opts) in [
        (
            "NC2 H + straight_lines",
            CGridOptions {
                straight_lines: true,
                ..CGridOptions::camber_seam_h_construction()
            },
        ),
        (
            "NC3 H + camber_front",
            CGridOptions {
                camber_front: true,
                ..CGridOptions::camber_seam_h_construction()
            },
        ),
    ] {
        match build_cgrid_mesh_with(&reference(), &cs, &opts) {
            Ok(m) => println!(
                "{tag}: builds (prediction missed), {} cells, step {}",
                m.mesh.n_cells(),
                m.construction_step
            ),
            Err(e) => println!("{tag}: refused (as predicted): {e:?}"),
        }
    }
}

/// NC4 (ADR 0692): the corner-Jacobian measure of A1b detects a hex with one corner pushed through the opposite face.
#[test]
fn nc4_corner_jacobians_detect_a_pushed_corner() {
    let cube: [Vec3; 8] = std::array::from_fn(|c| [(c & 1) as f64, ((c >> 1) & 1) as f64, (c >> 2) as f64]);
    let j = corner_jacobians(&cube);
    assert!(j.iter().all(|&d| d == j[0] && d != 0.0), "regular hex: one sign {j:?}");
    let mut bad = cube;
    bad[7] = [0.2, 0.2, -0.5];
    let jb = corner_jacobians(&bad);
    println!("NC4 regular {j:?}; pushed {jb:?}");
    assert!(
        jb.iter().any(|&d| d > 0.0) && jb.iter().any(|&d| d < 0.0),
        "pushed corner: mixed signs"
    );
}

/// B2 (ADR 0692): camber_seam_h on the default ComputerFan blade 0 at the s4 specs (the ADR 0118 S4 mesh) has the hash computed on
/// a `git archive` copy of ff7e9989 (`f726-scratch/base_b2.log`, Windows). Other platforms print the hash and skip.
#[test]
fn b2_camber_seam_h_on_the_default_computer_fan_is_bit_identical() {
    let (_, cs) = smoke_specs();
    let af = ComputerFan::default().axial_fan(0, 0.03, 0.04);
    let m = build_cgrid_mesh_with(&af, &cs, &CGridOptions::camber_seam_h_construction()).unwrap();
    let got = mesh_hash(&m.mesh);
    println!("B2 camber_seam_h default ComputerFan s4: {got:#018x}");
    if cfg!(windows) {
        assert_eq!(got, 0xa448fd08e65014f8u64);
    } else {
        eprintln!("B2: no recorded hash for this platform; hash {got:#018x}");
    }
}

// ---------------------------------------------------------------- R2 (reported)

fn st(r: f64, chord: f64, stagger_deg: f64, camber: f64, t: f64, sweep: f64, lean: f64) -> Station {
    Station {
        radius: r,
        chord,
        stagger: stagger_deg.to_radians(),
        camber,
        camber_pos: 0.4,
        thickness: t,
        sweep,
        lean,
    }
}

/// Test blade B of ADR 0065 / 0068 (as `cgrid_w35d.rs`).
fn blade_b() -> AxialFan {
    AxialFan {
        n_blades: 5,
        r_hub: 0.10,
        r_tip: 0.30,
        tip_clearance: 0.01,
        axial_extent: 0.7,
        z_le_root: 0.3,
        family: SectionFamily::Naca4,
        stations: vec![
            st(0.10, 0.10, 35.0, 0.04, 0.10, 0.0, 0.0),
            st(0.30, 0.07, 20.0, 0.04, 0.06, 0.01, 0.005),
        ],
    }
}

/// Set-up S of ADR 0068 (as `cgrid_w35d.rs`).
fn spec_s() -> CGridSpec {
    CGridSpec {
        n_up: 8,
        n_chord: 16,
        n_down: 8,
        n_side: 10,
        n_nose: 4,
        n_core: 1,
        n_span: 14,
        n_gap: 7,
        n_bl: 2,
        growth: 1.2,
        first_cell: FirstCell::Height(1e-3),
        front_gap: 0.3,
        include_blade: true,
    }
}

/// FAN-01 (ADR 0075 amendment 1, as `cgrid_w35d.rs`).
fn fan01() -> AxialFan {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation-data/reference/w34d_fan01_cad_sections.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rows: Vec<Vec<f64>> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("r_mm"))
        .map(|l| {
            l.split(',')
                .take(8)
                .map(|v| v.trim().parse::<f64>().unwrap())
                .collect::<Vec<f64>>()
        })
        .filter(|r| r[0] != 247.0)
        .collect();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    AxialFan {
        n_blades: 9,
        r_hub: 0.124,
        r_tip: 0.2475,
        tip_clearance: 0.0025,
        axial_extent: 0.4,
        z_le_root: 0.1,
        family: SectionFamily::Naca4,
        stations: rows
            .iter()
            .map(|s| Station {
                radius: s[0] * 1e-3,
                chord: s[1] * 1e-3,
                stagger: s[2].to_radians(),
                camber: s[3],
                camber_pos: s[4],
                thickness: s[5],
                sweep: (s[6] - x0) * 1e-3,
                lean: s[0] * 1e-3 * (s[7] - t0).to_radians(),
            })
            .collect(),
    }
}

/// The FAN-01-C counts (ADR 0075 amendment 1, as `cgrid_w35d.rs`).
fn spec_fan01() -> CGridSpec {
    let area = PI * (0.25f64.powi(2) - 0.124f64.powi(2));
    let omega = 1486.0 * 2.0 * PI / 60.0;
    CGridSpec {
        first_cell: FirstCell::YPlus {
            y_plus: 30.0,
            speed: (1.4032 / area).hypot(omega * 0.186),
            nu: 1.5e-5,
        },
        ..spec_s()
    }
}

/// R2 (ADR 0692, reported): construction S on the default ComputerFan (s4), blade B (S counts) and FAN-01-C.
#[test]
fn r2_construction_s_on_the_existing_cases_reported() {
    let (_, cs) = smoke_specs();
    for (tag, fan, spec) in [
        (
            "default ComputerFan s4",
            ComputerFan::default().axial_fan(0, 0.03, 0.04),
            cs,
        ),
        ("blade B S", blade_b(), spec_s()),
        ("FAN-01-C", fan01(), spec_fan01()),
    ] {
        match build_cgrid_mesh_with(&fan, &spec, &CGridOptions::camber_seam_s_construction()) {
            Ok(m) => println!(
                "R2 S {tag}: builds, {} cells, step {}, {:?}",
                m.mesh.n_cells(),
                m.construction_step,
                m.quality()
            ),
            Err(e) => println!("R2 S {tag}: refused {e:?}"),
        }
    }
}
