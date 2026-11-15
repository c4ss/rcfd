//! A warping unknown for Beam2: the 14-unknown element and its linear buckling path (ADR 0546, F-598). Opt-in.
//!
//! The section of a thin-walled beam twisting by `phi(x)` warps out of its plane by `-omega phi'` (Vlasov) and stores
//! `(1/2) E C_w phi''^2` per length, `C_w` the warping constant ([`WarpingSection`]). The element of a warping block has seven
//! unknowns per node, `(u, v, w, phi, theta_y, theta_z, chi)` in local axes, `chi = phi'` the rate of twist (ADR 0546 D2): the
//! twist is the cubic Hermite of `(phi_0, chi_0, phi_1, chi_1)` with the energy `(1/2) int (G J phi'^2 + E C_w phi''^2) dx`, the
//! axial and bending entries are those of [`crate::beam::local_stiffness`] bit for bit, and the geometric stiffness is that of
//! ADR 0473 D1 and ADR 0508 D1 with the Hermite twist and the block's own Wagner coefficients.
//!
//! The six-unknown numbering of the model ([`crate::model::DofMap`]) is not changed: the warping unknowns are appended after it,
//! one per node of a warping element in ascending node order ([`WarpingDofs`], D3). Nothing else in the library reads a
//! [`Warping`]: the model's own static solve, modes and other paths are those of the six-unknown model. [`buckling_factors`] is
//! the buckling path of [`crate::buckling::factors`] on the augmented matrices ([`matrices`], D5), with the fork supports of the
//! model's constraints and the warping free unless [`Warping::restrain`] holds it. The refusals of ADR 0546 D4 (RF1 to RF9) are
//! listed on the functions.
//!
//! ADR 0548 (F-626) adds the static solve with the warping unknown ([`solve_static`]), the pre-stress of that solve with the
//! total torque of the warping elements ([`WarpingPrestress::from_static`]; RF8 lifted for it, [`buckling_factors_with`]), the
//! 14-unknown consistent mass ([`local_mass`], the Hermite twist, no warping inertia), the modes ([`modal_analysis`]) and the
//! pre-stressed modes ([`prestressed_modes`]).

use crate::assemble::{element_dofs, point_stiffness};
use crate::beam::{self, GAUSS4_W, GAUSS4_X};
use crate::buckling::BucklingOptions;
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reduce};
use crate::element::ElementMatrix;
use crate::error::{FemError, Result, SetKind};
use crate::geometric::{assemble_geometric, Prestress};
use crate::mesh::{BeamSection, BlockProperty, ElementKind};
use crate::model::{Material, SolidModel};
use rcfd_core::vec3::Vec3;
use rcfd_linalg::eigen::{lowest_buckling_factors, BucklingEigenOptions, SturmCheck};
use rcfd_linalg::CsrMatrix;

/// Two warping elements at a node must have the same frame within this distance of their unit axes (RF3, ADR 0546 D4).
pub const FRAME_TOLERANCE: f64 = 1e-9;

/// A torque in the pre-stress of a warping element above this fraction of the element's scale is refused (RF8).
pub const TORQUE_TOLERANCE: f64 = 1e-9;

/// The local indices of the twist unknowns `(phi_0, chi_0, phi_1, chi_1)` of the 14-unknown element.
pub const TWIST: [usize; 4] = [3, 6, 10, 13];

/// The index in the 14-unknown order of the unknown `i` of the 12-unknown order of [`crate::beam`].
pub fn index_of(i: usize) -> usize {
    if i < 6 {
        i
    } else {
        i + 1
    }
}

fn warping_error(reason: String) -> FemError {
    FemError::InvalidValue { key: "warping", reason }
}

/// The warping constant and the Wagner coefficients of the section of a warping block (ADR 0546 D1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WarpingSection {
    c_w: f64,
    wagner: [f64; 2],
}

impl WarpingSection {
    /// `C_w` (m^6, non-negative) and the Wagner coefficients `beta_y = int z (y^2 + z^2) dA/I_y`, `beta_z = int y (y^2 + z^2)
    /// dA/I_z` (m) in the centroidal convention of ADR 0508 (exact only when the shear centre is the centroid). Refuses a
    /// negative or non-finite `c_w` (key `"c_w"`) and a non-finite coefficient (key `"wagner"`; RF7).
    pub fn new(c_w: f64, beta_y: f64, beta_z: f64) -> Result<WarpingSection> {
        if !(c_w.is_finite() && c_w >= 0.0) {
            return Err(FemError::InvalidValue {
                key: "c_w",
                reason: format!("{c_w} must be non-negative and finite"),
            });
        }
        if !(beta_y.is_finite() && beta_z.is_finite()) {
            return Err(FemError::InvalidValue {
                key: "wagner",
                reason: format!("({beta_y}, {beta_z}) must be finite"),
            });
        }
        Ok(WarpingSection {
            c_w,
            wagner: [beta_y, beta_z],
        })
    }

    /// No warping stiffness and no Wagner coefficients (Saint-Venant torsion; the negative control N1 of ADR 0546).
    pub fn saint_venant() -> WarpingSection {
        WarpingSection {
            c_w: 0.0,
            wagner: [0.0; 2],
        }
    }

    /// The thin-walled doubly symmetric I of two flanges `b_f x t_f` and a web `h_w x t_w` (ADR 0546 D1): the section (`A = 2
    /// b_f t_f + h_w t_w`, `I_y = 2 (b_f t_f^3/12 + b_f t_f (h_c/2)^2) + t_w h_w^3/12` about `e_2`, `I_z = 2 t_f b_f^3/12 + h_w
    /// t_w^3/12`, `J = (2 b_f t_f^3 + h_w t_w^3)/3`, the shear factor `shear` both ways) and `C_w = t_f b_f^3 h_c^2/24`, `h_c = h_w +
    /// t_f`, with the Wagner coefficients zero (the integrands are odd over a section with two axes of symmetry). The flanges
    /// lie along the section axis y. Refuses a dimension that is not positive and finite, naming it.
    pub fn doubly_symmetric_i(
        b_f: f64,
        t_f: f64,
        h_w: f64,
        t_w: f64,
        shear: f64,
    ) -> Result<(BeamSection, WarpingSection)> {
        for (key, v) in [("b_f", b_f), ("t_f", t_f), ("h_w", h_w), ("t_w", t_w)] {
            if !(v.is_finite() && v > 0.0) {
                return Err(FemError::InvalidValue {
                    key,
                    reason: format!("{v} must be positive and finite"),
                });
            }
        }
        let a = 2.0 * b_f * t_f + h_w * t_w;
        let i_z = 2.0 * t_f * b_f.powi(3) / 12.0 + h_w * t_w.powi(3) / 12.0;
        let h_c = h_w + t_f;
        let i_y = 2.0 * (b_f * t_f.powi(3) / 12.0 + b_f * t_f * (h_c / 2.0).powi(2)) + t_w * h_w.powi(3) / 12.0;
        let j = (2.0 * b_f * t_f.powi(3) + h_w * t_w.powi(3)) / 3.0;
        let section = BeamSection::new(a, i_y, i_z, j, shear, shear)?;
        let c_w = t_f * b_f.powi(3) * h_c * h_c / 24.0;
        Ok((section, WarpingSection { c_w, wagner: [0.0; 2] }))
    }

    /// The warping constant `C_w`, m^6.
    pub fn c_w(&self) -> f64 {
        self.c_w
    }

    /// The Wagner coefficients `(beta_y, beta_z)`, m.
    pub fn wagner(&self) -> [f64; 2] {
        self.wagner
    }
}

/// The opt-in of ADR 0546: the Beam2 blocks that take the warping unknown, each with its [`WarpingSection`], and the node sets
/// at which the warping is held (`chi = 0`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Warping {
    blocks: Vec<(usize, WarpingSection)>,
    restrained: Vec<String>,
}

impl Warping {
    /// No block.
    pub fn new() -> Warping {
        Warping::default()
    }
    /// The same with block `block` taking the warping unknown with `section`.
    pub fn block(mut self, block: usize, section: WarpingSection) -> Warping {
        self.blocks.push((block, section));
        self
    }
    /// The same with the warping held at the nodes of `node_set`.
    pub fn restrain(mut self, node_set: &str) -> Warping {
        self.restrained.push(node_set.to_string());
        self
    }
    /// The blocks and their sections, in the order given.
    pub fn blocks(&self) -> &[(usize, WarpingSection)] {
        &self.blocks
    }
    /// The restrained node sets.
    pub fn restrained(&self) -> &[String] {
        &self.restrained
    }
}

/// The numbering of the warping unknowns (ADR 0546 D3): after the `base` equations of the model's `DofMap`, one per node of a
/// warping element in ascending node order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarpingDofs {
    base: usize,
    of_node: Vec<Option<usize>>,
    nodes: Vec<usize>,
}

impl WarpingDofs {
    /// The equations of the six-unknown model (the first warping equation).
    pub fn base_equations(&self) -> usize {
        self.base
    }
    /// The number of warping unknowns.
    pub fn n_warping(&self) -> usize {
        self.nodes.len()
    }
    /// All equations, `base + n_warping`.
    pub fn n_equations(&self) -> usize {
        self.base + self.nodes.len()
    }
    /// The equation of the warping unknown of `node`, if it has one.
    pub fn equation(&self, node: usize) -> Option<usize> {
        self.of_node.get(node).copied().flatten()
    }
    /// The node of a warping equation, `None` for an equation of the six-unknown model or beyond the last.
    pub fn node_of(&self, equation: usize) -> Option<usize> {
        equation.checked_sub(self.base).and_then(|k| self.nodes.get(k).copied())
    }
}

/// The values and slopes at `xi = x/L` of the cubic Hermite functions of `(phi_0, chi_0, phi_1, chi_1)`, and the second
/// derivatives.
fn hermite(l: f64, xi: f64) -> ([f64; 4], [f64; 4], [f64; 4]) {
    let (x2, x3) = (xi * xi, xi * xi * xi);
    (
        [
            1.0 - 3.0 * x2 + 2.0 * x3,
            l * (xi - 2.0 * x2 + x3),
            3.0 * x2 - 2.0 * x3,
            l * (x3 - x2),
        ],
        [
            (-6.0 * xi + 6.0 * x2) / l,
            1.0 - 4.0 * xi + 3.0 * x2,
            (6.0 * xi - 6.0 * x2) / l,
            3.0 * x2 - 2.0 * xi,
        ],
        [
            (-6.0 + 12.0 * xi) / (l * l),
            (-4.0 + 6.0 * xi) / l,
            (6.0 - 12.0 * xi) / (l * l),
            (-2.0 + 6.0 * xi) / l,
        ],
    )
}

/// `int c(xi) a_i a_j dx` over the element on the twist unknowns, `a` the slopes (`second = false`) or the second derivatives
/// of the Hermite functions; four-point Gauss (exact for `c` linear).
fn twist_block(l: f64, c: impl Fn(f64) -> f64, second: bool) -> [[f64; 4]; 4] {
    let mut m = [[0.0; 4]; 4];
    for (x, w) in GAUSS4_X.iter().zip(GAUSS4_W) {
        let xi = 0.5 * (x + 1.0);
        let wl = 0.5 * w * l * c(xi);
        let (_, d1, d2) = hermite(l, xi);
        let a = if second { d2 } else { d1 };
        for i in 0..4 {
            for j in 0..4 {
                m[i][j] += wl * a[i] * a[j];
            }
        }
    }
    m
}

/// A 12 x 12 matrix in the 14-unknown order, the rows and columns of the twist (`3`, `9` of the 12) left out (zero).
fn embed_without_twist(k12: &ElementMatrix) -> ElementMatrix {
    let mut k = ElementMatrix::zeros(14);
    let src = k12.values();
    let v = k.values_mut();
    for i in 0..12 {
        if i == 3 || i == 9 {
            continue;
        }
        for j in 0..12 {
            if j == 3 || j == 9 {
                continue;
            }
            v[index_of(i) * 14 + index_of(j)] = src[i * 12 + j];
        }
    }
    k
}

/// A 12 x 12 matrix in the 14-unknown order (every entry).
fn embed(k12: &ElementMatrix) -> ElementMatrix {
    let mut k = ElementMatrix::zeros(14);
    let src = k12.values();
    let v = k.values_mut();
    for i in 0..12 {
        for j in 0..12 {
            v[index_of(i) * 14 + index_of(j)] = src[i * 12 + j];
        }
    }
    k
}

fn add_twist(k: &mut ElementMatrix, c: f64, m: &[[f64; 4]; 4]) {
    let v = k.values_mut();
    for a in 0..4 {
        for b in 0..4 {
            v[TWIST[a] * 14 + TWIST[b]] += c * m[a][b];
        }
    }
}

/// The 14 x 14 stiffness in local axes (ADR 0546 D2): [`crate::beam::local_stiffness`] in the 14-unknown order with its torsion
/// pair replaced by the Hermite energy `(1/2) int (G J phi'^2 + E C_w phi''^2) dx` on `(phi_0, chi_0, phi_1, chi_1)`.
pub fn local_stiffness(length: f64, section: &BeamSection, material: &Material, c_w: f64) -> ElementMatrix {
    let (e, g) = beam::moduli(material);
    let mut k = embed_without_twist(&beam::local_stiffness(length, section, material));
    add_twist(&mut k, g * section.torsion(), &twist_block(length, |_| 1.0, false));
    add_twist(&mut k, e * c_w, &twist_block(length, |_| 1.0, true));
    k
}

/// The twist and section rotations `theta`, their derivatives and the translation slopes over the 14 unknowns.
type Rows14 = ([[f64; 14]; 3], [[f64; 14]; 3], [[f64; 14]; 3]);

/// The rows of the second-order fields of ADR 0508 D1 in the 14-unknown order at `xi`, the twist and its slope on the Hermite
/// functions.
fn rows14(length: f64, section: &BeamSection, material: &Material, xi: f64) -> Rows14 {
    let r = beam::second_order_rows(length, section, material, xi);
    let mut theta = [[0.0; 14]; 3];
    let mut theta_p = [[0.0; 14]; 3];
    let mut slope = [[0.0; 14]; 3];
    for c in 0..3 {
        for i in 0..12 {
            slope[c][index_of(i)] = r.slope[c][i];
            if c > 0 {
                theta[c][index_of(i)] = r.theta[c][i];
                theta_p[c][index_of(i)] = r.theta_p[c][i];
            }
        }
    }
    let (h, d1, _) = hermite(length, xi);
    for a in 0..4 {
        theta[0][TWIST[a]] = h[a];
        theta_p[0][TWIST[a]] = d1[a];
    }
    (theta, theta_p, slope)
}

fn add_product(k: &mut ElementMatrix, c: f64, a: &[f64; 14], b: &[f64; 14]) {
    let v = k.values_mut();
    for i in 0..14 {
        for j in 0..14 {
            v[i * 14 + j] += c * (a[i] * b[j] + b[i] * a[j]);
        }
    }
}

/// The 14 x 14 geometric stiffness in local axes (ADR 0546 D2): the axial-force terms of
/// [`crate::beam::local_geometric_stiffness`] in the 14-unknown order with Wagner's `N (I_y + I_z)/A int phi'^2` on the Hermite
/// twist and, when `end_forces` (the twelve local end forces of ADR 0508 D3) are given, the moment and shear-force terms of ADR
/// 0508 D1 with the Hermite twist and the Wagner terms `(1/2) int (beta_y M_y - beta_z M_z) phi'^2 dx` of `wagner`.
pub fn local_geometric_stiffness(
    length: f64,
    section: &BeamSection,
    material: &Material,
    axial_force: f64,
    end_forces: Option<&[f64; 12]>,
    wagner: [f64; 2],
) -> ElementMatrix {
    let l = length;
    let mut k = embed_without_twist(&beam::local_geometric_stiffness(l, section, material, axial_force));
    let nip = axial_force * (section.i_y() + section.i_z()) / section.area();
    add_twist(&mut k, nip, &twist_block(l, |_| 1.0, false));
    let Some(f) = end_forces else {
        return k;
    };
    let (r0, r1) = beam::end_resultants(f);
    for (x, w) in GAUSS4_X.iter().zip(GAUSS4_W) {
        let xi = 0.5 * (x + 1.0);
        let wl = 0.5 * w * l;
        let (th, thp, sl) = rows14(l, section, material, xi);
        let m = |c: usize| (1.0 - xi) * r0[c] + xi * r1[c];
        let (vy, vz, t, my, mz) = (m(1), m(2), m(3), m(4), m(5));
        let (ph, ty, tz) = (&th[0], &th[1], &th[2]);
        let (php, typ, tzp) = (&thp[0], &thp[1], &thp[2]);
        let (up, vp, wp) = (&sl[0], &sl[1], &sl[2]);
        // Moment terms (ADR 0508 D1).
        add_product(&mut k, -0.5 * wl * t, ty, tzp);
        add_product(&mut k, 0.5 * wl * t, tz, typ);
        add_product(&mut k, -0.5 * wl * my, tz, php);
        add_product(&mut k, 0.5 * wl * my, ph, tzp);
        add_product(&mut k, -0.5 * wl * mz, ph, typ);
        add_product(&mut k, 0.5 * wl * mz, ty, php);
        // Shear-force terms.
        add_product(&mut k, 0.5 * wl * vy, ph, ty);
        add_product(&mut k, -wl * vy, tz, up);
        add_product(&mut k, wl * vy, ph, wp);
        add_product(&mut k, 0.5 * wl * vz, ph, tz);
        add_product(&mut k, -wl * vz, ph, vp);
        add_product(&mut k, wl * vz, ty, up);
        // Per-section Wagner terms: the Hessian of (1/2) b phi'^2 is b phi'_a phi'_b (add_product doubles).
        let b = wagner[0] * my - wagner[1] * mz;
        add_product(&mut k, 0.5 * wl * b, php, php);
    }
    k
}

/// `T^T A T` for a 14 x 14 local matrix, `T = diag(R, R, 1, R, R, 1)` with `R` the rows `e_1, e_2, e_3` (ADR 0546 D2).
pub fn to_global(a: &ElementMatrix, axes: &[Vec3; 3]) -> ElementMatrix {
    let mut t = [[0.0; 14]; 14];
    for node in 0..2 {
        let o = 7 * node;
        for blk in 0..2 {
            for r in 0..3 {
                for c in 0..3 {
                    t[o + 3 * blk + r][o + 3 * blk + c] = axes[r][c];
                }
            }
        }
        t[o + 6][o + 6] = 1.0;
    }
    let av = a.values();
    // B = A T.
    let mut b = [[0.0; 14]; 14];
    for i in 0..14 {
        for j in 0..14 {
            let mut s = 0.0;
            for k in 0..14 {
                s += av[i * 14 + k] * t[k][j];
            }
            b[i][j] = s;
        }
    }
    let mut out = ElementMatrix::zeros(14);
    let ov = out.values_mut();
    for i in 0..14 {
        for j in 0..14 {
            let mut s = 0.0;
            for k in 0..14 {
                s += t[k][i] * b[k][j];
            }
            ov[i * 14 + j] = s;
        }
    }
    for i in 0..14 {
        for j in 0..i {
            let s = 0.5 * (ov[i * 14 + j] + ov[j * 14 + i]);
            ov[i * 14 + j] = s;
            ov[j * 14 + i] = s;
        }
    }
    out
}

/// The global 14 x 14 stiffness of a warping element.
pub fn element_stiffness(
    coords: &[Vec3],
    section: &BeamSection,
    orientation: Vec3,
    material: &Material,
    c_w: f64,
) -> Result<ElementMatrix> {
    let f = beam::frame_of(coords, orientation)?;
    Ok(to_global(&local_stiffness(f.length, section, material, c_w), &f.axes))
}

/// The global 14 x 14 geometric stiffness of a warping element ([`local_geometric_stiffness`]). Refuses non-finite inputs
/// (key `"stress"`).
pub fn element_geometric_stiffness(
    coords: &[Vec3],
    section: &BeamSection,
    orientation: Vec3,
    material: &Material,
    axial_force: f64,
    end_forces: Option<&[f64; 12]>,
    wagner: [f64; 2],
) -> Result<ElementMatrix> {
    let finite = axial_force.is_finite()
        && wagner.iter().all(|v| v.is_finite())
        && end_forces.map_or(true, |f| f.iter().all(|v| v.is_finite()));
    if !finite {
        return Err(FemError::InvalidValue {
            key: "stress",
            reason: "the axial force, the end forces or the Wagner coefficients of a warping element are not finite"
                .to_string(),
        });
    }
    let f = beam::frame_of(coords, orientation)?;
    Ok(to_global(
        &local_geometric_stiffness(f.length, section, material, axial_force, end_forces, wagner),
        &f.axes,
    ))
}

/// What the checks of a model and its [`Warping`] give: the numbering, each element's section (`None` outside the warping
/// blocks) and the restrained warping equations (ascending).
struct Layout {
    dofs: WarpingDofs,
    section_of: Vec<Option<WarpingSection>>,
    restrained: Vec<usize>,
}

fn layout(model: &SolidModel, warping: &Warping) -> Result<Layout> {
    let mesh = model.mesh();
    // RF1.
    if warping.blocks.is_empty() {
        return Err(warping_error("no block takes the warping unknown".to_string()));
    }
    let mut of_block: Vec<Option<WarpingSection>> = vec![None; mesh.blocks().len()];
    for &(b, s) in &warping.blocks {
        let Some(block) = mesh.blocks().get(b) else {
            return Err(warping_error(format!(
                "block {b} does not exist (the mesh has {})",
                mesh.blocks().len()
            )));
        };
        if block.kind() != ElementKind::Beam2 || !matches!(block.property(), Some(BlockProperty::Beam { .. })) {
            return Err(warping_error(format!(
                "block {b} is not a Beam2 block with a Beam property: only a beam takes the warping unknown"
            )));
        }
        if of_block[b].is_some() {
            return Err(warping_error(format!("block {b} is named twice")));
        }
        of_block[b] = Some(s);
    }
    let n_nodes = mesh.nodes().len();
    let section_of: Vec<Option<WarpingSection>> = (0..mesh.n_elements()).map(|e| of_block[mesh.block_of(e)]).collect();
    let mut other = vec![false; n_nodes];
    let mut frame_at: Vec<Option<[Vec3; 2]>> = vec![None; n_nodes];
    for (e, s) in section_of.iter().enumerate() {
        if s.is_none() {
            for &n in mesh.element_nodes(e) {
                other[n] = true;
            }
        }
    }
    for (e, s) in section_of.iter().enumerate() {
        if s.is_none() {
            continue;
        }
        let nodes = mesh.element_nodes(e);
        let (_, v) = beam::beam_property(mesh, e).expect("a Beam2 element of a Beam block");
        let f = beam::frame(mesh.nodes()[nodes[0]], mesh.nodes()[nodes[1]], v).map_err(|source| FemError::Element {
            element: e,
            source: Box::new(source),
        })?;
        let axes = [f.axes[0], f.axes[1]];
        for &n in nodes {
            // RF2.
            if other[n] {
                return Err(warping_error(format!(
                    "node {n} of the warping element {e} is also a node of an element outside the warping blocks"
                )));
            }
            // RF3.
            match frame_at[n] {
                None => frame_at[n] = Some(axes),
                Some(prev) => {
                    let d = (0..2)
                        .flat_map(|k| (0..3).map(move |c| (k, c)))
                        .map(|(k, c)| (prev[k][c] - axes[k][c]).abs())
                        .fold(0.0, f64::max);
                    if d > FRAME_TOLERANCE {
                        return Err(warping_error(format!(
                            "the warping elements at node {n} have different frames ({d:e} apart; element {e}): the \
                             warping unknown is offered along a straight member of one orientation only"
                        )));
                    }
                }
            }
        }
    }
    let base = model.dof_map().n_equations();
    let mut of_node = vec![None; n_nodes];
    let mut nodes = Vec::new();
    for n in 0..n_nodes {
        if frame_at[n].is_some() {
            of_node[n] = Some(base + nodes.len());
            nodes.push(n);
        }
    }
    let dofs = WarpingDofs { base, of_node, nodes };
    let mut restrained = Vec::new();
    for name in &warping.restrained {
        // RF6.
        let set = mesh.node_set(name).ok_or_else(|| FemError::UnknownSet {
            kind: SetKind::Node,
            name: name.clone(),
        })?;
        for &n in set {
            let eq = dofs.equation(n).ok_or_else(|| {
                warping_error(format!(
                    "node {n} of the restrained set '{name}' has no warping unknown"
                ))
            })?;
            restrained.push(eq);
        }
    }
    restrained.sort_unstable();
    restrained.dedup();
    Ok(Layout {
        dofs,
        section_of,
        restrained,
    })
}

/// The numbering of the warping unknowns of `model` for `warping`, after the checks RF1, RF2, RF3 and RF6 of ADR 0546 D4.
pub fn warping_dofs(model: &SolidModel, warping: &Warping) -> Result<WarpingDofs> {
    Ok(layout(model, warping)?.dofs)
}

/// The augmented matrices of [`matrices`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WarpingMatrices {
    /// `K` on all equations (the model's then the warping ones).
    pub stiffness: CsrMatrix,
    /// `K_sigma` on the same pattern.
    pub geometric: CsrMatrix,
    /// The numbering.
    pub dofs: WarpingDofs,
    /// The restrained warping equations, ascending.
    pub restrained: Vec<usize>,
}

fn entry_checks(model: &SolidModel) -> Result<()> {
    model.validate()?;
    model.check_supported_with_materials()?;
    crate::mat_ortho::refuse_with_cyclic(model)?;
    // RF4.
    if crate::mpc::uses_transformation(model) {
        return Err(warping_error(
            "the warping unknown is not offered on the transformation path (linear constraints, rigid sets, cyclic \
             constraints, elastic supports)"
                .to_string(),
        ));
    }
    Ok(())
}

/// The stiffness and geometric stiffness of `model` with the warping unknowns of `warping` (ADR 0546 D5): `K` of the model plus,
/// per warping element, the difference of its 14-unknown and its 12-unknown stiffness; `K_sigma` of the model with the warping
/// elements masked plus their 14-unknown geometric stiffness (the block's Wagner coefficients; the end forces of
/// `prestress` when it has them). Both on the union of their patterns.
pub fn matrices(
    model: &SolidModel,
    prestress: &Prestress,
    warping: &Warping,
    threads: Option<usize>,
) -> Result<WarpingMatrices> {
    matrices_impl(model, prestress, warping, threads, true)
}

/// [`matrices`], with the refusal RF8 of a torque in the pre-stress applied only when `check_torque` (ADR 0548 D3).
fn matrices_impl(
    model: &SolidModel,
    prestress: &Prestress,
    warping: &Warping,
    threads: Option<usize>,
    check_torque: bool,
) -> Result<WarpingMatrices> {
    entry_checks(model)?;
    prestress.check_fits(model)?;
    let lay = layout(model, warping)?;
    let mesh = model.mesh();
    let base_dofs = model.dof_map();
    let k = point_stiffness(model, threads)?;
    let masked = prestress.masked(|e| lay.section_of[e].is_none());
    let kg = assemble_geometric(model, &masked, threads)?;
    let mut tk: Vec<(usize, usize, f64)> = Vec::with_capacity(2 * (k.val.len() + kg.val.len()));
    let mut tg: Vec<(usize, usize, f64)> = Vec::with_capacity(tk.capacity());
    for (m, own_is_k) in [(&k, true), (&kg, false)] {
        for r in 0..m.n {
            for p in m.row_ptr[r]..m.row_ptr[r + 1] {
                let (c, v) = (m.col[p], m.val[p]);
                if own_is_k {
                    tk.push((r, c, v));
                    tg.push((r, c, 0.0));
                } else {
                    tg.push((r, c, v));
                    tk.push((r, c, 0.0));
                }
            }
        }
    }
    let materials = model.material_indices()?;
    for (e, ws) in lay.section_of.iter().enumerate() {
        let Some(ws) = ws else {
            continue;
        };
        let located = |source: FemError| FemError::Element {
            element: e,
            source: Box::new(source),
        };
        let nodes = mesh.element_nodes(e);
        let (section, v) = beam::beam_property(mesh, e).expect("a Beam2 element of a Beam block");
        let coords = [mesh.nodes()[nodes[0]], mesh.nodes()[nodes[1]]];
        let material = &model.materials()[materials[e]].1;
        let f = beam::frame_of(&coords, v).map_err(located)?;
        let base_eqs = element_dofs(&base_dofs, nodes);
        let mut eqs = Vec::with_capacity(14);
        for (i, &n) in nodes.iter().enumerate() {
            eqs.extend_from_slice(&base_eqs[6 * i..6 * i + 6]);
            eqs.push(lay.dofs.equation(n).expect("a node of a warping element"));
        }
        // K: the 14-unknown stiffness minus the 12-unknown one (the torsion block only).
        let mut dk = local_stiffness(f.length, &section, material, ws.c_w);
        let k12 = embed(&beam::local_stiffness(f.length, &section, material));
        for (a, b) in dk.values_mut().iter_mut().zip(k12.values()) {
            *a -= b;
        }
        let dk = to_global(&dk, &f.axes);
        // K_sigma: the axial force of the element's stress point and its end forces.
        let stress = prestress.of_element(e);
        let n_ax = section.area() * beam::normal_stress(&stress[0], f.axes[0]);
        let ends = prestress.beam_terms(e).map(|(ef, _)| *ef);
        if let (true, Some(ef)) = (check_torque, &ends) {
            // RF8.
            let l = f.length;
            let ip = ((section.i_y() + section.i_z()) / section.area()).sqrt();
            let scale = [
                ef[4].abs(),
                ef[5].abs(),
                ef[10].abs(),
                ef[11].abs(),
                ef[1].abs() * l,
                ef[2].abs() * l,
                ef[7].abs() * l,
                ef[8].abs() * l,
                ef[0].abs() * ip,
                ef[6].abs() * ip,
            ]
            .into_iter()
            .fold(0.0, f64::max);
            let torque = ef[3].abs().max(ef[9].abs());
            if torque > TORQUE_TOLERANCE * scale {
                return Err(located(warping_error(format!(
                    "the pre-stress has the torque {torque:e} on a warping element (scale {scale:e}): the pre-stress is \
                     that of the six-unknown static solve, offered for torsion-free reference loads only"
                ))));
            }
        }
        let g = element_geometric_stiffness(&coords, &section, v, material, n_ax, ends.as_ref(), ws.wagner)
            .map_err(located)?;
        let (dkv, gv) = (dk.values(), g.values());
        for i in 0..14 {
            for j in 0..14 {
                tk.push((eqs[i], eqs[j], dkv[i * 14 + j]));
                tk.push((eqs[i], eqs[j], 0.0));
                tg.push((eqs[i], eqs[j], gv[i * 14 + j]));
                tg.push((eqs[i], eqs[j], 0.0));
            }
        }
    }
    let n = lay.dofs.n_equations();
    Ok(WarpingMatrices {
        stiffness: CsrMatrix::from_triplets(n, tk),
        geometric: CsrMatrix::from_triplets(n, tg),
        dofs: lay.dofs,
        restrained: lay.restrained,
    })
}

/// The result of [`buckling_factors`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WarpingBucklingResult {
    /// Load factors, positive, ascending.
    pub factors: Vec<f64>,
    /// Modes, `K`-normalised, one value per equation of [`WarpingDofs`] (0 at prescribed and restrained equations).
    pub modes: Vec<Vec<f64>>,
    /// Residuals of the pairs.
    pub residuals: Vec<f64>,
    /// Iterations of the subspace iteration.
    pub iterations: usize,
    /// The Sturm check.
    pub sturm: SturmCheck,
    /// Number of free unknowns.
    pub unknowns: usize,
    /// `Some(S)` when fewer factors than asked exist below `S`.
    pub exhausted: Option<f64>,
    /// The numbering of the modes.
    pub dofs: WarpingDofs,
}

/// The lowest positive buckling factors of `model` with the warping unknowns of `warping` for the pre-stress `prestress`
/// (ADR 0546 D5): `(K_aug + lambda K_sigma,aug) x = 0` of [`matrices`], the model's zero prescribed displacements and the
/// restrained warping unknowns eliminated, the zero-shift pivot rule of ADR 0402, the eigensolver of
/// [`crate::buckling::factors`] with the options. Refuses RF1 to RF9 of ADR 0546 D4 (a nonzero prescribed value with key
/// `"constraint"`, `coordinate_order` with key `"factorisation"`).
pub fn buckling_factors(
    model: &SolidModel,
    prestress: &Prestress,
    warping: &Warping,
    options: &BucklingOptions,
) -> Result<WarpingBucklingResult> {
    buckling_impl(model, prestress, warping, options, true)
}

/// [`buckling_factors`], with RF8 applied only when `check_torque` (ADR 0548 D3).
fn buckling_impl(
    model: &SolidModel,
    prestress: &Prestress,
    warping: &Warping,
    options: &BucklingOptions,
    check_torque: bool,
) -> Result<WarpingBucklingResult> {
    // RF9.
    if options.coordinate_order {
        return Err(FemError::InvalidValue {
            key: "factorisation",
            reason: "the coordinate order is not offered on the warping path (ADR 0546)".to_string(),
        });
    }
    entry_checks(model)?;
    let base_dofs = model.dof_map();
    let fixed = prescribed_equations(model, &[])?;
    // RF5.
    if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
        let (node, component) = base_dofs.node_component(eq).expect("existing equation");
        return Err(FemError::InvalidValue {
            key: "constraint",
            reason: format!(
                "prescribed value {v:e} at node {node} component {component}: a buckling analysis takes zero values only"
            ),
        });
    }
    check_rigid_body_motions(model, &fixed)?;
    let mats = matrices_impl(model, prestress, warping, options.threads, check_torque)?;
    let mut all = fixed;
    all.extend(mats.restrained.iter().map(|&eq| (eq, 0.0)));
    let n = mats.dofs.n_equations();
    let zero = vec![0.0; n];
    let kr = reduce(&mats.stiffness, &zero, &all)?;
    let gr = reduce(&mats.geometric, &zero, &all)?;
    let free = kr.free().to_vec();
    let nf = free.len();
    if options.factors == 0 || options.factors >= nf {
        return Err(FemError::InvalidValue {
            key: "factors",
            reason: format!(
                "{} factors requested of a model with {nf} free unknowns (at least 1 and fewer than the unknowns)",
                options.factors
            ),
        });
    }
    if let Ok(Some((row, ratio))) = crate::mpc::zero_shift_failing_pivot(&kr.matrix) {
        let eq = free[row];
        let name = match base_dofs.node_component(eq) {
            Some((node, component)) => format!("node {node} component {component}"),
            None => format!(
                "the warping unknown of node {}",
                mats.dofs.node_of(eq).expect("a warping equation")
            ),
        };
        return Err(FemError::Solver {
            solver: "lowest_buckling_factors",
            reason: format!(
                "{name}: pivot ratio {ratio:e} of K_FF at zero shift, below 1e-8 (the rule of Shift::Auto, ADR 0384): \
                 the prescribed displacements leave a motion free (a mechanism); a buckling analysis needs every motion \
                 held (ADR 0402)"
            ),
        });
    }
    let opts = BucklingEigenOptions {
        tolerance: options.tolerance,
        residual_test: options.residual_test,
        max_iterations: options.max_iterations,
        subspace_size: options.subspace_size,
        factorisation: options.factorisation,
        ..BucklingEigenOptions::default()
    };
    let eig =
        lowest_buckling_factors(&kr.matrix, &gr.matrix, options.factors, &opts).map_err(|e| FemError::Solver {
            solver: "lowest_buckling_factors",
            reason: e.to_string(),
        })?;
    let modes = eig
        .vectors
        .iter()
        .map(|v| {
            let mut full = vec![0.0; n];
            for (&eq, &x) in free.iter().zip(v) {
                full[eq] = x;
            }
            full
        })
        .collect();
    Ok(WarpingBucklingResult {
        factors: eig.values,
        modes,
        residuals: eig.residuals,
        iterations: eig.iterations,
        sturm: eig.sturm,
        unknowns: nf,
        exhausted: eig.exhausted,
        dofs: mats.dofs,
    })
}

// ---------------------------------------------------------------------------------------------------------------------------
// ADR 0548 (F-626): the warping unknown on the static solve, the modes and the pre-stressed modes.
// ---------------------------------------------------------------------------------------------------------------------------

/// `int H_i H_j dx` of the cubic Hermite twist functions over the element (four-point Gauss, exact for the degree 6).
fn hermite_mass_block(l: f64) -> [[f64; 4]; 4] {
    let mut m = [[0.0; 4]; 4];
    for (x, w) in GAUSS4_X.iter().zip(GAUSS4_W) {
        let xi = 0.5 * (x + 1.0);
        let wl = 0.5 * w * l;
        let (h, _, _) = hermite(l, xi);
        for i in 0..4 {
            for j in 0..4 {
                m[i][j] += wl * h[i] * h[j];
            }
        }
    }
    m
}

/// The 14 x 14 consistent mass in local axes (ADR 0548 D4): [`crate::beam::local_mass`] in the 14-unknown order with its
/// torsion pair `rho I_p L/6 [2, 1; 1, 2]` replaced by `rho I_p int H_i H_j dx` on the Hermite twist, `I_p = I_y + I_z`. No
/// warping inertia `rho C_w`.
pub fn local_mass(length: f64, section: &BeamSection, material: &Material) -> ElementMatrix {
    let mut m = embed_without_twist(&beam::local_mass(length, section, material));
    let rho_ip = material.density() * (section.i_y() + section.i_z());
    add_twist(&mut m, rho_ip, &hermite_mass_block(length));
    m
}

/// The global 14 x 14 consistent mass of a warping element.
pub fn element_mass(
    coords: &[Vec3],
    section: &BeamSection,
    orientation: Vec3,
    material: &Material,
) -> Result<ElementMatrix> {
    let f = beam::frame_of(coords, orientation)?;
    Ok(to_global(&local_mass(f.length, section, material), &f.axes))
}

/// A warping element of a model: its index, its 14 global equations, its section, frame and material index.
struct WarpElement {
    element: usize,
    eqs: [usize; 14],
    section: BeamSection,
    warping: WarpingSection,
    length: f64,
    axes: [Vec3; 3],
    material: usize,
}

fn warp_elements(model: &SolidModel, lay: &Layout) -> Result<Vec<WarpElement>> {
    let mesh = model.mesh();
    let base_dofs = model.dof_map();
    let materials = model.material_indices()?;
    let mut out = Vec::new();
    for (e, ws) in lay.section_of.iter().enumerate() {
        let Some(ws) = ws else {
            continue;
        };
        let nodes = mesh.element_nodes(e);
        let (section, v) = beam::beam_property(mesh, e).expect("a Beam2 element of a Beam block");
        let coords = [mesh.nodes()[nodes[0]], mesh.nodes()[nodes[1]]];
        let f = beam::frame_of(&coords, v).map_err(|source| FemError::Element {
            element: e,
            source: Box::new(source),
        })?;
        let base_eqs = element_dofs(&base_dofs, nodes);
        let mut eqs = [0usize; 14];
        for (i, &n) in nodes.iter().enumerate() {
            eqs[7 * i..7 * i + 6].copy_from_slice(&base_eqs[6 * i..6 * i + 6]);
            eqs[7 * i + 6] = lay.dofs.equation(n).expect("a node of a warping element");
        }
        out.push(WarpElement {
            element: e,
            eqs,
            section,
            warping: *ws,
            length: f.length,
            axes: f.axes,
            material: materials[e],
        });
    }
    Ok(out)
}

/// The triplets of a matrix of the six-unknown model.
fn triplets_of(m: &CsrMatrix) -> Vec<(usize, usize, f64)> {
    let mut t = Vec::with_capacity(m.val.len());
    for r in 0..m.n {
        for p in m.row_ptr[r]..m.row_ptr[r + 1] {
            t.push((r, m.col[p], m.val[p]));
        }
    }
    t
}

/// `base` (a matrix of the six-unknown model) plus, per warping element, `T^T (A14 - embed(A12)) T` of the pair `local`
/// returns (`A14`, `A12`).
fn augmented(
    base: &CsrMatrix,
    model: &SolidModel,
    lay: &Layout,
    local: impl Fn(&WarpElement, &Material) -> (ElementMatrix, ElementMatrix),
) -> Result<CsrMatrix> {
    let mut t = triplets_of(base);
    for w in warp_elements(model, lay)? {
        let material = &model.materials()[w.material].1;
        let (mut d, a12) = local(&w, material);
        let a12 = embed(&a12);
        for (a, b) in d.values_mut().iter_mut().zip(a12.values()) {
            *a -= b;
        }
        let d = to_global(&d, &w.axes);
        let dv = d.values();
        for i in 0..14 {
            for j in 0..14 {
                t.push((w.eqs[i], w.eqs[j], dv[i * 14 + j]));
            }
        }
    }
    Ok(CsrMatrix::from_triplets(lay.dofs.n_equations(), t))
}

fn stiffness_aug(model: &SolidModel, lay: &Layout, threads: Option<usize>) -> Result<CsrMatrix> {
    let k = point_stiffness(model, threads)?;
    augmented(&k, model, lay, |w, m| {
        (
            local_stiffness(w.length, &w.section, m, w.warping.c_w),
            beam::local_stiffness(w.length, &w.section, m),
        )
    })
}

fn mass_aug(model: &SolidModel, lay: &Layout, threads: Option<usize>) -> Result<CsrMatrix> {
    let m = crate::mass::consistent_with(model, threads)?;
    augmented(&m, model, lay, |w, mat| {
        (
            local_mass(w.length, &w.section, mat),
            beam::local_mass(w.length, &w.section, mat),
        )
    })
}

/// The name of an equation of the augmented numbering: `(node, component)`, component 6 the warping unknown.
fn node_component(model_dofs: &crate::model::DofMap, dofs: &WarpingDofs, eq: usize) -> (usize, usize) {
    match model_dofs.node_component(eq) {
        Some(nc) => nc,
        None => (dofs.node_of(eq).expect("a warping equation"), 6),
    }
}

/// The result of [`solve_static`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WarpingStaticResult {
    /// The displacement on every equation of [`WarpingDofs`]: the model's `DofMap` order, then the warping unknowns.
    pub displacement: Vec<f64>,
    /// `K u - f` at the prescribed and the restrained equations, 0 elsewhere (a bimoment at a restrained warping unknown).
    pub reactions: Vec<f64>,
    /// The numbering.
    pub dofs: WarpingDofs,
    /// `|f_F - K_FF u_F| / |f_F|` of the reduced system (0 when `f_F` is zero).
    pub relative_residual: f64,
}

impl WarpingStaticResult {
    /// The displacement of the six-unknown model (a vector of the model's `DofMap`).
    pub fn six_unknown(&self) -> &[f64] {
        &self.displacement[..self.dofs.base_equations()]
    }
    /// The warping unknown `chi = phi'` of `node`, if it has one.
    pub fn warping_at(&self, node: usize) -> Option<f64> {
        self.dofs.equation(node).map(|eq| self.displacement[eq])
    }
}

/// The linear static solve of `model` with the warping unknowns of `warping` (ADR 0548 D1): `K_aug` of ADR 0546 D5, the model's
/// nodal loads (no bimoment load), its prescribed values and the restrained warping unknowns (`chi = 0`), the free rigid
/// motions refused as [`crate::solve::solve_static`], the sparse `L D L^T` after minimum degree with the pivot rule of ADR
/// 0275 (`FemError::SingularStiffness`, component 6 for a warping unknown), the reactions. Refuses RF1 to RF4 and RF6 of ADR
/// 0546 D4.
pub fn solve_static(model: &SolidModel, warping: &Warping, threads: Option<usize>) -> Result<WarpingStaticResult> {
    entry_checks(model)?;
    let lay = layout(model, warping)?;
    let base_dofs = model.dof_map();
    let mut f = crate::loads::nodal_load_vector(model, &[])?;
    let mut fixed = prescribed_equations(model, &[])?;
    check_rigid_body_motions(model, &fixed)?;
    let k = stiffness_aug(model, &lay, threads)?;
    let n = lay.dofs.n_equations();
    f.resize(n, 0.0);
    fixed.extend(lay.restrained.iter().map(|&eq| (eq, 0.0)));
    let sys = reduce(&k, &f, &fixed)?;
    let free = sys.free().to_vec();
    let u_free = if free.is_empty() {
        Vec::new()
    } else {
        let diag = sys.matrix.diag();
        let singular = |row: usize, pivot: f64| {
            let (node, component) = node_component(&base_dofs, &lay.dofs, free[row]);
            FemError::SingularStiffness {
                node,
                component,
                pivot,
                diagonal: diag[row],
            }
        };
        let ldlt = rcfd_linalg::sparse_chol::SparseLdlt::factor(&sys.matrix).map_err(|e| match e {
            rcfd_linalg::skyline::SkylineError::Pivot { row, pivot, .. } => singular(row, pivot),
            other => FemError::Solver {
                solver: "warping static solve",
                reason: other.to_string(),
            },
        })?;
        for (&row, &d) in ldlt.order().iter().zip(ldlt.pivots()) {
            if d <= crate::solve::PIVOT_TOLERANCE * diag[row] {
                return Err(singular(row, d));
            }
        }
        ldlt.solve(&sys.rhs)
    };
    let relative_residual = {
        let mut ku = vec![0.0; u_free.len()];
        if !u_free.is_empty() {
            sys.matrix.matvec(&u_free, &mut ku);
        }
        let num: f64 = ku
            .iter()
            .zip(&sys.rhs)
            .map(|(a, b)| (b - a) * (b - a))
            .sum::<f64>()
            .sqrt();
        let den: f64 = sys.rhs.iter().map(|b| b * b).sum::<f64>().sqrt();
        if den > 0.0 {
            num / den
        } else {
            0.0
        }
    };
    let u = sys.expand(&u_free);
    let reactions = crate::constraint::reactions(&k, &u, &f, &fixed);
    Ok(WarpingStaticResult {
        displacement: u,
        reactions,
        dofs: lay.dofs,
        relative_residual,
    })
}

/// Where a [`WarpingPrestress`] comes from (ADR 0548 D2).
#[derive(Debug, Clone, PartialEq)]
enum Source {
    TorsionFree,
    WithTorsion,
    Static {
        blocks: Vec<(usize, WarpingSection)>,
        dofs: WarpingDofs,
    },
}

/// The pre-stress of the warping paths (ADR 0548 D2): a [`Prestress`] and its source, which decides whether a torque on a
/// warping element is taken (RF8 of ADR 0546 applies to [`WarpingPrestress::torsion_free`] only).
#[derive(Debug, Clone, PartialEq)]
pub struct WarpingPrestress {
    prestress: Prestress,
    source: Source,
}

impl WarpingPrestress {
    /// The pre-stress of the warping static solve `result` of `model` and `warping` (ADR 0548 D2): the stress points of
    /// [`Prestress::from_static`] of the six-unknown part, and the end forces of every Beam2 element, those of a warping element
    /// its 14-unknown local forces `K14 u` without the two bimoments (so its torque is the total torque `G J phi' - E C_w
    /// phi'''`), those of another beam [`crate::beam::local_end_forces`]; the Wagner coefficients `wagner` of the beams outside
    /// the warping blocks (each warping block has its own). Refuses a result of another numbering (key `"warping"`).
    pub fn from_static(
        model: &SolidModel,
        warping: &Warping,
        result: &WarpingStaticResult,
        wagner: [f64; 2],
    ) -> Result<WarpingPrestress> {
        let lay = layout(model, warping)?;
        if lay.dofs != result.dofs || result.displacement.len() != lay.dofs.n_equations() {
            return Err(warping_error(
                "the static result has another numbering of the warping unknowns than this model and warping"
                    .to_string(),
            ));
        }
        let six = result.six_unknown();
        let sr = crate::model::SolidResult::new(model.dof_map(), six.to_vec())?;
        let p = Prestress::from_static(model, &sr)?;
        let mut forces = crate::geometric_struct::beam_end_forces(model, six)?;
        for w in warp_elements(model, &lay)? {
            let material = &model.materials()[w.material].1;
            let mut ul = [0.0; 14];
            for node in 0..2 {
                let o = 7 * node;
                for blk in 0..2 {
                    for k in 0..3 {
                        ul[o + 3 * blk + k] = (0..3)
                            .map(|c| w.axes[k][c] * result.displacement[w.eqs[o + 3 * blk + c]])
                            .sum();
                    }
                }
                ul[o + 6] = result.displacement[w.eqs[o + 6]];
            }
            let kl = local_stiffness(w.length, &w.section, material, w.warping.c_w);
            let kv = kl.values();
            let mut f14 = [0.0; 14];
            for (i, o) in f14.iter_mut().enumerate() {
                *o = (0..14).map(|j| kv[i * 14 + j] * ul[j]).sum();
            }
            let fe = &mut forces[w.element];
            for (i, v) in fe.iter_mut().enumerate() {
                *v = f14[index_of(i)];
            }
        }
        Ok(WarpingPrestress {
            prestress: p.with_beam_end_forces(forces, wagner),
            source: Source::Static {
                blocks: warping.blocks().to_vec(),
                dofs: lay.dofs,
            },
        })
    }

    /// Any pre-stress, taken as ADR 0546 takes it: a torque on a warping element is refused where it is used (RF8).
    pub fn torsion_free(prestress: Prestress) -> WarpingPrestress {
        WarpingPrestress {
            prestress,
            source: Source::TorsionFree,
        }
    }

    /// A pre-stress whose end forces the caller states are those of the warping model (a closed form, a check): a torque is
    /// taken as given. Not checked.
    pub fn with_torsion(prestress: Prestress) -> WarpingPrestress {
        WarpingPrestress {
            prestress,
            source: Source::WithTorsion,
        }
    }

    /// The same pre-stress times `factor`.
    pub fn scaled(&self, factor: f64) -> WarpingPrestress {
        WarpingPrestress {
            prestress: self.prestress.scaled(factor),
            source: self.source.clone(),
        }
    }

    /// The pre-stress.
    pub fn prestress(&self) -> &Prestress {
        &self.prestress
    }

    /// Whether it was made by [`WarpingPrestress::from_static`].
    pub fn is_from_static(&self) -> bool {
        matches!(self.source, Source::Static { .. })
    }

    /// RF10 (ADR 0548 D3): a pre-stress of the warping static solve of other blocks or another numbering is refused. Returns
    /// whether RF8 applies.
    fn check_for(&self, model: &SolidModel, warping: &Warping) -> Result<bool> {
        match &self.source {
            Source::TorsionFree => Ok(true),
            Source::WithTorsion => Ok(false),
            Source::Static { blocks, dofs } => {
                let lay = layout(model, warping)?;
                if blocks.as_slice() != warping.blocks() || *dofs != lay.dofs {
                    return Err(warping_error(
                        "the pre-stress comes from the warping static solve of other blocks, sections or numbering"
                            .to_string(),
                    ));
                }
                Ok(false)
            }
        }
    }
}

/// [`buckling_factors`] with the pre-stress of ADR 0548 D2 (D3): a torque on a warping element is taken from a pre-stress of
/// [`WarpingPrestress::from_static`] or [`WarpingPrestress::with_torsion`] and refused (RF8) from
/// [`WarpingPrestress::torsion_free`]; a `from_static` pre-stress of other blocks or sections is refused (RF10, key
/// `"warping"`). The other steps and refusals are those of [`buckling_factors`].
pub fn buckling_factors_with(
    model: &SolidModel,
    prestress: &WarpingPrestress,
    warping: &Warping,
    options: &BucklingOptions,
) -> Result<WarpingBucklingResult> {
    let check_torque = prestress.check_for(model, warping)?;
    buckling_impl(model, &prestress.prestress, warping, options, check_torque)
}

/// The result of [`modal_analysis`] and [`prestressed_modes`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WarpingModalResult {
    /// Eigenvalues `omega^2`, ascending, rad^2/s^2.
    pub eigenvalues: Vec<f64>,
    /// Frequencies, Hz.
    pub frequencies: Vec<f64>,
    /// Modes, M-orthonormal, one value per equation of [`WarpingDofs`] (0 at prescribed and restrained equations).
    pub modes: Vec<Vec<f64>>,
    /// Residuals of the pairs.
    pub residuals: Vec<f64>,
    /// Iterations of the subspace iteration.
    pub iterations: usize,
    /// The shift used.
    pub shift: f64,
    /// The Sturm check.
    pub sturm: SturmCheck,
    /// Number of free unknowns.
    pub unknowns: usize,
    /// Whether the constraints leave a rigid motion free.
    pub rigid_free: bool,
    /// The numbering of the modes.
    pub dofs: WarpingDofs,
}

/// The modes of `model` with the warping unknowns of `warping` (ADR 0548 D5): `K_aug x = omega^2 M_aug x` (the mass of D4), the
/// steps of [`crate::geometric::prestressed_modes`] on the elimination path. Refuses RF1 to RF4, RF6, a nonzero prescribed
/// value (key `"constraint"`), `coordinate_order` (key `"factorisation"`), an unknown without mass (key `"density"`).
pub fn modal_analysis(
    model: &SolidModel,
    warping: &Warping,
    options: &crate::modal::ModalOptions,
) -> Result<WarpingModalResult> {
    modes_impl(model, None, warping, options)
}

/// The pre-stressed modes (ADR 0548 D5): `(K_aug + K_sigma,aug) x = omega^2 M_aug x` with the pre-stress of D2 (RF8 and RF10 as
/// [`buckling_factors_with`]); the other steps and refusals of [`modal_analysis`].
pub fn prestressed_modes(
    model: &SolidModel,
    prestress: &WarpingPrestress,
    warping: &Warping,
    options: &crate::modal::ModalOptions,
) -> Result<WarpingModalResult> {
    modes_impl(model, Some(prestress), warping, options)
}

fn modes_impl(
    model: &SolidModel,
    prestress: Option<&WarpingPrestress>,
    warping: &Warping,
    options: &crate::modal::ModalOptions,
) -> Result<WarpingModalResult> {
    use rcfd_linalg::eigen::{lowest_eigenpairs, Shift, SubspaceOptions};
    if options.coordinate_order {
        return Err(FemError::InvalidValue {
            key: "factorisation",
            reason: "the coordinate order is not offered on the warping path (ADR 0548)".to_string(),
        });
    }
    entry_checks(model)?;
    let lay = layout(model, warping)?;
    let base_dofs = model.dof_map();
    let mut fixed = prescribed_equations(model, &[])?;
    if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
        let (node, component) = base_dofs.node_component(eq).expect("existing equation");
        return Err(FemError::InvalidValue {
            key: "constraint",
            reason: format!(
                "prescribed value {v:e} at node {node} component {component}: a modal analysis takes zero values only"
            ),
        });
    }
    let rigid_free = match check_rigid_body_motions(model, &fixed) {
        Ok(()) => false,
        Err(FemError::RigidBodyMotionFree { .. }) => true,
        Err(e) => return Err(e),
    };
    let k = match prestress {
        None => stiffness_aug(model, &lay, options.threads)?,
        Some(p) => {
            let check_torque = p.check_for(model, warping)?;
            let mats = matrices_impl(model, &p.prestress, warping, options.threads, check_torque)?;
            crate::geometric::add_on_pattern(&mats.stiffness, &mats.geometric)
        }
    };
    let m = mass_aug(model, &lay, options.threads)?;
    let n = lay.dofs.n_equations();
    fixed.extend(lay.restrained.iter().map(|&eq| (eq, 0.0)));
    let zero = vec![0.0; n];
    let kr = reduce(&k, &zero, &fixed)?;
    let mr = reduce(&m, &zero, &fixed)?;
    let free = kr.free().to_vec();
    let nf = free.len();
    if options.modes == 0 || options.modes >= nf {
        return Err(FemError::InvalidValue {
            key: "modes",
            reason: format!(
                "{} modes requested of a model with {nf} free unknowns (at least 1 and fewer than the unknowns)",
                options.modes
            ),
        });
    }
    for (i, d) in mr.matrix.diag().iter().enumerate() {
        if d.is_nan() || *d <= 0.0 {
            let (node, component) = node_component(&base_dofs, &lay.dofs, free[i]);
            return Err(FemError::InvalidValue {
                key: "density",
                reason: format!("node {node} component {component} has no mass (diagonal {d:e})"),
            });
        }
    }
    let shift = options
        .shift
        .unwrap_or(if rigid_free { Shift::Auto } else { Shift::Value(0.0) });
    let opts = SubspaceOptions {
        shift,
        tolerance: options.tolerance,
        residual_test: options.residual_test,
        max_iterations: options.max_iterations,
        subspace_size: options.subspace_size,
        factorisation: options.factorisation,
        ..SubspaceOptions::default()
    };
    let eig = lowest_eigenpairs(&kr.matrix, &mr.matrix, options.modes, &opts).map_err(|e| FemError::Solver {
        solver: "lowest_eigenpairs",
        reason: e.to_string(),
    })?;
    let modes = eig
        .vectors
        .iter()
        .map(|v| {
            let mut full = vec![0.0; n];
            for (&eq, &x) in free.iter().zip(v) {
                full[eq] = x;
            }
            full
        })
        .collect();
    Ok(WarpingModalResult {
        frequencies: eig.values.iter().map(|&l| crate::modal::frequency(l)).collect(),
        eigenvalues: eig.values,
        modes,
        residuals: eig.residuals,
        iterations: eig.iterations,
        shift: eig.shift,
        sturm: eig.sturm,
        unknowns: nf,
        rigid_free,
        dofs: lay.dofs,
    })
}
