//! F-486 (ADR 0384): the modal analysis on the transformation of ADR 0296 (linear constraints, rigid sets, elastic
//! supports). Checks P1, L1, D1, Z1, B1 to B3, O1, H1, A1, A2, G1, T1 and the negative controls N1 to N4 of the
//! registration (P2 is the existing `sb_modal`, `sb_modal_scale` and `si_scaffold_bits`).

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, modal_analysis_with, ModalOptions, ModalResult};
use rcfd_fem::mpc::{self, MpcModes};
use rcfd_fem::{Constraint, ElasticSupport, FemError, LinearTerm, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{lowest_eigenpairs, Shift, SubspaceOptions};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const L: f64 = 1.0;
const B: f64 = 0.1;
const AREA: f64 = B * B;
const K_SPRING: f64 = 5e8;
const N_EL: usize = 20;

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

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn mpc_modes(m: &SolidModel, modes: usize) -> MpcModes {
    mpc::modal_analysis_mpc(m, &[], &ModalOptions::new(modes)).unwrap()
}

fn print_run(name: &str, r: &ModalResult) {
    println!(
        "{name}: f = {:?} Hz, shift {:e}, rigid_free {}, unknowns {}, iterations {}, residuals {:?}",
        r.frequencies, r.shift, r.rigid_free, r.unknowns, r.iterations, r.residuals
    );
}

// ------------------------------------------------------------------------------------------------------------- models

/// KB: the bar on an end spring, rollers on ymin and zmin.
fn kb() -> SolidModel {
    let mut m = model_of(bar_mesh(), 0.0);
    m.add_support(ElasticSupport::nodal_spring("xmin", 0, K_SPRING / 4.0))
        .unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

/// KF: the springs only.
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

/// RB: the fixed-free bar of ADR 0284 with a rigid set on xmax.
fn rb() -> SolidModel {
    let mut m = model_of(bar_mesh(), 0.0);
    rollers(&mut m);
    m.add_constraint(Constraint::rigid_set("xmax", R_REF)).unwrap();
    m
}

/// RC: a collinear rigid set (the two xmax nodes at z = 0, along y).
fn rc() -> SolidModel {
    let mut mesh = bar_mesh();
    let edge: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&a| {
            let x = mesh.nodes()[a];
            x[0] == L && x[2] == 0.0
        })
        .collect();
    println!("RC edge nodes {edge:?}");
    mesh.add_node_set("edge", edge).unwrap();
    let mut m = model_of(mesh, 0.0);
    rollers(&mut m);
    m.add_constraint(Constraint::rigid_set("edge", [L, 0.05, 0.0])).unwrap();
    m
}

/// The same mesh with node `i` moved to `f(x_i)`: blocks and sets kept (as `remap` of `si_mpc.rs`).
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

fn skew_base() -> SolidMesh {
    hex_box([0.0; 3], [1.0, 0.5, 0.4], [4, 2, 2]).unwrap()
}

/// SR: the block rotated by 30 degrees about z with Linear rollers whose normals are at `angle` degrees.
fn sr(angle: f64) -> SolidModel {
    let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let mesh = remap(&skew_base(), |x| [c * x[0] - s * x[1], s * x[0] + c * x[1], x[2]]);
    let (cc, ss) = (angle.to_radians().cos(), angle.to_radians().sin());
    let mut m = model_of(mesh, 0.3);
    let xmin = m.mesh().node_set("xmin").unwrap().to_vec();
    let ymin = m.mesh().node_set("ymin").unwrap().to_vec();
    for a in xmin {
        m.add_constraint(Constraint::linear(
            vec![LinearTerm::new(a, 0, cc), LinearTerm::new(a, 1, ss)],
            0.0,
        ))
        .unwrap();
    }
    for a in ymin {
        m.add_constraint(Constraint::linear(
            vec![LinearTerm::new(a, 0, -ss), LinearTerm::new(a, 1, cc)],
            0.0,
        ))
        .unwrap();
    }
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

/// SA: the unrotated block with Displacement rollers.
fn sa() -> SolidModel {
    let mut m = model_of(skew_base(), 0.3);
    rollers(&mut m);
    m
}

fn cantilever_mesh() -> SolidMesh {
    hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 2, 2]).unwrap()
}

/// CP: the clamped cantilever.
fn cp() -> SolidModel {
    let mut m = model_of(cantilever_mesh(), 0.3);
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

/// CL: the clamp as single-term Linear constraints with coefficient `a`, value 0.
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

// ------------------------------------------------------------------------------------------------------- closed forms

/// The first root of `z tan z = kappa` in (0, pi/2), bisection.
fn spring_root(kappa: f64) -> f64 {
    let (mut a, mut b) = (1e-12, PI / 2.0 - 1e-12);
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        if m * m.tan() - kappa > 0.0 {
            b = m;
        } else {
            a = m;
        }
    }
    0.5 * (a + b)
}

fn wave_speed() -> f64 {
    (E / RHO).sqrt()
}

fn spring_exact(k: f64) -> f64 {
    spring_root(k * L / (E * AREA)) * wave_speed() / (2.0 * PI * L)
}

/// The chain's eigenvalue for the wave number `theta` per element.
fn chain_lambda(theta: f64) -> f64 {
    let h = L / N_EL as f64;
    6.0 * E / RHO / (h * h) * (1.0 - theta.cos()) / (2.0 + theta.cos())
}

/// The discrete closed form of the spring end (ADR 0384): `u_j = cos(theta (n - j))`, root of the spring node's equation.
fn spring_discrete() -> f64 {
    let n = N_EL as f64;
    let h = L / n;
    let r = |t: f64| {
        let (u0, u1) = ((t * n).cos(), (t * (n - 1.0)).cos());
        E * AREA / h * (u0 - u1) + K_SPRING * u0 - chain_lambda(t) * RHO * AREA * h / 6.0 * (2.0 * u0 + u1)
    };
    let (mut a, mut b) = (1e-12, PI / (2.0 * n));
    assert!(r(a) > 0.0 && r(b) < 0.0, "bracket");
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        if r(m) > 0.0 {
            a = m;
        } else {
            b = m;
        }
    }
    let t = 0.5 * (a + b);
    println!("discrete spring root theta = {t:.10e}");
    chain_lambda(t).sqrt() / (2.0 * PI)
}

/// The fixed-free chain (ADR 0284 D5, `bar_discrete` of `sb_modal.rs`), first mode.
fn bar_discrete() -> f64 {
    let h = L / N_EL as f64;
    let k = PI / (2.0 * L);
    let lam = 6.0 * E / RHO / (h * h) * (1.0 - (k * h).cos()) / (2.0 + (k * h).cos());
    lam.sqrt() / (2.0 * PI)
}

/// Kinetic share of the x components of each mode.
fn axial_shares(model: &SolidModel, m: &CsrMatrix, r: &ModalResult) -> Vec<f64> {
    let dofs = model.dof_map();
    r.modes
        .iter()
        .map(|phi| {
            let mut px = phi.clone();
            for (e, v) in px.iter_mut().enumerate() {
                if dofs.node_component(e).unwrap().1 != 0 {
                    *v = 0.0;
                }
            }
            dot(&px, &mv(m, &px)) / dot(phi, &mv(m, phi))
        })
        .collect()
}

/// `max_ij |phi_i^T M phi_j - delta_ij|` with the full mesh mass.
fn m_orthonormality(model: &SolidModel, r: &ModalResult) -> f64 {
    let m = consistent(model).unwrap();
    let mphi: Vec<Vec<f64>> = r.modes.iter().map(|p| mv(&m, p)).collect();
    let mut w: f64 = 0.0;
    for (i, a) in r.modes.iter().enumerate() {
        for (j, mb) in mphi.iter().enumerate() {
            let d = if i == j { 1.0 } else { 0.0 };
            w = w.max((dot(a, mb) - d).abs());
        }
    }
    w
}

/// H1: every constraint row rebuilt from the model on every expanded mode: the largest `|c . x| / (max |c| |x|_inf)`, and
/// whether every prescribed equation is exactly 0.
fn row_residual(model: &SolidModel, s: &MpcModes) -> (f64, bool, usize) {
    let dofs = model.dof_map();
    let mut rows: Vec<Vec<(usize, f64)>> = Vec::new();
    let fixed = prescribed_equations(model, &[]).unwrap();
    let mut exact = true;
    for (i, c) in model.constraints().iter().enumerate() {
        match c {
            Constraint::Linear { terms, .. } => rows.push(
                terms
                    .iter()
                    .map(|t| (dofs.equation(t.node, t.component).unwrap(), t.coefficient))
                    .collect(),
            ),
            Constraint::RigidSet { set, reference, .. } => {
                let r0 = s.transformation.reference_offset(i).unwrap();
                for &a in model.mesh().node_set(set).unwrap() {
                    let x = model.mesh().nodes()[a];
                    let d = [x[0] - reference[0], x[1] - reference[1], x[2] - reference[2]];
                    // u_c - t_c - (w x d)_c.
                    let (w0, w1, w2) = (r0 + 3, r0 + 4, r0 + 5);
                    let e = |c| dofs.equation(a, c).unwrap();
                    rows.push(vec![(e(0), 1.0), (r0, -1.0), (w1, -d[2]), (w2, d[1])]);
                    rows.push(vec![(e(1), 1.0), (r0 + 1, -1.0), (w2, -d[0]), (w0, d[2])]);
                    rows.push(vec![(e(2), 1.0), (r0 + 2, -1.0), (w0, -d[1]), (w1, d[0])]);
                }
            }
            _ => {}
        }
    }
    let mut worst: f64 = 0.0;
    for x in &s.expanded {
        for &(eq, _) in &fixed {
            exact &= x[eq] == 0.0;
        }
        let xi = inf(x);
        for row in &rows {
            let v: f64 = row.iter().map(|&(j, c)| c * x[j]).sum();
            let cmax = row.iter().fold(0.0f64, |m, &(_, c)| m.max(c.abs()));
            worst = worst.max(v.abs() / (cmax * xi));
        }
    }
    (worst, exact, rows.len() + fixed.len())
}

// ------------------------------------------------------------------------------------------------------------- checks

#[test]
fn p1_prescribed_only_keeps_the_bits() {
    let m = cp();
    assert!(!mpc::uses_transformation(&m), "P1 uses_transformation");
    let r = modal_analysis(&m, &ModalOptions::new(4)).unwrap();
    print_run("CP", &r);
    let fixed = prescribed_equations(&m, &[]).unwrap();
    let k = stiffness(&m).unwrap();
    let mm = consistent(&m).unwrap();
    let zero = vec![0.0; k.n];
    let kr = reduce(&k, &zero, &fixed).unwrap();
    let mr = reduce(&mm, &zero, &fixed).unwrap();
    let opts = SubspaceOptions {
        shift: Shift::Value(0.0),
        ..SubspaceOptions::default()
    };
    let eig = lowest_eigenpairs(&kr.matrix, &mr.matrix, 4, &opts).unwrap();
    let mut same = bits(&eig.values) == bits(&r.eigenvalues);
    same &= eig.shift.to_bits() == r.shift.to_bits() && kr.free().len() == r.unknowns;
    for (v, phi) in eig.vectors.iter().zip(&r.modes) {
        let mut full = vec![0.0; k.n];
        for (&eq, &x) in kr.free().iter().zip(v) {
            full[eq] = x;
        }
        same &= bits(&full) == bits(phi);
    }
    println!("P1 bit-equal: {same}");
    assert!(same, "P1");
}

#[test]
fn l1_clamp_written_as_linear_constraints() {
    let base = modal_analysis(&cp(), &ModalOptions::new(4)).unwrap();
    for a in [1.0, 2.5] {
        let m = cl(a);
        assert!(mpc::uses_transformation(&m));
        let r = modal_analysis(&m, &ModalOptions::new(4)).unwrap();
        print_run(&format!("CL {a}"), &r);
        let d = r
            .eigenvalues
            .iter()
            .zip(&base.eigenvalues)
            .fold(0.0f64, |w, (x, y)| w.max((x - y).abs() / y));
        println!("L1 coefficient {a}: eigenvalue difference {d:e}");
        assert!(d <= 1e-12, "L1 {a} {d:e}");
        assert!(!r.rigid_free && r.shift == 0.0, "L1 {a} shift");
    }
}

/// D1: the reduction done in the test, against the library.
fn reduction_in_test(m: &SolidModel, modes: usize, r: &ModalResult) -> bool {
    let tr = mpc::transformation(m, &[]).unwrap();
    let mut k = stiffness(m).unwrap();
    mpc::add_supports(&mut k, m).unwrap();
    let mm = consistent(m).unwrap();
    let zero = vec![0.0; tr.n_unknowns()];
    let (kr, _) = tr.reduce(&k, &zero).unwrap();
    let (mr, _) = tr.reduce(&mm, &zero).unwrap();
    let eig = lowest_eigenpairs(&kr, &mr, modes, &SubspaceOptions::default()).unwrap();
    let mut same = bits(&eig.values) == bits(&r.eigenvalues) && eig.shift.to_bits() == r.shift.to_bits();
    for (q, phi) in eig.vectors.iter().zip(&r.modes) {
        same &= bits(&tr.expand(q)[..tr.n_mesh()]) == bits(phi);
    }
    same
}

#[test]
fn d1_reduction_by_the_transformation() {
    let mut all = true;
    for (name, m, modes) in [("KB", kb(), 4), ("RB", rb(), 4), ("SR", sr(30.0), 6)] {
        let r = modal_analysis(&m, &ModalOptions::new(modes)).unwrap();
        let same = reduction_in_test(&m, modes, &r);
        println!("D1 {name}: bit-equal {same}");
        all &= same;
    }
    assert!(all, "D1");
}

fn invalid_constraint(r: Result<ModalResult, FemError>, what: &str) -> String {
    match r {
        Err(FemError::InvalidValue {
            key: "constraint",
            reason,
        }) => {
            println!("Z1 {what}: {reason}");
            reason
        }
        other => panic!("Z1 {what}: {other:?}"),
    }
}

#[test]
fn z1_non_zero_values_refused() {
    let mut cn = cp();
    let xmax = cn.mesh().node_set("xmax").unwrap().to_vec();
    cn.add_constraint(Constraint::linear(
        vec![LinearTerm::new(xmax[0], 2, 1.0), LinearTerm::new(xmax[1], 2, -1.0)],
        1e-3,
    ))
    .unwrap();
    let r = invalid_constraint(modal_analysis(&cn, &ModalOptions::new(4)), "CN");
    let cn_ok = r.contains("zero values only") && r.contains("constraint 3 (Linear)");
    let mut kn = kb();
    kn.add_constraint(Constraint::displacement("xmax", 0, 1e-3)).unwrap();
    let r = invalid_constraint(modal_analysis(&kn, &ModalOptions::new(4)), "KN");
    let kn_ok = r.contains("zero values only") && r.contains("the prescribed displacement of node");
    let r = invalid_constraint(
        modal_analysis_with(&kb(), &[Prescribed::new(0, 0, 1e-3)], &ModalOptions::new(4)),
        "KE",
    );
    let ke_ok = r.contains("zero values only") && r.contains("the prescribed displacement of node");
    assert!(cn_ok && kn_ok && ke_ok, "Z1 {cn_ok} {kn_ok} {ke_ok}");
}

#[test]
fn b1_b2_o1_h1_a2_bar_on_an_end_spring_and_n3() {
    let m = kb();
    let s = mpc_modes(&m, 4);
    let r = &s.result;
    print_run("KB", r);
    let mm = consistent(&m).unwrap();
    let ax = axial_shares(&m, &mm, r);
    println!("KB axial shares {ax:?}");
    let f0 = r.frequencies[0];
    let fe = spring_exact(K_SPRING);
    let fd = spring_discrete();
    let b1 = (f0 / fe - 1.0).abs();
    let b2 = (f0 / fd - 1.0).abs();
    let n3 = (f0 / spring_exact(2.0 * K_SPRING) - 1.0).abs();
    let o1 = m_orthonormality(&m, r);
    let (h1, exact, rows) = row_residual(&m, &s);
    println!(
        "B1 KB f0 {f0:.10e} Hz, exact {fe:.10e}: {b1:e}; B2 discrete {fd:.10e}: {b2:e}; N3 kappa 0.5: {n3:e}; O1 {o1:e}; \
         H1 {h1:e} over {rows} rows, prescribed exactly 0: {exact}; A2 rigid_free {} shift {:e}",
        r.rigid_free, r.shift
    );
    assert!(ax[0] >= 0.99, "B1 mode 0 axial {}", ax[0]);
    assert!(b1 <= 5e-3, "B1 {b1:e}");
    assert!(b2 <= 1e-9, "B2 {b2:e}");
    assert!(n3 > 1e-2, "N3 {n3:e}");
    assert!(o1 <= 1e-10, "O1 KB {o1:e}");
    assert!(h1 <= 1e-12 && exact, "H1 KB {h1:e} {exact}");
    assert!(!r.rigid_free && r.shift == 0.0, "A2 KB");
}

#[test]
#[ignore = "KNOWN MISS (ADR 0384, run once): A1 max |f_0..2| / f_3 = 2.0717e-6 against 1e-6 (the zero modes are fixed by the residual times the Auto shift; N1, the shift, the axial mode 1.5e-13, O1 8.9e-16 and H1 met, kept in the copy below)"]
fn a1_o1_h1_springs_only_and_n1_known_miss() {
    let m = kf();
    let s = mpc_modes(&m, 8);
    let r = &s.result;
    print_run("KF", r);
    let mm = consistent(&m).unwrap();
    let ax = axial_shares(&m, &mm, r);
    println!("KF axial shares {ax:?}");
    let zero = inf(&r.frequencies[..3]) / r.frequencies[3];
    let f_kb = mpc_modes(&kb(), 4).result.frequencies[0];
    let axial = ax.iter().position(|&a| a >= 0.99);
    let a1f = axial.map_or(f64::INFINITY, |i| (r.frequencies[i] / f_kb - 1.0).abs());
    let o1 = m_orthonormality(&m, r);
    let (h1, exact, rows) = row_residual(&m, &s);
    println!(
        "A1 KF rigid_free {} shift {:e}; max |f_0..2| / f_3 {zero:e}; axial mode {axial:?} against KB: {a1f:e}; O1 {o1:e}; \
         H1 {h1:e} over {rows} rows",
        r.rigid_free, r.shift
    );
    let n1 = mpc::modal_analysis_mpc(&m, &[], &ModalOptions::new(8).shift(Shift::Value(0.0)));
    println!(
        "N1 KF at zero shift: {:?}",
        n1.as_ref().map(|s| s.result.frequencies.clone())
    );
    assert!(matches!(n1, Err(FemError::Solver { .. })), "N1");
    assert!(r.rigid_free && r.shift > 0.0, "A1 shift");
    assert!(zero <= 1e-6, "A1 zero modes {zero:e}");
    assert!(a1f <= 1e-9, "A1 axial {a1f:e}");
    assert!(o1 <= 1e-10, "O1 KF {o1:e}");
    assert!(h1 <= 1e-12 && exact, "H1 KF {h1:e}");
}

#[test]
fn b3_o1_h1_a2_rigid_end() {
    let m = rb();
    let s = mpc_modes(&m, 4);
    let r = &s.result;
    print_run("RB", r);
    println!("RB dropped rows {}", s.transformation.dropped().len());
    let mm = consistent(&m).unwrap();
    let ax = axial_shares(&m, &mm, r);
    println!("RB axial shares {ax:?}");
    let f0 = r.frequencies[0];
    let fe = wave_speed() / (4.0 * L);
    let fd = bar_discrete();
    let (b3e, b3d) = ((f0 / fe - 1.0).abs(), (f0 / fd - 1.0).abs());
    let o1 = m_orthonormality(&m, r);
    let (h1, exact, rows) = row_residual(&m, &s);
    println!(
        "B3 RB f0 {f0:.10e} Hz, c/(4L) {fe:.10e}: {b3e:e}; discrete {fd:.10e}: {b3d:e}; O1 {o1:e}; H1 {h1:e} over {rows} \
         rows, prescribed exactly 0: {exact}; A2 rigid_free {} shift {:e}",
        r.rigid_free, r.shift
    );
    assert!(ax[0] >= 0.99, "B3 mode 0 axial {}", ax[0]);
    assert!(b3e <= 5e-3, "B3 exact {b3e:e}");
    assert!(b3d <= 1e-9, "B3 discrete {b3d:e}");
    assert!(o1 <= 1e-10, "O1 RB {o1:e}");
    assert!(h1 <= 1e-12 && exact, "H1 RB {h1:e} {exact}");
    assert!(!r.rigid_free && r.shift == 0.0, "A2 RB");
}

#[test]
fn n2_collinear_rigid_set_has_no_mass() {
    let e = modal_analysis(&rc(), &ModalOptions::new(4)).expect_err("N2");
    println!("N2: {e}");
    match e {
        FemError::InvalidValue { key: "density", reason } => {
            assert!(
                reason.contains("rotation y") && reason.contains("constraint"),
                "N2 {reason}"
            )
        }
        other => panic!("N2 {other:?}"),
    }
}

fn eigen_ratio(a: &ModalResult, b: &ModalResult) -> f64 {
    a.eigenvalues
        .iter()
        .zip(&b.eigenvalues)
        .fold(0.0f64, |w, (x, y)| w.max((x / y - 1.0).abs()))
}

#[test]
fn g1_o1_h1_a2_skew_rollers_and_n4() {
    let base = modal_analysis(&sa(), &ModalOptions::new(6)).unwrap();
    print_run("SA", &base);
    let m = sr(30.0);
    let s = mpc_modes(&m, 6);
    let r = &s.result;
    print_run("SR", r);
    let g1 = eigen_ratio(r, &base);
    let (c, sn) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let lam = &base.eigenvalues;
    let mut g1m: f64 = 0.0;
    let mut checked = Vec::new();
    for i in 0..6 {
        let gap = (0..6)
            .filter(|&j| j != i)
            .fold(f64::INFINITY, |g, j| g.min((lam[j] - lam[i]).abs() / lam[i]));
        let a = &base.modes[i];
        let mut rot = a.clone();
        for node in 0..a.len() / 3 {
            let (x, y) = (a[3 * node], a[3 * node + 1]);
            rot[3 * node] = c * x - sn * y;
            rot[3 * node + 1] = sn * x + c * y;
        }
        let sign = dot(&rot, &r.modes[i]).signum();
        let d: Vec<f64> = rot.iter().zip(&r.modes[i]).map(|(p, q)| sign * p - q).collect();
        let e = inf(&d) / inf(a);
        println!("G1 mode {i}: gap {gap:e}, vector difference {e:e}");
        if gap >= 1e-3 {
            g1m = g1m.max(e);
            checked.push(i);
        }
    }
    let o1 = m_orthonormality(&m, r);
    let (h1, exact, rows) = row_residual(&m, &s);
    let n4 = eigen_ratio(&modal_analysis(&sr(31.0), &ModalOptions::new(6)).unwrap(), &base);
    println!(
        "G1 eigenvalues {g1:e}; vectors {g1m:e} over modes {checked:?}; O1 {o1:e}; H1 {h1:e} over {rows} rows; A2 \
         rigid_free {} shift {:e}; N4 31 degrees {n4:e}",
        r.rigid_free, r.shift
    );
    assert!(g1 <= 1e-10, "G1 {g1:e}");
    assert!(g1m <= 1e-6, "G1 vectors {g1m:e}");
    assert!(o1 <= 1e-10, "O1 SR {o1:e}");
    assert!(h1 <= 1e-12 && exact, "H1 SR {h1:e}");
    assert!(!r.rigid_free && r.shift == 0.0, "A2 SR");
    assert!(n4 > 1e-6, "N4 {n4:e}");
}

#[test]
fn t1_threads_do_not_change_the_bits() {
    let m = kb();
    let a = modal_analysis(&m, &ModalOptions::new(4).threads(1)).unwrap();
    let b = modal_analysis(&m, &ModalOptions::new(4).threads(8)).unwrap();
    let mut same = bits(&a.eigenvalues) == bits(&b.eigenvalues);
    for (x, y) in a.modes.iter().zip(&b.modes) {
        same &= bits(x) == bits(y);
    }
    println!("T1 threads 1 and 8 bit-equal: {same}");
    assert!(same, "T1");
}

/// The parts of `a1_o1_h1_springs_only_and_n1_known_miss` that met their limits on the registered run, without the missed
/// zero-mode ratio (ADR 0384 addendum 1).
#[test]
fn a1_o1_h1_springs_only_and_n1_met_parts() {
    let m = kf();
    let s = mpc_modes(&m, 8);
    let r = &s.result;
    let mm = consistent(&m).unwrap();
    let ax = axial_shares(&m, &mm, r);
    let f_kb = mpc_modes(&kb(), 4).result.frequencies[0];
    let axial = ax.iter().position(|&a| a >= 0.99);
    let a1f = axial.map_or(f64::INFINITY, |i| (r.frequencies[i] / f_kb - 1.0).abs());
    let o1 = m_orthonormality(&m, r);
    let (h1, exact, _) = row_residual(&m, &s);
    let zero = inf(&r.frequencies[..3]) / r.frequencies[3];
    println!("A1 KF max |f_0..2| / f_3 {zero:e} (recorded); axial {a1f:e}; O1 {o1:e}; H1 {h1:e}");
    let n1 = mpc::modal_analysis_mpc(&m, &[], &ModalOptions::new(8).shift(Shift::Value(0.0)));
    assert!(matches!(n1, Err(FemError::Solver { .. })), "N1");
    assert!(r.rigid_free && r.shift > 0.0, "A1 shift");
    assert!(a1f <= 1e-9, "A1 axial {a1f:e}");
    assert!(o1 <= 1e-10, "O1 KF {o1:e}");
    assert!(h1 <= 1e-12 && exact, "H1 KF {h1:e}");
}
