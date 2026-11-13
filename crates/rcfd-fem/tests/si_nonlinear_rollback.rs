//! F-497 (ADR 0406): the roll-back of a failed attempt of the nonlinear driver, checked by bit equality.
//! The one-step solve of the load-controlled Hex8 bar of ADR 0404 with four iterations allowed (one halving) against the
//! two-step solve: checks R1 to R4, negative controls N1 and N2.

use rcfd_fem::builders::hex_box;
use rcfd_fem::nonlinear::{solve, NewtonOptions, NonlinearResult};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel};

const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;

fn bar_load_control() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.5, 0.5], [2, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    let p = E_MOD * 1.2 * (1.2 * 1.2 - 1.0) / 2.0;
    m.add_load(Load::traction("xmax", [p, 0.0, 0.0])).unwrap();
    m
}

fn differing(a: &[f64], b: &[f64]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
}

fn same_bits(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && differing(a, b) == 0
}

fn run(model: &SolidModel, steps: usize, max_iterations: usize) -> NonlinearResult {
    solve(
        model,
        &[],
        &[],
        &NewtonOptions::new(steps).max_iterations(max_iterations),
    )
    .unwrap()
}

#[test]
#[ignore = "record of ADR 0406 R3, missed in the registered run by a defect of the test (it compared the user-step number of the one-step solve, 1, with that of the two-step solve, 1 and 2); R1, R2, R4, N1, N2 were met in that run; corrected by ADR 0406 amendment 1, r1_to_r4_n1_n2_corrected"]
fn r1_to_r4_n1_n2_rollback_of_a_failed_attempt() {
    let bar = bar_load_control();
    let one = run(&bar, 1, 4);
    let two = run(&bar, 2, 20);
    let mut missed: Vec<String> = Vec::new();
    let mut check = |name: &str, ok: bool, detail: String| {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            missed.push(format!("{name}: {detail}"));
        }
    };

    let d = differing(one.result.displacement_values(), two.result.displacement_values());
    check(
        "R1 displacement entries differing in bits",
        d == 0,
        format!("{d} (limit == 0)"),
    );

    let (ra, rb) = (one.result.reactions().unwrap(), two.result.reactions().unwrap());
    let d = differing(ra, rb);
    check(
        "R2 reaction entries differing in bits",
        d == 0,
        format!("{d} (limit == 0)"),
    );

    let mut ok = one.steps.len() == 2 && two.steps.len() == 2;
    let mut detail = format!(
        "records {} and {}, halvings {:?}",
        one.steps.len(),
        two.steps.len(),
        one.steps.iter().map(|s| s.halvings).collect::<Vec<_>>()
    );
    if ok {
        for (a, b) in one.steps.iter().zip(&two.steps) {
            let rec = a.halvings == 1
                && a.step == 1
                && b.step == a.step
                && a.load_factor.to_bits() == b.load_factor.to_bits()
                && same_bits(&a.residuals, &b.residuals)
                && same_bits(&a.energies, &b.energies);
            detail += &format!(
                "; load factor {} residuals {} energies {} (lengths {} {})",
                a.load_factor,
                a.residuals.len(),
                a.energies.len(),
                b.residuals.len(),
                b.energies.len()
            );
            ok &= rec;
        }
    }
    check(
        "R3 records (halvings, load factors, residual and energy histories)",
        ok,
        detail,
    );

    let (ia, ib) = (
        one.result.stats().unwrap().iterations(),
        two.result.stats().unwrap().iterations(),
    );
    check(
        "R4 iterations and states",
        ia == ib && one.states == two.states,
        format!("iterations {ia} and {ib}, states equal {}", one.states == two.states),
    );

    // N1: the first attempt really fails.
    let mut o = NewtonOptions::new(1).max_iterations(4);
    o.max_halvings = 0;
    let r = solve(&bar, &[], &[], &o);
    let ok = matches!(&r, Err(FemError::Solver { solver, reason })
        if *solver == "newton" && reason.contains("load step 1 of 1") && reason.contains("0 halvings"));
    check("N1 first attempt fails without halving", ok, format!("{r:?}"));

    // N2: with the default iterations there is no failed attempt.
    let plain = run(&bar, 1, 20);
    let ok = plain.steps.len() == 1 && plain.steps.iter().all(|s| s.halvings == 0) && two.steps.len() == 2;
    check(
        "N2 no failed attempt with 20 iterations",
        ok,
        format!("{} record against {}", plain.steps.len(), two.steps.len()),
    );

    assert!(missed.is_empty(), "missed: {missed:#?}");
}

#[test]
fn r1_to_r4_n1_n2_corrected() {
    let bar = bar_load_control();
    let one = run(&bar, 1, 4);
    let two = run(&bar, 2, 20);
    let mut missed: Vec<String> = Vec::new();
    let mut check = |name: &str, ok: bool, detail: String| {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            missed.push(format!("{name}: {detail}"));
        }
    };

    let d = differing(one.result.displacement_values(), two.result.displacement_values());
    check(
        "R1 displacement entries differing in bits",
        d == 0,
        format!("{d} (limit == 0)"),
    );

    let (ra, rb) = (one.result.reactions().unwrap(), two.result.reactions().unwrap());
    let d = differing(ra, rb);
    check(
        "R2 reaction entries differing in bits",
        d == 0,
        format!("{d} (limit == 0)"),
    );

    let mut ok = one.steps.len() == 2 && two.steps.len() == 2;
    let mut detail = format!(
        "records {} and {}, halvings {:?}",
        one.steps.len(),
        two.steps.len(),
        one.steps.iter().map(|s| s.halvings).collect::<Vec<_>>()
    );
    if ok {
        for (k, (a, b)) in one.steps.iter().zip(&two.steps).enumerate() {
            let rec = a.halvings == 1
                && a.step == 1
                && b.step == k + 1
                && a.load_factor.to_bits() == b.load_factor.to_bits()
                && same_bits(&a.residuals, &b.residuals)
                && same_bits(&a.energies, &b.energies);
            detail += &format!(
                "; load factor {} residuals {} energies {} (lengths {} {})",
                a.load_factor,
                a.residuals.len(),
                a.energies.len(),
                b.residuals.len(),
                b.energies.len()
            );
            ok &= rec;
        }
    }
    check(
        "R3 records (halvings, load factors, residual and energy histories)",
        ok,
        detail,
    );

    let (ia, ib) = (
        one.result.stats().unwrap().iterations(),
        two.result.stats().unwrap().iterations(),
    );
    check(
        "R4 iterations and states",
        ia == ib && one.states == two.states,
        format!("iterations {ia} and {ib}, states equal {}", one.states == two.states),
    );

    // N1: the first attempt really fails.
    let mut o = NewtonOptions::new(1).max_iterations(4);
    o.max_halvings = 0;
    let r = solve(&bar, &[], &[], &o);
    let ok = matches!(&r, Err(FemError::Solver { solver, reason })
        if *solver == "newton" && reason.contains("load step 1 of 1") && reason.contains("0 halvings"));
    check("N1 first attempt fails without halving", ok, format!("{r:?}"));

    // N2: with the default iterations there is no failed attempt.
    let plain = run(&bar, 1, 20);
    let ok = plain.steps.len() == 1 && plain.steps.iter().all(|s| s.halvings == 0) && two.steps.len() == 2;
    check(
        "N2 no failed attempt with 20 iterations",
        ok,
        format!("{} record against {}", plain.steps.len(), two.steps.len()),
    );

    assert!(missed.is_empty(), "missed: {missed:#?}");
}
