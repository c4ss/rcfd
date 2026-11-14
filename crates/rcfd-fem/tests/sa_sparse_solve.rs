//! F-431 (ADR 0447): the sparse `L D L^T` in the static, modal and buckling solves. Checks S1 to S6, the negative control
//! N2, and the heavy records H0 and H1 (`#[ignore]`) of the registration.
//!
//! Model S: the clamped Hex8 cube `cube(11)` of `si_amg.rs` (ADR 0305; `hex_box([0; 3], [1; 3], [11; 3])`, E = 2e11,
//! nu = 0.3, `xmin` clamped, traction (0, 0, -1e6) on `xmax`) with the density 7850 that a modal analysis needs; the heavy
//! cube is `cube(46)` (304 842 unknowns).

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::Factorisation;
use rcfd_linalg::skyline::envelope_size;
use rcfd_linalg::sparse_chol::{factor_count, minimum_degree_order};
use rcfd_linalg::CsrMatrix;
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

/// Model S.
fn model_s() -> SolidModel {
    cube(11, 7850.0)
}

fn sparse() -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::SparseDirect)
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

#[test]
fn s1_static_solution_equals_the_envelope_solve() {
    let s = model_s();
    let t = Instant::now();
    let rs = solve_static(&s, &sparse()).unwrap();
    let ts = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let re = solve_static(&s, &StaticOptions::default()).unwrap();
    let te = t.elapsed().as_secs_f64();
    let (us, ue) = (rs.displacement_values(), re.displacement_values());
    let diff = us.iter().zip(ue).fold(0.0f64, |m, (a, b)| m.max((a - b).abs())) / max_abs(ue);
    let st = rs.stats().unwrap();
    println!(
        "S1: {} unknowns: max |u_s - u_e| / max |u_e| = {diff:.3e} (limit 1e-10); solver {:?}, relative residual {:.3e} \
         (limit 1e-12); sparse {ts:.3} s, envelope {te:.3} s",
        st.unknowns(),
        st.solver(),
        st.relative_residual()
    );
    assert_eq!(st.unknowns(), 4752, "S is cube(11)");
    assert!(diff <= 1e-10, "S1 difference {diff:e}");
    assert_eq!(st.solver(), "sparse-ldlt", "S1 name");
    assert!(st.relative_residual() <= 1e-12, "S1 residual");
}

#[test]
fn s2_modal_frequencies_through_either_factorisation_and_n2() {
    let s = model_s();
    let fs = modal_analysis(&s, &ModalOptions::new(6).factorisation(Factorisation::Sparse)).unwrap();
    let fe = modal_analysis(&s, &ModalOptions::new(6)).unwrap();
    let diff = fs
        .frequencies
        .iter()
        .zip(&fe.frequencies)
        .fold(0.0f64, |m, (a, b)| m.max(((a - b) / b).abs()));
    println!(
        "S2: frequencies sparse {:?}\n    envelope {:?}\n    max relative difference {diff:.3e} (limit 1e-9); iterations {} and {}",
        fs.frequencies, fe.frequencies, fs.iterations, fe.iterations
    );
    assert!(diff <= 1e-9, "S2 {diff:e}");
    let same = bits(&fs.frequencies) == bits(&fe.frequencies);
    println!("N2: sparse frequencies bit-equal to the envelope's: {same} (must be false)");
    assert!(!same, "N2: the sparse factorisation did not run");
}

#[test]
fn s3_buckling_factors_through_either_factorisation() {
    let s = model_s();
    let p = Prestress::uniform(&s, [-1.0e8, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let bs = factors(&s, &p, &BucklingOptions::new(2).factorisation(Factorisation::Sparse)).unwrap();
    let be = factors(&s, &p, &BucklingOptions::new(2)).unwrap();
    let diff = bs
        .factors
        .iter()
        .zip(&be.factors)
        .fold(0.0f64, |m, (a, b)| m.max(((a - b) / b).abs()));
    println!(
        "S3: factors sparse {:?}, envelope {:?}, max relative difference {diff:.3e} (limit 1e-9)",
        bs.factors, be.factors
    );
    assert_eq!(bs.factors.len(), 2);
    assert!(diff <= 1e-9, "S3 {diff:e}");
}

#[test]
fn s4_bit_identical_for_1_and_8_threads() {
    let s = model_s();
    let r1 = solve_static(&s, &sparse().threads(1)).unwrap();
    let r8 = solve_static(&s, &sparse().threads(8)).unwrap();
    assert_eq!(bits(r1.displacement_values()), bits(r8.displacement_values()), "S4 u");
    assert_eq!(bits(r1.reactions().unwrap()), bits(r8.reactions().unwrap()), "S4 reactions");
    println!("S4: displacements and reactions bit-identical for 1 and 8 threads");
}

#[test]
fn s5_hinge_mechanism_is_refused() {
    // The model of sb_static.rs E5: cube A [0, 1]^3 and cube B [1, 2] x [0, 1] x [1, 2] sharing the edge x = 1, z = 1.
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
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 0.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("clamp", c, 0.0)).unwrap();
    }
    let r = solve_static_with(&m, &[], &[NodalForceAt::new(13, [0.0, 0.0, 1.0])], &sparse());
    match r {
        Err(e @ FemError::SingularStiffness { .. }) => println!("S5: {e}"),
        other => panic!("S5: expected SingularStiffness, got {other:?}"),
    }
}

#[test]
fn s6_refused_on_the_transformation_path_and_with_refinement() {
    let mut m = model_s();
    m.add_constraint(Constraint::rigid_set("xmax", [1.0, 0.5, 0.5])).unwrap();
    match solve_static(&m, &sparse()) {
        Err(e @ FemError::InvalidValue { key: "solver", .. }) => println!("S6a: {e}"),
        other => panic!("S6a: expected InvalidValue(solver), got {other:?}"),
    }
    match solve_static(&model_s(), &sparse().refinement(1)) {
        Err(e @ FemError::Solver { .. }) => println!("S6b: {e}"),
        other => panic!("S6b: expected a refusal, got {other:?}"),
    }
}

/// The reduced stiffness and load of a model (as the static solve forms them).
fn reduced(model: &SolidModel) -> (CsrMatrix, Vec<f64>) {
    let f = nodal_load_vector(model, &[]).unwrap();
    let fixed = prescribed_equations(model, &[]).unwrap();
    let k = stiffness_with(model, None).unwrap();
    let sys = reduce(&k, &f, &fixed).unwrap();
    (sys.matrix.clone(), sys.rhs.clone())
}

#[test]
#[ignore = "heavy record (ADR 0447 H0): cube(46), 304 842 unknowns: the ordering, nnz(L) and the envelope's estimate"]
fn h0_heavy_symbolic_and_envelope_estimate() {
    let model = cube(46, 0.0);
    let t = Instant::now();
    let (k, _) = reduced(&model);
    let ta = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let order = minimum_degree_order(&k);
    let to = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let nnz = factor_count(&k, &order);
    let tc = t.elapsed().as_secs_f64();
    let n = k.n;
    let bytes = 8 * (2 * nnz + (n + 1) + 3 * n);
    println!(
        "H0: n = {n}, nnz(K) = {}, assembly and reduction {ta:.1} s, ordering {to:.1} s, count {tc:.1} s; nnz(L) = {nnz}, \
         predicted factor bytes {bytes} ({:.2} GB)",
        k.nnz(),
        bytes as f64 / 1e9
    );
    // D6: the envelope after RCM, from the matrix only.
    let sky = rcfd_linalg::reorder::rcm_order(n, &lower_upper(&k).0, &lower_upper(&k).1);
    let env = envelope_size(&k, Some(&sky));
    let inv = rcfd_linalg::reorder::inverse_permutation(&sky);
    let mut first: Vec<usize> = (0..n).collect();
    for r in 0..n {
        for q in k.row_ptr[r]..k.row_ptr[r + 1] {
            let (a, b) = (inv[r], inv[k.col[q]]);
            let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
            first[hi] = first[hi].min(lo);
        }
    }
    let work: f64 = first
        .iter()
        .enumerate()
        .map(|(i, &f)| {
            let w = (i - f) as f64;
            w * (w + 1.0) / 2.0
        })
        .sum();
    let ebytes = 8 * (env + n) + 8 * (3 * n + 1);
    println!(
        "H0: envelope after RCM {env} entries, {ebytes} bytes ({:.2} GB), work {work:.3e} multiply-adds, estimated time \
         {:.0} s at 4.6e9 per second (ADR 0274)",
        ebytes as f64 / 1e9,
        work / 4.6e9
    );
}

fn lower_upper(k: &CsrMatrix) -> (Vec<usize>, Vec<usize>) {
    let (mut lower, mut upper) = (Vec::new(), Vec::new());
    for r in 0..k.n {
        for q in k.row_ptr[r]..k.row_ptr[r + 1] {
            let c = k.col[q];
            if c < r {
                lower.push(c);
                upper.push(r);
            } else if c > r {
                lower.push(r);
                upper.push(c);
            }
        }
    }
    (lower, upper)
}

#[test]
#[ignore = "heavy record (ADR 0447 H1): cube(46), 304 842 unknowns, solved by the sparse L D L^T; minutes and gigabytes"]
fn h1_heavy_clamped_cube_solved() {
    let model = cube(46, 0.0);
    let t = Instant::now();
    let r = solve_static(&model, &sparse()).unwrap();
    let secs = t.elapsed().as_secs_f64();
    let st = r.stats().unwrap();
    // The counts of the factor the solve built (the same pattern, order and formula as `storage_bytes`).
    let (k, _) = reduced(&model);
    let nnz = factor_count(&k, &minimum_degree_order(&k));
    let bytes = 8 * (2 * nnz + (k.n + 1) + 3 * k.n);
    println!(
        "H1: {} unknowns, solver {}, solve_static {secs:.1} s, relative residual {:.3e} (limit 1e-10), nnz(L) {nnz},          factor storage {bytes} bytes ({:.2} GB)",
        st.unknowns(),
        st.solver(),
        st.relative_residual(),
        bytes as f64 / 1e9
    );
    assert_eq!(st.unknowns(), 304_842);
    assert!(st.relative_residual() <= 1e-10, "H1 residual");
}
