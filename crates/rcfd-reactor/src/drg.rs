//! Dynamic mechanism reduction by the directed relation graph (Lu and Law 2005; roadmap F-223, ADR 0053 D2).
//!
//! Interaction coefficient of species `A` on `B` at a state: `r_AB = sum_{i involves B} |nu_{A,i} q_i| / sum_i |nu_{A,i} q_i|`, with `q_i`
//! the net rate of progress of reaction `i` and `nu_{A,i}` the net stoichiometric coefficient of `A` in it. The active set is
//! everything reachable from the target species along edges `r_AB >= eps`. The reduced mechanism keeps the full species list (for the
//! thermodynamic sums and the third-body concentrations) and the reactions whose participants are all active; the ODE is integrated
//! over the active variables only, the inactive mass fractions are frozen.

use crate::mech::{Mechanism, R_GAS};
use crate::ode::{integrate, Flow, Options, Step};
use crate::reactors::{batch_rhs, BatchKind, Energy, ReactorError, Stop, Tol, Trajectory};

/// Concentrations (mol/m3) of a state `[T, Y]` for a batch of the given kind (`param` = pressure or density).
pub fn concentrations(mech: &Mechanism, kind: BatchKind, param: f64, phi: &[f64]) -> Vec<f64> {
    let y: Vec<f64> = phi[1..].iter().map(|v| v.max(0.0)).collect();
    let rho = match kind {
        BatchKind::ConstantPressure => param * mech.mean_w(&y) / (R_GAS * phi[0]),
        BatchKind::ConstantVolume => param,
    };
    y.iter().zip(&mech.species).map(|(y, s)| rho * y / s.w).collect()
}

/// Directed graph of the interaction coefficients at the state `(t, c)`: `graph[a]` lists `(b, r_ab)` for `b != a`.
pub fn interaction_graph(mech: &Mechanism, t: f64, c: &[f64]) -> Vec<Vec<(usize, f64)>> {
    let ns = mech.species.len();
    let mut q = vec![0.0; mech.reactions.len()];
    mech.rates_of_progress(t, c, &mut q);
    let mut den = vec![0.0; ns];
    let mut num: Vec<Vec<(usize, f64)>> = vec![Vec::new(); ns];
    for (r, qi) in mech.reactions.iter().zip(&q) {
        // net coefficient of every participating species, and the participant list
        let mut part: Vec<(usize, f64)> = Vec::new();
        for &(k, nu) in &r.products {
            match part.iter_mut().find(|p| p.0 == k) {
                Some(p) => p.1 += nu,
                None => part.push((k, nu)),
            }
        }
        for &(k, nu) in &r.reactants {
            match part.iter_mut().find(|p| p.0 == k) {
                Some(p) => p.1 -= nu,
                None => part.push((k, -nu)),
            }
        }
        for &(a, nu_a) in &part {
            let w = (nu_a * qi).abs();
            if w == 0.0 {
                continue;
            }
            den[a] += w;
            for &(b, _) in &part {
                if b == a {
                    continue;
                }
                match num[a].iter_mut().find(|e| e.0 == b) {
                    Some(e) => e.1 += w,
                    None => num[a].push((b, w)),
                }
            }
        }
    }
    num.into_iter()
        .enumerate()
        .map(|(a, v)| {
            v.into_iter()
                .map(|(b, w)| (b, if den[a] > 0.0 { w / den[a] } else { 0.0 }))
                .collect()
        })
        .collect()
}

/// Species reachable from `targets` along edges with coefficient `>= eps`.
pub fn active_set(graph: &[Vec<(usize, f64)>], targets: &[usize], eps: f64) -> Vec<bool> {
    let mut on = vec![false; graph.len()];
    let mut stack: Vec<usize> = Vec::new();
    for &t in targets {
        if !on[t] {
            on[t] = true;
            stack.push(t);
        }
    }
    while let Some(a) = stack.pop() {
        for &(b, r) in &graph[a] {
            if r >= eps && !on[b] {
                on[b] = true;
                stack.push(b);
            }
        }
    }
    on
}

/// The mechanism with only the reactions whose participants are all active (species list unchanged).
pub fn reduced_mechanism(mech: &Mechanism, active: &[bool]) -> Mechanism {
    Mechanism {
        elements: mech.elements.clone(),
        species: mech.species.clone(),
        reactions: mech
            .reactions
            .iter()
            .filter(|r| r.reactants.iter().chain(&r.products).all(|&(k, _)| active[k]))
            .cloned()
            .collect(),
    }
}

/// Union of the active sets over sampled states (a static skeletal mechanism).
pub fn skeletal_active(
    mech: &Mechanism,
    kind: BatchKind,
    param: f64,
    states: &[Vec<f64>],
    targets: &[usize],
    eps: f64,
) -> Vec<bool> {
    let mut on = vec![false; mech.species.len()];
    for st in states {
        let c = concentrations(mech, kind, param, st);
        let g = interaction_graph(mech, st[0], &c);
        for (o, a) in on.iter_mut().zip(active_set(&g, targets, eps)) {
            *o |= a;
        }
    }
    on
}

fn active_index(active: &[bool]) -> Vec<usize> {
    let mut idx = vec![0usize];
    idx.extend(active.iter().enumerate().filter(|(_, a)| **a).map(|(k, _)| k + 1));
    idx
}

fn options(tol: Tol, idx: &[usize]) -> Options {
    let mut o = Options::new(tol.rtol, tol.atol_y, idx.len());
    o.atol[0] = tol.atol_t;
    o
}

/// Advance the state over `dt` with the reduced mechanism `red` integrating the active variables only. Returns the state and the
/// right-hand-side evaluation count.
pub fn advance_active(
    red: &Mechanism,
    kind: BatchKind,
    energy: Energy,
    param: f64,
    phi: &[f64],
    dt: f64,
    active: &[bool],
    tol: Tol,
) -> Result<(Vec<f64>, usize), ReactorError> {
    let idx = active_index(active);
    let y0: Vec<f64> = idx.iter().map(|&i| phi[i]).collect();
    let mut full = phi.to_vec();
    let mut ff = vec![0.0; phi.len()];
    let mut rhs = |_t: f64, y: &[f64], f: &mut [f64]| {
        for (j, &i) in idx.iter().enumerate() {
            full[i] = y[j];
        }
        batch_rhs(red, kind, energy, param, &full, &mut ff);
        for (j, &i) in idx.iter().enumerate() {
            f[j] = ff[i];
        }
    };
    let (y, st) = integrate(&mut rhs, 0.0, dt, &y0, &options(tol, &idx), &mut |_| Flow::Continue)?;
    let mut out = phi.to_vec();
    for (j, &i) in idx.iter().enumerate() {
        out[i] = y[j];
    }
    if out.iter().any(|v| !v.is_finite()) {
        return Err(ReactorError::Input("non-finite state after the reduced step".into()));
    }
    Ok((out, st.rhs_evals))
}

/// Batch run with a fixed reduced mechanism over the active variables, same stop rules and trajectory as `reactors::run_batch`.
pub fn run_batch_active(
    red: &Mechanism,
    active: &[bool],
    kind: BatchKind,
    energy: Energy,
    p: f64,
    y0: &[f64],
    t_end: f64,
    tol: Tol,
    stop: Stop,
) -> Result<Trajectory, ReactorError> {
    let ns = red.species.len();
    if y0.len() != ns + 1 {
        return Err(ReactorError::Input(format!(
            "state has {} entries, expected {}",
            y0.len(),
            ns + 1
        )));
    }
    let param = match kind {
        BatchKind::ConstantPressure => p,
        BatchKind::ConstantVolume => p * red.mean_w(&y0[1..]) / (R_GAS * y0[0]),
    };
    let idx = active_index(active);
    let mut full = y0.to_vec();
    let mut ff = vec![0.0; ns + 1];
    let mut traj = Trajectory::default();
    batch_rhs(red, kind, energy, param, y0, &mut ff);
    traj.t.push(0.0);
    traj.state.push(y0.to_vec());
    traj.dtemp.push(ff[0]);
    let mut fmax = ff[0];
    let t_init = y0[0];
    let ya: Vec<f64> = idx.iter().map(|&i| y0[i]).collect();
    let mut rhs = |_t: f64, y: &[f64], f: &mut [f64]| {
        for (j, &i) in idx.iter().enumerate() {
            full[i] = y[j];
        }
        batch_rhs(red, kind, energy, param, &full, &mut ff);
        for (j, &i) in idx.iter().enumerate() {
            f[j] = ff[i];
        }
    };
    let base = y0.to_vec();
    let mut obs = |s: &Step<'_>| {
        let mut st = base.clone();
        for (j, &i) in idx.iter().enumerate() {
            st[i] = s.y1[j];
        }
        traj.t.push(s.t1);
        traj.state.push(st);
        traj.dtemp.push(s.f1[0]);
        fmax = fmax.max(s.f1[0]);
        if let Stop::AfterIgnition { rise } = stop {
            if s.y1[0] - t_init > rise && s.f1[0] < 0.1 * fmax {
                return Flow::Stop;
            }
        }
        Flow::Continue
    };
    integrate(&mut rhs, 0.0, t_end, &ya, &options(tol, &idx), &mut obs)?;
    Ok(traj)
}

/// In-situ reduction: at every step the active set is built from the current state and the step is integrated with it.
pub struct DrgStepper<'a> {
    /// Full mechanism.
    pub mech: &'a Mechanism,
    /// Target species (indices).
    pub targets: Vec<usize>,
    /// Threshold.
    pub eps: f64,
    /// Reactor kind.
    pub kind: BatchKind,
    /// Pressure or density as in `batch_rhs`.
    pub param: f64,
    /// Tolerances.
    pub tol: Tol,
    /// Accumulated active species counts and steps (for the mean).
    pub active_sum: usize,
    /// Steps taken.
    pub steps: usize,
    /// Right-hand-side evaluations.
    pub rhs_evals: usize,
}

impl<'a> DrgStepper<'a> {
    /// New stepper with zeroed counters.
    pub fn new(mech: &'a Mechanism, targets: Vec<usize>, eps: f64, kind: BatchKind, param: f64, tol: Tol) -> Self {
        DrgStepper {
            mech,
            targets,
            eps,
            kind,
            param,
            tol,
            active_sum: 0,
            steps: 0,
            rhs_evals: 0,
        }
    }

    /// One adiabatic step of size `dt`.
    pub fn step(&mut self, phi: &[f64], dt: f64) -> Result<Vec<f64>, ReactorError> {
        let c = concentrations(self.mech, self.kind, self.param, phi);
        let g = interaction_graph(self.mech, phi[0], &c);
        let act = active_set(&g, &self.targets, self.eps);
        let red = reduced_mechanism(self.mech, &act);
        let (out, n) = advance_active(&red, self.kind, Energy::Adiabatic, self.param, phi, dt, &act, self.tol)?;
        self.active_sum += act.iter().filter(|a| **a).count();
        self.steps += 1;
        self.rhs_evals += n;
        Ok(out)
    }

    /// Mean number of active species per step.
    pub fn mean_active(&self) -> f64 {
        self.active_sum as f64 / self.steps.max(1) as f64
    }
}
