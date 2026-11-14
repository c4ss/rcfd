//! F-542 (ADR 0459): the bit record of the default paths that F-542 touches (check B1 of the registration).
//!
//! This file uses only the API of main 2504fd62: it was run once in the worktree at that commit (no library line changed)
//! before any library code of F-542, and the digests it printed are the constants of `BASE`. After the change it runs
//! unchanged and compares. The models cover the rotating modes (elimination and transformation, with and without stress
//! stiffening, a model with a temperature field and an expansion), the dense whirl modes, the rotating modes of a cyclic
//! sector, the buckling factors (both paths, a pre-stress from a static result and a uniform one) and the transient
//! integrator (both paths, a load history and Rayleigh damping).

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::{upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::modal::{ModalOptions, ModalResult};
use rcfd_fem::model::{ElasticSupport, LinearTerm};
use rcfd_fem::rotating::{HarmonicSpeedModes, RotatingOptions, RotatingPart, RotatingSector, SpeedModes};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, Method, Rayleigh, TransientOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// FNV-1a 64 over the count and the `to_bits()` of each value, little-endian bytes (the digest of ADR 0291).
fn digest<I: IntoIterator<Item = f64>>(values: I) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let mut n = 0u64;
    let mut buf = Vec::new();
    for v in values {
        buf.push(v.to_bits());
        n += 1;
    }
    eat(n);
    for b in buf {
        eat(b);
    }
    h
}

const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [-0.5, 0.0, 0.0];

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

fn opts(n: usize) -> ModalOptions {
    ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12)
}

fn modal_digests(tag: &str, r: &ModalResult, out: &mut Vec<(String, u64)>) {
    out.push((format!("{tag} eigenvalues"), digest(r.eigenvalues.iter().copied())));
    out.push((format!("{tag} modes"), digest(r.modes.iter().flatten().copied())));
}

fn speed_digests(tag: &str, s: SpeedModes, out: &mut Vec<(String, u64)>) {
    match s {
        SpeedModes::Modes(r) => modal_digests(tag, &r, out),
        SpeedModes::LossOfStiffness(l) => panic!("{tag}: unexpected loss of stiffness {l:?}"),
    }
}

/// A Hex20 cantilever 10 x 1 x 1 (`L = 1`, `h = 0.1`), clamped at `xmin`, optionally with a temperature field.
fn hex20_cantilever(temperature: bool) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 1, 1]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let t: Vec<f64> = mesh.nodes().iter().map(|x| 293.0 + 60.0 * x[0]).collect();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    if temperature {
        m.add_load(Load::temperature(t, 293.0)).unwrap();
    }
    m
}

/// A Hex8 cantilever 6 x 1 x 1 (`L = 1.2`, `h = 0.2`): `xmin` clamped in x and y, z held by linear constraints `u_z = 0`
/// node by node, a spring in z on the `xmax` nodes.
fn hex8_transformation() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [1.2, 0.2, 0.2], [6, 1, 1]).unwrap();
    let root: Vec<usize> = mesh.node_set("xmin").unwrap().to_vec();
    let tip: Vec<usize> = mesh.node_set("xmax").unwrap().to_vec();
    mesh.add_node_set("tip", tip).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 1, 0.0)).unwrap();
    for &n in &root {
        m.add_constraint(Constraint::linear(vec![LinearTerm::new(n, 2, 1.0)], 0.0))
            .unwrap();
    }
    m.add_support(ElasticSupport::nodal_spring("tip", 2, 2.0e6)).unwrap();
    m
}

/// The disc sector D of ADR 0395 (12 sectors, Hex20 3 x 2 x 1), `xmin` clamped, one cyclic constraint.
fn sector() -> SolidModel {
    let alpha = 2.0 * std::f64::consts::PI / 12.0;
    let b = hex_box([0.25, 0.0, -0.1], [0.75, alpha, 0.2], [3, 2, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, [0.0; 3], 12))
        .unwrap();
    m
}

fn run_transient(model: &SolidModel, out: &mut Vec<(String, u64)>, tag: &str) {
    let mut loaded = model.clone();
    loaded
        .add_load(Load::nodal_force("xmax", [0.0, 200.0, -500.0]))
        .unwrap();
    let history = LoadHistory::model_loads(&loaded, LoadTable::constant(1.0, 0.0, 1.0).unwrap()).unwrap();
    let options = TransientOptions::new(2.0e-4)
        .method(Method::average_acceleration())
        .damping(Rayleigh::from_ratio(0.02, 500.0, 5000.0).unwrap());
    let mut it = Integrator::new(model, &[history], &options).unwrap();
    let n = model.dof_map().n_equations();
    it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
    it.run(12).unwrap();
    out.push((
        format!("{tag} transient displacement"),
        digest(it.displacement().unwrap()),
    ));
    out.push((format!("{tag} transient velocity"), digest(it.velocity().unwrap())));
}

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    // Rotating modes on the elimination path, with a temperature field and an expansion (not part of the pre-stress).
    let m = hex20_cantilever(true);
    let part = RotatingPart::new(&m, AXIS, ORIGIN, &RotatingOptions::new(4).modal(opts(4))).unwrap();
    out.push((
        "hex20 rotating unit prestress".into(),
        digest(part.unit_prestress().stress().iter().flatten().copied()),
    ));
    out.push((
        "hex20 rotating unit geometric stiffness".into(),
        digest(part.unit_geometric_stiffness().val.iter().copied()),
    ));
    speed_digests("hex20 rotating 0", part.modes(0.0).unwrap(), &mut out);
    speed_digests("hex20 rotating 150", part.modes(150.0).unwrap(), &mut out);
    let soft = RotatingPart::new(
        &m,
        AXIS,
        ORIGIN,
        &RotatingOptions::new(4).modal(opts(4)).stress_stiffening(false),
    )
    .unwrap();
    speed_digests("hex20 rotating no stiffening 150", soft.modes(150.0).unwrap(), &mut out);
    // Dense whirl modes of the cantilever.
    let whirl = RotatingPart::new(
        &m,
        AXIS,
        ORIGIN,
        &RotatingOptions::new(3).modal(opts(3)).coriolis(true),
    )
    .unwrap();
    let w = whirl.whirl_modes(150.0).unwrap();
    let wm = w.modes().expect("whirl modes at 150 rad/s");
    out.push((
        "hex20 whirl 150".into(),
        digest(wm.modes.iter().flat_map(|x| [x.omega, x.whirl]).chain(wm.modes.iter().flat_map(|x| x.re.clone()))),
    ));
    // Rotating modes on the transformation path.
    let t = hex8_transformation();
    let tpart = RotatingPart::new(&t, AXIS, ORIGIN, &RotatingOptions::new(3).modal(opts(3))).unwrap();
    speed_digests("hex8 mpc rotating 100", tpart.modes(100.0).unwrap(), &mut out);
    // Rotating modes of a cyclic sector at index 1.
    let s = sector();
    let sec = RotatingSector::new(&s, &RotatingOptions::new(3).modal(opts(3))).unwrap();
    match sec.modes(1, 200.0).unwrap() {
        HarmonicSpeedModes::Modes(h) => {
            out.push(("sector rotating 1 200 eigenvalues".into(), digest(h.eigenvalues.iter().copied())));
        }
        HarmonicSpeedModes::LossOfStiffness(l) => panic!("sector: unexpected loss of stiffness {l:?}"),
    }
    // Buckling: from a static result on the elimination path, uniform on the transformation path.
    let c = hex20_cantilever(false);
    let mut cs = c.clone();
    cs.add_load(Load::nodal_force("xmax", [-2.0e4, 0.0, 0.0])).unwrap();
    let res = solve_static(&cs, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&cs, &res).unwrap();
    let b = factors(&cs, &pre, &BucklingOptions::new(2)).unwrap();
    out.push(("hex20 buckling factors".into(), digest(b.factors.iter().copied())));
    out.push(("hex20 buckling modes".into(), digest(b.modes.iter().flatten().copied())));
    let tpre = Prestress::uniform(&t, [-1.0e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let bt = factors(&t, &tpre, &BucklingOptions::new(2)).unwrap();
    out.push(("hex8 mpc buckling factors".into(), digest(bt.factors.iter().copied())));
    out.push(("hex8 mpc buckling modes".into(), digest(bt.modes.iter().flatten().copied())));
    // The transient integrator on both paths.
    run_transient(&c, &mut out, "hex20");
    run_transient(&t, &mut out, "hex8 mpc");
    out
}

/// The digests printed by the run at main 2504fd62 (`None` before that run).
const BASE: Option<&[(&str, u64)]> = Some(&[
    ("hex20 rotating unit prestress", 0x9aa0f5fbefa03c13),
    ("hex20 rotating unit geometric stiffness", 0x07242c8c81634eb7),
    ("hex20 rotating 0 eigenvalues", 0x234c1bb7b499970b),
    ("hex20 rotating 0 modes", 0x5a6edf6964e32343),
    ("hex20 rotating 150 eigenvalues", 0xc730fdad47471918),
    ("hex20 rotating 150 modes", 0xd72d2a1759500e8f),
    ("hex20 rotating no stiffening 150 eigenvalues", 0xbb63ca67747dd239),
    ("hex20 rotating no stiffening 150 modes", 0xa87231d7ba4f638a),
    ("hex20 whirl 150", 0x24448c214c32d57d),
    ("hex8 mpc rotating 100 eigenvalues", 0xdb4d62acc87d31fb),
    ("hex8 mpc rotating 100 modes", 0x4dd7c03fae08684e),
    ("sector rotating 1 200 eigenvalues", 0x57e377576baf684e),
    ("hex20 buckling factors", 0xa8ce1fce4f9ceab6),
    ("hex20 buckling modes", 0x7563fd204c82bc66),
    ("hex8 mpc buckling factors", 0xd5aea92828f16b61),
    ("hex8 mpc buckling modes", 0x888a45ad28f79e2f),
    ("hex20 transient displacement", 0x69a6f5e492518d7b),
    ("hex20 transient velocity", 0xc9224aecbcfc8800),
    ("hex8 mpc transient displacement", 0xabf42c0ab73c617b),
    ("hex8 mpc transient velocity", 0x71c28667eb68ccbb),
]);

#[test]
fn b1_default_paths_bits() {
    let got = all_digests();
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
    }
    let base = BASE.expect("the record of main 2504fd62 is filled in");
    let mut failed = Vec::new();
    assert_eq!(got.len(), base.len(), "number of digests");
    for ((name, d), (bname, bd)) in got.iter().zip(base) {
        let ok = name == bname && d == bd;
        println!("B1 {name}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            failed.push(name.clone());
        }
    }
    assert!(failed.is_empty(), "missed: {failed:?}");
}
