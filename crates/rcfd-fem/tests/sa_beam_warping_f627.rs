//! F-627 (ADR 0553): monosymmetric thin-walled sections for the warping element (the shear-centre offset, the monosymmetry
//! coefficient beta_x, the bimoment load). Checks D1 to D3, A1 to A8, N1, C1, V1, S1, S2, W1, W2, E1, E2' (amendment 1), E3, E4
//! and R of the registration. Each registered run is made once; every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    buckling_factors, buckling_factors_with, local_geometric_stiffness, local_stiffness, modal_analysis,
    section_local_geometric_stiffness, section_local_stiffness, solve_static, Warping, WarpingPrestress,
    WarpingSection,
};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::element::ElementMatrix;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A value that is
/// not finite is a miss.
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
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
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
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const KAPPA: f64 = 5.0 / 6.0;
const M_REF: f64 = 1000.0;

fn g_mod() -> f64 {
    E / (2.0 * (1.0 + NU))
}

fn steel() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

/// The monosymmetric I of the readings: flange 1 (`0.15 x 0.012`, the larger) on `+e_3`.
fn mono() -> (BeamSection, WarpingSection) {
    WarpingSection::monosymmetric_i(0.15, 0.012, 0.08, 0.008, 0.2, 0.006, KAPPA).unwrap()
}

/// `z` of the mid-plane of flange 1 from the centroid (D5).
const Z_1: f64 = 7.1868131868e-2;

/// A beam of `n` elements, length `l`, along the global axis `axis` (0 = x, 1 = y), orientation `y` (axis x) or `z` (axis y);
/// node sets `x0`, `xl`, `nodes`, `mid`. Constraint components are permuted for axis y as in ADR 0546 A5.
fn mesh_along(n: usize, l: f64, section: &BeamSection, axis: usize) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=n)
        .map(|i| {
            let mut p = [0.0; 3];
            p[axis] = l * i as f64 / n as f64;
            p
        })
        .collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(*section, orient(axis)).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    mesh.add_node_set("nodes", (0..=n).collect()).unwrap();
    mesh.add_node_set("mid", vec![n / 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m
}

fn orient(axis: usize) -> Vec3 {
    if axis == 0 {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// The global components of the local `(u, v, w, phi)` for the beam along `axis`.
fn comps(axis: usize) -> [usize; 4] {
    if axis == 0 {
        [0, 1, 2, 3]
    } else {
        [1, 2, 0, 4]
    }
}

/// Fork supports: `u = 0` at node 0, `v = w = phi = 0` at both ends, warping free.
fn fork(n: usize, l: f64, section: &BeamSection, axis: usize) -> SolidModel {
    let mut m = mesh_along(n, l, section, axis);
    let c = comps(axis);
    for k in c {
        m.add_constraint(Constraint::displacement("x0", k, 0.0)).unwrap();
    }
    for &k in &c[1..] {
        m.add_constraint(Constraint::displacement("xl", k, 0.0)).unwrap();
    }
    m
}

/// End moments `s M_ref` about `e_2` at `xl` and `-s M_ref` at `x0`.
fn moment_loaded(m: &SolidModel, axis: usize, s: f64) -> SolidModel {
    let mut l = m.clone();
    let e2 = orient(axis);
    let mom = |f: f64| [f * e2[0], f * e2[1], f * e2[2]];
    l.add_load(Load::nodal_moment("x0", mom(-s * M_REF))).unwrap();
    l.add_load(Load::nodal_moment("xl", mom(s * M_REF))).unwrap();
    l
}

fn six_prestress(loaded: &SolidModel) -> (Prestress, Vec<f64>) {
    let r = rcfd_fem::solve::solve_static(loaded, &StaticOptions::default()).unwrap();
    let p = Prestress::from_static(loaded, &r)
        .unwrap()
        .with_beam_resultants(loaded, r.displacement_values(), [0.0; 2])
        .unwrap();
    (p, r.displacement_values().to_vec())
}

/// One LTB run: the factor, the mode, whether flange 1 is in compression (from the static solution, `z_1 dtheta_y/dx`).
struct Ltb {
    factor: f64,
    mode: Vec<f64>,
    flange1_compressed: bool,
    strain1: f64,
}

fn ltb(section: &BeamSection, ws: WarpingSection, n: usize, l: f64, axis: usize, s: f64) -> Ltb {
    ltb_with(section, ws, n, l, axis, s, &BucklingOptions::new(3))
}

/// The options of amendment 2: three factors, a subspace of 40.
fn amended_options() -> BucklingOptions {
    let mut o = BucklingOptions::new(3);
    o.subspace_size = Some(40);
    o
}

fn ltb_amended(section: &BeamSection, ws: WarpingSection, n: usize, l: f64, axis: usize, s: f64) -> Ltb {
    ltb_with(section, ws, n, l, axis, s, &amended_options())
}

fn ltb_with(
    section: &BeamSection,
    ws: WarpingSection,
    n: usize,
    l: f64,
    axis: usize,
    s: f64,
    opts: &BucklingOptions,
) -> Ltb {
    let m = fork(n, l, section, axis);
    let loaded = moment_loaded(&m, axis, s);
    let (pre, u) = six_prestress(&loaded);
    let w = Warping::new().block(0, ws);
    let r = buckling_factors(&m, &pre, &w, opts).unwrap();
    let dofs = m.dof_map();
    let e2 = orient(axis);
    let theta_y = |node: usize| -> f64 { (0..3).map(|c| e2[c] * u[dofs.equation(node, 3 + c).unwrap()]).sum() };
    let strain1 = Z_1 * (theta_y(n) - theta_y(0)) / l;
    println!(
        "  factors {:?} (n {n}, L {l}, axis {axis}, sign {s:+}), flange-1 strain {strain1:.4e}",
        r.factors
    );
    Ltb {
        factor: r.factors[0],
        mode: r.modes[0].clone(),
        flange1_compressed: strain1 < 0.0,
        strain1,
    }
}

/// The closed form of D3: (`|M|` with the larger flange in compression, `|M|` with it in tension), and `P_y`.
fn closed(section: &BeamSection, ws: &WarpingSection, l: f64) -> (f64, f64, f64) {
    let k = PI / l;
    let py = E * section.i_z() * k * k;
    let gjw = g_mod() * section.torsion() + E * ws.c_w() * k * k;
    let bx = ws.wagner_about_shear_centre()[0];
    let root = ((bx * py / 2.0).powi(2) + py * gjw).sqrt();
    (bx.abs() * py / 2.0 + root, -bx.abs() * py / 2.0 + root, py)
}

#[test]
fn d1_to_d3_section() {
    let mut c = Checks::new();
    let (s, ws) = mono();
    let bx = ws.wagner_about_shear_centre()[0];
    let got = [
        ("A", s.area(), 3.64e-3),
        ("I_y", s.i_y(), 2.6931510037e-5),
        ("I_z", s.i_z(), 3.7199333333e-6),
        ("J", s.torsion(), 1.1445333333e-7),
        ("C_w", ws.c_w(), 1.3670248453e-8),
        ("z_0", ws.shear_centre()[1], 5.2580303363e-2),
        ("beta_y^C", ws.wagner()[0], -4.7431546579e-2),
        ("beta_x", bx, -1.5259215331e-1),
    ];
    for (name, v, want) in got {
        println!("D1 {name} {v:.12e} (hand {want:.10e})");
        c.at_most(&format!("D1 {name}"), rel(v, want), 1e-9);
    }
    c.holds(
        "D1 y_0 = beta_z^C = 0",
        ws.shear_centre()[0] == 0.0 && ws.wagner()[1] == 0.0,
        &format!("{:?} {:?}", ws.shear_centre(), ws.wagner()),
    );
    let (se, we) = WarpingSection::monosymmetric_i(0.1, 0.01, 0.1, 0.01, 0.18, 0.006, KAPPA).unwrap();
    let (sd, wd) = WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, KAPPA).unwrap();
    for (name, a, b) in [
        ("A", se.area(), sd.area()),
        ("I_y", se.i_y(), sd.i_y()),
        ("I_z", se.i_z(), sd.i_z()),
        ("J", se.torsion(), sd.torsion()),
        ("C_w", we.c_w(), wd.c_w()),
    ] {
        c.at_most(&format!("D2 equal flanges {name}"), rel(a, b), 1e-12);
    }
    println!("D2 z_0 {:e}, beta_y^C {:e}", we.shear_centre()[1], we.wagner()[0]);
    c.at_most("D2 |z_0|", we.shear_centre()[1].abs(), 1e-12);
    c.at_most("D2 |beta_y^C|", we.wagner()[0].abs(), 1e-12);
    let fe = ltb(&se, we, 16, 3.0, 0, 1.0).factor;
    let fd = ltb(&sd, wd, 16, 3.0, 0, 1.0).factor;
    println!("D3 equal-flange factor {fe:.15e}, doubly symmetric {fd:.15e}");
    c.at_most("D3 equal flanges against ADR 0546", rel(fe, fd), 1e-9);
    c.finish();
}

/// The first-run record of A1 to A8 and N1 (ADR 0553 amendment 2): run 1 stopped at the second solve, the `-M_ref` load case
/// at 16 elements, refused by `lowest_buckling_factors` with "Sturm count: 32 eigenvalues below 5.397979218577e9, but 3 were
/// found" (the default subspace of 11 held exactly three positive Ritz values; the negative factors of this sign-asymmetric
/// spectrum dominate). No A or N1 value was measured. Kept unchanged except for its name and this note.
#[test]
#[ignore = "ADR 0553 amendment 2: run-1 record, refused by the eigensolver (Sturm mismatch) at the -M_ref case"]
fn a_n_run1_known_miss() {
    let mut c = Checks::new();
    let (s, ws) = mono();
    let z0 = ws.shear_centre()[1];
    let run =
        |n: usize, l: f64, axis: usize| -> (Ltb, Ltb) { (ltb(&s, ws, n, l, axis, 1.0), ltb(&s, ws, n, l, axis, -1.0)) };
    // Returns (compressed run, tension run) with the sign of each.
    let split = |p: Ltb, q: Ltb| -> ((Ltb, f64), (Ltb, f64)) {
        if p.flange1_compressed {
            ((p, 1.0), (q, -1.0))
        } else {
            ((q, -1.0), (p, 1.0))
        }
    };
    let (big, small, py) = closed(&s, &ws, 3.0);
    println!("closed |M| flange 1 compressed {big:.8e}, in tension {small:.8e}, P_y {py:.8e}");
    let (p, q) = run(16, 3.0, 0);
    c.holds(
        "A3 one load case each way",
        p.flange1_compressed != q.flange1_compressed,
        &format!("strains {:.4e} {:.4e}", p.strain1, q.strain1),
    );
    let ((cmp, s_c), (ten, s_t)) = split(p, q);
    let m_c = cmp.factor * M_REF;
    let m_t = ten.factor * M_REF;
    println!(
        "A1 M {m_c:.8e} signed {:.4e}; A2 M {m_t:.8e} signed {:.4e}",
        m_c / big - 1.0,
        m_t / small - 1.0
    );
    c.at_most("A1 |M/M_- - 1| flange 1 compressed (acceptance)", rel(m_c, big), 2e-2);
    c.at_most("A2 |M/M_+ - 1| flange 1 in tension (acceptance)", rel(m_t, small), 2e-2);
    c.holds(
        "A3 the larger |M| has flange 1 in compression",
        m_c > m_t,
        &format!("{m_c:.6e} > {m_t:.6e}"),
    );
    // A8: the mode at mid-span, v/phi against M/P_y + z_0 (M signed in the convention of D2).
    for (run, sign, name) in [(&cmp, s_c, "compressed"), (&ten, s_t, "tension")] {
        let m = fork(16, 3.0, &s, 0);
        let dofs = m.dof_map();
        let v = run.mode[dofs.equation(8, 1).unwrap()];
        let ph = run.mode[dofs.equation(8, 3).unwrap()];
        let want = sign * run.factor * M_REF / py + z0;
        println!("A8 {name}: v/phi {:.8e}, closed {want:.8e}", v / ph);
        c.at_most(&format!("A8 mode ratio ({name})"), rel(v / ph, want), 1e-2);
    }
    // N1.
    let k = PI / 3.0;
    let gjw = g_mod() * s.torsion() + E * ws.c_w() * k * k;
    let m0 = (py * gjw).sqrt();
    let bc = ws.wagner()[0];
    let rc = ((bc * py / 2.0).powi(2) + py * gjw).sqrt();
    let (bc_big, bc_small) = (bc.abs() * py / 2.0 + rc, -bc.abs() * py / 2.0 + rc);
    println!("N1 beta_x = 0 form {m0:.8e}; beta^C forms {bc_big:.8e} {bc_small:.8e}");
    c.above("N1 A1 against beta_x = 0", rel(m_c, m0), 0.3);
    c.above("N1 A2 against beta_x = 0", rel(m_t, m0), 0.3);
    c.above("N1 A1 against beta^C", rel(m_c, bc_big), 0.15);
    c.above("N1 A2 against beta^C", rel(m_t, bc_small), 0.15);
    // A4: 8 elements.
    let (p8, q8) = run(8, 3.0, 0);
    let ((c8, _), (t8, _)) = split(p8, q8);
    c.at_most("A4 flange 1 compressed, 8 elements", rel(c8.factor * M_REF, big), 2e-2);
    c.at_most(
        "A4 flange 1 in tension, 8 elements",
        rel(t8.factor * M_REF, small),
        2e-2,
    );
    // A5: L 1.5 and 6.
    for l in [1.5, 6.0] {
        let (b, sm, _) = closed(&s, &ws, l);
        let (pl, ql) = run(16, l, 0);
        let ((cl, _), (tl, _)) = split(pl, ql);
        println!(
            "A5 L {l}: compressed {:.8e} (closed {b:.8e}), tension {:.8e} (closed {sm:.8e})",
            cl.factor * M_REF,
            tl.factor * M_REF
        );
        c.at_most(&format!("A5 compressed, L {l}"), rel(cl.factor * M_REF, b), 2e-2);
        c.at_most(&format!("A5 tension, L {l}"), rel(tl.factor * M_REF, sm), 2e-2);
    }
    // A6: along global y.
    let (py_, qy) = run(16, 3.0, 1);
    let ((cy, _), (ty, _)) = split(py_, qy);
    c.at_most("A6 axis y, compressed", rel(cy.factor, cmp.factor), 1e-6);
    c.at_most("A6 axis y, tension", rel(ty.factor, ten.factor), 1e-6);
    // A7: the pre-stress of the warping static solve.
    for (sign, f_ref, name) in [(s_c, cmp.factor, "compressed"), (s_t, ten.factor, "tension")] {
        let m = fork(16, 3.0, &s, 0);
        let loaded = moment_loaded(&m, 0, sign);
        let w = Warping::new().block(0, ws);
        let r = solve_static(&loaded, &w, None).unwrap();
        let p = WarpingPrestress::from_static(&loaded, &w, &r, [0.0; 2]).unwrap();
        let f = buckling_factors_with(&m, &p, &w, &BucklingOptions::new(3))
            .unwrap()
            .factors[0];
        c.at_most(
            &format!("A7 from_static against six-unknown ({name})"),
            rel(f, f_ref),
            1e-9,
        );
    }
    c.finish();
}

/// A1' to A8' and N1' (ADR 0553 amendment 2): the checks of A1 to A8 and N1 with a buckling subspace of 40.
#[test]
fn a_n_amended_lateral_torsional_buckling_of_the_monosymmetric_i() {
    let mut c = Checks::new();
    let (s, ws) = mono();
    let z0 = ws.shear_centre()[1];
    let run = |n: usize, l: f64, axis: usize| -> (Ltb, Ltb) {
        (
            ltb_amended(&s, ws, n, l, axis, 1.0),
            ltb_amended(&s, ws, n, l, axis, -1.0),
        )
    };
    // Returns (compressed run, tension run) with the sign of each.
    let split = |p: Ltb, q: Ltb| -> ((Ltb, f64), (Ltb, f64)) {
        if p.flange1_compressed {
            ((p, 1.0), (q, -1.0))
        } else {
            ((q, -1.0), (p, 1.0))
        }
    };
    let (big, small, py) = closed(&s, &ws, 3.0);
    println!("closed |M| flange 1 compressed {big:.8e}, in tension {small:.8e}, P_y {py:.8e}");
    let (p, q) = run(16, 3.0, 0);
    c.holds(
        "A3' one load case each way",
        p.flange1_compressed != q.flange1_compressed,
        &format!("strains {:.4e} {:.4e}", p.strain1, q.strain1),
    );
    let ((cmp, s_c), (ten, s_t)) = split(p, q);
    let m_c = cmp.factor * M_REF;
    let m_t = ten.factor * M_REF;
    println!(
        "A1' M {m_c:.8e} signed {:.4e}; A2 M {m_t:.8e} signed {:.4e}",
        m_c / big - 1.0,
        m_t / small - 1.0
    );
    c.at_most("A1' |M/M_- - 1| flange 1 compressed (acceptance)", rel(m_c, big), 2e-2);
    c.at_most(
        "A2' |M/M_+ - 1| flange 1 in tension (acceptance)",
        rel(m_t, small),
        2e-2,
    );
    c.holds(
        "A3' the larger |M| has flange 1 in compression",
        m_c > m_t,
        &format!("{m_c:.6e} > {m_t:.6e}"),
    );
    // A8: the mode at mid-span, v/phi against M/P_y + z_0 (M signed in the convention of D2).
    for (run, sign, name) in [(&cmp, s_c, "compressed"), (&ten, s_t, "tension")] {
        let m = fork(16, 3.0, &s, 0);
        let dofs = m.dof_map();
        let v = run.mode[dofs.equation(8, 1).unwrap()];
        let ph = run.mode[dofs.equation(8, 3).unwrap()];
        let want = sign * run.factor * M_REF / py + z0;
        println!("A8' {name}: v/phi {:.8e}, closed {want:.8e}", v / ph);
        c.at_most(&format!("A8' mode ratio ({name})"), rel(v / ph, want), 1e-2);
    }
    // N1.
    let k = PI / 3.0;
    let gjw = g_mod() * s.torsion() + E * ws.c_w() * k * k;
    let m0 = (py * gjw).sqrt();
    let bc = ws.wagner()[0];
    let rc = ((bc * py / 2.0).powi(2) + py * gjw).sqrt();
    let (bc_big, bc_small) = (bc.abs() * py / 2.0 + rc, -bc.abs() * py / 2.0 + rc);
    println!("N1' beta_x = 0 form {m0:.8e}; beta^C forms {bc_big:.8e} {bc_small:.8e}");
    c.above("N1' A1 against beta_x = 0", rel(m_c, m0), 0.3);
    c.above("N1' A2 against beta_x = 0", rel(m_t, m0), 0.3);
    c.above("N1' A1 against beta^C", rel(m_c, bc_big), 0.15);
    c.above("N1' A2 against beta^C", rel(m_t, bc_small), 0.15);
    // A4: 8 elements.
    let (p8, q8) = run(8, 3.0, 0);
    let ((c8, _), (t8, _)) = split(p8, q8);
    c.at_most("A4' flange 1 compressed, 8 elements", rel(c8.factor * M_REF, big), 2e-2);
    c.at_most(
        "A4' flange 1 in tension, 8 elements",
        rel(t8.factor * M_REF, small),
        2e-2,
    );
    // A5: L 1.5 and 6.
    for l in [1.5, 6.0] {
        let (b, sm, _) = closed(&s, &ws, l);
        let (pl, ql) = run(16, l, 0);
        let ((cl, _), (tl, _)) = split(pl, ql);
        println!(
            "A5' L {l}: compressed {:.8e} (closed {b:.8e}), tension {:.8e} (closed {sm:.8e})",
            cl.factor * M_REF,
            tl.factor * M_REF
        );
        c.at_most(&format!("A5' compressed, L {l}"), rel(cl.factor * M_REF, b), 2e-2);
        c.at_most(&format!("A5' tension, L {l}"), rel(tl.factor * M_REF, sm), 2e-2);
    }
    // A6: along global y.
    let (py_, qy) = run(16, 3.0, 1);
    let ((cy, _), (ty, _)) = split(py_, qy);
    c.at_most("A6' axis y, compressed", rel(cy.factor, cmp.factor), 1e-6);
    c.at_most("A6' axis y, tension", rel(ty.factor, ten.factor), 1e-6);
    // A7: the pre-stress of the warping static solve.
    for (sign, f_ref, name) in [(s_c, cmp.factor, "compressed"), (s_t, ten.factor, "tension")] {
        let m = fork(16, 3.0, &s, 0);
        let loaded = moment_loaded(&m, 0, sign);
        let w = Warping::new().block(0, ws);
        let r = solve_static(&loaded, &w, None).unwrap();
        let p = WarpingPrestress::from_static(&loaded, &w, &r, [0.0; 2]).unwrap();
        let f = buckling_factors_with(&m, &p, &w, &amended_options()).unwrap().factors[0];
        c.at_most(
            &format!("A7' from_static against six-unknown ({name})"),
            rel(f, f_ref),
            1e-9,
        );
    }
    c.finish();
}

#[test]
fn c1_flexural_torsional_column() {
    let mut c = Checks::new();
    let (s, ws) = mono();
    let z0 = ws.shear_centre()[1];
    let k = PI / 3.0;
    let py = E * s.i_z() * k * k;
    let gjw = g_mod() * s.torsion() + E * ws.c_w() * k * k;
    let ipa = (s.i_y() + s.i_z()) / s.area();
    // (Py - P)(GJw - P r0^2) - P^2 z0^2 = 0, r0^2 = Ip/A + z0^2.
    let (c2, c1, c0) = (ipa, -(py * (ipa + z0 * z0) + gjw), py * gjw);
    let p1 = (-c1 - (c1 * c1 - 4.0 * c2 * c0).sqrt()) / (2.0 * c2);
    let m = fork(16, 3.0, &s, 0);
    let mut loaded = m.clone();
    loaded.add_load(Load::nodal_force("xl", [-1000.0, 0.0, 0.0])).unwrap();
    let r = rcfd_fem::solve::solve_static(&loaded, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&loaded, &r).unwrap();
    let f = buckling_factors(&m, &pre, &Warping::new().block(0, ws), &BucklingOptions::new(3)).unwrap();
    let p = f.factors[0] * 1000.0;
    println!(
        "C1 P {p:.8e}, closed P_1 {p1:.8e} (hand 6.09657056e5), P_y {py:.8e}; factors {:?}",
        f.factors
    );
    c.at_most("C1 |P/P_1 - 1|", rel(p, p1), 1e-2);
    c.above("C1 against the uncoupled P_y", rel(p, py), 0.1);
    c.finish();
}

#[test]
fn v1_coupled_vibration() {
    let mut c = Checks::new();
    let (s, ws) = mono();
    let z0 = ws.shear_centre()[1];
    let k = PI / 3.0;
    let (a, ip) = (s.area(), s.i_y() + s.i_z());
    let k11 = E * s.i_z() * k.powi(4);
    let k22 = g_mod() * s.torsion() * k * k + E * ws.c_w() * k.powi(4);
    let (m11, m12, m22) = (RHO * a, RHO * a * z0, RHO * (ip + a * z0 * z0));
    // det(K - w2 M) = 0.
    let qa = m11 * m22 - m12 * m12;
    let qb = -(k11 * m22 + k22 * m11);
    let qc = k11 * k22;
    let disc = (qb * qb - 4.0 * qa * qc).sqrt();
    let w = [(-qb - disc) / (2.0 * qa), (-qb + disc) / (2.0 * qa)];
    let mut m = fork(16, 3.0, &s, 0);
    for comp in [0, 2, 4] {
        m.add_constraint(Constraint::displacement("nodes", comp, 0.0)).unwrap();
    }
    let r = modal_analysis(&m, &Warping::new().block(0, ws), &ModalOptions::new(3)).unwrap();
    println!(
        "V1 omega^2 {:?}, closed {:.8e} {:.8e} (hand 2.33976263e4 7.19848243e4)",
        r.eigenvalues, w[0], w[1]
    );
    c.at_most("V1 lowest", rel(r.eigenvalues[0], w[0]), 1e-2);
    c.at_most("V1 second", rel(r.eigenvalues[1], w[1]), 1e-2);
    c.finish();
}

/// The cantilever along `x`, root with all six components held and its warping held.
fn cantilever(section: &BeamSection) -> SolidModel {
    let mut m = mesh_along(16, 3.0, section, 0);
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("x0", comp, 0.0)).unwrap();
    }
    m
}

#[test]
fn s_w_static_lateral_force_and_bimoment() {
    let mut c = Checks::new();
    let (s, ws) = mono();
    let z0 = ws.shear_centre()[1];
    let l = 3.0;
    let gj = g_mod() * s.torsion();
    let ecw = E * ws.c_w();
    let k = (gj / ecw).sqrt();
    let w = Warping::new().block(0, ws).restrain("x0");
    // S1.
    let f = 1000.0;
    let mut m = cantilever(&s);
    m.add_load(Load::nodal_force("xl", [0.0, f, 0.0])).unwrap();
    let r = solve_static(&m, &w, None).unwrap();
    let dofs = m.dof_map();
    let phi = r.displacement[dofs.equation(16, 3).unwrap()];
    let v = r.displacement[dofs.equation(16, 1).unwrap()];
    let phi_want = z0 * f / gj * (l - (k * l).tanh() / k);
    let v_want = f * l.powi(3) / (3.0 * E * s.i_z()) + f * l / (KAPPA * g_mod() * s.area()) + z0 * phi_want;
    println!("S1 phi(L) {phi:.10e} (closed {phi_want:.10e}), v(L) {v:.10e} (closed {v_want:.10e})");
    c.at_most("S1 phi(L) signed", (phi / phi_want - 1.0).abs(), 1e-3);
    c.at_most("S1 v(L)", rel(v, v_want), 1e-3);
    // S2.
    let mut m2 = cantilever(&s);
    m2.add_load(Load::nodal_force("xl", [0.0, 0.0, f])).unwrap();
    let r2 = solve_static(&m2, &w, None).unwrap();
    let wl = r2.displacement[dofs.equation(16, 2).unwrap()];
    let phi2 = r2.displacement[dofs.equation(16, 3).unwrap()];
    let chi_max = (0..=16).map(|n| r2.warping_at(n).unwrap().abs()).fold(0.0, f64::max);
    println!("S2 w(L) {wl:.6e}, phi(L) {phi2:e}, max|chi| {chi_max:e}");
    c.holds(
        "S2 no twist along the symmetry axis",
        wl.is_finite() && wl != 0.0 && phi2.abs() <= 1e-14 * wl.abs() && chi_max <= 1e-14 * wl.abs() / l,
        &format!("|phi| {:e}, |chi| {chi_max:e}", phi2.abs()),
    );
    // W1, W2.
    let b = 100.0;
    let wb = Warping::new().block(0, ws).restrain("x0").bimoment("xl", b);
    let m3 = cantilever(&s);
    let r3 = solve_static(&m3, &wb, None).unwrap();
    let phi3 = r3.displacement[dofs.equation(16, 3).unwrap()];
    let chi3 = r3.warping_at(16).unwrap();
    let v3 = r3.displacement[dofs.equation(16, 1).unwrap()];
    let rb = r3.reactions[r3.dofs.equation(0).unwrap()];
    let rt = r3.reactions[dofs.equation(0, 3).unwrap()];
    let phi_b = b / gj * (1.0 - 1.0 / (k * l).cosh());
    let chi_b = b * (k * l).tanh() / (ecw * k);
    let rb_want = -b / (k * l).cosh();
    println!(
        "W1 phi(L) {phi3:.10e} ({phi_b:.10e}), chi(L) {chi3:.10e} ({chi_b:.10e}), root bimoment reaction {rb:.10e} \
         ({rb_want:.10e}), v(L) {v3:.10e} (z_0 phi {:.10e}), root twist reaction {rt:e}",
        z0 * phi3
    );
    c.at_most("W1 phi(L) signed", (phi3 / phi_b - 1.0).abs(), 1e-3);
    c.at_most("W1 chi(L) signed", (chi3 / chi_b - 1.0).abs(), 1e-3);
    c.at_most("W1 root bimoment reaction signed", (rb / rb_want - 1.0).abs(), 1e-3);
    c.at_most("W1 v(L) against z_0 phi(L)", (v3 / (z0 * phi3) - 1.0).abs(), 1e-3);
    c.at_most("W2 root twist reaction / (B k)", rt.abs() / (b * k), 1e-9);
    c.finish();
}

fn dense(a: &ElementMatrix) -> [[f64; 14]; 14] {
    let mut d = [[0.0; 14]; 14];
    for (i, row) in d.iter_mut().enumerate() {
        for (j, x) in row.iter_mut().enumerate() {
            *x = a.get(i, j);
        }
    }
    d
}

/// The test's own `O^T A O` (D1: `O[v][phi] = -z_0`, `O[w][phi] = y_0`).
fn hand_transform(a: &ElementMatrix, y0: f64, z0: f64) -> [[f64; 14]; 14] {
    let mut o = [[0.0; 14]; 14];
    for (i, row) in o.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for base in [0, 7] {
        o[base + 1][base + 3] = -z0;
        o[base + 2][base + 3] = y0;
    }
    let ad = dense(a);
    let mut out = [[0.0; 14]; 14];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, x) in row.iter_mut().enumerate() {
            let mut s = 0.0;
            for p in 0..14 {
                for q in 0..14 {
                    s += o[p][i] * ad[p][q] * o[q][j];
                }
            }
            *x = s;
        }
    }
    out
}

#[test]
fn e1_to_e4_element() {
    let mut c = Checks::new();
    let (s, wm) = mono();
    let mat = steel();
    let l = 0.7;
    let (y0, z0) = (0.01, 0.04);
    let ws0 = WarpingSection::new(wm.c_w(), 0.7, -0.4).unwrap();
    let ws = ws0.with_shear_centre(y0, z0).unwrap();
    // E1.
    let k = local_stiffness(l, &s, &mat, wm.c_w());
    let kc = section_local_stiffness(l, &s, &mat, &ws);
    let hand = hand_transform(&k, y0, z0);
    let kmax = k.values().iter().fold(0.0f64, |a, b| a.max(b.abs()));
    let mut outside_equal = true;
    let mut err: f64 = 0.0;
    for i in 0..14 {
        for j in 0..14 {
            if i == 3 || i == 10 || j == 3 || j == 10 {
                err = err.max((kc.get(i, j) - hand[i][j]).abs());
            } else if kc.get(i, j).to_bits() != k.get(i, j).to_bits() {
                outside_equal = false;
            }
        }
    }
    c.holds("E1 entries outside the twist rows bit-equal", outside_equal, "");
    c.at_most("E1 twist rows against O^T K O", err / kmax, 1e-12);
    // E2' (amendment 1).
    let ff = {
        let mut f = [0.0; 12];
        f[1] = -40.0;
        f[7] = 40.0;
        f[2] = 25.0;
        f[8] = -25.0;
        f[3] = -6.0;
        f[9] = 6.0;
        f[4] = -3.0;
        f[5] = -2.0;
        f[10] = -f[4] - 25.0 * l;
        f[11] = -f[5] - 40.0 * l;
        f
    };
    let n_ax = -5e3;
    let g = section_local_geometric_stiffness(l, &s, &mat, n_ax, Some(&ff), &ws);
    let mut fs = ff;
    for o in [0, 6] {
        fs[o + 3] += z0 * ff[o + 1] - y0 * ff[o + 2];
    }
    let beta_s = [0.7 - 2.0 * z0, -0.4 - 2.0 * y0];
    let ga = local_geometric_stiffness(l, &s, &mat, n_ax, None, [0.0; 2]);
    let gm = hand_transform(&local_geometric_stiffness(l, &s, &mat, 0.0, Some(&fs), beta_s), y0, z0);
    let mut hmax: f64 = 0.0;
    let mut gerr: f64 = 0.0;
    for i in 0..14 {
        for j in 0..14 {
            let h = ga.get(i, j) + gm[i][j];
            hmax = hmax.max(h.abs());
            gerr = gerr.max((g.get(i, j) - h).abs());
        }
    }
    c.at_most("E2'(a) against the dense composition", gerr / hmax, 1e-12);
    let wsc = ws.wagner_about_shear_centre();
    println!("E2'(b) {wsc:?}");
    c.at_most("E2'(b) beta_y^S", (wsc[0] - 0.62).abs(), 1e-15);
    c.at_most("E2'(b) beta_z^S", (wsc[1] + 0.42).abs(), 1e-15);
    // E3.
    let g3 = section_local_geometric_stiffness(l, &s, &mat, n_ax, None, &ws);
    let g3r = local_geometric_stiffness(l, &s, &mat, n_ax, None, ws.wagner());
    c.holds(
        "E3 axial part bit-equal",
        g3.values()
            .iter()
            .zip(g3r.values())
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "",
    );
    // E4.
    let k4 = section_local_stiffness(l, &s, &mat, &ws0);
    let g4 = section_local_geometric_stiffness(l, &s, &mat, n_ax, Some(&ff), &ws0);
    let g4r = local_geometric_stiffness(l, &s, &mat, n_ax, Some(&ff), ws0.wagner());
    let bits = |a: &ElementMatrix, b: &ElementMatrix| {
        a.values()
            .iter()
            .zip(b.values())
            .all(|(x, y)| x.to_bits() == y.to_bits())
    };
    c.holds("E4 zero offset: stiffness bit-equal", bits(&k4, &k), "");
    c.holds("E4 zero offset: geometric bit-equal", bits(&g4, &g4r), "");
    c.finish();
}

fn key_of(e: &FemError) -> String {
    match e {
        FemError::InvalidValue { key, .. } => key.to_string(),
        FemError::Element { source, .. } => key_of(source),
        FemError::UnknownSet { .. } => "UnknownSet".to_string(),
        other => format!("{other:?}"),
    }
}

#[test]
fn r_refusals() {
    let mut c = Checks::new();
    let (s, ws) = mono();
    let mut check = |name: &str, got: Option<String>, want: &str| {
        c.holds(name, got.as_deref() == Some(want), &format!("{got:?} (want {want})"));
    };
    let err = |r: Result<(), FemError>| r.err().map(|e| key_of(&e));
    check(
        "R with_shear_centre non-finite",
        err(ws.with_shear_centre(0.0, f64::NAN).map(|_| ())),
        "shear_centre",
    );
    check(
        "R monosymmetric_i b_2 = 0",
        err(WarpingSection::monosymmetric_i(0.15, 0.012, 0.0, 0.008, 0.2, 0.006, KAPPA).map(|_| ())),
        "b_2",
    );
    check(
        "R monosymmetric_i t_w infinite",
        err(WarpingSection::monosymmetric_i(0.15, 0.012, 0.08, 0.008, 0.2, f64::INFINITY, KAPPA).map(|_| ())),
        "t_w",
    );
    let m = cantilever(&s);
    let w = Warping::new().block(0, ws).restrain("x0");
    check(
        "R bimoment on an unknown set",
        err(solve_static(&m, &w.clone().bimoment("nope", 1.0), None).map(|_| ())),
        "UnknownSet",
    );
    check(
        "R bimoment non-finite",
        err(solve_static(&m, &w.clone().bimoment("xl", f64::NAN), None).map(|_| ())),
        "bimoment",
    );
    {
        // A node without a warping unknown: a second, disconnected non-warping beam.
        let n = 4;
        let mut nodes: Vec<Vec3> = (0..=n).map(|i| [i as f64, 0.0, 0.0]).collect();
        nodes.extend((0..=n).map(|i| [i as f64, 5.0, 0.0]));
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(ElementKind::Beam2, (0..n).flat_map(|i| [i, i + 1]).collect())
            .unwrap();
        mesh.add_block(
            ElementKind::Beam2,
            (0..n).flat_map(|i| [n + 1 + i, n + 2 + i]).collect(),
        )
        .unwrap();
        for b in 0..2 {
            mesh.set_block_property(b, BlockProperty::beam(s, [0.0, 1.0, 0.0]).unwrap())
                .unwrap();
        }
        mesh.add_element_set("all", (0..2 * n).collect()).unwrap();
        mesh.add_node_set("x0", vec![0, n + 1]).unwrap();
        mesh.add_node_set("other", vec![2 * n + 1]).unwrap();
        let mut mm = SolidModel::new(mesh);
        mm.add_material("all", steel()).unwrap();
        for comp in 0..6 {
            mm.add_constraint(Constraint::displacement("x0", comp, 0.0)).unwrap();
        }
        check(
            "R bimoment on a node without a warping unknown",
            err(solve_static(&mm, &Warping::new().block(0, ws).bimoment("other", 1.0), None).map(|_| ())),
            "warping",
        );
    }
    // RF8 on the shear-centre torque: a lateral tip force on the fork beam (by a mid-span force).
    {
        let fm = fork(16, 3.0, &s, 0);
        let mut loaded = fm.clone();
        loaded.add_load(Load::nodal_force("mid", [0.0, 1000.0, 0.0])).unwrap();
        let (pre, _) = six_prestress(&loaded);
        check(
            "R RF8 sees the shear-centre torque (monosymmetric)",
            err(buckling_factors(&fm, &pre, &Warping::new().block(0, ws), &BucklingOptions::new(1)).map(|_| ())),
            "warping",
        );
        let (sd, wd) = WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, KAPPA).unwrap();
        let fd = fork(16, 3.0, &sd, 0);
        let mut ld = fd.clone();
        ld.add_load(Load::nodal_force("mid", [0.0, 1000.0, 0.0])).unwrap();
        let (pd, _) = six_prestress(&ld);
        let ok = buckling_factors(&fd, &pd, &Warping::new().block(0, wd), &BucklingOptions::new(1)).is_ok();
        c.holds("R the same with doubly_symmetric_i accepted", ok, "");
    }
    c.finish();
}
