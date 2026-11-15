//! Shared set-ups of the F-617 files (ADR 0539): the models, fields and runs of ADR 0534 (FE1, FE2, HB1, the beam of CONV,
//! the bar of V3T), written against the API that exists at 8ebc67e8 (no predictor call here).
#![allow(dead_code)]

use rcfd_fem::builders::hex_box;
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{NewtonOptions, TimeHistory};
use rcfd_fem::thermal::TemperatureFieldHistory;
use rcfd_fem::{Constraint, Material, SolidMesh, SolidModel};

pub const E_S: f64 = 2.0e11;
pub const NU_S: f64 = 0.3;
pub const ALPHA: f64 = 1e-4;
pub const REF: f64 = 300.0;
pub const DT: f64 = 100.0;

/// A list of measured values against their limits; a non-finite value is a miss, the test fails at the end if any missed.
pub struct Checks(pub Vec<String>);

impl Checks {
    pub fn new() -> Checks {
        Checks(Vec::new())
    }
    pub fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    pub fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    pub fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    pub fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// `max` that keeps a NaN.
pub fn worse(w: &mut f64, v: f64) {
    if v.is_nan() || v > *w {
        *w = v;
    }
}

pub fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
pub fn digest(v: &[f64]) -> u64 {
    digest_bytes(v.iter().flat_map(|x| x.to_bits().to_le_bytes()))
}

pub fn digest_bytes<I: IntoIterator<Item = u8>>(bytes: I) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

pub fn tight() -> NewtonOptions {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-13;
    o.energy_tolerance = 1.0;
    o
}

/// `r_b = 2 frac(0.618034 b) - 1`.
pub fn r(b: usize) -> f64 {
    2.0 * (0.618034 * b as f64).fract() - 1.0
}

pub fn steel(alpha: f64) -> Material {
    Material::isotropic_elastic(E_S, NU_S, 7850.0, alpha).unwrap()
}

pub fn node_at(mesh: &SolidMesh, x: [f64; 3]) -> usize {
    mesh.nodes()
        .iter()
        .position(|p| (0..3).all(|i| (p[i] - x[i]).abs() < 1e-12))
        .expect("a node at the point")
}

/// The 3-2-1 support at the corners A = 0, B = (lx, 0, 0), C = (0, ly, 0).
pub fn three_two_one(mut mesh: SolidMesh, lx: f64, ly: f64, material: Material) -> SolidModel {
    let a = node_at(&mesh, [0.0, 0.0, 0.0]);
    let b = node_at(&mesh, [lx, 0.0, 0.0]);
    let c = node_at(&mesh, [0.0, ly, 0.0]);
    mesh.add_node_set("A", vec![a]).unwrap();
    mesh.add_node_set("B", vec![b]).unwrap();
    mesh.add_node_set("C", vec![c]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("A", comp, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("B", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("B", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("C", 2, 0.0)).unwrap();
    m
}

pub fn free_cube(material: Material) -> SolidModel {
    three_two_one(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap(), 1.0, 1.0, material)
}

/// The cube held on all six faces (HB1).
pub fn held_cube(material: Material) -> SolidModel {
    let mut held = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap());
    held.add_material("all", material).unwrap();
    for (set, comp) in [
        ("xmin", 0),
        ("xmax", 0),
        ("ymin", 1),
        ("ymax", 1),
        ("zmin", 2),
        ("zmax", 2),
    ] {
        held.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
    }
    held
}

/// The beam of CONV: `[2, 0.5, 0.5]` of `8 x 2 x 2` Hex8 held 3-2-1.
pub fn beam(alpha: f64) -> SolidModel {
    three_two_one(
        hex_box([0.0; 3], [2.0, 0.5, 0.5], [8, 2, 2]).unwrap(),
        2.0,
        0.5,
        steel(alpha),
    )
}

/// The bar of V3T: `[1, 0.2, 0.2]` of `[3, 1, 1]` Hex8, three symmetry planes.
pub fn bar(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

pub fn uniform(model: &SolidModel, t: f64) -> Vec<f64> {
    vec![t; model.mesh().nodes().len()]
}

pub fn inf_norm(v: &[f64]) -> f64 {
    let mut w: f64 = 0.0;
    for x in v {
        worse(&mut w, x.abs());
    }
    w
}

/// `max |u_i - a (X_i - X_A,i)|` over the nodes and components (`X_A = 0`, the 3-2-1 cube).
pub fn expansion_error(model: &SolidModel, u: &[f64], a: f64) -> f64 {
    let dofs = model.dof_map();
    let mut w: f64 = 0.0;
    for (node, x) in model.mesh().nodes().iter().enumerate() {
        for c in 0..3 {
            let q = dofs.equation(node, c).unwrap();
            worse(&mut w, (u[q] - a * x[c]).abs());
        }
    }
    w
}

pub fn flat_history(h: &TimeHistory) -> Vec<f64> {
    let mut v = Vec::new();
    for s in &h.steps {
        v.extend_from_slice(&s.u);
        v.extend_from_slice(&s.internal_force);
        for e in &s.states {
            v.push(e.values().len() as f64);
            v.extend_from_slice(e.values());
        }
        v.push(s.halvings as f64);
        v.push(s.substeps as f64);
        v.push(s.iterations as f64);
    }
    v
}

/// The fields of FE1 (step held), FE2 (ramp), the hot start.
pub fn step_field(model: &SolidModel) -> TemperatureFieldHistory {
    TemperatureFieldHistory::new(
        REF,
        vec![
            (0.0, uniform(model, REF)),
            (1.0, uniform(model, REF + DT)),
            (3.0, uniform(model, REF + DT)),
        ],
    )
    .unwrap()
}

pub fn ramp_field(model: &SolidModel) -> TemperatureFieldHistory {
    TemperatureFieldHistory::new(REF, vec![(0.0, uniform(model, REF)), (4.0, uniform(model, REF + DT))]).unwrap()
}

pub fn hot_field(model: &SolidModel) -> TemperatureFieldHistory {
    TemperatureFieldHistory::new(
        REF,
        vec![(0.0, uniform(model, REF + DT)), (2.0, uniform(model, REF + DT))],
    )
    .unwrap()
}

/// The nonuniform ramp of CONV over three steps (`t = 0, 3`): `T_b = T_ref + 50 r_b t / 3`.
pub fn beam_field(model: &SolidModel) -> TemperatureFieldHistory {
    let nn = model.mesh().nodes().len();
    TemperatureFieldHistory::new(
        REF,
        vec![
            (0.0, vec![REF; nn]),
            (3.0, (0..nn).map(|b| REF + 50.0 * r(b)).collect()),
        ],
    )
    .unwrap()
}

/// A run of the integrator: `u` and the bits of `u`, `v`, `a`, the internal force at every step end, the largest halving
/// count, the corrections, the state bytes at the end.
#[derive(Default)]
pub struct Run {
    pub u: Vec<Vec<f64>>,
    pub bits: Vec<Vec<u64>>,
    pub flat: Vec<f64>,
    pub halvings: usize,
    pub corrections: Vec<usize>,
    pub bytes: Vec<u8>,
}

pub fn advance(it: &mut NlIntegrator, steps: usize, run: &mut Run) {
    for _ in 0..steps {
        let r = it.step().unwrap();
        run.halvings = run.halvings.max(r.halvings);
        run.corrections.push(r.iterations);
        let st = it.state().unwrap();
        let mut v = st.u.clone();
        v.extend_from_slice(&st.v);
        v.extend_from_slice(&st.a);
        v.extend_from_slice(&st.internal_force);
        run.bits.push(bits(&v));
        run.flat.extend_from_slice(&v);
        run.u.push(st.u);
    }
    run.bytes = it.state().unwrap().to_bytes();
}

pub fn vibrate(model: &SolidModel, options: &NlTransientOptions, u0: &[f64], v0: &[f64], steps: usize) -> Run {
    let mut it = NlIntegrator::new(model, &[], options).unwrap();
    it.start(0.0, u0, v0).unwrap();
    let mut run = Run::default();
    advance(&mut it, steps, &mut run);
    run
}

/// The nonuniform fields of V3T.
pub fn v3t_field(b: &SolidModel, dt: f64) -> TemperatureFieldHistory {
    let nn = b.mesh().nodes().len();
    let f1: Vec<f64> = (0..nn).map(|k| REF + 50.0 + 20.0 * r(k)).collect();
    let f2: Vec<f64> = (0..nn).map(|k| REF + 120.0 - 30.0 * r(k)).collect();
    TemperatureFieldHistory::new(
        REF,
        vec![
            (0.0, f1.clone()),
            (20.0 * dt, f1),
            (21.0 * dt, f2.clone()),
            (80.0 * dt, f2),
        ],
    )
    .unwrap()
}

/// The initial velocity of the xmax face of V3T.
pub fn v3t_velocity(b: &SolidModel) -> Vec<f64> {
    let mut vb = vec![0.0; b.dof_map().n_equations()];
    for &nd in b.mesh().node_set("xmax").unwrap() {
        vb[b.dof_map().equation(nd, 0).unwrap()] = 40.0;
    }
    vb
}

/// The free expansion `a (X - X_A)` (`X_A = 0`) of the 3-2-1 cube: the displacement, or with `a` a rate the velocity.
pub fn expanded(model: &SolidModel, a: f64) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    for (node, x) in model.mesh().nodes().iter().enumerate() {
        for comp in 0..3 {
            u[dofs.equation(node, comp).unwrap()] = a * x[comp];
        }
    }
    u
}
