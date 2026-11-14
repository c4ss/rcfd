//! F-427 (ADR 0507): check B0 (record of D9), the paths next to the contact module give the bits they gave before F-427:
//! `solve_static` of a Hex8 bar under an end traction, `nonlinear::solve` of a stretched Hex8 bar, `nonlinear::solve_from` of
//! that bar with a zero contribution, `solve_static` of two blocks side by side by `tie::assemble_parts` with a pressure on one,
//! and the integration-point field of that solve. Uses only API that existed at 031b09c6; the expected digests were printed by
//! this file run on the unchanged base 031b09c6 (`f427-scratch/b0-base.log`) and written into `EXPECTED` before any library
//! code of F-427.

use rcfd_fem::builders::hex_box;
use rcfd_fem::nonlinear::{initial_states, solve, solve_from, Contribution, NewtonOptions, NonlinearResult};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::tie::assemble_parts;
use rcfd_fem::{Constraint, Load, Material, SolidModel, SolidResult};

/// Digests of 031b09c6 in the order of the run, from the base run (`f427-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 6]> = Some([
    ("static bar", 0x8ac6_856b_3ecf_ca75),
    ("nonlinear bar", 0xb264_aa60_c51a_6114),
    ("solve_from zero contribution", 0x3c4c_bedd_12fd_3c3f),
    ("static two parts", 0x806b_2e95_7dde_59f2),
    ("field two parts", 0x195a_1a03_53de_289d),
    ("count", 0x0000_0000_0000_0005),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn flat_static(r: &SolidResult) -> Vec<f64> {
    let mut v = r.displacement_values().to_vec();
    if let Some(re) = r.reactions() {
        v.extend_from_slice(re);
    }
    v
}

fn flat(r: &NonlinearResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap());
    for s in &r.steps {
        v.push(s.load_factor);
        v.extend_from_slice(&s.residuals);
        v.extend_from_slice(&s.energies);
    }
    v
}

/// A bar `[1, 0.1, 0.1]` of `[4, 1, 1]` Hex8 cells on three symmetry planes.
fn bar_base() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [4, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

struct Zero;

impl Contribution for Zero {
    fn name(&self) -> &str {
        "zero"
    }
    fn add(
        &self,
        _u: &[f64],
        _load_factor: f64,
        _force: &mut [f64],
        _tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        Ok(())
    }
}

#[test]
fn b0_default_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let mut m = bar_base();
    m.add_load(Load::traction("xmax", [2.0e9, 0.0, 0.0])).unwrap();
    let r = solve_static(&m, &StaticOptions::default()).unwrap();
    got.push(("static bar", digest(&flat_static(&r))));
    let mut m = bar_base();
    m.add_constraint(Constraint::displacement("xmax", 0, 3e-3)).unwrap();
    let r = solve(&m, &[], &[], &NewtonOptions::new(2)).unwrap();
    got.push(("nonlinear bar", digest(&flat(&r))));
    // solve_from of the traction bar from zero (no prescribed value differs from zero) with a zero contribution.
    let mut m = bar_base();
    m.add_load(Load::traction("xmax", [2.0e9, 0.0, 0.0])).unwrap();
    let n = m.dof_map().n_equations();
    let load = rcfd_fem::loads::nodal_load_vector(&m, &[]).unwrap();
    let states = initial_states(&m).unwrap();
    let e = solve_from(&m, &[], &load, &vec![0.0; n], &states, &NewtonOptions::new(1), &[&Zero]).unwrap();
    let mut v = e.u.clone();
    v.extend_from_slice(&e.internal_force);
    v.extend_from_slice(&e.residuals);
    v.extend_from_slice(&e.energies);
    got.push(("solve_from zero contribution", digest(&v)));
    // Two blocks side by side, each supported, a pressure on the second.
    let a = hex_box([0.0; 3], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let b = hex_box([0.0, 0.0, 1.5], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let mesh = assemble_parts(&[("lower", &a), ("upper", &b)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("lower.all", steel()).unwrap();
    m.add_material("upper.all", steel()).unwrap();
    for p in ["lower", "upper"] {
        m.add_constraint(Constraint::displacement(&format!("{p}.xmin"), 0, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement(&format!("{p}.ymin"), 1, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement(&format!("{p}.zmin"), 2, 0.0)).unwrap();
    }
    m.add_load(Load::pressure("upper.zmax", 1.0e8)).unwrap();
    let r = solve_static(&m, &StaticOptions::default()).unwrap();
    got.push(("static two parts", digest(&flat_static(&r))));
    let f = IntegrationPointField::compute(&m, r.displacement_values()).unwrap();
    let mut v: Vec<f64> = f.stress().iter().flatten().copied().collect();
    v.extend(f.positions().iter().flatten().copied());
    v.extend_from_slice(f.weights());
    got.push(("field two parts", digest(&v)));
    got.push(("count", got.len() as u64));
    for (name, d) in &got {
        println!(
            "    (\"{name}\", 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            d >> 48,
            (d >> 32) & 0xffff,
            (d >> 16) & 0xffff,
            d & 0xffff
        );
    }
    match EXPECTED {
        None => panic!("no expected digests yet: print them on the base"),
        Some(expected) => {
            let mut bad = Vec::new();
            for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
                if name != en || d != ed {
                    bad.push(format!("{name}: 0x{d:016x} != 0x{ed:016x}"));
                }
            }
            assert!(got.len() == expected.len() && bad.is_empty(), "B0 changed: {bad:?}");
        }
    }
}
