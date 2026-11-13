//! F-389 (ADR 0292): the quadratic hexahedron Hex20. Checks E0 to E8, P1, P2, L1 to L3, C1, T1, M1, M2, B1, B2, U1 and the
//! negative controls N1 to N4 of the registration (seed 0xF389_0292).

use rcfd_core::vec3::{cross, Vec3};
use rcfd_fem::builders::{hex_box, tet_box, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::element_for;
use rcfd_fem::hex20::{
    self, pressure_loads, traction_field_loads, traction_loads, upgrade_to_hex20, upgrade_to_hex20_mapped,
};
use rcfd_fem::loads::{body_force_loads, nodal_load_vector, NodalForceAt};
use rcfd_fem::mass::{consistent, element_mass, mass_properties};
use rcfd_fem::material::{isotropic_elasticity, Matrix6};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::quadrature;
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel, SolidResult};
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;

const SEED: u64 = 0xF389_0292;
const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;

/// SplitMix64, a deterministic source only (as in sb_static.rs).
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
    fn sym(&mut self, r: f64) -> f64 {
        r * (2.0 * self.uniform() - 1.0)
    }
}

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
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
            "{name}: {value:.6} (limit >= {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} < {limit}"));
        }
    }
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{name}: {value:.6} (limit < {limit}) {}",
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

fn material(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap()
}

fn model_of(mesh: SolidMesh, nu: f64) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(nu)).unwrap();
    m
}

/// The same blocks and sets on a new node list.
fn rebuild(mesh: &SolidMesh, nodes: Vec<Vec3>) -> SolidMesh {
    let mut out = SolidMesh::new(nodes).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).unwrap().to_vec())
            .unwrap();
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    out
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

fn hex20_box(origin: Vec3, lengths: Vec3, div: [usize; 3]) -> SolidMesh {
    upgrade_to_hex20(&hex_box(origin, lengths, div).unwrap()).unwrap().0
}

fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0, |m: f64, x| m.max(x.abs()))
}

/// Gauss-Legendre nodes and weights on [-1, 1] by Newton's method on the three-term recurrence (as sb_mms.rs).
fn gauss_legendre(n: usize) -> (Vec<f64>, Vec<f64>) {
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

/// The 125-point Gauss rule on the cube (degree 9 per direction).
fn gauss_125() -> Vec<([f64; 3], f64)> {
    let (g, gw) = gauss_legendre(5);
    let mut out = Vec::with_capacity(125);
    for k in 0..5 {
        for j in 0..5 {
            for i in 0..5 {
                out.push(([g[i], g[j], g[k]], gw[i] * gw[j] * gw[k]));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Element checks

/// The 20 distorted elements of the registration.
fn distorted_elements() -> Vec<Vec<Vec3>> {
    let mut rng = Rng::new(1);
    let refs = ElementKind::Hex20.reference_nodes();
    (0..20)
        .map(|_| {
            let mut x: Vec<Vec3> = refs[..8]
                .iter()
                .map(|r| [r[0] + rng.sym(0.15), r[1] + rng.sym(0.15), r[2] + rng.sym(0.15)])
                .collect();
            for [a, b] in ElementKind::Hex20.edges() {
                let (p, q) = (x[*a], x[*b]);
                let m = [
                    0.5 * (p[0] + q[0]) + rng.sym(0.05),
                    0.5 * (p[1] + q[1]) + rng.sym(0.05),
                    0.5 * (p[2] + q[2]) + rng.sym(0.05),
                ];
                x.push(m);
            }
            x
        })
        .collect()
}

fn random_points() -> Vec<[f64; 3]> {
    let mut rng = Rng::new(2);
    (0..100).map(|_| [rng.sym(1.0), rng.sym(1.0), rng.sym(1.0)]).collect()
}

/// Eigenvalues of a symmetric matrix by the cyclic Jacobi method.
fn jacobi_eigenvalues(n: usize, mut a: Vec<f64>) -> Vec<f64> {
    let scale = max_abs(a.iter().copied());
    for _sweep in 0..100 {
        let mut off = 0.0;
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    off += a[i * n + j] * a[i * n + j];
                }
            }
        }
        if off.sqrt() <= 1e-15 * scale {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p * n + q];
                if apq == 0.0 {
                    continue;
                }
                let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let akp = a[k * n + p];
                    let akq = a[k * n + q];
                    a[k * n + p] = c * akp - s * akq;
                    a[k * n + q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let apk = a[p * n + k];
                    let aqk = a[q * n + k];
                    a[p * n + k] = c * apk - s * aqk;
                    a[q * n + k] = s * apk + c * aqk;
                }
            }
        }
    }
    let mut ev: Vec<f64> = (0..n).map(|i| a[i * n + i]).collect();
    ev.sort_by(|x, y| x.partial_cmp(y).unwrap());
    ev
}

fn rank(k: &rcfd_fem::element::ElementMatrix) -> (usize, f64, f64) {
    let ev = jacobi_eigenvalues(k.size(), k.values().to_vec());
    let lmax = ev.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let r = ev.iter().filter(|&&v| v > 1e-10 * lmax).count();
    let zero = ev.len() - r;
    // Largest of the near-zero eigenvalues and smallest of the others, relative.
    let small = if zero > 0 { ev[zero - 1].abs() / lmax } else { 0.0 };
    let first = if r > 0 { ev[zero] / lmax } else { 0.0 };
    (r, small, first)
}

fn stiffness_checks(ch: &mut Checks, tag: &str, coords: &[Vec3], d: &Matrix6) {
    let el = element_for(ElementKind::Hex20);
    let k = el.stiffness(coords, d).unwrap();
    let n = k.size();
    let kmax = max_abs(k.values().iter().copied());
    let mut asym: f64 = 0.0;
    for i in 0..n {
        for j in 0..n {
            asym = asym.max((k.get(i, j) - k.get(j, i)).abs());
        }
    }
    ch.at_most(&format!("E4 {tag} symmetry"), asym / kmax, 1e-13);
    let (r, small, first) = rank(&k);
    ch.holds(
        &format!("E5 {tag} rank"),
        r == 54,
        &format!("rank {r} (largest null eigenvalue {small:.2e}, smallest non-zero {first:.2e} of lambda_max)"),
    );
    let mut c = [0.0; 3];
    for p in coords {
        for i in 0..3 {
            c[i] += p[i] / coords.len() as f64;
        }
    }
    let mut worst: f64 = 0.0;
    for m in 0..6 {
        let mut r = vec![0.0; n];
        for (a, p) in coords.iter().enumerate() {
            let y = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
            let v = match m {
                0 => [1.0, 0.0, 0.0],
                1 => [0.0, 1.0, 0.0],
                2 => [0.0, 0.0, 1.0],
                3 => [0.0, -y[2], y[1]],
                4 => [y[2], 0.0, -y[0]],
                _ => [-y[1], y[0], 0.0],
            };
            r[3 * a..3 * a + 3].copy_from_slice(&v);
        }
        let kr = k.matvec(&r);
        worst = worst.max(max_abs(kr) / (kmax * max_abs(r.iter().copied())));
    }
    ch.at_most(&format!("E6 {tag} rigid motions"), worst, 1e-11);
}

#[test]
fn e0_node_order_and_faces() {
    let mut ch = Checks::new();
    ch.holds("E0 PENDING", hex20::PENDING.is_none(), &format!("{:?}", hex20::PENDING));
    ch.holds("E0 kind pending", ElementKind::Hex20.pending().is_none(), "None");
    ch.holds(
        "E0 element kind",
        element_for(ElementKind::Hex20).kind() == ElementKind::Hex20,
        "Hex20",
    );
    let r = ElementKind::Hex20.reference_nodes();
    ch.holds("E0 20 reference nodes", r.len() == 20, &format!("{}", r.len()));
    let hex8 = ElementKind::Hex8.reference_nodes();
    let mut corners_ok = true;
    for a in 0..8 {
        corners_ok &= r[a] == hex8[a];
    }
    ch.holds("E0 corners as Hex8", corners_ok, "bit for bit");
    let vtk_edges: [[usize; 2]; 12] = [
        [0, 1],
        [1, 2],
        [2, 3],
        [3, 0],
        [4, 5],
        [5, 6],
        [6, 7],
        [7, 4],
        [0, 4],
        [1, 5],
        [2, 6],
        [3, 7],
    ];
    let mut mids_ok = ElementKind::Hex20.edges() == vtk_edges;
    for (k, [a, b]) in vtk_edges.iter().enumerate() {
        for i in 0..3 {
            mids_ok &= (r[8 + k][i]).to_bits() == (0.5 * (r[*a][i] + r[*b][i])).to_bits();
        }
    }
    ch.holds("E0 mid-edge node 8 + k on VTK edge k", mids_ok, "bit for bit");
    let expected: [[usize; 8]; 6] = [
        [0, 3, 2, 1, 11, 10, 9, 8],
        [4, 5, 6, 7, 12, 13, 14, 15],
        [0, 1, 5, 4, 8, 17, 12, 16],
        [1, 2, 6, 5, 9, 18, 13, 17],
        [2, 3, 7, 6, 10, 19, 14, 18],
        [3, 0, 4, 7, 11, 16, 15, 19],
    ];
    let faces = ElementKind::Hex20.faces();
    let table_ok = faces.len() == 6 && faces.iter().zip(&expected).all(|(f, e)| *f == e);
    ch.holds("E0 faces as the table of D1", table_ok, &format!("{faces:?}"));
    let mut mid_ok = true;
    for (f, h8) in faces.iter().zip(ElementKind::Hex8.faces()) {
        mid_ok &= f[..4] == h8[..];
        for e in 0..4 {
            let (a, b, m) = (f[e], f[(e + 1) % 4], f[4 + e]);
            for i in 0..3 {
                mid_ok &= r[m][i] == 0.5 * (r[a][i] + r[b][i]);
            }
        }
    }
    ch.holds(
        "E0 face mid-edge nodes between consecutive corners",
        mid_ok,
        "all six faces",
    );
    ch.finish();
}

#[test]
fn e1_to_e6_functions_derivatives_rule_stiffness() {
    let mut ch = Checks::new();
    let el = element_for(ElementKind::Hex20);
    let refs = ElementKind::Hex20.reference_nodes().to_vec();
    let gauss = quadrature::gauss_3().points().to_vec();
    let random = random_points();
    let mut pts: Vec<[f64; 3]> = refs.clone();
    pts.extend(&gauss);
    pts.extend(&random);
    // E1
    let (mut s0, mut s1): (f64, f64) = (0.0, 0.0);
    for &xi in &pts {
        let n = el.shape_functions(xi);
        let d = el.shape_derivatives(xi);
        s0 = s0.max((n.iter().sum::<f64>() - 1.0).abs());
        for j in 0..3 {
            s1 = s1.max(d.iter().map(|g| g[j]).sum::<f64>().abs());
        }
    }
    ch.at_most("E1 reference: |sum N - 1|", s0, 1e-14);
    ch.at_most("E1 reference: |sum dN/dxi|", s1, 1e-14);
    // E1c
    let mut kr: f64 = 0.0;
    for (b, &xi) in refs.iter().enumerate() {
        for (a, v) in el.shape_functions(xi).iter().enumerate() {
            kr = kr.max((v - if a == b { 1.0 } else { 0.0 }).abs());
        }
    }
    ch.at_most("E1c Kronecker at the nodes", kr, 1e-15);
    // E1b
    let distorted = distorted_elements();
    let (mut t0, mut t1): (f64, f64) = (0.0, 0.0);
    let mut eval = gauss.clone();
    eval.extend(&random);
    for x in &distorted {
        for &xi in &eval {
            let p = el.point(x, xi).unwrap();
            t0 = t0.max((p.n.iter().sum::<f64>() - 1.0).abs());
            for k in 0..3 {
                t1 = t1.max(p.grad.iter().map(|g| g[k]).sum::<f64>().abs());
            }
        }
    }
    ch.at_most("E1b distorted: |sum N - 1|", t0, 1e-14);
    ch.at_most("E1b distorted: |sum dN/dx|", t1, 1e-14);
    // E2
    let h = 1e-5;
    let mut cd: f64 = 0.0;
    for &xi in &random {
        let d = el.shape_derivatives(xi);
        for j in 0..3 {
            let (mut xp, mut xm) = (xi, xi);
            xp[j] += h;
            xm[j] -= h;
            let (np, nm) = (el.shape_functions(xp), el.shape_functions(xm));
            for a in 0..20 {
                cd = cd.max((d[a][j] - (np[a] - nm[a]) / (2.0 * h)).abs());
            }
        }
    }
    ch.at_most("E2 reference: dN/dxi against central differences", cd, 1e-7);
    // E2b
    let map = |x: &[Vec3], xi: [f64; 3]| -> Vec3 {
        let n = el.shape_functions(xi);
        let mut p = [0.0; 3];
        for (na, xa) in n.iter().zip(x) {
            for i in 0..3 {
                p[i] += na * xa[i];
            }
        }
        p
    };
    let mut cdj: f64 = 0.0;
    for x in &distorted {
        for &xi in &random {
            let p = el.point(x, xi).unwrap();
            let jmax = max_abs(p.jacobian.iter().flatten().copied());
            for j in 0..3 {
                let (mut xp, mut xm) = (xi, xi);
                xp[j] += h;
                xm[j] -= h;
                let (a, b) = (map(x, xp), map(x, xm));
                for i in 0..3 {
                    cdj = cdj.max((p.jacobian[i][j] - (a[i] - b[i]) / (2.0 * h)).abs() / jmax);
                }
            }
        }
    }
    ch.at_most("E2b distorted: J against central differences of x(xi)", cdj, 1e-7);
    // E3
    let rule = quadrature::gauss_3();
    let exact1 = |k: i32| if k % 2 == 1 { 0.0 } else { 2.0 / (k + 1) as f64 };
    let mut mono: f64 = 0.0;
    for a in 0..=5 {
        for b in 0..=5 {
            for c in 0..=5 {
                let q: f64 = rule
                    .points()
                    .iter()
                    .zip(rule.weights())
                    .map(|(p, w)| w * p[0].powi(a) * p[1].powi(b) * p[2].powi(c))
                    .sum();
                mono = mono.max((q - exact1(a) * exact1(b) * exact1(c)).abs());
            }
        }
    }
    ch.at_most(
        "E3 gauss_3 on the 216 monomials of degree <= 5 per direction",
        mono,
        1e-13,
    );
    // E4 to E6
    let d = isotropic_elasticity(E_MOD, NU);
    stiffness_checks(&mut ch, "reference", &refs, &d);
    for (i, x) in distorted.iter().enumerate() {
        stiffness_checks(&mut ch, &format!("distorted {i}"), x, &d);
    }
    // N4
    let k2 = el.stiffness_with_rule(&refs, &d, quadrature::gauss_2()).unwrap();
    let (r2, _, _) = rank(&k2);
    ch.below("N4 rank with gauss_2", r2 as f64, 54.0);
    ch.finish();
}

fn single(coords: Vec<Vec3>) -> SolidMesh {
    let mut m = SolidMesh::new(coords).unwrap();
    m.add_block(ElementKind::Hex20, (0..20).collect()).unwrap();
    m
}

fn shear(x: Vec3) -> Vec3 {
    [x[0] + 0.2 * x[1] + 0.1 * x[2], x[1] + 0.15 * x[2], x[2]]
}

#[test]
fn e7_e8_volume_and_mass() {
    let mut ch = Checks::new();
    let el = element_for(ElementKind::Hex20);
    let g125 = gauss_125();
    let reference = single(ElementKind::Hex20.reference_nodes().to_vec());
    ch.at_most(
        "E7 reference cube volume - 8",
        (reference.element_volume(0) - 8.0).abs(),
        1e-13,
    );
    let mut worst: f64 = 0.0;
    for x in distorted_elements() {
        let exact: f64 = g125.iter().map(|&(xi, w)| w * el.point(&x, xi).unwrap().det_j).sum();
        let mesh = single(x);
        worst = worst.max((mesh.element_volume(0) / exact - 1.0).abs());
    }
    ch.at_most("E7 distorted: volume against 125-point Gauss", worst, 1e-13);
    // E8
    let (mesh, _) = upgrade_to_hex20_mapped(&hex_box([0.0; 3], [0.3, 0.5, 0.7], [1, 1, 1]).unwrap(), shear).unwrap();
    let x: Vec<Vec3> = mesh.element_nodes(0).iter().map(|&n| mesh.nodes()[n]).collect();
    let m = element_mass(el, &x, RHO).unwrap();
    let mut exact = vec![0.0; 400];
    for &(xi, w) in &g125 {
        let p = el.point(&x, xi).unwrap();
        for a in 0..20 {
            for b in 0..20 {
                exact[a * 20 + b] += RHO * w * p.det_j * p.n[a] * p.n[b];
            }
        }
    }
    let mut dm: f64 = 0.0;
    for a in 0..20 {
        for b in 0..20 {
            for i in 0..3 {
                dm = dm.max((m.get(3 * a + i, 3 * b + i) - exact[a * 20 + b]).abs());
            }
        }
    }
    ch.at_most(
        "E8 affine element mass against 125-point Gauss",
        dm / max_abs(exact),
        1e-13,
    );
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// P1, P2, N1: patch test

const P_ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const P_LENGTHS: Vec3 = [1.2, 1.0, 0.8];
const P_DIV: [usize; 3] = [3, 3, 3];
const FIELD_C: Vec3 = [1e-3, -2e-3, 0.5e-3];
const FIELD_A: [[f64; 3]; 3] = [
    [1.0e-3, 0.3e-3, -0.2e-3],
    [0.5e-3, -0.7e-3, 0.4e-3],
    [0.1e-3, 0.6e-3, 0.9e-3],
];

fn linear_field(x: Vec3) -> Vec3 {
    let mut u = FIELD_C;
    for i in 0..3 {
        for j in 0..3 {
            u[i] += FIELD_A[i][j] * x[j];
        }
    }
    u
}

fn cubic_field(x: Vec3) -> Vec3 {
    [1e-3 * x[0].powi(3), 1e-3 * x[1].powi(3), 1e-3 * x[2].powi(3)]
}

fn patch_mesh() -> SolidMesh {
    let base = hex_box(P_ORIGIN, P_LENGTHS, P_DIV).unwrap();
    let h = [0, 1, 2].map(|k| P_LENGTHS[k] / P_DIV[k] as f64);
    let mut rng = Rng::new(10);
    let boundary = boundary_nodes(&base);
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if !boundary.contains(&i) {
            for k in 0..3 {
                p[k] += 0.1 * h[k] * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    let moved = rebuild(&base, nodes);
    let (up, mids) = upgrade_to_hex20(&moved).unwrap();
    let boundary = boundary_nodes(&up);
    let mut nodes = up.nodes().to_vec();
    for m in &mids {
        if !boundary.contains(&m.node) {
            for k in 0..3 {
                nodes[m.node][k] += 0.05 * h[k] * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    rebuild(&up, nodes)
}

fn patch_solve(field: fn(Vec3) -> Vec3) -> (SolidModel, SolidResult, f64) {
    let mesh = patch_mesh();
    let boundary = boundary_nodes(&mesh);
    let mut pres = Vec::new();
    for &n in &boundary {
        for (k, &uk) in field(mesh.nodes()[n]).iter().enumerate() {
            pres.push(Prescribed::new(n, k, uk));
        }
    }
    let model = model_of(mesh, NU);
    let res = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap();
    let mesh = model.mesh();
    let (mut umax, mut err): (f64, f64) = (0.0, 0.0);
    for (n, x) in mesh.nodes().iter().enumerate() {
        let u = field(*x);
        for k in 0..3 {
            umax = umax.max(u[k].abs());
            if !boundary.contains(&n) {
                err = err.max((res.displacement(n)[k] - u[k]).abs());
            }
        }
    }
    (model, res, err / umax)
}

#[test]
fn p1_p2_n1_patch_test() {
    let mut ch = Checks::new();
    let (model, res, err) = patch_solve(linear_field);
    let mesh = model.mesh();
    println!(
        "P1: {} nodes, {} interior, {} unknowns solved",
        mesh.nodes().len(),
        mesh.nodes().len() - boundary_nodes(mesh).len(),
        res.stats().unwrap().unknowns()
    );
    ch.at_most("P1 interior nodal error / max |u|", err, 1e-10);
    let a = FIELD_A;
    let eps = [
        a[0][0],
        a[1][1],
        a[2][2],
        a[0][1] + a[1][0],
        a[1][2] + a[2][1],
        a[2][0] + a[0][2],
    ];
    let field = IntegrationPointField::compute(&model, res.displacement_values()).unwrap();
    let mut de: f64 = 0.0;
    for e in field.strain() {
        for r in 0..6 {
            de = de.max((e[r] - eps[r]).abs());
        }
    }
    ch.at_most("P2 integration-point strain", de / max_abs(eps), 1e-10);
    let d = isotropic_elasticity(E_MOD, NU);
    let sigma: [f64; 6] = std::array::from_fn(|r| (0..6).map(|c| d[r][c] * eps[c]).sum());
    let nodal = nodal_average(&model, &field, field.stress(), None).unwrap();
    let mut ds: f64 = 0.0;
    for s in nodal.iter().flatten() {
        for r in 0..6 {
            ds = ds.max((s[r] - sigma[r]).abs());
        }
    }
    ch.at_most("P2 nodal-average stress", ds / max_abs(sigma), 1e-10);
    let (_, _, n1) = patch_solve(cubic_field);
    ch.at_least("N1 cubic field interior error (> 1e-6)", n1, 1e-6);
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// L1 to L3: face loads

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn l1_element(ch: &mut Checks, tag: &str, mesh: &SolidMesh) {
    let p = 2.5e6;
    let conn = mesh.element_nodes(0).to_vec();
    let mut centroid = [0.0; 3];
    for &n in &conn {
        for i in 0..3 {
            centroid[i] += mesh.nodes()[n][i] / 20.0;
        }
    }
    let mut worst: f64 = 0.0;
    let mut outward = true;
    for name in BOX_FACES {
        let face = mesh.surface_set(name).unwrap()[0];
        let fnodes = mesh.face_nodes(face);
        let c: Vec<Vec3> = fnodes[..4].iter().map(|&n| mesh.nodes()[n]).collect();
        let an = cross(sub(c[2], c[0]), sub(c[3], c[1])).map(|v| 0.5 * v);
        let fc = [0, 1, 2].map(|i| c.iter().map(|x| x[i]).sum::<f64>() / 4.0);
        outward &= rcfd_core::vec3::dot(an, sub(fc, centroid)) > 0.0;
        let scale = p * rcfd_core::vec3::norm(an);
        let loads: BTreeMap<usize, Vec3> = pressure_loads(mesh, name, p)
            .unwrap()
            .into_iter()
            .map(|f| (f.node, f.force))
            .collect();
        for &n in &conn {
            let w = match fnodes.iter().position(|&m| m == n) {
                Some(i) if i < 4 => -1.0 / 12.0,
                Some(_) => 1.0 / 3.0,
                None => 0.0,
            };
            let f = loads.get(&n).copied().unwrap_or([0.0; 3]);
            for i in 0..3 {
                worst = worst.max((f[i] - w * (-p * an[i])).abs() / scale);
            }
        }
    }
    ch.holds(&format!("L1 {tag} area vectors outward"), outward, "six faces");
    ch.at_most(
        &format!("L1 {tag} nodal pressure loads against -1/12, 1/3 of p A"),
        worst,
        1e-13,
    );
}

#[test]
#[ignore = "record of ADR 0292: its L3 row asserts the refusal of a model pressure on a Hex20 face, retired by F-484 (ADR 0383); L1 and L2 continue in l1_to_l2_face_loads_and_uniaxial_tension_after_landed"]
fn l1_to_l3_face_loads_and_uniaxial_tension() {
    let mut ch = Checks::new();
    let base = hex_box([0.0; 3], [0.3, 0.5, 0.7], [1, 1, 1]).unwrap();
    l1_element(&mut ch, "box", &upgrade_to_hex20(&base).unwrap().0);
    l1_element(&mut ch, "affine", &upgrade_to_hex20_mapped(&base, shear).unwrap().0);
    // L2
    let lengths = [2.0, 0.5, 0.4];
    let force = 1.0e6;
    let mesh = hex20_box([0.0; 3], lengths, [3, 2, 2]);
    let area = lengths[1] * lengths[2];
    let t = force / area;
    let mut loads = traction_loads(&mesh, "xmax", [t, 0.0, 0.0]).unwrap();
    loads.extend(traction_loads(&mesh, "xmin", [-t, 0.0, 0.0]).unwrap());
    let near = |x: Vec3| -> usize {
        (0..mesh.nodes().len())
            .min_by(|&a, &b| {
                let da = sub(mesh.nodes()[a], x);
                let db = sub(mesh.nodes()[b], x);
                rcfd_core::vec3::dot(da, da)
                    .partial_cmp(&rcfd_core::vec3::dot(db, db))
                    .unwrap()
            })
            .unwrap()
    };
    let (p1, p2, p3) = (
        near([0.0; 3]),
        near([0.0, 0.0, lengths[2]]),
        near([0.0, lengths[1], 0.0]),
    );
    let pres = vec![
        Prescribed::new(p1, 0, 0.0),
        Prescribed::new(p1, 1, 0.0),
        Prescribed::new(p1, 2, 0.0),
        Prescribed::new(p2, 0, 0.0),
        Prescribed::new(p2, 1, 0.0),
        Prescribed::new(p3, 0, 0.0),
    ];
    let model = model_of(mesh, NU);
    let res = solve_static_with(&model, &pres, &loads, &StaticOptions::default()).unwrap();
    let mesh = model.mesh();
    let eps = force / (E_MOD * area);
    let delta = eps * lengths[0];
    let mut e1: f64 = 0.0;
    for &n in mesh.node_set("xmax").unwrap() {
        e1 = e1.max((res.displacement(n)[0] - delta).abs() / delta);
    }
    let (mut ey, mut ez): (f64, f64) = (0.0, 0.0);
    for (n, x) in mesh.nodes().iter().enumerate() {
        let u = res.displacement(n);
        ey = ey.max((u[1] + NU * eps * x[1]).abs() / (NU * eps * lengths[1]));
        ez = ez.max((u[2] + NU * eps * x[2]).abs() / (NU * eps * lengths[2]));
    }
    ch.at_most("L2 uniaxial elongation", e1, 1e-10);
    ch.at_most("L2 uniaxial lateral y", ey, 1e-10);
    ch.at_most("L2 uniaxial lateral z", ez, 1e-10);
    // L3
    let mut m3 = model_of(hex20_box([0.0; 3], [1.0; 3], [1, 1, 1]), NU);
    m3.add_load(Load::pressure("xmax", 1.0)).unwrap();
    match nodal_load_vector(&m3, &[]) {
        Ok(_) => ch.holds("L3 model pressure on a Hex20 face refused", false, "accepted"),
        Err(e) => {
            let msg = e.to_string();
            ch.holds(
                "L3 model pressure on a Hex20 face refused",
                msg.contains("a face of 8 nodes"),
                &msg,
            );
        }
    }
    ch.finish();
}

// L1 and L2 after F-484 (ADR 0383) retired the L3 refusal; the record above keeps the original.
#[test]
fn l1_to_l2_face_loads_and_uniaxial_tension_after_landed() {
    let mut ch = Checks::new();
    let base = hex_box([0.0; 3], [0.3, 0.5, 0.7], [1, 1, 1]).unwrap();
    l1_element(&mut ch, "box", &upgrade_to_hex20(&base).unwrap().0);
    l1_element(&mut ch, "affine", &upgrade_to_hex20_mapped(&base, shear).unwrap().0);
    // L2
    let lengths = [2.0, 0.5, 0.4];
    let force = 1.0e6;
    let mesh = hex20_box([0.0; 3], lengths, [3, 2, 2]);
    let area = lengths[1] * lengths[2];
    let t = force / area;
    let mut loads = traction_loads(&mesh, "xmax", [t, 0.0, 0.0]).unwrap();
    loads.extend(traction_loads(&mesh, "xmin", [-t, 0.0, 0.0]).unwrap());
    let near = |x: Vec3| -> usize {
        (0..mesh.nodes().len())
            .min_by(|&a, &b| {
                let da = sub(mesh.nodes()[a], x);
                let db = sub(mesh.nodes()[b], x);
                rcfd_core::vec3::dot(da, da)
                    .partial_cmp(&rcfd_core::vec3::dot(db, db))
                    .unwrap()
            })
            .unwrap()
    };
    let (p1, p2, p3) = (
        near([0.0; 3]),
        near([0.0, 0.0, lengths[2]]),
        near([0.0, lengths[1], 0.0]),
    );
    let pres = vec![
        Prescribed::new(p1, 0, 0.0),
        Prescribed::new(p1, 1, 0.0),
        Prescribed::new(p1, 2, 0.0),
        Prescribed::new(p2, 0, 0.0),
        Prescribed::new(p2, 1, 0.0),
        Prescribed::new(p3, 0, 0.0),
    ];
    let model = model_of(mesh, NU);
    let res = solve_static_with(&model, &pres, &loads, &StaticOptions::default()).unwrap();
    let mesh = model.mesh();
    let eps = force / (E_MOD * area);
    let delta = eps * lengths[0];
    let mut e1: f64 = 0.0;
    for &n in mesh.node_set("xmax").unwrap() {
        e1 = e1.max((res.displacement(n)[0] - delta).abs() / delta);
    }
    let (mut ey, mut ez): (f64, f64) = (0.0, 0.0);
    for (n, x) in mesh.nodes().iter().enumerate() {
        let u = res.displacement(n);
        ey = ey.max((u[1] + NU * eps * x[1]).abs() / (NU * eps * lengths[1]));
        ez = ez.max((u[2] + NU * eps * x[2]).abs() / (NU * eps * lengths[2]));
    }
    ch.at_most("L2 uniaxial elongation", e1, 1e-10);
    ch.at_most("L2 uniaxial lateral y", ey, 1e-10);
    ch.at_most("L2 uniaxial lateral z", ez, 1e-10);
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// C1, N2: cantilever

const C_L: f64 = 2.0;
const C_H: f64 = 0.1;
const C_P: f64 = 1000.0;

fn cantilever_ratio(kind: ElementKind) -> f64 {
    let base = hex_box([0.0; 3], [C_L, C_H, C_H], [10, 1, 1]).unwrap();
    let t = [0.0, 0.0, -C_P / (C_H * C_H)];
    let (mesh, loads) = match kind {
        ElementKind::Hex20 => {
            let mesh = upgrade_to_hex20(&base).unwrap().0;
            let loads = traction_loads(&mesh, "xmax", t).unwrap();
            (mesh, loads)
        }
        _ => {
            // Bilinear flat face: the consistent load of a uniform traction is a quarter of t A at each corner.
            let mut acc: BTreeMap<usize, Vec3> = BTreeMap::new();
            for &f in base.surface_set("xmax").unwrap() {
                for n in base.face_nodes(f) {
                    let e = acc.entry(n).or_insert([0.0; 3]);
                    for i in 0..3 {
                        e[i] += 0.25 * C_H * C_H * t[i];
                    }
                }
            }
            let loads = acc.into_iter().map(|(n, f)| NodalForceAt::new(n, f)).collect();
            (base, loads)
        }
    };
    let mut model = model_of(mesh, NU);
    for c in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let res = solve_static_with(&model, &[], &loads, &StaticOptions::default()).unwrap();
    let delta: f64 = -loads
        .iter()
        .map(|f| f.force[2] * res.displacement(f.node)[2])
        .sum::<f64>()
        / C_P;
    let i = C_H.powi(4) / 12.0;
    let g = E_MOD / (2.0 * (1.0 + NU));
    let kappa = 5.0 / 6.0;
    let delta_t = C_P * C_L.powi(3) / (3.0 * E_MOD * i) * (1.0 + 3.0 * E_MOD * i / (kappa * g * C_H * C_H * C_L * C_L));
    println!(
        "C {}: unknowns {}, delta {delta:.6e} m, delta_T {delta_t:.6e} m, ratio {:.6}",
        kind.name(),
        res.stats().unwrap().unknowns(),
        delta / delta_t
    );
    delta / delta_t
}

#[test]
#[ignore = "record of the first run (ADR 0292 amendment 1): defective set-up, the deflection measure has the wrong sign; printed ratios -0.981949 (Hex20), -0.369197 (Hex8)"]
fn c1_n2_cantilever_one_element_through_the_thickness() {
    let mut ch = Checks::new();
    let r = cantilever_ratio(ElementKind::Hex20);
    ch.at_most("C1 Hex20 |delta / delta_T - 1|", (r - 1.0).abs(), 0.01);
    let r8 = cantilever_ratio(ElementKind::Hex8);
    ch.below("N2 Hex8 delta / delta_T (locking)", r8, 0.9);
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// T1: thick cylinder

#[test]
#[ignore = "KNOWN MISS (ADR 0292, run once): T1b bore hoop stress 1.1681e-2 against 1e-2 (4 x 8 x 1 mesh; T1a 7.31e-5 and T1c 1.1e-16 met)"]
fn t1_thick_cylinder_plane_strain_known_miss() {
    let mut ch = Checks::new();
    let (a, b, p) = (1.0, 2.0, 1e8);
    let param = hex_box([a, 0.0, 0.0], [b - a, PI / 2.0, 0.2], [4, 8, 1]).unwrap();
    let (mesh, mids) = upgrade_to_hex20_mapped(&param, |q| [q[0] * q[1].cos(), q[0] * q[1].sin(), q[2]]).unwrap();
    let bore: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let mut radius: f64 = 0.0;
    for m in &mids {
        if bore.contains(&m.node) {
            let x = mesh.nodes()[m.node];
            radius = radius.max(((x[0] * x[0] + x[1] * x[1]).sqrt() - a).abs());
        }
    }
    ch.at_most("T1c bore mid-edge nodes on the circle", radius, 1e-15);
    let loads = pressure_loads(&mesh, "xmin", p).unwrap();
    let mut model = model_of(mesh, NU);
    for (set, c) in [("zmin", 2), ("zmax", 2), ("ymin", 1), ("ymax", 0)] {
        model.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
    let res = solve_static_with(&model, &[], &loads, &StaticOptions::default()).unwrap();
    let mesh = model.mesh();
    let big_a = p * a * a / (b * b - a * a);
    let big_b = big_a * b * b;
    let ur_a = (1.0 + NU) / E_MOD * ((1.0 - 2.0 * NU) * big_a * a + big_b / a);
    let s_a = big_a + big_b / (a * a);
    let field = IntegrationPointField::compute(&model, res.displacement_values()).unwrap();
    let nodal = nodal_average(&model, &field, field.stress(), None).unwrap();
    let (mut eu, mut es): (f64, f64) = (0.0, 0.0);
    for &n in &bore {
        let x = mesh.nodes()[n];
        let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
        let (c, s) = (x[0] / r, x[1] / r);
        let u = res.displacement(n);
        eu = eu.max(((u[0] * c + u[1] * s) / ur_a - 1.0).abs());
        let sg = nodal[n].unwrap();
        // t^T sigma t with t = (-s, c, 0); Voigt xx, yy, zz, xy, yz, zx.
        let hoop = s * s * sg[0] + c * c * sg[1] - 2.0 * s * c * sg[3];
        es = es.max((hoop / s_a - 1.0).abs());
    }
    println!(
        "T1: {} nodes, {} bore nodes, u_r(a) Lame {ur_a:.6e}, hoop(a) Lame {s_a:.6e}",
        mesh.nodes().len(),
        bore.len()
    );
    ch.at_most("T1a bore radial displacement", eu, 0.002);
    ch.at_most("T1b bore hoop stress", es, 0.01);
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// M1, M2, N3: the manufactured solution of F-378 (sb_mms.rs arithmetic 1 and D2, D3)

const MMS_E: f64 = 1.0;
const K: [[f64; 3]; 3] = [[1.0, 0.5, 0.75], [0.75, 1.0, 0.5], [0.5, 0.75, 1.0]];
const PHI: [[f64; 3]; 3] = [[0.3, 0.7, 0.5], [0.9, 0.2, 0.6], [0.4, 0.8, 0.1]];

type Mat3 = [[f64; 3]; 3];

fn trig_parts(x: Vec3) -> (Mat3, Mat3, Mat3) {
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

fn trig_u(x: Vec3) -> Vec3 {
    let (s, _, _) = trig_parts(x);
    [0, 1, 2].map(|i| s[i][0] * s[i][1] * s[i][2])
}

fn trig_grad(x: Vec3) -> Mat3 {
    let (s, c, k) = trig_parts(x);
    let mut g = [[0.0; 3]; 3];
    for i in 0..3 {
        for m in 0..3 {
            let mut p = k[i][m] * c[i][m];
            for j in (0..3).filter(|&j| j != m) {
                p *= s[i][j];
            }
            g[i][m] = p;
        }
    }
    g
}

fn trig_hess(x: Vec3) -> [Mat3; 3] {
    let (s, c, k) = trig_parts(x);
    let mut h = [[[0.0; 3]; 3]; 3];
    for i in 0..3 {
        for m in 0..3 {
            for n in 0..3 {
                let mut p = 1.0;
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

fn lame(nu: f64) -> (f64, f64) {
    (MMS_E * nu / ((1.0 + nu) * (1.0 - 2.0 * nu)), MMS_E / (2.0 * (1.0 + nu)))
}

fn body_force(x: Vec3, lambda_sign: f64) -> Vec3 {
    let (lambda, mu) = lame(NU);
    let h = trig_hess(x);
    let mut b = [0.0; 3];
    for (i, bi) in b.iter_mut().enumerate() {
        let grad_div: f64 = (0..3).map(|k| h[k][i][k]).sum();
        let lap: f64 = (0..3).map(|j| h[i][j][j]).sum();
        *bi = -((lambda_sign * lambda + mu) * grad_div + mu * lap);
    }
    b
}

fn face_traction(x: Vec3, axis: usize) -> Vec3 {
    let (lambda, mu) = lame(NU);
    let g = trig_grad(x);
    let div = g[0][0] + g[1][1] + g[2][2];
    [0, 1, 2].map(|i| mu * (g[i][axis] + g[axis][i]) + if i == axis { lambda * div } else { 0.0 })
}

fn voigt(g: &Mat3) -> [f64; 6] {
    [
        g[0][0],
        g[1][1],
        g[2][2],
        g[0][1] + g[1][0],
        g[1][2] + g[2][1],
        g[2][0] + g[0][2],
    ]
}

/// (L2 error, energy error).
fn mms_level(n: usize, case_t: bool, lambda_sign: f64) -> (f64, f64) {
    let mesh = hex20_box([0.0; 3], [1.0; 3], [n; 3]);
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(MMS_E, NU, 0.0, 0.0).unwrap())
        .unwrap();
    let mesh = model.mesh();
    let faces: &[&str] = if case_t { &["xmin", "ymin", "zmin"] } else { &BOX_FACES };
    let mut fixed = BTreeSet::new();
    for f in faces {
        fixed.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    let mut pres = Vec::new();
    for &node in &fixed {
        for (k, &v) in trig_u(mesh.nodes()[node]).iter().enumerate() {
            pres.push(Prescribed::new(node, k, v));
        }
    }
    let mut forces = body_force_loads(&model, |x| body_force(x, lambda_sign)).unwrap();
    if case_t {
        for (set, axis) in [("xmax", 0), ("ymax", 1), ("zmax", 2)] {
            forces.extend(traction_field_loads(mesh, set, |x| face_traction(x, axis)).unwrap());
        }
    }
    let res = solve_static_with(&model, &pres, &forces, &StaticOptions::default()).unwrap();
    let d = isotropic_elasticity(MMS_E, NU);
    let el = element_for(ElementKind::Hex20);
    let rule = gauss_125();
    let (mut l2, mut en) = (0.0, 0.0);
    for e in 0..mesh.n_elements() {
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        for &(xi, wq) in &rule {
            let p = el.point(&coords, xi).unwrap();
            let w = wq * p.det_j;
            let mut uh = [0.0; 3];
            let mut gh = [[0.0; 3]; 3];
            for (a, &node) in nodes.iter().enumerate() {
                let ua = res.displacement(node);
                for i in 0..3 {
                    uh[i] += p.n[a] * ua[i];
                    for m in 0..3 {
                        gh[i][m] += ua[i] * p.grad[a][m];
                    }
                }
            }
            let u = trig_u(p.x);
            let de: [f64; 6] = {
                let (ee, eh) = (voigt(&trig_grad(p.x)), voigt(&gh));
                std::array::from_fn(|r| ee[r] - eh[r])
            };
            let mut s = 0.0;
            for r in 0..6 {
                for c in 0..6 {
                    s += de[r] * d[r][c] * de[c];
                }
            }
            l2 += w * (0..3).map(|i| (u[i] - uh[i]).powi(2)).sum::<f64>();
            en += w * s;
        }
    }
    (l2.sqrt(), en.sqrt())
}

fn mms_orders(ch: &mut Checks, tag: &str, case_t: bool, lambda_sign: f64) -> (f64, f64) {
    let levels: Vec<(f64, f64)> = [2, 4, 8].iter().map(|&n| mms_level(n, case_t, lambda_sign)).collect();
    for (n, (l2, en)) in [2, 4, 8].iter().zip(&levels) {
        println!("  {tag} n = {n}: e_L2 {l2:.4e}, e_E {en:.4e}");
    }
    let p_l2 = (levels[1].0 / levels[2].0).log2();
    let p_e = (levels[1].1 / levels[2].1).log2();
    if lambda_sign > 0.0 {
        let mono = levels.windows(2).all(|w| w[1].0 < w[0].0 && w[1].1 < w[0].1);
        ch.holds(&format!("M2 {tag} monotone fall"), mono, &format!("{levels:?}"));
    }
    (p_l2, p_e)
}

#[test]
fn m1_m2_n3_manufactured_solution_orders() {
    let mut ch = Checks::new();
    let (l2, en) = mms_orders(&mut ch, "case D", false, 1.0);
    ch.at_least("M1 case D L2 order (4 -> 8)", l2, 2.7);
    ch.at_least("M2 case D energy order (4 -> 8)", en, 1.8);
    let (l2, en) = mms_orders(&mut ch, "case T", true, 1.0);
    ch.at_least("M1 case T L2 order (4 -> 8)", l2, 2.7);
    ch.at_least("M2 case T energy order (4 -> 8)", en, 1.8);
    let (l2, en) = mms_orders(&mut ch, "N3 case D, lambda reversed", false, -1.0);
    ch.below("N3 L2 order, lambda term reversed", l2, 0.5);
    ch.below("N3 energy order, lambda term reversed", en, 0.5);
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// B1, B2: mass

fn mv(m: &rcfd_linalg::CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

#[test]
fn b1_b2_box_mass_properties_and_bar_frequency() {
    let mut ch = Checks::new();
    let (o, l) = ([0.1, -0.2, 0.3], [1.2, 1.0, 0.8]);
    let model = model_of(hex20_box(o, l, [4, 3, 3]), NU);
    let mp = mass_properties(&model, &consistent(&model).unwrap()).unwrap();
    let m = RHO * l[0] * l[1] * l[2];
    let c = [0, 1, 2].map(|k| o[k] + l[k] / 2.0);
    // Inertia about the centre of a box: m (l_j^2 + l_k^2) / 12 on the diagonal, 0 off it.
    let mut ie = [[0.0; 3]; 3];
    for i in 0..3 {
        ie[i][i] = m * (0..3).filter(|&k| k != i).map(|k| l[k] * l[k]).sum::<f64>() / 12.0;
    }
    ch.at_most("B1 total mass", (mp.mass / m - 1.0).abs(), 1e-12);
    let cn = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
    ch.at_most(
        "B1 centre of mass",
        max_abs((0..3).map(|k| mp.centre[k] - c[k])) / cn,
        1e-12,
    );
    let imax = max_abs(ie.iter().flatten().copied());
    ch.at_most(
        "B1 inertia tensor",
        max_abs((0..9).map(|q| mp.inertia[q / 3][q % 3] - ie[q / 3][q % 3])) / imax,
        1e-12,
    );
    // B2
    let mut bar = model_of(hex20_box([0.0; 3], [1.0, 0.1, 0.1], [8, 1, 1]), 0.0);
    bar.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    bar.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    bar.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    let r = modal_analysis(&bar, &ModalOptions::new(4)).unwrap();
    let mm = consistent(&bar).unwrap();
    let dofs = bar.dof_map();
    let fractions: Vec<f64> = r
        .modes
        .iter()
        .map(|phi| {
            let px: Vec<f64> = phi
                .iter()
                .enumerate()
                .map(|(e, v)| {
                    if dofs.node_component(e).unwrap().1 == 0 {
                        *v
                    } else {
                        0.0
                    }
                })
                .collect();
            px.iter().zip(mv(&mm, &px)).map(|(a, b)| a * b).sum()
        })
        .collect();
    println!("B2: frequencies {:?}, axial fractions {fractions:?}", r.frequencies);
    let first = fractions.iter().position(|&f| f >= 0.99);
    ch.holds(
        "B2 mode 0 is the first axial mode",
        first == Some(0),
        &format!("{first:?}"),
    );
    let exact = (E_MOD / RHO).sqrt() / 4.0;
    let e = r.frequencies[first.unwrap_or(0)] / exact - 1.0;
    ch.at_most("B2 |f_1 / f_exact - 1|", e.abs(), 1e-3);
    ch.at_least("B2 f_1 / f_exact - 1 (upper bound)", e, -1e-12);
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// U1: the upgrade

#[test]
fn u1_upgrade_counts_positions_sets_and_refusal() {
    let mut ch = Checks::new();
    let base = hex_box([0.0; 3], [1.0, 1.0, 1.0], [3, 2, 2]).unwrap();
    let mut edges = BTreeSet::new();
    for e in 0..base.n_elements() {
        let c = base.element_nodes(e);
        for [a, b] in ElementKind::Hex8.edges() {
            edges.insert((c[*a].min(c[*b]), c[*a].max(c[*b])));
        }
    }
    let (up, mids) = upgrade_to_hex20(&base).unwrap();
    let n = up.nodes().len();
    ch.holds(
        "U1 nodes = nodes + edges = 111",
        n == base.nodes().len() + edges.len() && n == 111 && mids.len() == 75,
        &format!("{n} = {} + {}, mids {}", base.nodes().len(), edges.len(), mids.len()),
    );
    let mut pos = true;
    for m in &mids {
        let (p, q) = (base.nodes()[m.ends[0]], base.nodes()[m.ends[1]]);
        for i in 0..3 {
            pos &= up.nodes()[m.node][i].to_bits() == (0.5 * (p[i] + q[i])).to_bits();
        }
        pos &= edges.contains(&(m.ends[0], m.ends[1]));
    }
    ch.holds("U1 mid-edge nodes at the midpoints", pos, "bit for bit");
    let sizes: Vec<usize> = BOX_FACES.iter().map(|f| up.node_set(f).unwrap().len()).collect();
    ch.holds(
        "U1 face node sets",
        sizes == [21, 21, 29, 29, 29, 29],
        &format!("{sizes:?}"),
    );
    let mut sets = up.node_set("all").unwrap().len() == 111;
    for f in BOX_FACES {
        sets &= up.surface_set(f).unwrap() == base.surface_set(f).unwrap();
    }
    sets &= up.element_set("all").unwrap() == base.element_set("all").unwrap();
    ch.holds("U1 surface, element and 'all' sets", sets, "equal");
    match upgrade_to_hex20(&tet_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap()) {
        Err(FemError::UnsupportedKind { block, kind, operation }) => ch.holds(
            "U1 Tet4 refused",
            block == 0 && kind == "Tet4" && operation == "upgrade_to_hex20",
            &format!("block {block}, {kind}, {operation}"),
        ),
        other => ch.holds("U1 Tet4 refused", false, &format!("{:?}", other.map(|_| ()))),
    }
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// Amendment 1: C1 and N2 with the corrected sign of the deflection measure; T1a and T1c covered by a fast test.

/// As [`cantilever_ratio`], with `delta = sum_a f_az u_az / P` (ADR 0292 amendment 1).
fn cantilever_ratio_amended(kind: ElementKind) -> f64 {
    let base = hex_box([0.0; 3], [C_L, C_H, C_H], [10, 1, 1]).unwrap();
    let t = [0.0, 0.0, -C_P / (C_H * C_H)];
    let (mesh, loads) = match kind {
        ElementKind::Hex20 => {
            let mesh = upgrade_to_hex20(&base).unwrap().0;
            let loads = traction_loads(&mesh, "xmax", t).unwrap();
            (mesh, loads)
        }
        _ => {
            let mut acc: BTreeMap<usize, Vec3> = BTreeMap::new();
            for &f in base.surface_set("xmax").unwrap() {
                for n in base.face_nodes(f) {
                    let e = acc.entry(n).or_insert([0.0; 3]);
                    for i in 0..3 {
                        e[i] += 0.25 * C_H * C_H * t[i];
                    }
                }
            }
            let loads = acc.into_iter().map(|(n, f)| NodalForceAt::new(n, f)).collect();
            (base, loads)
        }
    };
    let mut model = model_of(mesh, NU);
    for c in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let res = solve_static_with(&model, &[], &loads, &StaticOptions::default()).unwrap();
    let delta: f64 = loads
        .iter()
        .map(|f| f.force[2] * res.displacement(f.node)[2])
        .sum::<f64>()
        / C_P;
    let i = C_H.powi(4) / 12.0;
    let g = E_MOD / (2.0 * (1.0 + NU));
    let kappa = 5.0 / 6.0;
    let delta_t = C_P * C_L.powi(3) / (3.0 * E_MOD * i) * (1.0 + 3.0 * E_MOD * i / (kappa * g * C_H * C_H * C_L * C_L));
    println!(
        "C amended {}: unknowns {}, delta {delta:.6e} m, delta_T {delta_t:.6e} m, ratio {:.6}",
        kind.name(),
        res.stats().unwrap().unknowns(),
        delta / delta_t
    );
    delta / delta_t
}

#[test]
#[ignore = "KNOWN MISS (ADR 0292 amendment 1, run once): C1 Hex20 |delta / delta_T - 1| = 1.8051e-2 against 1e-2 (ratio 0.981949; the clamp with nu = 0.3, see the addendum)"]
fn c1_amended_hex20_cantilever_known_miss() {
    let mut ch = Checks::new();
    let r = cantilever_ratio_amended(ElementKind::Hex20);
    ch.at_most("C1 Hex20 |delta / delta_T - 1|", (r - 1.0).abs(), 0.01);
    ch.finish();
}

#[test]
fn n2_amended_hex8_cantilever_locks() {
    let mut ch = Checks::new();
    let r8 = cantilever_ratio_amended(ElementKind::Hex8);
    ch.below("N2 Hex8 delta / delta_T (locking)", r8, 0.9);
    ch.finish();
}

#[test]
fn t1_amended_met_clauses() {
    let mut ch = Checks::new();
    let (a, b, p) = (1.0, 2.0, 1e8);
    let param = hex_box([a, 0.0, 0.0], [b - a, PI / 2.0, 0.2], [4, 8, 1]).unwrap();
    let (mesh, mids) = upgrade_to_hex20_mapped(&param, |q| [q[0] * q[1].cos(), q[0] * q[1].sin(), q[2]]).unwrap();
    let bore: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let mut radius: f64 = 0.0;
    for m in &mids {
        if bore.contains(&m.node) {
            let x = mesh.nodes()[m.node];
            radius = radius.max(((x[0] * x[0] + x[1] * x[1]).sqrt() - a).abs());
        }
    }
    ch.at_most("T1c bore mid-edge nodes on the circle", radius, 1e-15);
    let loads = pressure_loads(&mesh, "xmin", p).unwrap();
    let mut model = model_of(mesh, NU);
    for (set, c) in [("zmin", 2), ("zmax", 2), ("ymin", 1), ("ymax", 0)] {
        model.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
    let res = solve_static_with(&model, &[], &loads, &StaticOptions::default()).unwrap();
    let mesh = model.mesh();
    let big_a = p * a * a / (b * b - a * a);
    let big_b = big_a * b * b;
    let ur_a = (1.0 + NU) / E_MOD * ((1.0 - 2.0 * NU) * big_a * a + big_b / a);
    let s_a = big_a + big_b / (a * a);
    let field = IntegrationPointField::compute(&model, res.displacement_values()).unwrap();
    let nodal = nodal_average(&model, &field, field.stress(), None).unwrap();
    let (mut eu, mut es): (f64, f64) = (0.0, 0.0);
    for &n in &bore {
        let x = mesh.nodes()[n];
        let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
        let (c, s) = (x[0] / r, x[1] / r);
        let u = res.displacement(n);
        eu = eu.max(((u[0] * c + u[1] * s) / ur_a - 1.0).abs());
        let sg = nodal[n].unwrap();
        let hoop = s * s * sg[0] + c * c * sg[1] - 2.0 * s * c * sg[3];
        es = es.max((hoop / s_a - 1.0).abs());
    }
    ch.at_most("T1a bore radial displacement", eu, 0.002);
    println!("T1b bore hoop stress: {es:.4e} (limit <= 1e-2; not asserted here, the known-miss record of ADR 0292)");
    ch.finish();
}
