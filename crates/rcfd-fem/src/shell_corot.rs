//! Geometrically nonlinear Shell4 and Shell3: the corotated facet element of the Newton driver (ADR 0490, F-578).
//!
//! * Unknowns (ADR 0490 D1): per node `(u_x, u_y, u_z, theta_x, theta_y, theta_z)`, the last three the total rotation
//!   pseudo-vector of the node, updated additively by the driver; the nodal triad is `R(theta) = exp([theta]_x)` times its
//!   reference (Rodrigues). The map is singular at `|theta| = 2 pi k`, `k >= 1`.
//! * Kinematics (D2): the reference facet of the linear element ([`crate::shell::facet`], [`crate::shell3::facet`]), the
//!   current projected points `p_a = X_a + u_a + R(theta_a) r_a` (the rigid offset link of the warped Shell4 turned with its
//!   node), the current frame from the projected points by the rule of the facet, the local deformational translations
//!   `d_a = Lambda (p_a - c) - Y_a` and rotations `phi_a = log(Lambda R(theta_a) Lambda_0^T)`.
//! * Energy (D2): `Pi = (1/2) d^T K_r d + Pi_m`. For the default membranes (the bilinear Shell4 membrane, the constant-strain
//!   Shell3 membrane) `K_r` is the linear local stiffness with the membrane matrix of the section set to zero (bending,
//!   transverse shear, drilling penalty) and `Pi_m` the total Lagrangian Green-strain membrane of the same interpolation and
//!   rule (St. Venant-Kirchhoff); for the drilling membranes (ANDES, Allman) `K_r` is the whole linear local stiffness and
//!   `Pi_m = 0` (the corotated linear membrane).
//! * Force and tangent (D2): `f = dPi/dq` and `K = d^2 Pi/dq^2` by forward differentiation of second order over the element
//!   unknowns; the tangent is formed on its upper triangle and mirrored, so it is symmetric bit for bit.
//!
//! [`crate::nonlinear`] evaluates Shell4 and Shell3 elements through [`model_force_and_tangent`] and
//! [`model_internal_force`]; [`element_force_and_tangent`] gives one element of a model (for checks).

use crate::element::ElementMatrix;
use crate::error::{FemError, Result};
use crate::mesh::{ElementKind, SolidMesh};
use crate::model::{Material, SolidModel};
use crate::shell::{SectionStiffness, Shell4Membrane, ShellOptions};
use crate::shell3::{Shell3Membrane, Shell3Options};
use rcfd_core::vec3::Vec3;

/// Below this value of `t^2` the Rodrigues coefficients take their Taylor series (ADR 0490 arithmetic 1).
pub const RODRIGUES_SERIES_BELOW: f64 = 1e-2;

/// Below this value of `sin^2 t` the factor `t / sin t` of the logarithm takes its Taylor series (ADR 0490 arithmetic 1).
pub const LOG_SERIES_BELOW: f64 = 1e-2;

// ------------------------------------------------------------------------------------------ second-order forward values

/// A value with its gradient and the upper triangle of its Hessian (row-major packed, `i <= j`) with respect to `n`
/// element unknowns.
#[derive(Debug, Clone)]
struct Hd {
    v: f64,
    g: Vec<f64>,
    h: Vec<f64>,
}

fn packed(n: usize) -> usize {
    n * (n + 1) / 2
}

impl Hd {
    fn constant(n: usize, v: f64) -> Hd {
        Hd {
            v,
            g: vec![0.0; n],
            h: vec![0.0; packed(n)],
        }
    }
    fn variable(n: usize, v: f64, i: usize) -> Hd {
        let mut x = Hd::constant(n, v);
        x.g[i] = 1.0;
        x
    }
    fn n(&self) -> usize {
        self.g.len()
    }
    fn add(&self, b: &Hd) -> Hd {
        Hd {
            v: self.v + b.v,
            g: self.g.iter().zip(&b.g).map(|(x, y)| x + y).collect(),
            h: self.h.iter().zip(&b.h).map(|(x, y)| x + y).collect(),
        }
    }
    fn sub(&self, b: &Hd) -> Hd {
        Hd {
            v: self.v - b.v,
            g: self.g.iter().zip(&b.g).map(|(x, y)| x - y).collect(),
            h: self.h.iter().zip(&b.h).map(|(x, y)| x - y).collect(),
        }
    }
    fn add_f(&self, c: f64) -> Hd {
        let mut x = self.clone();
        x.v += c;
        x
    }
    fn scale(&self, c: f64) -> Hd {
        Hd {
            v: self.v * c,
            g: self.g.iter().map(|x| x * c).collect(),
            h: self.h.iter().map(|x| x * c).collect(),
        }
    }
    /// `self += c b`.
    fn axpy(&mut self, c: f64, b: &Hd) {
        self.v += c * b.v;
        for (x, y) in self.g.iter_mut().zip(&b.g) {
            *x += c * y;
        }
        for (x, y) in self.h.iter_mut().zip(&b.h) {
            *x += c * y;
        }
    }
    fn mul(&self, b: &Hd) -> Hd {
        let n = self.n();
        let (a, av, bv) = (self, self.v, b.v);
        let g = a.g.iter().zip(&b.g).map(|(x, y)| av * y + bv * x).collect();
        let mut h = Vec::with_capacity(packed(n));
        let mut k = 0;
        for i in 0..n {
            let (ai, bi) = (a.g[i], b.g[i]);
            for j in i..n {
                h.push(av * b.h[k] + bv * a.h[k] + (ai * b.g[j] + bi * a.g[j]));
                k += 1;
            }
        }
        Hd { v: av * bv, g, h }
    }
    /// `f(self)` from `f`, `f'` and `f''` at `self.v`.
    fn chain(&self, f0: f64, f1: f64, f2: f64) -> Hd {
        let n = self.n();
        let g = self.g.iter().map(|x| f1 * x).collect();
        let mut h = Vec::with_capacity(packed(n));
        let mut k = 0;
        for i in 0..n {
            let gi = self.g[i];
            for j in i..n {
                h.push(f1 * self.h[k] + f2 * (gi * self.g[j]));
                k += 1;
            }
        }
        Hd { v: f0, g, h }
    }
    fn recip(&self) -> Hd {
        let x = self.v;
        self.chain(1.0 / x, -1.0 / (x * x), 2.0 / (x * x * x))
    }
    fn div(&self, b: &Hd) -> Hd {
        self.mul(&b.recip())
    }
    fn sqrt(&self) -> Hd {
        let r = self.v.sqrt();
        self.chain(r, 0.5 / r, -0.25 / (r * self.v))
    }
    fn sin(&self) -> Hd {
        let (s, c) = self.v.sin_cos();
        self.chain(s, c, -s)
    }
    fn cos(&self) -> Hd {
        let (s, c) = self.v.sin_cos();
        self.chain(c, -s, -c)
    }
    fn atan(&self) -> Hd {
        let x = self.v;
        let d = 1.0 / (1.0 + x * x);
        self.chain(x.atan(), d, -2.0 * x * d * d)
    }
    /// The polynomial `sum_k c_k self^k` by Horner's rule.
    fn polynomial(&self, c: &[f64]) -> Hd {
        let mut acc = Hd::constant(self.n(), *c.last().expect("one coefficient at least"));
        for &ck in c.iter().rev().skip(1) {
            acc = acc.mul(self).add_f(ck);
        }
        acc
    }
}

type V3 = [Hd; 3];
type M3 = [[Hd; 3]; 3];

fn v_sub(a: &V3, b: &V3) -> V3 {
    [a[0].sub(&b[0]), a[1].sub(&b[1]), a[2].sub(&b[2])]
}

fn v_dot(a: &V3, b: &V3) -> Hd {
    a[0].mul(&b[0]).add(&a[1].mul(&b[1])).add(&a[2].mul(&b[2]))
}

fn v_cross(a: &V3, b: &V3) -> V3 {
    [
        a[1].mul(&b[2]).sub(&a[2].mul(&b[1])),
        a[2].mul(&b[0]).sub(&a[0].mul(&b[2])),
        a[0].mul(&b[1]).sub(&a[1].mul(&b[0])),
    ]
}

fn v_scale(a: &V3, s: &Hd) -> V3 {
    [a[0].mul(s), a[1].mul(s), a[2].mul(s)]
}

/// `a / |a|`, refusing a vector of zero or non-finite length.
fn v_unit(a: &V3, what: &str) -> Result<V3> {
    let l2 = v_dot(a, a);
    if !(l2.v > 0.0 && l2.v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "geometry",
            reason: format!("the current {what} of the shell element has length^2 {:e}", l2.v),
        });
    }
    Ok(v_scale(a, &l2.sqrt().recip()))
}

/// `sum_k (-s)^k / (2k + 1)!`, `k <= 5` (ADR 0490 arithmetic 1).
const SERIES_A: [f64; 6] = [
    1.0,
    -1.0 / 6.0,
    1.0 / 120.0,
    -1.0 / 5040.0,
    1.0 / 362_880.0,
    -1.0 / 39_916_800.0,
];
/// `sum_k (-s)^k / (2k + 2)!`, `k <= 5`.
const SERIES_B: [f64; 6] = [
    0.5,
    -1.0 / 24.0,
    1.0 / 720.0,
    -1.0 / 40_320.0,
    1.0 / 3_628_800.0,
    -1.0 / 479_001_600.0,
];

/// `asin(x)/x = sum_k c_k x^(2k)`, `c_k = (2k)! / (4^k (k!)^2 (2k + 1))`, `k <= 9`.
fn series_asin() -> [f64; 10] {
    let mut c = [0.0; 10];
    let mut central = 1.0; // (2k)! / (4^k (k!)^2)
    for (k, ck) in c.iter_mut().enumerate() {
        if k > 0 {
            central *= (2 * k - 1) as f64 / (2 * k) as f64;
        }
        *ck = central / (2 * k + 1) as f64;
    }
    c
}

/// `R = I + a [theta]_x + b [theta]_x^2` (Rodrigues; D1).
fn rodrigues(theta: &V3) -> M3 {
    let n = theta[0].n();
    let s = v_dot(theta, theta);
    let (a, b) = if s.v < RODRIGUES_SERIES_BELOW {
        (s.polynomial(&SERIES_A), s.polynomial(&SERIES_B))
    } else {
        let t = s.sqrt();
        (t.sin().div(&t), Hd::constant(n, 1.0).sub(&t.cos()).div(&s))
    };
    let diag = Hd::constant(n, 1.0).sub(&b.mul(&s));
    // [theta]_x
    let zero = Hd::constant(n, 0.0);
    let skew = [
        [zero.clone(), theta[2].scale(-1.0), theta[1].clone()],
        [theta[2].clone(), zero.clone(), theta[0].scale(-1.0)],
        [theta[1].scale(-1.0), theta[0].clone(), zero],
    ];
    let mut r: Vec<[Hd; 3]> = Vec::with_capacity(3);
    for i in 0..3 {
        let row: Vec<Hd> = (0..3)
            .map(|j| {
                let mut x = a.mul(&skew[i][j]).add(&b.mul(&theta[i].mul(&theta[j])));
                if i == j {
                    x = x.add(&diag);
                }
                x
            })
            .collect();
        r.push(row.try_into().expect("three columns"));
    }
    r.try_into().expect("three rows")
}

/// `log R` for a rotation of less than 90 degrees: `(t / sin t) v`, `v` the axial vector of the skew part (D2 step 3).
fn logarithm(r: &M3) -> Result<V3> {
    let v = [
        r[2][1].sub(&r[1][2]).scale(0.5),
        r[0][2].sub(&r[2][0]).scale(0.5),
        r[1][0].sub(&r[0][1]).scale(0.5),
    ];
    let c = r[0][0].add(&r[1][1]).add(&r[2][2]).add_f(-1.0).scale(0.5);
    if c.v.is_nan() || c.v <= 0.0 {
        return Err(FemError::InvalidValue {
            key: "rotation",
            reason: format!(
                "a deformational rotation of the shell element reaches 90 degrees or more (cos = {:e}): the element is \
                 too coarse for the rotation (ADR 0490 D2)",
                c.v
            ),
        });
    }
    let sigma = v_dot(&v, &v);
    let g = if sigma.v < LOG_SERIES_BELOW {
        sigma.polynomial(&series_asin())
    } else {
        let s = sigma.sqrt();
        s.div(&c).atan().div(&s)
    };
    Ok(v_scale(&v, &g))
}

// ---------------------------------------------------------------------------------------------------------- the element

/// One point of the Green-strain membrane: weight and the derivatives `(N_a,X, N_a,Y)` on the projected facet.
#[derive(Debug, Clone)]
struct MembranePoint {
    weight: f64,
    nx: Vec<f64>,
    ny: Vec<f64>,
}

/// What an element evaluation needs from the reference configuration.
#[derive(Debug, Clone)]
struct Corotated {
    reference: Vec<Vec3>,
    links: Vec<Vec3>,
    axes0: [Vec3; 3],
    local: Vec<[f64; 2]>,
    k_rest: Vec<f64>,
    membrane: Option<([[f64; 3]; 3], Vec<MembranePoint>)>,
}

const CORNERS: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
const GAUSS_2: f64 = 0.577_350_269_189_625_8;

/// The 2 x 2 Gauss points of the bilinear membrane on the projected facet (the rule and weights of `shell.rs`).
fn bilinear_points(local: &[[f64; 2]; 4]) -> Vec<MembranePoint> {
    let mut out = Vec::with_capacity(4);
    for xi in [-GAUSS_2, GAUSS_2] {
        for eta in [-GAUSS_2, GAUSS_2] {
            let mut dx = [0.0; 4];
            let mut de = [0.0; 4];
            for (i, c) in CORNERS.iter().enumerate() {
                dx[i] = 0.25 * c[0] * (1.0 + c[1] * eta);
                de[i] = 0.25 * c[1] * (1.0 + c[0] * xi);
            }
            let mut j = [[0.0; 2]; 2];
            for i in 0..4 {
                j[0][0] += dx[i] * local[i][0];
                j[0][1] += dx[i] * local[i][1];
                j[1][0] += de[i] * local[i][0];
                j[1][1] += de[i] * local[i][1];
            }
            let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
            let inv = [[j[1][1] / det, -j[0][1] / det], [-j[1][0] / det, j[0][0] / det]];
            let nx = (0..4).map(|i| inv[0][0] * dx[i] + inv[0][1] * de[i]).collect();
            let ny = (0..4).map(|i| inv[1][0] * dx[i] + inv[1][1] * de[i]).collect();
            out.push(MembranePoint { weight: det, nx, ny });
        }
    }
    out
}

/// The one point of the constant-strain membrane (the derivatives of `shell3.rs`).
fn triangle_point(local: &[[f64; 2]; 3], area: f64) -> Vec<MembranePoint> {
    let a2 = 2.0 * area;
    let mut nx = vec![0.0; 3];
    let mut ny = vec![0.0; 3];
    for i in 0..3 {
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        nx[i] = (local[j][1] - local[k][1]) / a2;
        ny[i] = (local[k][0] - local[j][0]) / a2;
    }
    vec![MembranePoint { weight: area, nx, ny }]
}

fn without_membrane(s: &SectionStiffness) -> SectionStiffness {
    SectionStiffness::new([[0.0; 3]; 3], None, s.bending, s.shear, s.drilling)
}

fn corotated(mesh: &SolidMesh, element: usize, coords: &[Vec3], material: &Material) -> Result<Corotated> {
    let kind = mesh.element_kind(element);
    let section = crate::shell::model_section(mesh, element, material)?;
    if section.coupling.is_some() {
        return Err(FemError::InvalidValue {
            key: "nonlinear",
            reason: format!(
                "the section of the {} element {element} couples membrane and bending (a layup): the corotated shell \
                 takes uncoupled sections only (ADR 0490 D2)",
                kind.name()
            ),
        });
    }
    match kind {
        ElementKind::Shell4 => {
            let f = crate::shell::facet(coords)?;
            let membrane = crate::shell::model_membrane(mesh, element);
            let options = ShellOptions::default().membrane(membrane);
            let green = membrane == Shell4Membrane::Bilinear;
            let k = if green {
                crate::shell::local_stiffness_section(&f, &without_membrane(&section), &options)
            } else {
                crate::shell::local_stiffness_section(&f, &section, &options)
            };
            let e3 = f.axes[2];
            Ok(Corotated {
                reference: coords.to_vec(),
                links: f.offsets.iter().map(|h| [-h * e3[0], -h * e3[1], -h * e3[2]]).collect(),
                axes0: f.axes,
                local: f.local.to_vec(),
                k_rest: k.values().to_vec(),
                membrane: green.then(|| (section.membrane, bilinear_points(&f.local))),
            })
        }
        ElementKind::Shell3 => {
            let f = crate::shell3::facet(coords)?;
            let membrane = crate::shell3::model_membrane(mesh, element);
            let options = Shell3Options::default().membrane(membrane);
            let green = membrane == Shell3Membrane::ConstantStrain;
            let k = if green {
                crate::shell3::local_stiffness_section(&f, &without_membrane(&section), &options)
            } else {
                crate::shell3::local_stiffness_section(&f, &section, &options)
            };
            Ok(Corotated {
                reference: coords.to_vec(),
                links: vec![[0.0; 3]; 3],
                axes0: f.axes,
                local: f.local.to_vec(),
                k_rest: k.values().to_vec(),
                membrane: green.then(|| (section.membrane, triangle_point(&f.local, f.area))),
            })
        }
        other => Err(FemError::InvalidValue {
            key: "element kind",
            reason: format!(
                "the corotated shell takes Shell4 and Shell3 elements, not {}",
                other.name()
            ),
        }),
    }
}

/// The internal force and, if asked, the tangent (dense, row-major) of a corotated element at `ue`.
fn evaluate(c: &Corotated, ue: &[f64], want_tangent: bool) -> Result<(Vec<f64>, Option<Vec<f64>>)> {
    let nn = c.reference.len();
    let m = 6 * nn;
    if ue.len() != m {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: m,
            found: ue.len(),
        });
    }
    if let Some(i) = ue.iter().position(|v| !v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "displacement",
            reason: format!("element unknown {i} is not finite"),
        });
    }
    let q: Vec<Hd> = ue.iter().enumerate().map(|(i, &v)| Hd::variable(m, v, i)).collect();
    // The nodal rotations and the displacements `w_a = u_a + (R_a - I) r_a` of the projected points (D2 step 1).
    let mut w: Vec<V3> = Vec::with_capacity(nn);
    let mut rot: Vec<M3> = Vec::with_capacity(nn);
    for a in 0..nn {
        let theta = [q[6 * a + 3].clone(), q[6 * a + 4].clone(), q[6 * a + 5].clone()];
        let r = rodrigues(&theta);
        let link = c.links[a];
        let wa: Vec<Hd> = (0..3)
            .map(|i| {
                let mut x = q[6 * a + i].clone();
                if link != [0.0; 3] {
                    // `(R - I) r`: the offset turned with the node (zero at rest).
                    for (j, lj) in link.iter().enumerate() {
                        let rij = if i == j { r[i][j].add_f(-1.0) } else { r[i][j].clone() };
                        x.axpy(*lj, &rij);
                    }
                }
                x
            })
            .collect();
        w.push(wa.try_into().expect("three components"));
        rot.push(r);
    }
    // ADR 0490 amendment 2: the positions relative to the first projected point, each a difference of displacements plus
    // a difference of reference points, so that the rounding is relative to the element and not to the coordinates.
    let w_rel: Vec<V3> = w.iter().map(|wa| v_sub(wa, &w[0])).collect();
    let p: Vec<V3> = (0..nn)
        .map(|a| {
            let pa: Vec<Hd> = (0..3)
                .map(|i| {
                    let dx = (c.reference[a][i] - c.reference[0][i]) + (c.links[a][i] - c.links[0][i]);
                    w_rel[a][i].add_f(dx)
                })
                .collect();
            pa.try_into().expect("three components")
        })
        .collect();
    // The current frame (D2 step 2).
    let normal = if nn == 4 {
        v_cross(&v_sub(&p[2], &p[0]), &v_sub(&p[3], &p[1]))
    } else {
        v_cross(&v_sub(&p[1], &p[0]), &v_sub(&p[2], &p[0]))
    };
    let e3 = v_unit(&normal, "normal")?;
    let edge = v_sub(&p[1], &p[0]);
    let along = v_dot(&edge, &e3);
    let e1 = v_unit(&v_sub(&edge, &v_scale(&e3, &along)), "first edge")?;
    let e2 = v_cross(&e3, &e1);
    let frame = [e1, e2, e3];
    let mut centre: Vec<Hd> = (0..3).map(|_| Hd::constant(m, 0.0)).collect();
    for pa in &p {
        for k in 0..3 {
            centre[k].axpy(1.0 / nn as f64, &pa[k]);
        }
    }
    let centre: V3 = centre.try_into().expect("three components");
    // The local deformational unknowns (D2 step 3).
    let mut d: Vec<Hd> = Vec::with_capacity(m);
    for a in 0..nn {
        let rel = v_sub(&p[a], &centre);
        for (k, ek) in frame.iter().enumerate() {
            let y = if k < 2 { c.local[a][k] } else { 0.0 };
            d.push(v_dot(ek, &rel).add_f(-y));
        }
        // M = R_a Lambda_0^T, R_def = Lambda M.
        let r = &rot[a];
        let mut rdef: Vec<[Hd; 3]> = Vec::with_capacity(3);
        let mcols: Vec<V3> = (0..3)
            .map(|jj| {
                let col: Vec<Hd> = (0..3)
                    .map(|i| {
                        let mut x = Hd::constant(m, 0.0);
                        for l in 0..3 {
                            x.axpy(c.axes0[jj][l], &r[i][l]);
                        }
                        x
                    })
                    .collect();
                col.try_into().expect("three components")
            })
            .collect();
        for ek in &frame {
            let row: Vec<Hd> = mcols.iter().map(|col| v_dot(ek, col)).collect();
            rdef.push(row.try_into().expect("three columns"));
        }
        let rdef: M3 = rdef.try_into().expect("three rows");
        let phi = logarithm(&rdef)?;
        d.extend(phi);
    }
    // The energy (D2 steps 4, 5): f = J^T s + grad Pi_m, K = J^T K_r J + sum s_i Hess d_i + Hess Pi_m.
    let s: Vec<f64> = (0..m)
        .map(|i| (0..m).map(|j| c.k_rest[i * m + j] * d[j].v).sum())
        .collect();
    let mut acc = Hd::constant(m, 0.0);
    for (si, di) in s.iter().zip(&d) {
        acc.axpy(*si, di);
    }
    if let Some((a, points)) = &c.membrane {
        for pt in points {
            // ADR 0490 amendment 2: the Green strain from the displacement gradients `h_X = sum N_a,X w_a` (the reference
            // tangents are `e_1`, `e_2` of the facet), without forming `g . g - 1`.
            let mut hx: Vec<Hd> = (0..3).map(|_| Hd::constant(m, 0.0)).collect();
            let mut hy: Vec<Hd> = (0..3).map(|_| Hd::constant(m, 0.0)).collect();
            for (b, wb) in w_rel.iter().enumerate() {
                for k in 0..3 {
                    hx[k].axpy(pt.nx[b], &wb[k]);
                    hy[k].axpy(pt.ny[b], &wb[k]);
                }
            }
            let hx: V3 = hx.try_into().expect("three components");
            let hy: V3 = hy.try_into().expect("three components");
            let along = |e: &Vec3, h: &V3| {
                let mut x = Hd::constant(m, 0.0);
                for k in 0..3 {
                    x.axpy(e[k], &h[k]);
                }
                x
            };
            let (e1, e2) = (&c.axes0[0], &c.axes0[1]);
            let eps = [
                along(e1, &hx).add(&v_dot(&hx, &hx).scale(0.5)),
                along(e2, &hy).add(&v_dot(&hy, &hy).scale(0.5)),
                along(e1, &hy).add(&along(e2, &hx)).add(&v_dot(&hx, &hy)),
            ];
            for r in 0..3 {
                let mut ae = Hd::constant(m, 0.0);
                for (cs, es) in a[r].iter().zip(&eps) {
                    ae.axpy(*cs, es);
                }
                acc.axpy(0.5 * pt.weight, &eps[r].mul(&ae));
            }
        }
    }
    // The membrane energy enters `acc` as an energy: its gradient and Hessian are those of `Pi_m`; the first part of `acc`
    // is `sum s_i d_i`, whose gradient is `J^T s` and whose Hessian is `sum s_i Hess d_i`.
    let force = acc.g.clone();
    if !want_tangent {
        return Ok((force, None));
    }
    let mut k = vec![0.0; m * m];
    // J^T K_r J on the upper triangle.
    let mut kj = vec![0.0; m * m]; // K_r J
    for i in 0..m {
        for j in 0..m {
            let mut v = 0.0;
            for l in 0..m {
                v += c.k_rest[i * m + l] * d[l].g[j];
            }
            kj[i * m + j] = v;
        }
    }
    let mut idx = 0;
    for i in 0..m {
        for j in i..m {
            let mut v = 0.0;
            for l in 0..m {
                v += d[l].g[i] * kj[l * m + j];
            }
            let x = v + acc.h[idx];
            k[i * m + j] = x;
            k[j * m + i] = x;
            idx += 1;
        }
    }
    Ok((force, Some(k)))
}

fn locate(element: usize) -> impl Fn(FemError) -> FemError {
    move |source| match source {
        e @ FemError::Element { .. } => e,
        other => FemError::Element {
            element,
            source: Box::new(other),
        },
    }
}

/// The internal force and the tangent of the shell element `element` of a mesh with the node coordinates `coords` and its
/// material, at the element unknowns `ue` (node-major, six per node) (ADR 0490 D2).
pub fn model_force_and_tangent(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
    ue: &[f64],
) -> Result<(Vec<f64>, ElementMatrix)> {
    let c = corotated(mesh, element, coords, material)?;
    let (f, k) = evaluate(&c, ue, true)?;
    let n = f.len();
    let mut out = ElementMatrix::zeros(n);
    out.values_mut().copy_from_slice(&k.expect("tangent requested"));
    Ok((f, out))
}

/// The internal force of the shell element `element` (as [`model_force_and_tangent`]).
pub fn model_internal_force(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
    ue: &[f64],
) -> Result<Vec<f64>> {
    let c = corotated(mesh, element, coords, material)?;
    Ok(evaluate(&c, ue, false)?.0)
}

/// The internal force and the tangent of the shell element `element` of `model` at the element unknowns `ue`; errors are
/// located (`FemError::Element`).
pub fn element_force_and_tangent(model: &SolidModel, element: usize, ue: &[f64]) -> Result<(Vec<f64>, ElementMatrix)> {
    let mesh = model.mesh();
    if element >= mesh.n_elements() {
        return Err(FemError::InvalidValue {
            key: "element",
            reason: format!("element {element} of {}", mesh.n_elements()),
        });
    }
    let materials = model.material_indices()?;
    let coords: Vec<Vec3> = mesh.element_nodes(element).iter().map(|&n| mesh.nodes()[n]).collect();
    model_force_and_tangent(mesh, element, &coords, &model.materials()[materials[element]].1, ue)
        .map_err(locate(element))
}

/// The refusals of the Newton driver for a model with shells (ADR 0490 D3): the shell checks of the linear path
/// ([`crate::shell::check_shell_model`]) and a shell element whose material is not `IsotropicElastic` (key `"nonlinear"`).
pub(crate) fn check_model(model: &SolidModel) -> Result<()> {
    crate::shell::check_shell_model(model)?;
    let mesh = model.mesh();
    let materials = model.material_indices()?;
    for e in 0..mesh.n_elements() {
        let kind = mesh.element_kind(e);
        if !matches!(kind, ElementKind::Shell4 | ElementKind::Shell3) {
            continue;
        }
        let (set, m) = &model.materials()[materials[e]];
        if !matches!(m, Material::IsotropicElastic { .. }) {
            return Err(FemError::InvalidValue {
                key: "nonlinear",
                reason: format!(
                    "element {e} ({}) has the {} material of element set '{set}': the corotated shell (ADR 0490) takes \
                     the isotropic elastic material only",
                    kind.name(),
                    m.name()
                ),
            });
        }
    }
    Ok(())
}
