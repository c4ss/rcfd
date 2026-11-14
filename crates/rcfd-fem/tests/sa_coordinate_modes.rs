//! F-595 (ADR 0531): the coordinate nested dissection of the supernodal factor in the modal and buckling analyses. Checks E1
//! to E5, the negative controls N1 and N2, and the heavy record H1 (`#[ignore]`) of the registration.
//!
//! Model S: the clamped Hex8 cube `cube(11, 7850)` of `sa_supernodal_modes.rs` (4 752 unknowns); model R: S with a rigid set
//! on `xmax` (the transformation path); the heavy cube C `cube(32, 7850)` (104 544 unknowns).

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::buckling::{factors, BucklingOptions, BucklingResult};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::mass::consistent_with;
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel};
use rcfd_linalg::eigen::{
    lowest_buckling_factors, lowest_eigenpairs, BucklingEigenOptions, Factorisation, ResidualTest, Shift,
    SubspaceOptions,
};
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

fn prestress(s: &SolidModel) -> Prestress {
    Prestress::uniform(s, [-1.0e8, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn flat(v: &[Vec<f64>]) -> Vec<u64> {
    v.iter().flat_map(|c| bits(c)).collect()
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "equal lengths");
    a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max(((x - y) / y).abs()))
}

fn levels_modes(n: usize) -> ModalOptions {
    ModalOptions::new(n).factorisation(Factorisation::Supernodal)
}

fn coords_modes(n: usize) -> ModalOptions {
    levels_modes(n).coordinate_order(true)
}

fn levels_buckling(n: usize) -> BucklingOptions {
    BucklingOptions::new(n).factorisation(Factorisation::Supernodal)
}

fn coords_buckling(n: usize) -> BucklingOptions {
    levels_buckling(n).coordinate_order(true)
}

/// The node coordinates of the free equations of `model` (built here, independently of the library).
fn free_points(model: &SolidModel, free: &[usize]) -> Vec<[f64; 3]> {
    let dofs = model.dof_map();
    free.iter()
        .map(|&q| model.mesh().nodes()[dofs.node_component(q).unwrap().0])
        .collect()
}

fn modal_e1(s: &SolidModel) -> (ModalResult, ModalResult) {
    (
        modal_analysis(s, &coords_modes(6)).unwrap(),
        modal_analysis(s, &levels_modes(6)).unwrap(),
    )
}

#[test]
fn e1_modal_coordinates_against_levels_and_n1() {
    let s = model_s();
    let t = Instant::now();
    let (c, l) = modal_e1(&s);
    let secs = t.elapsed().as_secs_f64();
    let diff = max_rel(&c.frequencies, &l.frequencies);
    let worst = c.residuals.iter().fold(0.0f64, |m, x| m.max(*x));
    let same = bits(&c.frequencies) == bits(&l.frequencies) && flat(&c.modes) == flat(&l.modes);
    println!(
        "E1: {} unknowns; frequencies {:?}; coordinates against levels {diff:.3e} (limit 1e-9); largest residual \
         {worst:.3e}; Sturm {:?}; iterations {} and {}; both runs {secs:.2} s. N1: bit-equal {same}",
        c.unknowns, c.frequencies, c.sturm, c.iterations, l.iterations
    );
    assert!(diff.is_finite() && diff <= 1e-9, "E1 {diff:e}");
    assert!(worst.is_finite() && worst <= 1e-10, "E1 residuals {worst:e}");
    assert_eq!(c.sturm.counted, c.sturm.expected, "E1 Sturm");
    assert!(!same, "N1: the coordinate order must change the factor");
}

#[test]
fn e2_buckling_coordinates_against_levels_and_n2() {
    let s = model_s();
    let p = prestress(&s);
    let c = factors(&s, &p, &coords_buckling(2)).unwrap();
    let l = factors(&s, &p, &levels_buckling(2)).unwrap();
    assert_eq!(c.factors.len(), 2, "E2 count");
    let diff = max_rel(&c.factors, &l.factors);
    let same = bits(&c.factors) == bits(&l.factors) && flat(&c.modes) == flat(&l.modes);
    println!(
        "E2: factors {:?}; coordinates against levels {diff:.3e} (limit 1e-9). N2: bit-equal {same}",
        c.factors
    );
    assert!(diff.is_finite() && diff <= 1e-9, "E2 {diff:e}");
    assert!(!same, "N2: the coordinate order must change the factor");
}

#[test]
fn e3_the_points_passed_are_those_of_the_free_unknowns() {
    let s = model_s();
    let fixed = prescribed_equations(&s, &[]).unwrap();
    let k = stiffness_with(&s, None).unwrap();
    let zero = vec![0.0; k.n];
    let kr = reduce(&k, &zero, &fixed).unwrap();
    let free = kr.free().to_vec();
    let points = free_points(&s, &free);
    // modal
    let m = consistent_with(&s, None).unwrap();
    let mr = reduce(&m, &zero, &fixed).unwrap();
    let opts = SubspaceOptions {
        shift: Shift::Value(0.0),
        factorisation: Factorisation::Supernodal,
        points: Some(points.clone()),
        ..SubspaceOptions::default()
    };
    let direct = lowest_eigenpairs(&kr.matrix, &mr.matrix, 6, &opts).unwrap();
    let fem = modal_analysis(&s, &coords_modes(6)).unwrap();
    let at_free =
        |modes: &[Vec<f64>]| -> Vec<Vec<f64>> { modes.iter().map(|v| free.iter().map(|&q| v[q]).collect()).collect() };
    assert_eq!(bits(&direct.values), bits(&fem.eigenvalues), "E3 modal values");
    assert_eq!(flat(&direct.vectors), flat(&at_free(&fem.modes)), "E3 modal vectors");
    // buckling
    let g = geometric_stiffness(&s, &prestress(&s), None).unwrap();
    let gr = reduce(&g, &zero, &fixed).unwrap();
    let bopts = BucklingEigenOptions {
        tolerance: 1e-10,
        residual_test: ResidualTest::Backward,
        max_iterations: 200,
        factorisation: Factorisation::Supernodal,
        points: Some(points),
        ..BucklingEigenOptions::default()
    };
    let bdirect = lowest_buckling_factors(&kr.matrix, &gr.matrix, 2, &bopts).unwrap();
    let bfem: BucklingResult = factors(&s, &prestress(&s), &coords_buckling(2)).unwrap();
    assert_eq!(bits(&bdirect.values), bits(&bfem.factors), "E3 buckling values");
    assert_eq!(
        flat(&bdirect.vectors),
        flat(&at_free(&bfem.modes)),
        "E3 buckling vectors"
    );
    println!(
        "E3: modal and buckling with coordinates equal the eigensolvers given the free unknowns' node points, bit for bit \
         ({} free unknowns)",
        free.len()
    );
}

type Record = (ModalResult, BucklingResult);

#[test]
fn e4_bit_identical_for_1_and_8_threads() {
    let s = model_s();
    let p = prestress(&s);
    let run = |threads: usize| -> Record {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            (
                modal_analysis(&s, &coords_modes(6).threads(threads)).unwrap(),
                factors(&s, &p, &coords_buckling(2).threads(threads)).unwrap(),
            )
        })
    };
    let a = run(1);
    let b = run(8);
    assert_eq!(bits(&a.0.frequencies), bits(&b.0.frequencies), "E4 frequencies");
    assert_eq!(flat(&a.0.modes), flat(&b.0.modes), "E4 modes");
    assert_eq!(bits(&a.1.factors), bits(&b.1.factors), "E4 buckling factors");
    assert_eq!(flat(&a.1.modes), flat(&b.1.modes), "E4 buckling modes");
    println!("E4: modal and buckling results with coordinates bit-identical for 1 and 8 threads");
}

fn expect_factorisation_refusal<T: std::fmt::Debug>(tag: &str, r: Result<T, FemError>) {
    match r {
        Err(FemError::InvalidValue { key, reason }) => {
            println!("{tag}: key {key}: {reason}");
            assert_eq!(key, "factorisation", "{tag} key");
        }
        other => panic!("{tag}: expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn e5_refusals_of_the_coordinate_option() {
    let s = model_s();
    let r = model_r();
    let p = prestress(&s);
    expect_factorisation_refusal("E5a", modal_analysis(&s, &ModalOptions::new(6).coordinate_order(true)));
    expect_factorisation_refusal("E5b", factors(&s, &p, &BucklingOptions::new(2).coordinate_order(true)));
    expect_factorisation_refusal("E5c", modal_analysis(&r, &coords_modes(6)));
    let pr = prestress(&r);
    expect_factorisation_refusal("E5d", factors(&r, &pr, &coords_buckling(2)));
}

#[test]
#[ignore = "heavy record (ADR 0531 H1): cube(32), 104 544 unknowns, two modal analyses by the supernodal L D L^T (level \
            structures, coordinates); minutes and gigabytes; run alone, its peak working set measured from outside"]
fn h1_heavy_coordinate_order_on_the_large_cube() {
    let model = cube(32, 7850.0);
    let check = |tag: &str, r: &ModalResult, secs: f64| {
        let worst = r.residuals.iter().fold(0.0f64, |m, x| m.max(*x));
        println!(
            "H1 {tag}: {} unknowns, modal_analysis {secs:.1} s, {} iterations; frequencies {:?}; largest residual \
             {worst:.3e} (limit 1e-10); Sturm {:?}",
            r.unknowns, r.iterations, r.frequencies, r.sturm
        );
        assert_eq!(r.unknowns, 104_544, "H1 {tag} unknowns");
        assert_eq!(r.frequencies.len(), 6, "H1 {tag} count");
        assert!(
            r.frequencies.iter().all(|f| f.is_finite() && *f > 0.0),
            "H1 {tag} frequencies finite and positive"
        );
        assert!(r.frequencies.windows(2).all(|w| w[0] <= w[1]), "H1 {tag} ascending");
        assert!(worst.is_finite() && worst <= 1e-10, "H1 {tag} residuals");
        assert_eq!(r.sturm.counted, r.sturm.expected, "H1 {tag} Sturm");
    };
    let t = Instant::now();
    let l = modal_analysis(&model, &levels_modes(6)).unwrap();
    check("levels", &l, t.elapsed().as_secs_f64());
    let lf = l.frequencies.clone();
    drop(l);
    let t = Instant::now();
    let c = modal_analysis(&model, &coords_modes(6)).unwrap();
    check("coordinates", &c, t.elapsed().as_secs_f64());
    let diff = max_rel(&c.frequencies, &lf);
    println!("H1: frequencies, coordinates against levels {diff:.3e} (limit 1e-9)");
    drop(c);
    // the counts of the factor of K_FF (symbolic) by the two orders
    let fixed = prescribed_equations(&model, &[]).unwrap();
    let k = stiffness_with(&model, None).unwrap();
    let zero = vec![0.0; k.n];
    let sys = reduce(&k, &zero, &fixed).unwrap();
    drop(k);
    let points = free_points(&model, sys.free());
    let graph = factor_count(&sys.matrix, &nested_dissection_order(&sys.matrix));
    let coords = factor_count(&sys.matrix, &nested_dissection_order_with_points(&sys.matrix, &points));
    let ratio = coords as f64 / graph as f64;
    println!(
        "H1: nnz(K_FF) {}, nnz(L) by level structures {graph} ({:.3} GB of values), by coordinates {coords} ({:.3} GB); \
         ratio {ratio:.4} (limit 0.85)",
        sys.matrix.nnz(),
        8.0 * graph as f64 / 1e9,
        8.0 * coords as f64 / 1e9
    );
    assert!(diff.is_finite() && diff <= 1e-9, "H1 frequencies {diff:e}");
    assert!(ratio.is_finite() && ratio <= 0.85, "H1 nnz ratio {ratio}");
}
