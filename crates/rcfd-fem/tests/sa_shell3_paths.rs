//! F-541 (ADR 0545): Shell3 on the remaining paths. The Shell4 checks of the paths that take Shell4 repeated on triangles where
//! they had no triangle repeat (D0, D2): TR1 to TR6 (ADR 0435 H1 to Z7, the transformation path), NT1 and NT2 (ADR 0522 F1 to
//! EZ, the nonlinear transient), AL1 and AL2 (ADR 0522 A2, the arc length); the consistency of the kept refusals RC1, RC2 and
//! the record of the known miss KM1 (D3). Each registered run is made once; every value is printed. Every aggregated error
//! uses a NaN-propagating maximum, and a NaN misses its check.

use rcfd_core::vec3::{cross, norm, Vec3};
use rcfd_fem::arclength::{trace, ArcLengthOptions, PathPoint};
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::damped::{damped_modes, DampedOptions};
use rcfd_fem::free_motions::free_rigid_motions;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::mpc::{modal_analysis_mpc, solve_static_mpc, ReferenceLoad};
use rcfd_fem::nl_transient::{Integrator as NlIntegrator, NlTransientOptions};
use rcfd_fem::nonlinear::{initial_states, solve, solve_in_time, NewtonOptions, TimeSteppingOptions};
use rcfd_fem::solve::{solve_static, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::transient::{Integrator as LinIntegrator, TransientOptions};
use rcfd_fem::{
    shell, shell3, BlockProperty, Constraint, ElasticSupport, ElementKind, FemError, LinearTerm, Load, Material,
    SolidMesh, SolidModel,
};
use rcfd_linalg::amg_elastic::ElasticAmgSettings;
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;
use std::time::Instant;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed (a NaN misses).
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
    fn record(&mut self, name: &str, value: f64) {
        println!("{name}: {value:.6e} (recorded)");
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

/// The larger of two values, NaN if either is NaN.
fn nmax(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// The largest absolute value, NaN if any value is NaN.
fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0f64, |m, x| nmax(m, x.abs()))
}

fn rel(meas: f64, exact: f64) -> f64 {
    ((meas - exact) / exact).abs()
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

// ------------------------------------------------------------------------------------ TR1 to TR6 (ADR 0435, D2)

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const HT: f64 = 0.05;

fn mat(nu: f64) -> Material {
    Material::isotropic_elastic(E, nu, RHO, 0.0).unwrap()
}

/// A grid of `nx x ny` cells on `[0, a] x [0, b]` at z = 0, nodes in rows of constant y (`j (nx + 1) + i`).
fn grid(a: f64, b: f64, nx: usize, ny: usize) -> (Vec<Vec3>, Vec<[usize; 4]>) {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([a * i as f64 / nx as f64, b * j as f64 / ny as f64, 0.0]);
        }
    }
    let mut el = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            el.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    (nodes, el)
}

/// Each cell `(a, b, c, d)` cut into the Shell3 `(a, b, c)` and `(a, c, d)` (D2).
fn cut(cells: &[[usize; 4]]) -> Vec<usize> {
    cells
        .iter()
        .flat_map(|&[a, b, c, d]| [a, b, c, a, c, d])
        .collect()
}

/// One block of the cells (Shell4) or of their cut (Shell3), element set "all", node set "all".
fn shell_mesh(nodes: Vec<Vec3>, cells: &[[usize; 4]], thickness: f64, tri: bool) -> SolidMesh {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let (kind, conn, ne) = if tri {
        (ElementKind::Shell3, cut(cells), 2 * cells.len())
    } else {
        (ElementKind::Shell4, cells.iter().flatten().copied().collect(), cells.len())
    };
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(thickness).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh
}

fn model_of(mesh: SolidMesh, m: Material) -> SolidModel {
    let mut model = SolidModel::new(mesh);
    model.add_material("all", m).unwrap();
    model
}

fn node_force_moment(model: &SolidModel, f: &[f64], node: usize) -> (Vec3, Vec3) {
    let s = model.dof_map().equation(node, 0).unwrap();
    ([f[s], f[s + 1], f[s + 2]], [f[s + 3], f[s + 4], f[s + 5]])
}

/// The plate `[0, 1]^2` of ADR 0435 H1, 4 x 4 cells cut, nodes `5 j + i`; sets "edge" (x = 0), "tip" (x = 1), "corner".
fn plate(pressure: bool) -> SolidModel {
    let (nodes, el) = grid(1.0, 1.0, 4, 4);
    let mut mesh = shell_mesh(nodes, &el, HT, true);
    mesh.add_node_set("edge", (0..5).map(|j| 5 * j).collect()).unwrap();
    mesh.add_node_set("tip", (0..5).map(|j| 5 * j + 4).collect()).unwrap();
    mesh.add_node_set("corner", vec![0]).unwrap();
    let mut m = model_of(mesh, mat(0.3));
    if pressure {
        m.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    }
    m
}

fn clamp(m: &mut SolidModel, set: &str) {
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
    }
}

/// The mixed model of ADR 0435 H2 with the plate cut: the fixed Hex8 block, the plate's nodes after the block's.
fn mixed() -> (SolidModel, usize) {
    let hex = hex_box([-0.2, 0.0, -0.1], [0.2, 1.0, 0.2], [1, 2, 1]).unwrap();
    let nh = hex.nodes().len();
    let (pn, pe) = grid(1.0, 1.0, 4, 4);
    let mut nodes = hex.nodes().to_vec();
    nodes.extend_from_slice(&pn);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    let ne_h = hex.blocks()[0].connectivity().len() / 8;
    let conn: Vec<usize> = cut(&pe).iter().map(|&k| nh + k).collect();
    let ne_s = conn.len() / 3;
    mesh.add_block(ElementKind::Shell3, conn).unwrap();
    mesh.set_block_property(1, BlockProperty::shell(HT).unwrap()).unwrap();
    mesh.add_element_set("solid", (0..ne_h).collect()).unwrap();
    mesh.add_element_set("shell", (ne_h..ne_h + ne_s).collect()).unwrap();
    mesh.add_node_set("block", (0..nh).collect()).unwrap();
    let edge: Vec<usize> = (0..5).map(|j| nh + 5 * j).collect();
    mesh.add_node_set("edge", edge.clone()).unwrap();
    let mut joint = hex.node_set("xmax").unwrap().to_vec();
    joint.extend_from_slice(&edge);
    mesh.add_node_set("joint", joint).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("solid", mat(0.3)).unwrap();
    m.add_material("shell", mat(0.3)).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("block", comp, 0.0)).unwrap();
    }
    m.add_load(Load::shell_pressure("shell", 1000.0)).unwrap();
    (m, nh)
}

/// The largest difference of the translations and of the rotations of `nodes`, each relative to the largest value of that
/// field in `b` (NaN-propagating).
fn field_diff(
    a: &rcfd_fem::SolidResult,
    b: &rcfd_fem::SolidResult,
    nodes: impl Iterator<Item = usize> + Clone,
) -> (f64, f64) {
    let mut out = [0.0; 2];
    for (f, range) in [(0usize, 0..3), (1, 3..6)] {
        let mut d = 0.0f64;
        let mut big = 0.0f64;
        for n in nodes.clone() {
            for k in range.clone() {
                d = nmax(d, (a.displacement(n)[k] - b.displacement(n)[k]).abs());
                big = nmax(big, b.displacement(n)[k].abs());
            }
        }
        out[f] = d / big;
    }
    (out[0], out[1])
}

fn modal_options() -> ModalOptions {
    ModalOptions::new(4)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward)
}

/// TR1 to TR3: ADR 0435 H1, H1b, H2, H2m, H3, H3r on the cut plate.
#[test]
fn tr1_to_tr3_a_shell3_plate_held_by_a_rigid_set() {
    let mut c = Checks::new();
    let mut clamped = plate(true);
    clamp(&mut clamped, "edge");
    let rc = solve_static(&clamped, &StaticOptions::default()).unwrap();
    for (name, reference) in [("TR1 (H1)", [0.0, 0.5, 0.0]), ("TR1 (H1b)", [0.0, 0.0, 0.0])] {
        let mut m = plate(true);
        m.add_constraint(Constraint::rigid_set("edge", reference)).unwrap();
        clamp(&mut m, "corner");
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        let (dt, dr) = field_diff(&r, &rc, 0..25);
        c.at_most(&format!("{name} translations against the clamped plate"), dt, 1e-10);
        c.at_most(&format!("{name} rotations against the clamped plate"), dr, 1e-10);
    }
    let (base, nh) = mixed();
    let mut mc = base.clone();
    clamp(&mut mc, "edge");
    let rmc = solve_static(&mc, &StaticOptions::default()).unwrap();
    let mut mj = base.clone();
    mj.add_constraint(Constraint::rigid_set("joint", [0.0, 0.5, 0.0]))
        .unwrap();
    let rmj = solve_static(&mj, &StaticOptions::default()).unwrap();
    let (dt, dr) = field_diff(&rmj, &rmc, nh..nh + 25);
    c.at_most("TR2 (H2) translations against the clamped plate", dt, 1e-10);
    c.at_most("TR2 (H2) rotations against the clamped plate", dr, 1e-10);
    let fc = modal_analysis(&mc, &modal_options()).unwrap().frequencies;
    let fj = modal_analysis(&mj, &modal_options()).unwrap().frequencies;
    for i in 0..4 {
        c.at_most(
            &format!("TR2 (H2m) frequency {} ({:.6} Hz)", i + 1, fc[i]),
            rel(fj[i], fc[i]),
            1e-8,
        );
    }
    let mut m = plate(false);
    clamp(&mut m, "edge");
    let x_r = [1.0, 0.5, 0.0];
    m.add_constraint(Constraint::rigid_set("tip", x_r)).unwrap();
    let (force, moment) = ([150.0, 300.0, 500.0], [100.0, -40.0, 25.0]);
    let s = solve_static_mpc(
        &m,
        &[],
        &[],
        &[ReferenceLoad::new(6, force, moment)],
        &StaticOptions::default(),
    )
    .unwrap();
    let (t, w) = (s.references[0].translation, s.references[0].rotation);
    let nodes = m.mesh().nodes().to_vec();
    let (mut du, mut dth, mut umax) = (0.0f64, 0.0f64, 0.0f64);
    for a in (0..5).map(|j| 5 * j + 4) {
        let d = s.result.displacement(a);
        let rigid = add(t, cross(w, sub(nodes[a], x_r)));
        du = nmax(du, max_abs(sub([d[0], d[1], d[2]], rigid)));
        dth = nmax(dth, max_abs(sub([d[3], d[4], d[5]], w)));
        umax = nmax(umax, max_abs([d[0], d[1], d[2]]));
    }
    c.at_most("TR3 (H3) tip translations u_a - t - w x d_a", du / umax, 1e-12);
    c.at_most("TR3 (H3) tip rotations theta_a - w", dth / norm(w), 1e-12);
    let reactions = s.result.reactions().unwrap();
    let (mut rf, mut rm) = ([0.0; 3], [0.0; 3]);
    for j in 0..5 {
        let a = 5 * j;
        let (fa, ma) = node_force_moment(&m, reactions, a);
        rf = add(rf, fa);
        rm = add(rm, add(cross(nodes[a], fa), ma));
    }
    c.at_most(
        "TR3 (H3r) root reaction forces balance the reference force",
        norm(add(rf, force)) / norm(force),
        1e-10,
    );
    c.at_most(
        "TR3 (H3r) root reaction moments balance (about the origin)",
        norm(add(add(rm, cross(x_r, force)), moment)) / (norm(x_r) * norm(force) + norm(moment)),
        1e-10,
    );
    // NC1: the rigid set alone leaves the six rigid motions.
    let mut nc = plate(true);
    nc.add_constraint(Constraint::rigid_set("edge", [0.0, 0.5, 0.0]))
        .unwrap();
    let z = free_rigid_motions(&nc, &[]).unwrap().count;
    c.holds("NC1 the rigid set alone: six free rigid motions", z == 6, &format!("{z}"));
    c.finish();
}

const KB: f64 = 0.2;
const KT: f64 = 0.05;

/// The strip `[0, 1] x [0, 0.2]` of ADR 0435 arithmetic 4, 4 x 1 cells (cut for Shell3), `t = 0.05`, `nu = 0`.
fn kstrip(tri: bool) -> SolidModel {
    let (nodes, el) = grid(1.0, KB, 4, 1);
    let mut mesh = shell_mesh(nodes, &el, KT, tri);
    mesh.add_node_set("root", vec![0, 5]).unwrap();
    mesh.add_node_set("tip", vec![4, 9]).unwrap();
    mesh.add_node_set("root0", vec![0]).unwrap();
    model_of(mesh, mat(0.0))
}

/// TR4, TR5: ADR 0435 K1, K2 on the cut strip; NC2.
#[test]
fn tr4_tr5_a_nodal_spring_on_a_shell3_rotation() {
    let mut c = Checks::new();
    let ei = E * KB * KT.powi(3) / 12.0;
    let k = 10.0 * ei;
    for (name, at_tip, mm) in [("TR4 (K1)", false, 3000.0), ("TR5 (K2)", true, 1500.0)] {
        let mut m = kstrip(true);
        for comp in [0, 1, 2, 3, 5] {
            m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
        }
        m.add_support(ElasticSupport::nodal_spring("root", 4, k)).unwrap();
        m.add_load(Load::nodal_moment(if at_tip { "tip" } else { "root" }, [0.0, mm, 0.0]))
            .unwrap();
        let s = solve_static_mpc(&m, &[], &[], &[], &StaticOptions::default()).unwrap();
        let dofs = m.dof_map();
        let nodes = m.mesh().nodes().to_vec();
        for root in [0usize, 5] {
            let th = s.result.displacement(root)[4];
            c.at_most(
                &format!("{name} node {root}: |k_theta theta - M| / M"),
                (k * th - mm).abs() / mm,
                1e-10,
            );
            if !at_tip {
                let e = dofs.equation(root, 4).unwrap();
                c.at_most(
                    &format!("{name} node {root}: spring force |S + M| / M"),
                    (s.support_forces[e] + mm).abs() / mm,
                    1e-10,
                );
            }
        }
        if at_tip {
            let th_root = mm / k;
            let m_tot = 2.0 * mm;
            for tip in [4usize, 9] {
                let d = s.result.displacement(tip);
                c.at_most(
                    &format!("{name} node {tip}: tip rotation theta_root + M L / (E I)"),
                    rel(d[4], th_root + m_tot / ei),
                    1e-10,
                );
                c.at_most(
                    &format!("{name} node {tip}: tip deflection -(theta_root L + M L^2 / (2 E I))"),
                    rel(d[2], -(th_root + m_tot / (2.0 * ei))),
                    1e-10,
                );
                // NC2: the closed form with the bending stiffness of t = 0.06 differs.
                let ei_wrong = E * KB * 0.06f64.powi(3) / 12.0;
                let nc2 = rel(d[4], th_root + m_tot / ei_wrong);
                c.holds(
                    &format!("NC2 node {tip}: the wrong E I differs by > 0.3"),
                    nc2 > 0.3 && nc2.is_finite(),
                    &format!("{nc2:.4e}"),
                );
            }
        } else {
            let th = mm / k;
            let (mut dth, mut duz) = (0.0f64, 0.0f64);
            for (n, x) in nodes.iter().enumerate() {
                let d = s.result.displacement(n);
                dth = nmax(dth, (d[4] - th).abs());
                duz = nmax(duz, (d[2] + th * x[0]).abs());
            }
            c.at_most(&format!("{name} every node turns rigidly: theta_y"), dth / th, 1e-10);
            c.at_most(&format!("{name} every node turns rigidly: u_z = -theta x"), duz / th, 1e-10);
        }
    }
    c.finish();
}

/// TR6: ADR 0435 Z1 to Z7 on the cut strip, plate and mixed model.
#[test]
fn tr6_free_rigid_motions_with_shell3_nodes() {
    let mut c = Checks::new();
    let count = |m: &SolidModel| free_rigid_motions(m, &[]).unwrap().count;
    let free = kstrip(true);
    let z1 = count(&free);
    c.holds("TR6 (Z1) free strip", z1 == 6, &format!("{z1}"));
    let mut z2m = kstrip(true);
    for comp in 0..3 {
        z2m.add_constraint(Constraint::displacement("root", comp, 0.0)).unwrap();
    }
    let z2 = count(&z2m);
    c.holds("TR6 (Z2) root translations", z2 == 1, &format!("{z2}"));
    let mut z3m = z2m.clone();
    z3m.add_support(ElasticSupport::nodal_spring("root0", 4, 1e6)).unwrap();
    let z3 = count(&z3m);
    c.holds("TR6 (Z3) root translations, spring on theta_y", z3 == 0, &format!("{z3}"));
    let mut z4m = kstrip(true);
    for comp in 0..4 {
        z4m.add_constraint(Constraint::displacement("root0", comp, 0.0))
            .unwrap();
    }
    z4m.add_constraint(Constraint::linear(vec![LinearTerm::new(0, 5, 1.0)], 0.0))
        .unwrap();
    let z4 = count(&z4m);
    c.holds(
        "TR6 (Z4) node 0 translations and theta_x, Linear theta_z",
        z4 == 1,
        &format!("{z4}"),
    );
    let mut z5m = plate(true);
    z5m.add_constraint(Constraint::rigid_set("edge", [0.0, 0.5, 0.0]))
        .unwrap();
    let z5 = count(&z5m);
    c.holds("TR6 (Z5) plate with a rigid set on its edge only", z5 == 6, &format!("{z5}"));
    let mut h1 = z5m.clone();
    clamp(&mut h1, "corner");
    let z6a = count(&h1);
    c.holds("TR6 (Z6) the H1 model", z6a == 0, &format!("{z6a}"));
    let (mut h2, _) = mixed();
    h2.add_constraint(Constraint::rigid_set("joint", [0.0, 0.5, 0.0]))
        .unwrap();
    let z6b = count(&h2);
    c.holds("TR6 (Z6) the H2 model", z6b == 0, &format!("{z6b}"));
    let frm = free_rigid_motions(&free, &[]).unwrap();
    let k = stiffness(&free).unwrap();
    let kmax = max_abs(k.val.iter().copied());
    let mut worst = 0.0f64;
    for x in frm.fields(&free) {
        let mut kx = vec![0.0; x.len()];
        k.matvec(&x, &mut kx);
        worst = nmax(worst, max_abs(kx) / (kmax * max_abs(x.iter().copied())));
    }
    c.at_most("TR6 (Z7) K X c of the free fields", worst, 1e-12);
    c.finish();
}

// ------------------------------------------------------------------------------------ RC1, RC2, KM1 (D3)

/// The strip of TR4 (cut or not), root clamped.
fn held_strip(tri: bool) -> SolidModel {
    let mut m = kstrip(tri);
    clamp(&mut m, "root");
    m.add_load(Load::nodal_force("tip", [0.0, 0.0, -10.0])).unwrap();
    m
}

/// The cyclic model of ADR 0454 N2: a Hex8 box and a separate shell element (Shell3, or Shell4), a cyclic constraint.
fn cyclic_model(tri: bool) -> SolidModel {
    let hex = hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let nh = hex.nodes().len();
    let mut nodes = hex.nodes().to_vec();
    nodes.extend_from_slice(&[[3.0, 0.0, 0.0], [4.0, 0.0, 0.0], [3.0, 1.0, 0.0]]);
    if !tri {
        nodes.push([4.0, 1.0, 0.0]);
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, hex.blocks()[0].connectivity().to_vec())
        .unwrap();
    if tri {
        mesh.add_block(ElementKind::Shell3, vec![nh, nh + 1, nh + 2]).unwrap();
    } else {
        mesh.add_block(ElementKind::Shell4, vec![nh, nh + 1, nh + 3, nh + 2])
            .unwrap();
    }
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    for name in ["ymin", "ymax"] {
        let faces = hex.surface_set(name).unwrap().to_vec();
        mesh.add_surface_set(name, faces).unwrap();
    }
    let mut m = model_of(mesh, mat(0.3));
    m.add_constraint(Constraint::cyclic("ymin", "ymax", [0.0, 0.0, 1.0], [0.0; 3], 4))
        .unwrap();
    m
}

fn amg() -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default()),
        tolerance: 1e-10,
        max_iterations: 100,
    })
}

/// The outcome of one entry path on a model, as `Ok(())` or the error.
type Outcome = Result<(), FemError>;

/// The entry paths of RC1 (D3): name, whether the cyclic model is used, the call.
fn paths() -> Vec<(&'static str, bool, Box<dyn Fn(&SolidModel) -> Outcome>)> {
    vec![
        ("solve_static (default)", false, Box::new(|m| solve_static(m, &StaticOptions::default()).map(|_| ()))),
        ("modal_analysis", false, Box::new(|m| modal_analysis(m, &ModalOptions::new(2)).map(|_| ()))),
        ("nonlinear::initial_states", false, Box::new(|m| initial_states(m).map(|_| ()))),
        ("solve_static smoothed-aggregation CG", false, Box::new(|m| solve_static(m, &amg()).map(|_| ()))),
        ("check_supported", false, Box::new(|m| m.check_supported())),
        (
            "transient::Integrator::new",
            false,
            Box::new(|m| LinIntegrator::new(m, &[], &TransientOptions::new(1e-3)).map(|_| ())),
        ),
        ("damped::damped_modes", false, Box::new(|m| damped_modes(m, &DampedOptions::new(2)).map(|_| ()))),
        (
            "harmonic_direct::DirectHarmonic::new",
            false,
            Box::new(|m| DirectHarmonic::new(m, &DirectHarmonicOptions::new()).map(|_| ())),
        ),
        (
            "nonlinear::solve_in_time",
            false,
            Box::new(|m| solve_in_time(m, &[], &[0.0, 1.0], &TimeSteppingOptions::new(f64::INFINITY)).map(|_| ())),
        ),
        (
            "nl_transient::Integrator::new energy-momentum",
            false,
            Box::new(|m| NlIntegrator::new(m, &[], &NlTransientOptions::new(1e-3).energy_momentum()).map(|_| ())),
        ),
        (
            "follower::tangent_modes",
            false,
            Box::new(|m| {
                let states = initial_states(m)?;
                let u = vec![0.0; m.dof_map().n_equations()];
                let none: [Prescribed; 0] = [];
                rcfd_fem::follower::tangent_modes(m, &none, &u, &states, 0.0, &[], &ModalOptions::new(1)).map(|_| ())
            }),
        ),
        (
            "mpc::solve_static_mpc cyclic",
            true,
            Box::new(|m| solve_static_mpc(m, &[], &[], &[], &StaticOptions::default()).map(|_| ())),
        ),
        (
            "mpc::modal_analysis_mpc cyclic",
            true,
            Box::new(|m| modal_analysis_mpc(m, &[], &ModalOptions::new(2)).map(|_| ())),
        ),
        (
            "cyclic::modal_analysis_harmonic",
            true,
            Box::new(|m| rcfd_fem::cyclic::modal_analysis_harmonic(m, &[], 0, &ModalOptions::new(2)).map(|_| ())),
        ),
    ]
}

/// The key of an `InvalidValue` (or of the source of an `Element`), else empty.
fn key_of(e: &FemError) -> String {
    match e {
        FemError::InvalidValue { key, .. } => key.to_string(),
        FemError::Element { source, .. } => format!("element: {}", key_of(source)),
        _ => String::new(),
    }
}

/// Whether the Shell3 outcome `s3` is consistent with the Shell4 outcome `s4` by the rule of D3.
fn consistent(s4: &Outcome, s3: &Outcome) -> bool {
    match (s4, s3) {
        (Ok(()), Ok(())) => true,
        (
            Err(FemError::NotYetSupported {
                item: i4,
                variant: v4,
                package: p4,
                ..
            }),
            Err(FemError::NotYetSupported {
                item: i3,
                variant: v3,
                package: p3,
                ..
            }),
        ) if *p4 == shell::FOLLOW_UP => {
            i4 == i3 && *p3 == shell3::FOLLOW_UP && (if *v4 == "Shell4" { *v3 == "Shell3" } else { v3 == v4 })
        }
        (Err(a), Err(b)) => std::mem::discriminant(a) == std::mem::discriminant(b) && key_of(a) == key_of(b),
        _ => false,
    }
}

/// RC1 and RC2 (D3), NC4.
#[test]
fn rc1_rc2_shell3_taken_where_shell4_is_taken() {
    let mut c = Checks::new();
    let (s4, s3) = (held_strip(false), held_strip(true));
    let (c4, c3) = (cyclic_model(false), cyclic_model(true));
    let (mut both_ok, mut both_err) = (0, 0);
    let mut outcomes = Vec::new();
    for (name, cyc, call) in paths() {
        let (m4, m3) = if cyc { (&c4, &c3) } else { (&s4, &s3) };
        let r4 = call(m4);
        let r3 = call(m3);
        match (&r4, &r3) {
            (Ok(()), Ok(())) => both_ok += 1,
            (Err(_), Err(_)) => both_err += 1,
            _ => {}
        }
        c.holds(
            &format!("RC1 {name}"),
            consistent(&r4, &r3),
            &format!("Shell4 {:?} / Shell3 {:?}", r4.as_ref().err(), r3.as_ref().err()),
        );
        outcomes.push((name, r4, r3));
    }
    c.holds(
        "NC4 RC1 has rows taken by both and rows refused by both",
        both_ok >= 1 && both_err >= 1,
        &format!("{both_ok} taken, {both_err} refused"),
    );
    for row in ["solve_static smoothed-aggregation CG", "check_supported", "mpc::solve_static_mpc cyclic"] {
        let (_, r4, r3) = outcomes.iter().find(|(n, _, _)| *n == row).unwrap();
        let ok4 = matches!(r4, Err(FemError::NotYetSupported { package, .. }) if *package == shell::FOLLOW_UP);
        let ok3 = matches!(r3, Err(FemError::NotYetSupported { package, .. }) if *package == shell3::FOLLOW_UP);
        c.holds(
            &format!("RC2 N2 row '{row}' refused for both kinds"),
            ok4 && ok3,
            &format!("{r4:?} / {r3:?}"),
        );
    }
    c.finish();
}

/// KM1 (D3): the three N2 rows taken for Shell3. A predicted miss: those rows refuse Shell4 too (D0); run once with
/// `--ignored` to record the refusals.
#[test]
#[ignore = "known miss of ADR 0545 KM1 (predicted at registration): the N2 rows multigrid, check_supported and cyclic refuse Shell4 too, so Shell3 stays refused (D0); refusal texts recorded in addendum 1"]
fn km1_n2_rows_taken_known_miss() {
    let mut c = Checks::new();
    let m = held_strip(true);
    let r = solve_static(&m, &amg()).map(|_| ());
    c.holds("KM1 smoothed-aggregation CG takes Shell3", r.is_ok(), &format!("{r:?}"));
    let r = m.check_supported();
    c.holds("KM1 check_supported takes Shell3", r.is_ok(), &format!("{r:?}"));
    let mc = cyclic_model(true);
    let r = solve_static_mpc(&mc, &[], &[], &[], &StaticOptions::default()).map(|_| ());
    c.holds("KM1 cyclic solve_static_mpc takes Shell3", r.is_ok(), &format!("{r:?}"));
    c.finish();
}

// ------------------------------------------------------------------------------------ NT1, NT2 (ADR 0522 D6)

const N_S: usize = 16;
const L_S: f64 = 12.0;
const B_S: f64 = 1.0;
const T_S: f64 = 0.1;
const E_S: f64 = 1.2e6;
const RHO_S: f64 = 1.0;
const BETA_1: f64 = 1.875_104_068_711_961;
const TIP_FORCE_A2: f64 = -1.042e-3;

fn ei_s() -> f64 {
    E_S * B_S * T_S.powi(3) / 12.0
}

fn f_eb() -> f64 {
    BETA_1 * BETA_1 / (2.0 * PI * L_S * L_S) * (ei_s() / (RHO_S * B_S * T_S)).sqrt()
}

/// The cantilever strip of ADR 0522 D6 cut into 32 Shell3, clamped at `x = 0`; the tip node of `y = 0` is node `N_S`.
fn strip3() -> SolidModel {
    let (nodes, el) = grid(L_S, B_S, N_S, 1);
    let mesh = shell_mesh(nodes, &el, T_S, true);
    let mut mesh = mesh;
    mesh.add_node_set("root", vec![0, N_S + 1]).unwrap();
    mesh.add_node_set("tip", vec![N_S, 2 * N_S + 1]).unwrap();
    let mut m = model_of(mesh, Material::isotropic_elastic(E_S, 0.0, RHO_S, 0.0).unwrap());
    clamp(&mut m, "root");
    m
}

/// The modal options of ADR 0522 amendment 4.
fn strip_modal() -> ModalOptions {
    let mut mo = ModalOptions::new(1);
    mo.max_iterations = 2000;
    mo.tolerance = 1e-8;
    mo
}

/// NT2: F1c on the Shell3 strip (fast).
#[test]
fn nt2_shell3_strip_first_frequency_against_euler_bernoulli() {
    let mut c = Checks::new();
    let f1 = modal_analysis(&strip3(), &strip_modal()).unwrap().frequencies[0];
    println!("NT2 f_1 {f1:.10e} Hz, f_EB {:.10e} Hz", f_eb());
    c.at_most("NT2 (F1c Shell3) |f_1/f_EB - 1|", (f1 / f_eb() - 1.0).abs(), 1e-2);
    c.record("NT2 signed f_1/f_EB - 1", f1 / f_eb() - 1.0);
    c.finish();
}

/// EZ of ADR 0522 amendment 4 on the Shell3 strip.
fn ez_clapeyron() -> f64 {
    let mut m = strip3();
    m.add_load(Load::nodal_force("tip", [0.0, 0.0, TIP_FORCE_A2])).unwrap();
    let r = solve(&m, &[], &[], &NewtonOptions::new(1)).unwrap();
    let us = r.result.displacement_values().to_vec();
    let n = us.len();
    let mut it = NlIntegrator::new(&m, &[], &NlTransientOptions::new(1.0)).unwrap();
    it.start(0.0, &us, &vec![0.0; n]).unwrap();
    let pi = it.strain_energy().unwrap();
    let f = nodal_load_vector(&m, &[]).unwrap();
    let work = 0.5 * f.iter().zip(&us).map(|(x, y)| x * y).sum::<f64>();
    println!("EZ Shell3: Pi {pi:.10e}, f . u/2 {work:.10e}");
    (pi / work - 1.0).abs()
}

/// NT1: the 100-period free vibration of ADR 0522 D6 and amendment 4 on the Shell3 strip. Heavy (4000 steps).
#[test]
#[ignore = "heavy registered record of ADR 0545 NT1 (100 periods of the Shell3 strip); run once with --ignored"]
fn nt1_shell3_strip_100_periods() {
    let mut c = Checks::new();
    let start = Instant::now();
    let m = strip3();
    let modal = modal_analysis(&m, &strip_modal()).unwrap();
    let f1 = modal.frequencies[0];
    let qz = m.dof_map().equation(N_S, 2).expect("tip u_z");
    let phi = &modal.modes[0];
    let a = 1e-3 * L_S;
    let omega = 2.0 * PI * f1;
    let v0: Vec<f64> = phi.iter().map(|x| omega * a * x / phi[qz]).collect();
    let n = v0.len();
    let h = 1.0 / (40.0 * f1);
    let mut it = NlIntegrator::new(&m, &[], &NlTransientOptions::new(h)).unwrap();
    it.start(0.0, &vec![0.0; n], &v0).unwrap();
    let h0 = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
    let mut energy_error = 0.0;
    let mut prev = (0.0, 0.0);
    let mut cross_t = vec![0.0];
    let (mut halvings, mut iterations) = (0, 0);
    for step in 1..=40 * 100u64 {
        let rep = it.step().unwrap();
        halvings += rep.halvings;
        iterations += rep.iterations;
        let t = step as f64 * h;
        let uz = it.displacement().unwrap()[qz];
        let hn = it.kinetic_energy().unwrap() + it.strain_energy().unwrap();
        energy_error = nmax(energy_error, ((hn - h0) / h0).abs());
        if step > 1 && (prev.1 > 0.0) != (uz > 0.0) {
            cross_t.push(prev.0 + h * prev.1 / (prev.1 - uz));
        }
        prev = (t, uz);
    }
    let kc = cross_t.len();
    let fm = if kc >= 2 {
        (kc - 1) as f64 / (2.0 * (cross_t[kc - 1] - cross_t[0]))
    } else {
        f64::NAN
    };
    let r = 2.0 * (0.5 * omega * h).atan() / (omega * h);
    let ez = ez_clapeyron();
    println!(
        "NT1 Shell3 strip 100 periods: f_1 {f1:.10e} Hz, f_m {fm:.10e} Hz, r {r:.8}, {kc} crossings, H_0 {h0:.6e}, \
         max |dH|/H_0 {energy_error:.4e}, EZ {ez:.4e}, {halvings} halvings, {iterations} iterations, {:.1} s",
        start.elapsed().as_secs_f64()
    );
    c.at_most("NT1 (F1) |f_m/f_1 - 1|", (fm / f1 - 1.0).abs(), 1e-2);
    c.at_most("NT1 (F1b) |f_m/(f_1 r) - 1|", (fm / (f1 * r) - 1.0).abs(), 1e-3);
    c.at_most("NT1 (F1c) |f_1/f_EB - 1|", (f1 / f_eb() - 1.0).abs(), 1e-2);
    c.at_most("NT1 (E1) max |H_n - H_0|/H_0", energy_error, 1e-5);
    c.at_most("NT1 (EZ) |Pi(u_s)/(f . u_s/2) - 1|", ez, 1e-4);
    c.record("NT1 signed f_m/f_1 - 1", fm / f1 - 1.0);
    c.record("NT1 wall time s", start.elapsed().as_secs_f64());
    c.finish();
}

// ------------------------------------------------------------------------------------ AL1, AL2 (ADR 0522 D5)

const R_R: f64 = 2540.0;
const HALF_LENGTH: f64 = 254.0;
const THETA_0: f64 = 0.1;
const T_R: f64 = 12.7;
const E_R: f64 = 3102.75;
const NU_R: f64 = 0.3;
const P_QUARTER: f64 = 250.0;
const P_0: f64 = 1000.0;

/// The quarter roof of ADR 0522 D5, `n x n` cells (cut for Shell3); returns the model and the apex node.
fn roof_shell(n: usize, tri: bool) -> (SolidModel, usize) {
    let mut nodes = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let s = THETA_0 * i as f64 / n as f64;
            let y = HALF_LENGTH * j as f64 / n as f64;
            nodes.push([R_R * s.sin(), y, R_R * s.cos() - R_R]);
        }
    }
    let idx = |i: usize, j: usize| j * (n + 1) + i;
    let mut cells = Vec::new();
    for j in 0..n {
        for i in 0..n {
            cells.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut mesh = shell_mesh(nodes, &cells, T_R, tri);
    mesh.add_node_set("sym_x", (0..=n).map(|j| idx(0, j)).collect())
        .unwrap();
    mesh.add_node_set("sym_y", (0..=n).map(|i| idx(i, 0)).collect())
        .unwrap();
    mesh.add_node_set("hinge", (0..=n).map(|j| idx(n, j)).collect())
        .unwrap();
    mesh.add_node_set("apex", vec![idx(0, 0)]).unwrap();
    let mut m = model_of(mesh, Material::isotropic_elastic(E_R, NU_R, 1.0, 0.0).unwrap());
    for c in [0, 4, 5] {
        m.add_constraint(Constraint::displacement("sym_x", c, 0.0)).unwrap();
    }
    for c in [1, 3, 5] {
        m.add_constraint(Constraint::displacement("sym_y", c, 0.0)).unwrap();
    }
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("hinge", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("apex", [0.0, 0.0, -P_QUARTER])).unwrap();
    (m, idx(0, 0))
}

/// What one roof trace gives.
struct Limit {
    p_lim: f64,
    stopped: bool,
    negative_after: bool,
}

/// The vertex of the parabola through three points, if it is concave.
fn vertex(p: [(f64, f64); 3]) -> Option<(f64, f64)> {
    let [(x0, y0), (x1, y1), (x2, y2)] = p;
    let d01 = (y1 - y0) / (x1 - x0);
    let d12 = (y2 - y1) / (x2 - x1);
    let a = (d12 - d01) / (x2 - x0);
    if a.is_nan() || a >= 0.0 {
        return None;
    }
    let b = d01 - a * (x0 + x1);
    let xv = -b / (2.0 * a);
    let yv = y0 + d01 * (xv - x0) + a * (xv - x0) * (xv - x1);
    Some((xv, yv))
}

/// The trace of `a2_fast_roof_shell4_and_hex20` (ADR 0522 amendment 1).
fn roof_limit(name: &str, model: &SolidModel, apex: usize) -> Limit {
    let start = Instant::now();
    let lin = solve_static(model, &StaticOptions::default()).unwrap();
    let c: f64 = lin.displacement_values().iter().map(|x| x * x).sum();
    assert!(c > 0.0 && c.is_finite(), "{name}: load scale {c:e}");
    let options = ArcLengthOptions::new(0.03 * (2.0 * c).sqrt())
        .load_scale(c)
        .max_points(300)
        .newton(NewtonOptions::new(1));
    let mut lam_max = f64::NEG_INFINITY;
    let mut stop = |p: &PathPoint| {
        let past = lam_max > 0.0 && p.load_factor <= 0.8 * lam_max;
        lam_max = lam_max.max(p.load_factor);
        past
    };
    let path = trace(model, &[], &[], &options, &[], &mut stop).unwrap();
    let qz = model.dof_map().equation(apex, 2).expect("apex u_z");
    let pts: Vec<(f64, f64)> = path
        .points
        .iter()
        .map(|p| (-p.displacement[qz], p.load_factor))
        .collect();
    let mut imax = 0;
    for (i, p) in pts.iter().enumerate() {
        if p.1.is_nan() || p.1 > pts[imax].1 {
            imax = i;
        }
    }
    let fit = if imax >= 1 && imax + 1 < pts.len() {
        vertex([pts[imax - 1], pts[imax], pts[imax + 1]])
    } else {
        None
    };
    let (w_at, lam) = fit.unwrap_or(pts[imax]);
    let negative_after = path.points[imax + 1..].iter().any(|p| p.negative_pivots >= 1);
    println!(
        "{name}: P_lim {:.6e} N (vertex {}), apex deflection at the limit {:.4e}, {} points, {} halvings, stopped {}, \
         negative pivot after the limit {}, {:.1} s",
        P_0 * lam,
        fit.is_some(),
        w_at,
        path.points.len(),
        path.points.iter().map(|p| p.halvings).sum::<usize>(),
        path.stopped,
        negative_after,
        start.elapsed().as_secs_f64()
    );
    Limit {
        p_lim: P_0 * lam,
        stopped: path.stopped,
        negative_after,
    }
}

/// AL1, AL2: the Shell3 roof 8 x 8 x 2 through its limit point, against the Shell4 8 x 8.
#[test]
fn al1_al2_shell3_roof_through_the_limit_point() {
    let mut c = Checks::new();
    let (m, apex) = roof_shell(8, true);
    let s3 = roof_limit("AL Shell3 8 x 8 x 2", &m, apex);
    c.holds(
        "AL1 Shell3 8: stopped by the rule past the limit, a negative pivot after it",
        s3.stopped && s3.negative_after && s3.p_lim.is_finite(),
        &format!(
            "stopped {}, negative after {}, P_lim {:e}",
            s3.stopped, s3.negative_after, s3.p_lim
        ),
    );
    let (m, apex) = roof_shell(8, false);
    let s4 = roof_limit("AL Shell4 8 x 8", &m, apex);
    c.at_most("AL2 |P_lim(Shell3 8)/P_lim(Shell4 8) - 1|", (s3.p_lim / s4.p_lim - 1.0).abs(), 1e-2);
    c.record("AL P_lim(Shell3 8), N", s3.p_lim);
    c.record("AL P_lim(Shell4 8), N", s4.p_lim);
    c.record("NC3 P_lim(Shell3 8)/2221.3 - 1 (Shell3 16 of ADR 0522)", s3.p_lim / 2221.3 - 1.0);
    c.finish();
}
