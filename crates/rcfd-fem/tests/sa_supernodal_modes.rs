//! F-594 (ADR 0509): the supernodal `L D L^T` in the modal, buckling and direct harmonic solves and on the transformation
//! path. Checks E1 to E8, the negative controls N1 to N3, and the heavy record H1 (`#[ignore]`) of the registration.
//!
//! Model S: the clamped Hex8 cube `cube(11, 7850)` of `sa_supernodal_solve.rs` (4 752 unknowns); model R: S with a rigid set
//! on `xmax` (the transformation path); the heavy cube `cube(32, 7850)` (104 544 unknowns).

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::buckling::{factors, BucklingOptions, BucklingResult};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicResponse};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::mpc::{solve_static_mpc, MpcSolution};
use rcfd_fem::solve::{solve_static, solve_static_with, DirectFactor, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::Factorisation;
use rcfd_linalg::sparse_chol::{factor_count, nested_dissection_order, nested_dissection_order_with_points};
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

fn model_r() -> SolidModel {
    let mut m = model_s();
    m.add_constraint(Constraint::rigid_set("xmax", [1.0, 0.5, 0.5]))
        .unwrap();
    m
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "equal lengths");
    a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max(((x - y) / y).abs()))
}

/// `max |a - b| / max |b|`.
fn vec_rel(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "equal lengths");
    let scale = b.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max((x - y).abs())) / scale
}

/// The complex difference `max_i |x_i - y_i| / max_i |y_i|` of two responses.
fn response_rel(x: &HarmonicResponse, y: &HarmonicResponse) -> f64 {
    let n = y.re.len();
    assert_eq!(x.re.len(), n);
    let (mut d, mut s) = (0.0f64, 0.0f64);
    for i in 0..n {
        d = d.max((x.re[i] - y.re[i]).hypot(x.im[i] - y.im[i]));
        s = s.max(y.re[i].hypot(y.im[i]));
    }
    d / s
}

fn supernodal_modes(n: usize) -> ModalOptions {
    ModalOptions::new(n).factorisation(Factorisation::Supernodal)
}

fn prestress(s: &SolidModel) -> Prestress {
    Prestress::uniform(s, [-1.0e8, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap()
}

#[test]
fn e1_modal_frequencies_agree_with_the_envelope_and_n1() {
    let s = model_s();
    let t = Instant::now();
    let fs = modal_analysis(&s, &supernodal_modes(6)).unwrap();
    let ts = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let fe = modal_analysis(&s, &ModalOptions::new(6)).unwrap();
    let te = t.elapsed().as_secs_f64();
    let fm = modal_analysis(&s, &ModalOptions::new(6).factorisation(Factorisation::Sparse)).unwrap();
    let diff = max_rel(&fs.frequencies, &fe.frequencies);
    let worst = fs.residuals.iter().fold(0.0f64, |m, r| m.max(*r));
    println!(
        "E1: {} unknowns; frequencies supernodal {:?}\n    envelope {:?}\n    max relative difference {diff:.3e} (limit 1e-9); \
         largest supernodal residual {worst:.3e} (limit 1e-10); Sturm {:?}; iterations {} and {}; supernodal {ts:.2} s, \
         envelope {te:.2} s",
        fs.unknowns, fs.frequencies, fe.frequencies, fs.sturm, fs.iterations, fe.iterations
    );
    assert_eq!(fs.unknowns, 4752, "S is cube(11)");
    assert!(diff.is_finite() && worst.is_finite(), "E1 finite");
    assert!(diff <= 1e-9, "E1 {diff:e}");
    assert!(worst <= 1e-10, "E1 residuals");
    assert_eq!(fs.sturm.counted, fs.sturm.expected, "E1 Sturm");
    let (same_e, same_m) = (
        bits(&fs.frequencies) == bits(&fe.frequencies),
        bits(&fs.frequencies) == bits(&fm.frequencies),
    );
    println!("N1: supernodal frequencies bit-equal to the envelope's {same_e}, to the minimum degree's {same_m} (both false)");
    assert!(!same_e && !same_m, "N1");
}

#[test]
fn e2_buckling_factors_agree_with_the_envelope() {
    let s = model_s();
    let p = prestress(&s);
    let bs = factors(
        &s,
        &p,
        &BucklingOptions::new(2).factorisation(Factorisation::Supernodal),
    )
    .unwrap();
    let be = factors(&s, &p, &BucklingOptions::new(2)).unwrap();
    assert_eq!(bs.factors.len(), 2, "E2 count");
    let diff = max_rel(&bs.factors, &be.factors);
    println!(
        "E2: factors supernodal {:?}, envelope {:?}, max relative difference {diff:.3e} (limit 1e-9); Sturm {:?}",
        bs.factors, be.factors, bs.sturm
    );
    assert!(diff.is_finite() && diff <= 1e-9, "E2 {diff:e}");
}

fn harmonic_cases(w1: f64) -> [(&'static str, DirectHarmonicOptions, f64); 3] {
    [
        (
            "(a) loss factor 0.02 at omega_1",
            DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(0.02)),
            w1,
        ),
        ("(b) undamped at 0.5 omega_1", DirectHarmonicOptions::new(), 0.5 * w1),
        (
            "(c) Rayleigh at 2 omega_1",
            DirectHarmonicOptions::new().rayleigh(0.02 * w1, 0.02 / w1),
            2.0 * w1,
        ),
    ]
}

fn omega_1(s: &SolidModel) -> f64 {
    modal_analysis(s, &ModalOptions::new(6)).unwrap().eigenvalues[0].sqrt()
}

#[test]
fn e3_harmonic_response_agrees_with_the_envelope_and_n2() {
    let s = model_s();
    let w1 = omega_1(&s);
    let load = HarmonicLoad::from_model(&s).unwrap();
    for (name, o, w) in harmonic_cases(w1) {
        let he = DirectHarmonic::new(&s, &o).unwrap();
        let hs = DirectHarmonic::new(&s, &o.factor(DirectFactor::SupernodalNestedDissection)).unwrap();
        let t = Instant::now();
        let rs = hs.response(&load, w).unwrap();
        let ts = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let re = he.response(&load, w).unwrap();
        let te = t.elapsed().as_secs_f64();
        let diff = response_rel(&rs, &re);
        println!(
            "E3 {name}: omega {w:.6e} rad/s, complex difference {diff:.3e} (limit 1e-8); supernodal {ts:.3} s, envelope \
             {te:.3} s"
        );
        assert!(diff.is_finite(), "E3 finite");
        assert!(diff <= 1e-8, "E3 {name} {diff:e}");
        if name.starts_with("(a)") {
            let same = bits(&rs.re) == bits(&re.re) && bits(&rs.im) == bits(&re.im);
            println!("N2: supernodal response bit-equal to the envelope's: {same} (must be false)");
            assert!(!same, "N2");
        }
    }
}

#[test]
fn e4_resonance_refused_and_the_minimum_degree_factor_refused() {
    let s = model_s();
    let w1 = omega_1(&s);
    let load = HarmonicLoad::from_model(&s).unwrap();
    let h = DirectHarmonic::new(
        &s,
        &DirectHarmonicOptions::new().factor(DirectFactor::SupernodalNestedDissection),
    )
    .unwrap();
    match h.response(&load, w1) {
        Err(e @ FemError::InvalidValue { key: "frequency", .. }) if e.to_string().contains("resonance") => {
            println!("E4a: {e}")
        }
        other => panic!(
            "E4a: expected the resonance refusal, got {:?}",
            other.map(|r| r.amplitude(0))
        ),
    }
    match h.response(&load, 1.001 * w1) {
        Ok(r) => {
            assert!(r.re.iter().chain(&r.im).all(|v| v.is_finite()), "E4b finite");
            println!("E4b: 1.001 omega_1 accepted");
        }
        Err(e) => panic!("E4b: {e}"),
    }
    match DirectHarmonic::new(
        &s,
        &DirectHarmonicOptions::new().factor(DirectFactor::SparseMinimumDegree),
    ) {
        Err(e @ FemError::InvalidValue { key: "solver", .. }) => println!("E4c: {e}"),
        other => panic!("E4c: expected InvalidValue(solver), got {:?}", other.map(|_| ())),
    }
}

fn mpc(model: &SolidModel, options: &StaticOptions) -> MpcSolution {
    solve_static_mpc(model, &[], &[], &[], options).unwrap()
}

#[test]
fn e5_transformation_path_static_equals_the_envelope_and_n3() {
    let r = model_r();
    let t = Instant::now();
    let ss = mpc(&r, &StaticOptions::with_solver(StaticSolver::SupernodalDirect));
    let ts = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let se = mpc(&r, &StaticOptions::default());
    let te = t.elapsed().as_secs_f64();
    let (us, ue) = (ss.result.displacement_values(), se.result.displacement_values());
    let du = vec_rel(us, ue);
    let flat = |s: &MpcSolution, rot: bool| -> Vec<f64> {
        s.references
            .iter()
            .flat_map(|m| if rot { m.rotation } else { m.translation })
            .collect()
    };
    let dt = vec_rel(&flat(&ss, false), &flat(&se, false));
    let dr = vec_rel(&flat(&ss, true), &flat(&se, true));
    let st = ss.result.stats().unwrap();
    println!(
        "E5: {} reduced unknowns; displacements {du:.3e}, reference translation {dt:.3e}, rotation {dr:.3e} (limits 1e-10); \
         factors {:?} and {:?}, solver {:?}, relative residual {:.3e} (limit 1e-12); supernodal {ts:.3} s, envelope {te:.3} s",
        st.unknowns(),
        ss.report.factor,
        se.report.factor,
        st.solver(),
        st.relative_residual()
    );
    assert!(du.is_finite() && dt.is_finite() && dr.is_finite(), "E5 finite");
    assert!(du <= 1e-10, "E5 displacements {du:e}");
    assert!(dt <= 1e-10, "E5 translation {dt:e}");
    assert!(dr <= 1e-10, "E5 rotation {dr:e}");
    assert_eq!(
        ss.report.factor,
        Some(DirectFactor::SupernodalNestedDissection),
        "E5 factor"
    );
    assert_eq!(se.report.factor, Some(DirectFactor::Envelope), "E5 envelope factor");
    assert_eq!(st.solver(), "sparse-ldlt-supernodal", "E5 name");
    assert!(st.relative_residual() <= 1e-12, "E5 residual");
    let same = bits(us) == bits(ue);
    println!("N3: supernodal displacements bit-equal to Direct's: {same} (must be false)");
    assert!(!same, "N3");
}

#[test]
fn e6_transformation_path_modal_agrees_with_the_envelope() {
    let r = model_r();
    let fs = modal_analysis(&r, &supernodal_modes(6)).unwrap();
    let fe = modal_analysis(&r, &ModalOptions::new(6)).unwrap();
    let diff = max_rel(&fs.frequencies, &fe.frequencies);
    println!(
        "E6: frequencies supernodal {:?}\n    envelope {:?}\n    max relative difference {diff:.3e} (limit 1e-9)",
        fs.frequencies, fe.frequencies
    );
    assert!(diff.is_finite() && diff <= 1e-9, "E6 {diff:e}");
}

type Record = (ModalResult, BucklingResult, HarmonicResponse, Vec<u64>, Vec<u64>);

#[test]
fn e7_bit_identical_for_1_and_8_threads() {
    let s = model_s();
    let r = model_r();
    let p = prestress(&s);
    let w1 = omega_1(&s);
    let load = HarmonicLoad::from_model(&s).unwrap();
    let run = |threads: usize| -> Record {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            let m = modal_analysis(&s, &supernodal_modes(6).threads(threads)).unwrap();
            let b = factors(
                &s,
                &p,
                &BucklingOptions::new(2)
                    .factorisation(Factorisation::Supernodal)
                    .threads(threads),
            )
            .unwrap();
            let h = DirectHarmonic::new(
                &s,
                &DirectHarmonicOptions::new()
                    .loss_factor("all", LossFactor::Constant(0.02))
                    .factor(DirectFactor::SupernodalNestedDissection)
                    .threads(threads),
            )
            .unwrap()
            .response(&load, w1)
            .unwrap();
            let t = mpc(
                &r,
                &StaticOptions::with_solver(StaticSolver::SupernodalDirect).threads(threads),
            );
            (
                m,
                b,
                h,
                bits(t.result.displacement_values()),
                bits(t.result.reactions().unwrap()),
            )
        })
    };
    let a = run(1);
    let b = run(8);
    let flat = |v: &[Vec<f64>]| -> Vec<u64> { v.iter().flat_map(|c| bits(c)).collect() };
    assert_eq!(bits(&a.0.frequencies), bits(&b.0.frequencies), "E7 frequencies");
    assert_eq!(flat(&a.0.modes), flat(&b.0.modes), "E7 modes");
    assert_eq!(bits(&a.1.factors), bits(&b.1.factors), "E7 buckling factors");
    assert_eq!(flat(&a.1.modes), flat(&b.1.modes), "E7 buckling modes");
    assert_eq!(bits(&a.2.re), bits(&b.2.re), "E7 harmonic re");
    assert_eq!(bits(&a.2.im), bits(&b.2.im), "E7 harmonic im");
    assert_eq!(a.3, b.3, "E7 transformation displacements");
    assert_eq!(a.4, b.4, "E7 transformation reactions");
    println!("E7: modal, buckling, harmonic and transformation-path results bit-identical for 1 and 8 threads");
}

#[test]
fn e8_refusals_of_adr_0503_f3b_f3c_repeated() {
    let supernodal = || StaticOptions::with_solver(StaticSolver::SupernodalDirect);
    match solve_static(&model_s(), &supernodal().refinement(1)) {
        Err(e @ FemError::Solver { .. }) => println!("E8 (F3b): {e}"),
        other => panic!("E8 (F3b): expected a refusal, got {other:?}"),
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
        Err(e @ FemError::SingularStiffness { .. }) => println!("E8 (F3c): {e}"),
        other => panic!("E8 (F3c): expected SingularStiffness, got {other:?}"),
    }
}

#[test]
#[ignore = "heavy record (ADR 0509 H1): cube(32), 104 544 unknowns, modal analysis by the supernodal L D L^T; minutes and \
            gigabytes; run alone, its peak working set measured from outside"]
fn h1_heavy_modal_analysis_by_the_supernodal_factor() {
    let model = cube(32, 7850.0);
    let t = Instant::now();
    let r = modal_analysis(&model, &supernodal_modes(6)).unwrap();
    let secs = t.elapsed().as_secs_f64();
    let worst = r.residuals.iter().fold(0.0f64, |m, x| m.max(*x));
    println!(
        "H1: {} unknowns, modal_analysis {secs:.1} s, {} iterations; frequencies {:?}; largest residual {worst:.3e} \
         (limit 1e-10); Sturm {:?}",
        r.unknowns, r.iterations, r.frequencies, r.sturm
    );
    assert_eq!(r.unknowns, 104_544);
    assert_eq!(r.frequencies.len(), 6);
    assert!(
        r.frequencies.iter().all(|f| f.is_finite() && *f > 0.0),
        "H1 frequencies finite and positive"
    );
    assert!(r.frequencies.windows(2).all(|w| w[0] <= w[1]), "H1 ascending");
    assert!(worst.is_finite() && worst <= 1e-10, "H1 residuals");
    assert_eq!(r.sturm.counted, r.sturm.expected, "H1 Sturm");
    // the counts of the factor (symbolic only): the graph order the eigensolver used and the coordinate order
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
    let graph = factor_count(&sys.matrix, &nested_dissection_order(&sys.matrix));
    let coords = factor_count(&sys.matrix, &nested_dissection_order_with_points(&sys.matrix, &points));
    println!(
        "H1: nnz(K_FF) {}, nnz(L) by level structures {graph} ({:.2} GB of values), by coordinates {coords} ({:.3} of it)",
        sys.matrix.nnz(),
        8.0 * graph as f64 / 1e9,
        coords as f64 / graph as f64
    );
}
