//! F-576 (ADR 0499): check N1 (record B0, D6). The quasi-static time-stepping driver of ADR 0489 (`solve_in_time`: a creep
//! cube under a held strain, the three-branch viscoelastic cube, the creep cube with a halving increment limit), the nonlinear
//! transient integrator (an elastic Hex8 bar under a step load with its strain energy, a J2 bar and a three-branch viscoelastic
//! rod started by a velocity, with the state bytes), the assembled internal force (a J2 bar from a non-zero committed state,
//! the viscoelastic cube) and the refusal of a shifted viscoelastic material by `Integrator::start` give the bits they gave
//! before F-576. Uses only API that existed at 39e7ef84; the expected digests were printed by this file run on the unchanged
//! base 39e7ef84 (`f576-scratch/b0-base.log`) and written into `EXPECTED` before any library code of F-576.

use rcfd_fem::builders::hex_box;
use rcfd_fem::material::{PronyTerm, TimeTemperatureShift};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{internal_force, solve, solve_in_time, NewtonOptions, TimeHistory, TimeSteppingOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{Constraint, Material, SolidModel};

/// Digests of 39e7ef84 in the order of the run, from the base run (`f576-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 13]> = Some([
    ("solve_in_time creep cube", 0x08b3_63fe_721a_8ae8),
    ("solve_in_time viscoelastic cube", 0xa93d_4936_1bc3_eb9e),
    ("solve_in_time creep cube halved", 0x5e65_b265_90a5_80f2),
    ("nl_transient elastic bar", 0x1a31_f1fa_d00e_072f),
    ("nl_transient elastic bar bytes", 0x1dd3_6d7c_a8f0_e581),
    ("nl_transient elastic bar strain energy", 0xf687_9eaf_4752_b3d7),
    ("nl_transient J2 bar", 0x4046_477e_938c_99fc),
    ("nl_transient J2 bar bytes", 0x1ca5_ba0c_c2df_395b),
    ("nl_transient viscoelastic rod", 0xd934_967f_2821_f389),
    ("nl_transient viscoelastic rod bytes", 0x7227_e6a4_8b76_42f9),
    ("internal_force J2 bar", 0xec5a_c15c_7cb3_587c),
    ("internal_force viscoelastic cube", 0x2a1d_4513_6dc0_8d70),
    ("nl_transient shifted refusal", 0x3708_f951_d284_c388),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    digest_bytes(v.iter().flat_map(|x| x.to_bits().to_le_bytes()))
}

fn digest_bytes<I: IntoIterator<Item = u8>>(bytes: I) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

const E: f64 = 2.0e9;

fn base(nu: f64) -> Material {
    Material::isotropic_elastic(E, nu, 1200.0, 0.0).unwrap()
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn j2() -> Material {
    Material::j2(steel(), 2.0e8, 2.0e9, 1.0e9).unwrap()
}

fn visco(nu: f64, shift: Option<TimeTemperatureShift>) -> Material {
    let terms = [(0.3, 0.01), (0.2, 0.1), (0.1, 1.0)]
        .iter()
        .map(|&(w, t)| PronyTerm::new(w, w, t).unwrap())
        .collect();
    Material::viscoelastic(base(nu), terms, shift).unwrap()
}

/// The unit cube of one Hex8 with three symmetry planes, `u_x = stretch` on `xmax` if non-zero.
fn cube(material: Material, stretch: f64) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    if stretch != 0.0 {
        m.add_constraint(Constraint::displacement("xmax", 0, stretch)).unwrap();
    }
    m
}

/// A bar `[1, 0.2, 0.2]` of `[3, 1, 1]` Hex8 with three symmetry planes.
fn bar(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

/// The rod of ADR 0485 V1: 20 Hex8 along x, the lateral motion held.
fn rod(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.05, 0.05], [20, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn xmax_x_equations(model: &SolidModel) -> Vec<usize> {
    let dofs = model.dof_map();
    model
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| dofs.equation(n, 0).unwrap())
        .collect()
}

/// Every field of every step end of a time history.
fn flat_history(h: &TimeHistory) -> Vec<f64> {
    let mut v = Vec::new();
    for s in &h.steps {
        v.push(s.time);
        v.extend_from_slice(&s.u);
        v.extend_from_slice(&s.internal_force);
        for e in &s.states {
            v.push(e.values().len() as f64);
            v.extend_from_slice(e.values());
        }
        v.push(s.halvings as f64);
        v.push(s.substeps as f64);
        v.push(s.max_creep_increment);
        v.push(s.iterations as f64);
    }
    v
}

/// The values of an nl_transient run: `u`, `v`, `a`, the internal force after every step, then the bytes of the state.
fn nl_run(model: &SolidModel, loads: &[LoadHistory], v0: &[f64], dt: f64, steps: u64) -> (Vec<f64>, Vec<u8>) {
    let mut it = NlIntegrator::new(model, loads, &NlTransientOptions::new(dt)).unwrap();
    let n = v0.len();
    it.start(0.0, &vec![0.0; n], v0).unwrap();
    let mut v = Vec::new();
    for _ in 0..steps {
        it.step().unwrap();
        let s = it.state().unwrap();
        v.extend_from_slice(&s.u);
        v.extend_from_slice(&s.v);
        v.extend_from_slice(&s.a);
        v.extend_from_slice(&s.internal_force);
    }
    (v, it.state().unwrap().to_bytes())
}

#[test]
fn n1_default_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let times = |steps: usize, dt: f64| -> Vec<f64> { (1..=steps).map(|k| k as f64 * dt).collect() };

    // solve_in_time: a creep cube under a held strain, the viscoelastic cube, the creep cube with a halving limit.
    let a3 = 1.0 / (2.0 * E * (E * 1e-3f64).powi(2));
    let creep = Material::creep(base(0.3), a3, 3.0, 0.0).unwrap();
    let h = solve_in_time(
        &cube(creep.clone(), 1e-3),
        &[],
        &times(20, 0.0125),
        &TimeSteppingOptions::new(f64::INFINITY),
    )
    .unwrap();
    got.push(("solve_in_time creep cube", digest(&flat_history(&h))));
    let mut tight = NewtonOptions::new(1);
    tight.residual_tolerance = 1e-13;
    tight.energy_tolerance = 1.0;
    let h = solve_in_time(
        &cube(visco(0.35, None), 1e-3),
        &[],
        &times(20, 0.02),
        &TimeSteppingOptions::new(f64::INFINITY).newton(tight.clone()),
    )
    .unwrap();
    got.push(("solve_in_time viscoelastic cube", digest(&flat_history(&h))));
    let h = solve_in_time(&cube(creep, 1e-3), &[], &[1.0], &TimeSteppingOptions::new(1e-4)).unwrap();
    println!("B0 halvings of the creep cube: {}", h.steps[1].halvings);
    got.push(("solve_in_time creep cube halved", digest(&flat_history(&h))));

    // nl_transient: an elastic bar under a step load (and its strain energy), a J2 bar and a viscoelastic rod.
    let model = bar(steel());
    let n = model.dof_map().n_equations();
    let eqs = xmax_x_equations(&model);
    let mut pattern = vec![0.0; n];
    for &q in &eqs {
        pattern[q] = 2.0e6;
    }
    let table = LoadTable::new(vec![(0.0, 1.0), (1.0, 1.0)]).unwrap();
    let loads = [LoadHistory::new(pattern, table)];
    let (v, bytes) = nl_run(&model, &loads, &vec![0.0; n], 2e-5, 30);
    got.push(("nl_transient elastic bar", digest(&v)));
    got.push(("nl_transient elastic bar bytes", digest_bytes(bytes)));
    let mut it = NlIntegrator::new(&model, &loads, &NlTransientOptions::new(2e-5)).unwrap();
    it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    it.run(10).unwrap();
    got.push((
        "nl_transient elastic bar strain energy",
        digest(&[it.strain_energy().unwrap()]),
    ));
    let jm = bar(j2());
    let mut v0 = vec![0.0; n];
    for &q in &eqs {
        v0[q] = 40.0;
    }
    let (v, bytes) = nl_run(&jm, &[], &v0, 2e-6, 30);
    got.push(("nl_transient J2 bar", digest(&v)));
    got.push(("nl_transient J2 bar bytes", digest_bytes(bytes)));
    let vr = rod(visco(0.0, None));
    let nr = vr.dof_map().n_equations();
    let v0: Vec<f64> = (0..nr).map(|i| 1e-3 * ((i as f64) * 0.11).sin()).collect();
    let mut v0 = v0;
    for q in 0..nr {
        let (_, c) = vr.dof_map().node_component(q).unwrap();
        if c != 0 {
            v0[q] = 0.0;
        }
    }
    for &q in vr.mesh().node_set("xmin").unwrap() {
        v0[vr.dof_map().equation(q, 0).unwrap()] = 0.0;
    }
    let (v, bytes) = nl_run(&vr, &[], &v0, 1e-5, 60);
    got.push(("nl_transient viscoelastic rod", digest(&v)));
    got.push(("nl_transient viscoelastic rod bytes", digest_bytes(bytes)));

    // The assembled internal force: a J2 bar from a non-zero committed state (after a plastic solve), the viscoelastic
    // cube from the states of a strain step.
    let mut jstretched = bar(j2());
    jstretched
        .add_constraint(Constraint::displacement("xmax", 0, 5e-3))
        .unwrap();
    let r = solve(&jstretched, &[], &[], &NewtonOptions::new(4)).unwrap();
    let u: Vec<f64> = r
        .result
        .displacement_values()
        .iter()
        .enumerate()
        .map(|(i, x)| x * (1.0 + 1e-3 * (i as f64).cos()))
        .collect();
    let f = internal_force(&jstretched, &u, &r.states).unwrap();
    got.push(("internal_force J2 bar", digest(&f)));
    let vc = cube(visco(0.35, None), 1e-3);
    let h = solve_in_time(
        &vc,
        &[],
        &[0.05],
        &TimeSteppingOptions::new(f64::INFINITY).newton(tight),
    )
    .unwrap();
    let last = h.steps.last().unwrap();
    let u: Vec<f64> = last.u.iter().map(|x| 1.01 * x).collect();
    let f = internal_force(&vc, &u, &last.states).unwrap();
    got.push(("internal_force viscoelastic cube", digest(&f)));

    // The refusal of a shifted viscoelastic material by Integrator::start (no temperature).
    let wlf = TimeTemperatureShift::wlf(373.0, 17.44, 51.6).unwrap();
    let sr = rod(visco(0.0, Some(wlf)));
    let ns = sr.dof_map().n_equations();
    let mut it = NlIntegrator::new(&sr, &[], &NlTransientOptions::new(1e-5)).unwrap();
    let msg = format!("{:?}", it.start(0.0, &vec![0.0; ns], &vec![0.0; ns]).err());
    println!("B0 refusal: {msg}");
    got.push(("nl_transient shifted refusal", digest_bytes(msg.bytes())));

    for (name, d) in &got {
        println!("B0 {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("EXPECTED is written from the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, e)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, e, "B0 {name}: {d:#018x} against {e:#018x}");
    }
}
