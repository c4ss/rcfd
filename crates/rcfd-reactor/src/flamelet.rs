//! Steady laminar flamelet library (Peters) in mixture-fraction space (roadmap F-129, ADR 0053 D6).
//!
//! Equations at unity Lewis number, constant pressure, adiabatic:
//! `(chi(Z)/2) Y_k'' + W_k wdot_k / rho = 0`, with the enthalpy linear in `Z` (exact for this closure) and the temperature recovered from
//! the enthalpy and the composition. The scalar dissipation profile is `chi(Z) = chi_st g(Z)/g(Z_st)`, `g(Z) = exp(-2 [erfc^-1(2Z)]^2)`.
//! The discretisation is the non-uniform three-point second derivative; the nonlinear system is solved by Newton's method with
//! backtracking, falling back on implicit pseudo-time stepping with a growing step; the Jacobian is block tridiagonal (finite
//! differences of the local source, scalar neighbour coupling) and is solved by block Thomas elimination with dense LU blocks.

use crate::mech::{Mechanism, R_GAS};
use crate::mixfrac::{erfc_inv, integrate_beta, interp, temperature_from_h, MixError, MixtureTable, Stream};
use crate::ode::{lu_factor, lu_solve};

/// Error of the flamelet solver; the text names the cause.
#[derive(Debug, Clone, PartialEq)]
pub struct FlameletError(pub String);

impl std::fmt::Display for FlameletError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for FlameletError {}

impl From<MixError> for FlameletError {
    fn from(e: MixError) -> Self {
        FlameletError(e.0)
    }
}

/// Scalar dissipation profile `chi(Z) = chi_st g(Z)/g(Z_st)`, `g(Z) = exp(-2 [erfc^-1(2 Z)]^2)`; zero at `Z = 0` and `Z = 1`.
pub fn chi_profile(z: f64, chi_st: f64, z_st: f64) -> f64 {
    let g = |z: f64| {
        if z <= 0.0 || z >= 1.0 {
            0.0
        } else {
            let e = erfc_inv(2.0 * z);
            (-2.0 * e * e).exp()
        }
    };
    chi_st * g(z) / g(z_st)
}

/// Solver controls.
#[derive(Debug, Clone, Copy)]
pub struct FlameletOptions {
    /// Convergence: largest Newton update of a mass fraction below this (the scaled residual).
    pub tol: f64,
    /// Newton iterations of one attempt.
    pub max_newton: usize,
    /// Pseudo-time steps.
    pub max_transient: usize,
    /// First pseudo-time step, s.
    pub dtau0: f64,
}

impl Default for FlameletOptions {
    fn default() -> Self {
        FlameletOptions {
            tol: 1e-10,
            max_newton: 25,
            max_transient: 600,
            dtau0: 1e-8,
        }
    }
}

/// A converged flamelet.
#[derive(Debug, Clone, PartialEq)]
pub struct FlameletSolution {
    /// Mixture-fraction nodes.
    pub z: Vec<f64>,
    /// Temperature at the nodes.
    pub t: Vec<f64>,
    /// Mass fractions, node-major: `y[i][k]`.
    pub y: Vec<Vec<f64>>,
    /// Newton iterations (Jacobian factorisations with `sigma = 0`) in the last, converging attempt.
    pub newton_iters: usize,
    /// Pseudo-time steps taken.
    pub transient_steps: usize,
    /// Largest Newton update of a mass fraction at the last iteration (the scaled residual).
    pub residual: f64,
}

impl FlameletSolution {
    /// Peak temperature.
    pub fn t_max(&self) -> f64 {
        self.t.iter().cloned().fold(0.0, f64::max)
    }

    /// As a [`MixtureTable`] (species-major rows).
    pub fn to_table(&self) -> MixtureTable {
        let ns = self.y[0].len();
        let y = (0..ns).map(|k| self.y.iter().map(|r| r[k]).collect()).collect();
        MixtureTable {
            z: self.z.clone(),
            t: self.t.clone(),
            y,
        }
    }
}

struct Problem<'a> {
    mech: &'a Mechanism,
    p: f64,
    z: &'a [f64],
    chi: &'a [f64],
    h: Vec<f64>,
    n: usize,
}

impl Problem<'_> {
    /// Temperature, density-weighted source `S_k = W_k wdot_k/rho` at node `i` for mass fractions `y`.
    fn source(&self, i: usize, y: &[f64], tg: f64, s: &mut [f64]) -> Result<f64, FlameletError> {
        let mech = self.mech;
        let yp: Vec<f64> = y.iter().map(|v| v.max(0.0)).collect();
        let t = temperature_from_h(mech, self.h[i], &yp, tg)?;
        let rho = self.p * mech.mean_w(&yp) / (R_GAS * t);
        let c: Vec<f64> = yp.iter().zip(&mech.species).map(|(y, sp)| rho * y / sp.w).collect();
        let mut wdot = vec![0.0; self.n];
        mech.production_rates(t, &c, &mut wdot);
        for k in 0..self.n {
            s[k] = mech.species[k].w * wdot[k] / rho;
        }
        Ok(t)
    }

    fn coeffs(&self, i: usize) -> (f64, f64, f64) {
        let hm = self.z[i] - self.z[i - 1];
        let hp = self.z[i + 1] - self.z[i];
        let cm = 2.0 / ((hm + hp) * hm);
        let cp = 2.0 / ((hm + hp) * hp);
        (cm, cp, cm + cp)
    }

    /// Residual `F` of the interior nodes (node-major, `(N-1) n`) for the full profile `y` (node-major, `(N+1) n`).
    fn residual(&self, y: &[f64], tg: &mut [f64]) -> Result<Vec<f64>, FlameletError> {
        let n = self.n;
        let nn = self.z.len();
        let mut f = vec![0.0; (nn - 2) * n];
        let mut s = vec![0.0; n];
        for i in 1..nn - 1 {
            let (cm, cp, c0) = self.coeffs(i);
            let t = self.source(i, &y[i * n..(i + 1) * n], tg[i], &mut s)?;
            tg[i] = t;
            for k in 0..n {
                f[(i - 1) * n + k] =
                    0.5 * self.chi[i] * (cm * y[(i - 1) * n + k] + cp * y[(i + 1) * n + k] - c0 * y[i * n + k]) + s[k];
            }
        }
        Ok(f)
    }

    /// Solve `(sigma I - J) delta = F` by block Thomas elimination. Returns the update of the interior nodes.
    fn newton_step(&self, y: &[f64], sigma: f64, f: &[f64], tg: &mut [f64]) -> Result<Vec<f64>, FlameletError> {
        let n = self.n;
        let m = self.z.len() - 2; // interior nodes
        let mut inv_prev: Vec<f64> = Vec::new();
        let mut rp_prev: Vec<f64> = Vec::new();
        let mut invs: Vec<Vec<f64>> = Vec::with_capacity(m);
        let mut rps: Vec<Vec<f64>> = Vec::with_capacity(m);
        let mut s0 = vec![0.0; n];
        let mut s1 = vec![0.0; n];
        for q in 0..m {
            let i = q + 1;
            let (cm, _, c0) = self.coeffs(i);
            let half = 0.5 * self.chi[i];
            // local Jacobian by forward differences
            let yi = &y[i * n..(i + 1) * n];
            let t0 = self.source(i, yi, tg[i], &mut s0)?;
            tg[i] = t0;
            let mut d = vec![0.0; n * n];
            let mut yp = yi.to_vec();
            for l in 0..n {
                let dl = 1e-7 * yi[l].abs().max(1e-6);
                yp[l] = yi[l] + dl;
                self.source(i, &yp, t0, &mut s1)?;
                yp[l] = yi[l];
                for k in 0..n {
                    d[k * n + l] = -(s1[k] - s0[k]) / dl;
                }
            }
            for k in 0..n {
                d[k * n + k] += sigma + half * c0;
            }
            let lo = -half * cm; // M[i][i-1]
            let mut r = f[q * n..(q + 1) * n].to_vec();
            if q > 0 {
                let up_prev = -0.5 * self.chi[i - 1] * self.coeffs(i - 1).1; // M[i-1][i]
                let fac = lo * up_prev;
                for a in 0..n * n {
                    d[a] -= fac * inv_prev[a];
                }
                // r -= lo * inv_prev r'_prev
                for a in 0..n {
                    let mut v = 0.0;
                    for b in 0..n {
                        v += inv_prev[a * n + b] * rp_prev[b];
                    }
                    r[a] -= lo * v;
                }
            }
            // invert d
            let mut lu = d.clone();
            let mut piv = vec![0usize; n];
            if !lu_factor(&mut lu, n, &mut piv) {
                return Err(FlameletError(format!(
                    "singular Jacobian block at node {i} (z = {})",
                    self.z[i]
                )));
            }
            let mut inv = vec![0.0; n * n];
            let mut col = vec![0.0; n];
            for c in 0..n {
                col.iter_mut().for_each(|v| *v = 0.0);
                col[c] = 1.0;
                lu_solve(&lu, n, &piv, &mut col);
                for a in 0..n {
                    inv[a * n + c] = col[a];
                }
            }
            inv_prev = inv.clone();
            rp_prev = r.clone();
            invs.push(inv);
            rps.push(r);
        }
        // back substitution
        let mut delta = vec![0.0; m * n];
        for q in (0..m).rev() {
            let i = q + 1;
            let mut rhs = rps[q].clone();
            if q + 1 < m {
                let up = -0.5 * self.chi[i] * self.coeffs(i).1;
                for a in 0..n {
                    rhs[a] -= up * delta[(q + 1) * n + a];
                }
            }
            for a in 0..n {
                let mut v = 0.0;
                for b in 0..n {
                    v += invs[q][a * n + b] * rhs[b];
                }
                delta[q * n + a] = v;
            }
        }
        Ok(delta)
    }
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |a, b| a.max(b.abs()))
}

/// Solve the flamelet on the grid `zgrid` (strictly increasing, from 0 to 1) with the per-node scalar dissipation `chi` (1/s). The
/// guess is a node-major profile `(N+1) x n` of mass fractions (the mixing line when `None`); the end nodes are the pure streams.
pub fn solve(
    mech: &Mechanism,
    p: f64,
    fuel: &Stream,
    oxid: &Stream,
    zgrid: &[f64],
    chi: &[f64],
    guess: Option<&[Vec<f64>]>,
    opts: &FlameletOptions,
) -> Result<FlameletSolution, FlameletError> {
    let nn = zgrid.len();
    let n = mech.species.len();
    if nn < 3 {
        return Err(FlameletError(format!("z grid has {nn} nodes, at least 3 are needed")));
    }
    if chi.len() != nn {
        return Err(FlameletError(format!(
            "chi has {} entries, the grid has {nn} nodes",
            chi.len()
        )));
    }
    if zgrid[0] != 0.0 || zgrid[nn - 1] != 1.0 {
        return Err(FlameletError(format!(
            "z grid must run from 0 to 1 (got {} to {})",
            zgrid[0],
            zgrid[nn - 1]
        )));
    }
    for i in 1..nn {
        if !(zgrid[i] > zgrid[i - 1]) {
            return Err(FlameletError(format!("z grid is not strictly increasing at index {i}")));
        }
    }
    let hf = fuel.h(mech);
    let ho = oxid.h(mech);
    let pr = Problem {
        mech,
        p,
        z: zgrid,
        chi,
        h: zgrid.iter().map(|&z| z * hf + (1.0 - z) * ho).collect(),
        n,
    };
    let mut y = vec![0.0; nn * n];
    for i in 0..nn {
        for k in 0..n {
            y[i * n + k] = match guess {
                Some(g) => g[i][k],
                None => zgrid[i] * fuel.y[k] + (1.0 - zgrid[i]) * oxid.y[k],
            };
        }
    }
    for k in 0..n {
        y[k] = oxid.y[k];
        y[(nn - 1) * n + k] = fuel.y[k];
    }
    let mut tg: Vec<f64> = vec![0.5 * (fuel.t + oxid.t); nn];
    let mut steps = 0usize;
    let mut last_res = f64::INFINITY;

    // pure Newton with backtracking on the max-norm of the Newton update; returns Some(iters) when converged
    let newton = |y: &mut Vec<f64>,
                  tg: &mut Vec<f64>,
                  iters: usize,
                  last_res: &mut f64|
     -> Result<Option<usize>, FlameletError> {
        for it in 1..=iters {
            let f = match pr.residual(y, tg) {
                Ok(f) => f,
                Err(_) => return Ok(None),
            };
            let d = match pr.newton_step(y, 0.0, &f, tg) {
                Ok(d) => d,
                Err(_) => return Ok(None),
            };
            // M delta = F with sigma = 0 means -J delta = F, i.e. delta = -J^-1 F: the Newton update is Y += delta
            let dn = max_abs(&d);
            *last_res = dn;
            if !dn.is_finite() {
                return Ok(None);
            }
            let r0 = max_abs(&f);
            let mut lam = 1.0;
            let mut accepted = false;
            for _ in 0..8 {
                let mut yt = y.clone();
                for q in 0..d.len() {
                    yt[n + q] += lam * d[q];
                }
                let mut tt = tg.clone();
                if let Ok(ft) = pr.residual(&yt, &mut tt) {
                    // accept when the residual does not grow, or when the update is already below the tolerance
                    if max_abs(&ft) <= r0 * (1.0 + 1e-12) || dn <= opts.tol {
                        *y = yt;
                        *tg = tt;
                        accepted = true;
                        break;
                    }
                }
                lam *= 0.5;
            }
            if !accepted {
                return Ok(None);
            }
            if dn <= opts.tol {
                return Ok(Some(it));
            }
        }
        Ok(None)
    };

    let y_start = y.clone();
    let tg_start = tg.clone();
    if let Some(it) = newton(&mut y, &mut tg, opts.max_newton, &mut last_res)? {
        return finish(&pr, y, tg, it, steps, last_res, fuel, oxid);
    }
    // pseudo-transient continuation
    y = y_start;
    tg = tg_start;
    let mut dtau = opts.dtau0;
    let mut f = pr.residual(&y, &mut tg)?;
    let mut rnorm = max_abs(&f);
    let mut consecutive_fail = 0;
    while steps < opts.max_transient {
        steps += 1;
        let step = pr.newton_step(&y, 1.0 / dtau, &f, &mut tg);
        let mut ok = false;
        if let Ok(d) = step {
            let mut yt = y.clone();
            for q in 0..d.len() {
                yt[n + q] = (yt[n + q] + d[q]).max(0.0);
            }
            let mut tt = tg.clone();
            if let Ok(ft) = pr.residual(&yt, &mut tt) {
                let rt = max_abs(&ft);
                if rt.is_finite() && (rt <= 2.0 * rnorm || dtau < 1e-9) {
                    y = yt;
                    tg = tt;
                    f = ft;
                    rnorm = rt;
                    ok = true;
                }
            }
        }
        if ok {
            consecutive_fail = 0;
            dtau = (dtau * 2.5).min(1e12);
            // periodically try the full Newton step from the current profile
            if dtau > 1e-3 || steps % 5 == 0 {
                let ys = y.clone();
                let ts = tg.clone();
                if let Some(it) = newton(&mut y, &mut tg, opts.max_newton.min(10), &mut last_res)? {
                    return finish(&pr, y, tg, it, steps, last_res, fuel, oxid);
                }
                y = ys;
                tg = ts;
                f = pr.residual(&y, &mut tg)?;
                rnorm = max_abs(&f);
            }
        } else {
            consecutive_fail += 1;
            dtau *= 0.25;
            if dtau < 1e-16 || consecutive_fail > 40 {
                break;
            }
        }
    }
    Err(FlameletError(format!(
        "flamelet did not converge in {steps} pseudo-time steps (last residual {rnorm:.3e}, step {dtau:.3e} s)"
    )))
}

#[allow(clippy::too_many_arguments)]
fn finish(
    pr: &Problem,
    y: Vec<f64>,
    mut tg: Vec<f64>,
    newton_iters: usize,
    transient_steps: usize,
    residual: f64,
    fuel: &Stream,
    oxid: &Stream,
) -> Result<FlameletSolution, FlameletError> {
    let n = pr.n;
    let nn = pr.z.len();
    let rows: Vec<Vec<f64>> = (0..nn).map(|i| y[i * n..(i + 1) * n].to_vec()).collect();
    let mut t = Vec::with_capacity(nn);
    for i in 0..nn {
        if i == 0 {
            t.push(oxid.t);
        } else if i == nn - 1 {
            t.push(fuel.t);
        } else {
            let yp: Vec<f64> = rows[i].iter().map(|v| v.max(0.0)).collect();
            t.push(temperature_from_h(pr.mech, pr.h[i], &yp, tg[i])?);
            tg[i] = t[i];
        }
    }
    Ok(FlameletSolution {
        z: pr.z.to_vec(),
        t,
        y: rows,
        newton_iters,
        transient_steps,
        residual,
    })
}

/// Node-major initial profile from an equilibrium table resampled onto `zgrid`.
pub fn guess_from_table(table: &MixtureTable, zgrid: &[f64]) -> Vec<Vec<f64>> {
    zgrid.iter().map(|&z| table.lookup(z).1).collect()
}

// ------------------------------------------------------------------------------------------------ flamelet table

/// Flamelet table over `(Z, chi_st)` (rows ascending in `chi_st`), bilinear lookup in `(Z, ln chi_st)`.
#[derive(Debug, Clone, PartialEq)]
pub struct FlameletTable {
    /// Common mixture-fraction grid.
    pub z: Vec<f64>,
    /// Stoichiometric scalar dissipation rates of the stored flamelets, strictly increasing.
    pub chi_st: Vec<f64>,
    /// Temperature: `t[i_chi][i_z]`.
    pub t: Vec<Vec<f64>>,
    /// Mass fractions: `y[i_chi][k][i_z]`.
    pub y: Vec<Vec<Vec<f64>>>,
    /// First `chi_st` at which the continuation failed (extinction), if any.
    pub extinction_chi: Option<f64>,
}

impl FlameletTable {
    /// Table from converged rows.
    pub fn from_rows(z: Vec<f64>, chi_st: Vec<f64>, rows: &[FlameletSolution]) -> Result<FlameletTable, FlameletError> {
        if chi_st.len() != rows.len() || rows.is_empty() {
            return Err(FlameletError(format!(
                "table row count mismatch: {} chi_st values, {} flamelets",
                chi_st.len(),
                rows.len()
            )));
        }
        for i in 1..chi_st.len() {
            if !(chi_st[i] > chi_st[i - 1]) || !(chi_st[i - 1] > 0.0) {
                return Err(FlameletError(format!(
                    "chi_st must be positive and strictly increasing (index {i})"
                )));
            }
        }
        let mut t = Vec::new();
        let mut y = Vec::new();
        for r in rows {
            if r.z != z {
                return Err(FlameletError("flamelet grids differ from the table grid".into()));
            }
            t.push(r.t.clone());
            y.push(r.to_table().y);
        }
        Ok(FlameletTable {
            z,
            chi_st,
            t,
            y,
            extinction_chi: None,
        })
    }

    fn bracket(&self, chi: f64) -> (usize, f64) {
        let n = self.chi_st.len();
        if n == 1 || chi <= self.chi_st[0] {
            return (0, 0.0);
        }
        if chi >= self.chi_st[n - 1] {
            return (n - 2, 1.0);
        }
        let l = chi.ln();
        let mut j = 0;
        while j + 2 < n && self.chi_st[j + 1] < chi {
            j += 1;
        }
        let (a, b) = (self.chi_st[j].ln(), self.chi_st[j + 1].ln());
        (j, (l - a) / (b - a))
    }

    /// Bilinear lookup of `(T, Y)` at `(z, chi_st)` (linear in `Z` and in `ln chi_st`, clamped to the table range).
    pub fn lookup(&self, z: f64, chi_st: f64) -> (f64, Vec<f64>) {
        if self.chi_st.len() == 1 {
            return (
                interp(&self.z, &self.t[0], z),
                self.y[0].iter().map(|r| interp(&self.z, r, z)).collect(),
            );
        }
        let (j, w) = self.bracket(chi_st);
        let t = (1.0 - w) * interp(&self.z, &self.t[j], z) + w * interp(&self.z, &self.t[j + 1], z);
        let y = (0..self.y[j].len())
            .map(|k| (1.0 - w) * interp(&self.z, &self.y[j][k], z) + w * interp(&self.z, &self.y[j + 1][k], z))
            .collect();
        (t, y)
    }

    /// Beta-PDF mean of `(T, Y)` at mean `zm`, variance `v` and `chi_st`.
    pub fn mean(&self, zm: f64, v: f64, chi_st: f64) -> Result<(f64, Vec<f64>), FlameletError> {
        let at = |j: usize| -> Result<(f64, Vec<f64>), FlameletError> {
            let t = integrate_beta(&self.z, &self.t[j], zm, v)?;
            let mut y = Vec::new();
            for r in &self.y[j] {
                y.push(integrate_beta(&self.z, r, zm, v)?);
            }
            Ok((t, y))
        };
        if self.chi_st.len() == 1 {
            return at(0);
        }
        let (j, w) = self.bracket(chi_st);
        let (t0, y0) = at(j)?;
        let (t1, y1) = at(j + 1)?;
        Ok((
            (1.0 - w) * t0 + w * t1,
            y0.iter().zip(&y1).map(|(a, b)| (1.0 - w) * a + w * b).collect(),
        ))
    }
}

/// Continuation along ascending `chi_st` from the burning branch: the first flamelet starts from the equilibrium profile, every next
/// one from the previous solution. The continuation stops at the first failure (extinction), which is recorded in the table.
#[allow(clippy::too_many_arguments)]
pub fn build_table(
    mech: &Mechanism,
    p: f64,
    fuel: &Stream,
    oxid: &Stream,
    zgrid: &[f64],
    z_st: f64,
    chi_list: &[f64],
    eq_guess: &MixtureTable,
    opts: &FlameletOptions,
) -> Result<(FlameletTable, Vec<FlameletSolution>), FlameletError> {
    let mut sols: Vec<FlameletSolution> = Vec::new();
    let mut used: Vec<f64> = Vec::new();
    let mut extinction = None;
    for &c in chi_list {
        let chi: Vec<f64> = zgrid.iter().map(|&z| chi_profile(z, c, z_st)).collect();
        let guess = match sols.last() {
            Some(s) => s.y.clone(),
            None => guess_from_table(eq_guess, zgrid),
        };
        match solve(mech, p, fuel, oxid, zgrid, &chi, Some(&guess), opts) {
            Ok(s) => {
                sols.push(s);
                used.push(c);
            }
            Err(_) => {
                extinction = Some(c);
                break;
            }
        }
    }
    if sols.is_empty() {
        return Err(FlameletError(format!(
            "no flamelet of the list converged (first chi_st = {})",
            chi_list.first().copied().unwrap_or(f64::NAN)
        )));
    }
    let mut tab = FlameletTable::from_rows(zgrid.to_vec(), used, &sols)?;
    tab.extinction_chi = extinction;
    Ok((tab, sols))
}
