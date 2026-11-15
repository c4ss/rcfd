//! F-665 (ADR 0589): a tangential follower tip force on Beam2 nodes with its load stiffness, and the complex modes of `(K +
//! lambda (K_sigma - K_f)) phi = omega^2 M phi` about a linear pre-stress; Beck's column of Beam2 elements (checks T1 to
//! T11 of the registration).

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam;
use rcfd_fem::builders::hex_box;
use rcfd_fem::follower::{prestressed_modes_full, ComplexModes, FollowerTipForce};
use rcfd_fem::geometric::{prestressed_modes, Prestress};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElementKind, FemError, LinearTerm, Load, Material, SolidMesh, SolidModel,
};
use rcfd_linalg::eigen::{ResidualTest, Shift};

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
const N: usize = 40;
const P_EB: f64 = 20.0509536;
const W_EB: f64 = 121.342510;
/// Arithmetic 1 (method A, the same discretisation) and 2 (method C, the Timoshenko continuum).
const P_A: f64 = 20.033162590116262;
const W_A: f64 = 121.189706;
const P_T: f64 = 20.03314308;

fn column(axis: Vec3, section: BeamSection, orientation: Vec3) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=N)
        .map(|i| {
            let s = i as f64 / N as f64;
            [s * axis[0], s * axis[1], s * axis[2]]
        })
        .collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..N).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, orientation).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..N).collect()).unwrap();
    mesh.add_node_set("all", (0..=N).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![N]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, 0.3, RHO, 0.0).unwrap())
        .unwrap();
    m
}

fn clamp(m: &mut SolidModel) {
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
}

/// A Beck column of D3: the model, the tip force, `E I` of the plane that flutters, `rho A`, the number of modes.
struct Column {
    model: SolidModel,
    force: Vec3,
    ei: f64,
    ra: f64,
    modes: usize,
}

fn planar() -> Column {
    let b = 0.01;
    let s = beam::rectangle_section(b, b, 0.1406 * b.powi(4), 5.0 / 6.0).unwrap();
    let ei = E * s.i_z();
    let mut m = column([1.0, 0.0, 0.0], s, [0.0, 1.0, 0.0]);
    clamp(&mut m);
    for c in [2, 3, 4] {
        m.add_constraint(Constraint::displacement("all", c, 0.0)).unwrap();
    }
    Column {
        model: m,
        force: [-20.0 * ei, 0.0, 0.0],
        ei,
        ra: RHO * s.area(),
        modes: 4,
    }
}

const AXIS: Vec3 = [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0];

fn oblique_unclamped() -> (SolidModel, f64, BeamSection) {
    let (b, h) = (0.02, 0.01);
    let s = beam::rectangle_section(b, h, 0.229 * b * h * h * h, 5.0 / 6.0).unwrap();
    (column(AXIS, s, [0.0, 0.0, 1.0]), E * s.i_y(), s)
}

fn oblique() -> Column {
    let (mut m, ei, s) = oblique_unclamped();
    clamp(&mut m);
    let p = 20.0 * ei;
    Column {
        model: m,
        force: [-p * AXIS[0], -p * AXIS[1], -p * AXIS[2]],
        ei,
        ra: RHO * s.area(),
        modes: 6,
    }
}

/// The pre-stress of the linear static solve with the dead tip force.
fn prestress(c: &Column) -> Prestress {
    let mut md = c.model.clone();
    md.add_load(Load::nodal_force("xl", c.force)).unwrap();
    let r = solve_static(&md, &StaticOptions::default()).unwrap();
    Prestress::from_static(&md, &r).unwrap()
}

fn is_flutter(r: &ComplexModes) -> bool {
    r.eigenvalues.iter().any(|z| z.im.abs() > 1e-6 * z.abs())
}

/// The bisection of D3: `(p_meas, W_meas, flutter(16), flutter(24), the largest backward error of every evaluation)`.
fn bisect(c: &Column) -> (f64, f64, bool, bool, f64) {
    let pre = prestress(c);
    let fp = FollowerTipForce::new(&c.model, "xl", c.force).unwrap();
    let mut eta: f64 = 0.0;
    let mut eval = |p: f64| -> Option<ComplexModes> {
        match prestressed_modes_full(&c.model, &pre, &[&fp], p / 20.0, &ModalOptions::new(c.modes)) {
            Ok(r) => {
                for e in &r.backward_errors {
                    eta = eta.max(*e);
                }
                Some(r)
            }
            Err(e) => {
                println!("p = {p}: refused {e}");
                None
            }
        }
    };
    let lo_r = eval(16.0);
    let hi_r = eval(24.0);
    let f16 = lo_r.as_ref().is_some_and(is_flutter);
    let f24 = hi_r.as_ref().is_some_and(is_flutter);
    if let Some(r) = &lo_r {
        println!("p = 16: {:?}", r.eigenvalues);
    }
    if let Some(r) = &hi_r {
        println!("p = 24: {:?}", r.eigenvalues);
    }
    let (mut lo, mut hi) = (16.0, 24.0);
    let mut upper = hi_r;
    for _ in 0..30 {
        let mid = 0.5 * (lo + hi);
        let r = eval(mid);
        if r.as_ref().is_some_and(is_flutter) {
            hi = mid;
            upper = r;
        } else {
            lo = mid;
        }
    }
    let scale = c.ra / c.ei;
    let w = upper
        .and_then(|r| {
            r.eigenvalues
                .iter()
                .find(|z| z.im.abs() > 1e-6 * z.abs())
                .map(|z| z.re * scale)
        })
        .unwrap_or(f64::NAN);
    (0.5 * (lo + hi), w, f16, f24, eta)
}

#[test]
fn t1_to_t6_planar_beck_column() {
    let mut c = Checks::new();
    let col = planar();
    let (p, w, f16, f24, eta) = bisect(&col);
    println!("planar: p_meas = {p:.12}, W_meas = {w:.9}");
    c.at_most("T1 |p_meas/20.0509536 - 1|", (p / P_EB - 1.0).abs(), 2e-3);
    c.at_most("T2 |p_meas/p_A - 1|", (p / P_A - 1.0).abs(), 1e-8);
    c.at_most("T3 |p_meas/p_T - 1|", (p / P_T - 1.0).abs(), 1e-5);
    c.at_most("T4 |W_meas/121.342510 - 1|", (w / W_EB - 1.0).abs(), 3e-3);
    c.at_most("T4 |W_meas/W_A - 1|", (w / W_A - 1.0).abs(), 1e-6);
    c.holds(
        "T5 planar bracket",
        !f16 && f24,
        &format!("flutter(16) = {f16}, flutter(24) = {f24}"),
    );
    c.at_most("T6 planar max backward error", eta, 2e-10);
    c.finish();
}

#[test]
fn t5_t6_t7_oblique_beck_column() {
    let mut c = Checks::new();
    let (pp, ..) = bisect(&planar());
    let (p, w, f16, f24, eta) = bisect(&oblique());
    println!("oblique: p_3D = {p:.12}, W = {w:.9}; planar p_meas = {pp:.12}");
    c.holds(
        "T5 oblique bracket",
        !f16 && f24,
        &format!("flutter(16) = {f16}, flutter(24) = {f24}"),
    );
    c.at_most("T6 oblique max backward error", eta, 2e-10);
    c.at_most("T7 |p_3D/p_meas - 1|", (p / pp - 1.0).abs(), 1e-8);
    c.finish();
}

fn splitmix(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    2.0 * ((z >> 11) as f64 / (1u64 << 53) as f64) - 1.0
}

#[test]
fn t8_load_stiffness() {
    let mut c = Checks::new();
    let col = oblique();
    let n = col.model.dof_map().n_equations();
    let fp = FollowerTipForce::new(&col.model, "xl", col.force).unwrap();
    let f_norm = col.force.iter().map(|v| v * v).sum::<f64>().sqrt();
    let lf = 0.7;
    let kf = fp.load_stiffness(lf);
    let mut state = 0x0589_F665_u64;
    let mut worst: f64 = 0.0;
    let f0 = fp.force(&vec![0.0; n], lf).unwrap();
    for _ in 0..3 {
        let u: Vec<f64> = (0..n).map(|_| splitmix(&mut state)).collect();
        let fu = fp.force(&u, lf).unwrap();
        let mut ku = vec![0.0; n];
        for &(r, cc, v) in &kf {
            ku[r] += v * u[cc];
        }
        let umax = u.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
        for r in 0..n {
            worst = worst.max((fu[r] - f0[r] - ku[r]).abs() / (f_norm * umax));
        }
    }
    c.at_most("T8(i) max |f(u) - f(0) - K_f u| / (|F| max|u|)", worst, 1e-14);
    let dofs = col.model.dof_map();
    let f1 = fp.force(&vec![0.0; n], 1.0).unwrap();
    let mut exact = true;
    for (eq, v) in f1.iter().enumerate() {
        let (node, comp) = dofs.node_component(eq).unwrap();
        let want = if node == N && comp < 3 { col.force[comp] } else { 0.0 };
        exact &= v.to_bits() == want.to_bits();
    }
    c.holds(
        "T8(ii) force(0, 1) = F on the tip translations, 0 elsewhere",
        exact,
        "bitwise",
    );
    let pl = planar();
    let fpl = FollowerTipForce::new(&pl.model, "xl", pl.force).unwrap();
    let d = pl.model.dof_map();
    let (ry, ctz) = (d.equation(N, 1).unwrap(), d.equation(N, 5).unwrap());
    let entry: f64 = fpl
        .load_stiffness(1.0)
        .iter()
        .filter(|t| t.0 == ry && t.1 == ctz)
        .map(|t| t.2)
        .sum();
    c.holds(
        "T8(iii) planar K_f[y, theta_z] = -P_ref",
        entry == pl.force[0],
        &format!("{entry:e} against {:e}", pl.force[0]),
    );
    c.holds(
        "T8(iii) oblique K_f entries",
        kf.len() == 6,
        &format!("{} entries", kf.len()),
    );
    c.finish();
}

#[test]
fn t9_t10_controls() {
    let mut c = Checks::new();
    let col = planar();
    let pre = prestress(&col);
    // T9: the dead tip force at 1.01 p_meas (p_meas of T1, recomputed here).
    let (p, ..) = bisect(&col);
    let r = prestressed_modes_full(&col.model, &pre, &[], 1.01 * p / 20.0, &ModalOptions::new(col.modes)).unwrap();
    let scale = col.ra / col.ei;
    println!(
        "T9 dead load at 1.01 p_meas: {:?}",
        r.eigenvalues
            .iter()
            .map(|z| (z.re * scale, z.im * scale))
            .collect::<Vec<_>>()
    );
    let im = r
        .eigenvalues
        .iter()
        .map(|z| z.im.abs() / z.abs())
        .fold(0.0_f64, f64::max);
    let lowest = r.eigenvalues.iter().map(|z| z.re).fold(f64::INFINITY, f64::min);
    c.holds(
        "T9 dead load: real, lowest Re < 0",
        im <= 1e-8 && lowest < 0.0,
        &format!("max |Im|/|w| = {im:e}, lowest Re = {:.6}", lowest * scale),
    );
    // T10: no follower at p = 1 against the symmetric pre-stressed modes.
    let full = prestressed_modes_full(&col.model, &pre, &[], 0.05, &ModalOptions::new(4)).unwrap();
    let sym = prestressed_modes(
        &col.model,
        &pre.scaled(0.05),
        &ModalOptions::new(4).residual_test(ResidualTest::Backward),
    )
    .unwrap();
    let mut rel: f64 = 0.0;
    let mut imr: f64 = 0.0;
    for (z, s) in full.eigenvalues.iter().zip(&sym.eigenvalues) {
        rel = rel.max((z.re / s - 1.0).abs());
        imr = imr.max(z.im.abs() / z.abs());
    }
    println!("T10 full {:?}, symmetric {:?}", full.eigenvalues, sym.eigenvalues);
    c.at_most("T10 max |w_f/w_s - 1|", rel, 1e-8);
    c.at_most("T10 max |Im|/|w|", imr, 1e-8);
    c.finish();
}

fn key_of(r: Result<impl std::fmt::Debug, FemError>) -> String {
    match r {
        Err(FemError::InvalidValue { key, .. }) => key.to_string(),
        Err(FemError::UnknownSet { .. }) => "unknown set".to_string(),
        Err(e) => format!("other: {e}"),
        Ok(v) => format!("accepted: {v:?}"),
    }
}

#[test]
fn t11_refusals() {
    let mut c = Checks::new();
    let col = planar();
    let pre = prestress(&col);
    let fp = FollowerTipForce::new(&col.model, "xl", col.force).unwrap();
    let mut check = |name: &str, got: String, want: &str| {
        c.holds(name, got == want, &format!("{got:?} (want {want:?})"));
    };
    check(
        "T11 unknown set",
        key_of(FollowerTipForce::new(&col.model, "nowhere", col.force).map(|_| ())),
        "unknown set",
    );
    check(
        "T11 NaN force",
        key_of(FollowerTipForce::new(&col.model, "xl", [f64::NAN, 0.0, 0.0]).map(|_| ())),
        "force",
    );
    let mut hex = SolidModel::new(hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap());
    hex.add_material("all", Material::isotropic_elastic(E, 0.3, RHO, 0.0).unwrap())
        .unwrap();
    check(
        "T11 Hex8 node",
        key_of(FollowerTipForce::new(&hex, "xmax", [1.0, 0.0, 0.0]).map(|_| ())),
        "follower force",
    );
    let run = |m: &SolidModel, pre: &Prestress, lf: f64, o: ModalOptions| {
        key_of(prestressed_modes_full(m, pre, &[&fp], lf, &o).map(|r| r.eigenvalues))
    };
    check(
        "T11 Shift::Auto",
        run(&col.model, &pre, 0.5, ModalOptions::new(4).shift(Shift::Auto)),
        "shift",
    );
    let (free, ..) = oblique_unclamped();
    let pre_free = Prestress::uniform(&free, [0.0; 6]).unwrap();
    check(
        "T11 rigid-free, shift None",
        key_of(
            prestressed_modes_full(
                &free,
                &pre_free,
                &[&FollowerTipForce::new(&free, "xl", [-1.0, 0.0, 0.0]).unwrap()],
                0.5,
                &ModalOptions::new(4),
            )
            .map(|r| r.eigenvalues),
        ),
        "shift",
    );
    check(
        "T11 NaN load factor",
        run(&col.model, &pre, f64::NAN, ModalOptions::new(4)),
        "load factor",
    );
    let mut lin = col.model.clone();
    lin.add_constraint(Constraint::linear(
        vec![LinearTerm::new(N, 1, 1.0), LinearTerm::new(N - 1, 1, -1.0)],
        0.0,
    ))
    .unwrap();
    check(
        "T11 linear constraint",
        run(&lin, &pre, 0.5, ModalOptions::new(4)),
        "load stiffness",
    );
    let mut moved = col.model.clone();
    moved.add_constraint(Constraint::displacement("xl", 0, 1e-3)).unwrap();
    check(
        "T11 non-zero prescribed value",
        run(&moved, &pre, 0.5, ModalOptions::new(4)),
        "constraint",
    );
    check(
        "T11 modes = 0",
        run(&col.model, &pre, 0.5, ModalOptions::new(0)),
        "modes",
    );
    c.finish();
}
