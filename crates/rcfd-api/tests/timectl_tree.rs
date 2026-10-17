//! ADR 0050 S11: the case-tree front end of the adaptive time-step control equals the library driver, lands on the write
//! times, keeps unset defaults inert and reports located errors. Limits as registered in the ADR.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::stepctl::{ControlledRun, StepController, StepSettings};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::f64::consts::PI;
use std::sync::Arc;

fn tg_text(solver_extra: &str, output: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 16
ny = 16
lx = {l:.17e}
ly = {l:.17e}
periodic_x = true
periodic_y = true
[physics]
nu = 0.05
[initial]
velocity_expression_x = "sin(x/(1 [m]))*cos(y/(1 [m])) * 1 [m/s]"
velocity_expression_y = "-cos(x/(1 [m]))*sin(y/(1 [m])) * 1 [m/s]"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.1
end_time = 1.0
{solver_extra}
[output]
{output}
"#,
        l = 2.0 * PI
    )
}

fn sim(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

fn err_path(text: &str) -> String {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    match Simulation::new(&case) {
        Ok(_) => panic!("expected an error"),
        Err(d) => d[0].path.clone(),
    }
}

fn library(settings: StepSettings, dt0: f64) -> (PimpleSolver, ControlledRun) {
    let l = 2.0 * PI;
    let mesh = Arc::new(box_2d(16, 16, l, l, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = PimpleSettings::piso(0.05);
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom, u, p, st);
    let mut run = ControlledRun::new(StepController::new(settings, dt0), false);
    run.run(&mut s, 1.0);
    (s, run)
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// Tree dt sequence and state equal the library run bit for bit; landing on the write times to 1e-12 relative.
#[test]
fn s11_tree_equals_library_and_lands_on_write_times() {
    let mut s = sim(&tg_text(
        "max_courant = 0.3\nmin_dt = 1e-4\ndt_growth = 1.2\n",
        "write_times = [0.25, 0.7]",
    ));
    s.run(10000);
    let settings = StepSettings {
        co_max: 0.3,
        safety: 0.9,
        growth: 1.2,
        shrink: 0.5,
        min_dt: 1e-4,
        max_dt: 0.1,
        write_times: vec![0.25, 0.7],
        ..StepSettings::default()
    };
    let (lib, run) = library(settings, 0.1);
    let tree_times = s.landed_times().to_vec();
    println!("landed {:?}, library {:?}", tree_times, run.landed);
    assert_eq!(tree_times.len(), run.landed.len());
    for (a, b) in tree_times.iter().zip(&run.landed) {
        assert_eq!(a.to_bits(), b.to_bits());
    }
    for (t, want) in tree_times.iter().zip([0.25, 0.7, 1.0]) {
        assert!((t - want).abs() <= 1e-12 * want, "{t} vs {want}");
    }
    assert_eq!(
        bits(&s.field("ux").unwrap()),
        bits(&lib.u.values.iter().map(|v| v[0]).collect::<Vec<_>>())
    );
    assert_eq!(bits(&s.field("p").unwrap()), bits(&lib.p.values));
    assert_eq!(s.proposed_dt().map(f64::to_bits), Some(run.ctl.dt_next.to_bits()));
    // the handed-out landings are the same, once each
    assert_eq!(s.take_write_landing().map(f64::to_bits), Some(tree_times[0].to_bits()));
    assert_eq!(s.take_write_landing().map(f64::to_bits), Some(tree_times[1].to_bits()));
}

/// Unset keys are inert: no controller, fixed dt, legacy `max_courant` alone does not switch the new path on.
#[test]
fn s11_unset_defaults_are_inert() {
    let mut a = sim(&tg_text("", ""));
    assert!(a.proposed_dt().is_none());
    a.run(100);
    assert!(a.landed_times().is_empty());
    let mut b = sim(&tg_text("max_courant = 0.3", ""));
    assert!(b.proposed_dt().is_none());
    b.run(1000);
    assert!(b.landed_times().is_empty());
    assert!(b.take_write_landing().is_none());
}

#[test]
fn s11_located_errors() {
    assert_eq!(err_path(&tg_text("dt_growth = 0.5", "")), "solver.dt_growth");
    assert_eq!(
        err_path(&tg_text("max_courant_rms = 0.3\nmax_dt = 1e-5\nmin_dt = 1e-3", "")),
        "solver.min_dt"
    );
    assert_eq!(err_path(&tg_text("", "write_times = [2.0]")), "output.write_times");
    assert_eq!(err_path(&tg_text("period = 1.0", "")), "solver.steps_per_period");
    let steady = "[mesh]\ngenerator = \"cavity_2d\"\nnx = 8\nny = 8\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"simplec\"\nmax_courant_rms = 0.3\n";
    assert_eq!(err_path(steady), "solver.max_courant_rms");
}

/// Restart: a saved and restored run continues with the identical step sequence (controller state in the file).
#[test]
fn s11_restart_continues_the_step_sequence() {
    let text = tg_text("max_courant = 0.3\nmin_dt = 1e-4\nmax_courant_rms = 0.2\n", "");
    let mut full = sim(&text);
    full.run(10000);
    let mut half = sim(&text);
    for _ in 0..5 {
        half.step();
    }
    let dir = std::env::temp_dir().join("rcfd_timectl_restart");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("r.rcfd");
    half.save_restart(&path).unwrap();
    let mut resumed = sim(&text);
    resumed.load_restart(&path).unwrap();
    resumed.run(10000);
    assert_eq!(bits(&full.field("ux").unwrap()), bits(&resumed.field("ux").unwrap()));
    assert_eq!(full.time().to_bits(), resumed.time().to_bits());
}
