//! Turbomachinery post-processing (F-123, ADR 0054): constant-span and constant-streamwise surfaces, pitch-averaged profiles and
//! performance reports. Nothing here is validated against a turbomachinery flow solution; the checks are geometric and algebraic.
//!
//! Cylindrical frame: `z = (p - o) . a`, `r = |(p - o) - z a|`, `theta` from `ref_dir` about `a` (right-handed). Meridional curves
//! are polylines `(z, r)`.
//!
//! Averages (formulas): area-weighted `<f>_A = sum f_i A_i / sum A_i`; mass-flow-weighted
//! `<f>_m = sum rho (u.n)_i f_i A_i / sum rho (u.n)_i A_i` with `n` the downstream unit normal of the station.
//! The circumferential average at one `(z, r)` is the arithmetic mean over `n_theta` equispaced angles of one pitch (or the full
//! circle), the trapezoid rule of a periodic function.
//!
//! Performance (all closed forms): pump head `H = dp0 / (rho g)`, hydraulic efficiency `eta = rho g H Q / (T omega)`; compressor
//! stage `PR = p0_out / p0_in`, `tau = T0_out / T0_in`, `eta_is = (PR^k - 1) / (tau - 1)`,
//! `eta_p = k ln PR / ln tau`, `k = (gamma - 1) / gamma`.

use crate::integrals::wall_force_and_moment;
use crate::surfaces::{cut_cylinder, cut_implicit, Surface};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_fv::{FvGeometry, VectorField};
use rcfd_mesh::Mesh;

fn bad(msg: String) -> Error {
    Error::Other(msg)
}

/// Rotation axis and angle origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurboAxis {
    /// Point on the axis.
    pub origin: Vec3,
    /// Unit axis (right-handed rotation sense).
    pub axis: Vec3,
    /// Unit vector orthogonal to the axis, `theta = 0`.
    pub ref_dir: Vec3,
}

impl TurboAxis {
    /// Axis through `origin` along `axis`; `ref_dir` is orthogonalised against the axis. Errors: zero or parallel vectors.
    pub fn new(origin: Vec3, axis: Vec3, ref_dir: Vec3) -> Result<TurboAxis> {
        if !origin
            .iter()
            .chain(axis.iter())
            .chain(ref_dir.iter())
            .all(|x| x.is_finite())
        {
            return Err(bad("TurboAxis::new: inputs must be finite".into()));
        }
        if vec3::norm(axis) == 0.0 {
            return Err(bad("TurboAxis::new: axis must be non-zero".into()));
        }
        let a = vec3::unit(axis);
        let e = vec3::sub(ref_dir, vec3::scale(a, vec3::dot(ref_dir, a)));
        if vec3::norm(e) <= 1e-12 * vec3::norm(ref_dir).max(1e-300) {
            return Err(bad("TurboAxis::new: ref_dir must not be parallel to the axis".into()));
        }
        Ok(TurboAxis {
            origin,
            axis: a,
            ref_dir: vec3::unit(e),
        })
    }

    /// `(z, r, theta)` of a point, `theta` in `(-pi, pi]`.
    pub fn cylindrical(&self, p: Vec3) -> (f64, f64, f64) {
        let d = vec3::sub(p, self.origin);
        let z = vec3::dot(d, self.axis);
        let rv = vec3::sub(d, vec3::scale(self.axis, z));
        let e2 = vec3::cross(self.axis, self.ref_dir);
        (z, vec3::norm(rv), vec3::dot(rv, e2).atan2(vec3::dot(rv, self.ref_dir)))
    }

    /// Point from `(z, r, theta)`.
    pub fn point(&self, z: f64, r: f64, theta: f64) -> Vec3 {
        let e2 = vec3::cross(self.axis, self.ref_dir);
        let radial = vec3::add(vec3::scale(self.ref_dir, theta.cos()), vec3::scale(e2, theta.sin()));
        vec3::add(
            vec3::add(self.origin, vec3::scale(self.axis, z)),
            vec3::scale(radial, r),
        )
    }

    fn radial(&self, theta: f64) -> Vec3 {
        let e2 = vec3::cross(self.axis, self.ref_dir);
        vec3::add(vec3::scale(self.ref_dir, theta.cos()), vec3::scale(e2, theta.sin()))
    }
}

/// Meridional polyline `(z_i, r_i)` (a hub or shroud contour).
#[derive(Debug, Clone, PartialEq)]
pub struct Meridional {
    /// Points `(z, r)`.
    pub pts: Vec<(f64, f64)>,
}

impl Meridional {
    /// At least two finite points with `r >= 0` and no repeated consecutive point.
    pub fn new(pts: Vec<(f64, f64)>) -> Result<Meridional> {
        if pts.len() < 2 {
            return Err(bad("Meridional::new: at least two points are needed".into()));
        }
        for (i, p) in pts.iter().enumerate() {
            if !p.0.is_finite() || !p.1.is_finite() || p.1 < 0.0 {
                return Err(bad(format!("Meridional::new: point {i} must be finite with r >= 0")));
            }
        }
        if pts.windows(2).any(|w| w[0] == w[1]) {
            return Err(bad("Meridional::new: consecutive points must differ".into()));
        }
        Ok(Meridional { pts })
    }

    /// Whether `z` is strictly increasing (required by [`span_surface`]).
    pub fn is_z_monotone(&self) -> bool {
        self.pts.windows(2).all(|w| w[1].0 > w[0].0)
    }

    /// Linear interpolation `r(z)`, constant beyond the end points (needs a monotone `z`).
    pub fn r_at(&self, z: f64) -> f64 {
        let p = &self.pts;
        if z <= p[0].0 {
            return p[0].1;
        }
        if z >= p[p.len() - 1].0 {
            return p[p.len() - 1].1;
        }
        let k = p.partition_point(|q| q.0 <= z) - 1;
        let t = (z - p[k].0) / (p[k + 1].0 - p[k].0);
        p[k].1 + t * (p[k + 1].1 - p[k].1)
    }

    /// Total arc length.
    pub fn length(&self) -> f64 {
        self.pts
            .windows(2)
            .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
            .sum()
    }

    /// Point at the normalised arc length `m in [0, 1]`.
    pub fn point_at(&self, m: f64) -> (f64, f64) {
        let target = m.clamp(0.0, 1.0) * self.length();
        let mut acc = 0.0;
        let n = self.pts.len() - 1;
        for k in 0..n {
            let (a, b) = (self.pts[k], self.pts[k + 1]);
            let l = (b.0 - a.0).hypot(b.1 - a.1);
            if acc + l >= target || k + 1 == n {
                let t = ((target - acc) / l).clamp(0.0, 1.0);
                return (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
            }
            acc += l;
        }
        self.pts[n]
    }
}

/// Family of constant-span surfaces.
#[derive(Debug, Clone, PartialEq)]
pub enum SpanFamily {
    /// Cylinders `r = radius`.
    Cylinder {
        /// Radius.
        radius: f64,
    },
    /// Revolution of `r = R_h(z) + s (R_t(z) - R_h(z))` (hub and shroud with monotone `z`).
    Curves {
        /// Hub contour.
        hub: Meridional,
        /// Shroud contour.
        shroud: Meridional,
    },
}

fn snap_radial(s: &mut Surface, axis: &TurboAxis, radius_of_z: impl Fn(f64) -> f64) {
    for p in &mut s.points {
        let (z, r, th) = axis.cylindrical(*p);
        if r > 1e-14 {
            *p = axis.point(z, radius_of_z(z), th);
        }
    }
}

/// Constant-span surface at span fraction `span` (`0` hub, `1` shroud; ignored for [`SpanFamily::Cylinder`]): the marching-tet
/// cut of `r - R_s(z)` with every vertex then moved radially to `r = R_s(z)` (exact radius; area converges as the inscribed
/// polygon). The facet normal points to larger radius.
pub fn span_surface(mesh: &Mesh, axis: &TurboAxis, family: &SpanFamily, span: f64) -> Result<Surface> {
    if !span.is_finite() || !(0.0..=1.0).contains(&span) {
        return Err(bad(format!("span_surface: span = {span} must lie in [0, 1]")));
    }
    let radius_of_z: Box<dyn Fn(f64) -> f64> = match family {
        SpanFamily::Cylinder { radius } => {
            if !radius.is_finite() || *radius <= 0.0 {
                return Err(bad("span_surface: cylinder radius must be positive".into()));
            }
            let r = *radius;
            Box::new(move |_| r)
        }
        SpanFamily::Curves { hub, shroud } => {
            if !hub.is_z_monotone() || !shroud.is_z_monotone() {
                return Err(bad(
                    "span_surface: hub and shroud need strictly increasing z (radial machines are not supported)"
                        .into(),
                ));
            }
            let (h, t) = (hub.clone(), shroud.clone());
            Box::new(move |z| {
                let (rh, rt) = (h.r_at(z), t.r_at(z));
                rh + span * (rt - rh)
            })
        }
    };
    let ax = *axis;
    let mut s = cut_implicit(mesh, |p| {
        let (z, r, _) = ax.cylindrical(p);
        r - radius_of_z(z)
    })?;
    snap_radial(&mut s, axis, &radius_of_z);
    Ok(s)
}

/// Cylinder `r = radius` with the same snapping (shortcut for [`SpanFamily::Cylinder`]).
pub fn span_surface_cylinder(mesh: &Mesh, axis: &TurboAxis, radius: f64) -> Result<Surface> {
    span_surface(mesh, axis, &SpanFamily::Cylinder { radius }, 0.0)
}

/// Unsnapped quadric cut `r = radius` (second order in the mesh size), for comparison.
pub fn unsnapped_cylinder(mesh: &Mesh, axis: &TurboAxis, radius: f64) -> Result<Surface> {
    cut_cylinder(mesh, axis.origin, axis.axis, radius)
}

/// Unit meridional normal `(n_z, n_r)` of the segment `H -> S`, oriented along `downstream`, and the segment length.
type Pair = (f64, f64);

fn station_frame(h: Pair, s: Pair, downstream: Pair) -> Result<(Pair, Pair, f64)> {
    let (dz, dr) = (s.0 - h.0, s.1 - h.1);
    let l = dz.hypot(dr);
    if l.is_nan() || l <= 0.0 || !l.is_finite() {
        return Err(bad("station: hub and shroud points coincide".into()));
    }
    let t = (dz / l, dr / l);
    let mut n = (-t.1, t.0);
    let dot = n.0 * downstream.0 + n.1 * downstream.1;
    if dot == 0.0 || !dot.is_finite() {
        return Err(bad(
            "station: the downstream direction is parallel to the station".into()
        ));
    }
    if dot < 0.0 {
        n = (-n.0, -n.1);
    }
    Ok((t, n, l))
}

/// Constant-streamwise surface at normalised arc length `m` of the hub and shroud contours: the revolution of the segment from the
/// hub point to the shroud point (plane, cone or cylinder), cut from the mesh, clipped between hub and shroud, then moved in the
/// meridional plane onto the exact segment. The facet normal points downstream (along the mean hub-to-outlet direction).
pub fn streamwise_surface(
    mesh: &Mesh,
    axis: &TurboAxis,
    hub: &Meridional,
    shroud: &Meridional,
    m: f64,
) -> Result<Surface> {
    if !m.is_finite() || !(0.0..=1.0).contains(&m) {
        return Err(bad(format!("streamwise_surface: m = {m} must lie in [0, 1]")));
    }
    let h = hub.point_at(m);
    let s = shroud.point_at(m);
    let end = |c: &Meridional, k: usize| c.pts[k];
    let n_last = hub.pts.len() - 1;
    let m_last = shroud.pts.len() - 1;
    let down = (
        0.5 * ((end(hub, n_last).0 - end(hub, 0).0) + (end(shroud, m_last).0 - end(shroud, 0).0)),
        0.5 * ((end(hub, n_last).1 - end(hub, 0).1) + (end(shroud, m_last).1 - end(shroud, 0).1)),
    );
    let (t, n, l) = station_frame(h, s, down)?;
    let ax = *axis;
    let coords = move |p: Vec3| {
        let (z, r, _) = ax.cylindrical(p);
        let (dz, dr) = (z - h.0, r - h.1);
        (n.0 * dz + n.1 * dr, t.0 * dz + t.1 * dr)
    };
    let cut = cut_implicit(mesh, |p| coords(p).0)?;
    let vals: Vec<f64> = cut
        .points
        .iter()
        .map(|&p| {
            let tau = coords(p).1;
            tau.min(l - tau)
        })
        .collect();
    let (mut s2, v2) = cut.clip_by_values(&vals, 0.0, true)?;
    for (p, v) in s2.points.iter_mut().zip(&v2) {
        let (_, _, th) = axis.cylindrical(*p);
        let (_, tau) = coords(*p);
        let tau = if *v == 0.0 {
            if tau < 0.5 * l {
                0.0
            } else {
                l
            }
        } else {
            tau.clamp(0.0, l)
        };
        let (zn, rn) = (h.0 + tau * t.0, h.1 + tau * t.1);
        *p = axis.point(zn, rn, th);
    }
    Ok(s2)
}

/// 360-degree reconstruction of a one-passage surface from `n_passages` rotated copies.
pub fn reconstruct_360(surface: &Surface, n_passages: usize, axis: &TurboAxis) -> Result<Surface> {
    let parts = surface.rotational_copies(n_passages, axis.axis, axis.origin)?;
    Ok(Surface::merged(&parts))
}

/// Mean of `sample` over `n_theta` equispaced angles `theta0 + k pitch / n_theta` at `(z, r)`; `pitch = None` is the full circle.
/// For `A + B cos(N theta)` the mean is `A` to round-off when `N pitch / (2 pi)` is an integer and `n_theta` does not divide `N`
/// times that integer (otherwise the harmonic aliases to `A + B`).
pub fn circumferential_mean(
    axis: &TurboAxis,
    z: f64,
    r: f64,
    theta0: f64,
    n_theta: usize,
    pitch: Option<f64>,
    mut sample: impl FnMut(Vec3) -> f64,
) -> Result<f64> {
    if n_theta == 0 {
        return Err(bad("circumferential_mean: n_theta must be at least 1".into()));
    }
    let pitch = pitch.unwrap_or(2.0 * std::f64::consts::PI);
    if !(pitch.is_finite() && pitch > 0.0) || !theta0.is_finite() || !z.is_finite() || !(r.is_finite() && r >= 0.0) {
        return Err(bad(
            "circumferential_mean: z, r, theta0 and pitch must be finite with r >= 0, pitch > 0".into(),
        ));
    }
    let mut s = 0.0;
    for k in 0..n_theta {
        let v = sample(axis.point(z, r, theta0 + pitch * k as f64 / n_theta as f64));
        if !v.is_finite() {
            return Err(bad(format!("circumferential_mean: sample {k} is not finite")));
        }
        s += v;
    }
    Ok(s / n_theta as f64)
}

/// Sample points of a station (a hub-to-shroud segment of the meridional plane) over the span and the pitch.
#[derive(Debug, Clone)]
pub struct StationGrid {
    /// Sample points, span-major: index `j * n_theta + k`.
    pub points: Vec<Vec3>,
    /// Downstream unit normal at every sample.
    pub normals: Vec<Vec3>,
    /// Span intervals (even); `n_span + 1` span levels.
    pub n_span: usize,
    /// Angles per span level.
    pub n_theta: usize,
    /// Span fraction of every level.
    pub span: Vec<f64>,
    /// Radius of every level.
    pub radius: Vec<f64>,
    /// Composite-Simpson weight times `r dl` per level (per unit angle).
    weights: Vec<f64>,
}

impl StationGrid {
    /// Station from `hub = (z, r)` to `shroud = (z, r)`, `n_span` Simpson intervals (even, at least 2), `n_theta` angles over
    /// `pitch` (`None`: full circle) starting at `theta0`; `downstream` orients the normals.
    pub fn new(
        axis: &TurboAxis,
        hub: (f64, f64),
        shroud: (f64, f64),
        downstream: (f64, f64),
        n_span: usize,
        n_theta: usize,
        theta0: f64,
        pitch: Option<f64>,
    ) -> Result<StationGrid> {
        if n_span < 2 || n_span % 2 != 0 {
            return Err(bad("StationGrid::new: n_span must be even and at least 2".into()));
        }
        if n_theta == 0 {
            return Err(bad("StationGrid::new: n_theta must be at least 1".into()));
        }
        let pitch = pitch.unwrap_or(2.0 * std::f64::consts::PI);
        if !(pitch.is_finite() && pitch > 0.0) || !theta0.is_finite() {
            return Err(bad("StationGrid::new: theta0 must be finite and pitch positive".into()));
        }
        if hub.1 < 0.0 || shroud.1 < 0.0 || !hub.0.is_finite() || !shroud.0.is_finite() {
            return Err(bad(
                "StationGrid::new: hub and shroud points must be finite with r >= 0".into(),
            ));
        }
        let (_, n, l) = station_frame(hub, shroud, downstream)?;
        let mut g = StationGrid {
            points: vec![],
            normals: vec![],
            n_span,
            n_theta,
            span: vec![],
            radius: vec![],
            weights: vec![],
        };
        let ds = 1.0 / n_span as f64;
        for j in 0..=n_span {
            let s = j as f64 * ds;
            let (z, r) = (hub.0 + s * (shroud.0 - hub.0), hub.1 + s * (shroud.1 - hub.1));
            let sw = if j == 0 || j == n_span {
                1.0
            } else if j % 2 == 1 {
                4.0
            } else {
                2.0
            };
            g.span.push(s);
            g.radius.push(r);
            g.weights.push(sw * ds / 3.0 * l * r);
            for k in 0..n_theta {
                let th = theta0 + pitch * k as f64 / n_theta as f64;
                g.points.push(axis.point(z, r, th));
                g.normals.push(vec3::add(
                    vec3::scale(axis.axis, n.0),
                    vec3::scale(axis.radial(th), n.1),
                ));
            }
        }
        Ok(g)
    }

    /// Averages of a field `f` and the mass-flux density `mf = rho (u . n)` sampled at [`StationGrid::points`].
    pub fn average(&self, f: &[f64], mass_flux: &[f64]) -> Result<StationAverages> {
        let n = self.points.len();
        if f.len() != n || mass_flux.len() != n {
            return Err(bad(format!(
                "StationGrid::average: f has {} and mass_flux {} entries, the grid has {n} points",
                f.len(),
                mass_flux.len()
            )));
        }
        if f.iter().chain(mass_flux.iter()).any(|x| !x.is_finite()) {
            return Err(bad("StationGrid::average: f and mass_flux must be finite".into()));
        }
        let nt = self.n_theta as f64;
        let (mut area_profile, mut mass_profile) = (Vec::new(), Vec::new());
        let (mut sw, mut swf, mut smf, mut smff, mut abs_mf) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for j in 0..=self.n_span {
            let (mut sf, mut sm, mut smf_j, mut am) = (0.0, 0.0, 0.0, 0.0);
            for k in 0..self.n_theta {
                let i = j * self.n_theta + k;
                sf += f[i];
                sm += mass_flux[i];
                smf_j += mass_flux[i] * f[i];
                am += mass_flux[i].abs();
            }
            area_profile.push(sf / nt);
            mass_profile.push(if sm != 0.0 { smf_j / sm } else { f64::NAN });
            let w = self.weights[j];
            sw += w;
            swf += w * sf / nt;
            smf += w * sm / nt;
            smff += w * smf_j / nt;
            abs_mf += w * am / nt;
        }
        if smf.abs() <= 1e-14 * abs_mf.max(1e-300) {
            return Err(bad(
                "StationGrid::average: zero net mass flow through the station".into()
            ));
        }
        let tau = 2.0 * std::f64::consts::PI;
        Ok(StationAverages {
            span: self.span.clone(),
            radius: self.radius.clone(),
            area_profile,
            mass_profile,
            area_mean: swf / sw,
            mass_mean: smff / smf,
            mass_flow: tau * smf,
            area: tau * sw,
        })
    }
}

/// Result of [`StationGrid::average`].
#[derive(Debug, Clone, PartialEq)]
pub struct StationAverages {
    /// Span fraction of every level.
    pub span: Vec<f64>,
    /// Radius of every level.
    pub radius: Vec<f64>,
    /// Pitch (area) average at every level.
    pub area_profile: Vec<f64>,
    /// Pitch mass-flux-weighted average at every level (`NaN` where the level carries no net flux).
    pub mass_profile: Vec<f64>,
    /// Area-weighted average over the whole station.
    pub area_mean: f64,
    /// Mass-flow-weighted average over the whole station.
    pub mass_mean: f64,
    /// Mass flow through the 360-degree station (positive downstream).
    pub mass_flow: f64,
    /// Area of the 360-degree station (Simpson in span, so exact for a plane or cone).
    pub area: f64,
}

// -----------------------------------------------------------------------------------------------------------------------
// performance
// -----------------------------------------------------------------------------------------------------------------------

fn pos(who: &str, name: &str, x: f64) -> Result<()> {
    if x.is_finite() && x > 0.0 {
        Ok(())
    } else {
        Err(bad(format!("{who}: {name} = {x} must be finite and positive")))
    }
}

/// Incompressible pump performance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PumpPerformance {
    /// Total-pressure rise `p0_out - p0_in` (Pa).
    pub dp0: f64,
    /// Head `dp0 / (rho g)` (m).
    pub head: f64,
    /// Hydraulic power `dp0 Q` (W).
    pub hydraulic_power: f64,
    /// Shaft power `T omega` (W).
    pub shaft_power: f64,
    /// `rho g H Q / (T omega)`.
    pub efficiency: f64,
}

/// Pump head and hydraulic efficiency from mass-flow-averaged total pressures `p0_in`, `p0_out` (Pa), volume flow `q` (m3/s),
/// shaft torque on the fluid `torque` (N m) and angular speed `omega` (rad/s). Errors: non-positive `rho`, `g`, `q`, `torque`,
/// `omega`, non-finite pressures.
pub fn pump_performance(
    rho: f64,
    g: f64,
    q: f64,
    p0_in: f64,
    p0_out: f64,
    torque: f64,
    omega: f64,
) -> Result<PumpPerformance> {
    const W: &str = "pump_performance";
    pos(W, "rho", rho)?;
    pos(W, "g", g)?;
    pos(W, "q", q)?;
    pos(W, "torque", torque)?;
    pos(W, "omega", omega)?;
    if !p0_in.is_finite() || !p0_out.is_finite() {
        return Err(bad(format!("{W}: p0_in and p0_out must be finite")));
    }
    let dp0 = p0_out - p0_in;
    let head = dp0 / (rho * g);
    let shaft_power = torque * omega;
    Ok(PumpPerformance {
        dp0,
        head,
        hydraulic_power: dp0 * q,
        shaft_power,
        efficiency: rho * g * head * q / shaft_power,
    })
}

/// Compressor stage performance (perfect gas, adiabatic).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompressorPerformance {
    /// Total-to-total pressure ratio.
    pub pressure_ratio: f64,
    /// Total temperature ratio.
    pub temperature_ratio: f64,
    /// Isentropic efficiency `(PR^k - 1) / (tau - 1)`.
    pub eta_isentropic: f64,
    /// Polytropic efficiency `k ln PR / ln tau`.
    pub eta_polytropic: f64,
}

/// Compressor total-to-total ratios and efficiencies. Errors: `gamma <= 1`, non-positive totals, `PR <= 1` or `tau <= 1`.
pub fn compressor_performance(
    gamma: f64,
    p0_in: f64,
    p0_out: f64,
    t0_in: f64,
    t0_out: f64,
) -> Result<CompressorPerformance> {
    const W: &str = "compressor_performance";
    if !gamma.is_finite() || gamma <= 1.0 {
        return Err(bad(format!("{W}: gamma = {gamma} must be finite and greater than 1")));
    }
    pos(W, "p0_in", p0_in)?;
    pos(W, "p0_out", p0_out)?;
    pos(W, "t0_in", t0_in)?;
    pos(W, "t0_out", t0_out)?;
    let (pr, tau) = (p0_out / p0_in, t0_out / t0_in);
    if pr <= 1.0 || tau <= 1.0 {
        return Err(bad(format!(
            "{W}: a compressor needs pressure ratio > 1 and temperature ratio > 1 (got {pr} and {tau})"
        )));
    }
    let k = (gamma - 1.0) / gamma;
    Ok(CompressorPerformance {
        pressure_ratio: pr,
        temperature_ratio: tau,
        eta_isentropic: (pr.powf(k) - 1.0) / (tau - 1.0),
        eta_polytropic: k * pr.ln() / tau.ln(),
    })
}

/// Head coefficient `g H / u_t^2`, `u_t = omega r_ref`.
pub fn head_coefficient(g: f64, head: f64, omega: f64, r_ref: f64) -> Result<f64> {
    pos("head_coefficient", "g", g)?;
    pos("head_coefficient", "omega", omega)?;
    pos("head_coefficient", "r_ref", r_ref)?;
    if !head.is_finite() {
        return Err(bad("head_coefficient: head must be finite".into()));
    }
    Ok(g * head / (omega * r_ref).powi(2))
}

/// Flow coefficient `Q / (u_t A_ref)`, `u_t = omega r_ref`.
pub fn flow_coefficient(q: f64, omega: f64, r_ref: f64, a_ref: f64) -> Result<f64> {
    pos("flow_coefficient", "omega", omega)?;
    pos("flow_coefficient", "r_ref", r_ref)?;
    pos("flow_coefficient", "a_ref", a_ref)?;
    if !q.is_finite() {
        return Err(bad("flow_coefficient: q must be finite".into()));
    }
    Ok(q / (omega * r_ref * a_ref))
}

/// Torque delivered to the fluid about the axis from the walls of the rotor: `-(sum M_wall) . a`, where `M_wall` is the moment
/// of the fluid on the patches about `axis.origin` ([`wall_force_and_moment`]). Dynamic units (`rho` given).
#[allow(clippy::too_many_arguments)]
pub fn torque_on_fluid(
    geom: &FvGeometry,
    patches: &[&str],
    u: &VectorField,
    p_face: &[f64],
    nu: f64,
    rho: f64,
    axis: &TurboAxis,
) -> Result<f64> {
    if patches.is_empty() {
        return Err(bad("torque_on_fluid: no patches given".into()));
    }
    let mut m = vec3::ZERO;
    for p in patches {
        let (_, mom) = wall_force_and_moment(geom, p, u, p_face, nu, rho, axis.origin)?;
        m = vec3::add(m, mom);
    }
    Ok(-vec3::dot(m, axis.axis))
}

/// Table of named values with units, exportable to CSV and HTML.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PerformanceReport {
    /// Title (HTML heading).
    pub title: String,
    /// Rows `(name, value, unit)`.
    pub rows: Vec<(String, f64, String)>,
}

impl PerformanceReport {
    /// Empty report.
    pub fn new(title: &str) -> PerformanceReport {
        PerformanceReport {
            title: title.to_string(),
            rows: vec![],
        }
    }

    /// Append a row; the value must be finite.
    pub fn push(&mut self, name: &str, value: f64, unit: &str) -> Result<()> {
        if !value.is_finite() {
            return Err(bad(format!("PerformanceReport::push: value of '{name}' is not finite")));
        }
        self.rows.push((name.to_string(), value, unit.to_string()));
        Ok(())
    }

    /// Report of a pump.
    pub fn from_pump(p: &PumpPerformance, q: f64) -> Result<PerformanceReport> {
        let mut r = PerformanceReport::new("Pump performance");
        r.push("total_pressure_rise", p.dp0, "Pa")?;
        r.push("head", p.head, "m")?;
        r.push("volume_flow", q, "m3/s")?;
        r.push("hydraulic_power", p.hydraulic_power, "W")?;
        r.push("shaft_power", p.shaft_power, "W")?;
        r.push("hydraulic_efficiency", p.efficiency, "1")?;
        Ok(r)
    }

    /// Report of a compressor stage.
    pub fn from_compressor(c: &CompressorPerformance) -> Result<PerformanceReport> {
        let mut r = PerformanceReport::new("Compressor stage performance");
        r.push("total_pressure_ratio", c.pressure_ratio, "1")?;
        r.push("total_temperature_ratio", c.temperature_ratio, "1")?;
        r.push("isentropic_efficiency", c.eta_isentropic, "1")?;
        r.push("polytropic_efficiency", c.eta_polytropic, "1")?;
        Ok(r)
    }

    /// `name,value,unit` lines with 17 significant digits (round-trips every value); names and units with commas, quotes or
    /// newlines are quoted.
    pub fn to_csv(&self) -> String {
        fn q(s: &str) -> String {
            if s.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        }
        let mut out = String::from("name,value,unit\n");
        for (n, v, u) in &self.rows {
            out.push_str(&format!("{},{:.16e},{}\n", q(n), v, q(u)));
        }
        out
    }

    /// Self-contained HTML page with an escaped table.
    pub fn to_html(&self) -> String {
        fn esc(s: &str) -> String {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
        }
        let mut out = String::from("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>");
        out.push_str(&esc(&self.title));
        out.push_str("</title></head><body>\n<h1>");
        out.push_str(&esc(&self.title));
        out.push_str("</h1>\n<table border=\"1\"><tr><th>name</th><th>value</th><th>unit</th></tr>\n");
        for (n, v, u) in &self.rows {
            out.push_str(&format!(
                "<tr><td>{}</td><td>{:.10e}</td><td>{}</td></tr>\n",
                esc(n),
                v,
                esc(u)
            ));
        }
        out.push_str("</table>\n</body></html>\n");
        out
    }
}
