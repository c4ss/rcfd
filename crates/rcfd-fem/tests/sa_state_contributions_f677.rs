//! F-677 (ADR 0599): the state dependence of the contribution tangents on the buckling and direct harmonic paths about a
//! nonlinear state, a blind 3-D check. Quarter tube (Hex20 SVK, symmetry planes `x = 0`, `y = 0`, `z = 0`, clamped end) under
//! external follower pressure, conservative and with a follower tangent between free unknowns that changes with the state:
//! the set-up checks S0t, SCt, SDt, SDq and the control NSc; the buckling checks BT0t, BFD, BFV, B0c, P1c, NBc, NB2c, NMb;
//! the harmonic checks HT0t, HFD, H0c, NHc, NH2c, NMh. NMb and NMh evaluate every contribution tangent at zero displacement in
//! place of the state (the test contribution `AtZero`). Every measured value is printed; a non-finite value or a refused call
//! is a miss.

use rcfd_fem::buckling::{BucklingOptions, BucklingResult};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::element::ElementState;
use rcfd_fem::follower::{
    buckling_factors, tangent_buckling_factors, tangent_buckling_factors_with, tangent_modes, FollowerPressure,
};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicResponse};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::nonlinear::{initial_states, internal_force, solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel};
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

// ----------------------------------------------------------------------------------------------- set-up (ADR 0599 D2)

const R: f64 = 1.0;
const T: f64 = 0.15;
/// The half-length of the tube, m (`z = 0` its mid-plane, `z = L` clamped).
const L: f64 = 4.0;
const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO_STEEL: f64 = 7850.0;
/// The external follower pressure on the tube, Pa (`0.692` of the linear follower critical pressure, ADR 0599 probe).
const P_T: f64 = 1.75e8;
/// The radial harmonic force on each free `x` equation of the nodes of `ymin`, N.
const F_TUBE: f64 = 1e3;
const RAYLEIGH: (f64, f64) = (1.0, 1e-5);
/// The step of the fourth-order central difference, m (ADR 0590 D3).
const A_STEP: f64 = 1e-4;
const FACTORS: usize = 2;

/// The quarter tube: 1 x 12 x 8 Hex20 in `(r, theta, z)`, symmetry planes `y = 0` (`ymin`), `x = 0` (`ymax`), `z = 0`
/// (`zmin`, if `mid_plane`), clamped at `z = L` (`zmax`).
fn tube_model(mid_plane: bool) -> SolidModel {
    let b = hex_box([R - T / 2.0, 0.0, 0.0], [T, PI / 2.0, L], [1, 12, 8]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, NU, RHO_STEEL, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    if mid_plane {
        m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    }
    for k in 0..3 {
        m.add_constraint(Constraint::displacement("zmax", k, 0.0)).unwrap();
    }
    m
}

/// The pre-stress of the dead pressure `p` on `set` (the linear static solve of a copy with `Load::pressure`).
fn dead_prestress(model: &SolidModel, set: &str, p: f64) -> (SolidModel, Prestress) {
    let mut dead = model.clone();
    dead.add_load(Load::pressure(set, p)).unwrap();
    let s = solve_static(&dead, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&dead, &s).unwrap();
    (dead, pre)
}

/// A contribution whose tangent (and force) is that of `inner` at zero displacement, whatever the state (ADR 0599 D2: every
/// contribution tangent evaluated at `u = 0` in place of the state).
struct AtZero<'a>(&'a FollowerPressure);

impl Contribution for AtZero<'_> {
    fn name(&self) -> &str {
        "at zero"
    }
    fn add(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        self.0.add(&vec![0.0; u.len()], load_factor, force, tangent)
    }
}

/// A state of the residual `r(w) = f_int(w) - f_p(w, 1)` and its load-factor derivative `g(w) = -f_p(w, 1)`.
struct State<'a> {
    model: &'a SolidModel,
    u: Vec<f64>,
    states: Vec<ElementState>,
    fp: &'a FollowerPressure,
}

impl State<'_> {
    fn residual(&self, w: &[f64]) -> Vec<f64> {
        let mut f = internal_force(self.model, w, &self.states).unwrap();
        for (a, b) in f.iter_mut().zip(self.fp.force(w, 1.0).unwrap()) {
            *a -= b;
        }
        f
    }
    fn derivative(&self, w: &[f64]) -> Vec<f64> {
        self.fp.force(w, 1.0).unwrap().iter().map(|x| -x).collect()
    }
    fn d4_of(&self, v: &[f64], f: impl Fn(&[f64]) -> Vec<f64>) -> Vec<f64> {
        let a = A_STEP;
        let at = |t: f64| -> Vec<f64> {
            let w: Vec<f64> = self.u.iter().zip(v).map(|(x, p)| x + t * p).collect();
            f(&w)
        };
        let (p1, m1, p2, m2) = (at(a), at(-a), at(2.0 * a), at(-2.0 * a));
        (0..v.len())
            .map(|k| (8.0 * (p1[k] - m1[k]) - (p2[k] - m2[k])) / (12.0 * a))
            .collect()
    }
    /// D4 of the residual along `v` (ADR 0599 D3).
    fn d4(&self, v: &[f64]) -> Vec<f64> {
        self.d4_of(v, |w| self.residual(w))
    }
    /// D4 of the load-factor derivative along `v`.
    fn d4g(&self, v: &[f64]) -> Vec<f64> {
        self.d4_of(v, |w| self.derivative(w))
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

fn free_mask(model: &SolidModel) -> Vec<bool> {
    let mut free = vec![true; model.dof_map().n_equations()];
    for (eq, _) in prescribed_equations(model, &[]).unwrap() {
        free[eq] = false;
    }
    free
}

/// The nonlinear state of the tube under the follower pressure (S0t).
fn tube_state(c: &mut Checks, model: &SolidModel, fp: &FollowerPressure) -> Option<(Vec<f64>, Vec<ElementState>)> {
    let ext = DriverExtensions {
        contributions: vec![fp as &dyn Contribution],
        step_control: None,
    };
    match solve_with(model, &[], &[], &NewtonOptions::new(8), ext) {
        Ok(r) => {
            let u = r.result.displacement_values().to_vec();
            let mut umax = [0.0f64; 3];
            for (eq, x) in u.iter().enumerate() {
                let (_, k) = model.dof_map().node_component(eq).unwrap();
                umax[k] = umax[k].max(x.abs());
            }
            c.holds(
                "S0t: the tube",
                true,
                &format!("converged, {} steps, max |u| per component {umax:?}", r.steps.len()),
            );
            Some((u, r.states))
        }
        Err(e) => {
            c.holds("S0t: the tube", false, &e.to_string());
            None
        }
    }
}

/// The dense follower load stiffness `K_p` at `w`, load factor 1.
fn kp_dense(fp: &FollowerPressure, w: &[f64]) -> Vec<Vec<f64>> {
    dense(&CsrMatrix::from_triplets(w.len(), fp.load_stiffness(w, 1.0).unwrap()))
}

/// SCt: `max_ij,free |K_ij - K_ji| / max_ij,free |K_ij|`.
fn antisymmetry(k: &[Vec<f64>], free: &[bool]) -> f64 {
    let (mut a, mut big) = (0.0f64, 0.0f64);
    for i in (0..k.len()).filter(|&i| free[i]) {
        for j in (0..k.len()).filter(|&j| free[j]) {
            a = a.max((k[i][j] - k[j][i]).abs());
            big = big.max(k[i][j].abs());
        }
    }
    a / big
}

fn symmetric(k: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = k.len();
    (0..n)
        .map(|i| (0..n).map(|j| 0.5 * (k[i][j] + k[j][i])).collect())
        .collect()
}

// --------------------------------------------------------------------------------------------- set-up checks (a1, a2)

#[test]
fn tube_setup_is_conservative_and_state_dependent() {
    let mut c = Checks::new();
    let tube = tube_model(true);
    let n = tube.dof_map().n_equations();
    let free = free_mask(&tube);
    let fp = FollowerPressure::new(&tube, "xmax", P_T).unwrap();
    let Some((u, _)) = tube_state(&mut c, &tube, &fp) else {
        c.finish();
        return;
    };
    let zero = vec![0.0; n];
    let (ks, k0) = (kp_dense(&fp, &u), kp_dense(&fp, &zero));
    let (a_s, a_0) = (antisymmetry(&ks, &free), antisymmetry(&k0, &free));
    println!("SCt at u*: {a_s:e}, at 0: {a_0:e}");
    c.at_most("SCt", a_s.max(a_0), 1e-12);
    let (ss, s0) = (symmetric(&ks), symmetric(&k0));
    let (mut d, mut big) = (0.0f64, 0.0f64);
    for i in (0..n).filter(|&i| free[i]) {
        for j in (0..n).filter(|&j| free[j]) {
            d = d.max((ss[i][j] - s0[i][j]).abs());
            big = big.max(ss[i][j].abs());
        }
    }
    c.at_least("SDt", d / big, 1e-3);
    let (dead, pre) = dead_prestress(&tube, "xmax", P_T);
    match buckling_factors(&dead, &pre, &[&fp], &BucklingOptions::new(FACTORS)) {
        Ok(bl) => {
            let x0 = &bl.modes[0];
            let kg = dense(&geometric_stiffness(&tube, &pre, None).unwrap());
            let ds: Vec<Vec<f64>> = ss
                .iter()
                .zip(&s0)
                .map(|(a, b)| a.iter().zip(b).map(|(x, y)| x - y).collect())
                .collect();
            let num = dot(x0, &matvec(&ds, x0));
            let den = dot(x0, &matvec(&kg, x0)) - dot(x0, &matvec(&s0, x0));
            println!(
                "SDq: x0^T dS x0 = {num:e}, x0^T (K_sigma - S0) x0 = {den:e}, linear factors {:?}",
                bl.factors
            );
            c.at_least("SDq", (num / den).abs(), 1e-5);
        }
        Err(e) => c.holds("SDq: follower::buckling_factors", false, &e.to_string()),
    }
    let open = tube_model(false);
    let fpo = FollowerPressure::new(&open, "xmax", P_T).unwrap();
    let ko = kp_dense(&fpo, &vec![0.0; open.dof_map().n_equations()]);
    c.at_least("NSc", antisymmetry(&ko, &free_mask(&open)), 1e-1);
    c.finish();
}

// ---------------------------------------------------------------------------------------------------- buckling (a3, a5)

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

/// `(mu_FD, mu_FD without D4_g, vector measure)` along the modes of `b` (ADR 0599 D3).
fn buckling_measures(st: &State, kg: &[Vec<f64>], free: &[bool], b: &BucklingResult) -> (Vec<f64>, Vec<f64>, f64) {
    let mut mu_fd = Vec::new();
    let mut mu_dead = Vec::new();
    let mut vec_worst = 0.0f64;
    for (x, mu) in b.modes.iter().zip(&b.factors) {
        let (xh, _) = unit(x);
        let dr = st.d4(&xh);
        let dg = st.d4g(&xh);
        let kgx = matvec(kg, &xh);
        let num = dot(&xh, &dr);
        let s = dot(&xh, &kgx);
        mu_fd.push(-num / (s + dot(&xh, &dg)));
        mu_dead.push(-num / s);
        let (mut q, mut m) = (0.0f64, 0.0f64);
        for k in (0..xh.len()).filter(|&k| free[k]) {
            q = q.max((dr[k] + mu * (kgx[k] + dg[k])).abs());
            m = m.max(dr[k].abs());
        }
        vec_worst = vec_worst.max(q / m);
    }
    (mu_fd, mu_dead, vec_worst)
}

#[test]
fn tube_buckling_about_the_pressurised_state() {
    let mut c = Checks::new();
    let tube = tube_model(true);
    let n = tube.dof_map().n_equations();
    let fp = FollowerPressure::new(&tube, "xmax", P_T).unwrap();
    let at_zero = AtZero(&fp);
    let list: Vec<&dyn Contribution> = vec![&fp];
    let list_zero: Vec<&dyn Contribution> = vec![&at_zero];
    let Some((u, states)) = tube_state(&mut c, &tube, &fp) else {
        c.finish();
        return;
    };
    let (dead, pre) = dead_prestress(&tube, "xmax", P_T);
    let options = BucklingOptions::new(FACTORS);
    let zero = vec![0.0; n];
    let rest = initial_states(&tube).unwrap();
    let e = |r: Result<BucklingResult, FemError>| r.map_err(|e| e.to_string());
    let bs = e(tangent_buckling_factors_with(
        &tube,
        &[],
        &u,
        &states,
        1.0,
        &list,
        &pre,
        &options,
    ));
    let b0 = e(tangent_buckling_factors_with(
        &tube,
        &[],
        &zero,
        &rest,
        0.0,
        &list,
        &pre,
        &options,
    ));
    let bn = e(tangent_buckling_factors(&tube, &[], &u, &states, &pre, &options));
    let bz = e(tangent_buckling_factors_with(
        &tube,
        &[],
        &u,
        &states,
        1.0,
        &list_zero,
        &pre,
        &options,
    ));
    let bl = e(buckling_factors(&dead, &pre, &[&fp], &options));
    let ok = buckling_ok(&mut c, "BT0t: with the follower at the state", &bs)
        & buckling_ok(&mut c, "BT0t: with the follower at zero load", &b0)
        & buckling_ok(&mut c, "BT0t: without contributions at the state", &bn)
        & buckling_ok(&mut c, "BT0t: with AtZero at the state", &bz)
        & buckling_ok(&mut c, "BT0t: follower::buckling_factors", &bl);
    if !ok {
        c.finish();
        return;
    }
    let (bs, b0, bn, bz, bl) = (bs.unwrap(), b0.unwrap(), bn.unwrap(), bz.unwrap(), bl.unwrap());
    let st = State {
        model: &tube,
        u: u.clone(),
        states: states.clone(),
        fp: &fp,
    };
    let kg = dense(&geometric_stiffness(&tube, &pre, None).unwrap());
    let free = free_mask(&tube);
    let (mu_fd, mu_dead, vec_worst) = buckling_measures(&st, &kg, &free, &bs);
    let (mu_fd_z, _, _) = buckling_measures(&st, &kg, &free, &bz);
    println!(
        "tube: mu_FD {mu_fd:?}, mu {:?}; AtZero mu_FD {mu_fd_z:?}, mu {:?}; linear {:?}",
        bs.factors, bz.factors, bl.factors
    );
    c.at_most("BFD", worst(&mu_fd, &bs.factors), 1e-6);
    c.at_most("BFV", vec_worst, 1e-4);
    c.at_most("B0c", worst(&b0.factors, &bl.factors), 1e-6);
    c.at_most(
        "P1c",
        ((1.0 + bs.factors[0]) - bl.factors[0]).abs() / bl.factors[0],
        1e-1,
    );
    c.at_least("NBc", worst(&bn.factors, &bs.factors), 1e-2);
    c.at_least("NB2c", worst(&mu_dead, &bs.factors), 1e-2);
    c.at_least("NMb", worst(&mu_fd_z, &bz.factors), 1e-5);
    c.finish();
}

// ---------------------------------------------------------------------------------------------------- harmonic (a4, a5)

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
        let (a, b) = (kre[k] - w * beta * kim[k], kim[k] + w * beta * kre[k]);
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

#[test]
fn tube_harmonic_about_the_pressurised_state() {
    let mut c = Checks::new();
    let tube = tube_model(true);
    let n = tube.dof_map().n_equations();
    let fp = FollowerPressure::new(&tube, "xmax", P_T).unwrap();
    let at_zero = AtZero(&fp);
    let list: Vec<&dyn Contribution> = vec![&fp];
    let list_zero: Vec<&dyn Contribution> = vec![&at_zero];
    let Some((u, states)) = tube_state(&mut c, &tube, &fp) else {
        c.finish();
        return;
    };
    let zero = vec![0.0; n];
    let rest = initial_states(&tube).unwrap();
    let ts = tangent_modes(&tube, &[], &u, &states, 1.0, &list, &ModalOptions::new(2));
    let ma = modal_analysis(&tube, &ModalOptions::new(2));
    let (ts, ma) = match (ts, ma) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => {
            c.holds("HT0t: the modes", false, &format!("{:?} {:?}", a.err(), b.err()));
            c.finish();
            return;
        }
    };
    let w1 = 2.0 * PI * ts.frequencies[0];
    let w1r = 2.0 * PI * ma.frequencies[0];
    println!(
        "tube: frequencies at the state {:?} Hz, at rest {:?} Hz",
        ts.frequencies, ma.frequencies
    );
    let free = free_mask(&tube);
    let dofs = tube.dof_map();
    let mut f = vec![0.0; n];
    for &k in tube.mesh().node_set("ymin").unwrap() {
        let eq = dofs.equation(k, 0).unwrap();
        if free[eq] {
            f[eq] += F_TUBE;
        }
    }
    let load = HarmonicLoad::real(f.clone()).unwrap();
    let opt_a = DirectHarmonicOptions::new().threads(1);
    let opt_b = DirectHarmonicOptions::new().threads(1).rayleigh(RAYLEIGH.0, RAYLEIGH.1);
    let run = |solver: Result<DirectHarmonic, FemError>, omega: f64| -> Result<HarmonicResponse, String> {
        solver.and_then(|s| s.response(&load, omega)).map_err(|e| e.to_string())
    };
    let with = |u: &[f64], s: &[ElementState], lf: f64, l: &[&dyn Contribution], o: &DirectHarmonicOptions| {
        DirectHarmonic::about_state_with(&tube, &[], u, s, lf, l, o)
    };
    let hs_a = run(with(&u, &states, 1.0, &list, &opt_a), 0.5 * w1);
    let hs_b = run(with(&u, &states, 1.0, &list, &opt_b), 1.5 * w1);
    let hz_a = run(with(&u, &states, 1.0, &list_zero, &opt_a), 0.5 * w1);
    let hz_b = run(with(&u, &states, 1.0, &list_zero, &opt_b), 1.5 * w1);
    let h0_a = run(with(&zero, &rest, 0.0, &list, &opt_a), 0.5 * w1r);
    let h0_b = run(with(&zero, &rest, 0.0, &list, &opt_b), 1.5 * w1r);
    let hl_a = run(DirectHarmonic::new(&tube, &opt_a), 0.5 * w1r);
    let hl_b = run(DirectHarmonic::new(&tube, &opt_b), 1.5 * w1r);
    let hn_a = run(DirectHarmonic::about_state(&tube, &[], &u, &states, &opt_a), 0.5 * w1);
    let all = [&hs_a, &hs_b, &hz_a, &hz_b, &h0_a, &h0_b, &hl_a, &hl_b, &hn_a];
    let ok = all.iter().all(|r| r.is_ok());
    c.holds(
        "HT0t",
        ok,
        &format!(
            "{:?}",
            all.iter().map(|r| r.as_ref().err().cloned()).collect::<Vec<_>>()
        ),
    );
    if !ok {
        c.finish();
        return;
    }
    let [hs_a, hs_b, hz_a, hz_b, h0_a, h0_b, hl_a, hl_b, hn_a] =
        [hs_a, hs_b, hz_a, hz_b, h0_a, h0_b, hl_a, hl_b, hn_a].map(|r| r.unwrap());
    let mass = dense(&consistent(&tube).unwrap());
    let st = State {
        model: &tube,
        u: u.clone(),
        states: states.clone(),
        fp: &fp,
    };
    let none = (0.0, 0.0);
    c.at_most("HFD omega_a", harmonic_error(&st, &free, &mass, &f, &hs_a, none), 1e-6);
    c.at_most(
        "HFD omega_b",
        harmonic_error(&st, &free, &mass, &f, &hs_b, RAYLEIGH),
        1e-6,
    );
    c.at_most("H0c omega_a", response_difference(&h0_a, &hl_a), 1e-7);
    c.at_most("H0c omega_b", response_difference(&h0_b, &hl_b), 1e-7);
    c.at_least("NHc", response_difference(&hn_a, &hs_a), 1e-2);
    c.at_least("NH2c", harmonic_error(&st, &free, &mass, &f, &hn_a, none), 1e-2);
    c.at_least("NMh omega_a", harmonic_error(&st, &free, &mass, &f, &hz_a, none), 2e-6);
    c.at_least(
        "NMh omega_b",
        harmonic_error(&st, &free, &mass, &f, &hz_b, RAYLEIGH),
        2e-6,
    );
    c.finish();
}
