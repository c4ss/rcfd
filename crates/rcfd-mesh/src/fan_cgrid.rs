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
//! non-orthogonality gate of ADR 0068 is missed; the attribution of the y+ = 1 skewness, which is not the facet sagitta but the
//! thin wake-stack and end-wall cells, is in ADR 0072, diagnostics G and W) are in the ADR 0068 and 0072 addenda and
//! `docs/meshing.md`. A specification whose line family folds is rejected with a located error.
//!
//! [`build_cgrid_mesh_with`] with [`CGridOptions`] selects the wave 34A construction (ADR 0072): in-plane elliptic smoothing with
//! periodic seams and a radial remap of the far grid of the thin end-wall layers; [`build_cgrid_mesh`] keeps the construction above.

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
    /// First cell height of the tip-gap stacks (`h1` unless `CGridOptions::gap_fit` lowers it, ADR 0079).
    pub h_gap: f64,
    /// Radius of each node layer.
    pub radii: Vec<f64>,
    /// Per layer: the in-plane unit normal `(n_z, n_s)` used for the stack of each C-block line `i`.
    pub normals: Vec<Vec<[f64; 2]>>,
    /// Per layer: number of exact stack cells on each line `i` (`n_bl` on the blade, 0 or `n_bl` in the wake).
    pub stack_cells: Vec<Vec<usize>>,
    /// Per mesh cell: `(block, a, b, k)` with block 0 = C block `(i, j)`, 1 = F block `(a, b)`, 2 = core block `(m, t)`, and the
    /// radial cell layer `k` (the trailing-edge wedge of the core carries the tag of its first part).
    pub cell_tags: Vec<(u8, usize, usize, usize)>,
    /// Step of the construction chain that built the mesh (ADR 0072): 0 for the 33A construction and for the full smoothing; with
    /// smoothing 1 = one-dimensional controls, 2 = first quarter of the outer rows kept algebraic, 3 / 4 / 5 = smoothing displacement
    /// scaled by 0.5 / 0.25 / 0.
    pub construction_step: usize,
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

/// The exact controls of the algebraic grid fade out over 1 / EXACT_DIV of the outer cells of a line (ADR 0072).
const EXACT_DIV: usize = 4;

/// Pass cap of the local untangling that precedes the smoothing (ADR 0072).
const UNTANGLE_PASSES: usize = 500;

/// Laplace passes of the start grid of the last camber-seam fallback steps (ADR 0079 part 3).
const LAPLACE_PASSES: usize = 400;

/// Iteration cap of the in-plane smoothing (ADR 0072).
const SMOOTH_ITER: usize = 4000;

/// Target aspect ratio of the wake stack rows (axial spacing over the first height).
const WAKE_ASPECT: f64 = 300.0;

/// Chord samples of the camber line that carries the camber seam (ADR 0079).
const CAMBER_SAMPLES: usize = 256;

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

/// Construction options of the C-grid generator (ADR 0072). `Default` is the construction of ADR 0068 (wave 33A), unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CGridOptions {
    /// W2 of ADR 0072: one first height `h1` and the full `n_bl` stack on every wake line (33A: `max(h1, dz/300)`, and no stack
    /// where it does not fit), so that neighbouring wake-stack lines have equal heights and the thin wake cells are not wedges.
    pub uniform_wake_stack: bool,
    /// W1 of ADR 0072: the nodes away from the blade take their angle and axial position from the in-plane grid built at a remapped
    /// radius whose increments are proportional to `Delta r^2` for layers thinner than the mean span cell (no rotation of the
    /// far grid between the thin end-wall layers); the blade wall, its stacks and the wake stacks keep the true section of every
    /// layer and the outer nodes blend from one to the other over a length set by their difference. The tip gap is a radial
    /// extrusion of the tip layer.
    pub radial_remap: bool,
    /// In-plane elliptic smoothing of every layer (ADR 0072): Winslow's equations with control functions from the algebraic grid,
    /// the boundary-layer and wake stacks, the outlet lines and the inlet row held, the seams treated as periodic interior lines
    /// (both copies move together, the pair stays an exact rotation image) and the front edge as an interior interface between
    /// the C and F blocks. A folded algebraic grid is untangled locally first. If the result folds anywhere, a fixed chain of
    /// weaker settings is tried (the same setting in every layer): plain spacing controls, the first quarter of the outer rows
    /// kept algebraic, the smoothing displacement scaled by 0.5, 0.25, 0.
    pub smoothing: bool,
    /// ADR 0079 part 1: the periodic seams follow the passage centre line (the camber line inside the chord, the wake curve
    /// downstream, a C1 extension of the leading-edge camber slope that turns axial over 0.2 chord upstream) at half a pitch on either
    /// side, instead of two straight axial lines; the front edge is centred on the centre line and the F block follows it.
    pub camber_seam: bool,
    /// ADR 0079 part 1: the tip-gap first height is `min(h1, L_gap / (2 S + (n_gap - 2 n_bl) g^n_bl))` (`S` the stack sum), the largest
    /// height for which both gap stacks and a middle of continuous growth fit; the span keeps `h1`.
    pub gap_fit: bool,
    /// ADR 0079 C2: the wake first height is `max(h1, dz / 50)` instead of `max(h1, dz / 300)`.
    pub wake_aspect_50: bool,
    /// ADR 0079 C5: the stack offsets are divided by the cosine of the lean angle between the in-plane wall normal and the 3-D surface
    /// normal, so that the wall-normal thickness of every stack cell is `h1 g^q` in every layer (wake lines take the trailing-edge value).
    pub lean_stack: bool,
    /// ADR 0079 C6: in the smoothing, the nose lines keep their first `max(1, (n_side - n_bl) / 4)` rows beyond the stack algebraic.
    pub nose_rows_algebraic: bool,
    /// ADR 0079 C4: in the smoothing, the front-edge row (the C/F interface, corner included) keeps its algebraic positions.
    pub front_edge_held: bool,
    /// ADR 0079 part 1 (F3): a radial filter of the far in-plane node positions across the layers ([`radial_filter`]).
    pub radial_filter: bool,
    /// ADR 0079 part 3: a specification outside the documented control range ([`control_range_problems`]) is refused with
    /// located errors before any construction.
    pub checked_range: bool,
}

impl CGridOptions {
    /// Construction G of ADR 0079 (case value `c_grid_construction = "camber_seam"`): construction E of ADR 0072 with the seams
    /// following the blade where straight seams leave no margin, the fitted tip gap, the ADR 0079 part 2 changes accepted by its
    /// registered rule, and the checked control range.
    pub fn camber_seam_construction() -> Self {
        CGridOptions {
            radial_remap: true,
            smoothing: true,
            camber_seam: true,
            gap_fit: true,
            radial_filter: true,
            checked_range: true,
            ..CGridOptions::default()
        }
    }
}

/// Reference to a node of the smoothing graph: canonical id and the pitchwise offset of this copy.
type NodeRef = (usize, f64);

/// Elliptic (Winslow) smoothing of a layer grid in place (ADR 0072). Equations (Thompson, Warsi & Mastin, Numerical Grid
/// Generation, 1985, ch. on elliptic systems; recalled, UNVERIFIED): `alpha (x_xixi + phi x_xi) - 2 beta x_xieta + gamma
/// (x_etaeta + psi x_eta) = 0` with `alpha = |x_eta|^2`, `beta = x_xi . x_eta`, `gamma = |x_xi|^2`; the control functions
/// `phi = -(x_xi . x_xixi)/|x_xi|^2`, `psi = -(x_eta . x_etaeta)/|x_eta|^2` are taken pointwise from the algebraic grid so that
/// the spacing along every line is kept while the angles relax; with `exact_near_wall` they blend, next to the stack edge, into the
/// controls for which the algebraic grid is an exact solution. Picard iteration: line-implicit along the C-block j-lines,
/// pointwise elsewhere. The irregular seam corner (six cells in the periodic plane) is placed at the mean of its neighbours.
/// `freeze` keeps the first quarter of the outer rows of every C line algebraic.
#[allow(clippy::too_many_arguments)]
fn smooth_layer(
    l: &mut Layer2d,
    lay: &Layout,
    pitch: f64,
    max_iter: usize,
    freeze: bool,
    exact_near_wall: bool,
    nose_alg: bool,
    hold_front: bool,
    laplace_passes: usize,
) {
    let (n_i, ns, h_c, n_fe, n_up) = (lay.n_i(), lay.n_side, lay.h_c(), lay.n_fe(), lay.n_up);
    let nc_nodes = (n_i + 1) * (ns + 1);
    let cid = |i: usize, j: usize| i * (ns + 1) + j;
    let fid = |a: usize, b: usize| nc_nodes + a * (n_fe + 1) + b;
    let mut pts: Vec<(f64, f64)> = vec![(0.0, 0.0); nc_nodes + n_up * (n_fe + 1)];
    for i in 0..=n_i {
        for j in 0..=ns {
            pts[cid(i, j)] = l.c[i][j];
        }
    }
    for a in 0..n_up {
        for b in 0..=n_fe {
            pts[fid(a, b)] = l.f[a][b];
        }
    }
    let cref = |i: usize, j: usize| -> NodeRef {
        if j == ns && i >= n_i - h_c {
            (cid(n_i - i, ns), pitch)
        } else {
            (cid(i, j), 0.0)
        }
    };
    let fref = |a: usize, b: usize| -> NodeRef {
        if a == n_up {
            cref(h_c + b, ns)
        } else if b == n_fe {
            (fid(a, 0), pitch)
        } else {
            (fid(a, b), 0.0)
        }
    };
    let sh = |r: NodeRef, d: f64| -> NodeRef { (r.0, r.1 + d) };
    // regular nodes: (id, [E, W, N, S, NE, NW, SE, SW])
    let mut reg: Vec<(usize, [NodeRef; 8])> = Vec::new();
    // C-block j-lines (solved line-implicitly along j): (first entry of `reg`, count)
    let mut lines: Vec<(usize, usize)> = Vec::new();
    for i in 1..n_i {
        // the stack (and with reeze the first rows beyond it, a quarter of the outer cells) keep the algebraic wall-normal lines
        let j0 = l.stack_cells[i]
            + 1
            + if freeze {
                (ns - l.stack_cells[i]) / 4
            } else if nose_alg && i >= h_c && i <= n_i - h_c {
                ((ns - l.stack_cells[i]) / 4).max(1)
            } else {
                0
            };
        if j0 < ns {
            lines.push((reg.len(), ns - j0));
        }
        for j in j0..ns {
            reg.push((
                cid(i, j),
                [
                    cref(i + 1, j),
                    cref(i - 1, j),
                    cref(i, j + 1),
                    cref(i, j - 1),
                    cref(i + 1, j + 1),
                    cref(i - 1, j + 1),
                    cref(i + 1, j - 1),
                    cref(i - 1, j - 1),
                ],
            ));
        }
    }
    for i in 1..h_c {
        reg.push((
            cid(i, ns),
            [
                cref(i + 1, ns),
                cref(i - 1, ns),
                sh(cref(n_i - i, ns - 1), -pitch),
                cref(i, ns - 1),
                sh(cref(n_i - i - 1, ns - 1), -pitch),
                sh(cref(n_i - i + 1, ns - 1), -pitch),
                cref(i + 1, ns - 1),
                cref(i - 1, ns - 1),
            ],
        ));
    }
    for i in (h_c + 1)..(n_i - h_c) {
        if hold_front {
            break;
        }
        let b = i - h_c;
        reg.push((
            cid(i, ns),
            [
                cref(i + 1, ns),
                cref(i - 1, ns),
                fref(n_up - 1, b),
                cref(i, ns - 1),
                fref(n_up - 1, b + 1),
                fref(n_up - 1, b - 1),
                cref(i + 1, ns - 1),
                cref(i - 1, ns - 1),
            ],
        ));
    }
    for a in 1..n_up {
        for b in 0..n_fe {
            let (w, nw, sw) = if b == 0 {
                (
                    sh(fref(a, n_fe - 1), -pitch),
                    sh(fref(a - 1, n_fe - 1), -pitch),
                    sh(fref(a + 1, n_fe - 1), -pitch),
                )
            } else {
                (fref(a, b - 1), fref(a - 1, b - 1), fref(a + 1, b - 1))
            };
            reg.push((
                fid(a, b),
                [
                    fref(a, b + 1),
                    w,
                    fref(a - 1, b),
                    fref(a + 1, b),
                    fref(a - 1, b + 1),
                    nw,
                    fref(a + 1, b + 1),
                    sw,
                ],
            ));
        }
    }
    let corner = cid(h_c, ns);
    let corner_nb: [NodeRef; 6] = [
        cref(h_c - 1, ns),
        cref(h_c + 1, ns),
        cref(h_c, ns - 1),
        fref(n_up - 1, 0),
        sh(cref(n_i - h_c - 1, ns), -pitch),
        sh(cref(n_i - h_c, ns - 1), -pitch),
    ];
    let at = |pts: &[(f64, f64)], r: NodeRef| -> (f64, f64) {
        let p = pts[r.0];
        (p.0, p.1 + r.1)
    };
    // Control functions of the algebraic grid. Away from the walls the one-dimensional spacing controls (the spacing along
    // every line is kept, the angles relax); next to the stack edge they blend into the controls that make the algebraic
    // grid an exact solution, so the near-wall rows keep their wall-normal structure and the first free rows cannot be drawn
    // onto a convex wall (the outward growth a Winslow grid needs around a small nose radius would otherwise compress them).
    let controls = |pts: &[(f64, f64)]| -> Vec<(f64, f64)> {
        reg.iter()
            .map(|(id, nb)| {
                let p = pts[*id];
                let v: Vec<(f64, f64)> = nb.iter().map(|r| at(pts, *r)).collect();
                let (e, w, n, s) = (v[0], v[1], v[2], v[3]);
                let xi = ((e.0 - w.0) * 0.5, (e.1 - w.1) * 0.5);
                let et = ((n.0 - s.0) * 0.5, (n.1 - s.1) * 0.5);
                let xixi = (e.0 - 2.0 * p.0 + w.0, e.1 - 2.0 * p.1 + w.1);
                let etet = (n.0 - 2.0 * p.0 + s.0, n.1 - 2.0 * p.1 + s.1);
                let g11 = xi.0 * xi.0 + xi.1 * xi.1;
                let g22 = et.0 * et.0 + et.1 * et.1;
                let one_d = (
                    -(xi.0 * xixi.0 + xi.1 * xixi.1) / g11.max(1e-300),
                    -(et.0 * etet.0 + et.1 * etet.1) / g22.max(1e-300),
                );
                // weight of the exact controls: C-block line nodes only, 1 at the stack edge, 0 halfway out
                let omega = if exact_near_wall && *id < nc_nodes && id % (ns + 1) < ns {
                    let (i, j) = (id / (ns + 1), id % (ns + 1));
                    let nb_i = l.stack_cells[i];
                    let half = ((ns - nb_i) / EXACT_DIV).max(1) as f64;
                    fade((j - nb_i - 1) as f64 / half)
                } else {
                    0.0
                };
                if omega == 0.0 {
                    return one_d;
                }
                // exact controls: alpha phi x_xi + gamma psi x_eta = -(alpha x_xixi - 2 beta x_xieta + gamma x_etaeta)
                let alpha = g22;
                let gamma = g11;
                let beta = xi.0 * et.0 + xi.1 * et.1;
                let xe = (
                    0.25 * (v[4].0 - v[5].0 - v[6].0 + v[7].0),
                    0.25 * (v[4].1 - v[5].1 - v[6].1 + v[7].1),
                );
                let rhs = (
                    -(alpha * xixi.0 - 2.0 * beta * xe.0 + gamma * etet.0),
                    -(alpha * xixi.1 - 2.0 * beta * xe.1 + gamma * etet.1),
                );
                // columns alpha x_xi and gamma x_eta
                let (a11, a21, a12, a22) = (alpha * xi.0, alpha * xi.1, gamma * et.0, gamma * et.1);
                let det = a11 * a22 - a12 * a21;
                if !(det.abs() > 1e-6 * (a11.hypot(a21) * a12.hypot(a22)).max(1e-300)) {
                    return one_d;
                }
                let ex = ((rhs.0 * a22 - a12 * rhs.1) / det, (a11 * rhs.1 - a21 * rhs.0) / det);
                (
                    omega * ex.0 + (1.0 - omega) * one_d.0,
                    omega * ex.1 + (1.0 - omega) * one_d.1,
                )
            })
            .collect()
    };
    // ADR 0079 part 3: the last steps of the camber-seam fallback chain start from a Laplace-smoothed grid (every free node at the
    // mean of its four neighbours, Jacobi) instead of the algebraic one, which folds at high stagger beyond what the local
    // untangling below repairs
    for _ in 0..laplace_passes {
        let old = pts.clone();
        for (id, nb) in &reg {
            let mut m = (0.0, 0.0);
            for r in &nb[..4] {
                let v = at(&old, *r);
                m = (m.0 + 0.25 * v.0, m.1 + 0.25 * v.1);
            }
            pts[*id] = m;
        }
        if !hold_front {
            let mut c = (0.0, 0.0);
            for r in &corner_nb {
                let v = at(&old, *r);
                c = (c.0 + v.0 / 6.0, c.1 + v.1 / 6.0);
            }
            pts[corner] = c;
        }
    }
    // Local untangling of a folded algebraic grid: the free nodes around every quad of non-positive area are moved to the mean
    // of their four neighbours until no quad is folded (or the pass cap is reached); untouched elsewhere.
    {
        let mut reg_of = vec![usize::MAX; pts.len()];
        for (q, (id, _)) in reg.iter().enumerate() {
            reg_of[*id] = q;
        }
        let mut quads: Vec<[NodeRef; 4]> = Vec::new();
        for i in 0..n_i {
            for j in 0..ns {
                quads.push([cref(i, j), cref(i + 1, j), cref(i + 1, j + 1), cref(i, j + 1)]);
            }
        }
        for a in 0..n_up {
            for b in 0..n_fe {
                quads.push([fref(a, b), fref(a + 1, b), fref(a + 1, b + 1), fref(a, b + 1)]);
            }
        }
        for _ in 0..UNTANGLE_PASSES {
            let mut marked = vec![false; pts.len()];
            let mut any = false;
            for q in &quads {
                let p: Vec<(f64, f64)> = q.iter().map(|r| at(&pts, *r)).collect();
                let mut ar = 0.0;
                for m in 0..4 {
                    ar += p[m].0 * p[(m + 1) % 4].1 - p[(m + 1) % 4].0 * p[m].1;
                }
                if !(ar > 0.0) {
                    any = true;
                    for r in q {
                        marked[r.0] = true;
                    }
                }
            }
            if !any {
                break;
            }
            // grow the marked set by one ring of free neighbours
            let seeds: Vec<usize> = (0..pts.len()).filter(|&id| marked[id]).collect();
            for id in seeds {
                if reg_of[id] != usize::MAX {
                    for r in &reg[reg_of[id]].1[..4] {
                        marked[r.0] = true;
                    }
                }
            }
            for (id, &mk) in marked.iter().enumerate() {
                let q = reg_of[id];
                if !mk || q == usize::MAX {
                    continue;
                }
                let nb = &reg[q].1;
                let mut m = (0.0, 0.0);
                for r in &nb[..4] {
                    let v = at(&pts, *r);
                    m = (m.0 + 0.25 * v.0, m.1 + 0.25 * v.1);
                }
                pts[id] = m;
            }
        }
    }
    let ctrl = controls(&pts);
    let scale = l.chord.max(1e-300);
    let n_line_nodes: usize = lines.iter().map(|l| l.1).sum();
    let mut lower: Vec<f64> = Vec::new();
    let mut diag: Vec<f64> = Vec::new();
    let mut upper: Vec<f64> = Vec::new();
    let mut rhs: Vec<(f64, f64)> = Vec::new();
    for _ in 0..max_iter {
        let mut moved: f64 = 0.0;
        // line-implicit pass along the C-block j-lines (the stiff direction next to the stacks): coefficients frozen at the
        // current iterate, a tridiagonal solve per line and component
        for &(start, cnt) in &lines {
            lower.clear();
            diag.clear();
            upper.clear();
            rhs.clear();
            for q in start..start + cnt {
                let nb = &reg[q].1;
                let v: Vec<(f64, f64)> = nb.iter().map(|r| at(&pts, *r)).collect();
                let (e, w, n, s) = (v[0], v[1], v[2], v[3]);
                let xi = ((e.0 - w.0) * 0.5, (e.1 - w.1) * 0.5);
                let et = ((n.0 - s.0) * 0.5, (n.1 - s.1) * 0.5);
                let alpha = et.0 * et.0 + et.1 * et.1;
                let gamma = xi.0 * xi.0 + xi.1 * xi.1;
                let beta = xi.0 * et.0 + xi.1 * et.1;
                let (phi, psi) = ctrl[q];
                let (cn, cs) = (gamma * (1.0 + 0.5 * psi), gamma * (1.0 - 0.5 * psi));
                let mut b = (
                    alpha * (e.0 + w.0 + phi * xi.0) - 0.5 * beta * (v[4].0 - v[5].0 - v[6].0 + v[7].0),
                    alpha * (e.1 + w.1 + phi * xi.1) - 0.5 * beta * (v[4].1 - v[5].1 - v[6].1 + v[7].1),
                );
                let first = q == start;
                let last = q + 1 == start + cnt;
                if first {
                    b = (b.0 + cs * s.0, b.1 + cs * s.1);
                }
                if last {
                    b = (b.0 + cn * n.0, b.1 + cn * n.1);
                }
                lower.push(if first { 0.0 } else { -cs });
                upper.push(if last { 0.0 } else { -cn });
                diag.push(2.0 * (alpha + gamma));
                rhs.push(b);
            }
            // Thomas algorithm
            let m = cnt;
            for t in 1..m {
                let f = lower[t] / diag[t - 1];
                diag[t] -= f * upper[t - 1];
                rhs[t] = (rhs[t].0 - f * rhs[t - 1].0, rhs[t].1 - f * rhs[t - 1].1);
            }
            let mut sol = vec![(0.0, 0.0); m];
            sol[m - 1] = (rhs[m - 1].0 / diag[m - 1], rhs[m - 1].1 / diag[m - 1]);
            for t in (0..m - 1).rev() {
                sol[t] = (
                    (rhs[t].0 - upper[t] * sol[t + 1].0) / diag[t],
                    (rhs[t].1 - upper[t] * sol[t + 1].1) / diag[t],
                );
            }
            if sol.iter().all(|p| p.0.is_finite() && p.1.is_finite()) {
                for (t, p) in sol.into_iter().enumerate() {
                    let id = reg[start + t].0;
                    let old = pts[id];
                    moved = moved.max(norm2((p.0 - old.0, p.1 - old.1)));
                    pts[id] = p;
                }
            }
        }
        for (q, (id, nb)) in reg.iter().enumerate().skip(n_line_nodes) {
            let v: Vec<(f64, f64)> = nb.iter().map(|r| at(&pts, *r)).collect();
            let (e, w, n, s) = (v[0], v[1], v[2], v[3]);
            let xi = ((e.0 - w.0) * 0.5, (e.1 - w.1) * 0.5);
            let et = ((n.0 - s.0) * 0.5, (n.1 - s.1) * 0.5);
            let alpha = et.0 * et.0 + et.1 * et.1;
            let gamma = xi.0 * xi.0 + xi.1 * xi.1;
            let beta = xi.0 * et.0 + xi.1 * et.1;
            let (phi, psi) = ctrl[q];
            let den = 2.0 * (alpha + gamma);
            if !(den > 0.0) {
                continue;
            }
            let x = (alpha * (e.0 + w.0 + phi * xi.0) + gamma * (n.0 + s.0 + psi * et.0)
                - 0.5 * beta * (v[4].0 - v[5].0 - v[6].0 + v[7].0))
                / den;
            let y = (alpha * (e.1 + w.1 + phi * xi.1) + gamma * (n.1 + s.1 + psi * et.1)
                - 0.5 * beta * (v[4].1 - v[5].1 - v[6].1 + v[7].1))
                / den;
            let old = pts[*id];
            moved = moved.max(norm2((x - old.0, y - old.1)));
            pts[*id] = (x, y);
        }
        if !hold_front {
            let mut cz = 0.0;
            let mut cs = 0.0;
            for r in &corner_nb {
                let p = at(&pts, *r);
                cz += p.0 / 6.0;
                cs += p.1 / 6.0;
            }
            let old = pts[corner];
            moved = moved.max(norm2((cz - old.0, cs - old.1)));
            pts[corner] = (cz, cs);
        }
        if moved < 1e-12 * scale {
            break;
        }
    }
    for i in 0..=n_i {
        for j in 0..=ns {
            l.c[i][j] = at(&pts, cref(i, j));
        }
    }
    for a in 0..n_up {
        for b in 0..=n_fe {
            l.f[a][b] = at(&pts, fref(a, b));
        }
    }
}

/// In-plane grid of one layer in the unrolled plane `(z, s)` of the radius it was built at.
#[derive(Debug, Clone)]
struct Layer2d {
    /// Radius of the unrolled plane (`s = r theta`).
    r: f64,
    /// C-block nodes `[i][j]`.
    c: Vec<Vec<(f64, f64)>>,
    /// F-block nodes `[a][b]`, rows `a < n_up` (row `n_up` is the C-block front edge).
    f: Vec<Vec<(f64, f64)>>,
    /// Core-block interior nodes `[m][t]` (only `1 <= m < n_chord`, `1 <= t < n_core` are used).
    core: Vec<Vec<(f64, f64)>>,
    /// In-plane unit wall normal of every line `i`.
    normals: Vec<[f64; 2]>,
    /// Exact stack cells of every line `i`.
    stack_cells: Vec<usize>,
    /// Local chord.
    chord: f64,
}

/// Geometry of one layer in the unrolled plane of radius `r` (`k` only names the layer in error messages).
#[allow(clippy::too_many_arguments)]
fn build_layer(
    fan: &AxialFan,
    spec: &CGridSpec,
    lay: &Layout,
    k: usize,
    r: f64,
    h1: f64,
    opts: &CGridOptions,
) -> Result<Layer2d, FanError> {
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
    let straight_misfit = || {
        err(
            "n_side",
            format!(
                "the blade (s from {s_wmin:.4} to {s_wmax:.4}) does not fit between its seams ({s_lo:.4}, {s_hi:.4}): pitch {pitch:.4}"
            ),
        )
    };
    // ADR 0079: with `camber_seam` the check is made below, once it is known whether the seams follow the centre line
    if !opts.camber_seam && !(s_wmin - s_lo > 0.0 && s_hi - s_wmax > 0.0) {
        return Err(straight_misfit());
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
    // passage centre line of the camber seam (ADR 0079): camber line inside the chord (sampled, linear in z), the wake curve
    // downstream, the leading-edge camber slope turning axial over `lam_w` upstream
    let cam: Vec<(f64, f64)> = if opts.camber_seam {
        (0..=CAMBER_SAMPLES)
            .map(|m| fan.camber_zs(&st, 0.5 * (1.0 - (PI * m as f64 / CAMBER_SAMPLES as f64).cos())))
            .collect()
    } else {
        Vec::new()
    };
    if opts.camber_seam && cam.windows(2).any(|w| !(w[1].0 > w[0].0)) {
        return Err(err(
            "stagger",
            "the camber line does not run downstream monotonically, so it cannot carry the seams (camber_seam)".into(),
        ));
    }
    let sigma_le = if opts.camber_seam {
        let (a, b) = (fan.camber_zs(&st, 0.0), fan.camber_zs(&st, 1e-6));
        (b.1 - a.1) / (b.0 - a.0)
    } else {
        0.0
    };
    // upstream the centre line turns axial over a third of the front gap, so that the seams meet the front edge within
    // atan(sigma_le e^-3) of the axial direction (a seam parallel to the front edge folds the corner at high stagger)
    let lam_u = spec.front_gap * chord / 3.0;
    let centre = |z: f64| -> f64 {
        let (z_le, s_le0) = cam[0];
        if z >= z_te {
            s_wake(z)
        } else if z <= z_le {
            s_le0 - sigma_le * lam_u * (1.0 - (-(z_le - z) / lam_u).exp())
        } else {
            let m = cam.partition_point(|p| p.0 <= z).clamp(1, cam.len() - 1);
            let (a, b) = (cam[m - 1], cam[m]);
            a.1 + (b.1 - a.1) * (z - a.0) / (b.0 - a.0)
        }
    };
    let follow = opts.camber_seam;
    let centre_blend = centre;
    let s_lo_at = |z: f64| -> f64 {
        if follow {
            centre_blend(z) - 0.5 * pitch
        } else {
            s_lo
        }
    };
    let s_hi_at = |z: f64| -> f64 {
        if follow {
            s_lo_at(z) + pitch
        } else {
            s_hi
        }
    };
    if opts.camber_seam && !follow && !(s_wmin - s_lo > 0.0 && s_hi - s_wmax > 0.0) {
        return Err(straight_misfit());
    }
    if follow {
        if let Some(p) = sp
            .iter()
            .chain(pp.iter())
            .find(|p| !(p.1 - s_lo_at(p.0) > 0.0 && s_hi_at(p.0) - p.1 > 0.0))
        {
            return Err(err(
                "n_side",
                format!(
                    "the blade point (z {:.4}, s {:.4}) does not fit between the camber seams ({:.4}, {:.4}): pitch {pitch:.4}",
                    p.0,
                    p.1,
                    s_lo_at(p.0),
                    s_hi_at(p.0)
                ),
            ));
        }
    }
    let wake_out = if follow {
        let zw = fan.axial_extent;
        !(s_wake(zw) - s_lo_at(zw) > 0.0 && s_hi_at(zw) - s_wake(zw) > 0.0)
    } else {
        !(s_inf - s_lo > 0.0 && s_hi - s_inf > 0.0)
    };
    if wake_out {
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
    let mut normals: Vec<[f64; 2]> = Vec::with_capacity(n_i + 1);
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
    let mut stack_cells: Vec<usize> = Vec::with_capacity(n_i + 1);
    for i in 0..=n_i {
        let hh = i.min(n_i - i);
        if i >= nd && i <= n_i - nd {
            stack_cells.push(nb);
        } else if opts.uniform_wake_stack {
            let dm = (w[i].1 - s_lo_at(w[i].0)).min(s_hi_at(w[i].0) - w[i].1);
            if !(growth_sum(h1, nb) <= 0.25 * dm) {
                return Err(err(
                    "n_bl",
                    format!(
                        "the wake stack of line {i} (thickness {:.3e}) does not fit before the seam",
                        growth_sum(h1, nb)
                    ),
                ));
            }
            stack_cells.push(nb);
        } else {
            let p = nd - hh;
            let up = if p < nd { zz[p + 1] - zz[p] } else { 0.0 };
            let dn = if p > 0 { zz[p] - zz[p - 1] } else { 0.0 };
            let dz = up.max(dn);
            let dm = (w[i].1 - s_lo_at(w[i].0)).min(s_hi_at(w[i].0) - w[i].1);
            // the first height grows with the local axial spacing (by at most the cell growth per line), so
            // that the rows stay aligned between neighbouring lines and the wake aspect stays bounded
            let d1w = h1.max(dz / if opts.wake_aspect_50 { 50.0 } else { WAKE_ASPECT });
            d1s[i] = d1w;
            stack_cells.push(if growth_sum(d1w, nb) <= 0.25 * dm { nb } else { 0 });
        }
    }
    if opts.lean_stack {
        // ADR 0079 C5: wall-normal stack thickness h1 g^q in every layer
        let cos_lean = |i: usize| -> f64 {
            let (x, side) = if i <= h {
                (xs[nc - (i - nd)], Side::Suction)
            } else {
                (xs[i - h], Side::Pressure)
            };
            // in-plane normal and tangent as 3-D vectors at the wall node, radial surface tangent by a central difference
            let th = w[i].1 / r;
            let e_t = [-th.sin(), th.cos(), 0.0];
            let n = normals[i];
            let n_ip = [n[1] * e_t[0], n[1] * e_t[1], n[0]];
            let t_ip = [n[0] * e_t[0], n[0] * e_t[1], -n[1]];
            let dr = 1e-6 * r;
            let (pa, pb) = (fan.surface_point(r + dr, x, side), fan.surface_point(r - dr, x, side));
            let t_r = [pa[0] - pb[0], pa[1] - pb[1], pa[2] - pb[2]];
            let c = [
                t_ip[1] * t_r[2] - t_ip[2] * t_r[1],
                t_ip[2] * t_r[0] - t_ip[0] * t_r[2],
                t_ip[0] * t_r[1] - t_ip[1] * t_r[0],
            ];
            let cn = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
            ((c[0] * n_ip[0] + c[1] * n_ip[1] + c[2] * n_ip[2]) / cn).abs()
        };
        let c_te = cos_lean(nd);
        for (i, d1) in d1s.iter_mut().enumerate() {
            let c = if i >= nd && i <= n_i - nd { cos_lean(i) } else { c_te };
            if !(c > 0.1) {
                return Err(err(
                    "stagger",
                    format!("the blade leans by more than 84 deg out of the layer at line {i} (lean_stack)"),
                ));
            }
            *d1 /= c;
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
    // the nose target of the front edge: straight ahead of the leading edge (straight seams) or on the centre line (camber seam)
    let (s_n, s_lo_f, s_hi_f) = if follow {
        (s_le, s_lo_at(z_f), s_hi_at(z_f))
    } else {
        (s_le, s_lo, s_hi)
    };
    let lo_part = capped_cells(s_n - s_lo_f, nn, d_nose, true).unwrap_or_else(|_| uniform(s_n - s_lo_f, nn));
    let hi_part = capped_cells(s_hi_f - s_n, nn, d_nose, false).unwrap_or_else(|_| uniform(s_hi_f - s_n, nn));
    let mut s_front = vec![0.0; n_fe + 1];
    for b in 0..=nn {
        s_front[b] = s_lo_f + lo_part[b];
    }
    for b in 1..=nn {
        s_front[nn + b] = s_n + hi_part[b];
    }
    s_front[n_fe] = s_hi_f;
    let z_f_nodes = graded_cells(z_f, spec.n_up, None, Some((d_corner, g, 0))).map_err(|e| err("n_up", e))?;

    // C block (in-plane points)
    let mut cp: Vec<Vec<(f64, f64)>> = vec![vec![(0.0, 0.0); ns + 1]; n_i + 1];
    for i in 0..=n_i {
        let hh = i.min(n_i - i);
        let o = if i < h_c {
            (z_out[hh], s_lo_at(z_out[hh]))
        } else if i > n_i - h_c {
            (z_out[hh], s_hi_at(z_out[hh]))
        } else if i == h_c {
            (z_f, s_lo_at(z_f))
        } else if i == n_i - h_c {
            (z_f, s_hi_at(z_f))
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
    // F block (rows below the front edge)
    let fp: Vec<Vec<(f64, f64)>> = (0..spec.n_up)
        .map(|a| {
            let shift = if follow {
                centre_blend(z_f_nodes[a]) - centre_blend(z_f)
            } else {
                0.0
            };
            (0..=n_fe)
                .map(|b| {
                    if follow {
                        (z_f_nodes[a], s_front[b] + shift)
                    } else {
                        (z_f_nodes[a], s_front[b])
                    }
                })
                .collect()
        })
        .collect();
    // core interior nodes
    let mut core = vec![vec![(0.0, 0.0); lay.n_core + 1]; nc + 1];
    for m in 1..nc {
        for (t, c) in core[m].iter_mut().enumerate().take(lay.n_core).skip(1) {
            let f = t as f64 / lay.n_core as f64;
            *c = (sp[m].0 + (pp[m].0 - sp[m].0) * f, sp[m].1 + (pp[m].1 - sp[m].1) * f);
        }
    }
    Ok(Layer2d {
        r,
        c: cp,
        f: fp,
        core,
        normals,
        stack_cells,
        chord,
    })
}

/// Smooth step from 1 (t <= 0) to 0 (t >= 1).
fn fade(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// Remapped radii of the span layers (ADR 0072 W1): increments `Delta r_k min(1, Delta r_k / mean)` scaled to span the blade; the
/// tip-gap layers stay at the tip radius.
fn remapped_radii(radii: &[f64], n_span: usize) -> Vec<f64> {
    let (r0, r1) = (radii[0], radii[n_span]);
    let mean = (r1 - r0) / n_span as f64;
    let q: Vec<f64> = (0..n_span)
        .map(|k| {
            let d = radii[k + 1] - radii[k];
            d * (d / mean).min(1.0)
        })
        .collect();
    let tot: f64 = q.iter().sum();
    let mut out = Vec::with_capacity(radii.len());
    let mut acc = 0.0;
    out.push(r0);
    for qk in &q {
        acc += qk;
        out.push(r0 + (r1 - r0) * acc / tot);
    }
    out[n_span] = r1;
    while out.len() < radii.len() {
        out.push(r1);
    }
    out
}

/// Write the nodes of layer `k` (radius `rk`) into `grid`. `a` is the in-plane grid of the true section; with `b` (the grid at the
/// remapped radius, ADR 0072 W1) the nodes outside the stacks blend from `a` (near the stack edge) to `b` (beyond the blend length).
fn write_layer(lay: &Layout, k: usize, rk: f64, a: &Layer2d, b: Option<&Layer2d>, grid: &mut [Vec3]) {
    let (n_i, ns) = (lay.n_i(), lay.n_side);
    // (z, theta) of a node of a layer grid
    let zt = |l: &Layer2d, p: (f64, f64)| (p.0, p.1 / l.r);
    let mut put = |id: usize, p: (f64, f64)| {
        grid[id] = [rk * p.1.cos(), rk * p.1.sin(), p.0];
    };
    // physical in-plane coordinates (z, rk theta)
    let phys = |p: (f64, f64)| (p.0, rk * p.1);
    match b {
        None => {
            for i in 0..=n_i {
                for j in 0..=ns {
                    put(lay.c(i, j, k), zt(a, a.c[i][j]));
                }
            }
            for (ai, row) in a.f.iter().enumerate() {
                for (bi, p) in row.iter().enumerate() {
                    put(lay.f(ai, bi, k), zt(a, *p));
                }
            }
        }
        Some(b) => {
            // blend length: a minimum of 0.05 local chords, or four times the largest stack-edge difference
            let mut dmax: f64 = 0.0;
            for i in 0..=n_i {
                let nb = a.stack_cells[i];
                let (pa, pb) = (phys(zt(a, a.c[i][nb])), phys(zt(b, b.c[i][nb])));
                dmax = dmax.max(norm2((pa.0 - pb.0, pa.1 - pb.1)));
            }
            let ell = (0.05 * a.chord).max(4.0 * dmax);
            for i in 0..=n_i {
                let nb = a.stack_cells[i];
                let pa: Vec<(f64, f64)> = (0..=ns).map(|j| phys(zt(a, a.c[i][j]))).collect();
                let mut arc = vec![0.0; ns + 1];
                for j in nb + 1..=ns {
                    arc[j] = arc[j - 1] + norm2((pa[j].0 - pa[j - 1].0, pa[j].1 - pa[j - 1].1));
                }
                let len = ell.min(0.5 * arc[ns]).max(1e-300);
                for j in 0..=ns {
                    let p = if j <= nb {
                        zt(a, a.c[i][j])
                    } else {
                        let w = fade(arc[j] / len);
                        let (qa, qb) = (zt(a, a.c[i][j]), zt(b, b.c[i][j]));
                        (qb.0 + w * (qa.0 - qb.0), qb.1 + w * (qa.1 - qb.1))
                    };
                    put(lay.c(i, j, k), p);
                }
            }
            for (ai, row) in b.f.iter().enumerate() {
                for (bi, p) in row.iter().enumerate() {
                    put(lay.f(ai, bi, k), zt(b, *p));
                }
            }
        }
    }
    // core interior nodes follow the true section
    for m in 1..lay.n_c {
        for t in 1..lay.n_core {
            put(lay.core(m, t, k), zt(a, a.core[m][t]));
        }
    }
}

/// Radial filter of the in-plane node positions (ADR 0079 part 1, F3): every layer is smoothed in the plane independently, and a
/// discrete step of the smoothing (how many untangling passes a layer needs) can move a far node of one layer by several layer
/// thicknesses against its neighbours. Two Jacobi passes move every interior layer's `(z, theta)` half-way towards the linear
/// interpolation of its two neighbours in `r`. Untouched: the hub and shroud layers, the wall and stack nodes, the outlet lines and the
/// inlet row (they stay in their planes); the weight rises from 0 at each stack edge to 1 on the seam or front edge. Both seam copies are
/// filtered with the same weights, so the pair stays an exact rotation image.
fn radial_filter(lay: &Layout, radii: &[f64], stacks: &[Vec<usize>], grid: &mut [Vec3]) {
    let (n_i, ns, n_k) = (lay.n_i(), lay.n_side, radii.len() - 1);
    // (node id per layer offset 0, weight): the C nodes j > stack and the F nodes a >= 1
    let mut nodes: Vec<(usize, f64)> = Vec::new();
    for i in 1..n_i {
        let nb = stacks.iter().map(|s| s[i]).max().unwrap_or(0);
        for j in nb + 1..=ns {
            // 0 at the stack edge, 1 on the seam or front edge (the upper seam nodes are separate ids, filtered alike)
            nodes.push((lay.c(i, j, 0), 1.0 - fade((j - nb) as f64 / (ns - nb) as f64)));
        }
    }
    for a in 1..lay.n_up {
        for b in 0..=lay.n_fe() {
            nodes.push((lay.f(a, b, 0), 1.0));
        }
    }
    let stride = lay.stride();
    for _ in 0..2 {
        let old: Vec<Vec3> = grid.to_vec();
        for &(id0, w) in &nodes {
            if w == 0.0 {
                continue;
            }
            for k in 1..n_k {
                let (rm, r0, rp) = (radii[k - 1], radii[k], radii[k + 1]);
                if !(rp > rm) {
                    continue;
                }
                let zt = |p: Vec3| (p[2], p[1].atan2(p[0]));
                let (pm, p0, pp) = (
                    zt(old[id0 + (k - 1) * stride]),
                    zt(old[id0 + k * stride]),
                    zt(old[id0 + (k + 1) * stride]),
                );
                let t = (r0 - rm) / (rp - rm);
                let target = (pm.0 + t * (pp.0 - pm.0), pm.1 + t * (pp.1 - pm.1));
                let f = 0.5 * w;
                let (z, th) = (p0.0 + f * (target.0 - p0.0), p0.1 + f * (target.1 - p0.1));
                grid[id0 + k * stride] = [r0 * th.cos(), r0 * th.sin(), z];
            }
        }
    }
}

/// First quad with non-positive area in the unrolled plane of its layer: every C-block quad, and (with `check_f`) every F-block
/// quad. `(layer, radius, block, line or row, row or column)`.
fn folded_quad(
    lay: &Layout,
    radii: &[f64],
    grid: &[Vec3],
    check_f: bool,
) -> Option<(usize, f64, &'static str, usize, usize)> {
    let area = |q: [usize; 4], r: f64| -> f64 {
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
        a
    };
    for (k, &r) in radii.iter().enumerate() {
        for j in 0..lay.n_side {
            for i in 0..lay.n_i() {
                let q = [
                    lay.c(i, j, k),
                    lay.c(i + 1, j, k),
                    lay.c(i + 1, j + 1, k),
                    lay.c(i, j + 1, k),
                ];
                if !(area(q, r) > 0.0) {
                    return Some((k, r, "C block", i, j));
                }
            }
        }
        if check_f {
            for a in 0..lay.n_up {
                for b in 0..lay.n_fe() {
                    let q = [
                        lay.f(a, b, k),
                        lay.f(a + 1, b, k),
                        lay.f(a + 1, b + 1, k),
                        lay.f(a, b + 1, k),
                    ];
                    if !(area(q, r) > 0.0) {
                        return Some((k, r, "F block", a, b));
                    }
                }
            }
        }
    }
    None
}

/// The documented control range of the camber-seam construction (ADR 0079 amendment 5, chosen from a development scan on test blade B
/// of ADR 0065): `0.15 <= n_nose / n_chord <= 0.375`, `max(8, n_bl + 6) <= n_side <= 2 n_chord`, `0.15 <= front_gap <= 1.0`. Inside
/// it every scanned blade-B-like combination built; it does not make strongly staggered blades (FAN-01 tip, 70 deg) robust.
pub fn control_range_problems(spec: &CGridSpec) -> Vec<FanError> {
    let mut e = Vec::new();
    let nc = spec.n_chord as f64;
    let ratio = spec.n_nose as f64 / nc.max(1.0);
    if !(0.15..=0.375).contains(&ratio) {
        e.push(FanError::new(
            "n_nose",
            format!(
                "outside the documented range of c_grid_construction camber_seam: 0.15 n_chord <= n_nose <= 0.375 n_chord ({:.1} to {:.1} for n_chord {}), has {}",
                0.15 * nc,
                0.375 * nc,
                spec.n_chord,
                spec.n_nose
            ),
        ));
    }
    let lo = 8.max(spec.n_bl + 6);
    if !(spec.n_side >= lo && spec.n_side <= 2 * spec.n_chord) {
        e.push(FanError::new(
            "n_side",
            format!(
                "outside the documented range of c_grid_construction camber_seam: max(8, n_bl + 6) = {lo} <= n_side <= 2 n_chord = {}, has {}",
                2 * spec.n_chord,
                spec.n_side
            ),
        ));
    }
    if !(spec.front_gap >= 0.15 && spec.front_gap <= 1.0) {
        e.push(FanError::new(
            "front_gap",
            format!(
                "outside the documented range of c_grid_construction camber_seam: 0.15 <= front_gap <= 1.0 chords, has {}",
                spec.front_gap
            ),
        ));
    }
    e
}

/// Build the periodic single-passage C-grid mesh with the construction of ADR 0068 (wave 33A).
pub fn build_cgrid_mesh(fan: &AxialFan, spec: &CGridSpec) -> Result<CGridMesh, Vec<FanError>> {
    build_cgrid_mesh_with(fan, spec, &CGridOptions::default())
}

/// Build the periodic single-passage C-grid mesh with the given construction options (ADR 0072).
pub fn build_cgrid_mesh_with(
    fan: &AxialFan,
    spec: &CGridSpec,
    opts: &CGridOptions,
) -> Result<CGridMesh, Vec<FanError>> {
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
    if opts.checked_range {
        errs.extend(control_range_problems(spec));
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
    // ADR 0079: with `gap_fit` the gap stacks start at the largest height for which both stacks and a middle of continuous growth fit
    let h_gap = if opts.gap_fit && spec.n_gap >= 2 * nb {
        let stack: f64 = (0..nb).map(|q| g.powi(q as i32)).sum();
        h1.min(fan.tip_clearance / (2.0 * stack + (spec.n_gap - 2 * nb) as f64 * g.powi(nb as i32)))
    } else {
        h1
    };
    let rs_gap = graded_two_sided(fan.tip_clearance, spec.n_gap, h_gap, g, nb).map_err(|e| one(e, "n_gap"))?;
    let mut radii: Vec<f64> = rs_span.iter().map(|x| fan.r_hub + x).collect();
    radii[spec.n_span] = fan.r_tip;
    radii.extend(rs_gap[1..].iter().map(|x| fan.r_tip + x));
    *radii.last_mut().expect("layers") = fan.r_shroud();

    let rho = remapped_radii(&radii, spec.n_span);
    // The smoothed construction tries (blend factor of the smoothing, rows beyond the stack kept algebraic) in this order until
    // the mesh is untangled; one setting serves every layer, so neighbouring layers stay consistent.
    let configs: &[(f64, bool, bool, usize)] = if opts.smoothing && opts.camber_seam {
        &[
            (1.0, false, true, 0),
            (1.0, false, false, 0),
            (1.0, true, false, 0),
            (0.5, false, false, 0),
            (0.25, false, false, 0),
            (0.0, false, false, 0),
            (1.0, false, true, LAPLACE_PASSES),
            (1.0, false, false, LAPLACE_PASSES),
        ]
    } else if opts.smoothing {
        &[
            (1.0, false, true, 0),
            (1.0, false, false, 0),
            (1.0, true, false, 0),
            (0.5, false, false, 0),
            (0.25, false, false, 0),
            (0.0, false, false, 0),
        ]
    } else {
        &[(0.0, false, false, 0)]
    };
    let attempt = |step: usize,
                   lambda: f64,
                   freeze: bool,
                   exact: bool,
                   laplace: usize|
     -> Result<CGridMesh, Vec<FanError>> {
        let stride = lay.stride();
        let mut grid = vec![[0.0; 3]; stride * (n_k + 1)];
        let mut normals = Vec::with_capacity(n_k + 1);
        let mut stacks = Vec::with_capacity(n_k + 1);
        let mut cache: Vec<(f64, Layer2d)> = Vec::new();
        for (k, &r) in radii.iter().enumerate() {
            // W1: the tip gap is a radial extrusion of the tip layer; the far grid is built at the remapped radius
            let r_true = if opts.radial_remap { r.min(fan.r_tip) } else { r };
            let mut layer = |rb: f64| -> Result<Layer2d, Vec<FanError>> {
                if let Some((_, l)) = cache.iter().find(|(rc, _)| *rc == rb) {
                    return Ok(l.clone());
                }
                let mut l = build_layer(fan, spec, &lay, k, rb, h1, opts).map_err(|e| vec![e])?;
                if lambda > 0.0 {
                    let alg = l.clone();
                    smooth_layer(
                        &mut l,
                        &lay,
                        rb * fan.sector_angle(),
                        SMOOTH_ITER,
                        freeze,
                        exact,
                        opts.nose_rows_algebraic,
                        opts.front_edge_held,
                        laplace,
                    );
                    if lambda < 1.0 {
                        let mix = |p: &mut (f64, f64), q: (f64, f64)| {
                            *p = (q.0 + lambda * (p.0 - q.0), q.1 + lambda * (p.1 - q.1));
                        };
                        for (row, arow) in l.c.iter_mut().zip(&alg.c) {
                            for (p, q) in row.iter_mut().zip(arow) {
                                mix(p, *q);
                            }
                        }
                        for (row, arow) in l.f.iter_mut().zip(&alg.f) {
                            for (p, q) in row.iter_mut().zip(arow) {
                                mix(p, *q);
                            }
                        }
                    }
                }
                if rb == fan.r_tip && opts.radial_remap {
                    cache.push((rb, l.clone()));
                }
                Ok(l)
            };
            let a = layer(r_true)?;
            let b = if opts.radial_remap && rho[k] != r_true {
                Some(layer(rho[k])?)
            } else {
                None
            };
            write_layer(&lay, k, r, &a, b.as_ref(), &mut grid);
            normals.push(a.normals);
            stacks.push(a.stack_cells);
        }
        if opts.radial_filter {
            radial_filter(&lay, &radii, &stacks, &mut grid);
        }
        if let Some((k, r, blk, i, j)) = folded_quad(&lay, &radii, &grid, opts.smoothing) {
            return Err(one(
            format!(
                "the line family is folded at layer {k} (r = {r:.5}), {blk} around line {i}, row {j}: the leading-edge fan cannot be built for this blade with the front_gap {} and n_nose {} given; reduce front_gap or change n_nose",
                spec.front_gap, spec.n_nose
            ),
            "front_gap",
        ));
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
            h_gap,
            radii: radii.clone(),
            normals,
            stack_cells: stacks,
            cell_tags,
            construction_step: step,
        })
    };
    let mut last = None;
    for (step, &(lambda, freeze, exact, laplace)) in configs.iter().enumerate() {
        match attempt(step, lambda, freeze, exact, laplace) {
            Ok(m) => return Ok(m),
            Err(e) => {
                // report the failure of the first (full) construction
                if last.is_none() {
                    last = Some(e);
                }
            }
        }
    }
    Err(last.unwrap_or_default())
}
