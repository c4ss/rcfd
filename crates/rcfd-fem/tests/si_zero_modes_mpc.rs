//! F-488 (ADR 0399): zero modes on the transformation. The eigenvalue measure of the zero modes against the automatic shift,
//! their number against the free rigid motions counted from the springs, the linear rows and the rigid sets
//! (`rcfd_fem::free_motions`), and the report of `rigid_free` with a shift given by the caller. Checks Z1, Z2, R1, S1, C1,
//! E1 to E3 and the negative controls N1 to N3 of the registration. The models of ADR 0384 are rebuilt here by copies of the
//! helpers of `si_modal_mpc.rs` (that file is not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::free_motions::{free_rigid_motions, FreeRigidMotions};
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis_with, ModalOptions, ModalResult};
use rcfd_fem::mpc::{self, MpcModes};
use rcfd_fem::{Constraint, ElasticSupport, FemError, LinearTerm, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::Shift;
use rcfd_linalg::CsrMatrix;

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const L: f64 = 1.0;
const B: f64 = 0.1;
const K_SPRING: f64 = 5e8;
const N_EL: usize = 20;
/// The spring of the free blocks (ADR 0399), N/m.
const K_BLOCK: f64 = 1e7;
/// The zero-mode measure of the acceptance: `|lambda| <= 1e-9 sigma`.
const ZERO_MEASURE: f64 = 1e-9;

fn model_of(mesh: SolidMesh, nu: f64) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, nu, RHO, 0.0).unwrap())
        .unwrap();
    m
}

fn bar_mesh() -> SolidMesh {
    hex_box([0.0; 3], [L, B, B], [N_EL, 1, 1]).unwrap()
}

fn inf(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// ------------------------------------------------------------------------------------------------------------- models

/// KB: the bar on an end spring, rollers on ymin and zmin (ADR 0384).
fn kb() -> SolidModel {
    let mut m = model_of(bar_mesh(), 0.0);
    m.add_support(ElasticSupport::nodal_spring("xmin", 0, K_SPRING / 4.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

/// KF: the springs only (ADR 0384).
fn kf() -> SolidModel {
    let mut m = model_of(bar_mesh(), 0.0);
    m.add_support(ElasticSupport::nodal_spring("xmin", 0, K_SPRING / 4.0))
        .unwrap();
    m
}

const R_REF: Vec3 = [L, 0.05, 0.05];

fn rollers(m: &mut SolidModel) {
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
}

/// RB: the fixed-free bar with a rigid set on xmax (ADR 0384).
fn rb() -> SolidModel {
    let mut m = model_of(bar_mesh(), 0.0);
    rollers(&mut m);
    m.add_constraint(Constraint::rigid_set("xmax", R_REF)).unwrap();
    m
}

/// RC: a collinear rigid set (ADR 0384).
fn rc() -> SolidModel {
    let mut mesh = bar_mesh();
    let edge: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&a| {
            let x = mesh.nodes()[a];
            x[0] == L && x[2] == 0.0
        })
        .collect();
    mesh.add_node_set("edge", edge).unwrap();
    let mut m = model_of(mesh, 0.0);
    rollers(&mut m);
    m.add_constraint(Constraint::rigid_set("edge", [L, 0.05, 0.0])).unwrap();
    m
}

/// The same mesh with node `i` moved to `f(x_i)`: blocks and sets kept (as `remap` of `si_modal_mpc.rs`).
fn remap<F: FnMut(Vec3) -> Vec3>(mesh: &SolidMesh, mut f: F) -> SolidMesh {
    let nodes: Vec<Vec3> = mesh.nodes().iter().map(|&x| f(x)).collect();
    let mut out = SolidMesh::new(nodes).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    let names: Vec<String> = mesh.node_set_names().map(String::from).collect();
    for n in names {
        out.add_node_set(&n, mesh.node_set(&n).unwrap().to_vec()).unwrap();
    }
    let names: Vec<String> = mesh.element_set_names().map(String::from).collect();
    for n in names {
        out.add_element_set(&n, mesh.element_set(&n).unwrap().to_vec()).unwrap();
    }
    let names: Vec<String> = mesh.surface_set_names().map(String::from).collect();
    for n in names {
        out.add_surface_set(&n, mesh.surface_set(&n).unwrap().to_vec()).unwrap();
    }
    out
}

fn block_mesh() -> SolidMesh {
    hex_box([0.0; 3], [1.0, 0.5, 0.4], [4, 2, 2]).unwrap()
}

/// SR: the block rotated by 30 degrees about z with Linear rollers (ADR 0384).
fn sr() -> SolidModel {
    let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let mesh = remap(&block_mesh(), |x| [c * x[0] - s * x[1], s * x[0] + c * x[1], x[2]]);
    let mut m = model_of(mesh, 0.3);
    let xmin = m.mesh().node_set("xmin").unwrap().to_vec();
    let ymin = m.mesh().node_set("ymin").unwrap().to_vec();
    for a in xmin {
        m.add_constraint(Constraint::linear(
            vec![LinearTerm::new(a, 0, c), LinearTerm::new(a, 1, s)],
            0.0,
        ))
        .unwrap();
    }
    for a in ymin {
        m.add_constraint(Constraint::linear(
            vec![LinearTerm::new(a, 0, -s), LinearTerm::new(a, 1, c)],
            0.0,
        ))
        .unwrap();
    }
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

/// SA: the unrotated block with Displacement rollers (ADR 0384).
fn sa() -> SolidModel {
    let mut m = model_of(block_mesh(), 0.3);
    rollers(&mut m);
    m
}

fn cantilever_mesh() -> SolidMesh {
    hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 2, 2]).unwrap()
}

/// CP: the clamped cantilever (ADR 0384).
fn cp() -> SolidModel {
    let mut m = model_of(cantilever_mesh(), 0.3);
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

/// CL1, CL2: the clamp as single-term Linear constraints with coefficient `a` (ADR 0384).
fn cl(a: f64) -> SolidModel {
    let mut m = model_of(cantilever_mesh(), 0.3);
    let xmin = m.mesh().node_set("xmin").unwrap().to_vec();
    for node in xmin {
        for c in 0..3 {
            m.add_constraint(Constraint::linear(vec![LinearTerm::new(node, c, a)], 0.0))
                .unwrap();
        }
    }
    m
}

/// CN: CP with a Linear tie of value 1e-3 (ADR 0384).
fn cn() -> SolidModel {
    let mut m = cp();
    let xmax = m.mesh().node_set("xmax").unwrap().to_vec();
    m.add_constraint(Constraint::linear(
        vec![LinearTerm::new(xmax[0], 2, 1.0), LinearTerm::new(xmax[1], 2, -1.0)],
        1e-3,
    ))
    .unwrap();
    m
}

/// KN: KB with a prescribed value 1e-3 on xmax (ADR 0384).
fn kn() -> SolidModel {
    let mut m = kb();
    m.add_constraint(Constraint::displacement("xmax", 0, 1e-3)).unwrap();
    m
}

fn node_at(mesh: &SolidMesh, p: Vec3) -> usize {
    let found: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&a| {
            let x = mesh.nodes()[a];
            (0..3).all(|i| (x[i] - p[i]).abs() <= 1e-12)
        })
        .collect();
    assert_eq!(found.len(), 1, "node at {p:?}");
    found[0]
}

/// FB1: the free block with one x spring at the corner at the origin.
fn fb1() -> SolidModel {
    let mut mesh = block_mesh();
    let a = node_at(&mesh, [0.0, 0.0, 0.0]);
    mesh.add_node_set("corner", vec![a]).unwrap();
    let mut m = model_of(mesh, 0.3);
    m.add_support(ElasticSupport::nodal_spring("corner", 0, K_BLOCK))
        .unwrap();
    m
}

/// FB2: FB1 with a Linear tie u_x(1, 0, 0) - u_x(1, 0.5, 0) = 0 and a rigid set on xmin.
fn fb2() -> SolidModel {
    let mut m = fb1();
    let a = node_at(m.mesh(), [1.0, 0.0, 0.0]);
    let b = node_at(m.mesh(), [1.0, 0.5, 0.0]);
    m.add_constraint(Constraint::linear(
        vec![LinearTerm::new(a, 0, 1.0), LinearTerm::new(b, 0, -1.0)],
        0.0,
    ))
    .unwrap();
    m.add_constraint(Constraint::rigid_set("xmin", [0.0, 0.13, 0.17])).unwrap();
    m
}

/// FR: the free block with a rigid set on xmax only.
fn fr() -> SolidModel {
    let mut m = model_of(block_mesh(), 0.3);
    m.add_constraint(Constraint::rigid_set("xmax", [1.0, 0.13, 0.17])).unwrap();
    m
}

/// FS: the free block with two x springs on one line (nodes at (0, 0, 0) and (1, 0, 0)).
fn fs() -> SolidModel {
    let mut mesh = block_mesh();
    let a = node_at(&mesh, [0.0, 0.0, 0.0]);
    let b = node_at(&mesh, [1.0, 0.0, 0.0]);
    mesh.add_node_set("corner", vec![a]).unwrap();
    mesh.add_node_set("corner2", vec![b]).unwrap();
    let mut m = model_of(mesh, 0.3);
    m.add_support(ElasticSupport::nodal_spring("corner", 0, K_BLOCK))
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("corner2", 0, K_BLOCK))
        .unwrap();
    m
}

struct Case {
    name: &'static str,
    model: SolidModel,
    extra: Vec<Prescribed>,
    modes: usize,
}

fn case(name: &'static str, model: SolidModel, modes: usize) -> Case {
    Case {
        name,
        model,
        extra: Vec::new(),
        modes,
    }
}

/// The models of ADR 0384 that give modes.
fn adr0384_solved() -> Vec<Case> {
    vec![
        case("KB", kb(), 4),
        case("KF", kf(), 8),
        case("RB", rb(), 4),
        case("SR", sr(), 6),
        case("SA", sa(), 6),
        case("CP", cp(), 4),
        case("CL1", cl(1.0), 4),
        case("CL2", cl(2.5), 4),
    ]
}

/// The models of ADR 0384 that are refused.
fn adr0384_refused() -> Vec<Case> {
    vec![
        case("RC", rc(), 4),
        case("CN", cn(), 4),
        case("KN", kn(), 4),
        Case {
            name: "KE",
            model: kb(),
            extra: vec![Prescribed::new(0, 0, 1e-3)],
            modes: 4,
        },
    ]
}

/// The new models of ADR 0399.
fn new_models() -> Vec<Case> {
    vec![
        case("FB1", fb1(), 8),
        case("FB2", fb2(), 8),
        case("FR", fr(), 8),
        case("FS", fs(), 8),
    ]
}

// ------------------------------------------------------------------------------------------------------------ helpers

fn count_of(c: &Case) -> FreeRigidMotions {
    let f = free_rigid_motions(&c.model, &c.extra).unwrap();
    println!(
        "count {}: {} free, {} rows, singular values {:?}, centre {:?}, length {}",
        c.name, f.count, f.rows, f.singular_values, f.centre, f.length
    );
    f
}

fn run(c: &Case, shift: Option<Shift>) -> Result<ModalResult, FemError> {
    let mut o = ModalOptions::new(c.modes);
    if let Some(s) = shift {
        o = o.shift(s);
    }
    modal_analysis_with(&c.model, &c.extra, &o)
}

fn rho_of(k: &CsrMatrix, m: &CsrMatrix) -> f64 {
    k.diag().iter().map(|v| v.abs()).sum::<f64>() / m.diag().iter().sum::<f64>()
}

/// `s_p = 1e-3 rho` (ADR 0399): of the reduced matrices on the transformation, of the unreduced ones on the old path.
fn caller_shift(c: &Case) -> f64 {
    if mpc::uses_transformation(&c.model) {
        let s = mpc::modal_analysis_mpc(&c.model, &c.extra, &ModalOptions::new(c.modes)).unwrap();
        1e-3 * rho_of(&s.reduced_stiffness, &s.reduced_mass)
    } else {
        1e-3 * rho_of(&stiffness(&c.model).unwrap(), &consistent(&c.model).unwrap())
    }
}

/// S1: the smallest held singular value and the largest free one, over `s_max`.
fn s1(f: &FreeRigidMotions) -> (f64, f64) {
    let rank = 6 - f.count;
    let smax = f.singular_values[0];
    if smax == 0.0 {
        return (f64::INFINITY, 0.0);
    }
    let held = if rank > 0 { f.singular_values[rank - 1] / smax } else { f64::INFINITY };
    let free = if rank < 6 { f.singular_values[rank] / smax } else { 0.0 };
    (held, free)
}

/// The least-squares fit of `x` by the fields: `|x - F c|_inf / |x|_inf`.
fn fit_residual(fields: &[Vec<f64>], x: &[f64]) -> f64 {
    let n = fields.len();
    if n == 0 {
        return 1.0;
    }
    let mut a = vec![0.0; n * n];
    let mut b = vec![0.0; n];
    for i in 0..n {
        b[i] = dot(&fields[i], x);
        for j in 0..n {
            a[i * n + j] = dot(&fields[i], &fields[j]);
        }
    }
    // Gauss elimination with partial pivoting.
    for col in 0..n {
        let p = (col..n)
            .max_by(|&r, &s| a[r * n + col].abs().total_cmp(&a[s * n + col].abs()))
            .unwrap();
        for j in 0..n {
            a.swap(col * n + j, p * n + j);
        }
        b.swap(col, p);
        for r in col + 1..n {
            let f = a[r * n + col] / a[col * n + col];
            for j in col..n {
                a[r * n + j] -= f * a[col * n + j];
            }
            b[r] -= f * b[col];
        }
    }
    let mut c = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = b[i];
        for j in i + 1..n {
            s -= a[i * n + j] * c[j];
        }
        c[i] = s / a[i * n + i];
    }
    let mut r = x.to_vec();
    for (f, &ci) in fields.iter().zip(&c) {
        for (ri, v) in r.iter_mut().zip(f) {
            *ri -= ci * v;
        }
    }
    inf(&r) / inf(x)
}

struct ZeroModes {
    n0: usize,
    lowest: bool,
    worst: f64,
    next: f64,
    r1: f64,
}

/// The zero modes of an automatic-shift result: their number by the measure, whether they are the lowest, the largest
/// `|lambda| / sigma` among them, the next `lambda / sigma`, and R1 against the free fields of the count.
fn zero_modes(c: &Case, s: &MpcModes, f: &FreeRigidMotions) -> ZeroModes {
    let r = &s.result;
    let sigma = r.shift;
    let zero: Vec<bool> = r
        .eigenvalues
        .iter()
        .map(|l| sigma > 0.0 && l.abs() <= ZERO_MEASURE * sigma)
        .collect();
    let n0 = zero.iter().filter(|&&z| z).count();
    let lowest = zero.iter().take(n0).all(|&z| z);
    let worst = r.eigenvalues[..n0]
        .iter()
        .fold(0.0f64, |w, l| w.max(l.abs() / sigma));
    let next = r.eigenvalues.get(n0).map_or(f64::NAN, |l| l / sigma);
    let fields = f.fields(&c.model);
    let r1 = r.modes[..n0]
        .iter()
        .fold(0.0f64, |w, x| w.max(fit_residual(&fields, x)));
    println!(
        "zero modes {}: sigma {sigma:e}, rigid_free {}, eigenvalues {:?}; n0 {n0} (lowest {lowest}), count {}; max \
         |lambda|/sigma {worst:e}; next lambda/sigma {next:e}; R1 {r1:e}",
        c.name, r.rigid_free, r.eigenvalues, f.count
    );
    ZeroModes {
        n0,
        lowest,
        worst,
        next,
        r1,
    }
}

/// Z1 / Z2, R1 and S1 on one model with its hand count.
fn check_zero_modes(c: &Case, hand: usize) -> ZeroModes {
    let f = count_of(c);
    let s = mpc::modal_analysis_mpc(&c.model, &c.extra, &ModalOptions::new(c.modes)).unwrap();
    let z = zero_modes(c, &s, &f);
    let (held, free) = s1(&f);
    println!("S1 {}: held {held:e}, free {free:e}", c.name);
    assert!(s.result.rigid_free && s.result.shift > 0.0, "{} shift", c.name);
    assert_eq!(f.count, hand, "{} count against the hand count", c.name);
    assert_eq!(z.n0, f.count, "{} zero modes against the count", c.name);
    assert!(z.lowest, "{} zero modes are the lowest", c.name);
    assert!(z.worst <= ZERO_MEASURE, "{} |lambda|/sigma {:e}", c.name, z.worst);
    assert!(z.r1 <= 1e-6, "R1 {} {:e}", c.name, z.r1);
    assert!(held > 1e-6 && free <= 1e-12, "S1 {} {held:e} {free:e}", c.name);
    z
}

// ------------------------------------------------------------------------------------------------------------- checks

/// Z1 (KF and FB1), R1, S1 and N3.
#[test]
fn z1_r1_s1_springs_only_and_n3() {
    let kf = case("KF", kf(), 8);
    check_zero_modes(&kf, 3);
    let fb1 = case("FB1", fb1(), 8);
    let z = check_zero_modes(&fb1, 5);
    // N3: the spring mode (index 5) above the measure by more than 1e3.
    let n3 = z.next / ZERO_MEASURE;
    println!("N3 FB1 lambda_5 / (1e-9 sigma) = {n3:e}");
    assert!(n3 > 1e3, "N3 {n3:e}");
}

/// Z2 (FB2 and FR), R1, S1 and N1 (FS).
#[test]
fn z2_r1_s1_tie_rigid_sets_and_n1() {
    check_zero_modes(&case("FB2", fb2(), 8), 4);
    check_zero_modes(&case("FR", fr(), 8), 6);
    // N1: two springs on one line hold one direction.
    check_zero_modes(&case("FS", fs(), 8), 5);
}

/// C1, E1, E2, E3 on every model of ADR 0384 and the new models, and N2.
#[test]
fn c1_e1_e2_e3_every_model_and_n2() {
    // C1: the counts of ADR 0384's models.
    let mut c1_ok = true;
    let mut s1_ok = true;
    for c in adr0384_solved().iter().chain(adr0384_refused().iter()) {
        let want = if c.name == "KF" { 3 } else { 0 };
        let f = count_of(c);
        let (held, free) = s1(&f);
        if !(held > 1e-6 && free <= 1e-12) {
            println!("S1 {}: held {held:e}, free {free:e}", c.name);
            s1_ok = false;
        }
        if f.count != want {
            println!("C1 {}: count {} against {want}", c.name, f.count);
            c1_ok = false;
        }
    }
    // N2: the bar without its springs.
    let n2 = count_of(&case("bare bar", model_of(bar_mesh(), 0.0), 8)).count;
    println!("N2 bare bar count {n2}");
    // C1 (rigid_free = count > 0), E1, E2 on every model that gives modes.
    let mut e1_ok = true;
    let mut e2_ok = true;
    for c in adr0384_solved().iter().chain(new_models().iter()) {
        let count = count_of(c).count;
        let base = run(c, None).unwrap();
        let sp = caller_shift(c);
        let auto = run(c, Some(Shift::Auto)).unwrap();
        let value = run(c, Some(Shift::Value(sp))).unwrap();
        let zero = run(c, Some(Shift::Value(0.0)));
        println!(
            "E1 {}: default rigid_free {} shift {:e}; Auto {} {:e}; Value({sp:e}) {} {:e}; Value(0) {:?}",
            c.name,
            base.rigid_free,
            base.shift,
            auto.rigid_free,
            auto.shift,
            value.rigid_free,
            value.shift,
            zero.as_ref().map(|r| (r.rigid_free, r.shift))
        );
        if base.rigid_free != (count > 0) {
            println!("C1 {}: rigid_free {} with count {count}", c.name, base.rigid_free);
            c1_ok = false;
        }
        let zero_ok = if base.rigid_free {
            matches!(zero, Err(FemError::Solver { .. }))
        } else {
            matches!(&zero, Ok(r) if !r.rigid_free && r.shift == 0.0)
        };
        if auto.rigid_free != base.rigid_free || value.rigid_free != base.rigid_free || !zero_ok {
            println!("E1 {}: disagreement", c.name);
            e1_ok = false;
        }
        if ["KB", "RB", "SR", "CL1"].contains(&c.name) && !(value.shift == sp && sp != 0.0 && !value.rigid_free) {
            println!("E2 {}: shift {:e} rigid_free {}", c.name, value.shift, value.rigid_free);
            e2_ok = false;
        }
    }
    // E3: the refused models give the default's error at every caller shift.
    let mut e3_ok = true;
    for c in adr0384_refused() {
        let base = format!("{:?}", run(&c, None).expect_err("refused"));
        for s in [Shift::Auto, Shift::Value(1e3), Shift::Value(0.0)] {
            let e = format!("{:?}", run(&c, Some(s)).expect_err("refused"));
            if e != base {
                println!("E3 {} at {s:?}: {e} against {base}", c.name);
                e3_ok = false;
            }
        }
        println!("E3 {}: {base}", c.name);
    }
    assert!(c1_ok, "C1");
    assert!(s1_ok, "S1");
    assert!(e1_ok, "E1");
    assert!(e2_ok, "E2");
    assert!(e3_ok, "E3");
    assert_eq!(n2, 6, "N2");
}
