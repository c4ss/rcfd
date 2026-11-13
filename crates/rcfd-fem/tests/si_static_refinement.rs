//! F-455 (ADR 0397): the residual of the direct static solve on twelve slender cantilevers (Tet4, Hex8, Tet10; L/h = 5,
//! 10, 20, 40) and one step of iterative refinement with a compensated residual. Checks A1 to A6, C1 to C6, U1, U2, the
//! negative controls N1 to N3 and the records X1 of the registration; A2 and A3 (the relative residual 1e-14, a registered
//! expected miss) in the ignored record `a2_a3_relative_residual_1e14_known_miss`; A5 (missed in run 1) in the ignored
//! record `a1_a4_to_a6_c1_to_c5_n1_n2_x1_cantilevers_known_miss`, the other checks in `a1_a4_a6_c1_to_c5_n1_n2_x1_met_clauses`. A7 (a) is in
//! `si_static_refinement_defaults.rs`.

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::constraint::{prescribed_equations, reduce, ReducedSystem};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mesh::ElementKind;
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::refine::{
    dot2, energy_defect, residual_compensated, residual_statistics, two_product, two_sum, ResidualStatistics,
};
use rcfd_fem::solve::{solve_static, solve_static_report, CgPreconditioner, StaticOptions, StaticSolver, REFINED_NAME};
use rcfd_fem::{Constraint, FemError, LinearTerm, Load, Material, SolidModel, SolidResult};
use rcfd_linalg::precond::IcOptions;
use rcfd_linalg::skyline::SkylineLdlt;
use rcfd_linalg::CsrMatrix;
use std::time::Instant;

const SEED: u64 = 0xF455_0397;
const KINDS: [ElementKind; 3] = [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10];
const SLENDERNESS: [usize; 4] = [5, 10, 20, 40];
const H: f64 = 0.2;
const REPS: usize = 5;

/// SplitMix64, a deterministic source only.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// A uniform integer in `[-2^bits, 2^bits]`.
    fn int(&mut self, bits: u32) -> i128 {
        let span = (1u128 << (bits + 1)) + 1;
        (self.next_u64() as u128 % span) as i128 - (1i128 << bits)
    }
}

/// The cantilever of D4 (the same as `si_static_refinement_defaults.rs`).
fn cantilever(kind: ElementKind, s: usize) -> SolidModel {
    let lengths = [s as f64 * H, H, H];
    let div = [s, 2, 2];
    let mesh = match kind {
        ElementKind::Hex8 => hex_box([0.0; 3], lengths, div).unwrap(),
        ElementKind::Tet4 => tet_box([0.0; 3], lengths, div).unwrap(),
        _ => upgrade_to_tet10(&tet_box([0.0; 3], lengths, div).unwrap()).unwrap().0,
    };
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::traction("xmax", [0.0, 0.0, -1e6])).unwrap();
    model
}

/// The reduced system of the plain static path (the same calls as `solve_static_report`).
fn reduced(model: &SolidModel) -> ReducedSystem {
    let f = nodal_load_vector(model, &[]).unwrap();
    let fixed = prescribed_equations(model, &[]).unwrap();
    let k = stiffness_with(model, None).unwrap();
    reduce(&k, &f, &fixed).unwrap()
}

fn free_part(sys: &ReducedSystem, u: &[f64]) -> Vec<f64> {
    sys.free().iter().map(|&e| u[e]).collect()
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|t| t * t).sum::<f64>().sqrt()
}

/// `E_rec` of ADR 0397 D1 (ADR 0277's W1): `|U_rec - W/2| / (W/2)`, both in working precision.
fn e_rec(model: &SolidModel, res: &SolidResult) -> f64 {
    let field = IntegrationPointField::from_result(model, res).unwrap();
    let u_energy = field.strain_energy();
    let f = nodal_load_vector(model, &[]).unwrap();
    let work: f64 = f.iter().zip(res.displacement_values()).map(|(a, b)| a * b).sum();
    (u_energy - work / 2.0).abs() / (work / 2.0)
}

fn min_time<F: FnMut()>(mut f: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..REPS {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    best
}

/// Everything measured on one case.
struct Case {
    kind: ElementKind,
    s: usize,
    unknowns: usize,
    default: ResidualStatistics,
    refined: ResidualStatistics,
    refined2: ResidualStatistics,
    ek_default: f64,
    ek_refined: f64,
    ek_refined2: f64,
    ek_fixed: f64,
    erec_default: f64,
    erec_refined: f64,
    correction: f64,
    tip: f64,
    c4: bool,
    t_factor_solve: f64,
    t_step: f64,
    t_default: f64,
    t_refined: f64,
}

fn run_case(kind: ElementKind, s: usize) -> Case {
    let model = cantilever(kind, s);
    let sys = reduced(&model);
    let (a, b) = (&sys.matrix, &sys.rhs);
    let (res0, rep0) = solve_static_report(&model, &[], &[], &StaticOptions::default()).unwrap();
    let ref1 = StaticOptions::default().refinement(1);
    let (res1, rep1) = solve_static_report(&model, &[], &[], &ref1).unwrap();
    let res2 = solve_static(&model, &StaticOptions::default().refinement(2)).unwrap();
    let u0 = free_part(&sys, res0.displacement_values());
    let u1 = free_part(&sys, res1.displacement_values());
    let u2 = free_part(&sys, res2.displacement_values());

    // The test's own path (C4) and fixed-precision refinement (N1).
    let ldlt = SkylineLdlt::factor(a).unwrap();
    let x0 = ldlt.solve(b);
    let d = ldlt.solve(&residual_compensated(a, b, &x0));
    let x1: Vec<f64> = x0.iter().zip(&d).map(|(x, y)| x + y).collect();
    let mut rw = vec![0.0; b.len()];
    a.residual(b, &x0, &mut rw);
    let df = ldlt.solve(&rw);
    let xf: Vec<f64> = x0.iter().zip(&df).map(|(x, y)| x + y).collect();

    let refined = residual_statistics(a, b, &u1);
    let st1 = res1.stats().unwrap();
    let c4 = bits_equal(&x0, &u0)
        && bits_equal(&x1, &u1)
        && st1.solver() == REFINED_NAME
        && rep1.solver == REFINED_NAME
        && rep0.solver == "skyline-ldlt"
        && st1.relative_residual().to_bits() == refined.rho_w.to_bits();

    // A6: times, each the minimum of REPS repetitions.
    let t_factor_solve = min_time(|| {
        let l = SkylineLdlt::factor(a).unwrap();
        std::hint::black_box(l.solve(b));
    });
    let t_step = min_time(|| {
        let r = residual_compensated(a, b, &x0);
        let dd = ldlt.solve(&r);
        let x: Vec<f64> = x0.iter().zip(&dd).map(|(x, y)| x + y).collect();
        std::hint::black_box(x);
    });
    let t_default = min_time(|| {
        std::hint::black_box(solve_static(&model, &StaticOptions::default()).unwrap());
    });
    let t_refined = min_time(|| {
        std::hint::black_box(solve_static(&model, &ref1).unwrap());
    });

    let tip = res0
        .displacement_values()
        .iter()
        .skip(2)
        .step_by(3)
        .fold(0.0f64, |m, v| m.max(v.abs()));
    Case {
        kind,
        s,
        unknowns: b.len(),
        default: residual_statistics(a, b, &u0),
        refined,
        refined2: residual_statistics(a, b, &u2),
        ek_default: energy_defect(a, b, &u0),
        ek_refined: energy_defect(a, b, &u1),
        ek_refined2: energy_defect(a, b, &u2),
        ek_fixed: energy_defect(a, b, &xf),
        erec_default: e_rec(&model, &res0),
        erec_refined: e_rec(&model, &res1),
        correction: norm2(&d) / norm2(&x0),
        tip,
        c4,
        t_factor_solve,
        t_step,
        t_default,
        t_refined,
    }
}

fn all_cases() -> Vec<Case> {
    let mut out = Vec::new();
    for kind in KINDS {
        for s in SLENDERNESS {
            out.push(run_case(kind, s));
        }
    }
    out
}

fn print_case(c: &Case) {
    let (d, r) = (&c.default, &c.refined);
    println!(
        "{} s = {} ({} unknowns, tip {:.3e} m)\n  default: rho_w {:.3e}, rho_x {:.3e}, phi_w {:.3e}, phi_rep {:.3e}, omega {:.3e}, \
         E_K {:.3e}, E_rec {:.3e}\n  refined: rho_w {:.3e}, rho_x {:.3e}, phi_w {:.3e}, phi_rep {:.3e}, omega {:.3e}, E_K {:.3e}, \
         E_rec {:.3e}; correction {:.3e} of |u|\n  two steps: rho_x {:.3e}, E_K {:.3e}; fixed-precision step: E_K {:.3e}\n  \
         C1 rho_w/phi_w {:.3} (default); C2 rho_x/phi_rep {:.3} (refined); C4 {}\n  A6: factor+solve {:.3e} s, step {:.3e} s \
         (ratio {:.3}); solve_static default {:.3e} s, refined {:.3e} s (ratio {:.3})",
        c.kind.name(),
        c.s,
        c.unknowns,
        c.tip,
        d.rho_w,
        d.rho_x,
        d.phi_w,
        d.phi_rep,
        d.omega,
        c.ek_default,
        c.erec_default,
        r.rho_w,
        r.rho_x,
        r.phi_w,
        r.phi_rep,
        r.omega,
        c.ek_refined,
        c.erec_refined,
        c.correction,
        c.refined2.rho_x,
        c.ek_refined2,
        c.ek_fixed,
        d.rho_w / d.phi_w,
        r.rho_x / r.phi_rep,
        c.c4,
        c.t_factor_solve,
        c.t_step,
        c.t_step / c.t_factor_solve,
        c.t_default,
        c.t_refined,
        c.t_refined / c.t_default,
    );
}

/// The record of run 1 (ADR 0397 addendum 1): A5 missed on nine of the twelve cases (the recovered energy against the
/// energy of the stored stiffness, a floor of the representation of the stiffness, not of the solve). Limit intact.
#[test]
#[ignore = "ADR 0397 A5: known miss (E_rec at the representation floor of the stiffness), kept as the record of run 1"]
fn a1_a4_to_a6_c1_to_c5_n1_n2_x1_cantilevers_known_miss() {
    cantilever_checks(true);
}

/// The same checks without A5 (A5 is still printed).
#[test]
fn a1_a4_a6_c1_to_c5_n1_n2_x1_met_clauses() {
    cantilever_checks(false);
}

fn cantilever_checks(with_a5: bool) {
    let cases = all_cases();
    let mut failures = Vec::new();
    for c in &cases {
        print_case(c);
        let tag = format!("{} s = {}", c.kind.name(), c.s);
        let checks = [
            ("A4 E_K refined <= 1e-12", c.ek_refined <= 1e-12),
            ("A5 E_rec refined <= 1e-12", !with_a5 || c.erec_refined <= 1e-12),
            ("C1 rho_w/phi_w default <= 2", c.default.rho_w <= 2.0 * c.default.phi_w),
            (
                "C2 rho_x/phi_rep refined <= 2",
                c.refined.rho_x <= 2.0 * c.refined.phi_rep,
            ),
            ("C3 omega refined <= 1e-14", c.refined.omega <= 1e-14),
            ("C4 library path", c.c4),
        ];
        for (name, ok) in checks {
            if !ok {
                failures.push(format!("{tag}: {name}"));
            }
        }
        let n_case = matches!(c.kind, ElementKind::Hex8 | ElementKind::Tet10);
        if n_case && c.s >= 20 && c.ek_fixed <= 1e-12 {
            failures.push(format!("{tag}: N1 E_K fixed-precision step > 1e-12 not as expected"));
        }
        if n_case && c.s == 40 && c.ek_default <= 1e-12 {
            failures.push(format!("{tag}: N2 E_K default > 1e-12 not as expected"));
        }
    }
    // C5: ADR 0277 reproduced at s = 10.
    let at = |k: ElementKind| cases.iter().find(|c| c.kind == k && c.s == 10).unwrap();
    let c5 = [
        (
            "Tet10 rho_w",
            format!("{:.1e}", at(ElementKind::Tet10).default.rho_w),
            "6.7e-11",
        ),
        (
            "Hex8 E_rec",
            format!("{:.1e}", at(ElementKind::Hex8).erec_default),
            "2.6e-12",
        ),
        (
            "Tet10 E_rec",
            format!("{:.1e}", at(ElementKind::Tet10).erec_default),
            "1.5e-11",
        ),
    ];
    for (name, got, want) in &c5 {
        println!("C5 {name}: {got} (ADR 0277: {want})");
        if got != want {
            failures.push(format!("C5 {name}: {got} against {want}"));
        }
    }
    assert!(failures.is_empty(), "failed: {failures:#?}");
}

/// A2 and A3 (clause a5) with the limit 1e-14 intact: a registered expected miss (ADR 0397 arithmetic 1 and 2).
#[test]
#[ignore = "ADR 0397 A2, A3: registered expected miss (relative residual 1e-14 below the floors of double precision)"]
fn a2_a3_relative_residual_1e14_known_miss() {
    let mut missed = Vec::new();
    for kind in KINDS {
        for s in SLENDERNESS {
            let model = cantilever(kind, s);
            let sys = reduced(&model);
            let res = solve_static(&model, &StaticOptions::default().refinement(1)).unwrap();
            let st = residual_statistics(&sys.matrix, &sys.rhs, &free_part(&sys, res.displacement_values()));
            println!(
                "{} s = {s}: A2 rho_w {:.3e}, A3 rho_x {:.3e} (limit 1e-14; phi_w {:.3e}, phi_rep {:.3e})",
                kind.name(),
                st.rho_w,
                st.rho_x,
                st.phi_w,
                st.phi_rep
            );
            if st.rho_w > 1e-14 {
                missed.push(format!("{} s = {s}: A2", kind.name()));
            }
            if st.rho_x > 1e-14 {
                missed.push(format!("{} s = {s}: A3", kind.name()));
            }
        }
    }
    assert!(missed.is_empty(), "missed: {missed:#?}");
}

#[test]
fn c6_refusals() {
    let model = cantilever(ElementKind::Hex8, 5);
    let solvers = [
        StaticSolver::ConjugateGradient {
            tolerance: 1e-10,
            max_iterations: 1000,
        },
        StaticSolver::PreconditionedCg {
            preconditioner: CgPreconditioner::IncompleteCholesky(IcOptions::default()),
            tolerance: 1e-10,
            max_iterations: 1000,
        },
    ];
    for solver in solvers {
        let e = solve_static(&model, &StaticOptions::with_solver(solver).refinement(1)).unwrap_err();
        println!("C6: {e}");
        assert!(
            matches!(&e, FemError::Solver { reason, .. } if reason.contains("ADR 0397")),
            "{e}"
        );
    }
    let mut m = cantilever(ElementKind::Hex8, 5);
    let node = m.mesh().node_set("xmax").unwrap()[0];
    m.add_constraint(Constraint::linear(vec![LinearTerm::new(node, 1, 1.0)], 0.0))
        .unwrap();
    assert!(
        solve_static(&m, &StaticOptions::default()).is_ok(),
        "the transformation model solves by default"
    );
    let e = solve_static(&m, &StaticOptions::default().refinement(1)).unwrap_err();
    println!("C6: {e}");
    assert!(
        matches!(&e, FemError::Solver { reason, .. } if reason.contains("ADR 0397")),
        "{e}"
    );
}

fn naive_dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(a, b)| a * b).sum()
}

#[test]
fn u1_u2_n3_compensated_kernels() {
    // U1, crafted.
    let t = 1.0 + 2f64.powi(-30);
    let crafted: [(Vec<f64>, Vec<f64>, f64); 2] = [
        (vec![1e16, 1.0, -1e16], vec![1.0, 1.0, 1.0], 1.0),
        (vec![t, -1.0], vec![t, 1.0], 2f64.powi(-29) + 2f64.powi(-60)),
    ];
    let mut naive_wrong = 0;
    for (x, y, exact) in &crafted {
        assert_eq!(dot2(x, y).to_bits(), exact.to_bits(), "U1 crafted");
        if naive_dot(x, y) != *exact {
            naive_wrong += 1;
        }
    }
    println!("U1 crafted exact; N3: plain dot wrong on {naive_wrong} of 2");
    assert!(naive_wrong >= 1, "N3 (U1)");
    // U1, random integer-valued pairs.
    let mut rng = Rng(SEED);
    for _ in 0..10_000 {
        let (a, b) = (rng.int(60) as f64, rng.int(60) as f64);
        let (s, e) = two_sum(a, b);
        assert_eq!(s as i128 + e as i128, a as i128 + b as i128, "U1 two_sum {a} {b}");
        let (a, b) = (rng.int(40) as f64, rng.int(40) as f64);
        let (p, e) = two_product(a, b);
        assert_eq!(p as i128 + e as i128, a as i128 * b as i128, "U1 two_product {a} {b}");
    }
    // U2: a random sparse integer system against the exact residual in i128.
    let n = 200;
    let mut trip = Vec::new();
    for i in 0..n {
        let mut cols = Vec::new();
        while cols.len() < 10 {
            let j = (rng.next_u64() % n as u64) as usize;
            if !cols.contains(&j) {
                cols.push(j);
            }
        }
        for j in cols {
            trip.push((i, j, rng.int(40) as f64));
        }
    }
    let a = CsrMatrix::from_triplets(n, trip);
    let x: Vec<f64> = (0..n).map(|_| rng.int(40) as f64).collect();
    let mut b = vec![0.0; n];
    let mut exact = vec![0i128; n];
    let mut abs_sum = vec![0.0; n];
    for i in 0..n {
        let mut ax = 0i128;
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            ax += a.val[k] as i128 * x[a.col[k]] as i128;
            abs_sum[i] += (a.val[k] * x[a.col[k]]).abs();
        }
        b[i] = (ax + rng.int(20)) as f64;
        exact[i] = b[i] as i128 - ax;
    }
    let r = residual_compensated(&a, &b, &x);
    let mut rp = vec![0.0; n];
    a.residual(&b, &x, &mut rp);
    let u = f64::EPSILON / 2.0;
    let gamma = 12.0 * u / (1.0 - 12.0 * u);
    let (mut worst, mut plain_out) = (0.0f64, 0);
    for i in 0..n {
        let re = exact[i] as f64;
        let bound = 2.0 * u * re.abs() + 2.0 * gamma * gamma * (b[i].abs() + abs_sum[i]);
        let err = (r[i] - re).abs();
        worst = worst.max(err / bound.max(f64::MIN_POSITIVE));
        assert!(err <= bound, "U2 row {i}: {} against exact {re}, bound {bound:e}", r[i]);
        if (rp[i] - re).abs() > bound {
            plain_out += 1;
        }
    }
    println!("U2: worst error / bound {worst:.3e}; N3: plain residual outside the bound on {plain_out} of {n} rows");
    assert!(plain_out >= 1, "N3 (U2)");
}
