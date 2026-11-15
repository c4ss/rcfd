//! F-668 (ADR 0592): the negative-pivot count of the pre-stressed warping harmonic and transient solves. Checks C1 to C5, S1
//! and R of the registration (B0 is the existing bit records). Each registered run is made once; every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    buckling_factors_with, load_vector, solve_static, Warping, WarpingHarmonic, WarpingHarmonicOptions,
    WarpingPrestress, WarpingSection, WarpingTransient,
};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::transient::{LoadHistory, LoadTable, TransientOptions};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

/// Registered results; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn count(&mut self, name: &str, got: &Result<usize, FemError>, want: usize) {
        match got {
            Ok(n) => self.holds(name, *n == want, &format!("{n} (registered {want})")),
            Err(e) => self.holds(name, false, &format!("refused: {e} (registered {want})")),
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const L: f64 = 3.0;
const T: f64 = 1000.0;

fn g_mod() -> f64 {
    E / (2.0 * (1.0 + NU))
}

fn i_parts() -> (BeamSection, WarpingSection) {
    WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, 5.0 / 6.0).unwrap()
}

/// The fork beam of `n` elements along `x`: `u = 0` at node 0, `v = w = phi = 0` at both ends; node sets `x0`, `xl`, `mid`,
/// `nodes`. No load.
fn fork(n: usize, section: &BeamSection) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=n).map(|i| [L * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(*section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    mesh.add_node_set("mid", vec![n / 2]).unwrap();
    mesh.add_node_set("nodes", (0..=n).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap())
        .unwrap();
    for c in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    for c in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", c, 0.0)).unwrap();
    }
    m
}

/// The torsion-only model of ADR 0548 F1: the fork beam with the components 0, 1, 2, 4, 5 held at every node.
fn torsion_only(n: usize, section: &BeamSection) -> SolidModel {
    let mut m = fork(n, section);
    for c in [0, 1, 2, 4, 5] {
        m.add_constraint(Constraint::displacement("nodes", c, 0.0)).unwrap();
    }
    m
}

fn with_torque(mut m: SolidModel) -> SolidModel {
    m.add_load(Load::nodal_moment("mid", [T, 0.0, 0.0])).unwrap();
    m
}

/// The pre-stress of the readings: the warping static solve of the `n`-element fork beam with the force `(n_ax, 0, 0)` at
/// `xl` (tension positive), `WarpingPrestress::from_static`.
fn axial_prestress(n: usize, s: &BeamSection, w: &Warping, n_ax: f64) -> WarpingPrestress {
    let mut l = fork(n, s);
    l.add_load(Load::nodal_force("xl", [n_ax, 0.0, 0.0])).unwrap();
    let r = solve_static(&l, w, None).unwrap();
    WarpingPrestress::from_static(&l, w, &r, [0.0; 2]).unwrap()
}

/// `|N_T| = (G J + E C_w k_1^2) A/I_p`.
fn n_t(s: &BeamSection, ws: &WarpingSection) -> f64 {
    let k = PI / L;
    (g_mod() * s.torsion() + E * ws.c_w() * k * k) * s.area() / (s.i_y() + s.i_z())
}

/// The registered time step `T_1(-0.5)/400` (ADR 0592 checks; ADR 0591 arithmetic 1).
const DT: f64 = 9.214657e-5;

fn key_of(e: &FemError) -> String {
    match e {
        FemError::InvalidValue { key, .. } => key.to_string(),
        FemError::FieldLength { key, .. } => key.to_string(),
        FemError::Element { source, .. } => key_of(source),
        other => format!("{other:?}"),
    }
}

/// The harmonic and the transient counts of `model` with the pre-stress `p` (none: the default constructors).
fn counts(
    model: &SolidModel,
    w: &Warping,
    p: Option<&WarpingPrestress>,
) -> (Result<usize, FemError>, Result<usize, FemError>) {
    let f = load_vector(model, w).unwrap();
    let lh = LoadHistory::new(f, LoadTable::constant(1.0, 0.0, 10.0).unwrap());
    let topts = TransientOptions::new(DT);
    let (h, t) = match p {
        Some(p) => (
            WarpingHarmonic::new(model, w, &WarpingHarmonicOptions::new().prestress(p.clone())).unwrap(),
            WarpingTransient::with_prestress(model, w, &[lh], &topts, p).unwrap(),
        ),
        None => (
            WarpingHarmonic::new(model, w, &WarpingHarmonicOptions::new()).unwrap(),
            WarpingTransient::new(model, w, &[lh], &topts).unwrap(),
        ),
    };
    (h.negative_eigenvalues(), t.negative_eigenvalues())
}

#[test]
fn c1_to_c5_counts_of_the_fork_beam() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let nt = n_t(&s, &ws);
    println!("|N_T| {nt:.10e}");
    // (label, fraction, torsion-only, registered count)
    let cases: [(&str, f64, bool, usize); 6] = [
        ("C1+/C2+", 0.5, false, 0),
        ("C1-/C2-", -0.5, false, 1),
        ("C3", -0.5, true, 0),
        ("C4", -0.25, false, 0),
        ("C4'", -1.2, false, 3),
        ("C4T", -1.2, true, 1),
    ];
    for (label, frac, tonly, want) in cases {
        let p = axial_prestress(16, &s, &w, frac * nt);
        let model = if tonly {
            with_torque(torsion_only(16, &s))
        } else {
            with_torque(fork(16, &s))
        };
        let (h, t) = counts(&model, &w, Some(&p));
        c.count(&format!("{label} harmonic, N = {frac} |N_T|"), &h, want);
        c.count(&format!("{label} transient, N = {frac} |N_T|"), &t, want);
        if let (Ok(a), Ok(b)) = (&h, &t) {
            c.holds(
                &format!("{label} transient equals harmonic"),
                a == b,
                &format!("{b} against {a}"),
            );
        }
    }
    let (h, t) = counts(&with_torque(fork(16, &s)), &w, None);
    c.count("C5 harmonic, no pre-stress", &h, 0);
    c.count("C5 transient, no pre-stress", &t, 0);
    c.finish();
}

#[test]
fn s1_buckling_factors_below_one_agree() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let nt = n_t(&s, &ws);
    for (frac, want) in [(-0.5, 1usize), (-1.2, 3)] {
        let p = axial_prestress(16, &s, &w, frac * nt);
        let model = with_torque(fork(16, &s));
        let r = buckling_factors_with(&fork(16, &s), &p, &w, &BucklingOptions::new(4)).unwrap();
        println!("S1 N = {frac} |N_T|: factors {:?}", r.factors);
        let below = r.factors.iter().filter(|&&x| x < 1.0).count();
        let (h, _) = counts(&model, &w, Some(&p));
        let ok = h.as_ref().is_ok_and(|&n| n == below) && below == want;
        c.holds(
            &format!("S1 N = {frac} |N_T|: factors below 1 against the count"),
            ok,
            &format!("{below} factors below 1, count {h:?} (registered {want})"),
        );
    }
    c.finish();
}

#[test]
fn r_singular_stiffness_is_refused() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    // The fork beam with the axial motion not held.
    let n = 16;
    let nodes: Vec<Vec3> = (0..=n).map(|i| [L * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(s, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    mesh.add_node_set("mid", vec![n / 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap())
        .unwrap();
    for set in ["x0", "xl"] {
        for comp in [1, 2, 3] {
            m.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
        }
    }
    let m = with_torque(m);
    let (h, t) = counts(&m, &w, None);
    for (name, r) in [("R harmonic", h), ("R transient", t)] {
        match r {
            Ok(k) => c.holds(name, false, &format!("counted {k}, registered a refusal")),
            Err(e) => {
                let key = key_of(&e);
                c.holds(name, key == "stiffness", &format!("refused, key {key:?}: {e}"));
            }
        }
    }
    c.finish();
}
