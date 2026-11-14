//! F-529 (ADR 0503): the supernodal `L D L^T` after nested dissection in the static solve. Checks F1 to F3, the negative
//! control N4, and the heavy record H1 (`#[ignore]`) of the registration.
//!
//! Model S: the clamped Hex8 cube `cube(11, 7850)` of `sa_sparse_solve.rs` (4 752 unknowns); the heavy cube `cube(46, 0)`
//! (304 842 unknowns).

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::solve::{
    solve_static, solve_static_report, solve_static_with, DirectFactor, StaticOptions, StaticSolver,
};
use rcfd_fem::{Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::sparse_chol::{factor_count, nested_dissection_order_with_points};
use std::time::Instant;

fn cube(n: usize, density: f64) -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [n; 3]).unwrap());
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, density, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1.0e6])).unwrap();
    m
}

fn model_s() -> SolidModel {
    cube(11, 7850.0)
}

fn supernodal() -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::SupernodalDirect)
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn f1_the_static_solve_reports_the_factor_it_built_and_n4() {
    let s = model_s();
    let t = Instant::now();
    let (rs, reps) = solve_static_report(&s, &[], &[], &supernodal()).unwrap();
    let ts = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let (re, repe) = solve_static_report(&s, &[], &[], &StaticOptions::default()).unwrap();
    let te = t.elapsed().as_secs_f64();
    let (_, repm) = solve_static_report(&s, &[], &[], &StaticOptions::with_solver(StaticSolver::SparseDirect)).unwrap();
    let cg = StaticSolver::ConjugateGradient {
        tolerance: 1e-8,
        max_iterations: 20_000,
    };
    let (_, repc) = solve_static_report(&s, &[], &[], &StaticOptions::with_solver(cg)).unwrap();
    let (us, ue) = (rs.displacement_values(), re.displacement_values());
    assert!(us.iter().chain(ue).all(|v| v.is_finite()), "finite displacements");
    let scale = ue.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let diff = us.iter().zip(ue).fold(0.0f64, |m, (a, b)| m.max((a - b).abs())) / scale;
    assert!(diff.is_finite());
    let st = rs.stats().unwrap();
    println!(
        "F1: {} unknowns: factors built: supernodal {:?}, direct {:?}, sparse {:?}, cg {:?}; solver {:?}; max |u_s - u_e| / \
         max |u_e| = {diff:.3e} (limit 1e-10); relative residual {:.3e} (limit 1e-12); supernodal {ts:.3} s, envelope {te:.3} s",
        st.unknowns(),
        reps.factor,
        repe.factor,
        repm.factor,
        repc.factor,
        st.solver(),
        st.relative_residual()
    );
    assert_eq!(st.unknowns(), 4752, "S is cube(11)");
    assert_eq!(
        reps.factor,
        Some(DirectFactor::SupernodalNestedDissection),
        "F1 supernodal"
    );
    assert_eq!(st.solver(), "sparse-ldlt-supernodal", "F1 name");
    assert_eq!(repe.factor, Some(DirectFactor::Envelope), "F1 direct");
    assert_eq!(repm.factor, Some(DirectFactor::SparseMinimumDegree), "F1 sparse");
    assert_eq!(repc.factor, None, "F1 cg");
    assert!(diff <= 1e-10, "F1 difference {diff:e}");
    assert!(st.relative_residual() <= 1e-12, "F1 residual");
    // N4
    assert_ne!(
        bits(us),
        bits(ue),
        "N4: the supernodal displacements are not the envelope's bits"
    );
}

#[test]
fn f2_bit_identical_for_1_and_8_threads() {
    let s = model_s();
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| solve_static(&s, &supernodal().threads(threads)).unwrap())
    };
    let r1 = run(1);
    let r8 = run(8);
    assert_eq!(bits(r1.displacement_values()), bits(r8.displacement_values()), "F2 u");
    assert_eq!(
        bits(r1.reactions().unwrap()),
        bits(r8.reactions().unwrap()),
        "F2 reactions"
    );
    println!("F2: displacements and reactions bit-identical for 1 and 8 threads");
}

#[test]
fn f3_refusals() {
    let mut m = model_s();
    m.add_constraint(Constraint::rigid_set("xmax", [1.0, 0.5, 0.5]))
        .unwrap();
    match solve_static(&m, &supernodal()) {
        Err(e @ FemError::InvalidValue { key: "solver", .. }) => println!("F3a: {e}"),
        other => panic!("F3a: expected InvalidValue(solver), got {other:?}"),
    }
    match solve_static(&model_s(), &supernodal().refinement(1)) {
        Err(e @ FemError::Solver { .. }) => println!("F3b: {e}"),
        other => panic!("F3b: expected a refusal, got {other:?}"),
    }
    // the hinge mechanism of sa_sparse_solve.rs S5
    let a = hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let mut nodes = a.nodes().to_vec();
    let mut conn_b = Vec::new();
    let b = hex_box([1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    for &v in b.blocks()[0].connectivity() {
        let x = b.nodes()[v];
        let id = match nodes.iter().position(|p| *p == x) {
            Some(i) => i,
            None => {
                nodes.push(x);
                nodes.len() - 1
            }
        };
        conn_b.push(id);
    }
    assert_eq!(nodes.len(), 14, "two nodes shared");
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, a.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Hex8, conn_b).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_node_set("clamp", a.node_set("xmin").unwrap().to_vec())
        .unwrap();
    let mut h = SolidModel::new(mesh);
    h.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 0.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        h.add_constraint(Constraint::displacement("clamp", c, 0.0)).unwrap();
    }
    match solve_static_with(&h, &[], &[NodalForceAt::new(13, [0.0, 0.0, 1.0])], &supernodal()) {
        Err(e @ FemError::SingularStiffness { .. }) => println!("F3c: {e}"),
        other => panic!("F3c: expected SingularStiffness, got {other:?}"),
    }
}

#[test]
#[ignore = "heavy record (ADR 0503 H1): cube(46), 304 842 unknowns, solved by the supernodal L D L^T; minutes and gigabytes; \
            run alone, its peak working set measured from outside"]
fn h1_heavy_clamped_cube_solved_by_the_supernodal_factor() {
    let model = cube(46, 0.0);
    let t = Instant::now();
    let (r, rep) = solve_static_report(&model, &[], &[], &supernodal()).unwrap();
    let secs = t.elapsed().as_secs_f64();
    let st = r.stats().unwrap();
    println!(
        "H1: {} unknowns, solver {}, factor {:?}, solve_static_report {secs:.1} s, relative residual {:.3e} (limit 1e-10)",
        st.unknowns(),
        st.solver(),
        rep.factor,
        st.relative_residual()
    );
    // the count of the factor the solve built: the same reduced matrix, points and order (symbolic only)
    let f = nodal_load_vector(&model, &[]).unwrap();
    let fixed = prescribed_equations(&model, &[]).unwrap();
    let k = stiffness_with(&model, None).unwrap();
    let sys = reduce(&k, &f, &fixed).unwrap();
    drop(k);
    let dofs = model.dof_map();
    let points: Vec<[f64; 3]> = sys
        .free()
        .iter()
        .map(|&q| model.mesh().nodes()[dofs.node_component(q).unwrap().0])
        .collect();
    let order = nested_dissection_order_with_points(&sys.matrix, &points);
    let nnz = factor_count(&sys.matrix, &order);
    println!(
        "H1: nnz(K) {}, nnz(L) {nnz}, factor values {:.2} GB",
        sys.matrix.nnz(),
        8.0 * nnz as f64 / 1e9
    );
    assert_eq!(st.unknowns(), 304_842);
    assert_eq!(rep.factor, Some(DirectFactor::SupernodalNestedDissection));
    assert!(
        st.relative_residual().is_finite() && st.relative_residual() <= 1e-10,
        "H1 residual"
    );
}
