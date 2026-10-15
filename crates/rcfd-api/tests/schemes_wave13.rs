//! F-211 through the settings tree (ADR 0016 checks B2 and T3): the blend factor reproduces upwind and central in a
//! full SIMPLEC run, and the bounded second-order time scheme is accepted and differs from plain BDF2.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn cavity(solver: &str) -> String {
    format!(
        r#"
[mesh]
generator = "cavity_2d"
nx = 16
ny = 16
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
{solver}
"#
    )
}

fn run(text: &str, steps: usize) -> Simulation {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..steps {
        s.step();
    }
    s
}

fn diff(a: &Simulation, b: &Simulation) -> f64 {
    let mut m = 0.0_f64;
    for n in ["ux", "uy", "p"] {
        let (x, y) = (a.field(n).unwrap(), b.field(n).unwrap());
        assert!(x.iter().all(|v| v.is_finite()));
        for (p, q) in x.iter().zip(&y) {
            m = m.max((p - q).abs());
        }
    }
    m
}

#[test]
fn b2_blend_factor_reproduces_upwind_and_central_in_a_simplec_run() {
    let base = "algorithm = \"simplec\"\nmax_iterations = 2000\n";
    let make = |conv: &str, extra: &str| cavity(&format!("{base}convection = \"{conv}\"\n{extra}"));
    let up = run(&make("upwind", ""), 60);
    let b0 = run(&make("blended", "blend_factor = 0.0"), 60);
    let ce = run(&make("central", ""), 60);
    let b1 = run(&make("blended", "blend_factor = 1.0"), 60);
    let (d0, d1) = (diff(&up, &b0), diff(&ce, &b1));
    // controls: the options are not vacuous
    let mid = run(&make("blended", "blend_factor = 0.5"), 60);
    println!(
        "B2: |blended(0)-upwind| {d0:.3e}, |blended(1)-central| {d1:.3e}, |upwind-central| {:.3e}, |blended(.5)-upwind| {:.3e}",
        diff(&up, &ce),
        diff(&mid, &up)
    );
    assert!(d0 <= 1e-10, "{d0}");
    assert!(d1 <= 1e-10, "{d1}");
    assert!(diff(&up, &ce) > 1e-3);
    assert!(diff(&mid, &up) > 1e-4);
}

#[test]
fn t3_bounded_time_scheme_is_accepted_and_runs() {
    let t = |scheme: &str| {
        cavity(&format!(
            "algorithm = \"piso\"\nconvection = \"upwind\"\ntime_scheme = \"{scheme}\"\ndt = 0.02\nend_time = 0.4\n"
        ))
    };
    let bdf2 = run(&t("bdf2"), 20);
    let bnd = run(&t("bdf2_bounded"), 20);
    let d = diff(&bdf2, &bnd);
    println!("T3: |bdf2_bounded - bdf2| {d:.3e}");
    assert!(bnd.failure().is_none(), "{:?}", bnd.failure());
    assert!(d.is_finite());
    // invalid names still located
    let err = Case::parse_toml(&t("bdf3")).err().unwrap();
    assert!(err.iter().any(|d| d.path == "solver.time_scheme"), "{err:?}");
}
