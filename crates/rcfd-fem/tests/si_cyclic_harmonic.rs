//! F-489 (ADR 0395): cyclic symmetry, harmonic static loads and rigid sets at a non-zero harmonic index. Checks H1 to H8
//! and the negative controls N1 to N8 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::cyclic::{
    full_rotor, modal_analysis_harmonic, nodal_forces_of, rotation_matrix, solve_static_harmonic, FullRotor,
    HarmonicLoad, HarmonicModes, Mat3,
};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::mpc::{self, ReferenceLoad};
use rcfd_fem::solve::{StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const NU: f64 = 0.3;
const N: usize = 12;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
/// Index of the rigid set in the sector models (three Displacement, the Cyclic, then the RigidSet).
const RIGID: usize = 4;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn le(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.3e} (limit {limit:.0e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what}: {}", if ok { "holds" } else { "FAILS" });
        if !ok {
            self.failed.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "failed: {:?}", self.failed);
    }
}

fn alpha() -> f64 {
    2.0 * PI / N as f64
}

fn polar_sector(r0: f64, dr: f64, z0: f64, dz: f64, alpha: f64, div: [usize; 3]) -> SolidMesh {
    let b = hex_box([r0, 0.0, z0], [dr, alpha, dz], div).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

fn nodes_where<F: Fn(Vec3) -> bool>(mesh: &SolidMesh, f: F) -> Vec<usize> {
    (0..mesh.nodes().len()).filter(|&p| f(mesh.nodes()[p])).collect()
}

fn radius(x: Vec3) -> f64 {
    (x[0] * x[0] + x[1] * x[1]).sqrt()
}

/// The disc sector of D4 with the node sets `rim` (outer face, `0 < phi < alpha`), `shroud` (outer face) and `pair` (the two
/// outer nodes at `phi = alpha/2`, `z = +/-0.1`).
fn d_mesh() -> SolidMesh {
    let a = alpha();
    let mut mesh = polar_sector(0.25, 0.75, -0.1, 0.2, a, [3, 2, 1]);
    let outer = |x: Vec3| (radius(x) - 1.0).abs() < 1e-12;
    let phi = |x: Vec3| x[1].atan2(x[0]);
    let rim = nodes_where(&mesh, |x| outer(x) && phi(x) > 1e-9 && phi(x) < a - 1e-9);
    assert_eq!(rim.len(), 7, "rim nodes");
    let shroud = nodes_where(&mesh, outer);
    let pair = nodes_where(&mesh, |x| {
        outer(x) && (phi(x) - a / 2.0).abs() < 1e-12 && (x[2].abs() - 0.1).abs() < 1e-12
    });
    assert_eq!(pair.len(), 2, "pair nodes");
    mesh.add_node_set("rim", rim).unwrap();
    mesh.add_node_set("shroud", shroud).unwrap();
    mesh.add_node_set("pair", pair).unwrap();
    mesh
}

fn x_ref() -> Vec3 {
    [(alpha() / 2.0).cos(), (alpha() / 2.0).sin(), 0.0]
}

fn model_on(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap())
        .unwrap();
    m
}

fn clamp(m: &mut SolidModel, set: &str) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

/// D (no rigid set), DR (`rim`) or DSH (`shroud`).
fn d_sector(rigid: Option<&str>) -> SolidModel {
    let mut m = model_on(d_mesh());
    clamp(&mut m, "xmin");
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, [0.0; 3], N))
        .unwrap();
    if let Some(set) = rigid {
        m.add_constraint(Constraint::rigid_set(set, x_ref())).unwrap();
    }
    m
}

fn apply(r: &Mat3, v: Vec3) -> Vec3 {
    [
        r[0][0] * v[0] + r[0][1] * v[1] + r[0][2] * v[2],
        r[1][0] * v[0] + r[1][1] * v[1] + r[1][2] * v[2],
        r[2][0] * v[0] + r[2][1] * v[1] + r[2][2] * v[2],
    ]
}

fn rot(j: usize) -> Mat3 {
    rotation_matrix(AXIS, 2.0 * PI * j as f64 / N as f64)
}

fn phase(k: usize, j: usize) -> (f64, f64) {
    let jk = (j * k) % N;
    if jk == 0 {
        (1.0, 0.0)
    } else if 2 * jk == N {
        (-1.0, 0.0)
    } else {
        let t = 2.0 * PI * jk as f64 / N as f64;
        (t.cos(), t.sin())
    }
}

fn comb(c: f64, a: Vec3, s: f64, b: Vec3) -> Vec3 {
    [c * a[0] - s * b[0], c * a[1] - s * b[1], c * a[2] - s * b[2]]
}

/// The full model of a sector model of D4: the rotor, `xmin` clamped, one rigid set per sector (sets `{set}_{j}`).
fn full_of(sector: &SolidModel, rigid: Option<&str>) -> (FullRotor, SolidModel) {
    let rotor = full_rotor(sector).unwrap();
    let mut mesh = rotor.mesh().clone();
    if let Some(set) = rigid {
        let nodes = sector.mesh().node_set(set).unwrap().to_vec();
        for j in 0..N {
            let mut v: Vec<usize> = nodes.iter().map(|&p| rotor.node(j, p)).collect();
            v.sort_unstable();
            mesh.add_node_set(&format!("{set}_{j}"), v).unwrap();
        }
    }
    let mut full = model_on(mesh);
    clamp(&mut full, "xmin");
    if let Some(set) = rigid {
        for j in 0..N {
            full.add_constraint(Constraint::rigid_set(&format!("{set}_{j}"), apply(&rot(j), x_ref())))
                .unwrap();
        }
    }
    (rotor, full)
}

fn inf(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

/// The loads of D4: (cosine part, sine part, the two nodal forces of the cosine part).
fn parts(mesh: &SolidMesh) -> (Vec<f64>, Vec<f64>, Vec<NodalForceAt>) {
    let low0 = mesh.surface_set_nodes("ymin").unwrap()[0];
    let high0 = mesh.surface_set_nodes("ymax").unwrap()[0];
    let target = [0.6 * (alpha() / 2.0).cos(), 0.6 * (alpha() / 2.0).sin(), 0.1];
    let interior = (0..mesh.nodes().len())
        .min_by(|&a, &b| {
            let d = |p: usize| {
                let x = mesh.nodes()[p];
                (x[0] - target[0]).powi(2) + (x[1] - target[1]).powi(2) + (x[2] - target[2]).powi(2)
            };
            d(a).total_cmp(&d(b))
        })
        .unwrap();
    let cos_forces = vec![
        NodalForceAt::new(low0, [1e4, -2e4, 3e4]),
        NodalForceAt::new(high0, [-5e3, 1e4, 2e4]),
    ];
    let mut mc = model_on(mesh.clone());
    mc.add_load(Load::pressure("xmax", 1e6)).unwrap();
    let fa = nodal_load_vector(&mc, &cos_forces).unwrap();
    let mut ms = model_on(mesh.clone());
    ms.add_load(Load::traction("zmax", [2e5, -1e5, 5e5])).unwrap();
    let fb = nodal_load_vector(&ms, &[NodalForceAt::new(interior, [3e4, 1e4, -1e4])]).unwrap();
    (fa, fb, cos_forces)
}

const F_COS: (Vec3, Vec3) = ([1e4, 2e4, -3e4], [1e3, -2e3, 5e2]);
const F_SIN: (Vec3, Vec3) = ([-2e4, 1e4, 1e4], [-5e2, 1e3, 2e3]);

/// H1 to H3: every harmonic 0 .. 6 of one sector model against its full model.
/// Returns the smallest `max |u_full|` over the harmonics (the set-up guard of H3b, ADR 0395 amendment 2).
fn static_equivalence(name: &str, id: &str, rigid: Option<&str>, reference_loads: bool, ch: &mut Checks) -> f64 {
    let sector = d_sector(rigid);
    let (rotor, full) = full_of(&sector, rigid);
    let sdofs = sector.dof_map();
    let fdofs = full.dof_map();
    let (fa, fb, _) = parts(sector.mesh());
    let opts = StaticOptions::default();
    let (mut eu, mut et, mut ew) = (0.0f64, 0.0f64, 0.0f64);
    let mut smallest = f64::INFINITY;
    for k in 0..=N / 2 {
        let single = k == 0 || 2 * k == N;
        let sin = if single { vec![0.0; fb.len()] } else { fb.clone() };
        let mut load = HarmonicLoad::new(k, fa.clone(), sin.clone());
        let (fs, ms) = if single { ([0.0; 3], [0.0; 3]) } else { F_SIN };
        if reference_loads {
            load = load.references(
                vec![ReferenceLoad::new(RIGID, F_COS.0, F_COS.1)],
                vec![ReferenceLoad::new(RIGID, fs, ms)],
            );
        }
        let hs = solve_static_harmonic(&sector, &[], &load, &opts).unwrap();
        let f_full = rotor.expand_load(&sdofs, &fdofs, k, &fa, &sin);
        let forces = nodal_forces_of(&fdofs, &f_full);
        let mut full_refs = Vec::new();
        if reference_loads {
            for j in 0..N {
                let (c, s) = phase(k, j);
                full_refs.push(ReferenceLoad::new(
                    3 + j,
                    apply(&rot(j), comb(c, F_COS.0, s, fs)),
                    apply(&rot(j), comb(c, F_COS.1, s, ms)),
                ));
            }
        }
        let fs_sol = mpc::solve_static_mpc(&full, &[], &forces, &full_refs, &opts).unwrap();
        let u_full = fs_sol.result.displacement_values();
        let u_exp = rotor.expand_field(&sdofs, &fdofs, k, &hs.cos, &hs.sin);
        let scale = inf(u_full);
        smallest = smallest.min(scale);
        let d: Vec<f64> = u_exp.iter().zip(u_full).map(|(a, b)| a - b).collect();
        let e_u = inf(&d) / scale;
        let (mut e_t, mut e_w) = (0.0f64, 0.0f64);
        if rigid.is_some() {
            let wmax = fs_sol
                .references
                .iter()
                .map(|m| inf(&m.rotation))
                .fold(0.0f64, f64::max);
            for j in 0..N {
                let (c, s) = phase(k, j);
                let (rc, rs) = (&hs.reference_cos[0], &hs.reference_sin[0]);
                let t = apply(&rot(j), comb(c, rc.translation, s, rs.translation));
                let w = apply(&rot(j), comb(c, rc.rotation, s, rs.rotation));
                let m = &fs_sol.references[j];
                assert_eq!(m.constraint, 3 + j);
                let dt: Vec<f64> = (0..3).map(|i| t[i] - m.translation[i]).collect();
                let dw: Vec<f64> = (0..3).map(|i| w[i] - m.rotation[i]).collect();
                e_t = e_t.max(inf(&dt) / scale);
                e_w = e_w.max(inf(&dw) / wmax);
            }
        }
        println!(
            "{name} k = {k}: e_u {e_u:.3e}, e_t {e_t:.3e}, e_w {e_w:.3e}, max |u| {scale:.4e}, sector unknowns {}, \
             residual {:.2e}, full dropped rows {}",
            hs.unknowns,
            hs.relative_residual,
            fs_sol.dropped.len()
        );
        eu = eu.max(e_u);
        et = et.max(e_t);
        ew = ew.max(e_w);
    }
    ch.le(&format!("{id} {name} e_u"), eu, 1e-8);
    if rigid.is_some() {
        ch.le(&format!("{id} {name} e_t"), et, 1e-8);
        ch.le(&format!("{id} {name} e_w"), ew, 1e-8);
    }
    smallest
}

#[test]
fn h1_harmonic_load_clamped_disc() {
    let mut ch = Checks::new();
    static_equivalence("D", "H1", None, false, &mut ch);
    ch.finish();
}

#[test]
fn h2_harmonic_load_disc_with_rim_rigid_set() {
    let mut ch = Checks::new();
    static_equivalence("DR", "H2", Some("rim"), true, &mut ch);
    ch.finish();
}

#[test]
#[ignore = "record of ADR 0395: a defective set-up (the exact solution is zero at k = 0 and 6, amendment 2); corrected by h3b below"]
fn h3_harmonic_load_disc_with_shroud_rigid_set() {
    let mut ch = Checks::new();
    static_equivalence("DSH", "H3", Some("shroud"), false, &mut ch);
    ch.finish();
}

/// H3b (ADR 0395 amendment 2): DSH with the reference loads of DR; guard `max |u_full| >= 1e-12` m at every `k`.
#[test]
fn h3b_harmonic_load_disc_with_shroud_rigid_set_and_reference_loads() {
    let mesh = d_mesh();
    let xmin = mesh.surface_set_nodes("xmin").unwrap();
    for set in ["ymin", "ymax"] {
        let p = mesh.surface_set_nodes(set).unwrap()[0];
        println!(
            "H3b: first node {p} of {set} at {:?}, on the clamped face xmin: {}",
            mesh.nodes()[p],
            xmin.binary_search(&p).is_ok()
        );
    }
    let mut ch = Checks::new();
    let smallest = static_equivalence("DSH", "H3b", Some("shroud"), true, &mut ch);
    println!("H3b: smallest max |u_full| over k = {smallest:.4e} m");
    assert!(
        smallest >= 1e-12,
        "defective set-up: max |u_full| = {smallest:e} m at some k"
    );
    ch.finish();
}

#[test]
fn h4_expanded_pressure_equals_full_pressure() {
    let mut ch = Checks::new();
    let sector = d_sector(None);
    let (rotor, full) = full_of(&sector, None);
    let mut mc = model_on(d_mesh());
    mc.add_load(Load::pressure("xmax", 1e6)).unwrap();
    let fa = nodal_load_vector(&mc, &[]).unwrap();
    let fe = rotor.expand_load(&sector.dof_map(), &full.dof_map(), 0, &fa, &vec![0.0; fa.len()]);
    let mut fm = full.clone();
    fm.add_load(Load::pressure("xmax", 1e6)).unwrap();
    let ff = nodal_load_vector(&fm, &[]).unwrap();
    let d: Vec<f64> = fe.iter().zip(&ff).map(|(a, b)| a - b).collect();
    ch.le("H4 expanded pressure", inf(&d) / inf(&ff), 1e-12);
    ch.finish();
}

fn equivalence_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes).tolerance(1e-9);
    o.max_iterations = 400;
    o
}

/// H5, H6: the frequencies of a sector with a rigid set against the full model's (the procedure of ADR 0386 E1/E2).
fn modal_equivalence(name: &str, id: &str, rigid: &str, ch: &mut Checks) {
    let sector = d_sector(Some(rigid));
    let harmonics: Vec<HarmonicModes> = (0..=N / 2)
        .map(|k| modal_analysis_harmonic(&sector, &[], k, &equivalence_options(4)).unwrap())
        .collect();
    for h in &harmonics {
        println!(
            "{name} k = {}: multiplicity {}, f = {:?} Hz, pair splits {:?}, shift {:e}, iterations {}, unknowns {}",
            h.harmonic, h.multiplicity, h.frequencies, h.pair_splits, h.shift, h.iterations, h.unknowns
        );
    }
    let lambda_c = 0.999
        * harmonics
            .iter()
            .map(|h| h.values.iter().fold(0.0f64, |m, &v| m.max(v)))
            .fold(f64::INFINITY, f64::min);
    let mut below: Vec<f64> = harmonics
        .iter()
        .flat_map(|h| h.eigenvalues.iter().copied())
        .filter(|&l| l < lambda_c)
        .collect();
    below.sort_by(f64::total_cmp);
    let n_c = below.len();
    let (_, full) = full_of(&sector, Some(rigid));
    let fr = modal_analysis(&full, &equivalence_options(n_c + 8)).unwrap();
    println!(
        "{name} full: {} unknowns, {} modes, shift {:e}, iterations {}, largest {:e}, cut-off {lambda_c:e}, n_c {n_c}",
        fr.unknowns,
        fr.eigenvalues.len(),
        fr.shift,
        fr.iterations,
        fr.eigenvalues.last().unwrap()
    );
    assert!(
        *fr.eigenvalues.last().unwrap() > lambda_c,
        "defective set-up: the full model's computed modes do not pass the cut-off"
    );
    let near = below
        .iter()
        .chain(fr.eigenvalues.iter())
        .map(|l| (l / lambda_c - 1.0).abs())
        .fold(f64::INFINITY, f64::min);
    println!("{name}: nearest eigenvalue to the cut-off at {near:.3e} relative");
    assert!(
        near >= 1e-7,
        "defective set-up: an eigenvalue within 1e-7 of the cut-off"
    );
    let full_below: Vec<f64> = fr.eigenvalues.iter().copied().filter(|&l| l < lambda_c).collect();
    ch.holds(
        &format!("{id} {name} count"),
        full_below.len() == n_c,
        &format!("full count {} = sector count {n_c}", full_below.len()),
    );
    let worst = below
        .iter()
        .zip(&full_below)
        .map(|(s, f)| ((s / f).sqrt() - 1.0).abs())
        .fold(0.0f64, f64::max);
    ch.le(&format!("{id} {name} frequencies"), worst, 1e-8);
    let split = harmonics
        .iter()
        .filter(|h| h.multiplicity == 2)
        .flat_map(|h| h.pair_splits.iter().copied())
        .fold(0.0f64, f64::max);
    ch.le(&format!("{id} {name} pair split"), split, 1e-8);
}

#[test]
fn h5_frequencies_with_rim_rigid_set() {
    let mut ch = Checks::new();
    modal_equivalence("DR", "H5", "rim", &mut ch);
    ch.finish();
}

#[test]
fn h6_frequencies_with_shroud_rigid_set() {
    let mut ch = Checks::new();
    modal_equivalence("DSH", "H6", "shroud", &mut ch);
    ch.finish();
}

#[test]
fn h7_reference_unknowns_doubled() {
    let mut ch = Checks::new();
    let dr = d_sector(Some("rim"));
    let n = dr.dof_map().n_equations();
    let t2 = mpc::harmonic_transformation(&dr, &[], 2).unwrap();
    println!(
        "H7 k = 2: {} unknowns, references {:?}",
        t2.n_unknowns(),
        t2.references()
    );
    ch.holds(
        "H7 k = 2",
        t2.n_unknowns() == 2 * n + 12
            && t2.n_mesh() == 2 * n
            && t2.references() == [(RIGID, 2 * n), (RIGID, 2 * n + 6)]
            && t2.reference_offset(RIGID) == Some(2 * n),
        "2n + 12 unknowns, the rim listed at 2n and 2n + 6",
    );
    let t6 = mpc::harmonic_transformation(&dr, &[], 6).unwrap();
    ch.holds(
        "H7 k = 6",
        t6.n_unknowns() == n + 6 && t6.references() == [(RIGID, n)],
        "n + 6 unknowns, the rim once",
    );
    let t0 = mpc::transformation(&dr, &[]).unwrap();
    ch.holds(
        "H7 k = 0",
        t0.n_unknowns() == n + 6 && t0.references() == [(RIGID, n)],
        "n + 6 unknowns, the rim once",
    );
    ch.finish();
}

#[test]
fn h8_k0_equals_the_static_solve_on_the_transformation() {
    let mut ch = Checks::new();
    let d = d_sector(None);
    let (fa, _, cos_forces) = parts(d.mesh());
    let opts = StaticOptions::default();
    let hs = solve_static_harmonic(&d, &[], &HarmonicLoad::new(0, fa.clone(), vec![0.0; fa.len()]), &opts).unwrap();
    let mut dl = d.clone();
    dl.add_load(Load::pressure("xmax", 1e6)).unwrap();
    let s = mpc::solve_static_mpc(&dl, &[], &cos_forces, &[], &opts).unwrap();
    let u = s.result.displacement_values();
    let diff: Vec<f64> = hs.cos.iter().zip(u).map(|(a, b)| a - b).collect();
    let e = inf(&diff) / inf(u);
    println!("H8: bit-equal {}", diff.iter().all(|&x| x == 0.0));
    ch.le("H8 k = 0 against solve_static_mpc", e, 1e-14);
    ch.finish();
}

fn invalid(r: Result<impl std::fmt::Debug, FemError>, what: &str) -> (&'static str, String) {
    match r {
        Err(FemError::InvalidValue { key, reason }) => {
            println!("{what}: refused, key {key}: {reason}");
            (key, reason)
        }
        other => panic!("{what}: expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn n1_to_n8_refusals() {
    let opts = StaticOptions::default();
    let d = d_sector(None);
    let (fa, fb, _) = parts(d.mesh());
    let n = fa.len();
    // N1: a sine part at k = 0 and 6; a sine reference load at k = 6.
    for k in [0, 6] {
        let (key, _) = invalid(
            solve_static_harmonic(&d, &[], &HarmonicLoad::new(k, fa.clone(), fb.clone()), &opts),
            &format!("N1 k = {k}"),
        );
        assert_eq!(key, "harmonic load");
    }
    let dr = d_sector(Some("rim"));
    let load = HarmonicLoad::new(6, fa.clone(), vec![0.0; n]).references(
        vec![ReferenceLoad::new(RIGID, F_COS.0, F_COS.1)],
        vec![ReferenceLoad::new(RIGID, F_SIN.0, F_SIN.1)],
    );
    let (key, _) = invalid(solve_static_harmonic(&dr, &[], &load, &opts), "N1 reference k = 6");
    assert_eq!(key, "harmonic load");
    // N2: the model's own loads.
    let mut dl = d.clone();
    dl.add_load(Load::pressure("xmax", 1e6)).unwrap();
    let (key, _) = invalid(
        solve_static_harmonic(&dl, &[], &HarmonicLoad::new(1, fa.clone(), fb.clone()), &opts),
        "N2",
    );
    assert_eq!(key, "loads");
    // N3: a part of the wrong length.
    let r = solve_static_harmonic(&d, &[], &HarmonicLoad::new(1, fa[..n - 1].to_vec(), fb.clone()), &opts);
    println!("N3: {r:?}", r = r.as_ref().err());
    assert!(matches!(r, Err(FemError::FieldLength { .. })), "N3");
    // N4: k = 7.
    let (key, _) = invalid(
        solve_static_harmonic(&d, &[], &HarmonicLoad::new(7, fa.clone(), fb.clone()), &opts),
        "N4",
    );
    assert_eq!(key, "harmonic");
    // N5: a non-zero prescribed value at k = 2 refused, at k = 0 solved.
    let node = d.mesh().node_set("pair").unwrap()[0];
    let (key, reason) = invalid(
        solve_static_harmonic(
            &d,
            &[Prescribed::new(node, 2, 1e-3)],
            &HarmonicLoad::new(2, fa.clone(), fb.clone()),
            &opts,
        ),
        "N5 k = 2",
    );
    assert_eq!(key, "constraint");
    assert!(reason.contains("zero values only"), "N5: {reason}");
    let s0 = solve_static_harmonic(
        &d,
        &[Prescribed::new(node, 2, 1e-3)],
        &HarmonicLoad::new(0, fa.clone(), vec![0.0; n]),
        &opts,
    )
    .unwrap();
    let e = d.dof_map().equation(node, 2).unwrap();
    println!("N5 k = 0: u_z at node {node} = {:e}", s0.cos[e]);
    assert_eq!(s0.cos[e], 1e-3, "N5 k = 0 prescribed value");
    // N6: a reference load on a Displacement constraint.
    let load = HarmonicLoad::new(1, fa.clone(), fb.clone())
        .references(vec![ReferenceLoad::new(0, F_COS.0, F_COS.1)], Vec::new());
    let (key, _) = invalid(solve_static_harmonic(&d, &[], &load, &opts), "N6");
    assert_eq!(key, "reference load");
    // N7: the CG solver.
    let cg = StaticOptions::with_solver(StaticSolver::ConjugateGradient {
        tolerance: 1e-10,
        max_iterations: 1000,
    });
    let (key, _) = invalid(
        solve_static_harmonic(&d, &[], &HarmonicLoad::new(1, fa.clone(), fb.clone()), &cg),
        "N7",
    );
    assert_eq!(key, "solver");
    // N8: a collinear rigid set at k = 2.
    let mut dp = d_sector(None);
    dp.add_constraint(Constraint::rigid_set("pair", x_ref())).unwrap();
    let r = solve_static_harmonic(&dp, &[], &HarmonicLoad::new(2, fa.clone(), fb.clone()), &opts);
    match r {
        Err(FemError::Solver { reason, .. }) => {
            println!("N8: {reason}");
            assert!(
                reason.contains("reference rotation") && reason.contains("RigidSet"),
                "N8: {reason}"
            );
        }
        other => panic!("N8: expected a singular refusal, got {:?}", other.err()),
    }
}
