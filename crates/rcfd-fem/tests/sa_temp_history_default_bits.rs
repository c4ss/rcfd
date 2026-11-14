//! F-592 (ADR 0526): check N1 (record B0, D7). The paths F-592 touches give the bits they gave before F-592: the quasi-static
//! driver with the constant temperature of ADR 0499 (the shifted cube), the nonlinear transient integrator with Rayleigh
//! damping (the unshifted viscoelastic rod without and with a temperature, a J2 bar with a temperature), the shifted rod at a
//! temperature without damping, the energy-momentum step of an elastic bar with damping and a temperature, the strain energy at
//! a temperature, and the refusal of a shifted rod with stiffness damping and no temperature. Uses only API that existed at
//! f168d994; the expected digests were printed by this file run on the unchanged base f168d994 (`f592-scratch/b0-base.log`) and
//! written into `EXPECTED` before any library code of F-592.

use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::material::{PronyTerm, TimeTemperatureShift};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{solve_in_time_with, NewtonOptions, TimeHistory, TimeLoading, TimeSteppingOptions};
use rcfd_fem::transient::{LoadTable, MotionHistory, Rayleigh};
use rcfd_fem::{Constraint, Material, SolidModel};

/// Digests of f168d994 in the order of the run, from the base run (`f592-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 12]> = Some([
    ("solve_in_time_with shifted cube at 383 K", 0x3e3b_e042_8d8d_30d4),
    ("nl_transient damped viscoelastic rod", 0xe4a4_3bac_9cdb_adf6),
    ("nl_transient damped viscoelastic rod bytes", 0xb339_72a3_8f7b_7f85),
    ("nl_transient damped viscoelastic rod at 383 K", 0xe4a4_3bac_9cdb_adf6),
    (
        "nl_transient damped viscoelastic rod at 383 K bytes",
        0xb339_72a3_8f7b_7f85,
    ),
    ("nl_transient shifted rod at 383 K", 0x2fc8_39a6_7ee8_8242),
    ("nl_transient shifted rod at 383 K bytes", 0x18b2_ef49_40fc_b9f8),
    ("nl_transient damped J2 bar at 383 K", 0x5ddd_3cf6_79b5_0c45),
    ("nl_transient damped J2 bar at 383 K bytes", 0xca58_52fd_ff61_3f8e),
    (
        "nl_transient energy-momentum damped elastic bar at 383 K",
        0x2a16_a0e0_6d96_2abf,
    ),
    ("nl_transient strain energy at 383 K", 0xb45b_4efc_ec72_cfb9),
    ("nl_transient shifted damped refusal", 0x3708_f951_d284_c388),
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

fn wlf() -> TimeTemperatureShift {
    TimeTemperatureShift::wlf(373.0, 17.44, 51.6).unwrap()
}

fn visco(nu: f64, shift: Option<TimeTemperatureShift>) -> Material {
    let terms = [(0.3, 0.01), (0.2, 0.1), (0.1, 1.0)]
        .iter()
        .map(|&(w, t)| PronyTerm::new(w, w, t).unwrap())
        .collect();
    Material::viscoelastic(base(nu), terms, shift).unwrap()
}

/// The unit cube of one Hex8 with three symmetry planes.
fn cube(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
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
fn nl_run(model: &SolidModel, options: &NlTransientOptions, v0: &[f64], steps: u64) -> (Vec<f64>, Vec<u8>) {
    let mut it = NlIntegrator::new(model, &[], options).unwrap();
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

/// An axial velocity of the rod (zero on the held components and at `xmin`).
fn rod_velocity(model: &SolidModel) -> Vec<f64> {
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let mut v0: Vec<f64> = (0..n).map(|i| 1e-3 * ((i as f64) * 0.11).sin()).collect();
    for (q, x) in v0.iter_mut().enumerate() {
        let (_, c) = dofs.node_component(q).unwrap();
        if c != 0 {
            *x = 0.0;
        }
    }
    for &nd in model.mesh().node_set("xmin").unwrap() {
        v0[dofs.equation(nd, 0).unwrap()] = 0.0;
    }
    v0
}

#[test]
fn n1_default_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let damping = Rayleigh {
        mass: 20.0,
        stiffness: 2.0e-6,
    };

    // The driver with the constant temperature of ADR 0499: the shifted cube under a strain step (a motion history).
    let mut tight = NewtonOptions::new(1);
    tight.residual_tolerance = 1e-13;
    tight.energy_tolerance = 1.0;
    let model = cube(visco(0.35, Some(wlf())));
    let list = model
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 0, 1e-3))
        .collect();
    let loading = TimeLoading::new().motion(MotionHistory::new(list, LoadTable::constant(1.0, 0.0, 1.0).unwrap()));
    let times: Vec<f64> = (1..=20).map(|k| k as f64 * 3e-5).collect();
    let h = solve_in_time_with(
        &model,
        &[],
        &loading,
        &times,
        &TimeSteppingOptions::new(f64::INFINITY).newton(tight).temperature(383.0),
    )
    .unwrap();
    got.push(("solve_in_time_with shifted cube at 383 K", digest(&flat_history(&h))));

    // nl_transient with Rayleigh damping: the unshifted viscoelastic rod without and with a temperature.
    let vr = rod(visco(0.0, None));
    let v0 = rod_velocity(&vr);
    let (v, bytes) = nl_run(&vr, &NlTransientOptions::new(1e-5).damping(damping), &v0, 60);
    got.push(("nl_transient damped viscoelastic rod", digest(&v)));
    got.push(("nl_transient damped viscoelastic rod bytes", digest_bytes(bytes)));
    let (v, bytes) = nl_run(
        &vr,
        &NlTransientOptions::new(1e-5).damping(damping).temperature(383.0),
        &v0,
        60,
    );
    got.push(("nl_transient damped viscoelastic rod at 383 K", digest(&v)));
    got.push((
        "nl_transient damped viscoelastic rod at 383 K bytes",
        digest_bytes(bytes),
    ));

    // The shifted rod at a temperature without damping (the path of ADR 0499 V1T).
    let sr = rod(visco(0.0, Some(wlf())));
    let (v, bytes) = nl_run(&sr, &NlTransientOptions::new(1e-5).temperature(383.0), &v0, 60);
    got.push(("nl_transient shifted rod at 383 K", digest(&v)));
    got.push(("nl_transient shifted rod at 383 K bytes", digest_bytes(bytes)));

    // A J2 bar with Rayleigh damping at a temperature.
    let jm = bar(j2());
    let n = jm.dof_map().n_equations();
    let mut vb = vec![0.0; n];
    for q in xmax_x_equations(&jm) {
        vb[q] = 40.0;
    }
    let jd = Rayleigh {
        mass: 20.0,
        stiffness: 2.0e-8,
    };
    let (v, bytes) = nl_run(
        &jm,
        &NlTransientOptions::new(2e-6).damping(jd).temperature(383.0),
        &vb,
        30,
    );
    got.push(("nl_transient damped J2 bar at 383 K", digest(&v)));
    got.push(("nl_transient damped J2 bar at 383 K bytes", digest_bytes(bytes)));

    // The energy-momentum step of an elastic bar with damping and a temperature, and the strain energy at a temperature.
    let eb = bar(steel());
    let (v, _) = nl_run(
        &eb,
        &NlTransientOptions::new(2e-6)
            .damping(jd)
            .temperature(383.0)
            .energy_momentum(),
        &vb,
        30,
    );
    got.push(("nl_transient energy-momentum damped elastic bar at 383 K", digest(&v)));
    let mut it = NlIntegrator::new(&eb, &[], &NlTransientOptions::new(2e-6).temperature(383.0)).unwrap();
    it.start(0.0, &vec![0.0; n], &vb).unwrap();
    it.run(10).unwrap();
    got.push((
        "nl_transient strain energy at 383 K",
        digest(&[it.strain_energy().unwrap()]),
    ));

    // The refusal of the shifted rod with stiffness damping and no temperature.
    let msg = format!(
        "{:?}",
        NlIntegrator::new(&sr, &[], &NlTransientOptions::new(1e-5).damping(damping)).err()
    );
    println!("B0 refusal: {msg}");
    got.push(("nl_transient shifted damped refusal", digest_bytes(msg.bytes())));

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
