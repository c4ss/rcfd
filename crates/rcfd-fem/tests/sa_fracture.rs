//! F-429 (ADR 0547): fracture parameters. Checks P1 to P6, PH1 to PH3 (heavy record), PF, G1 to G3, R1 to R4, the controls
//! N1, N2 and F1 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::fracture::{
    centre_crack_plate, j_integral, opening_k, paris_cycles, penny_crack_cylinder, plane_strain_k, CrackFront,
    CrackedCylinder, CrackedPlate, FrontValue, OpeningFit, PennyCrack, PlateCrack,
};
use rcfd_fem::hex20::traction_loads;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::solve::{solve_static_with, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const A: f64 = 0.01;
const SIGMA: f64 = 1.0e8;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A non-finite
/// value is a miss of F1.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !value.is_finite() {
            self.0.push(format!("F1 {name} = {value} is not finite"));
        } else if !ok {
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

fn rel(v: f64, r: f64) -> f64 {
    (v / r - 1.0).abs()
}

fn k_ref() -> f64 {
    let w = 10.0 * A;
    SIGMA * (PI * A).sqrt() * (1.0 / (PI * A / (2.0 * w)).cos()).sqrt()
}

fn k_of(j: f64) -> f64 {
    plane_strain_k(j, E_MOD, NU).unwrap_or(f64::NAN)
}

fn material() -> Material {
    Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap()
}

// --------------------------------------------------------------------------------------------------------------- plate

/// The registered plate `P` with half crack length `a`.
fn plate(a: f64) -> CrackedPlate {
    centre_crack_plate(&PlateCrack::new(a, 10.0 * A, 20.0 * A, 0.05 * A, A / 40.0, 1.4, 4)).unwrap()
}

#[derive(Clone, Copy)]
enum Loading {
    Tension,
    Shear,
}

fn plate_loads(mesh: &SolidMesh, loading: Loading) -> Vec<NodalForceAt> {
    let s = SIGMA;
    let parts: Vec<(&str, Vec3)> = match loading {
        Loading::Tension => vec![("top", [0.0, s, 0.0]), ("bottom", [0.0, -s, 0.0])],
        Loading::Shear => vec![
            ("top", [s, 0.0, 0.0]),
            ("bottom", [-s, 0.0, 0.0]),
            ("right", [0.0, s, 0.0]),
            ("left", [0.0, -s, 0.0]),
        ],
    };
    let mut f = Vec::new();
    for (set, t) in parts {
        f.extend(traction_loads(mesh, set, t).unwrap());
    }
    f
}

fn plate_model(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("right_pin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("right_pin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("left_pin", 1, 0.0)).unwrap();
    m
}

struct Solved {
    plate: CrackedPlate,
    model: SolidModel,
    u: Vec<f64>,
    forces: Vec<NodalForceAt>,
}

fn solve_plate(a: f64, loading: Loading) -> Solved {
    let plate = plate(a);
    let forces = plate_loads(&plate.mesh, loading);
    let model = plate_model(plate.mesh.clone());
    let options = StaticOptions::with_solver(StaticSolver::Direct).refinement(2);
    let res = solve_static_with(&model, &[], &forces, &options).unwrap();
    let u = res.displacement_values().to_vec();
    Solved {
        plate,
        model,
        u,
        forces,
    }
}

fn ring(s: &Solved, k: usize) -> Vec<FrontValue> {
    j_integral(&s.model, &s.u, &s.plate.crack_front(), k).unwrap()
}

fn opening(s: &Solved, r_min: f64, r_max: f64) -> rcfd_fem::Result<OpeningFit> {
    opening_k(&s.model, &s.u, &s.plate.crack_front(), &s.plate.pairs, r_min, r_max)
}

fn work(s: &Solved) -> f64 {
    let dofs = s.model.dof_map();
    s.forces
        .iter()
        .map(|f| {
            (0..3)
                .map(|c| f.force[c] * s.u[dofs.equation(f.node, c).unwrap()])
                .sum::<f64>()
        })
        .sum()
}

fn refused(r: &rcfd_fem::Result<impl std::fmt::Debug>, key: &str, text: &str) -> (bool, String) {
    match r {
        Err(FemError::InvalidValue { key: k, reason }) => {
            (*k == key && reason.contains(text), format!("{k}: {reason}"))
        }
        Err(e) => (false, format!("other error {e}")),
        Ok(v) => (false, format!("accepted {v:?}")),
    }
}

#[test]
fn p1_p2_p4_p6_n1_n2_r2_tension() {
    let mut c = Checks::new();
    let s = solve_plate(A, Loading::Tension);
    println!(
        "P: {} nodes, {} elements, columns_to_edge {}, {} pairs; K_ref {:.6e}",
        s.plate.mesh.nodes().len(),
        s.plate.mesh.n_elements(),
        s.plate.columns_to_edge,
        s.plate.pairs.len(),
        k_ref()
    );
    let rings: Vec<Vec<FrontValue>> = (1..=4).map(|k| ring(&s, k)).collect();
    for (k, r) in rings.iter().enumerate() {
        for v in r {
            println!(
                "ring {} station {}: J {:.6e}, K {:.6e}, K/K_ref - 1 = {:+.4e}",
                k + 1,
                v.station,
                v.j,
                k_of(v.j),
                k_of(v.j) / k_ref() - 1.0
            );
        }
    }
    // P1.
    let mut p1: f64 = 0.0;
    for r in &rings[1..4] {
        for v in r {
            p1 = p1.max(rel(k_of(v.j), k_ref()));
        }
    }
    c.at_most(
        "P1 K from J of rings 2-4, both stations, against the secant form",
        p1,
        0.02,
    );
    // P2.
    let mut p2: f64 = 0.0;
    for st in 0..2 {
        let js: Vec<f64> = rings[1..4].iter().map(|r| r[st].j).collect();
        let (lo, hi) = js
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &v| (l.min(v), h.max(v)));
        p2 = p2.max((hi - lo) / lo);
    }
    c.at_most("P2 spread of the J of rings 2-4", p2, 0.01);
    // P4.
    let fit = opening(&s, 0.1 * A, 0.4 * A).unwrap();
    println!("P4 fit: {fit:?}");
    c.at_most(
        "P4 K from the opening against the secant form",
        rel(fit.k, k_ref()),
        0.05,
    );
    // P6.
    let r3 = &rings[2];
    c.at_most(
        "P6 the two stations' ring-3 J",
        (r3[0].j - r3[1].j).abs() / r3[0].j,
        1e-10,
    );
    // N1.
    let reversed = CrackFront::straight(s.plate.front.clone(), [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]);
    let rev = j_integral(&s.model, &s.u, &reversed, 3).unwrap();
    let mut n1: f64 = 0.0;
    for (a, b) in rev.iter().zip(r3) {
        n1 = n1.max((a.j + b.j).abs() / b.j.abs());
    }
    c.at_most("N1 reversed advance gives -J", n1, 1e-12);
    // N2 (printed).
    println!(
        "N2 ring-1 J {:.6e} against ring-3 J {:.6e}: relative difference {:.4e} (printed, no limit)",
        rings[0][0].j,
        r3[0].j,
        rel(rings[0][0].j, r3[0].j)
    );
    // R2.
    let edge = s.plate.columns_to_edge;
    let beyond = j_integral(&s.model, &s.u, &s.plate.crack_front(), edge + 1);
    let (ok, detail) = refused(&beyond, "ring", "reaches the boundary");
    c.holds(&format!("R2a ring {} refused", edge + 1), ok, &detail);
    let last = j_integral(&s.model, &s.u, &s.plate.crack_front(), edge);
    let ok = last.as_ref().is_ok_and(|v| v.iter().all(|x| x.j.is_finite()));
    c.holds(
        &format!("R2b ring {edge} accepted"),
        ok,
        &format!("{:?}", last.as_ref().map(|v| v.iter().map(|x| x.j).collect::<Vec<_>>())),
    );
    c.finish();
}

#[test]
fn p3_energy_difference() {
    let mut c = Checks::new();
    let s = solve_plate(A, Loading::Tension);
    let r3 = ring(&s, 3);
    let j3 = 0.5 * (r3[0].j + r3[1].j);
    let minus = solve_plate(0.98 * A, Loading::Tension);
    let plus = solve_plate(1.02 * A, Loading::Tension);
    let t = 0.05 * A;
    let (wm, wp) = (work(&minus), work(&plus));
    let j_e = (wp - wm) / (4.0 * t * 0.04 * A);
    println!("P3: f.u(0.98 a) {wm:.12e}, f.u(1.02 a) {wp:.12e}, J_E {j_e:.6e}, ring-3 J {j3:.6e}");
    c.at_most("P3 J against the energy difference", rel(j_e, j3), 0.02);
    c.finish();
}

#[test]
fn p5_shear_opening() {
    let mut c = Checks::new();
    let tension = solve_plate(A, Loading::Tension);
    let kt = opening(&tension, 0.1 * A, 0.4 * A).unwrap();
    let shear = solve_plate(A, Loading::Shear);
    let ks = opening(&shear, 0.1 * A, 0.4 * A).unwrap();
    let js = ring(&shear, 3);
    println!(
        "P5: tension K_0 {:.6e}, shear K_0 {:.6e}; shear ring-3 J {:.6e} (K from it {:.6e}, printed, no limit)",
        kt.k,
        ks.k,
        js[0].j,
        k_of(js[0].j.max(0.0))
    );
    c.at_most(
        "P5 shear mode-I factor from the opening / tension value",
        ks.k.abs() / kt.k.abs(),
        1e-8,
    );
    c.finish();
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

#[test]
fn r1_front_not_straight() {
    let mut c = Checks::new();
    let p = plate(A);
    let t = 0.05 * A;
    for (name, shift, expect_refused) in [("R1a", 1e-4 * t, true), ("R1b", 1e-8 * t, false)] {
        let mut nodes = p.mesh.nodes().to_vec();
        nodes[p.front[1]][0] += shift;
        let model = plate_model(rebuild(&p.mesh, nodes));
        let zero = vec![0.0; model.dof_map().n_equations()];
        let r = j_integral(&model, &zero, &p.crack_front(), 3);
        if expect_refused {
            let (ok, detail) = refused(&r, "crack front", "not straight");
            c.holds(
                &format!("{name} middle front node moved by {shift:e}: refused"),
                ok,
                &detail,
            );
        } else {
            let ok = r.as_ref().is_ok_and(|v| v.iter().all(|x| x.j.is_finite()));
            c.holds(
                &format!("{name} middle front node moved by {shift:e}: accepted"),
                ok,
                &format!("{:?}", r.as_ref().map(|v| v.len()).map_err(|e| e.to_string())),
            );
        }
    }
    c.finish();
}

#[test]
fn r3_refusals() {
    let mut c = Checks::new();
    let p = plate(A);
    let model = plate_model(p.mesh.clone());
    let zero = vec![0.0; model.dof_map().n_equations()];
    let f = &p.front;
    let n = [0.0, 1.0, 0.0];
    let e = [1.0, 0.0, 0.0];
    let rows: Vec<(&str, rcfd_fem::Result<Vec<FrontValue>>, &str, &str)> = vec![
        (
            "ring 0",
            j_integral(&model, &zero, &p.crack_front(), 0),
            "ring",
            "ring 0",
        ),
        (
            "front of 2 nodes",
            j_integral(&model, &zero, &CrackFront::straight(vec![f[0], f[2]], n, e), 3),
            "crack front",
            "odd number",
        ),
        (
            "front node twice",
            j_integral(&model, &zero, &CrackFront::straight(vec![f[0], f[1], f[0]], n, e), 3),
            "crack front",
            "twice",
        ),
        (
            "advance not normal to the front",
            j_integral(
                &model,
                &zero,
                &CrackFront::straight(f.clone(), n, [0.5f64.sqrt(), 0.0, 0.5f64.sqrt()]),
                3,
            ),
            "crack front",
            "advance",
        ),
    ];
    for (name, r, key, text) in rows {
        let (ok, detail) = refused(&r, key, text);
        c.holds(&format!("R3 {name}"), ok, &detail);
    }
    let (ok, detail) = refused(
        &opening_k(&model, &zero, &p.crack_front(), &p.pairs, 0.402 * A, 0.403 * A),
        "opening",
        "at least 2",
    );
    c.holds("R3 opening interval with fewer than 2 points", ok, &detail);
    let (ok, detail) = refused(
        &opening_k(&model, &zero, &p.crack_front(), &p.pairs, 0.3 * A, 0.3 * A),
        "opening",
        "empty",
    );
    c.holds("R3 opening r_min >= r_max", ok, &detail);
    let dk = |a: f64| 1.12 * 100.0 * (PI * a).sqrt();
    for (name, r) in [
        ("a1 <= a0", paris_cycles(1e-11, 3.0, 1e-2, 1e-3, dk)),
        ("C <= 0", paris_cycles(0.0, 3.0, 1e-3, 1e-2, dk)),
        ("m <= 0", paris_cycles(1e-11, 0.0, 1e-3, 1e-2, dk)),
        ("Delta K = 0", paris_cycles(1e-11, 3.0, 1e-3, 1e-2, |_| 0.0)),
    ] {
        let (ok, detail) = refused(&r, "paris", "");
        c.holds(&format!("R3 paris {name}"), ok, &detail);
    }
    let (ok, detail) = refused(&plane_strain_k(-1.0, E_MOD, NU), "j", "non-negative");
    c.holds("R3 plane_strain_k of a negative J", ok, &detail);
    let mut g = plate_model(p.mesh.clone());
    g.add_load(Load::gravity([0.0, -9.81, 0.0])).unwrap();
    let (ok, detail) = refused(&j_integral(&g, &zero, &p.crack_front(), 3), "load", "body-force");
    c.holds("R3 a model with gravity", ok, &detail);
    c.finish();
}

// --------------------------------------------------------------------------------------------------------------- Paris

#[test]
fn g1_to_g3_paris() {
    let mut c = Checks::new();
    let (cc, ds, d, a0, a1) = (1e-11, 100.0, 1.12, 1e-3, 1e-2);
    let dk = |a: f64| d * ds * (PI * a).sqrt();
    let k = |m: f64| cc * (d * ds * PI.sqrt()).powf(m);
    let n3 = paris_cycles(cc, 3.0, a0, a1, dk).unwrap();
    let c3 = 2.0 * (a0.powf(-0.5) - a1.powf(-0.5)) / k(3.0);
    println!("G1: {n3:.12e} against {c3:.12e}");
    c.at_most("G1 m = 3 against the closed form", rel(n3, c3), 1e-6);
    let n2 = paris_cycles(cc, 2.0, a0, a1, dk).unwrap();
    let c2 = (a1 / a0).ln() / k(2.0);
    println!("G2: {n2:.12e} against {c2:.12e}");
    c.at_most("G2 m = 2 against the logarithm", rel(n2, c2), 1e-6);
    let n3b = paris_cycles(cc, 3.0, a0, a1, |a| dk(a) * (1.0 + a / a1)).unwrap();
    let anti = |x: f64| {
        let q = 1.0 + x * x;
        -1.0 / x
            - x.atan()
            - (x / (2.0 * q) + x.atan() / 2.0)
            - (x / (4.0 * q * q) + 3.0 * x / (8.0 * q) + 3.0 * x.atan() / 8.0)
    };
    let c3b = 2.0 / (k(3.0) * a1.sqrt()) * (anti(1.0) - anti((a0 / a1).sqrt()));
    println!("G3: {n3b:.12e} against {c3b:.12e}");
    c.at_most("G3 m = 3 with the factor (1 + a/a1)", rel(n3b, c3b), 1e-6);
    c.finish();
}

// --------------------------------------------------------------------------------------------------------------- penny

fn penny_model(cyl: &CrackedCylinder) -> (SolidModel, Vec<NodalForceAt>) {
    let loads = traction_loads(&cyl.mesh, "top", [0.0, 0.0, SIGMA]).unwrap();
    let mut m = SolidModel::new(cyl.mesh.clone());
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("ligament", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("y0", 1, 0.0)).unwrap();
    (m, loads)
}

fn penny_ref() -> f64 {
    2.0 * SIGMA * (A / PI).sqrt()
}

fn coarse_penny() -> CrackedCylinder {
    penny_crack_cylinder(&PennyCrack::new(
        A,
        10.0 * A,
        10.0 * A,
        A / 8.0,
        2,
        4,
        0.25 * A,
        1,
        0.5 * A,
        1.5,
        1.6,
    ))
    .unwrap()
}

#[test]
fn pf_r4_coarse_penny() {
    let mut c = Checks::new();
    let cyl = coarse_penny();
    let (model, loads) = penny_model(&cyl);
    println!(
        "PF: {} nodes, {} elements, K_ref {:.6e}",
        cyl.mesh.nodes().len(),
        cyl.mesh.n_elements(),
        penny_ref()
    );
    let res = solve_static_with(&model, &[], &loads, &StaticOptions::with_solver(StaticSolver::Direct)).unwrap();
    let u = res.displacement_values();
    let v = j_integral(&model, u, &cyl.crack_front(), 2).unwrap();
    let mut pf: f64 = 0.0;
    for x in &v {
        println!(
            "PF station {}: J {:.6e}, K {:.6e}, K/K_ref - 1 = {:+.4e}",
            x.station,
            x.j,
            k_of(x.j),
            k_of(x.j) / penny_ref() - 1.0
        );
        pf = pf.max(rel(k_of(x.j), penny_ref()));
    }
    c.at_most("PF coarse penny, ring 2, every station", pf, 0.10);
    // R4.
    let n = [0.0, 0.0, 1.0];
    let (ok, detail) = refused(
        &j_integral(
            &model,
            u,
            &CrackFront::straight(cyl.front.clone(), n, [1.0, 0.0, 0.0]),
            2,
        ),
        "crack front",
        "not straight",
    );
    c.holds("R4a the penny front given as straight", ok, &detail);
    let moved = CrackFront::circular(cyl.front.clone(), n, [1e-4 * A, 0.0, 0.0]).half_symmetric();
    let (ok, detail) = refused(&j_integral(&model, u, &moved, 2), "crack front", "circle");
    c.holds("R4b centre moved by 1e-4 a: refused", ok, &detail);
    let near = CrackFront::circular(cyl.front.clone(), n, [1e-8 * A, 0.0, 0.0]).half_symmetric();
    let r = j_integral(&model, u, &near, 2);
    let ok = r.as_ref().is_ok_and(|v| v.iter().all(|x| x.j.is_finite()));
    c.holds(
        "R4b centre moved by 1e-8 a: accepted",
        ok,
        &format!("{:?}", r.as_ref().map(|v| v.len()).map_err(|e| e.to_string())),
    );
    c.finish();
}

#[test]
#[ignore = "heavy record of ADR 0547 (PH1 to PH3): about 96 000 unknowns, supernodal factor; run once alone"]
fn ph1_to_ph3_penny_heavy() {
    let mut c = Checks::new();
    let cyl = penny_crack_cylinder(&PennyCrack::new(
        A,
        10.0 * A,
        10.0 * A,
        A / 32.0,
        4,
        12,
        0.25 * A,
        2,
        0.5 * A,
        1.25,
        1.4,
    ))
    .unwrap();
    let (model, loads) = penny_model(&cyl);
    println!(
        "PH: {} nodes, {} elements, {} unknowns, K_ref {:.6e}",
        cyl.mesh.nodes().len(),
        cyl.mesh.n_elements(),
        model.dof_map().n_equations(),
        penny_ref()
    );
    let start = std::time::Instant::now();
    let res = solve_static_with(
        &model,
        &[],
        &loads,
        &StaticOptions::with_solver(StaticSolver::SupernodalDirect),
    )
    .unwrap();
    println!("PH solve {:.1} s", start.elapsed().as_secs_f64());
    let u = res.displacement_values();
    let mut per_ring = Vec::new();
    for k in 2..=4 {
        let v = j_integral(&model, u, &cyl.crack_front(), k).unwrap();
        let ks: Vec<f64> = v.iter().map(|x| k_of(x.j)).collect();
        for (x, kk) in v.iter().zip(&ks) {
            println!(
                "ring {k} station {}: J {:.6e}, K {:.6e}, K/K_ref - 1 = {:+.4e}",
                x.station,
                x.j,
                kk,
                kk / penny_ref() - 1.0
            );
        }
        per_ring.push(ks);
    }
    let k3 = &per_ring[1];
    let ph1 = k3.iter().map(|&k| rel(k, penny_ref())).fold(0.0, f64::max);
    c.at_most("PH1 ring-3 K at every station against 2 sigma sqrt(a/pi)", ph1, 0.03);
    let mean = k3.iter().sum::<f64>() / k3.len() as f64;
    let (lo, hi) = k3
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &v| (l.min(v), h.max(v)));
    c.at_most("PH2 ring-3 K uniform along the front", (hi - lo) / mean, 0.02);
    for (k, ks) in [(2, &per_ring[0]), (4, &per_ring[2])] {
        let e = ks.iter().map(|&v| rel(v, penny_ref())).fold(0.0, f64::max);
        println!("PH3 ring {k}: largest error {e:.4e} (printed, no limit)");
        c.holds(&format!("F1 PH3 ring {k} finite"), e.is_finite(), "");
    }
    c.finish();
}
