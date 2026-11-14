//! Four-node shell (Shell4): flat facet with assumed transverse shear strain, resultants and surface stresses. Declared
//! by ADR 0291; filled by ADR 0432 (F-392, the package of ADR 0295).
//!
//! A Shell4 element joins four nodes with six unknowns each, `(u_x, u_y, u_z, theta_x, theta_y, theta_z)` in global axes
//! (small rotations). Its facet ([`facet`], ADR 0432 arithmetic 1): the normal `e_3` of the diagonals' cross product, the
//! mean plane through the centroid, the nodes projected on it (offsets `h_i` along `e_3`, refused above [`WARP_LIMIT`] of
//! the size), in-plane axes `e_1` along the first projected edge and `e_2 = e_3 x e_1`. In local axes: a bilinear membrane,
//! Reissner-Mindlin bending with bilinear `w, theta_x, theta_y` and the transverse shear strain of MITC4 (covariant
//! components tied at the edge mid-points, after Dvorkin and Bathe; ADR 0432 arithmetic 4), and a drilling penalty tying
//! `theta_z` to the in-plane rotation of the membrane field ([`DRILLING_FACTOR`], arithmetic 5), all by 2 x 2 Gauss. The
//! consistent mass carries the rotary inertia `rho t^3 / 12` on the three rotations (arithmetic 6). The global matrices are
//! `T^T A T` with the rigid offsets from the nodes to the projected points (arithmetic 7). A uniform pressure along `e_3`
//! ([`crate::model::Load::ShellPressure`]) has the consistent load [`pressure_load_vector`]; [`stresses`] gives the
//! resultants and surface stresses at the element centres.
//!
//! The solid-element routines of the [`Element`] trait do not apply: [`element`] is a placeholder that refuses naming
//! [`SOLID_ROUTINES`], so the solid stress recovery refuses a shell (ADR 0432 D7). Gravity and the centrifugal load take the
//! consistent load [`body_load_vector`] instead (ADR 0435 D2), a nodal body-force field that of [`body_load_vector_at`] and
//! a nodal temperature field the membrane thermal load [`thermal_load_vector`] (uniform through the thickness; ADR 0445),
//! whose thermal strain [`stresses`] subtracts. The assembly calls
//! [`model_element_stiffness`] and [`model_element_mass`] for Shell4 (D6).
//!
//! A block may choose a drilling membrane in place of the bilinear one ([`Shell4Membrane`], ADR 0470): four overlapping ANDES
//! optimal membrane triangles, whose edges are those of the ANDES Shell3 (ADR 0466), so that the two share edges. Opt-in
//! through [`crate::BlockProperty::shell4_with_membrane`]; a model whose Shell4 blocks keep the default gives the former
//! results bit for bit.

use crate::assemble::element_dofs;
use crate::element::{Element, ElementMatrix, Pending};
use crate::error::{FemError, Result};
use crate::mesh::{BlockProperty, ElementKind, SolidMesh};
use crate::model::{Material, SolidModel};
use rcfd_core::vec3::{cross, dot, norm, Vec3};

/// The work package that adds Shell4; `None` once it has landed (ADR 0432).
pub const PENDING: Option<&'static str> = None;

/// The package named when a path that does not take shells is given a shell model (ADR 0432 D3).
pub const FOLLOW_UP: &str = "a follow-up of ADR 0432";

/// The package named by the solid-element routines of Shell4 (ADR 0432 D7).
pub const SOLID_ROUTINES: &str =
    "a follow-up of ADR 0432 (the solid-element routines of Shell4; its matrices are in rcfd_fem::shell)";

/// A facet whose node offset from its mean plane exceeds this fraction of `sqrt(area)` is refused (ADR 0432 arithmetic 1).
pub const WARP_LIMIT: f64 = 0.05;

/// The factor `alpha` of the drilling penalty `alpha G t int (theta_z - omega)^2 dA` (ADR 0432 arithmetic 5).
pub const DRILLING_FACTOR: f64 = 1e-3;

/// The transverse shear factor of a homogeneous shell (ADR 0432 arithmetic 3).
pub const SHEAR_FACTOR: f64 = 5.0 / 6.0;

static ELEMENT: Pending = Pending::new(ElementKind::Shell4, SOLID_ROUTINES);

/// The element of the kind for the solid-element routines: a placeholder that refuses (ADR 0432 D7).
pub fn element() -> &'static dyn Element {
    &ELEMENT
}

/// Reference coordinates of the nodes (none: a shell has no isoparametric volume map).
pub fn reference_nodes() -> &'static [[f64; 3]] {
    &[]
}

/// The local faces (none: no surface set names a shell face).
pub fn faces() -> &'static [&'static [usize]] {
    &[]
}

/// Derivatives of the shape functions with respect to the reference coordinates (none).
pub fn shape_gradients(_xi: [f64; 3]) -> Vec<[f64; 3]> {
    Vec::new()
}

/// The treatment of the transverse shear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TransverseShear {
    /// The MITC4 assumed strain (the element).
    #[default]
    AssumedStrain,
    /// The displacement-based shear by the full 2 x 2 rule (the locking control of ADR 0432 check SS3).
    FullIntegration,
}

/// The membrane of a Shell4 element (ADR 0470): the default bilinear membrane with the drilling penalty, or the ANDES
/// membrane of four overlapping ANDES optimal triangles. Chosen per block by [`crate::BlockProperty::shell4_with_membrane`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Shell4Membrane {
    /// The bilinear membrane with the drilling penalty tying `theta_z` to the membrane rotation (ADR 0432).
    #[default]
    Bilinear,
    /// The mean of the two diagonal splits into ANDES optimal membrane triangles (the Shell3 membrane of ADR 0466 arithmetic
    /// 4), no drilling penalty: every edge carries the ANDES edge field, so the element shares edges with the ANDES Shell3
    /// (ADR 0470 arithmetic 1).
    AndesOpt,
}

/// The two diagonal splits of a Shell4 facet into counter-clockwise triangles (ADR 0470 arithmetic 1).
const ANDES_TRIANGLES: [[usize; 3]; 4] = [[0, 1, 2], [0, 2, 3], [0, 1, 3], [1, 2, 3]];

/// The local membrane unknowns `(u, v, theta_z)` of a node.
const MEMBRANE_UNKNOWNS: [usize; 3] = [0, 1, 5];

/// Options of the element matrices. The default is the element; the other values exist for the controls of ADR 0432 and
/// are not reachable from a model, except the membrane (ADR 0470).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ShellOptions {
    /// The transverse shear.
    pub shear: TransverseShear,
    /// The drilling factor `alpha`; the ANDES membrane does not use it.
    pub drilling: f64,
    /// The membrane (ADR 0470).
    pub membrane: Shell4Membrane,
}

impl Default for ShellOptions {
    fn default() -> ShellOptions {
        ShellOptions {
            shear: TransverseShear::AssumedStrain,
            drilling: DRILLING_FACTOR,
            membrane: Shell4Membrane::Bilinear,
        }
    }
}

impl ShellOptions {
    /// The same options with a transverse shear treatment.
    pub fn shear(mut self, shear: TransverseShear) -> ShellOptions {
        self.shear = shear;
        self
    }
    /// The same options with a drilling factor.
    pub fn drilling(mut self, drilling: f64) -> ShellOptions {
        self.drilling = drilling;
        self
    }
    /// The same options with a membrane (ADR 0470).
    pub fn membrane(mut self, membrane: Shell4Membrane) -> ShellOptions {
        self.membrane = membrane;
        self
    }
}

/// The facet of a shell element (ADR 0432 arithmetic 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Facet {
    /// `[e_1, e_2, e_3]`, the rows of the rotation to local axes.
    pub axes: [Vec3; 3],
    /// The centroid of the nodes.
    pub centre: Vec3,
    /// Local coordinates `(X, Y)` of the projected nodes.
    pub local: [[f64; 2]; 4],
    /// Offsets `h_i` of the nodes from the mean plane along `e_3`.
    pub offsets: [f64; 4],
    /// `max |h_i| / sqrt(area)`.
    pub warp: f64,
    /// `|d_1 x d_2| / 2`.
    pub area: f64,
}

const CORNERS: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
const G: f64 = 0.577_350_269_189_625_8;
const GAUSS: [f64; 2] = [-G, G];

fn shape(xi: f64, eta: f64) -> ([f64; 4], [f64; 4], [f64; 4]) {
    let mut n = [0.0; 4];
    let mut dx = [0.0; 4];
    let mut de = [0.0; 4];
    for (i, c) in CORNERS.iter().enumerate() {
        n[i] = 0.25 * (1.0 + c[0] * xi) * (1.0 + c[1] * eta);
        dx[i] = 0.25 * c[0] * (1.0 + c[1] * eta);
        de[i] = 0.25 * c[1] * (1.0 + c[0] * xi);
    }
    (n, dx, de)
}

/// The values at a point of the reference square: shape functions, their Cartesian derivatives, the Jacobian rows and its
/// determinant.
struct Point {
    n: [f64; 4],
    nx: [f64; 4],
    ny: [f64; 4],
    j: [[f64; 2]; 2],
    inv: [[f64; 2]; 2],
    det: f64,
}

fn point(local: &[[f64; 2]; 4], xi: f64, eta: f64) -> Point {
    let (n, dx, de) = shape(xi, eta);
    let mut j = [[0.0; 2]; 2];
    for i in 0..4 {
        j[0][0] += dx[i] * local[i][0];
        j[0][1] += dx[i] * local[i][1];
        j[1][0] += de[i] * local[i][0];
        j[1][1] += de[i] * local[i][1];
    }
    let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
    let inv = [[j[1][1] / det, -j[0][1] / det], [-j[1][0] / det, j[0][0] / det]];
    let mut nx = [0.0; 4];
    let mut ny = [0.0; 4];
    for i in 0..4 {
        nx[i] = inv[0][0] * dx[i] + inv[0][1] * de[i];
        ny[i] = inv[1][0] * dx[i] + inv[1][1] * de[i];
    }
    Point { n, nx, ny, j, inv, det }
}

/// The facet of four node coordinates (ADR 0432 arithmetic 1). Refuses other than four coordinates, a degenerate facet
/// (`InvalidValue { key: "geometry" }`), a warp ratio above [`WARP_LIMIT`] (`InvalidValue { key: "warp" }`) and a
/// non-convex projected quadrilateral ([`FemError::InvertedElement`] with element 0; the model checks name the element).
pub fn facet(coords: &[Vec3]) -> Result<Facet> {
    if coords.len() != 4 {
        return Err(FemError::FieldLength {
            key: "coordinates",
            expected: 4,
            found: coords.len(),
        });
    }
    let mut centre = [0.0; 3];
    for x in coords {
        for k in 0..3 {
            centre[k] += 0.25 * x[k];
        }
    }
    let d1 = sub(coords[2], coords[0]);
    let d2 = sub(coords[3], coords[1]);
    let n = cross(d1, d2);
    let nn = norm(n);
    if !(nn > 0.0 && nn.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "geometry",
            reason: format!("the facet {coords:?} is degenerate (|d_1 x d_2| = {nn:e})"),
        });
    }
    let area = 0.5 * nn;
    let e3 = [n[0] / nn, n[1] / nn, n[2] / nn];
    let mut offsets = [0.0; 4];
    let mut p = [[0.0; 3]; 4];
    for i in 0..4 {
        offsets[i] = dot(sub(coords[i], centre), e3);
        for k in 0..3 {
            p[i][k] = coords[i][k] - offsets[i] * e3[k];
        }
    }
    let warp = offsets.iter().fold(0.0f64, |m, h| m.max(h.abs())) / area.sqrt();
    if warp.is_nan() || warp > WARP_LIMIT {
        return Err(FemError::InvalidValue {
            key: "warp",
            reason: format!(
                "the facet is warped: node offset / sqrt(area) = {warp:.4e} above the limit {WARP_LIMIT} (ADR 0432)"
            ),
        });
    }
    let a = sub(p[1], p[0]);
    let la = norm(a);
    if !(la > 0.0 && la.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "geometry",
            reason: format!("the first edge of the facet {coords:?} has length {la:e}"),
        });
    }
    let e1 = [a[0] / la, a[1] / la, a[2] / la];
    let e2 = cross(e3, e1);
    let mut local = [[0.0; 2]; 4];
    for i in 0..4 {
        let d = sub(p[i], centre);
        local[i] = [dot(d, e1), dot(d, e2)];
    }
    for (i, c) in CORNERS.iter().enumerate() {
        let det = point(&local, c[0], c[1]).det;
        if det.is_nan() || det <= 0.0 {
            return Err(FemError::InvertedElement {
                element: 0,
                local_node: i,
                jacobian: det,
            });
        }
    }
    Ok(Facet {
        axes: [e1, e2, e3],
        centre,
        local,
        offsets,
        warp,
        area,
    })
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `E`, `nu`, `G` of the material.
fn moduli(material: &Material) -> (f64, f64, f64) {
    let e = material.youngs_modulus();
    let nu = material.poisson_ratio();
    (e, nu, e / (2.0 * (1.0 + nu)))
}

/// The plane-stress matrix `E t / (1 - nu^2) [[1, nu, 0], [nu, 1, 0], [0, 0, (1 - nu) / 2]]` times `s`.
fn plane_matrix(e: f64, nu: f64, s: f64) -> [[f64; 3]; 3] {
    let c = e * s / (1.0 - nu * nu);
    [[c, nu * c, 0.0], [nu * c, c, 0.0], [0.0, 0.0, 0.5 * (1.0 - nu) * c]]
}

/// `K += w B^T D B` for a 3-row `B` over 24 unknowns.
fn add_btdb3(k: &mut [f64], b: &[[f64; 24]; 3], d: &[[f64; 3]; 3], w: f64) {
    let mut db = [[0.0; 24]; 3];
    for r in 0..3 {
        for c in 0..24 {
            db[r][c] = d[r][0] * b[0][c] + d[r][1] * b[1][c] + d[r][2] * b[2][c];
        }
    }
    for i in 0..24 {
        for j in 0..24 {
            k[i * 24 + j] += w * (b[0][i] * db[0][j] + b[1][i] * db[1][j] + b[2][i] * db[2][j]);
        }
    }
}

/// The covariant shear component `gamma_xi` (`which = 0`) or `gamma_eta` (`which = 1`) at a point, as a row over the 24
/// local unknowns: `w_xi + beta . J_xi` with `beta_x = theta_y`, `beta_y = -theta_x` (ADR 0432 arithmetic 3, 4).
fn covariant_row(local: &[[f64; 2]; 4], xi: f64, eta: f64, which: usize) -> [f64; 24] {
    let (n, dx, de) = shape(xi, eta);
    let p = point(local, xi, eta);
    let d = if which == 0 { dx } else { de };
    let jv = p.j[which];
    let mut row = [0.0; 24];
    for i in 0..4 {
        row[6 * i + 2] += d[i];
        row[6 * i + 4] += n[i] * jv[0];
        row[6 * i + 3] -= n[i] * jv[1];
    }
    row
}

/// The transverse shear rows `(gamma_xz, gamma_yz)` at a Gauss point.
fn shear_rows(p: &Point, xi: f64, eta: f64, shear: TransverseShear, tying: &[[f64; 24]; 4]) -> [[f64; 24]; 2] {
    let mut out = [[0.0; 24]; 2];
    match shear {
        TransverseShear::AssumedStrain => {
            let [a, c, d, b] = tying;
            let mut cov = [[0.0; 24]; 2];
            for q in 0..24 {
                cov[0][q] = 0.5 * (1.0 - eta) * a[q] + 0.5 * (1.0 + eta) * c[q];
                cov[1][q] = 0.5 * (1.0 - xi) * d[q] + 0.5 * (1.0 + xi) * b[q];
            }
            for q in 0..24 {
                out[0][q] = p.inv[0][0] * cov[0][q] + p.inv[0][1] * cov[1][q];
                out[1][q] = p.inv[1][0] * cov[0][q] + p.inv[1][1] * cov[1][q];
            }
        }
        TransverseShear::FullIntegration => {
            for i in 0..4 {
                out[0][6 * i + 2] = p.nx[i];
                out[0][6 * i + 4] = p.n[i];
                out[1][6 * i + 2] = p.ny[i];
                out[1][6 * i + 3] = -p.n[i];
            }
        }
    }
    out
}

/// The four tying rows of MITC4: `gamma_xi` at A `(0, -1)` and C `(0, 1)`, `gamma_eta` at D `(-1, 0)` and B `(1, 0)`.
fn tying_rows(local: &[[f64; 2]; 4]) -> [[f64; 24]; 4] {
    [
        covariant_row(local, 0.0, -1.0, 0),
        covariant_row(local, 0.0, 1.0, 0),
        covariant_row(local, -1.0, 0.0, 1),
        covariant_row(local, 1.0, 0.0, 1),
    ]
}

fn membrane_rows(p: &Point) -> [[f64; 24]; 3] {
    let mut b = [[0.0; 24]; 3];
    for i in 0..4 {
        b[0][6 * i] = p.nx[i];
        b[1][6 * i + 1] = p.ny[i];
        b[2][6 * i] = p.ny[i];
        b[2][6 * i + 1] = p.nx[i];
    }
    b
}

fn bending_rows(p: &Point) -> [[f64; 24]; 3] {
    let mut b = [[0.0; 24]; 3];
    for i in 0..4 {
        b[0][6 * i + 4] = p.nx[i];
        b[1][6 * i + 3] = -p.ny[i];
        b[2][6 * i + 4] = p.ny[i];
        b[2][6 * i + 3] = -p.nx[i];
    }
    b
}

/// The 24 x 24 stiffness in local axes at the projected nodes, unknowns `(u, v, w, theta_x, theta_y, theta_z)` node by node
/// (ADR 0432 arithmetic 2 to 5).
pub fn local_stiffness(f: &Facet, thickness: f64, material: &Material, options: &ShellOptions) -> ElementMatrix {
    local_stiffness_section(f, &homogeneous_section(material, thickness), options)
}

/// The transverse shear stiffness of a section (ADR 0440 D6): `Q = H gamma`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum ShearStiffness {
    /// `H = h I` (a homogeneous or transversely isotropic section).
    Isotropic(f64),
    /// A general symmetric `H` (a laminate).
    Matrix([[f64; 2]; 2]),
}

/// The stiffness of a shell section (ADR 0310 section 2.4 point 4, ADR 0440 D6): the resultants `N = A eps + B kappa`, `M
/// = B eps + D kappa`, `Q = H gamma`, and the pair `(G, t)` of the drilling penalty `alpha G t`. The one function that
/// gives it for a block is [`section_of`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct SectionStiffness {
    /// `A`, N/m.
    pub membrane: [[f64; 3]; 3],
    /// `B`, N; `None` for a section symmetric about its mid-plane.
    pub coupling: Option<[[f64; 3]; 3]>,
    /// `D`, N m.
    pub bending: [[f64; 3]; 3],
    /// `H`, N/m.
    pub shear: ShearStiffness,
    /// `(G, t)` of the drilling penalty.
    pub drilling: [f64; 2],
}

impl SectionStiffness {
    /// A section from its parts.
    pub fn new(
        membrane: [[f64; 3]; 3],
        coupling: Option<[[f64; 3]; 3]>,
        bending: [[f64; 3]; 3],
        shear: ShearStiffness,
        drilling: [f64; 2],
    ) -> SectionStiffness {
        SectionStiffness {
            membrane,
            coupling,
            bending,
            shear,
            drilling,
        }
    }
}

/// The section of a homogeneous shell of thickness `t` (ADR 0432 arithmetic 2 to 5): `A = E t / (1 - nu^2) [...]`, `D` the
/// same with `t^3 / 12`, no coupling, `H = kappa G t`, drilling `(G, t)`.
pub fn homogeneous_section(material: &Material, thickness: f64) -> SectionStiffness {
    let (e, nu, g) = moduli(material);
    let t = thickness;
    SectionStiffness {
        membrane: plane_matrix(e, nu, t),
        coupling: None,
        bending: plane_matrix(e, nu, t * t * t / 12.0),
        shear: ShearStiffness::Isotropic(SHEAR_FACTOR * g * t),
        drilling: [g, t],
    }
}

/// The section-stiffness hook (ADR 0440 D6): the section of a shell block from its property and material. `Shell {
/// thickness }` gives [`homogeneous_section`]; `Layup` gives [`crate::laminate::section_stiffness`].
pub fn section_of(property: &BlockProperty, material: &Material) -> Result<SectionStiffness> {
    match property {
        BlockProperty::Shell { thickness, .. } => Ok(homogeneous_section(material, *thickness)),
        BlockProperty::Layup { plies } => crate::laminate::section_stiffness(plies),
        other => Err(FemError::InvalidValue {
            key: "property",
            reason: format!("a {} property has no shell section", other.name()),
        }),
    }
}

/// `K += w (B1^T C B2 + B2^T C^T B1)` for 3-row `B1`, `B2` over 24 unknowns (the membrane-bending coupling).
fn add_coupling(k: &mut [f64], b1: &[[f64; 24]; 3], c: &[[f64; 3]; 3], b2: &[[f64; 24]; 3], w: f64) {
    let mut cb2 = [[0.0; 24]; 3];
    let mut ctb1 = [[0.0; 24]; 3];
    for r in 0..3 {
        for q in 0..24 {
            cb2[r][q] = c[r][0] * b2[0][q] + c[r][1] * b2[1][q] + c[r][2] * b2[2][q];
            ctb1[r][q] = c[0][r] * b1[0][q] + c[1][r] * b1[1][q] + c[2][r] * b1[2][q];
        }
    }
    for i in 0..24 {
        for j in 0..24 {
            let mut v = 0.0;
            for r in 0..3 {
                v += b1[r][i] * cb2[r][j] + b2[r][i] * ctb1[r][j];
            }
            k[i * 24 + j] += w * v;
        }
    }
}

/// The 24 x 24 local stiffness for a given section (ADR 0440 D6). With no coupling and an isotropic shear it is the
/// element of ADR 0432 with the operations of [`local_stiffness`].
pub fn local_stiffness_section(f: &Facet, section: &SectionStiffness, options: &ShellOptions) -> ElementMatrix {
    let dm = section.membrane;
    let db = section.bending;
    // ADR 0470 D1: the ANDES membrane takes neither the bilinear membrane nor the drilling penalty.
    let andes = options.membrane == Shell4Membrane::AndesOpt;
    let dr = if andes {
        0.0
    } else {
        options.drilling * section.drilling[0] * section.drilling[1]
    };
    let tying = tying_rows(&f.local);
    let mut k = ElementMatrix::zeros(24);
    let v = k.values_mut();
    for &xi in &GAUSS {
        for &eta in &GAUSS {
            let p = point(&f.local, xi, eta);
            let w = p.det;
            let bm = membrane_rows(&p);
            let bb = bending_rows(&p);
            if !andes {
                add_btdb3(v, &bm, &dm, w);
            }
            add_btdb3(v, &bb, &db, w);
            if let Some(c) = &section.coupling {
                add_coupling(v, &bm, c, &bb, w);
            }
            let s = shear_rows(&p, xi, eta, options.shear, &tying);
            let mut d = [0.0; 24];
            for i in 0..4 {
                d[6 * i + 5] = p.n[i];
                d[6 * i + 1] -= 0.5 * p.nx[i];
                d[6 * i] += 0.5 * p.ny[i];
            }
            match section.shear {
                ShearStiffness::Isotropic(ds) => {
                    for i in 0..24 {
                        for j in 0..24 {
                            v[i * 24 + j] += w * (ds * (s[0][i] * s[0][j] + s[1][i] * s[1][j]) + dr * d[i] * d[j]);
                        }
                    }
                }
                ShearStiffness::Matrix(h) => {
                    for i in 0..24 {
                        for j in 0..24 {
                            let q = h[0][0] * s[0][i] * s[0][j]
                                + h[0][1] * s[0][i] * s[1][j]
                                + h[1][0] * s[1][i] * s[0][j]
                                + h[1][1] * s[1][i] * s[1][j];
                            v[i * 24 + j] += w * (q + dr * d[i] * d[j]);
                        }
                    }
                }
            }
        }
    }
    if andes {
        add_andes_membrane(v, &f.local, &dm);
    }
    symmetrise(&mut k);
    k
}

/// `K += (1/2) sum_T K_T` over the four triangles of the two diagonal splits, `K_T` the ANDES optimal membrane on the
/// unknowns `(u, v, theta_z)` of its nodes (ADR 0470 arithmetic 1).
fn add_andes_membrane(k: &mut [f64], local: &[[f64; 2]; 4], am: &[[f64; 3]; 3]) {
    for tri in ANDES_TRIANGLES {
        let (kt, _, _) = crate::shell3::andes_membrane(&[local[tri[0]], local[tri[1]], local[tri[2]]], am);
        for a in 0..3 {
            for c in 0..3 {
                let p = 6 * tri[a] + MEMBRANE_UNKNOWNS[c];
                for b in 0..3 {
                    for d in 0..3 {
                        k[p * 24 + 6 * tri[b] + MEMBRANE_UNKNOWNS[d]] += 0.5 * kt[(3 * a + c) * 9 + 3 * b + d];
                    }
                }
            }
        }
    }
}

/// The rows over the 24 local unknowns of the mean basic strain of the ANDES membrane, `sum_T (1/2) A_T Bbar_T / sum_T (1/2)
/// A_T` (ADR 0470 arithmetic 8).
fn andes_centre_rows(local: &[[f64; 2]; 4], am: &[[f64; 3]; 3]) -> [[f64; 24]; 3] {
    let mut rows = [[0.0; 24]; 3];
    let mut area = 0.0;
    for tri in ANDES_TRIANGLES {
        let (_, b, a) = crate::shell3::andes_membrane(&[local[tri[0]], local[tri[1]], local[tri[2]]], am);
        area += 0.5 * a;
        for (r, row) in rows.iter_mut().enumerate() {
            for n in 0..3 {
                for c in 0..3 {
                    row[6 * tri[n] + MEMBRANE_UNKNOWNS[c]] += 0.5 * a * b[r][3 * n + c];
                }
            }
        }
    }
    for row in rows.iter_mut() {
        for x in row.iter_mut() {
            *x /= area;
        }
    }
    rows
}

/// The 24 x 24 consistent mass in local axes (ADR 0432 arithmetic 6).
pub fn local_mass(f: &Facet, thickness: f64, density: f64) -> ElementMatrix {
    let t = thickness;
    let (mt, mr) = (density * t, density * t * t * t / 12.0);
    let mut m = ElementMatrix::zeros(24);
    let v = m.values_mut();
    for &xi in &GAUSS {
        for &eta in &GAUSS {
            let p = point(&f.local, xi, eta);
            for i in 0..4 {
                for j in 0..4 {
                    let s = p.det * p.n[i] * p.n[j];
                    for c in 0..3 {
                        v[(6 * i + c) * 24 + 6 * j + c] += mt * s;
                        v[(6 * i + 3 + c) * 24 + 6 * j + 3 + c] += mr * s;
                    }
                }
            }
        }
    }
    symmetrise(&mut m);
    m
}

/// The 24 x 24 consistent mass in local axes of a layup with the mass moments `m0 = int rho dz`, `m1 = int rho z dz`, `m2 = int
/// rho z^2 dz` (ADR 0496 D5): `m0` on the translations, `m2` on the rotations, and the coupling of `u = u0 + z beta` (`beta_x =
/// theta_y`, `beta_y = -theta_x`): `+m1` between `u_x` and `theta_y`, `-m1` between `u_y` and `theta_x`.
pub fn local_mass_layup(f: &Facet, m0: f64, m1: f64, m2: f64) -> ElementMatrix {
    let mut m = ElementMatrix::zeros(24);
    let v = m.values_mut();
    for &xi in &GAUSS {
        for &eta in &GAUSS {
            let p = point(&f.local, xi, eta);
            for i in 0..4 {
                for j in 0..4 {
                    let s = p.det * p.n[i] * p.n[j];
                    for c in 0..3 {
                        v[(6 * i + c) * 24 + 6 * j + c] += m0 * s;
                        v[(6 * i + 3 + c) * 24 + 6 * j + 3 + c] += m2 * s;
                    }
                    v[(6 * i) * 24 + 6 * j + 4] += m1 * s;
                    v[(6 * i + 4) * 24 + 6 * j] += m1 * s;
                    v[(6 * i + 1) * 24 + 6 * j + 3] -= m1 * s;
                    v[(6 * i + 3) * 24 + 6 * j + 1] -= m1 * s;
                }
            }
        }
    }
    symmetrise(&mut m);
    m
}

/// The 24 x 24 geometric stiffness in local axes of the membrane resultant `n` (N/m, the facet's in-plane axes; ADR 0473 D2):
/// `K[(a, i), (b, i)] = int grad N_a . n grad N_b dA` for `i = u, v, w`, the bilinear functions, 2 x 2 Gauss on the projected
/// facet; the rotations take no term.
pub fn local_geometric_stiffness(f: &Facet, n: [[f64; 2]; 2]) -> ElementMatrix {
    let mut k = ElementMatrix::zeros(24);
    let v = k.values_mut();
    for &xi in &GAUSS {
        for &eta in &GAUSS {
            let p = point(&f.local, xi, eta);
            for a in 0..4 {
                let ga = [p.nx[a], p.ny[a]];
                for b in 0..4 {
                    let gb = [p.nx[b], p.ny[b]];
                    let s = p.det
                        * (ga[0] * (n[0][0] * gb[0] + n[0][1] * gb[1]) + ga[1] * (n[1][0] * gb[0] + n[1][1] * gb[1]));
                    for c in 0..3 {
                        v[(6 * a + c) * 24 + 6 * b + c] += s;
                    }
                }
            }
        }
    }
    symmetrise(&mut k);
    k
}

/// The global geometric stiffness of a shell element for the stress `stress` (global Voigt, tensor shear) of its one
/// pre-stress point: the membrane resultant `t (Lambda S Lambda^T)_{alpha beta}` (ADR 0473 D2, D4). A non-finite stress is
/// refused (key `"stress"`).
pub fn element_geometric_stiffness(coords: &[Vec3], thickness: f64, stress: &[f64; 6]) -> Result<ElementMatrix> {
    let f = facet(coords)?;
    let n = crate::geometric_struct::membrane_resultant(&f.axes, thickness, stress)?;
    Ok(to_global(&local_geometric_stiffness(&f, n), &transformation(&f)))
}

/// The 24 x 24 `t^2/12` rotation terms of the geometric stiffness in local axes of the membrane resultant `n` (ADR 0508 D2):
/// `K[(a, c), (b, c)] = (t^2/12) int grad N_a . n grad N_b dA` for `c = theta_x, theta_y`, the integral of
/// [`local_geometric_stiffness`]; the drilling rotation takes none.
pub fn local_rotation_geometric_stiffness(f: &Facet, thickness: f64, n: [[f64; 2]; 2]) -> ElementMatrix {
    let c2 = thickness * thickness / 12.0;
    let mut k = ElementMatrix::zeros(24);
    let v = k.values_mut();
    for &xi in &GAUSS {
        for &eta in &GAUSS {
            let p = point(&f.local, xi, eta);
            for a in 0..4 {
                let ga = [p.nx[a], p.ny[a]];
                for b in 0..4 {
                    let gb = [p.nx[b], p.ny[b]];
                    let s = c2
                        * p.det
                        * (ga[0] * (n[0][0] * gb[0] + n[0][1] * gb[1]) + ga[1] * (n[1][0] * gb[0] + n[1][1] * gb[1]));
                    for c in 3..5 {
                        v[(6 * a + c) * 24 + 6 * b + c] += s;
                    }
                }
            }
        }
    }
    symmetrise(&mut k);
    k
}

/// The global `t^2/12` rotation terms of a shell element for the stress of its one pre-stress point (ADR 0508 D2); the matrix
/// that [`element_geometric_stiffness`] gains with the option.
pub fn element_rotation_geometric_stiffness(
    coords: &[Vec3],
    thickness: f64,
    stress: &[f64; 6],
) -> Result<ElementMatrix> {
    let f = facet(coords)?;
    let n = crate::geometric_struct::membrane_resultant(&f.axes, thickness, stress)?;
    Ok(to_global(
        &local_rotation_geometric_stiffness(&f, thickness, n),
        &transformation(&f),
    ))
}

/// The 3 x 3 rotational factor `R` of the spin softening of a shell in local axes for the unit axis `a` in local components:
/// `theta^T R theta = |theta x e_3|^2 - (a . (theta x e_3))^2` (ADR 0473 D3).
pub(crate) fn rotary_softening_factor(a: Vec3) -> [[f64; 3]; 3] {
    [
        [1.0 - a[1] * a[1], a[0] * a[1], 0.0],
        [a[0] * a[1], 1.0 - a[0] * a[0], 0.0],
        [0.0, 0.0, 0.0],
    ]
}

/// The 24 x 24 spin-softening mass in local axes for the unit axis `a` in local components (ADR 0473 D3): the kinetic energy of
/// the motion perpendicular to the axis of `u + zeta theta x e_3`, `m_ab (delta_ij - a_i a_j)` on the translations and `m^r_ab
/// R` on the rotations with the scalars of [`local_mass`]; the drilling rotation takes no term.
pub fn local_spin_softening_mass(f: &Facet, thickness: f64, density: f64, a: Vec3) -> ElementMatrix {
    let t = thickness;
    let (mt, mr) = (density * t, density * t * t * t / 12.0);
    let r = rotary_softening_factor(a);
    let mut m = ElementMatrix::zeros(24);
    let v = m.values_mut();
    for &xi in &GAUSS {
        for &eta in &GAUSS {
            let p = point(&f.local, xi, eta);
            for i in 0..4 {
                for j in 0..4 {
                    let s = p.det * p.n[i] * p.n[j];
                    for c in 0..3 {
                        for d in 0..3 {
                            let pt = if c == d { 1.0 } else { 0.0 } - a[c] * a[d];
                            v[(6 * i + c) * 24 + 6 * j + d] += mt * s * pt;
                            v[(6 * i + 3 + c) * 24 + 6 * j + 3 + d] += mr * s * r[c][d];
                        }
                    }
                }
            }
        }
    }
    symmetrise(&mut m);
    m
}

/// The global spin-softening mass of a shell element for a rotation about the unit `axis` (ADR 0473 D3).
pub fn element_spin_softening_mass(coords: &[Vec3], thickness: f64, density: f64, axis: Vec3) -> Result<ElementMatrix> {
    let f = facet(coords)?;
    let al = [dot(f.axes[0], axis), dot(f.axes[1], axis), dot(f.axes[2], axis)];
    Ok(to_global(
        &local_spin_softening_mass(&f, thickness, density, al),
        &transformation(&f),
    ))
}

/// The 24 x 24 spin-softening mass in local axes of a layup with the mass moments `m0`, `m1`, `m2` (ADR 0496 D5) for the unit
/// axis `a` in local components (ADR 0502 D3): the kinetic energy of the motion perpendicular to the axis of `u + z W theta`,
/// `W theta = theta x e_3`: `m0 P` on the translations, `m2 R` on the rotations ([`rotary_softening_factor`]) and `m1 P W`
/// between the translations and the rotations (and its transpose); the drilling rotation takes no term.
pub fn local_spin_softening_mass_layup(f: &Facet, m0: f64, m1: f64, m2: f64, a: Vec3) -> ElementMatrix {
    let r = rotary_softening_factor(a);
    let mut pm = [[0.0; 3]; 3];
    for (c, row) in pm.iter_mut().enumerate() {
        for (d, v) in row.iter_mut().enumerate() {
            *v = if c == d { 1.0 } else { 0.0 } - a[c] * a[d];
        }
    }
    // (P W)[c] = (-P[c][1], P[c][0], 0).
    let pw: [[f64; 3]; 3] = [
        [-pm[0][1], pm[0][0], 0.0],
        [-pm[1][1], pm[1][0], 0.0],
        [-pm[2][1], pm[2][0], 0.0],
    ];
    let mut m = ElementMatrix::zeros(24);
    let v = m.values_mut();
    for &xi in &GAUSS {
        for &eta in &GAUSS {
            let p = point(&f.local, xi, eta);
            for i in 0..4 {
                for j in 0..4 {
                    let s = p.det * p.n[i] * p.n[j];
                    for c in 0..3 {
                        for d in 0..3 {
                            v[(6 * i + c) * 24 + 6 * j + d] += m0 * s * pm[c][d];
                            v[(6 * i + 3 + c) * 24 + 6 * j + 3 + d] += m2 * s * r[c][d];
                            v[(6 * i + c) * 24 + 6 * j + 3 + d] += m1 * s * pw[c][d];
                            v[(6 * i + 3 + c) * 24 + 6 * j + d] += m1 * s * pw[d][c];
                        }
                    }
                }
            }
        }
    }
    symmetrise(&mut m);
    m
}

/// The global spin-softening mass of a layup shell element for a rotation about the unit `axis` (ADR 0502 D3).
pub fn element_spin_softening_mass_layup(
    coords: &[Vec3],
    m0: f64,
    m1: f64,
    m2: f64,
    axis: Vec3,
) -> Result<ElementMatrix> {
    let f = facet(coords)?;
    let al = [dot(f.axes[0], axis), dot(f.axes[1], axis), dot(f.axes[2], axis)];
    Ok(to_global(
        &local_spin_softening_mass_layup(&f, m0, m1, m2, al),
        &transformation(&f),
    ))
}

/// The spin-softening mass of a shell element of a model (ADR 0473 D3, ADR 0502 D3): a Shell4 element of a Layup block takes,
/// for one isotropic ply, [`element_spin_softening_mass`] of the ply's thickness and density (the call of its `Shell` twin)
/// and otherwise [`element_spin_softening_mass_layup`] of the laminate's mass moments; any other element
/// [`element_spin_softening_mass`] of the block's thickness and `material`'s density as before.
pub(crate) fn model_spin_softening_mass(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
    axis: Vec3,
) -> Result<ElementMatrix> {
    if let Some(BlockProperty::Layup { plies }) = mesh.blocks()[mesh.block_of(element)].property() {
        if mesh.element_kind(element) == ElementKind::Shell4 {
            if let Some((ply_material, t)) = crate::laminate::single_isotropic(plies) {
                return element_spin_softening_mass(coords, t, ply_material.density(), axis);
            }
            let l = crate::laminate::laminate(plies)?;
            return element_spin_softening_mass_layup(coords, l.mass[0], l.mass[1], l.mass[2], axis);
        }
    }
    let t = thickness_or_error(mesh, element)?;
    element_spin_softening_mass(coords, t, material.density(), axis)
}

fn symmetrise(a: &mut ElementMatrix) {
    let n = a.size();
    let v = a.values_mut();
    for i in 0..n {
        for j in 0..i {
            let s = 0.5 * (v[i * n + j] + v[j * n + i]);
            v[i * n + j] = s;
            v[j * n + i] = s;
        }
    }
}

/// The transformation `T` (24 x 24, row-major) from the global unknowns at the nodes to the local unknowns at the
/// projected points: per node `[[Lambda, -Lambda [r]_x], [0, Lambda]]`, `r = -h e_3` (ADR 0432 arithmetic 7).
pub fn transformation(f: &Facet) -> Vec<f64> {
    let mut t = vec![0.0; 24 * 24];
    let l = f.axes;
    let e3 = l[2];
    for i in 0..4 {
        let r = [-f.offsets[i] * e3[0], -f.offsets[i] * e3[1], -f.offsets[i] * e3[2]];
        // [r]_x
        let rx = [[0.0, -r[2], r[1]], [r[2], 0.0, -r[0]], [-r[1], r[0], 0.0]];
        for a in 0..3 {
            for b in 0..3 {
                t[(6 * i + a) * 24 + 6 * i + b] = l[a][b];
                t[(6 * i + 3 + a) * 24 + 6 * i + 3 + b] = l[a][b];
                let s: f64 = (0..3).map(|k| l[a][k] * rx[k][b]).sum();
                t[(6 * i + a) * 24 + 6 * i + 3 + b] = -s;
            }
        }
    }
    t
}

/// `T^T A T`, exactly symmetric.
fn to_global(a: &ElementMatrix, t: &[f64]) -> ElementMatrix {
    let av = a.values();
    let mut at = vec![0.0; 24 * 24];
    for i in 0..24 {
        for j in 0..24 {
            let mut s = 0.0;
            for k in 0..24 {
                s += av[i * 24 + k] * t[k * 24 + j];
            }
            at[i * 24 + j] = s;
        }
    }
    let mut out = ElementMatrix::zeros(24);
    let ov = out.values_mut();
    for i in 0..24 {
        for j in 0..24 {
            let mut s = 0.0;
            for k in 0..24 {
                s += t[k * 24 + i] * at[k * 24 + j];
            }
            ov[i * 24 + j] = s;
        }
    }
    symmetrise(&mut out);
    out
}

/// The global stiffness of a shell element from its node coordinates, thickness and material (the element: default
/// options).
pub fn element_stiffness(coords: &[Vec3], thickness: f64, material: &Material) -> Result<ElementMatrix> {
    element_stiffness_with(coords, thickness, material, &ShellOptions::default())
}

/// The global stiffness with given options (the controls of ADR 0432).
pub fn element_stiffness_with(
    coords: &[Vec3],
    thickness: f64,
    material: &Material,
    options: &ShellOptions,
) -> Result<ElementMatrix> {
    let f = facet(coords)?;
    Ok(to_global(
        &local_stiffness(&f, thickness, material, options),
        &transformation(&f),
    ))
}

/// The global stiffness for a given section (ADR 0440 D6).
pub fn element_stiffness_section(
    coords: &[Vec3],
    section: &SectionStiffness,
    options: &ShellOptions,
) -> Result<ElementMatrix> {
    let f = facet(coords)?;
    Ok(to_global(
        &local_stiffness_section(&f, section, options),
        &transformation(&f),
    ))
}

/// The section of a shell element of a model, from its block's property through [`section_of`].
pub(crate) fn model_section(mesh: &SolidMesh, element: usize, material: &Material) -> Result<SectionStiffness> {
    let property = mesh.blocks()[mesh.block_of(element)].property();
    match property {
        Some(p @ (BlockProperty::Shell { .. } | BlockProperty::Layup { .. })) => section_of(p, material),
        _ => thickness_or_error(mesh, element).map(|t| homogeneous_section(material, t)),
    }
}

/// The global consistent mass of a shell element.
pub fn element_mass(coords: &[Vec3], thickness: f64, material: &Material) -> Result<ElementMatrix> {
    let f = facet(coords)?;
    Ok(to_global(
        &local_mass(&f, thickness, material.density()),
        &transformation(&f),
    ))
}

/// The consistent nodal load (forces and moments, global axes, the element's unknown order) of a uniform pressure `p`
/// along the element normal `e_3` (ADR 0432 arithmetic 8).
pub fn pressure_load_vector(coords: &[Vec3], p: f64) -> Result<[f64; 24]> {
    let f = facet(coords)?;
    let mut fl = [0.0; 24];
    for &xi in &GAUSS {
        for &eta in &GAUSS {
            let q = point(&f.local, xi, eta);
            for i in 0..4 {
                fl[6 * i + 2] += p * q.n[i] * q.det;
            }
        }
    }
    let t = transformation(&f);
    let mut out = [0.0; 24];
    for (j, o) in out.iter_mut().enumerate() {
        *o = (0..24).map(|i| t[i * 24 + j] * fl[i]).sum();
    }
    Ok(out)
}

/// The consistent nodal load (forces and moments, global axes, the element's unknown order) of a body force per unit
/// volume `b(x)` (global components, N/m^3) over the thickness: `F_i = t int N_i b(x) dA` by the 2 x 2 rule at the points
/// `x = sum N_j p_j` of the projected facet, carried to the nodes by `T^T` (forces `F_i` and, on a warped facet, the moments
/// `r_i x F_i`; ADR 0435 D2).
pub fn body_load_vector<B>(coords: &[Vec3], thickness: f64, b: B) -> Result<[f64; 24]>
where
    B: Fn(Vec3) -> Result<Vec3>,
{
    body_load_vector_at(coords, thickness, |_, x| b(x))
}

/// [`body_load_vector`] with a body force given by the values of the facet's four shape functions at the point and the
/// point (ADR 0445 D1): a nodal field `b = sum_j N_j b_j` gives the consistent load `t int N_i b dA` of the interpolated field.
pub fn body_load_vector_at<B>(coords: &[Vec3], thickness: f64, b: B) -> Result<[f64; 24]>
where
    B: Fn(&[f64; 4], Vec3) -> Result<Vec3>,
{
    let f = facet(coords)?;
    let e3 = f.axes[2];
    let mut p = [[0.0; 3]; 4];
    for i in 0..4 {
        for k in 0..3 {
            p[i][k] = coords[i][k] - f.offsets[i] * e3[k];
        }
    }
    let mut force = [[0.0; 3]; 4];
    for &xi in &GAUSS {
        for &eta in &GAUSS {
            let q = point(&f.local, xi, eta);
            let mut x = [0.0; 3];
            for i in 0..4 {
                for k in 0..3 {
                    x[k] += q.n[i] * p[i][k];
                }
            }
            let bx = b(&q.n, x)?;
            for i in 0..4 {
                let s = thickness * q.n[i] * q.det;
                for k in 0..3 {
                    force[i][k] += s * bx[k];
                }
            }
        }
    }
    let mut fl = [0.0; 24];
    for i in 0..4 {
        for a in 0..3 {
            fl[6 * i + a] = dot(f.axes[a], force[i]);
        }
    }
    let t = transformation(&f);
    let mut out = [0.0; 24];
    for (j, o) in out.iter_mut().enumerate() {
        *o = (0..24).map(|i| t[i * 24 + j] * fl[i]).sum();
    }
    Ok(out)
}

/// The consistent nodal load (forces and moments, global axes, the element's unknown order) of a density-proportional body
/// force on a layup (ADR 0502 D1): `b(N, x)` the acceleration (global components, m/s^2) at the mid-surface point `x` of the
/// projected facet, `m0 = sum rho_k t_k` and `m1 = sum rho_k t_k zbar_k` the mass moments; per node the force `m0 int N_i b
/// dA` and the moment of the first mass moment `m1 int N_i (e_3 x b) dA` (local `(-b_2, b_1, 0)` on `(theta_x, theta_y,
/// theta_z)`), by the 2 x 2 rule, carried to the nodes by `T^T`.
pub fn layup_body_load_vector_at<B>(coords: &[Vec3], m0: f64, m1: f64, b: B) -> Result<[f64; 24]>
where
    B: Fn(&[f64; 4], Vec3) -> Result<Vec3>,
{
    let f = facet(coords)?;
    let e3 = f.axes[2];
    let mut p = [[0.0; 3]; 4];
    for i in 0..4 {
        for k in 0..3 {
            p[i][k] = coords[i][k] - f.offsets[i] * e3[k];
        }
    }
    let mut acc = [[0.0; 3]; 4];
    for &xi in &GAUSS {
        for &eta in &GAUSS {
            let q = point(&f.local, xi, eta);
            let mut x = [0.0; 3];
            for i in 0..4 {
                for k in 0..3 {
                    x[k] += q.n[i] * p[i][k];
                }
            }
            let bx = b(&q.n, x)?;
            for i in 0..4 {
                let s = q.n[i] * q.det;
                for k in 0..3 {
                    acc[i][k] += s * bx[k];
                }
            }
        }
    }
    let mut fl = [0.0; 24];
    for i in 0..4 {
        let bl = [dot(f.axes[0], acc[i]), dot(f.axes[1], acc[i]), dot(f.axes[2], acc[i])];
        for a in 0..3 {
            fl[6 * i + a] = m0 * bl[a];
        }
        fl[6 * i + 3] = -m1 * bl[1];
        fl[6 * i + 4] = m1 * bl[0];
    }
    let t = transformation(&f);
    let mut out = [0.0; 24];
    for (j, o) in out.iter_mut().enumerate() {
        *o = (0..24).map(|i| t[i * 24 + j] * fl[i]).sum();
    }
    Ok(out)
}

/// The consistent nodal load (forces and moments, global axes, the element's unknown order) of the membrane thermal strain
/// `eps_th = alpha dT (1, 1, 0)` in local axes, uniform through the thickness, of the temperature changes `delta_t_nodal`
/// (`T_i - T_ref` of the four nodes, interpolated by the facet's functions): `sum_gp det B_m^T A eps_th` at the 2 x 2 points
/// of the stiffness, `A` the membrane stiffness of the homogeneous section, carried to the nodes by `T^T` (ADR 0445 D2).
/// Refuses other than four temperatures and a material without a finite expansion coefficient.
pub fn thermal_load_vector(
    coords: &[Vec3],
    thickness: f64,
    material: &Material,
    delta_t_nodal: &[f64],
) -> Result<[f64; 24]> {
    thermal_load_vector_with(coords, thickness, material, delta_t_nodal, Shell4Membrane::Bilinear)
}

/// The consistent membrane thermal load for a membrane (ADR 0470 arithmetic 7): that of [`thermal_load_vector`] for the
/// bilinear membrane; for the ANDES membrane `(1/2) sum_T A_T Bbar_T^T A eps_th` over the four triangles, `eps_th = alpha
/// dT_T (1, 1, 0)` with `dT_T` the mean of the changes at the triangle's nodes, so that the free expansion is the discrete
/// solution.
pub fn thermal_load_vector_with(
    coords: &[Vec3],
    thickness: f64,
    material: &Material,
    delta_t_nodal: &[f64],
    membrane: Shell4Membrane,
) -> Result<[f64; 24]> {
    if delta_t_nodal.len() != 4 {
        return Err(FemError::FieldLength {
            key: "temperature",
            expected: 4,
            found: delta_t_nodal.len(),
        });
    }
    let alpha = material.expansion();
    if !alpha.is_finite() {
        return Err(FemError::InvalidValue {
            key: "expansion",
            reason: format!(
                "a {} material has no single expansion coefficient for a shell thermal load",
                material.name()
            ),
        });
    }
    let f = facet(coords)?;
    let a = homogeneous_section(material, thickness).membrane;
    let mut fl = [0.0; 24];
    match membrane {
        Shell4Membrane::Bilinear => {
            for &xi in &GAUSS {
                for &eta in &GAUSS {
                    let q = point(&f.local, xi, eta);
                    let dt: f64 = (0..4).map(|i| q.n[i] * delta_t_nodal[i]).sum();
                    let e = alpha * dt;
                    let eps = [e, e, 0.0];
                    let s = [
                        a[0][0] * eps[0] + a[0][1] * eps[1] + a[0][2] * eps[2],
                        a[1][0] * eps[0] + a[1][1] * eps[1] + a[1][2] * eps[2],
                        a[2][0] * eps[0] + a[2][1] * eps[1] + a[2][2] * eps[2],
                    ];
                    let bm = membrane_rows(&q);
                    for (c, fc) in fl.iter_mut().enumerate() {
                        *fc += q.det * (bm[0][c] * s[0] + bm[1][c] * s[1] + bm[2][c] * s[2]);
                    }
                }
            }
        }
        Shell4Membrane::AndesOpt => {
            for tri in ANDES_TRIANGLES {
                let (_, b, ar) =
                    crate::shell3::andes_membrane(&[f.local[tri[0]], f.local[tri[1]], f.local[tri[2]]], &a);
                let e = alpha * (delta_t_nodal[tri[0]] + delta_t_nodal[tri[1]] + delta_t_nodal[tri[2]]) / 3.0;
                let s = [
                    a[0][0] * e + a[0][1] * e,
                    a[1][0] * e + a[1][1] * e,
                    a[2][0] * e + a[2][1] * e,
                ];
                for n in 0..3 {
                    for c in 0..3 {
                        let q = 3 * n + c;
                        fl[6 * tri[n] + MEMBRANE_UNKNOWNS[c]] +=
                            0.5 * ar * (b[0][q] * s[0] + b[1][q] * s[1] + b[2][q] * s[2]);
                    }
                }
            }
        }
    }
    let t = transformation(&f);
    let mut out = [0.0; 24];
    for (j, o) in out.iter_mut().enumerate() {
        *o = (0..24).map(|i| t[i * 24 + j] * fl[i]).sum();
    }
    Ok(out)
}

/// The consistent nodal load of a temperature uniform through the thickness of a layup (ADR 0496 D4): at the 2 x 2 points
/// `dT = sum N_i dT_i`, `sum_gp det (B_m^T N_th dT + B_b^T M_th dT)` with the laminate's thermal resultants per kelvin, carried
/// to the nodes by `T^T` (the bilinear membrane).
pub fn layup_thermal_load_vector(
    coords: &[Vec3],
    laminate: &crate::laminate::Laminate,
    delta_t_nodal: &[f64],
) -> Result<[f64; 24]> {
    if delta_t_nodal.len() != 4 {
        return Err(FemError::FieldLength {
            key: "temperature",
            expected: 4,
            found: delta_t_nodal.len(),
        });
    }
    let f = facet(coords)?;
    let mut fl = [0.0; 24];
    for &xi in &GAUSS {
        for &eta in &GAUSS {
            let q = point(&f.local, xi, eta);
            let dt: f64 = (0..4).map(|i| q.n[i] * delta_t_nodal[i]).sum();
            let n = [laminate.n_th[0] * dt, laminate.n_th[1] * dt, laminate.n_th[2] * dt];
            let m = [laminate.m_th[0] * dt, laminate.m_th[1] * dt, laminate.m_th[2] * dt];
            let bm = membrane_rows(&q);
            let bb = bending_rows(&q);
            for (c, fc) in fl.iter_mut().enumerate() {
                *fc += q.det
                    * (bm[0][c] * n[0]
                        + bm[1][c] * n[1]
                        + bm[2][c] * n[2]
                        + bb[0][c] * m[0]
                        + bb[1][c] * m[1]
                        + bb[2][c] * m[2]);
            }
        }
    }
    let t = transformation(&f);
    let mut out = [0.0; 24];
    for (j, o) in out.iter_mut().enumerate() {
        *o = (0..24).map(|i| t[i * 24 + j] * fl[i]).sum();
    }
    Ok(out)
}

/// The thermal load of a Shell4 element of a model (ADR 0496 D4): a `Shell` block gives [`thermal_load_vector_with`] of its
/// thickness, the element's material and membrane, as before; a layup of one isotropic ply the same with the ply's material
/// and thickness; any other layup [`layup_thermal_load_vector`].
pub(crate) fn model_thermal_load_vector(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
    delta_t_nodal: &[f64],
) -> Result<Vec<f64>> {
    if let Some(BlockProperty::Layup { plies }) = mesh.blocks()[mesh.block_of(element)].property() {
        if let Some((ply_material, t)) = crate::laminate::single_isotropic(plies) {
            return thermal_load_vector_with(coords, t, ply_material, delta_t_nodal, Shell4Membrane::Bilinear)
                .map(|v| v.to_vec());
        }
        let l = crate::laminate::laminate(plies)?;
        return layup_thermal_load_vector(coords, &l, delta_t_nodal).map(|v| v.to_vec());
    }
    let t = thickness_or_error(mesh, element)?;
    let membrane = model_membrane(mesh, element);
    thermal_load_vector_with(coords, t, material, delta_t_nodal, membrane).map(|v| v.to_vec())
}

/// The thickness of the block of a shell element, if it is a Shell4 or Shell3 element (ADR 0454 D2) of a block with a Shell
/// property.
pub fn shell_thickness(mesh: &SolidMesh, element: usize) -> Option<f64> {
    if !matches!(mesh.element_kind(element), ElementKind::Shell4 | ElementKind::Shell3) {
        return None;
    }
    match mesh.blocks()[mesh.block_of(element)].property() {
        Some(BlockProperty::Shell { thickness, .. }) => Some(*thickness),
        _ => None,
    }
}

pub(crate) fn thickness_or_error(mesh: &SolidMesh, element: usize) -> Result<f64> {
    // ADR 0496 D7: a path that needs one thickness and does not take a layup refuses it naming the follow-up.
    if let Some(BlockProperty::Layup { .. }) = mesh.blocks()[mesh.block_of(element)].property() {
        if matches!(mesh.element_kind(element), ElementKind::Shell4 | ElementKind::Shell3) {
            return Err(FemError::NotYetSupported {
                item: "shell property",
                variant: "Layup",
                location: format!("element {element}"),
                package: crate::laminate::FOLLOW_UP,
            });
        }
    }
    shell_thickness(mesh, element).ok_or_else(|| FemError::InvalidValue {
        key: "property",
        reason: format!(
            "block {} ({}) has no Shell property: a shell needs its thickness",
            mesh.block_of(element),
            mesh.element_kind(element).name()
        ),
    })
}

/// The thickness that carries the membrane resultant of a shell element's pre-stress, `N = t sigma` (ADR 0502 D2): a Shell4
/// element of a Layup block the total thickness of its plies (one isotropic ply: its thickness), any other element
/// [`thickness_or_error`] as before.
pub(crate) fn membrane_thickness(mesh: &SolidMesh, element: usize) -> Result<f64> {
    if let Some(BlockProperty::Layup { plies }) = mesh.blocks()[mesh.block_of(element)].property() {
        if mesh.element_kind(element) == ElementKind::Shell4 {
            if let Some((_, t)) = crate::laminate::single_isotropic(plies) {
                return Ok(t);
            }
            return Ok(crate::laminate::laminate(plies)?.thickness);
        }
    }
    thickness_or_error(mesh, element)
}

/// The facet of an element of a model, its refusals naming the element (ADR 0432 D4).
fn model_facet(mesh: &SolidMesh, e: usize) -> Result<Facet> {
    let coords: Vec<Vec3> = mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect();
    facet(&coords).map_err(|source| match source {
        FemError::InvertedElement {
            local_node, jacobian, ..
        } => FemError::InvertedElement {
            element: e,
            local_node,
            jacobian,
        },
        other => FemError::Element {
            element: e,
            source: Box::new(other),
        },
    })
}

/// The checks of a model with shells (ADR 0432 D4, ADR 0454 D2): every Shell4 and Shell3 block has a Shell property, no
/// node of a shell element is a node of a solid element (the first in element and node order, named), every shell element
/// has a valid facet.
pub fn check_shell_model(model: &SolidModel) -> Result<()> {
    let mesh = model.mesh();
    let mut any = false;
    for (b, block) in mesh.blocks().iter().enumerate() {
        if matches!(block.kind(), ElementKind::Shell4 | ElementKind::Shell3) {
            any = true;
            // ADR 0496 D7: a Shell4 block takes a layup (its plies checked); a Shell3 block refuses it.
            if let Some(BlockProperty::Layup { plies }) = block.property() {
                if block.kind() == ElementKind::Shell3 {
                    return Err(FemError::NotYetSupported {
                        item: "shell property",
                        variant: "Layup",
                        location: format!("block {b} (Shell3)"),
                        package: crate::laminate::FOLLOW_UP,
                    });
                }
                crate::laminate::check_plies(plies).map_err(|e| match e {
                    FemError::InvalidValue { key, reason } => FemError::InvalidValue {
                        key,
                        reason: format!("block {b}, {reason}"),
                    },
                    other => other,
                })?;
                continue;
            }
            if !matches!(block.property(), Some(BlockProperty::Shell { .. })) {
                return Err(FemError::InvalidValue {
                    key: "property",
                    reason: format!(
                        "block {b} ({}) has no Shell property: a shell needs its thickness",
                        block.kind().name()
                    ),
                });
            }
        }
    }
    if !any {
        return Ok(());
    }
    let mut solid_of: Vec<Option<usize>> = vec![None; mesh.nodes().len()];
    for e in 0..mesh.n_elements() {
        if mesh.element_kind(e).is_solid() {
            for &n in mesh.element_nodes(e) {
                if solid_of[n].is_none() {
                    solid_of[n] = Some(e);
                }
            }
        }
    }
    for e in 0..mesh.n_elements() {
        let kind = mesh.element_kind(e);
        if !matches!(kind, ElementKind::Shell4 | ElementKind::Shell3) {
            continue;
        }
        for &n in mesh.element_nodes(e) {
            if let Some(solid) = solid_of[n] {
                return Err(FemError::Element {
                    element: e,
                    source: Box::new(FemError::InvalidValue {
                        key: "node",
                        reason: format!(
                            "node {n} of the shell element {e} is also a node of the solid element {solid}: a solid has no \
                             rotational stiffness (a shell-solid joint is {FOLLOW_UP})"
                        ),
                    }),
                });
            }
        }
        if kind == ElementKind::Shell3 {
            crate::shell3::model_facet(mesh, e)?;
        } else {
            model_facet(mesh, e)?;
        }
    }
    // ADR 0466 D3: no edge between shell elements of different membrane classes.
    crate::shell3::check_membrane_edges(mesh)
}

/// The stiffness of a shell element of a model (its block's thickness, its material).
pub(crate) fn model_element_stiffness(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
) -> Result<ElementMatrix> {
    let section = model_section(mesh, element, material)?;
    let options = ShellOptions::default().membrane(model_membrane(mesh, element));
    element_stiffness_section(coords, &section, &options)
}

/// The membrane of a Shell4 element of a model: its block's `Shell` property's Shell4 membrane, the bilinear membrane
/// otherwise (ADR 0470 D2).
pub(crate) fn model_membrane(mesh: &SolidMesh, element: usize) -> Shell4Membrane {
    match mesh.blocks()[mesh.block_of(element)].property() {
        Some(BlockProperty::Shell { shell4_membrane, .. }) if mesh.element_kind(element) == ElementKind::Shell4 => {
            *shell4_membrane
        }
        _ => Shell4Membrane::Bilinear,
    }
}

/// The consistent mass of a shell element of a model.
pub(crate) fn model_element_mass(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
) -> Result<ElementMatrix> {
    // ADR 0496 D3, D5: the mass of a layup; one isotropic ply takes the homogeneous mass of its material.
    if let Some(BlockProperty::Layup { plies }) = mesh.blocks()[mesh.block_of(element)].property() {
        if mesh.element_kind(element) == ElementKind::Shell4 {
            if let Some((ply_material, t)) = crate::laminate::single_isotropic(plies) {
                return element_mass(coords, t, ply_material);
            }
            let l = crate::laminate::laminate(plies)?;
            let f = facet(coords)?;
            return Ok(to_global(
                &local_mass_layup(&f, l.mass[0], l.mass[1], l.mass[2]),
                &transformation(&f),
            ));
        }
    }
    let t = thickness_or_error(mesh, element)?;
    element_mass(coords, t, material)
}

/// The resultants and surface stresses of a shell element at its centre, in its local axes (ADR 0432 arithmetic 9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellStresses {
    /// `[e_1, e_2, e_3]` of the element.
    pub axes: [Vec3; 3],
    /// Membrane forces `(N_xx, N_yy, N_xy)`, N/m.
    pub membrane: [f64; 3],
    /// Moments `(M_xx, M_yy, M_xy)`, N m/m (`M = D_b kappa`, `sigma = 12 z M / t^3`).
    pub bending: [f64; 3],
    /// Transverse shear forces `(Q_x, Q_y)`, N/m (the assumed strain).
    pub shear: [f64; 2],
    /// `(sigma_xx, sigma_yy, sigma_xy)` at `z = +t/2` (the `+e_3` side), Pa.
    pub top: [f64; 3],
    /// The same at `z = -t/2`.
    pub bottom: [f64; 3],
}

/// The resultants and surface stresses of every shell element of a model for a displacement vector (one value per unknown
/// of `model.dof_map()`), as (element, stresses) in element order. With a temperature field the membrane forces are those of
/// the elastic strain `eps - alpha dT_c (1, 1, 0)`, `dT_c` at the centre (ADR 0445 D2). A Shell3 element gives those of
/// ADR 0454 arithmetic 8, at its centroid.
pub fn stresses(model: &SolidModel, displacement: &[f64]) -> Result<Vec<(usize, ShellStresses)>> {
    let dofs = model.dof_map();
    if displacement.len() != dofs.n_equations() {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: dofs.n_equations(),
            found: displacement.len(),
        });
    }
    let mesh = model.mesh();
    let materials = model.material_indices()?;
    let temperature = crate::thermal::temperature_field(model)?;
    let mut out = Vec::new();
    for e in 0..mesh.n_elements() {
        if mesh.element_kind(e) == ElementKind::Shell3 {
            out.push((e, shell3_stresses(model, displacement, e, temperature)?));
            continue;
        }
        if mesh.element_kind(e) != ElementKind::Shell4 {
            continue;
        }
        let located = |source: FemError| FemError::Element {
            element: e,
            source: Box::new(source),
        };
        // ADR 0496 D3, D7: a layup of one isotropic ply takes the homogeneous recovery with its ply's material and thickness,
        // any other layup its resultants.
        let (t, ply_material) = match mesh.blocks()[mesh.block_of(e)].property() {
            Some(BlockProperty::Layup { plies }) => match crate::laminate::single_isotropic(plies) {
                Some((m, t)) => (t, Some(m)),
                None => {
                    out.push((e, layup_stresses(model, displacement, e, plies, temperature)?));
                    continue;
                }
            },
            _ => (thickness_or_error(mesh, e).map_err(located)?, None),
        };
        let f = model_facet(mesh, e)?;
        let ue: Vec<f64> = element_dofs(&dofs, mesh.element_nodes(e))
            .iter()
            .map(|&q| displacement[q])
            .collect();
        if ue.len() != 24 {
            return Err(located(FemError::FieldLength {
                key: "element unknowns",
                expected: 24,
                found: ue.len(),
            }));
        }
        let tr = transformation(&f);
        let ul: Vec<f64> = (0..24).map(|i| (0..24).map(|j| tr[i * 24 + j] * ue[j]).sum()).collect();
        let material = ply_material.unwrap_or(&model.materials()[materials[e]].1);
        let section = model_section(mesh, e, material).map_err(located)?;
        let dm = section.membrane;
        let db = section.bending;
        let p = point(&f.local, 0.0, 0.0);
        let strain = |b: &[[f64; 24]; 3]| -> [f64; 3] {
            let mut s = [0.0; 3];
            for (r, row) in b.iter().enumerate() {
                s[r] = row.iter().zip(&ul).map(|(a, x)| a * x).sum();
            }
            s
        };
        // ADR 0470 D1: the ANDES membrane reports its mean basic strain.
        let mut eps = match model_membrane(mesh, e) {
            Shell4Membrane::AndesOpt => strain(&andes_centre_rows(&f.local, &dm)),
            Shell4Membrane::Bilinear => strain(&membrane_rows(&p)),
        };
        if let Some((nodal, reference)) = temperature {
            // ADR 0445 D2: the membrane thermal strain at the centre, uniform through the thickness.
            let dt: f64 = mesh
                .element_nodes(e)
                .iter()
                .zip(&p.n)
                .map(|(&n, &na)| na * (nodal[n] - reference))
                .sum();
            let th = material.expansion() * dt;
            eps[0] -= th;
            eps[1] -= th;
        }
        let kappa = strain(&bending_rows(&p));
        let mul = |d: &[[f64; 3]; 3], v: [f64; 3]| -> [f64; 3] {
            [
                d[0][0] * v[0] + d[0][1] * v[1] + d[0][2] * v[2],
                d[1][0] * v[0] + d[1][1] * v[1] + d[1][2] * v[2],
                d[2][0] * v[0] + d[2][1] * v[1] + d[2][2] * v[2],
            ]
        };
        let mut membrane = mul(&dm, eps);
        let mut bending = mul(&db, kappa);
        if let Some(c) = &section.coupling {
            let ck = mul(c, kappa);
            let cte = [
                c[0][0] * eps[0] + c[1][0] * eps[1] + c[2][0] * eps[2],
                c[0][1] * eps[0] + c[1][1] * eps[1] + c[2][1] * eps[2],
                c[0][2] * eps[0] + c[1][2] * eps[1] + c[2][2] * eps[2],
            ];
            for k in 0..3 {
                membrane[k] += ck[k];
                bending[k] += cte[k];
            }
        }
        let tying = tying_rows(&f.local);
        let s = shear_rows(&p, 0.0, 0.0, TransverseShear::AssumedStrain, &tying);
        let g0 = s[0].iter().zip(&ul).map(|(a, x)| a * x).sum::<f64>();
        let g1 = s[1].iter().zip(&ul).map(|(a, x)| a * x).sum::<f64>();
        let shear = match section.shear {
            ShearStiffness::Isotropic(ds) => [ds * g0, ds * g1],
            ShearStiffness::Matrix(h) => [h[0][0] * g0 + h[0][1] * g1, h[1][0] * g0 + h[1][1] * g1],
        };
        let mut top = [0.0; 3];
        let mut bottom = [0.0; 3];
        for k in 0..3 {
            top[k] = membrane[k] / t + 6.0 * bending[k] / (t * t);
            bottom[k] = membrane[k] / t - 6.0 * bending[k] / (t * t);
        }
        out.push((
            e,
            ShellStresses {
                axes: f.axes,
                membrane,
                bending,
                shear,
                top,
                bottom,
            },
        ));
    }
    Ok(out)
}

/// Facet, membrane strain, curvature, transverse shear strain and temperature change at an element centre (ADR 0496 D6).
type LayupCentre = (Facet, [f64; 3], [f64; 3], [f64; 2], f64);

/// Element, membrane strain, curvature and temperature change at its centre (ADR 0496 D6).
pub(crate) type CentreStrain = (usize, [f64; 3], [f64; 3], f64);

/// The centre values of Shell4 element `e` (bilinear membrane): its facet, the membrane strain, the curvature, the assumed
/// transverse shear strain and the temperature change at the centre (ADR 0496 D6).
fn layup_centre(
    model: &SolidModel,
    displacement: &[f64],
    e: usize,
    temperature: Option<(&[f64], f64)>,
) -> Result<LayupCentre> {
    let mesh = model.mesh();
    let f = model_facet(mesh, e)?;
    let ue: Vec<f64> = element_dofs(&model.dof_map(), mesh.element_nodes(e))
        .iter()
        .map(|&q| displacement[q])
        .collect();
    if ue.len() != 24 {
        return Err(FemError::Element {
            element: e,
            source: Box::new(FemError::FieldLength {
                key: "element unknowns",
                expected: 24,
                found: ue.len(),
            }),
        });
    }
    let tr = transformation(&f);
    let ul: Vec<f64> = (0..24).map(|i| (0..24).map(|j| tr[i * 24 + j] * ue[j]).sum()).collect();
    let p = point(&f.local, 0.0, 0.0);
    let strain = |b: &[[f64; 24]; 3]| -> [f64; 3] {
        let mut s = [0.0; 3];
        for (r, row) in b.iter().enumerate() {
            s[r] = row.iter().zip(&ul).map(|(a, x)| a * x).sum();
        }
        s
    };
    let eps = strain(&membrane_rows(&p));
    let kappa = strain(&bending_rows(&p));
    let tying = tying_rows(&f.local);
    let sr = shear_rows(&p, 0.0, 0.0, TransverseShear::AssumedStrain, &tying);
    let g0 = sr[0].iter().zip(&ul).map(|(a, x)| a * x).sum::<f64>();
    let g1 = sr[1].iter().zip(&ul).map(|(a, x)| a * x).sum::<f64>();
    let dt = match temperature {
        Some((nodal, reference)) => mesh
            .element_nodes(e)
            .iter()
            .zip(&p.n)
            .map(|(&n, &na)| na * (nodal[n] - reference))
            .sum(),
        None => 0.0,
    };
    Ok((f, eps, kappa, [g0, g1], dt))
}

/// The membrane strain, curvature and centre temperature change of every Shell4 element of a Layup block (ADR 0496 D6), in
/// element order.
pub(crate) fn centre_strains(model: &SolidModel, displacement: &[f64]) -> Result<Vec<CentreStrain>> {
    let dofs = model.dof_map();
    if displacement.len() != dofs.n_equations() {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: dofs.n_equations(),
            found: displacement.len(),
        });
    }
    let mesh = model.mesh();
    let temperature = crate::thermal::temperature_field(model)?;
    let mut out = Vec::new();
    for e in 0..mesh.n_elements() {
        if mesh.element_kind(e) != ElementKind::Shell4 {
            continue;
        }
        if !matches!(
            mesh.blocks()[mesh.block_of(e)].property(),
            Some(BlockProperty::Layup { .. })
        ) {
            continue;
        }
        let (_, eps, kappa, _, dt) = layup_centre(model, displacement, e, temperature)?;
        out.push((e, eps, kappa, dt));
    }
    Ok(out)
}

/// The resultants of Shell4 element `e` of a layup (not one isotropic ply) at its centre (ADR 0496 D2, D7): `N = A eps + B
/// kappa - N_th dT`, `M = B eps + D kappa - M_th dT`, `Q = H gamma`; `top` and `bottom` the element-axes stresses of the
/// outer plies at `z = +-t/2`, `Qbar (eps + z kappa - alphabar dT)`.
fn layup_stresses(
    model: &SolidModel,
    displacement: &[f64],
    e: usize,
    plies: &[crate::mesh::Ply],
    temperature: Option<(&[f64], f64)>,
) -> Result<ShellStresses> {
    let located = |source: FemError| FemError::Element {
        element: e,
        source: Box::new(source),
    };
    let l = crate::laminate::laminate(plies).map_err(located)?;
    let (f, eps, kappa, gamma, dt) = layup_centre(model, displacement, e, temperature)?;
    let mut membrane = [0.0; 3];
    let mut bending = [0.0; 3];
    for k in 0..3 {
        membrane[k] = l.a[k][0] * eps[0]
            + l.a[k][1] * eps[1]
            + l.a[k][2] * eps[2]
            + l.b[k][0] * kappa[0]
            + l.b[k][1] * kappa[1]
            + l.b[k][2] * kappa[2]
            - l.n_th[k] * dt;
        bending[k] = l.b[0][k] * eps[0]
            + l.b[1][k] * eps[1]
            + l.b[2][k] * eps[2]
            + l.d[k][0] * kappa[0]
            + l.d[k][1] * kappa[1]
            + l.d[k][2] * kappa[2]
            - l.m_th[k] * dt;
    }
    let h = l.h;
    let shear = [
        h[0][0] * gamma[0] + h[0][1] * gamma[1],
        h[1][0] * gamma[0] + h[1][1] * gamma[1],
    ];
    let face = |k: usize, z: f64| -> Result<[f64; 3]> {
        let ply = &plies[k];
        let pc = crate::laminate::ply_constants(ply, k)?;
        let qb = crate::laminate::rotated_stiffness(&pc.q, ply.angle);
        let ab = crate::laminate::rotated_expansion(pc.alpha, ply.angle);
        let m = [
            eps[0] + z * kappa[0] - ab[0] * dt,
            eps[1] + z * kappa[1] - ab[1] * dt,
            eps[2] + z * kappa[2] - ab[2] * dt,
        ];
        Ok([
            qb[0][0] * m[0] + qb[0][1] * m[1] + qb[0][2] * m[2],
            qb[1][0] * m[0] + qb[1][1] * m[1] + qb[1][2] * m[2],
            qb[2][0] * m[0] + qb[2][1] * m[1] + qb[2][2] * m[2],
        ])
    };
    let n = plies.len();
    let top = face(n - 1, l.z[n]).map_err(located)?;
    let bottom = face(0, l.z[0]).map_err(located)?;
    Ok(ShellStresses {
        axes: f.axes,
        membrane,
        bending,
        shear,
        top,
        bottom,
    })
}

/// The stresses of Shell3 element `e` (ADR 0454 arithmetic 8).
fn shell3_stresses(
    model: &SolidModel,
    displacement: &[f64],
    e: usize,
    temperature: Option<(&[f64], f64)>,
) -> Result<ShellStresses> {
    let mesh = model.mesh();
    let located = |source: FemError| FemError::Element {
        element: e,
        source: Box::new(source),
    };
    let t = thickness_or_error(mesh, e).map_err(located)?;
    let f = crate::shell3::model_facet(mesh, e)?;
    let ue: Vec<f64> = element_dofs(&model.dof_map(), mesh.element_nodes(e))
        .iter()
        .map(|&q| displacement[q])
        .collect();
    if ue.len() != 18 {
        return Err(located(FemError::FieldLength {
            key: "element unknowns",
            expected: 18,
            found: ue.len(),
        }));
    }
    let materials = model.material_indices()?;
    let material = &model.materials()[materials[e]].1;
    let section = model_section(mesh, e, material).map_err(located)?;
    let thermal = match temperature {
        Some((nodal, reference)) => {
            let dt: f64 = mesh.element_nodes(e).iter().map(|&n| nodal[n] - reference).sum::<f64>() / 3.0;
            material.expansion() * dt
        }
        None => 0.0,
    };
    let membrane = crate::shell3::model_membrane(mesh, e);
    Ok(crate::shell3::element_stresses(&f, &ue, &section, t, thermal, membrane))
}
