//! Body-fitted C-grid periodic single-passage hexahedral mesh of an [`AxialFan`] (wave 33A, ADR 0068 with its amendments).
//!
//! Every radial node layer carries the same 2-D multi-block grid in the unrolled plane `(z, s = r theta)`:
//!
//! * a **C block** whose grid lines `i` run around the blade (lower seam, suction side, leading edge, pressure side,
//!   upper seam, with the wake cut carried downstream as interior faces) and whose lines `j` leave the wall in `n_side`
//!   cells: `n_bl` cells of exactly geometric height `h1 g^q` along the in-plane wall normal, then cells that grow by a
//!   constant ratio and stop growing at 1.5 times the mean of the remaining cells. Each line leaves the stack along the
//!   wall normal and turns smoothly to aim at its target on a seam or on the front edge (a "pursuit" line);
//! * an **F block** between the inlet and the front edge of the C block;
//! * a **core block** inside the blade section, present only in the tip gap (the cells below the blade tip are deleted).
//!   The last chord cell (the trailing-edge wedge) is one polyhedral cell across the thickness.
//!
//! The upper seam is the lower seam translated by the pitch `2 pi r / N` in `s` with the identical node list, so the two
//! seams match under the rotation `2 pi / N` to round-off. The blade is the exact section: no truncation, closed trailing
//! edge (a single node), the leading and trailing nodes collapse the core columns to prism cells.
//!
//! Measured quality and its limits (the fan around the leading edge is oblique when the front edge is close, so the
//! non-orthogonality gate of ADR 0068 is missed; skewness is dominated by the flat facets of annular cells next to
//! the hub and shroud when the first cell is much thinner than the sagitta of a cell) are in the ADR 0068 addendum
//! and `docs/meshing.md`. A specification whose line family folds is rejected with a located error.

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::fan::{AxialFan, FanError, Side};
use crate::fan_mesh::{first_cell_height_from_yplus, graded_cells, quality_summary, FirstCell, QualitySummary};
use crate::mesh::{BoundarySpec, Mesh, PatchKind};
use rcfd_core::vec3::Vec3;
use std::f64::consts::PI;

/// Mesh controls of the C-grid generator.
#[derive(Debug, Clone, PartialEq)]
pub struct CGridSpec {
    /// Axial cells of the F block (inlet to the front edge).
    pub n_up: usize,
    /// Cells along each side of the blade (leading to trailing edge).
    pub n_chord: usize,
    /// Cells in the wake (trailing edge to the outlet).
    pub n_down: usize,
    /// Cells from the wall (or wake line) to the seam, boundary-layer cells included.
    pub n_side: usize,
    /// Cells per side around the nose that belong to the front edge (fan of lines to the F block).
    pub n_nose: usize,
    /// Cells across the thickness of the core block (tip gap above the blade).
    pub n_core: usize,
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
    /// Distance of the front edge ahead of the leading edge, in local chords.
    pub front_gap: f64,
    /// `false` keeps the core cells below the tip as well (an empty passage with the same nodes).
    pub include_blade: bool,
}

/// Index arithmetic of the node layout of one layer (C block, F block, interior core nodes).
#[derive(Debug, Clone, Copy)]
struct Layout {
    n_down: usize,
    n_c: usize,
    n_nose: usize,
    n_side: usize,
    n_up: usize,
    n_core: usize,
}

impl Layout {
    fn h(&self) -> usize {
        self.n_down + self.n_c
    }
    fn n_i(&self) -> usize {
        2 * self.h()
    }
    fn h_c(&self) -> usize {
        self.h() - self.n_nose
    }
    fn n_fe(&self) -> usize {
        2 * self.n_nose
    }
    fn n_c_nodes(&self) -> usize {
        (self.n_i() + 1) * (self.n_side + 1)
    }
    fn n_f_nodes(&self) -> usize {
        (self.n_up + 1) * (self.n_fe() + 1)
    }
    fn n_core_nodes(&self) -> usize {
        (self.n_c + 1) * (self.n_core + 1)
    }
    fn stride(&self) -> usize {
        self.n_c_nodes() + self.n_f_nodes() + self.n_core_nodes()
    }
    /// C-block node; the wake cut (`j = 0`, wake indices on both sides) is one set of nodes.
    fn c(&self, i: usize, j: usize, k: usize) -> usize {
        let (n_i, nd) = (self.n_i(), self.n_down);
        let ic = if j == 0 && i >= n_i - nd { n_i - i } else { i };
        k * self.stride() + ic + (n_i + 1) * j
    }
    /// F-block node; the last row is the front edge of the C block.
    fn f(&self, a: usize, b: usize, k: usize) -> usize {
        if a == self.n_up {
            self.c(self.h_c() + b, self.n_side, k)
        } else {
            k * self.stride() + self.n_c_nodes() + a + (self.n_up + 1) * b
        }
    }
    /// Core-block node; the wall columns are C-block wall nodes and the end columns collapse.
    fn core(&self, m: usize, t: usize, k: usize) -> usize {
        if m == 0 || m == self.n_c || t == 0 {
            return self.c(self.h() - m, 0, k);
        }
        if t == self.n_core {
            return self.c(self.h() + m, 0, k);
        }
        k * self.stride() + self.n_c_nodes() + self.n_f_nodes() + m + (self.n_c + 1) * t
    }
}

/// The generated C-grid mesh with its bookkeeping.
#[derive(Debug, Clone)]
pub struct CGridMesh {
    /// The polyhedral mesh (cell zone `rotor`, cyclic pair `seam_lo` / `seam_hi`).
    pub mesh: Mesh,
    /// All layout points (unused duplicates stay at the origin); see [`CGridMesh::c_node`] and friends.
    pub grid: Vec<Vec3>,
    lay: Layout,
    /// Radial cell layers (`n_span + n_gap`).
    pub n_k: usize,
    /// Cells below the blade tip.
    pub n_span: usize,
    /// Boundary-layer cells.
    pub n_bl: usize,
    /// Growth ratio.
    pub growth: f64,
    /// First cell height used.
    pub h1: f64,
    /// Radius of each node layer.
    pub radii: Vec<f64>,
    /// Per layer: the in-plane unit normal `(n_z, n_s)` used for the stack of each C-block line `i`.
    pub normals: Vec<Vec<[f64; 2]>>,
    /// Per layer: number of exact stack cells on each line `i` (`n_bl` on the blade, 0 or `n_bl` in the wake).
    pub stack_cells: Vec<Vec<usize>>,
}

impl CGridMesh {
    /// Cells around the C block.
    pub fn n_i(&self) -> usize {
        self.lay.n_i()
    }
    /// Cells from the wall to the seam.
    pub fn n_side(&self) -> usize {
        self.lay.n_side
    }
    /// Wake cells per side.
    pub fn n_down(&self) -> usize {
        self.lay.n_down
    }
    /// Cells per side along the blade.
    pub fn n_chord(&self) -> usize {
        self.lay.n_c
    }
    /// Front-edge cells per side.
    pub fn n_nose(&self) -> usize {
        self.lay.n_nose
    }
    /// Axial cells of the F block.
    pub fn n_up(&self) -> usize {
        self.lay.n_up
    }
    /// Cells across the thickness of the core block.
    pub fn n_core(&self) -> usize {
        self.lay.n_core
    }
    /// C-block node `(i, j, k)` as an index into [`CGridMesh::grid`].
    pub fn c_node(&self, i: usize, j: usize, k: usize) -> usize {
        self.lay.c(i, j, k)
    }
    /// F-block node `(a, b, k)`.
    pub fn f_node(&self, a: usize, b: usize, k: usize) -> usize {
        self.lay.f(a, b, k)
    }
    /// Core-block node `(m, t, k)`.
    pub fn core_node(&self, m: usize, t: usize, k: usize) -> usize {
        self.lay.core(m, t, k)
    }
    /// Wall node `ch` (0 = trailing edge on the suction side, `n_chord` = leading edge, `2 n_chord` = trailing edge on the
    /// pressure side) of layer `k`.
    pub fn wall_node(&self, ch: usize, k: usize) -> usize {
        self.lay.c(self.lay.n_down + ch, 0, k)
    }
    /// Node ids of the lower seam (`seam_lo`) of layer `k`, ordered from the outlet to the inlet, and of the upper seam in the
    /// same order (equal length): the upper seam node `x` is the lower seam node `x` rotated by `2 pi / N`.
    pub fn seam_nodes(&self, k: usize) -> (Vec<usize>, Vec<usize>) {
        let l = &self.lay;
        let (mut lo, mut hi) = (Vec::new(), Vec::new());
        for h in 0..=l.h_c() {
            lo.push(l.c(h, l.n_side, k));
            hi.push(l.c(l.n_i() - h, l.n_side, k));
        }
        for a in (0..l.n_up).rev() {
            lo.push(l.f(a, 0, k));
            hi.push(l.f(a, l.n_fe(), k));
        }
        (lo, hi)
    }
    /// Quality distribution of the mesh.
    pub fn quality(&self) -> QualitySummary {
        quality_summary(&self.mesh)
    }
}

fn hex_faces(p: [usize; 8]) -> [[usize; 4]; 6] {
    [
        [p[0], p[2], p[6], p[4]],
        [p[1], p[3], p[7], p[5]],
        [p[0], p[1], p[5], p[4]],
        [p[2], p[3], p[7], p[6]],
        [p[0], p[1], p[3], p[2]],
        [p[4], p[5], p[7], p[6]],
    ]
}

/// Remove repeated nodes of a face (collapsed edges); fewer than three distinct nodes is no face.
fn clean_face(f: [usize; 4]) -> Option<Vec<usize>> {
    let mut v: Vec<usize> = Vec::with_capacity(4);
    for &n in &f {
        if v.last() != Some(&n) {
            v.push(n);
        }
    }
    while v.len() > 1 && v.first() == v.last() {
        v.pop();
    }
    if v.len() >= 3 {
        Some(v)
    } else {
        None
    }
}

/// `n + 1` offsets `0 .. len` of cells that start at `first`, grow by a constant ratio and stop growing at
/// `CAP_FACTOR` times the mean cell (the ratio is solved). When `first` is already above the cap the cells are uniform.
fn capped_cells(len: f64, n: usize, first: f64, from_high_end: bool) -> Result<Vec<f64>, String> {
    if n == 0 || !(len > 0.0) || !(first > 0.0) {
        return Err(format!(
            "needs a positive length, first cell and at least one cell (length {len:e}, first {first:e}, {n} cells)"
        ));
    }
    let mean = len / n as f64;
    let cap = CAP_FACTOR * mean;
    let sizes = |rho: f64| -> Vec<f64> { (0..n).map(|m| (first * rho.powi(m as i32)).min(cap)).collect() };
    let h = if first >= mean {
        vec![mean; n]
    } else {
        let (mut a, mut b) = (1.0, 1e3);
        for _ in 0..200 {
            let c = 0.5 * (a + b);
            if sizes(c).iter().sum::<f64>() > len {
                b = c;
            } else {
                a = c;
            }
        }
        let h = sizes(0.5 * (a + b));
        let sc = len / h.iter().sum::<f64>();
        h.iter().map(|v| v * sc).collect()
    };
    let mut x = vec![0.0; n + 1];
    for m in 0..n {
        let hm = if from_high_end { h[n - 1 - m] } else { h[m] };
        x[m + 1] = x[m] + hm;
    }
    x[n] = len;
    Ok(x)
}

/// `n + 1` offsets `0 .. len` with `nb` exact boundary-layer cells `h1 g^m` at both ends and, between them, cells that
/// grow by a constant ratio from both ends and stop growing at `CAP_FACTOR` times the mean of the middle cells.
fn graded_two_sided(len: f64, n: usize, h1: f64, g: f64, nb: usize) -> Result<Vec<f64>, String> {
    if n < 2 * nb + 1 || !(len > 0.0) {
        return Err(format!(
            "{n} cells cannot hold two stacks of {nb} cells plus a middle (length {len:e})"
        ));
    }
    let bl: Vec<f64> = (0..nb).map(|m| h1 * g.powi(m as i32)).collect();
    let l_mid = len - 2.0 * bl.iter().sum::<f64>();
    let n_mid = n - 2 * nb;
    if !(l_mid > 0.0) {
        return Err(format!(
            "the boundary-layer stacks (total {:e}) fill the whole length {len:e}; reduce n_bl or the first cell height",
            2.0 * bl.iter().sum::<f64>()
        ));
    }
    let first = h1 * g.powi(nb as i32);
    let mean = l_mid / n_mid as f64;
    let cap = CAP_FACTOR * mean;
    let sizes = |rho: f64| -> Vec<f64> {
        (0..n_mid)
            .map(|m| {
                (first * rho.powi(m as i32))
                    .min(first * rho.powi((n_mid - 1 - m) as i32))
                    .min(cap)
            })
            .collect()
    };
    let mid = if first >= mean {
        vec![mean; n_mid]
    } else {
        let (mut a, mut b) = (1.0, 1e3);
        for _ in 0..200 {
            let c = 0.5 * (a + b);
            if sizes(c).iter().sum::<f64>() > l_mid {
                b = c;
            } else {
                a = c;
            }
        }
        let h = sizes(0.5 * (a + b));
        let sc = l_mid / h.iter().sum::<f64>();
        h.iter().map(|v| v * sc).collect()
    };
    let mut h: Vec<f64> = bl.clone();
    h.extend(mid);
    h.extend(bl.iter().rev());
    let mut x = vec![0.0; n + 1];
    for m in 0..n {
        x[m + 1] = x[m] + h[m];
    }
    x[n] = len;
    Ok(x)
}

/// Largest cell as a multiple of the mean cell in the outer parts of a line (see [`capped_cells`]).
const CAP_FACTOR: f64 = 1.5;

/// Target aspect ratio of the wake stack rows (axial spacing over the first height).
const WAKE_ASPECT: f64 = 300.0;

/// Union of faces that share edges (each shared edge is traversed in opposite directions): the outline polygon.
fn merge_faces(faces: &[Vec<usize>]) -> Option<Vec<usize>> {
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for f in faces {
        for m in 0..f.len() {
            edges.push((f[m], f[(m + 1) % f.len()]));
        }
    }
    let all = edges.clone();
    edges.retain(|&(a, b)| !all.contains(&(b, a)));
    let mut poly = vec![edges.first()?.0];
    let mut cur = edges[0].1;
    let mut guard = 0;
    while cur != poly[0] {
        poly.push(cur);
        cur = edges.iter().find(|e| e.0 == cur)?.1;
        guard += 1;
        if guard > edges.len() + 1 {
            return None;
        }
    }
    if poly.len() == edges.len() {
        Some(poly)
    } else {
        None
    }
}

fn norm2(a: (f64, f64)) -> f64 {
    a.0.hypot(a.1)
}

/// Geometry of one layer in the unrolled plane, written into `grid` (layout ids).
#[allow(clippy::too_many_arguments)]
fn build_layer(
    fan: &AxialFan,
    spec: &CGridSpec,
    lay: &Layout,
    k: usize,
    r: f64,
    h1: f64,
    grid: &mut [Vec3],
    normals: &mut Vec<[f64; 2]>,
    stack_cells: &mut Vec<usize>,
) -> Result<(), FanError> {
    let ctx = format!("layer k={k} (r = {r:.5})");
    let err = |path: &str, m: String| FanError::new(path, format!("{m} [{ctx}]"));
    let st = fan.station_at(r);
    let pitch = r * fan.sector_angle();
    let (nd, nc, nn, ns, nb) = (lay.n_down, lay.n_c, lay.n_nose, lay.n_side, spec.n_bl);
    let (h, n_i, h_c, n_fe) = (lay.h(), lay.n_i(), lay.h_c(), lay.n_fe());
    let g = spec.growth;
    let chord = st.chord;

    // chord stations and section points
    let xs: Vec<f64> = (0..=nc)
        .map(|m| {
            let u = m as f64 / nc as f64;
            0.85 * 0.5 * (1.0 - (PI * u).cos()) + 0.15 * u
        })
        .collect();
    let sp: Vec<(f64, f64)> = xs.iter().map(|&x| fan.surface_zs(&st, x, Side::Suction)).collect();
    let mut pp: Vec<(f64, f64)> = xs.iter().map(|&x| fan.surface_zs(&st, x, Side::Pressure)).collect();
    pp[0] = sp[0];
    pp[nc] = sp[nc];
    let z_te = sp[nc].0;
    let s_te = sp[nc].1;
    let s_le = sp[0].1;
    let z_min = sp.iter().chain(pp.iter()).map(|p| p.0).fold(f64::INFINITY, f64::min);
    let z_f = z_min - spec.front_gap * chord;
    if !(z_f > 0.0) {
        return Err(err(
            "z_le_root",
            format!(
                "the front edge z = {z_f:.4} (leading edge {z_min:.4} minus {} chords) is not inside the domain: increase z_le_root or reduce front_gap",
                spec.front_gap
            ),
        ));
    }
    if !(z_te < fan.axial_extent) {
        return Err(err(
            "z_le_root",
            format!(
                "the trailing edge z = {z_te:.4} is beyond the outlet {}",
                fan.axial_extent
            ),
        ));
    }
    let s_centre = 0.5 * (s_le + s_te);
    let s_lo = s_centre - 0.5 * pitch;
    let s_hi = s_lo + pitch;
    let (s_wmin, s_wmax) = sp
        .iter()
        .chain(pp.iter())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |a, p| (a.0.min(p.1), a.1.max(p.1)));
    if !(s_wmin - s_lo > 0.0 && s_hi - s_wmax > 0.0) {
        return Err(err(
            "n_side",
            format!(
                "the blade (s from {s_wmin:.4} to {s_wmax:.4}) does not fit between its seams ({s_lo:.4}, {s_hi:.4}): pitch {pitch:.4}"
            ),
        ));
    }

    // wake line: leaves along the pressure-surface tangent, bends to axial
    let pt = fan.surface_zs(&st, 1.0 - 1e-5, Side::Pressure);
    let dzt = z_te - pt.0;
    if !(dzt > 0.0) {
        return Err(err(
            "stagger",
            "the pressure surface does not run downstream at the trailing edge".into(),
        ));
    }
    let sigma0 = (s_te - pt.1) / dzt;
    let lam_w = 0.2 * chord;
    let s_wake = |z: f64| s_te + sigma0 * lam_w * (1.0 - (-(z - z_te) / lam_w).exp());
    let s_inf = s_te + sigma0 * lam_w;
    if !(s_inf - s_lo > 0.0 && s_hi - s_inf > 0.0) {
        return Err(err(
            "n_down",
            format!("the wake line (s from {s_te:.4} to {s_inf:.4}) leaves the passage ({s_lo:.4}, {s_hi:.4})"),
        ));
    }
    let dz_te = (z_te - sp[nc - 1].0).abs().max(1e-12);
    // wake axial cells start at the trailing-edge spacing and stop growing at twice the mean cell
    let len_w = fan.axial_extent - z_te;
    let zz = capped_cells(len_w, nd, dz_te, false).map_err(|e| err("n_down", e))?;
    let wk: Vec<(f64, f64)> = zz
        .iter()
        .map(|d| {
            let z = z_te + d;
            (z, s_wake(z))
        })
        .collect();

    // wall-and-wake curve
    let w: Vec<(f64, f64)> = (0..=n_i)
        .map(|i| {
            if i <= nd {
                wk[nd - i]
            } else if i <= h {
                sp[nc - (i - nd)]
            } else if i <= n_i - nd {
                pp[i - h]
            } else {
                wk[i - (n_i - nd)]
            }
        })
        .collect();
    normals.clear();
    for i in 0..=n_i {
        let (a, b) = if i == 0 {
            (0, 1)
        } else if i == n_i {
            (n_i - 1, n_i)
        } else {
            (i - 1, i + 1)
        };
        let t = (w[b].0 - w[a].0, w[b].1 - w[a].1);
        let l = norm2(t);
        if !(l > 0.0) {
            return Err(err(
                "n_chord",
                format!("coincident neighbouring wall nodes at line {i}"),
            ));
        }
        normals.push([-t.1 / l, t.0 / l]);
    }
    // the outlet lines stay in the plane z = axial_extent: their stack runs pitchwise, with the sign of the wake normal
    for i in [0, n_i] {
        let sg = if normals[i][1] < 0.0 { -1.0 } else { 1.0 };
        normals[i] = [0.0, sg];
    }

    // stacks
    let growth_sum = |d1: f64, n: usize| -> f64 { (0..n).map(|q| d1 * g.powi(q as i32)).sum() };
    let mut d1s = vec![h1; n_i + 1];
    stack_cells.clear();
    for i in 0..=n_i {
        let hh = i.min(n_i - i);
        if i >= nd && i <= n_i - nd {
            stack_cells.push(nb);
        } else {
            let p = nd - hh;
            let up = if p < nd { zz[p + 1] - zz[p] } else { 0.0 };
            let dn = if p > 0 { zz[p] - zz[p - 1] } else { 0.0 };
            let dz = up.max(dn);
            let dm = (w[i].1 - s_lo).min(s_hi - w[i].1);
            // the first height grows with the local axial spacing (by at most the cell growth per line), so
            // that the rows stay aligned between neighbouring lines and the wake aspect stays bounded
            let d1w = h1.max(dz / WAKE_ASPECT);
            d1s[i] = d1w;
            stack_cells.push(if growth_sum(d1w, nb) <= 0.25 * dm { nb } else { 0 });
        }
    }
    let stack_pt = |i: usize, q: usize| -> (f64, f64) {
        let y: f64 = growth_sum(d1s[i], q);
        (w[i].0 + normals[i][0] * y, w[i].1 + normals[i][1] * y)
    };
    let edge: Vec<(f64, f64)> = (0..=n_i).map(|i| stack_pt(i, stack_cells[i])).collect();

    // seam points
    let z_avg: Vec<f64> = (0..=h).map(|hh| 0.5 * (edge[hh].0 + edge[n_i - hh].0)).collect();
    let q_c = z_avg[h_c];
    let lam = 0.25 * chord;
    if !(q_c > z_f) {
        return Err(err(
            "n_nose",
            format!("the corner station (z = {q_c:.4}) is not downstream of the front edge ({z_f:.4})"),
        ));
    }
    let z_out: Vec<f64> = (0..=h_c)
        .map(|hh| {
            if hh == h_c {
                z_f
            } else if hh == 0 {
                fan.axial_extent
            } else {
                z_avg[hh] - (q_c - z_f) * (-(z_avg[hh] - q_c) / lam).exp()
            }
        })
        .collect();
    for hh in 0..h_c {
        if !(z_out[hh] > z_out[hh + 1]) {
            return Err(err(
                "n_chord",
                format!(
                    "the seam points of stations {hh} and {} are not ordered (tangled lines)",
                    hh + 1
                ),
            ));
        }
    }
    if !(z_out[0] <= fan.axial_extent + 1e-12 * fan.axial_extent) {
        return Err(err("n_down", "the outlet seam point lies beyond the outlet".into()));
    }
    let d_corner = z_out[h_c - 1] - z_f;
    // front-edge s nodes
    let uniform = |len: f64, n: usize| -> Vec<f64> { (0..=n).map(|q| len * q as f64 / n as f64).collect() };
    let _ = d_corner;
    // front-edge cells start at the nose with a few wall cells' width and grow towards the seam corners
    let d_nose = 3.0 * norm2((sp[1].0 - sp[0].0, sp[1].1 - sp[0].1));
    let lo_part = capped_cells(s_le - s_lo, nn, d_nose, true).unwrap_or_else(|_| uniform(s_le - s_lo, nn));
    let hi_part = capped_cells(s_hi - s_le, nn, d_nose, false).unwrap_or_else(|_| uniform(s_hi - s_le, nn));
    let mut s_front = vec![0.0; n_fe + 1];
    for b in 0..=nn {
        s_front[b] = s_lo + lo_part[b];
    }
    for b in 1..=nn {
        s_front[nn + b] = s_le + hi_part[b];
    }
    s_front[n_fe] = s_hi;
    let z_f_nodes = graded_cells(z_f, spec.n_up, None, Some((d_corner, g, 0))).map_err(|e| err("n_up", e))?;

    let radius = r;
    let mut put = |id: usize, z: f64, s: f64| {
        let th = s / radius;
        grid[id] = [radius * th.cos(), radius * th.sin(), z];
    };
    // C block (in-plane points, written to the grid after smoothing)
    let mut cp: Vec<Vec<(f64, f64)>> = vec![vec![(0.0, 0.0); ns + 1]; n_i + 1];
    for i in 0..=n_i {
        let hh = i.min(n_i - i);
        let o = if i < h_c {
            (z_out[hh], s_lo)
        } else if i > n_i - h_c {
            (z_out[hh], s_hi)
        } else if i == h_c {
            (z_f, s_lo)
        } else if i == n_i - h_c {
            (z_f, s_hi)
        } else {
            (z_f, s_front[i - h_c])
        };
        let nbi = stack_cells[i];
        let e = edge[i];
        let len = norm2((o.0 - e.0, o.1 - e.1));
        let first = d1s[i] * g.powi(nbi as i32);
        if ns <= nbi {
            return Err(err(
                "n_side",
                format!("needs more than the {nbi} stack cells on line {i}"),
            ));
        }
        let rest = capped_cells(len, ns - nbi, first, false).map_err(|m| {
            err(
                "n_side",
                format!(
                    "line {i}: {m}; the stack (thickness {:.3e}) leaves no room before the seam",
                    norm2((e.0 - w[i].0, e.1 - w[i].1))
                ),
            )
        })?;
        for q in 0..=nbi {
            let p = stack_pt(i, q);
            cp[i][q] = p;
        }
        // Pursuit line: it leaves the stack edge along the wall normal and turns smoothly to aim at
        // its target, which it reaches exactly. The step lengths keep the fractions `rest[q] / len`
        // of the remaining distance, so every line has the same row structure.
        let nr = ns - nbi;
        let mut cur = e;
        for q in 1..=nr {
            let f0 = rest[q - 1] / len;
            let f1 = rest[q] / len;
            let (tx, ty) = (o.0 - cur.0, o.1 - cur.1);
            let rem = norm2((tx, ty));
            let (ux, uy) = if rem > 0.0 { (tx / rem, ty / rem) } else { (0.0, 0.0) };
            let x = (q - 1) as f64 / (0.7 * nr as f64);
            let x = x.clamp(0.0, 1.0);
            // lines whose normal already points within 30 degrees of the target aim at it at once
            // (a normal departure would make them drift sideways past their neighbours)
            let cos_t = (o.0 - e.0) * normals[i][0] / len + (o.1 - e.1) * normals[i][1] / len;
            let th = cos_t.clamp(-1.0, 1.0).acos();
            let xt = ((th - PI / 6.0) / (PI / 6.0)).clamp(0.0, 1.0);
            let wt = xt * xt * (3.0 - 2.0 * xt);
            let beta = if i == 0 || i == n_i {
                1.0
            } else {
                1.0 - wt * (1.0 - x * x * (3.0 - 2.0 * x))
            };
            let (mut dx, mut dy) = (
                (1.0 - beta) * normals[i][0] + beta * ux,
                (1.0 - beta) * normals[i][1] + beta * uy,
            );
            let dl = norm2((dx, dy));
            if dl > 1e-12 {
                dx /= dl;
                dy /= dl;
            } else {
                dx = ux;
                dy = uy;
            }
            let step = if q == nr { rem } else { (f1 - f0) / (1.0 - f0) * rem };
            let nxt = if q == nr {
                o
            } else {
                (cur.0 + dx * step, cur.1 + dy * step)
            };
            cp[i][nbi + q] = nxt;
            cur = nxt;
        }
    }
    for (i, col) in cp.iter().enumerate() {
        for (j, p) in col.iter().enumerate() {
            put(lay.c(i, j, k), p.0, p.1);
        }
    }
    // F block (rows below the front edge)
    for a in 0..spec.n_up {
        for b in 0..=n_fe {
            put(lay.f(a, b, k), z_f_nodes[a], s_front[b]);
        }
    }
    // core interior nodes
    for m in 1..nc {
        for t in 1..lay.n_core {
            let f = t as f64 / lay.n_core as f64;
            put(
                lay.core(m, t, k),
                sp[m].0 + (pp[m].0 - sp[m].0) * f,
                sp[m].1 + (pp[m].1 - sp[m].1) * f,
            );
        }
    }
    Ok(())
}

/// Build the periodic single-passage C-grid mesh.
pub fn build_cgrid_mesh(fan: &AxialFan, spec: &CGridSpec) -> Result<CGridMesh, Vec<FanError>> {
    let mut errs = fan.validate();
    let mut bad = |p: &str, m: String| errs.push(FanError::new(p, m));
    for (name, v) in [
        ("n_up", spec.n_up),
        ("n_chord", spec.n_chord),
        ("n_down", spec.n_down),
        ("n_nose", spec.n_nose),
        ("n_core", spec.n_core),
        ("n_span", spec.n_span),
        ("n_gap", spec.n_gap),
    ] {
        if v < 1 {
            bad(name, "needs at least one cell".into());
        }
    }
    if spec.n_nose + 2 > spec.n_chord {
        bad(
            "n_nose",
            format!("must be at most n_chord - 2 = {}", spec.n_chord.saturating_sub(2)),
        );
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
    if !(spec.front_gap > 0.0 && spec.front_gap <= 3.0) {
        bad("front_gap", "must lie in (0, 3] chords".into());
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
    let one = |e: String, p: &str| vec![FanError::new(p, e)];
    let g = spec.growth;
    let nb = spec.n_bl;
    let lay = Layout {
        n_down: spec.n_down,
        n_c: spec.n_chord,
        n_nose: spec.n_nose,
        n_side: spec.n_side,
        n_up: spec.n_up,
        n_core: spec.n_core,
    };
    let n_k = spec.n_span + spec.n_gap;
    let rs_span = graded_two_sided(fan.r_tip - fan.r_hub, spec.n_span, h1, g, nb).map_err(|e| one(e, "n_span"))?;
    let rs_gap = graded_two_sided(fan.tip_clearance, spec.n_gap, h1, g, nb).map_err(|e| one(e, "n_gap"))?;
    let mut radii: Vec<f64> = rs_span.iter().map(|x| fan.r_hub + x).collect();
    radii[spec.n_span] = fan.r_tip;
    radii.extend(rs_gap[1..].iter().map(|x| fan.r_tip + x));
    *radii.last_mut().expect("layers") = fan.r_shroud();

    let stride = lay.stride();
    let mut grid = vec![[0.0; 3]; stride * (n_k + 1)];
    let mut normals = Vec::with_capacity(n_k + 1);
    let mut stacks = Vec::with_capacity(n_k + 1);
    for (k, &r) in radii.iter().enumerate() {
        let (mut nv, mut sv) = (Vec::new(), Vec::new());
        build_layer(fan, spec, &lay, k, r, h1, &mut grid, &mut nv, &mut sv).map_err(|e| vec![e])?;
        normals.push(nv);
        stacks.push(sv);
    }

    // every C-block quad must have positive area in the unrolled plane (a folded line family is a located error)
    for (k, &r) in radii.iter().enumerate() {
        for j in 0..lay.n_side {
            for i in 0..lay.n_i() {
                let q = [
                    lay.c(i, j, k),
                    lay.c(i + 1, j, k),
                    lay.c(i + 1, j + 1, k),
                    lay.c(i, j + 1, k),
                ];
                let pts: Vec<(f64, f64)> = q
                    .iter()
                    .map(|&id| {
                        let p = grid[id];
                        (p[2], r * p[1].atan2(p[0]))
                    })
                    .collect();
                let mut a = 0.0;
                for m in 0..4 {
                    let (p0, p1) = (pts[m], pts[(m + 1) % 4]);
                    a += p0.0 * p1.1 - p1.0 * p0.1;
                }
                if !(a > 0.0) {
                    return Err(one(
                        format!(
                            "the line family is folded at layer {k} (r = {r:.5}), around line {i}, row {j}: the leading-edge fan cannot be built for this blade with the front_gap {} and n_nose {} given; reduce front_gap or change n_nose",
                            spec.front_gap, spec.n_nose
                        ),
                        "front_gap",
                    ));
                }
            }
        }
    }
    // cells
    let (n_i, ns, h_c, n_fe, nd, nc) = (lay.n_i(), lay.n_side, lay.h_c(), lay.n_fe(), lay.n_down, lay.n_c);
    let names = ["inlet", "outlet", "seam_lo", "seam_hi", "hub", "shroud", "blade"];
    let (p_in, p_out, p_lo, p_hi, p_hub, p_shr, p_blade) = (0, 1, 2, 3, 4, 5, 6);
    let mut cells_raw: Vec<[usize; 8]> = Vec::new();
    // per cell: the patch of each of its six faces (None = interior)
    let mut cell_patch: Vec<[Option<usize>; 6]> = Vec::new();
    let hubshroud = |k: usize, pat: &mut [Option<usize>; 6]| {
        if k == 0 {
            pat[4] = Some(p_hub);
        }
        if k == n_k - 1 {
            pat[5] = Some(p_shr);
        }
    };
    let mut cell_info: Vec<(u8, usize, usize, usize)> = Vec::new();
    for k in 0..n_k {
        for j in 0..ns {
            for i in 0..n_i {
                let mut p = [0usize; 8];
                for (c, pc) in p.iter_mut().enumerate() {
                    *pc = lay.c(i + (c & 1), j + ((c >> 1) & 1), k + (c >> 2));
                }
                let mut pat = [None; 6];
                if i == 0 {
                    pat[0] = Some(p_out);
                }
                if i == n_i - 1 {
                    pat[1] = Some(p_out);
                }
                if j == 0 && i >= nd && i < n_i - nd && k < spec.n_span && spec.include_blade {
                    pat[2] = Some(p_blade);
                }
                if j == ns - 1 {
                    if i < h_c {
                        pat[3] = Some(p_lo);
                    } else if i >= n_i - h_c {
                        pat[3] = Some(p_hi);
                    }
                }
                hubshroud(k, &mut pat);
                cells_raw.push(p);
                cell_patch.push(pat);
                cell_info.push((0, i, j, k));
            }
        }
        for b in 0..n_fe {
            for a in 0..spec.n_up {
                let mut p = [0usize; 8];
                for (c, pc) in p.iter_mut().enumerate() {
                    *pc = lay.f(a + (c & 1), b + ((c >> 1) & 1), k + (c >> 2));
                }
                let mut pat = [None; 6];
                if a == 0 {
                    pat[0] = Some(p_in);
                }
                if b == 0 {
                    pat[2] = Some(p_lo);
                }
                if b == n_fe - 1 {
                    pat[3] = Some(p_hi);
                }
                hubshroud(k, &mut pat);
                cells_raw.push(p);
                cell_patch.push(pat);
                cell_info.push((1, a, b, k));
            }
        }
        let k0 = if spec.include_blade { spec.n_span } else { 0 };
        if k >= k0 {
            for t in 0..lay.n_core {
                for m in 0..nc {
                    let mut p = [0usize; 8];
                    for (c, pc) in p.iter_mut().enumerate() {
                        *pc = lay.core(m + (c & 1), t + ((c >> 1) & 1), k + (c >> 2));
                    }
                    let mut pat = [None; 6];
                    if k == k0 && spec.include_blade {
                        pat[4] = Some(p_blade);
                    }
                    hubshroud(k, &mut pat);
                    cells_raw.push(p);
                    cell_patch.push(pat);
                    cell_info.push((2, m, t, k));
                }
            }
        }
    }
    let mut used = vec![false; grid.len()];
    for p in &cells_raw {
        for &n in p {
            used[n] = true;
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
    let mut cells: Vec<Vec<Vec<usize>>> = Vec::with_capacity(cells_raw.len());
    let mut cell_tags: Vec<_> = Vec::with_capacity(cells_raw.len());
    let mut patch_faces: Vec<Vec<Vec<usize>>> = vec![Vec::new(); names.len()];
    // The core cells of the last chord cell (the trailing-edge wedge) are one polyhedral cell per layer: split across the
    // thickness they would be slivers a few micrometres thick whose mutual faces have large skewness.
    let mut apex_slot: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    // radial faces of the apex cells, merged per layer after the loop: (faces, patch)
    let mut apex_bottom: std::collections::HashMap<usize, (Vec<Vec<usize>>, Option<usize>)> =
        std::collections::HashMap::new();
    let mut apex_top: std::collections::HashMap<usize, (Vec<Vec<usize>>, Option<usize>)> =
        std::collections::HashMap::new();
    for ((p, pat), tag) in cells_raw.iter().zip(&cell_patch).zip(&cell_info) {
        let q = p.map(|n| remap[n]);
        let apex = tag.0 == 2 && tag.1 == nc - 1 && lay.n_core > 1;
        let mut fl = Vec::with_capacity(6);
        for (d, f) in hex_faces(q).into_iter().enumerate() {
            if apex && ((d == 2 && tag.2 > 0) || (d == 3 && tag.2 + 1 < lay.n_core)) {
                continue;
            }
            if let Some(face) = clean_face(f) {
                if apex && (d == 4 || d == 5) {
                    let e = if d == 4 { &mut apex_bottom } else { &mut apex_top };
                    let ent = e.entry(tag.3).or_insert((Vec::new(), pat[d]));
                    ent.0.push(face);
                    continue;
                }
                if let Some(pi) = pat[d] {
                    patch_faces[pi].push(face.clone());
                }
                fl.push(face);
            }
        }
        if apex {
            if let Some(&slot) = apex_slot.get(&tag.3) {
                cells[slot].extend(fl);
                continue;
            }
            apex_slot.insert(tag.3, cells.len());
        }
        cells.push(fl);
        cell_tags.push(*tag);
    }
    for (k, slot) in &apex_slot {
        for group in [&apex_bottom, &apex_top] {
            if let Some((faces, patch)) = group.get(k) {
                let poly = if faces.len() == 1 {
                    faces[0].clone()
                } else {
                    merge_faces(faces).ok_or_else(|| {
                        one(
                            "the trailing-edge cell faces do not merge into one polygon".into(),
                            "mesh",
                        )
                    })?
                };
                if let Some(pi) = patch {
                    patch_faces[*pi].push(poly.clone());
                }
                cells[*slot].push(poly);
            }
        }
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
    let mut mesh = Mesh::from_cell_faces(points, &cells, &specs).map_err(|e| {
        let msg = e.to_string();
        // locate the cell named in the message (if any) by block and layer
        let at = msg
            .strip_prefix("mesh error: cell ")
            .or_else(|| msg.split("cell ").nth(1))
            .and_then(|t| t.split(|ch: char| !ch.is_ascii_digit()).next())
            .and_then(|t| t.parse::<usize>().ok())
            .and_then(|c| cell_tags.get(c).copied())
            .map(|(blk, a, b, k)| {
                let name = match blk {
                    0 => "C block (i, j)",
                    1 => "F block (a, b)",
                    _ => "core block (m, t)",
                };
                format!(" at {name} = ({a}, {b}), layer k = {k}")
            })
            .unwrap_or_default();
        one(
            format!(
                "{msg}{at}: the cell family is tangled for this blade with n_nose {}, n_side {}, n_chord {}, n_up {}, front_gap {};                  the controls are not compatible (change n_nose, n_side or front_gap)",
                spec.n_nose, spec.n_side, spec.n_chord, spec.n_up, spec.front_gap
            ),
            "mesh",
        )
    })?;
    mesh.set_periodic_rotation("seam_lo", "seam_hi", [0.0; 3], [0.0, 0.0, 1.0], fan.sector_angle())
        .map_err(|e| one(e.to_string(), "mesh"))?;
    mesh.add_cell_zone("rotor", (0..mesh.n_cells()).collect())
        .map_err(|e| one(e.to_string(), "mesh"))?;
    if let Some(c) = mesh.cell_volumes().iter().position(|&v| !(v > 0.0)) {
        return Err(one(
            format!("cell {c} has non-positive volume: the blade is too strongly twisted or staggered for the mesh controls"),
            "mesh",
        ));
    }
    Ok(CGridMesh {
        mesh,
        grid,
        lay,
        n_k,
        n_span: spec.n_span,
        n_bl: nb,
        growth: g,
        h1,
        radii,
        normals,
        stack_cells: stacks,
    })
}
