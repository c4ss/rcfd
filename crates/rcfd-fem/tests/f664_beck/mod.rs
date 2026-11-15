//! Beck's column of ADR 0580 D5 for F-664 (ADR 0587): a copy of the set-up and of the bisection of `sa_f653.rs` (that file is
//! not edited), with every evaluation kept.

#![allow(dead_code)]

use rcfd_fem::builders::hex_box;
use rcfd_fem::follower::{tangent_modes_full, ComplexModes, FollowerPressure};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::nonlinear::{initial_states, Contribution};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};

pub const E: f64 = 2.0e11;
pub const RHO: f64 = 7850.0;
pub const L: f64 = 1.0;
pub const B: f64 = 0.04;
pub const H: f64 = 0.02;

pub fn ei() -> f64 {
    E * B * H * H * H / 12.0
}

pub fn p_ref() -> f64 {
    20.0 * ei() / (L * L * B * H)
}

fn beck() -> SolidModel {
    let base = hex_box([0.0; 3], [L, B, H], [50, 1, 1]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

pub struct Beck {
    pub model: SolidModel,
    pub dead: SolidModel,
    pub ulin: Vec<f64>,
    pub pre: Prestress,
}

pub fn setup() -> Beck {
    let model = beck();
    let mut dead = beck();
    dead.add_load(Load::pressure("xmax", p_ref())).unwrap();
    let s = solve_static(&dead, &StaticOptions::default()).unwrap();
    let ulin = s.displacement_values().to_vec();
    let pre = Prestress::from_static(&dead, &s).unwrap();
    Beck { model, dead, ulin, pre }
}

/// The state `u = (p/20) u_lin` and the load factor `p/20`.
pub fn state(b: &Beck, p: f64) -> (Vec<f64>, f64) {
    let lf = p / 20.0;
    (b.ulin.iter().map(|x| lf * x).collect(), lf)
}

pub fn modes_at(b: &Beck, p: f64, contributions: &[&dyn Contribution], count: usize) -> ComplexModes {
    let (u, lf) = state(b, p);
    let states = initial_states(&b.model).unwrap();
    tangent_modes_full(&b.model, &[], &u, &states, lf, contributions, &ModalOptions::new(count)).unwrap()
}

pub fn is_flutter(r: &ComplexModes) -> bool {
    r.eigenvalues.iter().any(|v| v.im.abs() > 1e-6 * v.abs())
}

/// The bisection of ADR 0580 D5 (`p = 16`, `24`, then 30 halvings while the bracket holds): every `(p, result)` in order.
pub fn bisection(b: &Beck, fp: &FollowerPressure) -> Vec<(f64, ComplexModes)> {
    let list: Vec<&dyn Contribution> = vec![fp];
    let mut out = Vec::new();
    let lo_r = modes_at(b, 16.0, &list, 6);
    let hi_r = modes_at(b, 24.0, &list, 6);
    let bracket = !is_flutter(&lo_r) && is_flutter(&hi_r);
    out.push((16.0, lo_r));
    out.push((24.0, hi_r));
    if bracket {
        let (mut lo, mut hi) = (16.0f64, 24.0f64);
        for _ in 0..30 {
            let mid = 0.5 * (lo + hi);
            let r = modes_at(b, mid, &list, 6);
            if is_flutter(&r) {
                hi = mid;
            } else {
                lo = mid;
            }
            out.push((mid, r));
        }
    }
    out
}
