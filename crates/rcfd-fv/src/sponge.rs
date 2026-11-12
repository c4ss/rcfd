//! Sponge (absorbing) layer of the compressible solver (F-331, ADR 0237): an opt-in source term that relaxes the state
//! of chosen cells to a reference state, so that waves leaving the region of interest are absorbed before they reach
//! the boundary.
//!
//! Two forms. [`SpongeForm::Plain`] relaxes the five conserved variables with one rate per cell, `S = -rate (U - U_ref)`
//! (Israeli and Orszag, J. Comput. Phys. 41 (1981); Bodony, J. Comput. Phys. 212 (2006)). It reflects nothing at normal
//! incidence for any profile of the rate, and `(cos - 1)/(cos + 1)` of the long-wave part of a wave arriving at the
//! angle `theta` (ADR 0237 DR2, DR3). [`SpongeForm::Matched`] is the perfectly matched layer of linear acoustics about a
//! state at rest in its unsplit form (Berenger, J. Comput. Phys. 114 (1994); Hu, J. Comput. Phys. 173 (2001); derived in
//! ADR 0237 DR4): mass, energy and the momentum along the layer normal relax as in the plain form, the tangential
//! momentum is not damped but forced by `-rate grad_t Q`, with the memory `dQ/dt = p - p_ref` per cell. In the
//! differential equations it reflects nothing at any angle; a wave returns with the reflection of the boundary behind
//! the layer times `R0^cos(theta)`, `R0` the two-way attenuation at normal incidence.
//!
//! [`SpongeForm::MatchedShifted`] (F-340, ADR 0248) is the matched form with three opt-in additions: a complex frequency
//! shift `a` (the memory decays, `dQ/dt = p - p_ref - a Q`, so the layer stays bounded at zero frequency and no longer
//! absorbs below `omega ~ a`; ADR 0248 DR11), a second planar layer with an orthogonal normal (a corner, DR9) and a
//! damping of the momentum along the layer (`along`, a fraction of the layer rate; it is a mismatch and reflects about
//! `1.3 along` of a pulse arriving at 60 degrees, DR12).
//!
//! The source is explicit: `rate dt` must stay well below 2.5 (SSP-RK3).

use crate::geometry::FvGeometry;
use crate::riemann::{to_prim, Cons, Gas, Prim};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};

/// The form of the sponge source (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpongeForm {
    /// Every conserved variable relaxes to the reference state.
    Plain,
    /// The matched layer of one planar layer; needs a reference state at rest.
    Matched {
        /// Unit normal of the layer, pointing from the interior into the layer.
        normal: Vec3,
    },
    /// The matched form with a frequency shift, an optional second layer and an optional damping along the layer
    /// (F-340, ADR 0248); needs a reference state at rest.
    MatchedShifted(MatchedOptions),
}

/// The options of [`SpongeForm::MatchedShifted`] (ADR 0248 E1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchedOptions {
    /// Unit normal of the layer whose rates are [`Sponge::rate`], pointing from the interior into the layer.
    pub normal: Vec3,
    /// The frequency shift `a` [1/s], finite and `>= 0`: the decay rate of the memories. Zero is the form without decay.
    pub shift: f64,
    /// The damping of the momentum along the layer as a fraction of the layer rate, finite and `>= 0` (zero: none).
    pub along: f64,
    /// A second planar layer whose normal is orthogonal to `normal` (a corner of two layers); its rates are evaluated
    /// at the cell centres when the sponge is set.
    pub second: Option<SpongeLayer>,
}

/// The memories of [`SpongeForm::MatchedShifted`], one value per cell: `q` of `p - p_ref`, `r` of `q` and `m` of the
/// momentum, each with the decay `shift` (ADR 0248 DR11, DR14).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpongeMemories {
    /// `dq/dt = p - p_ref - shift q`.
    pub q: Vec<f64>,
    /// `dr/dt = q - shift r`.
    pub r: Vec<f64>,
    /// `dm/dt = momentum - shift m`.
    pub m: Vec<Vec3>,
}

/// The frame of a sponge when the mesh moves (F-341, ADR 0265). The reference state is a lab-frame state in either
/// frame (ADR 0265 DR1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpongeFrame {
    /// The sponge is for a mesh that does not move: `CompressibleSolver::step_ale` refuses it (ADR 0237 D5). The default.
    #[default]
    StaticMesh,
    /// The rates and the memories belong to the cells and move with them (ADR 0265 DR2, DR3): the memory of a cell is the
    /// time integral of `p - p_ref` along the path of the cell, and the source enters the ALE step at the stage volume
    /// (DR4). The matched forms are matched only at normal incidence on a moving mesh (DR5); with a mesh velocity along a
    /// matched layer the attached memory makes the layer unstable for waves grazing along it (growth about
    /// `rate M / (2 (1 - M))`, ADR 0344 DR2): use [`SpongeFrame::AttachedWithLabMemory`] there.
    AttachedToMesh,
    /// The rates belong to the cells as in [`SpongeFrame::AttachedToMesh`]; the memories of the matched forms are those of
    /// the lab frame (F-407, ADR 0344 DR3): the memory of a cell moving with `w_c` gains `w_c . grad Q` (and likewise `r`
    /// and `m` of the shifted form), so on a mesh translating along a planar layer the layer is the matched layer at rest.
    /// The plain form has no memory and gives the bits of `AttachedToMesh`; so does a cell that does not move.
    AttachedWithLabMemory,
}

/// A sponge: the reference state, one damping rate [1/s] per cell (zero outside the layer) and the form.
#[derive(Debug, Clone, PartialEq)]
pub struct Sponge {
    /// The state the layer relaxes to.
    pub reference: Prim,
    /// Damping rate of every cell [1/s], finite and `>= 0`.
    pub rate: Vec<f64>,
    /// Plain or matched.
    pub form: SpongeForm,
}

/// A planar layer with a polynomial profile: the rate is zero in front of the plane through `origin`, rises as
/// `rate_max (d/thickness)^exponent` with the depth `d` along `normal` and stays `rate_max` beyond `thickness`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpongeLayer {
    /// A point on the front plane of the layer.
    pub origin: Vec3,
    /// Normal of the layer, pointing from the interior into the layer (any length but zero).
    pub normal: Vec3,
    /// Thickness of the layer [m].
    pub thickness: f64,
    /// Rate at the end of the layer [1/s].
    pub rate_max: f64,
    /// Exponent of the profile (2 is the usual choice; 0 is a uniform rate).
    pub exponent: f64,
}

/// The peak rate of a polynomial profile whose two-way attenuation at normal incidence is `reflection` (ADR 0237 DR5):
/// `(exponent + 1) c ln(1/reflection) / (2 thickness)`.
pub fn rate_max_for_reflection(thickness: f64, sound_speed: f64, reflection: f64, exponent: f64) -> Result<f64> {
    let ok = thickness.is_finite()
        && thickness > 0.0
        && sound_speed.is_finite()
        && sound_speed > 0.0
        && reflection > 0.0
        && reflection < 1.0
        && exponent.is_finite()
        && exponent >= 0.0;
    if !ok {
        return Err(Error::InvalidValue {
            path: "sponge".into(),
            reason: format!(
                "the peak rate needs a positive thickness and sound speed, a reflection in (0, 1) and an exponent >= 0, \
                 got {thickness}, {sound_speed}, {reflection}, {exponent}"
            ),
        });
    }
    Ok((exponent + 1.0) * sound_speed * (1.0 / reflection).ln() / (2.0 * thickness))
}

impl SpongeLayer {
    fn check(&self) -> Result<Vec3> {
        let len = vec3::norm(self.normal);
        let ok = self.origin.iter().all(|x| x.is_finite())
            && len.is_finite()
            && len > 0.0
            && self.thickness.is_finite()
            && self.thickness > 0.0
            && self.rate_max.is_finite()
            && self.rate_max >= 0.0
            && self.exponent.is_finite()
            && self.exponent >= 0.0;
        if !ok {
            return Err(Error::InvalidValue {
                path: "sponge layer".into(),
                reason:
                    "a finite origin, a non-zero normal, a positive thickness, a peak rate >= 0 and an exponent >= 0 \
                         are required"
                        .into(),
            });
        }
        Ok(vec3::scale(self.normal, 1.0 / len))
    }

    /// The rate at the point `x`; an error for an invalid layer.
    pub fn rate(&self, x: Vec3) -> Result<f64> {
        let n = self.check()?;
        Ok(self.rate_checked(n, x))
    }

    fn rate_checked(&self, n: Vec3, x: Vec3) -> f64 {
        let d = vec3::dot(vec3::sub(x, self.origin), n);
        if d <= 0.0 {
            0.0
        } else {
            self.rate_max * (d.min(self.thickness) / self.thickness).powf(self.exponent)
        }
    }

    /// The rate of every cell of `geom` (at the cell centre).
    pub fn rates(&self, geom: &FvGeometry) -> Result<Vec<f64>> {
        let n = self.check()?;
        Ok(geom
            .mesh
            .cell_centres()
            .iter()
            .map(|&x| self.rate_checked(n, x))
            .collect())
    }
}

impl Sponge {
    /// The sponge of one planar layer on `geom`, in the plain form or (`matched`) in the matched form with the layer's
    /// normal.
    pub fn planar(geom: &FvGeometry, reference: Prim, layer: &SpongeLayer, matched: bool) -> Result<Self> {
        let normal = layer.check()?;
        let sponge = Self {
            reference,
            rate: layer.rates(geom)?,
            form: if matched {
                SpongeForm::Matched { normal }
            } else {
                SpongeForm::Plain
            },
        };
        sponge.validate(geom.n_cells())?;
        Ok(sponge)
    }

    /// The sponge of one planar layer in the shifted matched form (ADR 0248): frequency shift `shift` [1/s] and the
    /// damping along the layer `along` (a fraction of the rate).
    pub fn planar_shifted(
        geom: &FvGeometry,
        reference: Prim,
        layer: &SpongeLayer,
        shift: f64,
        along: f64,
    ) -> Result<Self> {
        let normal = layer.check()?;
        let sponge = Self {
            reference,
            rate: layer.rates(geom)?,
            form: SpongeForm::MatchedShifted(MatchedOptions {
                normal,
                shift,
                along,
                second: None,
            }),
        };
        sponge.validate(geom.n_cells())?;
        Ok(sponge)
    }

    /// The sponge of two planar layers with orthogonal normals that overlap in a corner, in the shifted matched form
    /// (ADR 0248 DR9).
    pub fn corner(
        geom: &FvGeometry,
        reference: Prim,
        first: &SpongeLayer,
        second: &SpongeLayer,
        shift: f64,
        along: f64,
    ) -> Result<Self> {
        let normal = first.check()?;
        let sponge = Self {
            reference,
            rate: first.rates(geom)?,
            form: SpongeForm::MatchedShifted(MatchedOptions {
                normal,
                shift,
                along,
                second: Some(*second),
            }),
        };
        sponge.validate(geom.n_cells())?;
        Ok(sponge)
    }

    /// The rates of the second layer of the shifted form at the cell centres of `geom`; zero in every cell for the
    /// other forms and without a second layer.
    pub fn second_rates(&self, geom: &FvGeometry) -> Result<Vec<f64>> {
        match self.form {
            SpongeForm::MatchedShifted(MatchedOptions { second: Some(l), .. }) => l.rates(geom),
            _ => Ok(vec![0.0; geom.n_cells()]),
        }
    }

    /// Check the sponge against a mesh of `n_cells` cells: one finite rate `>= 0` per cell, a reference with positive
    /// finite density and pressure and a finite velocity; for the matched form a unit normal and a reference at rest.
    pub fn validate(&self, n_cells: usize) -> Result<()> {
        let fail = |reason: String| {
            Err(Error::InvalidValue {
                path: "sponge".into(),
                reason,
            })
        };
        if self.rate.len() != n_cells {
            return fail(format!(
                "one rate per cell is required ({} rates for {n_cells} cells)",
                self.rate.len()
            ));
        }
        if self.rate.iter().any(|r| !(r.is_finite() && *r >= 0.0)) {
            return fail("every rate must be finite and >= 0".into());
        }
        let r = &self.reference;
        if !(r.rho.is_finite() && r.rho > 0.0 && r.p.is_finite() && r.p > 0.0 && r.u.iter().all(|x| x.is_finite())) {
            return fail(
                "the reference state needs a positive finite density and pressure and a finite velocity".into(),
            );
        }
        if let SpongeForm::Matched { normal } = self.form {
            let len = vec3::norm(normal);
            if !len.is_finite() || (len - 1.0).abs() > 1e-12 {
                return fail("the matched form needs a unit normal".into());
            }
            if r.u != [0.0; 3] {
                return fail("the matched form is derived for a reference state at rest; use the plain form".into());
            }
        }
        if let SpongeForm::MatchedShifted(o) = self.form {
            let len = vec3::norm(o.normal);
            if !len.is_finite() || (len - 1.0).abs() > 1e-12 {
                return fail("the matched form needs a unit normal".into());
            }
            if r.u != [0.0; 3] {
                return fail("the matched form is derived for a reference state at rest; use the plain form".into());
            }
            if !(o.shift.is_finite() && o.shift >= 0.0 && o.along.is_finite() && o.along >= 0.0) {
                return fail("the shift and the damping along the layer must be finite and >= 0".into());
            }
            if let Some(l) = o.second {
                let n2 = l.check()?;
                if vec3::dot(o.normal, n2).abs() > 1e-12 {
                    return fail("the normal of the second layer must be orthogonal to the first".into());
                }
            }
        }
        Ok(())
    }
}

/// Add the sponge source of the state `u` with the memory `q` to the residual per unit volume `res` and return `dQ/dt`
/// of every cell (zero in the plain form and outside the layer). Cells with rate zero are not touched.
pub(crate) fn add_source(
    geom: &FvGeometry,
    gas: Gas,
    sponge: &Sponge,
    u: &[Cons],
    q: &[f64],
    res: &mut [Cons],
) -> Vec<f64> {
    let rate = &sponge.rate;
    let mut qdot = vec![0.0; u.len()];
    let uref = sponge.reference.to_cons(gas);
    match sponge.form {
        SpongeForm::Plain => {
            for c in 0..u.len() {
                let s = rate[c];
                if s > 0.0 {
                    for k in 0..5 {
                        res[c][k] -= s * (u[c][k] - uref[k]);
                    }
                }
            }
        }
        SpongeForm::Matched { normal: n } => {
            let mesh = &geom.mesh;
            let (own, nei) = (mesh.owner(), mesh.neighbour());
            let sf = mesh.face_area_vectors();
            let vol = mesh.cell_volumes();
            // Green-Gauss sum of (Q_f - Q_c) S_f; the face value is the mean on a face between two layer cells and the
            // cell's own value elsewhere (ADR 0237 D2)
            let mut g = vec![[0.0; 3]; u.len()];
            for f in 0..geom.n_internal {
                let (o, nb) = (own[f], nei[f]);
                if rate[o] > 0.0 && rate[nb] > 0.0 {
                    let h = 0.5 * (q[nb] - q[o]);
                    vec3::axpy(&mut g[o], h, sf[f]);
                    vec3::axpy(&mut g[nb], h, sf[f]);
                }
            }
            for c in 0..u.len() {
                let s = rate[c];
                if s > 0.0 {
                    let prim = to_prim(gas, &u[c]);
                    let grad = vec3::scale(g[c], 1.0 / vol[c]);
                    let force = vec3::scale(vec3::sub(grad, vec3::scale(n, vec3::dot(grad, n))), -s);
                    let m = [u[c][1] - uref[1], u[c][2] - uref[2], u[c][3] - uref[3]];
                    let mn = vec3::dot(m, n);
                    res[c][0] -= s * (u[c][0] - uref[0]);
                    for i in 0..3 {
                        res[c][1 + i] += force[i] - s * mn * n[i];
                    }
                    res[c][4] += vec3::dot(prim.u, force) - s * (u[c][4] - uref[4]);
                    qdot[c] = prim.p - sponge.reference.p;
                }
            }
        }
        // the shifted form has its own routine, `add_source_shifted`
        SpongeForm::MatchedShifted(_) => {}
    }
    qdot
}

/// Green-Gauss sum of `(q_f - q_c) S_f` per cell: the face value is the mean on a face between two cells for which
/// `inside` holds and the cell's own value elsewhere (ADR 0237 D2).
fn layer_gradient_sum(geom: &FvGeometry, q: &[f64], inside: &dyn Fn(usize) -> bool) -> Vec<Vec3> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let sf = mesh.face_area_vectors();
    let mut g = vec![[0.0; 3]; q.len()];
    for f in 0..geom.n_internal {
        let (o, nb) = (own[f], nei[f]);
        if inside(o) && inside(nb) {
            let h = 0.5 * (q[nb] - q[o]);
            vec3::axpy(&mut g[o], h, sf[f]);
            vec3::axpy(&mut g[nb], h, sf[f]);
        }
    }
    g
}

/// The source of [`SpongeForm::MatchedShifted`] (ADR 0248 DR14): add it for the state `u` with the memories `mem` to
/// the residual per unit volume `res` and return the time derivatives of the memories (zero outside the layers).
/// `rate2` holds the rates of the second layer (all zero without one). With one layer, `shift = 0` and `along = 0` the
/// arithmetic on the state and on `q` is that of [`SpongeForm::Matched`], operation for operation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn add_source_shifted(
    geom: &FvGeometry,
    gas: Gas,
    sponge: &Sponge,
    opts: &MatchedOptions,
    rate2: &[f64],
    u: &[Cons],
    mem: &SpongeMemories,
    res: &mut [Cons],
) -> SpongeMemories {
    let rate = &sponge.rate;
    let n_cells = u.len();
    let mut dot = SpongeMemories {
        q: vec![0.0; n_cells],
        r: vec![0.0; n_cells],
        m: vec![[0.0; 3]; n_cells],
    };
    let uref = sponge.reference.to_cons(gas);
    let n = opts.normal;
    let (a, beta) = (opts.shift, opts.along);
    let c2 = gas.gamma * sponge.reference.p / sponge.reference.rho;
    let gm1 = gas.gamma - 1.0;
    let vol = geom.mesh.cell_volumes();
    let Some(second) = opts.second else {
        // one layer: the statements of the matched form, plus the terms of the shift and of the damping along the
        // layer where they are not zero
        let g = layer_gradient_sum(geom, &mem.q, &|c| rate[c] > 0.0);
        for c in 0..n_cells {
            let s = rate[c];
            if s > 0.0 {
                let prim = to_prim(gas, &u[c]);
                let grad = vec3::scale(g[c], 1.0 / vol[c]);
                let mut force = vec3::scale(vec3::sub(grad, vec3::scale(n, vec3::dot(grad, n))), -s);
                let m = [u[c][1] - uref[1], u[c][2] - uref[2], u[c][3] - uref[3]];
                let mut mn = vec3::dot(m, n);
                if beta != 0.0 {
                    for i in 0..3 {
                        force[i] -= beta * s * (m[i] - mn * n[i]);
                    }
                }
                let (mut d_rho, mut d_e, mut d_p) = (u[c][0] - uref[0], u[c][4] - uref[4], prim.p - sponge.reference.p);
                if a != 0.0 {
                    mn -= a * vec3::dot(mem.m[c], n);
                    d_rho -= a * mem.q[c] / c2;
                    d_e -= a * mem.q[c] / gm1;
                    d_p -= a * mem.q[c];
                }
                res[c][0] -= s * d_rho;
                for i in 0..3 {
                    res[c][1 + i] += force[i] - s * mn * n[i];
                }
                res[c][4] += vec3::dot(prim.u, force) - s * d_e;
                dot.q[c] = d_p;
                dot.r[c] = mem.q[c] - a * mem.r[c];
                for i in 0..3 {
                    dot.m[c][i] = m[i] - a * mem.m[c][i];
                }
            }
        }
        return dot;
    };
    // two layers with orthogonal normals (DR9, DR11, DR14); the second normal is valid by `Sponge::validate`
    let n2 = vec3::scale(second.normal, 1.0 / vec3::norm(second.normal));
    let e3 = vec3::cross(n, n2);
    let inside = |c: usize| rate[c] + rate2[c] > 0.0;
    let gq = layer_gradient_sum(geom, &mem.q, &inside);
    let gr = layer_gradient_sum(geom, &mem.r, &inside);
    for c in 0..n_cells {
        let (s1, s2) = (rate[c], rate2[c]);
        if s1 + s2 > 0.0 {
            let prim = to_prim(gas, &u[c]);
            let grad_q = vec3::scale(gq[c], 1.0 / vol[c]);
            let grad_r = vec3::scale(gr[c], 1.0 / vol[c]);
            let m = [u[c][1] - uref[1], u[c][2] - uref[2], u[c][3] - uref[3]];
            let (m1, m2) = (vec3::dot(m, n), vec3::dot(m, n2));
            let (q, r) = (mem.q[c], mem.r[c]);
            let f1 = -s2 * vec3::dot(grad_q, n);
            let f2 = -s1 * vec3::dot(grad_q, n2);
            let f3 = -((s1 + s2) * vec3::dot(grad_q, e3) + s1 * s2 * vec3::dot(grad_r, e3));
            let mut force = [0.0; 3];
            for i in 0..3 {
                force[i] =
                    f1 * n[i] + f2 * n2[i] + f3 * e3[i] - beta * (s1 * (m[i] - m1 * n[i]) + s2 * (m[i] - m2 * n2[i]));
            }
            let (mem1, mem2) = (vec3::dot(mem.m[c], n), vec3::dot(mem.m[c], n2));
            res[c][0] -= (s1 + s2) * (u[c][0] - uref[0] - a * q / c2) + s1 * s2 * (q - a * r) / c2;
            for i in 0..3 {
                res[c][1 + i] += force[i] - s1 * (m1 - a * mem1) * n[i] - s2 * (m2 - a * mem2) * n2[i];
            }
            res[c][4] +=
                vec3::dot(prim.u, force) - (s1 + s2) * (u[c][4] - uref[4] - a * q / gm1) - s1 * s2 * (q - a * r) / gm1;
            dot.q[c] = prim.p - sponge.reference.p - a * q;
            dot.r[c] = q - a * r;
            for i in 0..3 {
                dot.m[c][i] = m[i] - a * mem.m[c][i];
            }
        }
    }
    dot
}

/// The transport of the memories of the matched forms for [`SpongeFrame::AttachedWithLabMemory`] (ADR 0344 DR3, DR4): add
/// `w_c . grad` of each memory to its time derivative in `dot`, for the cells of the layer (`rate2` the rates of the second
/// layer, all zero without one). The gradient is the Green-Gauss sum of the matched source on `geom` divided by the cell
/// volume; a cell with `w_c = 0` is not touched. Nothing is done for the plain form.
pub(crate) fn add_lab_transport(
    geom: &FvGeometry,
    sponge: &Sponge,
    rate2: &[f64],
    wc: &[Vec3],
    mem: &SpongeMemories,
    dot: &mut SpongeMemories,
) {
    let rate = &sponge.rate;
    let vol = geom.mesh.cell_volumes();
    let moving = |c: usize| wc[c] != [0.0; 3];
    let add = |inside: &dyn Fn(usize) -> bool, x: &[f64], xd: &mut [f64]| {
        let g = layer_gradient_sum(geom, x, inside);
        for c in 0..x.len() {
            if inside(c) && moving(c) {
                xd[c] += vec3::dot(wc[c], g[c]) / vol[c];
            }
        }
    };
    match sponge.form {
        SpongeForm::Plain => {}
        SpongeForm::Matched { .. } => add(&|c| rate[c] > 0.0, &mem.q, &mut dot.q),
        SpongeForm::MatchedShifted(o) => {
            let inside = |c: usize| {
                if o.second.is_some() {
                    rate[c] + rate2[c] > 0.0
                } else {
                    rate[c] > 0.0
                }
            };
            add(&inside, &mem.q, &mut dot.q);
            add(&inside, &mem.r, &mut dot.r);
            for k in 0..3 {
                let x: Vec<f64> = mem.m.iter().map(|v| v[k]).collect();
                let g = layer_gradient_sum(geom, &x, &inside);
                for c in 0..x.len() {
                    if inside(c) && moving(c) {
                        dot.m[c][k] += vec3::dot(wc[c], g[c]) / vol[c];
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::gen::box_2d;
    use std::sync::Arc;

    const AIR: Gas = Gas { gamma: 1.4 };

    fn geom(nx: usize, ny: usize, lx: f64, ly: f64) -> FvGeometry {
        FvGeometry::new(Arc::new(box_2d(nx, ny, lx, ly, false, false).unwrap())).unwrap()
    }

    /// S1 of ADR 0237: the registered layer (40 cells of 0.005, exponent 2, two-way attenuation 1e-6).
    #[test]
    fn s1_profile_of_the_registered_layer() {
        let (l, c, r0, m) = (0.2, 1.0, 1e-6, 2.0);
        let rate_max = rate_max_for_reflection(l, c, r0, m).unwrap();
        println!("S1 rate_max = {rate_max:.6} (103.6163)");
        assert!((rate_max / 103.6163 - 1.0).abs() <= 1e-6);
        let g = geom(240, 1, 1.2, 0.005);
        let layer = SpongeLayer {
            origin: [1.0, 0.0, 0.0],
            normal: [2.0, 0.0, 0.0],
            thickness: l,
            rate_max,
            exponent: m,
        };
        let rates = layer.rates(&g).unwrap();
        let cc = g.mesh.cell_centres();
        let mut cells: Vec<usize> = (0..cc.len()).collect();
        cells.sort_by(|&a, &b| cc[a][0].total_cmp(&cc[b][0]));
        assert!(cells[..200].iter().all(|&c| rates[c] == 0.0), "rate in front");
        assert!(cells[200..].iter().all(|&c| rates[c] > 0.0));
        assert!(cells[200..].windows(2).all(|w| rates[w[1]] > rates[w[0]]), "monotone");
        let integral: f64 = cells[200..].iter().map(|&c| rates[c] * 0.005).sum();
        let want = 0.5 * c * (1.0 / r0).ln();
        let last = rates[cells[239]] / (rate_max * (39.5f64 / 40.0).powi(2));
        println!(
            "S1 sum rate dx = {integral:.6} against {want:.6} (relative {:.2e}, limit 1e-3); last cell / closed form - 1 = {:.2e} (1e-12)",
            (integral / want - 1.0).abs(),
            (last - 1.0).abs()
        );
        assert!((integral / want - 1.0).abs() <= 1e-3);
        assert!((last - 1.0).abs() <= 1e-12);
        // beyond the thickness the rate stays at its peak
        assert_eq!(layer.rate([5.0, 0.0, 0.0]).unwrap(), rate_max);
    }

    /// S2 of ADR 0237 (the part without a solver): invalid layers, rates and references are refused.
    #[test]
    fn s2_invalid_layers_and_sponges_are_refused() {
        assert!(rate_max_for_reflection(0.0, 1.0, 1e-6, 2.0).is_err());
        assert!(rate_max_for_reflection(0.2, 1.0, 0.0, 2.0).is_err());
        assert!(rate_max_for_reflection(0.2, 1.0, 1.0, 2.0).is_err());
        assert!(rate_max_for_reflection(0.2, 1.0, f64::NAN, 2.0).is_err());
        let g = geom(8, 2, 1.0, 0.25);
        let good = SpongeLayer {
            origin: [0.5, 0.0, 0.0],
            normal: [1.0, 0.0, 0.0],
            thickness: 0.5,
            rate_max: 10.0,
            exponent: 2.0,
        };
        let rest = Prim {
            rho: 1.0,
            u: [0.0; 3],
            p: 1.0,
        };
        assert!(Sponge::planar(&g, rest, &good, true).is_ok());
        for bad in [
            SpongeLayer { thickness: 0.0, ..good },
            SpongeLayer {
                thickness: -1.0,
                ..good
            },
            SpongeLayer {
                normal: [0.0; 3],
                ..good
            },
            SpongeLayer { rate_max: -1.0, ..good },
            SpongeLayer {
                rate_max: f64::NAN,
                ..good
            },
            SpongeLayer { exponent: -1.0, ..good },
        ] {
            assert!(Sponge::planar(&g, rest, &bad, false).is_err(), "{bad:?}");
        }
        let moving = Prim {
            u: [0.1, 0.0, 0.0],
            ..rest
        };
        assert!(Sponge::planar(&g, moving, &good, false).is_ok());
        assert!(
            Sponge::planar(&g, moving, &good, true).is_err(),
            "matched form with a reference velocity"
        );
        for bad in [
            Prim { rho: 0.0, ..rest },
            Prim { p: -1.0, ..rest },
            Prim { rho: f64::NAN, ..rest },
        ] {
            assert!(Sponge::planar(&g, bad, &good, false).is_err());
        }
        let ok = Sponge::planar(&g, rest, &good, false).unwrap();
        let mut short = ok.clone();
        short.rate.pop();
        assert!(short.validate(g.n_cells()).is_err());
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            let mut s = ok.clone();
            s.rate[3] = bad;
            assert!(s.validate(g.n_cells()).is_err(), "rate {bad}");
        }
        let skew = Sponge {
            form: SpongeForm::Matched {
                normal: [0.5, 0.0, 0.0],
            },
            ..ok
        };
        assert!(skew.validate(g.n_cells()).is_err());
    }

    /// F1 of ADR 0237: the matched source on a prescribed field. Uniform grid, rate 3 in every cell, layer normal `x`,
    /// `Q = b y`; in the interior cells the tangential force is `-rate b`, the normal momentum relaxes, the energy takes
    /// the relaxation and the work of the force, and `dQ/dt = p - p_ref`.
    #[test]
    fn f1_matched_source_on_a_prescribed_field() {
        let (nx, ny) = (6, 8);
        let g = geom(nx, ny, 0.6, 0.8);
        let reference = Prim {
            rho: 1.2,
            u: [0.0; 3],
            p: 0.9,
        };
        let (rate, b) = (3.0, 0.7);
        let sponge = Sponge {
            reference,
            rate: vec![rate; g.n_cells()],
            form: SpongeForm::Matched {
                normal: [1.0, 0.0, 0.0],
            },
        };
        sponge.validate(g.n_cells()).unwrap();
        let state = Prim {
            rho: 1.3,
            u: [0.2, -0.15, 0.0],
            p: 1.1,
        };
        let cons = state.to_cons(AIR);
        let uref = reference.to_cons(AIR);
        let u = vec![cons; g.n_cells()];
        let cc = g.mesh.cell_centres();
        let q: Vec<f64> = cc.iter().map(|x| b * x[1]).collect();
        let mut res = vec![[0.0; 5]; g.n_cells()];
        let qdot = add_source(&g, AIR, &sponge, &u, &q, &mut res);
        let force_y = -rate * b;
        let want = [
            -rate * (cons[0] - uref[0]),
            -rate * cons[1],
            force_y,
            0.0,
            -rate * (cons[4] - uref[4]) + state.u[1] * force_y,
        ];
        let mut worst = 0.0f64;
        let mut seen = 0;
        for c in 0..g.n_cells() {
            let interior = cc[c][1] > 0.1 && cc[c][1] < 0.7;
            if !interior {
                continue;
            }
            seen += 1;
            for k in 0..5 {
                let scale = if want[k] != 0.0 { want[k].abs() } else { 1.0 };
                worst = worst.max((res[c][k] - want[k]).abs() / scale);
            }
            let dp = state.p - reference.p;
            worst = worst.max((qdot[c] - dp).abs() / dp.abs());
        }
        println!("F1 {seen} interior cells: largest deviation {worst:.2e} (limit 1e-12)");
        assert_eq!(seen, nx * (ny - 2));
        assert!(worst <= 1e-12, "{worst}");
        // the plain form on the same state: every conserved variable relaxes, no memory
        let plain = Sponge {
            form: SpongeForm::Plain,
            ..sponge
        };
        let mut res = vec![[0.0; 5]; g.n_cells()];
        let qdot = add_source(&g, AIR, &plain, &u, &q, &mut res);
        for c in 0..g.n_cells() {
            for k in 0..5 {
                assert!((res[c][k] + rate * (cons[k] - uref[k])).abs() <= 1e-14);
            }
            assert_eq!(qdot[c], 0.0);
        }
    }
}

/// Unit tests of the shifted matched form (F-340, ADR 0248): F2, F3, F4 and the layer part of S3.
#[cfg(test)]
mod tests_f340 {
    use super::*;
    use rcfd_mesh::gen::{box_2d, box_mesh, Grading};
    use std::sync::Arc;

    const AIR: Gas = Gas { gamma: 1.4 };
    const REFERENCE: Prim = Prim {
        rho: 1.2,
        u: [0.0; 3],
        p: 0.9,
    };

    fn rel(got: f64, want: f64) -> f64 {
        (got - want).abs() / if want != 0.0 { want.abs() } else { 1.0 }
    }

    /// F2 of ADR 0248: the source of DR14 with one layer on a prescribed field (the set-up of F1 of ADR 0237 with a
    /// shift, a damping along the layer and given memories).
    #[test]
    fn f2_shifted_source_with_one_layer_on_a_prescribed_field() {
        let (nx, ny) = (6, 8);
        let g = FvGeometry::new(Arc::new(box_2d(nx, ny, 0.6, 0.8, false, false).unwrap())).unwrap();
        let (rate, b, a, beta) = (3.0, 0.7, 0.25, 0.05);
        let opts = MatchedOptions {
            normal: [1.0, 0.0, 0.0],
            shift: a,
            along: beta,
            second: None,
        };
        let sponge = Sponge {
            reference: REFERENCE,
            rate: vec![rate; g.n_cells()],
            form: SpongeForm::MatchedShifted(opts),
        };
        sponge.validate(g.n_cells()).unwrap();
        let rate2 = sponge.second_rates(&g).unwrap();
        assert!(rate2.iter().all(|r| *r == 0.0));
        let state = Prim {
            rho: 1.3,
            u: [0.2, -0.15, 0.0],
            p: 1.1,
        };
        let cons = state.to_cons(AIR);
        let uref = REFERENCE.to_cons(AIR);
        let u = vec![cons; g.n_cells()];
        let cc = g.mesh.cell_centres();
        let (r_mem, m_mem) = (0.4, [0.3, -0.2, 0.0]);
        let mem = SpongeMemories {
            q: cc.iter().map(|x| b * x[1]).collect(),
            r: vec![r_mem; g.n_cells()],
            m: vec![m_mem; g.n_cells()],
        };
        let mut res = vec![[0.0; 5]; g.n_cells()];
        let dot = add_source_shifted(&g, AIR, &sponge, &opts, &rate2, &u, &mem, &mut res);
        let c2 = AIR.gamma * REFERENCE.p / REFERENCE.rho;
        let force_y = -rate * b - beta * rate * cons[2];
        let (mut worst, mut seen) = (0.0f64, 0);
        for c in 0..g.n_cells() {
            if !(cc[c][1] > 0.1 && cc[c][1] < 0.7) {
                continue;
            }
            seen += 1;
            let q = mem.q[c];
            let want = [
                -rate * (cons[0] - uref[0] - a * q / c2),
                -rate * (cons[1] - a * m_mem[0]),
                force_y,
                0.0,
                -rate * (cons[4] - uref[4] - a * q / 0.4) + state.u[1] * force_y,
            ];
            for k in 0..5 {
                worst = worst.max(rel(res[c][k], want[k]));
            }
            worst = worst.max(rel(dot.q[c], state.p - REFERENCE.p - a * q));
            worst = worst.max(rel(dot.r[c], q - a * r_mem));
            for i in 0..3 {
                worst = worst.max(rel(dot.m[c][i], cons[1 + i] - a * m_mem[i]));
            }
        }
        println!("F2 {seen} interior cells: largest deviation {worst:.2e} (limit 1e-12)");
        assert_eq!(seen, nx * (ny - 2));
        assert!(worst <= 1e-12, "{worst}");
    }

    /// F3 of ADR 0248: the source of DR14 with two layers on a three-dimensional grid (uniform rates 3 and 2, normals
    /// `x` and `y`, `q` and `r` linear in the three coordinates).
    #[test]
    fn f3_shifted_source_with_two_layers_on_a_prescribed_field() {
        let g = FvGeometry::new(Arc::new(
            box_mesh([5, 5, 5], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap(),
        ))
        .unwrap();
        let (s1, s2, a, beta) = (3.0, 2.0, 0.25, 0.05);
        let opts = MatchedOptions {
            normal: [1.0, 0.0, 0.0],
            shift: a,
            along: beta,
            second: Some(SpongeLayer {
                origin: [0.0, -10.0, 0.0],
                normal: [0.0, 3.0, 0.0],
                thickness: 1.0,
                rate_max: s2,
                exponent: 0.0,
            }),
        };
        let sponge = Sponge {
            reference: REFERENCE,
            rate: vec![s1; g.n_cells()],
            form: SpongeForm::MatchedShifted(opts),
        };
        sponge.validate(g.n_cells()).unwrap();
        let rate2 = sponge.second_rates(&g).unwrap();
        assert!(rate2.iter().all(|r| *r == s2));
        let state = Prim {
            rho: 1.3,
            u: [0.2, -0.15, 0.1],
            p: 1.1,
        };
        let cons = state.to_cons(AIR);
        let uref = REFERENCE.to_cons(AIR);
        let u = vec![cons; g.n_cells()];
        let cc = g.mesh.cell_centres();
        let (bq, br, m_mem) = ([0.7, -0.4, 0.5], [-0.3, 0.6, 0.2], [0.3, -0.2, 0.15]);
        let mem = SpongeMemories {
            q: cc.iter().map(|x| 0.1 + vec3::dot(bq, *x)).collect(),
            r: cc.iter().map(|x| -0.2 + vec3::dot(br, *x)).collect(),
            m: vec![m_mem; g.n_cells()],
        };
        let mut res = vec![[0.0; 5]; g.n_cells()];
        let dot = add_source_shifted(&g, AIR, &sponge, &opts, &rate2, &u, &mem, &mut res);
        let c2 = AIR.gamma * REFERENCE.p / REFERENCE.rho;
        let force = [
            -s2 * bq[0] - beta * s2 * cons[1],
            -s1 * bq[1] - beta * s1 * cons[2],
            -((s1 + s2) * bq[2] + s1 * s2 * br[2]) - beta * (s1 + s2) * cons[3],
        ];
        let (mut worst, mut seen) = (0.0f64, 0);
        for c in 0..g.n_cells() {
            if !cc[c].iter().all(|x| *x > 0.2 && *x < 0.8) {
                continue;
            }
            seen += 1;
            let (q, r) = (mem.q[c], mem.r[c]);
            let want = [
                -(s1 + s2) * (cons[0] - uref[0] - a * q / c2) - s1 * s2 * (q - a * r) / c2,
                force[0] - s1 * (cons[1] - a * m_mem[0]),
                force[1] - s2 * (cons[2] - a * m_mem[1]),
                force[2],
                vec3::dot(state.u, force) - (s1 + s2) * (cons[4] - uref[4] - a * q / 0.4) - s1 * s2 * (q - a * r) / 0.4,
            ];
            for k in 0..5 {
                worst = worst.max(rel(res[c][k], want[k]));
            }
            worst = worst.max(rel(dot.q[c], state.p - REFERENCE.p - a * q));
            worst = worst.max(rel(dot.r[c], q - a * r));
        }
        println!("F3 {seen} interior cells: largest deviation {worst:.2e} (limit 1e-12)");
        assert_eq!(seen, 27);
        assert!(worst <= 1e-12, "{worst}");
    }

    /// F4 of ADR 0248 (clause 2 of F-340, the held pressure): with the shift the memory tends to the pressure offset
    /// over `a` and the force along the layer stays bounded; in the form of ADR 0237 the memory grows linearly.
    #[test]
    fn f4_held_pressure_gives_a_bounded_memory_with_the_shift() {
        let g = FvGeometry::new(Arc::new(box_2d(6, 8, 0.6, 0.8, false, false).unwrap())).unwrap();
        let (rate, a, d, grad) = (3.0, 0.25, 1e-3, 0.8);
        let cc = g.mesh.cell_centres();
        let dp: Vec<f64> = cc.iter().map(|x| d * (1.0 + grad * x[1])).collect();
        let u: Vec<Cons> = dp
            .iter()
            .map(|dp| {
                Prim {
                    p: REFERENCE.p + dp,
                    ..REFERENCE
                }
                .to_cons(AIR)
            })
            .collect();
        let opts = MatchedOptions {
            normal: [1.0, 0.0, 0.0],
            shift: a,
            along: 0.0,
            second: None,
        };
        let shifted = Sponge {
            reference: REFERENCE,
            rate: vec![rate; g.n_cells()],
            form: SpongeForm::MatchedShifted(opts),
        };
        let old = Sponge {
            form: SpongeForm::Matched {
                normal: [1.0, 0.0, 0.0],
            },
            ..shifted.clone()
        };
        let rate2 = shifted.second_rates(&g).unwrap();
        let mut mem = SpongeMemories {
            q: vec![0.0; g.n_cells()],
            r: vec![0.0; g.n_cells()],
            m: vec![[0.0; 3]; g.n_cells()],
        };
        let mut q_old = vec![0.0; g.n_cells()];
        let (dt, steps) = (1e-3, 20_000);
        let mut res = vec![[0.0; 5]; g.n_cells()];
        for _ in 0..steps {
            let dot = add_source_shifted(&g, AIR, &shifted, &opts, &rate2, &u, &mem, &mut res);
            let qd = add_source(&g, AIR, &old, &u, &q_old, &mut res);
            for c in 0..g.n_cells() {
                mem.q[c] += dt * dot.q[c];
                q_old[c] += dt * qd[c];
            }
        }
        let t = dt * steps as f64;
        let level = 1.0 - (-a * t).exp();
        let mut res = vec![[0.0; 5]; g.n_cells()];
        add_source_shifted(&g, AIR, &shifted, &opts, &rate2, &u, &mem, &mut res);
        let (mut worst_q, mut worst_f, mut worst_old) = (0.0f64, 0.0f64, 0.0f64);
        for c in 0..g.n_cells() {
            worst_q = worst_q.max((mem.q[c] * a / dp[c] / level - 1.0).abs());
            worst_old = worst_old.max((q_old[c] / (dp[c] * t) - 1.0).abs());
            if cc[c][1] > 0.1 && cc[c][1] < 0.7 {
                worst_f = worst_f.max((res[c][2] / (-(rate / a) * d * grad * level) - 1.0).abs());
            }
        }
        println!(
            "F4 t = {t}: shifted memory against its closed form {worst_q:.2e}, force along the layer against its bound {worst_f:.2e} (limit 1e-3 each); form of ADR 0237: memory against offset times t {worst_old:.2e} (1e-9)"
        );
        assert!(worst_q <= 1e-3, "{worst_q}");
        assert!(worst_f <= 1e-3, "{worst_f}");
        assert!(worst_old <= 1e-9, "{worst_old}");
    }

    /// S3 of ADR 0248 (the part without a solver): invalid options of the shifted form are refused.
    #[test]
    fn s3_invalid_options_of_the_shifted_form_are_refused() {
        let g = FvGeometry::new(Arc::new(box_2d(8, 4, 1.0, 0.5, false, false).unwrap())).unwrap();
        let first = SpongeLayer {
            origin: [0.5, 0.0, 0.0],
            normal: [1.0, 0.0, 0.0],
            thickness: 0.5,
            rate_max: 10.0,
            exponent: 2.0,
        };
        let second = SpongeLayer {
            origin: [0.0, 0.25, 0.0],
            normal: [0.0, 2.0, 0.0],
            ..first
        };
        assert!(Sponge::planar_shifted(&g, REFERENCE, &first, 0.25, 0.0).is_ok());
        assert!(Sponge::planar_shifted(&g, REFERENCE, &first, 0.0, 0.05).is_ok());
        let corner = Sponge::corner(&g, REFERENCE, &first, &second, 0.25, 0.0).unwrap();
        let rate2 = corner.second_rates(&g).unwrap();
        assert_eq!(rate2.iter().filter(|r| **r > 0.0).count(), 16);
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(
                Sponge::planar_shifted(&g, REFERENCE, &first, bad, 0.0).is_err(),
                "shift {bad}"
            );
            assert!(
                Sponge::planar_shifted(&g, REFERENCE, &first, 0.25, bad).is_err(),
                "along {bad}"
            );
        }
        let moving = Prim {
            u: [0.1, 0.0, 0.0],
            ..REFERENCE
        };
        assert!(Sponge::planar_shifted(&g, moving, &first, 0.25, 0.0).is_err());
        for bad in [
            SpongeLayer {
                normal: [0.0; 3],
                ..second
            },
            SpongeLayer {
                thickness: 0.0,
                ..second
            },
            SpongeLayer {
                normal: [1.0, 1.0, 0.0],
                ..second
            },
            SpongeLayer {
                normal: [1.0, 0.0, 0.0],
                ..second
            },
        ] {
            assert!(
                Sponge::corner(&g, REFERENCE, &first, &bad, 0.25, 0.0).is_err(),
                "{bad:?}"
            );
        }
        let mut skew = corner.clone();
        if let SpongeForm::MatchedShifted(o) = &mut skew.form {
            o.normal = [0.5, 0.0, 0.0];
        }
        assert!(skew.validate(g.n_cells()).is_err());
    }
}
