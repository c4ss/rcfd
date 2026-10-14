//! Chemical equilibrium of an ideal-gas mixture by Gibbs minimisation with element potentials
//! (the Newton iteration of Gordon and McBride, NASA RP-1311, in its ideal-gas form), at fixed T and P (TP),
//! fixed enthalpy and pressure (HP) or fixed internal energy and volume (UV).

use crate::mech::{Mechanism, P_STD, R_GAS};
use crate::ode::{lu_factor, lu_solve};

/// Equilibrium failure.
#[derive(Debug, Clone, PartialEq)]
pub struct EquilError(pub String);

impl std::fmt::Display for EquilError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EquilError {}

/// Equilibrium state.
#[derive(Debug, Clone, PartialEq)]
pub struct Equilibrium {
    /// Temperature, K.
    pub t: f64,
    /// Pressure, Pa.
    pub p: f64,
    /// Mole numbers per initial mole of mixture.
    pub n: Vec<f64>,
    /// Mole fractions.
    pub x: Vec<f64>,
    /// Mass fractions.
    pub y: Vec<f64>,
    /// Newton iterations of the last TP solve.
    pub iterations: usize,
}

fn element_moles(mech: &Mechanism, x: &[f64]) -> Vec<f64> {
    let mut b = vec![0.0; mech.elements.len()];
    for (k, s) in mech.species.iter().enumerate() {
        for e in 0..b.len() {
            b[e] += x[k] * s.atoms[e];
        }
    }
    b
}

/// Equilibrium at given T and P for the element totals of the mole-fraction vector `x0` (one mole of mixture).
pub fn equilibrium_tp(mech: &Mechanism, x0: &[f64], t: f64, p: f64) -> Result<Equilibrium, EquilError> {
    equilibrium_tp_guess(mech, x0, t, p, None)
}

fn equilibrium_tp_guess(
    mech: &Mechanism,
    x0: &[f64],
    t: f64,
    p: f64,
    guess: Option<&[f64]>,
) -> Result<Equilibrium, EquilError> {
    let ns = mech.species.len();
    let ne_all = mech.elements.len();
    let b_all = element_moles(mech, x0);
    let active_el: Vec<usize> = (0..ne_all).filter(|&e| b_all[e] > 1e-300).collect();
    let ne = active_el.len();
    let active_sp: Vec<usize> = (0..ns)
        .filter(|&k| (0..ne_all).all(|e| mech.species[k].atoms[e] == 0.0 || b_all[e] > 1e-300))
        .collect();
    let nsp = active_sp.len();
    let b: Vec<f64> = active_el.iter().map(|&e| b_all[e]).collect();
    let a: Vec<Vec<f64>> = active_sp
        .iter()
        .map(|&k| active_el.iter().map(|&e| mech.species[k].atoms[e]).collect())
        .collect();
    let c: Vec<f64> = active_sp
        .iter()
        .map(|&k| {
            let s = &mech.species[k];
            s.h_rt(t) - s.s_r(t) + (p / P_STD).ln()
        })
        .collect();
    let btot: f64 = b.iter().sum();
    let mut ln_n: Vec<f64> = match guess {
        Some(g) => active_sp.iter().map(|&k| g[k].max(1e-300).ln()).collect(),
        None => vec![(0.1 * btot / nsp as f64).ln(); nsp],
    };
    let mut ln_big = ln_n.iter().map(|v| v.exp()).sum::<f64>().ln();
    let m = ne + 1;
    let mut mat = vec![0.0; m * m];
    let mut piv = vec![0usize; m];
    let mut rhs = vec![0.0; m];
    let mut iters = 0;
    let mut converged = false;
    for it in 0..400 {
        iters = it + 1;
        let nk: Vec<f64> = ln_n.iter().map(|v| v.exp()).collect();
        let big = ln_big.exp();
        let mu: Vec<f64> = (0..nsp).map(|k| c[k] + ln_n[k] - ln_big).collect();
        for v in mat.iter_mut() {
            *v = 0.0;
        }
        for r in rhs.iter_mut() {
            *r = 0.0;
        }
        let mut sum_nk = 0.0;
        let mut sum_nmu = 0.0;
        for k in 0..nsp {
            sum_nk += nk[k];
            sum_nmu += nk[k] * mu[k];
            for j in 0..ne {
                for i in 0..ne {
                    mat[j * m + i] += a[k][j] * a[k][i] * nk[k];
                }
                mat[j * m + ne] += a[k][j] * nk[k];
                rhs[j] += a[k][j] * nk[k] * (mu[k] - 1.0);
            }
        }
        for j in 0..ne {
            mat[ne * m + j] = mat[j * m + ne];
            rhs[j] += b[j];
        }
        mat[ne * m + ne] = sum_nk - big;
        rhs[ne] = big - sum_nk + sum_nmu;
        if !lu_factor(&mut mat, m, &mut piv) {
            return Err(EquilError("singular equilibrium matrix".into()));
        }
        lu_solve(&mat, m, &piv, &mut rhs);
        let pi = &rhs[..ne];
        let dln_big = rhs[ne];
        let delta: Vec<f64> = (0..nsp)
            .map(|k| (0..ne).map(|j| a[k][j] * pi[j]).sum::<f64>() + dln_big - mu[k])
            .collect();
        // damping
        let mut lam = 1.0_f64;
        let mut dmax = 5.0 * dln_big.abs();
        for k in 0..nsp {
            let major = ln_n[k] - ln_big > -18.420_680_743_952_367; // x > 1e-8
            if major {
                dmax = dmax.max(delta[k].abs());
            } else if delta[k] - dln_big > 0.0 {
                let l2 = (-(ln_n[k] - ln_big) - 9.210_340_371_976_184).abs() / (delta[k] - dln_big);
                lam = lam.min(l2.max(1e-3));
            }
        }
        if dmax > 2.0 {
            lam = lam.min(2.0 / dmax);
        }
        let conv = (0..nsp)
            .map(|k| nk[k] * delta[k].abs() / big)
            .fold(0.0, f64::max)
            .max(dln_big.abs());
        for k in 0..nsp {
            ln_n[k] = (ln_n[k] + lam * delta[k]).max(-700.0);
        }
        ln_big += lam * dln_big;
        if lam == 1.0 && conv < 1e-13 {
            converged = true;
            break;
        }
    }
    if !converged {
        return Err(EquilError(format!(
            "equilibrium TP did not converge in {iters} iterations (T = {t}, P = {p})"
        )));
    }
    let mut n = vec![0.0; ns];
    for (i, &k) in active_sp.iter().enumerate() {
        n[k] = ln_n[i].exp();
    }
    let tot: f64 = n.iter().sum();
    let x: Vec<f64> = n.iter().map(|v| v / tot).collect();
    let y = mech.x_to_y(&x);
    Ok(Equilibrium {
        t,
        p,
        n,
        x,
        y,
        iterations: iters,
    })
}

/// Illinois-method root of `f` on `[lo, hi]` with `f(lo) f(hi) < 0`; stops when the bracket is below `xtol`.
fn illinois(
    mut lo: f64,
    mut hi: f64,
    xtol: f64,
    f: &mut dyn FnMut(f64) -> Result<f64, EquilError>,
) -> Result<f64, EquilError> {
    let (mut flo, mut fhi) = (f(lo)?, f(hi)?);
    if flo * fhi > 0.0 {
        return Err(EquilError(format!(
            "root not bracketed in [{lo}, {hi}] (f = {flo:e}, {fhi:e})"
        )));
    }
    let mut side = 0;
    for _ in 0..200 {
        let x = (hi * flo - lo * fhi) / (flo - fhi);
        let fx = f(x)?;
        if fx * fhi > 0.0 {
            hi = x;
            fhi = fx;
            if side == -1 {
                flo *= 0.5;
            }
            side = -1;
        } else if fx * flo > 0.0 {
            lo = x;
            flo = fx;
            if side == 1 {
                fhi *= 0.5;
            }
            side = 1;
        } else {
            return Ok(x);
        }
        if (hi - lo).abs() < xtol || fx == 0.0 {
            return Ok(x);
        }
    }
    Ok(0.5 * (lo + hi))
}

fn enthalpy_molar(mech: &Mechanism, n: &[f64], t: f64) -> f64 {
    mech.species.iter().zip(n).map(|(s, n)| n * R_GAS * t * s.h_rt(t)).sum()
}

/// Constraint pair for [`equilibrium`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    /// Fixed temperature and pressure.
    TP,
    /// Fixed enthalpy and pressure.
    HP,
    /// Fixed internal energy and volume.
    UV,
}

/// Equilibrium of the mixture with mole fractions `x0` initially at `t0`, `p0`, under the constraint.
pub fn equilibrium(
    mech: &Mechanism,
    x0: &[f64],
    t0: f64,
    p0: f64,
    constraint: Constraint,
) -> Result<Equilibrium, EquilError> {
    match constraint {
        Constraint::TP => equilibrium_tp(mech, x0, t0, p0),
        Constraint::HP => {
            let h0 = enthalpy_molar(mech, x0, t0);
            let scale = h0.abs() + 3.5 * R_GAS * t0;
            let mut last = None;
            let tmax = mech
                .species
                .iter()
                .map(|s| s.t_high)
                .fold(f64::INFINITY, f64::min)
                .min(6000.0);
            let t = illinois(250.0, tmax, 1e-9, &mut |t| {
                let eq = equilibrium_tp_guess(mech, x0, t, p0, last.as_ref().map(|e: &Equilibrium| e.n.as_slice()))?;
                let r = (enthalpy_molar(mech, &eq.n, t) - h0) / scale;
                last = Some(eq);
                Ok(r)
            })?;
            equilibrium_tp(mech, x0, t, p0)
        }
        Constraint::UV => {
            let u0: f64 = mech
                .species
                .iter()
                .zip(x0)
                .map(|(s, x)| x * R_GAS * t0 * (s.h_rt(t0) - 1.0))
                .sum();
            let v0 = R_GAS * t0 / p0; // volume per initial mole
            let scale = u0.abs() + 2.5 * R_GAS * t0;
            let tmax = mech
                .species
                .iter()
                .map(|s| s.t_high)
                .fold(f64::INFINITY, f64::min)
                .min(6000.0);
            let mut pcur = p0;
            let solve_at = |t: f64, pcur: &mut f64| -> Result<Equilibrium, EquilError> {
                for _ in 0..200 {
                    let eq = equilibrium_tp(mech, x0, t, *pcur)?;
                    let ntot: f64 = eq.n.iter().sum();
                    let pn = ntot * R_GAS * t / v0;
                    let done = (pn - *pcur).abs() < 1e-13 * pn;
                    *pcur = pn;
                    if done {
                        return equilibrium_tp(mech, x0, t, pn);
                    }
                }
                Err(EquilError("UV pressure iteration did not converge".into()))
            };
            let t = illinois(250.0, tmax, 1e-9, &mut |t| {
                let eq = solve_at(t, &mut pcur)?;
                let u: f64 = mech
                    .species
                    .iter()
                    .zip(&eq.n)
                    .map(|(s, n)| n * R_GAS * t * (s.h_rt(t) - 1.0))
                    .sum();
                Ok((u - u0) / scale)
            })?;
            solve_at(t, &mut pcur)
        }
    }
}
