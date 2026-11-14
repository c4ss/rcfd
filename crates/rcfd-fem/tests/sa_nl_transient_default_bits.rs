//! F-423 (ADR 0479): check B1 (record B0, D7), the nonlinear static path, its contribution and halving paths, the J2 path
//! of F-422, the linear transient integrator of F-398 and the consistent mass give the bits they gave before F-423: a
//! stretched Hex8 bar through `nonlinear::solve`, `solve_with` with a spring contribution of non-zero force and tangent,
//! `solve_with` with a zero contribution that refuses one call (the halving path), a J2 bar through `nonlinear::solve`
//! (states included), `transient::Integrator` of bar B under the A5 load (Newmark; generalised-alpha 0.8 with damping) and
//! `mass::consistent` of bar B. Uses only API that existed at 23d6b8e6; the expected digests were printed by this file run
//! on the unchanged base 23d6b8e6 (`f423-scratch/b0-base.log`) and written into `EXPECTED` before any library code of F-423.

use rcfd_fem::builders::hex_box;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nonlinear::{solve, solve_with, Contribution, DriverExtensions, NewtonOptions, NonlinearResult};
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, Method, Rayleigh, TransientOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::cell::Cell;

/// Digests of 23d6b8e6 in the order of the run, from the base run (`f423-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 7]> = Some([
    ("bar Hex8 solve", 0x3068_3d9e_fc11_14e6),
    ("solve_with spring", 0x14b3_b8ae_bac6_dec7),
    ("solve_with halving", 0xeb70_2f70_cab2_3dcf),
    ("J2 bar solve", 0xebb0_7433_b75f_b8d2),
    ("transient Newmark", 0x77ba_a467_14ec_2a02),
    ("transient generalised-alpha damped", 0x6047_3cd7_5d1f_61bb),
    ("consistent mass bar B", 0x808f_f368_b7e2_560c),
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

/// Every value of a nonlinear result, in a fixed order: displacement, reactions, then per increment the load factor, the
/// step, the halvings, the residuals and the energies, then the state values.
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

/// A bar `[1, 0.2, 0.2]` of `[3, 1, 1]` Hex8, three symmetry planes; stretched by `u_x = 2e-3` at `xmax` if `stretched`.
fn bar(stretched: bool) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    if stretched {
        m.add_constraint(Constraint::displacement("xmax", 0, 2e-3)).unwrap();
    }
    m
}

/// A linear spring `k u` on the `x` equations of the `xmax` nodes.
struct Spring {
    equations: Vec<usize>,
    k: f64,
}

impl Contribution for Spring {
    fn name(&self) -> &str {
        "spring"
    }
    fn add(
        &self,
        u: &[f64],
        _load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        for &q in &self.equations {
            force[q] += self.k * u[q];
            tangent.push((q, q, self.k));
        }
        Ok(())
    }
}

/// A zero contribution that refuses its call number `fail_at` (1-based).
struct FailOnce {
    calls: Cell<usize>,
    fail_at: usize,
}

impl Contribution for FailOnce {
    fn name(&self) -> &str {
        "fail once"
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
        if n == self.fail_at {
            return Err(FemError::InvalidValue {
                key: "fail once",
                reason: "refused once".to_string(),
            });
        }
        Ok(())
    }
}

/// Bar B of ADR 0389 D5 with its mass-normalised first mode and the A5 pattern `M phi_1`.
fn bar_b() -> (SolidModel, Vec<f64>, f64) {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    let modal = modal_analysis(&m, &ModalOptions::new(2)).unwrap();
    let phi = modal.modes[0].clone();
    let mass = consistent(&m).unwrap();
    let mut pattern = vec![0.0; phi.len()];
    mass.matvec(&phi, &mut pattern);
    let omega = modal.eigenvalues[0].sqrt();
    (m, pattern, omega)
}

fn transient_bytes(model: &SolidModel, pattern: &[f64], omega: f64, method: Method, damped: bool) -> Vec<u8> {
    let period = 2.0 * std::f64::consts::PI / omega;
    let table = LoadTable::new(vec![(0.0, 1.0), (10.0 * period, 1.0)]).unwrap();
    let mut o = TransientOptions::new(period / 40.0).method(method);
    if damped {
        o = o.damping(Rayleigh::from_ratio(0.02, omega, omega).unwrap());
    }
    let mut it = Integrator::new(model, &[LoadHistory::new(pattern.to_vec(), table)], &o).unwrap();
    let zero = vec![0.0; pattern.len()];
    it.start(0.0, &zero, &zero).unwrap();
    it.run(20).unwrap();
    it.state().unwrap().to_bytes()
}

fn csr_values(a: &CsrMatrix) -> Vec<f64> {
    let mut v: Vec<f64> = a.row_ptr.iter().map(|&x| x as f64).collect();
    v.extend(a.col.iter().map(|&x| x as f64));
    v.extend_from_slice(&a.val);
    v
}

#[test]
fn b1_default_bits_of_the_base() {
    let mut got: Vec<(&str, u64)> = Vec::new();

    let r = solve(&bar(true), &[], &[], &NewtonOptions::new(2)).unwrap();
    got.push(("bar Hex8 solve", digest(&flat(&r))));

    let mut model = bar(false);
    model.add_load(Load::traction("xmax", [1.0e8, 0.0, 0.0])).unwrap();
    let dofs = model.dof_map();
    let equations = model
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| dofs.equation(n, 0).unwrap())
        .collect();
    let spring = Spring { equations, k: 1.0e9 };
    let ext = DriverExtensions {
        contributions: vec![&spring],
        step_control: None,
    };
    let r = solve_with(&model, &[], &[], &NewtonOptions::new(2), ext).unwrap();
    got.push(("solve_with spring", digest(&flat(&r))));

    let fail = FailOnce {
        calls: Cell::new(0),
        fail_at: 3,
    };
    let ext = DriverExtensions {
        contributions: vec![&fail],
        step_control: None,
    };
    let r = solve_with(&bar(true), &[], &[], &NewtonOptions::new(1), ext).unwrap();
    got.push(("solve_with halving", digest(&flat(&r))));

    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [1, 1, 1]).unwrap();
    let mut j2 = SolidModel::new(mesh);
    j2.add_material("all", Material::j2(steel(), 2.0e8, 2.0e9, 1.0e9).unwrap())
        .unwrap();
    j2.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    j2.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    j2.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    j2.add_constraint(Constraint::displacement("xmax", 0, 5e-3)).unwrap();
    let r = solve(&j2, &[], &[], &NewtonOptions::new(10)).unwrap();
    got.push(("J2 bar solve", digest(&flat(&r))));

    let (b, pattern, omega) = bar_b();
    let bytes = transient_bytes(&b, &pattern, omega, Method::average_acceleration(), false);
    got.push(("transient Newmark", digest_bytes(bytes)));
    let bytes = transient_bytes(&b, &pattern, omega, Method::GeneralisedAlpha { rho_inf: 0.8 }, true);
    got.push(("transient generalised-alpha damped", digest_bytes(bytes)));
    got.push(("consistent mass bar B", digest(&csr_values(&consistent(&b).unwrap()))));

    for (name, d) in &got {
        println!("B0 {name}: 0x{d:016x}");
    }
    match EXPECTED {
        None => panic!("no digests recorded yet: run on the base and write them into EXPECTED"),
        Some(expected) => {
            let mut missed = Vec::new();
            for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
                assert_eq!(name, en, "the order of the record");
                let ok = d == ed;
                println!("B1 {name}: {}", if ok { "met" } else { "MISSED" });
                if !ok {
                    missed.push(*name);
                }
            }
            assert!(missed.is_empty(), "digests changed: {missed:?}");
        }
    }
}
