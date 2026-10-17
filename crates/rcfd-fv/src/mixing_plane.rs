//! Mixing-plane (stage) interface: circumferential averaging of flux-conserving profiles in radial bands (F-126, ADR 0055).
//!
//! Constant density, kinematic pressure `p/rho`. Bands are annuli `[r_k, r_{k+1})` about the rotation axis. The donor side
//! (faces with area vectors `S_f` oriented in the flow direction, centres `x_f`, ABSOLUTE velocity `u_f`, kinematic
//! pressure `p_f`, one scalar `h_f`) is reduced to one state per band; [`MixingPlane::pass`] expands the band states onto the
//! faces of the receiving side. With `phi_f = u_f . S_f` and `n_b` the unit sum of the band's area vectors the conserved
//! quantities of a band are
//!
//! * volume flux `Q = sum phi_f` (mass),
//! * angular momentum flux `Gamma Q` with `Gamma = sum phi_f r_f u_theta,f / Q`,
//! * normal momentum flux plus pressure force `F = sum (phi_f u_n,f + p_f S_f . n_b)`,
//! * the flux of the scalar `H Q`, `H = sum phi_f h_f / Q` (total enthalpy of an adiabatic steady mixing),
//! * the radial velocity is passed as the flux-weighted mean (it is not a conserved invariant of the model).
//!
//! The receiving face `g` of band `b` gets `u_n = Q / A'_b` (`A'_b = sum S'_g . n_b` of the RECEIVER, so mass is conserved to
//! round-off for any receiving tiling), `u_theta = Gamma / r_g` (angular momentum flux conserved exactly; a free vortex is
//! reproduced exactly), `u_r`, `h = H`, and the mixed-out pressure `p = (F - Q u_n)/A'_b`. The mechanical energy flux
//! `sum phi (p + |u|^2/2)` is not conserved (mixing is dissipative); [`mechanical_energy_flux`] gives both sides so the loss is
//! `E_donor - E_receiver`, which is non-negative for uniform pressure and non-negative normal velocity (ADR 0055 D3).
//! A band whose flux cancels (`|Q| <= 1e-9 sum |phi|`) is a located error: the flux weighting is singular and there is no
//! silent fallback. Not implemented: compressible mixed-out state, frame-dependent rothalpy constraints, a coupling to the
//! segregated solver.

use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};

fn bad(path: &str, reason: impl Into<String>) -> Error {
    Error::InvalidValue {
        path: path.to_string(),
        reason: reason.into(),
    }
}

/// A face of the donor side.
#[derive(Debug, Clone, Copy)]
pub struct DonorFace {
    /// Area vector (oriented in the flow direction).
    pub s: Vec3,
    /// Face centre.
    pub x: Vec3,
    /// Absolute velocity.
    pub u: Vec3,
    /// Kinematic static pressure `p/rho`.
    pub p: f64,
    /// Scalar to be flux-averaged (e.g. total enthalpy).
    pub h: f64,
}

/// A face of the receiving side (area vector oriented in the flow direction).
#[derive(Debug, Clone, Copy)]
pub struct ReceiverFace {
    /// Area vector.
    pub s: Vec3,
    /// Face centre.
    pub x: Vec3,
}

/// Reduced state of one radial band.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandState {
    /// Band index.
    pub band: usize,
    /// Number of donor faces.
    pub n_faces: usize,
    /// Volume flux `Q`.
    pub q: f64,
    /// Donor area `sum S . n`.
    pub area: f64,
    /// Unit band normal.
    pub normal: Vec3,
    /// Specific angular momentum `Gamma = <r u_theta>_flux`.
    pub gamma: f64,
    /// Flux-weighted radial velocity.
    pub u_r: f64,
    /// Flux-weighted scalar.
    pub h: f64,
    /// Normal momentum flux plus pressure force `F`.
    pub momentum: f64,
    /// Area-weighted mean pressure of the donor faces.
    pub p_mean: f64,
}

/// State passed to a receiving face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassedState {
    /// Band of the face.
    pub band: usize,
    /// Absolute velocity vector.
    pub u: Vec3,
    /// Mixed-out kinematic pressure.
    pub p: f64,
    /// Scalar.
    pub h: f64,
}

/// Radial bands about an axis.
#[derive(Debug, Clone)]
pub struct MixingPlane {
    origin: Vec3,
    axis: Vec3,
    edges: Vec<f64>,
}

impl MixingPlane {
    /// Bands with the given strictly increasing, non-negative edges (at least two). Zero axis, non-finite or non-monotone edges
    /// are located errors.
    pub fn new(origin: Vec3, axis: Vec3, edges: Vec<f64>) -> Result<MixingPlane> {
        let n = vec3::norm(axis);
        if !n.is_finite() || n < 1e-300 || origin.iter().any(|v| !v.is_finite()) {
            return Err(bad(
                "mixing_plane.axis",
                "axis must be a finite non-zero vector and origin finite",
            ));
        }
        if edges.len() < 2 {
            return Err(bad(
                "mixing_plane.edges",
                "at least two band edges (one band) are needed",
            ));
        }
        for (k, w) in edges.windows(2).enumerate() {
            if !(w[0].is_finite() && w[1].is_finite() && w[0] >= 0.0 && w[1] > w[0]) {
                return Err(bad(
                    &format!("mixing_plane.edges[{}]", k + 1),
                    format!(
                        "band edges must be finite, non-negative and strictly increasing ({} then {})",
                        w[0], w[1]
                    ),
                ));
            }
        }
        Ok(MixingPlane {
            origin,
            axis: vec3::scale(axis, 1.0 / n),
            edges,
        })
    }

    /// `n` equal-width bands between `r_min` and `r_max`.
    pub fn uniform(origin: Vec3, axis: Vec3, r_min: f64, r_max: f64, n: usize) -> Result<MixingPlane> {
        if n == 0 {
            return Err(bad("mixing_plane.edges", "the number of bands must be at least 1"));
        }
        let edges = (0..=n)
            .map(|k| {
                if k == n {
                    r_max
                } else {
                    r_min + (r_max - r_min) * k as f64 / n as f64
                }
            })
            .collect();
        MixingPlane::new(origin, axis, edges)
    }

    /// Number of bands.
    pub fn n_bands(&self) -> usize {
        self.edges.len() - 1
    }

    /// Band edges.
    pub fn edges(&self) -> &[f64] {
        &self.edges
    }

    /// Radius and tangential/radial unit vectors `(r, r_hat, theta_hat)` of a point.
    pub fn polar(&self, x: Vec3) -> (f64, Vec3, Vec3) {
        let d = vec3::sub(x, self.origin);
        let rv = vec3::sub(d, vec3::scale(self.axis, vec3::dot(d, self.axis)));
        let r = vec3::norm(rv);
        if r < 1e-300 {
            return (0.0, [0.0; 3], [0.0; 3]);
        }
        let rh = vec3::scale(rv, 1.0 / r);
        (r, rh, vec3::cross(self.axis, rh))
    }

    /// Band of a point (lower edge inclusive, upper exclusive, the last band includes its upper edge); `None` outside.
    pub fn band_of(&self, x: Vec3) -> Option<usize> {
        let r = self.polar(x).0;
        let nb = self.n_bands();
        if r < self.edges[0] || r > self.edges[nb] {
            return None;
        }
        if r == self.edges[nb] {
            return Some(nb - 1);
        }
        // largest k with edges[k] <= r
        let k = self.edges.partition_point(|&e| e <= r);
        Some(k - 1)
    }

    fn check_vec(path: &str, v: Vec3) -> Result<()> {
        if v.iter().any(|c| !c.is_finite()) {
            return Err(bad(path, "non-finite component"));
        }
        Ok(())
    }

    /// Area-weighted band mean of a face field `f` (the circumferential average on the donor faces of each band).
    pub fn area_mean(&self, faces: &[(Vec3, Vec3)], f: &[f64]) -> Result<Vec<f64>> {
        if faces.len() != f.len() {
            return Err(bad(
                "mixing_plane.field",
                format!("{} faces but {} field values", faces.len(), f.len()),
            ));
        }
        let nb = self.n_bands();
        let (mut num, mut den) = (vec![0.0; nb], vec![0.0; nb]);
        for (i, ((s, x), v)) in faces.iter().zip(f).enumerate() {
            let b = self.band_of(*x).ok_or_else(|| {
                bad(
                    &format!("mixing_plane.face[{i}]"),
                    "the face centre lies outside all bands",
                )
            })?;
            if !v.is_finite() {
                return Err(bad(&format!("mixing_plane.face[{i}]"), "non-finite field value"));
            }
            let a = vec3::norm(*s);
            num[b] += a * v;
            den[b] += a;
        }
        for (b, d) in den.iter().enumerate() {
            if *d <= 0.0 {
                return Err(bad(&format!("mixing_plane.band[{b}]"), "no face area in the band"));
            }
        }
        Ok(num.iter().zip(&den).map(|(n, d)| n / d).collect())
    }

    /// Reduce the donor faces to one state per band.
    pub fn average(&self, donor: &[DonorFace]) -> Result<Vec<BandState>> {
        let nb = self.n_bands();
        let mut bands: Vec<Vec<usize>> = vec![vec![]; nb];
        for (i, d) in donor.iter().enumerate() {
            let path = format!("mixing_plane.donor[{i}]");
            Self::check_vec(&path, d.s)?;
            Self::check_vec(&path, d.x)?;
            Self::check_vec(&path, d.u)?;
            if !d.p.is_finite() || !d.h.is_finite() {
                return Err(bad(&path, "non-finite pressure or scalar"));
            }
            let b = self
                .band_of(d.x)
                .ok_or_else(|| bad(&path, "the face centre lies outside all bands"))?;
            bands[b].push(i);
        }
        let mut out = Vec::with_capacity(nb);
        for (b, idx) in bands.iter().enumerate() {
            if idx.is_empty() {
                return Err(bad(&format!("mixing_plane.band[{b}]"), "no donor face in the band"));
            }
            let ssum = idx.iter().fold([0.0; 3], |a, &i| vec3::add(a, donor[i].s));
            let sn = vec3::norm(ssum);
            if sn <= 0.0 {
                return Err(bad(
                    &format!("mixing_plane.band[{b}]"),
                    "the donor area vectors of the band cancel",
                ));
            }
            let n = vec3::scale(ssum, 1.0 / sn);
            let (mut q, mut qabs, mut area, mut mom, mut g, mut ur, mut h, mut pa) =
                (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
            for &i in idx {
                let d = &donor[i];
                let phi = vec3::dot(d.u, d.s);
                let (r, rh, th) = self.polar(d.x);
                let sa = vec3::dot(d.s, n);
                q += phi;
                qabs += phi.abs();
                area += sa;
                pa += d.p * sa;
                mom += phi * vec3::dot(d.u, n) + d.p * sa;
                g += phi * r * vec3::dot(d.u, th);
                ur += phi * vec3::dot(d.u, rh);
                h += phi * d.h;
            }
            if q.abs() <= 1e-9 * qabs || qabs == 0.0 {
                return Err(bad(
                    &format!("mixing_plane.band[{b}]"),
                    format!("the flux through the band cancels (net {q:.3e}, absolute {qabs:.3e}); flux-weighted averaging is singular"),
                ));
            }
            out.push(BandState {
                band: b,
                n_faces: idx.len(),
                q,
                area,
                normal: n,
                gamma: g / q,
                u_r: ur / q,
                h: h / q,
                momentum: mom,
                p_mean: pa / area,
            });
        }
        Ok(out)
    }

    /// Expand the band states onto the receiving faces.
    pub fn pass(&self, bands: &[BandState], recv: &[ReceiverFace]) -> Result<Vec<PassedState>> {
        if bands.len() != self.n_bands() {
            return Err(bad(
                "mixing_plane.bands",
                format!("{} band states for {} bands", bands.len(), self.n_bands()),
            ));
        }
        let mut idx = Vec::with_capacity(recv.len());
        let mut area = vec![0.0; bands.len()];
        for (g, f) in recv.iter().enumerate() {
            let path = format!("mixing_plane.receiver[{g}]");
            Self::check_vec(&path, f.s)?;
            Self::check_vec(&path, f.x)?;
            let b = self
                .band_of(f.x)
                .ok_or_else(|| bad(&path, "the face centre lies outside all bands"))?;
            area[b] += vec3::dot(f.s, bands[b].normal);
            idx.push(b);
        }
        for (b, a) in area.iter().enumerate() {
            if *a <= 0.0 {
                return Err(bad(
                    &format!("mixing_plane.band[{b}]"),
                    "the receiving faces of the band have no area along the flow direction (empty band or reversed orientation)",
                ));
            }
        }
        let mut out = Vec::with_capacity(recv.len());
        for (g, f) in recv.iter().enumerate() {
            let b = idx[g];
            let st = &bands[b];
            let (r, rh, th) = self.polar(f.x);
            if r <= 0.0 {
                return Err(bad(
                    &format!("mixing_plane.receiver[{g}]"),
                    "the face centre lies on the axis",
                ));
            }
            let un = st.q / area[b];
            let u = vec3::add(
                vec3::add(vec3::scale(st.normal, un), vec3::scale(th, st.gamma / r)),
                vec3::scale(rh, st.u_r),
            );
            out.push(PassedState {
                band: b,
                u,
                p: (st.momentum - st.q * un) / area[b],
                h: st.h,
            });
        }
        Ok(out)
    }

    /// Per-band receiver area `sum S . n_b` along the band normals of `bands` (the `A'_b` of [`MixingPlane::pass`]). A uniform
    /// flow passes unchanged only if this equals the donor `BandState::area`; the passed normal velocity is scaled by the ratio.
    pub fn band_areas(&self, bands: &[BandState], recv: &[ReceiverFace]) -> Result<Vec<f64>> {
        if bands.len() != self.n_bands() {
            return Err(bad(
                "mixing_plane.bands",
                format!("{} band states for {} bands", bands.len(), self.n_bands()),
            ));
        }
        let mut area = vec![0.0; bands.len()];
        for (g, f) in recv.iter().enumerate() {
            let b = self.band_of(f.x).ok_or_else(|| {
                bad(
                    &format!("mixing_plane.receiver[{g}]"),
                    "the face centre lies outside all bands",
                )
            })?;
            area[b] += vec3::dot(f.s, bands[b].normal);
        }
        Ok(area)
    }

    /// Per-band flux `sum u . S` of the passed state on the receiving faces.
    pub fn band_flux(&self, recv: &[ReceiverFace], state: &[PassedState]) -> Vec<f64> {
        let mut q = vec![0.0; self.n_bands()];
        for (f, s) in recv.iter().zip(state) {
            q[s.band] += vec3::dot(s.u, f.s);
        }
        q
    }

    /// Per-band angular momentum flux `sum phi r u_theta` of the passed state.
    pub fn band_angular_momentum(&self, recv: &[ReceiverFace], state: &[PassedState]) -> Vec<f64> {
        let mut l = vec![0.0; self.n_bands()];
        for (f, s) in recv.iter().zip(state) {
            let (r, _, th) = self.polar(f.x);
            l[s.band] += vec3::dot(s.u, f.s) * r * vec3::dot(s.u, th);
        }
        l
    }
}

/// Mechanical energy flux `sum phi (p + |u|^2/2)` of faces with area vectors, velocities and kinematic pressures.
pub fn mechanical_energy_flux(s: &[Vec3], u: &[Vec3], p: &[f64]) -> f64 {
    s.iter()
        .zip(u)
        .zip(p)
        .map(|((s, u), p)| vec3::dot(*u, *s) * (p + 0.5 * vec3::dot(*u, *u)))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::gen::{annulus_3d, Grading};
    use std::f64::consts::PI;

    const GL_X: [f64; 3] = [-0.774_596_669_241_483_4, 0.0, 0.774_596_669_241_483_4];
    const GL_W: [f64; 3] = [5.0 / 9.0, 8.0 / 9.0, 5.0 / 9.0];
    const AX: Vec3 = [0.0, 0.0, 1.0];

    /// Gauss-Legendre samples of the annulus plane: `n_cells` radial cells over [0.5, 1.0], `nt` sectors; each sample
    /// carries `S = (0,0,w r dr dtheta)` at `(r cos t, r sin t, 0)`.
    fn gauss_faces(n_cells: usize, nt: usize, f: impl Fn(f64, f64) -> DonorFace) -> Vec<DonorFace> {
        let mut out = vec![];
        let dr = 0.5 / n_cells as f64;
        for c in 0..n_cells {
            let (ra, rb) = (0.5 + c as f64 * dr, 0.5 + (c + 1) as f64 * dr);
            for (xi, wi) in GL_X.iter().zip(GL_W) {
                let r = 0.5 * (ra + rb) + 0.5 * (rb - ra) * xi;
                for j in 0..nt {
                    let t = 2.0 * PI * (j as f64 + 0.5) / nt as f64;
                    let a = wi * 0.5 * (rb - ra) * r * 2.0 * PI / nt as f64;
                    let mut d = f(r, t);
                    d.s = [0.0, 0.0, a];
                    d.x = [r * t.cos(), r * t.sin(), 0.0];
                    out.push(d);
                }
            }
        }
        out
    }

    fn blank() -> DonorFace {
        DonorFace {
            s: [0.0; 3],
            x: [0.0; 3],
            u: [0.0, 0.0, 1.0],
            p: 0.0,
            h: 0.0,
        }
    }

    /// Patch faces `(area vector oriented +z, centre)` of `zmax` (donor) or `zmin` (receiver, flipped) of an annulus_3d.
    fn patch(n_theta: usize, n_r: usize, name: &str) -> Vec<(Vec3, Vec3)> {
        let m = annulus_3d(n_theta, n_r, 2, 0.5, 1.0, 0.4, 0.0, Grading::Uniform).unwrap();
        let p = m.patch(name).unwrap();
        let sign = if name == "zmin" { -1.0 } else { 1.0 };
        p.faces()
            .map(|f| (vec3::scale(m.face_area_vectors()[f], sign), m.face_centres()[f]))
            .collect()
    }

    fn donor_from(faces: &[(Vec3, Vec3)], u: impl Fn(Vec3) -> Vec3, p: f64, h: impl Fn(Vec3) -> f64) -> Vec<DonorFace> {
        faces
            .iter()
            .map(|&(s, x)| DonorFace {
                s,
                x,
                u: u(x),
                p,
                h: h(x),
            })
            .collect()
    }

    fn receiver_from(faces: &[(Vec3, Vec3)]) -> Vec<ReceiverFace> {
        faces.iter().map(|&(s, x)| ReceiverFace { s, x }).collect()
    }

    fn rel(a: f64, b: f64) -> f64 {
        (a - b).abs() / b.abs().max(1e-300)
    }

    #[test]
    fn x1_circumferential_and_band_average_equal_the_closed_form() {
        let mp = MixingPlane::uniform([0.0; 3], AX, 0.5, 1.0, 5).unwrap();
        let faces = gauss_faces(10, 16, |r, t| {
            let mut d = blank();
            d.p = 1.0 + 2.0 * r + 3.0 * r * r + 0.5 * (3.0 * t).cos() + 0.2 * t.sin();
            d
        });
        let geo: Vec<(Vec3, Vec3)> = faces.iter().map(|d| (d.s, d.x)).collect();
        let f: Vec<f64> = faces.iter().map(|d| d.p).collect();
        let mean = mp.area_mean(&geo, &f).unwrap();
        let prim = |r: f64| 0.5 * r * r + 2.0 * r.powi(3) / 3.0 + 0.75 * r.powi(4);
        let mut worst: f64 = 0.0;
        for b in 0..5 {
            let (r1, r2) = (0.5 + 0.1 * b as f64, 0.6 + 0.1 * b as f64);
            let exact = (prim(r2) - prim(r1)) / (0.5 * (r2 * r2 - r1 * r1));
            worst = worst.max(rel(mean[b], exact));
        }
        println!("X1 band average closed form: worst relative error {worst:.2e} (limit 1e-13)");
        assert!(worst <= 1e-13);
        let mut worst_k: f64 = 0.0;
        for k in 1..16 {
            let f: Vec<f64> = faces
                .iter()
                .map(|d| {
                    let t = d.x[1].atan2(d.x[0]);
                    (k as f64 * t).cos() + 0.3 * (k as f64 * t).sin()
                })
                .collect();
            for m in mp.area_mean(&geo, &f).unwrap() {
                worst_k = worst_k.max(m.abs());
            }
        }
        println!("X1 circumferential mean of harmonics k = 1..15: {worst_k:.2e} (limit 1e-14)");
        assert!(worst_k <= 1e-14);
    }

    type FaceList = Vec<(Vec3, Vec3)>;

    fn mesh_pair() -> (FaceList, FaceList) {
        (patch(48, 16, "zmax"), patch(29, 11, "zmin"))
    }

    #[test]
    fn x2_mass_conservation_on_a_mismatched_receiver() {
        let (d, r) = mesh_pair();
        let mp = MixingPlane::uniform([0.0; 3], AX, 0.5, 1.0, 4).unwrap();
        let donor = donor_from(
            &d,
            |x| {
                let (rr, t) = (x[0].hypot(x[1]), x[1].atan2(x[0]));
                [0.0, 0.0, 1.0 + 0.5 * (2.0 * t).sin() + 0.3 * rr]
            },
            0.0,
            |_| 0.0,
        );
        let bands = mp.average(&donor).unwrap();
        let recv = receiver_from(&r);
        let st = mp.pass(&bands, &recv).unwrap();
        let q = mp.band_flux(&recv, &st);
        let mut worst: f64 = 0.0;
        for b in 0..4 {
            worst = worst.max(rel(q[b], bands[b].q));
        }
        let tot = rel(q.iter().sum(), bands.iter().map(|b| b.q).sum());
        println!("X2 per-band flux mismatch {worst:.2e}, total {tot:.2e} (limit 1e-13)");
        assert!(worst <= 1e-13 && tot <= 1e-13);
    }

    /// Registered X3 (ADR 0055) as written: it MISSES because the donor and receiver band areas of the mesh pair differ
    /// (addendum 2); kept with its limit intact.
    #[test]
    #[ignore]
    fn x3_uniform_axial_flow_passes_unchanged_known_miss() {
        let (d, r) = mesh_pair();
        let mp = MixingPlane::uniform([0.0; 3], AX, 0.5, 1.0, 4).unwrap();
        let donor = donor_from(&d, |_| [0.0, 0.0, 2.5], 7.0, |_| 3.0);
        let bands = mp.average(&donor).unwrap();
        let st = mp.pass(&bands, &receiver_from(&r)).unwrap();
        let mut worst: f64 = 0.0;
        for s in &st {
            worst = worst
                .max(rel(s.u[2], 2.5))
                .max(s.u[0].abs())
                .max(s.u[1].abs())
                .max(rel(s.p, 7.0))
                .max(rel(s.h, 3.0));
        }
        println!("X3 uniform flow worst deviation {worst:.2e} (limit 1e-13)");
        assert!(worst <= 1e-13);
    }

    fn gauss_receiver(n_cells: usize, nt: usize) -> Vec<ReceiverFace> {
        gauss_faces(n_cells, nt, |_, _| blank())
            .iter()
            .map(|d| ReceiverFace { s: d.s, x: d.x })
            .collect()
    }

    #[test]
    fn x3b_uniform_axial_flow_passes_unchanged_with_equal_band_areas() {
        let mp = MixingPlane::uniform([0.0; 3], AX, 0.5, 1.0, 5).unwrap();
        let donor = gauss_faces(10, 16, |_, _| {
            let mut d = blank();
            d.u = [0.0, 0.0, 2.5];
            d.p = 7.0;
            d.h = 3.0;
            d
        });
        let bands = mp.average(&donor).unwrap();
        let recv = gauss_receiver(15, 23);
        let st = mp.pass(&bands, &recv).unwrap();
        let mut worst: f64 = 0.0;
        for s in &st {
            worst = worst
                .max(rel(s.u[2], 2.5))
                .max(s.u[0].abs())
                .max(s.u[1].abs())
                .max(rel(s.p, 7.0))
                .max(rel(s.h, 3.0));
        }
        println!("X3b uniform flow, equal band areas: worst deviation {worst:.2e} (limit 1e-13)");
        assert!(worst <= 1e-13);
    }

    #[test]
    fn x3c_mesh_pair_scales_the_normal_velocity_by_the_band_area_ratio() {
        let (d, r) = mesh_pair();
        let mp = MixingPlane::uniform([0.0; 3], AX, 0.5, 1.0, 4).unwrap();
        let donor = donor_from(&d, |_| [0.0, 0.0, 2.5], 7.0, |_| 3.0);
        let bands = mp.average(&donor).unwrap();
        let recv = receiver_from(&r);
        let st = mp.pass(&bands, &recv).unwrap();
        let areas = mp.band_areas(&bands, &recv).unwrap();
        let (mut worst, mut mism): (f64, f64) = (0.0, 0.0);
        for s in &st {
            let b = s.band;
            worst = worst
                .max(rel(s.u[2], 2.5 * bands[b].area / areas[b]))
                .max(s.u[0].abs())
                .max(s.u[1].abs())
                .max(rel(s.h, 3.0));
        }
        for b in 0..4 {
            mism = mism.max((areas[b] / bands[b].area - 1.0).abs());
        }
        println!("X3c identity deviation {worst:.2e} (limit 1e-13); largest band area mismatch of the pair {mism:.3e} (reported)");
        assert!(worst <= 1e-13);
    }

    #[test]
    fn x4_swirl_angular_momentum_conserved() {
        let (omega_s, w, g0) = (3.0, 2.0, 0.7);
        let mp = MixingPlane::uniform([0.0; 3], AX, 0.5, 1.0, 10).unwrap();
        let dr = 0.05;
        // solid body, Gauss donor samples
        let solid = gauss_faces(10, 16, |r, t| {
            let mut d = blank();
            d.u = [-omega_s * r * t.sin(), omega_s * r * t.cos(), w];
            d
        });
        let bands = mp.average(&solid).unwrap();
        let mut worst_closed: f64 = 0.0;
        for (b, st) in bands.iter().enumerate() {
            let (r1, r2): (f64, f64) = (0.5 + dr * b as f64, 0.5 + dr * (b + 1) as f64);
            let exact = w * omega_s * 2.0 * PI * (r2.powi(4) - r1.powi(4)) / 4.0;
            worst_closed = worst_closed.max(rel(st.gamma * st.q, exact));
        }
        println!("X4 donor angular momentum flux vs closed form: {worst_closed:.2e} (limit 1e-12)");
        assert!(worst_closed <= 1e-12);
        // mismatched mesh receiver: conservation per band
        let (_, r) = mesh_pair();
        let recv = receiver_from(&r);
        let st = mp.pass(&bands, &recv).unwrap();
        let l = mp.band_angular_momentum(&recv, &st);
        let mut worst_l: f64 = 0.0;
        for b in 0..10 {
            worst_l = worst_l.max(rel(l[b], bands[b].gamma * bands[b].q));
        }
        let tot = rel(l.iter().sum(), bands.iter().map(|b| b.gamma * b.q).sum());
        println!("X4 angular momentum flux across the mismatched receiver: per band {worst_l:.2e}, total {tot:.2e} (limit 1e-12)");
        assert!(worst_l <= 1e-12 && tot <= 1e-12);
        // band-resolution error of the passed solid-body profile
        let mut worst_prof: f64 = 0.0;
        for (f, s) in recv.iter().zip(&st) {
            let (rr, _, th) = mp.polar(f.x);
            worst_prof = worst_prof.max((vec3::dot(s.u, th) - omega_s * rr).abs());
        }
        let bound = omega_s * dr * (1.0 + dr / (2.0 * 0.5));
        println!("X4 solid-body profile error {worst_prof:.4e} (limit {bound:.4e})");
        assert!(worst_prof <= bound);
        // free vortex reproduced exactly
        let vortex = gauss_faces(10, 16, |r, t| {
            let mut d = blank();
            d.u = [-g0 / r * t.sin(), g0 / r * t.cos(), w];
            d
        });
        let bv = mp.average(&vortex).unwrap();
        let sv = mp.pass(&bv, &recv).unwrap();
        let mut worst_v: f64 = 0.0;
        for (f, s) in recv.iter().zip(&sv) {
            let (rr, _, th) = mp.polar(f.x);
            worst_v = worst_v.max((vec3::dot(s.u, th) - g0 / rr).abs());
        }
        println!("X4 free vortex reproduced: {worst_v:.2e} (limit 1e-12)");
        assert!(worst_v <= 1e-12);
    }

    #[test]
    fn x5_two_jets_mixed_out_pressure_momentum_and_loss() {
        let (alpha, u1, u2, p0) = (0.3, 2.0, 1.0, 4.0);
        let mp = MixingPlane::new([0.0; 3], AX, vec![0.0, 2.0]).unwrap();
        let area = 1.0;
        let mk = |a: f64, u: f64| DonorFace {
            s: [0.0, 0.0, a],
            x: [1.0, 0.0, 0.0],
            u: [0.0, 0.0, u],
            p: p0,
            h: 0.0,
        };
        let donor = vec![mk(alpha * area, u1), mk((1.0 - alpha) * area, u2)];
        let bands = mp.average(&donor).unwrap();
        let recv = vec![ReceiverFace {
            s: [0.0, 0.0, area],
            x: [1.0, 0.0, 0.0],
        }];
        // the receiver face is on r = 1; use a single receiving face of the full area
        let st = mp.pass(&bands, &recv).unwrap();
        let u_mean = alpha * u1 + (1.0 - alpha) * u2;
        let dp = alpha * (1.0 - alpha) * (u1 - u2).powi(2);
        let (e1, e2) = (rel(st[0].u[2], u_mean), rel(st[0].p, p0 + dp));
        // normal momentum before / after
        let f_before: f64 = donor.iter().map(|d| d.u[2] * d.u[2] * d.s[2] + d.p * d.s[2]).sum();
        let f_after = st[0].u[2] * st[0].u[2] * area + st[0].p * area;
        let e3 = rel(f_after, f_before);
        let es = mechanical_energy_flux(
            &donor.iter().map(|d| d.s).collect::<Vec<_>>(),
            &donor.iter().map(|d| d.u).collect::<Vec<_>>(),
            &donor.iter().map(|d| d.p).collect::<Vec<_>>(),
        );
        let er = mechanical_energy_flux(&[recv[0].s], &[st[0].u], &[st[0].p]);
        let loss = es - er;
        let exact = 0.5 * area * alpha * (1.0 - alpha) * (u1 - u2).powi(2) * (u_mean + (1.0 - 2.0 * alpha) * (u1 - u2));
        println!(
            "X5 u {e1:.2e}, p {e2:.2e} (1e-13), momentum {e3:.2e} (1e-13), loss {loss:.12} vs {exact:.12} ({:.2e}, 1e-12)",
            rel(loss, exact)
        );
        assert!(e1 <= 1e-13 && e2 <= 1e-13 && e3 <= 1e-13);
        assert!(rel(loss, exact) <= 1e-12 && loss >= 0.0);
        // uniform flow: no loss
        let uni = vec![mk(0.4, 1.7), mk(0.6, 1.7)];
        let bu = mp.average(&uni).unwrap();
        let su = mp.pass(&bu, &recv).unwrap();
        let eu = mechanical_energy_flux(
            &uni.iter().map(|d| d.s).collect::<Vec<_>>(),
            &uni.iter().map(|d| d.u).collect::<Vec<_>>(),
            &uni.iter().map(|d| d.p).collect::<Vec<_>>(),
        );
        let eur = mechanical_energy_flux(&[recv[0].s], &[su[0].u], &[su[0].p]);
        assert!((eu - eur).abs() <= 1e-14 * eu.abs(), "{}", eu - eur);
    }

    #[test]
    fn x6_scalar_flux_conserved_and_band_mean_closed_form() {
        let mp = MixingPlane::uniform([0.0; 3], AX, 0.5, 1.0, 5).unwrap();
        let (h0, h1) = (2.0, 3.0);
        // closed form: uniform axial flow, flux-weighted mean of h = h0 + h1 r over a band
        let g = gauss_faces(10, 16, |r, _| {
            let mut d = blank();
            d.h = h0 + h1 * r;
            d
        });
        let bg = mp.average(&g).unwrap();
        let mut worst_c: f64 = 0.0;
        for (b, st) in bg.iter().enumerate() {
            let (r1, r2): (f64, f64) = (0.5 + 0.1 * b as f64, 0.6 + 0.1 * b as f64);
            let exact = h0 + h1 * (2.0 / 3.0) * (r2.powi(3) - r1.powi(3)) / (r2 * r2 - r1 * r1);
            worst_c = worst_c.max(rel(st.h, exact));
        }
        println!("X6 band mean closed form {worst_c:.2e} (limit 1e-13)");
        assert!(worst_c <= 1e-13);
        // flux conservation on the mismatched mesh pair
        let (d, r) = mesh_pair();
        let donor = donor_from(
            &d,
            |x| [0.0, 0.0, 1.0 + 0.2 * x[0]],
            0.0,
            |x| 2.0 + 3.0 * x[0].hypot(x[1]),
        );
        let bands = mp.average(&donor).unwrap();
        let recv = receiver_from(&r);
        let st = mp.pass(&bands, &recv).unwrap();
        let mut before = [0.0; 5];
        for f in &donor {
            before[mp.band_of(f.x).unwrap()] += vec3::dot(f.u, f.s) * f.h;
        }
        let mut after = [0.0; 5];
        for (f, s) in recv.iter().zip(&st) {
            after[s.band] += vec3::dot(s.u, f.s) * s.h;
        }
        let worst = (0..5).map(|b| rel(after[b], before[b])).fold(0.0, f64::max);
        println!("X6 scalar flux conservation {worst:.2e} (limit 1e-13)");
        assert!(worst <= 1e-13);
    }

    /// Registered X7 as written (ADR 0055): 10 bands of width 0.05 on the 48 x 16 donor mesh; the 16 layers do not tile the
    /// bands (addendum 4 gives the measured value). Kept next to the corrected 8-band test.
    #[test]
    #[ignore]
    fn x7_mesh_geometry_swirl_as_registered_10_bands_known_miss() {
        let (omega_s, w) = (3.0, 2.0);
        let mp = MixingPlane::uniform([0.0; 3], AX, 0.5, 1.0, 10).unwrap();
        let (d, _) = mesh_pair();
        let donor = donor_from(&d, |x| [-omega_s * x[1], omega_s * x[0], w], 0.0, |_| 0.0);
        let bands = mp.average(&donor).unwrap();
        let mut worst: f64 = 0.0;
        for (b, st) in bands.iter().enumerate() {
            let (r1, r2): (f64, f64) = (0.5 + 0.05 * b as f64, 0.5 + 0.05 * (b + 1) as f64);
            let exact = w * omega_s * 2.0 * PI * (r2.powi(4) - r1.powi(4)) / 4.0;
            worst = worst.max(rel(st.gamma * st.q, exact));
        }
        println!("X7 as registered (10 bands): worst relative error {worst:.3e} (limit 1e-2)");
        assert!(worst <= 1e-2);
    }

    #[test]
    fn x7_mesh_geometry_swirl_close_to_the_closed_form() {
        // ADR 0055 addendum 1: 8 bands of two radial layers each (edges aligned with the 16 layers of the donor mesh)
        let (omega_s, w) = (3.0, 2.0);
        let mp = MixingPlane::uniform([0.0; 3], AX, 0.5, 1.0, 8).unwrap();
        let (d, _) = mesh_pair();
        let donor = donor_from(&d, |x| [-omega_s * x[1], omega_s * x[0], w], 0.0, |_| 0.0);
        let bands = mp.average(&donor).unwrap();
        let mut worst: f64 = 0.0;
        for (b, st) in bands.iter().enumerate() {
            let (r1, r2): (f64, f64) = (0.5 + 0.0625 * b as f64, 0.5 + 0.0625 * (b + 1) as f64);
            let exact = w * omega_s * 2.0 * PI * (r2.powi(4) - r1.powi(4)) / 4.0;
            worst = worst.max(rel(st.gamma * st.q, exact));
        }
        println!("X7 mesh-geometry donor angular momentum vs closed form: {worst:.3e} (limit 1e-2)");
        assert!(worst <= 1e-2);
    }

    #[test]
    fn x8_located_errors() {
        let err = |r: Result<MixingPlane>| r.unwrap_err().to_string();
        assert!(err(MixingPlane::new([0.0; 3], AX, vec![0.5])).contains("mixing_plane.edges"));
        assert!(err(MixingPlane::new([0.0; 3], AX, vec![0.5, 0.5])).contains("edges[1]"));
        assert!(err(MixingPlane::new([0.0; 3], AX, vec![0.5, 1.0, 0.8])).contains("edges[2]"));
        assert!(err(MixingPlane::new([0.0; 3], AX, vec![0.5, f64::NAN])).contains("edges[1]"));
        assert!(err(MixingPlane::new([0.0; 3], [0.0; 3], vec![0.5, 1.0])).contains("mixing_plane.axis"));
        let mp = MixingPlane::new([0.0; 3], AX, vec![0.5, 1.0]).unwrap();
        let ok = DonorFace {
            s: [0.0, 0.0, 1.0],
            x: [0.7, 0.0, 0.0],
            u: [0.0, 0.0, 1.0],
            p: 0.0,
            h: 0.0,
        };
        // non-finite data
        let mut d = ok;
        d.u[1] = f64::NAN;
        assert!(mp.average(&[d]).unwrap_err().to_string().contains("donor[0]"));
        // face outside all bands names the face
        let mut d = ok;
        d.x = [2.0, 0.0, 0.0];
        let e = mp.average(&[ok, d]).unwrap_err().to_string();
        assert!(e.contains("donor[1]") && e.contains("outside"), "{e}");
        // empty band
        let mp2 = MixingPlane::new([0.0; 3], AX, vec![0.5, 0.8, 1.0]).unwrap();
        let e = mp2.average(&[ok]).unwrap_err().to_string();
        assert!(e.contains("band[1]") && e.contains("no donor"), "{e}");
        // cancelling flux
        let mut back = ok;
        back.u = [0.0, 0.0, -1.0];
        let e = mp.average(&[ok, back]).unwrap_err().to_string();
        assert!(e.contains("band[0]") && e.contains("cancels"), "{e}");
        // receiver: empty band, reversed orientation, outside, on axis
        let bands = mp.average(&[ok]).unwrap();
        let r_ok = ReceiverFace {
            s: [0.0, 0.0, 1.0],
            x: [0.7, 0.0, 0.0],
        };
        let mut rev = r_ok;
        rev.s = [0.0, 0.0, -1.0];
        assert!(mp.pass(&bands, &[rev]).unwrap_err().to_string().contains("band[0]"));
        let mut out = r_ok;
        out.x = [3.0, 0.0, 0.0];
        assert!(mp.pass(&bands, &[out]).unwrap_err().to_string().contains("receiver[0]"));
        assert!(mp
            .pass(&[], &[r_ok])
            .unwrap_err()
            .to_string()
            .contains("mixing_plane.bands"));
        let mp0 = MixingPlane::new([0.0; 3], AX, vec![0.0, 1.0]).unwrap();
        let bz = mp0.average(&[ok]).unwrap();
        let mut axis = r_ok;
        axis.x = [0.0, 0.0, 0.0];
        assert!(mp0.pass(&bz, &[axis]).unwrap_err().to_string().contains("on the axis"));
        // mismatched array lengths
        assert!(mp
            .area_mean(&[(ok.s, ok.x)], &[1.0, 2.0])
            .unwrap_err()
            .to_string()
            .contains("mixing_plane.field"));
    }
}
