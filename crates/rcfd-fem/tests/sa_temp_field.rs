//! F-613 (ADR 0534): a nodal temperature field against time on the quasi-static driver and the nonlinear transient
//! integrator, with the multiplicative thermal strain of the material's expansion. Checks FE1, FE1s, FE1c (free expansion of
//! an unconstrained cube under a temperature step), FE2 (the step-end rule under a ramp), HB1, HB1u (the held cube at finite
//! strain), TG1 (the consistent tangent), LIN-h, LIN-t, LINo (the small-strain limit against the linear thermal path), UB1,
//! NCU, UB2 (a uniform field gives the bits of ADR 0526 without expansion), FEi, V3T, NC3 (the integrator) and RF4
//! (refusals) of the registration. Every aggregated error is required finite.

use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::mat_visco::shift_factor;
use rcfd_fem::material::{PointContext, PronyTerm, TimeTemperatureShift};
use rcfd_fem::mesh::SolidFormulation;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{self, solve_in_time_with, NewtonOptions, TimeHistory, TimeLoading, TimeSteppingOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::thermal::TemperatureFieldHistory;
use rcfd_fem::transient::{LoadTable, MotionHistory, Rayleigh};
use rcfd_fem::{BlockProperty, Constraint, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// A list of measured values against their limits; every value is printed, a non-finite value is a miss, the test fails
/// at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn within(&mut self, name: &str, value: f64, lo: f64, hi: f64) {
        let ok = value.is_finite() && value >= lo && value <= hi;
        println!(
            "{name}: {value:.6e} (limit in [{lo:e}, {hi:e}]) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not in [{lo:e}, {hi:e}]"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// `max` that keeps a NaN (b9).
fn worse(w: &mut f64, v: f64) {
    if v.is_nan() || v > *w {
        *w = v;
    }
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. }
        | FemError::FieldLength { key, .. }
        | FemError::InvalidMaterial { key, .. } => Some(key),
        FemError::Element { source, .. } => key_of(source),
        _ => None,
    }
}

fn refused_with<T>(r: rcfd_fem::Result<T>, key: &str) -> (bool, String) {
    match r {
        Ok(_) => (false, "accepted".to_string()),
        Err(e) => (key_of(&e) == Some(key), format!("{e}")),
    }
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; x.len()];
    m.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn tight() -> NewtonOptions {
    let mut o = NewtonOptions::new(1);
    o.residual_tolerance = 1e-13;
    o.energy_tolerance = 1.0;
    o
}

fn flat_history(h: &TimeHistory) -> Vec<Vec<u64>> {
    h.steps
        .iter()
        .map(|s| {
            let mut v = s.u.clone();
            v.extend_from_slice(&s.internal_force);
            for e in &s.states {
                v.push(e.values().len() as f64);
                v.extend_from_slice(e.values());
            }
            v.push(s.halvings as f64);
            v.push(s.substeps as f64);
            bits(&v)
        })
        .collect()
}

/// `r_b = 2 frac(0.618034 b) - 1` (arithmetic 9).
fn r(b: usize) -> f64 {
    2.0 * (0.618034 * b as f64).fract() - 1.0
}

// ------------------------------------------------------------------------------------------------- the elastic cube

const E_S: f64 = 2.0e11;
const NU_S: f64 = 0.3;
const ALPHA: f64 = 1e-4;
const REF: f64 = 300.0;
const DT: f64 = 100.0;

fn steel(alpha: f64) -> Material {
    Material::isotropic_elastic(E_S, NU_S, 7850.0, alpha).unwrap()
}

/// The node of `mesh` at `x` (exact within 1e-12).
fn node_at(mesh: &SolidMesh, x: [f64; 3]) -> usize {
    mesh.nodes()
        .iter()
        .position(|p| (0..3).all(|i| (p[i] - x[i]).abs() < 1e-12))
        .expect("a node at the point")
}

/// The 3-2-1 support of ADR 0281 arithmetic 3 at the corners A = 0, B = (lx, 0, 0), C = (0, ly, 0).
fn three_two_one(mut mesh: SolidMesh, lx: f64, ly: f64, material: Material) -> SolidModel {
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

fn free_cube(material: Material) -> SolidModel {
    three_two_one(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap(), 1.0, 1.0, material)
}

/// `max |u_i - a X_i| / L` over the nodes and components (`X_A = 0`, `L = 1`).
fn expansion_error(model: &SolidModel, u: &[f64], a: f64) -> f64 {
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

fn uniform(model: &SolidModel, t: f64) -> Vec<f64> {
    vec![t; model.mesh().nodes().len()]
}

fn inf_norm(v: &[f64]) -> f64 {
    let mut w: f64 = 0.0;
    for x in v {
        worse(&mut w, x.abs());
    }
    w
}

#[test]
fn fe1_fe2_hb1_free_expansion_and_the_held_cube() {
    let mut c = Checks::new();
    let a = ALPHA * DT;
    let model = free_cube(steel(ALPHA));

    // FE1: the step T_ref -> T_ref + dT over the first step, held.
    let field = TemperatureFieldHistory::new(
        REF,
        vec![
            (0.0, uniform(&model, REF)),
            (1.0, uniform(&model, REF + DT)),
            (3.0, uniform(&model, REF + DT)),
        ],
    )
    .unwrap();
    let opts = TimeSteppingOptions::new(f64::INFINITY)
        .newton(tight())
        .temperature_field(field);
    let run = solve_in_time_with(&model, &[], &TimeLoading::new(), &[1.0, 2.0, 3.0], &opts).unwrap();
    let mut fe1: f64 = 0.0;
    let mut fe1s: f64 = 0.0;
    let mut halvings = 0;
    for s in &run.steps[1..] {
        worse(&mut fe1, expansion_error(&model, &s.u, a));
    }
    for s in &run.steps {
        worse(&mut fe1s, inf_norm(&s.internal_force) / (E_S * a));
        halvings = halvings.max(s.halvings);
    }
    let at0 = inf_norm(&run.steps[0].u) / a;
    c.holds(
        "FE1c set-up (u = 0 at t = 0, no halving, 4 entries)",
        at0.is_finite() && at0 <= 1e-12 && halvings == 0 && run.steps.len() == 4,
        &format!(
            "max |u(0)|/(a L) = {at0:e}, halvings {halvings}, {} entries",
            run.steps.len()
        ),
    );
    c.at_most(
        "FE1 free expansion under a temperature step, |u - alpha dT X|/L",
        fe1,
        1e-12,
    );
    c.at_most("FE1s internal forces / (E alpha dT L^2)", fe1s, 1e-10);

    // FE2: the ramp over four steps.
    let table = LoadTable::new(vec![(0.0, REF), (4.0, REF + DT)]).unwrap();
    let ramp =
        TemperatureFieldHistory::new(REF, vec![(0.0, uniform(&model, REF)), (4.0, uniform(&model, REF + DT))]).unwrap();
    let run = solve_in_time_with(
        &model,
        &[],
        &TimeLoading::new(),
        &[1.0, 2.0, 3.0, 4.0],
        &TimeSteppingOptions::new(f64::INFINITY)
            .newton(tight())
            .temperature_field(ramp),
    )
    .unwrap();
    let mut fe2: f64 = 0.0;
    for s in &run.steps {
        let an = ALPHA * (table.value(s.time).unwrap() - REF);
        worse(&mut fe2, expansion_error(&model, &s.u, an));
    }
    c.at_most("FE2 the ramp, |u - a_n X|/L at the step ends", fe2, 1e-12);

    // HB1: the cube held on all six faces.
    let mut held = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap());
    held.add_material("all", steel(ALPHA)).unwrap();
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
    let field = TemperatureFieldHistory::new(
        REF,
        vec![
            (0.0, uniform(&held, REF)),
            (1.0, uniform(&held, REF + DT)),
            (3.0, uniform(&held, REF + DT)),
        ],
    )
    .unwrap();
    let run = solve_in_time_with(
        &held,
        &[],
        &TimeLoading::new(),
        &[1.0, 2.0, 3.0],
        &TimeSteppingOptions::new(f64::INFINITY)
            .newton(tight())
            .temperature_field(field),
    )
    .unwrap();
    let theta = 1.0 + a;
    let sxx = -E_S / (1.0 - 2.0 * NU_S) * (theta * theta - 1.0) / (2.0 * theta);
    println!("HB1 closed form S_xx = {sxx:.12e}");
    let dofs = held.dof_map();
    let xmax: Vec<usize> = held
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| dofs.equation(n, 0).unwrap())
        .collect();
    let mut hb1: f64 = 0.0;
    let mut hb1u: f64 = 0.0;
    for s in &run.steps[1..] {
        let sum: f64 = xmax.iter().map(|&q| s.internal_force[q]).sum();
        println!("HB1 t = {}: sum f_x(xmax) = {sum:.12e}", s.time);
        worse(&mut hb1, (sum - sxx).abs() / sxx.abs());
        worse(&mut hb1u, inf_norm(&s.u) / a);
    }
    c.at_most("HB1 held cube, the face force against the closed form", hb1, 1e-10);
    c.at_most("HB1u held cube, max |u| / (alpha dT L)", hb1u, 1e-12);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- the tangent

fn dense(k: &CsrMatrix) -> Vec<Vec<f64>> {
    let mut d = vec![vec![0.0; k.n]; k.n];
    for (row, dr) in d.iter_mut().enumerate() {
        for p in k.row_ptr[row]..k.row_ptr[row + 1] {
            dr[k.col[p]] += k.val[p];
        }
    }
    d
}

#[test]
fn tg1_the_consistent_tangent_with_a_field() {
    let mut c = Checks::new();
    let mut model = SolidModel::new(hex_box([0.0; 3], [2.0, 0.5, 0.5], [2, 1, 1]).unwrap());
    model.add_material("all", steel(ALPHA)).unwrap();
    let n = model.dof_map().n_equations();
    let nn = model.mesh().nodes().len();
    let temps: Vec<f64> = (0..nn).map(|b| REF + 60.0 + 40.0 * r(b)).collect();
    let u: Vec<f64> = (0..n).map(|q| 1e-2 * ((q as f64) * 0.73 + 0.2).sin()).collect();
    let states = nonlinear::initial_states(&model).unwrap();
    let ctx = PointContext::default();
    let k = dense(&nonlinear::tangent_with_field(&model, &u, &states, Some(1), &ctx, &temps, REF).unwrap());
    let h = 1e-7;
    let mut diff: f64 = 0.0;
    let mut scale: f64 = 0.0;
    for j in 0..n {
        let mut up = u.clone();
        let mut um = u.clone();
        up[j] += h;
        um[j] -= h;
        let fp = nonlinear::internal_force_with_field(&model, &up, &states, &ctx, &temps, REF).unwrap();
        let fm = nonlinear::internal_force_with_field(&model, &um, &states, &ctx, &temps, REF).unwrap();
        for i in 0..n {
            let fd = (fp[i] - fm[i]) / (2.0 * h);
            worse(&mut diff, (k[i][j] - fd).abs());
            worse(&mut scale, k[i][j].abs());
        }
    }
    c.at_most(
        "TG1 tangent with a field against the central difference",
        diff / scale,
        1e-6,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- the small-strain limit

/// `max |u_nl - u_lin| / max |u_lin|` for the amplitude `amp` of the field (arithmetic 9).
fn lin_error(model: &SolidModel, amp: f64) -> f64 {
    let nn = model.mesh().nodes().len();
    let temps: Vec<f64> = (0..nn).map(|b| REF + amp * r(b)).collect();
    let mut linear = model.clone();
    linear.add_load(Load::temperature(temps.clone(), REF)).unwrap();
    let lin = solve_static(&linear, &StaticOptions::default()).unwrap();
    let field = TemperatureFieldHistory::constant(REF, temps, 0.0, 1.0).unwrap();
    let run = solve_in_time_with(
        model,
        &[],
        &TimeLoading::new(),
        &[1.0],
        &TimeSteppingOptions::new(f64::INFINITY)
            .newton(tight())
            .temperature_field(field),
    )
    .unwrap();
    let u = &run.steps[1].u;
    let dofs = model.dof_map();
    let mut diff: f64 = 0.0;
    let mut scale: f64 = 0.0;
    for node in 0..nn {
        let ul = lin.displacement(node);
        for comp in 0..3 {
            let q = dofs.equation(node, comp).unwrap();
            worse(&mut diff, (u[q] - ul[comp]).abs());
            worse(&mut scale, ul[comp].abs());
        }
    }
    diff / scale
}

#[test]
fn lin_the_small_strain_limit_of_a_non_uniform_field() {
    let mut c = Checks::new();
    let mat = steel(1e-6);
    let hex = three_two_one(
        hex_box([0.0; 3], [2.0, 0.5, 0.5], [8, 2, 2]).unwrap(),
        2.0,
        0.5,
        mat.clone(),
    );
    let (tet, _) = upgrade_to_tet10(&tet_box([0.0; 3], [2.0, 0.5, 0.5], [4, 1, 1]).unwrap()).unwrap();
    let tet = three_two_one(tet, 2.0, 0.5, mat);
    for (name, model) in [("h", &hex), ("t", &tet)] {
        let e1 = lin_error(model, 1.0);
        let e2 = lin_error(model, 0.5);
        println!("LIN-{name} e(A/2) = {e2:.6e}");
        c.at_most(&format!("LIN-{name} nonlinear against linear thermal solve"), e1, 1e-4);
        c.within(&format!("LINo-{name} e(A)/e(A/2)"), e1 / e2, 1.8, 2.2);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- ADR 0526 set-ups

const E0: f64 = 2.0e9;
const NU: f64 = 0.35;
const W3: [f64; 3] = [0.3, 0.2, 0.1];
const TAU3: [f64; 3] = [0.01, 0.1, 1.0];
const T_REF: f64 = 373.0;
const T1: f64 = 383.0;
const T2: f64 = 393.0;
const EPS0: f64 = 1e-3;

fn base(nu: f64, alpha: f64) -> Material {
    Material::isotropic_elastic(E0, nu, 1200.0, alpha).unwrap()
}

fn equal(w: &[f64], tau: &[f64], nu: f64, alpha: f64, shift: Option<TimeTemperatureShift>) -> Material {
    let terms = w
        .iter()
        .zip(tau)
        .map(|(&wi, &ti)| PronyTerm::new(wi, wi, ti).unwrap())
        .collect();
    Material::viscoelastic(base(nu, alpha), terms, shift).unwrap()
}

fn wlf() -> TimeTemperatureShift {
    TimeTemperatureShift::wlf(T_REF, 17.44, 51.6).unwrap()
}

fn wlf_formula(t: f64) -> f64 {
    10f64.powf(-17.44 * (t - T_REF) / (51.6 + t - T_REF))
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

fn strain_step(model: &SolidModel, t_far: f64) -> TimeLoading {
    let list = model
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 0, EPS0))
        .collect();
    TimeLoading::new().motion(MotionHistory::new(list, LoadTable::constant(1.0, 0.0, t_far).unwrap()))
}

/// The uniform field of the values of a table at its times.
fn uniform_of(model: &SolidModel, table: &LoadTable, reference: f64) -> TemperatureFieldHistory {
    let nn = model.mesh().nodes().len();
    TemperatureFieldHistory::new(
        reference,
        table.points().iter().map(|&(t, v)| (t, vec![v; nn])).collect(),
    )
    .unwrap()
}

/// `max_n |x_n - y_n|_inf / max_n |y_n|_inf`.
fn relative(x: &[Vec<f64>], y: &[Vec<f64>]) -> f64 {
    let mut diff: f64 = 0.0;
    let mut scale: f64 = 0.0;
    for (a, b) in x.iter().zip(y) {
        for (p, q) in a.iter().zip(b) {
            worse(&mut diff, (p - q).abs());
            worse(&mut scale, q.abs());
        }
    }
    if x.len() != y.len() {
        return f64::NAN;
    }
    diff / scale
}

#[test]
fn ub1_ncu_a_uniform_field_on_the_driver() {
    let mut c = Checks::new();
    let a1 = wlf_formula(T1);
    let a2 = wlf_formula(T2);
    let model = cube(equal(&W3, &TAU3, NU, 0.0, Some(wlf())));
    let (h1, h2) = (0.02 * a1, 0.02 * a2);
    let ts = 50.0 * h1;
    let times: Vec<f64> = (1..=100)
        .map(|k| {
            if k <= 50 {
                k as f64 * h1
            } else {
                ts + (k - 50) as f64 * h2
            }
        })
        .collect();
    let t_last = times[99];
    let table = LoadTable::new(vec![(0.0, T1), (ts, T1), (times[50], T2), (2.0 * t_last, T2)]).unwrap();
    let loading = strain_step(&model, 2.0 * t_last);
    let history = solve_in_time_with(
        &model,
        &[],
        &loading,
        &times,
        &TimeSteppingOptions::new(f64::INFINITY)
            .newton(tight())
            .temperature_history(table.clone()),
    )
    .unwrap();
    let field = solve_in_time_with(
        &model,
        &[],
        &loading,
        &times,
        &TimeSteppingOptions::new(f64::INFINITY)
            .newton(tight())
            .temperature_field(uniform_of(&model, &table, T_REF)),
    )
    .unwrap();
    let (fa, fb) = (flat_history(&history), flat_history(&field));
    let first = fa.iter().zip(&fb).position(|(x, y)| x != y);
    c.holds(
        "UB1 uniform field without expansion gives the bits of ADR 0526 on the driver",
        fa.len() == 101 && fa.len() == fb.len() && first.is_none(),
        &format!("{} and {} step ends, first differing {first:?}", fa.len(), fb.len()),
    );

    // NCU: the same field with an expansion.
    let hot = cube(equal(&W3, &TAU3, NU, 1e-5, Some(wlf())));
    let expanded = solve_in_time_with(
        &hot,
        &[],
        &loading,
        &times,
        &TimeSteppingOptions::new(f64::INFINITY)
            .newton(tight())
            .temperature_field(uniform_of(&hot, &table, T_REF)),
    )
    .unwrap();
    let ue: Vec<Vec<f64>> = expanded.steps.iter().map(|s| s.u.clone()).collect();
    let uh: Vec<Vec<f64>> = history.steps.iter().map(|s| s.u.clone()).collect();
    c.at_least("NCU the field with an expansion differs", relative(&ue, &uh), 1e-3);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- the rod (ADR 0526 V2T)

fn rod(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.05, 0.05], [20, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

struct Rod {
    lambda: f64,
    v0: Vec<f64>,
}

fn rod_modes() -> Rod {
    let m = rod(base(0.0, 0.0));
    let modal = modal_analysis(&m, &ModalOptions::new(2)).unwrap();
    let mass = rcfd_fem::mass::consistent(&m).unwrap();
    let mut phi = modal.modes[0].clone();
    let norm = dot(&phi, &mv(&mass, &phi)).sqrt();
    for x in phi.iter_mut() {
        *x /= norm;
    }
    let lambda = modal.eigenvalues[0];
    let omega = lambda.sqrt();
    let peak = phi.iter().fold(0.0, |a: f64, b| a.max(b.abs()));
    let amp = 1e-6 * omega / (peak * PI / 2.0);
    let v0 = phi.iter().map(|x| amp * x).collect();
    Rod { lambda, v0 }
}

/// A run of the integrator: `u` and the bits of `u`, `v`, `a`, the internal force at every step end, the largest halving
/// count, the state bytes at the end.
#[derive(Default)]
struct Run {
    u: Vec<Vec<f64>>,
    bits: Vec<Vec<u64>>,
    halvings: usize,
    bytes: Vec<u8>,
}

fn advance(it: &mut NlIntegrator, steps: usize, run: &mut Run) {
    for _ in 0..steps {
        let r = it.step().unwrap();
        run.halvings = run.halvings.max(r.halvings);
        let st = it.state().unwrap();
        let mut v = st.u.clone();
        v.extend_from_slice(&st.v);
        v.extend_from_slice(&st.a);
        v.extend_from_slice(&st.internal_force);
        run.bits.push(bits(&v));
        run.u.push(st.u);
    }
    run.bytes = it.state().unwrap().to_bytes();
}

fn vibrate(model: &SolidModel, options: &NlTransientOptions, u0: &[f64], v0: &[f64], steps: usize) -> Run {
    let mut it = NlIntegrator::new(model, &[], options).unwrap();
    it.start(0.0, u0, v0).unwrap();
    let mut run = Run::default();
    advance(&mut it, steps, &mut run);
    run
}

#[test]
fn ub2_a_uniform_field_on_the_integrator() {
    let mut c = Checks::new();
    let r = rod_modes();
    let omega = r.lambda.sqrt();
    let dt = 2.0 * PI / omega / 200.0;
    let a1 = shift_factor(&wlf(), T1).unwrap();
    let tau_t: Vec<f64> = TAU3.iter().map(|t| t / (0.1 * omega)).collect();
    let tau_ref: Vec<f64> = tau_t.iter().map(|t| t / a1).collect();
    let shifted = rod(equal(&W3, &tau_ref, 0.0, 0.0, Some(wlf())));
    let damping = Rayleigh::from_ratio(0.02, omega, omega).unwrap();
    let o = NlTransientOptions::new(dt).damping(damping);
    let table = LoadTable::new(vec![(0.0, T1), (400.0 * dt, T1), (401.0 * dt, T2), (1600.0 * dt, T2)]).unwrap();
    let zero = vec![0.0; r.v0.len()];
    let run_a = vibrate(
        &shifted,
        &o.clone().temperature_history(table.clone()),
        &zero,
        &r.v0,
        800,
    );
    let run_f = vibrate(
        &shifted,
        &o.temperature_field(uniform_of(&shifted, &table, T_REF)),
        &zero,
        &r.v0,
        800,
    );
    let first = run_a.bits.iter().zip(&run_f.bits).position(|(x, y)| x != y);
    c.holds(
        "UB2 uniform field without expansion gives the bits of ADR 0526 on the integrator",
        run_a.bits.len() == 800 && run_f.bits.len() == 800 && first.is_none() && run_a.bytes == run_f.bytes,
        &format!(
            "{} and {} steps, first differing {first:?}, bytes equal {}, halvings {} and {}",
            run_a.bits.len(),
            run_f.bits.len(),
            run_a.bytes == run_f.bytes,
            run_a.halvings,
            run_f.halvings
        ),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- the integrator

/// The bar of S3e of ADR 0526: `[1, 0.2, 0.2]` of `[3, 1, 1]` Hex8, three symmetry planes.
fn bar(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

#[test]
fn fei_v3t_nc3_the_integrator() {
    let mut c = Checks::new();

    // FEi: the cube at its thermal equilibrium.
    let a = ALPHA * DT;
    let model = free_cube(steel(ALPHA));
    let n = model.dof_map().n_equations();
    let dofs = model.dof_map();
    let mut u0 = vec![0.0; n];
    for (node, x) in model.mesh().nodes().iter().enumerate() {
        for comp in 0..3 {
            u0[dofs.equation(node, comp).unwrap()] = a * x[comp];
        }
    }
    let field = TemperatureFieldHistory::constant(REF, uniform(&model, REF + DT), 0.0, 1.0).unwrap();
    let damping = Rayleigh {
        mass: 10.0,
        stiffness: 1e-6,
    };
    let o = NlTransientOptions::new(1e-5)
        .damping(damping)
        .newton(tight())
        .temperature_field(field);
    let run = vibrate(&model, &o, &u0, &vec![0.0; n], 50);
    let mut fei: f64 = 0.0;
    for u in &run.u {
        worse(&mut fei, expansion_error(&model, u, a));
    }
    c.holds(
        "FEi set-up (50 steps)",
        run.u.len() == 50,
        &format!("{} steps", run.u.len()),
    );
    c.at_most(
        "FEi integrator at the thermal equilibrium, |u - alpha dT X|/L",
        fei,
        1e-12,
    );

    // V3T: a non-uniform field step against the restart.
    let b = bar(steel(1e-5));
    let nb = b.dof_map().n_equations();
    let nn = b.mesh().nodes().len();
    let f1: Vec<f64> = (0..nn).map(|k| REF + 50.0 + 20.0 * r(k)).collect();
    let f2: Vec<f64> = (0..nn).map(|k| REF + 120.0 - 30.0 * r(k)).collect();
    let mut vb = vec![0.0; nb];
    for &nd in b.mesh().node_set("xmax").unwrap() {
        vb[b.dof_map().equation(nd, 0).unwrap()] = 40.0;
    }
    let dt = 2e-6;
    let jd = Rayleigh {
        mass: 20.0,
        stiffness: 2.0e-8,
    };
    let o = NlTransientOptions::new(dt).damping(jd);
    let step = TemperatureFieldHistory::new(
        REF,
        vec![
            (0.0, f1.clone()),
            (20.0 * dt, f1.clone()),
            (21.0 * dt, f2.clone()),
            (80.0 * dt, f2.clone()),
        ],
    )
    .unwrap();
    let zero = vec![0.0; nb];
    let run_a = vibrate(&b, &o.clone().temperature_field(step), &zero, &vb, 40);
    let mut run_b = Run::default();
    let mut first = NlIntegrator::new(
        &b,
        &[],
        &o.clone()
            .temperature_field(TemperatureFieldHistory::constant(REF, f1.clone(), 0.0, 80.0 * dt).unwrap()),
    )
    .unwrap();
    first.start(0.0, &zero, &vb).unwrap();
    advance(&mut first, 20, &mut run_b);
    let mut second = NlIntegrator::new(
        &b,
        &[],
        &o.clone()
            .temperature_field(TemperatureFieldHistory::constant(REF, f2, 0.0, 80.0 * dt).unwrap()),
    )
    .unwrap();
    second.restore(first.state().unwrap()).unwrap();
    advance(&mut second, 20, &mut run_b);
    let first_diff = run_a.bits.iter().zip(&run_b.bits).position(|(x, y)| x != y);
    c.holds(
        "V3T non-uniform field step against the restart: bits of u, v, a, f_int and the state bytes; no halving",
        run_a.bits.len() == 40
            && run_b.bits.len() == 40
            && first_diff.is_none()
            && run_a.bytes == run_b.bytes
            && run_a.halvings == 0
            && run_b.halvings == 0,
        &format!(
            "{} and {} steps, first differing {first_diff:?}, bytes equal {}, halvings {} and {}",
            run_a.bits.len(),
            run_b.bits.len(),
            run_a.bytes == run_b.bytes,
            run_a.halvings,
            run_b.halvings
        ),
    );
    let run_c = vibrate(
        &b,
        &o.temperature_field(TemperatureFieldHistory::constant(REF, f1, 0.0, 80.0 * dt).unwrap()),
        &zero,
        &vb,
        40,
    );
    c.at_least(
        "NC3 the step run differs from the constant F1",
        relative(&run_a.u, &run_c.u),
        1e-2,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- refusals

#[test]
fn rf4_refusals() {
    let mut c = Checks::new();
    let model = free_cube(steel(ALPHA));
    let nn = model.mesh().nodes().len();
    let field = TemperatureFieldHistory::constant(REF, uniform(&model, REF + DT), 0.0, 2.0).unwrap();
    let table = LoadTable::constant(REF, 0.0, 2.0).unwrap();
    let base_opts = TimeSteppingOptions::new(f64::INFINITY).newton(tight());
    let none = TimeLoading::new();

    // The driver.
    let cases: Vec<(&str, SolidModel, TimeSteppingOptions, &str)> = vec![
        (
            "driver with a field and a temperature",
            model.clone(),
            base_opts.clone().temperature(REF).temperature_field(field.clone()),
            "temperature",
        ),
        (
            "driver with a field and a history",
            model.clone(),
            base_opts
                .clone()
                .temperature_history(table.clone())
                .temperature_field(field.clone()),
            "temperature",
        ),
        (
            "driver with a field not covering the run",
            model.clone(),
            base_opts.clone().temperature_field(
                TemperatureFieldHistory::constant(REF, uniform(&model, REF + DT), 0.0, 0.5).unwrap(),
            ),
            "temperature",
        ),
        (
            "driver with a field of the wrong length",
            model.clone(),
            base_opts
                .clone()
                .temperature_field(TemperatureFieldHistory::constant(REF, vec![REF; nn - 1], 0.0, 2.0).unwrap()),
            "temperature",
        ),
        {
            let mut mesh = hex_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap();
            mesh.set_block_property(0, BlockProperty::solid(SolidFormulation::BBar))
                .unwrap();
            let bb = three_two_one(mesh, 1.0, 1.0, steel(ALPHA));
            (
                "driver with a BBar block",
                bb,
                base_opts.clone().temperature_field(field.clone()),
                "temperature",
            )
        },
        {
            let cold = free_cube(steel(1e-2));
            let f = TemperatureFieldHistory::constant(REF, uniform(&cold, REF - 150.0), 0.0, 2.0).unwrap();
            (
                "driver with a non-positive thermal stretch",
                cold,
                base_opts.clone().temperature_field(f),
                "temperature",
            )
        },
    ];
    for (name, m, opts, key) in cases {
        let (ok, d) = refused_with(solve_in_time_with(&m, &[], &none, &[1.0], &opts), key);
        c.holds(&format!("RF4 {name}"), ok, &d);
    }

    // The history.
    let row = vec![REF; nn];
    let bad: Vec<(&str, rcfd_fem::Result<TemperatureFieldHistory>)> = vec![
        ("one point", TemperatureFieldHistory::new(REF, vec![(0.0, row.clone())])),
        (
            "times not increasing",
            TemperatureFieldHistory::new(REF, vec![(1.0, row.clone()), (1.0, row.clone())]),
        ),
        (
            "fields of unequal length",
            TemperatureFieldHistory::new(REF, vec![(0.0, row.clone()), (1.0, vec![REF; nn + 1])]),
        ),
        (
            "a non-finite value",
            TemperatureFieldHistory::new(REF, vec![(0.0, row.clone()), (1.0, vec![f64::NAN; nn])]),
        ),
    ];
    for (name, r) in bad {
        let (ok, d) = refused_with(r, "temperature");
        c.holds(&format!("RF4 TemperatureFieldHistory::new with {name}"), ok, &d);
    }

    // The integrator.
    let dt = 1e-5;
    let n = model.dof_map().n_equations();
    let zero = vec![0.0; n];
    let (ok, d) = refused_with(
        NlIntegrator::new(
            &model,
            &[],
            &NlTransientOptions::new(dt)
                .temperature(REF)
                .temperature_field(field.clone()),
        ),
        "temperature",
    );
    c.holds("RF4 Integrator::new with a field and a temperature", ok, &d);
    let (ok, d) = refused_with(
        NlIntegrator::new(
            &model,
            &[],
            &NlTransientOptions::new(dt)
                .energy_momentum()
                .temperature_field(field.clone()),
        ),
        "temperature",
    );
    c.holds("RF4 Integrator::new with the energy-momentum step", ok, &d);
    let late = TemperatureFieldHistory::constant(REF, uniform(&model, REF), 1.0, 2.0).unwrap();
    let mut it = NlIntegrator::new(&model, &[], &NlTransientOptions::new(dt).temperature_field(late)).unwrap();
    let (ok, d) = refused_with(it.start(0.0, &zero, &zero), "temperature");
    c.holds("RF4 start at a time outside the field", ok, &d);
    let short = TemperatureFieldHistory::constant(REF, uniform(&model, REF), 0.0, 10.0 * dt).unwrap();
    let mut it = NlIntegrator::new(&model, &[], &NlTransientOptions::new(dt).temperature_field(short)).unwrap();
    it.start(0.0, &zero, &zero).unwrap();
    let before = it.time().unwrap();
    let (ok, d) = refused_with(it.run(20), "temperature");
    let after = it.time().unwrap();
    c.holds(
        "RF4 run beyond the field refused before any step",
        ok && before.to_bits() == after.to_bits(),
        &format!("{d}; time {before:e} -> {after:e}"),
    );
    let (ok, d) = refused_with(it.strain_energy(), "strain energy");
    c.holds("RF4 strain energy with a field", ok, &d);
    c.finish();
}
