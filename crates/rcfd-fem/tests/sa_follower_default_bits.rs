//! F-424 (ADR 0480): check N1 (record of D8), the Newton driver without a load-factor control and the buckling factors
//! without extra matrices give the bits they gave before F-424: `nonlinear::solve` of a stretched Hex8 bar and of a Hex20
//! cantilever, `solve_with` with a zero contribution that refuses one call (the halving path) and with a step-control hook,
//! `solve` of a block under a dead pressure, `buckling::factors` of a Hex20 column, the assembled force and tangent of a
//! distorted Tet10 model, and the error of `solve_with` when the halvings are exhausted. Uses only API that existed at
//! 4c422025; the expected digests were printed by this file run on the unchanged base 4c422025
//! (`f424-scratch/n1-base.log`) and written into `EXPECTED` before any library code of F-424.

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::nonlinear::{
    initial_states, internal_force, solve, solve_with, tangent, Contribution, DriverExtensions, LoadStep,
    NewtonOptions, NonlinearResult, StepControl,
};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel};
use std::cell::Cell;

/// Digests of 4c422025 in the order of the run, from the base run (`f424-scratch/n1-base.log`).
const EXPECTED: Option<[(&str, u64); 9]> = Some([
    ("bar Hex8", 0x3068_3d9e_fc11_14e6),
    ("cantilever Hex20", 0x8b36_a7da_6105_2e68),
    ("solve_with halving", 0xeb70_2f70_cab2_3dcf),
    ("solve_with step control", 0x62e4_4e54_6bfd_afc0),
    ("block dead pressure", 0x6859_bf9a_1859_39f8),
    ("buckling column Hex20", 0xbd5c_f14b_c1a6_ec1e),
    ("Tet10 force and tangent", 0x9de7_cf3d_6210_19b2),
    ("halvings exhausted", 0xf081_6095_8677_f427),
    ("count", 0x0000_0000_0000_0008),
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

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

/// Every value of a nonlinear result, in a fixed order.
fn flat(r: &NonlinearResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap());
    for s in &r.steps {
        v.push(s.load_factor);
        v.push(s.step as f64);
        v.push(s.halvings as f64);
        v.extend_from_slice(&s.residuals);
        v.extend_from_slice(&s.energies);
    }
    for s in &r.states {
        v.push(s.values().len() as f64);
        v.extend_from_slice(s.values());
    }
    v
}

/// A bar `[1, 0.2, 0.2]` of `[3, 1, 1]` Hex8 cells, three symmetry planes, stretched by `u_x = 2e-3` at `xmax`.
fn bar() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmax", 0, 2e-3)).unwrap();
    m
}

/// A Hex20 cantilever `[1, 0.05, 0.05]` of `[10, 1, 1]` cells, clamped at `xmin`, a traction along `y` at `xmax`.
fn cantilever() -> SolidModel {
    let mesh = upgrade_to_hex20(&hex_box([0.0; 3], [1.0, 0.05, 0.05], [10, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 2.0e6, 0.0])).unwrap();
    m
}

/// A Hex8 block `[0.2, 0.1, 0.1]` of `[2, 1, 1]` cells on `xmin` (held in x, y, z by symmetry planes), a dead pressure on
/// `xmax`.
fn block() -> SolidModel {
    let mesh = hex_box([0.0; 3], [0.2, 0.1, 0.1], [2, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_load(Load::pressure("xmax", 3.0e8)).unwrap();
    m
}

/// A Hex20 column `[1, 0.05, 0.05]` of `[8, 1, 1]` cells, clamped at `xmin`, the compression `1e6` Pa at `xmax`.
fn column() -> SolidModel {
    let mesh = upgrade_to_hex20(&hex_box([0.0; 3], [1.0, 0.05, 0.05], [8, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [-1.0e6, 0.0, 0.0])).unwrap();
    m
}

/// A zero contribution that refuses its call number `fail_at` (1-based), or every call (`fail_at = Some(0)`).
struct Fail {
    calls: Cell<usize>,
    fail_at: Option<usize>,
}

impl Contribution for Fail {
    fn name(&self) -> &str {
        "fail"
    }
    fn add(
        &self,
        _u: &[f64],
        _load_factor: f64,
        _force: &mut [f64],
        _tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        let n = self.calls.get() + 1;
        self.calls.set(n);
        if self.fail_at == Some(0) || Some(n) == self.fail_at {
            return Err(FemError::InvalidValue {
                key: "fail",
                reason: "registered refusal".to_string(),
            });
        }
        Ok(())
    }
}

/// A hook that takes 0.6 of the proposed increment at the first attempt of each load step and the proposal after it.
struct Shorter {
    last_step: usize,
}

impl StepControl for Shorter {
    fn name(&self) -> &str {
        "shorter"
    }
    fn load_factor(&mut self, committed: f64, proposed: f64, step: usize, _halvings: usize) -> rcfd_fem::Result<f64> {
        if step != self.last_step {
            self.last_step = step;
            return Ok(committed + 0.6 * (proposed - committed));
        }
        Ok(proposed)
    }
    fn converged(&mut self, _increment: &LoadStep) {}
}

/// SplitMix64, a deterministic source only.
fn uniform(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

#[test]
fn n1_default_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let r = solve(&bar(), &[], &[], &NewtonOptions::new(2)).unwrap();
    got.push(("bar Hex8", digest(&flat(&r))));
    let r = solve(&cantilever(), &[], &[], &NewtonOptions::new(2)).unwrap();
    got.push(("cantilever Hex20", digest(&flat(&r))));
    let fail = Fail {
        calls: Cell::new(0),
        fail_at: Some(3),
    };
    let ext = DriverExtensions {
        contributions: vec![&fail],
        step_control: None,
    };
    let r = solve_with(&bar(), &[], &[], &NewtonOptions::new(1), ext).unwrap();
    got.push(("solve_with halving", digest(&flat(&r))));
    let mut hook = Shorter { last_step: 0 };
    let ext = DriverExtensions {
        contributions: Vec::new(),
        step_control: Some(&mut hook),
    };
    let r = solve_with(&cantilever(), &[], &[], &NewtonOptions::new(2), ext).unwrap();
    println!("N1 hook increments {}", r.steps.len());
    got.push(("solve_with step control", digest(&flat(&r))));
    let r = solve(&block(), &[], &[], &NewtonOptions::new(2)).unwrap();
    got.push(("block dead pressure", digest(&flat(&r))));
    let c = column();
    let s = solve_static(&c, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&c, &s).unwrap();
    let b = factors(&c, &pre, &BucklingOptions::new(2)).unwrap();
    let mut v = b.factors.clone();
    for m in &b.modes {
        v.extend_from_slice(m);
    }
    v.extend_from_slice(&b.residuals);
    v.push(b.iterations as f64);
    got.push(("buckling column Hex20", digest(&v)));
    // Assembled force and tangent of a distorted Tet10 model.
    let mut mesh = upgrade_to_tet10(&tet_box([0.0; 3], [1.0, 0.5, 0.4], [2, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut seed = 0xF424_0480_u64;
    let nodes: Vec<Vec3> = mesh
        .nodes()
        .iter()
        .map(|p| {
            let mut q = *p;
            for v in q.iter_mut() {
                *v += 0.02 * (2.0 * uniform(&mut seed) - 1.0);
            }
            q
        })
        .collect();
    mesh = {
        let mut m2 = rcfd_fem::SolidMesh::new(nodes).unwrap();
        for b in mesh.blocks() {
            m2.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
        }
        let ne = m2.n_elements();
        m2.add_element_set("all", (0..ne).collect()).unwrap();
        m2
    };
    let mut t = SolidModel::new(mesh);
    t.add_material("all", steel()).unwrap();
    let n = t.dof_map().n_equations();
    let u: Vec<f64> = (0..n).map(|_| 1e-3 * (2.0 * uniform(&mut seed) - 1.0)).collect();
    let states = initial_states(&t).unwrap();
    let mut ft = internal_force(&t, &u, &states).unwrap();
    ft.extend_from_slice(&tangent(&t, &u, &states, Some(3)).unwrap().val);
    got.push(("Tet10 force and tangent", digest(&ft)));
    // The error when the halvings are exhausted.
    let always = Fail {
        calls: Cell::new(0),
        fail_at: Some(0),
    };
    let ext = DriverExtensions {
        contributions: vec![&always],
        step_control: None,
    };
    let e = solve_with(&bar(), &[], &[], &NewtonOptions::new(1), ext)
        .unwrap_err()
        .to_string();
    println!("N1 error: {e}");
    got.push(("halvings exhausted", digest_bytes(e.into_bytes())));
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
            assert!(got.len() == expected.len() && bad.is_empty(), "N1 changed: {bad:?}");
        }
    }
}
