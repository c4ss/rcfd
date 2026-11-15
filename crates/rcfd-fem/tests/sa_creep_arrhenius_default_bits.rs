//! F-581 (ADR 0558): check N1 (record B0, D5). The creep material without the Arrhenius factor: its point update with and
//! without a temperature in the context, the time-stepping driver with a uniform temperature, a temperature history, a halving
//! step and a load history, and the refusals of the static driver and of an orthotropic base give the bits they gave before
//! F-581. Uses only API that existed at 15f1fee9; the expected digests were printed by this file run on the unchanged base
//! 15f1fee9 (`f581-scratch/b0-base.log`) and written into `EXPECTED` before any library code of F-581.

use rcfd_fem::builders::hex_box;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mat_creep::update;
use rcfd_fem::material::{MaterialAxes, PointContext};
use rcfd_fem::nonlinear::{
    solve, solve_in_time, solve_in_time_with, NewtonOptions, TimeHistory, TimeLoading, TimeSteppingOptions,
};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{Constraint, Load, Material, SolidModel};

/// Digests of 15f1fee9 in the order of the run, from the base run (`f581-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 10]> = Some([
    ("update n 3 m 0 at T", 0x71e8_58bb_3306_3432),
    ("update n 3 m 0 no T", 0x71e8_58bb_3306_3432),
    ("update n 4 m -0.5 at T", 0xc19a_340d_4b1d_5990),
    ("update n 4 m -0.5 no T", 0xc19a_340d_4b1d_5990),
    ("solve_in_time pulled cube at T", 0x2e7a_22d3_1d65_72f2),
    (
        "solve_in_time stretched cube temperature history",
        0x08b3_63fe_721a_8ae8,
    ),
    ("solve_in_time stretched cube halved at T", 0x5e65_b265_90a5_80f2),
    ("solve_in_time_with cube load ramp", 0x7214_ff47_c2c3_f62a),
    ("static refusal", 0x5207_24a8_8cbc_4d6a),
    ("orthotropic refusal", 0x120f_9bb7_2f47_e37e),
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
const EPS0: f64 = 1e-3;

fn base() -> Material {
    Material::isotropic_elastic(E, 0.3, 1200.0, 0.0).unwrap()
}

fn creep(a: f64, n: f64, m: f64) -> Material {
    Material::creep(base(), a, n, m).unwrap()
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

fn stretched(material: Material) -> SolidModel {
    let mut m = cube(material);
    m.add_constraint(Constraint::displacement("xmax", 0, EPS0)).unwrap();
    m
}

fn pulled(material: Material) -> SolidModel {
    let mut m = cube(material);
    m.add_load(Load::traction("xmax", [0.2, 0.0, 0.0])).unwrap();
    m
}

fn relax_a() -> f64 {
    1.0 / (2.0 * E * (E * EPS0).powi(2))
}

fn times(steps: usize, dt: f64) -> Vec<f64> {
    (1..=steps).map(|k| k as f64 * dt).collect()
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
fn n1_default_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();

    // The point update with and without a temperature, two laws.
    let strain = [7.1e-4, -2.3e-4, -3.9e-4, 1.7e-4, -0.8e-4, 2.6e-4];
    let state = [1.1e-4, -0.4e-4, -0.7e-4, 0.3e-4, 0.5e-4, -0.2e-4, 3.0e-4];
    for (name_t, name_n, n, m) in [
        ("update n 3 m 0 at T", "update n 3 m 0 no T", 3.0, 0.0),
        ("update n 4 m -0.5 at T", "update n 4 m -0.5 no T", 4.0, -0.5),
    ] {
        let mat = creep(1e-26, n, m);
        for (name, temp) in [(name_t, Some(700.0)), (name_n, None)] {
            let mut p = PointContext::new().with_time(2.0, 0.5).unwrap();
            if let Some(t) = temp {
                p = p.with_temperature(t).unwrap();
            }
            let u = update(&mat, &strain, &state, &p).unwrap();
            let mut v = u.stress.to_vec();
            v.extend(u.tangent.iter().flatten());
            v.extend_from_slice(&u.state);
            v.push(u.iterations as f64);
            got.push((name, digest(&v)));
        }
    }

    // The driver: a uniform temperature, a temperature history, a halving step at a temperature, a load history.
    let h = solve_in_time(
        &pulled(creep(1.25e-9, 3.0, 0.0)),
        &[],
        &times(20, 0.1),
        &TimeSteppingOptions::new(f64::INFINITY).temperature(700.0),
    )
    .unwrap();
    got.push(("solve_in_time pulled cube at T", digest(&flat_history(&h))));
    let table = LoadTable::new(vec![(0.0, 700.0), (0.1, 700.0), (0.2, 750.0), (1.0, 750.0)]).unwrap();
    let h = solve_in_time(
        &stretched(creep(relax_a(), 3.0, 0.0)),
        &[],
        &times(20, 0.0125),
        &TimeSteppingOptions::new(f64::INFINITY).temperature_history(table),
    )
    .unwrap();
    got.push((
        "solve_in_time stretched cube temperature history",
        digest(&flat_history(&h)),
    ));
    let h = solve_in_time(
        &stretched(creep(relax_a(), 3.0, 0.0)),
        &[],
        &[1.0],
        &TimeSteppingOptions::new(1e-4).temperature(700.0),
    )
    .unwrap();
    println!("B0 halvings of the stretched cube at T: {}", h.steps[1].halvings);
    got.push(("solve_in_time stretched cube halved at T", digest(&flat_history(&h))));
    let pm = pulled(creep(1.25e-9, 3.0, 0.0));
    let pattern = nodal_load_vector(&pm, &[]).unwrap();
    let loading = TimeLoading::new().load(LoadHistory::new(
        pattern,
        LoadTable::new(vec![(0.0, 0.0), (2.0, 2.0)]).unwrap(),
    ));
    let h = solve_in_time_with(
        &cube(creep(1.25e-9, 3.0, 0.0)),
        &[],
        &loading,
        &times(20, 0.1),
        &TimeSteppingOptions::new(f64::INFINITY),
    )
    .unwrap();
    got.push(("solve_in_time_with cube load ramp", digest(&flat_history(&h))));

    // The refusals of the creep material: the static driver, an orthotropic base.
    let r = solve(&stretched(creep(relax_a(), 3.0, 0.0)), &[], &[], &NewtonOptions::new(1));
    let msg = format!("{:?}", r.err());
    println!("B0 static refusal: {msg}");
    got.push(("static refusal", digest_bytes(msg.bytes())));
    let ortho = Material::orthotropic(
        [1.4e11, 1.0e10, 1.0e10],
        [0.3, 0.3, 0.45],
        [5.0e9, 5.0e9, 3.5e9],
        1600.0,
        [0.0; 3],
        MaterialAxes::fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap(),
    )
    .unwrap();
    let oc = Material::creep(ortho, 1e-26, 3.0, 0.0).unwrap();
    let r = update(&oc, &strain, &state, &PointContext::new().with_time(2.0, 0.5).unwrap());
    let msg = format!("{:?}", r.err());
    println!("B0 orthotropic refusal: {msg}");
    got.push(("orthotropic refusal", digest_bytes(msg.bytes())));

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
