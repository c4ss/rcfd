//! F-549 (ADR 0470): a drilling membrane for Shell4 matching the ANDES edge, four overlapping ANDES OPT triangles. The checks
//! of the registration, in its order; every limit is the registered one and every prototype number is that of
//! `f549-scratch/est*.log`.

use rcfd_core::vec3::{cross, dot, norm, Vec3};
use rcfd_fem::assemble::{assemble_with, stiffness};
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::shell::{self, Shell4Membrane, ShellOptions};
use rcfd_fem::shell3::{self, Shell3Membrane, Shell3Options, ANDES_ALPHA_B};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{BlockProperty, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::skyline::SkylineLdlt;

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
            "{name}: {value:.10} (limit < {limit:e}) {}",
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
const QUAD: Shell4Membrane = Shell4Membrane::AndesOpt;
const ANDES: Shell3Membrane = Shell3Membrane::AndesOpt;

fn material(nu: f64, rho: f64) -> Material {
    Material::isotropic_elastic(E, nu, rho, 0.0).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn opts(m: Shell4Membrane) -> ShellOptions {
    ShellOptions::default().membrane(m)
}

/// A mesh of one Shell4 block with a membrane, element set "all", node set "all".
fn quad_mesh(nodes: Vec<Vec3>, quads: &[[usize; 4]], thickness: f64, membrane: Shell4Membrane) -> SolidMesh {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, quads.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell4_with_membrane(thickness, membrane).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..quads.len()).collect()).unwrap();
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

/// A grid on the node lines `xs` x `ys` at z = 0, nodes in rows of constant y; cells row by row.
fn grid_lines(xs: &[f64], ys: &[f64]) -> (Vec<Vec3>, Vec<[usize; 4]>) {
    let nx = xs.len() - 1;
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for &y in ys {
        for &x in xs {
            nodes.push([x, y, 0.0]);
        }
    }
    let mut el = Vec::new();
    for j in 0..ys.len() - 1 {
        for i in 0..nx {
            el.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    (nodes, el)
}

fn grid(a: f64, b: f64, nx: usize, ny: usize) -> (Vec<Vec3>, Vec<[usize; 4]>) {
    let xs: Vec<f64> = (0..=nx).map(|i| a * i as f64 / nx as f64).collect();
    let ys: Vec<f64> = (0..=ny).map(|j| b * j as f64 / ny as f64).collect();
    grid_lines(&xs, &ys)
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

/// Solves the model with a stiffness assembled element by element (the control N2), no load.
fn solve_assembled<F>(m: &SolidModel, pres: &[Prescribed], f: F) -> Vec<f64>
where
    F: Fn(&rcfd_fem::assemble::ElementContext) -> rcfd_fem::Result<rcfd_fem::element::ElementMatrix> + Sync,
{
    let k = assemble_with(m, None, f).unwrap();
    let load = vec![0.0; k.n];
    let fixed = prescribed_equations(m, pres).unwrap();
    let sys = reduce(&k, &load, &fixed).unwrap();
    let ldlt = SkylineLdlt::factor(&sys.matrix).unwrap();
    sys.expand(&ldlt.solve(&sys.rhs))
}

// ------------------------------------------------------------------------------------ CQ1, CQ2, P1, N1, O1 to O3

const L: f64 = 1.0;
const B: f64 = 0.25;
const T_STRIP: f64 = 0.01;

/// The strip of ADR 0466 (a1) in `nx x ny` Shell4 cells with a membrane; `distort` moves the interior node columns along x
/// by `+- h / 4` at the top and bottom edges, alternating (ADR 0470 arithmetic 5).
fn strip(nx: usize, ny: usize, membrane: Shell4Membrane, distort: bool) -> SolidModel {
    let (mut nodes, quads) = grid(L, B, nx, ny);
    if distort {
        let h = L / nx as f64;
        for j in 0..=ny {
            for i in 1..nx {
                let s = if i % 2 == 0 { 1.0 } else { -1.0 };
                let half = ny as f64 / 2.0;
                nodes[j * (nx + 1) + i][0] += 0.25 * h * s * (j as f64 - half) / half;
            }
        }
    }
    model_of(quad_mesh(nodes, &quads, T_STRIP, membrane), material(0.0, RHO))
}

/// The tip deflection on the axis of the strip clamped at x = 0 (all six unknowns, or the five without `theta_z` when
/// `hold_theta` is false), loaded by the consistent forces of the parabolic shear `P = 1 N` (`couple = false`) or of the
/// linear `sigma_xx` of the couple `M = 1 N m` (ADR 0466's test, unchanged).
fn strip_tip(m: &SolidModel, nx: usize, ny: usize, couple: bool, hold_theta: bool) -> f64 {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut pres = Vec::new();
    for j in 0..=ny {
        for comp in 0..6 {
            if comp < 5 || hold_theta {
                pres.push(Prescribed::new(idx(0, j), comp, 0.0));
            }
        }
    }
    let ys: Vec<f64> = (0..=ny).map(|j| B * j as f64 / ny as f64).collect();
    let c = B / 2.0;
    let i_z = T_STRIP * B * B * B / 12.0;
    let q = |y: f64| -> f64 {
        if couple {
            -(y - c) / i_z * T_STRIP
        } else {
            1.5 / B * (1.0 - ((y - c) / c).powi(2))
        }
    };
    let mut f = vec![0.0; ny + 1];
    for j in 0..ny {
        let (y0, y1) = (ys[j], ys[j + 1]);
        let h = y1 - y0;
        let qm = q(0.5 * (y0 + y1));
        f[j] += h / 6.0 * (q(y0) + 2.0 * qm);
        f[j + 1] += h / 6.0 * (2.0 * qm + q(y1));
    }
    let forces: Vec<NodalForceAt> = (0..=ny)
        .map(|j| {
            let force = if couple { [f[j], 0.0, 0.0] } else { [0.0, f[j], 0.0] };
            NodalForceAt::new(idx(nx, j), force)
        })
        .collect();
    let r = solve_static_with(m, &pres, &forces, &StaticOptions::default()).unwrap();
    r.displacement(idx(nx, ny / 2))[1]
}

fn timoshenko() -> f64 {
    let i_z = T_STRIP * B * B * B / 12.0;
    let g = E / 2.0;
    L * L * L / (3.0 * E * i_z) + L / (5.0 / 6.0 * g * B * T_STRIP)
}

#[test]
fn cq_in_plane_bending_of_the_cantilever_strip_on_quadrilaterals() {
    let mut c = Checks::new();
    let i_z = T_STRIP * B * B * B / 12.0;
    let delta = timoshenko();
    let x1 = strip_tip(&strip(8, 2, QUAD, false), 8, 2, false, true) / delta;
    println!("CQ1 u_y / delta = {x1:.10}");
    c.at_most("CQ1 8 x 2 Shell4 ANDES: |u_y / delta - 1|", (x1 - 1.0).abs(), 2e-2);
    let x2 = strip_tip(&strip(8, 2, QUAD, false), 8, 2, true, true) / (L * L / (2.0 * E * i_z));
    println!("CQ2 u_y / (M L^2 / (2 E I)) = {x2:.10}");
    c.at_most("CQ2 8 x 2, end couple", (x2 - 1.0).abs(), 2e-2);
    let x16 = strip_tip(&strip(16, 4, QUAD, false), 16, 4, false, true) / delta;
    println!("16 x 4 u_y / delta = {x16:.10}");
    let n1 = strip_tip(&strip(8, 2, Shell4Membrane::Bilinear, false), 8, 2, false, true) / delta;
    for (name, got, proto) in [
        ("CQ1", x1, 0.9957912994),
        ("CQ2", x2, 0.9967626398),
        ("16 x 4", x16, 0.9981052714),
        ("N1", n1, 0.8838414209),
    ] {
        c.at_most(&format!("P1 {name} against the prototype"), rel(got, proto), 1e-5);
    }
    c.below("N1 default bilinear Shell4, 8 x 2: u_y / delta", n1, 0.9);
    let eb = x1 * delta / (L * L * L / (3.0 * E * i_z));
    println!("O1 (printed) CQ1 against Euler-Bernoulli P L^3 / (3 E I): {eb:.10}");
    let o2 = strip_tip(&strip(8, 2, QUAD, false), 8, 2, false, false) / delta;
    println!("O2 (printed) CQ1 with theta_z free at the clamp: {o2:.10}");
    let o3a = strip_tip(&strip(8, 2, QUAD, true), 8, 2, false, true) / delta;
    let o3b = strip_tip(&strip(8, 2, Shell4Membrane::Bilinear, true), 8, 2, false, true) / delta;
    println!("O3 (printed) distorted 8 x 2: ANDES Shell4 {o3a:.10}, bilinear {o3b:.10}");
    c.finish();
}

// ------------------------------------------------------------------------------------ MP1 to MP4, QP1, QP2, N2

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

const T_PATCH: f64 = 0.05;

/// The patch model of ADR 0454 arithmetic 17 in space: cells 0, 4, 8 Shell4 with `quad` and the others two Shell3 with
/// `tri` (`all_quads = false`), or all nine cells Shell4 with `quad`.
fn patch_model(q: &[[f64; 3]; 3], all_quads: bool, quad: Shell4Membrane, tri: Shell3Membrane) -> SolidModel {
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
            if all_quads || n % 4 == 0 {
                quads.push(cell);
            } else {
                tris.extend_from_slice(&cut(cell, n % 2));
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, quads.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell4_with_membrane(T_PATCH, quad).unwrap())
        .unwrap();
    if !tris.is_empty() {
        mesh.add_block(ElementKind::Shell3, tris.iter().flatten().copied().collect())
            .unwrap();
        mesh.set_block_property(1, BlockProperty::shell_with_membrane(T_PATCH, tri).unwrap())
            .unwrap();
    }
    mesh.add_element_set("all", (0..quads.len() + tris.len()).collect())
        .unwrap();
    model_of(mesh, material(0.3, RHO))
}

#[test]
fn mp_qp_patch_tests_with_the_andes_shell4() {
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
    let (t, nu) = (T_PATCH, 0.3);
    let n_exact = mul3(&plane(E, nu, t), [a[1], b[2], a[2] + b[1]]);
    let m_exact = mul3(&plane(E, nu, t * t * t / 12.0), [c1, c2, c3]);
    let prescribed = |exact: &[[f64; 6]]| -> Vec<Prescribed> {
        let mut pres = Vec::new();
        for &k in &boundary {
            for comp in 0..6 {
                pres.push(Prescribed::new(k, comp, exact[k][comp]));
            }
        }
        pres
    };
    let interior_error = |model: &SolidModel, u: &[f64], exact: &[[f64; 6]]| -> f64 {
        let dofs = model.dof_map();
        let mut err = 0.0f64;
        for &k in &interior {
            for comp in 0..6 {
                let eq = dofs.equation(k, comp).unwrap();
                err = err.max((u[eq] - exact[k][comp]).abs());
            }
        }
        err
    };
    for (label, all_quads) in [("mixed", false), ("all Shell4", true)] {
        let model = patch_model(&q, all_quads, QUAD, ANDES);
        for (name, field, is_membrane) in [
            ("membrane", &membrane as &dyn Fn([f64; 2]) -> [f64; 6], true),
            ("bending", &bending as &dyn Fn([f64; 2]) -> [f64; 6], false),
        ] {
            let exact: Vec<[f64; 6]> = PATCH.iter().map(|&p| field(p)).collect();
            let scale = exact.iter().flatten().fold(0.0f64, |m, v| m.max(v.abs()));
            let r = solve_static_with(&model, &prescribed(&exact), &[], &StaticOptions::default());
            if !all_quads && is_membrane {
                c.holds(
                    "RF1 (a) the mixed ANDES Shell3 + ANDES Shell4 patch is taken (no refusal)",
                    r.is_ok(),
                    &format!("{:?}", r.as_ref().err()),
                );
            }
            let u = r.unwrap().displacement_values().to_vec();
            let err = interior_error(&model, &u, &exact);
            let id = match (all_quads, is_membrane) {
                (false, true) => "MP1",
                (false, false) => "MP3",
                (true, true) => "QP1",
                (true, false) => "QP2",
            };
            c.at_most(
                &format!("{id} {label} {name} patch: interior unknowns"),
                err / scale,
                1e-8,
            );
            if all_quads {
                continue;
            }
            let st = shell::stresses(&model, &u).unwrap();
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
            if is_membrane {
                c.at_most("MP2 mixed membrane patch: N against A_m eps", e_main / big, 1e-8);
                c.at_most("MP2 mixed membrane patch: m and Q", e_other / big, 1e-8);
            } else {
                c.at_most("MP4 mixed bending patch: m against D_b kappa", e_main / big, 1e-8);
                c.at_most("MP4 mixed bending patch: Q and N times the side", e_other / big, 1e-8);
            }
        }
    }
    // N2: the mixed membrane patch with the default Shell4 (ADR 0466 PT5), assembled element by element.
    let model = patch_model(&q, false, Shell4Membrane::Bilinear, ANDES);
    let exact: Vec<[f64; 6]> = PATCH.iter().map(|&p| membrane(p)).collect();
    let scale = exact.iter().flatten().fold(0.0f64, |m, v| m.max(v.abs()));
    let andes3 = Shell3Options::default().membrane(ANDES);
    let u = solve_assembled(&model, &prescribed(&exact), |ctx| {
        if ctx.kind == ElementKind::Shell3 {
            shell3::element_stiffness_with(ctx.coords, t, ctx.material, &andes3)
        } else {
            shell::element_stiffness(ctx.coords, t, ctx.material)
        }
    });
    c.above(
        "N2 mixed membrane patch with the default Shell4 (ADR 0466 PT5): interior unknowns",
        interior_error(&model, &u, &exact) / scale,
        1e-2,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ BE1

#[test]
fn be_pure_bending_of_a_rectangle_is_exact() {
    let mut c = Checks::new();
    let t = 0.01;
    let mut worst = 0.0f64;
    for (a, b) in [(0.25, 1.0), (1.0, 1.0), (4.0, 1.0)] {
        for nu in [0.0, 0.3] {
            let mat = material(nu, RHO);
            let (x0, y0) = (0.2, 0.1);
            let x: Vec<Vec3> = vec![[x0, y0, 0.0], [x0 + a, y0, 0.0], [x0 + a, y0 + b, 0.0], [x0, y0 + b, 0.0]];
            let k = shell::element_stiffness_with(&x, t, &mat, &opts(QUAD)).unwrap();
            for along_x in [true, false] {
                let kb = 1e-3;
                let mut d = vec![0.0; 24];
                for (i, p) in x.iter().enumerate() {
                    let (px, py) = (p[0], p[1]);
                    let f = if along_x {
                        [-kb * px * py, kb * (px * px + nu * py * py) / 2.0, kb * px]
                    } else {
                        [kb * (py * py + nu * px * px) / 2.0, -kb * px * py, -kb * py]
                    };
                    d[6 * i] = f[0];
                    d[6 * i + 1] = f[1];
                    d[6 * i + 5] = f[2];
                }
                let mut u = 0.0;
                for i in 0..24 {
                    for j in 0..24 {
                        u += 0.5 * d[i] * k.get(i, j) * d[j];
                    }
                }
                let second = if along_x {
                    a * ((y0 + b).powi(3) - y0.powi(3)) / 3.0
                } else {
                    b * ((x0 + a).powi(3) - x0.powi(3)) / 3.0
                };
                let exact = 0.5 * E * kb * kb * t * second;
                worst = worst.max((u / exact - 1.0).abs());
            }
        }
    }
    c.at_most("BE1 rectangles in pure bending: max |U / U_exact - 1|", worst, 1e-10);
    c.finish();
}

// ------------------------------------------------------------------------------------------- SE1, SE2, ISO1, D1, FR1

/// The eigenvalues of a symmetric matrix by cyclic Jacobi rotations (test code, that of ADR 0466's test), ascending.
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

/// The count of `|lambda| <= 1e-10 lambda_max` and `lambda_7 / lambda_max`.
fn zero_count(k: &[f64], n: usize) -> (usize, f64) {
    let ev = jacobi_eigenvalues(k, n);
    let lmax = ev[n - 1];
    (ev.iter().filter(|l| l.abs() <= 1e-10 * lmax).count(), ev[6] / lmax)
}

/// The warped element of ADR 0470 arithmetic 3.
fn element_in_space() -> Vec<Vec3> {
    vec![[0.3, -0.2, 0.5], [1.4, 0.0, 0.52], [1.2, 0.75, 0.47], [0.4, 0.85, 0.515]]
}

#[test]
fn se_iso_d_single_element_and_fr_free_plate() {
    let mut c = Checks::new();
    let x = element_in_space();
    let mat = material(0.3, RHO);
    let centre = {
        let mut s = [0.0; 3];
        for p in &x {
            for k in 0..3 {
                s[k] += 0.25 * p[k];
            }
        }
        s
    };
    for t in [0.01, 0.1] {
        let k = shell::element_stiffness_with(&x, t, &mat, &opts(QUAD)).unwrap();
        let (zeros, l7) = zero_count(k.values(), 24);
        c.holds(
            &format!("SE1 t = {t}: six zero eigenvalues"),
            zeros == 6,
            &format!("{zeros}"),
        );
        c.above(&format!("SE1 t = {t}: lambda_7 / lambda_max"), l7, 1e-7);
        let kmax = max_abs(k.values());
        let mut res = 0.0f64;
        for mode in 0..6 {
            let mut r = [0.0; 24];
            for (i, p) in x.iter().enumerate() {
                if mode < 3 {
                    r[6 * i + mode] = 1.0;
                } else {
                    let mut th = [0.0; 3];
                    th[mode - 3] = 1.0;
                    let d = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
                    let u = cross(th, d);
                    r[6 * i..6 * i + 3].copy_from_slice(&u);
                    r[6 * i + 3..6 * i + 6].copy_from_slice(&th);
                }
            }
            let rmax = max_abs(&r);
            for i in 0..24 {
                let s: f64 = (0..24).map(|j| k.get(i, j) * r[j]).sum();
                res = res.max(s.abs() / (kmax * rmax));
            }
        }
        c.at_most(&format!("SE2 t = {t}: rigid motions |K r|"), res, 1e-11);
        let mut sym = true;
        for i in 0..24 {
            for j in 0..24 {
                sym &= k.get(i, j).to_bits() == k.get(j, i).to_bits();
            }
        }
        c.holds(&format!("SE2 t = {t}: K symmetric bit for bit"), sym, "");
        let mut worst = 0.0f64;
        for p in [[1usize, 2, 3, 0], [2, 3, 0, 1]] {
            let xp: Vec<Vec3> = p.iter().map(|&i| x[i]).collect();
            let kp = shell::element_stiffness_with(&xp, t, &mat, &opts(QUAD)).unwrap();
            for a in 0..4 {
                for b in 0..4 {
                    for ca in 0..6 {
                        for cb in 0..6 {
                            let v = kp.get(6 * a + ca, 6 * b + cb) - k.get(6 * p[a] + ca, 6 * p[b] + cb);
                            worst = worst.max(v.abs() / kmax);
                        }
                    }
                }
            }
        }
        c.at_most(&format!("ISO1 t = {t}: relabelled against K"), worst, 1e-11);
    }
    // D1: the model takes the block's Shell4 membrane.
    let m = model_of(quad_mesh(x.clone(), &[[0, 1, 2, 3]], 0.02, QUAD), mat.clone());
    let kg = stiffness(&m).unwrap();
    let ke = shell::element_stiffness_with(&x, 0.02, &mat, &opts(QUAD)).unwrap();
    let mut same = true;
    for i in 0..24 {
        for p in kg.row_ptr[i]..kg.row_ptr[i + 1] {
            same &= kg.val[p].to_bits() == ke.get(i, kg.col[p]).to_bits();
        }
    }
    c.holds("D1 the model stiffness equals the element's bit for bit", same, "");
    // FR1: the free 4 x 4 plate, all Shell4 with the new membrane.
    let (nodes, quads) = grid(1.0, 0.8, 4, 4);
    let m = model_of(quad_mesh(nodes, &quads, 0.1, QUAD), material(0.3, RHO));
    let k = stiffness(&m).unwrap();
    let n = k.n;
    let mut dense = vec![0.0; n * n];
    for i in 0..n {
        for p in k.row_ptr[i]..k.row_ptr[i + 1] {
            dense[i * n + k.col[p]] = k.val[p];
        }
    }
    let (zeros, l7) = zero_count(&dense, n);
    println!("FR1 lambda_7 / lambda_max = {l7:.3e}");
    c.holds(
        "FR1 free plate: six zero eigenvalues",
        zeros == 6,
        &format!("{zeros}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- TH1, ST1

#[test]
fn th_free_thermal_expansion_and_st_drilling_stress() {
    let mut c = Checks::new();
    let (t, alpha, dt) = (0.01, 1.2e-5, 50.0);
    let (nodes, quads) = grid_lines(&[0.0, 0.15, 0.35, 0.6], &[0.0, 0.18, 0.4]);
    let n = nodes.len();
    let mat = Material::isotropic_elastic(E, 0.3, RHO, alpha).unwrap();
    let mut m = model_of(quad_mesh(nodes, &quads, t, QUAD), mat);
    m.add_load(Load::temperature(vec![293.0 + dt; n], 293.0)).unwrap();
    let pres: Vec<Prescribed> = (0..6).map(|comp| Prescribed::new(0, comp, 0.0)).collect();
    let r = solve_static_with(&m, &pres, &[], &StaticOptions::default()).unwrap();
    let x0 = m.mesh().nodes()[0];
    let (mut err, mut scale) = (0.0f64, 0.0f64);
    for (k, x) in m.mesh().nodes().iter().enumerate() {
        let u = r.displacement(k);
        for comp in 0..3 {
            let ex = alpha * dt * (x[comp] - x0[comp]);
            err = err.max((u[comp] - ex).abs());
            scale = scale.max(ex.abs());
        }
    }
    c.at_most(
        "TH1 thermal: translations against alpha dT (x - x_0)",
        err / scale,
        1e-8,
    );
    let st = shell::stresses(&m, r.displacement_values()).unwrap();
    let nmax = st.iter().fold(0.0f64, |a, (_, s)| a.max(max_abs(&s.membrane)));
    c.at_most(
        "TH1 thermal: membrane forces / (E alpha dT t)",
        nmax / (E * alpha * dt * t),
        1e-8,
    );
    // ST1: one element in its plane, only drilling rotations.
    let x: Vec<Vec3> = vec![[0.0, 0.0, 0.0], [1.1, 0.1, 0.0], [0.9, 0.8, 0.0], [0.15, 1.0, 0.0]];
    let nu = 0.3;
    let m = model_of(quad_mesh(x.clone(), &[[0, 1, 2, 3]], t, QUAD), material(nu, RHO));
    let theta = [1e-3, -2e-3, 0.5e-3, 0.8e-3];
    let mut u = vec![0.0; 24];
    for i in 0..4 {
        u[6 * i + 5] = theta[i];
    }
    let st = shell::stresses(&m, &u).unwrap();
    let s = st[0].1;
    // The mean basic strain in the element's axes: (alpha_b / A) sum_edges (l^2 / 12)(theta_j - theta_i) n (x) n.
    let (e1, e2) = (s.axes[0], s.axes[1]);
    let loc: Vec<[f64; 2]> = x.iter().map(|p| [dot(*p, e1), dot(*p, e2)]).collect();
    let mut area = 0.0;
    for i in 0..4 {
        let j = (i + 1) % 4;
        area += 0.5 * (loc[i][0] * loc[j][1] - loc[j][0] * loc[i][1]);
    }
    let mut eps = [0.0; 3];
    for i in 0..4 {
        let j = (i + 1) % 4;
        let d = [loc[j][0] - loc[i][0], loc[j][1] - loc[i][1]];
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let nrm = [d[1] / l, -d[0] / l];
        let w = ANDES_ALPHA_B / area * l * l / 12.0 * (theta[j] - theta[i]);
        eps[0] += w * nrm[0] * nrm[0];
        eps[1] += w * nrm[1] * nrm[1];
        eps[2] += 2.0 * w * nrm[0] * nrm[1];
    }
    let n_exact = mul3(&plane(E, nu, t), eps);
    let diff = (0..3).fold(0.0f64, |a, k| a.max((s.membrane[k] - n_exact[k]).abs()));
    c.at_most(
        "ST1 drilling-only state: N against A_m eps of the closed form",
        diff / max_abs(&n_exact),
        1e-12,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------ RF1, RF2

fn membrane_refusal<T: std::fmt::Debug>(r: &Result<T, FemError>) -> bool {
    match r {
        Err(FemError::Element { source, .. }) => {
            matches!(source.as_ref(), FemError::InvalidValue { key: "membrane", reason } if reason.contains("shares the edge") && reason.contains("with element"))
        }
        _ => false,
    }
}

/// Two cells side by side: the first a Shell4 with the new membrane (block 0), the second (block 1) a Shell4 with the default
/// membrane (`other = None`) or two Shell3 with the membrane `other`.
fn two_blocks(other: Option<Shell3Membrane>) -> SolidModel {
    let (nodes, quads) = grid(2.0, 1.0, 2, 1);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, quads[0].to_vec()).unwrap();
    mesh.set_block_property(0, BlockProperty::shell4_with_membrane(0.01, QUAD).unwrap())
        .unwrap();
    let ne = match other {
        None => {
            mesh.add_block(ElementKind::Shell4, quads[1].to_vec()).unwrap();
            mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
            2
        }
        Some(mem) => {
            mesh.add_block(
                ElementKind::Shell3,
                cut(quads[1], 0).iter().flatten().copied().collect(),
            )
            .unwrap();
            mesh.set_block_property(1, BlockProperty::shell_with_membrane(0.01, mem).unwrap())
                .unwrap();
            3
        }
    };
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    model_of(mesh, material(0.3, RHO))
}

#[test]
fn rf_mixed_membranes_refused_and_the_andes_pairing_taken() {
    let mut c = Checks::new();
    for (id, other) in [
        ("RF1 (b) next to a default Shell4", None),
        (
            "RF1 (c) next to a constant-strain Shell3",
            Some(Shell3Membrane::ConstantStrain),
        ),
        ("RF1 (d) next to an Allman Shell3", Some(Shell3Membrane::Allman)),
    ] {
        let r = solve_static(&two_blocks(other), &StaticOptions::default());
        println!("{id}: {:?}", r.as_ref().err());
        c.holds(&format!("{id}: refused (membrane)"), membrane_refusal(&r), "");
    }
    let q = rotation([1.0, -2.0, 2.0], 0.7);
    let r = solve_static(
        &patch_model(&q, false, Shell4Membrane::Bilinear, ANDES),
        &StaticOptions::default(),
    );
    println!("RF1 (e): {:?}", r.as_ref().err());
    c.holds(
        "RF1 (e) ANDES Shell3 next to the default Shell4 still refused (membrane)",
        membrane_refusal(&r),
        "",
    );
    // RF2: the property on a Shell3 block, on a Shell4 block, and t = 0.
    let (nodes, quads) = grid(1.0, 1.0, 1, 1);
    let mut mesh = SolidMesh::new(nodes.clone()).unwrap();
    mesh.add_block(
        ElementKind::Shell3,
        cut(quads[0], 0).iter().flatten().copied().collect(),
    )
    .unwrap();
    let r = mesh.set_block_property(0, BlockProperty::shell4_with_membrane(0.01, QUAD).unwrap());
    println!("RF2 {:?}", r.as_ref().err());
    c.holds(
        "RF2 the Shell4 membrane on a Shell3 block refused (UnsupportedKind, a Shell4 membrane)",
        matches!(
            r,
            Err(FemError::UnsupportedKind {
                block: 0,
                operation: "a Shell4 membrane",
                ..
            })
        ),
        "",
    );
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, quads[0].to_vec()).unwrap();
    let r = mesh.set_block_property(0, BlockProperty::shell4_with_membrane(0.01, QUAD).unwrap());
    c.holds("RF2 the Shell4 membrane on a Shell4 block taken", r.is_ok(), "");
    let r = BlockProperty::shell4_with_membrane(0.0, QUAD);
    c.holds(
        "RF2 thickness 0 refused (thickness)",
        matches!(r, Err(FemError::InvalidValue { key: "thickness", .. })),
        "",
    );
    c.finish();
}
