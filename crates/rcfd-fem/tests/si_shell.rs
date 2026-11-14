//! F-392 (ADR 0432): the four-node flat-facet shell Shell4. Checks D1, PT1 to PT4, SS1 to SS4, CT1, CT2, RG1, CY1, FQ1, FQ2,
//! FR1 to FR3, W1, W2, S1, N2, N3, K1, M1, the stronger checks P1 to P6 against the numpy prototype of the registration
//! (`f392-scratch/estimate3.log`) and the negative controls N1, N4 (B1 to B3 are the existing tests; the rcfd-post row of N2
//! is in `rcfd-post/tests/si_shell_output.rs`).

use rcfd_core::vec3::{cross, dot, norm, Vec3};
use rcfd_fem::assemble::{assemble_with, stiffness};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::mass::{consistent, mass_properties};
use rcfd_fem::modal::{modal_analysis, modal_analysis_with, ModalOptions};
use rcfd_fem::shell::{self, ShellOptions, TransverseShear};
use rcfd_fem::solve::{solve_static, solve_static_with, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::{
    BlockProperty, Constraint, DofMap, ElasticSupport, ElementKind, FemError, Load, Material, SolidMesh, SolidModel,
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

/// A mesh of one Shell4 block with the given nodes and elements, element set "all", node set "all".
fn shell_mesh(nodes: Vec<Vec3>, elements: &[[usize; 4]], thickness: f64) -> SolidMesh {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = elements.iter().flatten().copied().collect();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(thickness).unwrap()).unwrap();
    mesh.add_element_set("all", (0..elements.len()).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh
}

fn model_of(mesh: SolidMesh, mat: Material) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    m
}

/// A grid of `nx x ny` elements on `[0, a] x [0, b]` at z = 0, nodes in rows of constant y; element nodes `(i, j), (i + 1,
/// j), (i + 1, j + 1), (i, j + 1)`.
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

// ------------------------------------------------------------------------------------------------------------ D1

#[test]
fn d1_unknown_counts() {
    let mut c = Checks::new();
    let solid = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    c.holds(
        "D1 solid-only mesh",
        DofMap::for_mesh(&solid) == DofMap::uniform(solid.nodes().len(), 3),
        "uniform 3",
    );
    let (nodes, el) = grid(1.0, 1.0, 2, 2);
    let mesh = shell_mesh(nodes, &el, 0.01);
    let d = DofMap::for_mesh(&mesh);
    c.holds(
        "D1 shell-only mesh",
        d == DofMap::uniform(9, 6),
        &format!("{} unknowns", d.n_equations()),
    );
    let mut mixed = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let ns = mixed.nodes().len();
    let mut nodes = mixed.nodes().to_vec();
    nodes.extend_from_slice(&[[5.0, 0.0, 0.0], [6.0, 0.0, 0.0], [6.0, 1.0, 0.0], [5.0, 1.0, 0.0]]);
    let mut m2 = SolidMesh::new(nodes).unwrap();
    m2.add_block(ElementKind::Hex8, mixed.blocks()[0].connectivity().to_vec()).unwrap();
    m2.add_block(ElementKind::Shell4, vec![ns, ns + 1, ns + 2, ns + 3]).unwrap();
    mixed = m2;
    let d = DofMap::for_mesh(&mixed);
    let ok = (0..ns).all(|n| d.components(n) == 3)
        && (ns..ns + 4).all(|n| d.components(n) == 6)
        && d.n_equations() == 3 * ns + 24;
    c.holds("D1 Hex8 box and a separate shell", ok, &format!("{} unknowns", d.n_equations()));
    c.finish();
}

// ------------------------------------------------------------------------------------------------------ PT1 to PT4

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

#[test]
fn pt_patch_tests_distorted_mesh_in_space() {
    let mut c = Checks::new();
    let q = rotation([1.0, -2.0, 2.0], 0.7);
    let origin = [0.3, -0.2, 0.5];
    let nodes: Vec<Vec3> = PATCH
        .iter()
        .map(|p| {
            let r = apply(&q, [p[0], p[1], 0.0]);
            [origin[0] + r[0], origin[1] + r[1], origin[2] + r[2]]
        })
        .collect();
    let mut el = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            el.push([4 * j + i, 4 * j + i + 1, 4 * (j + 1) + i + 1, 4 * (j + 1) + i]);
        }
    }
    let (t, nu) = (0.05, 0.3);
    let model = model_of(shell_mesh(nodes, &el, t), material(nu, RHO));
    let boundary: Vec<usize> = (0..16)
        .filter(|&k| {
            let p = PATCH[k];
            p[0] == 0.0 || p[0] == 1.0 || p[1] == 0.0 || p[1] == 1.0
        })
        .collect();
    let interior: Vec<usize> = (0..16).filter(|k| !boundary.contains(k)).collect();
    // The two exact fields (ADR 0432 arithmetic 17), six global unknowns per node.
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
    let dm = plane(E, nu, t);
    let db = plane(E, nu, t * t * t / 12.0);
    let n_exact = mul3(&dm, [a[1], b[2], a[2] + b[1]]);
    let m_exact = mul3(&db, [c1, c2, c3]);
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
        let id = if is_membrane { "PT1" } else { "PT3" };
        c.at_most(&format!("{id} {name} patch: interior unknowns"), err / scale, 1e-8);
        // Resultants in each element's axes against the exact constant ones.
        let st = shell::stresses(&model, r.displacement_values()).unwrap();
        let mut e_main = 0.0f64;
        let mut e_other = 0.0f64;
        let mut big = 0.0f64;
        for (_, s) in &st {
            let pa = {
                let v = apply(&transpose(&q), s.axes[0]);
                [v[0], v[1]]
            };
            let pb = {
                let v = apply(&transpose(&q), s.axes[1]);
                [v[0], v[1]]
            };
            if is_membrane {
                let ex = tensor_in(n_exact, pa, pb);
                big = big.max(ex.iter().fold(0.0f64, |m, v| m.max(v.abs())));
                for k in 0..3 {
                    e_main = e_main.max((s.membrane[k] - ex[k]).abs());
                    e_other = e_other.max(s.bending[k].abs());
                }
                for k in 0..2 {
                    e_other = e_other.max(s.shear[k].abs());
                }
            } else {
                let ex = tensor_in(m_exact, pa, pb);
                big = big.max(ex.iter().fold(0.0f64, |m, v| m.max(v.abs())));
                for k in 0..3 {
                    e_main = e_main.max((s.bending[k] - ex[k]).abs());
                    e_other = e_other.max(s.membrane[k].abs());
                }
                for k in 0..2 {
                    e_other = e_other.max(s.shear[k].abs());
                }
            }
        }
        if is_membrane {
            c.at_most("PT2 membrane patch: N against D_m eps", e_main / big, 1e-8);
            c.at_most("PT2 membrane patch: m and Q", e_other / big, 1e-8);
        } else {
            c.at_most("PT4 bending patch: m against D_b kappa", e_main / big, 1e-8);
            // Q and N times the patch side (1 m) relative to max |m|.
            c.at_most("PT4 bending patch: Q and N times the side", e_other / big, 1e-8);
        }
    }
    c.finish();
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

// ---------------------------------------------------------------------------------------------- SS1 to SS4, P1, N4

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

/// The simply supported plate of ADR 0432 arithmetic 11: model, prescribed unknowns, centre node.
fn ss_plate(t: f64, rho: f64) -> (SolidModel, Vec<Prescribed>, usize) {
    let n = 16;
    let (nodes, el) = grid(1.0, 1.0, n, n);
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
    let mut m = model_of(shell_mesh(nodes, &el, t), material(0.3, rho));
    m.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    (m, pres, (n / 2) * (n + 1) + n / 2)
}

#[test]
fn ss_simply_supported_plate_and_locking_control() {
    let mut c = Checks::new();
    let cn = navier(799);
    c.at_most("SS4 Navier coefficient |c - 0.004062|", (cn - 0.004062).abs(), 5e-7);
    println!("Navier coefficient c = {cn:.12}");
    let q = 1000.0;
    let ratio = |w: f64, t: f64| w / (cn * q / (E * t * t * t / (12.0 * (1.0 - 0.09))));
    let mut ss = [0.0; 2];
    for (i, (t, limit, id, proto)) in [(0.01, 1e-2, "SS1", 0.9992544947), (0.001, 1.5e-2, "SS2", 0.9987399464)]
        .into_iter()
        .enumerate()
    {
        let (m, pres, centre) = ss_plate(t, RHO);
        let r = solve_static_with(&m, &pres, &[], &StaticOptions::default()).unwrap();
        let x = ratio(r.displacement(centre)[2], t);
        ss[i] = x;
        println!("{id} t/a = {t}: w_c / w_Navier = {x:.10}");
        c.at_most(&format!("{id} SS plate t/a = {t} against Navier"), (x - 1.0).abs(), limit);
        c.at_most(&format!("P1 SS plate t/a = {t} against the prototype"), rel(x, proto), 1e-5);
    }
    // SS3: the fully integrated shear control at t/a = 0.001 (amendment 1, item 2).
    let t = 0.001;
    let (m, pres, centre) = ss_plate(t, RHO);
    let opts = ShellOptions::default().shear(TransverseShear::FullIntegration);
    let k = assemble_with(&m, None, |ctx| shell::element_stiffness_with(ctx.coords, t, ctx.material, &opts)).unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let fixed = prescribed_equations(&m, &pres).unwrap();
    let sys = reduce(&k, &f, &fixed).unwrap();
    let ldlt = SkylineLdlt::factor(&sys.matrix).unwrap();
    let u = sys.expand(&ldlt.solve(&sys.rhs));
    let eq = m.dof_map().equation(centre, 2).unwrap();
    let x = ratio(u[eq], t);
    println!("SS3 full-integration control: {x:.10e}");
    c.below("SS3 full-integration control t/a = 0.001", x, 0.2);
    c.at_most("P1 control against the prototype", rel(x, 1.7700823307e-3), 1e-5);
    c.holds(
        "N4 element not locked, control locked",
        ss[1] > 0.98 && x < 0.2,
        &format!("{:.6} and {:.3e}", ss[1], x),
    );
    c.finish();
}

// --------------------------------------------------------------------------------------------------- CT1, CT2, P2

#[test]
fn ct_cantilever_strip() {
    let mut c = Checks::new();
    let (l, b, t, p, n) = (1.0, 0.1, 0.01, 1.0, 15);
    let (nodes, el) = grid(l, b, n, 1);
    let m = model_of(shell_mesh(nodes, &el, t), material(0.0, RHO));
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
    c.at_most("CT1 cantilever tip deflection", (x - 1.0).abs(), 5e-3);
    c.at_most("P2 cantilever tip against the prototype", rel(x, 0.9989488888), 1e-5);
    let st = shell::stresses(&m, r.displacement_values()).unwrap();
    let (e, s) = st[7];
    let mid = 6.0 * p * l / 2.0 / (b * t * t);
    println!("CT2 element {e}: top {:.8e}, bottom {:.8e}, 6M/(b t^2) = {mid:.8e}", s.top[0], s.bottom[0]);
    c.at_most("CT2 mid-span top stress against -6M/(b t^2)", rel(s.top[0], -mid), 2e-2);
    c.at_most("CT2 mid-span bottom stress against +6M/(b t^2)", rel(s.bottom[0], mid), 2e-2);
    c.at_most("P2 mid-span stress against the prototype", rel(s.top[0] / -mid, 0.9999999999), 1e-5);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- RG1, P3

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
    let el: Vec<[usize; 4]> = (0..nq).map(|i| [idx(i, 0), idx(i + 1, 0), idx(i + 1, 1), idx(i, 1)]).collect();
    let m = model_of(shell_mesh(nodes.clone(), &el, t), material(0.0, RHO));
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
    c.at_most("RG1 pinched ring", (x - 1.0).abs(), 2e-2);
    c.at_most("P3 ring against the prototype", rel(x, 0.9985882399), 1e-5);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- CY1, P4

#[test]
fn cy_cylinder_under_internal_pressure() {
    let mut c = Checks::new();
    let (nq, nz, r0, lh, t, p) = (32, 4, 1.0, 0.5, 0.01, 1e5);
    let idx = |i: usize, j: usize| (nz + 1) * i + j;
    let mut nodes = Vec::new();
    for i in 0..=nq {
        let ph = PI / 2.0 * i as f64 / nq as f64;
        for j in 0..=nz {
            nodes.push([r0 * ph.cos(), r0 * ph.sin(), lh * j as f64 / nz as f64]);
        }
    }
    let mut el = Vec::new();
    for i in 0..nq {
        for j in 0..nz {
            el.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut m = model_of(shell_mesh(nodes.clone(), &el, t), material(0.3, RHO));
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
    let polygon = (PI / 128.0).cos();
    let (mut worst, mut worst_poly) = (0.0f64, 0.0f64);
    for (k, x) in nodes.iter().enumerate() {
        let u = r.displacement(k);
        let ur = (u[0] * x[0] + u[1] * x[1]) / r0;
        worst = worst.max((ur / reference - 1.0).abs());
        worst_poly = worst_poly.max((ur / (polygon * reference) - 1.0).abs());
    }
    c.at_most("CY1 cylinder: max |u_r / (p R^2 / (E t)) - 1|", worst, 5e-3);
    c.at_most("P4 cylinder against the polygon closed form", worst_poly, 1e-6);
    c.finish();
}

// --------------------------------------------------------------------------------------------------- FQ1, FQ2, P5

#[test]
fn fq_simply_supported_plate_frequencies() {
    let mut c = Checks::new();
    let t = 0.01;
    let (m, pres, _) = ss_plate(t, RHO);
    let opts = ModalOptions::new(4).tolerance(1e-12).residual_test(ResidualTest::Backward);
    let r = modal_analysis_with(&m, &pres, &opts).unwrap();
    let d = E * t * t * t / (12.0 * (1.0 - 0.09));
    let w = |mn: f64| PI * PI * mn * (d / (RHO * t)).sqrt();
    let omega: Vec<f64> = r.frequencies.iter().map(|f| 2.0 * PI * f).collect();
    println!("FQ frequencies {:?} Hz", r.frequencies);
    c.at_most("FQ1 (1,1)", rel(omega[0], w(2.0)), 1e-2);
    c.at_most("FQ2 (1,2)", rel(omega[1], w(5.0)), 3e-2);
    c.at_most("FQ2 (2,1)", rel(omega[2], w(5.0)), 3e-2);
    c.at_most("FQ2 (2,2)", rel(omega[3], w(8.0)), 3e-2);
    let proto = [48.1756947203, 121.7454596068, 121.7454596145, 194.998287129];
    for (i, pf) in proto.iter().enumerate() {
        c.at_most(&format!("P5 frequency {} against the prototype", i + 1), rel(r.frequencies[i], *pf), 1e-5);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------- FR1 to FR3, P6, N1

fn free_plate(warp: f64) -> SolidModel {
    let (mut nodes, el) = grid(1.0, 0.8, 8, 8);
    for (k, x) in nodes.iter_mut().enumerate() {
        x[2] += warp * (1.7 * k as f64 + 0.3).sin();
    }
    model_of(shell_mesh(nodes, &el, 0.01), material(0.3, RHO))
}

#[test]
fn fr_free_plate_six_zero_frequencies() {
    let mut c = Checks::new();
    let opts = ModalOptions::new(8).tolerance(1e-12).residual_test(ResidualTest::Backward);
    let m = free_plate(0.0);
    let r = modal_analysis(&m, &opts).unwrap();
    println!("FR1 frequencies {:?} Hz", r.frequencies);
    let f = &r.frequencies;
    let zero = f[..6].iter().fold(0.0f64, |a, v| a.max(v.abs()));
    c.at_most("FR1 free plate max |f_1..6| / f_7", zero / f[6], 1e-4);
    c.above("FR1 f_7", f[6], 0.0);
    c.at_most("P6 f_7 against the prototype", rel(f[6], 41.1269190351), 1e-5);
    c.at_most("P6 f_8 against the prototype", rel(f[7], 52.7170891366), 1e-5);
    // FR3 and N1 (amendment 1): Sturm counts below lambda_7 / 4.
    let lambda7 = r.eigenvalues[6];
    let k = stiffness(&m).unwrap();
    let mm = consistent(&m).unwrap();
    let n6 = count_below(&k, &mm, lambda7 / 4.0, &SkylineOrdering::Rcm).unwrap();
    c.holds("FR3 Sturm count below lambda_7 / 4 is 6", n6 == 6, &format!("{n6}"));
    let opts0 = ShellOptions::default().drilling(0.0);
    let k0 = assemble_with(&m, None, |ctx| shell::element_stiffness_with(ctx.coords, 0.01, ctx.material, &opts0)).unwrap();
    let n0 = count_below(&k0, &mm, lambda7 / 4.0, &SkylineOrdering::Rcm).unwrap();
    println!("N1 Sturm count with alpha = 0: {n0}");
    c.above("N1 alpha = 0: more than six zeros", n0 as f64, 6.0);
    // FR2: the warped variant (rigid offsets).
    let mw = free_plate(0.002);
    let rw = modal_analysis(&mw, &opts).unwrap();
    println!("FR2 frequencies {:?} Hz", rw.frequencies);
    let fw = &rw.frequencies;
    let zero = fw[..6].iter().fold(0.0f64, |a, v| a.max(v.abs()));
    c.at_most("FR2 warped free plate max |f_1..6| / f_7", zero / fw[6], 1e-4);
    c.at_most("FR2 warped f_7 against the prototype", rel(fw[6], 41.1488072747), 1e-5);
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------- W1, W2

fn lifted_strip(delta: f64) -> SolidModel {
    let nodes = vec![
        [-1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, delta],
        [0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
    ];
    let mut mesh = shell_mesh(nodes, &[[0, 1, 4, 5], [1, 2, 3, 4]], 0.01);
    mesh.add_node_set("root", vec![0, 5]).unwrap();
    mesh.add_node_set("tip", vec![2]).unwrap();
    let mut m = model_of(mesh, material(0.3, RHO));
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("tip", [0.0, 0.0, 1.0])).unwrap();
    m
}

fn warp_ratio(x: [Vec3; 4]) -> f64 {
    let mut ctr = [0.0; 3];
    for p in &x {
        for k in 0..3 {
            ctr[k] += 0.25 * p[k];
        }
    }
    let sub = |a: Vec3, b: Vec3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let n = cross(sub(x[2], x[0]), sub(x[3], x[1]));
    let nn = norm(n);
    let h = dot(sub(x[0], ctr), n) / nn;
    h.abs() / (0.5 * nn).sqrt()
}

#[test]
fn w_warped_facet_refused_with_the_element() {
    let mut c = Checks::new();
    let ratio = warp_ratio([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.22], [0.0, 1.0, 0.0]]);
    let text = format!("{ratio:.4e}");
    let m = lifted_strip(0.22);
    let is_warp = |r: &Result<(), FemError>| -> (bool, String) {
        match r {
            Err(e @ FemError::Element { element: 1, source }) => {
                let msg = e.to_string();
                (
                    matches!(source.as_ref(), FemError::InvalidValue { key: "warp", .. })
                        && msg.contains(&text)
                        && msg.contains("0.05"),
                    msg,
                )
            }
            other => (false, format!("{other:?}")),
        }
    };
    let (ok, msg) = is_warp(&solve_static(&m, &StaticOptions::default()).map(|_| ()));
    c.holds("W1 solve_static refuses the warped element 1", ok, &msg);
    let (ok, msg) = is_warp(&modal_analysis(&m, &ModalOptions::new(2)).map(|_| ()));
    c.holds("W1 modal_analysis refuses the warped element 1", ok, &msg);
    let (ok, msg) = is_warp(&stiffness(&m).map(|_| ()));
    c.holds("W1 stiffness refuses the warped element 1", ok, &msg);
    let ok = solve_static(&lifted_strip(0.18), &StaticOptions::default());
    c.holds(
        "W2 warp within the limit solves",
        ok.is_ok(),
        &format!("ratio {:.4e}: {:?}", warp_ratio([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.18], [0.0, 1.0, 0.0]]), ok.as_ref().err()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- S1, N2, N3

fn follow_up<T: std::fmt::Debug>(r: &Result<T, FemError>) -> bool {
    matches!(r, Err(FemError::NotYetSupported { variant: "Shell4", package, .. }) if *package == shell::FOLLOW_UP)
}

fn cantilever_model() -> SolidModel {
    let (nodes, el) = grid(1.0, 0.1, 4, 1);
    let mut mesh = shell_mesh(nodes, &el, 0.01);
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
fn s1_n2_n3_refusals() {
    let mut c = Checks::new();
    // S1: a shell on four nodes of a Hex8 box.
    let mut mesh = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let at = [[2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [2.0, 1.0, 1.0], [2.0, 0.0, 1.0]];
    let conn: Vec<usize> = at
        .iter()
        .map(|p| mesh.nodes().iter().position(|q| q == p).unwrap())
        .collect();
    let first = conn[0];
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
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
    // N2: a valid shell model on the paths that do not take shells.
    let base = cantilever_model();
    c.holds(
        "N2 the base model solves",
        solve_static(&base, &StaticOptions::default()).is_ok(),
        "",
    );
    let mut m = base.clone();
    m.add_support(ElasticSupport::nodal_spring("tip", 2, 1e6)).unwrap();
    let r = solve_static(&m, &StaticOptions::default());
    c.holds("N2 nodal spring (transformation)", follow_up(&r), &format!("{r:?}"));
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
    let mut m = base.clone();
    m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    let r = solve_static(&m, &StaticOptions::default());
    let ok = matches!(&r, Err(FemError::Element { source, .. })
        if matches!(source.as_ref(), FemError::NotYetSupported { package, .. } if *package == shell::SOLID_ROUTINES));
    c.holds("N2 gravity on a shell", ok, &format!("{r:?}"));
    // N3: invalid data.
    let (nodes, el) = grid(1.0, 1.0, 1, 1);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, el[0].to_vec()).unwrap();
    mesh.add_element_set("all", vec![0]).unwrap();
    let m = model_of(mesh, material(0.3, RHO));
    let r = solve_static(&m, &StaticOptions::default());
    c.holds(
        "N3 Shell4 block without property",
        matches!(&r, Err(FemError::InvalidValue { key: "property", .. })),
        &format!("{r:?}"),
    );
    let hex = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let mut m = SolidModel::new(hex);
    let r = m.add_load(Load::shell_pressure("all", 1.0));
    c.holds(
        "N3 shell pressure on Hex8 elements",
        matches!(&r, Err(FemError::InvalidValue { key: "shell pressure", .. })),
        &format!("{r:?}"),
    );
    let dart = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.3, 0.3, 0.0], [0.0, 1.0, 0.0]];
    let m = model_of(shell_mesh(dart, &[[0, 1, 2, 3]], 0.01), material(0.3, RHO));
    let r = solve_static(&m, &StaticOptions::default());
    c.holds(
        "N3 non-convex facet",
        matches!(&r, Err(FemError::InvertedElement { element: 0, .. })),
        &format!("{r:?}"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ K1, M1

#[test]
fn k1_m1_rigid_motions_and_mass() {
    let mut c = Checks::new();
    let q = rotation([1.0, -2.0, 2.0], 0.7);
    let origin = [0.3, -0.2, 0.5];
    let place = |p: [f64; 2]| -> Vec3 {
        let r = apply(&q, [p[0], p[1], 0.0]);
        [origin[0] + r[0], origin[1] + r[1], origin[2] + r[2]]
    };
    let warped = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.18], [0.0, 1.0, 0.0]];
    let flat: Vec<Vec3> = [[0.0, 0.0], [0.6, 0.05], [0.55, 0.45], [-0.05, 0.4]].iter().map(|&p| place(p)).collect();
    let mat = material(0.3, RHO);
    for (name, x) in [("warped", warped), ("flat in space", flat)] {
        let k = shell::element_stiffness(&x, 0.01, &mat).unwrap();
        let kmax = k.values().iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let mut worst = 0.0f64;
        for mode in 0..6 {
            let mut r = vec![0.0; 24];
            for (i, xi) in x.iter().enumerate() {
                if mode < 3 {
                    r[6 * i + mode] = 1.0;
                } else {
                    let mut w = [0.0; 3];
                    w[mode - 3] = 1.0;
                    let u = cross(w, *xi);
                    r[6 * i..6 * i + 3].copy_from_slice(&u);
                    r[6 * i + 3..6 * i + 6].copy_from_slice(&w);
                }
            }
            let rmax = r.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            let kr = k.matvec(&r);
            worst = worst.max(kr.iter().fold(0.0f64, |m, v| m.max(v.abs())) / (kmax * rmax));
        }
        c.at_most(&format!("K1 {name}: |K r| / (max |K| max |r|)"), worst, 1e-12);
        let sym = (0..24).all(|i| (0..24).all(|j| k.get(i, j).to_bits() == k.get(j, i).to_bits()));
        c.holds(&format!("K1 {name}: symmetric bit for bit"), sym, "");
    }
    // M1: one flat element [0, 0.6] x [0, 0.4] placed by Q.
    let (t, w, h) = (0.01, 0.6, 0.4);
    let nodes: Vec<Vec3> = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]].iter().map(|&p| place(p)).collect();
    let m = model_of(shell_mesh(nodes, &[[0, 1, 2, 3]], t), mat);
    let mp = mass_properties(&m, &consistent(&m).unwrap()).unwrap();
    let area = w * h;
    let mass = RHO * t * area;
    c.at_most("M1 mass", rel(mp.mass, mass), 1e-12);
    let centre = place([w / 2.0, h / 2.0]);
    let dc = norm([mp.centre[0] - centre[0], mp.centre[1] - centre[1], mp.centre[2] - centre[2]]);
    let diag = (w * w + h * h).sqrt();
    c.at_most("M1 centre", dc / diag, 1e-12);
    let (ix, iy) = (w * h * h * h / 12.0, h * w * w * w / 12.0);
    let rot = RHO * t * t * t * area / 12.0;
    let dl = [RHO * t * ix + rot, RHO * t * iy + rot, RHO * t * (ix + iy) + rot];
    let mut expected = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            expected[i][j] = (0..3).map(|k| q[i][k] * dl[k] * q[j][k]).sum();
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
