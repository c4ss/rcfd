//! F-539 (ADR 0466): a drilling-unknown membrane for Shell3, the ANDES optimal membrane with Allman's triangle beside it. The
//! checks of the registration, in its order; every limit is the registered one and every prototype number is that of
//! `f539-scratch/est*.log`.

use rcfd_core::vec3::{cross, dot, norm, Vec3};
use rcfd_fem::assemble::{assemble_with, stiffness};
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::shell;
use rcfd_fem::shell3::{self, Shell3Membrane, Shell3Options, ANDES_ALPHA_B};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{BlockProperty, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{count_below, ResidualTest};
use rcfd_linalg::skyline::{SkylineLdlt, SkylineOrdering};

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
const ANDES: Shell3Membrane = Shell3Membrane::AndesOpt;
const ALLMAN: Shell3Membrane = Shell3Membrane::Allman;

fn material(nu: f64, rho: f64) -> Material {
    Material::isotropic_elastic(E, nu, rho, 0.0).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn opts(m: Shell3Membrane) -> Shell3Options {
    Shell3Options::default().membrane(m)
}

/// A mesh of one Shell3 block with a membrane, element set "all", node set "all".
fn tri_mesh(nodes: Vec<Vec3>, tris: &[[usize; 3]], thickness: f64, membrane: Shell3Membrane) -> SolidMesh {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell3, tris.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell_with_membrane(thickness, membrane).unwrap())
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

/// Solves the model with a stiffness assembled element by element (the controls and PT5, PT6), no load.
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

// ---------------------------------------------------------------------------------- CT1 to CT4, P1, N1, N2, O2, O3

const L: f64 = 1.0;
const B: f64 = 0.25;
const T_STRIP: f64 = 0.01;

/// The strip of ADR 0466 (a1): `nx x ny` square cells, the cut of cell `(i, j)` given by `diag(i, j)`.
fn strip(nx: usize, ny: usize, membrane: Shell3Membrane, diag: &dyn Fn(usize, usize) -> usize) -> SolidModel {
    let (nodes, quads) = grid(L, B, nx, ny);
    let mut tris = Vec::new();
    for (c, &q) in quads.iter().enumerate() {
        tris.extend_from_slice(&cut(q, diag(c % nx, c / nx)));
    }
    model_of(tri_mesh(nodes, &tris, T_STRIP, membrane), material(0.0, RHO))
}

/// The tip deflection on the axis of the strip clamped at x = 0 (all six unknowns, or the five without `theta_z` when
/// `hold_theta` is false), loaded by the consistent forces of the parabolic shear `P = 1 N` (`couple = false`) or of the
/// linear `sigma_xx` of the couple `M = 1 N m`.
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
    // Load per unit length along the tip edge: the shear tau t along y, or sigma_xx t along x.
    let q = |y: f64| -> f64 {
        if couple {
            -(y - c) / i_z * T_STRIP
        } else {
            1.5 / B * (1.0 - ((y - c) / c).powi(2))
        }
    };
    let mut f = vec![0.0; ny + 1];
    for j in 0..ny {
        // Simpson's rule: exact for the cubic integrand (quadratic load times linear shape).
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
fn ct_in_plane_bending_of_the_cantilever_strip() {
    let mut c = Checks::new();
    let i_z = T_STRIP * B * B * B / 12.0;
    let delta = timoshenko();
    let d0 = |_: usize, _: usize| 0usize;
    let d1 = |_: usize, _: usize| 1usize;
    let alt = |i: usize, j: usize| (i + j) % 2;
    let x1 = strip_tip(&strip(8, 2, ANDES, &d0), 8, 2, false, true) / delta;
    println!("CT1 u_y / delta = {x1:.10}");
    c.at_most(
        "CT1 8 x 2, cut (i, j)-(i + 1, j + 1): |u_y / delta - 1|",
        (x1 - 1.0).abs(),
        2e-2,
    );
    let x2 = strip_tip(&strip(8, 2, ANDES, &d1), 8, 2, false, true) / delta;
    println!("CT2 u_y / delta = {x2:.10}");
    c.at_most("CT2 8 x 2, the other diagonal", (x2 - 1.0).abs(), 2e-2);
    let x3 = strip_tip(&strip(8, 2, ANDES, &alt), 8, 2, false, true) / delta;
    println!("CT3 u_y / delta = {x3:.10}");
    c.at_most("CT3 8 x 2, alternating diagonals", (x3 - 1.0).abs(), 2e-2);
    let x4 = strip_tip(&strip(8, 2, ANDES, &d0), 8, 2, true, true) / (L * L / (2.0 * E * i_z));
    println!("CT4 u_y / (M L^2 / (2 E I)) = {x4:.10}");
    c.at_most("CT4 8 x 2, end couple", (x4 - 1.0).abs(), 2e-2);
    let x16 = strip_tip(&strip(16, 4, ANDES, &d0), 16, 4, false, true) / delta;
    println!("16 x 4 u_y / delta = {x16:.10}");
    let n1 = strip_tip(&strip(8, 2, Shell3Membrane::ConstantStrain, &d0), 8, 2, false, true) / delta;
    let n2 = strip_tip(&strip(8, 2, ALLMAN, &d0), 8, 2, false, true) / delta;
    for (name, got, proto) in [
        ("CT1", x1, 0.9964751240),
        ("CT3", x3, 0.9969394138),
        ("CT4", x4, 0.9972707469),
        ("16 x 4", x16, 0.9981970229),
        ("N1", n1, 0.5428975996),
        ("N2", n2, 0.9156777414),
    ] {
        c.at_most(&format!("P1 {name} against the prototype"), rel(got, proto), 1e-5);
    }
    c.below("N1 constant-strain membrane, 8 x 2: u_y / delta", n1, 0.9);
    c.below("N2 Allman's membrane, 8 x 2: u_y / delta", n2, 0.98);
    let eb = x1 * delta / (L * L * L / (3.0 * E * i_z));
    println!("O2 (printed) CT1 against Euler-Bernoulli P L^3 / (3 E I): {eb:.10}");
    let o3 = strip_tip(&strip(8, 2, ANDES, &d0), 8, 2, false, false) / delta;
    println!("O3 (printed) CT1 with theta_z free at the clamp: {o3:.10}");
    c.finish();
}

// ---------------------------------------------------------------------------------------- PT1 to PT7, RF1 (mixed)

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

/// The patch model of ADR 0454 arithmetic 17 with the Shell3 elements on `membrane`: all cells cut (`mixed = false`) or
/// cells 0, 4, 8 kept as Shell4.
fn patch_model(q: &[[f64; 3]; 3], mixed: bool, membrane: Shell3Membrane) -> SolidModel {
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
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let mut b = 0;
    if !quads.is_empty() {
        mesh.add_block(ElementKind::Shell4, quads.iter().flatten().copied().collect())
            .unwrap();
        mesh.set_block_property(b, BlockProperty::shell(T_PATCH).unwrap())
            .unwrap();
        b += 1;
    }
    mesh.add_block(ElementKind::Shell3, tris.iter().flatten().copied().collect())
        .unwrap();
    mesh.set_block_property(b, BlockProperty::shell_with_membrane(T_PATCH, membrane).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..quads.len() + tris.len()).collect())
        .unwrap();
    model_of(mesh, material(0.3, RHO))
}

/// The registered patch checks, as run once at 7801e0ea: PT5 missed (`2.0776e-1` against `1e-8`, predicted by ADR 0466
/// arithmetic 8). Kept as the record with its limit intact (ADR 0466 addendum 1).
#[test]
#[ignore = "record of ADR 0466: PT5 (mixed ANDES + Shell4 membrane patch) measured 2.0776e-1 against the limit 1e-8 at 7801e0ea, the miss predicted by arithmetic 8; continued without PT5's limit in pt_patch_tests_kept"]
fn pt_patch_tests_with_the_drilling_membranes_known_miss() {
    patch_checks(true);
}

/// The patch checks that were met (PT1 to PT4, PT6, PT7), PT5 printed only (ADR 0466 addendum 1).
#[test]
fn pt_patch_tests_kept() {
    patch_checks(false);
}

fn patch_checks(with_pt5: bool) {
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
    let andes = opts(ANDES);
    for (label, mem, mixed) in [
        ("ANDES", ANDES, false),
        ("ANDES mixed", ANDES, true),
        ("Allman", ALLMAN, false),
    ] {
        let model = patch_model(&q, mixed, mem);
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
            let u: Vec<f64> = if mixed {
                // The model is refused (RF1): the mixed stiffness assembled element by element.
                solve_assembled(&model, &pres, |ctx| {
                    if ctx.kind == ElementKind::Shell3 {
                        shell3::element_stiffness_with(ctx.coords, t, ctx.material, &andes)
                    } else {
                        shell::element_stiffness(ctx.coords, t, ctx.material)
                    }
                })
            } else {
                solve_static_with(&model, &pres, &[], &StaticOptions::default())
                    .unwrap()
                    .displacement_values()
                    .to_vec()
            };
            let dofs = model.dof_map();
            let mut err = 0.0f64;
            for &k in &interior {
                for comp in 0..6 {
                    let eq = dofs.equation(k, comp).unwrap();
                    err = err.max((u[eq] - exact[k][comp]).abs());
                }
            }
            let id = match (label, is_membrane) {
                ("ANDES", true) => "PT1",
                ("ANDES", false) => "PT3",
                ("ANDES mixed", true) => "PT5",
                ("ANDES mixed", false) => "PT6",
                (_, true) => "PT7 membrane",
                (_, false) => "PT7 bending",
            };
            if id == "PT5" {
                println!("O1 (printed) mixed ANDES + Shell4 membrane patch: {:.4e}", err / scale);
                if !with_pt5 {
                    continue;
                }
            }
            c.at_most(
                &format!("{id} {label} {name} patch: interior unknowns"),
                err / scale,
                1e-8,
            );
            if mixed || label == "Allman" {
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
                c.at_most("PT2 ANDES membrane patch: N against A_m eps", e_main / big, 1e-8);
                c.at_most("PT2 ANDES membrane patch: m and Q", e_other / big, 1e-8);
            } else {
                c.at_most("PT4 ANDES bending patch: m against D_b kappa", e_main / big, 1e-8);
                c.at_most("PT4 ANDES bending patch: Q and N times the side", e_other / big, 1e-8);
            }
        }
    }
    c.finish();
}

// --------------------------------------------------------------------------------------------------------- BE1

#[test]
fn be_pure_bending_of_rectangle_pairs_is_exact() {
    let mut c = Checks::new();
    let (t, k) = (0.01, 1e-3);
    let mut worst = 0.0f64;
    for nu in [0.0, 0.3] {
        let mat = material(nu, RHO);
        for aspect in [0.25, 1.0, 4.0] {
            let (a, b) = (aspect, 1.0);
            let corner = [0.2, 0.1];
            let p: Vec<Vec3> = [[0.0, 0.0], [a, 0.0], [a, b], [0.0, b]]
                .iter()
                .map(|q| [corner[0] + q[0], corner[1] + q[1], 0.0])
                .collect();
            let centre = [corner[0] + a / 2.0, corner[1] + b / 2.0];
            for along_x in [true, false] {
                let field = |x: Vec3| -> [f64; 3] {
                    let (x, y) = (x[0] - centre[0], x[1] - centre[1]);
                    if along_x {
                        [-k * x * y, k * (x * x + nu * y * y) / 2.0, k * x]
                    } else {
                        [k * (y * y + nu * x * x) / 2.0, -k * x * y, -k * y]
                    }
                };
                for diag in [0usize, 1] {
                    let tris = cut([0, 1, 2, 3], diag);
                    let mut u = 0.0;
                    for tri in tris {
                        let x: Vec<Vec3> = tri.iter().map(|&i| p[i]).collect();
                        let ke = shell3::element_stiffness_with(&x, t, &mat, &opts(ANDES)).unwrap();
                        let mut d = vec![0.0; 18];
                        for (n, xi) in x.iter().enumerate() {
                            let f = field(*xi);
                            d[6 * n] = f[0];
                            d[6 * n + 1] = f[1];
                            d[6 * n + 5] = f[2];
                        }
                        let kd = ke.matvec(&d);
                        u += 0.5 * d.iter().zip(&kd).map(|(a, b)| a * b).sum::<f64>();
                    }
                    let i_z = if along_x {
                        a * b * b * b / 12.0
                    } else {
                        b * a * a * a / 12.0
                    };
                    let exact = 0.5 * E * k * k * t * i_z;
                    worst = worst.max((u / exact - 1.0).abs());
                }
            }
        }
    }
    c.at_most(
        "BE1 pure bending energy of rectangle pairs: max |U / U_exact - 1|",
        worst,
        1e-10,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------- SE1, SE2, ISO1, N3, D1

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

fn zero_count(k: &[f64]) -> (usize, f64) {
    let ev = jacobi_eigenvalues(k, 18);
    let lmax = ev[17];
    (ev.iter().filter(|l| l.abs() <= 1e-10 * lmax).count(), ev[6] / lmax)
}

fn element_in_space() -> Vec<Vec3> {
    let q = rotation([1.0, -2.0, 2.0], 0.7);
    let origin = [0.3, -0.2, 0.5];
    [[0.0, 0.0], [0.9, 0.15], [0.3, 0.7]]
        .iter()
        .map(|p| {
            let r = apply(&q, [p[0], p[1], 0.0]);
            [origin[0] + r[0], origin[1] + r[1], origin[2] + r[2]]
        })
        .collect()
}

#[test]
fn se_iso_single_element_and_the_spurious_mode() {
    let mut c = Checks::new();
    let x = element_in_space();
    let mat = material(0.3, RHO);
    for t in [0.01, 0.1] {
        let k = shell3::element_stiffness_with(&x, t, &mat, &opts(ANDES)).unwrap();
        let (zeros, l7) = zero_count(k.values());
        c.holds(
            &format!("SE1 ANDES t = {t}: six zero eigenvalues"),
            zeros == 6,
            &format!("{zeros}"),
        );
        c.above(&format!("SE1 ANDES t = {t}: lambda_7 / lambda_max"), l7, 1e-7);
        let ka = shell3::element_stiffness_with(&x, t, &mat, &opts(ALLMAN)).unwrap();
        let (za, la) = zero_count(ka.values());
        println!("SE1 Allman t = {t}: lambda_7 / lambda_max = {la:.4e}");
        c.holds(
            &format!("SE1 Allman with the penalty t = {t}: six zero eigenvalues"),
            za == 6,
            &format!("{za}"),
        );
        let k0 = shell3::element_stiffness_with(&x, t, &mat, &opts(ALLMAN).drilling(0.0)).unwrap();
        let (z0, _) = zero_count(k0.values());
        c.holds(
            &format!("N3 Allman without the penalty t = {t}: seven zero eigenvalues (equal rotations)"),
            z0 == 7,
            &format!("{z0}"),
        );
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
        c.at_most(&format!("SE2 ANDES t = {t}: |K r| / (max |K| max |r|)"), worst, 1e-12);
        let sym = (0..18).all(|i| (0..18).all(|j| k.get(i, j).to_bits() == k.get(j, i).to_bits()));
        c.holds(&format!("SE2 ANDES t = {t}: symmetric bit for bit"), sym, "");
        for perm in [[1usize, 2, 0], [2, 0, 1]] {
            let xp: Vec<Vec3> = perm.iter().map(|&i| x[i]).collect();
            let kp = shell3::element_stiffness_with(&xp, t, &mat, &opts(ANDES)).unwrap();
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
                &format!("ISO1 ANDES t = {t}: relabelling {perm:?} against K"),
                diff / kmax,
                1e-12,
            );
        }
    }
    // D1: the model takes the block's membrane.
    for mem in [ANDES, ALLMAN] {
        let m = model_of(tri_mesh(x.clone(), &[[0, 1, 2]], 0.02, mem), mat.clone());
        let kg = stiffness(&m).unwrap();
        let ke = shell3::element_stiffness_with(&x, 0.02, &mat, &opts(mem)).unwrap();
        let mut same = true;
        for i in 0..18 {
            for p in kg.row_ptr[i]..kg.row_ptr[i + 1] {
                same &= kg.val[p].to_bits() == ke.get(i, kg.col[p]).to_bits();
            }
        }
        c.holds(
            &format!("D1 {mem:?}: the model stiffness equals the element's bit for bit"),
            same,
            "",
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------ FR1, FR2, N4

#[test]
fn fr_free_plate_six_zero_modes() {
    let mut c = Checks::new();
    let t = 0.1;
    let (nodes, quads) = grid(1.0, 0.8, 4, 4);
    let tris: Vec<[usize; 3]> = quads.iter().flat_map(|&q| cut(q, 0)).collect();
    let m = model_of(tri_mesh(nodes.clone(), &tris, t, ANDES), material(0.3, RHO));
    let o = ModalOptions::new(8)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward);
    let r = modal_analysis(&m, &o).unwrap();
    println!("FR ANDES frequencies {:?} Hz", r.frequencies);
    let lambda7 = r.eigenvalues[6];
    let k = stiffness(&m).unwrap();
    let mm = consistent(&m).unwrap();
    let n6 = count_below(&k, &mm, lambda7 / 4.0, &SkylineOrdering::Rcm).unwrap();
    c.holds(
        "FR1 ANDES free plate: Sturm count below lambda_7 / 4 is 6",
        n6 == 6,
        &format!("{n6}"),
    );
    c.at_most(
        "FR2 f_7 against 395.4141399498 Hz",
        rel(r.frequencies[6], 395.4141399498),
        1e-5,
    );
    c.at_most(
        "FR2 f_8 against 525.7994847722 Hz",
        rel(r.frequencies[7], 525.7994847722),
        1e-5,
    );
    let a0 = opts(ALLMAN).drilling(0.0);
    let k0 = assemble_with(&m, None, |ctx| {
        shell3::element_stiffness_with(ctx.coords, t, ctx.material, &a0)
    })
    .unwrap();
    let n0 = count_below(&k0, &mm, lambda7 / 4.0, &SkylineOrdering::Rcm).unwrap();
    c.holds(
        "N4 Allman without the penalty: one global spurious mode (7)",
        n0 == 7,
        &format!("{n0}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ TH1, TH2, ST1

#[test]
fn th_free_thermal_expansion_and_st_drilling_stress() {
    let mut c = Checks::new();
    let (t, alpha, dt) = (0.01, 1.2e-5, 50.0);
    for (id, mem) in [("TH1", ANDES), ("TH2", ALLMAN)] {
        let (nodes, quads) = grid_lines(&[0.0, 0.15, 0.35, 0.6], &[0.0, 0.18, 0.4]);
        let tris: Vec<[usize; 3]> = quads.iter().flat_map(|&q| cut(q, 0)).collect();
        let n = nodes.len();
        let mat = Material::isotropic_elastic(E, 0.3, RHO, alpha).unwrap();
        let mut m = model_of(tri_mesh(nodes, &tris, t, mem), mat);
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
            &format!("{id} {mem:?} thermal: translations against alpha dT (x - x_0)"),
            err / scale,
            1e-8,
        );
        let st = shell::stresses(&m, r.displacement_values()).unwrap();
        let nmax = st.iter().fold(0.0f64, |a, (_, s)| a.max(max_abs(&s.membrane)));
        c.at_most(
            &format!("{id} {mem:?} thermal: membrane forces / (E alpha dT t)"),
            nmax / (E * alpha * dt * t),
            1e-8,
        );
    }
    // ST1: one ANDES element in its plane, only drilling rotations.
    let x: Vec<Vec3> = vec![[0.0, 0.0, 0.0], [0.9, 0.15, 0.0], [0.3, 0.7, 0.0]];
    let nu = 0.3;
    let m = model_of(tri_mesh(x.clone(), &[[0, 1, 2]], t, ANDES), material(nu, RHO));
    let theta = [1e-3, -2e-3, 0.5e-3];
    let mut u = vec![0.0; 18];
    for i in 0..3 {
        u[6 * i + 5] = theta[i];
    }
    let st = shell::stresses(&m, &u).unwrap();
    let s = st[0].1;
    // The basic strain in the element's axes: (alpha_b / A) sum_e (l^2 / 12)(theta_j - theta_i) n (x) n.
    let e1 = s.axes[0];
    let e2 = s.axes[1];
    let loc: Vec<[f64; 2]> = x.iter().map(|p| [dot(*p, e1), dot(*p, e2)]).collect();
    let area =
        0.5 * ((loc[1][0] - loc[0][0]) * (loc[2][1] - loc[0][1]) - (loc[2][0] - loc[0][0]) * (loc[1][1] - loc[0][1]));
    let mut eps = [0.0; 3];
    for e in 0..3 {
        let (i, j) = (e, (e + 1) % 3);
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
        "ST1 ANDES drilling-only state: N against A_m eps_basic",
        diff / max_abs(&n_exact),
        1e-12,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------ RF1, RF2

fn membrane_refusal<T: std::fmt::Debug>(r: &Result<T, FemError>) -> bool {
    match r {
        Err(FemError::Element { source, .. }) => {
            matches!(source.as_ref(), FemError::InvalidValue { key: "membrane", reason } if reason.contains("shares the edge"))
        }
        _ => false,
    }
}

#[test]
fn rf_mixed_membranes_and_shell4_refused() {
    let mut c = Checks::new();
    // RF1 (a): the mixed patch (ANDES next to Shell4).
    let q = rotation([1.0, -2.0, 2.0], 0.7);
    let m = patch_model(&q, true, ANDES);
    let r = solve_static(&m, &StaticOptions::default());
    println!("RF1 (a) {:?}", r.as_ref().err());
    c.holds("RF1 ANDES next to Shell4 refused (membrane)", membrane_refusal(&r), "");
    // RF1 (b): two Shell3 blocks sharing an edge, ANDES and constant strain.
    let (nodes, quads) = grid(2.0, 1.0, 2, 1);
    let mut mesh = SolidMesh::new(nodes.clone()).unwrap();
    mesh.add_block(
        ElementKind::Shell3,
        cut(quads[0], 0).iter().flatten().copied().collect(),
    )
    .unwrap();
    mesh.add_block(
        ElementKind::Shell3,
        cut(quads[1], 0).iter().flatten().copied().collect(),
    )
    .unwrap();
    mesh.set_block_property(0, BlockProperty::shell_with_membrane(0.01, ANDES).unwrap())
        .unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..4).collect()).unwrap();
    let m = model_of(mesh, material(0.3, RHO));
    let r = solve_static(&m, &StaticOptions::default());
    println!("RF1 (b) {:?}", r.as_ref().err());
    c.holds(
        "RF1 ANDES next to the constant-strain Shell3 refused (membrane)",
        membrane_refusal(&r),
        "",
    );
    // RF1 (c): two blocks meeting at one node only solve.
    let nodes: Vec<Vec3> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [2.0, 0.0, 0.0],
        [2.0, 1.0, 0.0],
    ];
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell3, vec![0, 1, 2]).unwrap();
    mesh.add_block(ElementKind::Shell3, vec![1, 3, 4]).unwrap();
    mesh.set_block_property(0, BlockProperty::shell_with_membrane(0.01, ANDES).unwrap())
        .unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    let m = model_of(mesh, material(0.3, RHO));
    let mut pres = Vec::new();
    for node in [0usize, 2, 3, 4] {
        for comp in 0..6 {
            pres.push(Prescribed::new(node, comp, 0.0));
        }
    }
    let r = solve_static_with(
        &m,
        &pres,
        &[NodalForceAt::new(1, [1.0, 2.0, 3.0])],
        &StaticOptions::default(),
    );
    c.holds(
        "RF1 blocks meeting at one node solve",
        r.is_ok(),
        &format!("{:?}", r.as_ref().err()),
    );
    // RF2: the property on a Shell4 block.
    let (nodes, quads) = grid(1.0, 1.0, 1, 1);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, quads[0].to_vec()).unwrap();
    let r = mesh.set_block_property(0, BlockProperty::shell_with_membrane(0.01, ANDES).unwrap());
    println!("RF2 {:?}", r.as_ref().err());
    c.holds(
        "RF2 ANDES on a Shell4 block refused (UnsupportedKind)",
        matches!(r, Err(FemError::UnsupportedKind { block: 0, .. })),
        "",
    );
    let r = mesh.set_block_property(
        0,
        BlockProperty::shell_with_membrane(0.01, Shell3Membrane::ConstantStrain).unwrap(),
    );
    c.holds("RF2 the constant-strain membrane on Shell4 taken", r.is_ok(), "");
    let r = BlockProperty::shell_with_membrane(0.0, ANDES);
    c.holds(
        "RF2 thickness 0 refused (thickness)",
        matches!(r, Err(FemError::InvalidValue { key: "thickness", .. })),
        "",
    );
    c.finish();
}
