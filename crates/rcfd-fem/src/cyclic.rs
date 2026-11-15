//! Cyclic symmetry: sector constraints ([`crate::model::Constraint::Cyclic`]), modes by harmonic index, expansion to the
//! full rotor. Declared by ADR 0291; filled by ADR 0386 (the registration of F-394; plan ADR 0297).
//!
//! A rotor of `N` identical sectors is solved on one sector. The high boundary of the sector is its low boundary rotated by
//! `alpha = 2 pi / N` about the axis (right-hand sense): [`boundary`] matches the two node for node within
//! [`MATCH_TOLERANCE`] and refuses the first unmatched node. With the cosine part `a` and the sine part `b` of the complex
//! sector amplitude of harmonic index `k` (`theta = k alpha`) the condition between the sectors is the real pair of rows
//! `a_H = R (cos theta a_L - sin theta b_L)`, `b_H = R (sin theta a_L + cos theta b_L)` (ADR 0386, derived there). They are
//! rows of the transformation `x = T q` of ADR 0296: for `k = 0` the rows `u_H = R u_L` are part of
//! [`crate::mpc::transformation`] (statics with a load that repeats in every sector, and the `k = 0` modes); for `0 < 2 k <
//! N` the problem over `(a, b)` has twice the size and every eigenvalue twice (a pair of full modes); for `2 k = N` the rows
//! `a_H = -R a_L` act on `a` alone ([`crate::mpc::harmonic_transformation`], [`modal_analysis_harmonic`]). [`full_rotor`]
//! copies the sector mesh to the full rotor and [`FullRotor::expand`] expands a sector mode to it, M-orthonormal.
//!
//! ADR 0395 (F-489): a static load of harmonic index `k` (a cosine and a sine part per sector) is solved on the sector by the
//! same rows ([`solve_static_harmonic`]; [`FullRotor::expand_load`] and [`FullRotor::expand_field`] give the full load and
//! the full solution), and a rigid set acts at `k > 0` with its six reference unknowns doubled (one block per part).
//!
//! ADR 0462 (F-544): the modal and the static harmonic paths take the orthotropic and the tabulated material of ADR 0452 when
//! every material with its own axes has cylindrical axes about the cyclic axis and origin (the material is then the same in
//! every sector); other axes are refused by name ([`crate::mat_ortho::check_cyclic_axes`]).
//!
//! ADR 0556 (F-628): with [`crate::model::CyclicOptions::shells`] the boundaries are node sets, the static and the modal
//! paths (on the transformation and harmonic) take Shell4 and Shell3, and the rotation unknowns of the matched six-unknown
//! nodes are rotated by `R` as the translations.
//!
//! ADR 0561 (F-647): with the same option [`full_rotor`] copies Shell4 and Shell3 blocks, and [`FullRotor::expand`],
//! [`FullRotor::expand_field`] and [`FullRotor::expand_load`] expand the rotations of a six-unknown node by the same `R^j` as
//! its translations; the rotating sector's whirl paths take the shells' Coriolis matrix ([`crate::rotating`]).
//!
//! ADR 0575 (F-648): with [`crate::model::CyclicOptions::shell_extensions`] as well, the shells option takes the extensions of
//! ADR 0456 D4: interpolated high nodes follow the low shell edges rotated by `R` (node to edge, linear weights), and the
//! rotation unknowns of interpolated and axis nodes get the rows of the translations.
//!
//! ADR 0583 (F-662): on such a boundary the low nodes the high ties leave undetermined (the support of the null space of
//! the weight matrix) follow the high shell edges rotated back by `R^T`, and the high nodes whose weights lie wholly on them
//! are released as masters ([`CyclicBoundary::two_sided`]): where the low side is the finer, it is the dependent side.
//!
//! ADR 0595 (F-672): with [`crate::model::CyclicOptions::beams`] as well, the static and the modal paths take Beam2 elements
//! (the rows of ADR 0556 D2 on their six-unknown nodes; the sector stands for the ring of its beams rotated by `R^j`,
//! orientation vectors included).
//!
//! ADR 0601 (F-674): with [`crate::model::CyclicOptions::full_rotor_beams`] as well, [`full_rotor`] takes Beam2 blocks: each
//! becomes `N` consecutive blocks of the full mesh, copy `j` with the orientation `R^j v` ([`FullRotor::block`]).

use crate::assemble::{point_stiffness, stiffness_with};
use crate::constraint::Prescribed;
use crate::error::{FemError, Result, SetKind};
use crate::loads::NodalForceAt;
use crate::mass::consistent_with;
use crate::mesh::{BlockProperty, ElementKind, FaceRef, SolidMesh};
use crate::modal::{frequency, ModalOptions};
use crate::model::{Constraint, DofMap, SolidModel};
use crate::mpc::{self, ReferenceLoad, ReferenceMotion, Transformation, REFERENCE_UNKNOWNS};
use crate::solve::{StaticOptions, StaticSolver, PIVOT_TOLERANCE};
use rcfd_core::vec3::Vec3;
use rcfd_linalg::eigen::{lowest_eigenpairs, Shift, SturmCheck, SubspaceOptions};
use rcfd_linalg::skyline::{SkylineError, SkylineLdlt};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// The work package that adds cyclic symmetry; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// Relative tolerance of the boundary match (ADR 0386 D1): the image of a low node must lie within `MATCH_TOLERANCE L` of a
/// high node, `L` the largest distance of a node of the two boundaries from the axis.
pub const MATCH_TOLERANCE: f64 = 1e-8;

/// A 3 x 3 matrix, rows first.
pub type Mat3 = [[f64; 3]; 3];

/// The rotation by `angle` about the unit vector `n`: `I + sin(angle) [n]x + (1 - cos(angle)) [n]x^2` (Rodrigues).
pub fn rotation_matrix(n: Vec3, angle: f64) -> Mat3 {
    let k = [[0.0, -n[2], n[1]], [n[2], 0.0, -n[0]], [-n[1], n[0], 0.0]];
    let (s, c1) = (angle.sin(), 1.0 - angle.cos());
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let k2: f64 = (0..3).map(|m| k[i][m] * k[m][j]).sum();
            r[i][j] = if i == j { 1.0 } else { 0.0 } + s * k[i][j] + c1 * k2;
        }
    }
    r
}

fn apply(r: &Mat3, v: Vec3) -> Vec3 {
    [
        r[0][0] * v[0] + r[0][1] * v[1] + r[0][2] * v[2],
        r[1][0] * v[0] + r[1][1] * v[1] + r[1][2] * v[2],
        r[2][0] * v[0] + r[2][1] * v[1] + r[2][2] * v[2],
    ]
}

fn rotate_point(r: &Mat3, origin: Vec3, x: Vec3) -> Vec3 {
    let y = apply(r, [x[0] - origin[0], x[1] - origin[1], x[2] - origin[2]]);
    [origin[0] + y[0], origin[1] + y[1], origin[2] + y[2]]
}

fn distance(a: Vec3, b: Vec3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn cyclic_error(reason: String) -> FemError {
    FemError::InvalidValue { key: "cyclic", reason }
}

/// The matched boundaries of a cyclic constraint.
#[derive(Debug, Clone, PartialEq)]
pub struct CyclicBoundary {
    constraint: usize,
    sectors: usize,
    axis: Vec3,
    origin: Vec3,
    rotation: Mat3,
    pairs: Vec<(usize, usize)>,
    length: f64,
    tolerance: f64,
    axis_nodes: Vec<usize>,
    interpolated: Vec<(usize, Vec<(usize, f64)>)>,
    two_sided: Vec<TwoSidedNode>,
}

/// A low node of the two-sided shell tie of ADR 0583 D2: the node, its weights over low nodes (taken as they are) and over
/// released high nodes (taken through `e^{-i theta} R^T`).
pub type TwoSidedNode = (usize, Vec<(usize, f64)>, Vec<(usize, f64)>);

impl CyclicBoundary {
    /// Index of the constraint in [`SolidModel::constraints`].
    pub fn constraint(&self) -> usize {
        self.constraint
    }
    /// Number of sectors `N`.
    pub fn sectors(&self) -> usize {
        self.sectors
    }
    /// The unit axis.
    pub fn axis(&self) -> Vec3 {
        self.axis
    }
    /// The point on the axis.
    pub fn origin(&self) -> Vec3 {
        self.origin
    }
    /// The rotation `R` by `2 pi / N` about the axis.
    pub fn rotation(&self) -> &Mat3 {
        &self.rotation
    }
    /// The matched nodes `(low, high)`, ascending in the low node.
    pub fn pairs(&self) -> &[(usize, usize)] {
        &self.pairs
    }
    /// The length `L`: the largest distance of a boundary node from the axis.
    pub fn length(&self) -> f64 {
        self.length
    }
    /// The absolute tolerance of the match, `MATCH_TOLERANCE L`.
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }
    /// The nodes on the axis (on both boundaries; ADR 0456 D4), ascending; empty without the option.
    pub fn axis_nodes(&self) -> &[usize] {
        &self.axis_nodes
    }
    /// The interpolated high nodes with their weights over low nodes (ADR 0456 D4), ascending in the high node; empty
    /// without the option (then [`CyclicBoundary::pairs`] holds the match).
    pub fn interpolated(&self) -> &[(usize, Vec<(usize, f64)>)] {
        &self.interpolated
    }
    /// The low nodes of an interpolated shell boundary that the high ties leave undetermined, tied to the high shell edges
    /// rotated back (ADR 0583 D2), ascending in the low node; empty unless such nodes exist.
    pub fn two_sided(&self) -> &[TwoSidedNode] {
        &self.two_sided
    }
    /// Whether the boundary uses an extension of ADR 0456.
    pub fn is_extended(&self) -> bool {
        !self.axis_nodes.is_empty() || !self.interpolated.is_empty() || !self.two_sided.is_empty()
    }
}

/// Whether a model has a cyclic constraint with the shells option of ADR 0556 D1.
pub(crate) fn shells_opted_in(model: &SolidModel) -> bool {
    model.constraints().iter().any(|c| {
        matches!(
            c,
            Constraint::Cyclic {
                options: crate::model::CyclicOptions { shells: true, .. },
                ..
            }
        )
    })
}

/// Whether a model has a cyclic constraint with the shells and the beams options of ADR 0595 D1.
pub(crate) fn beams_opted_in(model: &SolidModel) -> bool {
    model.constraints().iter().any(|c| {
        matches!(
            c,
            Constraint::Cyclic {
                options: crate::model::CyclicOptions {
                    shells: true,
                    beams: true,
                    ..
                },
                ..
            }
        )
    })
}

/// Whether a model has a cyclic constraint with the shells, beams and full-rotor options of ADR 0601 D1.
fn full_rotor_beams_opted_in(model: &SolidModel) -> bool {
    model.constraints().iter().any(|c| {
        matches!(
            c,
            Constraint::Cyclic {
                options: crate::model::CyclicOptions {
                    shells: true,
                    beams: true,
                    full_rotor_beams: true,
                    ..
                },
                ..
            }
        )
    })
}

/// ADR 0595 D1: the beams option qualifies the shells option and is not taken with interpolated boundaries or nodes on the
/// axis. A no-op for a constraint without it.
pub(crate) fn check_beams_option(model: &SolidModel) -> Result<()> {
    for (i, c) in model.constraints().iter().enumerate() {
        if let Constraint::Cyclic { options, .. } = c {
            if !options.beams {
                continue;
            }
            if !options.shells {
                return Err(cyclic_error(format!(
                    "constraint {i}: the beams option qualifies the shells option of the cyclic constraint (ADR 0595)"
                )));
            }
            if options.interpolation.is_some() || options.axis_nodes {
                return Err(cyclic_error(format!(
                    "constraint {i}: Beam2 in the cyclic constraint (ADR 0595) is not taken with interpolated \
                     boundaries or nodes on the axis"
                )));
            }
        }
    }
    Ok(())
}

/// The index of the one cyclic constraint of a model; none or more than one is refused.
pub fn the_cyclic_constraint(model: &SolidModel) -> Result<usize> {
    let list: Vec<usize> = model
        .constraints()
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, Constraint::Cyclic { .. }))
        .map(|(i, _)| i)
        .collect();
    match list.as_slice() {
        &[i] => Ok(i),
        _ => Err(cyclic_error(format!(
            "a cyclic analysis needs exactly one Cyclic constraint; the model has {}",
            list.len()
        ))),
    }
}

/// Matches the boundaries of cyclic constraint `constraint` (ADR 0386 D1): each low node's image under the rotation by `2 pi
/// / N` must lie within `MATCH_TOLERANCE L` of exactly one high node, and every high node must be such an image. Refuses a
/// node on both boundaries, the first low node without a partner (naming it, its image and the nearest high node), a high
/// node taken twice and the first high node left without a partner.
pub fn boundary(model: &SolidModel, constraint: usize) -> Result<CyclicBoundary> {
    let Some(Constraint::Cyclic {
        low,
        high,
        axis,
        origin,
        sectors,
        options,
    }) = model.constraints().get(constraint)
    else {
        return Err(cyclic_error(format!(
            "constraint {constraint} is not a Cyclic constraint"
        )));
    };
    if *sectors < 2 {
        return Err(FemError::InvalidValue {
            key: "sectors",
            reason: format!("{sectors} sectors: a cyclic rotor has at least 2"),
        });
    }
    if options.beams {
        check_beams_option(model)?;
    }
    let mesh = model.mesh();
    // ADR 0556 D1: with shells the boundaries are node sets, and the extensions of ADR 0456 are not taken.
    let (lows, highs) = if options.shells {
        if (options.interpolation.is_some() || options.axis_nodes) && !options.shell_extensions {
            return Err(cyclic_error(format!(
                "constraint {constraint}: shells in the cyclic constraint (ADR 0556) are not taken with interpolated \
                 boundaries or nodes on the axis (ADR 0456 D4; a follow-up of ADR 0556)"
            )));
        }
        let nodes_of = |name: &str| -> Result<Vec<usize>> {
            let set = mesh.node_set(name).ok_or_else(|| FemError::UnknownSet {
                kind: SetKind::Node,
                name: name.to_string(),
            })?;
            let mut v = set.to_vec();
            v.sort_unstable();
            v.dedup();
            Ok(v)
        };
        (nodes_of(low)?, nodes_of(high)?)
    } else {
        if options.shell_extensions {
            return Err(cyclic_error(format!(
                "constraint {constraint}: shell_extensions qualifies the shells option of the cyclic constraint (ADR 0575)"
            )));
        }
        (mesh.surface_set_nodes(low)?, mesh.surface_set_nodes(high)?)
    };
    let both: Vec<usize> = lows
        .iter()
        .copied()
        .filter(|p| highs.binary_search(p).is_ok())
        .collect();
    if let (Some(&p), false) = (both.first(), options.axis_nodes) {
        return Err(cyclic_error(format!(
            "node {p} is on both boundaries '{low}' and '{high}' (a node on the axis or a set given twice)"
        )));
    }
    let an = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let n = [axis[0] / an, axis[1] / an, axis[2] / an];
    let x = mesh.nodes();
    let mut length: f64 = 0.0;
    for &p in lows.iter().chain(&highs) {
        let d = [x[p][0] - origin[0], x[p][1] - origin[1], x[p][2] - origin[2]];
        let along = d[0] * n[0] + d[1] * n[1] + d[2] * n[2];
        let perp = [d[0] - along * n[0], d[1] - along * n[1], d[2] - along * n[2]];
        length = length.max((perp[0] * perp[0] + perp[1] * perp[1] + perp[2] * perp[2]).sqrt());
    }
    if length == 0.0 {
        return Err(cyclic_error(format!(
            "every node of '{low}' and '{high}' is on the axis"
        )));
    }
    let tolerance = MATCH_TOLERANCE * length;
    // ADR 0456 D4: the nodes on both boundaries must lie on the axis.
    for &p in &both {
        let d = [x[p][0] - origin[0], x[p][1] - origin[1], x[p][2] - origin[2]];
        let along = d[0] * n[0] + d[1] * n[1] + d[2] * n[2];
        let off =
            ((d[0] - along * n[0]).powi(2) + (d[1] - along * n[1]).powi(2) + (d[2] - along * n[2]).powi(2)).sqrt();
        if off > tolerance {
            return Err(cyclic_error(format!(
                "node {p} is on both boundaries '{low}' and '{high}' but {off:e} from the axis (tolerance {tolerance:e})"
            )));
        }
    }
    let axis_nodes = both;
    if let Some(interp) = options.interpolation {
        // ADR 0456 D4: each high node follows the low faces rotated into the high position.
        let rotation = rotation_matrix(n, 2.0 * PI / *sectors as f64);
        let o = *origin;
        let map = |p: Vec3| rotate_point(&rotation, o, p);
        let interpolated = if options.shells {
            // ADR 0575 D3: a shell has no faces; the high nodes follow the low shell edges.
            if interp.method == crate::tie::TieMethod::Mortar {
                return Err(cyclic_error(format!(
                    "constraint {constraint}: the mortar method is not taken for shell edges (ADR 0575; node to edge only)"
                )));
            }
            let slaves: Vec<usize> = highs
                .iter()
                .copied()
                .filter(|h| axis_nodes.binary_search(h).is_err())
                .collect();
            let weights = shell_edge_weights(mesh, constraint, "low", &lows, &slaves, &map, &interp)?;
            // ADR 0583 D2: the low nodes these ties leave undetermined follow the high edges rotated back.
            two_sided_ties(
                model,
                constraint,
                &lows,
                &highs,
                &axis_nodes,
                weights,
                &rotation,
                o,
                &interp,
            )?
        } else {
            let exclude: std::collections::BTreeSet<usize> = axis_nodes.iter().copied().collect();
            let w = crate::tie::weights(mesh, high, low, &map, &interp, &Default::default(), &exclude)?;
            (
                w.rows.into_iter().filter(|r| r.1 == 0).map(|r| (r.0, r.2)).collect(),
                Vec::new(),
            )
        };
        let (interpolated, two_sided) = interpolated;
        return Ok(CyclicBoundary {
            constraint,
            sectors: *sectors,
            axis: n,
            origin: *origin,
            rotation,
            pairs: Vec::new(),
            length,
            tolerance,
            axis_nodes,
            interpolated,
            two_sided,
        });
    }
    let window = tolerance;
    let accept = |d: f64| d <= tolerance;
    let rotation = rotation_matrix(n, 2.0 * PI / *sectors as f64);
    // The high nodes sorted by their first coordinate, for a window search.
    let mut sorted: Vec<(f64, usize)> = highs
        .iter()
        .filter(|h| axis_nodes.binary_search(h).is_err())
        .map(|&h| (x[h][0], h))
        .collect();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut partner_of_high: Vec<Option<usize>> = vec![None; x.len()];
    let mut pairs = Vec::with_capacity(lows.len());
    for &l in &lows {
        if axis_nodes.binary_search(&l).is_ok() {
            continue;
        }
        let y = rotate_point(&rotation, *origin, x[l]);
        let start = sorted.partition_point(|e| e.0 < y[0] - window);
        let mut best: Option<(usize, f64)> = None;
        for &(xh, h) in &sorted[start..] {
            if xh > y[0] + window {
                break;
            }
            let d = distance(y, x[h]);
            if best.map_or(true, |(_, b)| d < b) {
                best = Some((h, d));
            }
        }
        let found = match best {
            Some((h, d)) if accept(d) => h,
            _ => {
                let (h, d) = highs
                    .iter()
                    .map(|&h| (h, distance(y, x[h])))
                    .fold((usize::MAX, f64::INFINITY), |b, c| if c.1 < b.1 { c } else { b });
                return Err(cyclic_error(format!(
                    "low node {l} at {:?} of '{low}': its image {y:?} under the rotation by 2 pi/{sectors} has no node of \
                     the high boundary '{high}' within {tolerance:e} (nearest: node {h} at {d:e})",
                    x[l]
                )));
            }
        };
        if let Some(l0) = partner_of_high[found] {
            return Err(cyclic_error(format!(
                "low nodes {l0} and {l} of '{low}' both map to high node {found} of '{high}'"
            )));
        }
        partner_of_high[found] = Some(l);
        pairs.push((l, found));
    }
    if let Some(&h) = highs
        .iter()
        .find(|&&h| partner_of_high[h].is_none() && axis_nodes.binary_search(&h).is_err())
    {
        return Err(cyclic_error(format!(
            "high node {h} at {:?} of '{high}' is the image of no node of the low boundary '{low}' (tolerance {tolerance:e})",
            x[h]
        )));
    }
    Ok(CyclicBoundary {
        constraint,
        sectors: *sectors,
        axis: n,
        origin: *origin,
        rotation,
        pairs,
        length,
        tolerance,
        axis_nodes,
        interpolated: Vec::new(),
        two_sided: Vec::new(),
    })
}

/// One interpolated high node with its weights over low nodes.
type InterpolatedNode = (usize, Vec<(usize, f64)>);

/// The node-to-edge weights of ADR 0575 D3: each slave node on the nearest shell edge of the master set `lows` (the low set,
/// or the high set for the two-sided tie of ADR 0583) mapped by `map`, weights
/// `(A, 1 - t), (B, t)` at the projection parameter `t` (a zero weight dropped); above the tolerance (that of `options`, else
/// [`crate::tie::GAP_FACTOR`] times the mean mapped edge length) refused with [`FemError::TieGap`].
fn shell_edge_weights(
    mesh: &SolidMesh,
    constraint: usize,
    side: &str,
    lows: &[usize],
    slaves: &[usize],
    map: &dyn Fn(Vec3) -> Vec3,
    options: &crate::tie::TieOptions,
) -> Result<Vec<InterpolatedNode>> {
    let in_low = |p: usize| lows.binary_search(&p).is_ok();
    let mut edges = std::collections::BTreeSet::new();
    for block in mesh.blocks() {
        let k = match block.kind() {
            ElementKind::Shell4 => 4,
            ElementKind::Shell3 => 3,
            _ => continue,
        };
        for e in block.connectivity().chunks_exact(k) {
            for i in 0..k {
                let (a, b) = (e[i], e[(i + 1) % k]);
                if a != b && in_low(a) && in_low(b) {
                    edges.insert((a.min(b), a.max(b)));
                }
            }
        }
    }
    if edges.is_empty() {
        return Err(cyclic_error(format!(
            "constraint {constraint}: the {side} node set has no shell edge (ADR 0575: interpolated shell boundaries)"
        )));
    }
    let x = mesh.nodes();
    let segments: Vec<(usize, usize, Vec3, Vec3)> =
        edges.into_iter().map(|(a, b)| (a, b, map(x[a]), map(x[b]))).collect();
    let mean = segments.iter().map(|s| distance(s.2, s.3)).sum::<f64>() / segments.len() as f64;
    let tolerance = match options.tolerance {
        Some(t) => t,
        None => crate::tie::GAP_FACTOR * mean,
    };
    let mut out = Vec::with_capacity(slaves.len());
    for &p in slaves {
        let y = x[p];
        let mut best: Option<(bool, f64, usize, f64)> = None;
        for (k, &(_, _, a, b)) in segments.iter().enumerate() {
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ay = [y[0] - a[0], y[1] - a[1], y[2] - a[2]];
            let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
            let raw = (ay[0] * ab[0] + ay[1] * ab[1] + ay[2] * ab[2]) / l2;
            let inside = (0.0..=1.0).contains(&raw);
            let t = raw.clamp(0.0, 1.0);
            let d = distance(y, [a[0] + t * ab[0], a[1] + t * ab[1], a[2] + t * ab[2]]);
            let better = match best {
                None => true,
                Some((bi, bd, _, _)) => (inside && !bi) || (inside == bi && d < bd),
            };
            if better {
                best = Some((inside, d, k, t));
            }
        }
        let (_, d, k, t) = best.expect("a segment");
        if d > tolerance {
            return Err(FemError::TieGap {
                node: p,
                distance: d,
                tolerance,
            });
        }
        let (a, b, _, _) = segments[k];
        let w: Vec<(usize, f64)> = [(a, 1.0 - t), (b, t)].into_iter().filter(|&(_, w)| w != 0.0).collect();
        out.push((p, w));
    }
    Ok(out)
}

/// Weights and null-space components at most this size count as zero in the selection of ADR 0583 D2 and D3 (rounding of the
/// projections; the weights are at most 1).
const TWO_SIDED_TAU: f64 = 1e-10;

/// The two-sided shell tie of ADR 0583 D2, D3: from the ADR 0575 high ties `weights`, the low nodes they leave undetermined
/// (the support `F` of the null space of the weight matrix), the high nodes released (their weights wholly on `F`) and, for
/// each node of `F`, its weights over low nodes and released high nodes after the kept high ties are substituted and the
/// coupling within `F` solved. With `F` empty the high ties are returned as they came and the list is empty.
#[allow(clippy::too_many_arguments)]
fn two_sided_ties(
    model: &SolidModel,
    constraint: usize,
    lows: &[usize],
    highs: &[usize],
    axis_nodes: &[usize],
    weights: Vec<InterpolatedNode>,
    rotation: &Mat3,
    origin: Vec3,
    options: &crate::tie::TieOptions,
) -> Result<(Vec<InterpolatedNode>, Vec<TwoSidedNode>)> {
    use std::collections::BTreeMap;
    let tau = TWO_SIDED_TAU;
    let nl = lows.len();
    let column = |p: usize| lows.binary_search(&p).expect("a master of a high tie is a low node");
    // D3: the reduced row echelon form of W (rows the high slaves, columns the low nodes), partial pivoting by columns.
    let mut w = vec![vec![0.0f64; nl]; weights.len()];
    for (r, (_, ws)) in weights.iter().enumerate() {
        for &(l, x) in ws {
            w[r][column(l)] += x;
        }
    }
    let mut pivots: Vec<(usize, usize)> = Vec::new();
    let mut is_pivot = vec![false; nl];
    let mut r0 = 0;
    for j in 0..nl {
        if r0 == w.len() {
            break;
        }
        let (imax, vmax) = (r0..w.len())
            .map(|i| (i, w[i][j].abs()))
            .fold((r0, -1.0f64), |b, c| if c.1 > b.1 { c } else { b });
        if vmax <= tau {
            continue;
        }
        w.swap(r0, imax);
        let p = w[r0][j];
        for v in w[r0].iter_mut() {
            *v /= p;
        }
        let pivot_row = w[r0].clone();
        for (i, row) in w.iter_mut().enumerate() {
            if i != r0 {
                let f = row[j];
                if f != 0.0 {
                    for (v, &q) in row.iter_mut().zip(&pivot_row) {
                        *v -= f * q;
                    }
                }
            }
        }
        pivots.push((r0, j));
        is_pivot[j] = true;
        r0 += 1;
    }
    // F: the support of the null vectors e_j - sum_r w[r][j] e_{p(r)} of the free columns j; axis nodes excluded.
    let mut in_f = vec![false; nl];
    for j in (0..nl).filter(|&j| !is_pivot[j]) {
        in_f[j] = true;
        for &(r, p) in &pivots {
            if w[r][j].abs() > tau {
                in_f[p] = true;
            }
        }
    }
    for (j, &p) in lows.iter().enumerate() {
        if axis_nodes.binary_search(&p).is_ok() {
            in_f[j] = false;
        }
    }
    if !in_f.iter().any(|&b| b) {
        return Ok((weights, Vec::new()));
    }
    // D4 (iii): the unknown counts of the high ties (the check of ADR 0575 D1.4, before the low ties are built).
    let dofs = model.dof_map();
    for (high, ws) in &weights {
        let nh = dofs.components(*high);
        if let Some(&(low, _)) = ws.iter().find(|&&(low, _)| dofs.components(low) != nh) {
            let nl = dofs.components(low);
            return Err(cyclic_error(format!(
                "constraint {constraint}: interpolated high node {high} has {nh} unknowns and its low node {low} has {nl} \
                 (ADR 0575: an interpolated node has the unknowns of its low nodes)"
            )));
        }
    }
    // D2.2: the released high nodes (every weight above tau on a node of F).
    let in_f_node = |p: usize| lows.binary_search(&p).is_ok_and(|j| in_f[j]);
    let mut kept: Vec<InterpolatedNode> = Vec::new();
    let mut released: Vec<usize> = Vec::new();
    for (h, ws) in weights {
        if ws.iter().all(|&(l, x)| x.abs() <= tau || in_f_node(l)) {
            released.push(h);
        } else {
            kept.push((h, ws));
        }
    }
    // D2.3: the nodes of F on the high shell edges rotated back by R^T.
    let fs: Vec<usize> = lows.iter().copied().filter(|&p| in_f_node(p)).collect();
    let mut rt = [[0.0f64; 3]; 3];
    for (i, row) in rt.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = rotation[j][i];
        }
    }
    let back = |p: Vec3| rotate_point(&rt, origin, p);
    let low_weights = shell_edge_weights(model.mesh(), constraint, "high", highs, &fs, &back, options)?;
    // D2.4: substitute the kept high ties (P^{-1} U_b = sum w U_l) and the axis nodes (P^{-1} U_p = U_p); collect A, B_C, B_D.
    let nf = fs.len();
    let mut cols: Vec<(bool, usize)> = Vec::new(); // (a released high node, node)
    let mut col_of: BTreeMap<(bool, usize), usize> = BTreeMap::new();
    let mut a = vec![vec![0.0f64; nf]; nf];
    let mut rhs: Vec<BTreeMap<usize, f64>> = vec![BTreeMap::new(); nf];
    let mut add = |row: &mut BTreeMap<usize, f64>, key: (bool, usize), x: f64| {
        let c = *col_of.entry(key).or_insert_with(|| {
            cols.push(key);
            cols.len() - 1
        });
        *row.entry(c).or_insert(0.0) += x;
    };
    for (i, (_, vs)) in low_weights.iter().enumerate() {
        for &(b, v) in vs {
            if axis_nodes.binary_search(&b).is_ok() {
                add(&mut rhs[i], (false, b), v);
            } else if released.contains(&b) {
                add(&mut rhs[i], (true, b), v);
            } else {
                let (_, ws) = kept
                    .iter()
                    .find(|k| k.0 == b)
                    .expect("a high node is kept, released or on the axis");
                for &(l, x) in ws {
                    match fs.binary_search(&l) {
                        Ok(g) => a[i][g] += v * x,
                        Err(_) => add(&mut rhs[i], (false, l), v * x),
                    }
                }
            }
        }
    }
    // (I - A) G = [B_C | B_D], Gauss-Jordan with partial pivoting.
    let nc = col_of.len();
    let mut m: Vec<Vec<f64>> = (0..nf)
        .map(|i| {
            let mut row = vec![0.0f64; nf + nc];
            for (g, v) in row.iter_mut().enumerate().take(nf) {
                *v = if i == g { 1.0 } else { 0.0 } - a[i][g];
            }
            for (&c, &x) in &rhs[i] {
                row[nf + c] = x;
            }
            row
        })
        .collect();
    for k in 0..nf {
        let (imax, vmax) = (k..nf)
            .map(|i| (i, m[i][k].abs()))
            .fold((k, -1.0f64), |b, c| if c.1 > b.1 { c } else { b });
        if vmax <= tau {
            return Err(cyclic_error(format!(
                "constraint {constraint}: the two-sided tie of the low nodes {fs:?} is singular at low node {} (ADR 0583 D2: \
                 undetermined low nodes that follow only each other)",
                fs[k]
            )));
        }
        m.swap(k, imax);
        let p = m[k][k];
        for v in m[k].iter_mut() {
            *v /= p;
        }
        let pivot_row = m[k].clone();
        for (i, row) in m.iter_mut().enumerate() {
            if i != k {
                let f = row[k];
                if f != 0.0 {
                    for (v, &q) in row.iter_mut().zip(&pivot_row) {
                        *v -= f * q;
                    }
                }
            }
        }
    }
    let mut out = Vec::with_capacity(nf);
    for (i, &f) in fs.iter().enumerate() {
        let mut gl: Vec<(usize, f64)> = Vec::new();
        let mut gh: Vec<(usize, f64)> = Vec::new();
        for (c, &(is_high, node)) in cols.iter().enumerate() {
            let g = m[i][nf + c];
            if g != 0.0 {
                if is_high {
                    gh.push((node, g));
                } else {
                    gl.push((node, g));
                }
            }
        }
        gl.sort_by_key(|e| e.0);
        gh.sort_by_key(|e| e.0);
        out.push((f, gl, gh));
    }
    Ok((kept, out))
}

/// One mode of the real sector problem: the cosine part `a` and the sine part `b` (zero for a single harmonic), each over
/// the model's [`DofMap`].
#[derive(Debug, Clone, PartialEq)]
pub struct SectorMode {
    /// The cosine part `a`.
    pub cos: Vec<f64>,
    /// The sine part `b`.
    pub sin: Vec<f64>,
}

/// The modes of one harmonic index of a sector (ADR 0386 D1).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicModes {
    /// The harmonic index `k`.
    pub harmonic: usize,
    /// The number of sectors `N`.
    pub sectors: usize,
    /// 1 for `k = 0` and `2 k = N` (single modes), 2 otherwise (each sector frequency is a pair of full modes).
    pub multiplicity: usize,
    /// Every eigenvalue of the real problem, ascending (`values.len() * multiplicity` of them).
    pub eigenvalues: Vec<f64>,
    /// One eigenvalue per sector frequency (a pair merged by its mean), rad^2/s^2.
    pub values: Vec<f64>,
    /// The frequencies of `values`, Hz.
    pub frequencies: Vec<f64>,
    /// The relative split `|lambda_2i+1 - lambda_2i| / |mean|` of each pair (0 for a single harmonic).
    pub pair_splits: Vec<f64>,
    /// One mode per eigenvalue of the real problem, M-orthonormal over `(a, b)`.
    pub modes: Vec<SectorMode>,
    /// The residuals of the eigenpairs of the real problem.
    pub residuals: Vec<f64>,
    /// Iterations of the subspace iteration.
    pub iterations: usize,
    /// The shift used.
    pub shift: f64,
    /// The Sturm check of the eigensolver.
    pub sturm: SturmCheck,
    /// Number of independent unknowns of the real problem.
    pub unknowns: usize,
    /// Whether the factor at zero shift was not accepted (a free motion of the harmonic).
    pub rigid_free: bool,
    /// The transformation.
    pub transformation: Transformation,
    /// `T^T K T` (`K` doubled block-diagonally for a pair).
    pub reduced_stiffness: CsrMatrix,
    /// `T^T M T`.
    pub reduced_mass: CsrMatrix,
}

/// `blockdiag(a, a)`.
pub(crate) fn doubled_matrix(a: &CsrMatrix) -> CsrMatrix {
    let n = a.n;
    let mut row_ptr = a.row_ptr.clone();
    let mut col = a.col.clone();
    let mut val = a.val.clone();
    let nnz = a.col.len();
    for r in 0..n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            col.push(a.col[p] + n);
            val.push(a.val[p]);
        }
        row_ptr.push(nnz + a.row_ptr[r + 1]);
    }
    CsrMatrix::rect(2 * n, 2 * n, row_ptr, col, val)
}

fn number_of_sectors(model: &SolidModel, ci: usize) -> usize {
    match &model.constraints()[ci] {
        Constraint::Cyclic { sectors, .. } => *sectors,
        _ => unreachable!("the cyclic constraint"),
    }
}

/// The entry check of the cyclic harmonic paths: for a model with the orthotropic or the tabulated material (ADR 0462 D2)
/// the checks of a path that takes them on solid elements and [`crate::mat_ortho::check_cyclic_axes`], else
/// [`SolidModel::check_supported`] as before. Returns whether the model has either material.
fn check_cyclic_model(model: &SolidModel) -> Result<bool> {
    check_beams_option(model)?;
    if crate::mat_ortho::uses_point_materials(model) {
        model.check_supported_solids_with_materials()?;
        crate::mat_ortho::check_cyclic_axes(model)?;
        Ok(true)
    } else if shells_opted_in(model) {
        // ADR 0556 D1: Shell4 and Shell3 are taken (a shell node shared with a solid stays refused); Beam2 stays refused.
        // ADR 0595 D1: with the beams option as well, Beam2 is taken.
        model.check_supported_with_structures()?;
        if !beams_opted_in(model) {
            refuse_cyclic_beams(model)?;
        }
        Ok(false)
    } else {
        model.check_supported()?;
        Ok(false)
    }
}

/// A model with Beam2 and a cyclic constraint is refused naming [`crate::beam::FOLLOW_UP`] (ADR 0556 D1: a beam's
/// orientation vector is a global direction).
pub(crate) fn refuse_cyclic_beams(model: &SolidModel) -> Result<()> {
    let beams = model
        .mesh()
        .blocks()
        .iter()
        .any(|b| b.kind() == crate::mesh::ElementKind::Beam2);
    if let (true, Some(i)) = (
        beams,
        model
            .constraints()
            .iter()
            .position(|c| matches!(c, Constraint::Cyclic { .. })),
    ) {
        return Err(FemError::NotYetSupported {
            item: "constraint",
            variant: "Cyclic",
            location: format!("constraint {i}, a model with Beam2"),
            package: crate::beam::FOLLOW_UP,
        });
    }
    Ok(())
}

/// The sector stiffness: the per-point stiffness for a model with either material (ADR 0462 D2), else [`stiffness_with`].
fn cyclic_stiffness(model: &SolidModel, points: bool, threads: Option<usize>) -> Result<CsrMatrix> {
    if points {
        point_stiffness(model, threads)
    } else {
        stiffness_with(model, threads)
    }
}

/// The modal analysis of harmonic index `harmonic` of a sector with one cyclic constraint (ADR 0386 D1): `options.modes`
/// sector frequencies (a pair counted once), with the per-node prescribed values `extra` (all zero). `k = 0` runs
/// [`mpc::modal_analysis_mpc`] (its rows are in the transformation); `0 < 2 k < N` the doubled real problem over the cosine and
/// sine parts, `2 k = N` the single one ([`mpc::harmonic_transformation`]; a rigid set with its reference unknowns doubled,
/// ADR 0395). Refuses a harmonic index above `N / 2`, a model without exactly one cyclic constraint and a non-zero
/// constraint value.
pub fn modal_analysis_harmonic(
    model: &SolidModel,
    extra: &[Prescribed],
    harmonic: usize,
    options: &ModalOptions,
) -> Result<HarmonicModes> {
    model.validate()?;
    let points = check_cyclic_model(model)?;
    let ci = the_cyclic_constraint(model)?;
    let sectors = number_of_sectors(model, ci);
    if 2 * harmonic > sectors {
        return Err(FemError::InvalidValue {
            key: "harmonic",
            reason: format!(
                "harmonic index {harmonic} is above N/2 for a rotor of {sectors} sectors (the largest is {})",
                sectors / 2
            ),
        });
    }
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    if harmonic == 0 {
        let s = mpc::modal_analysis_mpc(model, extra, options)?;
        let r = s.result;
        let modes = r
            .modes
            .iter()
            .map(|m| SectorMode {
                cos: m.clone(),
                sin: vec![0.0; n],
            })
            .collect();
        return Ok(HarmonicModes {
            harmonic,
            sectors,
            multiplicity: 1,
            values: r.eigenvalues.clone(),
            frequencies: r.frequencies.clone(),
            pair_splits: vec![0.0; r.eigenvalues.len()],
            eigenvalues: r.eigenvalues,
            modes,
            residuals: r.residuals,
            iterations: r.iterations,
            shift: r.shift,
            sturm: r.sturm,
            unknowns: r.unknowns,
            rigid_free: r.rigid_free,
            transformation: s.transformation,
            reduced_stiffness: s.reduced_stiffness,
            reduced_mass: s.reduced_mass,
        });
    }
    let tr = mpc::harmonic_transformation(model, extra, harmonic)?;
    let multiplicity = if 2 * harmonic < sectors { 2 } else { 1 };
    let mut k = cyclic_stiffness(model, points, options.threads)?;
    mpc::add_supports(&mut k, model)?;
    let m = consistent_with(model, options.threads)?;
    let (k2, m2) = if multiplicity == 2 {
        (doubled_matrix(&k), doubled_matrix(&m))
    } else {
        (k, m)
    };
    let zero = vec![0.0; tr.n_unknowns()];
    let (kr, _) = tr.reduce(&k2, &zero)?;
    let (mr, _) = tr.reduce(&m2, &zero)?;
    let nq = kr.n;
    let count = options.modes * multiplicity;
    if options.modes == 0 || count >= nq {
        return Err(FemError::InvalidValue {
            key: "modes",
            reason: format!(
                "{} modes ({count} eigenvalues of the real problem) requested of harmonic {harmonic} with {nq} free \
                 unknowns (at least 1 and fewer than the unknowns)",
                options.modes
            ),
        });
    }
    for (a, d) in mr.diag().iter().enumerate() {
        if d.is_nan() || *d <= 0.0 {
            let i = tr.independent()[a];
            return Err(FemError::InvalidValue {
                key: "density",
                reason: format!("{} has no mass (diagonal {d:e})", unknown_label(&tr, model, i)),
            });
        }
    }
    let opts = SubspaceOptions {
        shift: options.shift.unwrap_or(Shift::Auto),
        tolerance: options.tolerance,
        residual_test: options.residual_test,
        max_iterations: options.max_iterations,
        subspace_size: options.subspace_size,
        factorisation: options.factorisation,
        ..SubspaceOptions::default()
    };
    let eig = lowest_eigenpairs(&kr, &mr, count, &opts).map_err(|e| FemError::Solver {
        solver: "lowest_eigenpairs",
        reason: e.to_string(),
    })?;
    let rigid_free = match options.shift {
        None => eig.shift != 0.0,
        Some(_) => mpc::zero_shift_refused(&kr),
    };
    let modes: Vec<SectorMode> = eig
        .vectors
        .iter()
        .map(|q| {
            let x = tr.expand(q);
            SectorMode {
                cos: x[..n].to_vec(),
                sin: if multiplicity == 2 {
                    x[n..2 * n].to_vec()
                } else {
                    vec![0.0; n]
                },
            }
        })
        .collect();
    let (values, pair_splits): (Vec<f64>, Vec<f64>) = if multiplicity == 2 {
        eig.values
            .chunks(2)
            .map(|p| {
                let mean = 0.5 * (p[0] + p[1]);
                let split = if mean == 0.0 {
                    (p[1] - p[0]).abs()
                } else {
                    (p[1] - p[0]).abs() / mean.abs()
                };
                (mean, split)
            })
            .unzip()
    } else {
        (eig.values.clone(), vec![0.0; eig.values.len()])
    };
    Ok(HarmonicModes {
        harmonic,
        sectors,
        multiplicity,
        frequencies: values.iter().map(|&l| frequency(l)).collect(),
        values,
        pair_splits,
        eigenvalues: eig.values,
        modes,
        residuals: eig.residuals,
        iterations: eig.iterations,
        shift: eig.shift,
        sturm: eig.sturm,
        unknowns: nq,
        rigid_free,
        transformation: tr,
        reduced_stiffness: kr,
        reduced_mass: mr,
    })
}

/// The full rotor built from a sector mesh (ADR 0386 D1): `N` rotated copies, each high node of sector `j` the low node of
/// sector `j + 1`.
#[derive(Debug, Clone, PartialEq)]
pub struct FullRotor {
    mesh: SolidMesh,
    node_of: Vec<Vec<usize>>,
    is_high: Vec<bool>,
    sectors: usize,
    axis: Vec3,
    block_of: Vec<Vec<usize>>,
}

impl FullRotor {
    /// The mesh of the full rotor.
    pub fn mesh(&self) -> &SolidMesh {
        &self.mesh
    }
    /// The number of sectors.
    pub fn sectors(&self) -> usize {
        self.sectors
    }
    /// The node of the full mesh that is node `node` of sector `sector`.
    pub fn node(&self, sector: usize, node: usize) -> usize {
        self.node_of[sector][node]
    }
    /// The block of the full mesh that holds copy `sector` of block `block` of the sector mesh: `block` itself, except
    /// where a Beam2 block was split into one block per copy (ADR 0601 D2).
    pub fn block(&self, sector: usize, block: usize) -> usize {
        self.block_of[sector][block]
    }

    /// The full mode of a sector mode of harmonic index `harmonic` (ADR 0386): at node `p` of sector `j`, `s R^j (cos(j
    /// theta) a_p - sin(j theta) b_p)`, `theta = 2 pi k / N`, `s = sqrt(2 / N)` for a pair and `sqrt(1 / N)` for a single
    /// harmonic, so that an M-orthonormal sector mode gives an M-orthonormal full mode. `sector_dofs` and `full_dofs` are the
    /// numberings of the sector model and of the full model (three components per node; a six-unknown node also its
    /// rotations by `R^j`, ADR 0561).
    pub fn expand(&self, sector_dofs: &DofMap, full_dofs: &DofMap, harmonic: usize, mode: &SectorMode) -> Vec<f64> {
        let nsec = self.sectors;
        let single = harmonic % nsec == 0 || 2 * (harmonic % nsec) == nsec;
        let scale = if single {
            (1.0 / nsec as f64).sqrt()
        } else {
            (2.0 / nsec as f64).sqrt()
        };
        let mut out = vec![0.0; full_dofs.n_equations()];
        for j in 0..nsec {
            let r = rotation_matrix(self.axis, 2.0 * PI * j as f64 / nsec as f64);
            let jk = (j * harmonic) % nsec;
            let (c, s) = if jk == 0 {
                (1.0, 0.0)
            } else if 2 * jk == nsec {
                (-1.0, 0.0)
            } else {
                let t = 2.0 * PI * jk as f64 / nsec as f64;
                (t.cos(), t.sin())
            };
            for p in 0..self.is_high.len() {
                if self.is_high[p] {
                    continue;
                }
                // ADR 0561 D1: a six-unknown node also takes its rotations by the same R^j.
                for base in triplets(sector_dofs, p) {
                    let mut v = [0.0; 3];
                    for (i, vi) in v.iter_mut().enumerate() {
                        let e = sector_dofs.equation(p, base + i).expect("sector component");
                        *vi = c * mode.cos[e] - s * mode.sin[e];
                    }
                    let w = apply(&r, v);
                    let full = self.node_of[j][p];
                    for (i, wi) in w.iter().enumerate() {
                        out[full_dofs.equation(full, base + i).expect("full component")] = scale * wi;
                    }
                }
            }
        }
        out
    }

    /// `(c, s) = (cos(j theta), sin(j theta))` of sector `j`, exact at `j k = 0` and `2 j k = N` (mod N).
    fn phase(&self, harmonic: usize, j: usize) -> (f64, f64) {
        let nsec = self.sectors;
        let jk = (j * harmonic) % nsec;
        if jk == 0 {
            (1.0, 0.0)
        } else if 2 * jk == nsec {
            (-1.0, 0.0)
        } else {
            let t = 2.0 * PI * jk as f64 / nsec as f64;
            (t.cos(), t.sin())
        }
    }

    /// The full field of a harmonic static solution (ADR 0395): at node `p` of sector `j`, `R^j (cos(j theta) a_p - sin(j
    /// theta) b_p)`, scale 1, each full node written once from the sector in which it is not a high node.
    pub fn expand_field(
        &self,
        sector_dofs: &DofMap,
        full_dofs: &DofMap,
        harmonic: usize,
        cos: &[f64],
        sin: &[f64],
    ) -> Vec<f64> {
        let mut out = vec![0.0; full_dofs.n_equations()];
        for j in 0..self.sectors {
            let r = rotation_matrix(self.axis, 2.0 * PI * j as f64 / self.sectors as f64);
            let (c, s) = self.phase(harmonic, j);
            for p in 0..self.is_high.len() {
                if self.is_high[p] {
                    continue;
                }
                for base in triplets(sector_dofs, p) {
                    let mut v = [0.0; 3];
                    for (i, vi) in v.iter_mut().enumerate() {
                        let e = sector_dofs.equation(p, base + i).expect("sector component");
                        *vi = c * cos[e] - s * sin[e];
                    }
                    let w = apply(&r, v);
                    let full = self.node_of[j][p];
                    for (i, wi) in w.iter().enumerate() {
                        out[full_dofs.equation(full, base + i).expect("full component")] = *wi;
                    }
                }
            }
        }
        out
    }

    /// The expanded load of a harmonic load (ADR 0395): the assembly over the sectors of `R^j (cos(j theta) f_a - sin(j
    /// theta) f_b)` at every node of sector `j` (high nodes included: a shared node sums the contributions of its two
    /// sectors).
    pub fn expand_load(
        &self,
        sector_dofs: &DofMap,
        full_dofs: &DofMap,
        harmonic: usize,
        cos: &[f64],
        sin: &[f64],
    ) -> Vec<f64> {
        let mut out = vec![0.0; full_dofs.n_equations()];
        for j in 0..self.sectors {
            let r = rotation_matrix(self.axis, 2.0 * PI * j as f64 / self.sectors as f64);
            let (c, s) = self.phase(harmonic, j);
            for p in 0..self.is_high.len() {
                for base in triplets(sector_dofs, p) {
                    let mut v = [0.0; 3];
                    for (i, vi) in v.iter_mut().enumerate() {
                        let e = sector_dofs.equation(p, base + i).expect("sector component");
                        *vi = c * cos[e] - s * sin[e];
                    }
                    let w = apply(&r, v);
                    let full = self.node_of[j][p];
                    for (i, wi) in w.iter().enumerate() {
                        out[full_dofs.equation(full, base + i).expect("full component")] += *wi;
                    }
                }
            }
        }
        out
    }
}

/// The first components of the triplets of sector node `p`: the translations, and the rotations of a six-unknown node (ADR
/// 0561 D1).
fn triplets(dofs: &DofMap, p: usize) -> &'static [usize] {
    if dofs.components(p) == 6 {
        &[0, 3]
    } else {
        &[0]
    }
}

/// The full rotor of a sector model with one cyclic constraint (ADR 0386 D1): the sector mesh copied `N` times (copy `j`
/// rotated by `j 2 pi / N`), the low nodes of copy `j >= 1` identified with the matched high nodes of copy `j - 1` and the
/// high nodes of copy `N - 1` with the low nodes of copy 0. Blocks keep their index, kind and property (copy `j` of element
/// `e` of block `b` is element `start_b + j n_b + (e - start_b^sector)`); node, element and surface sets are the unions of
/// their copies. Refuses a block of a structural kind; with [`crate::model::CyclicOptions::shells`] Shell4 and Shell3 blocks
/// are copied as the solid blocks are (ADR 0561 D1; Beam2 stays refused, also with [`crate::model::CyclicOptions::beams`]:
/// a block has one orientation vector, ADR 0595 D4). With [`crate::model::CyclicOptions::full_rotor_beams`] as well, a Beam2
/// block becomes `N` consecutive blocks, copy `j` with the orientation `R^j v` (ADR 0601 D2: the element numbering is
/// unchanged, the block indices after a split block shift; [`FullRotor::block`]).
pub fn full_rotor(model: &SolidModel) -> Result<FullRotor> {
    let ci = the_cyclic_constraint(model)?;
    let bd = boundary(model, ci)?;
    if bd.is_extended() {
        return Err(cyclic_error(format!(
            "constraint {ci}: the full rotor of a boundary with nodes on the axis or interpolated nodes is not built (ADR              0456 D4)"
        )));
    }
    let mesh = model.mesh();
    let shells = shells_opted_in(model);
    let split_beams = full_rotor_beams_opted_in(model);
    for (b, block) in mesh.blocks().iter().enumerate() {
        let shell = matches!(block.kind(), ElementKind::Shell4 | ElementKind::Shell3);
        let beam = split_beams && block.kind() == ElementKind::Beam2;
        if !block.kind().is_solid() && !(shells && shell) && !beam {
            return Err(FemError::UnsupportedKind {
                block: b,
                kind: block.kind().name(),
                operation: "full_rotor",
            });
        }
    }
    let nsec = bd.sectors();
    let x = mesh.nodes();
    let n = x.len();
    let mut high_of_low: Vec<Option<usize>> = vec![None; n];
    let mut low_of_high: Vec<Option<usize>> = vec![None; n];
    let mut is_high = vec![false; n];
    for &(l, h) in bd.pairs() {
        high_of_low[l] = Some(h);
        low_of_high[h] = Some(l);
        is_high[h] = true;
    }
    let mut nodes: Vec<Vec3> = Vec::with_capacity(n * nsec);
    let mut node_of = vec![vec![usize::MAX; n]; nsec];
    for j in 0..nsec {
        let r = rotation_matrix(bd.axis(), 2.0 * PI * j as f64 / nsec as f64);
        for p in 0..n {
            node_of[j][p] = match (high_of_low[p], low_of_high[p]) {
                (Some(h), _) if j >= 1 => node_of[j - 1][h],
                (_, Some(l)) if j == nsec - 1 => node_of[0][l],
                _ => {
                    nodes.push(rotate_point(&r, bd.origin(), x[p]));
                    nodes.len() - 1
                }
            };
        }
    }
    let mut full = SolidMesh::new(nodes)?;
    // Element numbering: block by block, sector by sector.
    let mut sector_start = Vec::new();
    let mut full_start = Vec::new();
    let (mut s0, mut f0) = (0usize, 0usize);
    let mut block_of = vec![vec![usize::MAX; mesh.blocks().len()]; nsec];
    for (b, block) in mesh.blocks().iter().enumerate() {
        let npe = block.kind().nodes_per_element();
        if split_beams && block.kind() == ElementKind::Beam2 {
            // ADR 0601 D2: one block per copy, copy j with the orientation R^j v (copy 0 the sector's property, cloned).
            for (j, map) in node_of.iter().enumerate() {
                let conn: Vec<usize> = block.connectivity().iter().map(|&p| map[p]).collect();
                let fb = full.add_block(block.kind(), conn)?;
                block_of[j][b] = fb;
                match block.property() {
                    Some(BlockProperty::Beam { section, orientation }) if j >= 1 => {
                        let r = rotation_matrix(bd.axis(), 2.0 * PI * j as f64 / nsec as f64);
                        full.set_block_property(fb, BlockProperty::beam(*section, apply(&r, *orientation))?)?;
                    }
                    Some(prop) => full.set_block_property(fb, prop.clone())?,
                    None => {}
                }
            }
        } else {
            let mut conn = Vec::with_capacity(block.connectivity().len() * nsec);
            for map in &node_of {
                conn.extend(block.connectivity().iter().map(|&p| map[p]));
            }
            let fb = full.add_block(block.kind(), conn)?;
            debug_assert!(split_beams || fb == b);
            for row in block_of.iter_mut() {
                row[b] = fb;
            }
            if let Some(prop) = block.property() {
                full.set_block_property(fb, prop.clone())?;
            }
        }
        sector_start.push(s0);
        full_start.push(f0);
        let ne = block.connectivity().len() / npe;
        s0 += ne;
        f0 += ne * nsec;
    }
    let element_of = |j: usize, e: usize| -> usize {
        let b = mesh.block_of(e);
        let ne = mesh.blocks()[b].len();
        full_start[b] + j * ne + (e - sector_start[b])
    };
    for name in mesh.node_set_names() {
        let set = mesh.node_set(name).expect("listed");
        let mut v: Vec<usize> = node_of.iter().flat_map(|map| set.iter().map(|&p| map[p])).collect();
        v.sort_unstable();
        v.dedup();
        full.add_node_set(name, v)?;
    }
    for name in mesh.element_set_names() {
        let set = mesh.element_set(name).expect("listed");
        let mut v: Vec<usize> = (0..nsec)
            .flat_map(|j| set.iter().map(move |&e| (j, e)))
            .map(|(j, e)| element_of(j, e))
            .collect();
        v.sort_unstable();
        full.add_element_set(name, v)?;
    }
    for name in mesh.surface_set_names() {
        let set = mesh.surface_set(name).ok_or_else(|| FemError::UnknownSet {
            kind: SetKind::Surface,
            name: name.to_string(),
        })?;
        let mut v: Vec<FaceRef> = Vec::with_capacity(set.len() * nsec);
        for j in 0..nsec {
            for f in set {
                v.push(FaceRef {
                    element: element_of(j, f.element),
                    face: f.face,
                });
            }
        }
        full.add_surface_set(name, v)?;
    }
    Ok(FullRotor {
        mesh: full,
        node_of,
        is_high,
        sectors: nsec,
        axis: bd.axis(),
        block_of,
    })
}

/// A static load of harmonic index `harmonic` on a sector (ADR 0395): the cosine part `cos` and the sine part `sin`, each a
/// load vector over the model's [`DofMap`]; the load of sector `j` in its own components is `cos(j theta) f_a - sin(j theta)
/// f_b`. Reference loads on rigid sets likewise, one list per part.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicLoad {
    /// The harmonic index `k`.
    pub harmonic: usize,
    /// The cosine part `f_a`.
    pub cos: Vec<f64>,
    /// The sine part `f_b` (zero for `k = 0` and `2 k = N`).
    pub sin: Vec<f64>,
    /// Forces and moments at reference points, cosine part.
    pub reference_cos: Vec<ReferenceLoad>,
    /// Forces and moments at reference points, sine part.
    pub reference_sin: Vec<ReferenceLoad>,
}

impl HarmonicLoad {
    /// A harmonic load with its cosine and sine parts and no reference loads.
    pub fn new(harmonic: usize, cos: Vec<f64>, sin: Vec<f64>) -> HarmonicLoad {
        HarmonicLoad {
            harmonic,
            cos,
            sin,
            reference_cos: Vec::new(),
            reference_sin: Vec::new(),
        }
    }
    /// The same load with reference loads (cosine and sine parts).
    pub fn references(mut self, cos: Vec<ReferenceLoad>, sin: Vec<ReferenceLoad>) -> HarmonicLoad {
        self.reference_cos = cos;
        self.reference_sin = sin;
        self
    }
}

/// The solution of a harmonic static load on a sector (ADR 0395).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicStatic {
    /// The harmonic index `k`.
    pub harmonic: usize,
    /// The number of sectors `N`.
    pub sectors: usize,
    /// The cosine part `a` over the model's [`DofMap`].
    pub cos: Vec<f64>,
    /// The sine part `b` (zero for `k = 0` and `2 k = N`).
    pub sin: Vec<f64>,
    /// The reference motions of the rigid sets, cosine part, in the order of the constraints.
    pub reference_cos: Vec<ReferenceMotion>,
    /// The reference motions of the rigid sets, sine part (zero for a single harmonic).
    pub reference_sin: Vec<ReferenceMotion>,
    /// `|f_r - K_r q| / |f_r|`.
    pub relative_residual: f64,
    /// Number of independent unknowns.
    pub unknowns: usize,
    /// The transformation.
    pub transformation: Transformation,
    /// `T^T K T` (`K` doubled block-diagonally for a pair).
    pub reduced_matrix: CsrMatrix,
}

fn harmonic_load_error(reason: String) -> FemError {
    FemError::InvalidValue {
        key: "harmonic load",
        reason,
    }
}

/// The name of unknown `i` of a (harmonic) transformation: "node 4 component 1", "node 4 component 1 (sine part)" or "the
/// reference rotation y of constraint 3 (RigidSet, sine part)".
pub(crate) fn unknown_label(tr: &Transformation, model: &SolidModel, i: usize) -> String {
    let n = model.dof_map().n_equations();
    if i < tr.n_mesh() {
        let (node, component) = model.dof_map().node_component(i % n).expect("mesh unknown");
        let part = if i >= n { " (sine part)" } else { "" };
        return format!("node {node} component {component}{part}");
    }
    let Some(&(constraint, r0)) = tr.references().iter().rev().find(|&&(_, o)| o <= i) else {
        return format!("unknown {i}");
    };
    let first = tr
        .references()
        .iter()
        .find(|&&(c, _)| c == constraint)
        .map_or(r0, |&(_, o)| o);
    let part = if r0 > first { ", sine part" } else { "" };
    let what = [
        "translation x",
        "translation y",
        "translation z",
        "rotation x",
        "rotation y",
        "rotation z",
    ][(i - r0).min(REFERENCE_UNKNOWNS - 1)];
    format!("the reference {what} of constraint {constraint} (RigidSet{part})")
}

/// The static solve of a harmonic load on a sector with one cyclic constraint (ADR 0395): `k = 0` on
/// [`mpc::transformation`] (any prescribed values), `0 < 2 k < N` on the doubled problem over the cosine and sine parts and
/// `2 k = N` on the single one ([`mpc::harmonic_transformation`], zero values only), rigid sets with their reference
/// unknowns per part; the direct solver only. The full solution is [`FullRotor::expand_field`] of the result under the load
/// [`FullRotor::expand_load`]. ADR 0462 D2: the temperature field of a model with a `Tabulated` material gives the table's
/// stiffness and no thermal load (add [`crate::loads::nodal_load_vector`] of the model to the cosine part at `k = 0`).
/// Refuses a harmonic above `N / 2`, a model with its own loads (give the parts as vectors), a
/// part of the wrong length, a non-finite entry, a non-zero sine part at `k = 0` or `2 k = N`, a reference load on a
/// constraint that is not a rigid set and a solver other than `Direct`.
pub fn solve_static_harmonic(
    model: &SolidModel,
    extra: &[Prescribed],
    load: &HarmonicLoad,
    options: &StaticOptions,
) -> Result<HarmonicStatic> {
    model.validate()?;
    let points = check_cyclic_model(model)?;
    let ci = the_cyclic_constraint(model)?;
    let sectors = number_of_sectors(model, ci);
    let harmonic = load.harmonic;
    if 2 * harmonic > sectors {
        return Err(FemError::InvalidValue {
            key: "harmonic",
            reason: format!(
                "harmonic index {harmonic} is above N/2 for a rotor of {sectors} sectors (the largest is {})",
                sectors / 2
            ),
        });
    }
    if !matches!(options.solver, StaticSolver::Direct) {
        return Err(FemError::InvalidValue {
            key: "solver",
            reason: "a harmonic static solve takes the Direct solver only (ADR 0395)".into(),
        });
    }
    // ADR 0462 D2: the temperature field of a model with a Tabulated material is the table's state, not a load.
    let table = points
        && model
            .materials()
            .iter()
            .any(|(_, m)| matches!(m, crate::model::Material::Tabulated { .. }));
    let own = model
        .loads()
        .iter()
        .filter(|l| !(table && matches!(l, crate::model::Load::Temperature { .. })))
        .count();
    if own > 0 {
        return Err(FemError::InvalidValue {
            key: "loads",
            reason: format!(
                "the model has {own} loads of its own; a harmonic static solve takes its load as the cosine and sine \
                 vectors (loads::nodal_load_vector of a model with the loads builds them)"
            ),
        });
    }
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    for (key, v) in [("cosine part", &load.cos), ("sine part", &load.sin)] {
        if v.len() != n {
            return Err(FemError::FieldLength {
                key,
                expected: n,
                found: v.len(),
            });
        }
        if let Some(i) = v.iter().position(|x| !x.is_finite()) {
            return Err(harmonic_load_error(format!("{key} entry {i} is not finite")));
        }
    }
    let doubled = harmonic != 0 && 2 * harmonic < sectors;
    if !doubled {
        if let Some(i) = load.sin.iter().position(|&x| x != 0.0) {
            return Err(harmonic_load_error(format!(
                "sine part entry {i} is {:e}: harmonic index {harmonic} of {sectors} sectors has no sine part \
                 (sin(j theta) = 0 in every sector)",
                load.sin[i]
            )));
        }
        if let Some(l) = load
            .reference_sin
            .iter()
            .find(|l| l.force.iter().chain(&l.moment).any(|&x| x != 0.0))
        {
            return Err(harmonic_load_error(format!(
                "sine reference load on constraint {}: harmonic index {harmonic} of {sectors} sectors has no sine part",
                l.constraint
            )));
        }
    }
    let tr = if harmonic == 0 {
        mpc::transformation(model, extra)?
    } else {
        mpc::harmonic_transformation(model, extra, harmonic)?
    };
    let nn = tr.n_unknowns();
    let mut f = load.cos.clone();
    if doubled {
        f.extend_from_slice(&load.sin);
    }
    f.resize(nn, 0.0);
    let parts: &[&[ReferenceLoad]] = if doubled {
        &[&load.reference_cos, &load.reference_sin]
    } else {
        &[&load.reference_cos]
    };
    for (part, list) in parts.iter().enumerate() {
        for l in list.iter() {
            let r0 = tr
                .references()
                .iter()
                .filter(|&&(c, _)| c == l.constraint)
                .nth(part)
                .map(|&(_, o)| o)
                .ok_or_else(|| FemError::InvalidValue {
                    key: "reference load",
                    reason: format!("constraint {} is not a rigid set", l.constraint),
                })?;
            if l.force.iter().chain(&l.moment).any(|v| !v.is_finite()) {
                return Err(FemError::InvalidValue {
                    key: "reference load",
                    reason: format!("force {:?} or moment {:?} is not finite", l.force, l.moment),
                });
            }
            for i in 0..3 {
                f[r0 + i] += l.force[i];
                f[r0 + 3 + i] += l.moment[i];
            }
        }
    }
    let mut k = cyclic_stiffness(model, points, options.threads)?;
    mpc::add_supports(&mut k, model)?;
    let k = if doubled { doubled_matrix(&k) } else { k };
    let (kr, fr) = tr.reduce(&k, &f)?;
    let q = if kr.n == 0 {
        Vec::new()
    } else {
        let diag = kr.diag();
        let singular = |row: usize, pivot: f64| FemError::Solver {
            solver: "skyline-ldlt",
            reason: format!(
                "singular reduced stiffness at {}: pivot {pivot:e}, diagonal {:e}",
                unknown_label(&tr, model, tr.independent()[row]),
                diag[row]
            ),
        };
        let ldlt = SkylineLdlt::factor(&kr).map_err(|e| match e {
            SkylineError::Pivot { row, pivot, .. } => singular(row, pivot),
            other => FemError::Solver {
                solver: "skyline-ldlt",
                reason: other.to_string(),
            },
        })?;
        for (&row, &d) in ldlt.order().iter().zip(ldlt.pivots()) {
            if d <= PIVOT_TOLERANCE * diag[row] {
                return Err(singular(row, d));
            }
        }
        ldlt.solve(&fr)
    };
    let relative_residual = {
        let bn = fr.iter().map(|v| v * v).sum::<f64>().sqrt();
        if bn == 0.0 || q.is_empty() {
            0.0
        } else {
            let mut r = vec![0.0; fr.len()];
            kr.residual(&fr, &q, &mut r);
            r.iter().map(|v| v * v).sum::<f64>().sqrt() / bn
        }
    };
    let x = tr.expand(&q);
    let motion = |constraint: usize, r0: usize| ReferenceMotion {
        constraint,
        translation: [x[r0], x[r0 + 1], x[r0 + 2]],
        rotation: [x[r0 + 3], x[r0 + 4], x[r0 + 5]],
    };
    let mut reference_cos = Vec::new();
    let mut reference_sin = Vec::new();
    for &(c, r0) in tr.references() {
        if reference_cos.iter().any(|m: &ReferenceMotion| m.constraint == c) {
            reference_sin.push(motion(c, r0));
        } else {
            reference_cos.push(motion(c, r0));
            if !doubled {
                reference_sin.push(ReferenceMotion {
                    constraint: c,
                    translation: [0.0; 3],
                    rotation: [0.0; 3],
                });
            }
        }
    }
    Ok(HarmonicStatic {
        harmonic,
        sectors,
        cos: x[..n].to_vec(),
        sin: if doubled { x[n..2 * n].to_vec() } else { vec![0.0; n] },
        reference_cos,
        reference_sin,
        relative_residual,
        unknowns: kr.n,
        transformation: tr,
        reduced_matrix: kr,
    })
}

/// A load vector over `dofs` as nodal forces (one per node with a non-zero force, ascending), for
/// [`mpc::solve_static_mpc`] of the full model under an expanded load (ADR 0395).
pub fn nodal_forces_of(dofs: &DofMap, f: &[f64]) -> Vec<NodalForceAt> {
    let mut out = Vec::new();
    for node in 0..dofs.n_nodes() {
        let mut v = [0.0; 3];
        for (i, vi) in v.iter_mut().enumerate() {
            if let Some(e) = dofs.equation(node, i) {
                *vi = f[e];
            }
        }
        if v.iter().any(|&x| x != 0.0) {
            out.push(NodalForceAt::new(node, v));
        }
    }
    out
}
