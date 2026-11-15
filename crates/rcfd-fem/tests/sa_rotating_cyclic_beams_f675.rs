//! F-675 (ADR 0598): the rotating paths on a cyclic sector of beams. Checks CB1, CB2, MA, MB, MC, W, W0, SM, B3 and the
//! negative controls NC1, NC2 of the registration (B1 and B2 are the existing tests and `baseline_hashes`). The wheel sector
//! and the ring are the builders of `sa_cyclic_beams_f672.rs` (ADR 0595), copied.

use rcfd_fem::beam::{element_coriolis, SOLID_ROUTINES};
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::{modal_analysis_harmonic, rotation_matrix, Mat3};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::rotating::{
    coriolis_matrix, coriolis_matrix_with_shells, HarmonicSpeedModes, HarmonicWhirlSpeedModes, RotatingOptions,
    RotatingPart, RotatingSector, SpeedModes, WhirlSpeedModes,
};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const N: usize = 6;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
/// Ring elements per sector.
const NR: usize = 4;
const EPS: f64 = f64::EPSILON;
/// The loaded sector nodes of ADR 0595 (their node sets are kept, the builders being copies).
const LOADED: [usize; 5] = [0, 1, 2, 3, 5];

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn ea() -> Vec3 {
    let l = 5.0f64.sqrt();
    [2.0 / l, -1.0 / l, 0.0]
}

fn eb() -> Vec3 {
    cross(axis(), ea())
}

fn comb(a: f64, x: Vec3, b: f64, y: Vec3) -> Vec3 {
    [0, 1, 2].map(|k| a * x[k] + b * y[k])
}

/// `P(phi, r, z) = O + r (cos phi e_a + sin phi e_b) + z n`.
fn p(phi: f64, r: f64, z: f64) -> Vec3 {
    let (n, a, b) = (axis(), ea(), eb());
    let (s, c) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (c * a[k] + s * b[k]) + z * n[k])
}

fn alpha() -> f64 {
    2.0 * PI / N as f64
}

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn ring_section() -> BeamSection {
    BeamSection::new(1.8e-3, 1.35e-7, 5.4e-7, 3.7e-7, 5.0 / 6.0, 5.0 / 6.0).unwrap()
}

fn spoke_section() -> BeamSection {
    BeamSection::new(1.2e-3, 1.0e-7, 2.5e-7, 2.4e-7, 0.85, 0.75).unwrap()
}

fn v_ring() -> Vec3 {
    comb(0.8, axis(), 0.6, ea())
}

fn v_spoke() -> Vec3 {
    comb(0.6, eb(), 0.8, axis())
}

fn rot(j: usize) -> Mat3 {
    rotation_matrix(axis(), 2.0 * PI * j as f64 / N as f64)
}

fn apply(r: &Mat3, v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| r[i][0] * v[0] + r[i][1] * v[1] + r[i][2] * v[2])
}

fn modal_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes);
    o.tolerance = 1e-12;
    o.residual_test = ResidualTest::Backward;
    o
}

fn beam_options() -> CyclicOptions {
    CyclicOptions::new().shells(true).beams(true)
}

fn clamp(m: &mut SolidModel) {
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("hub", comp, 0.0)).unwrap();
    }
}

/// The wheel sector W of ADR 0595 with the cyclic constraint of `options`.
fn sector(options: CyclicOptions) -> SolidModel {
    let a = alpha();
    let mut nodes: Vec<Vec3> = (0..=NR).map(|i| p(i as f64 * a / NR as f64, 1.0, 0.0)).collect();
    nodes.push(p(a / 2.0, 0.65, 0.125));
    nodes.push(p(a / 2.0, 0.3, 0.25));
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2, 2, 3, 3, 4])
        .unwrap();
    mesh.add_block(ElementKind::Beam2, vec![6, 5, 5, 2]).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(ring_section(), v_ring()).unwrap())
        .unwrap();
    mesh.set_block_property(1, BlockProperty::beam(spoke_section(), v_spoke()).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..6).collect()).unwrap();
    mesh.add_node_set("low", vec![0]).unwrap();
    mesh.add_node_set("high", vec![NR]).unwrap();
    mesh.add_node_set("hub", vec![6]).unwrap();
    for q in LOADED {
        mesh.add_node_set(&format!("p{q}"), vec![q]).unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with("low", "high", axis(), ORIGIN, N, options))
        .unwrap();
    m
}

/// The ring node of sector node `q` in sector `j`.
fn ring_node(j: usize, q: usize) -> usize {
    match q {
        0..=NR => (NR * j + q) % (NR * N),
        5 => NR * N + j,
        6 => NR * N + N + j,
        _ => unreachable!("sector node"),
    }
}

/// The full ring of beams; `rotated`: orientations `R^j v` in sector `j` (else `v` everywhere: control NC-O).
fn ring(rotated: bool) -> SolidModel {
    let a = alpha();
    let mut nodes: Vec<Vec3> = (0..NR * N).map(|i| p(i as f64 * a / NR as f64, 1.0, 0.0)).collect();
    for j in 0..N {
        nodes.push(p((j as f64 + 0.5) * a, 0.65, 0.125));
    }
    for j in 0..N {
        nodes.push(p((j as f64 + 0.5) * a, 0.3, 0.25));
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    for j in 0..N {
        let r = if rotated { rot(j) } else { rot(0) };
        let conn: Vec<usize> = (0..NR).flat_map(|i| [ring_node(j, i), ring_node(j, i + 1)]).collect();
        let b = mesh.add_block(ElementKind::Beam2, conn).unwrap();
        mesh.set_block_property(b, BlockProperty::beam(ring_section(), apply(&r, v_ring())).unwrap())
            .unwrap();
        let conn = vec![ring_node(j, 6), ring_node(j, 5), ring_node(j, 5), ring_node(j, 2)];
        let b = mesh.add_block(ElementKind::Beam2, conn).unwrap();
        mesh.set_block_property(b, BlockProperty::beam(spoke_section(), apply(&r, v_spoke())).unwrap())
            .unwrap();
    }
    mesh.add_element_set("all", (0..6 * N).collect()).unwrap();
    mesh.add_node_set("hub", (0..N).map(|j| ring_node(j, 6)).collect())
        .unwrap();
    for j in 0..N {
        for q in LOADED {
            let rn = ring_node(j, q);
            mesh.add_node_set(&format!("p{rn}"), vec![rn]).unwrap();
        }
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m
}

fn opts(m: usize) -> RotatingOptions {
    RotatingOptions::new(m).modal(modal_options(m))
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

/// `c_ij = -c_ji` and `c_ii = 0` bit for bit, and at least one entry non-zero.
fn skew_bits(a: &CsrMatrix) -> (bool, bool) {
    let get = |r: usize, c: usize| -> Option<f64> {
        (a.row_ptr[r]..a.row_ptr[r + 1])
            .find(|&p| a.col[p] == c)
            .map(|p| a.val[p])
    };
    let mut skew = true;
    let mut nonzero = false;
    for r in 0..a.n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            let (c, v) = (a.col[p], a.val[p]);
            nonzero |= v != 0.0;
            if r == c {
                skew &= v == 0.0;
            } else {
                skew &= get(c, r).map(|w| w.to_bits() == (-v).to_bits()).unwrap_or(v == 0.0);
            }
        }
    }
    (skew, nonzero)
}

// ---------------------------------------------------------------------------------------------- CB1, CB2 (one element)

#[test]
fn cb1_cb2_beam_coriolis_rigid_fields() {
    let (x0, x1) = ([0.2, -0.1, 0.4], [0.5, 0.3, 0.1]);
    let s = ring_section();
    let a = axis();
    let c = element_coriolis(&[x0, x1], &s, v_ring(), &mat(), a).unwrap();
    let cv = c.values();
    let mid = [0, 1, 2].map(|k| 0.5 * (x0[k] + x1[k]));
    let rigid = |v: Vec3, th: Vec3| -> Vec<f64> {
        let mut q = Vec::with_capacity(12);
        for x in [x0, x1] {
            let t = cross(th, sub(x, mid));
            q.extend([v[0] + t[0], v[1] + t[1], v[2] + t[2], th[0], th[1], th[2]]);
        }
        q
    };
    let form = |q: &[f64], r: &[f64]| -> f64 {
        (0..12)
            .map(|i| q[i] * (0..12).map(|j| cv[i * 12 + j] * r[j]).sum::<f64>())
            .sum()
    };
    let (v1, v2) = ([0.3, -0.7, 0.5], [-0.4, 0.2, 0.9]);
    let (t1, t2) = ([0.6, 0.1, -0.8], [-0.2, 0.9, 0.3]);
    let z = [0.0; 3];
    let l = norm(sub(x1, x0));
    let ma = RHO * s.area() * l;
    // The frame of ADR 0417: e_1 along the axis, e_3 = e_1 x v / |e_1 x v|, e_2 = e_3 x e_1.
    let e1 = [0, 1, 2].map(|k| (x1[k] - x0[k]) / l);
    let e3u = cross(e1, v_ring());
    let e3 = [0, 1, 2].map(|k| e3u[k] / norm(e3u));
    let e2 = cross(e3, e1);
    // J_c a = rho (A L^3 / 12 e_1 (e_1 . a) + L I_z e_2 (e_2 . a) + L I_y e_3 (e_3 . a)).
    let (c1, c2, c3) = (
        RHO * s.area() * l * l * l / 12.0 * dot(e1, a),
        RHO * l * s.i_z() * dot(e2, a),
        RHO * l * s.i_y() * dot(e3, a),
    );
    let ja = [0, 1, 2].map(|k| c1 * e1[k] + c2 * e2[k] + c3 * e3[k]);
    let tt = form(&rigid(v1, z), &rigid(v2, z));
    let tt_ref = ma * dot(v1, cross(a, v2));
    let rr = form(&rigid(z, t1), &rigid(z, t2));
    let rr_ref = dot(cross(t2, t1), ja);
    let rr_section = dot(cross(t2, t1), [0, 1, 2].map(|k| c2 * e2[k] + c3 * e3[k]));
    let mx = form(&rigid(v1, z), &rigid(z, t2));
    let mx_rel = mx.abs() / (ma * l * norm(v1) * norm(t2));
    let e_tt = (tt / tt_ref - 1.0).abs();
    let e_rr = (rr / rr_ref - 1.0).abs();
    println!(
        "CB1 translations: {tt:.12e} against {tt_ref:.12e}: {e_tt:.3e} (limit <= 1e-13) {}",
        met(e_tt <= 1e-13)
    );
    println!(
        "CB1 rotations: {rr:.12e} against {rr_ref:.12e} (section part {rr_section:.6e}): {e_rr:.3e} (limit <= 1e-13) {}",
        met(e_rr <= 1e-13)
    );
    println!(
        "CB1 mixed: {mx:.3e}, relative {mx_rel:.3e} (limit <= 1e-13) {}",
        met(mx_rel <= 1e-13)
    );
    let mut skew = true;
    let mut nonzero = false;
    for i in 0..12 {
        skew &= cv[i * 12 + i] == 0.0;
        for j in 0..12 {
            if i != j {
                skew &= cv[i * 12 + j].to_bits() == (-cv[j * 12 + i]).to_bits();
            }
            nonzero |= cv[i * 12 + j] != 0.0;
        }
    }
    println!(
        "CB2 element: skew bit for bit {skew}, non-zero {nonzero} {}",
        met(skew && nonzero)
    );
    assert!(e_tt <= 1e-13 && e_rr <= 1e-13 && mx_rel <= 1e-13 && skew && nonzero);
}

// ---------------------------------------------------------------------------------------------- MA, MB, MC, W, W0, SM

/// The list protocol of ADR 0561 D4 / ADR 0598 D4. `sector`: per harmonic the entries; `ring`: the ring's entries; `sqrt`: the
/// entries are eigenvalues. Returns (sector count, ring count, ring complete, per-harmonic max relative difference).
fn compare(sector: &[Vec<f64>], ring: &[f64], sqrt: bool) -> (usize, usize, bool, Vec<f64>) {
    let c = (1.0 - 1e-6)
        * sector
            .iter()
            .map(|v| v.iter().copied().fold(f64::NEG_INFINITY, f64::max))
            .fold(f64::INFINITY, f64::min);
    let mut sec: Vec<(f64, usize)> = sector
        .iter()
        .enumerate()
        .flat_map(|(k, v)| v.iter().filter(|&&x| x < c).map(move |&x| (x, k)))
        .collect();
    sec.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut rv: Vec<f64> = ring.iter().copied().filter(|&x| x < c).collect();
    rv.sort_by(|a, b| a.total_cmp(b));
    let complete = ring.iter().copied().fold(f64::NEG_INFINITY, f64::max) >= c;
    let f = |x: f64| if sqrt { x.sqrt() } else { x };
    let mut per = vec![0.0f64; sector.len()];
    for (&(s, k), &r) in sec.iter().zip(&rv) {
        per[k] = per[k].max((f(s) / f(r) - 1.0).abs());
    }
    (sec.len(), rv.len(), complete, per)
}

fn sector_eigenvalues(sec: &RotatingSector, k: usize, w: f64) -> Vec<f64> {
    match sec.modes(k, w).unwrap() {
        HarmonicSpeedModes::Modes(h) => h.eigenvalues.clone(),
        HarmonicSpeedModes::LossOfStiffness(l) => panic!("set-up failure: loss of stiffness {l:?}"),
    }
}

fn ring_eigenvalues(part: &RotatingPart, w: f64) -> Vec<f64> {
    match part.modes(w).unwrap() {
        SpeedModes::Modes(r) => r.eigenvalues,
        SpeedModes::LossOfStiffness(l) => panic!("set-up failure: loss of stiffness {l:?}"),
    }
}

/// The omegas of the sector's whirl modes of `k`, the reduced unknowns and the largest frequency of the problem.
fn sector_whirl(sec: &RotatingSector, k: usize, w: f64) -> (Vec<f64>, usize, f64) {
    match sec.whirl_modes(k, w).unwrap() {
        HarmonicWhirlSpeedModes::Modes { modes, .. } => (
            modes.modes.iter().map(|m| m.omega).collect(),
            modes.unknowns,
            modes.frequencies_all.iter().copied().fold(0.0, f64::max),
        ),
        HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("set-up failure: loss of stiffness {l:?}"),
    }
}

fn ring_whirl(part: &RotatingPart, w: f64) -> (Vec<f64>, usize, f64) {
    match part.whirl_modes(w).unwrap() {
        WhirlSpeedModes::Modes(m) => (
            m.modes.iter().map(|x| x.omega).collect(),
            m.unknowns,
            m.frequencies_all.iter().copied().fold(0.0, f64::max),
        ),
        WhirlSpeedModes::LossOfStiffness(l) => panic!("set-up failure: loss of stiffness {l:?}"),
    }
}

fn fmt_list(v: &[f64]) -> Vec<String> {
    v.iter().map(|d| format!("{d:.3e}")).collect()
}

#[test]
#[ignore = "run-1 record of ADR 0598 (amendment 1): MA, MB, MC, W, W0 and SM met (the registered results); CB2 of the assembled matrices reported false because the measure compared +0 with -0 by their bits (2 pairs of zeros in W, 10 in the ring, no other difference); continued in cb2_assembled_after_amendment_1"]
fn ma_mb_mc_w_rotating_sector_of_beams_against_the_full_ring_run1_record() {
    let m = sector(beam_options());
    let full = ring(true);
    let omega1 = (0..=N / 2)
        .map(|k| {
            modal_analysis_harmonic(&m, &[], k, &modal_options(4))
                .unwrap()
                .eigenvalues[0]
        })
        .fold(f64::INFINITY, f64::min)
        .sqrt();
    let speeds = [0.2 * omega1, 0.4 * omega1];
    println!("omega_1 = {omega1:.6} rad/s; speeds {speeds:?}");
    let mut fails: Vec<String> = Vec::new();
    let sets: [(&str, RotatingOptions, RotatingOptions); 3] = [
        ("MA (both effects)", opts(4), opts(30)),
        (
            "MB (pre-stressed)",
            opts(4).spin_softening(false),
            opts(30).spin_softening(false),
        ),
        (
            "MC (spin-softened)",
            opts(4).stress_stiffening(false),
            opts(30).stress_stiffening(false),
        ),
    ];
    for (name, so, ro) in &sets {
        let sec = RotatingSector::new(&m, so).unwrap();
        let part = RotatingPart::new(&full, axis(), ORIGIN, &ro.clone().beams(true)).unwrap();
        for &w in &speeds {
            let lists: Vec<Vec<f64>> = (0..=N / 2).map(|k| sector_eigenvalues(&sec, k, w)).collect();
            let rl = ring_eigenvalues(&part, w);
            let (ns, nr, complete, per) = compare(&lists, &rl, true);
            let ok = ns == nr && complete && per.iter().all(|&d| d <= 1e-8);
            println!(
                "{name} Omega {w:.4}: counts {ns} / {nr}, ring complete {complete}, per harmonic {:?} (limit <= 1e-8) {}",
                fmt_list(&per),
                met(ok)
            );
            println!(
                "    sector frequencies (Hz) k = 0..3: {:?}",
                lists
                    .iter()
                    .map(|v| v
                        .iter()
                        .map(|l| format!("{:.4}", l.sqrt() / (2.0 * PI)))
                        .collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            );
            if !ok {
                fails.push(format!("{name} {w}: {ns} {nr} {complete} {per:?}"));
            }
        }
    }
    // W: the whirl modes, Coriolis on.
    let sec_w = RotatingSector::new(&m, &opts(4).coriolis(true)).unwrap();
    let part_w = RotatingPart::new(&full, axis(), ORIGIN, &opts(30).coriolis(true).beams(true)).unwrap();
    for &w in &speeds {
        let mut lists = Vec::new();
        let mut floor: f64 = 0.0;
        for k in 0..=N / 2 {
            let (om, nq, wmax) = sector_whirl(&sec_w, k, w);
            floor = floor.max(10.0 * nq as f64 * EPS * wmax / om[0]);
            lists.push(om);
        }
        let (rl, nq, wmax) = ring_whirl(&part_w, w);
        floor = floor.max(10.0 * nq as f64 * EPS * wmax / rl[0]);
        let (ns, nr, complete, per) = compare(&lists, &rl, false);
        let ok = ns == nr && complete && per.iter().all(|&d| d <= 1e-8);
        println!(
            "W (whirl) Omega {w:.4}: counts {ns} / {nr}, ring complete {complete}, per harmonic {:?} (limit <= 1e-8) {}; \
             ring omega_max {wmax:.4e} rad/s, n_q {nq}, largest 10 n_q eps omega_max / omega_lo {floor:.3e}",
            fmt_list(&per),
            met(ok)
        );
        println!(
            "    sector whirl omegas (rad/s) k = 0..3: {:?}",
            lists
                .iter()
                .map(|v| v.iter().map(|x| format!("{x:.4}")).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        );
        if !ok {
            fails.push(format!("W {w}: {ns} {nr} {complete} {per:?}"));
        }
    }
    // W0: the Coriolis term moves the k = 1 frequencies at the top speed.
    let top = speeds[1];
    let (ow, _, _) = sector_whirl(&sec_w, 1, top);
    let sec_a = RotatingSector::new(&m, &opts(4)).unwrap();
    let la: Vec<f64> = sector_eigenvalues(&sec_a, 1, top).iter().map(|l| l.sqrt()).collect();
    let top_w = ow.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let top_a = la.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let c1 = (1.0 - 1e-6) * top_w.min(top_a);
    let mut ws: Vec<f64> = ow.iter().copied().filter(|&x| x < c1).collect();
    let mut wa: Vec<f64> = la.iter().copied().filter(|&x| x < c1).collect();
    ws.sort_by(|a, b| a.total_cmp(b));
    wa.sort_by(|a, b| a.total_cmp(b));
    let w0 = ws
        .iter()
        .zip(&wa)
        .map(|(x, y)| (x / y - 1.0).abs())
        .fold(0.0f64, f64::max);
    println!(
        "W0 Coriolis effect, k = 1, top speed: {w0:.4e} (limit >= 1e-4) {}; whirl {ws:?}, set A {wa:?}",
        met(w0 >= 1e-4)
    );
    if w0 < 1e-4 {
        fails.push(format!("W0 {w0:e}"));
    }
    // CB2 on the assembled matrices.
    for (name, c) in [
        ("sector", sec_w.part().coriolis().unwrap()),
        ("ring", part_w.coriolis().unwrap()),
    ] {
        let (skew, nonzero) = skew_bits(&c);
        println!(
            "CB2 {name}: skew bit for bit {skew}, non-zero {nonzero} {}",
            met(skew && nonzero)
        );
        if !(skew && nonzero) {
            fails.push(format!("CB2 {name}"));
        }
    }
    // SM: smoke of the other paths on the same Coriolis matrix.
    let mut ok = true;
    let mut detail = String::new();
    for k in 0..=N / 2 {
        match sec_w.split_modes(k, top) {
            Ok(r) => {
                ok &= !r.modes.modes.is_empty() && r.modes.modes.iter().all(|x| x.omega.is_finite() && x.omega > 0.0)
            }
            Err(e) => {
                ok = false;
                detail.push_str(&format!("split_modes({k}): {e:?}; "));
            }
        }
        match sec_w.whirl_state(k, top) {
            Ok(r) => ok &= !r.state.modes.is_empty() && r.state.modes.iter().all(|x| x.omega.is_finite()),
            Err(e) => {
                ok = false;
                detail.push_str(&format!("whirl_state({k}): {e:?}; "));
            }
        }
    }
    match sec_w.whirl_campbell(&[0, 1, 2, 3], &speeds) {
        Ok(c) => ok &= c.families.len() == 4,
        Err(e) => {
            ok = false;
            detail.push_str(&format!("whirl_campbell: {e:?}; "));
        }
    }
    match sec_w.campbell(&[0, 1, 2, 3], &speeds) {
        Ok(c) => ok &= c.families.len() == 4,
        Err(e) => {
            ok = false;
            detail.push_str(&format!("campbell: {e:?}"));
        }
    }
    println!(
        "SM split_modes, whirl_state, whirl_campbell, campbell: Ok and finite {ok} {} {detail}",
        met(ok)
    );
    if !ok {
        fails.push(format!("SM {detail}"));
    }
    assert!(fails.is_empty(), "{fails:?}");
}

// ---------------------------------------------------------------------------------------------- CB2 (amendment 1)

/// ADR 0598 amendment 1: `c_ij == -c_ji` as values (the two zeros equal), `c_ii == 0`, a non-zero entry; and the number of
/// pairs whose bits differ, each of which must be a pair of zeros. Returns (skew, non-zero, bit pairs, all of them zeros).
fn skew_values(a: &CsrMatrix) -> (bool, bool, usize, bool) {
    let get = |r: usize, c: usize| -> Option<f64> {
        (a.row_ptr[r]..a.row_ptr[r + 1])
            .find(|&p| a.col[p] == c)
            .map(|p| a.val[p])
    };
    let (mut skew, mut nonzero, mut pairs, mut zeros) = (true, false, 0, true);
    for r in 0..a.n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            let (c, v) = (a.col[p], a.val[p]);
            nonzero |= v != 0.0;
            if r == c {
                skew &= v == 0.0;
                continue;
            }
            let w = get(c, r).unwrap_or(0.0);
            skew &= w == -v;
            if w.to_bits() != (-v).to_bits() {
                pairs += 1;
                zeros &= v == 0.0 && w == 0.0;
            }
        }
    }
    (skew, nonzero, pairs, zeros)
}

#[test]
fn cb2_assembled_after_amendment_1() {
    let m = sector(beam_options());
    let full = ring(true);
    let sec_w = RotatingSector::new(&m, &opts(4).coriolis(true)).unwrap();
    let part_w = RotatingPart::new(&full, axis(), ORIGIN, &opts(30).coriolis(true).beams(true)).unwrap();
    let mut all = true;
    for (name, c) in [
        ("sector", sec_w.part().coriolis().unwrap()),
        ("ring", part_w.coriolis().unwrap()),
    ] {
        let (skew, nonzero, pairs, zeros) = skew_values(&c);
        let ok = skew && nonzero && zeros;
        println!(
            "CB2 (amended) {name}: c_ij == -c_ji and c_ii == 0 {skew}, non-zero {nonzero}, pairs differing in bits {pairs} (all              pairs of zeros {zeros}) {}",
            met(ok)
        );
        all &= ok;
    }
    assert!(all);
}

// ---------------------------------------------------------------------------------------------- B3

fn same_bits(a: &CsrMatrix, b: &CsrMatrix) -> bool {
    a.row_ptr == b.row_ptr && a.col == b.col && a.val.iter().zip(&b.val).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn whirl_bits(p: &RotatingPart, w: f64) -> Vec<u64> {
    match p.whirl_modes(w).unwrap() {
        WhirlSpeedModes::Modes(m) => m.modes.iter().map(|x| x.omega.to_bits()).collect(),
        WhirlSpeedModes::LossOfStiffness(l) => panic!("{l:?}"),
    }
}

#[test]
fn b3_the_beams_option_changes_no_bit_without_beams() {
    let mesh = hex_box([0.0, -0.05, -0.05], [1.0, 0.1, 0.1], [8, 1, 1]).unwrap();
    let mut hm = SolidModel::new(mesh);
    hm.add_material("all", mat()).unwrap();
    for c in 0..3 {
        hm.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let ax = [0.0, 0.0, 1.0];
    let o = opts(4).coriolis(true);
    let a = RotatingPart::new(&hm, ax, [0.0; 3], &o).unwrap();
    let b = RotatingPart::new(&hm, ax, [0.0; 3], &o.clone().beams(true)).unwrap();
    let same_c = same_bits(&a.coriolis().unwrap(), &b.coriolis().unwrap());
    let (wa, wb) = (whirl_bits(&a, 200.0), whirl_bits(&b, 200.0));
    println!(
        "B3a Hex8 part: Coriolis bits equal {same_c}, whirl omegas bits equal {} ({} modes)",
        wa == wb,
        wa.len()
    );
    let m = sector(beam_options());
    let s1 = RotatingSector::new(&m, &opts(4)).unwrap();
    let s2 = RotatingSector::new(&m, &opts(4).beams(true)).unwrap();
    let same_s = same_bits(&s1.part().coriolis().unwrap(), &s2.part().coriolis().unwrap());
    println!("B3b W with and without RotatingOptions::beams: Coriolis bits equal {same_s}");
    assert!(same_c && wa == wb && !wa.is_empty() && same_s);
}

// ---------------------------------------------------------------------------------------------- NC1, NC2

fn refused_as_before<T: std::fmt::Debug>(r: Result<T, FemError>) -> bool {
    match r {
        Err(FemError::Element { source, .. }) => matches!(source.as_ref(),
            FemError::NotYetSupported { variant, package, .. } if *variant == "Beam2" && *package == SOLID_ROUTINES),
        _ => false,
    }
}

#[test]
fn nc1_nc2_refusals_without_the_option() {
    let full = ring(true);
    let w = 30.0;
    let p1 = RotatingPart::new(&full, axis(), ORIGIN, &opts(30).coriolis(true)).unwrap();
    let p2 = RotatingPart::new(&full, axis(), ORIGIN, &opts(30).coriolis(true).shells(true)).unwrap();
    let r1 = refused_as_before(p1.whirl_modes(w));
    let r2 = refused_as_before(p2.whirl_modes(w));
    println!("NC1 ring without RotatingOptions::beams: refused as before {r1}; with shells(true) {r2}");
    let m = sector(beam_options());
    let r3 = refused_as_before(coriolis_matrix(&m, axis(), None));
    let r4 = refused_as_before(coriolis_matrix_with_shells(&m, axis(), None));
    println!("NC2 coriolis_matrix of W refused as before {r3}; coriolis_matrix_with_shells {r4}");
    assert!(r1 && r2 && r3 && r4);
}
