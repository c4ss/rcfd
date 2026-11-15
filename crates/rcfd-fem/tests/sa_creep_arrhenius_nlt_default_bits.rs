//! F-651 (ADR 0578): the bit record B0 (D4), re-run as B1. The plain creep material in the nonlinear transient integrator
//! without a temperature, at 700 K and at -5 K, the factor material at 700 K, the factor material on the time-stepping driver
//! at 700 K, and an elastic cube in the integrator with a temperature history give the bits they gave before F-651. Uses only
//! API that existed at e348eb34; the expected digests were printed by this file run on the unchanged base e348eb34
//! (`f651-scratch/b0-base.log`) and written into `EXPECTED` before any library code of F-651.

use rcfd_fem::builders::hex_box;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{solve_in_time_with, TimeHistory, TimeLoading, TimeSteppingOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{Constraint, Load, Material, SolidModel};

/// Digests of e348eb34 in the order of the run, from the base run (`f651-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 6]> = Some([
    ("nl_transient plain creep cube no T", 0x485f_adda_93da_07b0),
    ("nl_transient plain creep cube at 700 K", 0x485f_adda_93da_07b0),
    ("nl_transient plain creep cube at -5 K", 0x485f_adda_93da_07b0),
    ("nl_transient factor creep cube at 700 K", 0x485f_adda_93da_07b0),
    ("solve_in_time_with factor cube at 700 K", 0x441c_7ea2_2862_1c68),
    ("nl_transient elastic cube temperature history", 0x3612_8082_8ffd_23fc),
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
const SIGMA: f64 = 2.0e3;
/// `A f(T1)` of the record: the creep time `(SIGMA/E)/(A SIGMA^3)` is 1 s.
const A_EFF: f64 = 1.0 / (E * SIGMA * SIGMA);
const Q: f64 = 2.0e5;
const R: f64 = 8.314_462_618;

fn base() -> Material {
    Material::isotropic_elastic(E, 0.3, 1200.0, 0.0).unwrap()
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

fn pulled(material: Material) -> SolidModel {
    let mut m = cube(material);
    m.add_load(Load::traction("xmax", [SIGMA, 0.0, 0.0])).unwrap();
    m
}

/// The step load of the traction on `xmax` as a history with the constant factor 1 on `[0, 1]`.
fn step_load() -> LoadHistory {
    let pattern = nodal_load_vector(&pulled(base()), &[]).unwrap();
    LoadHistory::new(pattern, LoadTable::constant(1.0, 0.0, 1.0).unwrap())
}

/// `u`, `v`, `a`, the internal force after every step, then the bytes of the state (one value per byte).
fn nl_run(model: &SolidModel, options: &NlTransientOptions, steps: u64) -> Vec<f64> {
    let mut it = NlIntegrator::new(model, &[step_load()], options).unwrap();
    let n = model.dof_map().n_equations();
    it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    let mut v = Vec::new();
    for _ in 0..steps {
        it.step().unwrap();
        let s = it.state().unwrap();
        v.extend_from_slice(&s.u);
        v.extend_from_slice(&s.v);
        v.extend_from_slice(&s.a);
        v.extend_from_slice(&s.internal_force);
    }
    v.extend(it.state().unwrap().to_bytes().into_iter().map(f64::from));
    v
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

#[test]
fn b0_default_bits_of_the_integrator_and_the_driver() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let f1 = (-(Q / (R * 700.0))).exp();
    let plain = || Material::creep(base(), A_EFF, 3.0, 0.0).unwrap();
    let factor = || Material::creep_arrhenius(base(), A_EFF / f1, 3.0, 0.0, Q).unwrap();
    let dt = 2e-4;
    let r0 = nl_run(&cube(plain()), &NlTransientOptions::new(dt), 60);
    got.push(("nl_transient plain creep cube no T", digest(&r0)));
    let r700 = nl_run(&cube(plain()), &NlTransientOptions::new(dt).temperature(700.0), 60);
    got.push(("nl_transient plain creep cube at 700 K", digest(&r700)));
    let rm5 = nl_run(&cube(plain()), &NlTransientOptions::new(dt).temperature(-5.0), 60);
    got.push(("nl_transient plain creep cube at -5 K", digest(&rm5)));
    let rf = nl_run(&cube(factor()), &NlTransientOptions::new(dt).temperature(700.0), 60);
    got.push(("nl_transient factor creep cube at 700 K", digest(&rf)));
    let loading = TimeLoading::new().load(step_load());
    let times: Vec<f64> = (1..=20).map(|k| k as f64 * 0.01).collect();
    let h = solve_in_time_with(
        &cube(factor()),
        &[],
        &loading,
        &times,
        &TimeSteppingOptions::new(f64::INFINITY).temperature(700.0),
    )
    .unwrap();
    got.push(("solve_in_time_with factor cube at 700 K", digest(&flat_history(&h))));
    let table = LoadTable::new(vec![(0.0, 700.0), (1.0, 750.0)]).unwrap();
    let re = nl_run(
        &cube(base()),
        &NlTransientOptions::new(dt).temperature_history(table),
        30,
    );
    got.push(("nl_transient elastic cube temperature history", digest(&re)));

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
