//! F-380 (ADR 0281): thermal strain. Checks T1a to T4b, Q1, S1, the extensions E1, E2, B2, X1, X2 and the negative
//! controls N1 to N3 of the registration (seed 0xF380_0281). Check B1 is in `sb_thermal_bits.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::element_dofs;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::{b_matrix, element_for};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::material::apply;
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel, SolidResult};
use std::collections::BTreeSet;

const SEED: u64 = 0xF380_0281;
const KINDS: [ElementKind; 3] = [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10];
const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const ALPHA: f64 = 1.2e-5;
const T_REF: f64 = 293.15;
const DT: f64 = 50.0;
const BOX_L: Vec3 = [1.2, 1.0, 0.8];
const BOX_DIV: [usize; 3] = [3, 3, 2];

struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(SEED.wrapping_add(tag))
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.3e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn more_than(&mut self, name: &str, value: f64, bound: f64) {
        let ok = value > bound;
        println!(
            "{name}: {value:.3e} (control, must be > {bound:e}) {}",
            if ok { "holds" } else { "FAILED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {bound:e}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn kind_index(kind: ElementKind) -> u64 {
    match kind {
        ElementKind::Tet4 => 0,
        ElementKind::Hex8 => 1,
        _ => 2,
    }
}

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

/// A box of the kind; for Tet10 the Tet4 box upgraded (straight edges). With `perturb`, the interior grid nodes move by
/// `0.1 h_k (2U - 1)` per coordinate before the upgrade (ADR 0281 D4).
fn box_of(kind: ElementKind, origin: Vec3, lengths: Vec3, div: [usize; 3], perturb: Option<u64>) -> SolidMesh {
    let base = match kind {
        ElementKind::Hex8 => hex_box(origin, lengths, div).unwrap(),
        _ => tet_box(origin, lengths, div).unwrap(),
    };
    let base = match perturb {
        None => base,
        Some(tag) => {
            let mut rng = Rng::new(tag);
            let mut boundary = BTreeSet::new();
            for f in BOX_FACES {
                boundary.extend(base.node_set(f).unwrap().iter().copied());
            }
            let h = [
                lengths[0] / div[0] as f64,
                lengths[1] / div[1] as f64,
                lengths[2] / div[2] as f64,
            ];
            let mut nodes = base.nodes().to_vec();
            for (i, p) in nodes.iter_mut().enumerate() {
                if !boundary.contains(&i) {
                    for k in 0..3 {
                        p[k] += 0.1 * h[k] * (2.0 * rng.uniform() - 1.0);
                    }
                }
            }
            rebuild(&base, nodes)
        }
    };
    match kind {
        ElementKind::Tet10 => upgrade_to_tet10(&base).unwrap().0,
        _ => base,
    }
}

fn node_near(mesh: &SolidMesh, x: Vec3) -> usize {
    let mut best = (f64::INFINITY, 0);
    for (i, p) in mesh.nodes().iter().enumerate() {
        let d = (0..3).map(|k| (p[k] - x[k]).powi(2)).sum::<f64>();
        if d < best.0 {
            best = (d, i);
        }
    }
    assert!(best.0 < 1e-24, "no node at {x:?}");
    best.1
}

fn material(alpha: f64) -> Material {
    Material::isotropic_elastic(E_MOD, NU, 7850.0, alpha).unwrap()
}

fn model_of(mesh: SolidMesh, alpha: f64) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(alpha)).unwrap();
    m
}

fn perturbed_box(kind: ElementKind) -> SolidMesh {
    box_of(kind, [0.0; 3], BOX_L, BOX_DIV, Some(10 + kind_index(kind)))
}

/// The 3-2-1 support of the box (ADR 0281 arithmetic 3).
fn support_321(mesh: &SolidMesh) -> Vec<Prescribed> {
    let a = node_near(mesh, [0.0, 0.0, 0.0]);
    let b = node_near(mesh, [BOX_L[0], 0.0, 0.0]);
    let c = node_near(mesh, [0.0, BOX_L[1], 0.0]);
    vec![
        Prescribed::new(a, 0, 0.0),
        Prescribed::new(a, 1, 0.0),
        Prescribed::new(a, 2, 0.0),
        Prescribed::new(b, 1, 0.0),
        Prescribed::new(b, 2, 0.0),
        Prescribed::new(c, 2, 0.0),
    ]
}

fn solve(model: &SolidModel, pres: &[Prescribed]) -> (SolidResult, IntegrationPointField) {
    let r = solve_static_with(model, pres, &[], &StaticOptions::default()).unwrap();
    let f = IntegrationPointField::from_result(model, &r).unwrap();
    (r, f)
}

fn max_abs_stress(field: &IntegrationPointField) -> f64 {
    field
        .stress()
        .iter()
        .flat_map(|s| s.iter())
        .fold(0.0f64, |a, v| a.max(v.abs()))
}

fn volume(field: &IntegrationPointField) -> f64 {
    field.weights().iter().sum()
}

// ------------------------------------------------------------------------------------------------------------- T1

#[test]
fn t1_free_expansion_of_a_statically_determinate_box() {
    let mut ck = Checks::new();
    let su = E_MOD * ALPHA * DT;
    let sh = su / (1.0 - 2.0 * NU);
    let d = rcfd_fem::material::isotropic_elasticity(E_MOD, NU);
    for kind in KINDS {
        let k = kind.name();
        let mesh = perturbed_box(kind);
        let n = mesh.nodes().len();
        let pres = support_321(&mesh);
        let mut m = model_of(mesh, ALPHA);
        m.add_load(Load::temperature(vec![T_REF + DT; n], T_REF)).unwrap();
        let (r, field) = solve(&m, &pres);
        let mut err: f64 = 0.0;
        let mut xmax: f64 = 0.0;
        for (i, x) in m.mesh().nodes().iter().enumerate() {
            let u = r.displacement(i);
            for c in 0..3 {
                err = err.max((u[c] - ALPHA * DT * x[c]).abs());
                xmax = xmax.max(x[c].abs());
            }
        }
        ck.at_most(
            &format!("T1a {k} free expansion u - alpha dT x"),
            err / (ALPHA * DT * xmax),
            1e-10,
        );
        ck.at_most(
            &format!("T1b {k} free expansion max |sigma| / E alpha dT"),
            max_abs_stress(&field) / su,
            1e-10,
        );
        let total: f64 = field
            .strain()
            .iter()
            .flat_map(|e| apply(&d, e))
            .fold(0.0f64, |a, v| a.max(v.abs()));
        ck.more_than(
            &format!("N1 {k} D eps (thermal strain not subtracted) / E alpha dT"),
            total / su,
            1e-1,
        );
        let ue = field.strain_energy() / (E_MOD * (ALPHA * DT).powi(2) * volume(&field));
        ck.at_most(
            &format!("S1 {k} free expansion U_el / (E (alpha dT)^2 V)"),
            ue.abs(),
            1e-18,
        );
        println!(
            "T1 {k}: {} unknowns, residual {:e}, s_h = {sh:e}",
            3 * n,
            r.stats().unwrap().relative_residual()
        );
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------------- T2

#[test]
fn t2_box_held_on_all_faces_is_hydrostatic() {
    let mut ck = Checks::new();
    let su = E_MOD * ALPHA * DT;
    let sh = su / (1.0 - 2.0 * NU);
    for kind in KINDS {
        let k = kind.name();
        let mesh = perturbed_box(kind);
        let n = mesh.nodes().len();
        let mut m = model_of(mesh, ALPHA);
        for (face, c) in [
            ("xmin", 0),
            ("xmax", 0),
            ("ymin", 1),
            ("ymax", 1),
            ("zmin", 2),
            ("zmax", 2),
        ] {
            m.add_constraint(Constraint::displacement(face, c, 0.0)).unwrap();
        }
        m.add_load(Load::temperature(vec![T_REF + DT; n], T_REF)).unwrap();
        let (_, field) = solve(&m, &[]);
        let mut err: f64 = 0.0;
        let mut uni: f64 = 0.0;
        for s in field.stress() {
            for c in 0..3 {
                err = err.max((s[c] + sh).abs());
                uni = uni.max((s[c] + su).abs());
            }
            for v in &s[3..] {
                err = err.max(v.abs());
            }
        }
        ck.at_most(&format!("T2 {k} sigma + E alpha dT/(1 - 2 nu)"), err / sh, 1e-10);
        ck.more_than(&format!("N2 {k} against the uniaxial -E alpha dT"), uni / su, 1e-1);
        let exact = 3.0 * E_MOD * (ALPHA * DT).powi(2) * volume(&field) / (2.0 * (1.0 - 2.0 * NU));
        ck.at_most(
            &format!("S1 {k} box on all faces U_el against 3 E (alpha dT)^2 V/(2 (1 - 2 nu))"),
            (field.strain_energy() - exact).abs() / exact,
            1e-10,
        );
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------------- T3

#[test]
fn t3_bar_held_at_both_ends_with_free_sides() {
    let mut ck = Checks::new();
    let su = E_MOD * ALPHA * DT;
    let (b, area) = (0.1, 0.01);
    for kind in KINDS {
        let k = kind.name();
        let mesh = box_of(
            kind,
            [0.0, -0.05, -0.05],
            [1.0, b, b],
            [8, 2, 2],
            Some(20 + kind_index(kind)),
        );
        let n = mesh.nodes().len();
        let a = node_near(&mesh, [0.0, 0.0, 0.0]);
        let c = node_near(&mesh, [0.0, 0.05, 0.0]);
        let mut m = model_of(mesh, ALPHA);
        m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("xmax", 0, 0.0)).unwrap();
        m.add_load(Load::temperature(vec![T_REF + DT; n], T_REF)).unwrap();
        let pres = [
            Prescribed::new(a, 1, 0.0),
            Prescribed::new(a, 2, 0.0),
            Prescribed::new(c, 2, 0.0),
        ];
        let (r, field) = solve(&m, &pres);
        let mut axial: f64 = 0.0;
        let mut other: f64 = 0.0;
        for s in field.stress() {
            axial = axial.max((s[0] + su).abs());
            for v in &s[1..] {
                other = other.max(v.abs());
            }
        }
        ck.at_most(&format!("T3 {k} bar sigma_xx + E alpha dT"), axial / su, 1e-10);
        ck.at_most(&format!("T3 {k} bar other stress components"), other / su, 1e-10);
        let lat = (1.0 + NU) * ALPHA * DT;
        let mut uerr: f64 = 0.0;
        for (i, x) in m.mesh().nodes().iter().enumerate() {
            let u = r.displacement(i);
            let exact = [0.0, lat * x[1], lat * x[2]];
            for q in 0..3 {
                uerr = uerr.max((u[q] - exact[q]).abs());
            }
        }
        ck.at_most(
            &format!("E2 {k} bar displacement against (0, (1 + nu) alpha dT y, z)"),
            uerr / (lat * b / 2.0),
            1e-10,
        );
        let reac = r.reactions().unwrap();
        let rx: f64 = m.mesh().node_set("xmax").unwrap().iter().map(|&i| reac[3 * i]).sum();
        ck.at_most(
            &format!("E2 {k} bar reactions on xmax against -E alpha dT A"),
            (rx + su * area).abs() / (su * area),
            1e-10,
        );
        let exact = E_MOD * (ALPHA * DT).powi(2) * volume(&field) / 2.0;
        ck.at_most(
            &format!("S1 {k} bar U_el against E (alpha dT)^2 V/2"),
            (field.strain_energy() - exact).abs() / exact,
            1e-10,
        );
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------------- T4

/// The free beam of ADR 0281 arithmetic 6 with the temperature `T_ref + t0 + dt z/h`; returns (curvature, max |sigma|, max
/// nodal error against U, max |U|).
fn beam(t0: f64, dt: f64) -> (f64, f64, f64, f64) {
    let (l, b, h) = (1.0, 0.1, 0.1);
    let mesh = box_of(
        ElementKind::Tet10,
        [0.0, -b / 2.0, -h / 2.0],
        [l, b, h],
        [10, 2, 2],
        None,
    );
    let a = node_near(&mesh, [0.0, 0.0, 0.0]);
    let mid = node_near(&mesh, [l / 2.0, 0.0, 0.0]);
    let bb = node_near(&mesh, [l, 0.0, 0.0]);
    let c = node_near(&mesh, [0.0, b / 2.0, 0.0]);
    let temps: Vec<f64> = mesh.nodes().iter().map(|x| T_REF + t0 + dt * x[2] / h).collect();
    let mut m = model_of(mesh, ALPHA);
    m.add_load(Load::temperature(temps, T_REF)).unwrap();
    let pres = [
        Prescribed::new(a, 0, 0.0),
        Prescribed::new(a, 1, 0.0),
        Prescribed::new(a, 2, 0.0),
        Prescribed::new(bb, 1, 0.0),
        Prescribed::new(bb, 2, 0.0),
        Prescribed::new(c, 2, 0.0),
    ];
    let (r, field) = solve(&m, &pres);
    let uz = |i: usize| r.displacement(i)[2];
    let kappa = -(uz(a) - 2.0 * uz(mid) + uz(bb)) / (l / 2.0).powi(2);
    let w = [ALPHA * dt * b / (4.0 * h), -ALPHA * dt * l / (2.0 * h), 0.0];
    let mut err: f64 = 0.0;
    let mut umax: f64 = 0.0;
    for (i, x) in m.mesh().nodes().iter().enumerate() {
        let th = ALPHA * (t0 + dt * x[2] / h);
        let ustar = [
            ALPHA * (t0 * x[0] + dt * x[0] * x[2] / h),
            th * x[1],
            ALPHA * (t0 * x[2] + dt * x[2] * x[2] / (2.0 * h)) - ALPHA * dt * (x[0] * x[0] + x[1] * x[1]) / (2.0 * h),
        ];
        let rot = [
            w[1] * x[2] - w[2] * x[1],
            w[2] * x[0] - w[0] * x[2],
            w[0] * x[1] - w[1] * x[0],
        ];
        let u = r.displacement(i);
        for q in 0..3 {
            let exact = ustar[q] + rot[q];
            err = err.max((u[q] - exact).abs());
            umax = umax.max(exact.abs());
        }
    }
    (kappa, max_abs_stress(&field), err, umax)
}

#[test]
fn t4_free_beam_with_a_linear_temperature_through_the_thickness() {
    let mut ck = Checks::new();
    let su = E_MOD * ALPHA * DT;
    let h = 0.1;
    let exact = ALPHA * DT / h;
    let (kappa, smax, err, umax) = beam(20.0, DT);
    println!("T4 Tet10 beam: curvature {kappa:e} against {exact:e}");
    ck.at_most(
        "T4a Tet10 beam curvature against alpha dT/h",
        (kappa - exact).abs() / exact,
        1e-2,
    );
    ck.at_most("T4b Tet10 beam max |sigma| / E alpha dT", smax / su, 1e-2);
    ck.at_most("E1 Tet10 beam nodal displacement against u* + w x x", err / umax, 1e-9);
    let (k0, _, _, _) = beam(20.0, 0.0);
    ck.more_than(
        "N3 Tet10 beam without gradient, curvature measure",
        (k0 - exact).abs() / exact,
        0.5,
    );
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------------- Q1

#[test]
fn q1_recovered_stress_is_in_discrete_equilibrium() {
    let mut ck = Checks::new();
    for kind in KINDS {
        let k = kind.name();
        let mesh = perturbed_box(kind);
        let mut rng = Rng::new(30 + kind_index(kind));
        let temps: Vec<f64> = (0..mesh.nodes().len())
            .map(|_| T_REF + 50.0 + 20.0 * (2.0 * rng.uniform() - 1.0))
            .collect();
        let pres = support_321(&mesh);
        let mut m = model_of(mesh, ALPHA);
        m.add_load(Load::temperature(temps, T_REF)).unwrap();
        let fth = nodal_load_vector(&m, &[]).unwrap();
        let (r, field) = solve(&m, &pres);
        let mesh = m.mesh();
        let dofs = m.dof_map();
        let mut rint = vec![0.0; dofs.n_equations()];
        for e in 0..mesh.n_elements() {
            let el = element_for(mesh.element_kind(e));
            let nodes = mesh.element_nodes(e);
            let coords: Vec<Vec3> = nodes.iter().map(|&i| mesh.nodes()[i]).collect();
            let eqs = element_dofs(&dofs, nodes);
            let size = eqs.len();
            let ips = el.integration_points(&coords, el.stiffness_rule()).unwrap();
            for (ip, q) in ips.iter().zip(field.points_of(e)) {
                let bm = b_matrix(&ip.geometry.grad);
                let s = field.stress()[q];
                for (cidx, &eq) in eqs.iter().enumerate() {
                    let v: f64 = (0..6).map(|row| bm[row * size + cidx] * s[row]).sum();
                    rint[eq] += ip.weight * v;
                }
            }
        }
        let reac = r.reactions().unwrap();
        let fmax = fth.iter().fold(0.0f64, |a, v| a.max(v.abs()));
        let diff = rint.iter().zip(reac).fold(0.0f64, |a, (x, y)| a.max((x - y).abs()));
        ck.at_most(
            &format!("Q1 {k} sum w B^T sigma against the reactions"),
            diff / fmax,
            1e-9,
        );
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------------- B2

#[test]
fn b2_zero_thermal_strain_keeps_the_bits() {
    let mut ck = Checks::new();
    for kind in KINDS {
        let k = kind.name();
        let mesh = perturbed_box(kind);
        let n = mesh.nodes().len();
        let pres = support_321(&mesh);
        let plain = |alpha: f64| {
            let mut m = model_of(mesh.clone(), alpha);
            m.add_load(Load::nodal_force("xmax", [1.0e4, -2.0e3, 3.0e3])).unwrap();
            m
        };
        // (a) alpha = 0 with a temperature change, (b) T = T_ref at every node; each against the same model (same
        // material) without the temperature load.
        for (label, alpha, t) in [("alpha = 0", 0.0, T_REF + DT), ("T = T_ref", ALPHA, T_REF)] {
            let base_model = plain(alpha);
            let mut m = plain(alpha);
            m.add_load(Load::temperature(vec![t; n], T_REF)).unwrap();
            let m = &m;
            let (base_r, base_f) = solve(&base_model, &pres);
            let fl = nodal_load_vector(m, &[]).unwrap();
            let fb = nodal_load_vector(&base_model, &[]).unwrap();
            let bits = |a: &[f64], b: &[f64]| a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
            let (r, f) = solve(m, &pres);
            let dl = bits(&fl, &fb);
            let du = bits(r.displacement_values(), base_r.displacement_values());
            let dr = bits(r.reactions().unwrap(), base_r.reactions().unwrap());
            let ds = f
                .stress()
                .iter()
                .zip(base_f.stress())
                .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x != y))
                .count();
            println!("B2 {k} {label}: differing load {dl}, displacement {du}, reactions {dr} bits; stress points by value {ds}");
            ck.at_most(
                &format!("B2 {k} {label} differing bits, load + displacement + reactions"),
                (dl + du + dr) as f64,
                0.0,
            );
            ck.at_most(
                &format!("B2 {k} {label} points with a stress differing by value"),
                ds as f64,
                0.0,
            );
        }
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------------- X1, X2

#[test]
fn x1_x2_refusals() {
    let mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let n = mesh.nodes().len();
    // X1: two temperature fields.
    let mut m = model_of(mesh.clone(), ALPHA);
    m.add_load(Load::temperature(vec![350.0; n], T_REF)).unwrap();
    m.add_load(Load::temperature(vec![320.0; n], T_REF)).unwrap();
    let e = nodal_load_vector(&m, &[]).unwrap_err();
    println!("X1 load: {e}");
    assert!(matches!(e, FemError::InvalidValue { key: "temperature", .. }));
    let u = vec![0.0; 3 * n];
    let e = IntegrationPointField::compute(&m, &u).unwrap_err();
    println!("X1 recovery: {e}");
    assert!(matches!(e, FemError::InvalidValue { key: "temperature", .. }));
    // X2: a thermal load needs a material.
    let mut m = SolidModel::new(mesh);
    m.add_load(Load::temperature(vec![350.0; n], T_REF)).unwrap();
    let e = nodal_load_vector(&m, &[]).unwrap_err();
    println!("X2: {e}");
    assert!(matches!(e, FemError::MaterialUnassigned { element: 0 }));
}
