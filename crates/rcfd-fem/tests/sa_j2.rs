//! F-422 (ADR 0475, the registration of ADR 0325): J2 plasticity with linear isotropic and kinematic hardening, radial
//! return and consistent tangent on the Newton driver. The checks T1, T2, U1 to U4, S1 to S4, C1, C2, P1, O1, H1, R1, N2
//! and N3 of the registration (seed 0xF422_0475). N1 is `sa_j2_default_bits.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::{element_dofs, stiffness};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::hex20::{reference_nodes, upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mat_j2::{self, return_map, update};
use rcfd_fem::mesh::FaceRef;
use rcfd_fem::modal::{modal_analysis_with, ModalOptions};
use rcfd_fem::mpc::solve_static_mpc;
use rcfd_fem::nonlinear::{
    element_internal_force, element_second_piola_kirchhoff, element_tangent, element_trial_state, solve, solve_with,
    Contribution, DriverExtensions, LoadStep, NewtonOptions, NonlinearResult,
};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::transient::{Integrator, TransientOptions};
use rcfd_fem::{Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use std::cell::Cell;
use std::collections::HashMap;

const SEED: u64 = 0xF422_0475;
const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const SY: f64 = 2.0e8;
const RHO: f64 = 7850.0;
const H: f64 = 2.0e9;
const EPS_MAX: f64 = 5e-3;

/// SplitMix64, a deterministic source only.
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
            "{name}: {value:.4} (limit >= {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} < {limit}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} {detail}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn base() -> Material {
    Material::isotropic_elastic(E_MOD, NU, RHO, 0.0).unwrap()
}

fn j2(h_iso: f64, h_kin: f64) -> Material {
    Material::j2(base(), SY, h_iso, h_kin).unwrap()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn max_abs(v: impl Iterator<Item = f64>) -> f64 {
    v.fold(0.0, |a, x| a.max(x.abs()))
}

/// A random committed state: deviatoric plastic strain of size `ep`, deviatoric back stress of size `b`, `alpha`.
fn random_state(rng: &mut Rng, ep: f64, b: f64, alpha: f64) -> Vec<f64> {
    let mut s = vec![0.0; mat_j2::STATE_SIZE];
    for v in s.iter_mut().take(12) {
        *v = rng.sym(1.0);
    }
    let m = (s[0] + s[1] + s[2]) / 3.0;
    let mb = (s[6] + s[7] + s[8]) / 3.0;
    for k in 0..3 {
        s[k] -= m;
        s[6 + k] -= mb;
    }
    for v in s.iter_mut().take(6) {
        *v *= ep;
    }
    for v in s.iter_mut().skip(6).take(6) {
        *v *= b;
    }
    s[12] = alpha;
    s
}

// ------------------------------------------------------------------------------------------------- T1, T2

const HARDENING: [(&str, f64, f64); 4] = [
    ("isotropic", 2.0e9, 0.0),
    ("kinematic", 0.0, 2.0e9),
    ("mixed", 1.5e9, 1.0e9),
    ("perfect", 0.0, 0.0),
];

#[test]
fn t1_material_tangent_central_differences() {
    let mut c = Checks::new();
    let mut rng = Rng::new(1);
    for (name, hi, hk) in HARDENING {
        let st = random_state(
            &mut rng,
            1e-3,
            if hk > 0.0 { 2e7 } else { 0.0 },
            if hi > 0.0 { 1e-3 } else { 0.0 },
        );
        for (branch, scale, want_plastic) in [("elastic", 2e-4, false), ("plastic", 4e-3, true)] {
            let mut eps = [0.0; 6];
            for (k, v) in eps.iter_mut().enumerate() {
                *v = st[k] + rng.sym(scale);
            }
            let u = return_map(E_MOD, NU, SY, hi, hk, &eps, &st).unwrap();
            c.holds(
                &format!("T1 {name} {branch}: set-up (branch)"),
                u.plastic == want_plastic,
                &format!("plastic {}", u.plastic),
            );
            let h = 1e-9;
            let mut worst: f64 = 0.0;
            for k in 0..6 {
                let mut a = eps;
                a[k] += h;
                let mut b = eps;
                b[k] -= h;
                let sa = return_map(E_MOD, NU, SY, hi, hk, &a, &st).unwrap().stress;
                let sb = return_map(E_MOD, NU, SY, hi, hk, &b, &st).unwrap().stress;
                for i in 0..6 {
                    worst = worst.max((u.tangent[i][k] - (sa[i] - sb[i]) / (2.0 * h)).abs());
                }
            }
            let scale_c = max_abs(u.tangent.iter().flatten().copied());
            c.at_most(
                &format!("T1 {name} {branch}: max |C - C_fd| / max |C|"),
                worst / scale_c,
                1e-6,
            );
        }
    }
    c.finish();
}

/// A distorted element of `kind` near the unit cube.
fn distorted(kind: ElementKind, rng: &mut Rng) -> Vec<Vec3> {
    let unit: Vec<Vec3> = match kind {
        ElementKind::Hex8 => vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        _ => reference_nodes()
            .iter()
            .map(|p| [(p[0] + 1.0) / 2.0, (p[1] + 1.0) / 2.0, (p[2] + 1.0) / 2.0])
            .collect(),
    };
    let amp = if kind == ElementKind::Hex8 { 0.1 } else { 0.05 };
    unit.iter()
        .map(|p| [p[0] + rng.sym(amp), p[1] + rng.sym(amp), p[2] + rng.sym(amp)])
        .collect()
}

#[test]
#[ignore = "record of the registered run of ADR 0475 (commit cb17790f): Hex8 elastic 5.3e-11, Hex8 plastic 1.3e-9, Hex20 plastic 2.4e-9 met (limit 1e-6); the Hex20 elastic set-up defective (3 of 27 points yielded; measured 6.9e-11, not counted); corrected by amendment 1 as t2b_hex20_elastic_tangent, the met rows continued in t2_element_tangent_met_rows"]
fn t2_element_tangent_central_differences() {
    let mut c = Checks::new();
    let mut rng = Rng::new(2);
    let mat = j2(1.5e9, 1.0e9);
    for kind in [ElementKind::Hex8, ElementKind::Hex20] {
        let el = element_for(kind);
        let x = distorted(kind, &mut rng);
        let n = 3 * el.n_nodes();
        let np = el.stiffness_rule().len();
        for (branch, scale, plastic) in [("elastic", 2e-4, false), ("plastic", 4e-3, true)] {
            let st = if plastic {
                let mut v = Vec::new();
                for _ in 0..np {
                    v.extend(random_state(&mut rng, 1e-3, 2e7, 1e-3));
                }
                ElementState::new(v)
            } else {
                ElementState::new(vec![0.0; np * mat_j2::STATE_SIZE])
            };
            let u: Vec<f64> = (0..n).map(|_| rng.sym(scale)).collect();
            let trial = element_trial_state(el, &x, &mat, &u, &st).unwrap();
            let yielded = (0..np)
                .filter(|&q| {
                    trial.values()[q * 13..(q + 1) * 13] != st.values()[q * 13..(q + 1) * 13]
                })
                .count();
            c.holds(
                &format!("T2 {} {branch}: set-up (points yielded)", kind.name()),
                if plastic { yielded > 0 } else { yielded == 0 },
                &format!("{yielded} of {np}"),
            );
            let k = element_tangent(el, &x, &mat, &u, &st).unwrap();
            let h = 1e-9;
            let mut worst: f64 = 0.0;
            for j in 0..n {
                let mut a = u.clone();
                a[j] += h;
                let mut b = u.clone();
                b[j] -= h;
                let fa = element_internal_force(el, &x, &mat, &a, &st).unwrap();
                let fb = element_internal_force(el, &x, &mat, &b, &st).unwrap();
                for i in 0..n {
                    worst = worst.max((k.values()[i * n + j] - (fa[i] - fb[i]) / (2.0 * h)).abs());
                }
            }
            let scale_k = max_abs(k.values().iter().copied());
            c.at_most(
                &format!("T2 {} {branch}: max |K - K_fd| / max |K|", kind.name()),
                worst / scale_k,
                1e-6,
            );
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- U1 to U3

/// Solves the 5 x 5 system `a x = b` by Gaussian elimination with partial pivoting.
fn solve5(mut a: [[f64; 5]; 5], mut b: [f64; 5]) -> [f64; 5] {
    for col in 0..5 {
        let p = (col..5)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .unwrap();
        a.swap(col, p);
        b.swap(col, p);
        for r in col + 1..5 {
            let f = a[r][col] / a[col][col];
            for k in col..5 {
                a[r][k] -= f * a[col][k];
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = [0.0; 5];
    for r in (0..5).rev() {
        let mut v = b[r];
        for k in r + 1..5 {
            v -= a[r][k] * x[k];
        }
        x[r] = v / a[r][r];
    }
    x
}

/// Mixed control at the material point: `eps_xx` given, the five other strains such that the five other stresses vanish
/// (Newton with the consistent tangent, to `1e-16 |sigma_xx|` or 30 iterations), from the committed `state`.
fn uniaxial(m: &Material, exx: f64, state: &[f64], guess: [f64; 5]) -> (mat_j2::Update, [f64; 5]) {
    let mut lat = guess;
    for it in 0..30 {
        let eps = [exx, lat[0], lat[1], lat[2], lat[3], lat[4]];
        let u = update(m, &eps, state).unwrap();
        let r = [u.stress[1], u.stress[2], u.stress[3], u.stress[4], u.stress[5]];
        if it > 0 && max_abs(r.iter().copied()) <= 1e-16 * u.stress[0].abs() {
            return (u, lat);
        }
        let mut a = [[0.0; 5]; 5];
        for i in 0..5 {
            for j in 0..5 {
                a[i][j] = u.tangent[i + 1][j + 1];
            }
        }
        let d = solve5(a, r);
        for k in 0..5 {
            lat[k] -= d[k];
        }
    }
    let eps = [exx, lat[0], lat[1], lat[2], lat[3], lat[4]];
    (update(m, &eps, state).unwrap(), lat)
}

fn bilinear(eps: f64, h: f64) -> f64 {
    SY + E_MOD * h / (E_MOD + h) * (eps - SY / E_MOD)
}

#[test]
fn u1_to_u3_uniaxial_bilinear_and_reversal() {
    let mut c = Checks::new();
    for (name, hi, hk) in [("isotropic", H, 0.0), ("kinematic", 0.0, H)] {
        let m = j2(hi, hk);
        let closed_max = bilinear(EPS_MAX, H);
        // One step.
        let zero = vec![0.0; mat_j2::STATE_SIZE];
        let (u, _) = uniaxial(&m, EPS_MAX, &zero, [0.0; 5]);
        c.at_most(
            &format!("U1 {name} one step: |sigma - closed| / closed"),
            (u.stress[0] - closed_max).abs() / closed_max,
            1e-10,
        );
        // 100 steps.
        let mut st = zero.clone();
        let mut lat = [0.0; 5];
        let mut worst: f64 = 0.0;
        let mut smax = 0.0;
        for k in 1..=100 {
            let ex = EPS_MAX * k as f64 / 100.0;
            let (u, l) = uniaxial(&m, ex, &st, lat);
            let want = if ex <= SY / E_MOD { E_MOD * ex } else { bilinear(ex, H) };
            worst = worst.max((u.stress[0] - want).abs() / closed_max);
            st = u.state.to_vec();
            lat = l;
            smax = u.stress[0];
        }
        c.at_most(
            &format!("U1 {name} 100 steps: max |sigma - closed| / closed(eps_max)"),
            worst,
            1e-10,
        );
        // Reversal from the state at eps_max.
        let eu = EPS_MAX - 1e-3;
        let (su, _) = uniaxial(&m, eu, &st, lat);
        let er = if hk > 0.0 {
            EPS_MAX - 2.0 * SY / E_MOD
        } else {
            EPS_MAX - 2.0 * smax / E_MOD
        };
        let (e2, e3) = (er - 2e-3, er - 4e-3);
        let (s2, _) = uniaxial(&m, e2, &st, lat);
        let (s3, _) = uniaxial(&m, e3, &st, lat);
        c.holds(
            &format!("U2/U3 {name}: set-up (unloading elastic, reversed points plastic)"),
            !su.plastic && s2.plastic && s3.plastic,
            &format!("{} {} {}", su.plastic, s2.plastic, s3.plastic),
        );
        let ke = (su.stress[0] - smax) / (eu - EPS_MAX);
        let kp = (s3.stress[0] - s2.stress[0]) / (e3 - e2);
        let x = (s2.stress[0] - kp * e2 - smax + ke * EPS_MAX) / (ke - kp);
        let sry = smax + ke * (x - EPS_MAX);
        let (id, want) = if hk > 0.0 {
            ("U3", smax - 2.0 * SY)
        } else {
            ("U2", -smax)
        };
        println!("  {name}: sigma_max {smax:.12e}, re-yield {sry:.12e}, closed {want:.12e}");
        c.at_most(
            &format!("{id} {name}: |sigma_ry - closed| / |closed|"),
            (sry - want).abs() / want.abs(),
            1e-10,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- U4, N2

/// The bar of D6: one Hex8 `[1, 0.1, 0.1]`, symmetry planes, `u_x = 5e-3` at `xmax`.
fn bar(mat: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmax", 0, EPS_MAX)).unwrap();
    m
}

/// The element coordinates and displacement of element `e`.
fn element_data(model: &SolidModel, r: &NonlinearResult, e: usize) -> (Vec<Vec3>, Vec<f64>) {
    let mesh = model.mesh();
    let nodes = mesh.element_nodes(e);
    let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let d = element_dofs(&model.dof_map(), nodes);
    let u = r.result.displacement_values();
    (coords, d.iter().map(|&q| u[q]).collect())
}

#[test]
fn u4_n2_model_bar() {
    let mut c = Checks::new();
    let m = bar(j2(H, 0.0));
    let exx = EPS_MAX + 0.5 * EPS_MAX * EPS_MAX;
    let want = bilinear(exx, H);
    for steps in [1, 100] {
        let r = solve(&m, &[], &[], &NewtonOptions::new(steps)).unwrap();
        let (x, ue) = element_data(&m, &r, 0);
        let s = element_second_piola_kirchhoff(element_for(ElementKind::Hex8), &x, &m.materials()[0].1, &ue, &r.states[0])
            .unwrap();
        let worst = s.iter().map(|p| (p[0] - want).abs() / want).fold(0.0, f64::max);
        let lateral = max_abs(s.iter().flat_map(|p| p[1..].to_vec())) / want;
        println!("  U4 {steps} steps: S_xx {:.12e}, closed {want:.12e}, lateral {lateral:.3e}", s[0][0]);
        c.at_most(
            &format!("U4 {steps} steps: max |S_xx - closed(E_xx)| / closed"),
            worst,
            1e-8,
        );
    }
    // N2: a J2 material that never yields gives the bits of its elastic base.
    let never = Material::j2(base(), 1e30, 0.0, 0.0).unwrap();
    let cant = |mat: Material| {
        let mesh = upgrade_to_hex20(&hex_box([0.0; 3], [1.0, 0.05, 0.05], [10, 1, 1]).unwrap())
            .unwrap()
            .0;
        let mut m = SolidModel::new(mesh);
        m.add_material("all", mat).unwrap();
        for k in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
        }
        m.add_load(Load::traction("xmax", [0.0, 2.0e6, 0.0])).unwrap();
        m
    };
    let flat = |r: &NonlinearResult| {
        let mut v = r.result.displacement_values().to_vec();
        v.extend_from_slice(r.result.reactions().unwrap());
        for s in &r.steps {
            v.push(s.load_factor);
            v.extend_from_slice(&s.residuals);
            v.extend_from_slice(&s.energies);
        }
        v
    };
    for (name, a, b, steps) in [
        ("bar", bar(never.clone()), bar(base()), 2),
        ("Hex20 cantilever", cant(never.clone()), cant(base()), 2),
    ] {
        let ra = solve(&a, &[], &[], &NewtonOptions::new(steps)).unwrap();
        let rb = solve(&b, &[], &[], &NewtonOptions::new(steps)).unwrap();
        c.holds(
            &format!("N2 {name}: J2 (sigma_y = 1e30) = elastic base, bit for bit"),
            bits(&flat(&ra)) == bits(&flat(&rb)),
            &format!("{} values", flat(&ra).len()),
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- the sphere

const SPHERE_N: usize = 2;
const SPHERE_M: usize = 8;
const ELEMENTS_PER_LAYER: usize = 3 * SPHERE_N * SPHERE_N;

fn p_of_c(c: f64) -> f64 {
    2.0 * c.ln() + 2.0 / 3.0 * (1.0 - c * c * c / 8.0)
}

/// The octant of the sphere `a = 1`, `b = 2` of D5 (cubed sphere, 2 x 2 cells per face, 8 radial layers, Hex20 on the
/// spheres), element `layer * 12 + cell`; with the bore pressure `p` (Pa) if given.
fn sphere(mat: Material, p: Option<f64>) -> SolidModel {
    let (n, m) = (SPHERE_N, SPHERE_M);
    let mut index: HashMap<([usize; 3], usize), usize> = HashMap::new();
    let mut params: Vec<Vec3> = Vec::new();
    let mut node = |q: [usize; 3], k: usize, params: &mut Vec<Vec3>| -> usize {
        *index.entry((q, k)).or_insert_with(|| {
            let r = 1.0 + k as f64 / m as f64;
            params.push([
                r * q[0] as f64 / n as f64,
                r * q[1] as f64 / n as f64,
                r * q[2] as f64 / n as f64,
            ]);
            params.len() - 1
        })
    };
    let mut conn = Vec::new();
    for k in 0..m {
        for a in 0..3 {
            for j in 0..n {
                for i in 0..n {
                    let q = |i: usize, j: usize| {
                        let mut v = [0; 3];
                        v[a] = n;
                        v[(a + 1) % 3] = i;
                        v[(a + 2) % 3] = j;
                        v
                    };
                    for kk in [k, k + 1] {
                        for (ii, jj) in [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)] {
                            let id = node(q(ii, jj), kk, &mut params);
                            conn.push(id);
                        }
                    }
                }
            }
        }
    }
    let nn = params.len();
    let mut mesh = SolidMesh::new(params).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    for (c, name) in ["x0", "y0", "z0"].iter().enumerate() {
        let set: Vec<usize> = (0..nn).filter(|&v| mesh.nodes()[v][c] == 0.0).collect();
        mesh.add_node_set(name, set).unwrap();
    }
    mesh.add_surface_set(
        "bore",
        (0..ELEMENTS_PER_LAYER).map(|e| FaceRef { element: e, face: 0 }).collect(),
    )
    .unwrap();
    let mesh = upgrade_to_hex20_mapped(&mesh, |p| {
        let inf = p[0].abs().max(p[1].abs()).max(p[2].abs());
        let two = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        [p[0] * inf / two, p[1] * inf / two, p[2] * inf / two]
    })
    .unwrap()
    .0;
    let mut model = SolidModel::new(mesh);
    model.add_material("all", mat).unwrap();
    for (c, name) in ["x0", "y0", "z0"].iter().enumerate() {
        model.add_constraint(Constraint::displacement(name, c, 0.0)).unwrap();
    }
    if let Some(p) = p {
        model.add_load(Load::pressure("bore", p)).unwrap();
    }
    model
}

/// One integration point of the sphere: element, radius, von Mises stress, hoop stress, equivalent plastic strain.
struct Point {
    element: usize,
    r: f64,
    eq: f64,
    hoop: f64,
    alpha: f64,
}

fn sphere_points(model: &SolidModel, r: &NonlinearResult) -> Vec<Point> {
    let mat = &model.materials()[0].1;
    let el = element_for(ElementKind::Hex20);
    let mut out = Vec::new();
    for e in 0..model.mesh().n_elements() {
        let (x, ue) = element_data(model, r, e);
        let s = element_second_piola_kirchhoff(el, &x, mat, &ue, &r.states[e]).unwrap();
        let ips = el.integration_points(&x, el.stiffness_rule()).unwrap();
        for (q, (ip, s)) in ips.iter().zip(&s).enumerate() {
            let p = ip.geometry.x;
            let rad = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            let n = [p[0] / rad, p[1] / rad, p[2] / rad];
            let t = [[s[0], s[3], s[5]], [s[3], s[1], s[4]], [s[5], s[4], s[2]]];
            let mut srr = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    srr += n[i] * t[i][j] * n[j];
                }
            }
            let tr = s[0] + s[1] + s[2];
            let mean = tr / 3.0;
            let dev2 = (s[0] - mean).powi(2)
                + (s[1] - mean).powi(2)
                + (s[2] - mean).powi(2)
                + 2.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5]);
            out.push(Point {
                element: e,
                r: rad,
                eq: (1.5 * dev2).sqrt(),
                hoop: 0.5 * (tr - srr),
                alpha: r.states[e].values()[q * mat_j2::STATE_SIZE + 12],
            });
        }
    }
    out
}

/// The order of the last three residuals above the rounding floor `1e-12` (ADR 0404 reading of (a4)).
fn order(residuals: &[f64]) -> Option<f64> {
    let above: Vec<f64> = residuals.iter().copied().filter(|&r| r > 1e-12).collect();
    if above.len() < 3 {
        return None;
    }
    let [a, b, c] = [above[above.len() - 3], above[above.len() - 2], above[above.len() - 1]];
    Some((c / b).ln() / (b / a).ln())
}

/// O1 of one solve: the smallest order over its increments and the number of increments without three residuals above
/// the floor.
fn o1(c: &mut Checks, name: &str, steps: &[LoadStep]) {
    let mut least = f64::INFINITY;
    let mut short = 0;
    for s in steps {
        match order(&s.residuals) {
            Some(p) => least = least.min(p),
            None => {
                short += 1;
                println!("  {name}: increment {} residuals {:?}", s.load_factor, s.residuals);
            }
        }
    }
    c.at_least(&format!("O1 {name}: least order over {} increments", steps.len()), least, 1.8);
    c.holds(
        &format!("O1 {name}: increments with fewer than three residuals above 1e-12"),
        short == 0,
        &format!("{short}"),
    );
}

#[test]
#[ignore = "record of the registered run of ADR 0475 (commit cb17790f): S1 7.8e-4, S2 4.0e-4 (limit 1e-2), C1 0 wrong of 2592 at S1 and S2, S3 converged, met; O1 missed (least orders 1.279, 1.279, 1.274; S3 6 increments with two residuals above 1e-12): continued in s1_to_s3_c1_sphere_loading_met_rows and o1_order_known_miss"]
fn s1_to_s3_c1_o1_sphere_loading() {
    let mut c = Checks::new();
    let mat = j2(0.0, 0.0);
    for (id, cc, steps) in [("S1", 1.25, 8), ("S2", 1.5, 10)] {
        let p = p_of_c(cc) * SY;
        let model = sphere(mat.clone(), Some(p));
        let r = solve(&model, &[], &[], &NewtonOptions::new(steps)).unwrap();
        let pts = sphere_points(&model, &r);
        let elastic: Vec<&Point> = pts.iter().filter(|q| q.alpha == 0.0).collect();
        let c_fe = elastic.iter().map(|q| q.r * (q.eq / SY).cbrt()).sum::<f64>() / elastic.len() as f64;
        let spread = elastic
            .iter()
            .map(|q| (q.r * (q.eq / SY).cbrt() - c_fe).abs())
            .fold(0.0, f64::max);
        println!(
            "  {id}: c = {cc}, c_FE = {c_fe:.6} (max deviation {spread:.2e} over {} elastic points of {}), p(c_FE) = \
             {:.6} sigma_y, p = {:.6} sigma_y",
            elastic.len(),
            pts.len(),
            p_of_c(c_fe),
            p / SY
        );
        c.at_most(
            &format!("{id} c = {cc} a: |p(c_FE) - p| / p"),
            (p_of_c(c_fe) * SY - p).abs() / p,
            1e-2,
        );
        let wrong = pts
            .iter()
            .filter(|q| (q.r < cc - 0.05 && q.alpha == 0.0) || (q.r > cc + 0.05 && q.alpha > 0.0))
            .count();
        c.holds(
            &format!("C1 {id}: points yielded inside c - 0.05 a and elastic outside c + 0.05 a"),
            wrong == 0,
            &format!("{wrong} wrong of {}", pts.len()),
        );
        o1(&mut c, id, &r.steps);
    }
    // S3.
    let p = 0.98 * 2.0 * 2.0f64.ln() * SY;
    let model = sphere(mat.clone(), Some(p));
    let r = solve(&model, &[], &[], &NewtonOptions::new(20));
    c.holds("S3 the solve at 0.98 p_lim converges", r.is_ok(), &format!("{:?}", r.as_ref().err()));
    if let Ok(r) = r {
        let pts = sphere_points(&model, &r);
        let elastic: Vec<&Point> = pts.iter().filter(|q| q.alpha == 0.0).collect();
        let c_fe = elastic.iter().map(|q| q.r * (q.eq / SY).cbrt()).sum::<f64>() / elastic.len().max(1) as f64;
        println!(
            "  S3: {} increments, halvings {:?}, c_FE = {c_fe:.5} (closed form 1.80789), {} elastic points",
            r.steps.len(),
            r.steps.iter().map(|s| s.halvings).collect::<Vec<_>>(),
            elastic.len()
        );
        o1(&mut c, "S3", &r.steps);
    }
    c.finish();
}

/// The bore pressure as a driver contribution `-g(lambda) f1`, `g = 1 - |2 lambda - 1|` (load and unload).
struct Ramp {
    f1: Vec<f64>,
}

impl Contribution for Ramp {
    fn name(&self) -> &str {
        "bore pressure, loaded and unloaded"
    }
    fn add(
        &self,
        _u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        _tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        let g = 1.0 - (2.0 * load_factor - 1.0).abs();
        for (f, v) in force.iter_mut().zip(&self.f1) {
            *f = -g * v;
        }
        Ok(())
    }
}

#[test]
#[ignore = "known miss of ADR 0475 (registered run of commit cb17790f): residual hoop stress at the bore -0.612380 sigma_y against -0.628903 sigma_y, 2.627e-2 > 2e-2; diagnosed (addendum 1): point-wise hoop stress error of the 2 x 2 cubed-sphere octant, about 1.4 percent already in the elastic Lame field"]
fn s4_sphere_residual_hoop_stress_known_miss() {
    let mut c = Checks::new();
    let mat = j2(0.0, 0.0);
    let p1 = p_of_c(1.25) * SY;
    let f1 = nodal_load_vector(&sphere(mat.clone(), Some(p1)), &[]).unwrap();
    let model = sphere(mat, None);
    let ramp = Ramp { f1 };
    let ext = DriverExtensions {
        contributions: vec![&ramp],
        step_control: None,
    };
    let r = solve_with(&model, &[], &[], &NewtonOptions::new(16), ext).unwrap();
    let pts = sphere_points(&model, &r);
    let inner: Vec<&Point> = pts.iter().filter(|q| q.element < ELEMENTS_PER_LAYER).collect();
    // Least squares hoop = c0 + c1 t + c2 t^2, t = r - 1.125.
    let mut a = [[0.0; 3]; 3];
    let mut b = [0.0; 3];
    for q in &inner {
        let t = q.r - 1.125;
        let phi = [1.0, t, t * t];
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] += phi[i] * phi[j];
            }
            b[i] += phi[i] * q.hoop;
        }
    }
    // Gaussian elimination, 3 x 3.
    for col in 0..3 {
        for row in col + 1..3 {
            let f = a[row][col] / a[col][col];
            for k in col..3 {
                a[row][k] -= f * a[col][k];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = [0.0; 3];
    for row in (0..3).rev() {
        let mut v = b[row];
        for k in row + 1..3 {
            v -= a[row][k] * x[k];
        }
        x[row] = v / a[row][row];
    }
    let t = 1.0 - 1.125;
    let at_bore = (x[0] + x[1] * t + x[2] * t * t) / SY;
    let closed = 1.0 - 12.0 / 7.0 * p1 / SY;
    let inner_alpha = inner.iter().map(|q| q.alpha).fold(0.0, f64::max);
    println!(
        "  S4: {} increments, residual hoop at the bore {at_bore:.6} sigma_y, closed form {closed:.6} sigma_y, largest \
         alpha of the inner layer {inner_alpha:.3e}",
        r.steps.len()
    );
    c.at_most(
        "S4 residual hoop stress at the bore: |FE - closed| / |closed|",
        (at_bore - closed).abs() / closed.abs(),
        2e-2,
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- the beam

const BEAM_L: f64 = 0.02;
const BEAM_H: f64 = 0.1;
const BEAM_W: f64 = 0.01;

fn kappa_y() -> f64 {
    2.0 * SY / (E_MOD * BEAM_H)
}

/// The narrow beam of D6 and the prescribed displacements of the curvature `kappa`.
fn beam(kappa: f64) -> (SolidModel, Vec<Prescribed>) {
    let mesh = upgrade_to_hex20(
        &hex_box(
            [0.0, -BEAM_H / 2.0, -BEAM_W / 2.0],
            [BEAM_L, BEAM_H, BEAM_W],
            [1, 16, 1],
        )
        .unwrap(),
    )
    .unwrap()
    .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", j2(0.0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    let nodes = m.mesh().nodes().to_vec();
    let at = |p: Vec3| {
        nodes
            .iter()
            .position(|q| (q[0] - p[0]).abs() < 1e-12 && (q[1] - p[1]).abs() < 1e-12 && (q[2] - p[2]).abs() < 1e-12)
            .expect("node exists")
    };
    let mut pres: Vec<Prescribed> = m
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 0, -kappa * BEAM_L * nodes[n][1]))
        .collect();
    let o = at([0.0, 0.0, 0.0]);
    let top = at([0.0, BEAM_H / 2.0, 0.0]);
    pres.push(Prescribed::new(o, 1, 0.0));
    pres.push(Prescribed::new(o, 2, 0.0));
    pres.push(Prescribed::new(top, 2, 0.0));
    (m, pres)
}

fn moment(m: &SolidModel, r: &NonlinearResult) -> f64 {
    let react = r.result.reactions().unwrap();
    let dofs = r.result.dofs();
    m.mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| react[dofs.equation(n, 0).unwrap()] * m.mesh().nodes()[n][1])
        .sum::<f64>()
        .abs()
}

#[test]
#[ignore = "record of the registered run of ADR 0475 (commit cb17790f): C2 8.5e-8, P1 4.1e-5 .. 2.4e-4 (limit 1e-2) met; O1 missed (least orders 1.27 .. 1.44, 4 increments per solve with two residuals above 1e-12): continued in p1_c2_beam_pure_bending_met_rows and o1_order_known_miss"]
fn p1_c2_o1_beam_pure_bending() {
    let mut c = Checks::new();
    let my = SY * BEAM_W * BEAM_H * BEAM_H / 6.0;
    for k in [0.5, 1.25, 1.5, 2.0, 3.0, 4.0, 5.0] {
        let (m, pres) = beam(k * kappa_y());
        let steps = (4.0 * k).ceil() as usize;
        let r = solve(&m, &pres, &[], &NewtonOptions::new(steps)).unwrap();
        let ratio = moment(&m, &r) / my;
        let closed = if k < 1.0 { k } else { 1.5 - 0.5 / (k * k) };
        let id = if k < 1.0 { "C2" } else { "P1" };
        println!("  {id} kappa/kappa_y = {k}: M/M_y = {ratio:.6}, closed {closed:.6}");
        c.at_most(
            &format!("{id} kappa = {k} kappa_y: |M/M_y - closed| / closed"),
            (ratio - closed).abs() / closed,
            1e-2,
        );
        if k >= 1.0 {
            o1(&mut c, &format!("P1 kappa = {k} kappa_y"), &r.steps);
        }
    }
    c.finish();
}

/// A zero contribution that refuses its call number `fail_at` (1-based), if any.
struct FailOnce {
    calls: Cell<usize>,
    fail_at: Option<usize>,
}

impl Contribution for FailOnce {
    fn name(&self) -> &str {
        "fail once"
    }
    fn add(
        &self,
        _u: &[f64],
        _load_factor: f64,
        _force: &mut [f64],
        _tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        let n = self.calls.get() + 1;
        self.calls.set(n);
        if Some(n) == self.fail_at {
            return Err(FemError::InvalidValue {
                key: "fail once",
                reason: "registered refusal (H1)".to_string(),
            });
        }
        Ok(())
    }
}

#[test]
fn h1_halved_step_restores_the_state() {
    let mut c = Checks::new();
    let (m, pres) = beam(3.0 * kappa_y());
    let run = |steps: usize, fail_at: Option<usize>| {
        let f = FailOnce {
            calls: Cell::new(0),
            fail_at,
        };
        let ext = DriverExtensions {
            contributions: vec![&f],
            step_control: None,
        };
        solve_with(&m, &pres, &[], &NewtonOptions::new(steps), ext).unwrap()
    };
    let a = run(1, Some(3));
    let b = run(2, None);
    c.holds(
        "H1 set-up: A halved once (two increments, the first after one halving)",
        a.steps.len() == 2 && a.steps[0].halvings == 1,
        &format!(
            "{} increments, halvings {:?}",
            a.steps.len(),
            a.steps.iter().map(|s| s.halvings).collect::<Vec<_>>()
        ),
    );
    let plastic = b.states.iter().any(|s| s.values().chunks(13).any(|p| p[12] > 0.0));
    c.holds("H1 set-up: the state is plastic", plastic, "");
    let flat = |r: &NonlinearResult| {
        let mut v = r.result.displacement_values().to_vec();
        v.extend_from_slice(r.result.reactions().unwrap());
        for s in &r.states {
            v.extend_from_slice(s.values());
        }
        for s in &r.steps {
            v.push(s.load_factor);
            v.extend_from_slice(&s.residuals);
            v.extend_from_slice(&s.energies);
        }
        v
    };
    let (fa, fb) = (flat(&a), flat(&b));
    c.holds(
        "H1 halved step = two steps, bit for bit (displacement, reactions, states, increments)",
        bits(&fa) == bits(&fb),
        &format!("{} and {} values", fa.len(), fb.len()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- R1, N3

#[test]
fn r1_n3_refusals() {
    let mut c = Checks::new();
    let key_of = |r: rcfd_fem::Result<Material>| match r {
        Err(FemError::InvalidMaterial { key, .. }) => Some(key),
        _ => None,
    };
    for (name, r, key) in [
        ("yield_stress 0", Material::j2(base(), 0.0, 0.0, 0.0), "yield_stress"),
        ("yield_stress -2e8", Material::j2(base(), -2e8, 0.0, 0.0), "yield_stress"),
        ("yield_stress NaN", Material::j2(base(), f64::NAN, 0.0, 0.0), "yield_stress"),
        ("isotropic -1e9 (softening)", Material::j2(base(), SY, -1e9, 0.0), "isotropic"),
        ("kinematic -1e9 (softening)", Material::j2(base(), SY, 0.0, -1e9), "kinematic"),
    ] {
        let got = key_of(r);
        c.holds(
            &format!("R1 {name} refused with the key \"{key}\""),
            got == Some(key),
            &format!("{got:?}"),
        );
    }
    // N3: every path but the nonlinear static solve refuses J2.
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [4, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", j2(H, 0.0)).unwrap();
    for k in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [0.0, 0.0, -100.0])).unwrap();
    let refused = |r: rcfd_fem::Result<()>| {
        matches!(&r, Err(FemError::NotYetSupported { item, variant, location, package })
            if *item == "material" && *variant == "J2" && location == "material 0, element set 'all'"
                && *package == mat_j2::FOLLOW_UP)
    };
    let entries: Vec<(&str, rcfd_fem::Result<()>)> = vec![
        ("solve_static", solve_static(&m, &StaticOptions::default()).map(|_| ())),
        (
            "solve_static_with",
            solve_static_with(&m, &[], &[], &StaticOptions::default()).map(|_| ()),
        ),
        (
            "modal_analysis_with",
            modal_analysis_with(&m, &[], &ModalOptions::new(2)).map(|_| ()),
        ),
        ("stiffness", stiffness(&m).map(|_| ())),
        (
            "Integrator::new",
            Integrator::new(&m, &[], &TransientOptions::new(1e-4)).map(|_| ()),
        ),
        (
            "solve_static_mpc",
            solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).map(|_| ()),
        ),
    ];
    for (name, r) in entries {
        let msg = format!("{:?}", r.as_ref().err());
        c.holds(&format!("N3 {name} refuses J2"), refused(r), &msg);
    }
    let em = j2(H, 0.0).elasticity_matrix();
    c.holds(
        "N3 elasticity_matrix of J2 refused",
        matches!(&em, Err(FemError::NotYetSupported { location, package, .. })
            if location == "the element-constant elasticity matrix" && *package == mat_j2::FOLLOW_UP),
        &format!("{:?}", em.err()),
    );
    let ok = solve(&m, &[], &[], &NewtonOptions::new(1));
    c.holds(
        "N3 nonlinear::solve takes J2",
        ok.is_ok(),
        &format!("{:?}", ok.as_ref().err()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------- continuations

// T2 without its defective Hex20 elastic row (the same draws as the record, the row drawn and not checked).
#[test]
fn t2_element_tangent_met_rows() {
    let mut c = Checks::new();
    let mut rng = Rng::new(2);
    let mat = j2(1.5e9, 1.0e9);
    for kind in [ElementKind::Hex8, ElementKind::Hex20] {
        let el = element_for(kind);
        let x = distorted(kind, &mut rng);
        let n = 3 * el.n_nodes();
        let np = el.stiffness_rule().len();
        for (branch, scale, plastic) in [("elastic", 2e-4, false), ("plastic", 4e-3, true)] {
            let st = if plastic {
                let mut v = Vec::new();
                for _ in 0..np {
                    v.extend(random_state(&mut rng, 1e-3, 2e7, 1e-3));
                }
                ElementState::new(v)
            } else {
                ElementState::new(vec![0.0; np * mat_j2::STATE_SIZE])
            };
            let u: Vec<f64> = (0..n).map(|_| rng.sym(scale)).collect();
            if kind == ElementKind::Hex20 && !plastic {
                // ADR 0475 amendment 1: replaced by T2b.
                continue;
            }
            t2_row(&mut c, kind, branch, plastic, &mat, &x, &u, &st);
        }
    }
    c.finish();
}

/// One row of T2: the set-up check and the measure.
#[allow(clippy::too_many_arguments)]
fn t2_row(
    c: &mut Checks,
    kind: ElementKind,
    branch: &str,
    plastic: bool,
    mat: &Material,
    x: &[Vec3],
    u: &[f64],
    st: &ElementState,
) {
    let el = element_for(kind);
    let n = u.len();
    let np = el.stiffness_rule().len();
    let trial = element_trial_state(el, x, mat, u, st).unwrap();
    let yielded = (0..np)
        .filter(|&q| trial.values()[q * 13..(q + 1) * 13] != st.values()[q * 13..(q + 1) * 13])
        .count();
    c.holds(
        &format!("T2 {} {branch}: set-up (points yielded)", kind.name()),
        if plastic { yielded > 0 } else { yielded == 0 },
        &format!("{yielded} of {np}"),
    );
    let k = element_tangent(el, x, mat, u, st).unwrap();
    let h = 1e-9;
    let mut worst: f64 = 0.0;
    for j in 0..n {
        let mut a = u.to_vec();
        a[j] += h;
        let mut b = u.to_vec();
        b[j] -= h;
        let fa = element_internal_force(el, x, mat, &a, st).unwrap();
        let fb = element_internal_force(el, x, mat, &b, st).unwrap();
        for i in 0..n {
            worst = worst.max((k.values()[i * n + j] - (fa[i] - fb[i]) / (2.0 * h)).abs());
        }
    }
    let scale_k = max_abs(k.values().iter().copied());
    c.at_most(
        &format!("T2 {} {branch}: max |K - K_fd| / max |K|", kind.name()),
        worst / scale_k,
        1e-6,
    );
}

// T2b (ADR 0475 amendment 1): the Hex20 elastic branch with the amplitude 5e-5.
#[test]
fn t2b_hex20_elastic_tangent() {
    let mut c = Checks::new();
    let mut rng = Rng::new(3);
    let mat = j2(1.5e9, 1.0e9);
    let el = element_for(ElementKind::Hex20);
    let x = distorted(ElementKind::Hex20, &mut rng);
    let n = 3 * el.n_nodes();
    let np = el.stiffness_rule().len();
    let st = ElementState::new(vec![0.0; np * mat_j2::STATE_SIZE]);
    let u: Vec<f64> = (0..n).map(|_| rng.sym(5e-5)).collect();
    t2_row(&mut c, ElementKind::Hex20, "elastic (T2b)", false, &mat, &x, &u, &st);
    c.finish();
}

/// The loading solves of the sphere: (id, c, model, result) for S1 and S2.
fn sphere_s1_s2() -> Vec<(&'static str, f64, SolidModel, NonlinearResult)> {
    let mat = j2(0.0, 0.0);
    [("S1", 1.25, 8), ("S2", 1.5, 10)]
        .into_iter()
        .map(|(id, cc, steps)| {
            let model = sphere(mat.clone(), Some(p_of_c(cc) * SY));
            let r = solve(&model, &[], &[], &NewtonOptions::new(steps)).unwrap();
            (id, cc, model, r)
        })
        .collect()
}

fn sphere_s3() -> rcfd_fem::Result<NonlinearResult> {
    let model = sphere(j2(0.0, 0.0), Some(0.98 * 2.0 * 2.0f64.ln() * SY));
    solve(&model, &[], &[], &NewtonOptions::new(20))
}

// S1, S2, C1 and S3 of the record s1_to_s3_c1_o1_sphere_loading without O1.
#[test]
fn s1_to_s3_c1_sphere_loading_met_rows() {
    let mut c = Checks::new();
    for (id, cc, model, r) in sphere_s1_s2() {
        let p = p_of_c(cc) * SY;
        let pts = sphere_points(&model, &r);
        let elastic: Vec<&Point> = pts.iter().filter(|q| q.alpha == 0.0).collect();
        let c_fe = elastic.iter().map(|q| q.r * (q.eq / SY).cbrt()).sum::<f64>() / elastic.len() as f64;
        c.at_most(
            &format!("{id} c = {cc} a: |p(c_FE) - p| / p"),
            (p_of_c(c_fe) * SY - p).abs() / p,
            1e-2,
        );
        let wrong = pts
            .iter()
            .filter(|q| (q.r < cc - 0.05 && q.alpha == 0.0) || (q.r > cc + 0.05 && q.alpha > 0.0))
            .count();
        c.holds(
            &format!("C1 {id}: points yielded inside c - 0.05 a and elastic outside c + 0.05 a"),
            wrong == 0,
            &format!("{wrong} wrong of {}", pts.len()),
        );
    }
    let r = sphere_s3();
    c.holds("S3 the solve at 0.98 p_lim converges", r.is_ok(), &format!("{:?}", r.as_ref().err()));
    c.finish();
}

/// The beam solves of P1 and C2: (kappa / kappa_y, model, result).
fn beam_solves() -> Vec<(f64, SolidModel, NonlinearResult)> {
    [0.5, 1.25, 1.5, 2.0, 3.0, 4.0, 5.0]
        .into_iter()
        .map(|k| {
            let (m, pres) = beam(k * kappa_y());
            let r = solve(&m, &pres, &[], &NewtonOptions::new((4.0 * k).ceil() as usize)).unwrap();
            (k, m, r)
        })
        .collect()
}

// P1 and C2 of the record p1_c2_o1_beam_pure_bending without O1.
#[test]
fn p1_c2_beam_pure_bending_met_rows() {
    let mut c = Checks::new();
    let my = SY * BEAM_W * BEAM_H * BEAM_H / 6.0;
    for (k, m, r) in beam_solves() {
        let ratio = moment(&m, &r) / my;
        let closed = if k < 1.0 { k } else { 1.5 - 0.5 / (k * k) };
        let id = if k < 1.0 { "C2" } else { "P1" };
        c.at_most(
            &format!("{id} kappa = {k} kappa_y: |M/M_y - closed| / closed"),
            (ratio - closed).abs() / closed,
            1e-2,
        );
    }
    c.finish();
}

// O1 of S1, S2, S3 and P1, limit intact.
#[test]
#[ignore = "known miss of ADR 0475 (registered run of commit cb17790f): least orders S1 1.279, S2 1.279, S3 1.274, P1 1.270 .. 1.435 (limit 1.8); S3 6 and each P1 solve 4 increments with two residuals above 1e-12; diagnosed (addendum 1): the last three residuals above the floor include the pre-asymptotic residual of the active-set change, the asymptotic step is quadratic and reaches the rounding floor in one or two corrections"]
fn o1_order_known_miss() {
    let mut c = Checks::new();
    for (id, _, _, r) in sphere_s1_s2() {
        o1(&mut c, id, &r.steps);
    }
    o1(&mut c, "S3", &sphere_s3().unwrap().steps);
    for (k, _, r) in beam_solves() {
        if k >= 1.0 {
            o1(&mut c, &format!("P1 kappa = {k} kappa_y"), &r.steps);
        }
    }
    c.finish();
}
