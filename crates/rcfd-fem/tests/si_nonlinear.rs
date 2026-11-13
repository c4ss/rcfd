//! F-397 (ADR 0404, the registration of ADR 0300): total Lagrangian solid elements and the Newton driver with load steps.
//! The checks A1 to A7, the companions C1 to C8 and the negative controls N1 to N3 of the registration (seed 0xF397_0404).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::element::{element_for, Element, ElementMatrix, ElementState};
use rcfd_fem::geometric::element_geometric_stiffness;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::nonlinear::{
    element_internal_force, element_second_piola_kirchhoff, element_tangent, initial_states, internal_force, solve,
    tangent, NewtonOptions, NonlinearResult,
};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, ElementKind, FemError, LinearTerm, Load, Material, SolidMesh, SolidModel};

const SEED: u64 = 0xF397_0404;
const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const KINDS: [ElementKind; 4] = [
    ElementKind::Tet4,
    ElementKind::Hex8,
    ElementKind::Tet10,
    ElementKind::Hex20,
];

/// SplitMix64, a deterministic source only (as in si_hex20.rs).
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
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
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

fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0, |m: f64, x| m.max(x.abs()))
}

fn mat_vec(a: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [
        a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2],
        a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2],
        a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2],
    ]
}

fn mat_mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut c = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            c[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    c
}

const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// The rotation by 90 degrees about `(1, 2, 3) / sqrt(14)`: `R = [n]x + n n^T` (Rodrigues with cos = 0, sin = 1).
fn rotation_90() -> [[f64; 3]; 3] {
    let s = 14f64.sqrt();
    let n = [1.0 / s, 2.0 / s, 3.0 / s];
    let cross = [[0.0, -n[2], n[1]], [n[2], 0.0, -n[0]], [-n[1], n[0], 0.0]];
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = cross[i][j] + n[i] * n[j];
        }
    }
    r
}

/// A distorted element of size `l = 1` (ADR 0404 D8): tetrahedra on the unit reference, hexahedra on `[-1, 1]^3 / 2`, every
/// coordinate moved by `sym(0.05)`.
fn distorted(kind: ElementKind, rng: &mut Rng) -> Vec<Vec3> {
    let scale = match kind {
        ElementKind::Tet4 | ElementKind::Tet10 => 1.0,
        _ => 0.5,
    };
    kind.reference_nodes()
        .iter()
        .map(|p| {
            let mut q = [0.0; 3];
            for i in 0..3 {
                q[i] = scale * p[i] + rng.sym(0.05);
            }
            q
        })
        .collect()
}

fn flatten(u: &[Vec3]) -> Vec<f64> {
    u.iter().flat_map(|v| v.iter().copied()).collect()
}

fn rel_diff(a: &ElementMatrix, b: &ElementMatrix) -> f64 {
    max_abs(a.values().iter().zip(b.values()).map(|(x, y)| x - y)) / max_abs(b.values().iter().copied())
}

fn fd_tangent(el: &dyn Element, coords: &[Vec3], mat: &Material, u: &[f64], h: f64) -> ElementMatrix {
    let n = u.len();
    let state = ElementState::empty();
    let mut k = ElementMatrix::zeros(n);
    for j in 0..n {
        let mut up = u.to_vec();
        let mut dn = u.to_vec();
        up[j] += h;
        dn[j] -= h;
        let fp = element_internal_force(el, coords, mat, &up, &state).unwrap();
        let fm = element_internal_force(el, coords, mat, &dn, &state).unwrap();
        for i in 0..n {
            k.values_mut()[i * n + j] = (fp[i] - fm[i]) / (2.0 * h);
        }
    }
    k
}

// ------------------------------------------------------------------------------------------------- A1, C1, C2, N2

#[test]
fn a1_c1_c2_n2_tangent_against_central_differences() {
    let mut c = Checks::new();
    let mat = material(0.3);
    let d = mat.elasticity_matrix().unwrap();
    let state = ElementState::empty();
    for (tag, kind) in KINDS.iter().enumerate() {
        let el = element_for(*kind);
        let mut rng = Rng::new(tag as u64);
        let coords = distorted(*kind, &mut rng);
        let mut g = [[0.0; 3]; 3];
        for row in g.iter_mut() {
            for v in row.iter_mut() {
                *v = rng.sym(0.2);
            }
        }
        let t = [rng.sym(0.2), rng.sym(0.2), rng.sym(0.2)];
        let rho: Vec<Vec3> = coords
            .iter()
            .map(|_| [rng.sym(0.02), rng.sym(0.02), rng.sym(0.02)])
            .collect();
        let n = coords.len() as f64;
        let xc = [0, 1, 2].map(|i| coords.iter().map(|p| p[i]).sum::<f64>() / n);
        let name = kind.name();
        // C2: at u = 0 the tangent is the linear stiffness.
        let zero = vec![0.0; 3 * coords.len()];
        let k0 = element_tangent(el, &coords, &mat, &zero, &state).unwrap();
        let k_lin = el.stiffness(&coords, &d).unwrap();
        c.at_most(
            &format!("C2 {name}: max |K_T(0) - K| / max |K|"),
            rel_diff(&k0, &k_lin),
            1e-14,
        );
        println!("C2 {name}: bit-equal {}", k0 == k_lin);
        let mut i_plus_g = IDENTITY;
        for i in 0..3 {
            for j in 0..3 {
                i_plus_g[i][j] += g[i][j];
            }
        }
        for (state_id, q) in [(1, IDENTITY), (2, rotation_90())] {
            let m = mat_mul(&q, &i_plus_g);
            let u: Vec<Vec3> = coords
                .iter()
                .zip(&rho)
                .map(|(x, r)| {
                    let dx = [x[0] - xc[0], x[1] - xc[1], x[2] - xc[2]];
                    let mx = mat_vec(&m, dx);
                    [0, 1, 2].map(|i| mx[i] - dx[i] + t[i] + r[i])
                })
                .collect();
            let u = flatten(&u);
            let kt = element_tangent(el, &coords, &mat, &u, &state).unwrap();
            let kfd = fd_tangent(el, &coords, &mat, &u, 1e-6);
            c.at_most(
                &format!("A1 {name} state {state_id}: max |K_fd - K_T| / max |K_T|"),
                rel_diff(&kfd, &kt),
                1e-6,
            );
            let nsz = kt.size();
            let asym = max_abs(
                (0..nsz)
                    .flat_map(|i| (0..nsz).map(move |j| (i, j)))
                    .map(|(i, j)| kt.get(i, j) - kt.get(j, i)),
            ) / max_abs(kt.values().iter().copied());
            c.at_most(
                &format!("C1 {name} state {state_id}: max |K_T - K_T^T| / max |K_T|"),
                asym,
                1e-13,
            );
            if state_id == 2 {
                let s = element_second_piola_kirchhoff(el, &coords, &mat, &u, &state).unwrap();
                let kg = element_geometric_stiffness(el, &coords, &s).unwrap();
                let mut kmat = kt.clone();
                for (v, gv) in kmat.values_mut().iter_mut().zip(kg.values()) {
                    *v -= gv;
                }
                c.at_least(
                    &format!("N2 {name}: material part only, max |K_fd - K_mat| / max |K_mat|"),
                    rel_diff(&kfd, &kmat),
                    1e-3,
                );
            }
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- A2, N1, C4

fn hex_box_model(nu: f64) -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap());
    m.add_material("all", material(nu)).unwrap();
    m
}

#[test]
#[ignore = "record of ADR 0404 before ADR 0417 (F-391, Beam2) retired the C4 Beam2 row (refusal naming ADR 0294); superseded by the _after_beam_landed copy below (maintainer standing decision, 2026-11-13)"]
fn a2_n1_c4_rigid_rotation_and_entry_points() {
    let mut c = Checks::new();
    let mat = material(0.3);
    let d = mat.elasticity_matrix().unwrap();
    let state = ElementState::empty();
    let q = rotation_90();
    let t = [0.3, -0.2, 0.5];
    let rigid = |x: &Vec3| {
        let r = mat_vec(&q, *x);
        [0, 1, 2].map(|i| r[i] - x[i] + t[i])
    };
    for (tag, kind) in KINDS.iter().enumerate() {
        let el = element_for(*kind);
        let mut rng = Rng::new(tag as u64);
        let coords = distorted(*kind, &mut rng);
        let u = flatten(&coords.iter().map(rigid).collect::<Vec<_>>());
        let name = kind.name();
        let f = element_internal_force(el, &coords, &mat, &u, &state).unwrap();
        c.at_most(
            &format!("A2 {name}: max |f_int| / (E l^2)"),
            max_abs(f.iter().copied()) / E_MOD,
            1e-10,
        );
        let ku = el.stiffness(&coords, &d).unwrap().matvec(&u);
        c.at_least(&format!("N1 {name}: max |K u| / (E l^2)"), max_abs(ku) / E_MOD, 1e-2);
        // C4: the trait entry points are the functions of the module, bit for bit.
        let f_trait = el.internal_force(&coords, &mat, &u, &state).unwrap();
        let k_trait = el.tangent(&coords, &mat, &u, &state).unwrap();
        let k_fn = element_tangent(el, &coords, &mat, &u, &state).unwrap();
        c.holds(
            &format!("C4 {name}"),
            f_trait == f && k_trait == k_fn,
            "Element::internal_force and Element::tangent equal the nonlinear functions bit for bit",
        );
    }
    let beam = element_for(ElementKind::Beam2);
    let r = beam.internal_force(&[[0.0; 3], [1.0, 0.0, 0.0]], &mat, &[0.0; 6], &state);
    c.holds(
        "C4 Beam2",
        matches!(&r, Err(FemError::NotYetSupported { package, .. }) if *package == "ADR 0294"),
        &format!("still refuses naming ADR 0294: {r:?}"),
    );
    // The assembled Hex8 box.
    let model = hex_box_model(0.3);
    let u = flatten(&model.mesh().nodes().iter().map(rigid).collect::<Vec<_>>());
    let states = initial_states(&model).unwrap();
    let f = internal_force(&model, &u, &states).unwrap();
    c.at_most("A2 Hex8 box: max |f_int| / (E l^2)", max_abs(f) / E_MOD, 1e-10);
    c.finish();
}

/// A2, N1, C4 after ADR 0417 (F-391): the C4 Beam2 row refuses naming `beam::SOLID_ROUTINES`; every other row as registered.
#[test]
fn a2_n1_c4_rigid_rotation_and_entry_points_after_beam_landed() {
    let mut c = Checks::new();
    let mat = material(0.3);
    let d = mat.elasticity_matrix().unwrap();
    let state = ElementState::empty();
    let q = rotation_90();
    let t = [0.3, -0.2, 0.5];
    let rigid = |x: &Vec3| {
        let r = mat_vec(&q, *x);
        [0, 1, 2].map(|i| r[i] - x[i] + t[i])
    };
    for (tag, kind) in KINDS.iter().enumerate() {
        let el = element_for(*kind);
        let mut rng = Rng::new(tag as u64);
        let coords = distorted(*kind, &mut rng);
        let u = flatten(&coords.iter().map(rigid).collect::<Vec<_>>());
        let name = kind.name();
        let f = element_internal_force(el, &coords, &mat, &u, &state).unwrap();
        c.at_most(
            &format!("A2 {name}: max |f_int| / (E l^2)"),
            max_abs(f.iter().copied()) / E_MOD,
            1e-10,
        );
        let ku = el.stiffness(&coords, &d).unwrap().matvec(&u);
        c.at_least(&format!("N1 {name}: max |K u| / (E l^2)"), max_abs(ku) / E_MOD, 1e-2);
        // C4: the trait entry points are the functions of the module, bit for bit.
        let f_trait = el.internal_force(&coords, &mat, &u, &state).unwrap();
        let k_trait = el.tangent(&coords, &mat, &u, &state).unwrap();
        let k_fn = element_tangent(el, &coords, &mat, &u, &state).unwrap();
        c.holds(
            &format!("C4 {name}"),
            f_trait == f && k_trait == k_fn,
            "Element::internal_force and Element::tangent equal the nonlinear functions bit for bit",
        );
    }
    let beam = element_for(ElementKind::Beam2);
    let r = beam.internal_force(&[[0.0; 3], [1.0, 0.0, 0.0]], &mat, &[0.0; 6], &state);
    c.holds(
        "C4 Beam2",
        matches!(&r, Err(FemError::NotYetSupported { package, .. }) if *package == rcfd_fem::beam::SOLID_ROUTINES),
        &format!("refuses naming beam::SOLID_ROUTINES (ADR 0417 D7): {r:?}"),
    );
    // The assembled Hex8 box.
    let model = hex_box_model(0.3);
    let u = flatten(&model.mesh().nodes().iter().map(rigid).collect::<Vec<_>>());
    let states = initial_states(&model).unwrap();
    let f = internal_force(&model, &u, &states).unwrap();
    c.at_most("A2 Hex8 box: max |f_int| / (E l^2)", max_abs(f) / E_MOD, 1e-10);
    c.finish();
}

// ------------------------------------------------------------------------------------------------- A3, A4, C7

const BAR_L: f64 = 1.0;
const BAR_A: f64 = 0.5;

fn bar_mesh(kind: ElementKind) -> SolidMesh {
    let lengths = [BAR_L, BAR_A, BAR_A];
    let div = [2, 1, 1];
    match kind {
        ElementKind::Hex8 => hex_box([0.0; 3], lengths, div).unwrap(),
        ElementKind::Tet4 => tet_box([0.0; 3], lengths, div).unwrap(),
        ElementKind::Tet10 => upgrade_to_tet10(&tet_box([0.0; 3], lengths, div).unwrap()).unwrap().0,
        ElementKind::Hex20 => upgrade_to_hex20(&hex_box([0.0; 3], lengths, div).unwrap()).unwrap().0,
        _ => unreachable!(),
    }
}

fn bar_model(kind: ElementKind) -> SolidModel {
    let mut m = SolidModel::new(bar_mesh(kind));
    m.add_material("all", material(0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

fn p_exact() -> f64 {
    E_MOD * 1.2 * (1.2 * 1.2 - 1.0) / 2.0
}

fn bar_load_control(kind: ElementKind) -> SolidModel {
    let mut m = bar_model(kind);
    m.add_load(Load::traction("xmax", [p_exact(), 0.0, 0.0])).unwrap();
    m
}

fn mean_ux(model: &SolidModel, r: &NonlinearResult, set: &str) -> f64 {
    let nodes = model.mesh().node_set(set).unwrap();
    nodes.iter().map(|&n| r.result.displacement(n)[0]).sum::<f64>() / nodes.len() as f64
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

#[test]
fn a3_a4_c7_uniform_stretch() {
    let mut c = Checks::new();
    let pe = p_exact();
    for kind in KINDS {
        let name = kind.name();
        // Displacement control.
        let mut m = bar_model(kind);
        m.add_constraint(Constraint::displacement("xmax", 0, 0.2 * BAR_L))
            .unwrap();
        let r = solve(&m, &[], &[], &NewtonOptions::new(4)).unwrap();
        let reactions = r.result.reactions().unwrap();
        let dofs = r.result.dofs();
        let sum: f64 = m
            .mesh()
            .node_set("xmax")
            .unwrap()
            .iter()
            .map(|&n| reactions[dofs.equation(n, 0).unwrap()])
            .sum();
        let p = sum / (BAR_A * BAR_A);
        c.at_most(
            &format!("A3 {name} displacement control: |P / P_exact - 1|"),
            (p / pe - 1.0).abs(),
            1e-9,
        );
        let lateral = max_abs(
            (0..m.mesh().nodes().len()).flat_map(|n| [r.result.displacement(n)[1], r.result.displacement(n)[2]]),
        ) / BAR_L;
        c.at_most(&format!("C7 {name}: max |u_y|, |u_z| / L"), lateral, 1e-12);
        println!(
            "  {name} displacement control: {} increments, iterations {:?}",
            r.steps.len(),
            r.steps.iter().map(|s| s.residuals.len() - 1).collect::<Vec<_>>()
        );
        // Load control.
        let m = bar_load_control(kind);
        let r = solve(&m, &[], &[], &NewtonOptions::new(2)).unwrap();
        let lambda = 1.0 + mean_ux(&m, &r, "xmax") / BAR_L;
        c.at_most(
            &format!("A3 {name} load control: |lambda / 1.2 - 1|"),
            (lambda / 1.2 - 1.0).abs(),
            1e-9,
        );
        for s in &r.steps {
            println!("  {name} step {} residuals {:?}", s.step, s.residuals);
            match order(&s.residuals) {
                Some(p) => c.at_least(&format!("A4 {name} step {}: order", s.step), p, 1.8),
                None => c.holds(
                    &format!("A4 {name} step {}", s.step),
                    false,
                    "fewer than three residuals above 1e-12",
                ),
            }
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- A5, A6, C8, N3

const CANT_L: f64 = 1.0;
const CANT_H: f64 = 0.02;

fn cantilever(alpha: f64, scale: f64) -> SolidModel {
    let mesh = upgrade_to_hex20(&hex_box([0.0; 3], [CANT_L, CANT_H, CANT_H], [50, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(0.0)).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    let i = CANT_H * CANT_H.powi(3) / 12.0;
    let p = alpha * E_MOD * i / (CANT_L * CANT_L);
    m.add_load(Load::traction("xmax", [0.0, scale * p / (CANT_H * CANT_H), 0.0]))
        .unwrap();
    m
}

/// The two mid-edge nodes of the tip face on the neutral axis (`x = L`, `y = h / 2`).
fn tip_nodes(m: &SolidModel) -> Vec<usize> {
    let tol = 1e-9;
    let t: Vec<usize> = (0..m.mesh().nodes().len())
        .filter(|&n| {
            let p = m.mesh().nodes()[n];
            (p[0] - CANT_L).abs() < tol && (p[1] - CANT_H / 2.0).abs() < tol
        })
        .collect();
    assert_eq!(t.len(), 2, "two tip nodes on the neutral axis");
    t
}

fn tip(m: &SolidModel, u: impl Fn(usize) -> Vec3) -> (f64, f64) {
    let t = tip_nodes(m);
    let v = t.iter().map(|&n| u(n)[1]).sum::<f64>() / 2.0;
    let d = -t.iter().map(|&n| u(n)[0]).sum::<f64>() / 2.0;
    (v, d)
}

fn rk4(alpha: f64, c0: f64, steps: usize) -> [f64; 4] {
    let h = 1.0 / steps as f64;
    let f = |s: [f64; 4]| [s[1], -alpha * s[0].cos(), s[0].cos(), s[0].sin()];
    let mut s = [0.0, c0, 0.0, 0.0];
    let add = |s: [f64; 4], k: [f64; 4], a: f64| [s[0] + a * k[0], s[1] + a * k[1], s[2] + a * k[2], s[3] + a * k[3]];
    for _ in 0..steps {
        let k1 = f(s);
        let k2 = f(add(s, k1, h / 2.0));
        let k3 = f(add(s, k2, h / 2.0));
        let k4 = f(add(s, k3, h));
        for i in 0..4 {
            s[i] += h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
    }
    s
}

/// The elastica `theta'' = -alpha cos(theta)`, `theta(0) = 0`, `theta'(1) = 0` by shooting on `theta'(0)` (bisection):
/// the tip deflection and shortening over `L`.
fn elastica(alpha: f64) -> (f64, f64) {
    let (mut lo, mut hi) = (0.0, alpha);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if rk4(alpha, mid, 4000)[1] > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let s = rk4(alpha, lo, 4000);
    (s[3], 1.0 - s[2])
}

#[test]
#[ignore = "record of ADR 0404 A5, missed in the registered run: the cantilever at s = 1e-6 did not converge (relative residual 4.2e-5 after 20 iterations at the fifth halving; the rounding floor of E = (F^T F - I) / 2 at strains of 1e-8); corrected by ADR 0404 amendment 1, a5_linear_limit_corrected"]
fn a5_linear_limit_known_miss() {
    let mut c = Checks::new();
    let s = 1e-6;
    // The cantilever, alpha = 1.
    let lin = solve_static(&cantilever(1.0, 1.0), &StaticOptions::default()).unwrap();
    let nl = solve(&cantilever(1.0, s), &[], &[], &NewtonOptions::new(1)).unwrap();
    let ul = lin.displacement_values();
    let dev =
        max_abs(nl.result.displacement_values().iter().zip(ul).map(|(a, b)| a / s - b)) / max_abs(ul.iter().copied());
    c.at_most("A5 cantilever: max |u_nl / s - u_lin| / max |u_lin|", dev, 1e-5);
    // The Tet10 block.
    let lin = solve_static(&tet10_block(1.0), &StaticOptions::default()).unwrap();
    let nl = solve(&tet10_block(s), &[], &[], &NewtonOptions::new(1)).unwrap();
    let ul = lin.displacement_values();
    let dev =
        max_abs(nl.result.displacement_values().iter().zip(ul).map(|(a, b)| a / s - b)) / max_abs(ul.iter().copied());
    c.at_most("A5 Tet10 block: max |u_nl / s - u_lin| / max |u_lin|", dev, 1e-5);
    c.finish();
}

/// A5 after ADR 0404 amendment 1 (the Green-Lagrange strain from `H = F - I`): the same set-up and limits.
#[test]
fn a5_linear_limit_corrected() {
    let mut c = Checks::new();
    let s = 1e-6;
    // The cantilever, alpha = 1.
    let lin = solve_static(&cantilever(1.0, 1.0), &StaticOptions::default()).unwrap();
    let nl = solve(&cantilever(1.0, s), &[], &[], &NewtonOptions::new(1)).unwrap();
    let ul = lin.displacement_values();
    let dev =
        max_abs(nl.result.displacement_values().iter().zip(ul).map(|(a, b)| a / s - b)) / max_abs(ul.iter().copied());
    c.at_most("A5 cantilever: max |u_nl / s - u_lin| / max |u_lin|", dev, 1e-5);
    // The Tet10 block.
    let lin = solve_static(&tet10_block(1.0), &StaticOptions::default()).unwrap();
    let nl = solve(&tet10_block(s), &[], &[], &NewtonOptions::new(1)).unwrap();
    let ul = lin.displacement_values();
    let dev =
        max_abs(nl.result.displacement_values().iter().zip(ul).map(|(a, b)| a / s - b)) / max_abs(ul.iter().copied());
    c.at_most("A5 Tet10 block: max |u_nl / s - u_lin| / max |u_lin|", dev, 1e-5);
    c.finish();
}

#[test]
fn a6_c8_n3_elastica() {
    let mut c = Checks::new();
    let registered = [(1.0, 0.30172077, 0.05643324), (2.0, 0.49345748, 0.16064172)];
    for (alpha, v_reg, d_reg) in registered {
        let (v_ref, d_ref) = elastica(alpha);
        println!("  alpha {alpha}: elastica v = {v_ref:.10}, d = {d_ref:.10}");
        c.at_most(
            &format!("C8 alpha {alpha}: elastica against arithmetic 1"),
            (v_ref - v_reg).abs().max((d_ref - d_reg).abs()),
            1e-6,
        );
        let steps = (5.0 * alpha) as usize;
        let m = cantilever(alpha, 1.0);
        let r = solve(&m, &[], &[], &NewtonOptions::new(steps)).unwrap();
        println!(
            "  alpha {alpha}: {} increments, iterations {:?}, halvings {:?}",
            r.steps.len(),
            r.steps.iter().map(|s| s.residuals.len() - 1).collect::<Vec<_>>(),
            r.steps.iter().map(|s| s.halvings).collect::<Vec<_>>()
        );
        let (v, d) = tip(&m, |n| {
            let u = r.result.displacement(n);
            [u[0], u[1], u[2]]
        });
        println!("  alpha {alpha}: FE v = {v:.10}, d = {d:.10}");
        c.at_most(
            &format!("A6 alpha {alpha}: |v / v_ref - 1|"),
            (v / v_ref - 1.0).abs(),
            0.02,
        );
        c.at_most(
            &format!("A6 alpha {alpha}: |d / d_ref - 1|"),
            (d / d_ref - 1.0).abs(),
            0.02,
        );
        let lin = solve_static(&m, &StaticOptions::default()).unwrap();
        let (vl, _) = tip(&m, |n| {
            let u = lin.displacement(n);
            [u[0], u[1], u[2]]
        });
        c.at_least(
            &format!("N3 alpha {alpha}: linear |v_lin / v_ref - 1|"),
            (vl / v_ref - 1.0).abs(),
            0.05,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------- A7, C3

#[test]
fn a7_c3_inversion_halving_and_non_convergence() {
    let mut c = Checks::new();
    // Inversion.
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 2.0], [1, 1, 2]).unwrap();
    let zc = |e: usize| mesh_centroid_z(&mesh, e);
    assert!(zc(0) < 1.0 && zc(1) > 1.0, "element 1 is the upper one");
    mesh.add_element_set("lower", vec![0]).unwrap();
    mesh.add_element_set("upper", vec![1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material(
        "lower",
        Material::isotropic_elastic(1000.0 * E_MOD, 0.0, RHO, 0.0).unwrap(),
    )
    .unwrap();
    m.add_material("upper", material(0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmax", 2, -1.5)).unwrap();
    let r = solve(&m, &[], &[], &NewtonOptions::new(1));
    let ok = matches!(&r, Err(FemError::Element { element: 1, source })
        if matches!(source.as_ref(), FemError::InvalidValue { key, reason }
            if *key == "deformation gradient" && reason.contains("load step 1 of 1")));
    c.holds("A7 inversion", ok, &format!("{r:?}"));
    // Non-convergence after the halvings.
    let bar = bar_load_control(ElementKind::Hex8);
    let r = solve(&bar, &[], &[], &NewtonOptions::new(1).max_iterations(1));
    let ok = matches!(&r, Err(FemError::Solver { solver, reason })
        if *solver == "newton" && reason.contains("load step 1 of 1") && reason.contains("5 halvings"));
    c.holds("A7 non-convergence", ok, &format!("{r:?}"));
    // C3: one halving, then the two half steps.
    let r1 = solve(&bar, &[], &[], &NewtonOptions::new(1).max_iterations(4)).unwrap();
    let r2 = solve(&bar, &[], &[], &NewtonOptions::new(2)).unwrap();
    let h = r1.steps.last().unwrap().halvings;
    c.holds(
        "C3 halvings",
        h == 1,
        &format!("{h} halvings, {} increments", r1.steps.len()),
    );
    let (a, b) = (r1.result.displacement_values(), r2.result.displacement_values());
    let dev = max_abs(a.iter().zip(b).map(|(x, y)| x - y)) / max_abs(b.iter().copied());
    c.at_most("C3 max |u - u_2steps| / max |u_2steps|", dev, 1e-12);
    println!("C3 bit-equal {}", a == b);
    c.finish();
}

fn mesh_centroid_z(mesh: &SolidMesh, e: usize) -> f64 {
    let nodes = mesh.element_nodes(e);
    nodes.iter().map(|&n| mesh.nodes()[n][2]).sum::<f64>() / nodes.len() as f64
}

// ------------------------------------------------------------------------------------------------- C5, C6

fn tet10_block(scale: f64) -> SolidModel {
    let mesh = upgrade_to_tet10(&tet_box([0.0; 3], [1.0, 0.25, 0.25], [4, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(0.0)).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, scale * 1e7, scale * 5e6]))
        .unwrap();
    m
}

#[test]
fn c5_c6_refusals_and_threads() {
    let mut c = Checks::new();
    let mut m = bar_load_control(ElementKind::Hex8);
    m.add_constraint(Constraint::linear(vec![LinearTerm::new(1, 0, 1.0)], 0.0))
        .unwrap();
    let r = solve(&m, &[], &[], &NewtonOptions::new(1));
    c.holds(
        "C5 linear constraint",
        matches!(&r, Err(FemError::InvalidValue { key, .. }) if *key == "nonlinear"),
        &format!("{r:?}"),
    );
    let mut m = bar_load_control(ElementKind::Hex8);
    let n = m.mesh().nodes().len();
    m.add_load(Load::temperature(vec![20.0; n], 0.0)).unwrap();
    let r = solve(&m, &[], &[], &NewtonOptions::new(1));
    c.holds(
        "C5 temperature load",
        matches!(&r, Err(FemError::InvalidValue { key, .. }) if *key == "nonlinear"),
        &format!("{r:?}"),
    );
    let m = bar_load_control(ElementKind::Hex8);
    let r = solve(&m, &[], &[], &NewtonOptions::new(0));
    c.holds(
        "C5 steps = 0",
        matches!(&r, Err(FemError::InvalidValue { key, .. }) if *key == "steps"),
        &format!("{r:?}"),
    );
    let el = element_for(ElementKind::Hex8);
    let coords = ElementKind::Hex8.reference_nodes().to_vec();
    let r = element_internal_force(
        el,
        &coords,
        &material(0.0),
        &[0.0; 24],
        &ElementState::new(vec![0.0; 3]),
    );
    c.holds(
        "C5 state length",
        matches!(&r, Err(FemError::FieldLength { key, .. }) if *key == "state"),
        &format!("{r:?}"),
    );
    // C6: threads.
    let m = tet10_block(1.0);
    let r1 = solve(&m, &[], &[], &NewtonOptions::new(5).threads(1)).unwrap();
    let r4 = solve(&m, &[], &[], &NewtonOptions::new(5).threads(4)).unwrap();
    c.holds(
        "C6 solve threads 1 and 4",
        r1.result.displacement_values() == r4.result.displacement_values(),
        "displacements bit-equal",
    );
    let states = initial_states(&m).unwrap();
    let u = r1.result.displacement_values();
    let k1 = tangent(&m, u, &states, Some(1)).unwrap();
    let k4 = tangent(&m, u, &states, Some(4)).unwrap();
    c.holds(
        "C6 tangent threads 1 and 4",
        k1.val == k4.val && k1.col == k4.col,
        "tangent bit-equal",
    );
    println!("  Tet10 block: tip mean u_y = {:e}", tip_mean(&m, &r1, 1));
    c.finish();
}

fn tip_mean(m: &SolidModel, r: &NonlinearResult, comp: usize) -> f64 {
    let nodes = m.mesh().node_set("xmax").unwrap();
    nodes.iter().map(|&n| r.result.displacement(n)[comp]).sum::<f64>() / nodes.len() as f64
}
