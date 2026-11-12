//! F-378 (ADR 0279): verification by a manufactured solution. Checks F1, F2, A1 to A3 (cases D and T), N1, X1 to X3 and
//! the heavy record H1 of the registration (seed 0xF378_0279).
//!
//! The field `u_i = U prod_j sin(k_ij x_j + phi_ij)` on the unit cube, its body force `b = -div sigma` from the analytic
//! second derivatives, case D (every face prescribed) and case T (tractions `sigma n` on `xmax`, `ymax`, `zmax`, the other
//! three faces prescribed); L2 and energy-norm errors against the exact field at the points of 125-point rules of degree 7
//! (tetrahedra) and 9 per variable (hexahedra), built here from Gauss-Legendre nodes computed by Newton's method.

use rcfd_core::vec3::{cross, norm, Vec3};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, MidEdgeNode, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::element_for;
use rcfd_fem::loads::{body_force_loads, face_rule, face_shape_functions, nodal_load_vector, NodalForceAt};
use rcfd_fem::material::{isotropic_elasticity, Matrix6};
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{Load, Material, SolidModel, SolidResult};
use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::time::Instant;

const SEED: u64 = 0xF378_0279;
const E_MOD: f64 = 1.0;
const NU: f64 = 0.3;
const AMPLITUDE: f64 = 1.0;
const FD_STEP: f64 = 1e-4;
const FD_POINTS: usize = 200;

// Arithmetic 1: wavenumbers in units of pi and phases, row = component, column = coordinate.
const K: [[f64; 3]; 3] = [[1.0, 0.5, 0.75], [0.75, 1.0, 0.5], [0.5, 0.75, 1.0]];
const PHI: [[f64; 3]; 3] = [[0.3, 0.7, 0.5], [0.9, 0.2, 0.6], [0.4, 0.8, 0.1]];

// Arithmetic 9: the fast (acceptance) sequences and the heavy family.
const FAST_LINEAR: [usize; 3] = [4, 8, 16];
const FAST_TET10: [usize; 3] = [2, 4, 8];
const HEAVY_LINEAR: [usize; 3] = [6, 12, 24];
const HEAVY_TET10: [usize; 3] = [3, 6, 12];

/// SplitMix64, a deterministic source only (as in sb_loads.rs).
struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(SEED.wrapping_add(tag))
    }
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:.4} (limit >= {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} < {limit}"));
        }
    }
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{name}: {value:.4} (limit < {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} >= {limit}"));
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

fn lame(nu: f64) -> (f64, f64) {
    (E_MOD * nu / ((1.0 + nu) * (1.0 - 2.0 * nu)), E_MOD / (2.0 * (1.0 + nu)))
}

fn d_matrix() -> Matrix6 {
    isotropic_elasticity(E_MOD, NU)
}

/// A displacement field with its first and second derivatives: `grad[i][m] = d_m u_i`, `hess[i][m][n] = d_m d_n u_i`.
trait Field {
    fn u(&self, x: Vec3) -> Vec3;
    fn grad(&self, x: Vec3) -> [[f64; 3]; 3];
    fn hess(&self, x: Vec3) -> [[[f64; 3]; 3]; 3];
}

/// Voigt strain (engineering shear) from a displacement gradient.
fn voigt_strain(g: &[[f64; 3]; 3]) -> [f64; 6] {
    [
        g[0][0],
        g[1][1],
        g[2][2],
        g[0][1] + g[1][0],
        g[1][2] + g[2][1],
        g[2][0] + g[0][2],
    ]
}

/// The stress tensor `lambda tr(eps) I + 2 mu eps` of a field at a point.
fn stress_tensor(f: &dyn Field, x: Vec3) -> [[f64; 3]; 3] {
    let (lambda, mu) = lame(NU);
    let g = f.grad(x);
    let div = g[0][0] + g[1][1] + g[2][2];
    let mut s = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            s[i][j] = mu * (g[i][j] + g[j][i]) + if i == j { lambda * div } else { 0.0 };
        }
    }
    s
}

/// The body force `b_i = -[(s lambda + mu) d_i div u + mu Laplace u_i]` (arithmetic 3); `lambda_sign = -1` is the negative
/// control of arithmetic 6.
fn body_force(f: &dyn Field, x: Vec3, lambda_sign: f64) -> Vec3 {
    let (lambda, mu) = lame(NU);
    let h = f.hess(x);
    let mut b = [0.0; 3];
    for (i, bi) in b.iter_mut().enumerate() {
        let grad_div: f64 = (0..3).map(|k| h[k][i][k]).sum();
        let lap: f64 = (0..3).map(|j| h[i][j][j]).sum();
        *bi = -((lambda_sign * lambda + mu) * grad_div + mu * lap);
    }
    b
}

type Mat3 = [[f64; 3]; 3];

/// The manufactured field of arithmetic 1.
struct Trig;

impl Trig {
    fn parts(x: Vec3) -> (Mat3, Mat3, Mat3) {
        let mut s = [[0.0; 3]; 3];
        let mut c = [[0.0; 3]; 3];
        let mut k = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                k[i][j] = PI * K[i][j];
                let a = k[i][j] * x[j] + PHI[i][j];
                s[i][j] = a.sin();
                c[i][j] = a.cos();
            }
        }
        (s, c, k)
    }
}

impl Field for Trig {
    fn u(&self, x: Vec3) -> Vec3 {
        let (s, _, _) = Trig::parts(x);
        [0, 1, 2].map(|i| AMPLITUDE * s[i][0] * s[i][1] * s[i][2])
    }
    fn grad(&self, x: Vec3) -> [[f64; 3]; 3] {
        let (s, c, k) = Trig::parts(x);
        let mut g = [[0.0; 3]; 3];
        for i in 0..3 {
            for m in 0..3 {
                let mut p = AMPLITUDE * k[i][m] * c[i][m];
                for j in (0..3).filter(|&j| j != m) {
                    p *= s[i][j];
                }
                g[i][m] = p;
            }
        }
        g
    }
    fn hess(&self, x: Vec3) -> [[[f64; 3]; 3]; 3] {
        let (s, c, k) = Trig::parts(x);
        let mut h = [[[0.0; 3]; 3]; 3];
        for i in 0..3 {
            for m in 0..3 {
                for n in 0..3 {
                    let mut p = AMPLITUDE;
                    if m == n {
                        p *= -k[i][m] * k[i][m];
                        for j in 0..3 {
                            p *= s[i][j];
                        }
                    } else {
                        p *= k[i][m] * c[i][m] * k[i][n] * c[i][n];
                        for j in (0..3).filter(|&j| j != m && j != n) {
                            p *= s[i][j];
                        }
                    }
                    h[i][m][n] = p;
                }
            }
        }
        h
    }
}

/// `u_i = a_i + G_i . x + x^T Q_i x` (Q symmetric); X3 of the registration.
struct Poly {
    a: Vec3,
    g: [[f64; 3]; 3],
    q: [[[f64; 3]; 3]; 3],
}

impl Poly {
    fn linear() -> Poly {
        Poly {
            a: [0.1, -0.2, 0.3],
            g: [[0.5, 0.2, -0.1], [0.3, -0.4, 0.25], [-0.15, 0.35, 0.6]],
            q: [[[0.0; 3]; 3]; 3],
        }
    }
    fn quadratic() -> Poly {
        Poly {
            q: [
                [[0.2, 0.1, -0.05], [0.1, -0.3, 0.15], [-0.05, 0.15, 0.25]],
                [[-0.1, 0.2, 0.05], [0.2, 0.15, -0.1], [0.05, -0.1, 0.3]],
                [[0.3, -0.05, 0.1], [-0.05, 0.2, 0.2], [0.1, 0.2, -0.25]],
            ],
            ..Poly::linear()
        }
    }
}

impl Field for Poly {
    fn u(&self, x: Vec3) -> Vec3 {
        [0, 1, 2].map(|i| {
            let mut v = self.a[i];
            for m in 0..3 {
                v += self.g[i][m] * x[m];
                for n in 0..3 {
                    v += self.q[i][m][n] * x[m] * x[n];
                }
            }
            v
        })
    }
    fn grad(&self, x: Vec3) -> [[f64; 3]; 3] {
        let mut g = self.g;
        for (i, gi) in g.iter_mut().enumerate() {
            for (m, gim) in gi.iter_mut().enumerate() {
                for n in 0..3 {
                    *gim += 2.0 * self.q[i][m][n] * x[n];
                }
            }
        }
        g
    }
    fn hess(&self, _x: Vec3) -> [[[f64; 3]; 3]; 3] {
        let mut h = self.q;
        for hi in h.iter_mut() {
            for row in hi.iter_mut() {
                for v in row.iter_mut() {
                    *v *= 2.0;
                }
            }
        }
        h
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Error rules (D3, arithmetic 8)

/// Gauss-Legendre nodes and weights on [-1, 1] by Newton's method on the three-term recurrence of `P_n`.
fn gauss_legendre(n: usize) -> (Vec<f64>, Vec<f64>) {
    // P_n(t) and P_n'(t) from the recurrence k P_k = (2k - 1) t P_{k-1} - (k - 1) P_{k-2}.
    let legendre = |t: f64| -> (f64, f64) {
        let (mut p0, mut p1) = (1.0, t);
        for k in 2..=n {
            let p2 = ((2 * k - 1) as f64 * t * p1 - (k - 1) as f64 * p0) / k as f64;
            p0 = p1;
            p1 = p2;
        }
        (p1, n as f64 * (t * p1 - p0) / (t * t - 1.0))
    };
    let mut x = Vec::with_capacity(n);
    let mut w = Vec::with_capacity(n);
    for i in 1..=n {
        let mut t = (PI * (i as f64 - 0.25) / (n as f64 + 0.5)).cos();
        for _ in 0..100 {
            let (p, dp) = legendre(t);
            let dt = p / dp;
            t -= dt;
            if dt.abs() < 1e-16 {
                break;
            }
        }
        let (_, dp) = legendre(t);
        x.push(t);
        w.push(2.0 / ((1.0 - t * t) * dp * dp));
    }
    (x, w)
}

/// The 125-point error rule of an element kind as (reference point, weight).
fn error_rule(kind: ElementKind) -> Vec<([f64; 3], f64)> {
    let (g, gw) = gauss_legendre(5);
    let mut out = Vec::with_capacity(125);
    match kind {
        ElementKind::Hex8 => {
            for k in 0..5 {
                for j in 0..5 {
                    for i in 0..5 {
                        out.push(([g[i], g[j], g[k]], gw[i] * gw[j] * gw[k]));
                    }
                }
            }
        }
        _ => {
            let s: Vec<f64> = g.iter().map(|t| 0.5 * (1.0 + t)).collect();
            let sw: Vec<f64> = gw.iter().map(|w| 0.5 * w).collect();
            for k in 0..5 {
                for j in 0..5 {
                    for i in 0..5 {
                        let (u, v, w) = (s[i], s[j], s[k]);
                        let p = [u, (1.0 - u) * v, (1.0 - u) * (1.0 - v) * w];
                        let jac = (1.0 - u) * (1.0 - u) * (1.0 - v);
                        out.push((p, sw[i] * sw[j] * sw[k] * jac));
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Meshes, loads, solve (D2)

struct Cube {
    mesh: SolidMesh,
    mids: Vec<MidEdgeNode>,
}

fn cube(kind: ElementKind, n: usize) -> Cube {
    let (o, l, d) = ([0.0; 3], [1.0; 3], [n; 3]);
    match kind {
        ElementKind::Hex8 => Cube {
            mesh: hex_box(o, l, d).unwrap(),
            mids: Vec::new(),
        },
        ElementKind::Tet4 => Cube {
            mesh: tet_box(o, l, d).unwrap(),
            mids: Vec::new(),
        },
        _ => {
            let (mesh, mids) = upgrade_to_tet10(&tet_box(o, l, d).unwrap()).unwrap();
            Cube { mesh, mids }
        }
    }
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap())
        .unwrap();
    model
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Case {
    /// Every face prescribed.
    D,
    /// `xmin`, `ymin`, `zmin` prescribed, tractions on `xmax`, `ymax`, `zmax`.
    T,
}

const LOADED: [(&str, usize); 3] = [("xmax", 0), ("ymax", 1), ("zmax", 2)];

/// The consistent nodal load `int N_a t(x) dS` of a traction given by a function on a surface set, accumulated into a
/// node-major vector (three entries per node) in face order and point order, as the library does for a uniform traction.
fn add_traction<T>(f: &mut [f64], mesh: &SolidMesh, set: &str, traction: T)
where
    T: Fn(Vec3) -> Vec3,
{
    for &face in mesh.surface_set(set).unwrap() {
        let nodes = mesh.face_nodes(face);
        let x: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let mut fe = vec![[0.0; 3]; nodes.len()];
        for (uv, w) in face_rule(nodes.len()).unwrap() {
            let (nf, d) = face_shape_functions(nodes.len(), uv).unwrap();
            let mut xu = [0.0; 3];
            let mut xv = [0.0; 3];
            let mut xq = [0.0; 3];
            for ((xa, da), na) in x.iter().zip(&d).zip(&nf) {
                for i in 0..3 {
                    xu[i] += xa[i] * da[0];
                    xv[i] += xa[i] * da[1];
                    xq[i] += xa[i] * na;
                }
            }
            let s = norm(cross(xu, xv));
            let t0 = traction(xq);
            let t = [t0[0] * s, t0[1] * s, t0[2] * s];
            for (fa, na) in fe.iter_mut().zip(&nf) {
                for i in 0..3 {
                    fa[i] += w * na * t[i];
                }
            }
        }
        for (&node, fa) in nodes.iter().zip(&fe) {
            for i in 0..3 {
                f[3 * node + i] += fa[i];
            }
        }
    }
}

/// The traction `sigma n` of a field on a box face with outward normal `+e_axis`.
fn face_traction(field: &dyn Field, x: Vec3, axis: usize) -> Vec3 {
    let s = stress_tensor(field, x);
    [s[0][axis], s[1][axis], s[2][axis]]
}

fn prescribed_nodes(mesh: &SolidMesh, case: Case) -> BTreeSet<usize> {
    let faces: &[&str] = match case {
        Case::D => &BOX_FACES,
        Case::T => &["xmin", "ymin", "zmin"],
    };
    let mut set = BTreeSet::new();
    for name in faces {
        set.extend(mesh.node_set(name).unwrap().iter().copied());
    }
    set
}

/// The value prescribed at a boundary node: the exact field at the node (nodal interpolation).
fn prescribed_value(field: &dyn Field, c: &Cube, node: usize) -> Vec3 {
    let _ = &c.mids;
    field.u(c.mesh.nodes()[node])
}

fn solve_case(field: &dyn Field, c: &Cube, case: Case, lambda_sign: f64) -> (SolidModel, SolidResult) {
    let model = model_of(c.mesh.clone());
    let mesh = model.mesh();
    let mut prescribed = Vec::new();
    for node in prescribed_nodes(mesh, case) {
        let v = prescribed_value(field, c, node);
        for (k, &vk) in v.iter().enumerate() {
            prescribed.push(Prescribed::new(node, k, vk));
        }
    }
    let mut forces = body_force_loads(&model, |x| body_force(field, x, lambda_sign)).unwrap();
    if case == Case::T {
        let mut f = vec![0.0; 3 * mesh.nodes().len()];
        for (set, axis) in LOADED {
            add_traction(&mut f, mesh, set, |x| face_traction(field, x, axis));
        }
        forces.extend(
            f.chunks(3)
                .enumerate()
                .map(|(node, v)| NodalForceAt::new(node, [v[0], v[1], v[2]])),
        );
    }
    let result = solve_static_with(&model, &prescribed, &forces, &StaticOptions::default()).unwrap();
    (model, result)
}

/// Errors (L2, energy) and the norms of the exact field (L2, energy) with the error rule (D3).
fn errors(field: &dyn Field, model: &SolidModel, result: &SolidResult) -> [f64; 4] {
    let mesh = model.mesh();
    let d = d_matrix();
    let tet_rule = error_rule(ElementKind::Tet4);
    let hex_rule = error_rule(ElementKind::Hex8);
    let mut acc = [0.0; 4];
    for e in 0..mesh.n_elements() {
        let kind = mesh.element_kind(e);
        let element = element_for(kind);
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let ue: Vec<&[f64]> = nodes.iter().map(|&a| result.displacement(a)).collect();
        let rule = if kind == ElementKind::Hex8 {
            &hex_rule
        } else {
            &tet_rule
        };
        for &(xi, wq) in rule {
            let p = element.point(&coords, xi).unwrap();
            let w = wq * p.det_j;
            let mut uh = [0.0; 3];
            let mut gh = [[0.0; 3]; 3];
            for (a, ua) in ue.iter().enumerate() {
                for i in 0..3 {
                    uh[i] += p.n[a] * ua[i];
                    for m in 0..3 {
                        gh[i][m] += ua[i] * p.grad[a][m];
                    }
                }
            }
            let u = field.u(p.x);
            let g = field.grad(p.x);
            let eps = voigt_strain(&g);
            let eps_h = voigt_strain(&gh);
            let de: [f64; 6] = std::array::from_fn(|r| eps[r] - eps_h[r]);
            let energy = |v: &[f64; 6]| -> f64 {
                let mut s = 0.0;
                for r in 0..6 {
                    for c in 0..6 {
                        s += v[r] * d[r][c] * v[c];
                    }
                }
                s
            };
            let du2: f64 = (0..3).map(|i| (u[i] - uh[i]) * (u[i] - uh[i])).sum();
            let u2: f64 = u.iter().map(|v| v * v).sum();
            acc[0] += w * du2;
            acc[1] += w * energy(&de);
            acc[2] += w * u2;
            acc[3] += w * energy(&eps);
        }
    }
    acc.map(f64::sqrt)
}

struct Level {
    n: usize,
    unknowns: usize,
    l2: f64,
    energy: f64,
    seconds: f64,
}

fn run_sequence(field: &dyn Field, kind: ElementKind, ns: &[usize], case: Case, lambda_sign: f64) -> Vec<Level> {
    ns.iter()
        .map(|&n| {
            let t0 = Instant::now();
            let c = cube(kind, n);
            let (model, result) = solve_case(field, &c, case, lambda_sign);
            let [l2, energy, norm_l2, norm_energy] = errors(field, &model, &result);
            let seconds = t0.elapsed().as_secs_f64();
            let unknowns = result.stats().map(|s| s.unknowns()).unwrap_or(0);
            println!(
                "  {} case {case:?} sign {lambda_sign:+} n = {n:2}: free unknowns {unknowns}, e_L2 {l2:.4e} \
                 (rel {:.3e}), e_E {energy:.4e} (rel {:.3e}), {seconds:.2} s",
                kind.name(),
                l2 / norm_l2,
                energy / norm_energy
            );
            Level {
                n,
                unknowns,
                l2,
                energy,
                seconds,
            }
        })
        .collect()
}

fn order(coarse: f64, fine: f64) -> f64 {
    (coarse / fine).log2()
}

fn orders(levels: &[Level]) -> (f64, f64) {
    let (a, b) = (&levels[levels.len() - 2], &levels[levels.len() - 1]);
    assert_eq!(b.n, 2 * a.n, "each level halves h");
    (order(a.l2, b.l2), order(a.energy, b.energy))
}

fn limits(kind: ElementKind) -> (f64, f64) {
    match kind {
        ElementKind::Tet10 => (2.7, 1.8),
        _ => (1.8, 0.9),
    }
}

/// A1 to A3 of one element and case (and H1 with another family).
fn check_orders(checks: &mut Checks, tag: &str, kind: ElementKind, levels: &[Level]) {
    check_orders_with(checks, tag, kind, levels, true);
}

/// As [`check_orders`]; with `assert_l2 = false` the L2 order is printed, not asserted (ADR 0279 amendment 1).
fn check_orders_with(checks: &mut Checks, tag: &str, kind: ElementKind, levels: &[Level], assert_l2: bool) {
    let (l2_limit, e_limit) = limits(kind);
    let (p_l2, p_e) = orders(levels);
    for w in levels.windows(2) {
        println!(
            "  {tag} {} pair {} -> {}: L2 order {:.4}, energy order {:.4}",
            kind.name(),
            w[0].n,
            w[1].n,
            order(w[0].l2, w[1].l2),
            order(w[0].energy, w[1].energy)
        );
    }
    let name = kind.name();
    if assert_l2 {
        checks.at_least(&format!("A1-{tag} {name} L2 order (finest pair)"), p_l2, l2_limit);
    } else {
        println!(
            "A1-{tag} {name} L2 order (finest pair): {p_l2:.4} (limit >= {l2_limit}; not asserted here, the known-miss \
             record of ADR 0279 amendment 1)"
        );
    }
    checks.at_least(&format!("A2-{tag} {name} energy order (finest pair)"), p_e, e_limit);
    let mono_l2 = levels.windows(2).all(|w| w[1].l2 < w[0].l2);
    let mono_e = levels.windows(2).all(|w| w[1].energy < w[0].energy);
    let l2s: Vec<String> = levels.iter().map(|l| format!("{:.3e}", l.l2)).collect();
    let es: Vec<String> = levels.iter().map(|l| format!("{:.3e}", l.energy)).collect();
    checks.holds(
        &format!("A3-{tag} {name} monotone fall"),
        mono_l2 && mono_e,
        &format!("L2 {l2s:?}, energy {es:?}"),
    );
    let total: f64 = levels.iter().map(|l| l.seconds).sum();
    let max_unknowns = levels.iter().map(|l| l.unknowns).max().unwrap_or(0);
    println!("  {tag} {name}: wall time {total:.1} s, largest solve {max_unknowns} free unknowns");
}

fn element_orders(kind: ElementKind, ns: &[usize]) {
    element_orders_with(kind, ns, true);
}

/// As [`element_orders`]; with `assert_l2_t = false` the L2 order of case T is printed, not asserted (amendment 1).
fn element_orders_with(kind: ElementKind, ns: &[usize], assert_l2_t: bool) {
    let field = Trig;
    let mut checks = Checks::new();
    let t0 = Instant::now();
    let d = run_sequence(&field, kind, ns, Case::D, 1.0);
    check_orders(&mut checks, "D", kind, &d);
    let t = run_sequence(&field, kind, ns, Case::T, 1.0);
    check_orders_with(&mut checks, "T", kind, &t, assert_l2_t);
    // N1: the body force with the lambda term reversed, case D.
    let neg = run_sequence(&field, kind, ns, Case::D, -1.0);
    let (p_l2, p_e) = orders(&neg);
    checks.below(&format!("N1 {} L2 order, lambda term reversed", kind.name()), p_l2, 0.5);
    checks.below(
        &format!("N1 {} energy order, lambda term reversed", kind.name()),
        p_e,
        0.5,
    );
    println!("{}: total wall time {:.1} s", kind.name(), t0.elapsed().as_secs_f64());
    checks.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// Tests (D4)

#[test]
fn f1_f2_field_derivatives_and_body_force() {
    let field = Trig;
    let mut rng = Rng::new(1);
    let h = FD_STEP;
    let (mut g_err, mut g_max, mut b_err, mut b_max) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for _ in 0..FD_POINTS {
        let x = [rng.uniform(), rng.uniform(), rng.uniform()];
        let g = field.grad(x);
        let b = body_force(&field, x, 1.0);
        let mut b_fd = [0.0; 3];
        for m in 0..3 {
            let mut xp = x;
            let mut xm = x;
            xp[m] += h;
            xm[m] -= h;
            let (up, um) = (field.u(xp), field.u(xm));
            let (sp, sm) = (stress_tensor(&field, xp), stress_tensor(&field, xm));
            for i in 0..3 {
                let fd = (up[i] - um[i]) / (2.0 * h);
                g_err = g_err.max((fd - g[i][m]).abs());
                g_max = g_max.max(g[i][m].abs());
                b_fd[i] -= (sp[i][m] - sm[i][m]) / (2.0 * h);
            }
        }
        for i in 0..3 {
            b_err = b_err.max((b_fd[i] - b[i]).abs());
            b_max = b_max.max(b[i].abs());
        }
    }
    println!("max |grad u| = {g_max:.4e}, max |b| = {b_max:.4e} over {FD_POINTS} points");
    let mut checks = Checks::new();
    checks.at_most(
        "F1 gradient against central differences (relative)",
        g_err / g_max,
        1e-6,
    );
    checks.at_most(
        "F2 body force against central differences of the stress (relative)",
        b_err / b_max,
        1e-6,
    );
    checks.finish();
}

#[test]
fn x1_traction_helper_against_uniform_traction() {
    let t0: Vec3 = [0.3, -0.5, 0.7];
    let mut checks = Checks::new();
    for kind in [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10] {
        let c = cube(kind, 2);
        let mut model = model_of(c.mesh.clone());
        for (set, _) in LOADED {
            model.add_load(Load::traction(set, t0)).unwrap();
        }
        let lib = nodal_load_vector(&model, &[]).unwrap();
        let mut mine = vec![0.0; 3 * model.mesh().nodes().len()];
        for (set, _) in LOADED {
            add_traction(&mut mine, model.mesh(), set, |_| t0);
        }
        let max = lib.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let diff = lib.iter().zip(&mine).fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
        checks.at_most(
            &format!("X1 {} traction helper (relative)", kind.name()),
            diff / max,
            1e-14,
        );
    }
    checks.finish();
}

fn factorial(n: u32) -> f64 {
    (1..=n).map(f64::from).product()
}

#[test]
fn x2_error_rules_integrate_monomials() {
    let mut checks = Checks::new();
    let rule = error_rule(ElementKind::Tet4);
    let mut worst = 0.0f64;
    for a in 0..=7u32 {
        for b in 0..=(7 - a) {
            for c in 0..=(7 - a - b) {
                let exact = factorial(a) * factorial(b) * factorial(c) / factorial(a + b + c + 3);
                let q: f64 = rule
                    .iter()
                    .map(|(p, w)| w * p[0].powi(a as i32) * p[1].powi(b as i32) * p[2].powi(c as i32))
                    .sum();
                worst = worst.max((q - exact).abs());
            }
        }
    }
    checks.at_most("X2 tetrahedron rule, total degree <= 7 (absolute)", worst, 1e-14);
    let rule = error_rule(ElementKind::Hex8);
    let line = |a: u32| if a % 2 == 1 { 0.0 } else { 2.0 / f64::from(a + 1) };
    let mut worst = 0.0f64;
    for a in 0..=9u32 {
        for b in 0..=9u32 {
            for c in 0..=9u32 {
                let exact = line(a) * line(b) * line(c);
                let q: f64 = rule
                    .iter()
                    .map(|(p, w)| w * p[0].powi(a as i32) * p[1].powi(b as i32) * p[2].powi(c as i32))
                    .sum();
                worst = worst.max((q - exact).abs());
            }
        }
    }
    checks.at_most("X2 cube rule, degree <= 9 per variable (absolute)", worst, 1e-14);
    checks.finish();
}

#[test]
fn x3_polynomial_fields_reproduced() {
    let mut checks = Checks::new();
    for kind in [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10] {
        let field = if kind == ElementKind::Tet10 {
            Poly::quadratic()
        } else {
            Poly::linear()
        };
        for case in [Case::D, Case::T] {
            let c = cube(kind, 2);
            let (model, result) = solve_case(&field, &c, case, 1.0);
            let [l2, energy, nl2, ne] = errors(&field, &model, &result);
            checks.at_most(
                &format!("X3 {} case {case:?} relative L2 error", kind.name()),
                l2 / nl2,
                1e-9,
            );
            checks.at_most(
                &format!("X3 {} case {case:?} relative energy error", kind.name()),
                energy / ne,
                1e-9,
            );
        }
    }
    checks.finish();
}

/// KNOWN MISS (ADR 0279 amendment 1), the record of the registered run (commit f489c085): A1-T Tet4, L2 order of case T over
/// the pair 8 -> 16 is 1.7925 against the limit 1.8 (errors 8.690e-2, 2.731e-2, 7.883e-3; pair 4 -> 8 1.6699). Limit kept.
/// Every other check of this test was met (A1-D 1.9793, A2-D 0.9940, A2-T 0.9750, A3 both cases, N1 -0.0014 and 0.0000).
#[test]
#[ignore = "KNOWN MISS (ADR 0279 amendment 1): Tet4 case T L2 order 1.7925 against 1.8"]
fn a_tet4_orders_cases_d_and_t_and_negative_control_known_miss() {
    element_orders(ElementKind::Tet4, &FAST_LINEAR);
}

/// Coverage of the Tet4 path (ADR 0279 amendment 1): the sequences of the registered run, asserting the clauses that were
/// met (A1-D, A2-D, A3-D, A2-T, A3-T, N1); A1-T is printed, its miss is the record above.
#[test]
fn a_tet4_met_clauses_coverage() {
    element_orders_with(ElementKind::Tet4, &FAST_LINEAR, false);
}

#[test]
fn a_hex8_orders_cases_d_and_t_and_negative_control() {
    element_orders(ElementKind::Hex8, &FAST_LINEAR);
}

#[test]
fn a_tet10_orders_cases_d_and_t_and_negative_control() {
    element_orders(ElementKind::Tet10, &FAST_TET10);
}

/// H1 (heavy record): the finer family, case D, the limits of A1 to A3. Run singly:
/// `cargo test -p rcfd-fem --release --test sb_mms -- --ignored h1 --nocapture`.
#[test]
#[ignore = "heavy record (ADR 0279 H1): solves of up to 36 501 unknowns, minutes in release"]
fn h1_heavy_finer_family_case_d() {
    let field = Trig;
    let mut checks = Checks::new();
    let t0 = Instant::now();
    for (kind, ns) in [
        (ElementKind::Tet4, HEAVY_LINEAR),
        (ElementKind::Hex8, HEAVY_LINEAR),
        (ElementKind::Tet10, HEAVY_TET10),
    ] {
        let levels = run_sequence(&field, kind, &ns, Case::D, 1.0);
        check_orders(&mut checks, "H1", kind, &levels);
    }
    let total = t0.elapsed().as_secs_f64();
    checks.at_most("H1 total wall time, s (budget)", total, 1800.0);
    checks.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// Diagnostics of ADR 0279 amendment 1 (recorded, no limit)

fn print_orders(tag: &str, kind: ElementKind, levels: &[Level]) {
    for w in levels.windows(2) {
        println!(
            "{tag} {} case T pair {} -> {}: L2 order {:.4}, energy order {:.4}",
            kind.name(),
            w[0].n,
            w[1].n,
            order(w[0].l2, w[1].l2),
            order(w[0].energy, w[1].energy)
        );
    }
}

/// G1: Tet4 case T on the heavy family 6, 12, 24.
#[test]
#[ignore = "diagnostic record (ADR 0279 amendment 1, G1): Tet4 case T on 6, 12, 24; about two minutes"]
fn g1_tet4_case_t_heavy_family_diagnostic() {
    let levels = run_sequence(&Trig, ElementKind::Tet4, &HEAVY_LINEAR, Case::T, 1.0);
    print_orders("G1", ElementKind::Tet4, &levels);
}

/// G2: Tet4 case T on 16 and 32.
#[test]
#[ignore = "diagnostic record (ADR 0279 amendment 1, G2): Tet4 case T on 16, 32 (98 304 unknowns); minutes"]
fn g2_tet4_case_t_one_level_finer_diagnostic() {
    let levels = run_sequence(&Trig, ElementKind::Tet4, &[16, 32], Case::T, 1.0);
    print_orders("G2", ElementKind::Tet4, &levels);
}

/// The L2 error of case T split into elements with a node on a loaded face (near) and the rest, with the error rule.
fn split_l2(field: &dyn Field, model: &SolidModel, result: &SolidResult) -> (f64, f64) {
    let mesh = model.mesh();
    let mut loaded = BTreeSet::new();
    for (set, _) in LOADED {
        loaded.extend(mesh.node_set(set).unwrap().iter().copied());
    }
    let tet_rule = error_rule(ElementKind::Tet4);
    let hex_rule = error_rule(ElementKind::Hex8);
    let (mut near, mut rest) = (0.0, 0.0);
    for e in 0..mesh.n_elements() {
        let kind = mesh.element_kind(e);
        let element = element_for(kind);
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let rule = if kind == ElementKind::Hex8 {
            &hex_rule
        } else {
            &tet_rule
        };
        let mut s = 0.0;
        for &(xi, wq) in rule {
            let p = element.point(&coords, xi).unwrap();
            let mut uh = [0.0; 3];
            for (a, &node) in nodes.iter().enumerate() {
                let ua = result.displacement(node);
                for i in 0..3 {
                    uh[i] += p.n[a] * ua[i];
                }
            }
            let u = field.u(p.x);
            s += wq * p.det_j * (0..3).map(|i| (u[i] - uh[i]) * (u[i] - uh[i])).sum::<f64>();
        }
        if nodes.iter().any(|a| loaded.contains(a)) {
            near += s;
        } else {
            rest += s;
        }
    }
    (near.sqrt(), rest.sqrt())
}

/// G3: Tet4 and Hex8, case T on 8 and 16, the L2 error near the loaded faces and away from them.
#[test]
#[ignore = "diagnostic record (ADR 0279 amendment 1, G3): case T error near the loaded faces; seconds"]
fn g3_case_t_error_near_the_loaded_faces_diagnostic() {
    for kind in [ElementKind::Tet4, ElementKind::Hex8] {
        let mut parts = Vec::new();
        for n in [8, 16] {
            let c = cube(kind, n);
            let (model, result) = solve_case(&Trig, &c, Case::T, 1.0);
            let (near, rest) = split_l2(&Trig, &model, &result);
            println!(
                "G3 {} n = {n}: L2 error near the loaded faces {near:.4e}, rest {rest:.4e}",
                kind.name()
            );
            parts.push((near, rest));
        }
        println!(
            "G3 {} pair 8 -> 16: order near {:.4}, order rest {:.4}",
            kind.name(),
            order(parts[0].0, parts[1].0),
            order(parts[0].1, parts[1].1)
        );
    }
}
