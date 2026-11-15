//! F-669 (ADR 0590): buckling and the direct harmonic response about a nonlinear static state of a hyperelastic model, and
//! the modes of the BBar bar at K/G = 1e4 about a stretched state. Buckling (Bar C, compressed): S0, BT0, BFD (the factors
//! against the fourth-order central difference of the residual along each mode), B0 (zero load against
//! `buckling::factors`), P1, NB1, NB2. Harmonic (Bar S, stretched): S0, HT0, HFD (the response against the central
//! difference of the residual along it), H0 (zero load against `DirectHarmonic::new`), NH1, NH2; HR the refusals. BBar (Bar
//! B): S0, TB0, KMb, FB1, CB0, NBb and the records FB2, RB. Every measured value is printed; a non-finite value or a refused
//! call is a miss.

use rcfd_fem::buckling::{factors, BucklingOptions, BucklingResult};
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::element::ElementState;
use rcfd_fem::follower::{tangent_buckling_factors, tangent_modes};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicResponse};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::mass::consistent;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::nonlinear::{initial_states, internal_force, solve, tangent, NewtonOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, FemError, Load, Material, SolidFormulation, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// Measured values against their limits; the test fails at the end if any missed.
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
            self.0.push(format!("{name} = {value:e} not <= {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not >= {limit:e}"));
        }
    }
    fn record(&mut self, name: &str, value: f64) {
        println!("{name}: {value:.6e} (record)");
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

// ----------------------------------------------------------------------------------------------- set-up (ADR 0590 D4)

const RHO: f64 = 1000.0;
/// The axial force on each node of `xmax` of Bar S and Bar B, N.
const P_STRETCH: f64 = 4e3;
/// The axial force on each node of `xmax` of Bar C, N.
const P_COMPRESS: f64 = -250.0;
/// The step of the fourth-order central difference, m (ADR 0590 D3, arithmetic 2).
const A_STEP: f64 = 1e-4;
/// The step of ADR 0586's two-point difference (record FB2), m.
const A_STEP_2: f64 = 5e-6;
/// The step of the full-matrix central difference (KMb), m.
const H_STEP: f64 = 1e-6;
/// The lateral harmonic force on each node of `xmax`, N.
const F_Z: f64 = 100.0;
const MODES: usize = 6;
const FACTORS: usize = 4;
const RAYLEIGH: (f64, f64) = (0.5, 1e-3);

fn nhc() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap()
}
fn mrc() -> HyperelasticEnergy {
    HyperelasticEnergy::mooney_rivlin(3.5e5, 1.5e5, 1e7).unwrap()
}
fn nhi() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6, 1e10).unwrap()
}

/// The bar `[0, 2] x [0, 0.5] x [0, 0.5]`, 8 x 2 x 2 Hex8, held on `xmin`, `p` in `x` on each node of `xmax`.
fn bar(energy: HyperelasticEnergy, p: f64, formulation: SolidFormulation) -> SolidModel {
    let mut mesh = rcfd_fem::builders::hex_box([0.0; 3], [2.0, 0.5, 0.5], [8, 2, 2]).unwrap();
    if formulation != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(formulation)).unwrap();
    }
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::hyperelastic(energy, RHO).unwrap())
        .unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::nodal_force("xmax", [p, 0.0, 0.0])).unwrap();
    model
}

/// The dead load times the load factor.
fn external(model: &SolidModel, p: f64, load_factor: f64) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &n in model.mesh().node_set("xmax").unwrap() {
        f[dofs.equation(n, 0).unwrap()] += load_factor * p;
    }
    f
}

/// A state of the nonlinear residual `r(u) = f_int(u) - load_factor f_ext`.
struct State<'a> {
    model: &'a SolidModel,
    u: Vec<f64>,
    states: Vec<ElementState>,
    fe: Vec<f64>,
}

impl State<'_> {
    fn residual(&self, w: &[f64]) -> Vec<f64> {
        let f = internal_force(self.model, w, &self.states).unwrap();
        f.iter().zip(&self.fe).map(|(a, b)| a - b).collect()
    }
    fn at(&self, v: &[f64], t: f64) -> Vec<f64> {
        let w: Vec<f64> = self.u.iter().zip(v).map(|(x, p)| x + t * p).collect();
        self.residual(&w)
    }
    /// D4[v]: the fourth-order central difference of the residual along `v` (ADR 0590 D3).
    fn d4(&self, v: &[f64]) -> Vec<f64> {
        let a = A_STEP;
        let (p1, m1, p2, m2) = (self.at(v, a), self.at(v, -a), self.at(v, 2.0 * a), self.at(v, -2.0 * a));
        (0..v.len())
            .map(|k| (8.0 * (p1[k] - m1[k]) - (p2[k] - m2[k])) / (12.0 * a))
            .collect()
    }
    /// ADR 0586's two-point central difference along `v` with the step `A_STEP_2`.
    fn d2(&self, v: &[f64]) -> Vec<f64> {
        let a = A_STEP_2;
        let (p1, m1) = (self.at(v, a), self.at(v, -a));
        (0..v.len()).map(|k| (p1[k] - m1[k]) / (2.0 * a)).collect()
    }
}

fn dense(a: &CsrMatrix) -> Vec<Vec<f64>> {
    let mut d = vec![vec![0.0; a.n]; a.n];
    for (i, row) in d.iter_mut().enumerate() {
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            row[a.col[k]] += a.val[k];
        }
    }
    d
}

fn matvec(a: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    a.iter()
        .map(|row| row.iter().zip(x).map(|(v, xj)| v * xj).sum())
        .collect()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn quad(a: &[Vec<f64>], x: &[f64]) -> f64 {
    dot(x, &matvec(a, x))
}

fn freq(lambda: f64) -> f64 {
    lambda.signum() * lambda.abs().sqrt() / (2.0 * PI)
}

fn unit(v: &[f64]) -> (Vec<f64>, f64) {
    let s = v.iter().fold(0.0f64, |s, x| s.max(x.abs()));
    if s == 0.0 {
        return (v.to_vec(), 0.0);
    }
    (v.iter().map(|x| x / s).collect(), s)
}

/// Worst relative difference of two lists, position by position, relative to `b`; infinite if the lengths differ.
fn worst(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return f64::INFINITY;
    }
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs() / y.abs())
        .fold(0.0f64, |m, v| if v.is_finite() { m.max(v) } else { f64::INFINITY })
}

fn nonlinear_state(c: &mut Checks, name: &str, model: &SolidModel) -> Option<(Vec<f64>, Vec<ElementState>)> {
    match solve(model, &[], &[], &NewtonOptions::new(10)) {
        Ok(r) => {
            let u = r.result.displacement_values().to_vec();
            let dofs = model.dof_map();
            let tip = model.mesh().node_set("xmax").unwrap();
            let mean_ux = tip.iter().map(|&k| u[dofs.equation(k, 0).unwrap()]).sum::<f64>() / tip.len() as f64;
            c.holds(
                name,
                true,
                &format!(
                    "converged, {} steps, stretch 1 + mean u_x(xmax)/2 = {:.8}",
                    r.steps.len(),
                    1.0 + mean_ux / 2.0
                ),
            );
            Some((u, r.states))
        }
        Err(e) => {
            c.holds(name, false, &e.to_string());
            None
        }
    }
}

// ------------------------------------------------------------------------------------------------------- buckling

fn buckling_ok(c: &mut Checks, name: &str, r: &Result<BucklingResult, String>) -> bool {
    match r {
        Ok(b) => {
            let worst_res = b.residuals.iter().cloned().fold(0.0f64, f64::max);
            let ok = b.factors.len() == FACTORS
                && b.factors.iter().all(|f| f.is_finite() && *f > 0.0)
                && worst_res <= 1e-10
                && b.exhausted.is_none();
            c.holds(
                name,
                ok,
                &format!(
                    "factors {:?}, iterations {}, worst residual {worst_res:.3e}, exhausted {:?}",
                    b.factors, b.iterations, b.exhausted
                ),
            );
            ok
        }
        Err(e) => {
            c.holds(name, false, e);
            false
        }
    }
}

fn buckling_row(c: &mut Checks, row: &str, energy: HyperelasticEnergy) {
    let model = bar(energy, P_COMPRESS, SolidFormulation::Full);
    let n = model.dof_map().n_equations();
    let Some((u, states)) = nonlinear_state(c, &format!("S0 {row}: the nonlinear solve of Bar C"), &model) else {
        return;
    };
    let linear = solve_static(&model, &StaticOptions::default()).unwrap();
    let prestress = Prestress::from_static(&model, &linear).unwrap();
    let options = BucklingOptions::new(FACTORS);
    let zero = vec![0.0; n];
    let rest = initial_states(&model).unwrap();
    let bs = tangent_buckling_factors(&model, &[], &u, &states, &prestress, &options).map_err(|e| e.to_string());
    let bl = factors(&model, &prestress, &options).map_err(|e| e.to_string());
    let b0 = tangent_buckling_factors(&model, &[], &zero, &rest, &prestress, &options).map_err(|e| e.to_string());
    let ok_s = buckling_ok(c, &format!("BT0 {row}: tangent_buckling_factors at the state"), &bs);
    let ok_l = buckling_ok(c, &format!("BT0 {row}: buckling::factors"), &bl);
    let ok_0 = buckling_ok(c, &format!("BT0 {row}: tangent_buckling_factors at zero load"), &b0);
    if !(ok_s && ok_l && ok_0) {
        return;
    }
    let (bs, bl, b0) = (bs.unwrap(), bl.unwrap(), b0.unwrap());

    // BFD and NB2.
    let st = State {
        model: &model,
        u: u.clone(),
        states: states.clone(),
        fe: external(&model, P_COMPRESS, 1.0),
    };
    let kg = dense(&geometric_stiffness(&model, &prestress, None).unwrap());
    let k0 = dense(&tangent(&model, &zero, &rest, None).unwrap());
    let mut mu_fd = Vec::new();
    let mut mu_0 = Vec::new();
    for x in &bs.modes {
        let (xh, _) = unit(x);
        let g = dot(&xh, &st.d4(&xh));
        let s = quad(&kg, &xh);
        mu_fd.push(-g / s);
        mu_0.push(-quad(&k0, &xh) / s);
    }
    println!("{row}: mu_FD {mu_fd:?}, mu {:?}", bs.factors);
    c.at_most(&format!("BFD {row}"), worst(&mu_fd, &bs.factors), 1e-6);
    c.at_most(&format!("B0 {row}"), worst(&b0.factors, &bl.factors), 1e-10);
    c.at_most(
        &format!("P1 {row}"),
        ((1.0 + bs.factors[0]) - bl.factors[0]).abs() / bl.factors[0],
        5e-2,
    );
    c.at_least(&format!("NB1 {row}"), worst(&bs.factors, &bl.factors), 1e-2);
    c.at_least(&format!("NB2 {row}"), worst(&mu_0, &mu_fd), 1e-2);
}

#[test]
fn buckling_about_the_compressed_bar() {
    let mut c = Checks::new();
    buckling_row(&mut c, "NH", nhc());
    buckling_row(&mut c, "MR", mrc());
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- harmonic

fn lateral_load(model: &SolidModel) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &n in model.mesh().node_set("xmax").unwrap() {
        f[dofs.equation(n, 2).unwrap()] += F_Z;
    }
    f
}

/// `e_H` of ADR 0590 D3 on the free equations, the residual of `st`.
fn harmonic_error(
    st: &State,
    free: &[bool],
    mass: &[Vec<f64>],
    f: &[f64],
    x: &HarmonicResponse,
    rayleigh: (f64, f64),
) -> f64 {
    let n = f.len();
    let kx = |v: &[f64]| -> Vec<f64> {
        let (vh, s) = unit(v);
        if s == 0.0 {
            return vec![0.0; n];
        }
        st.d4(&vh).iter().map(|d| d * s).collect()
    };
    let (kre, kim) = (kx(&x.re), kx(&x.im));
    let (mre, mim) = (matvec(mass, &x.re), matvec(mass, &x.im));
    let (alpha, beta) = rayleigh;
    let w = x.omega;
    let mut q_max = 0.0f64;
    let mut k_max = 0.0f64;
    for k in (0..n).filter(|&k| free[k]) {
        // KX (1 + i w beta)
        let (a, b) = (kre[k] - w * beta * kim[k], kim[k] + w * beta * kre[k]);
        // (i w alpha - w^2) M X
        let (cm, dm) = (
            -w * w * mre[k] - w * alpha * mim[k],
            -w * w * mim[k] + w * alpha * mre[k],
        );
        let q = (a + cm - f[k]).hypot(b + dm);
        q_max = q_max.max(q);
        k_max = k_max.max(kre[k].hypot(kim[k]));
    }
    q_max / k_max
}

fn response_difference(a: &HarmonicResponse, b: &HarmonicResponse) -> f64 {
    let mut d = 0.0f64;
    let mut m = 0.0f64;
    for k in 0..a.re.len() {
        d = d.max((a.re[k] - b.re[k]).hypot(a.im[k] - b.im[k]));
        m = m.max(b.re[k].hypot(b.im[k]));
    }
    d / m
}

fn harmonic_row(c: &mut Checks, row: &str, energy: HyperelasticEnergy, negatives: bool) {
    let model = bar(energy, P_STRETCH, SolidFormulation::Full);
    let n = model.dof_map().n_equations();
    let Some((u, states)) = nonlinear_state(c, &format!("S0 {row}: the nonlinear solve of Bar S"), &model) else {
        return;
    };
    let zero = vec![0.0; n];
    let rest = initial_states(&model).unwrap();
    let ts = tangent_modes(&model, &[], &u, &states, 1.0, &[], &ModalOptions::new(MODES)).unwrap();
    let ma = modal_analysis(&model, &ModalOptions::new(MODES)).unwrap();
    let w1 = 2.0 * PI * ts.frequencies[0];
    let w1r = 2.0 * PI * ma.frequencies[0];
    println!("{row}: omega_1 at the state {w1:.8} rad/s, at rest {w1r:.8} rad/s");
    let f = lateral_load(&model);
    let load = HarmonicLoad::real(f.clone()).unwrap();
    let opt_a = DirectHarmonicOptions::new().threads(1);
    let opt_b = DirectHarmonicOptions::new().threads(1).rayleigh(RAYLEIGH.0, RAYLEIGH.1);

    let run = |solver: Result<DirectHarmonic, FemError>, omega: f64| -> Result<HarmonicResponse, String> {
        solver.and_then(|s| s.response(&load, omega)).map_err(|e| e.to_string())
    };
    let hs_a = run(DirectHarmonic::about_state(&model, &[], &u, &states, &opt_a), 0.5 * w1);
    let hs_b = run(DirectHarmonic::about_state(&model, &[], &u, &states, &opt_b), 1.5 * w1);
    let h0_a = run(
        DirectHarmonic::about_state(&model, &[], &zero, &rest, &opt_a),
        0.5 * w1r,
    );
    let h0_b = run(
        DirectHarmonic::about_state(&model, &[], &zero, &rest, &opt_b),
        1.5 * w1r,
    );
    let hl_a = run(DirectHarmonic::new(&model, &opt_a), 0.5 * w1r);
    let hl_b = run(DirectHarmonic::new(&model, &opt_b), 1.5 * w1r);
    let all = [&hs_a, &hs_b, &h0_a, &h0_b, &hl_a, &hl_b];
    let ok = all.iter().all(|r| r.is_ok());
    c.holds(
        &format!("HT0 {row}"),
        ok,
        &format!(
            "{:?}",
            all.iter().map(|r| r.as_ref().err().cloned()).collect::<Vec<_>>()
        ),
    );
    if !ok {
        return;
    }
    let (hs_a, hs_b, h0_a, h0_b, hl_a, hl_b) = (
        hs_a.unwrap(),
        hs_b.unwrap(),
        h0_a.unwrap(),
        h0_b.unwrap(),
        hl_a.unwrap(),
        hl_b.unwrap(),
    );

    let mass = dense(&consistent(&model).unwrap());
    let mut free = vec![true; n];
    for (eq, _) in prescribed_equations(&model, &[]).unwrap() {
        free[eq] = false;
    }
    let st = State {
        model: &model,
        u: u.clone(),
        states: states.clone(),
        fe: external(&model, P_STRETCH, 1.0),
    };
    c.at_most(
        &format!("HFD {row} omega_a"),
        harmonic_error(&st, &free, &mass, &f, &hs_a, (0.0, 0.0)),
        1e-6,
    );
    c.at_most(
        &format!("HFD {row} omega_b"),
        harmonic_error(&st, &free, &mass, &f, &hs_b, RAYLEIGH),
        1e-6,
    );
    c.at_most(&format!("H0 {row} omega_a"), response_difference(&h0_a, &hl_a), 1e-10);
    c.at_most(&format!("H0 {row} omega_b"), response_difference(&h0_b, &hl_b), 1e-10);
    if negatives {
        let linear = solve_static(&model, &StaticOptions::default()).unwrap();
        let prestress = Prestress::from_static(&model, &linear).unwrap();
        let hp = DirectHarmonic::new(&model, &opt_a.clone().prestress(prestress))
            .and_then(|s| s.response(&load, 0.5 * w1))
            .unwrap();
        c.at_least(&format!("NH1 {row}"), response_difference(&hs_a, &hp), 1e-3);
        let hr = DirectHarmonic::new(&model, &opt_a)
            .and_then(|s| s.response(&load, 0.5 * w1))
            .unwrap();
        c.at_least(
            &format!("NH2 {row}"),
            harmonic_error(&st, &free, &mass, &f, &hr, (0.0, 0.0)),
            1e-2,
        );
    }
}

#[test]
fn harmonic_about_the_stretched_bar() {
    let mut c = Checks::new();
    harmonic_row(&mut c, "NH", nhc(), true);
    harmonic_row(&mut c, "MR", mrc(), false);
    c.finish();
}

fn refused_with(r: Result<DirectHarmonic, FemError>, key: &str) -> (bool, String) {
    match r {
        Err(FemError::InvalidValue { key: k, reason }) => (k == key, format!("refused, key {k:?}: {reason}")),
        Err(e) => (false, format!("refused otherwise: {e}")),
        Ok(_) => (false, "accepted".to_string()),
    }
}

#[test]
fn refusals_of_about_state() {
    let mut c = Checks::new();
    let model = bar(nhc(), P_STRETCH, SolidFormulation::Full);
    let n = model.dof_map().n_equations();
    let zero = vec![0.0; n];
    let rest = initial_states(&model).unwrap();
    let linear = solve_static(&model, &StaticOptions::default()).unwrap();
    let prestress = Prestress::from_static(&model, &linear).unwrap();
    let cases = [
        (
            DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(0.02)),
            "loss factor",
        ),
        (DirectHarmonicOptions::new().prestress(prestress), "prestress"),
        (DirectHarmonicOptions::new().temperature(300.0), "temperature"),
    ];
    for (opt, key) in cases {
        let (ok, detail) = refused_with(DirectHarmonic::about_state(&model, &[], &zero, &rest, &opt), key);
        c.holds(&format!("HR {key}"), ok, &detail);
    }
    let solver = DirectHarmonic::about_state(&model, &[], &zero, &rest, &DirectHarmonicOptions::new()).unwrap();
    let load = HarmonicLoad::real(lateral_load(&model)).unwrap();
    let resp = solver.response(&load, 1.0).unwrap();
    let ok = matches!(
        solver.stress(&model, &resp),
        Err(FemError::InvalidValue { key: "stress", .. })
    );
    c.holds("HR stress", ok, "stress of an about_state solver");
    c.finish();
}

// --------------------------------------------------------------------------------------------------------- BBar

fn modes_ok(c: &mut Checks, name: &str, r: &Result<ModalResult, String>, tol: f64) -> bool {
    match r {
        Ok(m) => {
            let worst_res = m.residuals.iter().cloned().fold(0.0f64, f64::max);
            let ok = m.frequencies.len() == MODES && worst_res <= tol;
            c.holds(
                name,
                ok,
                &format!(
                    "{} frequencies {:?} Hz, iterations {}, worst residual {worst_res:.3e}",
                    m.frequencies.len(),
                    m.frequencies,
                    m.iterations
                ),
            );
            ok
        }
        Err(e) => {
            c.holds(name, false, e);
            false
        }
    }
}

fn directional(st: &State, modes: &ModalResult, mass: &[Vec<f64>], four: bool) -> Vec<f64> {
    modes
        .modes
        .iter()
        .map(|phi| {
            let (ph, _) = unit(phi);
            let d = if four { st.d4(&ph) } else { st.d2(&ph) };
            freq(dot(&ph, &d) / quad(mass, &ph))
        })
        .collect()
}

#[test]
fn bbar_modes_about_the_stretched_bar() {
    let mut c = Checks::new();
    let model = bar(nhi(), P_STRETCH, SolidFormulation::BBar);
    let n = model.dof_map().n_equations();
    let Some((u, states)) = nonlinear_state(&mut c, "S0 NHi: the nonlinear solve of Bar B", &model) else {
        c.finish();
        return;
    };
    let zero = vec![0.0; n];
    let rest = initial_states(&model).unwrap();
    let options = ModalOptions::new(MODES).tolerance(1e-8);
    let ts = tangent_modes(&model, &[], &u, &states, 1.0, &[], &options).map_err(|e| e.to_string());
    let tr = tangent_modes(&model, &[], &zero, &rest, 0.0, &[], &options).map_err(|e| e.to_string());
    let ma = modal_analysis(&model, &options).map_err(|e| e.to_string());
    let ok = modes_ok(&mut c, "TB0: tangent_modes at the stretched state", &ts, 1e-8)
        & modes_ok(&mut c, "TB0: tangent_modes at zero load", &tr, 1e-8)
        & modes_ok(&mut c, "TB0: modal_analysis (record RB)", &ma, 1e-8);
    if !ok {
        c.finish();
        return;
    }
    let (ts, tr, ma) = (ts.unwrap(), tr.unwrap(), ma.unwrap());
    let mass = dense(&consistent(&model).unwrap());
    let st = State {
        model: &model,
        u: u.clone(),
        states: states.clone(),
        fe: external(&model, P_STRETCH, 1.0),
    };
    let st0 = State {
        model: &model,
        u: zero.clone(),
        states: rest.clone(),
        fe: external(&model, P_STRETCH, 0.0),
    };

    // KMb.
    let kt = dense(&tangent(&model, &u, &states, None).unwrap());
    let mut d_max = 0.0f64;
    let mut w = u.clone();
    for j in 0..n {
        w[j] = u[j] + H_STEP;
        let rp = st.residual(&w);
        w[j] = u[j] - H_STEP;
        let rm = st.residual(&w);
        w[j] = u[j];
        for i in 0..n {
            d_max = d_max.max(((rp[i] - rm[i]) / (2.0 * H_STEP) - kt[i][j]).abs());
        }
    }
    let k_max = kt.iter().flatten().fold(0.0f64, |s, v| s.max(v.abs()));
    c.at_most("KMb", d_max / k_max, 1e-8);

    // FB1, CB0, NBb, FB2, RB.
    let f_fd = directional(&st, &ts, &mass, true);
    println!("FB1: f_FD {f_fd:?}, f {:?}", ts.frequencies);
    c.at_most("FB1", worst(&f_fd, &ts.frequencies), 1e-6);
    let f_fd0 = directional(&st0, &tr, &mass, true);
    c.at_most("CB0", worst(&f_fd0, &tr.frequencies), 1e-6);
    let k0 = dense(&tangent(&model, &zero, &rest, None).unwrap());
    let f_rest: Vec<f64> = ts
        .modes
        .iter()
        .map(|phi| freq(quad(&k0, phi) / quad(&mass, phi)))
        .collect();
    c.at_least("NBb", worst(&f_rest, &f_fd), 1e-2);
    let f_fd2 = directional(&st, &ts, &mass, false);
    c.record("FB2 (two-point, a = 5e-6)", worst(&f_fd2, &ts.frequencies));
    c.record(
        "RB (BBar zero load against modal_analysis)",
        worst(&tr.frequencies, &ma.frequencies),
    );
    c.finish();
}
