//! Body-fitted periodic single-passage hexahedral mesh of an [`AxialFan`] (wave 32A, ADR 0065).
//!
//! One structured index space `(i, j, k)` = (axial column, pitchwise, radial). Every radial node layer is a 2-D grid in the
//! unrolled plane `(z, s)`: columns are `z = const` lines; the seams follow the camber line inside the chord range and are
//! axial outside; the solid footprint (chord columns, middle band, layers below the blade tip) is deleted, so the faces of
//! kept cells next to deleted cells form the `blade` patch. The blade is cut by the planes `z = z_a` and `z = z_b` (chord
//! fractions `trunc_le`, `trunc_te` along the camber line), giving flat blunt end faces of height about `2 yt`.
//! This is a lower-fidelity mesh: see the limits in ADR 0065 and `docs/meshing.md`.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::fan::{AxialFan, FanError, Side, Station};
use crate::mesh::{BoundarySpec, Mesh, PatchKind};
use rcfd_core::vec3::Vec3;

/// How the first cell height is given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FirstCell {
    /// First cell height [m].
    Height(f64),
    /// Target `y+` at the first cell centre, with the estimate of [`first_cell_height_from_yplus`].
    YPlus {
        /// Target `y+`.
        y_plus: f64,
        /// Reference speed [m/s] (relative speed at the reference chord).
        speed: f64,
        /// Kinematic viscosity [m^2/s].
        nu: f64,
    },
}

/// First cell height for a target `y+` at the cell centre: `h = 2 y+ nu / u_tau`, `u_tau = U sqrt(Cf/2)`,
/// `Cf = 0.0576 Re_c^(-1/5)`, `Re_c = U c / nu` (flat-plate turbulent correlation, an estimate).
pub fn first_cell_height_from_yplus(y_plus: f64, speed: f64, nu: f64, chord: f64) -> f64 {
    let re = speed * chord / nu;
    let cf = 0.0576 * re.powf(-0.2);
    let u_tau = speed * (0.5 * cf).sqrt();
    2.0 * y_plus * nu / u_tau
}

/// Mesh controls.
#[derive(Debug, Clone, PartialEq)]
pub struct SectorSpec {
    /// Axial cells from the inlet to the leading-edge plane.
    pub n_up: usize,
    /// Axial cells along the truncated chord.
    pub n_chord: usize,
    /// Axial cells from the trailing-edge plane to the outlet.
    pub n_down: usize,
    /// Pitchwise cells from each seam to the blade (including the boundary layers).
    pub n_side: usize,
    /// Pitchwise cells across the blunt end faces / the solid footprint band.
    pub n_mid: usize,
    /// Radial cells from the hub to the blade tip.
    pub n_span: usize,
    /// Radial cells in the tip gap.
    pub n_gap: usize,
    /// Exact geometric boundary-layer cells on every wall.
    pub n_bl: usize,
    /// Growth ratio of the boundary-layer cells.
    pub growth: f64,
    /// First cell height.
    pub first_cell: FirstCell,
    /// Leading-edge truncation (chord fraction, along the camber line).
    pub trunc_le: f64,
    /// Trailing-edge truncation (chord fraction).
    pub trunc_te: f64,
    /// `false` leaves the solid footprint filled (an empty passage with the same nodes).
    pub include_blade: bool,
}

/// Summary of the mesh quality distribution.
#[derive(Debug, Clone, PartialEq)]
pub struct QualitySummary {
    /// Maximum non-orthogonality [deg].
    pub max_non_orthogonality: f64,
    /// Mean non-orthogonality [deg].
    pub mean_non_orthogonality: f64,
    /// 99th percentile of the non-orthogonality [deg].
    pub p99_non_orthogonality: f64,
    /// Faces above 50 deg.
    pub n_above_50: usize,
    /// Faces above 60 deg.
    pub n_above_60: usize,
    /// Maximum face skewness.
    pub max_skewness: f64,
    /// Maximum cell aspect ratio.
    pub max_aspect_ratio: f64,
    /// Minimum cell volume.
    pub min_volume: f64,
    /// Maximum ratio of the volumes of two cells sharing a face.
    pub max_volume_ratio: f64,
}

/// The generated passage mesh with its structured bookkeeping.
#[derive(Debug, Clone)]
pub struct SectorMesh {
    /// The polyhedral mesh (cell zone `rotor`, cyclic pair `seam_lo` / `seam_hi`).
    pub mesh: Mesh,
    /// All structured grid points, id `i + (n_i+1) (j + (n_j+1) k)`.
    pub grid: Vec<Vec3>,
    /// Cells in i.
    pub n_i: usize,
    /// Cells in j.
    pub n_j: usize,
    /// Cells in k.
    pub n_k: usize,
    /// Axial cells before the blade.
    pub n_up: usize,
    /// Axial cells along the chord.
    pub n_chord: usize,
    /// Pitchwise cells per side.
    pub n_side: usize,
    /// Pitchwise band cells.
    pub n_mid: usize,
    /// Radial cells below the blade tip (node layer `n_span` is the tip cap).
    pub n_span: usize,
    /// Boundary-layer cells.
    pub n_bl: usize,
    /// Growth ratio.
    pub growth: f64,
    /// First cell height used.
    pub h1: f64,
    /// Radius of each node layer.
    pub radii: Vec<f64>,
    /// Removed (solid) cells `(i, j, k)`.
    pub removed: Vec<[usize; 3]>,
}

impl SectorMesh {
    /// Structured point id.
    pub fn node(&self, i: usize, j: usize, k: usize) -> usize {
        i + (self.n_i + 1) * (j + (self.n_j + 1) * k)
    }

    /// Quality distribution of the mesh.
    pub fn quality(&self) -> QualitySummary {
        quality_summary(&self.mesh)
    }
}

/// Quality distribution of any mesh.
pub fn quality_summary(mesh: &Mesh) -> QualitySummary {
    let m = crate::quality::compute_metrics(mesh);
    let mut no = m.non_orthogonality.clone();
    let n = no.len().max(1);
    let mean = no.iter().sum::<f64>() / n as f64;
    no.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    QualitySummary {
        max_non_orthogonality: no.last().copied().unwrap_or(0.0),
        mean_non_orthogonality: mean,
        p99_non_orthogonality: no[((no.len() as f64 * 0.99) as usize).min(no.len().saturating_sub(1))],
        n_above_50: no.iter().filter(|&&x| x > 50.0).count(),
        n_above_60: no.iter().filter(|&&x| x > 60.0).count(),
        max_skewness: m.skewness.iter().cloned().fold(0.0, f64::max),
        max_aspect_ratio: m.aspect_ratio.iter().cloned().fold(0.0, f64::max),
        max_volume_ratio: m.volume_ratio.iter().cloned().fold(0.0, f64::max),
        min_volume: mesh.cell_volumes().iter().cloned().fold(f64::INFINITY, f64::min),
    }
}

/// `n + 1` offsets `0 = x_0 < ... < x_n = len` with exactly geometric boundary-layer cells `h1 g^m` (first `nb` cells)
/// from the low and/or high end and a geometric fit (ratio solved) of the cells in between.
pub fn graded_cells(
    len: f64,
    n: usize,
    lo: Option<(f64, f64, usize)>,
    hi: Option<(f64, f64, usize)>,
) -> Result<Vec<f64>, String> {
    if n == 0 || !(len > 0.0) {
        return Err(format!(
            "needs at least one cell and a positive length (length {len:e}, {n} cells)"
        ));
    }
    let (nl, nh) = (lo.map_or(0, |a| a.2), hi.map_or(0, |a| a.2));
    if nl + nh > n {
        return Err(format!("{n} cells cannot hold {nl}+{nh} boundary-layer cells"));
    }
    let bl = |w: Option<(f64, f64, usize)>| -> Vec<f64> {
        w.map_or(Vec::new(), |(h, g, nb)| (0..nb).map(|m| h * g.powi(m as i32)).collect())
    };
    let (bl_lo, bl_hi) = (bl(lo), bl(hi));
    let (sum_lo, sum_hi): (f64, f64) = (bl_lo.iter().sum(), bl_hi.iter().sum());
    let n_mid = n - nl - nh;
    let l_mid = len - sum_lo - sum_hi;
    let mut h = vec![0.0; n];
    h[..nl].copy_from_slice(&bl_lo);
    for (m, v) in bl_hi.iter().enumerate() {
        h[n - 1 - m] = *v;
    }
    if n_mid == 0 {
        if l_mid.abs() > 1e-9 * len {
            return Err(format!(
                "boundary-layer cells (total {:e}) do not fill the length {len:e}",
                sum_lo + sum_hi
            ));
        }
    } else {
        if l_mid <= 0.0 {
            return Err(format!(
                "the boundary-layer cells (total {:e}) fill the whole length {len:e}; reduce the layers or the first cell height",
                sum_lo + sum_hi
            ));
        }
        let start = |w: Option<(f64, f64, usize)>| w.map(|(h1, g, nb)| h1 * g.powi(nb as i32));
        let (sa, sb) = (start(lo), start(hi));
        let mid = |rho: f64| -> Vec<f64> {
            (0..n_mid)
                .map(|m| match (sa, sb) {
                    (None, None) => l_mid / n_mid as f64,
                    (Some(a), None) => a * rho.powi(m as i32),
                    (None, Some(b)) => b * rho.powi((n_mid - 1 - m) as i32),
                    (Some(a), Some(b)) => (a * rho.powi(m as i32)).min(b * rho.powi((n_mid - 1 - m) as i32)),
                })
                .collect()
        };
        let hm = if (sa.is_none() && sb.is_none()) || n_mid == 1 {
            vec![l_mid / n_mid as f64; n_mid]
        } else {
            let f = |rho: f64| mid(rho).iter().sum::<f64>() - l_mid;
            let (mut a, mut b) = (0.02, 30.0);
            if f(a) > 0.0 || f(b) < 0.0 {
                return Err(format!(
                    "cannot fit {n_mid} cells of geometric growth into the remaining length {l_mid:e} after the boundary layers"
                ));
            }
            for _ in 0..200 {
                let c = 0.5 * (a + b);
                if f(c) > 0.0 {
                    b = c;
                } else {
                    a = c;
                }
            }
            mid(0.5 * (a + b))
        };
        let scale = l_mid / hm.iter().sum::<f64>();
        for (m, v) in hm.iter().enumerate() {
            h[nl + m] = v * scale;
        }
    }
    let mut x = vec![0.0; n + 1];
    // low end cumulative from 0, high end cumulative from len (keeps the boundary-layer cells exact at both ends)
    for m in 0..n {
        x[m + 1] = x[m] + h[m];
    }
    let mut y = len;
    for m in (0..n).rev() {
        if m + 1 > n - nh {
            y -= h[m];
            x[m] = y;
        } else {
            break;
        }
    }
    x[n] = len;
    Ok(x)
}

/// Far-field height of the middle band as a fraction of the pitch: the band (the continuation of the blunt end faces)
/// opens linearly in z from the face height to this value at the inlet / outlet.
const BAND_FAR: f64 = 0.15;

/// Offsets from the seam of the `n + 1` nodes between the seam and a wall node `len` away. The first `nb` cells next to the
/// wall are exactly `d1 g^m` (offset along the grid line); the remaining nodes follow the reference distribution built with
/// the first spacing `h_ref`, shifted by a displacement that decays linearly to zero at the seam, so that a locally larger
/// `d1` (steep wall) does not change the far cells abruptly. `wall_hi` says the wall is at the high end of the line.
fn wall_line(len: f64, n: usize, d1: f64, h_ref: f64, g: f64, nb: usize, wall_hi: bool) -> Result<Vec<f64>, String> {
    let r = if wall_hi {
        graded_cells(len, n, None, Some((h_ref, g, nb)))?
    } else {
        graded_cells(len, n, Some((h_ref, g, nb)), None)?
    };
    // distance from the wall of node m (m = 0 at the wall)
    let w_ref: Vec<f64> = (0..=n).map(|m| if wall_hi { len - r[n - m] } else { r[m] }).collect();
    let exact = |m: usize| (0..m).map(|q| d1 * g.powi(q as i32)).sum::<f64>();
    let delta = exact(nb) - w_ref[nb];
    let rest = len - w_ref[nb];
    if delta >= rest * 0.9 {
        return Err(format!(
            "the boundary layers (thickness {:e} along the grid line) leave no room before the seam",
            exact(nb)
        ));
    }
    let w: Vec<f64> = (0..=n)
        .map(|m| {
            if m <= nb {
                exact(m)
            } else {
                w_ref[m] + delta * (len - w_ref[m]) / rest
            }
        })
        .collect();
    Ok((0..=n).map(|m| if wall_hi { len - w[n - m] } else { w[m] }).collect())
}

/// A blade surface expressed as `s(z)` on its axially monotone branch.
struct ZCurve<'a> {
    fan: &'a AxialFan,
    st: Station,
    side: Side,
    x_min: f64,
}

impl<'a> ZCurve<'a> {
    fn new(fan: &'a AxialFan, st: Station, side: Side, path: &str) -> Result<Self, FanError> {
        let z = |x: f64| fan.surface_zs(&st, x, side).0;
        let n = 4000;
        let xs: Vec<f64> = (0..=n)
            .map(|i| 0.5 * (1.0 - (std::f64::consts::PI * i as f64 / n as f64).cos()))
            .collect();
        let (mut im, mut zm) = (0, f64::INFINITY);
        for (i, &x) in xs.iter().enumerate() {
            let v = z(x);
            if v < zm {
                zm = v;
                im = i;
            }
        }
        for w in xs[im..].windows(2) {
            if z(w[1]) <= z(w[0]) {
                return Err(FanError::new(
                    path,
                    format!(
                        "the {:?} surface at r = {:.4} is not monotone in the axial direction (stagger {:.1} deg too large for this thickness and camber)",
                        side,
                        st.radius,
                        st.stagger.to_degrees()
                    ),
                ));
            }
        }
        Ok(Self {
            fan,
            st,
            side,
            x_min: xs[im],
        })
    }

    fn s_at_z(&self, zt: f64, path: &str) -> Result<f64, FanError> {
        let f = |x: f64| self.fan.surface_zs(&self.st, x, self.side);
        let (z0, z1) = (f(self.x_min).0, f(1.0).0);
        if zt < z0 || zt > z1 {
            return Err(FanError::new(
                path,
                format!("axial plane z = {zt:e} misses the blade surface [{z0:e}, {z1:e}]"),
            ));
        }
        let (mut a, mut b) = (self.x_min, 1.0);
        for _ in 0..200 {
            let c = 0.5 * (a + b);
            if f(c).0 < zt {
                a = c;
            } else {
                b = c;
            }
        }
        Ok(f(0.5 * (a + b)).1)
    }
}

/// Camber-line curve `s(z)`.
fn camber_s_at_z(fan: &AxialFan, st: &Station, zt: f64) -> f64 {
    let f = |x: f64| fan.camber_zs(st, x);
    let (mut a, mut b) = (0.0, 1.0);
    for _ in 0..200 {
        let c = 0.5 * (a + b);
        if f(c).0 < zt {
            a = c;
        } else {
            b = c;
        }
    }
    f(0.5 * (a + b)).1
}

fn hex_faces(p: [usize; 8]) -> Vec<Vec<usize>> {
    vec![
        vec![p[0], p[2], p[6], p[4]],
        vec![p[1], p[3], p[7], p[5]],
        vec![p[0], p[1], p[5], p[4]],
        vec![p[2], p[3], p[7], p[6]],
        vec![p[0], p[1], p[3], p[2]],
        vec![p[4], p[5], p[7], p[6]],
    ]
}

/// Build the periodic single-passage mesh.
pub fn build_sector_mesh(fan: &AxialFan, spec: &SectorSpec) -> Result<SectorMesh, Vec<FanError>> {
    let mut errs = fan.validate();
    let mut bad = |p: &str, m: String| errs.push(FanError::new(p, m));
    for (name, v) in [
        ("n_up", spec.n_up),
        ("n_chord", spec.n_chord),
        ("n_down", spec.n_down),
        ("n_mid", spec.n_mid),
        ("n_span", spec.n_span),
        ("n_gap", spec.n_gap),
    ] {
        if v < 1 {
            bad(name, "needs at least one cell".into());
        }
    }
    if spec.n_bl < 1 {
        bad("n_bl", "needs at least one boundary-layer cell".into());
    }
    if spec.n_side <= spec.n_bl {
        bad("n_side", format!("needs more than n_bl = {} cells per side", spec.n_bl));
    }
    if !(spec.growth >= 1.0 && spec.growth <= 2.0) {
        bad("growth", "must lie in [1, 2]".into());
    }
    if !(spec.trunc_le > 0.0 && spec.trunc_le < 0.3) {
        bad(
            "trunc_le",
            "must lie in (0, 0.3): the blunt leading face needs a positive height".into(),
        );
    }
    if !(spec.trunc_te > 0.0 && spec.trunc_te < 0.3) {
        bad("trunc_te", "must lie in (0, 0.3)".into());
    }
    let h1 = match spec.first_cell {
        FirstCell::Height(h) => h,
        FirstCell::YPlus { y_plus, speed, nu } => {
            if !(y_plus > 0.0 && speed > 0.0 && nu > 0.0) {
                bad("first_cell", "y_plus, speed and nu must be positive".into());
                1.0
            } else {
                let mid = fan.station_at(0.5 * (fan.r_hub + fan.r_tip));
                first_cell_height_from_yplus(y_plus, speed, nu, mid.chord)
            }
        }
    };
    if !(h1.is_finite() && h1 > 0.0) {
        bad("first_cell", "the first cell height must be positive".into());
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let g = spec.growth;
    let nb = spec.n_bl;
    let wall = Some((h1, g, nb));
    let one = |e: String, p: &str| vec![FanError::new(p, e)];

    let (n_i, n_j, n_k) = (
        spec.n_up + spec.n_chord + spec.n_down,
        2 * spec.n_side + spec.n_mid,
        spec.n_span + spec.n_gap,
    );
    let (ni1, nj1) = (n_i + 1, n_j + 1);
    let id = |i: usize, j: usize, k: usize| i + ni1 * (j + nj1 * k);

    // radial layers
    let rs_span = graded_cells(fan.r_tip - fan.r_hub, spec.n_span, wall, wall).map_err(|e| one(e, "n_span"))?;
    let rs_gap = graded_cells(fan.tip_clearance, spec.n_gap, wall, wall).map_err(|e| one(e, "n_gap"))?;
    let mut radii: Vec<f64> = rs_span.iter().map(|x| fan.r_hub + x).collect();
    radii[spec.n_span] = fan.r_tip;
    radii.extend(rs_gap[1..].iter().map(|x| fan.r_tip + x));
    *radii.last_mut().expect("layers") = fan.r_shroud();

    let mut grid = vec![[0.0; 3]; ni1 * nj1 * (n_k + 1)];
    let ang = fan.sector_angle();
    for (k, &r) in radii.iter().enumerate() {
        let path = format!("layer k={k} (r = {r:.5})");
        let st = fan.station_at(r);
        let suc = ZCurve::new(fan, st, Side::Suction, &path).map_err(|e| vec![e])?;
        let pres = ZCurve::new(fan, st, Side::Pressure, &path).map_err(|e| vec![e])?;
        let pitch = r * ang;
        // cut planes along the camber line
        let za = fan.camber_zs(&st, spec.trunc_le).0;
        let zb = fan.camber_zs(&st, 1.0 - spec.trunc_te).0;
        let nose = (
            suc.s_at_z(za, &path).map_err(|e| vec![e])?,
            pres.s_at_z(za, &path).map_err(|e| vec![e])?,
        );
        let tail = (
            suc.s_at_z(zb, &path).map_err(|e| vec![e])?,
            pres.s_at_z(zb, &path).map_err(|e| vec![e])?,
        );
        if !(za > 0.0 && zb < fan.axial_extent && zb > za) {
            return Err(one(
                format!(
                    "the blade section at r = {r:.4} spans z in [{za:.4}, {zb:.4}], outside the domain [0, {}]",
                    fan.axial_extent
                ),
                "z_le_root",
            ));
        }
        let sc_a = camber_s_at_z(fan, &st, za);
        let sc_b = camber_s_at_z(fan, &st, zb);
        // axial columns
        let up = graded_cells(za, spec.n_up, None, wall).map_err(|e| one(e, "n_up"))?;
        let ch = graded_cells(zb - za, spec.n_chord, wall, wall).map_err(|e| one(e, "n_chord"))?;
        let dn = graded_cells(fan.axial_extent - zb, spec.n_down, wall, None).map_err(|e| one(e, "n_down"))?;
        let mut zs = Vec::with_capacity(ni1);
        zs.extend(up.iter().copied());
        zs.extend(ch[1..].iter().map(|x| za + x));
        zs.extend(dn[1..].iter().map(|x| zb + x));
        zs[spec.n_up] = za;
        zs[spec.n_up + spec.n_chord] = zb;
        *zs.last_mut().expect("columns") = fan.axial_extent;
        // wall nodes of the chord columns
        let c0 = spec.n_up;
        let mut wall_lo = Vec::new();
        let mut wall_hi = Vec::new();
        for i in 0..=spec.n_chord {
            let z = zs[c0 + i];
            let (a, b) = if i == 0 {
                nose
            } else if i == spec.n_chord {
                tail
            } else {
                (
                    suc.s_at_z(z, &path).map_err(|e| vec![e])?,
                    pres.s_at_z(z, &path).map_err(|e| vec![e])?,
                )
            };
            wall_lo.push((z, a));
            wall_hi.push((z, b));
        }
        // |e_s . n| of each wall node (central differences of the wall nodes)
        let cos_inc = |w: &Vec<(f64, f64)>, i: usize| -> f64 {
            let (a, b) = (if i == 0 { 0 } else { i - 1 }, if i == w.len() - 1 { i } else { i + 1 });
            let (dz, ds) = (w[b].0 - w[a].0, w[b].1 - w[a].1);
            dz / dz.hypot(ds)
        };
        for i in 0..n_i + 1 {
            let z = zs[i];
            let open = |face: (f64, f64), xi: f64| -> (f64, f64) {
                let (h0, c) = (face.1 - face.0, 0.5 * (face.0 + face.1));
                let h = h0 + (BAND_FAR * pitch - h0).max(0.0) * xi;
                (c - 0.5 * h, c + 0.5 * h)
            };
            // the first spacing of the pitchwise lines blends from the value at the end face to h1 over 2 n_bl columns
            let blend = |d_end: f64, cols: usize| -> f64 {
                let w = (1.0 - cols as f64 / (2 * nb) as f64).max(0.0);
                h1 + (d_end - h1) * w
            };
            let (e_lo, e_hi, d_lo, d_hi) = if i < c0 {
                let (a, b) = open(nose, (za - z) / za);
                let e = (h1 / cos_inc(&wall_lo, 0), h1 / cos_inc(&wall_hi, 0));
                (a, b, blend(e.0, c0 - i), blend(e.1, c0 - i))
            } else if i > c0 + spec.n_chord {
                let (a, b) = open(tail, (z - zb) / (fan.axial_extent - zb));
                let e = (
                    h1 / cos_inc(&wall_lo, spec.n_chord),
                    h1 / cos_inc(&wall_hi, spec.n_chord),
                );
                (
                    a,
                    b,
                    blend(e.0, i - c0 - spec.n_chord),
                    blend(e.1, i - c0 - spec.n_chord),
                )
            } else {
                let c = i - c0;
                (
                    wall_lo[c].1,
                    wall_hi[c].1,
                    h1 / cos_inc(&wall_lo, c),
                    h1 / cos_inc(&wall_hi, c),
                )
            };
            let sc = if i <= c0 {
                sc_a
            } else if i >= c0 + spec.n_chord {
                sc_b
            } else {
                camber_s_at_z(fan, &st, z)
            };
            let (seam_lo, seam_hi) = (sc - 0.5 * pitch, sc + 0.5 * pitch);
            let ctx = format!("n_side (column {i}, layer {k})");
            let lo = wall_line(e_lo - seam_lo, spec.n_side, d_lo, h1, g, nb, true).map_err(|e| {
                one(
                    format!("{e}; the blade does not fit between its seams or the boundary layer is too thick"),
                    &ctx,
                )
            })?;
            let hi = wall_line(seam_hi - e_hi, spec.n_side, d_hi, h1, g, nb, false).map_err(|e| {
                one(
                    format!("{e}; the blade does not fit between its seams or the boundary layer is too thick"),
                    &ctx,
                )
            })?;
            let mut s_nodes = Vec::with_capacity(nj1);
            s_nodes.extend(lo.iter().map(|x| seam_lo + x));
            for m in 1..=spec.n_mid {
                s_nodes.push(e_lo + (e_hi - e_lo) * m as f64 / spec.n_mid as f64);
            }
            s_nodes.extend(hi[1..].iter().map(|x| e_hi + x));
            s_nodes[spec.n_side] = e_lo;
            s_nodes[spec.n_side + spec.n_mid] = e_hi;
            s_nodes[0] = seam_lo;
            s_nodes[n_j] = seam_hi;
            for (j, s) in s_nodes.iter().enumerate() {
                let th = s / r;
                grid[id(i, j, k)] = [r * th.cos(), r * th.sin(), z];
            }
        }
    }

    // cells
    let removed_cell = |i: usize, j: usize, k: usize| {
        spec.include_blade
            && i >= spec.n_up
            && i < spec.n_up + spec.n_chord
            && j >= spec.n_side
            && j < spec.n_side + spec.n_mid
            && k < spec.n_span
    };
    let mut used = vec![false; grid.len()];
    let mut cells_raw: Vec<[usize; 8]> = Vec::new();
    let mut cell_ijk = Vec::new();
    let mut removed = Vec::new();
    for k in 0..n_k {
        for j in 0..n_j {
            for i in 0..n_i {
                if removed_cell(i, j, k) {
                    removed.push([i, j, k]);
                    continue;
                }
                let mut p = [0usize; 8];
                for (c, pc) in p.iter_mut().enumerate() {
                    *pc = id(i + (c & 1), j + ((c >> 1) & 1), k + (c >> 2));
                    used[*pc] = true;
                }
                cells_raw.push(p);
                cell_ijk.push([i, j, k]);
            }
        }
    }
    let mut remap = vec![usize::MAX; grid.len()];
    let mut points = Vec::new();
    for (n, u) in used.iter().enumerate() {
        if *u {
            remap[n] = points.len();
            points.push(grid[n]);
        }
    }
    let mut cells = Vec::with_capacity(cells_raw.len());
    let names = ["inlet", "outlet", "seam_lo", "seam_hi", "hub", "shroud", "blade"];
    let mut patch_faces: Vec<Vec<Vec<usize>>> = vec![Vec::new(); names.len()];
    let exists = |i: isize, j: isize, k: isize| -> Option<bool> {
        if i < 0 || j < 0 || k < 0 || i >= n_i as isize || j >= n_j as isize || k >= n_k as isize {
            None
        } else {
            Some(!removed_cell(i as usize, j as usize, k as usize))
        }
    };
    for (p, ijk) in cells_raw.iter().zip(&cell_ijk) {
        let q = p.map(|n| remap[n]);
        let faces = hex_faces(q);
        let (i, j, k) = (ijk[0] as isize, ijk[1] as isize, ijk[2] as isize);
        let nbrs = [
            (i - 1, j, k),
            (i + 1, j, k),
            (i, j - 1, k),
            (i, j + 1, k),
            (i, j, k - 1),
            (i, j, k + 1),
        ];
        // patch index per direction when outside the grid: inlet, outlet, seam_lo, seam_hi, hub, shroud
        for (d, &(a, b, c)) in nbrs.iter().enumerate() {
            match exists(a, b, c) {
                None => patch_faces[d].push(faces[d].clone()),
                Some(false) => patch_faces[6].push(faces[d].clone()),
                Some(true) => {}
            }
        }
        cells.push(faces);
    }
    let kinds = [
        PatchKind::Inlet,
        PatchKind::Outlet,
        PatchKind::Generic,
        PatchKind::Generic,
        PatchKind::Wall,
        PatchKind::Wall,
        PatchKind::Wall,
    ];
    let specs: Vec<BoundarySpec> = names
        .iter()
        .zip(kinds)
        .zip(patch_faces)
        .filter(|((_, _), f)| !f.is_empty())
        .map(|((n, k), f)| BoundarySpec {
            name: (*n).into(),
            kind: k,
            faces: f,
        })
        .collect();
    let mut mesh = Mesh::from_cell_faces(points, &cells, &specs).map_err(|e| one(e.to_string(), "mesh"))?;
    mesh.set_periodic_rotation("seam_lo", "seam_hi", [0.0; 3], [0.0, 0.0, 1.0], ang)
        .map_err(|e| one(e.to_string(), "mesh"))?;
    mesh.add_cell_zone("rotor", (0..mesh.n_cells()).collect())
        .map_err(|e| one(e.to_string(), "mesh"))?;
    if let Some(c) = mesh.cell_volumes().iter().position(|&v| !(v > 0.0)) {
        let ijk = cell_ijk.get(c).copied().unwrap_or([0; 3]);
        return Err(one(
            format!(
                "cell {c} (i, j, k) = ({}, {}, {}) has non-positive volume: the blade is too strongly twisted or staggered for the mesh controls",
                ijk[0], ijk[1], ijk[2]
            ),
            "mesh",
        ));
    }
    Ok(SectorMesh {
        mesh,
        grid,
        n_i,
        n_j,
        n_k,
        n_up: spec.n_up,
        n_chord: spec.n_chord,
        n_side: spec.n_side,
        n_mid: spec.n_mid,
        n_span: spec.n_span,
        n_bl: nb,
        growth: g,
        h1,
        radii,
        removed,
    })
}
