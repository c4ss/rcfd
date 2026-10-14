//! Ffowcs Williams-Hawkings far-field acoustics (Farassat/Brentner formulation 1A) on a permeable data surface.
//!
//! The medium is uniform and at rest (density `rho0`, sound speed `c0`); the data surface may move on a prescribed
//! subsonic path. Per surface element the caller supplies uniformly sampled time series of position, normal, surface
//! velocity, acoustic pressure `p'`, density perturbation `rho'` and fluid velocity `u`. The result at an observer is the
//! sum of the thickness and loading integrals of formulation 1A (the volume quadrupole is not included).
//!
//! Permeable-surface source terms (Di Francescantonio 1997, Brentner and Farassat 2003):
//! `U_i = (1 - rho/rho0) v_i + rho u_i / rho0`, `L_i = p' n_i + rho u_i (u_n - v_n)` with `rho = rho0 + rho'`.

// NaN-rejecting comparisons (`!(x > 0.0)`) are intentional in the validation code.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use serde_json::{json, Value};

/// Error of the FW-H module; messages locate the offending input.
#[derive(Debug, Clone, PartialEq)]
pub struct FwhError(pub String);

impl std::fmt::Display for FwhError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FwhError {}

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// One surface element with its time series (all vectors have the same length as the global time axis).
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Area of the element (constant in time), m^2.
    pub area: f64,
    /// Position per sample.
    pub x: Vec<V3>,
    /// Unit outward normal per sample.
    pub n: Vec<V3>,
    /// Surface velocity per sample.
    pub v: Vec<V3>,
    /// Acoustic pressure p' per sample.
    pub p: Vec<f64>,
    /// Density perturbation rho' per sample.
    pub rho: Vec<f64>,
    /// Fluid velocity per sample.
    pub u: Vec<V3>,
}

/// Permeable data surface sampled on the uniform time axis `t0 + i dt`.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    /// Ambient density.
    pub rho0: f64,
    /// Speed of sound.
    pub c0: f64,
    /// First sample time.
    pub t0: f64,
    /// Sample interval.
    pub dt: f64,
    /// Elements.
    pub elements: Vec<Element>,
}

/// Thickness, loading and total acoustic pressure at one observer time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Signal {
    /// Observer time.
    pub t: f64,
    /// Thickness part.
    pub p_thickness: f64,
    /// Loading part.
    pub p_loading: f64,
}

impl Signal {
    /// Total acoustic pressure.
    pub fn total(&self) -> f64 {
        self.p_thickness + self.p_loading
    }
}

/// Derived series per element: everything needed at the emission time, with first derivatives.
struct Prepared {
    area: f64,
    x: Vec<V3>,
    v: Vec<V3>,
    vdot: Vec<V3>,
    n: Vec<V3>,
    ndot: Vec<V3>,
    uvec: Vec<V3>,
    udot: Vec<V3>,
    l: Vec<V3>,
    ldot: Vec<V3>,
}

/// First time derivative of a uniformly sampled series: 5-point central in the interior, lower order at the ends.
fn derivative<T: Copy>(s: &[T], dt: f64, add: impl Fn(T, T) -> T, mul: impl Fn(T, f64) -> T) -> Vec<T> {
    let n = s.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let d = if n >= 5 && i >= 2 && i + 2 < n {
            // (-f(i+2) + 8 f(i+1) - 8 f(i-1) + f(i-2)) / (12 dt)
            let a = add(mul(s[i + 1], 8.0), mul(s[i + 2], -1.0));
            let b = add(mul(s[i - 1], -8.0), s[i - 2]);
            mul(add(a, b), 1.0 / (12.0 * dt))
        } else if i == 0 {
            if n >= 3 {
                // (-3 f0 + 4 f1 - f2) / (2 dt)
                mul(add(add(mul(s[0], -3.0), mul(s[1], 4.0)), mul(s[2], -1.0)), 0.5 / dt)
            } else {
                mul(add(s[1], mul(s[0], -1.0)), 1.0 / dt)
            }
        } else if i + 1 == n {
            if n >= 3 {
                mul(add(add(mul(s[n - 1], 3.0), mul(s[n - 2], -4.0)), s[n - 3]), 0.5 / dt)
            } else {
                mul(add(s[n - 1], mul(s[n - 2], -1.0)), 1.0 / dt)
            }
        } else {
            mul(add(s[i + 1], mul(s[i - 1], -1.0)), 0.5 / dt)
        };
        out.push(d);
    }
    out
}

fn add3(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn deriv3(s: &[V3], dt: f64) -> Vec<V3> {
    derivative(s, dt, add3, scale)
}

/// 4-point Lagrange weights for a point at fractional position `f` in [0,1) between nodes 1 and 2 of the stencil.
fn lagrange(f: f64) -> [f64; 4] {
    [
        -f * (f - 1.0) * (f - 2.0) / 6.0,
        (f + 1.0) * (f - 1.0) * (f - 2.0) / 2.0,
        -(f + 1.0) * f * (f - 2.0) / 2.0,
        (f + 1.0) * f * (f - 1.0) / 6.0,
    ]
}

fn interp3(s: &[V3], i: usize, w: &[f64; 4]) -> V3 {
    let mut r = [0.0; 3];
    for (k, wk) in w.iter().enumerate() {
        for c in 0..3 {
            r[c] += wk * s[i - 1 + k][c];
        }
    }
    r
}

impl Surface {
    /// Number of time samples (0 if there are no elements).
    pub fn samples(&self) -> usize {
        self.elements.first().map_or(0, |e| e.p.len())
    }

    /// Check the data layout and the subsonic condition. Messages carry the element index and field.
    pub fn validate(&self) -> Result<(), FwhError> {
        let bad = |m: String| Err(FwhError(m));
        if !(self.rho0 > 0.0) || !(self.c0 > 0.0) {
            return bad(format!(
                "rho0 and c0 must be positive (rho0 = {}, c0 = {})",
                self.rho0, self.c0
            ));
        }
        if !(self.dt > 0.0) {
            return bad(format!("dt must be positive and uniform (dt = {})", self.dt));
        }
        let ns = self.samples();
        if self.elements.is_empty() || ns < 8 {
            return bad(format!(
                "surface needs at least one element and 8 time samples (have {} elements, {ns} samples)",
                self.elements.len()
            ));
        }
        for (k, e) in self.elements.iter().enumerate() {
            for (name, len) in [
                ("x", e.x.len()),
                ("n", e.n.len()),
                ("v", e.v.len()),
                ("p", e.p.len()),
                ("rho", e.rho.len()),
                ("u", e.u.len()),
            ] {
                if len != ns {
                    return bad(format!("elements[{k}].{name}: expected {ns} samples, got {len}"));
                }
            }
            if !(e.area > 0.0) {
                return bad(format!("elements[{k}].area: must be positive (got {})", e.area));
            }
            for (i, v) in e.v.iter().enumerate() {
                if norm(*v) >= self.c0 {
                    return bad(format!(
                        "elements[{k}].v[{i}]: surface speed {} is not subsonic (c0 = {})",
                        norm(*v),
                        self.c0
                    ));
                }
            }
        }
        Ok(())
    }

    fn prepare(&self) -> Vec<Prepared> {
        let dt = self.dt;
        let rho0 = self.rho0;
        self.elements
            .iter()
            .map(|e| {
                let ns = e.p.len();
                let mut uvec = Vec::with_capacity(ns);
                let mut l = Vec::with_capacity(ns);
                for i in 0..ns {
                    let rho = rho0 + e.rho[i];
                    let a = 1.0 - rho / rho0;
                    let b = rho / rho0;
                    let uu = add3(scale(e.v[i], a), scale(e.u[i], b));
                    let un = dot(e.u[i], e.n[i]);
                    let vn = dot(e.v[i], e.n[i]);
                    let li = add3(scale(e.n[i], e.p[i]), scale(e.u[i], rho * (un - vn)));
                    uvec.push(uu);
                    l.push(li);
                }
                Prepared {
                    area: e.area,
                    vdot: deriv3(&e.v, dt),
                    ndot: deriv3(&e.n, dt),
                    udot: deriv3(&uvec, dt),
                    ldot: deriv3(&l, dt),
                    x: e.x.clone(),
                    v: e.v.clone(),
                    n: e.n.clone(),
                    uvec,
                    l,
                }
            })
            .collect()
    }

    /// Observer times for which every element's retarded time lies in the usable part of the data
    /// (three samples away from either end). Returns `(t_min, t_max)`.
    pub fn valid_window(&self, obs: V3) -> Result<(f64, f64), FwhError> {
        self.validate()?;
        let ns = self.samples();
        let (mut dmin, mut dmax) = (f64::INFINITY, 0.0_f64);
        for e in &self.elements {
            for x in &e.x {
                let d = norm(sub(obs, *x));
                dmin = dmin.min(d);
                dmax = dmax.max(d);
            }
        }
        let t_first = self.t0 + 3.0 * self.dt;
        let t_last = self.t0 + (ns as f64 - 4.0) * self.dt;
        if t_last <= t_first {
            return Err(FwhError("time series too short for the interpolation stencil".into()));
        }
        // emission tau in [t_first, t_last], t = tau + r/c with r between dmin and dmax
        let lo = t_first + dmax / self.c0;
        let hi = t_last + dmin / self.c0;
        if hi <= lo {
            return Err(FwhError(format!(
                "no valid observer window: the record ({:.6e} s) is shorter than the travel-time spread ({:.6e} s)",
                t_last - t_first,
                (dmax - dmin) / self.c0
            )));
        }
        Ok((lo, hi))
    }

    /// Acoustic pressure at `obs` for each observer time in `times`.
    pub fn signal(&self, obs: V3, times: &[f64]) -> Result<Vec<Signal>, FwhError> {
        let (lo, hi) = self.valid_window(obs)?;
        for &t in times {
            if t < lo || t > hi {
                return Err(FwhError(format!(
                    "observer time {t:.6e} s is outside the valid window [{lo:.6e}, {hi:.6e}] s for this observer"
                )));
            }
        }
        let prep = self.prepare();
        let ns = self.samples();
        let c = self.c0;
        let rho0 = self.rho0;
        let mut out = Vec::with_capacity(times.len());
        for &t in times {
            let (mut pt, mut pl) = (0.0, 0.0);
            for e in &prep {
                // retarded time by Newton on g(tau) = tau + |obs - x(tau)|/c - t
                let locate = |tau: f64| -> (usize, f64) {
                    let s = (tau - self.t0) / self.dt;
                    let i = (s.floor() as isize).clamp(1, ns as isize - 3) as usize;
                    (i, s - i as f64)
                };
                let mut tau = t - norm(sub(obs, e.x[ns / 2])) / c;
                for _ in 0..60 {
                    let (i, f) = locate(tau);
                    let w = lagrange(f);
                    let y = interp3(&e.x, i, &w);
                    let vv = interp3(&e.v, i, &w);
                    let r = sub(obs, y);
                    let rn = norm(r);
                    let g = tau + rn / c - t;
                    let gp = 1.0 - dot(vv, r) / (c * rn);
                    let step = g / gp;
                    tau -= step;
                    if step.abs() < 1e-14 * (1.0 + tau.abs()) {
                        break;
                    }
                }
                let (i, f) = locate(tau);
                let w = lagrange(f);
                let y = interp3(&e.x, i, &w);
                let vv = interp3(&e.v, i, &w);
                let vd = interp3(&e.vdot, i, &w);
                let nn = interp3(&e.n, i, &w);
                let nd = interp3(&e.ndot, i, &w);
                let uu = interp3(&e.uvec, i, &w);
                let ud = interp3(&e.udot, i, &w);
                let ll = interp3(&e.l, i, &w);
                let ld = interp3(&e.ldot, i, &w);
                let rv = sub(obs, y);
                let r = norm(rv);
                let rh = scale(rv, 1.0 / r);
                let m = scale(vv, 1.0 / c);
                let mr = dot(m, rh);
                let m2 = dot(m, m);
                let mdr = dot(scale(vd, 1.0 / c), rh);
                let om = 1.0 - mr;
                let un = dot(uu, nn);
                let udn = dot(ud, nn) + dot(uu, nd);
                let common = r * mdr + c * (mr - m2);
                pt += e.area * (rho0 * udn / (r * om * om) + rho0 * un * common / (r * r * om * om * om));
                let lr = dot(ll, rh);
                let lm = dot(ll, m);
                let ldr = dot(ld, rh);
                pl += e.area
                    * ((ldr / c) / (r * om * om)
                        + (lr - lm) / (r * r * om * om)
                        + lr * common / (c * r * r * om * om * om));
            }
            let k = 1.0 / (4.0 * std::f64::consts::PI);
            out.push(Signal {
                t,
                p_thickness: pt * k,
                p_loading: pl * k,
            });
        }
        Ok(out)
    }

    /// Serialise as the `rcfd-fwh-surface` v1 JSON document.
    pub fn to_json(&self) -> Value {
        let v3 = |s: &[V3]| Value::Array(s.iter().map(|a| json!([a[0], a[1], a[2]])).collect());
        let els: Vec<Value> = self
            .elements
            .iter()
            .map(|e| {
                json!({"area": e.area, "x": v3(&e.x), "n": v3(&e.n), "v": v3(&e.v), "p": e.p, "rho": e.rho, "u": v3(&e.u)})
            })
            .collect();
        json!({"format": "rcfd-fwh-surface", "version": 1, "rho0": self.rho0, "c0": self.c0, "t0": self.t0, "dt": self.dt, "elements": els})
    }

    /// Parse the `rcfd-fwh-surface` v1 JSON document. Stationary shortcuts: `x0` (a single point) instead of `x`,
    /// `n0` instead of `n`; `v` may be omitted (zero), `rho` may be omitted (zero), `u` may be omitted (zero).
    /// Errors name the JSON path.
    pub fn from_json(doc: &Value) -> Result<Surface, FwhError> {
        let err = |m: String| FwhError(m);
        let obj = doc.as_object().ok_or_else(|| err("$: expected an object".into()))?;
        match obj.get("format").and_then(Value::as_str) {
            Some("rcfd-fwh-surface") => {}
            other => return Err(err(format!("$.format: expected \"rcfd-fwh-surface\", got {other:?}"))),
        }
        if obj.get("version").and_then(Value::as_u64) != Some(1) {
            return Err(err("$.version: only version 1 is supported".into()));
        }
        let num = |k: &str| -> Result<f64, FwhError> {
            obj.get(k)
                .and_then(Value::as_f64)
                .ok_or_else(|| err(format!("$.{k}: missing or not a number")))
        };
        let (rho0, c0, t0, dt) = (num("rho0")?, num("c0")?, num("t0")?, num("dt")?);
        let els = obj
            .get("elements")
            .and_then(Value::as_array)
            .ok_or_else(|| err("$.elements: missing or not an array".into()))?;
        let ns = els
            .first()
            .and_then(|e| e.get("p"))
            .and_then(Value::as_array)
            .map(Vec::len)
            .ok_or_else(|| err("$.elements[0].p: missing series".into()))?;
        let mut elements = Vec::new();
        for (k, e) in els.iter().enumerate() {
            let at = |f: &str| format!("$.elements[{k}].{f}");
            let area = e
                .get("area")
                .and_then(Value::as_f64)
                .ok_or_else(|| err(format!("{}: missing or not a number", at("area"))))?;
            let vec3 = |a: &Value, path: &str| -> Result<V3, FwhError> {
                let arr = a
                    .as_array()
                    .filter(|a| a.len() == 3)
                    .ok_or_else(|| err(format!("{path}: expected [x, y, z]")))?;
                let mut r = [0.0; 3];
                for (i, x) in arr.iter().enumerate() {
                    r[i] = x.as_f64().ok_or_else(|| err(format!("{path}[{i}]: not a number")))?;
                }
                Ok(r)
            };
            let series3 = |name: &str, single: Option<&str>, default: Option<V3>| -> Result<Vec<V3>, FwhError> {
                if let Some(v) = e.get(name) {
                    let arr = v
                        .as_array()
                        .ok_or_else(|| err(format!("{}: expected an array", at(name))))?;
                    if arr.len() != ns {
                        return Err(err(format!("{}: expected {ns} samples, got {}", at(name), arr.len())));
                    }
                    return arr
                        .iter()
                        .enumerate()
                        .map(|(i, a)| vec3(a, &format!("{}[{i}]", at(name))))
                        .collect();
                }
                if let Some(s) = single {
                    if let Some(v) = e.get(s) {
                        return Ok(vec![vec3(v, &at(s))?; ns]);
                    }
                }
                match default {
                    Some(d) => Ok(vec![d; ns]),
                    None => Err(err(format!("{}: missing", at(name)))),
                }
            };
            let series1 = |name: &str, default_zero: bool| -> Result<Vec<f64>, FwhError> {
                match e.get(name) {
                    Some(v) => {
                        let arr = v
                            .as_array()
                            .ok_or_else(|| err(format!("{}: expected an array", at(name))))?;
                        if arr.len() != ns {
                            return Err(err(format!("{}: expected {ns} samples, got {}", at(name), arr.len())));
                        }
                        arr.iter()
                            .enumerate()
                            .map(|(i, x)| {
                                x.as_f64()
                                    .ok_or_else(|| err(format!("{}[{i}]: not a number", at(name))))
                            })
                            .collect()
                    }
                    None if default_zero => Ok(vec![0.0; ns]),
                    None => Err(err(format!("{}: missing", at(name)))),
                }
            };
            elements.push(Element {
                area,
                x: series3("x", Some("x0"), None)?,
                n: series3("n", Some("n0"), None)?,
                v: series3("v", None, Some([0.0; 3]))?,
                p: series1("p", false)?,
                rho: series1("rho", true)?,
                u: series3("u", None, Some([0.0; 3]))?,
            });
        }
        let s = Surface {
            rho0,
            c0,
            t0,
            dt,
            elements,
        };
        s.validate()?;
        Ok(s)
    }
}
