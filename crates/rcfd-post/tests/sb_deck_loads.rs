//! F-384 (ADR 0436, the registration of ADR 0285): loads of the F-237 deck applied to the solid surface. The shell surface of a
//! solid surface set, the PLOAD4 cards as consistent nodal loads (resultant against deck_force_moment, refusals by id), the
//! Lame tube through the F-237 export (uniform wall pressure, plane strain), the reactions under a varying wall pressure
//! against the fluid's force, and the free expansion of a uniform TEMP (checks A1 to A6, K1 to K6, N1 to N5).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::builders::{hex_box, tet_box};
use rcfd_fem::convert::from_mesh;
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidMesh, SolidModel, SolidResult};
use rcfd_io::nastran::Deck;
use rcfd_mesh::gen::{annulus_3d, box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_post::solid_loads::*;
use rcfd_post::structural::*;
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const ALPHA: f64 = 1.2e-5;
const A_IN: f64 = 0.05;
const B_OUT: f64 = 0.1;
const P0: f64 = 1.0e5;
const TOL: f64 = 1e-9;
const ABOUT_TUBE: Vec3 = [0.0, 0.0, 0.5];

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
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
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
    fn err_names(&mut self, name: &str, r: Result<impl std::fmt::Debug, String>, needles: &[&str]) {
        match r {
            Ok(v) => self.holds(name, false, &format!("expected Err naming {needles:?}, got Ok({v:?})")),
            Err(m) => {
                let ok = needles.iter().all(|n| m.contains(n));
                self.holds(name, ok, &format!("Err \"{m}\" (must contain {needles:?})"));
            }
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    vec3::norm(vec3::sub(a, b))
}

fn material() -> Material {
    Material::isotropic_elastic(E_MOD, NU, RHO, ALPHA).unwrap()
}

/// The F-237 tube check's fluid mesh and a wall pressure at the face centres of `outer`.
fn fluid_tube(uniform: bool) -> (Mesh, Vec<f64>) {
    let m = annulus_3d(96, 1, 24, 0.02, 0.05, 1.0, 0.0, Grading::Uniform).unwrap();
    let p = m
        .patch("outer")
        .unwrap()
        .faces()
        .map(|f| {
            let c = m.face_centres()[f];
            if uniform {
                P0
            } else {
                1e5 * (1.0 + 0.5 * c[1].atan2(c[0]).cos() + 0.3 * c[2])
            }
        })
        .collect();
    (m, p)
}

/// A thick tube `a = 0.05`, `b = 0.1` (ADR 0436 D5) as a finite-volume mesh and its Hex8 solid, with the node sets `y0`
/// (`|y| <= 1e-12`) and `x0` (`|x| <= 1e-12`) of the symmetry rollers.
fn solid_tube(n_theta: usize, n_r: usize, n_z: usize, height: f64, z0: f64) -> (Mesh, SolidMesh) {
    let fv = annulus_3d(n_theta, n_r, n_z, A_IN, B_OUT, height, z0, Grading::Expansion(7.0)).unwrap();
    let mut s = from_mesh(&fv).unwrap();
    let y0: Vec<usize> = (0..s.nodes().len())
        .filter(|&n| s.nodes()[n][1].abs() <= 1e-12)
        .collect();
    let x0: Vec<usize> = (0..s.nodes().len())
        .filter(|&n| s.nodes()[n][0].abs() <= 1e-12)
        .collect();
    s.add_node_set("y0", y0).unwrap();
    s.add_node_set("x0", x0).unwrap();
    (fv, s)
}

fn tube_model(solid: &SolidMesh, plane_strain: bool) -> SolidModel {
    let mut m = SolidModel::new(solid.clone());
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement(
        if plane_strain { "all" } else { "zmin" },
        2,
        0.0,
    ))
    .unwrap();
    m.add_constraint(Constraint::displacement("y0", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    m
}

/// F-237's export onto the shell of a surface set, through text: weights, mapping, PLOAD4 deck, written and parsed.
fn export(p: &[f64], fluid_rects: &[ParamRect], shell: &ShellSurface, period: Option<f64>, sid: i64) -> Deck {
    let w = overlap_weights(fluid_rects, &shell.rects, period).unwrap();
    let pe = map_face_field(&w, p).unwrap();
    let deck = shell_deck(shell, &pe, sid).unwrap();
    Deck::parse(&deck.to_string_checked().unwrap()).unwrap()
}

/// The deck's load scale `S_d = sum |p_e| |A_e|` and length `L_d = max |x_g - about|`.
fn deck_scales(deck: &Deck, sid: i64, about: Vec3) -> (f64, f64) {
    let pos: std::collections::HashMap<i64, Vec3> = deck.grids.iter().cloned().collect();
    let mut s = 0.0;
    for l in deck.pload4.iter().filter(|l| l.sid == sid) {
        let e = deck.elements.iter().find(|e| e.id == l.eid).unwrap();
        let x: Vec<Vec3> = e.nodes.iter().map(|g| pos[g]).collect();
        let a = vec3::scale(vec3::cross(vec3::sub(x[2], x[0]), vec3::sub(x[3], x[1])), 0.5);
        s += l.p[0].abs() * vec3::norm(a);
    }
    let l = deck.grids.iter().map(|(_, x)| dist(*x, about)).fold(0.0, f64::max);
    (s, l)
}

/// A1 measures: `|F_n - F_d| / S_d`, `|M_n - M_d| / (S_d L_d)`, `|F_n - F_d| / |F_d|`.
fn resultant_errors(solid: &SolidMesh, forces: &[NodalForceAt], deck: &Deck, sid: i64, about: Vec3) -> [f64; 3] {
    let (f_n, m_n) = nodal_force_moment(solid, forces, about);
    let (f_d, m_d) = deck_force_moment(deck, sid, about).unwrap();
    let (s, l) = deck_scales(deck, sid, about);
    println!("  F_nodal {f_n:?} F_deck {f_d:?} M_nodal {m_n:?} M_deck {m_d:?} S_d {s:.6e} L_d {l:.4}");
    [
        dist(f_n, f_d) / s,
        dist(m_n, m_d) / (s * l),
        dist(f_n, f_d) / vec3::norm(f_d),
    ]
}

fn lame_u_bore() -> f64 {
    let a = P0 * A_IN * A_IN / (B_OUT * B_OUT - A_IN * A_IN);
    let b = a * B_OUT * B_OUT;
    (1.0 + NU) / E_MOD * ((1.0 - 2.0 * NU) * a * A_IN + b / A_IN)
}

fn radial(x: Vec3, u: &[f64]) -> f64 {
    (u[0] * x[0] + u[1] * x[1]) / (x[0] * x[0] + x[1] * x[1]).sqrt()
}

/// Largest bore `|u_r / u_r(a) - 1|`.
fn bore_ur_error(solid: &SolidMesh, r: &SolidResult) -> f64 {
    let ua = lame_u_bore();
    solid
        .node_set("inner")
        .unwrap()
        .iter()
        .map(|&n| (radial(solid.nodes()[n], r.displacement(n)) / ua - 1.0).abs())
        .fold(0.0, f64::max)
}

/// Sum of the reactions (force) and their moment about `about`.
fn reaction_resultant(solid: &SolidMesh, r: &SolidResult, about: Vec3) -> (Vec3, Vec3) {
    let reac = r.reactions().unwrap();
    let mut per_node = vec![[0.0; 3]; solid.nodes().len()];
    for (eq, &v) in reac.iter().enumerate() {
        let (n, c) = r.dofs().node_component(eq).unwrap();
        per_node[n][c] += v;
    }
    let (mut f, mut m) = ([0.0; 3], [0.0; 3]);
    for (n, rn) in per_node.iter().enumerate() {
        f = vec3::add(f, *rn);
        m = vec3::add(m, vec3::cross(vec3::sub(solid.nodes()[n], about), *rn));
    }
    (f, m)
}

/// A1 plane: fluid box `zmax`, `p = 1 + 2x + 3y`, onto the `zmin` set of a Hex8 box above it.
#[test]
fn a1_plane_resultant_equals_deck() {
    let mut c = Checks::new();
    let fluid = box_mesh([8, 8, 1], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let rects = patch_rects_plane(&fluid, "zmax", [0, 1]).unwrap();
    let p: Vec<f64> = fluid
        .patch("zmax")
        .unwrap()
        .faces()
        .map(|f| {
            let x = fluid.face_centres()[f];
            1.0 + 2.0 * x[0] + 3.0 * x[1]
        })
        .collect();
    let solid = hex_box([0.0, 0.0, 1.0], [1.0, 1.0, 0.2], [5, 7, 2]).unwrap();
    let shell = shell_from_surface_set(&solid, "zmin", SurfaceParam::Plane { axes: [0, 1] }).unwrap();
    let deck = export(&p, &rects, &shell.surface, None, 1);
    let forces = deck_pressure_loads(&solid, "zmin", &deck, 1, TOL).unwrap();
    let e = resultant_errors(&solid, &forces, &deck, 1, [0.5, 0.5, 1.0]);
    c.at_most("A1 plane |F_n - F_d|/S_d", e[0], 1e-12);
    c.at_most("A1 plane |M_n - M_d|/(S_d L_d)", e[1], 1e-12);
    c.at_most("A1b plane |F_n - F_d|/|F_d|", e[2], 1e-12);
    c.finish();
}

/// The uniform tube U: A1, A3, A4, K3, K4, N1, N4.
#[test]
fn tube_uniform_lame() {
    let mut c = Checks::new();
    let (fluid, p) = fluid_tube(true);
    let rects = patch_rects_cylinder(&fluid, "outer").unwrap();
    let (_fv, solid) = solid_tube(128, 32, 1, 0.01, 0.5);
    println!("U: {} nodes", solid.nodes().len());
    let shell = shell_from_surface_set(&solid, "inner", SurfaceParam::CylinderZ).unwrap();
    let deck = export(&p, &rects, &shell.surface, Some(2.0 * PI), 2);
    let exact = deck.pload4.iter().all(|l| l.p[0] == P0);
    c.holds(
        "K4 every element pressure equals p0 exactly",
        exact,
        &format!("{} cards", deck.pload4.len()),
    );
    let forces = deck_pressure_loads(&solid, "inner", &deck, 2, TOL).unwrap();
    let e = resultant_errors(&solid, &forces, &deck, 2, ABOUT_TUBE);
    c.at_most("A1 tube U |F_n - F_d|/S_d", e[0], 1e-12);
    c.at_most("A1 tube U |M_n - M_d|/(S_d L_d)", e[1], 1e-12);

    // K3: against Load::pressure on the same set
    let mut with_p = tube_model(&solid, true);
    with_p.add_load(Load::pressure("inner", P0)).unwrap();
    let f_ref = nodal_load_vector(&with_p, &[]).unwrap();
    let f_deck = nodal_load_vector(&tube_model(&solid, true), &forces).unwrap();
    let big = f_ref.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let diff = f_ref.iter().zip(&f_deck).fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
    c.at_most("K3 nodal loads vs Load::pressure max|df|/max|f|", diff / big, 1e-14);

    // A3, A4
    let model = tube_model(&solid, true);
    let r = solve_static_with(&model, &[], &forces, &StaticOptions::default()).unwrap();
    c.at_most(
        "A3 bore |u_r/u_r(a) - 1| (plane strain)",
        bore_ur_error(&solid, &r),
        1e-2,
    );
    let field = IntegrationPointField::from_result(&model, &r).unwrap();
    let avg = nodal_average(&model, &field, field.stress(), None).unwrap();
    let hoop_ref = 5.0 * P0 / 3.0;
    let mut hoop = 0.0f64;
    for &n in solid.node_set("inner").unwrap() {
        let x = solid.nodes()[n];
        let th = x[1].atan2(x[0]);
        let (tx, ty) = (-th.sin(), th.cos());
        let s = avg[n].unwrap();
        let st = s[0] * tx * tx + s[1] * ty * ty + 2.0 * s[3] * tx * ty;
        hoop = hoop.max((st / hoop_ref - 1.0).abs());
    }
    c.at_most("A4 bore |sigma_theta/(5p/3) - 1|", hoop, 2e-2);

    // N1: plane stress
    let ps = tube_model(&solid, false);
    let r1 = solve_static_with(&ps, &[], &forces, &StaticOptions::default()).unwrap();
    c.above(
        "N1 plane stress bore error against plane-strain Lame",
        bore_ur_error(&solid, &r1),
        2e-2,
    );
    c.finish();
}

/// The varying tube V: K1, A1, A1b, K2, A5, K6, A2 (a) to (c), N3, N5 (corner pressures).
#[test]
fn tube_varying_reactions() {
    let mut c = Checks::new();
    let (fluid, p) = fluid_tube(false);
    let rects = patch_rects_cylinder(&fluid, "outer").unwrap();
    let (_fv, solid) = solid_tube(64, 4, 20, 1.0, 0.0);
    println!("V: {} nodes", solid.nodes().len());
    let shell = shell_from_surface_set(&solid, "inner", SurfaceParam::CylinderZ).unwrap();

    // K1
    let reference = shell_tube(0.05, 0.0, 1.0, 64, 20).unwrap();
    let mut used = vec![false; reference.nodes.len()];
    let mut worst = 0.0f64;
    let mut all_matched = shell.surface.nodes.len() == reference.nodes.len();
    for x in &shell.surface.nodes {
        let (k, d) = reference
            .nodes
            .iter()
            .enumerate()
            .map(|(k, y)| (k, dist(*x, *y)))
            .fold((usize::MAX, f64::INFINITY), |b, q| if q.1 < b.1 { q } else { b });
        worst = worst.max(d);
        if k == usize::MAX || used[k] {
            all_matched = false;
        } else {
            used[k] = true;
        }
    }
    let inward = shell.surface.quads.iter().all(|q| {
        let x: Vec<Vec3> = q.iter().map(|&i| shell.surface.nodes[i]).collect();
        let a = vec3::scale(vec3::cross(vec3::sub(x[2], x[0]), vec3::sub(x[3], x[1])), 0.5);
        let ctr = vec3::scale(vec3::add(vec3::add(x[0], x[1]), vec3::add(x[2], x[3])), 0.25);
        vec3::dot(a, [ctr[0], ctr[1], 0.0]) < 0.0
    });
    c.holds(
        "K1 shell nodes = shell_tube(0.05, 0, 1, 64, 20) nodes",
        all_matched && worst <= 1e-14 && shell.surface.quads.len() == 1280,
        &format!(
            "{} nodes, {} quads, largest distance {worst:.3e} m (limit 1e-14)",
            shell.surface.nodes.len(),
            shell.surface.quads.len()
        ),
    );
    c.holds("K1 every quad normal points to the axis", inward, "");

    let deck = export(&p, &rects, &shell.surface, Some(2.0 * PI), 3);
    let forces = deck_pressure_loads(&solid, "inner", &deck, 3, TOL).unwrap();
    let e = resultant_errors(&solid, &forces, &deck, 3, ABOUT_TUBE);
    c.at_most("A1 tube V |F_n - F_d|/S_d", e[0], 1e-12);
    c.at_most("A1 tube V |M_n - M_d|/(S_d L_d)", e[1], 1e-12);
    c.at_most("A1b tube V |F_n - F_d|/|F_d|", e[2], 1e-12);

    // K2: the deck against the fluid
    let (f_f, m_f) = fluid_force_moment(&fluid, "outer", &p, ABOUT_TUBE).unwrap();
    let s: f64 = p
        .iter()
        .zip(fluid.patch("outer").unwrap().faces())
        .map(|(v, f)| v.abs() * fluid.face_area(f))
        .sum();
    let (f_d, m_d) = deck_force_moment(&deck, 3, ABOUT_TUBE).unwrap();
    println!("  F_fluid {f_f:?} M_fluid {m_f:?} S {s:.6e}");
    c.at_most("K2 deck vs fluid |F_d - F_f|/S", dist(f_d, f_f) / s, 1e-3);
    c.at_most("K2 deck vs fluid |M_d - M_f|/(S R)", dist(m_d, m_f) / (s * 0.05), 5e-3);

    // A5, K6, K2 (reactions' moment)
    let model = tube_model(&solid, true);
    let r = solve_static_with(&model, &[], &forces, &StaticOptions::default()).unwrap();
    let (f_r, m_r) = reaction_resultant(&solid, &r, ABOUT_TUBE);
    let (f_n, _) = nodal_force_moment(&solid, &forces, ABOUT_TUBE);
    println!("  F_reactions {f_r:?} M_reactions {m_r:?}");
    c.at_most("A5 |F_R + F_fluid|/S", vec3::norm(vec3::add(f_r, f_f)) / s, 1e-3);
    c.at_most(
        "K6 equilibrium |F_R + F_n|/S",
        vec3::norm(vec3::add(f_r, f_n)) / s,
        1e-9,
    );
    c.at_most(
        "K2 reactions' moment |M_R + M_f|/(S R)",
        vec3::norm(vec3::add(m_r, m_f)) / (s * 0.05),
        5e-3,
    );

    // A2 (a), (b): a grid of the first loaded element moved
    let e1 = deck
        .elements
        .iter()
        .find(|e| e.id == deck.pload4[0].eid)
        .unwrap()
        .clone();
    let g = e1.nodes[0];
    for (name, shift, refuse) in [
        ("A2(a) grid moved 2 tol", 2.0 * TOL, true),
        ("A2(b) grid moved tol/2", 0.5 * TOL, false),
    ] {
        let mut d = deck.clone();
        let k = d.grids.iter().position(|(id, _)| *id == g).unwrap();
        d.grids[k].1[0] += shift;
        let res = deck_pressure_loads(&solid, "inner", &d, 3, TOL).map(|f| f.len());
        if refuse {
            c.err_names(name, res, &[&format!("deck grid {g} ")]);
        } else {
            c.holds(name, res.is_ok(), &format!("{res:?}"));
        }
    }
    // A2 (c): the fourth grid replaced by a bore grid not on the element's face
    {
        let mut d = deck.clone();
        let other = d.elements.iter().find(|e| e.id == e1.id + 200).unwrap().nodes[0];
        let k = d.elements.iter().position(|e| e.id == e1.id).unwrap();
        d.elements[k].nodes[3] = other;
        let res = deck_pressure_loads(&solid, "inner", &d, 3, TOL).map(|f| f.len());
        c.err_names(
            "A2(c) element matching no face",
            res,
            &[&format!("deck element {}:", e1.id), "not the corners"],
        );
    }
    // N5: corner-varying pressure
    {
        let mut d = deck.clone();
        d.pload4[0].p = [1.0, 2.0, 3.0, 4.0];
        let res = deck_pressure_loads(&solid, "inner", &d, 3, TOL).map(|f| f.len());
        c.err_names(
            "N5 corner-varying PLOAD4",
            res,
            &[&format!("deck element {}:", d.pload4[0].eid), "corner-varying"],
        );
    }
    // N3: the shell flipped (normal into the solid)
    {
        let mut flipped = shell.surface.clone();
        for q in &mut flipped.quads {
            *q = [q[0], q[3], q[2], q[1]];
        }
        let d = export(&p, &rects, &flipped, Some(2.0 * PI), 3);
        let res = deck_pressure_loads(&solid, "inner", &d, 3, TOL).map(|f| f.len());
        c.err_names("N3 flipped shell", res, &["deck element ", "into the solid"]);
    }
    c.finish();
}

/// The temperature T: A6, K5, A2 (d) to (f), N4.
#[test]
fn tube_temperature_free_expansion() {
    let mut c = Checks::new();
    let (fv, solid) = solid_tube(128, 32, 1, 0.01, 0.5);
    let n = solid.nodes().len();
    let (t_ref, t) = (293.15, 393.15);
    let deck = temperature_deck(&fv, &vec![t; n], 5).unwrap();
    let deck = Deck::parse(&deck.to_string_checked().unwrap()).unwrap();
    let load = deck_temperature_load(&solid, &deck, 5, t_ref, TOL).unwrap();
    let mut model = tube_model(&solid, false);
    model.add_load(load.clone()).unwrap();
    let r = solve_static(&model, &StaticOptions::default()).unwrap();
    let dt = t - t_ref;
    let (mut er, mut ez) = (0.0f64, 0.0f64);
    for (k, x) in solid.nodes().iter().enumerate() {
        let u = r.displacement(k);
        let rr = (x[0] * x[0] + x[1] * x[1]).sqrt();
        er = er.max((radial(*x, u) / (ALPHA * dt * rr) - 1.0).abs());
        ez = ez.max((u[2] - ALPHA * dt * (x[2] - 0.5)).abs() / (ALPHA * dt * 0.01));
    }
    c.at_most("A6 |u_r/(alpha dT r) - 1|", er, 1e-6);
    c.at_most("A6 |u_z - alpha dT (z - z0)|/(alpha dT H)", ez, 1e-6);

    // N4: a second temperature field
    let mut twice = model.clone();
    let added = twice.add_load(load.clone());
    let res = added.map_err(|e| e.to_string()).and_then(|_| {
        solve_static(&twice, &StaticOptions::default())
            .map(|_| ())
            .map_err(|e| e.to_string())
    });
    c.err_names("N4 second temperature field", res, &["temperature"]);

    // K5: a linear field, the deck's grids reversed and renumbered
    let lin = |x: Vec3| 300.0 + 1000.0 * x[0] - 500.0 * x[1] + 200.0 * x[2];
    let written: Vec<f64> = solid.nodes().iter().map(|x| lin(*x)).collect();
    let mut d = temperature_deck(&fv, &written, 7).unwrap();
    d.grids.reverse();
    for g in &mut d.grids {
        g.0 += 1000;
    }
    d.temps.reverse();
    for tp in &mut d.temps {
        tp.1 += 1000;
    }
    let d = Deck::parse(&d.to_string_checked().unwrap()).unwrap();
    let ok = match deck_temperature_load(&solid, &d, 7, t_ref, TOL).unwrap() {
        Load::Temperature { nodal, .. } => nodal.iter().zip(&written).all(|(a, b)| a.to_bits() == b.to_bits()),
        _ => false,
    };
    c.holds("K5 TEMP by coordinates, reversed and renumbered: bit-identical", ok, "");

    // A2 (d) to (f)
    {
        let mut d2 = deck.clone();
        d2.grids[0].1[2] += 2.0 * TOL;
        let g = d2.grids[0].0;
        let res = deck_temperature_load(&solid, &d2, 5, t_ref, TOL).map(|_| ());
        c.err_names("A2(d) TEMP grid moved 2 tol", res, &[&format!("deck grid {g} ")]);
    }
    {
        let mut d2 = deck.clone();
        let (_, g, _) = d2.temps.remove(0);
        let node = (g - 1) as usize;
        let res = deck_temperature_load(&solid, &d2, 5, t_ref, TOL).map(|_| ());
        c.err_names("A2(e) TEMP entry missing", res, &[&format!("solid node {node} ")]);
    }
    {
        let mut d2 = deck.clone();
        let first = d2.temps[0];
        d2.temps.push(first);
        let res = deck_temperature_load(&solid, &d2, 5, t_ref, TOL).map(|_| ());
        c.err_names(
            "A2(f) TEMP entry duplicated",
            res,
            &[&format!("deck grid {}:", first.1)],
        );
    }
    c.finish();
}

/// N2 (a warped face) and N5 (a Tri3 surface).
#[test]
fn negative_controls_warped_and_triangles() {
    let mut c = Checks::new();
    let base = hex_box([0.0, 0.0, 1.0], [1.0, 1.0, 0.2], [1, 1, 1]).unwrap();
    let mut nodes = base.nodes().to_vec();
    let k = nodes.iter().position(|x| dist(*x, [1.0, 1.0, 1.0]) < 1e-12).unwrap();
    nodes[k][2] = 1.01;
    let mut warped = SolidMesh::new(nodes).unwrap();
    warped
        .add_block(base.blocks()[0].kind(), base.blocks()[0].connectivity().to_vec())
        .unwrap();
    warped
        .add_surface_set("zmin", base.surface_set("zmin").unwrap().to_vec())
        .unwrap();
    let shell = shell_from_surface_set(&warped, "zmin", SurfaceParam::Plane { axes: [0, 1] }).unwrap();
    let deck = shell_deck(&shell.surface, &[P0], 1).unwrap();
    let deck = Deck::parse(&deck.to_string_checked().unwrap()).unwrap();
    let forces = deck_pressure_loads(&warped, "zmin", &deck, 1, TOL).unwrap();
    let e = resultant_errors(&warped, &forces, &deck, 1, [0.5, 0.5, 1.0]);
    c.at_most("N2 warped face |F_n - F_d|/S_d", e[0], 1e-12);
    c.above("N2 warped face |M_n - M_d|/(S_d L_d)", e[1], 1e-6);

    let tets = tet_box([0.0; 3], [1.0; 3], [2, 2, 2]).unwrap();
    let res = shell_from_surface_set(&tets, "zmin", SurfaceParam::Plane { axes: [0, 1] }).map(|s| s.solid_nodes.len());
    c.err_names("N5 Tri3 surface refused", res, &["face ", "of element ", "3 nodes"]);
    c.finish();
}
