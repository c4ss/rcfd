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
    }
    qdot
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
