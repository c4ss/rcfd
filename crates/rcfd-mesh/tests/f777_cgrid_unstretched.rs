//! F-777 (ADR 0696): construction S' of the camber-seam C-grid (`CGridOptions::camber_seam_s_unstretched_construction`,
//! construction S of ADR 0692 with the first free cell of every straight line at the next stack height) on the strongly staggered
//! reference fan `ComputerFan::rom_reference()` and on a second strongly staggered blade (the reference with a per-blade deviation of
//! -1 deg stagger and +20 % chord, outer stagger 76.6 deg) at the ADR 0118 s4 specs: no folded cell within 85 deg, skewness 4.0
//! and aspect 1000; a nose-region check and a stack-growth check on the algebraic grid; a front_gap and n_nose scan inside the
//! camber_seam range; camber_seam_s unchanged in bits. Limits registered in ADR 0696 before this code ran.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the limits

use rcfd_core::vec3::Vec3;
use rcfd_mesh::fan::*;
use rcfd_mesh::fan_cgrid::*;
use rcfd_mesh::fan_design::*;
use rcfd_mesh::fan_mesh::*;
use rcfd_mesh::mesh::Mesh;
use rcfd_mesh::quality::compute_metrics;
use std::f64::consts::PI;

/// Registered limits (ADR 0696 A2 to A4, B2d to B2f).
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

/// The second blade of ADR 0696: the reference with the per-blade deviation stagger -1 deg, chord +20 % on every blade (stagger
/// 68.8 / 74.4 / 76.6 deg, chord 0.0192 / 0.0348 / 0.0432 m, the absolute thickness kept).
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

/// The algebraic grid of a construction (the fallback chain's step-5 grid): the same options without the smoothing.
fn algebraic(opts: CGridOptions) -> CGridOptions {
    CGridOptions {
        smoothing: false,
        ..opts
    }
}

// ---------------------------------------------------------------- measures (as f726_cgrid_staggered.rs)

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

/// FNV-1a over the node coordinates, face node lists, owner/neighbour and patch names and ranges (as ADR 0079 B1, ADR 0692 B2).
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

/// The remapped radii of ADR 0072 W1 (`fan_cgrid::remapped_radii`, reproduced): increments `Delta r_k min(1, Delta r_k / mean)`
/// scaled to span the blade; the tip-gap layers at the tip radius. The in-plane grid far from the stacks of layer `k` is the grid
/// built at this radius.
fn remapped(radii: &[f64], n_span: usize) -> Vec<f64> {
    let (r0, r1) = (radii[0], radii[n_span]);
    let mean = (r1 - r0) / n_span as f64;
    let q: Vec<f64> = (0..n_span)
        .map(|k| {
            let d = radii[k + 1] - radii[k];
            d * (d / mean).min(1.0)
        })
        .collect();
    let tot: f64 = q.iter().sum();
    let mut out = Vec::with_capacity(radii.len());
    let mut acc = 0.0;
    out.push(r0);
    for qk in &q {
        acc += qk;
        out.push(r0 + (r1 - r0) * acc / tot);
    }
    out[n_span] = r1;
    while out.len() < radii.len() {
        out.push(r1);
    }
    out
}

/// In-plane `(z, rho theta)` of a grid node, with `theta` unwrapped to within `pi` of `theta_near`.
fn in_plane(p: Vec3, rho: f64, theta_near: f64) -> (f64, f64) {
    let mut th = p[1].atan2(p[0]);
    th += 2.0 * PI * ((theta_near - th) / (2.0 * PI)).round();
    (p[2], rho * th)
}

/// The expected nose point `N` of S1 (ADR 0692) for the section at `rho`: `front_gap` chords from the leading edge along the
/// extended leading-edge camber line (the generator's finite-difference slope).
fn nose_point(fan: &AxialFan, rho: f64, front_gap: f64) -> (f64, f64) {
    let st = fan.station_at(rho);
    let (a, b) = (fan.camber_zs(&st, 0.0), fan.camber_zs(&st, 1e-6));
    let sig = (b.1 - a.1) / (b.0 - a.0);
    let z_n = a.0 - front_gap * st.chord / (1.0 + sig * sig).sqrt();
    (z_n, a.1 + sig * (z_n - a.0))
}

/// NR / NR2 measures: per layer `(k, dz, ds, front edge one z and s increasing)` of the nose node against `N(front_gap)`.
fn nose_region(m: &CGridMesh, fan: &AxialFan, front_gap: f64) -> Vec<(usize, f64, f64, bool)> {
    let rho = remapped(&m.radii, m.n_span);
    let (n_i, ns, nn) = (m.n_i(), m.n_side(), m.n_nose());
    let h_c = (n_i - 2 * nn) / 2;
    (0..=m.n_k)
        .map(|k| {
            let (z_n, s_n) = nose_point(fan, rho[k], front_gap);
            let th_n = s_n / rho[k];
            let pts: Vec<(f64, f64)> = (0..=2 * nn)
                .map(|b| in_plane(m.grid[m.c_node(h_c + b, ns, k)], rho[k], th_n))
                .collect();
            let p = in_plane(m.grid[m.c_node(n_i / 2, ns, k)], rho[k], th_n);
            let ordered = pts.iter().all(|q| q.0 == pts[0].0) && pts.windows(2).all(|w| w[1].1 > w[0].1);
            (k, p.0 - z_n, p.1 - s_n, ordered)
        })
        .collect()
}

/// A6 measure: over every C line with a stack (`nb >= 1`) of the layers written without the remap blend (`k = 0`, `k = n_span`,
/// the tip-gap layers), the largest `|ratio / growth - 1|` of the first free cell to the last stack cell, and where.
fn stack_growth(m: &CGridMesh, growth: f64) -> (f64, usize, usize, usize) {
    let rho = remapped(&m.radii, m.n_span);
    let layers: Vec<usize> = std::iter::once(0).chain(m.n_span..=m.n_k).collect();
    let mut worst = (0.0f64, 0, 0, 0);
    for &k in &layers {
        for i in 0..=m.n_i() {
            let nb = m.stack_cells[k][i];
            if nb == 0 {
                continue;
            }
            let p = |j: usize| m.grid[m.c_node(i, j, k)];
            let th0 = p(nb)[1].atan2(p(nb)[0]);
            let q: Vec<(f64, f64)> = [nb - 1, nb, nb + 1]
                .iter()
                .map(|&j| in_plane(p(j), rho[k], th0))
                .collect();
            let l = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0).hypot(b.1 - a.1);
            let dev = (l(q[1], q[2]) / l(q[0], q[1]) / growth - 1.0).abs();
            if !(dev <= worst.0) {
                worst = (dev, k, i, nb);
            }
        }
    }
    worst
}

/// A1 to A4 (B2a to B2f for the second blade) and R1 for construction S' on `fan` at s4.
fn a1_a4(tag: &str, fan: &AxialFan) {
    let mut ch = Checks::new();
    let m = match build_cgrid_mesh_with(fan, &s4(), &s_prime()) {
        Ok(m) => m,
        Err(e) => {
            ch.holds(&format!("{tag} A1a S' builds"), false, &format!("{e:?}"));
            ch.finish();
            unreachable!()
        }
    };
    ch.holds(
        &format!("{tag} A1a S' builds"),
        m.mesh.n_cells() > 0,
        &format!("{} cells, construction step {}", m.mesh.n_cells(), m.construction_step),
    );
    let q = m.quality();
    let (pos, neg, zero, first) = fold_count(&m);
    ch.holds(
        &format!("{tag} A1b no folded cell (corner Jacobians of one sign, min volume > 0)"),
        (pos == 0 || neg == 0) && zero == 0 && q.min_volume > 0.0,
        &format!(
            "positive {pos}, negative {neg}, zero {zero}, first minority {first:?}, min volume {:.4e}",
            q.min_volume
        ),
    );
    ch.at_most(&format!("{tag} A1c periodicity [m]"), periodicity(&m, 7), 1e-12);
    ch.at_most(
        &format!("{tag} A2 max non-orthogonality [deg]"),
        q.max_non_orthogonality,
        NON_ORTH_MAX,
    );
    ch.at_most(&format!("{tag} A3 max skewness"), q.max_skewness, SKEW_MAX);
    ch.at_most(&format!("{tag} A4 max aspect ratio"), q.max_aspect_ratio, ASPECT_MAX);
    println!("{tag} R1 S': {q:?}");
    println!("{tag} R1 {}", locate(&m));
    ch.finish();
}

// ---------------------------------------------------------------- the registered checks

/// A1a to A4 (ADR 0696, c3): S' on the reference at s4 (the numbers of ADR 0692 MU4 already seen: 84.688 deg, 3.240, 525.6).
#[test]
fn a1_a4_construction_s_prime_meshes_the_reference() {
    a1_a4("REF", &reference());
}

/// B2a to B2f (ADR 0696, c4): S' on the second blade (outer stagger 76.6 deg) at s4.
#[test]
fn b2_construction_s_prime_meshes_the_second_blade() {
    let st = second_blade().stations;
    println!(
        "B2 stations: stagger {:?} deg, chord {:?} m",
        st.iter().map(|s| s.stagger.to_degrees()).collect::<Vec<_>>(),
        st.iter().map(|s| s.chord).collect::<Vec<_>>()
    );
    assert!(st.last().unwrap().stagger.to_degrees() >= 75.0);
    a1_a4("B2", &second_blade());
}

/// NR and NR2 (ADR 0696, c8): on the algebraic grid of S' of the reference, the nose node of every layer is the point `front_gap`
/// chords along the extended leading-edge camber line of the layer's (remapped) section, and the front edge is one `z` with `s`
/// increasing across the nose.
#[test]
fn nr_the_nose_target_lies_on_the_extended_camber_line() {
    let fan = reference();
    let m = build_cgrid_mesh_with(&fan, &s4(), &algebraic(s_prime()))
        .expect("the algebraic grid of S' builds the reference");
    let mut ch = Checks::new();
    let res = nose_region(&m, &fan, 0.3);
    let dz = res.iter().map(|r| r.1.abs()).fold(0.0f64, f64::max);
    let ds = res.iter().map(|r| r.2.abs()).fold(0.0f64, f64::max);
    println!("NR per layer (k, dz, ds, ordered): {res:?}");
    ch.at_most("NR max |dz| [m]", dz, 1e-12);
    ch.at_most("NR max |ds| [m]", ds, 1e-9);
    ch.holds(
        "NR2 front edge one z, s increasing across the nose",
        res.iter().all(|r| r.3),
        &format!("{} of {} layers", res.iter().filter(|r| r.3).count(), res.len()),
    );
    ch.finish();
}

/// NC-NR (ADR 0696): the NR measure sees a moved nose (S' at front_gap 0.6 measured against the point of front_gap 0.3).
#[test]
fn nc_nr_a_moved_nose_is_seen() {
    let fan = reference();
    let spec = CGridSpec { front_gap: 0.6, ..s4() };
    let m = build_cgrid_mesh_with(&fan, &spec, &algebraic(s_prime())).expect("builds at front_gap 0.6");
    let res = nose_region(&m, &fan, 0.3);
    let worst = res.iter().map(|r| r.1.abs().max(r.2.abs())).fold(0.0f64, f64::max);
    let seen = res.iter().any(|r| r.1.abs() > 1e-6 || r.2.abs() > 1e-6);
    println!("NC-NR largest |dz| or |ds| {worst:.4e} m; seen {seen}");
    assert!(seen);
}

/// A6 (ADR 0696, c2): on the algebraic grid of S' of the reference, the first free cell of every line with a stack is the next
/// stack height (ratio to the last stack cell = growth) in the layers written without the remap blend.
#[test]
fn a6_the_first_free_cell_continues_the_stack() {
    let m = build_cgrid_mesh_with(&reference(), &s4(), &algebraic(s_prime())).expect("the algebraic grid of S' builds");
    let (dev, k, i, nb) = stack_growth(&m, 1.2);
    let mut ch = Checks::new();
    ch.at_most(
        &format!("A6 max |ratio / growth - 1| (layer {k}, line {i}, nb {nb})"),
        dev,
        1e-9,
    );
    ch.finish();
}

/// NC-A6 (ADR 0696): the A6 measure on the algebraic grid of S (the stretch of ADR 0692) finds a line off by more than 1 %.
#[test]
fn nc_a6_the_stretch_of_s_is_seen() {
    let m = build_cgrid_mesh_with(
        &reference(),
        &s4(),
        &algebraic(CGridOptions::camber_seam_s_construction()),
    )
    .expect("the algebraic grid of S builds");
    let (dev, k, i, nb) = stack_growth(&m, 1.2);
    println!("NC-A6 S: max |ratio / growth - 1| {dev:.4} (layer {k}, line {i}, nb {nb})");
    assert!(dev > 0.01);
}

/// NC1' (ADR 0696): camber_seam_h refuses the second blade at s4 (ADR 0692 mechanism M-a, pencil in ADR 0696).
#[test]
fn nc1_camber_seam_h_refuses_the_second_blade() {
    let r = build_cgrid_mesh_with(&second_blade(), &s4(), &CGridOptions::camber_seam_h_construction());
    match r {
        Ok(m) => panic!("camber_seam_h builds the second blade ({} cells)", m.mesh.n_cells()),
        Err(e) => println!("NC1' refused: {e:?}"),
    }
}

/// NC2' (ADR 0696, reported): S (with the first-cell stretch) on the second blade.
#[test]
fn nc2_construction_s_on_the_second_blade_reported() {
    match build_cgrid_mesh_with(&second_blade(), &s4(), &CGridOptions::camber_seam_s_construction()) {
        Ok(m) => {
            let q = m.quality();
            println!(
                "NC2' S on B2: {} cells, step {}, non-orthogonality {:.3} deg, skewness {:.4}, aspect {:.1}; {}",
                m.mesh.n_cells(),
                m.construction_step,
                q.max_non_orthogonality,
                q.max_skewness,
                q.max_aspect_ratio,
                locate(&m)
            );
        }
        Err(e) => println!("NC2' S on B2 refused: {e:?}"),
    }
}

/// BS (ADR 0696, c7): construction S of ADR 0692 on the reference at s4 has the hash computed on a `git archive` copy of 79b70068
/// (`f777-scratch/base_bs.log`, Windows). Other platforms print the hash and skip.
#[test]
fn bs_construction_s_is_bit_identical() {
    let m = build_cgrid_mesh_with(&reference(), &s4(), &CGridOptions::camber_seam_s_construction()).unwrap();
    let got = mesh_hash(&m.mesh);
    println!("BS camber_seam_s reference s4: {got:#018x}");
    if cfg!(windows) {
        assert_eq!(got, BS_HASH);
    } else {
        eprintln!("BS: no recorded hash for this platform; hash {got:#018x}");
    }
}

/// The hash of BS on the base copy (filled from `f777-scratch/base_bs.log` before the registered run).
const BS_HASH: u64 = 0x0105a2fcca924d23;

/// The registered scan of ADR 0696 (SC): per blade, `front_gap` x `n_nose`, the other s4 values.
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

/// SC0 (ADR 0696): every scan combination is inside the camber_seam range.
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

/// SC (ADR 0696, c5): S' builds at least 9 of the 12 scan combinations of each blade fold-free and periodic (A1a to A1c); quality
/// reported. About two minutes in release (24 builds on three threads): a heavy record, run once; the s4 point of the scan is
/// covered by the fast tests above.
#[test]
#[ignore = "heavy record of ADR 0696 SC (24 builds): cargo test -p rcfd-mesh --release --test f777_cgrid_unstretched sc_ -- --ignored --nocapture"]
fn sc_the_scan_builds_the_registered_fraction() {
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
                let line = match build_cgrid_mesh_with(fan, spec, &s_prime()) {
                    Ok(m) => {
                        let q = m.quality();
                        let (pos, neg, zero, _) = fold_count(&m);
                        let per = periodicity(&m, 7);
                        let built = m.mesh.n_cells() > 0
                            && (pos == 0 || neg == 0)
                            && zero == 0
                            && q.min_volume > 0.0
                            && per <= 1e-12;
                        let within = q.max_non_orthogonality <= NON_ORTH_MAX
                            && q.max_skewness <= SKEW_MAX
                            && q.max_aspect_ratio <= ASPECT_MAX;
                        (
                            n,
                            label.clone(),
                            built,
                            within,
                            format!(
                                "step {}, {} cells, folds {} / {} / {}, periodicity {per:.2e}, {:.3} deg, {:.4}, {:.1}",
                                m.construction_step,
                                m.mesh.n_cells(),
                                pos,
                                neg,
                                zero,
                                q.max_non_orthogonality,
                                q.max_skewness,
                                q.max_aspect_ratio
                            ),
                        )
                    }
                    Err(e) => (n, label.clone(), false, false, format!("refused: {e:?}")),
                };
                results.lock().unwrap().push(line);
            });
        }
    });
    let mut res = results.into_inner().unwrap();
    res.sort_by_key(|r| r.0);
    for r in &res {
        println!("SC {}: built {}, within 85 / 4.0 / 1000 {}; {}", r.1, r.2, r.3, r.4);
    }
    let mut ch = Checks::new();
    for name in ["REF", "B2"] {
        let mine: Vec<_> = res.iter().filter(|r| r.1.starts_with(&format!("{name} "))).collect();
        let built = mine.iter().filter(|r| r.2).count();
        let within = mine.iter().filter(|r| r.3).count();
        println!(
            "SC {name}: within the quality limits (reported) {within} of {}",
            mine.len()
        );
        ch.holds(
            &format!("SC {name} builds fold-free and periodic"),
            mine.len() == 12 && built >= 9,
            &format!("{built} of {} (limit 9 of 12)", mine.len()),
        );
    }
    ch.finish();
}
