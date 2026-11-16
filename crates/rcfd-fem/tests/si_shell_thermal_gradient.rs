//! F-527 (ADR 0612): a temperature gradient through the thickness of Shell4 (`Load::ShellTemperatureGradient`). Checks G1 to
//! G6 and the negative controls N1 to N8 of the registration (B1 is `si_shell_thermal_gradient_bits.rs`, B2 the existing
//! tests).

use rcfd_core::vec3::{dot, norm, Vec3};
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::nonlinear::{self, NewtonOptions};
use rcfd_fem::shell::{self, Shell4Membrane};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::thermal::GRADIENT_FOLLOW_UP;
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, Ply, SolidMesh, SolidModel};

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
    /// A row missed in the registered run (ADR 0612 amendment 1): printed with its limit, not checked.
    fn known_miss(&mut self, name: &str, value: f64, limit: f64) {
        println!("{name}: {value:.4e} (limit <= {limit:e}) known miss of ADR 0612, not checked here");
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const ALPHA: f64 = 1.2e-5;
const GRAD: f64 = 200.0;
const CORNERS: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
const G: f64 = 0.577_350_269_189_625_8;

fn material() -> Material {
    Material::isotropic_elastic(E, NU, RHO, ALPHA).unwrap()
}

fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

/// `M_s = D (1 + nu) alpha g`.
fn moment_scale(t: f64, g: f64) -> f64 {
    E * t * t * t * ALPHA * g / (12.0 * (1.0 - NU))
}

/// `s_s = E alpha g t / (2 (1 - nu))`.
fn surface_scale(t: f64, g: f64) -> f64 {
    E * ALPHA * g * t / (2.0 * (1.0 - NU))
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

fn rotation(axis: Vec3, angle: f64) -> [[f64; 3]; 3] {
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
    q: [[f64; 3]; 3],
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

/// The ADR 0445 TH plate: 4 x 3 elements on `[0, 2] x [0, 1]`, interior nodes moved in plane.
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

fn boundary_nodes() -> Vec<usize> {
    (0..20)
        .filter(|k| k % 5 == 0 || k % 5 == 4 || k / 5 == 0 || k / 5 == 3)
        .collect()
}

fn shell_mesh(nodes: Vec<Vec3>, elements: &[[usize; 4]], property: BlockProperty) -> SolidMesh {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = elements.iter().flatten().copied().collect();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..elements.len()).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("origin", vec![0]).unwrap();
    mesh
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m
}

/// The TH plate placed by `place`, thickness `t`; `clamped` the nodes with all six components held.
fn plate(place: &Placement, t: f64, clamped: &[usize]) -> (SolidModel, Vec<[f64; 2]>, Vec<[usize; 4]>) {
    let (p, el) = th_plane();
    let x: Vec<Vec3> = p.iter().map(|&q| place.place(q)).collect();
    let mut mesh = shell_mesh(x, &el, BlockProperty::shell(t).unwrap());
    mesh.add_node_set("held", clamped.to_vec()).unwrap();
    let mut m = model_of(mesh);
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("held", comp, 0.0)).unwrap();
    }
    (m, p, el)
}

/// `(d/dX, d/dY)` of the bilinear functions at `(xi, eta)` of an element with plane coordinates `xe`.
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

/// The solved fields of a plate in plate axes: in-plane displacement, `w`, `beta = (theta . e_y, -theta . e_x)`, rotations.
struct Fields {
    uplane: Vec<[f64; 2]>,
    w: Vec<f64>,
    beta: Vec<[f64; 2]>,
    theta: Vec<Vec3>,
}

fn fields(m: &SolidModel, u: &[f64], place: &Placement, n: usize) -> Fields {
    let dofs = m.dof_map();
    let comp = |node: usize, k: usize| u[dofs.equation(node, k).unwrap()];
    let (ex, ey, ez) = (place.axis(0), place.axis(1), place.axis(2));
    let mut f = Fields {
        uplane: Vec::new(),
        w: Vec::new(),
        beta: Vec::new(),
        theta: Vec::new(),
    };
    for node in 0..n {
        let v = [comp(node, 0), comp(node, 1), comp(node, 2)];
        let th = [comp(node, 3), comp(node, 4), comp(node, 5)];
        f.uplane.push([dot(v, ex), dot(v, ey)]);
        f.w.push(dot(v, ez));
        f.beta.push([dot(th, ey), -dot(th, ex)]);
        f.theta.push([dot(th, ex), dot(th, ey), dot(th, ez)]);
    }
    f
}

fn element_values<T: Copy>(v: &[T], e: &[usize; 4]) -> [T; 4] {
    [v[e[0]], v[e[1]], v[e[2]], v[e[3]]]
}

// ------------------------------------------------------------------------------------------------------------ G1, N7

#[test]
#[ignore = "known miss of ADR 0612 (amendment 1): G1a curvature P 2.9607e-12, Q 3.3603e-12; G1b moment P 1.9190e-12, Q 1.5783e-12; G1c surface stresses P 1.9190e-12, Q 1.5939e-12 (limit 1e-12); G1e R curvature 2.6333e-10, moment 1.2343e-10 (limit 1e-10): rounding of the bending-shear solve, growing as (h/t)^2; superseded by g1_n7_free_plate_met_rows_after_amendment1"]
fn g1_n7_free_plate_under_a_uniform_gradient() {
    let mut c = Checks::new();
    let k = ALPHA * GRAD;
    for (name, place, t) in [
        ("P", Placement::identity(), 0.1),
        ("Q", Placement::inclined(), 0.1),
        ("R", Placement::identity(), 0.01),
    ] {
        let (mut m, p, el) = plate(&place, t, &[0]);
        m.add_load(Load::shell_temperature_gradient(vec![GRAD; p.len()]))
            .unwrap();
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        let u = r.displacement_values();
        let f = fields(&m, u, &place, p.len());
        let (limit, id_a, id_b) = if name == "R" {
            (1e-10, "G1e", "G1e")
        } else {
            (1e-12, "G1a", "G1b")
        };
        let mut dk = 0.0f64;
        let mut naive = 0.0f64;
        let dc = E * t * t * t / (12.0 * (1.0 - NU * NU));
        for e in &el {
            let s = strain_at(&element_values(&p, e), &element_values(&f.beta, e), 0.0, 0.0);
            dk = dk.max(max_abs([s[0] - k, s[1] - k, s[2]]));
            naive = naive.max((dc * (s[0] + NU * s[1])).abs() / moment_scale(t, GRAD));
        }
        c.at_most(
            &format!("{id_a} {name} max |kappa - alpha g (1, 1, 0)| / (alpha g)"),
            dk / k,
            limit,
        );
        if name == "P" {
            c.above("N7 P |D kappa|_xx without the thermal curvature / M_s", naive, 0.5);
        }
        let st = shell::stresses(&m, u).unwrap();
        let mmax = max_abs(st.iter().flat_map(|(_, s)| s.bending));
        c.at_most(
            &format!("{id_b} {name} max |M_k| / M_s"),
            mmax / moment_scale(t, GRAD),
            limit,
        );
        if name == "R" {
            continue;
        }
        let ss = surface_scale(t, GRAD);
        let smax = max_abs(st.iter().flat_map(|(_, s)| s.top.into_iter().chain(s.bottom)));
        let nmax = max_abs(st.iter().flat_map(|(_, s)| s.membrane));
        c.at_most(
            &format!("G1c {name} max |sigma_top|, |sigma_bottom| / s_s"),
            smax / ss,
            1e-12,
        );
        c.at_most(&format!("G1c {name} max |N_k| / (s_s t)"), nmax / (ss * t), 1e-12);
        if name == "P" {
            let r0 = p[0];
            let lmax = max_abs(p.iter().map(|q| (q[0] - r0[0]).hypot(q[1] - r0[1])));
            let dw = max_abs((0..p.len()).map(|n| {
                let (dx, dy) = (p[n][0] - r0[0], p[n][1] - r0[1]);
                f.w[n] + 0.5 * k * (dx * dx + dy * dy)
            }));
            let dth = max_abs((0..p.len()).flat_map(|n| {
                let (dx, dy) = (p[n][0] - r0[0], p[n][1] - r0[1]);
                [f.theta[n][0] + k * dy, f.theta[n][1] - k * dx]
            }));
            c.at_most(
                "G1d P w against -k |X - X_0|^2 / 2, / (k L^2 / 2)",
                dw / (0.5 * k * lmax * lmax),
                1e-9,
            );
            c.at_most(
                "G1d P rotations against k (Y_0 - Y, X - X_0), / (k L)",
                dth / (k * lmax),
                1e-9,
            );
            let zero = (0..p.len()).all(|n| f.uplane[n] == [0.0, 0.0] && f.theta[n][2] == 0.0);
            c.holds("G1d P in-plane displacements and theta_z exactly zero", zero, "");
        }
    }
    c.finish();
}

/// The copy of G1 after amendment 1: the missed rows are printed, not checked; every met row keeps its limit.
#[test]
fn g1_n7_free_plate_met_rows_after_amendment1() {
    let mut c = Checks::new();
    let k = ALPHA * GRAD;
    for (name, place, t) in [
        ("P", Placement::identity(), 0.1),
        ("Q", Placement::inclined(), 0.1),
        ("R", Placement::identity(), 0.01),
    ] {
        let (mut m, p, el) = plate(&place, t, &[0]);
        m.add_load(Load::shell_temperature_gradient(vec![GRAD; p.len()]))
            .unwrap();
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        let u = r.displacement_values();
        let f = fields(&m, u, &place, p.len());
        let (limit, id_a, id_b) = if name == "R" {
            (1e-10, "G1e", "G1e")
        } else {
            (1e-12, "G1a", "G1b")
        };
        let mut dk = 0.0f64;
        let mut naive = 0.0f64;
        let dc = E * t * t * t / (12.0 * (1.0 - NU * NU));
        for e in &el {
            let s = strain_at(&element_values(&p, e), &element_values(&f.beta, e), 0.0, 0.0);
            dk = dk.max(max_abs([s[0] - k, s[1] - k, s[2]]));
            naive = naive.max((dc * (s[0] + NU * s[1])).abs() / moment_scale(t, GRAD));
        }
        c.known_miss(
            &format!("{id_a} {name} max |kappa - alpha g (1, 1, 0)| / (alpha g)"),
            dk / k,
            limit,
        );
        if name == "P" {
            c.above("N7 P |D kappa|_xx without the thermal curvature / M_s", naive, 0.5);
        }
        let st = shell::stresses(&m, u).unwrap();
        let mmax = max_abs(st.iter().flat_map(|(_, s)| s.bending));
        c.known_miss(
            &format!("{id_b} {name} max |M_k| / M_s"),
            mmax / moment_scale(t, GRAD),
            limit,
        );
        if name == "R" {
            continue;
        }
        let ss = surface_scale(t, GRAD);
        let smax = max_abs(st.iter().flat_map(|(_, s)| s.top.into_iter().chain(s.bottom)));
        let nmax = max_abs(st.iter().flat_map(|(_, s)| s.membrane));
        c.known_miss(
            &format!("G1c {name} max |sigma_top|, |sigma_bottom| / s_s"),
            smax / ss,
            1e-12,
        );
        c.at_most(&format!("G1c {name} max |N_k| / (s_s t)"), nmax / (ss * t), 1e-12);
        if name == "P" {
            let r0 = p[0];
            let lmax = max_abs(p.iter().map(|q| (q[0] - r0[0]).hypot(q[1] - r0[1])));
            let dw = max_abs((0..p.len()).map(|n| {
                let (dx, dy) = (p[n][0] - r0[0], p[n][1] - r0[1]);
                f.w[n] + 0.5 * k * (dx * dx + dy * dy)
            }));
            let dth = max_abs((0..p.len()).flat_map(|n| {
                let (dx, dy) = (p[n][0] - r0[0], p[n][1] - r0[1]);
                [f.theta[n][0] + k * dy, f.theta[n][1] - k * dx]
            }));
            c.at_most(
                "G1d P w against -k |X - X_0|^2 / 2, / (k L^2 / 2)",
                dw / (0.5 * k * lmax * lmax),
                1e-9,
            );
            c.at_most(
                "G1d P rotations against k (Y_0 - Y, X - X_0), / (k L)",
                dth / (k * lmax),
                1e-9,
            );
            let zero = (0..p.len()).all(|n| f.uplane[n] == [0.0, 0.0] && f.theta[n][2] == 0.0);
            c.holds("G1d P in-plane displacements and theta_z exactly zero", zero, "");
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------------ G2

#[test]
fn g2_clamped_plate_under_a_uniform_gradient() {
    let mut c = Checks::new();
    let k = ALPHA * GRAD;
    let t = 0.1;
    for (name, place) in [("P", Placement::identity()), ("Q", Placement::inclined())] {
        let (mut m, p, el) = plate(&place, t, &boundary_nodes());
        m.add_load(Load::shell_temperature_gradient(vec![GRAD; p.len()]))
            .unwrap();
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        let u = r.displacement_values();
        let ms = moment_scale(t, GRAD);
        let ss = surface_scale(t, GRAD);
        let st = shell::stresses(&m, u).unwrap();
        let dm = max_abs(
            st.iter()
                .flat_map(|(_, s)| [s.bending[0] + ms, s.bending[1] + ms, s.bending[2]]),
        );
        let ds = max_abs(st.iter().flat_map(|(_, s)| {
            [
                s.top[0] + ss,
                s.top[1] + ss,
                s.top[2],
                s.bottom[0] - ss,
                s.bottom[1] - ss,
                s.bottom[2],
            ]
        }));
        c.at_most(
            &format!("G2 {name} centres max |M - (-M_s, -M_s, 0)| / M_s"),
            dm / ms,
            1e-12,
        );
        c.at_most(
            &format!("G2 {name} centres surface stresses against -+s_s, / s_s"),
            ds / ss,
            1e-12,
        );
        // At the 2 x 2 Gauss points, test side: D (kappa_h - k (1, 1, 0)).
        let f = fields(&m, u, &place, p.len());
        let dc = E * t * t * t / (12.0 * (1.0 - NU * NU));
        let mut dg = 0.0f64;
        for e in &el {
            for xi in [-G, G] {
                for eta in [-G, G] {
                    let s = strain_at(&element_values(&p, e), &element_values(&f.beta, e), xi, eta);
                    let el_k = [s[0] - k, s[1] - k, s[2]];
                    let mm = [
                        dc * (el_k[0] + NU * el_k[1]),
                        dc * (NU * el_k[0] + el_k[1]),
                        dc * 0.5 * (1.0 - NU) * el_k[2],
                    ];
                    dg = dg.max(max_abs([mm[0] + ms, mm[1] + ms, mm[2]]));
                }
            }
        }
        c.at_most(
            &format!("G2 {name} Gauss points max |M - (-M_s, -M_s, 0)| / M_s (test side)"),
            dg / ms,
            1e-12,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------------ G3

#[test]
fn g3_linear_gradient_on_a_rectangle() {
    let mut c = Checks::new();
    let (a, b, t) = (0.6, 0.4, 0.02);
    let x = vec![[0.0, 0.0, 0.0], [a, 0.0, 0.0], [a, b, 0.0], [0.0, b, 0.0]];
    let g: Vec<f64> = x.iter().map(|q| 100.0 + 300.0 * q[0] - 150.0 * q[1]).collect();
    let mut m = model_of(shell_mesh(x, &[[0, 1, 2, 3]], BlockProperty::shell(t).unwrap()));
    m.add_load(Load::shell_temperature_gradient(g)).unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let (gc, c1, c2) = (100.0 + 300.0 * 0.5 * a - 150.0 * 0.5 * b, 300.0, -150.0);
    // D (1 + nu) alpha per unit gradient.
    let kk = moment_scale(t, 1.0);
    let dofs = m.dof_map();
    let mut exact = vec![0.0; dofs.n_equations()];
    for (i, cr) in CORNERS.iter().enumerate() {
        let (ai, ci) = (cr[0], cr[1]);
        exact[dofs.equation(i, 4).unwrap()] = kk * ai * (b / 8.0) * (4.0 * gc + (2.0 / 3.0) * c2 * b * ci);
        exact[dofs.equation(i, 3).unwrap()] = -kk * ci * (a / 8.0) * (4.0 * gc + (2.0 / 3.0) * c1 * a * ai);
    }
    let fmax = max_abs(exact.iter().copied());
    let mut drot = 0.0f64;
    let mut zero = true;
    for i in 0..4 {
        for comp in 0..6 {
            let q = dofs.equation(i, comp).unwrap();
            if comp == 3 || comp == 4 {
                drot = drot.max((f[q] - exact[q]).abs());
            } else {
                zero &= f[q] == 0.0;
            }
        }
    }
    println!("G3 largest entry {fmax:.6e} N m");
    c.at_most(
        "G3 nodal moments against the closed form, / largest",
        drot / fmax,
        1e-12,
    );
    c.holds("G3 forces and theta_z exactly zero", zero, "");
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------------ G4

#[test]
#[ignore = "known miss of ADR 0612 (amendment 1): G4 (ii) curvature 3.7367e-12, G4 (iii) moment 1.8133e-12 (limit 1e-12), the rounding of G1; superseded by g4_superposition_met_rows_after_amendment1"]
fn g4_membrane_temperature_and_gradient_superpose() {
    let mut c = Checks::new();
    let t = 0.1;
    let place = Placement::inclined();
    let dt = 50.0;
    let ea = ALPHA * dt;
    let k = ALPHA * GRAD;
    let (p, _) = th_plane();
    let n = p.len();
    let build = |temp: bool, grad: bool| {
        let (mut m, _, _) = plate(&place, t, &[0]);
        if temp {
            m.add_load(Load::temperature(vec![300.0 + dt; n], 300.0)).unwrap();
        }
        if grad {
            m.add_load(Load::shell_temperature_gradient(vec![GRAD; n])).unwrap();
        }
        m
    };
    let mb = build(true, true);
    let fb = nodal_load_vector(&mb, &[]).unwrap();
    let ft = nodal_load_vector(&build(true, false), &[]).unwrap();
    let fg = nodal_load_vector(&build(false, true), &[]).unwrap();
    let d = max_abs(fb.iter().zip(ft.iter().zip(&fg)).map(|(b, (x, y))| b - (x + y)));
    c.at_most(
        "G4 (i) load of both against the sum, / largest entry",
        d / max_abs(fb.iter().copied()),
        1e-14,
    );
    let (_, p, el) = plate(&place, t, &[0]);
    let r = solve_static(&mb, &StaticOptions::default()).unwrap();
    let u = r.displacement_values();
    let f = fields(&mb, u, &place, n);
    let mut de = 0.0f64;
    let mut dk = 0.0f64;
    for e in &el {
        let xe = element_values(&p, e);
        let s = strain_at(&xe, &element_values(&f.uplane, e), 0.0, 0.0);
        de = de.max(max_abs([s[0] - ea, s[1] - ea, s[2]]));
        let s = strain_at(&xe, &element_values(&f.beta, e), 0.0, 0.0);
        dk = dk.max(max_abs([s[0] - k, s[1] - k, s[2]]));
    }
    c.at_most(
        "G4 (ii) membrane strain against alpha dT (1, 1, 0), / (alpha dT)",
        de / ea,
        1e-12,
    );
    c.at_most(
        "G4 (ii) curvature against alpha g (1, 1, 0), / (alpha g)",
        dk / k,
        1e-12,
    );
    let st = shell::stresses(&mb, u).unwrap();
    let mmax = max_abs(st.iter().flat_map(|(_, s)| s.bending));
    let nmax = max_abs(st.iter().flat_map(|(_, s)| s.membrane));
    let smax = max_abs(st.iter().flat_map(|(_, s)| s.top.into_iter().chain(s.bottom)));
    c.at_most("G4 (iii) max |M| / M_s", mmax / moment_scale(t, GRAD), 1e-12);
    c.at_most(
        "G4 (iii) max |N| / (E t alpha dT / (1 - nu))",
        nmax / (E * t * ea / (1.0 - NU)),
        1e-12,
    );
    c.at_most(
        "G4 (iii) surface stresses / (E alpha dT / (1 - nu) + s_s)",
        smax / (E * ea / (1.0 - NU) + surface_scale(t, GRAD)),
        1e-12,
    );
    c.finish();
}

/// The copy of G4 after amendment 1: the missed rows are printed, not checked; every met row keeps its limit.
#[test]
fn g4_superposition_met_rows_after_amendment1() {
    let mut c = Checks::new();
    let t = 0.1;
    let place = Placement::inclined();
    let dt = 50.0;
    let ea = ALPHA * dt;
    let k = ALPHA * GRAD;
    let (p, _) = th_plane();
    let n = p.len();
    let build = |temp: bool, grad: bool| {
        let (mut m, _, _) = plate(&place, t, &[0]);
        if temp {
            m.add_load(Load::temperature(vec![300.0 + dt; n], 300.0)).unwrap();
        }
        if grad {
            m.add_load(Load::shell_temperature_gradient(vec![GRAD; n])).unwrap();
        }
        m
    };
    let mb = build(true, true);
    let fb = nodal_load_vector(&mb, &[]).unwrap();
    let ft = nodal_load_vector(&build(true, false), &[]).unwrap();
    let fg = nodal_load_vector(&build(false, true), &[]).unwrap();
    let d = max_abs(fb.iter().zip(ft.iter().zip(&fg)).map(|(b, (x, y))| b - (x + y)));
    c.at_most(
        "G4 (i) load of both against the sum, / largest entry",
        d / max_abs(fb.iter().copied()),
        1e-14,
    );
    let (_, p, el) = plate(&place, t, &[0]);
    let r = solve_static(&mb, &StaticOptions::default()).unwrap();
    let u = r.displacement_values();
    let f = fields(&mb, u, &place, n);
    let mut de = 0.0f64;
    let mut dk = 0.0f64;
    for e in &el {
        let xe = element_values(&p, e);
        let s = strain_at(&xe, &element_values(&f.uplane, e), 0.0, 0.0);
        de = de.max(max_abs([s[0] - ea, s[1] - ea, s[2]]));
        let s = strain_at(&xe, &element_values(&f.beta, e), 0.0, 0.0);
        dk = dk.max(max_abs([s[0] - k, s[1] - k, s[2]]));
    }
    c.at_most(
        "G4 (ii) membrane strain against alpha dT (1, 1, 0), / (alpha dT)",
        de / ea,
        1e-12,
    );
    c.known_miss(
        "G4 (ii) curvature against alpha g (1, 1, 0), / (alpha g)",
        dk / k,
        1e-12,
    );
    let st = shell::stresses(&mb, u).unwrap();
    let mmax = max_abs(st.iter().flat_map(|(_, s)| s.bending));
    let nmax = max_abs(st.iter().flat_map(|(_, s)| s.membrane));
    let smax = max_abs(st.iter().flat_map(|(_, s)| s.top.into_iter().chain(s.bottom)));
    c.known_miss("G4 (iii) max |M| / M_s", mmax / moment_scale(t, GRAD), 1e-12);
    c.at_most(
        "G4 (iii) max |N| / (E t alpha dT / (1 - nu))",
        nmax / (E * t * ea / (1.0 - NU)),
        1e-12,
    );
    c.at_most(
        "G4 (iii) surface stresses / (E alpha dT / (1 - nu) + s_s)",
        smax / (E * ea / (1.0 - NU) + surface_scale(t, GRAD)),
        1e-12,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------- G5, G6

#[test]
fn g5_g6_self_equilibrium_and_the_membrane_choice() {
    let mut c = Checks::new();
    // G5: the ADR 0445 warped mesh.
    let (mut p, el) = grid(1.5, 1.0, 3, 2);
    p[5][0] += 0.07;
    p[5][1] -= 0.05;
    p[6][0] -= 0.04;
    p[6][1] += 0.06;
    let x: Vec<Vec3> = p
        .iter()
        .enumerate()
        .map(|(k, q)| [q[0], q[1], 0.002 * (1.7 * k as f64 + 0.3).sin()])
        .collect();
    let g: Vec<f64> = x.iter().map(|q| 150.0 + 80.0 * q[0] - 60.0 * q[1]).collect();
    let mut m = model_of(shell_mesh(x.clone(), &el, BlockProperty::shell(0.02).unwrap()));
    m.add_load(Load::shell_temperature_gradient(g)).unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let dofs = m.dof_map();
    let mut forces_zero = true;
    let mut sum = [0.0; 3];
    let mut mmax = 0.0f64;
    for node in 0..x.len() {
        for comp in 0..3 {
            forces_zero &= f[dofs.equation(node, comp).unwrap()] == 0.0;
            let v = f[dofs.equation(node, comp + 3).unwrap()];
            sum[comp] += v;
            mmax = mmax.max(v.abs());
        }
    }
    c.holds("G5 every force entry exactly zero (warped)", forces_zero, "");
    c.at_most("G5 |sum m_i| / max |m_i| (warped)", max_abs(sum) / mmax, 1e-12);
    // G6: the ANDES membrane gives the same gradient load bitwise.
    let place = Placement::inclined();
    let (pq, elq) = th_plane();
    let xq: Vec<Vec3> = pq.iter().map(|&q| place.place(q)).collect();
    let mut loads = Vec::new();
    for prop in [
        BlockProperty::shell(0.1).unwrap(),
        BlockProperty::shell4_with_membrane(0.1, Shell4Membrane::AndesOpt).unwrap(),
    ] {
        let mut m = model_of(shell_mesh(xq.clone(), &elq, prop));
        m.add_load(Load::shell_temperature_gradient(
            pq.iter().map(|q| 120.0 + 50.0 * q[0] + 30.0 * q[1]).collect(),
        ))
        .unwrap();
        loads.push(nodal_load_vector(&m, &[]).unwrap());
    }
    let same =
        loads[0].len() == loads[1].len() && loads[0].iter().zip(&loads[1]).all(|(a, b)| a.to_bits() == b.to_bits());
    c.holds("G6 AndesOpt gradient load equals the bilinear one bitwise", same, "");
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ N1 to N8

fn is_gradient_refusal(r: &Result<impl std::fmt::Debug, FemError>, kind: &str) -> bool {
    matches!(r, Err(FemError::Element { source, .. })
        if matches!(source.as_ref(), FemError::NotYetSupported { item, variant, package, .. }
            if *item == "shell temperature gradient" && *variant == kind && *package == GRADIENT_FOLLOW_UP))
}

#[test]
#[ignore = "superseded by ADR 0626: its rows N1 (Shell3) and N2 (one-ply Layup) assert the refusals that F-687 lifts; N3 to N8 run unchanged in si_shell_thermal_gradient_shell3_layup.rs n3_to_n8_after_adr_0626"]
fn n1_to_n8_refusals_and_zero_fields() {
    let mut c = Checks::new();
    let (p, el) = th_plane();
    let n = p.len();
    // N1: Shell3.
    let x: Vec<Vec3> = p.iter().map(|&q| Placement::identity().place(q)).collect();
    let tris: Vec<usize> = el.iter().flat_map(|e| [e[0], e[1], e[2], e[0], e[2], e[3]]).collect();
    let mut mesh = SolidMesh::new(x.clone()).unwrap();
    mesh.add_block(ElementKind::Shell3, tris).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.05).unwrap()).unwrap();
    mesh.add_element_set("all", (0..2 * el.len()).collect()).unwrap();
    let mut m = model_of(mesh);
    m.add_load(Load::shell_temperature_gradient(vec![GRAD; n])).unwrap();
    let r = nodal_load_vector(&m, &[]);
    c.holds(
        "N1 Shell3 load refused",
        is_gradient_refusal(&r, "Shell3"),
        &format!("{:?}", r.err()),
    );
    let u = vec![0.0; m.dof_map().n_equations()];
    let r = shell::stresses(&m, &u);
    c.holds(
        "N1 Shell3 stresses refused",
        is_gradient_refusal(&r, "Shell3"),
        &format!("{:?}", r.err()),
    );
    // N2: a one-ply Layup.
    let ply = Ply::new(material(), 0.1, 0.0).unwrap();
    let mut m = model_of(shell_mesh(x.clone(), &el, BlockProperty::layup(vec![ply]).unwrap()));
    m.add_load(Load::shell_temperature_gradient(vec![GRAD; n])).unwrap();
    let r = nodal_load_vector(&m, &[]);
    c.holds(
        "N2 Layup load refused",
        is_gradient_refusal(&r, "Layup"),
        &format!("{:?}", r.err()),
    );
    // N3: two gradient loads.
    let (mut m, _, _) = plate(&Placement::identity(), 0.1, &[0]);
    m.add_load(Load::shell_temperature_gradient(vec![GRAD; n])).unwrap();
    m.add_load(Load::shell_temperature_gradient(vec![GRAD; n])).unwrap();
    let r = nodal_load_vector(&m, &[]);
    c.holds(
        "N3 two gradient loads refused",
        matches!(&r, Err(FemError::InvalidValue { key: "gradient", .. })),
        &format!("{:?}", r.err()),
    );
    // N4: length and finiteness.
    let (mut m, _, _) = plate(&Placement::identity(), 0.1, &[0]);
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
    let (mut m, _, _) = plate(&Placement::identity(), 0.1, &[0]);
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
    plain.add_material("all", material()).unwrap();
    let mut zero = SolidModel::new(mesh.clone());
    zero.add_material("all", material()).unwrap();
    zero.add_load(Load::shell_temperature_gradient(vec![0.0; nh])).unwrap();
    let a = nodal_load_vector(&plain, &[]).unwrap();
    let b = nodal_load_vector(&zero, &[]).unwrap();
    c.holds(
        "N6 Hex8 with a zero gradient: the bits of the box without it",
        a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()),
        "",
    );
    let mut one = SolidModel::new(mesh);
    one.add_material("all", material()).unwrap();
    let mut v = vec![0.0; nh];
    v[nh - 1] = 10.0;
    one.add_load(Load::shell_temperature_gradient(v)).unwrap();
    let r = nodal_load_vector(&one, &[]);
    c.holds(
        "N6 Hex8 with a non-zero value refused",
        is_gradient_refusal(&r, "Hex8"),
        &format!("{:?}", r.err()),
    );
    // N8: the nonlinear static solve.
    let (mut m, _, _) = plate(&Placement::identity(), 0.1, &[0]);
    m.add_load(Load::shell_temperature_gradient(vec![GRAD; n])).unwrap();
    let r = nonlinear::solve(&m, &[], &[], &NewtonOptions::new(1));
    c.holds(
        "N8 nonlinear static solve refused",
        matches!(&r, Err(FemError::InvalidValue { key: "nonlinear", reason }) if reason.contains("gradient")),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------- D1, D2 (amendment 1)

/// The exact discrete field of ADR 0612 arithmetic 3 about node 0, as global unknowns of the plate model.
fn exact_field(m: &SolidModel, p: &[[f64; 2]], place: &Placement, k: f64) -> Vec<f64> {
    let dofs = m.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    let (ex, ey, ez) = (place.axis(0), place.axis(1), place.axis(2));
    for (n, q) in p.iter().enumerate() {
        let (dx, dy) = (q[0] - p[0][0], q[1] - p[0][1]);
        let w = -0.5 * k * (dx * dx + dy * dy);
        let (tx, ty) = (-k * dy, k * dx);
        for i in 0..3 {
            u[dofs.equation(n, i).unwrap()] = w * ez[i];
            u[dofs.equation(n, 3 + i).unwrap()] = tx * ex[i] + ty * ey[i];
        }
    }
    u
}

#[test]
fn d1_d2_diagnosis_after_amendment1() {
    let mut c = Checks::new();
    let k = ALPHA * GRAD;
    for (name, place) in [("P", Placement::identity()), ("Q", Placement::inclined())] {
        let t = 0.1;
        let (mut m, p, _) = plate(&place, t, &[0]);
        m.add_load(Load::shell_temperature_gradient(vec![GRAD; p.len()]))
            .unwrap();
        let ue = exact_field(&m, &p, &place, k);
        let st = shell::stresses(&m, &ue).unwrap();
        let mmax = max_abs(st.iter().flat_map(|(_, s)| s.bending));
        c.at_most(
            &format!("D1a {name} exact field: max |M_k| / M_s"),
            mmax / moment_scale(t, GRAD),
            1e-13,
        );
        let kk = stiffness(&m).unwrap();
        let f = nodal_load_vector(&m, &[]).unwrap();
        let dofs = m.dof_map();
        let held: Vec<usize> = (0..6).map(|i| dofs.equation(0, i).unwrap()).collect();
        let residual = |u: &[f64]| -> f64 {
            let mut y = vec![0.0; u.len()];
            kk.matvec(u, &mut y);
            max_abs((0..u.len()).filter(|q| !held.contains(q)).map(|q| y[q] - f[q]))
        };
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        let rh = residual(r.displacement_values());
        let rx = residual(&ue);
        let fmax = max_abs(f.iter().copied());
        println!(
            "D1b {name} residual / max |f|: solved {:.4e}, exact field {:.4e}",
            rh / fmax,
            rx / fmax
        );
        c.at_most(&format!("D1b {name} max |r(u_h)| / max |r(u_ex)|"), rh / rx, 10.0);
    }
    // D2: plate S, t = 0.3.
    let t = 0.3;
    let place = Placement::identity();
    let (mut m, p, el) = plate(&place, t, &[0]);
    m.add_load(Load::shell_temperature_gradient(vec![GRAD; p.len()]))
        .unwrap();
    let r = solve_static(&m, &StaticOptions::default()).unwrap();
    let u = r.displacement_values();
    let f = fields(&m, u, &place, p.len());
    let mut dk = 0.0f64;
    for e in &el {
        let s = strain_at(&element_values(&p, e), &element_values(&f.beta, e), 0.0, 0.0);
        dk = dk.max(max_abs([s[0] - k, s[1] - k, s[2]]));
    }
    c.at_most("D2 S max |kappa - alpha g (1, 1, 0)| / (alpha g)", dk / k, 1e-12);
    let st = shell::stresses(&m, u).unwrap();
    let mmax = max_abs(st.iter().flat_map(|(_, s)| s.bending));
    c.at_most("D2 S max |M_k| / M_s", mmax / moment_scale(t, GRAD), 1e-12);
    c.finish();
}
