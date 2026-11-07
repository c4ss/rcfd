//! F-255 (ADR 0208): particle dispersion from the turbulence model's fields, in the case tree and the run path. This file
//! starts with the records R8 and R9 of ADR 0208 (checks D2, D3), made on main d953cd6 before any code of F-255; the
//! registered checks CT1 to CT8 and the parts of D2 and D3 that need the code follow them, with the key of its amendment
//! 2. The limits are those of ADR 0208, fixed before the code ran.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Case TC of ADR 0208: the box of case T (`box_3d`, 5 x 5 x 5 cells of 80 m, every wall at rest), the k-epsilon model at
/// its uniform initial state (k 1.5, epsilon 0.225), a cloud of 20 000 tracer parcels (d 1.8e-5 m, tau 1e-3 s) released at
/// the centre, with the random walk on the constants `dispersion_k`, `dispersion_epsilon` of ADR 0057.
const TC_CASE: &str = r#"
title = "dispersion TC"
[mesh]
generator = "box_3d"
nx = 5
ny = 5
nz = 5
lx = 400.0
ly = 400.0
lz = 400.0
[physics]
nu = 1.5e-5
rho = 1.2
[turbulence]
model = "k_epsilon"
k_initial = 1.5
epsilon_initial = 0.225
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[boundaries.zmin]
velocity = "fixed"
[boundaries.zmax]
velocity = "fixed"
[solver]
algorithm = "simple"
max_iterations = 2
dt = 0.01
[particles]
enabled = true
density = 1000.0
drag = "stokes"
integration = "analytic"
gravity = false
end_time = 10.0
dt = 0.05
dispersion = "drw"
dispersion_k = 1.5
dispersion_epsilon = 0.225
dispersion_seed = 20261107
[particles.injection.c]
kind = "point"
position = [200.0, 200.0, 200.0]
rate = 1.0e8
end_time = 2.0e-4
diameter = 1.8e-5
"#;

/// Case TCR of ADR 0208: TC with 2000 parcels tracked to 2 s (the run-path record R9).
fn tcr_text() -> String {
    TC_CASE
        .replace("rate = 1.0e8", "rate = 1.0e7")
        .replace("end_time = 10.0", "end_time = 2.0")
}

fn parse(t: &str) -> Case {
    Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"))
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f255_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The run path of the library (`drive` plus `write_results`) into `dir`.
fn run_path(t: &str, dir: &Path) -> Simulation {
    let case = parse(t);
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let m = Mutex::new(sim);
    let opts = RunOptions {
        out_dir: Some(dir.to_path_buf()),
        ..Default::default()
    };
    let out = runctl::drive(&m, &opts, &|| false, &mut |_| {});
    assert!(out.exit_code == 0 || out.exit_code == 5, "{out:?}");
    let sim = m.into_inner().unwrap();
    runctl::write_results(&sim, dir).unwrap();
    sim
}

/// FNV-1a 64 of the particle output files of `dir` (names sorted, each name then its bytes), as ADR 0192 R2.
fn particle_files_hash(dir: &Path) -> (u64, Vec<String>) {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("particle"))
        .collect();
    names.sort();
    let mut h = rcfd_core::hash::Fnv64::new();
    for n in &names {
        h.write(n.as_bytes());
        h.write(&std::fs::read(dir.join(n)).unwrap());
    }
    (h.finish(), names)
}

/// ADR 0208 R8: the settings content hash of case TC on main d953cd6 (a settings hash has no platform value).
const R8: u64 = 0xfcb3_9f54_9396_9989;

/// ADR 0208 R9: FNV-1a 64 of the names and bytes of the particle output files of case TCR through the run path on main
/// d953cd6 (Windows; the flow and the particles call `exp`, whose last bits may differ elsewhere, ADR 0137).
const R9: u64 = 0x9897_7560_c22d_623c;

/// ADR 0208 R8 (check D2): the settings hash of case TC is that of the base code. `RCFD_F255_RECORD=1` prints the value
/// only.
#[test]
fn d2_dispersion_case_settings_hash_matches_the_base_record() {
    let h = parse(TC_CASE).tree.content_hash();
    println!("R8 {h:016x}");
    if std::env::var("RCFD_F255_RECORD").as_deref() == Ok("1") {
        return;
    }
    assert_eq!(h, R8, "R8 {h:016x} differs from the base record {R8:016x}");
}

/// ADR 0208 R9 (check D3): a random-walk run on the constants writes the same particle files as on the base code. Compared
/// on Windows, printed elsewhere (ADR 0137). `RCFD_F255_RECORD=1` prints the value only.
#[test]
fn d3_dispersion_run_outputs_match_the_base_record() {
    let dir = tmp("d3");
    run_path(&tcr_text(), &dir);
    let (h, names) = particle_files_hash(&dir);
    println!("R9 {h:016x} over {names:?}");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(names.iter().any(|n| n == "particles.vtp"), "set-up: {names:?}");
    if std::env::var("RCFD_F255_RECORD").as_deref() == Ok("1") || !cfg!(windows) {
        return;
    }
    assert_eq!(h, R9, "R9 {h:016x} differs from the base record {R9:016x}");
}

// ---- After the code: CT1 to CT8 and the rest of D2, D3 (ADR 0208) ----

use rcfd_api::case::Diagnostic;
use rcfd_api::{particle_run, particles};
use rcfd_particles::{
    drw_msd, taylor_msd, Dispersion, DragLaw, Flow, InjKind, InjectorSpec, Interp, Options, Scheme, SizeDist, State,
    Tracker, VelMode, WallAction,
};
use rcfd_turb::sst::BETA_STAR;

const X0: [f64; 3] = [200.0, 200.0, 200.0];
/// Viscosity of the carrier, `rho nu`, as `flow_data` forms it.
const MU: f64 = 1.2 * 1.5e-5;
const TIMES: [f64; 4] = [0.25, 1.0, 4.0, 10.0];
const D_TRACER: f64 = 1.8e-5;
const D_INERTIAL: f64 = 5.692099788303082e-4;

/// Case T of ADR 0208: TC with the random walk on the k-epsilon model's fields.
fn t_text() -> String {
    with_table(
        &TC_CASE
            .replace("title = \"dispersion TC\"", "title = \"dispersion T\"")
            .replace("dispersion_k = 1.5\ndispersion_epsilon = 0.225\n", ""),
        FROM_MODEL,
    )
}

/// The table of ADR 0208 amendment 2 (in place of the registered `dispersion_fields = "turbulence_model"`).
const FROM_MODEL: &str = "[particles.dispersion_from_model]\nenabled = true\n";

/// The table of amendment 2 written with its default.
const FROM_MODEL_OFF: &str = "[particles.dispersion_from_model]\nenabled = false\n";

/// `text` with `table` before the injection table.
fn with_table(text: &str, table: &str) -> String {
    assert!(
        text.contains("[particles.injection.c]\n"),
        "set-up: the injection table"
    );
    text.replace(
        "[particles.injection.c]\n",
        &format!("{table}[particles.injection.c]\n"),
    )
}

const KE_INIT: &str = "model = \"k_epsilon\"\nk_initial = 1.5\nepsilon_initial = 0.225";

/// Case TS: T with k-omega SST (epsilon = 0.09 1.5 1.6666666666666667 = 0.225 to round-off).
fn ts_text() -> String {
    t_text()
        .replace(
            KE_INIT,
            "model = \"k_omega_sst\"\nk_initial = 1.5\nomega_initial = 1.6666666666666667",
        )
        .replace("dispersion_seed = 20261107", "dispersion_seed = 20261108")
}

/// Case TI: T with inertial parcels (tau = 1 s) and no crossing times.
fn ti_text() -> String {
    t_text()
        .replace("diameter = 1.8e-5", "diameter = 5.692099788303082e-4")
        .replace(
            "dispersion = \"drw\"\n",
            "dispersion = \"drw\"\ndispersion_crossing = false\n",
        )
        .replace("dispersion_seed = 20261107", "dispersion_seed = 20261109")
}

/// Case T6: T with the model at k 6, epsilon 3.6 (sigma^2 4, T_L 0.25).
fn t6_text() -> String {
    t_text()
        .replace(KE_INIT, "model = \"k_epsilon\"\nk_initial = 6.0\nepsilon_initial = 3.6")
        .replace("dispersion_seed = 20261107", "dispersion_seed = 20261110")
}

/// Case TC6: TC (the constants 1.5 and 0.225) with the model at 6 and 3.6.
fn tc6_text() -> String {
    TC_CASE.replace(KE_INIT, "model = \"k_epsilon\"\nk_initial = 6.0\nepsilon_initial = 3.6")
}

/// Case TW: T with 2000 parcels, cell-centre interpolation, tracked to 2 s.
fn tw_text() -> String {
    t_text()
        .replace("rate = 1.0e8", "rate = 1.0e7")
        .replace(
            "integration = \"analytic\"\n",
            "integration = \"analytic\"\ninterpolation = \"cell_centre\"\n",
        )
        .replace("end_time = 10.0", "end_time = 2.0")
}

/// Case P: TS on pimple (dt 0.05 to 0.5 s) with 2000 parcels tracked to 0.5 s.
fn p_text() -> String {
    ts_text()
        .replace(
            "algorithm = \"simple\"\nmax_iterations = 2\ndt = 0.01",
            "algorithm = \"pimple\"\ndt = 0.05\nend_time = 0.5",
        )
        .replace("rate = 1.0e8", "rate = 1.0e7")
        .replace("end_time = 10.0", "end_time = 0.5")
}

/// Case P2: P with two-way coupling.
fn p2_text() -> String {
    p_text().replace(
        "[particles]\nenabled = true\n",
        "[particles]\nenabled = true\ntwo_way = true\n",
    )
}

fn with_end(t: &str, end: f64) -> String {
    assert!(
        t.contains("end_time = 10.0"),
        "set-up: the text has the particle end time 10"
    );
    t.replace("end_time = 10.0", &format!("end_time = {end:?}"))
}

fn errors(text: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => c.semantic_errors(),
    }
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn tau_of(d: f64) -> f64 {
    1000.0 * d * d / (18.0 * MU)
}

/// `V` (mean-square displacement per component from `X0`) and `C` (`f` at each parcel's age, averaged) of a cloud; every
/// parcel active; `N`.
fn cloud(tr: &Tracker, f: &dyn Fn(f64) -> f64) -> (f64, f64, bool, usize) {
    let ps = &tr.particles;
    let n = ps.len() as f64;
    let v = ps
        .iter()
        .map(|p| (0..3).map(|c| (p.x[c] - X0[c]).powi(2)).sum::<f64>())
        .sum::<f64>()
        / (3.0 * n);
    let c = ps.iter().map(|p| f(p.age)).sum::<f64>() / n;
    (v, c, ps.iter().all(|p| p.state == State::Active), ps.len())
}

/// `sigma^2` and `T_L` of the tracker's fields in the cell of `X0`.
fn walk_scales(tr: &Tracker) -> (f64, f64) {
    let c = tr.locator().locate(X0, None).unwrap();
    let (k, e) = tr.dispersion_fields().unwrap();
    (2.0 * k[c] / 3.0, 0.15 * k[c] / e[c])
}

/// `track_frozen` on the simulation of `text` before any sweep.
fn frozen(text: &str) -> Tracker {
    let case = parse(text);
    assert!(case.semantic_errors().is_empty(), "{:?}", case.semantic_errors());
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    particles::track_frozen(&case, &sim).unwrap_or_else(|e| panic!("{e:?}"))
}

fn min_max(v: &[f64]) -> (f64, f64) {
    (
        v.iter().cloned().fold(f64::INFINITY, f64::min),
        v.iter().cloned().fold(0.0, f64::max),
    )
}

/// CT1: located errors and controls.
#[test]
fn ct1_located_errors() {
    let t = t_text();
    let key = "particles.dispersion_from_model.enabled";
    let seed = "dispersion_seed = 20261107\n";
    let cases: Vec<(String, &str)> = vec![
        (t.replace("dispersion = \"drw\"", "dispersion = \"none\""), key),
        (t.replace(KE_INIT, "model = \"laminar\""), key),
        (t.replace(KE_INIT, "model = \"spalart_allmaras\""), key),
        (
            t.replace(seed, &format!("{seed}dispersion_k = 1.5\n")),
            "particles.dispersion_k",
        ),
        (
            t.replace(seed, &format!("{seed}dispersion_epsilon = 0.225\n")),
            "particles.dispersion_epsilon",
        ),
        (
            t.replace(FROM_MODEL, "[particles.dispersion_from_model]\nenabled = \"yes\"\n"),
            key,
        ),
    ];
    for (i, (text, path)) in cases.iter().enumerate() {
        let errs = errors(text);
        println!("CT1 input {}: {errs:?}", i + 1);
        assert!(
            errs.iter().any(|d| d.path == *path),
            "CT1 input {}: no error at {path}: {errs:?}",
            i + 1
        );
    }
    let controls = [
        ("T", t.clone()),
        ("TS", ts_text()),
        ("TC", TC_CASE.to_string()),
        ("TC with the table off", with_table(TC_CASE, FROM_MODEL_OFF)),
        (
            "T disabled",
            t.replace("[particles]\nenabled = true", "[particles]\nenabled = false"),
        ),
    ];
    for (what, text) in controls {
        let errs = errors(&text);
        assert!(errs.is_empty(), "CT1 control {what}: {errs:?}");
    }
}

/// CT2: the tracker's fields are the model's, before and after steady sweeps; SST through beta* k omega.
#[test]
fn ct2_fields_are_the_models() {
    let case = parse(&t_text());
    let mut sim = Simulation::new(&case).unwrap();
    for sweeps in [0, 2] {
        for _ in 0..sweeps {
            sim.step();
        }
        let tr = particles::build_tracker(&case, &sim).unwrap();
        let (k, e) = tr.dispersion_fields().unwrap();
        let (mk, me) = (sim.field("k").unwrap(), sim.field("epsilon").unwrap());
        assert_eq!(bits(k), bits(&mk), "CT2 k after {sweeps} sweeps");
        assert_eq!(bits(e), bits(&me), "CT2 epsilon after {sweeps} sweeps");
        let (lo, hi) = min_max(k);
        println!(
            "CT2 k-epsilon after {sweeps} sweeps: k in [{lo:e}, {hi:e}], ratio {:.4}",
            hi / lo
        );
        if sweeps == 2 {
            assert!(
                hi / lo >= 1.05,
                "CT2 (b) set-up: the fields are not uniform after two sweeps"
            );
        }
    }
    let case = parse(&ts_text());
    let mut sim = Simulation::new(&case).unwrap();
    for sweeps in [0, 2] {
        for _ in 0..sweeps {
            sim.step();
        }
        let tr = particles::build_tracker(&case, &sim).unwrap();
        let (mk, mw) = (sim.field("k").unwrap(), sim.field("omega").unwrap());
        assert_sst_fields(&tr, &mk, &mw, &format!("CT2 (c) after {sweeps} sweeps"));
        println!(
            "CT2 SST after {sweeps} sweeps: epsilon[0] {:e}",
            tr.dispersion_fields().unwrap().1[0]
        );
    }
}

/// CT3 (acceptance): a tracer cloud on the k-epsilon model's homogeneous fields against D3 and against Taylor.
#[test]
fn ct3_tracer_cloud_on_k_epsilon_fields_follows_taylor() {
    let tau = tau_of(D_TRACER);
    for t in TIMES {
        let tr = frozen(&with_end(&t_text(), t));
        let (s2, t_l) = walk_scales(&tr);
        let (v, c, active, n) = cloud(&tr, &|a| drw_msd(s2, t_l, tau, a));
        let (_, ct, _, _) = cloud(&tr, &|a| taylor_msd(s2, t_l, a));
        let (e, et) = (rel(v, c), rel(v, ct));
        println!("CT3 t {t}: V {v:.6e} C {c:.6e} rel {e:.4}; Taylor {ct:.6e} rel {et:.4}; N {n}, all active {active} (tau {tau:e}, sigma2 {s2}, T_L {t_l})");
        assert_eq!(n, 20_000, "CT3 N");
        assert!(active, "CT3 all active");
        assert!(e <= 0.05 && et <= 0.05, "CT3 t {t}: {e} {et}");
    }
}

/// CT3b and CT4b (ADR 0208 amendment 3): the clouds of CT3 and CT4 against the closed form of the case's inputs
/// (`sigma2`, `t_l`), not of the tracker's fields.
fn against_inputs(id: &str, text: &str, sigma2: f64, t_l: f64) {
    let tau = tau_of(D_TRACER);
    for t in TIMES {
        let tr = frozen(&with_end(text, t));
        let (v, c, active, n) = cloud(&tr, &|a| drw_msd(sigma2, t_l, tau, a));
        let (_, ct, _, _) = cloud(&tr, &|a| taylor_msd(sigma2, t_l, a));
        let (e, et) = (rel(v, c), rel(v, ct));
        println!("{id} t {t}: V {v:.6e} C {c:.6e} rel {e:.4}; Taylor {ct:.6e} rel {et:.4}; N {n}, all active {active} (inputs: sigma2 {sigma2}, T_L {t_l})");
        assert!(active && n == 20_000, "{id} set-up");
        assert!(e <= 0.05 && et <= 0.05, "{id} t {t}: {e} {et}");
    }
}

/// CT3b: case T against `sigma^2 = 2 k/3`, `T_L = 0.15 k/epsilon` of `k_initial = 1.5`, `epsilon_initial = 0.225`.
#[test]
fn ct3b_tracer_cloud_against_the_case_inputs() {
    let (k, eps) = (1.5, 0.225);
    against_inputs("CT3b", &t_text(), 2.0 * k / 3.0, 0.15 * k / eps);
}

/// CT4b: case TS against the same `sigma^2` and `T_L = 0.15 k/(0.09 k omega)` of `k_initial = 1.5`,
/// `omega_initial = 1.6666666666666667` (`beta*` written as a number, not read from the code).
#[test]
fn ct4b_tracer_cloud_against_the_case_inputs() {
    let (k, omega) = (1.5, 1.6666666666666667);
    against_inputs("CT4b", &ts_text(), 2.0 * k / 3.0, 0.15 * k / (0.09 * k * omega));
}

/// CT4: the same on the SST model's fields. ADR 0208 amendment 3: the record of a check whose closed form follows the
/// tracker's own fields, so MU2 (a wrong SST conversion) does not make it fail; the corrected test is
/// `ct4b_tracer_cloud_against_the_case_inputs`.
#[test]
#[ignore = "ADR 0208 amendment 3: set-up defect (closed form from the tracker's fields, blind to MU2); met in its run (0.07 to 0.79 percent); corrected test ct4b_tracer_cloud_against_the_case_inputs"]
fn ct4_tracer_cloud_on_sst_fields_follows_taylor_setup_defect() {
    let tau = tau_of(D_TRACER);
    for t in TIMES {
        let tr = frozen(&with_end(&ts_text(), t));
        let (s2, t_l) = walk_scales(&tr);
        let (v, c, active, n) = cloud(&tr, &|a| drw_msd(s2, t_l, tau, a));
        let (_, ct, _, _) = cloud(&tr, &|a| taylor_msd(s2, t_l, a));
        let (e, et) = (rel(v, c), rel(v, ct));
        println!("CT4 t {t}: V {v:.6e} C {c:.6e} rel {e:.4}; Taylor {ct:.6e} rel {et:.4}; N {n}, all active {active} (sigma2 {s2}, T_L {t_l})");
        assert!(active && n == 20_000, "CT4 set-up");
        assert!(e <= 0.05 && et <= 0.05, "CT4 t {t}: {e} {et}");
    }
}

/// CT5 and CTN1: an inertial cloud (tau = T_L, no crossing) against D3; Taylor's tracer form misses it.
#[test]
fn ct5_ctn1_inertial_cloud() {
    let tau = tau_of(D_INERTIAL);
    for t in TIMES {
        let tr = frozen(&with_end(&ti_text(), t));
        let (s2, t_l) = walk_scales(&tr);
        let (v, c, active, n) = cloud(&tr, &|a| drw_msd(s2, t_l, tau, a));
        let e = rel(v, c);
        println!("CT5 t {t}: V {v:.6e} C {c:.6e} rel {e:.4}; N {n}, all active {active} (tau {tau})");
        assert!(active && n == 20_000, "CT5 set-up");
        assert!(e <= 0.05, "CT5 t {t}: {e}");
        if t == 1.0 {
            let (_, ct, _, _) = cloud(&tr, &|a| taylor_msd(s2, t_l, a));
            let et = rel(v, ct);
            println!("CTN1 t 1: V against Taylor's tracer form {ct:.6e}: rel {et:.4}");
            assert!(et >= 0.5, "CTN1: {et}");
        }
    }
}

/// CTN2: the walk follows the model's fields (k 6, epsilon 3.6), not those of T.
#[test]
fn ctn2_cloud_follows_other_model_fields() {
    let tau = tau_of(D_TRACER);
    let tr = frozen(&with_end(&t6_text(), 1.0));
    let (s2, t_l) = walk_scales(&tr);
    let (v, c, active, _) = cloud(&tr, &|a| drw_msd(s2, t_l, tau, a));
    let (_, c_t, _, _) = cloud(&tr, &|a| drw_msd(1.0, 1.0, tau, a));
    let (e, e_t) = (rel(v, c), rel(v, c_t));
    println!("CTN2 t 1: sigma2 {s2} T_L {t_l}; V {v:.6e} against its own C {c:.6e} rel {e:.4}; against T's {c_t:.6e} rel {e_t:.4}");
    assert!(active, "CTN2 set-up");
    assert!(e <= 0.05, "CTN2 own fields: {e}");
    assert!(e_t >= 0.5, "CTN2 fields of T: {e_t}");
}

/// CTN3: without the table of amendment 2 the walk follows the constants whatever the model holds.
#[test]
fn ctn3_constants_ignore_the_model() {
    let tau = tau_of(D_TRACER);
    let tr = frozen(&with_end(&tc6_text(), 1.0));
    let (v, c, active, _) = cloud(&tr, &|a| drw_msd(1.0, 0.15 * 1.5 / 0.225, tau, a));
    let e = rel(v, c);
    println!("CTN3 t 1: V {v:.6e} against the constants' C {c:.6e} rel {e:.4}");
    assert!(active, "CTN3 set-up");
    assert!(e <= 0.05, "CTN3: {e}");
}

/// CT6: tree equals library on non-uniform fields (after two steady sweeps): the run path, `track_frozen`, and a tracker
/// built by hand.
#[test]
fn ct6_tree_equals_library_on_nonuniform_fields() {
    let case = parse(&tw_text());
    let sim = Simulation::new(&case).unwrap();
    let m = Mutex::new(sim);
    let out = runctl::drive(&m, &RunOptions::default(), &|| false, &mut |_| {});
    assert!(out.exit_code == 0 || out.exit_code == 5, "{out:?}");
    let sim = m.into_inner().unwrap();
    let run = particle_run::steady_run(&sim, None).unwrap_or_else(|e| panic!("{e:?}"));
    let fr = particles::track_frozen(&case, &sim).unwrap();
    let (k, e) = (sim.field("k").unwrap(), sim.field("epsilon").unwrap());
    let (lo, hi) = min_max(&k);
    println!("CT6 after the run: k in [{lo:e}, {hi:e}]");
    assert!(hi / lo >= 1.05, "CT6 set-up: non-uniform fields");
    let opts = Options {
        drag: DragLaw::Stokes,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0; 3],
        default_wall: WallAction::Reflect(1.0),
        ..Options::default()
    };
    let mut tr = Tracker::new(sim.geom.mesh.clone(), opts, &[]).unwrap();
    tr.add_injector(InjectorSpec {
        name: "c".into(),
        kind: InjKind::Point(X0),
        rate: 1e7,
        t_start: 0.0,
        t_end: 2e-4,
        velocity: VelMode::Given([0.0; 3]),
        dist: SizeDist::Constant(D_TRACER),
        density: 1000.0,
        parcel_count: 1.0,
        seed: 1u64.wrapping_mul(1_000_003),
    })
    .unwrap();
    let model = Dispersion {
        c_l: 0.15,
        crossing: true,
    };
    tr.set_dispersion(model, k, e, 20261107u64.wrapping_add(0x5EED_D15B))
        .unwrap();
    let flow = Flow {
        u: &sim.velocity().values,
        grad: None,
        rho: 1.2,
        mu: MU,
    };
    while tr.time < 2.0 * (1.0 - 1e-12) {
        let h = 0.05f64.min(2.0 - tr.time);
        tr.advance(&flow, h).unwrap();
    }
    let f = tr.fingerprint();
    println!("CT6 fingerprint words {}, parcels {}", f.len(), tr.particles.len());
    assert_eq!(
        run.tracker.fingerprint(),
        f,
        "CT6 run path against the hand-built tracker"
    );
    assert_eq!(fr.fingerprint(), f, "CT6 track_frozen against the hand-built tracker");
}

/// The tracker's fields against `(k, BETA_STAR k omega)`.
fn assert_sst_fields(tr: &Tracker, k: &[f64], w: &[f64], what: &str) {
    let me: Vec<f64> = k.iter().zip(w).map(|(k, w)| BETA_STAR * k * w).collect();
    let (tk, te) = tr.dispersion_fields().unwrap();
    assert_eq!(bits(tk), bits(k), "{what}: k");
    assert_eq!(bits(te), bits(&me), "{what}: epsilon");
}

/// CT7: the transient refresh, in the run path (`Simulation::step`) and in `track_transient`.
#[test]
fn ct7_transient_refresh() {
    let case = parse(&p_text());
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let mut k_prev = sim.field("k").unwrap();
    for step in 1..=10 {
        sim.step();
        let (k, w) = (sim.field("k").unwrap(), sim.field("omega").unwrap());
        let pr = sim.particle_run().expect("the transient run path's particles");
        assert_sst_fields(&pr.tracker, &k, &w, &format!("CT7 (a) step {step}"));
        let changed = k.iter().zip(&k_prev).filter(|(a, b)| a != b).count();
        println!(
            "CT7 (a) step {step}: k[0] {:e}, cells changed {changed}, parcels {}",
            k[0],
            pr.tracker.particles.len()
        );
        assert!(changed > 0, "CT7 (a) step {step}: the fields did not change");
        k_prev = k;
    }
    let mut sim2 = Simulation::new(&case).unwrap();
    let tr = particles::track_transient(&case, &mut sim2).unwrap_or_else(|e| panic!("{e:?}"));
    assert_sst_fields(&tr, &sim2.field("k").unwrap(), &sim2.field("omega").unwrap(), "CT7 (b)");
    let pr = sim.particle_run().unwrap();
    assert_eq!(
        tr.fingerprint(),
        pr.tracker.fingerprint(),
        "CT7 (b) track_transient against the run path"
    );
    println!("CT7 (b) equal over {} words at t {}", tr.fingerprint().len(), tr.time);
}

/// CT8: the two-way refresh gives the walk the fields of the old time level.
#[test]
fn ct8_two_way_refresh_uses_the_old_level() {
    let case = parse(&p2_text());
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    for step in 1..=10 {
        let (k, w) = (sim.field("k").unwrap(), sim.field("omega").unwrap());
        sim.step();
        let pr = sim.particle_run().expect("the transient run path's particles");
        assert!(pr.is_two_way(), "CT8 set-up");
        assert_sst_fields(&pr.tracker, &k, &w, &format!("CT8 step {step}"));
        let k_new = sim.field("k").unwrap();
        println!("CT8 step {step}: old k[0] {:e}, new k[0] {:e}", k[0], k_new[0]);
    }
}

/// D2 after the code: TC and TC with the table of amendment 2 written off hash as R8.
#[test]
fn d2_written_default_keeps_the_hash() {
    let explicit = with_table(TC_CASE, FROM_MODEL_OFF);
    let (a, b) = (parse(TC_CASE).tree.content_hash(), parse(&explicit).tree.content_hash());
    println!("D2 TC {a:016x}, TC with the table off {b:016x}");
    assert_eq!(a, R8, "D2 TC");
    assert_eq!(b, R8, "D2 TC with the default written");
}

/// D3 after the code: TCR with the table of amendment 2 written off writes the files of R9.
#[test]
fn d3_written_default_keeps_the_outputs() {
    let dir = tmp("d3c");
    let text = with_table(&tcr_text(), FROM_MODEL_OFF);
    run_path(&text, &dir);
    let (h, names) = particle_files_hash(&dir);
    println!("D3 TCR with the table off {h:016x} over {names:?}");
    let _ = std::fs::remove_dir_all(&dir);
    if !cfg!(windows) {
        return;
    }
    assert_eq!(h, R9, "D3 TCR with the default written");
}
