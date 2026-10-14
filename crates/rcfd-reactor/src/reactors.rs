//! Reactor models on top of a [`Mechanism`]: batch (constant pressure / constant volume), perfectly stirred reactor,
//! constant-area plug flow. State vector everywhere: `[T, Y_1 .. Y_n]` (plus the residence time for plug flow).

use crate::mech::{Mechanism, R_GAS};
use crate::ode::{integrate, Flow, OdeError, Options, Step};
use std::fmt;

/// Energy treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Energy {
    /// Adiabatic: energy equation solved.
    Adiabatic,
    /// Temperature held fixed.
    Isothermal,
}

/// Batch reactor type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchKind {
    /// Constant pressure.
    ConstantPressure,
    /// Constant volume (constant mass density).
    ConstantVolume,
}

/// Integrator tolerances.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tol {
    /// Relative tolerance.
    pub rtol: f64,
    /// Absolute tolerance of the temperature, K.
    pub atol_t: f64,
    /// Absolute tolerance of mass fractions.
    pub atol_y: f64,
}

impl Default for Tol {
    fn default() -> Self {
        Tol {
            rtol: 1e-8,
            atol_t: 1e-8,
            atol_y: 1e-14,
        }
    }
}

impl Tol {
    fn options(&self, n: usize) -> Options {
        let mut o = Options::new(self.rtol, self.atol_y, n);
        o.atol[0] = self.atol_t;
        o
    }
}

/// Reactor failure.
#[derive(Debug, Clone, PartialEq)]
pub enum ReactorError {
    /// Integrator failure.
    Ode(OdeError),
    /// Invalid input or non-physical state.
    Input(String),
}

impl fmt::Display for ReactorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReactorError::Ode(e) => e.fmt(f),
            ReactorError::Input(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for ReactorError {}

impl From<OdeError> for ReactorError {
    fn from(e: OdeError) -> Self {
        ReactorError::Ode(e)
    }
}

/// Accepted-step history of a run.
#[derive(Debug, Clone, Default)]
pub struct Trajectory {
    /// Independent variable (time, or position for plug flow).
    pub t: Vec<f64>,
    /// Full state per node.
    pub state: Vec<Vec<f64>>,
    /// Derivative of the temperature per node.
    pub dtemp: Vec<f64>,
}

impl Trajectory {
    /// Temperature at node `i`.
    pub fn temp(&self, i: usize) -> f64 {
        self.state[i][0]
    }

    /// Final state.
    pub fn last(&self) -> &[f64] {
        self.state.last().expect("non-empty trajectory")
    }

    /// Time of the maximum temperature derivative, refined on the Hermite interpolant of T.
    pub fn time_of_max_dtdt(&self) -> Option<f64> {
        let n = self.t.len();
        if n < 3 {
            return None;
        }
        let imax = (0..n).max_by(|&a, &b| self.dtemp[a].total_cmp(&self.dtemp[b]))?;
        let mut best = (self.dtemp[imax], self.t[imax]);
        for iv in [imax.checked_sub(1), Some(imax)].into_iter().flatten() {
            if iv + 1 >= n {
                continue;
            }
            let (ta, tb) = (self.t[iv], self.t[iv + 1]);
            let h = tb - ta;
            let (t0, t1, m0, m1) = (self.temp(iv), self.temp(iv + 1), self.dtemp[iv], self.dtemp[iv + 1]);
            // derivative of the Hermite polynomial as a quadratic in s: fit through s = 0, 0.5, 1
            let g = |s: f64| {
                let h00 = 6.0 * s * s - 6.0 * s;
                let h01 = -h00;
                (h00 * t0 + h01 * t1) / h + (3.0 * s * s - 4.0 * s + 1.0) * m0 + (3.0 * s * s - 2.0 * s) * m1
            };
            let (g0, gh, g1) = (g(0.0), g(0.5), g(1.0));
            let a = 2.0 * g1 + 2.0 * g0 - 4.0 * gh;
            let b = g1 - g0 - a;
            let c = g0;
            if a < 0.0 {
                let sv = -b / (2.0 * a);
                if sv > 0.0 && sv < 1.0 {
                    let v = a * sv * sv + b * sv + c;
                    if v > best.0 {
                        best = (v, ta + sv * h);
                    }
                }
            }
        }
        Some(best.1)
    }

    /// First time the temperature exceeds `t_init + dt` (linear interpolation on the Hermite interpolant).
    pub fn time_of_rise(&self, rise: f64) -> Option<f64> {
        let target = self.temp(0) + rise;
        for i in 1..self.t.len() {
            if self.temp(i) >= target {
                let (mut lo, mut hi) = (self.t[i - 1], self.t[i]);
                let (y0, y1, f0, f1) = (&self.state[i - 1], &self.state[i], [self.dtemp[i - 1]], [self.dtemp[i]]);
                let y0 = [y0[0]];
                let y1 = [y1[0]];
                let st = Step {
                    t0: lo,
                    t1: hi,
                    y0: &y0,
                    y1: &y1,
                    f0: &f0,
                    f1: &f1,
                };
                for _ in 0..80 {
                    let mid = 0.5 * (lo + hi);
                    if st.hermite(0, mid) >= target {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                return Some(0.5 * (lo + hi));
            }
        }
        None
    }
}

fn clip(y: f64) -> f64 {
    y.max(0.0)
}

/// Batch reactor right-hand side. `param` is the pressure (constant pressure) or the mass density (constant volume).
pub fn batch_rhs(mech: &Mechanism, kind: BatchKind, energy: Energy, param: f64, y: &[f64], f: &mut [f64]) {
    let ns = mech.species.len();
    let t = y[0];
    let yk: Vec<f64> = y[1..].iter().map(|v| clip(*v)).collect();
    let wbar = mech.mean_w(&yk);
    let rho = match kind {
        BatchKind::ConstantPressure => param * wbar / (R_GAS * t),
        BatchKind::ConstantVolume => param,
    };
    let c: Vec<f64> = (0..ns).map(|k| rho * yk[k] / mech.species[k].w).collect();
    let mut wdot = vec![0.0; ns];
    mech.production_rates(t, &c, &mut wdot);
    for k in 0..ns {
        f[1 + k] = wdot[k] * mech.species[k].w / rho;
    }
    f[0] = match energy {
        Energy::Isothermal => 0.0,
        Energy::Adiabatic => {
            let mut q = 0.0;
            for k in 0..ns {
                let hk = R_GAS * t * mech.species[k].h_rt(t);
                q += match kind {
                    BatchKind::ConstantPressure => hk,
                    BatchKind::ConstantVolume => hk - R_GAS * t,
                } * wdot[k];
            }
            let cx = match kind {
                BatchKind::ConstantPressure => mech.cp_mass(t, &yk),
                BatchKind::ConstantVolume => mech.cv_mass(t, &yk),
            };
            -q / (rho * cx)
        }
    };
}

/// Stop rule of [`run_batch`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stop {
    /// Run to the end time.
    EndTime,
    /// Stop after the ignition event: the temperature has risen by `rise` K above the start and dT/dt has fallen to
    /// a tenth of its maximum.
    AfterIgnition {
        /// Temperature rise that counts as ignition.
        rise: f64,
    },
}

/// Run a batch reactor from `y0 = [T, Y...]` at pressure `p` (constant pressure) or at the density that `p` and `y0`
/// imply (constant volume).
pub fn run_batch(
    mech: &Mechanism,
    kind: BatchKind,
    energy: Energy,
    p: f64,
    y0: &[f64],
    t_end: f64,
    tol: Tol,
    stop: Stop,
) -> Result<Trajectory, ReactorError> {
    let ns = mech.species.len();
    if y0.len() != ns + 1 {
        return Err(ReactorError::Input(format!(
            "state has {} entries, expected {}",
            y0.len(),
            ns + 1
        )));
    }
    let wbar0 = mech.mean_w(&y0[1..]);
    let param = match kind {
        BatchKind::ConstantPressure => p,
        BatchKind::ConstantVolume => p * wbar0 / (R_GAS * y0[0]),
    };
    let mut traj = Trajectory::default();
    let mut f0 = vec![0.0; ns + 1];
    batch_rhs(mech, kind, energy, param, y0, &mut f0);
    traj.t.push(0.0);
    traj.state.push(y0.to_vec());
    traj.dtemp.push(f0[0]);
    let mut rhs = |_t: f64, y: &[f64], f: &mut [f64]| batch_rhs(mech, kind, energy, param, y, f);
    let mut fmax = f0[0];
    let t_init = y0[0];
    let mut obs = |s: &Step<'_>| {
        traj.t.push(s.t1);
        traj.state.push(s.y1.to_vec());
        traj.dtemp.push(s.f1[0]);
        fmax = fmax.max(s.f1[0]);
        if let Stop::AfterIgnition { rise } = stop {
            if s.y1[0] - t_init > rise && s.f1[0] < 0.1 * fmax {
                return Flow::Stop;
            }
        }
        Flow::Continue
    };
    integrate(&mut rhs, 0.0, t_end, y0, &tol.options(ns + 1), &mut obs)?;
    Ok(traj)
}

/// Ignition delay of a batch run by the maximum-dT/dt definition. `None` if the run did not ignite.
pub fn ignition_delay_max_dtdt(
    mech: &Mechanism,
    kind: BatchKind,
    p: f64,
    y0: &[f64],
    t_end: f64,
    tol: Tol,
) -> Result<Option<f64>, ReactorError> {
    let tr = run_batch(
        mech,
        kind,
        Energy::Adiabatic,
        p,
        y0,
        t_end,
        tol,
        Stop::AfterIgnition { rise: 400.0 },
    )?;
    if tr.last()[0] - y0[0] < 400.0 {
        return Ok(None);
    }
    Ok(tr.time_of_max_dtdt())
}

/// Perfectly stirred reactor specification (constant pressure, prescribed residence time `tau = rho V / mdot`).
#[derive(Debug, Clone)]
pub struct Psr {
    /// Residence time, s.
    pub tau: f64,
    /// Pressure, Pa.
    pub p: f64,
    /// Inlet temperature, K.
    pub t_in: f64,
    /// Inlet mass fractions.
    pub y_in: Vec<f64>,
    /// Energy treatment.
    pub energy: Energy,
}

/// PSR right-hand side.
pub fn psr_rhs(mech: &Mechanism, spec: &Psr, y: &[f64], f: &mut [f64]) {
    let ns = mech.species.len();
    let t = y[0];
    let yk: Vec<f64> = y[1..].iter().map(|v| clip(*v)).collect();
    let wbar = mech.mean_w(&yk);
    let rho = spec.p * wbar / (R_GAS * t);
    let c: Vec<f64> = (0..ns).map(|k| rho * yk[k] / mech.species[k].w).collect();
    let mut wdot = vec![0.0; ns];
    mech.production_rates(t, &c, &mut wdot);
    let mut q = 0.0;
    let mut flow = 0.0;
    for k in 0..ns {
        let w = mech.species[k].w;
        f[1 + k] = (spec.y_in[k] - y[1 + k]) / spec.tau + wdot[k] * w / rho;
        let hk = R_GAS * t * mech.species[k].h_rt(t);
        let hin = R_GAS * spec.t_in * mech.species[k].h_rt(spec.t_in);
        q += hk * wdot[k];
        flow += spec.y_in[k] * (hin - hk) / w;
    }
    f[0] = match spec.energy {
        Energy::Isothermal => 0.0,
        Energy::Adiabatic => (flow / spec.tau - q / rho) / mech.cp_mass(t, &yk),
    };
}

/// Integrate the PSR equations in pseudo-time until the state stops changing; returns the steady state.
/// `converged` is false if the budget of chunks ran out.
pub fn psr_steady(mech: &Mechanism, spec: &Psr, guess: &[f64], tol: Tol) -> Result<(Vec<f64>, bool), ReactorError> {
    let ns = mech.species.len();
    let mut y = guess.to_vec();
    let mut rhs = |_t: f64, yy: &[f64], f: &mut [f64]| psr_rhs(mech, spec, yy, f);
    for _ in 0..600 {
        let (yn, _) = integrate(&mut rhs, 0.0, 5.0 * spec.tau, &y, &tol.options(ns + 1), &mut |_| {
            Flow::Continue
        })?;
        let dt = (yn[0] - y[0]).abs();
        let dy = yn[1..]
            .iter()
            .zip(&y[1..])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        y = yn;
        if dt < 1e-7 && dy < 1e-12 {
            return Ok((y, true));
        }
    }
    Ok((y, false))
}

/// Result of a blowout search.
#[derive(Debug, Clone, PartialEq)]
pub struct Blowout {
    /// Smallest residence time found that still burns.
    pub tau_burning: f64,
    /// Largest residence time found that is extinguished.
    pub tau_extinct: f64,
    /// Steady state at `tau_burning`.
    pub state: Vec<f64>,
}

/// Find the PSR blowout residence time by continuation from the burning state at `tau_start` downwards
/// (factor 0.9 per step), then bisection in log tau until `tau_burning / tau_extinct` is within `1 + bracket`.
/// `t_burn` is the temperature above which the reactor counts as burning. Returns `Err` if `tau_start` does not burn.
pub fn psr_blowout(
    mech: &Mechanism,
    base: &Psr,
    tau_start: f64,
    t_guess: &[f64],
    t_burn: f64,
    bracket: f64,
    tol: Tol,
) -> Result<Blowout, ReactorError> {
    let mut spec = base.clone();
    spec.tau = tau_start;
    let (mut state, ok) = psr_steady(mech, &spec, t_guess, tol)?;
    if !ok || state[0] < t_burn {
        return Err(ReactorError::Input(format!(
            "no burning PSR solution at tau = {tau_start:e} s"
        )));
    }
    let mut tau_hi = tau_start;
    let mut tau_lo;
    loop {
        tau_lo = tau_hi * 0.9;
        spec.tau = tau_lo;
        if tau_lo < 1e-9 {
            return Err(ReactorError::Input("PSR does not blow out above tau = 1e-9 s".into()));
        }
        let (s, ok) = psr_steady(mech, &spec, &state, tol)?;
        if ok && s[0] >= t_burn {
            state = s;
            tau_hi = tau_lo;
        } else {
            break;
        }
    }
    // tau_hi burns (state), tau_lo is extinct
    while tau_hi / tau_lo > 1.0 + bracket {
        let mid = (tau_hi * tau_lo).sqrt();
        spec.tau = mid;
        let (s, ok) = psr_steady(mech, &spec, &state, tol)?;
        if ok && s[0] >= t_burn {
            state = s;
            tau_hi = mid;
        } else {
            tau_lo = mid;
        }
    }
    Ok(Blowout {
        tau_burning: tau_hi,
        tau_extinct: tau_lo,
        state,
    })
}

/// Constant-area, constant-pressure, steady plug flow. The result's independent variable is x; the last state
/// component is the residence time.
pub fn run_plug_flow(
    mech: &Mechanism,
    energy: Energy,
    p: f64,
    y0: &[f64],
    velocity: f64,
    length: f64,
    tol: Tol,
) -> Result<Trajectory, ReactorError> {
    let ns = mech.species.len();
    let rho0 = p * mech.mean_w(&y0[1..]) / (R_GAS * y0[0]);
    let mdot_flux = rho0 * velocity;
    let mut state0 = y0.to_vec();
    state0.push(0.0);
    let mut rhs = |_x: f64, y: &[f64], f: &mut [f64]| {
        batch_rhs(
            mech,
            BatchKind::ConstantPressure,
            energy,
            p,
            &y[..ns + 1],
            &mut f[..ns + 1],
        );
        let yk: Vec<f64> = y[1..=ns].iter().map(|v| clip(*v)).collect();
        let rho = p * mech.mean_w(&yk) / (R_GAS * y[0]);
        let dtdx = rho / mdot_flux; // dt/dx = 1/u
        for v in f[..ns + 1].iter_mut() {
            *v *= dtdx;
        }
        f[ns + 1] = dtdx;
    };
    let mut o = tol.options(ns + 2);
    o.atol[ns + 1] = 1e-12;
    let mut traj = Trajectory::default();
    let mut f0 = vec![0.0; ns + 2];
    rhs(0.0, &state0, &mut f0);
    traj.t.push(0.0);
    traj.state.push(state0.clone());
    traj.dtemp.push(f0[0]);
    let mut obs = |s: &Step<'_>| {
        traj.t.push(s.t1);
        traj.state.push(s.y1.to_vec());
        traj.dtemp.push(s.f1[0]);
        Flow::Continue
    };
    integrate(&mut rhs, 0.0, length, &state0, &o, &mut obs)?;
    Ok(traj)
}
