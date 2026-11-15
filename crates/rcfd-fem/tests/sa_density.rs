//! F-568 (ADR 0616): the error indicator per unit volume `eta_e/|K|^{1/2}` of `estimate::EnergyError` and the effectivity
//! criterion near one. Checks V1, V2, K1, P3, NC1, F2 and E2 of the registration (B1 is in `sa_density_bits.rs`); the
//! sequence n = 8, 16, 24 of E1, E1r and NC2 is a heavy record (ignored, run once).
//!
//! The manufactured field, the body force, the error rules, case D, the measurement and the two-layer plate with a hole are
//! copied verbatim from `sa_estimate.rs` (F-432, ADR 0478), which takes them from `sb_mms.rs` (F-378) and `sa_fatigue.rs`
//! (ADR 0449).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::element_for;
use rcfd_fem::estimate::{energy_error, EnergyError, PatchOptions};
use rcfd_fem::hex20::{faces, upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::body_force_loads;
use rcfd_fem::material::{isotropic_elasticity, Matrix6};
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::recover::tensor_norm;
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Load, Material, SolidModel, SolidResult};
use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::time::Instant;

const E_MOD: f64 = 1.0;
const NU: f64 = 0.3;

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

// ---------------------------------------------------------------------------------------------------------------------
// Fields (sb_mms.rs arithmetic 1)

fn lame(nu: f64) -> (f64, f64) {
    (E_MOD * nu / ((1.0 + nu) * (1.0 - 2.0 * nu)), E_MOD / (2.0 * (1.0 + nu)))
}

fn d_matrix() -> Matrix6 {
    isotropic_elasticity(E_MOD, NU)
}

type Mat3 = [[f64; 3]; 3];

trait Field {
    fn u(&self, x: Vec3) -> Vec3;
    fn grad(&self, x: Vec3) -> Mat3;
    fn hess(&self, x: Vec3) -> [Mat3; 3];
}

fn voigt_strain(g: &Mat3) -> [f64; 6] {
    [
        g[0][0],
        g[1][1],
        g[2][2],
        g[0][1] + g[1][0],
        g[1][2] + g[2][1],
        g[2][0] + g[0][2],
    ]
}

fn apply(d: &Matrix6, v: &[f64; 6]) -> [f64; 6] {
    std::array::from_fn(|r| (0..6).map(|c| d[r][c] * v[c]).sum())
}

fn energy(d: &Matrix6, v: &[f64; 6]) -> f64 {
    let mut s = 0.0;
    for r in 0..6 {
        for c in 0..6 {
            s += v[r] * d[r][c] * v[c];
        }
    }
    s
}

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

const K: Mat3 = [[1.0, 0.5, 0.75], [0.75, 1.0, 0.5], [0.5, 0.75, 1.0]];
const PHI: Mat3 = [[0.3, 0.7, 0.5], [0.9, 0.2, 0.6], [0.4, 0.8, 0.1]];

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
        [0, 1, 2].map(|i| s[i][0] * s[i][1] * s[i][2])
    }
    fn grad(&self, x: Vec3) -> Mat3 {
        let (s, c, k) = Trig::parts(x);
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
    fn hess(&self, x: Vec3) -> [Mat3; 3] {
        let (s, c, k) = Trig::parts(x);
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
}

// ---------------------------------------------------------------------------------------------------------------------
// Error rules (sb_mms.rs D3)

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

fn error_rule(hex: bool) -> Vec<([f64; 3], f64)> {
    let (g, gw) = gauss_legendre(5);
    let mut out = Vec::with_capacity(125);
    if hex {
        for k in 0..5 {
            for j in 0..5 {
                for i in 0..5 {
                    out.push(([g[i], g[j], g[k]], gw[i] * gw[j] * gw[k]));
                }
            }
        }
    } else {
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
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Meshes and solves

fn cube(kind: ElementKind, n: usize) -> SolidMesh {
    let (o, l, d) = ([0.0; 3], [1.0; 3], [n; 3]);
    match kind {
        ElementKind::Hex8 => hex_box(o, l, d).unwrap(),
        ElementKind::Tet4 => tet_box(o, l, d).unwrap(),
        ElementKind::Tet10 => upgrade_to_tet10(&tet_box(o, l, d).unwrap()).unwrap().0,
        ElementKind::Hex20 => upgrade_to_hex20(&hex_box(o, l, d).unwrap()).unwrap().0,
        _ => unreachable!(),
    }
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap())
        .unwrap();
    model
}

/// Case D of F-378: every face prescribed with the nodal values of the field, the body force of the field.
fn solve_case_d(
    field: &dyn Field,
    mesh: SolidMesh,
    lambda_sign: f64,
    solver: StaticSolver,
) -> (SolidModel, SolidResult) {
    let model = model_of(mesh);
    let mesh = model.mesh();
    let mut fixed = BTreeSet::new();
    for name in BOX_FACES {
        fixed.extend(mesh.node_set(name).unwrap().iter().copied());
    }
    let mut prescribed = Vec::new();
    for node in fixed {
        for (k, &v) in field.u(mesh.nodes()[node]).iter().enumerate() {
            prescribed.push(Prescribed::new(node, k, v));
        }
    }
    let forces = body_force_loads(&model, |x| body_force(field, x, lambda_sign)).unwrap();
    let result = solve_static_with(&model, &prescribed, &forces, &StaticOptions::with_solver(solver)).unwrap();
    (model, result)
}

struct Measured {
    n: usize,
    theta: f64,
    eta: f64,
    true_energy: f64,
    raw_l2: f64,
    rec_l2: f64,
}

/// True energy error, raw and recovered stress L2 errors with the 125-point rules, and the estimate.
fn measure(field: &dyn Field, model: &SolidModel, result: &SolidResult, n: usize, threads: Option<usize>) -> Measured {
    let mesh = model.mesh();
    let d = d_matrix();
    let (rec, est) = energy_error(
        model,
        result.displacement_values(),
        &PatchOptions::new().threads(threads),
    )
    .unwrap();
    let tet_rule = error_rule(false);
    let hex_rule = error_rule(true);
    let (mut en, mut raw, mut recl2) = (0.0, 0.0, 0.0);
    for e in 0..mesh.n_elements() {
        let kind = mesh.element_kind(e);
        let element = element_for(kind);
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let rule = if matches!(kind, ElementKind::Hex8 | ElementKind::Hex20) {
            &hex_rule
        } else {
            &tet_rule
        };
        for &(xi, wq) in rule {
            let p = element.point(&coords, xi).unwrap();
            let w = wq * p.det_j;
            let mut gh = [[0.0; 3]; 3];
            for (a, &node) in nodes.iter().enumerate() {
                let ua = result.displacement(node);
                for i in 0..3 {
                    for m in 0..3 {
                        gh[i][m] += ua[i] * p.grad[a][m];
                    }
                }
            }
            let eps = voigt_strain(&field.grad(p.x));
            let eps_h = voigt_strain(&gh);
            let de: [f64; 6] = std::array::from_fn(|r| eps[r] - eps_h[r]);
            let s = apply(&d, &eps);
            let sh = apply(&d, &eps_h);
            let ss = rec.stress_at(model, e, xi).unwrap();
            let dr: [f64; 6] = std::array::from_fn(|r| s[r] - sh[r]);
            let ds: [f64; 6] = std::array::from_fn(|r| s[r] - ss[r]);
            en += w * energy(&d, &de);
            raw += w * tensor_norm(&dr).powi(2);
            recl2 += w * tensor_norm(&ds).powi(2);
        }
    }
    let true_energy = en.sqrt();
    let m = Measured {
        n,
        theta: est.estimate() / true_energy,
        eta: est.estimate(),
        true_energy,
        raw_l2: raw.sqrt(),
        rec_l2: recl2.sqrt(),
    };
    println!(
        "  {} n = {n:2}: eta {:.5e}, true {:.5e}, theta {:.5}, raw L2 {:.5e}, recovered L2 {:.5e}, relative estimate {:.4e}, \
         {} patches, {} short",
        mesh.element_kind(0).name(),
        m.eta,
        m.true_energy,
        m.theta,
        m.raw_l2,
        m.rec_l2,
        est.relative(),
        rec.patches().len(),
        rec.short_patches().len()
    );
    m
}

fn run(kind: ElementKind, ns: &[usize], lambda_sign: f64, solver: StaticSolver) -> Vec<Measured> {
    ns.iter()
        .map(|&n| {
            let t0 = Instant::now();
            let (model, result) = solve_case_d(&Trig, cube(kind, n), lambda_sign, solver.clone());
            let m = measure(&Trig, &model, &result, n, None);
            println!("    ({:.2} s)", t0.elapsed().as_secs_f64());
            m
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// The plate with a hole (ADR 0449) with two layers

const HR: f64 = 0.005;
const HA: f64 = 0.1;
const HB: f64 = 0.05;
const HT: f64 = 0.001;
const S0: f64 = 50e6;

fn plate_map(p: Vec3) -> Vec3 {
    let (s, q) = (p[0], p[1]);
    let (circle, outer) = if q == 1.0 {
        ([0.0, HR], [0.0, HB])
    } else {
        let th = 0.5 * PI * q;
        let ell = q * (HA + HB);
        let o = if ell <= HB { [HA, ell] } else { [HA - (ell - HB), HB] };
        ([HR * th.cos(), HR * th.sin()], o)
    };
    let g = ((4.0 * s).exp() - 1.0) / (4f64.exp() - 1.0);
    [
        circle[0] + g * (outer[0] - circle[0]),
        circle[1] + g * (outer[1] - circle[1]),
        p[2],
    ]
}

fn plate(layers: usize) -> SolidModel {
    let (mut mesh, _) =
        upgrade_to_hex20_mapped(&hex_box([0.0; 3], [1.0, 1.0, HT], [24, 24, layers]).unwrap(), plate_map).unwrap();
    let right: Vec<_> = mesh
        .surface_set("xmax")
        .unwrap()
        .iter()
        .copied()
        .filter(|f| {
            let nodes = mesh.element_nodes(f.element);
            faces()[f.face]
                .iter()
                .all(|&a| (mesh.nodes()[nodes[a]][0] - HA).abs() <= 1e-12)
        })
        .collect();
    mesh.add_surface_set("right", right).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_load(Load::traction("right", [S0, 0.0, 0.0])).unwrap();
    m
}

fn solved_plate() -> (SolidModel, Vec<f64>) {
    let model = plate(2);
    let r = solve_static(&model, &StaticOptions::with_solver(StaticSolver::SparseDirect)).unwrap();
    let u = r.displacement_values().to_vec();
    (model, u)
}

fn on_hole(x: Vec3) -> bool {
    (x[0].hypot(x[1]) - HR).abs() <= 1e-9
}

// ---------------------------------------------------------------------------------------------------------------------
// The criterion near one (D2)

/// `|theta_next - 1| <= max(|theta_prev - 1|, 1e-2)`.
fn criterion(prev: f64, next: f64) -> bool {
    (next - 1.0).abs() <= (prev - 1.0).abs().max(1e-2)
}

fn check_criterion(c: &mut Checks, tag: &str, kind: ElementKind, levels: &[Measured]) {
    for w in levels.windows(2) {
        let (a, b) = ((w[0].theta - 1.0).abs(), (w[1].theta - 1.0).abs());
        c.holds(
            &format!(
                "{tag} {} |theta - 1| at n = {} <= max(|theta - 1| at n = {}, 1e-2)",
                kind.name(),
                w[1].n,
                w[0].n
            ),
            criterion(w[0].theta, w[1].theta),
            &format!("{a:.5} -> {b:.5}"),
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// V1, V2, K1

/// The plate and case D on Hex8, Tet10 and Hex20 at n = 4 (the models of B1), with their estimates.
fn four_models() -> Vec<(&'static str, SolidModel, EnergyError)> {
    let mut out = Vec::new();
    let (p, up) = solved_plate();
    let (_, est) = energy_error(&p, &up, &PatchOptions::new()).unwrap();
    out.push(("plate", p, est));
    for (name, kind) in [
        ("Hex8 n = 4", ElementKind::Hex8),
        ("Tet10 n = 4", ElementKind::Tet10),
        ("Hex20 n = 4", ElementKind::Hex20),
    ] {
        let (model, result) = solve_case_d(&Trig, cube(kind, 4), 1.0, StaticSolver::Direct);
        let (_, est) = energy_error(&model, result.displacement_values(), &PatchOptions::new()).unwrap();
        out.push((name, model, est));
    }
    out
}

#[test]
fn v1_v2_k1_volumes_densities_ranking() {
    let mut c = Checks::new();
    for (name, model, est) in four_models() {
        let n = model.mesh().n_elements();
        let (vol, den, eta) = (est.element_volumes(), est.element_densities(), est.element_errors());
        // V1
        if name == "plate" {
            let exact = HT * (HA * HB - PI * HR * HR / 4.0);
            let sum: f64 = vol.iter().sum();
            println!("V1 plate: sum |K| {sum:.15e}, exact {exact:.15e}");
            c.at_most(
                "V1 plate sum |K| against t (A B - pi r^2/4), relative",
                (sum / exact - 1.0).abs(),
                1e-6,
            );
        } else {
            let exact = if name.starts_with("Tet10") {
                1.0 / 384.0
            } else {
                1.0 / 64.0
            };
            let dev = vol.iter().map(|v| (v / exact - 1.0).abs()).fold(0.0, f64::max);
            c.at_most(&format!("V1 {name} largest |K| deviation, relative"), dev, 1e-13);
        }
        // V2
        let lengths = vol.len() == n && den.len() == n && eta.len() == n;
        let wrong = (0..n.min(den.len()).min(vol.len()).min(eta.len()))
            .filter(|&e| den[e].to_bits() != (eta[e] / vol[e].sqrt()).to_bits())
            .count();
        c.holds(
            &format!("V2 {name} density = eta_e / |K|^(1/2) to the bit"),
            lengths && wrong == 0,
            &format!(
                "lengths {} {} {} of {n}, {wrong} differ",
                vol.len(),
                den.len(),
                eta.len()
            ),
        );
        // K1
        let ranked = est.ranked_by_density();
        let mut seen = vec![false; n];
        for &e in &ranked {
            if e < n {
                seen[e] = true;
            }
        }
        let permutation = ranked.len() == n && seen.iter().all(|&s| s);
        let ordered = permutation
            && ranked
                .windows(2)
                .all(|w| den[w[0]] > den[w[1]] || (den[w[0]] == den[w[1]] && w[0] < w[1]));
        let ties = if permutation {
            ranked.windows(2).filter(|w| den[w[0]] == den[w[1]]).count()
        } else {
            0
        };
        c.holds(
            &format!("K1 {name} ranked_by_density a permutation, descending, ties ascending"),
            permutation && ordered,
            &format!("permutation {permutation}, ordered {ordered}, {ties} equal neighbours"),
        );
    }
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// P3, NC1

#[test]
fn p3_nc1_plate_density_ranking() {
    let mut c = Checks::new();
    let (model, u) = solved_plate();
    let mesh = model.mesh();
    let (_, est) = energy_error(&model, &u, &PatchOptions::new()).unwrap();
    let edge: Vec<usize> = (0..mesh.n_elements())
        .filter(|&e| {
            let nodes = mesh.element_nodes(e);
            mesh.element_kind(e)
                .faces()
                .iter()
                .any(|f| f.iter().all(|&a| on_hole(mesh.nodes()[nodes[a]])))
        })
        .collect();
    let tenth = mesh.n_elements().div_ceil(10);
    let outside = |ranked: &[usize]| -> (usize, Vec<usize>) {
        let top: BTreeSet<usize> = ranked[..tenth].iter().copied().collect();
        let mut ranks: Vec<usize> = edge
            .iter()
            .map(|&e| ranked.iter().position(|&r| r == e).unwrap())
            .collect();
        ranks.sort_unstable();
        (edge.iter().filter(|e| !top.contains(e)).count(), ranks)
    };
    let by_density = est.ranked_by_density();
    let (missing, ranks) = outside(&by_density);
    let den = est.element_densities();
    let edge_min = edge.iter().map(|&e| den[e]).fold(f64::INFINITY, f64::min);
    println!(
        "P3 record: {} elements, tenth {tenth}, {} hole-edge elements; their density ranks {ranks:?}",
        mesh.n_elements(),
        edge.len()
    );
    let centroid = |e: usize| -> Vec3 {
        let nodes = mesh.element_nodes(e);
        let mut x = [0.0; 3];
        for &a in nodes {
            for (k, xk) in x.iter_mut().enumerate() {
                *xk += mesh.nodes()[a][k] / nodes.len() as f64;
            }
        }
        x
    };
    let top = by_density[0];
    println!(
        "P3 record: largest density {:.4e} (element {top}, centroid {:?}), density of rank {tenth} {:.4e}, smallest at the \
         hole edge {edge_min:.4e}",
        den[top],
        centroid(top),
        den[by_density[tenth - 1]]
    );
    c.holds(
        "P3 the tenth of largest density contains every element with a face on the hole",
        edge.len() == 48 && missing == 0,
        &format!("{missing} of {} hole-edge elements outside the tenth", edge.len()),
    );
    let (missing_abs, ranks_abs) = outside(&est.ranked());
    println!("NC1 record: absolute ranks of the hole-edge elements {ranks_abs:?}");
    c.holds(
        "NC1 ranked by eta_e alone: 30 hole-edge elements outside the tenth (F-432)",
        missing_abs == 30,
        &format!("{missing_abs} of {} outside", edge.len()),
    );
    c.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// E2, F2 (fast), E1, E1r, NC2 (heavy)

/// The F-432 log (`f432-scratch/run_heavy.log`, `SparseDirect`): theta of Hex20 and Tet10 at n = 4, 8, 16.
const F432_HEX20: [(usize, f64); 3] = [(4, 1.33686), (8, 1.02782), (16, 0.99392)];
const F432_TET10: [(usize, f64); 3] = [(4, 1.09089), (8, 1.00514), (16, 0.99168)];

fn against_log(c: &mut Checks, tag: &str, kind: ElementKind, levels: &[Measured], limit: f64) {
    let log = if kind == ElementKind::Hex20 {
        &F432_HEX20
    } else {
        &F432_TET10
    };
    for m in levels {
        if let Some(&(_, t)) = log.iter().find(|&&(n, _)| n == m.n) {
            c.at_most(
                &format!(
                    "{tag} {} n = {} theta {:.6} against the F-432 log {t}",
                    kind.name(),
                    m.n,
                    m.theta
                ),
                (m.theta - t).abs(),
                limit,
            );
        }
    }
}

#[test]
fn e2_f2_tet10_hex20_fast() {
    let mut c = Checks::new();
    for kind in [ElementKind::Hex20, ElementKind::Tet10] {
        let levels = run(kind, &[4, 8], 1.0, StaticSolver::SparseDirect);
        against_log(&mut c, "E2", kind, &levels, 5e-6);
        check_criterion(&mut c, "F2", kind, &levels);
    }
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0616 E1 (Hex20 and Tet10 case D at n = 8, 16, 24 with SupernodalDirect; 311 469 unknowns for Tet10 at 24), run once, alone, with at least 10 GB free"]
fn e1_criterion_near_one_heavy() {
    let mut c = Checks::new();
    for kind in [ElementKind::Hex20, ElementKind::Tet10] {
        let levels = run(kind, &[8, 16, 24], 1.0, StaticSolver::SupernodalDirect);
        check_criterion(&mut c, "E1", kind, &levels);
        against_log(&mut c, "E1r", kind, &levels, 1e-5);
        if kind == ElementKind::Tet10 {
            let (a, b) = ((levels[0].theta - 1.0).abs(), (levels[1].theta - 1.0).abs());
            println!(
                "NC2 record: literal reading of F-432 on Tet10 8 -> 16: {a:.5} -> {b:.5}, holds {}",
                b <= a
            );
            c.holds(
                "NC2 the literal reading (no floor) fails on Tet10 8 -> 16, as registered",
                b > a,
                &format!("{a:.5} -> {b:.5}"),
            );
        }
    }
    c.finish();
}
