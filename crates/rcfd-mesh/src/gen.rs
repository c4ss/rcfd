//! Structured/block mesh generators for simple geometries (F-012): box,
//! channel, backward-facing step and cylinder O-grid. All produce hexahedral
//! cells in a canonical [`Mesh`] with named patches.

use crate::mesh::{BoundarySpec, Mesh, PatchKind};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};

/// Point distribution along one direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Grading {
    /// Equal cell sizes.
    Uniform,
    /// Geometric progression; the ratio is (last cell size) / (first cell size).
    Expansion(f64),
    /// Cells clustered at both ends: mirrored geometric progression where the
    /// ratio is (largest, central cell size) / (smallest, end cell size).
    Symmetric(f64),
    /// Prismatic wall layers (F-108): `layers` cells of height `first * growth^i` starting at
    /// coordinate 0 (and mirrored at `len` when `both`), followed by bulk cells whose sizes grow
    /// geometrically from the last layer to fill the rest of the axis (see
    /// [`crate::layers::wall_segment_sizes`]). `both` needs an even cell count.
    WallLayers {
        /// Height of the wall-adjacent cell.
        first: f64,
        /// Ratio of successive layer heights.
        growth: f64,
        /// Number of inflation layers at each wall.
        layers: usize,
        /// Layers at both ends of the axis instead of at coordinate 0 only.
        both: bool,
    },
}

fn geometric(n: usize, len: f64, ratio: f64) -> Vec<f64> {
    // sizes s0 * r^i with r^(n-1) = ratio
    if n == 1 || (ratio - 1.0).abs() < 1e-14 {
        return (0..=n).map(|i| len * i as f64 / n as f64).collect();
    }
    let r = ratio.powf(1.0 / (n as f64 - 1.0));
    let s0 = len * (r - 1.0) / (r.powi(n as i32) - 1.0);
    let mut x = vec![0.0];
    let mut s = s0;
    for _ in 0..n {
        x.push(x.last().expect("non-empty") + s);
        s *= r;
    }
    *x.last_mut().expect("non-empty") = len;
    x
}

/// The [`Grading::Expansion`] ratio (last / first) whose last cell of `n` on a length `len` has the size `h` (ADR 0112;
/// use the reciprocal for a first cell of size `h`). With one cell the cell is the whole length, accepted when `len >= h`.
pub fn expansion_for_end_cell(n: usize, len: f64, h: f64) -> Result<f64> {
    let last = |ln_ratio: f64| {
        let x = geometric(n, len, ln_ratio.exp());
        x[n] - x[n - 1]
    };
    let ok = n >= 1 && len > 0.0 && h > 0.0 && if n == 1 { h <= len } else { h < len };
    if !ok || (n > 1 && (h <= last(-50.0) || h >= last(50.0))) {
        return Err(Error::Mesh(format!(
            "an end cell of size {h:.4e} does not fit {n} cells on a length {len:.4e}"
        )));
    }
    if n == 1 {
        return Ok(1.0);
    }
    let (mut lo, mut hi) = (-50.0, 50.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if last(mid) < h {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok((0.5 * (lo + hi)).exp())
}

/// Coordinates (length `n + 1`) from 0 to `len` with the requested grading.
pub fn axis_coords(n: usize, len: f64, g: Grading) -> Result<Vec<f64>> {
    if n == 0 || len <= 0.0 {
        return Err(Error::Mesh("axis needs n >= 1 and positive length".into()));
    }
    match g {
        Grading::Uniform => Ok(geometric(n, len, 1.0)),
        Grading::Expansion(r) if r > 0.0 => Ok(geometric(n, len, r)),
        Grading::Symmetric(r) if r > 0.0 => {
            // sizes s0 r^i towards the centre, mirrored; odd n has one centre cell.
            let m = n / 2;
            let steps = if n % 2 == 1 { m } else { m.saturating_sub(1) };
            let q = if steps == 0 { 1.0 } else { r.powf(1.0 / steps as f64) };
            let mut sizes: Vec<f64> = (0..m).map(|i| q.powi(i as i32)).collect();
            if n % 2 == 1 {
                sizes.push(q.powi(m as i32));
            }
            for i in (0..m).rev() {
                sizes.push(q.powi(i as i32));
            }
            let total: f64 = sizes.iter().sum();
            let mut x = vec![0.0];
            for s in &sizes {
                x.push(x.last().expect("non-empty") + s * len / total);
            }
            *x.last_mut().expect("non-empty") = len;
            Ok(x)
        }
        Grading::WallLayers {
            first,
            growth,
            layers,
            both,
        } => {
            let sizes = crate::layers::wall_layer_axis_sizes(n, len, first, growth, layers, both)?;
            let mut x = vec![0.0];
            for s in &sizes {
                x.push(x.last().expect("non-empty") + s);
            }
            *x.last_mut().expect("non-empty") = len;
            Ok(x)
        }
        _ => Err(Error::Mesh("grading ratio must be positive".into())),
    }
}

/// Face definition of a hex given by corner ids `p[x + 2y + 4z]`.
fn hex_faces(p: [usize; 8]) -> Vec<Vec<usize>> {
    vec![
        vec![p[0], p[2], p[6], p[4]], // x-
        vec![p[1], p[3], p[7], p[5]], // x+
        vec![p[0], p[1], p[5], p[4]], // y-
        vec![p[2], p[3], p[7], p[6]], // y+
        vec![p[0], p[1], p[3], p[2]], // z-
        vec![p[4], p[5], p[7], p[6]], // z+
    ]
}

/// Specification of a masked structured grid.
struct MaskedGrid<'a> {
    xs: &'a [f64],
    ys: &'a [f64],
    zs: &'a [f64],
    /// Cell (i, j) of the xy plane exists?
    mask: &'a dyn Fn(usize, usize) -> bool,
    /// Patch for a boundary face: direction 0..6 (x-, x+, y-, y+, z-, z+) and cell (i, j).
    patch: &'a dyn Fn(usize, usize, usize) -> &'static str,
    kinds: &'a [(&'static str, PatchKind)],
}

fn build_masked(g: MaskedGrid<'_>) -> Result<Mesh> {
    let (nx, ny, nz) = (g.xs.len() - 1, g.ys.len() - 1, g.zs.len() - 1);
    let pid = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
    let mut points: Vec<Vec3> = Vec::with_capacity((nx + 1) * (ny + 1) * (nz + 1));
    for k in 0..=nz {
        for j in 0..=ny {
            for i in 0..=nx {
                points.push([g.xs[i], g.ys[j], g.zs[k]]);
            }
        }
    }
    let exists = |i: isize, j: isize| {
        i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny && (g.mask)(i as usize, j as usize)
    };
    let mut cells = Vec::new();
    let mut specs: Vec<BoundarySpec> = g
        .kinds
        .iter()
        .map(|(n, k)| BoundarySpec {
            name: (*n).to_string(),
            kind: k.clone(),
            faces: vec![],
        })
        .collect();
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if !(g.mask)(i, j) {
                    continue;
                }
                let mut p = [0usize; 8];
                for (c, pc) in p.iter_mut().enumerate() {
                    *pc = pid(i + (c & 1), j + ((c >> 1) & 1), k + (c >> 2));
                }
                let faces = hex_faces(p);
                let (ii, jj) = (i as isize, j as isize);
                let free = [
                    !exists(ii - 1, jj),
                    !exists(ii + 1, jj),
                    !exists(ii, jj - 1),
                    !exists(ii, jj + 1),
                    k == 0,
                    k + 1 == nz,
                ];
                for (d, f) in faces.iter().enumerate() {
                    if free[d] {
                        let name = (g.patch)(d, i, j);
                        let spec = specs
                            .iter_mut()
                            .find(|s| s.name == name)
                            .ok_or_else(|| Error::Mesh(format!("generator bug: patch {name} not declared")))?;
                        spec.faces.push(f.clone());
                    }
                }
                cells.push(faces);
            }
        }
    }
    specs.retain(|s| !s.faces.is_empty());
    Mesh::from_cell_faces(points, &cells, &specs)
}

const BOX_KINDS: [(&str, PatchKind); 6] = [
    ("xmin", PatchKind::Wall),
    ("xmax", PatchKind::Wall),
    ("ymin", PatchKind::Wall),
    ("ymax", PatchKind::Wall),
    ("zmin", PatchKind::Wall),
    ("zmax", PatchKind::Wall),
];
const BOX_NAMES: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

/// Axis-aligned box `[0,lx]x[0,ly]x[0,lz]` with `n = [nx, ny, nz]` hexes and
/// patches `xmin, xmax, ymin, ymax, zmin, zmax` (all walls).
pub fn box_mesh(n: [usize; 3], l: [f64; 3], g: [Grading; 3]) -> Result<Mesh> {
    let xs = axis_coords(n[0], l[0], g[0])?;
    let ys = axis_coords(n[1], l[1], g[1])?;
    let zs = axis_coords(n[2], l[2], g[2])?;
    build_masked(MaskedGrid {
        xs: &xs,
        ys: &ys,
        zs: &zs,
        mask: &|_, _| true,
        patch: &|d, _, _| BOX_NAMES[d],
        kinds: &BOX_KINDS,
    })
}

/// 2D (one cell thick, `empty` front/back) lid-driven-cavity style square.
/// Patches: `lid` (ymax), `walls` (xmin, xmax, ymin), `frontAndBack` (empty).
pub fn cavity_2d(n: usize, size: f64) -> Result<Mesh> {
    cavity_2d_rect(n, n, size, size)
}

/// Rectangular cavity `[0,lx]x[0,ly]` of `nx` x `ny` uniform cells, patches `lid` (ymax),
/// `walls` and `frontAndBack`; [`cavity_2d`] is the square case.
pub fn cavity_2d_rect(nx: usize, ny: usize, lx: f64, ly: f64) -> Result<Mesh> {
    let xs = axis_coords(nx, lx, Grading::Uniform)?;
    let ys = axis_coords(ny, ly, Grading::Uniform)?;
    let zs = axis_coords(1, lx / nx as f64, Grading::Uniform)?;
    build_masked(MaskedGrid {
        xs: &xs,
        ys: &ys,
        zs: &zs,
        mask: &|_, _| true,
        patch: &|d, _, _| match d {
            3 => "lid",
            4 | 5 => "frontAndBack",
            _ => "walls",
        },
        kinds: &[
            ("lid", PatchKind::Wall),
            ("walls", PatchKind::Wall),
            ("frontAndBack", PatchKind::Empty),
        ],
    })
}

/// 2D rectangle `[0,lx]x[0,ly]` (one cell in z, `empty` front/back) with patches
/// `xmin, xmax, ymin, ymax, frontAndBack`. With `periodic_x` / `periodic_y` the
/// opposite patch pairs are made cyclic.
pub fn box_2d(nx: usize, ny: usize, lx: f64, ly: f64, periodic_x: bool, periodic_y: bool) -> Result<Mesh> {
    box_2d_graded(nx, ny, lx, ly, [Grading::Uniform; 2], periodic_x, periodic_y)
}

/// [`box_2d`] with a grading per direction (for example [`Grading::Symmetric`]
/// to cluster cells at both walls of a cavity).
pub fn box_2d_graded(
    nx: usize,
    ny: usize,
    lx: f64,
    ly: f64,
    grading: [Grading; 2],
    periodic_x: bool,
    periodic_y: bool,
) -> Result<Mesh> {
    let xs = axis_coords(nx, lx, grading[0])?;
    let ys = axis_coords(ny, ly, grading[1])?;
    let zs = axis_coords(1, lx / nx as f64, Grading::Uniform)?;
    let mut m = build_masked(MaskedGrid {
        xs: &xs,
        ys: &ys,
        zs: &zs,
        mask: &|_, _| true,
        patch: &|d, _, _| match d {
            0 => "xmin",
            1 => "xmax",
            2 => "ymin",
            3 => "ymax",
            _ => "frontAndBack",
        },
        kinds: &[
            ("xmin", PatchKind::Wall),
            ("xmax", PatchKind::Wall),
            ("ymin", PatchKind::Wall),
            ("ymax", PatchKind::Wall),
            ("frontAndBack", PatchKind::Empty),
        ],
    })?;
    if periodic_x {
        m.set_periodic("xmin", "xmax")?;
        m.set_periodic("xmax", "xmin")?;
    }
    if periodic_y {
        m.set_periodic("ymin", "ymax")?;
        m.set_periodic("ymax", "ymin")?;
    }
    Ok(m)
}

/// Channel of length `l`, height `h`, width `w` with patches `inlet` (xmin),
/// `outlet` (xmax), `walls` (ymin, ymax) and `sides` (zmin, zmax; symmetry).
/// `nz == 1` with `two_d = true` makes the sides `empty` instead.
pub fn channel(n: [usize; 3], l: [f64; 3], g: [Grading; 3], two_d: bool) -> Result<Mesh> {
    let xs = axis_coords(n[0], l[0], g[0])?;
    let ys = axis_coords(n[1], l[1], g[1])?;
    let zs = axis_coords(n[2], l[2], g[2])?;
    let side_kind = if two_d { PatchKind::Empty } else { PatchKind::Symmetry };
    build_masked(MaskedGrid {
        xs: &xs,
        ys: &ys,
        zs: &zs,
        mask: &|_, _| true,
        patch: &|d, _, _| match d {
            0 => "inlet",
            1 => "outlet",
            2 | 3 => "walls",
            _ => "sides",
        },
        kinds: &[
            ("inlet", PatchKind::Inlet),
            ("outlet", PatchKind::Outlet),
            ("walls", PatchKind::Wall),
            ("sides", side_kind),
        ],
    })
}

/// Parameters of a backward-facing step.
#[derive(Debug, Clone, Copy)]
pub struct StepSpec {
    /// Inlet channel length (upstream of the step).
    pub l_in: f64,
    /// Length downstream of the step.
    pub l_out: f64,
    /// Inlet channel height.
    pub h_in: f64,
    /// Step height.
    pub step: f64,
    /// Span (z) thickness.
    pub depth: f64,
    /// Cells: upstream x, downstream x, inlet-height y, step-height y.
    pub n: [usize; 4],
}

impl StepSpec {
    /// Step geometry from the expansion ratio `ER = (h_in + step) / h_in` (downstream height over
    /// inlet height), e.g. `ER = 1.125` for Driver and Seegmiller (1985) where the inlet channel is
    /// eight step heights high. Fails for `ER <= 1` or non-positive lengths.
    pub fn from_expansion_ratio(er: f64, step: f64, l_in: f64, l_out: f64, depth: f64, n: [usize; 4]) -> Result<Self> {
        if er <= 1.0 || step <= 0.0 || l_in <= 0.0 || l_out <= 0.0 || depth <= 0.0 {
            return Err(Error::Mesh(
                "backward step needs expansion ratio > 1 and positive lengths".into(),
            ));
        }
        Ok(StepSpec {
            l_in,
            l_out,
            h_in: step / (er - 1.0),
            step,
            depth,
            n,
        })
    }

    /// Expansion ratio `(h_in + step) / h_in`.
    pub fn expansion_ratio(&self) -> f64 {
        (self.h_in + self.step) / self.h_in
    }
}

/// Point distributions of a graded backward-facing step. Each axis segment is graded with
/// [`Grading`] over its own length, measured in the direction of increasing coordinate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepGrading {
    /// Upstream x segment `[0, l_in]` (use `Expansion(1/r)` to cluster toward the step).
    pub x_up: Grading,
    /// Downstream x segment `[l_in, l_in + l_out]` (`Expansion(r)` clusters toward the step face).
    pub x_down: Grading,
    /// Floor-to-lip y segment `[0, step]` (`Symmetric(r)` clusters toward the floor and the lip).
    pub y_step: Grading,
    /// Lip-to-top-wall y segment `[step, step + h_in]` (`Symmetric(r)` clusters toward the lip and
    /// the upper wall).
    pub y_in: Grading,
}

impl StepGrading {
    /// Uniform spacing everywhere.
    pub fn uniform() -> Self {
        StepGrading {
            x_up: Grading::Uniform,
            x_down: Grading::Uniform,
            y_step: Grading::Uniform,
            y_in: Grading::Uniform,
        }
    }

    /// Cells clustered toward the walls and the step: ratio `rx` (largest / smallest x cell, small
    /// cells at the step face) and ratio `ry` (largest / smallest y cell, small cells at the floor,
    /// the shear-layer lip `y = step` and the upper wall).
    pub fn clustered(rx: f64, ry: f64) -> Self {
        StepGrading {
            x_up: Grading::Expansion(1.0 / rx),
            x_down: Grading::Expansion(rx),
            y_step: Grading::Symmetric(ry),
            y_in: Grading::Symmetric(ry),
        }
    }
}

/// 2D backward-facing step (one cell in z). Patches: `inlet`, `outlet`,
/// `walls` (floor, step face, upper wall), `frontAndBack` (empty).
pub fn backward_step(s: StepSpec) -> Result<Mesh> {
    backward_step_graded(s, StepGrading::uniform())
}

/// [`backward_step`] with graded (stretched) cell spacing, for wall-resolved or
/// wall-function meshes with clustering toward the walls and the step.
pub fn backward_step_graded(s: StepSpec, g: StepGrading) -> Result<Mesh> {
    let [nxu, nxd, nyi, nys] = s.n;
    if s.step <= 0.0 || s.h_in <= 0.0 {
        return Err(Error::Mesh(
            "backward step needs positive step and inlet heights".into(),
        ));
    }
    let mut xs = axis_coords(nxu, s.l_in, g.x_up)?;
    let d = axis_coords(nxd, s.l_out, g.x_down)?;
    xs.extend(d.iter().skip(1).map(|x| x + s.l_in));
    // y: step region [0, step] then inlet region [step, step + h_in]
    let mut ys = axis_coords(nys, s.step, g.y_step)?;
    let u = axis_coords(nyi, s.h_in, g.y_in)?;
    ys.extend(u.iter().skip(1).map(|y| y + s.step));
    let zs = axis_coords(1, s.depth, Grading::Uniform)?;
    // cell (i, j) exists unless it is upstream (i < nxu) and below the inlet (j < nys).
    build_masked(MaskedGrid {
        xs: &xs,
        ys: &ys,
        zs: &zs,
        mask: &|i, j| !(i < nxu && j < nys),
        patch: &|d, i, _| match d {
            0 if i == 0 => "inlet",
            1 if i + 1 == nxu + nxd => "outlet",
            4 | 5 => "frontAndBack",
            _ => "walls",
        },
        kinds: &[
            ("inlet", PatchKind::Inlet),
            ("outlet", PatchKind::Outlet),
            ("walls", PatchKind::Wall),
            ("frontAndBack", PatchKind::Empty),
        ],
    })
}

/// O-grid around a cylinder (2D, one cell in z): `n_theta` cells around,
/// `n_r` radial cells from `r_in` to `r_out` with radial grading.
/// Patches: `cylinder` (inner wall), `farfield`, `frontAndBack` (empty).
pub fn cylinder_ogrid(n_theta: usize, n_r: usize, r_in: f64, r_out: f64, grading: Grading, depth: f64) -> Result<Mesh> {
    if n_theta < 3 || r_in <= 0.0 || r_out <= r_in {
        return Err(Error::Mesh(
            "cylinder_ogrid needs n_theta >= 3 and 0 < r_in < r_out".into(),
        ));
    }
    let rs: Vec<f64> = axis_coords(n_r, r_out - r_in, grading)?
        .into_iter()
        .map(|r| r + r_in)
        .collect();
    let pid = |it: usize, ir: usize, k: usize| (it % n_theta) + n_theta * (ir + (n_r + 1) * k);
    let mut points = Vec::new();
    for k in 0..2 {
        for &r in &rs {
            for it in 0..n_theta {
                let th = 2.0 * std::f64::consts::PI * it as f64 / n_theta as f64;
                points.push([r * th.cos(), r * th.sin(), depth * k as f64]);
            }
        }
    }
    let mut cells = Vec::new();
    let mut cyl = Vec::new();
    let mut far = Vec::new();
    let mut fb = Vec::new();
    for ir in 0..n_r {
        for it in 0..n_theta {
            // x-axis of the hex = theta direction, y = radial, z = span
            let mut p = [0usize; 8];
            for (c, pc) in p.iter_mut().enumerate() {
                *pc = pid(it + (c & 1), ir + ((c >> 1) & 1), c >> 2);
            }
            let faces = hex_faces(p);
            if ir == 0 {
                cyl.push(faces[2].clone());
            }
            if ir + 1 == n_r {
                far.push(faces[3].clone());
            }
            fb.push(faces[4].clone());
            fb.push(faces[5].clone());
            cells.push(faces);
        }
    }
    let specs = vec![
        BoundarySpec {
            name: "cylinder".into(),
            kind: PatchKind::Wall,
            faces: cyl,
        },
        BoundarySpec {
            name: "farfield".into(),
            kind: PatchKind::Generic,
            faces: far,
        },
        BoundarySpec {
            name: "frontAndBack".into(),
            kind: PatchKind::Empty,
            faces: fb,
        },
    ];
    Mesh::from_cell_faces(points, &cells, &specs)
}

/// 3-D cylindrical annulus `r_in <= r <= r_out`, `z0 <= z <= z0 + height` about the z axis (full circle, `n_theta`
/// chords, `n_r` radial and `n_z` axial hexahedra; MRF / Taylor-Couette work, F-031). The hexahedron axes are
/// (radial, theta, z), a right-handed triple. Patches: `inner`, `outer`, `zmin`, `zmax` (all walls by default). The
/// outer and inner polygons depend on `n_theta` only, so two blocks with equal `n_theta` and different `n_r` / `n_z`
/// cover each other at a z plane (non-conformal join, [`crate::non_conformal`]).
#[allow(clippy::too_many_arguments)]
pub fn annulus_3d(
    n_theta: usize,
    n_r: usize,
    n_z: usize,
    r_in: f64,
    r_out: f64,
    height: f64,
    z0: f64,
    grading: Grading,
) -> Result<Mesh> {
    if n_theta < 3 || n_r < 1 || n_z < 1 || r_in <= 0.0 || r_out <= r_in || height <= 0.0 {
        return Err(Error::Mesh(
            "annulus_3d needs n_theta >= 3, n_r >= 1, n_z >= 1, 0 < r_in < r_out and height > 0".into(),
        ));
    }
    let rs: Vec<f64> = axis_coords(n_r, r_out - r_in, grading)?
        .into_iter()
        .map(|r| r + r_in)
        .collect();
    let zs = axis_coords(n_z, height, Grading::Uniform)?;
    let pid = |ir: usize, it: usize, k: usize| ir + (n_r + 1) * ((it % n_theta) + n_theta * k);
    let mut points = Vec::new();
    for &z in &zs {
        for it in 0..n_theta {
            let th = 2.0 * std::f64::consts::PI * it as f64 / n_theta as f64;
            for &r in &rs {
                points.push([r * th.cos(), r * th.sin(), z0 + z]);
            }
        }
    }
    let (mut cells, mut inner, mut outer, mut zmin, mut zmax) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for k in 0..n_z {
        for it in 0..n_theta {
            for ir in 0..n_r {
                let mut p = [0usize; 8];
                for (c, pc) in p.iter_mut().enumerate() {
                    *pc = pid(ir + (c & 1), it + ((c >> 1) & 1), k + (c >> 2));
                }
                let faces = hex_faces(p);
                if ir == 0 {
                    inner.push(faces[0].clone());
                }
                if ir + 1 == n_r {
                    outer.push(faces[1].clone());
                }
                if k == 0 {
                    zmin.push(faces[4].clone());
                }
                if k + 1 == n_z {
                    zmax.push(faces[5].clone());
                }
                cells.push(faces);
            }
        }
    }
    let spec = |name: &str, faces: Vec<Vec<usize>>| BoundarySpec {
        name: name.into(),
        kind: PatchKind::Wall,
        faces,
    };
    let specs = vec![
        spec("inner", inner),
        spec("outer", outer),
        spec("zmin", zmin),
        spec("zmax", zmax),
    ];
    Mesh::from_cell_faces(points, &cells, &specs)
}

/// 3-D annular sector `r_in <= r <= r_out`, `0 <= theta <= angle`, `0 <= z <= height` about the z axis (ADR 0116): the
/// hexahedra of [`annulus_3d`] (axes radial, theta, z) over `n_theta` chords of `angle / n_theta`, cut open at theta = 0 and
/// theta = angle. Every face is planar (chord rectangles on the walls), so an MRF zone's frame flux closes per cell to
/// round-off. Patches: `inner`, `outer`, `zmin`, `zmax` (walls) and `theta_min` / `theta_max`, a rotationally cyclic pair
/// (rotation `angle` about z).
#[allow(clippy::too_many_arguments)]
pub fn annulus_3d_sector(
    n_theta: usize,
    n_r: usize,
    n_z: usize,
    r_in: f64,
    r_out: f64,
    height: f64,
    angle: f64,
    grading: Grading,
) -> Result<Mesh> {
    if n_theta < 1
        || n_r < 1
        || n_z < 1
        || !(r_in > 0.0 && r_out > r_in && height > 0.0 && angle > 0.0 && angle < 2.0 * std::f64::consts::PI)
    {
        return Err(Error::Mesh(
            "annulus_3d_sector needs n_theta, n_r, n_z >= 1, 0 < r_in < r_out, height > 0 and 0 < angle < 2 pi".into(),
        ));
    }
    let rs: Vec<f64> = axis_coords(n_r, r_out - r_in, grading)?
        .into_iter()
        .map(|r| r + r_in)
        .collect();
    let zs = axis_coords(n_z, height, Grading::Uniform)?;
    let pid = |ir: usize, it: usize, k: usize| ir + (n_r + 1) * (it + (n_theta + 1) * k);
    let mut points = Vec::new();
    for &z in &zs {
        for it in 0..=n_theta {
            let th = angle * it as f64 / n_theta as f64;
            for &r in &rs {
                points.push([r * th.cos(), r * th.sin(), z]);
            }
        }
    }
    let mut patches: [Vec<Vec<usize>>; 6] = Default::default();
    let mut cells = Vec::new();
    for k in 0..n_z {
        for it in 0..n_theta {
            for ir in 0..n_r {
                let mut p = [0usize; 8];
                for (c, pc) in p.iter_mut().enumerate() {
                    *pc = pid(ir + (c & 1), it + ((c >> 1) & 1), k + (c >> 2));
                }
                let faces = hex_faces(p);
                // hex_faces order: r-min, r-max, theta-min, theta-max, z-min, z-max
                let on = [ir == 0, ir + 1 == n_r, it == 0, it + 1 == n_theta, k == 0, k + 1 == n_z];
                for (side, &b) in on.iter().enumerate() {
                    if b {
                        patches[side].push(faces[side].clone());
                    }
                }
                cells.push(faces);
            }
        }
    }
    let [inner, outer, tmin, tmax, zmin, zmax] = patches;
    let spec = |name: &str, kind: PatchKind, faces: Vec<Vec<usize>>| BoundarySpec {
        name: name.into(),
        kind,
        faces,
    };
    let specs = vec![
        spec("inner", PatchKind::Wall, inner),
        spec("outer", PatchKind::Wall, outer),
        spec("zmin", PatchKind::Wall, zmin),
        spec("zmax", PatchKind::Wall, zmax),
        spec("theta_min", PatchKind::Generic, tmin),
        spec("theta_max", PatchKind::Generic, tmax),
    ];
    let mut m = Mesh::from_cell_faces(points, &cells, &specs)?;
    m.set_periodic_rotation("theta_min", "theta_max", [0.0; 3], [0.0, 0.0, 1.0], angle)?;
    Ok(m)
}

/// One block of the staged annulus: points and hex cells of `annulus_3d` with the patch faces split by side, appended to
/// `points` (indices offset) and `cells`.
#[allow(clippy::too_many_arguments)]
fn annulus_block(
    points: &mut Vec<Vec3>,
    cells: &mut Vec<Vec<Vec<usize>>>,
    n_theta: usize,
    n_r: usize,
    n_z: usize,
    rs: &[f64],
    height: f64,
    z0: f64,
) -> Result<[Vec<Vec<usize>>; 4]> {
    let zs = axis_coords(n_z, height, Grading::Uniform)?;
    let base = points.len();
    let pid = |ir: usize, it: usize, k: usize| base + ir + (n_r + 1) * ((it % n_theta) + n_theta * k);
    for &z in &zs {
        for it in 0..n_theta {
            let th = 2.0 * std::f64::consts::PI * it as f64 / n_theta as f64;
            for &r in rs {
                points.push([r * th.cos(), r * th.sin(), z0 + z]);
            }
        }
    }
    let (mut inner, mut outer, mut lo, mut hi) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for k in 0..n_z {
        for it in 0..n_theta {
            for ir in 0..n_r {
                let mut p = [0usize; 8];
                for (c, pc) in p.iter_mut().enumerate() {
                    *pc = pid(ir + (c & 1), it + ((c >> 1) & 1), k + (c >> 2));
                }
                let faces = hex_faces(p);
                if ir == 0 {
                    inner.push(faces[0].clone());
                }
                if ir + 1 == n_r {
                    outer.push(faces[1].clone());
                }
                if k == 0 {
                    lo.push(faces[4].clone());
                }
                if k + 1 == n_z {
                    hi.push(faces[5].clone());
                }
                cells.push(faces);
            }
        }
    }
    Ok([inner, outer, lo, hi])
}

/// Two annular blocks `r_in <= r <= r_out` about the z axis that are NOT connected (a stage for a mixing plane, F-126,
/// ADR 0059): the lower block of `(n_theta, n_r, n_z)` cells over `0 <= z <= height/2`, the upper block of
/// `(n_theta_up, n_r_up, n_z_up)` cells over `height/2 <= z <= height`, with the radial grading `grading` in both. Patches:
/// `inner` and `outer` (both blocks), `zmin` (lower bottom), `plane_out` (lower top), `plane_in` (upper bottom), `zmax`
/// (upper top); no face joins the blocks. A mixing plane (or nothing at all) has to couple `plane_out` and `plane_in`.
#[allow(clippy::too_many_arguments)]
pub fn annulus_3d_stage(
    n_theta: usize,
    n_r: usize,
    n_z: usize,
    n_theta_up: usize,
    n_r_up: usize,
    n_z_up: usize,
    r_in: f64,
    r_out: f64,
    height: f64,
    grading: Grading,
) -> Result<Mesh> {
    if n_theta < 3
        || n_theta_up < 3
        || n_r < 1
        || n_r_up < 1
        || n_z < 1
        || n_z_up < 1
        || r_in <= 0.0
        || r_out <= r_in
        || height <= 0.0
    {
        return Err(Error::Mesh(
            "annulus_3d_stage needs at least 3 cells around, 1 radial and 1 axial cell in each block, 0 < r_in < r_out and height > 0"
                .into(),
        ));
    }
    let radii = |n: usize| -> Result<Vec<f64>> {
        Ok(axis_coords(n, r_out - r_in, grading)?
            .into_iter()
            .map(|r| r + r_in)
            .collect())
    };
    let half = 0.5 * height;
    let mut points = Vec::new();
    let mut cells = Vec::new();
    let [i1, o1, zmin, plane_out] = annulus_block(&mut points, &mut cells, n_theta, n_r, n_z, &radii(n_r)?, half, 0.0)?;
    let [i2, o2, plane_in, zmax] = annulus_block(
        &mut points,
        &mut cells,
        n_theta_up,
        n_r_up,
        n_z_up,
        &radii(n_r_up)?,
        half,
        half,
    )?;
    let spec = |name: &str, faces: Vec<Vec<usize>>| BoundarySpec {
        name: name.into(),
        kind: PatchKind::Wall,
        faces,
    };
    let join = |mut a: Vec<Vec<usize>>, b: Vec<Vec<usize>>| {
        a.extend(b);
        a
    };
    let specs = vec![
        spec("inner", join(i1, i2)),
        spec("outer", join(o1, o2)),
        spec("zmin", zmin),
        spec("plane_out", plane_out),
        spec("plane_in", plane_in),
        spec("zmax", zmax),
    ];
    Mesh::from_cell_faces(points, &cells, &specs)
}

/// Annular sector `r_in <= r <= r_out`, `0 <= theta <= angle` about the z axis
/// (2D, one cell in z) for rotational-periodicity work. Patches: `inner`,
/// `outer` (walls), `theta_min` / `theta_max` (a rotationally cyclic pair, the
/// rotation being `angle` about z) and `frontAndBack` (empty).
pub fn annulus_sector(
    n_theta: usize,
    n_r: usize,
    r_in: f64,
    r_out: f64,
    angle: f64,
    grading: Grading,
    depth: f64,
) -> Result<Mesh> {
    if n_theta < 1 || r_in <= 0.0 || r_out <= r_in || angle <= 0.0 || angle >= 2.0 * std::f64::consts::PI {
        return Err(Error::Mesh(
            "annulus_sector needs n_theta >= 1, 0 < r_in < r_out and 0 < angle < 2 pi".into(),
        ));
    }
    let rs: Vec<f64> = axis_coords(n_r, r_out - r_in, grading)?
        .into_iter()
        .map(|r| r + r_in)
        .collect();
    let pid = |it: usize, ir: usize, k: usize| it + (n_theta + 1) * (ir + (n_r + 1) * k);
    let mut points = Vec::new();
    for k in 0..2 {
        for &r in &rs {
            for it in 0..=n_theta {
                let th = angle * it as f64 / n_theta as f64;
                points.push([r * th.cos(), r * th.sin(), depth * k as f64]);
            }
        }
    }
    let (mut cells, mut inner, mut outer, mut tmin, mut tmax, mut fb) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for ir in 0..n_r {
        for it in 0..n_theta {
            let mut p = [0usize; 8];
            for (c, pc) in p.iter_mut().enumerate() {
                *pc = pid(it + (c & 1), ir + ((c >> 1) & 1), c >> 2);
            }
            let faces = hex_faces(p);
            if it == 0 {
                tmin.push(faces[0].clone());
            }
            if it + 1 == n_theta {
                tmax.push(faces[1].clone());
            }
            if ir == 0 {
                inner.push(faces[2].clone());
            }
            if ir + 1 == n_r {
                outer.push(faces[3].clone());
            }
            fb.push(faces[4].clone());
            fb.push(faces[5].clone());
            cells.push(faces);
        }
    }
    let spec = |name: &str, kind: PatchKind, faces: Vec<Vec<usize>>| BoundarySpec {
        name: name.into(),
        kind,
        faces,
    };
    let specs = vec![
        spec("inner", PatchKind::Wall, inner),
        spec("outer", PatchKind::Wall, outer),
        spec("theta_min", PatchKind::Generic, tmin),
        spec("theta_max", PatchKind::Generic, tmax),
        spec("frontAndBack", PatchKind::Empty, fb),
    ];
    let mut m = Mesh::from_cell_faces(points, &cells, &specs)?;
    m.set_periodic_rotation("theta_min", "theta_max", [0.0; 3], [0.0, 0.0, 1.0], angle)?;
    Ok(m)
}

/// Returns a copy of `mesh` with every point moved by `map` (topology unchanged).
/// Useful for building smoothly distorted meshes for convergence studies.
pub fn warp(mesh: &Mesh, map: impl Fn(Vec3) -> Vec3) -> Result<Mesh> {
    let mut topo = mesh.topology().clone();
    for p in topo.points.iter_mut() {
        *p = map(*p);
    }
    Mesh::from_topology(topo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quality::check_mesh;

    #[test]
    fn annulus_3d_volume_and_patches() {
        let m = annulus_3d(48, 5, 3, 0.5, 1.0, 0.3, 0.0, Grading::Uniform).unwrap();
        assert_eq!(m.n_cells(), 48 * 5 * 3);
        assert!(m.cell_volumes().iter().all(|&v| v > 0.0));
        // inscribed polygon area pi-like: n/2 sin(2 pi/n) (ro^2 - ri^2) * height
        let n = 48.0_f64;
        let exact = 0.5 * n * (2.0 * std::f64::consts::PI / n).sin() * (1.0 - 0.25) * 0.3;
        assert!((m.total_volume() - exact).abs() < 1e-12 * exact);
        for name in ["inner", "outer", "zmin", "zmax"] {
            assert!(m.patch(name).is_some(), "{name}");
        }
    }

    /// ADR 0116 G1: volume and patch areas of `annulus_3d_sector` against the closed forms of the polygonal (chord) sector,
    /// relative error <= 1e-12 each, uniform and radially graded.
    #[test]
    fn annulus_3d_sector_g1_polygonal_volume_and_areas() {
        let (n, ri, ro, lz, angle) = (8usize, 0.124, 0.25, 0.4, 40f64.to_radians());
        let dt = angle / n as f64;
        let ring = 0.5 * n as f64 * dt.sin() * (ro * ro - ri * ri);
        let chord = |r: f64| n as f64 * 2.0 * r * (0.5 * dt).sin() * lz;
        for g in [Grading::Uniform, Grading::Expansion(3.0)] {
            let m = annulus_3d_sector(n, 10, 24, ri, ro, lz, angle, g).unwrap();
            assert_eq!(m.n_cells(), n * 10 * 24);
            let area = |p: &str| m.patch(p).unwrap().faces().map(|f| m.face_area(f)).sum::<f64>();
            let checks = [
                ("volume", m.total_volume(), ring * lz),
                ("inner", area("inner"), chord(ri)),
                ("outer", area("outer"), chord(ro)),
                ("zmin", area("zmin"), ring),
                ("zmax", area("zmax"), ring),
                ("theta_min", area("theta_min"), (ro - ri) * lz),
                ("theta_max", area("theta_max"), (ro - ri) * lz),
            ];
            for (name, got, exact) in checks {
                let rel = (got - exact).abs() / exact;
                println!("G1 {g:?} {name}: relative error {rel:.3e} (limit 1e-12)");
                assert!(rel <= 1e-12, "{name}");
            }
        }
        println!("G1 reported: sin(dt) / dt = {:.5}", dt.sin() / dt);
    }

    /// ADR 0116 N2 (negative control of G1, run first): the volume against the curved sector must miss 1e-12 by at least 10x.
    #[test]
    fn annulus_3d_sector_n2_curved_reference_misses() {
        let (ri, ro, lz, angle) = (0.124, 0.25, 0.4, 40f64.to_radians());
        for g in [Grading::Uniform, Grading::Expansion(3.0)] {
            let m = annulus_3d_sector(8, 10, 24, ri, ro, lz, angle, g).unwrap();
            let curved = 0.5 * angle * (ro * ro - ri * ri) * lz;
            let n2 = (m.total_volume() - curved).abs() / curved;
            println!("N2 {g:?}: volume against the curved sector {n2:.3e} (must be >= 1e-11)");
            assert!(n2 >= 1e-11);
        }
    }

    /// ADR 0116 G7 (library part): invalid counts, radii, height and angles are errors.
    #[test]
    fn annulus_3d_sector_g7_invalid_arguments() {
        let (u, a) = (Grading::Uniform, 1.0);
        let pi2 = 2.0 * std::f64::consts::PI;
        for (nt, nr, nz, ri, ro, h, ang) in [
            (0, 2, 2, 0.5, 1.0, 1.0, a),
            (2, 0, 2, 0.5, 1.0, 1.0, a),
            (2, 2, 0, 0.5, 1.0, 1.0, a),
            (2, 2, 2, 0.0, 1.0, 1.0, a),
            (2, 2, 2, 1.0, 1.0, 1.0, a),
            (2, 2, 2, 0.5, 1.0, 0.0, a),
            (2, 2, 2, 0.5, 1.0, 1.0, 0.0),
            (2, 2, 2, 0.5, 1.0, 1.0, pi2),
            (2, 2, 2, 0.5, 1.0, 1.0, f64::NAN),
        ] {
            assert!(annulus_3d_sector(nt, nr, nz, ri, ro, h, ang, u).is_err());
        }
        assert!(annulus_3d_sector(1, 1, 1, 0.5, 1.0, 1.0, a, u).is_ok());
    }

    #[test]
    fn axis_coordinates() {
        let x = axis_coords(4, 2.0, Grading::Uniform).unwrap();
        assert_eq!(x, vec![0.0, 0.5, 1.0, 1.5, 2.0]);
        let x = axis_coords(3, 7.0, Grading::Expansion(4.0)).unwrap();
        // sizes 1, 2, 4
        assert!((x[1] - 1.0).abs() < 1e-12 && (x[2] - 3.0).abs() < 1e-12 && (x[3] - 7.0).abs() < 1e-12);
        assert!(axis_coords(0, 1.0, Grading::Uniform).is_err());
        assert!(axis_coords(2, 1.0, Grading::Expansion(-1.0)).is_err());
        let s = axis_coords(6, 1.0, Grading::Symmetric(4.0)).unwrap();
        assert_eq!(s.len(), 7);
        assert!((s[3] - 0.5).abs() < 1e-12);
        let d: Vec<f64> = s.windows(2).map(|w| w[1] - w[0]).collect();
        assert!((d[0] - d[5]).abs() < 1e-12 && d[0] < d[2]);
    }

    #[test]
    fn box_counts_and_volume() {
        let m = box_mesh([3, 4, 5], [1.0, 2.0, 3.0], [Grading::Uniform; 3]).unwrap();
        assert_eq!(m.n_cells(), 60);
        assert_eq!(m.n_points(), 4 * 5 * 6);
        assert_eq!(m.n_faces(), 94 + 133);
        assert!((m.total_volume() - 6.0).abs() < 1e-12);
        let names: Vec<_> = m.patches().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"]);
        assert_eq!(m.patch("xmin").unwrap().size, 20);
        assert_eq!(m.patch("zmax").unwrap().size, 12);
        assert_eq!(m.n_internal_faces(), 2 * 4 * 5 + 3 * 3 * 5 + 3 * 4 * 4);
        assert!(check_mesh(&m).is_ok());
    }

    #[test]
    fn cavity_and_channel_patches() {
        let m = cavity_2d(8, 1.0).unwrap();
        assert_eq!(m.n_cells(), 64);
        assert_eq!(m.patch("lid").unwrap().size, 8);
        assert_eq!(m.patch("walls").unwrap().size, 24);
        assert_eq!(m.patch("frontAndBack").unwrap().size, 128);
        assert_eq!(m.patch("frontAndBack").unwrap().kind, PatchKind::Empty);
        assert!((m.total_volume() - 1.0 / 8.0).abs() < 1e-12);
        let c = channel(
            [10, 4, 1],
            [5.0, 1.0, 0.1],
            [Grading::Uniform, Grading::Symmetric(5.0), Grading::Uniform],
            true,
        )
        .unwrap();
        assert_eq!(c.patch("inlet").unwrap().size, 4);
        assert_eq!(c.patch("outlet").unwrap().kind, PatchKind::Outlet);
        assert_eq!(c.patch("walls").unwrap().size, 20);
        assert!((c.total_volume() - 0.5).abs() < 1e-12);
        assert!(check_mesh(&c).is_ok());
    }

    #[test]
    fn backward_step_domain() {
        let s = StepSpec {
            l_in: 2.0,
            l_out: 6.0,
            h_in: 1.0,
            step: 0.5,
            depth: 0.1,
            n: [4, 12, 5, 3],
        };
        let m = backward_step(s).unwrap();
        // cells: upstream 4*5 + downstream 12*(5+3)
        assert_eq!(m.n_cells(), 4 * 5 + 12 * 8);
        let area = 2.0 * 1.0 + 6.0 * 1.5;
        assert!((m.total_volume() - area * 0.1).abs() < 1e-12);
        assert_eq!(m.patch("inlet").unwrap().size, 5);
        assert_eq!(m.patch("outlet").unwrap().size, 8);
        // walls: upstream top (4), upstream floor of inlet channel at y=step (4), step face (3),
        // downstream floor (12), downstream top (12)
        assert_eq!(m.patch("walls").unwrap().size, 4 + 4 + 3 + 12 + 12);
        assert!(check_mesh(&m).is_ok());
    }

    #[test]
    fn graded_step_clusters_and_keeps_topology() {
        let spec = StepSpec::from_expansion_ratio(1.125, 1.0, 5.0, 20.0, 0.1, [10, 40, 16, 6]).unwrap();
        assert!((spec.h_in - 8.0).abs() < 1e-12 && (spec.expansion_ratio() - 1.125).abs() < 1e-12);
        assert!(StepSpec::from_expansion_ratio(1.0, 1.0, 1.0, 1.0, 0.1, [1; 4]).is_err());
        let g = StepGrading::clustered(8.0, 6.0);
        let m = backward_step_graded(spec, g).unwrap();
        let u = backward_step(spec).unwrap();
        assert_eq!(m.n_cells(), u.n_cells());
        assert_eq!(m.n_faces(), u.n_faces());
        assert!((m.total_volume() - u.total_volume()).abs() < 1e-10);
        assert_eq!(m.patch("walls").unwrap().size, u.patch("walls").unwrap().size);
        assert!(check_mesh(&m).is_ok());
        // smallest x cell at the step face (x = l_in) on the downstream side, ratio 8
        let cc = m.cell_centres();
        let mut xs: Vec<f64> = cc.iter().filter(|c| c[1] < 0.5).map(|c| c[0]).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        let dx: Vec<f64> = xs.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(dx[0] < dx[dx.len() - 1] / 5.0, "{dx:?}");
        // floor cell much thinner than mid-height cell in the step region
        let mut ys: Vec<f64> = cc.iter().filter(|c| c[0] > 6.0).map(|c| c[1]).collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ys.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        assert!(ys[0] < 0.1 && ys[1] - ys[0] > 2.0 * ys[0], "{ys:?}");
    }

    #[test]
    fn cylinder_ogrid_area_matches_polygon_ring() {
        let (nt, nr, r0, r1) = (32, 10, 0.5, 5.0);
        let m = cylinder_ogrid(nt, nr, r0, r1, Grading::Expansion(10.0), 1.0).unwrap();
        assert_eq!(m.n_cells(), nt * nr);
        // vertices at radii r_i on n-gons: ring area = (n/2) sin(2 pi/n) (r1^2 - r0^2)
        let n = nt as f64;
        let exact = 0.5 * n * (2.0 * std::f64::consts::PI / n).sin() * (r1 * r1 - r0 * r0);
        assert!((m.total_volume() - exact).abs() < 1e-10 * exact);
        // and approaches the continuous annulus
        let circle = std::f64::consts::PI * (r1 * r1 - r0 * r0);
        assert!((m.total_volume() - circle).abs() / circle < 0.01);
        assert_eq!(m.patch("cylinder").unwrap().size, nt);
        assert_eq!(m.patch("farfield").unwrap().size, nt);
        assert_eq!(m.n_internal_faces(), nt * (nr - 1) + nt * nr);
        let r = check_mesh(&m);
        assert!(r.is_ok(), "{r:?}");
        assert!(r.max_non_orthogonality < 10.0);
        assert!(cylinder_ogrid(2, 4, 1.0, 2.0, Grading::Uniform, 1.0).is_err());
    }
}
