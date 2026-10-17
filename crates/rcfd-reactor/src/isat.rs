//! In-situ adaptive tabulation (Pope 1997) of a chemistry map `phi -> R(phi)` over one time step (roadmap F-223, ADR 0053 D1).
//!
//! A table record holds `(phi0, R0, A, M, Minv)`: the state, the mapped state, the sensitivity matrix `A = dR/dphi` and the
//! ellipsoid of accuracy `x^T M x <= 1` in scaled input coordinates `x = (phi - phi0)/s`. A query inside an ellipsoid is answered by
//! the linear map `R0 + A (phi - phi0)`; a query outside is integrated directly and then either grows the nearest ellipsoid (when the
//! linear prediction was within the tolerance at the query) or adds a record. The tolerance is in the scaled max-norm.
//! The sensitivity matrix is taken by forward differences of the map itself (one integration per active coordinate, normal
//! tolerance). Derivations of the initial ellipsoid, the growth update and its inverse are in the ADR. The search is a linear scan
//! with an exact axis-extent prefilter (no binary tree).

use crate::cell::CellModel;
use crate::mech::Mechanism;
use crate::ode::{integrate, lu_factor, lu_solve, Flow, Options};
use crate::reactors::{batch_rhs, BatchKind, Energy, ReactorError, Tol};

/// A chemistry map over one step.
pub trait ChemMap {
    /// Mapped state.
    fn apply(&mut self, phi: &[f64]) -> Result<Vec<f64>, ReactorError>;
}

fn opts(tol: Tol, n: usize) -> Options {
    let mut o = Options::new(tol.rtol, tol.atol_y, n);
    o.atol[0] = tol.atol_t;
    o
}

fn finite(y: Vec<f64>) -> Result<Vec<f64>, ReactorError> {
    if y.iter().any(|v| !v.is_finite()) {
        return Err(ReactorError::Input("non-finite state after the chemistry step".into()));
    }
    Ok(y)
}

/// Batch-reactor map (`reactors::batch_rhs`) over `dt`; `param` is the pressure or the mass density as in `batch_rhs`.
pub struct BatchMap<'a> {
    /// Mechanism.
    pub mech: &'a Mechanism,
    /// Constant pressure or constant volume.
    pub kind: BatchKind,
    /// Energy treatment.
    pub energy: Energy,
    /// Pressure (Pa) for constant pressure, mass density (kg/m3) for constant volume.
    pub param: f64,
    /// Step.
    pub dt: f64,
    /// Tolerances.
    pub tol: Tol,
    /// Accumulated right-hand-side evaluations of every integration done through this map.
    pub rhs_evals: usize,
    /// Number of integrations.
    pub integrations: usize,
}

impl<'a> BatchMap<'a> {
    /// New map with zeroed counters.
    pub fn new(mech: &'a Mechanism, kind: BatchKind, energy: Energy, param: f64, dt: f64, tol: Tol) -> Self {
        BatchMap {
            mech,
            kind,
            energy,
            param,
            dt,
            tol,
            rhs_evals: 0,
            integrations: 0,
        }
    }
}

impl ChemMap for BatchMap<'_> {
    fn apply(&mut self, phi: &[f64]) -> Result<Vec<f64>, ReactorError> {
        let (mech, kind, energy, param) = (self.mech, self.kind, self.energy, self.param);
        let mut rhs = |_t: f64, y: &[f64], f: &mut [f64]| batch_rhs(mech, kind, energy, param, y, f);
        let (y, st) = integrate(&mut rhs, 0.0, self.dt, phi, &opts(self.tol, phi.len()), &mut |_| {
            Flow::Continue
        })?;
        self.rhs_evals += st.rhs_evals;
        self.integrations += 1;
        finite(y)
    }
}

/// Grid-cell map (`CellModel`, constant density and heat capacity) over `dt`; identical to [`CellModel::advance`].
pub struct CellMap<'a> {
    /// The cell model.
    pub cell: &'a CellModel<'a>,
    /// Step.
    pub dt: f64,
    /// Accumulated right-hand-side evaluations.
    pub rhs_evals: usize,
    /// Number of integrations.
    pub integrations: usize,
}

impl<'a> CellMap<'a> {
    /// New map with zeroed counters.
    pub fn new(cell: &'a CellModel<'a>, dt: f64) -> Self {
        CellMap {
            cell,
            dt,
            rhs_evals: 0,
            integrations: 0,
        }
    }
}

impl ChemMap for CellMap<'_> {
    fn apply(&mut self, phi: &[f64]) -> Result<Vec<f64>, ReactorError> {
        let cell = self.cell;
        let mut rhs = |_t: f64, y: &[f64], f: &mut [f64]| cell.rhs(y, f);
        let (y, st) = integrate(&mut rhs, 0.0, self.dt, phi, &opts(cell.tol, phi.len()), &mut |_| {
            Flow::Continue
        })?;
        self.rhs_evals += st.rhs_evals;
        self.integrations += 1;
        finite(y)
    }
}

/// Scaled max-norm `max_k |a_k - b_k| / s_k`.
pub fn scaled_dist(a: &[f64], b: &[f64], s: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .zip(s)
        .map(|((a, b), s)| (a - b).abs() / s)
        .fold(0.0, f64::max)
}

/// Default error scales: 100 K and 0.01 in mass fraction (ADR 0053).
pub fn default_scales(n_species: usize) -> Vec<f64> {
    let mut s = vec![0.01; n_species + 1];
    s[0] = 100.0;
    s
}

/// What a query did to the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Answered by a linear map inside an ellipsoid of accuracy.
    Retrieved,
    /// Integrated; the nearest ellipsoid was grown to contain the query.
    Grown,
    /// Integrated; a record was added.
    Added,
    /// Integrated; the table is full and the result was not stored.
    Direct,
}

/// Counters of an ISAT table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IsatStats {
    /// Queries.
    pub queries: usize,
    /// Retrievals.
    pub retrieved: usize,
    /// Growths.
    pub grown: usize,
    /// Additions.
    pub added: usize,
    /// Direct integrations that could not be stored (table full).
    pub direct: usize,
}

struct Record {
    /// Active components of the stored state.
    phi0: Vec<f64>,
    /// Active components of the mapped state.
    r0: Vec<f64>,
    a: Vec<f64>,
    m: Vec<f64>,
    minv: Vec<f64>,
    half: Vec<f64>,
    grows: usize,
}

/// Species that can be nonzero in a mixture with the elements of `phi`: `true` for every species whose elements are all present
/// (positive element total). A species with an absent element has a mass fraction that is identically zero for all time, because
/// every reaction needs its reactants, so its coordinate is not part of the manifold of the map.
pub fn element_active_mask(mech: &Mechanism, phi: &[f64]) -> Vec<bool> {
    let tot = mech.element_mass_fractions(&phi[1..]);
    mech.species
        .iter()
        .map(|sp| sp.atoms.iter().zip(&tot).all(|(a, t)| *a == 0.0 || *t > 0.0))
        .collect()
}

/// Largest magnitude accepted in a coordinate outside the table's subspace: integrator round-off in structurally zero mass
/// fractions (observed up to 2.4e-15 with atol 1e-14), five orders below the resolution eps*scale = 1e-5 of the default tolerances. Such values are carried through unchanged.
pub const FROZEN_NOISE: f64 = 1e-10;

/// An ISAT table for states of dimension `n` (optionally on a subspace of the coordinates).
pub struct Isat {
    n_full: usize,
    /// Indices of the active coordinates (always includes 0, the temperature).
    act: Vec<usize>,
    /// Scales of the active coordinates.
    s: Vec<f64>,
    /// Tolerance in the scaled max-norm.
    pub eps: f64,
    /// Cap of the initial scaled extent.
    pub cap: f64,
    /// Capacity.
    pub max_records: usize,
    records: Vec<Record>,
    /// Counters.
    pub stats: IsatStats,
    /// Diagnostic of the last retrieval: `(record index, times that record has been grown, scaled x^T M x of the query)`.
    pub last_hit: Option<(usize, usize, f64)>,
}

impl Isat {
    /// New table: scale vector `s` (length `n`) and tolerance `eps > 0`; every coordinate is active.
    pub fn new(s: Vec<f64>, eps: f64) -> Isat {
        let n = s.len();
        Isat::with_active(s, eps, &vec![true; n])
    }

    /// New table on the subspace of the coordinates with `active[k]` (`active[0]`, the temperature, is forced on). The inactive
    /// coordinates must be zero to within [`FROZEN_NOISE`] in every query (an error otherwise) and are copied to the result; see
    /// [`element_active_mask`] for the structural zeros of a mixture (the caller maps species flags to coordinates `1 + k`).
    pub fn with_active(s: Vec<f64>, eps: f64, active: &[bool]) -> Isat {
        let act: Vec<usize> = (0..s.len()).filter(|&k| k == 0 || active[k]).collect();
        Isat {
            n_full: s.len(),
            s: act.iter().map(|&k| s[k]).collect(),
            act,
            eps,
            cap: 0.1,
            max_records: 20000,
            records: Vec::new(),
            stats: IsatStats::default(),
            last_hit: None,
        }
    }

    /// Dimension of the manifold (number of active coordinates).
    pub fn dim(&self) -> usize {
        self.act.len()
    }

    /// Number of records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// True when the table has no record.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    fn gather(&self, phi: &[f64]) -> Vec<f64> {
        self.act.iter().map(|&k| phi[k]).collect()
    }

    /// Scaled quadratic form `x^T M x` of the record, with the exact axis-extent pre-filter: `None` when the point is outside
    /// the axis extents of the ellipsoid.
    fn dist(&self, rec: &Record, pa: &[f64], x: &mut [f64]) -> Option<f64> {
        let n = self.act.len();
        for k in 0..n {
            let xk = (pa[k] - rec.phi0[k]) / self.s[k];
            if xk.abs() > rec.half[k] {
                return None;
            }
            x[k] = xk;
        }
        Some(self.quad_x(rec, x))
    }

    fn quad_x(&self, rec: &Record, x: &[f64]) -> f64 {
        let n = self.act.len();
        let mut d = 0.0;
        for i in 0..n {
            let mut row = 0.0;
            for j in 0..n {
                row += rec.m[i * n + j] * x[j];
            }
            d += x[i] * row;
        }
        d
    }

    fn linear(&self, rec: &Record, pa: &[f64], phi: &[f64]) -> Vec<f64> {
        let n = self.act.len();
        let dphi: Vec<f64> = (0..n).map(|k| pa[k] - rec.phi0[k]).collect();
        let mut out = phi.to_vec();
        for j in 0..n {
            let mut v = rec.r0[j];
            for k in 0..n {
                v += rec.a[j * n + k] * dphi[k];
            }
            out[self.act[j]] = v;
        }
        out
    }

    fn check_frozen(&self, phi: &[f64]) -> Result<(), ReactorError> {
        if phi.len() != self.n_full {
            return Err(ReactorError::Input(format!(
                "ISAT query has {} entries, the table has dimension {}",
                phi.len(),
                self.n_full
            )));
        }
        if self.act.len() < self.n_full {
            let mut it = self.act.iter().peekable();
            for (k, v) in phi.iter().enumerate() {
                if it.peek().is_some_and(|&&a| a == k) {
                    it.next();
                } else if v.abs() > FROZEN_NOISE {
                    return Err(ReactorError::Input(format!(
                        "ISAT query: coordinate {k} is outside the table's subspace but is {v:e} (|v| > {FROZEN_NOISE:e})"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Retrieve without integrating: the linear map of the record with the smallest `x^T M x <= 1`, or `None`.
    pub fn retrieve(&self, phi: &[f64]) -> Option<Vec<f64>> {
        let pa = self.gather(phi);
        let mut x = vec![0.0; self.act.len()];
        let mut best: Option<(f64, usize)> = None;
        for (i, rec) in self.records.iter().enumerate() {
            if let Some(d) = self.dist(rec, &pa, &mut x) {
                if d <= 1.0 && best.map_or(true, |(b, _)| d < b) {
                    best = Some((d, i));
                }
            }
        }
        best.map(|(_, i)| self.linear(&self.records[i], &pa, phi))
    }

    /// Answer a query: retrieve, or integrate with `map` and grow or add. Errors are those of the integration.
    pub fn query(&mut self, map: &mut dyn ChemMap, phi: &[f64]) -> Result<(Vec<f64>, Outcome), ReactorError> {
        self.check_frozen(phi)?;
        let n = self.act.len();
        self.stats.queries += 1;
        let pa = self.gather(phi);
        let mut x = vec![0.0; n];
        let mut hit: Option<(f64, usize)> = None;
        for (i, rec) in self.records.iter().enumerate() {
            if let Some(d) = self.dist(rec, &pa, &mut x) {
                if d <= 1.0 && hit.map_or(true, |(b, _)| d < b) {
                    hit = Some((d, i));
                }
            }
        }
        if let Some((d, i)) = hit {
            self.stats.retrieved += 1;
            self.last_hit = Some((i, self.records[i].grows, d));
            return Ok((self.linear(&self.records[i], &pa, phi), Outcome::Retrieved));
        }
        let r = map.apply(phi)?;
        // growth candidate: the record nearest in the scaled Euclidean distance of the stored states
        let mut near: Option<(f64, usize)> = None;
        for (i, rec) in self.records.iter().enumerate() {
            let e: f64 = (0..n).map(|k| ((pa[k] - rec.phi0[k]) / self.s[k]).powi(2)).sum();
            if near.map_or(true, |(b, _)| e < b) {
                near = Some((e, i));
            }
        }
        if let Some((_, i)) = near {
            let xs: Vec<f64> = (0..n).map(|k| (pa[k] - self.records[i].phi0[k]) / self.s[k]).collect();
            let d = self.quad_x(&self.records[i], &xs);
            let pred = self.linear(&self.records[i], &pa, phi);
            let ra = self.gather(&r);
            let pr = self.gather(&pred);
            let err = (0..n).map(|k| (pr[k] - ra[k]).abs() / self.s[k]).fold(0.0, f64::max);
            if d > 1.0 && err <= self.eps {
                self.grow(i, &xs, d);
                self.stats.grown += 1;
                return Ok((r, Outcome::Grown));
            }
        }
        if self.records.len() >= self.max_records {
            self.stats.direct += 1;
            return Ok((r, Outcome::Direct));
        }
        let rec = self.make_record(map, phi, &r)?;
        self.records.push(rec);
        self.stats.added += 1;
        Ok((r, Outcome::Added))
    }

    fn grow(&mut self, i: usize, x: &[f64], d: f64) {
        let n = self.act.len();
        let rec = &mut self.records[i];
        let mx: Vec<f64> = (0..n)
            .map(|r| (0..n).map(|c| rec.m[r * n + c] * x[c]).sum::<f64>())
            .collect();
        let beta = (1.0 / d - 1.0) / d;
        let gamma = 1.0 - 1.0 / d;
        for r in 0..n {
            for c in 0..n {
                rec.m[r * n + c] += beta * mx[r] * mx[c];
                rec.minv[r * n + c] += gamma * x[r] * x[c];
            }
        }
        for k in 0..n {
            rec.half[k] = rec.minv[k * n + k].max(0.0).sqrt();
        }
        rec.grows += 1;
    }

    fn make_record(&self, map: &mut dyn ChemMap, phi: &[f64], r_full: &[f64]) -> Result<Record, ReactorError> {
        let n = self.act.len();
        let r0 = self.gather(r_full);
        let mut a = vec![0.0; n * n];
        let mut p = phi.to_vec();
        for (c, &k) in self.act.iter().enumerate() {
            let delta = if k == 0 {
                1e-5 * phi[0].abs().max(100.0)
            } else {
                1e-5 * phi[k].abs().max(1e-3)
            };
            p[k] = phi[k] + delta;
            let rp = self.gather(&map.apply(&p)?);
            p[k] = phi[k];
            for j in 0..n {
                a[j * n + c] = (rp[j] - r0[j]) / delta;
            }
        }
        // M0 = A_s^T A_s / eps^2 + I / cap^2, A_s = S^-1 A S
        let mut m = vec![0.0; n * n];
        for k in 0..n {
            for l in k..n {
                let mut v = 0.0;
                for j in 0..n {
                    let ajk = a[j * n + k] * self.s[k] / self.s[j];
                    let ajl = a[j * n + l] * self.s[l] / self.s[j];
                    v += ajk * ajl;
                }
                v /= self.eps * self.eps;
                if k == l {
                    v += 1.0 / (self.cap * self.cap);
                }
                m[k * n + l] = v;
                m[l * n + k] = v;
            }
        }
        let mut lu = m.clone();
        let mut piv = vec![0usize; n];
        if !lu_factor(&mut lu, n, &mut piv) {
            return Err(ReactorError::Input("ISAT: singular ellipsoid matrix".into()));
        }
        let mut minv = vec![0.0; n * n];
        let mut col = vec![0.0; n];
        for c in 0..n {
            col.iter_mut().for_each(|v| *v = 0.0);
            col[c] = 1.0;
            lu_solve(&lu, n, &piv, &mut col);
            for r in 0..n {
                minv[r * n + c] = col[r];
            }
        }
        let half = (0..n).map(|k| minv[k * n + k].max(0.0).sqrt()).collect();
        Ok(Record {
            phi0: self.gather(phi),
            r0,
            a,
            m,
            minv,
            half,
            grows: 0,
        })
    }

    /// Sensitivity matrix of record `i` over the active coordinates (row-major, `A[j][k] = dR_j/dphi_k`), for tests.
    pub fn sensitivity(&self, i: usize) -> &[f64] {
        &self.records[i].a
    }
}

/// Moles of each element per kg of mixture, `sum_k atoms_jk Y_k / W_k`, of a state `[T, Y]` (proportional to the element mass).
pub fn element_moles(mech: &Mechanism, phi: &[f64]) -> Vec<f64> {
    mech.element_mass_fractions(&phi[1..])
}

/// Specific enthalpy over `cp` in kelvin: `h(T, Y)/cp(T, Y)`, used for the conservation bound of retrieved states.
pub fn enthalpy_in_kelvin(mech: &Mechanism, phi: &[f64]) -> f64 {
    let y: Vec<f64> = phi[1..].to_vec();
    mech.h_mass(phi[0], &y) / mech.cp_mass(phi[0], &y)
}

/// Enthalpy change `(h(b) - h(a))/cp(b)` in kelvin-equivalent between two states. Unlike the difference of `h/cp` (which is not a
/// conserved quantity when `cp` changes) this is the temperature error that a given enthalpy error would cause.
pub fn enthalpy_change_in_kelvin(mech: &Mechanism, a: &[f64], b: &[f64]) -> f64 {
    let ya = &a[1..];
    let yb = &b[1..];
    (mech.h_mass(b[0], yb) - mech.h_mass(a[0], ya)) / mech.cp_mass(b[0], yb)
}
