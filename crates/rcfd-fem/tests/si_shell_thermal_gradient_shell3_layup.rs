//! F-687 (ADR 0626): a temperature gradient through the thickness of Shell3 and of Layup Shell4 shells
//! (`Load::ShellTemperatureGradient`). Checks L1 to L3, C1 to C3, F1 to F3, X1 and the negative controls N1' (the copy of ADR
//! 0612 N3 to N8), N9 to N12 of the registration (B1b is `si_shell_thermal_gradient_layup_bits.rs`, B1 the ADR 0612 file).

use rcfd_core::vec3::{dot, norm, Vec3};
use rcfd_fem::builders::hex_box;
use rcfd_fem::laminate::{layup_response, rotated_expansion, rotated_stiffness};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::material::MaterialAxes;
use rcfd_fem::nonlinear::{self, NewtonOptions};
use rcfd_fem::shell::{self, ShellStresses};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::thermal::GRADIENT_FOLLOW_UP;
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, Ply, SolidMesh, SolidModel,
};

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.6e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

type M3 = [[f64; 3]; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const ALPHA: f64 = 1.2e-5;
const GRAD: f64 = 200.0;
const CORNERS: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
// The CF ply (ADR 0626 checks).
const E1: f64 = 1.4e11;
const E2: f64 = 1.0e10;
const NU12: f64 = 0.3;
const G12: f64 = 5.0e9;
const A1: f64 = -0.5e-6;
const A2: f64 = 3.0e-5;

fn steel() -> Material {
    Material::isotropic_elastic(E, NU, 7850.0, ALPHA).unwrap()
}

fn aluminium() -> Material {
    Material::isotropic_elastic(7.0e10, 0.33, 2700.0, 2.3e-5).unwrap()
}

fn cf() -> Material {
    let axes = MaterialAxes::fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
    Material::orthotropic(
        [E1, E2, 1.0e10],
        [NU12, 0.3, 0.45],
        [G12, 5.0e9, 3.5e9],
        1600.0,
        [A1, A2, A2],
        axes,
    )
    .unwrap()
}

fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

fn mv(a: &M3, v: [f64; 3]) -> [f64; 3] {
    [
        a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2],
        a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2],
        a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2],
    ]
}

// ------------------------------------------------------------------------------------------ the laminate, test side

#[derive(Clone, Copy)]
enum Mat {
    Steel,
    Al,
    Cf,
}

/// A ply of the test: its element-axes stiffness and `Qbar abar`, its thickness, and its material-axes `Q` and expansion.
struct HPly {
    t: f64,
    qbar: M3,
    qa: [f64; 3],
    q: M3,
    a: [f64; 2],
}

fn iso_q(e: f64, nu: f64) -> M3 {
    let c = e / (1.0 - nu * nu);
    [[c, nu * c, 0.0], [nu * c, c, 0.0], [0.0, 0.0, e / (2.0 * (1.0 + nu))]]
}

fn cf_q() -> M3 {
    let nu21 = NU12 * E2 / E1;
    let d = 1.0 - NU12 * nu21;
    [
        [E1 / d, NU12 * E2 / d, 0.0],
        [NU12 * E2 / d, E2 / d, 0.0],
        [0.0, 0.0, G12],
    ]
}

fn hand_ply(m: Mat, t: f64, deg: f64) -> HPly {
    match m {
        Mat::Steel | Mat::Al => {
            let (e, nu, a) = match m {
                Mat::Steel => (E, NU, ALPHA),
                _ => (7.0e10, 0.33, 2.3e-5),
            };
            let q = iso_q(e, nu);
            let c = e * a / (1.0 - nu);
            HPly {
                t,
                qbar: q,
                qa: [c, c, 0.0],
                q,
                a: [a, a],
            }
        }
        Mat::Cf => {
            let q = cf_q();
            let q0 = [q[0][0] * A1 + q[0][1] * A2, q[1][0] * A1 + q[1][1] * A2, 0.0];
            let (qbar, qa) = if deg == 0.0 {
                (q, q0)
            } else if deg == 90.0 {
                (
                    [[q[1][1], q[0][1], 0.0], [q[0][1], q[0][0], 0.0], [0.0, 0.0, q[2][2]]],
                    [q0[1], q0[0], 0.0],
                )
            } else {
                let th = deg.to_radians();
                let qb = rotated_stiffness(&q, th);
                let ab = rotated_expansion([A1, A2], th);
                (qb, mv(&qb, ab))
            };
            HPly {
                t,
                qbar,
                qa,
                q,
                a: [A1, A2],
            }
        }
    }
}

struct HLam {
    a: M3,
    b: M3,
    d: M3,
    mth: [f64; 3],
    pth: [f64; 3],
    z: Vec<f64>,
    t: f64,
    plies: Vec<HPly>,
}

/// `(material, thickness, angle in degrees)` bottom first.
type Spec = &'static [(Mat, f64, f64)];

const SYM: Spec = &[(Mat::Cf, 0.03, 0.0), (Mat::Cf, 0.04, 90.0), (Mat::Cf, 0.03, 0.0)];
const UNS: Spec = &[(Mat::Cf, 0.05, 0.0), (Mat::Cf, 0.05, 90.0)];
const BIM: Spec = &[(Mat::Steel, 0.05, 0.0), (Mat::Al, 0.05, 0.0)];
const ANG: Spec = &[(Mat::Cf, 0.02, 30.0), (Mat::Cf, 0.03, -45.0), (Mat::Cf, 0.025, 60.0)];

fn plies_of(spec: Spec) -> Vec<Ply> {
    spec.iter()
        .map(|&(m, t, deg)| {
            let mat = match m {
                Mat::Steel => steel(),
                Mat::Al => aluminium(),
                Mat::Cf => cf(),
            };
            Ply::new(mat, t, deg.to_radians()).unwrap()
        })
        .collect()
}

/// The laminate by the `z^2`, `z^3` difference forms (ADR 0626 arithmetic 1), test side.
fn hand_laminate(spec: Spec) -> HLam {
    let plies: Vec<HPly> = spec.iter().map(|&(m, t, deg)| hand_ply(m, t, deg)).collect();
    let t: f64 = plies.iter().map(|p| p.t).sum();
    let mut l = HLam {
        a: [[0.0; 3]; 3],
        b: [[0.0; 3]; 3],
        d: [[0.0; 3]; 3],
        mth: [0.0; 3],
        pth: [0.0; 3],
        z: vec![-0.5 * t],
        t,
        plies: Vec::new(),
    };
    for p in &plies {
        let z0 = *l.z.last().unwrap();
        let z1 = z0 + p.t;
        let (d1, d2, d3) = (z1 - z0, (z1 * z1 - z0 * z0) / 2.0, (z1 * z1 * z1 - z0 * z0 * z0) / 3.0);
        for i in 0..3 {
            for j in 0..3 {
                l.a[i][j] += p.qbar[i][j] * d1;
                l.b[i][j] += p.qbar[i][j] * d2;
                l.d[i][j] += p.qbar[i][j] * d3;
            }
            l.mth[i] += p.qa[i] * d2;
            l.pth[i] += p.qa[i] * d3;
        }
        l.z.push(z1);
    }
    l.plies = plies;
    l
}

impl HLam {
    fn moment_scale(&self, g: f64) -> f64 {
        max_abs(self.pth) * g
    }
    fn force_scale(&self, g: f64) -> f64 {
        g * 0.5 * self.t * self.plies.iter().map(|p| p.t * max_abs(p.qa)).sum::<f64>()
    }
    fn face_scale(&self, g: f64) -> f64 {
        self.plies.iter().map(|p| max_abs(p.qa)).fold(0.0, f64::max) * 0.5 * self.t * g
    }
    /// The free strain and curvature under a uniform `g`: `[A B; B D] [eps0; kappa] = g [M_th; P_th]`.
    fn free(&self, g: f64) -> ([f64; 3], [f64; 3]) {
        let mut m = [[0.0; 7]; 6];
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] = self.a[i][j];
                m[i][j + 3] = self.b[i][j];
                m[i + 3][j] = self.b[j][i];
                m[i + 3][j + 3] = self.d[i][j];
            }
            m[i][6] = g * self.mth[i];
            m[i + 3][6] = g * self.pth[i];
        }
        for c in 0..6 {
            let p = (c..6)
                .max_by(|&x, &y| m[x][c].abs().partial_cmp(&m[y][c].abs()).unwrap())
                .unwrap();
            m.swap(c, p);
            for r in 0..6 {
                if r != c {
                    let f = m[r][c] / m[c][c];
                    for k in c..7 {
                        m[r][k] -= f * m[c][k];
                    }
                }
            }
        }
        let x: Vec<f64> = (0..6).map(|i| m[i][6] / m[i][i]).collect();
        ([x[0], x[1], x[2]], [x[3], x[4], x[5]])
    }
}

// ------------------------------------------------------------------------------------------------ meshes and fields

fn rotation(axis: Vec3, angle: f64) -> M3 {
    let n = norm(axis);
    let a = [axis[0] / n, axis[1] / n, axis[2] / n];
    let (s, c) = angle.sin_cos();
    let k = [[0.0, -a[2], a[1]], [a[2], 0.0, -a[0]], [-a[1], a[0], 0.0]];
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let kk: f64 = (0..3).map(|m| k[i][m] * k[m][j]).sum();
            r[i][j] = f64::from(u8::from(i == j)) + s * k[i][j] + (1.0 - c) * kk;
        }
    }
    r
}

struct Placement {
    q: M3,
    d: Vec3,
}

impl Placement {
    fn identity() -> Placement {
        Placement {
            q: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            d: [0.0; 3],
        }
    }
    fn inclined() -> Placement {
        Placement {
            q: rotation([1.0, 2.0, 3.0], 0.7),
            d: [0.3, -0.2, 0.5],
        }
    }
    fn place(&self, p: [f64; 2]) -> Vec3 {
        let v = [p[0], p[1], 0.0];
        [
            dot(self.q[0], v) + self.d[0],
            dot(self.q[1], v) + self.d[1],
            dot(self.q[2], v) + self.d[2],
        ]
    }
    fn axis(&self, k: usize) -> Vec3 {
        [self.q[0][k], self.q[1][k], self.q[2][k]]
    }
}

fn grid(a: f64, b: f64, nx: usize, ny: usize) -> (Vec<[f64; 2]>, Vec<[usize; 4]>) {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([a * i as f64 / nx as f64, b * j as f64 / ny as f64]);
        }
    }
    let mut el = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            el.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    (nodes, el)
}

/// TH: the ADR 0445 plate, 4 x 3 elements on `[0, 2] x [0, 1]`, interior nodes moved in plane.
fn th_plane() -> (Vec<[f64; 2]>, Vec<[usize; 4]>) {
    let (mut p, el) = grid(2.0, 1.0, 4, 3);
    for j in 1..3 {
        for i in 1..4 {
            let k = j * 5 + i;
            p[k][0] += 0.06 * (1.3 * k as f64 + 0.2).sin();
            p[k][1] += 0.05 * (0.9 * k as f64 + 0.4).cos();
        }
    }
    (p, el)
}

/// RG: the regular 4 x 3 grid on `[0, 2] x [0, 1]`.
fn rg_plane() -> (Vec<[f64; 2]>, Vec<[usize; 4]>) {
    grid(2.0, 1.0, 4, 3)
}

fn boundary_nodes() -> Vec<usize> {
    (0..20)
        .filter(|k| k % 5 == 0 || k % 5 == 4 || k / 5 == 0 || k / 5 == 3)
        .collect()
}

fn triangles(el: &[[usize; 4]]) -> Vec<usize> {
    el.iter().flat_map(|e| [e[0], e[1], e[2], e[0], e[2], e[3]]).collect()
}

/// A one-block shell model (material steel on every element), `clamped` the nodes with all six components held.
fn model(
    nodes: Vec<Vec3>,
    kind: ElementKind,
    conn: Vec<usize>,
    property: BlockProperty,
    clamped: &[usize],
) -> SolidModel {
    let n = nodes.len();
    let per = if kind == ElementKind::Shell3 { 3 } else { 4 };
    let ne = conn.len() / per;
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("held", clamped.to_vec()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    if !clamped.is_empty() {
        for comp in 0..6 {
            m.add_constraint(Constraint::displacement("held", comp, 0.0)).unwrap();
        }
    }
    m
}

/// The one-block model of [`model`] without the `held` node set and constraints (ADR 0626 amendment 2).
fn free_model(
    nodes: Vec<Vec3>,
    kind: ElementKind,
    conn: Vec<usize>,
    property: BlockProperty,
    _clamped: &[usize],
) -> SolidModel {
    let per = if kind == ElementKind::Shell3 { 3 } else { 4 };
    let ne = conn.len() / per;
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m
}

fn placed(p: &[[f64; 2]], place: &Placement) -> Vec<Vec3> {
    p.iter().map(|&q| place.place(q)).collect()
}

fn layup_plate(p: &[[f64; 2]], el: &[[usize; 4]], place: &Placement, spec: Spec, clamped: &[usize]) -> SolidModel {
    let conn: Vec<usize> = el.iter().flatten().copied().collect();
    model(
        placed(p, place),
        ElementKind::Shell4,
        conn,
        BlockProperty::layup(plies_of(spec)).unwrap(),
        clamped,
    )
}

fn shell3_plate(place: &Placement, t: f64, clamped: &[usize]) -> (SolidModel, Vec<[f64; 2]>) {
    let (p, el) = th_plane();
    let m = model(
        placed(&p, place),
        ElementKind::Shell3,
        triangles(&el),
        BlockProperty::shell(t).unwrap(),
        clamped,
    );
    (m, p)
}

fn derivatives(xe: &[[f64; 2]; 4], xi: f64, eta: f64) -> [[f64; 2]; 4] {
    let dxi: Vec<f64> = CORNERS.iter().map(|c| 0.25 * c[0] * (1.0 + c[1] * eta)).collect();
    let deta: Vec<f64> = CORNERS.iter().map(|c| 0.25 * c[1] * (1.0 + c[0] * xi)).collect();
    let mut j = [[0.0; 2]; 2];
    for a in 0..4 {
        for k in 0..2 {
            j[0][k] += dxi[a] * xe[a][k];
            j[1][k] += deta[a] * xe[a][k];
        }
    }
    let det = j[0][0] * j[1][1] - j[0][1] * j[1][0];
    let inv = [[j[1][1] / det, -j[0][1] / det], [-j[1][0] / det, j[0][0] / det]];
    let mut out = [[0.0; 2]; 4];
    for a in 0..4 {
        out[a] = [
            inv[0][0] * dxi[a] + inv[0][1] * deta[a],
            inv[1][0] * dxi[a] + inv[1][1] * deta[a],
        ];
    }
    out
}

/// `(f_X,X, f_Y,Y, f_X,Y + f_Y,X)` of a nodal plane vector field at `(xi, eta)`.
fn strain_at(xe: &[[f64; 2]; 4], ue: &[[f64; 2]; 4], xi: f64, eta: f64) -> [f64; 3] {
    let d = derivatives(xe, xi, eta);
    let mut s = [0.0; 3];
    for a in 0..4 {
        s[0] += d[a][0] * ue[a][0];
        s[1] += d[a][1] * ue[a][1];
        s[2] += d[a][1] * ue[a][0] + d[a][0] * ue[a][1];
    }
    s
}

/// The solved in-plane displacement and `beta = (theta . e_y, -theta . e_x)` in plate axes.
fn plane_fields(m: &SolidModel, u: &[f64], place: &Placement, n: usize) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let dofs = m.dof_map();
    let comp = |node: usize, k: usize| u[dofs.equation(node, k).unwrap()];
    let (ex, ey) = (place.axis(0), place.axis(1));
    let mut up = Vec::new();
    let mut beta = Vec::new();
    for node in 0..n {
        let v = [comp(node, 0), comp(node, 1), comp(node, 2)];
        let th = [comp(node, 3), comp(node, 4), comp(node, 5)];
        up.push([dot(v, ex), dot(v, ey)]);
        beta.push([dot(th, ey), -dot(th, ex)]);
    }
    (up, beta)
}

fn element_values<T: Copy>(v: &[T], e: &[usize; 4]) -> [T; 4] {
    [v[e[0]], v[e[1]], v[e[2]], v[e[3]]]
}

/// The exact discrete field of ADR 0626 arithmetic 4 about node 0: membrane strain `eps0`, curvature `kappa`, as global
/// unknowns of the plate model.
fn exact_field(m: &SolidModel, p: &[[f64; 2]], place: &Placement, eps0: [f64; 3], kappa: [f64; 3]) -> Vec<f64> {
    let dofs = m.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    let (ex, ey, ez) = (place.axis(0), place.axis(1), place.axis(2));
    for (n, q) in p.iter().enumerate() {
        let (dx, dy) = (q[0] - p[0][0], q[1] - p[0][1]);
        let uu = eps0[0] * dx + 0.5 * eps0[2] * dy;
        let vv = eps0[1] * dy + 0.5 * eps0[2] * dx;
        let w = -0.5 * (kappa[0] * dx * dx + kappa[1] * dy * dy + kappa[2] * dx * dy);
        let bx = kappa[0] * dx + 0.5 * kappa[2] * dy;
        let by = kappa[1] * dy + 0.5 * kappa[2] * dx;
        let (tx, ty) = (-by, bx);
        for i in 0..3 {
            u[dofs.equation(n, i).unwrap()] = uu * ex[i] + vv * ey[i] + w * ez[i];
            u[dofs.equation(n, 3 + i).unwrap()] = tx * ex[i] + ty * ey[i];
        }
    }
    u
}

fn uniform(n: usize) -> Load {
    Load::shell_temperature_gradient(vec![GRAD; n])
}

/// `M_s = D (1 + nu) alpha g` of steel.
fn steel_moment(t: f64, g: f64) -> f64 {
    E * t * t * t * ALPHA * g / (12.0 * (1.0 - NU))
}

/// `s_s = E alpha g t / (2 (1 - nu))` of steel.
fn steel_surface(t: f64, g: f64) -> f64 {
    E * ALPHA * g * t / (2.0 * (1.0 - NU))
}

fn worst<F: Fn(&ShellStresses) -> f64>(st: &[(usize, ShellStresses)], f: F) -> f64 {
    st.iter().map(|(_, s)| f(s)).fold(0.0, f64::max)
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

// -------------------------------------------------------------------------------------------------------------- L1

#[test]
#[ignore = "defective set-up of the registered run (ADR 0626 amendment 2): the helper adds an empty node set, refused before the checks; superseded by l1_shell3_load_on_a_triangle_after_amendment2"]
fn l1_shell3_load_on_a_triangle() {
    let mut c = Checks::new();
    let (a, b, t) = (0.5, 0.3, 0.02);
    let x = vec![[0.0, 0.0, 0.0], [a, 0.0, 0.0], [0.0, b, 0.0]];
    let g: Vec<f64> = x.iter().map(|q| 100.0 + 300.0 * q[0] - 150.0 * q[1]).collect();
    let mut m = model(
        x,
        ElementKind::Shell3,
        vec![0, 1, 2],
        BlockProperty::shell(t).unwrap(),
        &[],
    );
    m.add_load(Load::shell_temperature_gradient(g.clone())).unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let gm = (g[0] + g[1] + g[2]) / 3.0;
    let ms = steel_moment(t, gm);
    let area = 0.5 * a * b;
    let nx = [-1.0 / a, 1.0 / a, 0.0];
    let ny = [-1.0 / b, 0.0, 1.0 / b];
    let dofs = m.dof_map();
    let mut exact = vec![0.0; dofs.n_equations()];
    for i in 0..3 {
        exact[dofs.equation(i, 4).unwrap()] = area * nx[i] * ms;
        exact[dofs.equation(i, 3).unwrap()] = -area * ny[i] * ms;
    }
    let fmax = max_abs(exact.iter().copied());
    let d = max_abs(f.iter().zip(&exact).map(|(x, y)| x - y));
    c.at_most(
        "L1 Shell3 triangle load against arithmetic 3, / max |f|",
        d / fmax,
        1e-12,
    );
    let zero = (0..3).all(|i| [0, 1, 2, 5].iter().all(|&k| f[dofs.equation(i, k).unwrap()] == 0.0));
    c.holds("L1 forces and theta_z exactly zero", zero, "");
    c.finish();
}

/// The corrected test of ADR 0626 amendment 2: the same model without the empty `held` set.
#[test]
fn l1_shell3_load_on_a_triangle_after_amendment2() {
    let mut c = Checks::new();
    let (a, b, t) = (0.5, 0.3, 0.02);
    let x = vec![[0.0, 0.0, 0.0], [a, 0.0, 0.0], [0.0, b, 0.0]];
    let g: Vec<f64> = x.iter().map(|q| 100.0 + 300.0 * q[0] - 150.0 * q[1]).collect();
    let mut m = free_model(
        x,
        ElementKind::Shell3,
        vec![0, 1, 2],
        BlockProperty::shell(t).unwrap(),
        &[],
    );
    m.add_load(Load::shell_temperature_gradient(g.clone())).unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let gm = (g[0] + g[1] + g[2]) / 3.0;
    let ms = steel_moment(t, gm);
    let area = 0.5 * a * b;
    let nx = [-1.0 / a, 1.0 / a, 0.0];
    let ny = [-1.0 / b, 0.0, 1.0 / b];
    let dofs = m.dof_map();
    let mut exact = vec![0.0; dofs.n_equations()];
    for i in 0..3 {
        exact[dofs.equation(i, 4).unwrap()] = area * nx[i] * ms;
        exact[dofs.equation(i, 3).unwrap()] = -area * ny[i] * ms;
    }
    let fmax = max_abs(exact.iter().copied());
    let d = max_abs(f.iter().zip(&exact).map(|(x, y)| x - y));
    c.at_most(
        "L1 Shell3 triangle load against arithmetic 3, / max |f|",
        d / fmax,
        1e-12,
    );
    let zero = (0..3).all(|i| [0, 1, 2, 5].iter().all(|&k| f[dofs.equation(i, k).unwrap()] == 0.0));
    c.holds("L1 forces and theta_z exactly zero", zero, "");
    c.finish();
}

// -------------------------------------------------------------------------------------------------------------- L2

#[test]
#[ignore = "defective set-up of the registered run (ADR 0626 amendment 2): the helper adds an empty node set, refused before the checks; superseded by l2_layup_load_on_a_rectangle_after_amendment2"]
fn l2_layup_load_on_a_rectangle() {
    let mut c = Checks::new();
    let (a, b) = (0.6, 0.4);
    let x = vec![[0.0, 0.0, 0.0], [a, 0.0, 0.0], [a, b, 0.0], [0.0, b, 0.0]];
    let g: Vec<f64> = x.iter().map(|q| 100.0 + 300.0 * q[0] - 150.0 * q[1]).collect();
    let (gc, c1, c2) = (100.0 + 300.0 * 0.5 * a - 150.0 * 0.5 * b, 300.0, -150.0);
    for (name, spec) in [("UNS", UNS), ("SYM", SYM), ("ANG", ANG)] {
        let l = hand_laminate(spec);
        let mut m = model(
            x.clone(),
            ElementKind::Shell4,
            vec![0, 1, 2, 3],
            BlockProperty::layup(plies_of(spec)).unwrap(),
            &[],
        );
        m.add_load(Load::shell_temperature_gradient(g.clone())).unwrap();
        let f = nodal_load_vector(&m, &[]).unwrap();
        let dofs = m.dof_map();
        let (n, mm) = (l.mth, l.pth);
        let mut exact = vec![0.0; dofs.n_equations()];
        for (i, cr) in CORNERS.iter().enumerate() {
            let (ai, ci) = (cr[0], cr[1]);
            let ix = ai * (b / 8.0) * (4.0 * gc + (2.0 / 3.0) * c2 * b * ci);
            let iy = ci * (a / 8.0) * (4.0 * gc + (2.0 / 3.0) * c1 * a * ai);
            exact[dofs.equation(i, 0).unwrap()] = n[0] * ix + n[2] * iy;
            exact[dofs.equation(i, 1).unwrap()] = n[1] * iy + n[2] * ix;
            exact[dofs.equation(i, 4).unwrap()] = mm[0] * ix + mm[2] * iy;
            exact[dofs.equation(i, 3).unwrap()] = -(mm[1] * iy + mm[2] * ix);
        }
        let fmax = max_abs(exact.iter().copied());
        let d = max_abs(f.iter().zip(&exact).map(|(x, y)| x - y));
        c.at_most(
            &format!("L2 {name} load against arithmetic 2, / max |f|"),
            d / fmax,
            1e-12,
        );
        let zero = (0..4).all(|i| [2, 5].iter().all(|&k| f[dofs.equation(i, k).unwrap()] == 0.0));
        c.holds(&format!("L2 {name} u_z and theta_z exactly zero"), zero, "");
        if name == "SYM" {
            let fs = max_abs((0..4).flat_map(|i| [f[dofs.equation(i, 0).unwrap()], f[dofs.equation(i, 1).unwrap()]]));
            c.at_most(
                "L2 SYM force entries / (N_s 0.24)",
                fs / (l.force_scale(gc) * a * b),
                1e-12,
            );
        }
    }
    c.finish();
}

/// The corrected test of ADR 0626 amendment 2: the same model without the empty `held` set.
#[test]
fn l2_layup_load_on_a_rectangle_after_amendment2() {
    let mut c = Checks::new();
    let (a, b) = (0.6, 0.4);
    let x = vec![[0.0, 0.0, 0.0], [a, 0.0, 0.0], [a, b, 0.0], [0.0, b, 0.0]];
    let g: Vec<f64> = x.iter().map(|q| 100.0 + 300.0 * q[0] - 150.0 * q[1]).collect();
    let (gc, c1, c2) = (100.0 + 300.0 * 0.5 * a - 150.0 * 0.5 * b, 300.0, -150.0);
    for (name, spec) in [("UNS", UNS), ("SYM", SYM), ("ANG", ANG)] {
        let l = hand_laminate(spec);
        let mut m = free_model(
            x.clone(),
            ElementKind::Shell4,
            vec![0, 1, 2, 3],
            BlockProperty::layup(plies_of(spec)).unwrap(),
            &[],
        );
        m.add_load(Load::shell_temperature_gradient(g.clone())).unwrap();
        let f = nodal_load_vector(&m, &[]).unwrap();
        let dofs = m.dof_map();
        let (n, mm) = (l.mth, l.pth);
        let mut exact = vec![0.0; dofs.n_equations()];
        for (i, cr) in CORNERS.iter().enumerate() {
            let (ai, ci) = (cr[0], cr[1]);
            let ix = ai * (b / 8.0) * (4.0 * gc + (2.0 / 3.0) * c2 * b * ci);
            let iy = ci * (a / 8.0) * (4.0 * gc + (2.0 / 3.0) * c1 * a * ai);
            exact[dofs.equation(i, 0).unwrap()] = n[0] * ix + n[2] * iy;
            exact[dofs.equation(i, 1).unwrap()] = n[1] * iy + n[2] * ix;
            exact[dofs.equation(i, 4).unwrap()] = mm[0] * ix + mm[2] * iy;
            exact[dofs.equation(i, 3).unwrap()] = -(mm[1] * iy + mm[2] * ix);
        }
        let fmax = max_abs(exact.iter().copied());
        let d = max_abs(f.iter().zip(&exact).map(|(x, y)| x - y));
        c.at_most(
            &format!("L2 {name} load against arithmetic 2, / max |f|"),
            d / fmax,
            1e-12,
        );
        let zero = (0..4).all(|i| [2, 5].iter().all(|&k| f[dofs.equation(i, k).unwrap()] == 0.0));
        c.holds(&format!("L2 {name} u_z and theta_z exactly zero"), zero, "");
        if name == "SYM" {
            let fs = max_abs((0..4).flat_map(|i| [f[dofs.equation(i, 0).unwrap()], f[dofs.equation(i, 1).unwrap()]]));
            c.at_most(
                "L2 SYM force entries / (N_s 0.24)",
                fs / (l.force_scale(gc) * a * b),
                1e-12,
            );
        }
    }
    c.finish();
}

// -------------------------------------------------------------------------------------------------------------- L3

#[test]
fn l3_one_isotropic_ply_is_the_shell_block() {
    let mut c = Checks::new();
    let (p, el) = th_plane();
    let place = Placement::inclined();
    let conn: Vec<usize> = el.iter().flatten().copied().collect();
    let t = 0.1;
    let mut out = Vec::new();
    for property in [
        BlockProperty::shell(t).unwrap(),
        BlockProperty::layup(vec![Ply::new(steel(), t, 0.0).unwrap()]).unwrap(),
    ] {
        let mut m = model(placed(&p, &place), ElementKind::Shell4, conn.clone(), property, &[0]);
        m.add_load(uniform(p.len())).unwrap();
        let f = nodal_load_vector(&m, &[]).unwrap();
        let u = solve_static(&m, &StaticOptions::default())
            .unwrap()
            .displacement_values()
            .to_vec();
        let mut s = Vec::new();
        for (_, r) in shell::stresses(&m, &u).unwrap() {
            s.extend(r.axes.iter().flatten());
            s.extend(r.membrane);
            s.extend(r.bending);
            s.extend(r.shear);
            s.extend(r.top);
            s.extend(r.bottom);
        }
        out.push((f, u, s));
    }
    c.holds(
        "L3 one steel ply: load bitwise the Shell block's",
        bits_equal(&out[0].0, &out[1].0),
        "",
    );
    c.holds(
        "L3 one steel ply: displacement bitwise",
        bits_equal(&out[0].1, &out[1].1),
        "",
    );
    c.holds(
        "L3 one steel ply: stresses bitwise",
        bits_equal(&out[0].2, &out[1].2),
        "",
    );
    c.finish();
}

// -------------------------------------------------------------------------------------------------------------- C1

#[test]
fn c1_clamped_shell3_plate() {
    let mut c = Checks::new();
    let t = 0.1;
    for (name, place) in [("P", Placement::identity()), ("Q", Placement::inclined())] {
        let (mut m, p) = shell3_plate(&place, t, &boundary_nodes());
        m.add_load(uniform(p.len())).unwrap();
        let u = solve_static(&m, &StaticOptions::default())
            .unwrap()
            .displacement_values()
            .to_vec();
        let st = shell::stresses(&m, &u).unwrap();
        let (ms, ss) = (steel_moment(t, GRAD), steel_surface(t, GRAD));
        let dm = worst(&st, |s| max_abs([s.bending[0] + ms, s.bending[1] + ms, s.bending[2]]));
        let ds = worst(&st, |s| {
            max_abs([
                s.top[0] + ss,
                s.top[1] + ss,
                s.top[2],
                s.bottom[0] - ss,
                s.bottom[1] - ss,
                s.bottom[2],
            ])
        });
        let dn = worst(&st, |s| max_abs(s.membrane));
        c.at_most(
            &format!("C1 {name} centroids |M - (-M_s, -M_s, 0)| / M_s"),
            dm / ms,
            1e-12,
        );
        c.at_most(&format!("C1 {name} faces against -+s_s, / s_s"), ds / ss, 1e-12);
        c.at_most(&format!("C1 {name} |N| / (s_s t)"), dn / (ss * t), 1e-12);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------ C2, C3, N11

fn clamped_layup_rows(c: &mut Checks, name: &str, m: &SolidModel, l: &HLam) {
    let u = solve_static(m, &StaticOptions::default())
        .unwrap()
        .displacement_values()
        .to_vec();
    let st = shell::stresses(m, &u).unwrap();
    let (ms, ns, fs) = (l.moment_scale(GRAD), l.force_scale(GRAD), l.face_scale(GRAD));
    let dm = worst(&st, |s| max_abs((0..3).map(|k| s.bending[k] + l.pth[k] * GRAD)));
    let dn = worst(&st, |s| max_abs((0..3).map(|k| s.membrane[k] + l.mth[k] * GRAD)));
    let (top, bot) = (&l.plies[l.plies.len() - 1], &l.plies[0]);
    let h = 0.5 * l.t * GRAD;
    let df = worst(&st, |s| {
        max_abs((0..3).flat_map(|k| [s.top[k] + top.qa[k] * h, s.bottom[k] - bot.qa[k] * h]))
    });
    c.at_most(&format!("{name} centres |M - (-P_th g)| / M_s"), dm / ms, 1e-12);
    c.at_most(&format!("{name} centres |N - (-M_th g)| / N_s"), dn / ns, 1e-12);
    c.at_most(&format!("{name} faces against arithmetic 5, / S_s"), df / fs, 1e-12);
    let mut dp = 0.0f64;
    for r in layup_response(m, &u).unwrap() {
        for ps in &r.plies {
            let hp = &l.plies[ps.ply];
            let qa = mv(&hp.q, [hp.a[0], hp.a[1], 0.0]);
            for s in 0..2 {
                let z = l.z[ps.ply + s];
                dp = dp.max(max_abs((0..3).map(|k| ps.stress[s][k] + qa[k] * z * GRAD)));
            }
        }
    }
    c.at_most(
        &format!("{name} ply stresses against -Q (alpha_1, alpha_2, 0) z g, / S_s"),
        dp / fs,
        1e-12,
    );
    if name == "C2 UNS" {
        let nmax = worst(&st, |s| max_abs(s.membrane));
        c.above("N11 UNS clamped max |N| / N_s (the coupling force)", nmax / ns, 0.1);
    }
}

#[test]
fn c2_c3_n11_clamped_layup_plates() {
    let mut c = Checks::new();
    let (p, el) = rg_plane();
    for (name, spec) in [("C2 SYM", SYM), ("C2 UNS", UNS)] {
        let mut m = layup_plate(&p, &el, &Placement::inclined(), spec, &boundary_nodes());
        m.add_load(uniform(p.len())).unwrap();
        clamped_layup_rows(&mut c, name, &m, &hand_laminate(spec));
    }
    let (p, el) = th_plane();
    let mut m = layup_plate(&p, &el, &Placement::inclined(), BIM, &boundary_nodes());
    m.add_load(uniform(p.len())).unwrap();
    clamped_layup_rows(&mut c, "C3 BIM", &m, &hand_laminate(BIM));
    c.finish();
}

// -------------------------------------------------------------------------------------------------------------- F1

#[test]
fn f1_free_shell3_plate() {
    let mut c = Checks::new();
    let t = 0.1;
    for (name, place) in [("P", Placement::identity()), ("Q", Placement::inclined())] {
        let (mut m, p) = shell3_plate(&place, t, &[0]);
        m.add_load(uniform(p.len())).unwrap();
        let u = solve_static(&m, &StaticOptions::default())
            .unwrap()
            .displacement_values()
            .to_vec();
        let st = shell::stresses(&m, &u).unwrap();
        let (ms, ss) = (steel_moment(t, GRAD), steel_surface(t, GRAD));
        c.at_most(
            &format!("F1 {name} max |M| / M_s"),
            worst(&st, |s| max_abs(s.bending)) / ms,
            1e-11,
        );
        c.at_most(
            &format!("F1 {name} max |N| / (s_s t)"),
            worst(&st, |s| max_abs(s.membrane)) / (ss * t),
            1e-11,
        );
    }
    c.finish();
}

// -------------------------------------------------------------------------------------------------------------- F2

#[test]
fn f2_free_bimetal_plate() {
    let mut c = Checks::new();
    let l = hand_laminate(BIM);
    let (eps0, kappa) = l.free(GRAD);
    let (ek, kk) = (max_abs(eps0), max_abs(kappa));
    for (name, place) in [("P", Placement::identity()), ("Q", Placement::inclined())] {
        let (p, el) = th_plane();
        let mut m = layup_plate(&p, &el, &place, BIM, &[0]);
        m.add_load(uniform(p.len())).unwrap();
        let u = solve_static(&m, &StaticOptions::default())
            .unwrap()
            .displacement_values()
            .to_vec();
        let (up, beta) = plane_fields(&m, &u, &place, p.len());
        let mut dk = 0.0f64;
        let mut de = 0.0f64;
        for e in &el {
            let xe = element_values(&p, e);
            let s = strain_at(&xe, &element_values(&beta, e), 0.0, 0.0);
            dk = dk.max(max_abs((0..3).map(|k| s[k] - kappa[k])));
            let s = strain_at(&xe, &element_values(&up, e), 0.0, 0.0);
            de = de.max(max_abs((0..3).map(|k| s[k] - eps0[k])));
        }
        c.at_most(
            &format!("F2 {name} curvature against the closed form, / |kappa|"),
            dk / kk,
            1e-11,
        );
        c.at_most(
            &format!("F2 {name} membrane strain against the closed form, / |eps0|"),
            de / ek,
            1e-11,
        );
        let st = shell::stresses(&m, &u).unwrap();
        c.at_most(
            &format!("F2 {name} max |M| / M_s"),
            worst(&st, |s| max_abs(s.bending)) / l.moment_scale(GRAD),
            1e-11,
        );
        c.at_most(
            &format!("F2 {name} max |N| / N_s"),
            worst(&st, |s| max_abs(s.membrane)) / l.force_scale(GRAD),
            1e-11,
        );
    }
    println!("F2 closed form: eps0 {eps0:?}, kappa {kappa:?}");
    c.finish();
}

// -------------------------------------------------------------------------------------------------------------- F3

#[test]
fn f3_free_cross_ply_plate() {
    let mut c = Checks::new();
    let l = hand_laminate(UNS);
    let (eps0, kappa) = l.free(GRAD);
    let (ek, kk) = (max_abs(eps0), max_abs(kappa));
    let (p, el) = rg_plane();
    let mut m = layup_plate(&p, &el, &Placement::inclined(), UNS, &[0]);
    m.add_load(uniform(p.len())).unwrap();
    let u = solve_static(&m, &StaticOptions::default())
        .unwrap()
        .displacement_values()
        .to_vec();
    let resp = layup_response(&m, &u).unwrap();
    let de = resp
        .iter()
        .map(|r| max_abs((0..3).map(|k| r.strain[k] - eps0[k])))
        .fold(0.0, f64::max);
    let dk = resp
        .iter()
        .map(|r| max_abs((0..3).map(|k| r.curvature[k] - kappa[k])))
        .fold(0.0, f64::max);
    c.at_most("F3 UNS centre strain against the closed form, / |eps0|", de / ek, 1e-10);
    c.at_most(
        "F3 UNS centre curvature against the closed form, / |kappa|",
        dk / kk,
        1e-10,
    );
    let st = shell::stresses(&m, &u).unwrap();
    c.at_most(
        "F3 UNS max |M| / M_s",
        worst(&st, |s| max_abs(s.bending)) / l.moment_scale(GRAD),
        1e-10,
    );
    c.at_most(
        "F3 UNS max |N| / N_s",
        worst(&st, |s| max_abs(s.membrane)) / l.force_scale(GRAD),
        1e-10,
    );
    println!("F3 closed form: eps0 {eps0:?}, kappa {kappa:?}");
    c.finish();
}

// -------------------------------------------------------------------------------------------------------------- X1

#[test]
fn x1_exact_discrete_fields() {
    let mut c = Checks::new();
    let t = 0.1;
    let k = ALPHA * GRAD;
    for (name, place) in [("P", Placement::identity()), ("Q", Placement::inclined())] {
        let (mut m, p) = shell3_plate(&place, t, &[0]);
        m.add_load(uniform(p.len())).unwrap();
        let u = exact_field(&m, &p, &place, [0.0; 3], [k, k, 0.0]);
        let st = shell::stresses(&m, &u).unwrap();
        c.at_most(
            &format!("X1 TH3 {name} max |M| / M_s"),
            worst(&st, |s| max_abs(s.bending)) / steel_moment(t, GRAD),
            1e-13,
        );
        c.at_most(
            &format!("X1 TH3 {name} max |N| / (s_s t)"),
            worst(&st, |s| max_abs(s.membrane)) / (steel_surface(t, GRAD) * t),
            1e-13,
        );
    }
    for (name, spec, rg) in [("TH Q BIM", BIM, false), ("RG Q UNS", UNS, true)] {
        let place = Placement::inclined();
        let (p, el) = if rg { rg_plane() } else { th_plane() };
        let l = hand_laminate(spec);
        let (eps0, kappa) = l.free(GRAD);
        let mut m = layup_plate(&p, &el, &place, spec, &[0]);
        m.add_load(uniform(p.len())).unwrap();
        let u = exact_field(&m, &p, &place, eps0, kappa);
        let st = shell::stresses(&m, &u).unwrap();
        c.at_most(
            &format!("X1 {name} max |M| / M_s"),
            worst(&st, |s| max_abs(s.bending)) / l.moment_scale(GRAD),
            1e-13,
        );
        c.at_most(
            &format!("X1 {name} max |N| / N_s"),
            worst(&st, |s| max_abs(s.membrane)) / l.force_scale(GRAD),
            1e-13,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------- N1' (ADR 0612 N3 to N8), N9 to N12

fn is_gradient_refusal(r: &Result<impl std::fmt::Debug, FemError>, kind: &str) -> bool {
    matches!(r, Err(FemError::Element { source, .. })
        if matches!(source.as_ref(), FemError::NotYetSupported { item, variant, package, .. }
            if *item == "shell temperature gradient" && *variant == kind && *package == GRADIENT_FOLLOW_UP))
}

/// The ADR 0612 TH plate of Shell4 (steel, a Shell block), node 0 clamped.
fn steel_plate(t: f64) -> SolidModel {
    let (p, el) = th_plane();
    let conn: Vec<usize> = el.iter().flatten().copied().collect();
    model(
        placed(&p, &Placement::identity()),
        ElementKind::Shell4,
        conn,
        BlockProperty::shell(t).unwrap(),
        &[0],
    )
}

/// The rows N3 to N8 of ADR 0612 `n1_to_n8_refusals_and_zero_fields`, unchanged (ADR 0626 preamble).
#[test]
fn n3_to_n8_after_adr_0626() {
    let mut c = Checks::new();
    let n = 20;
    // N3: two gradient loads.
    let mut m = steel_plate(0.1);
    m.add_load(uniform(n)).unwrap();
    m.add_load(uniform(n)).unwrap();
    let r = nodal_load_vector(&m, &[]);
    c.holds(
        "N3 two gradient loads refused",
        matches!(&r, Err(FemError::InvalidValue { key: "gradient", .. })),
        &format!("{:?}", r.err()),
    );
    // N4: length and finiteness.
    let mut m = steel_plate(0.1);
    let r = m.add_load(Load::shell_temperature_gradient(vec![GRAD; n - 1]));
    c.holds(
        "N4 n - 1 values refused",
        matches!(&r, Err(FemError::FieldLength { key: "gradient", .. })),
        &format!("{:?}", r.err()),
    );
    let mut v = vec![GRAD; n];
    v[3] = f64::NAN;
    let r = m.add_load(Load::shell_temperature_gradient(v));
    c.holds(
        "N4 a NaN refused",
        matches!(&r, Err(FemError::InvalidValue { key: "gradient", .. })),
        &format!("{:?}", r.err()),
    );
    // N5: a zero gradient.
    let mut m = steel_plate(0.1);
    m.add_load(Load::shell_temperature_gradient(vec![0.0; n])).unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let r = solve_static(&m, &StaticOptions::default()).unwrap();
    c.holds(
        "N5 zero gradient: zero load and zero displacement",
        f.iter().all(|&v| v == 0.0) && r.displacement_values().iter().all(|&v| v == 0.0),
        &format!("max |f| {:e}", max_abs(f.iter().copied())),
    );
    // N6: a Hex8 box.
    let mesh = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let nh = mesh.nodes().len();
    let mut plain = SolidModel::new(mesh.clone());
    plain.add_material("all", steel()).unwrap();
    let mut zero = SolidModel::new(mesh.clone());
    zero.add_material("all", steel()).unwrap();
    zero.add_load(Load::shell_temperature_gradient(vec![0.0; nh])).unwrap();
    let a = nodal_load_vector(&plain, &[]).unwrap();
    let b = nodal_load_vector(&zero, &[]).unwrap();
    c.holds(
        "N6 Hex8 with a zero gradient: the bits of the box without it",
        bits_equal(&a, &b),
        "",
    );
    let mut one = SolidModel::new(mesh);
    one.add_material("all", steel()).unwrap();
    let mut v = vec![0.0; nh];
    v[nh - 1] = 10.0;
    one.add_load(Load::shell_temperature_gradient(v)).unwrap();
    let r = nodal_load_vector(&one, &[]);
    c.holds(
        "N6 Hex8 with a non-zero value refused",
        is_gradient_refusal(&r, "Hex8"),
        &format!("{:?}", r.err()),
    );
    // N7 is a row of the ADR 0612 free-plate test (unchanged, running there).
    // N8: the nonlinear static solve.
    let mut m = steel_plate(0.1);
    m.add_load(uniform(n)).unwrap();
    let r = nonlinear::solve(&m, &[], &[], &NewtonOptions::new(1));
    c.holds(
        "N8 nonlinear static solve refused",
        matches!(&r, Err(FemError::InvalidValue { key: "nonlinear", reason }) if reason.contains("gradient")),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}

/// The RG plate (identity placement, steel Shell block `t = 0.1`), optionally with a Beam2 stiffener on its first row.
fn stiffened(with_beam: bool) -> SolidModel {
    let (p, el) = rg_plane();
    let mut mesh = SolidMesh::new(placed(&p, &Placement::identity())).unwrap();
    mesh.add_block(ElementKind::Shell4, el.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.1).unwrap()).unwrap();
    let mut ne = el.len();
    if with_beam {
        mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2, 2, 3, 3, 4])
            .unwrap();
        let section = BeamSection::new(0.005, 4.0e-6, 1.0e-6, 2.0e-6, 0.85, 0.85).unwrap();
        mesh.set_block_property(1, BlockProperty::beam(section, [0.0, 0.0, 1.0]).unwrap())
            .unwrap();
        ne += 4;
    }
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m
}

#[test]
fn n9_to_n12_refusals_kept() {
    let mut c = Checks::new();
    // N9: a non-zero gradient on the stiffener's nodes.
    let mut m = stiffened(true);
    m.add_load(uniform(20)).unwrap();
    let r = nodal_load_vector(&m, &[]);
    c.holds(
        "N9 Beam2 load refused",
        is_gradient_refusal(&r, "Beam2"),
        &format!("{:?}", r.err()),
    );
    let u = vec![0.0; m.dof_map().n_equations()];
    let r = shell::stresses(&m, &u);
    c.holds(
        "N9 Beam2 stresses refused",
        is_gradient_refusal(&r, "Beam2"),
        &format!("{:?}", r.err()),
    );
    // N10 (amendment 1): zero on the stiffener's nodes: the load of the shell model without the beam, bitwise.
    let g: Vec<f64> = (0..20).map(|n| if n < 5 { 0.0 } else { GRAD }).collect();
    let mut m = stiffened(true);
    m.add_load(Load::shell_temperature_gradient(g.clone())).unwrap();
    let mut s = stiffened(false);
    s.add_load(Load::shell_temperature_gradient(g)).unwrap();
    let fm = nodal_load_vector(&m, &[]);
    let fs = nodal_load_vector(&s, &[]).unwrap();
    c.holds(
        "N10 zero on the beam: the load of the shell model, bitwise",
        fm.as_ref().is_ok_and(|f| bits_equal(f, &fs)),
        &format!("{:?}", fm.as_ref().err()),
    );
    // N12: the nonlinear static solve (amendment 1: the layup row any refusal).
    let (p, el) = th_plane();
    let mut m = layup_plate(&p, &el, &Placement::identity(), BIM, &[0]);
    m.add_load(uniform(p.len())).unwrap();
    let r = nonlinear::solve(&m, &[], &[], &NewtonOptions::new(1));
    c.holds(
        "N12 BIM nonlinear solve refused",
        r.is_err(),
        &format!("{:?}", r.as_ref().err()),
    );
    let (mut m, p) = shell3_plate(&Placement::identity(), 0.1, &[0]);
    m.add_load(uniform(p.len())).unwrap();
    let r = nonlinear::solve(&m, &[], &[], &NewtonOptions::new(1));
    c.holds(
        "N12 TH3 nonlinear solve refused naming the gradient",
        matches!(&r, Err(FemError::InvalidValue { key: "nonlinear", reason }) if reason.contains("gradient")),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}
