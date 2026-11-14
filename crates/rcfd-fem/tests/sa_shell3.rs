//! F-419 (ADR 0454): the three-node flat shell Shell3. The checks of the registration, in its order; every limit is the
//! registered one and every prototype number is that of `f419-scratch/estimate*.log`.

use rcfd_core::vec3::{cross, dot, norm, Vec3};
use rcfd_fem::assemble::{assemble_with, stiffness};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::mass::{consistent, mass_properties};
use rcfd_fem::modal::{modal_analysis, modal_analysis_with, ModalOptions};
use rcfd_fem::shell::{self, TransverseShear};
use rcfd_fem::shell3::{self, Shell3Options};
use rcfd_fem::solve::{solve_static, solve_static_with, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::{
    BlockProperty, Constraint, ElasticSupport, ElementKind, FemError, Load, Material, SolidMesh, SolidModel,
};
use rcfd_linalg::amg_elastic::ElasticAmgSettings;
use rcfd_linalg::eigen::{count_below, ResidualTest};
use rcfd_linalg::skyline::{SkylineLdlt, SkylineOrdering};
use std::f64::consts::PI;

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
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{name}: {value:.6e} (limit < {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} >= {limit:e}"));
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

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;

fn material(nu: f64, rho: f64) -> Material {
    Material::isotropic_elastic(E, nu, rho, 0.0).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

/// A mesh of one Shell3 block, element set "all", node set "all".
fn tri_mesh(nodes: Vec<Vec3>, tris: &[[usize; 3]], thickness: f64) -> SolidMesh {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell3, tris.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell(thickness).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..tris.len()).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh
}

fn model_of(mesh: SolidMesh, mat: Material) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    m
}

/// The two triangles of the quadrilateral `q` (nodes counter-clockwise): `diag = 0` along `q0-q2`, `1` along `q1-q3`.
fn cut(q: [usize; 4], diag: usize) -> [[usize; 3]; 2] {
    if diag == 0 {
        [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]
    } else {
        [[q[0], q[1], q[3]], [q[1], q[2], q[3]]]
    }
}

/// A grid of `nx x ny` cells on `[0, a] x [0, b]` at z = 0, nodes in rows of constant y; cell nodes `(i, j), (i + 1, j), (i +
/// 1, j + 1), (i, j + 1)`, cells row by row.
fn grid(a: f64, b: f64, nx: usize, ny: usize) -> (Vec<Vec3>, Vec<[usize; 4]>) {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([a * i as f64 / nx as f64, b * j as f64 / ny as f64, 0.0]);
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

fn cut_all(quads: &[[usize; 4]], diag: usize) -> Vec<[usize; 3]> {
    quads.iter().flat_map(|&q| cut(q, diag)).collect()
}

/// The rotation of `angle` about `axis` (Rodrigues), as rows.
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

fn apply(q: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [dot(q[0], v), dot(q[1], v), dot(q[2], v)]
}

fn transpose(q: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut t = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            t[i][j] = q[j][i];
        }
    }
    t
}

fn plane(e: f64, nu: f64, s: f64) -> [[f64; 3]; 3] {
    let c = e * s / (1.0 - nu * nu);
    [[c, nu * c, 0.0], [nu * c, c, 0.0], [0.0, 0.0, 0.5 * (1.0 - nu) * c]]
}

fn mul3(d: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [dot(d[0], v), dot(d[1], v), dot(d[2], v)]
}

/// A plane tensor `(xx, yy, xy)` in patch axes expressed in the in-plane axes `a`, `b` (patch coordinates).
fn tensor_in(t: [f64; 3], a: [f64; 2], b: [f64; 2]) -> [f64; 3] {
    let m = [[t[0], t[2]], [t[2], t[1]]];
    let q = |u: [f64; 2], v: [f64; 2]| -> f64 {
        let mut s = 0.0;
        for i in 0..2 {
            for j in 0..2 {
                s += u[i] * m[i][j] * v[j];
            }
        }
        s
    };
    [q(a, a), q(b, b), q(a, b)]
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

// ---------------------------------------------------------------------------------------------------- PT1 to PT6

const PATCH: [[f64; 2]; 16] = [
    [0.0, 0.0],
    [1.0 / 3.0, 0.0],
    [2.0 / 3.0, 0.0],
    [1.0, 0.0],
    [0.0, 1.0 / 3.0],
    [0.28, 0.40],
    [0.71, 0.30],
    [1.0, 1.0 / 3.0],
    [0.0, 2.0 / 3.0],
    [0.36, 0.62],
    [0.62, 0.74],
    [1.0, 2.0 / 3.0],
    [0.0, 1.0],
    [1.0 / 3.0, 1.0],
    [2.0 / 3.0, 1.0],
    [1.0, 1.0],
];

/// The patch model of ADR 0454 arithmetic 17: all cells cut (`mixed = false`) or cells 0, 4, 8 kept as Shell4.
fn patch_model(q: &[[f64; 3]; 3], mixed: bool) -> SolidModel {
    let origin = [0.3, -0.2, 0.5];
    let nodes: Vec<Vec3> = PATCH
        .iter()
        .map(|p| {
            let r = apply(q, [p[0], p[1], 0.0]);
            [origin[0] + r[0], origin[1] + r[1], origin[2] + r[2]]
        })
        .collect();
    let mut quads = Vec::new();
    let mut tris = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            let n = 3 * j + i;
            let cell = [4 * j + i, 4 * j + i + 1, 4 * (j + 1) + i + 1, 4 * (j + 1) + i];
            if mixed && n % 4 == 0 {
                quads.push(cell);
            } else {
                tris.extend_from_slice(&cut(cell, n % 2));
            }
        }
    }
    let t = 0.05;
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let mut b = 0;
    if !quads.is_empty() {
        mesh.add_block(ElementKind::Shell4, quads.iter().flatten().copied().collect())
            .unwrap();
        mesh.set_block_property(b, BlockProperty::shell(t).unwrap()).unwrap();
        b += 1;
    }
    mesh.add_block(ElementKind::Shell3, tris.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(b, BlockProperty::shell(t).unwrap()).unwrap();
    mesh.add_element_set("all", (0..quads.len() + tris.len()).collect())
        .unwrap();
    model_of(mesh, material(0.3, RHO))
}

#[test]
fn pt_patch_tests_irregular_and_mixed_meshes() {
    let mut c = Checks::new();
    let q = rotation([1.0, -2.0, 2.0], 0.7);
    let boundary: Vec<usize> = (0..16)
        .filter(|&k| {
            let p = PATCH[k];
            p[0] == 0.0 || p[0] == 1.0 || p[1] == 0.0 || p[1] == 1.0
        })
        .collect();
    let interior: Vec<usize> = (0..16).filter(|k| !boundary.contains(k)).collect();
    let (a, b) = ([1e-3, 2e-3, -1.5e-3], [-5e-4, 1e-3, 2.5e-3]);
    let omega = 0.5 * (b[1] - a[2]);
    let membrane = |p: [f64; 2]| -> [f64; 6] {
        let u = [a[0] + a[1] * p[0] + a[2] * p[1], b[0] + b[1] * p[0] + b[2] * p[1], 0.0];
        let g = apply(&q, u);
        let r = apply(&q, [0.0, 0.0, omega]);
        [g[0], g[1], g[2], r[0], r[1], r[2]]
    };
    let (c1, c2, c3) = (2e-3, -1e-3, 1.5e-3);
    let bending = |p: [f64; 2]| -> [f64; 6] {
        let (x, y) = (p[0], p[1]);
        let w = -(c1 * x * x + c2 * y * y + c3 * x * y) / 2.0 + 1e-4 + 2e-4 * x;
        let bx = c1 * x + c3 * y / 2.0 - 2e-4;
        let by = c2 * y + c3 * x / 2.0;
        let g = apply(&q, [0.0, 0.0, w]);
        let r = apply(&q, [-by, bx, 0.0]);
        [g[0], g[1], g[2], r[0], r[1], r[2]]
    };
    let (t, nu) = (0.05, 0.3);
    let n_exact = mul3(&plane(E, nu, t), [a[1], b[2], a[2] + b[1]]);
    let m_exact = mul3(&plane(E, nu, t * t * t / 12.0), [c1, c2, c3]);
    for mixed in [false, true] {
        let model = patch_model(&q, mixed);
        let n_shell4 = if mixed { 3 } else { 0 };
        for (name, field, is_membrane) in [
            ("membrane", &membrane as &dyn Fn([f64; 2]) -> [f64; 6], true),
            ("bending", &bending as &dyn Fn([f64; 2]) -> [f64; 6], false),
        ] {
            let exact: Vec<[f64; 6]> = PATCH.iter().map(|&p| field(p)).collect();
            let scale = exact.iter().flatten().fold(0.0f64, |m, v| m.max(v.abs()));
            let mut pres = Vec::new();
            for &k in &boundary {
                for comp in 0..6 {
                    pres.push(Prescribed::new(k, comp, exact[k][comp]));
                }
            }
            let r = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap();
            let mut err = 0.0f64;
            for &k in &interior {
                for comp in 0..6 {
                    err = err.max((r.displacement(k)[comp] - exact[k][comp]).abs());
                }
            }
            let id = match (mixed, is_membrane) {
                (false, true) => "PT1",
                (false, false) => "PT3",
                (true, true) => "PT5",
                (true, false) => "PT6",
            };
            c.at_most(&format!("{id} {name} patch: interior unknowns"), err / scale, 1e-8);
            let st = shell::stresses(&model, r.displacement_values()).unwrap();
            c.holds(
                &format!("{id} stresses of every element"),
                st.len() == model.mesh().n_elements() && st.iter().enumerate().all(|(i, (e, _))| i == *e),
                &format!("{} of {} ({} Shell4)", st.len(), model.mesh().n_elements(), n_shell4),
            );
            let (mut e_main, mut e_other, mut big) = (0.0f64, 0.0f64, 0.0f64);
            for (_, s) in &st {
                let back = transpose(&q);
                let pa = {
                    let v = apply(&back, s.axes[0]);
                    [v[0], v[1]]
                };
                let pb = {
                    let v = apply(&back, s.axes[1]);
                    [v[0], v[1]]
                };
                let (ex, main, other) = if is_membrane {
                    (tensor_in(n_exact, pa, pb), s.membrane, s.bending)
                } else {
                    (tensor_in(m_exact, pa, pb), s.bending, s.membrane)
                };
                big = big.max(max_abs(&ex));
                for k in 0..3 {
                    e_main = e_main.max((main[k] - ex[k]).abs());
                    e_other = e_other.max(other[k].abs());
                }
                for k in 0..2 {
                    e_other = e_other.max(s.shear[k].abs());
                }
            }
            match id {
                "PT1" => {
                    c.at_most("PT2 membrane patch: N against A_m eps", e_main / big, 1e-8);
                    c.at_most("PT2 membrane patch: m and Q", e_other / big, 1e-8);
                }
                "PT3" => {
                    c.at_most("PT4 bending patch: m against D_b kappa", e_main / big, 1e-8);
                    c.at_most("PT4 bending patch: Q and N times the side", e_other / big, 1e-8);
                }
                "PT5" => {
                    c.at_most("PT5 mixed membrane patch: N of every element", e_main / big, 1e-8);
                    println!("PT5 (printed) mixed membrane patch: m and Q {:.4e}", e_other / big);
                }
                _ => {
                    c.at_most("PT6 mixed bending patch: m of every element", e_main / big, 1e-8);
                    println!("PT6 (printed) mixed bending patch: Q and N {:.4e}", e_other / big);
                }
            }
        }
    }
    c.finish();
}

// ---------------------------------------------------------------------------------------- SS1 to SS4, P1, N4, N5

/// The Navier coefficient summed to m, n <= `terms` (ADR 0432 arithmetic 10).
fn navier(terms: usize) -> f64 {
    let mut s = 0.0;
    for m in (1..=terms).step_by(2) {
        for n in (1..=terms).step_by(2) {
            let sign = if ((m + n) / 2 - 1) % 2 == 0 { 1.0 } else { -1.0 };
            let (mf, nf) = (m as f64, n as f64);
            s += sign / (mf * nf * (mf * mf + nf * nf).powi(2));
        }
    }
    16.0 / PI.powi(6) * s
}

/// The simply supported plate of ADR 0454 arithmetic 10 (32 x 32 squares cut along `diag`): model, prescribed, centre node.
fn ss_plate(t: f64, rho: f64, diag: usize) -> (SolidModel, Vec<Prescribed>, usize) {
    let n = 32;
    let (nodes, quads) = grid(1.0, 1.0, n, n);
    let mut pres = Vec::new();
    for (k, x) in nodes.iter().enumerate() {
        for comp in [0, 1, 5] {
            pres.push(Prescribed::new(k, comp, 0.0));
        }
        let onx = x[0] == 0.0 || x[0] == 1.0;
        let ony = x[1] == 0.0 || x[1] == 1.0;
        if onx || ony {
            pres.push(Prescribed::new(k, 2, 0.0));
        }
        if onx {
            pres.push(Prescribed::new(k, 3, 0.0));
        }
        if ony {
            pres.push(Prescribed::new(k, 4, 0.0));
        }
    }
    let mut m = model_of(tri_mesh(nodes, &cut_all(&quads, diag), t), material(0.3, rho));
    m.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    (m, pres, (n / 2) * (n + 1) + n / 2)
}

/// The centre deflection of the plate with the element stiffness of `opts`, by `assemble_with` (the controls).
fn ss_control(t: f64, opts: &Shell3Options) -> f64 {
    let (m, pres, centre) = ss_plate(t, RHO, 0);
    let k = assemble_with(&m, None, |ctx| {
        shell3::element_stiffness_with(ctx.coords, t, ctx.material, opts)
    })
    .unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let fixed = prescribed_equations(&m, &pres).unwrap();
    let sys = reduce(&k, &f, &fixed).unwrap();
    let ldlt = SkylineLdlt::factor(&sys.matrix).unwrap();
    let u = sys.expand(&ldlt.solve(&sys.rhs));
    u[m.dof_map().equation(centre, 2).unwrap()]
}

#[test]
fn ss_simply_supported_plate_diagonals_and_locking_controls() {
    let mut c = Checks::new();
    let cn = navier(799);
    println!("Navier coefficient c = {cn:.12}");
    c.at_most("SS4 Navier coefficient |c - 0.004062|", (cn - 0.004062).abs(), 5e-7);
    let q = 1000.0;
    let ratio = |w: f64, t: f64| w / (cn * q / (E * t * t * t / (12.0 * (1.0 - 0.09))));
    for (t, limit, id, proto) in [(0.01, 2e-2, "SS1", 0.9981577495), (0.001, 3e-2, "SS2", 0.9974430088)] {
        let mut x = [0.0; 2];
        for diag in 0..2 {
            let (m, pres, centre) = ss_plate(t, RHO, diag);
            let r = solve_static_with(&m, &pres, &[], &StaticOptions::default()).unwrap();
            x[diag] = ratio(r.displacement(centre)[2], t);
            println!("{id} t/a = {t}, diagonal {diag}: w_c / w_Navier = {:.10}", x[diag]);
        }
        c.at_most(
            &format!("{id} SS plate t/a = {t} against Navier"),
            (x[0] - 1.0).abs(),
            limit,
        );
        c.at_most(
            &format!("SS3 t/a = {t}: the two diagonal directions"),
            rel(x[0], x[1]),
            1e-2,
        );
        c.at_most(
            &format!("P1 SS plate t/a = {t} against the prototype"),
            rel(x[0], proto),
            1e-5,
        );
    }
    let t = 0.001;
    let plain = ratio(ss_control(t, &Shell3Options::default().stabilisation(0.0)), t);
    println!("N4 plain MITC3 (no stabilisation) t/a = 0.001: {plain:.10}");
    c.below("N4 plain MITC3 locks", plain, 0.95);
    c.at_most("P1 N4 against the prototype", rel(plain, 0.8988341478), 1e-5);
    let full = ratio(
        ss_control(
            t,
            &Shell3Options::default()
                .stabilisation(0.0)
                .shear(TransverseShear::FullIntegration),
        ),
        t,
    );
    println!("N5 displacement shear, no stabilisation, t/a = 0.001: {full:.10e}");
    c.below("N5 displacement shear locks", full, 0.2);
    c.at_most("P1 N5 against the prototype", rel(full, 2.863079e-3), 1e-5);
    c.finish();
}

// --------------------------------------------------------------------------------------------- CT1, CT2, P2

#[test]
fn ct_cantilever_strip() {
    let mut c = Checks::new();
    let (l, b, t, p, n) = (1.0, 0.1, 0.01, 1.0, 15);
    let (nodes, quads) = grid(l, b, n, 1);
    let m = model_of(tri_mesh(nodes, &cut_all(&quads, 0), t), material(0.0, RHO));
    let root = [0, n + 1];
    let tip = [n, 2 * n + 1];
    let mut pres = Vec::new();
    for &k in &root {
        for comp in 0..6 {
            pres.push(Prescribed::new(k, comp, 0.0));
        }
    }
    let forces: Vec<NodalForceAt> = tip.iter().map(|&k| NodalForceAt::new(k, [0.0, 0.0, p / 2.0])).collect();
    let r = solve_static_with(&m, &pres, &forces, &StaticOptions::default()).unwrap();
    let i = b * t * t * t / 12.0;
    let w = 0.5 * (r.displacement(tip[0])[2] + r.displacement(tip[1])[2]);
    let x = w / (p * l * l * l / (3.0 * E * i));
    println!("CT1 tip / (P L^3 / (3 E I)) = {x:.10}");
    c.at_most("CT1 cantilever tip deflection", (x - 1.0).abs(), 1e-2);
    c.at_most("P2 cantilever tip against the prototype", rel(x, 0.9998605036), 1e-5);
    let st = shell::stresses(&m, r.displacement_values()).unwrap();
    let mid = 6.0 * p * l / 2.0 / (b * t * t);
    for e in [14usize, 15] {
        let s = st[e].1;
        // The strip axis x in the element's axes.
        let ax = [s.axes[0][0], s.axes[1][0]];
        let top = tensor_in(s.top, ax, ax)[0];
        let bottom = tensor_in(s.bottom, ax, ax)[0];
        println!("CT2 element {e}: top {top:.8e}, bottom {bottom:.8e}, 6M/(b t^2) = {mid:.8e}");
        c.at_most(
            &format!("CT2 element {e}: top sigma_xx against -6M/(b t^2)"),
            rel(top, -mid),
            2e-2,
        );
        c.at_most(
            &format!("CT2 element {e}: bottom sigma_xx against +6M/(b t^2)"),
            rel(bottom, mid),
            2e-2,
        );
    }
    c.finish();
}

// --------------------------------------------------------------------------------------------------- RG1, P3

#[test]
fn rg_pinched_ring() {
    let mut c = Checks::new();
    let (nq, r0, b, t, p) = (32, 1.0, 0.05, 0.01, 1.0);
    let idx = |i: usize, j: usize| 2 * i + j;
    let mut nodes = Vec::new();
    for i in 0..=nq {
        let ph = PI / 2.0 * i as f64 / nq as f64;
        for z in [0.0, b] {
            nodes.push([r0 * ph.cos(), r0 * ph.sin(), z]);
        }
    }
    let quads: Vec<[usize; 4]> = (0..nq)
        .map(|i| [idx(i, 0), idx(i + 1, 0), idx(i + 1, 1), idx(i, 1)])
        .collect();
    let m = model_of(tri_mesh(nodes.clone(), &cut_all(&quads, 0), t), material(0.0, RHO));
    let mut pres = Vec::new();
    for k in 0..nodes.len() {
        for comp in [2, 3, 4] {
            pres.push(Prescribed::new(k, comp, 0.0));
        }
    }
    for j in 0..2 {
        for comp in [1, 5] {
            pres.push(Prescribed::new(idx(0, j), comp, 0.0));
        }
        for comp in [0, 5] {
            pres.push(Prescribed::new(idx(nq, j), comp, 0.0));
        }
    }
    let forces: Vec<NodalForceAt> = (0..2)
        .map(|j| NodalForceAt::new(idx(nq, j), [0.0, -p / 4.0, 0.0]))
        .collect();
    let r = solve_static_with(&m, &pres, &forces, &StaticOptions::default()).unwrap();
    let i = b * t * t * t / 12.0;
    let top = -0.5 * (r.displacement(idx(nq, 0))[1] + r.displacement(idx(nq, 1))[1]);
    let x = 2.0 * top / ((PI / 4.0 - 2.0 / PI) * p * r0.powi(3) / (E * i));
    println!("RG1 diameter change / closed form = {x:.10}");
    c.at_most("RG1 pinched ring", (x - 1.0).abs(), 3e-2);
    c.at_most("P3 ring against the prototype", rel(x, 0.9991065416), 1e-5);
    c.finish();
}

// ----------------------------------------------------------------------------------------------- CY1, P4, O1

/// The cylinder quarter of ADR 0454 arithmetic 13 on 32 x 16 cells, `checkerboard` or one diagonal: `(min, max)` of `u_r / (p
/// R^2 / (E t))` over the nodes.
fn cylinder(checkerboard: bool) -> (f64, f64) {
    let (nq, nz, r0, lh, t, p) = (32, 16, 1.0, 0.5, 0.01, 1e5);
    let idx = |i: usize, j: usize| (nz + 1) * i + j;
    let mut nodes = Vec::new();
    for i in 0..=nq {
        let ph = PI / 2.0 * i as f64 / nq as f64;
        for j in 0..=nz {
            nodes.push([r0 * ph.cos(), r0 * ph.sin(), lh * j as f64 / nz as f64]);
        }
    }
    let mut tris = Vec::new();
    for i in 0..nq {
        for j in 0..nz {
            let cell = [idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)];
            let diag = if checkerboard { (i + j) % 2 } else { 0 };
            tris.extend_from_slice(&cut(cell, diag));
        }
    }
    let mut m = model_of(tri_mesh(nodes.clone(), &tris, t), material(0.3, RHO));
    m.add_load(Load::shell_pressure("all", p)).unwrap();
    let mut pres = Vec::new();
    for j in 0..=nz {
        for comp in [1, 3, 5] {
            pres.push(Prescribed::new(idx(0, j), comp, 0.0));
        }
        for comp in [0, 4, 5] {
            pres.push(Prescribed::new(idx(nq, j), comp, 0.0));
        }
    }
    for i in 0..=nq {
        for comp in [2, 3, 4] {
            pres.push(Prescribed::new(idx(i, 0), comp, 0.0));
        }
    }
    let r = solve_static_with(&m, &pres, &[], &StaticOptions::default()).unwrap();
    let reference = p * r0 * r0 / (E * t);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for (k, x) in nodes.iter().enumerate() {
        let u = r.displacement(k);
        let ur = (u[0] * x[0] + u[1] * x[1]) / r0 / reference;
        lo = lo.min(ur);
        hi = hi.max(ur);
    }
    (lo, hi)
}

#[test]
fn cy_cylinder_under_internal_pressure() {
    let mut c = Checks::new();
    let (lo, hi) = cylinder(true);
    println!("CY1 checkerboard u_r / (p R^2 / (E t)) in [{lo:.10}, {hi:.10}]");
    c.at_most(
        "CY1 cylinder: max |u_r / (p R^2 / (E t)) - 1|",
        (lo - 1.0).abs().max((hi - 1.0).abs()),
        1e-2,
    );
    c.at_most("P4 cylinder min against the prototype", rel(lo, 0.9982414459), 1e-5);
    c.at_most("P4 cylinder max against the prototype", rel(hi, 1.0016208372), 1e-5);
    let (lo, hi) = cylinder(false);
    println!("O1 (observation, no limit) one-diagonal mesh: u_r / (p R^2 / (E t)) in [{lo:.10}, {hi:.10}]");
    c.finish();
}

// ------------------------------------------------------------------------------------------------- FQ1, P5

#[test]
fn fq_simply_supported_plate_first_frequency() {
    let mut c = Checks::new();
    let t = 0.01;
    let opts = ModalOptions::new(4)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward);
    let d = E * t * t * t / (12.0 * (1.0 - 0.09));
    let w11 = 2.0 * PI * PI * (d / (RHO * t)).sqrt();
    for diag in 0..2 {
        let (m, pres, _) = ss_plate(t, RHO, diag);
        let r = modal_analysis_with(&m, &pres, &opts).unwrap();
        println!("FQ diagonal {diag}: frequencies {:?} Hz", r.frequencies);
        c.at_most(
            &format!("FQ1 diagonal {diag}: lowest frequency against omega_11"),
            rel(2.0 * PI * r.frequencies[0], w11),
            2e-2,
        );
        if diag == 0 {
            let proto = [48.0684613135, 120.3395462744, 120.5716920194, 193.2516203810];
            for (i, pf) in proto.iter().enumerate() {
                c.at_most(
                    &format!("P5 frequency {} against the prototype", i + 1),
                    rel(r.frequencies[i], *pf),
                    1e-5,
                );
            }
        }
    }
    c.finish();
}

// -------------------------------------------------------------------------------------------- FR1 to FR3, P6, N1

fn free_plate(n: usize, t: f64) -> SolidModel {
    let (nodes, quads) = grid(1.0, 0.8, n, n);
    model_of(tri_mesh(nodes, &cut_all(&quads, 0), t), material(0.3, RHO))
}

#[test]
fn fr_free_plate_six_zero_frequencies() {
    let mut c = Checks::new();
    let opts = ModalOptions::new(8)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward);
    for (id, n, t, limit, p7, p8) in [
        ("FR1", 4usize, 0.1, 1e-6, 395.4141399498, 525.7994847722),
        ("FR2", 8, 0.01, 1e-4, 40.8196288856, 52.4645618939),
    ] {
        let m = free_plate(n, t);
        let r = modal_analysis(&m, &opts).unwrap();
        println!("{id} frequencies {:?} Hz", r.frequencies);
        let f = &r.frequencies;
        let zero = f[..6].iter().fold(0.0f64, |a, v| a.max(v.abs()));
        c.at_most(&format!("{id} free plate max |f_1..6| / f_7"), zero / f[6], limit);
        c.above(&format!("{id} f_7"), f[6], 0.0);
        c.at_most(&format!("P6 {id} f_7 against the prototype"), rel(f[6], p7), 1e-5);
        c.at_most(&format!("P6 {id} f_8 against the prototype"), rel(f[7], p8), 1e-5);
        let lambda7 = r.eigenvalues[6];
        let k = stiffness(&m).unwrap();
        let mm = consistent(&m).unwrap();
        let n6 = count_below(&k, &mm, lambda7 / 4.0, &SkylineOrdering::Rcm).unwrap();
        c.holds(
            &format!("FR3 {id} Sturm count below lambda_7 / 4 is 6"),
            n6 == 6,
            &format!("{n6}"),
        );
        if id == "FR1" {
            let o = Shell3Options::default().drilling(0.0);
            let k0 = assemble_with(&m, None, |ctx| {
                shell3::element_stiffness_with(ctx.coords, t, ctx.material, &o)
            })
            .unwrap();
            let n0 = count_below(&k0, &mm, lambda7 / 4.0, &SkylineOrdering::Rcm).unwrap();
            println!("N1 Sturm count with alpha_d = 0: {n0}");
            c.above("N1 alpha_d = 0: more than six zeros", n0 as f64, 6.0);
        }
    }
    c.finish();
}

// ---------------------------------------------------------------------------------------------- SE1, SE2, ISO1

/// The eigenvalues of a symmetric matrix by cyclic Jacobi rotations (test code), ascending.
fn jacobi_eigenvalues(a: &[f64], n: usize) -> Vec<f64> {
    let mut m = a.to_vec();
    for _sweep in 0..100 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| m[i * n + j] * m[i * n + j])
            .sum();
        let diag: f64 = (0..n).map(|i| m[i * n + i] * m[i * n + i]).sum();
        if off <= 1e-34 * diag {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = m[p * n + q];
                if apq == 0.0 {
                    continue;
                }
                let theta = (m[q * n + q] - m[p * n + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let cs = 1.0 / (t * t + 1.0).sqrt();
                let sn = t * cs;
                for k in 0..n {
                    let (akp, akq) = (m[k * n + p], m[k * n + q]);
                    m[k * n + p] = cs * akp - sn * akq;
                    m[k * n + q] = sn * akp + cs * akq;
                }
                for k in 0..n {
                    let (apk, aqk) = (m[p * n + k], m[q * n + k]);
                    m[p * n + k] = cs * apk - sn * aqk;
                    m[q * n + k] = sn * apk + cs * aqk;
                }
            }
        }
    }
    let mut ev: Vec<f64> = (0..n).map(|i| m[i * n + i]).collect();
    ev.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ev
}

#[test]
fn se_iso_single_element() {
    let mut c = Checks::new();
    let q = rotation([1.0, -2.0, 2.0], 0.7);
    let origin = [0.3, -0.2, 0.5];
    let x: Vec<Vec3> = [[0.0, 0.0], [0.9, 0.15], [0.3, 0.7]]
        .iter()
        .map(|p| {
            let r = apply(&q, [p[0], p[1], 0.0]);
            [origin[0] + r[0], origin[1] + r[1], origin[2] + r[2]]
        })
        .collect();
    let mat = material(0.3, RHO);
    for t in [0.01, 0.1] {
        let k = shell3::element_stiffness(&x, t, &mat).unwrap();
        let ev = jacobi_eigenvalues(k.values(), 18);
        let lmax = ev[17];
        let zeros = ev.iter().filter(|l| l.abs() <= 1e-10 * lmax).count();
        println!(
            "SE1 t = {t}: max |lambda_1..6| / lambda_max = {:.3e}, lambda_7 / lambda_max = {:.4e}",
            ev[..6].iter().fold(0.0f64, |m, l| m.max(l.abs())) / lmax,
            ev[6] / lmax
        );
        c.holds(
            &format!("SE1 t = {t}: six zero eigenvalues"),
            zeros == 6,
            &format!("{zeros}"),
        );
        c.above(&format!("SE1 t = {t}: lambda_7 / lambda_max"), ev[6] / lmax, 1e-7);
        let kmax = max_abs(k.values());
        let mut worst = 0.0f64;
        for mode in 0..6 {
            let mut r = vec![0.0; 18];
            for (i, xi) in x.iter().enumerate() {
                if mode < 3 {
                    r[6 * i + mode] = 1.0;
                } else {
                    let mut w = [0.0; 3];
                    w[mode - 3] = 1.0;
                    r[6 * i..6 * i + 3].copy_from_slice(&cross(w, *xi));
                    r[6 * i + 3..6 * i + 6].copy_from_slice(&w);
                }
            }
            worst = worst.max(max_abs(&k.matvec(&r)) / (kmax * max_abs(&r)));
        }
        c.at_most(&format!("SE2 t = {t}: |K r| / (max |K| max |r|)"), worst, 1e-12);
        let sym = (0..18).all(|i| (0..18).all(|j| k.get(i, j).to_bits() == k.get(j, i).to_bits()));
        c.holds(&format!("SE2 t = {t}: symmetric bit for bit"), sym, "");
        for perm in [[1usize, 2, 0], [2, 0, 1]] {
            let xp: Vec<Vec3> = perm.iter().map(|&i| x[i]).collect();
            let kp = shell3::element_stiffness(&xp, t, &mat).unwrap();
            let mut diff = 0.0f64;
            for (a, &na) in perm.iter().enumerate() {
                for (b, &nb) in perm.iter().enumerate() {
                    for ca in 0..6 {
                        for cb in 0..6 {
                            let d = kp.get(6 * a + ca, 6 * b + cb) - k.get(6 * na + ca, 6 * nb + cb);
                            diff = diff.max(d.abs());
                        }
                    }
                }
            }
            c.at_most(
                &format!("ISO1 t = {t}: relabelling {perm:?} against K"),
                diff / kmax,
                1e-12,
            );
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- AN1 to AN3

/// Two triangles: element 0 `(0, 0), (1, 0), (0, 1)`, element 1 `(1, 0), (2, 0), (1.5, h)`; nodes 0, 2, 3 held.
fn angle_model(h: f64) -> (SolidModel, Vec<Prescribed>) {
    let nodes = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [2.0, 0.0, 0.0],
        [1.5, h, 0.0],
    ];
    let mut mesh = tri_mesh(nodes, &[[0, 1, 2], [1, 3, 4]], 0.01);
    mesh.add_node_set("tip", vec![4]).unwrap();
    let mut m = model_of(mesh, material(0.3, RHO));
    m.add_load(Load::nodal_force("tip", [0.0, 0.0, 1.0])).unwrap();
    let mut pres = Vec::new();
    for k in [0, 2, 3] {
        for comp in 0..6 {
            pres.push(Prescribed::new(k, comp, 0.0));
        }
    }
    (m, pres)
}

#[test]
fn an_small_angle_refused_with_the_element() {
    let mut c = Checks::new();
    let h = 0.5 * 0.9f64.to_radians().tan();
    let (m, pres) = angle_model(h);
    let is_angle = |r: &Result<(), FemError>| -> (bool, String) {
        match r {
            Err(e @ FemError::Element { element: 1, source }) => {
                let msg = e.to_string();
                (
                    matches!(source.as_ref(), FemError::InvalidValue { key: "angle", .. })
                        && msg.contains("0.9000 deg")
                        && msg.contains("limit 1 deg"),
                    msg,
                )
            }
            other => (false, format!("{other:?}")),
        }
    };
    let (ok, msg) = is_angle(&solve_static_with(&m, &pres, &[], &StaticOptions::default()).map(|_| ()));
    c.holds("AN1 solve_static_with refuses element 1", ok, &msg);
    let (ok, msg) = is_angle(&modal_analysis_with(&m, &pres, &ModalOptions::new(2)).map(|_| ()));
    c.holds("AN1 modal_analysis_with refuses element 1", ok, &msg);
    let (ok, msg) = is_angle(&stiffness(&m).map(|_| ()));
    c.holds("AN1 stiffness refuses element 1", ok, &msg);
    let (m, pres) = angle_model(0.5 * 1.1f64.to_radians().tan());
    let r = solve_static_with(&m, &pres, &[], &StaticOptions::default());
    c.holds("AN2 1.1 deg solves", r.is_ok(), &format!("{:?}", r.as_ref().err()));
    let mut mesh = tri_mesh(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [2.0, 0.0, 0.0]],
        &[[0, 1, 2], [1, 3, 0]],
        0.01,
    );
    mesh.add_node_set("x", vec![0]).unwrap();
    let m = model_of(mesh, material(0.3, RHO));
    let r = stiffness(&m).map(|_| ());
    c.holds(
        "AN3 collinear triangle refused (geometry) naming element 1",
        matches!(&r, Err(FemError::Element { element: 1, source }) if matches!(source.as_ref(), FemError::InvalidValue { key: "geometry", .. })),
        &format!("{r:?}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- S1, N2, N3

fn follow_up<T: std::fmt::Debug>(r: &Result<T, FemError>) -> bool {
    matches!(r, Err(FemError::NotYetSupported { variant: "Shell3", package, .. }) if *package == shell3::FOLLOW_UP)
}

/// A Shell3 cantilever strip (4 cells cut), root clamped, tip loaded.
fn cantilever_model() -> SolidModel {
    let (nodes, quads) = grid(1.0, 0.1, 4, 1);
    let mut mesh = tri_mesh(nodes, &cut_all(&quads, 0), 0.01);
    mesh.add_node_set("root", vec![0, 5]).unwrap();
    mesh.add_node_set("tip", vec![4, 9]).unwrap();
    let mut m = model_of(mesh, material(0.3, RHO));
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("tip", [0.0, 0.0, 1.0])).unwrap();
    m
}

#[test]
#[ignore = "record of ADR 0454 before ADR 0490 (F-578) let nonlinear::initial_states take shells (row N2 nonlinear initial states); continued in s1_n2_n3_refusals_after_f578 (ADR 0291 pattern, maintainer standing decision)"]
fn s1_n2_n3_refusals() {
    let mut c = Checks::new();
    // S1: a Shell3 element on three nodes of a Hex8 box.
    let mut mesh = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let at = [[2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [2.0, 1.0, 1.0]];
    let conn: Vec<usize> = at
        .iter()
        .map(|p| mesh.nodes().iter().position(|q| q == p).unwrap())
        .collect();
    let first = conn[0];
    mesh.add_block(ElementKind::Shell3, conn).unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("shell", vec![2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(0.3, RHO)).unwrap();
    m.add_material("shell", material(0.3, RHO)).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    let node_text = format!("node {first} ");
    let shared = |r: Result<(), FemError>| -> (bool, String) {
        match &r {
            Err(e @ FemError::Element { element: 2, source }) => {
                let msg = e.to_string();
                (
                    matches!(source.as_ref(), FemError::InvalidValue { key: "node", .. }) && msg.contains(&node_text),
                    msg,
                )
            }
            other => (false, format!("{other:?}")),
        }
    };
    let (ok, msg) = shared(solve_static(&m, &StaticOptions::default()).map(|_| ()));
    c.holds("S1 solve_static", ok, &msg);
    let (ok, msg) = shared(modal_analysis(&m, &ModalOptions::new(2)).map(|_| ()));
    c.holds("S1 modal_analysis", ok, &msg);
    let (ok, msg) = shared(stiffness(&m).map(|_| ()));
    c.holds("S1 stiffness", ok, &msg);
    // N2: a valid Shell3 model on the other paths.
    let base = cantilever_model();
    let r = solve_static(&base, &StaticOptions::default());
    c.holds(
        "N2 the base model solves",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    let mut ms = base.clone();
    ms.add_support(ElasticSupport::nodal_spring("tip", 2, 1e6)).unwrap();
    let r = rcfd_fem::mpc::solve_static_mpc(&ms, &[], &[], &[], &StaticOptions::default());
    c.holds(
        "N2 nodal spring (the transformation) taken",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    let r = rcfd_fem::nonlinear::initial_states(&base);
    c.holds("N2 nonlinear initial states", follow_up(&r), &format!("{r:?}"));
    let amg = StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default()),
        tolerance: 1e-10,
        max_iterations: 100,
    });
    let r = solve_static(&base, &amg);
    c.holds("N2 smoothed-aggregation CG", follow_up(&r), &format!("{r:?}"));
    let r = base.check_supported();
    c.holds("N2 check_supported (the other paths)", follow_up(&r), &format!("{r:?}"));
    // N2: a cyclic constraint on a model with a Hex8 box and a separate Shell3 element.
    let hex = hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let nh = hex.nodes().len();
    let mut nodes = hex.nodes().to_vec();
    nodes.extend_from_slice(&[[3.0, 0.0, 0.0], [4.0, 0.0, 0.0], [3.0, 1.0, 0.0]]);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Shell3, vec![nh, nh + 1, nh + 2]).unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    for name in ["ymin", "ymax"] {
        let faces = hex.surface_set(name).unwrap().to_vec();
        mesh.add_surface_set(name, faces).unwrap();
    }
    let mut mc = model_of(mesh, material(0.3, RHO));
    mc.add_constraint(Constraint::cyclic("ymin", "ymax", [0.0, 0.0, 1.0], [0.0; 3], 4))
        .unwrap();
    let r = rcfd_fem::mpc::solve_static_mpc(&mc, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    let ok = matches!(&r, Err(e @ FemError::NotYetSupported { variant: "Cyclic", package, .. }) if *package == shell3::FOLLOW_UP && e.to_string().contains("Shell3"));
    c.holds(
        "N2 cyclic constraint with Shell3 (solve_static_mpc)",
        ok,
        &format!("{r:?}"),
    );
    // N3: invalid data.
    let (nodes, quads) = grid(1.0, 1.0, 1, 1);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(
        ElementKind::Shell3,
        cut(quads[0], 0).iter().flatten().copied().collect(),
    )
    .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    let m = model_of(mesh, material(0.3, RHO));
    let r = solve_static(&m, &StaticOptions::default());
    c.holds(
        "N3 Shell3 block without property",
        matches!(&r, Err(e @ FemError::InvalidValue { key: "property", .. }) if e.to_string().contains("Shell3")),
        &format!("{r:?}"),
    );
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let mut m = SolidModel::new(hex);
    let r = m.add_load(Load::shell_pressure("all", 1.0));
    c.holds(
        "N3 shell pressure on Hex8 elements",
        matches!(
            &r,
            Err(FemError::InvalidValue {
                key: "shell pressure",
                ..
            })
        ),
        &format!("{r:?}"),
    );
    let mut m = cantilever_model();
    let r = m.add_load(Load::shell_pressure("all", 1.0));
    c.holds(
        "N3 shell pressure on Shell3 elements taken",
        r.is_ok(),
        &format!("{r:?}"),
    );
    c.finish();
}

/// The copy of `s1_n2_n3_refusals` after ADR 0490 (F-578): the row "N2 nonlinear initial states" now expects the states (the Newton
/// driver takes shells); every other row unchanged.
#[test]
fn s1_n2_n3_refusals_after_f578() {
    let mut c = Checks::new();
    // S1: a Shell3 element on three nodes of a Hex8 box.
    let mut mesh = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let at = [[2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [2.0, 1.0, 1.0]];
    let conn: Vec<usize> = at
        .iter()
        .map(|p| mesh.nodes().iter().position(|q| q == p).unwrap())
        .collect();
    let first = conn[0];
    mesh.add_block(ElementKind::Shell3, conn).unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("shell", vec![2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(0.3, RHO)).unwrap();
    m.add_material("shell", material(0.3, RHO)).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    let node_text = format!("node {first} ");
    let shared = |r: Result<(), FemError>| -> (bool, String) {
        match &r {
            Err(e @ FemError::Element { element: 2, source }) => {
                let msg = e.to_string();
                (
                    matches!(source.as_ref(), FemError::InvalidValue { key: "node", .. }) && msg.contains(&node_text),
                    msg,
                )
            }
            other => (false, format!("{other:?}")),
        }
    };
    let (ok, msg) = shared(solve_static(&m, &StaticOptions::default()).map(|_| ()));
    c.holds("S1 solve_static", ok, &msg);
    let (ok, msg) = shared(modal_analysis(&m, &ModalOptions::new(2)).map(|_| ()));
    c.holds("S1 modal_analysis", ok, &msg);
    let (ok, msg) = shared(stiffness(&m).map(|_| ()));
    c.holds("S1 stiffness", ok, &msg);
    // N2: a valid Shell3 model on the other paths.
    let base = cantilever_model();
    let r = solve_static(&base, &StaticOptions::default());
    c.holds(
        "N2 the base model solves",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    let mut ms = base.clone();
    ms.add_support(ElasticSupport::nodal_spring("tip", 2, 1e6)).unwrap();
    let r = rcfd_fem::mpc::solve_static_mpc(&ms, &[], &[], &[], &StaticOptions::default());
    c.holds(
        "N2 nodal spring (the transformation) taken",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    let r = rcfd_fem::nonlinear::initial_states(&base);
    c.holds(
        "N2 nonlinear initial states (taken, ADR 0490 D3)",
        r.is_ok(),
        &format!("{r:?}"),
    );
    let amg = StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default()),
        tolerance: 1e-10,
        max_iterations: 100,
    });
    let r = solve_static(&base, &amg);
    c.holds("N2 smoothed-aggregation CG", follow_up(&r), &format!("{r:?}"));
    let r = base.check_supported();
    c.holds("N2 check_supported (the other paths)", follow_up(&r), &format!("{r:?}"));
    // N2: a cyclic constraint on a model with a Hex8 box and a separate Shell3 element.
    let hex = hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let nh = hex.nodes().len();
    let mut nodes = hex.nodes().to_vec();
    nodes.extend_from_slice(&[[3.0, 0.0, 0.0], [4.0, 0.0, 0.0], [3.0, 1.0, 0.0]]);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Shell3, vec![nh, nh + 1, nh + 2]).unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    for name in ["ymin", "ymax"] {
        let faces = hex.surface_set(name).unwrap().to_vec();
        mesh.add_surface_set(name, faces).unwrap();
    }
    let mut mc = model_of(mesh, material(0.3, RHO));
    mc.add_constraint(Constraint::cyclic("ymin", "ymax", [0.0, 0.0, 1.0], [0.0; 3], 4))
        .unwrap();
    let r = rcfd_fem::mpc::solve_static_mpc(&mc, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    let ok = matches!(&r, Err(e @ FemError::NotYetSupported { variant: "Cyclic", package, .. }) if *package == shell3::FOLLOW_UP && e.to_string().contains("Shell3"));
    c.holds(
        "N2 cyclic constraint with Shell3 (solve_static_mpc)",
        ok,
        &format!("{r:?}"),
    );
    // N3: invalid data.
    let (nodes, quads) = grid(1.0, 1.0, 1, 1);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(
        ElementKind::Shell3,
        cut(quads[0], 0).iter().flatten().copied().collect(),
    )
    .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    let m = model_of(mesh, material(0.3, RHO));
    let r = solve_static(&m, &StaticOptions::default());
    c.holds(
        "N3 Shell3 block without property",
        matches!(&r, Err(e @ FemError::InvalidValue { key: "property", .. }) if e.to_string().contains("Shell3")),
        &format!("{r:?}"),
    );
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let mut m = SolidModel::new(hex);
    let r = m.add_load(Load::shell_pressure("all", 1.0));
    c.holds(
        "N3 shell pressure on Hex8 elements",
        matches!(
            &r,
            Err(FemError::InvalidValue {
                key: "shell pressure",
                ..
            })
        ),
        &format!("{r:?}"),
    );
    let mut m = cantilever_model();
    let r = m.add_load(Load::shell_pressure("all", 1.0));
    c.holds(
        "N3 shell pressure on Shell3 elements taken",
        r.is_ok(),
        &format!("{r:?}"),
    );
    c.finish();
}

// ----------------------------------------------------------------------------------------------- LD1 to LD3, M1

/// The plate of ADR 0454 arithmetic 18: `[0, 0.6] x [0, 0.4]`, 3 x 2 cells cut, placed by `q` (or not), t = 0.01.
fn load_plate(q: Option<&[[f64; 3]; 3]>, mat: Material) -> SolidModel {
    let (mut nodes, quads) = grid(0.6, 0.4, 3, 2);
    if let Some(q) = q {
        let origin = [0.3, -0.2, 0.5];
        for x in nodes.iter_mut() {
            let r = apply(q, *x);
            *x = [origin[0] + r[0], origin[1] + r[1], origin[2] + r[2]];
        }
    }
    model_of(tri_mesh(nodes, &cut_all(&quads, 0), 0.01), mat)
}

/// `t A_e / 12 (2 b_i + b_j + b_k)` summed over the elements, per node (forces only), from nodal values `b`.
fn consistent_nodal(m: &SolidModel, b: &[Vec3], t: f64) -> Vec<Vec3> {
    let mesh = m.mesh();
    let mut out = vec![[0.0; 3]; mesh.nodes().len()];
    for e in 0..mesh.n_elements() {
        let nd = mesh.element_nodes(e);
        let x: Vec<Vec3> = nd.iter().map(|&n| mesh.nodes()[n]).collect();
        let sub = |a: Vec3, c: Vec3| [a[0] - c[0], a[1] - c[1], a[2] - c[2]];
        let area = 0.5 * norm(cross(sub(x[1], x[0]), sub(x[2], x[0])));
        for i in 0..3 {
            for k in 0..3 {
                let s: f64 = (0..3).map(|j| if i == j { 2.0 } else { 1.0 } * b[nd[j]][k]).sum();
                out[nd[i]][k] += t * area / 12.0 * s;
            }
        }
    }
    out
}

#[test]
fn ld_m1_loads_thermal_and_mass() {
    let mut c = Checks::new();
    let q = rotation([1.0, -2.0, 2.0], 0.7);
    let t = 0.01;
    let alpha = 1.2e-5;
    let mat = || Material::isotropic_elastic(E, 0.3, RHO, alpha).unwrap();
    // LD1: gravity, centrifugal, nodal body force.
    let base = load_plate(Some(&q), mat());
    let nodes = base.mesh().nodes().to_vec();
    let g = [0.5, -1.5, -9.81];
    let (axis, origin, omega) = ([0.0, 0.3, 1.0], [0.4, 0.2, 0.0], 120.0);
    let perp = |x: Vec3| -> Vec3 {
        let a = {
            let l = norm(axis);
            [axis[0] / l, axis[1] / l, axis[2] / l]
        };
        let d = [x[0] - origin[0], x[1] - origin[1], x[2] - origin[2]];
        let s = dot(d, a);
        [d[0] - s * a[0], d[1] - s * a[1], d[2] - s * a[2]]
    };
    let cases: Vec<(&str, Load, Vec<Vec3>)> = vec![
        (
            "gravity",
            Load::gravity(g),
            vec![[RHO * g[0], RHO * g[1], RHO * g[2]]; nodes.len()],
        ),
        (
            "centrifugal",
            Load::centrifugal(axis, origin, omega),
            nodes
                .iter()
                .map(|&x| {
                    let r = perp(x);
                    let s = RHO * omega * omega;
                    [s * r[0], s * r[1], s * r[2]]
                })
                .collect(),
        ),
        (
            "nodal body force",
            Load::body_force_nodal(nodes.iter().map(|x| [x[0] + 2.0 * x[1], -x[2], 3.0 - x[0]]).collect()),
            nodes.iter().map(|x| [x[0] + 2.0 * x[1], -x[2], 3.0 - x[0]]).collect(),
        ),
    ];
    for (name, load, b) in cases {
        let mut m = base.clone();
        m.add_load(load).unwrap();
        let f = nodal_load_vector(&m, &[]).unwrap();
        let expected = consistent_nodal(&m, &b, t);
        let dofs = m.dof_map();
        let big = expected.iter().flatten().fold(0.0f64, |a, v| a.max(v.abs()));
        let mut err = 0.0f64;
        for (n, ex) in expected.iter().enumerate() {
            for k in 0..3 {
                err = err.max((f[dofs.equation(n, k).unwrap()] - ex[k]).abs());
                err = err.max(f[dofs.equation(n, 3 + k).unwrap()].abs());
            }
        }
        c.at_most(
            &format!("LD1 {name}: nodal loads against the closed form"),
            err / big,
            1e-12,
        );
    }
    // LD2: pressure.
    let p = 2500.0;
    let mut m = base.clone();
    m.add_load(Load::shell_pressure("all", p)).unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let dofs = m.dof_map();
    let normal = apply(&q, [0.0, 0.0, 1.0]);
    let total = p * 0.6 * 0.4;
    let mut sum = [0.0; 3];
    let mut moment = 0.0f64;
    for n in 0..nodes.len() {
        for k in 0..3 {
            sum[k] += f[dofs.equation(n, k).unwrap()];
            moment = moment.max(f[dofs.equation(n, 3 + k).unwrap()].abs());
        }
    }
    let err = norm([
        sum[0] - total * normal[0],
        sum[1] - total * normal[1],
        sum[2] - total * normal[2],
    ]);
    c.at_most("LD2 pressure: |sum F - p A n| / (p A)", err / total, 1e-12);
    c.at_most("LD2 pressure: nodal moments / (p A)", moment / total, 1e-12);
    // LD3: free thermal expansion of the flat plate held at node 0.
    let dt = 50.0;
    let mut m = load_plate(None, mat());
    let n = m.mesh().nodes().len();
    m.add_load(Load::temperature(vec![293.0 + dt; n], 293.0)).unwrap();
    let pres: Vec<Prescribed> = (0..6).map(|comp| Prescribed::new(0, comp, 0.0)).collect();
    let r = solve_static_with(&m, &pres, &[], &StaticOptions::default()).unwrap();
    let mut err = 0.0f64;
    let mut scale = 0.0f64;
    for (k, x) in m.mesh().nodes().iter().enumerate() {
        let u = r.displacement(k);
        let ex = [alpha * dt * x[0], alpha * dt * x[1], 0.0];
        for i in 0..3 {
            err = err.max((u[i] - ex[i]).abs());
            scale = scale.max(ex[i].abs());
        }
    }
    c.at_most(
        "LD3 thermal: translations against alpha dT (x - x_0)",
        err / scale,
        1e-8,
    );
    let st = shell::stresses(&m, r.displacement_values()).unwrap();
    let nmax = st.iter().fold(0.0f64, |a, (_, s)| a.max(max_abs(&s.membrane)));
    c.at_most(
        "LD3 thermal: membrane forces / (E alpha dT t)",
        nmax / (E * alpha * dt * t),
        1e-8,
    );
    // M1: one element in space.
    let tri = [[0.0, 0.0], [0.6, 0.1], [0.2, 0.4]];
    let origin = [0.3, -0.2, 0.5];
    let place = |p: [f64; 2]| -> Vec3 {
        let r = apply(&q, [p[0], p[1], 0.0]);
        [origin[0] + r[0], origin[1] + r[1], origin[2] + r[2]]
    };
    let x: Vec<Vec3> = tri.iter().map(|&p| place(p)).collect();
    let m = model_of(tri_mesh(x, &[[0, 1, 2]], t), material(0.3, RHO));
    let mp = mass_properties(&m, &consistent(&m).unwrap()).unwrap();
    let area =
        0.5 * ((tri[1][0] - tri[0][0]) * (tri[2][1] - tri[0][1]) - (tri[2][0] - tri[0][0]) * (tri[1][1] - tri[0][1]));
    c.at_most("M1 mass", rel(mp.mass, RHO * t * area), 1e-12);
    let cx = [
        (tri[0][0] + tri[1][0] + tri[2][0]) / 3.0,
        (tri[0][1] + tri[1][1] + tri[2][1]) / 3.0,
    ];
    let centre = place(cx);
    let dc = norm([
        mp.centre[0] - centre[0],
        mp.centre[1] - centre[1],
        mp.centre[2] - centre[2],
    ]);
    c.at_most("M1 centre", dc / 0.6, 1e-12);
    // Second moments about the centroid: A / 12 sum d_i d_i^T.
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for p in &tri {
        let d = [p[0] - cx[0], p[1] - cx[1]];
        sxx += area / 12.0 * d[0] * d[0];
        syy += area / 12.0 * d[1] * d[1];
        sxy += area / 12.0 * d[0] * d[1];
    }
    let rot = RHO * t * t * t * area / 12.0;
    let jl = [
        [RHO * t * syy + rot, -RHO * t * sxy, 0.0],
        [-RHO * t * sxy, RHO * t * sxx + rot, 0.0],
        [0.0, 0.0, RHO * t * (sxx + syy) + rot],
    ];
    let mut expected = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            expected[i][j] = (0..3)
                .map(|a| (0..3).map(|b| q[i][a] * jl[a][b] * q[j][b]).sum::<f64>())
                .sum();
        }
    }
    let big = expected.iter().flatten().fold(0.0f64, |m, v| m.max(v.abs()));
    let mut err = 0.0f64;
    for i in 0..3 {
        for j in 0..3 {
            err = err.max((mp.inertia[i][j] - expected[i][j]).abs());
        }
    }
    c.at_most("M1 inertia", err / big, 1e-12);
    c.finish();
}
