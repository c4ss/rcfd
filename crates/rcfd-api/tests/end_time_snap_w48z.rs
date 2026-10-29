//! ADR 0160 (job 48Z) item 1: a fixed-step run whose summed steps end within the end tolerance (1e-12) of
//! `solver.end_time` reports `solver.end_time` as its final time, while every field, flux, statistic, particle and continued
//! run stays bit-identical (only the time stamp changes). Checks S1-S6 as registered in ADR 0160 before any run; none was
//! changed afterwards. This file uses only the API of the unchanged tree, so that its run there (R1) prints the values that
//! `PRE_FIX` records (R2). Every case text is a `format!` template (ADR 0156 B2a's literal scan skips it).

use rcfd_api::case::Case;
use rcfd_api::particles;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_tsstore::Reader;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;

// ------------------------------------------------------------------------------------------------ set-ups

/// The fixed-step paths of ADR 0145 (the paths that call `fixed_step`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Piso,
    PisoCourant,
    Frozen,
    VarDensity,
    Vof,
}

const KINDS: [Kind; 5] = [Kind::Piso, Kind::PisoCourant, Kind::Frozen, Kind::VarDensity, Kind::Vof];

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Piso => "piso",
            Kind::PisoCourant => "piso_courant",
            Kind::Frozen => "frozen",
            Kind::VarDensity => "variable_density",
            Kind::Vof => "vof",
        }
    }

    /// ADR 0145's step of the path's set-up.
    fn dt(self) -> f64 {
        match self {
            Kind::Piso | Kind::PisoCourant => 0.01,
            Kind::Frozen => 0.0025,
            Kind::VarDensity => 0.05,
            Kind::Vof => 1e-3,
        }
    }
}

/// Whole steps before the end time (variant S: `end = t_3 + 1e-13`).
const N: usize = 3;

/// The offset of variant S: below the end tolerance 1e-12, so the run ends after `N` steps short of the end time.
const SHORT: f64 = 1e-13;

/// ADR 0145's baseline cavity as a PISO run (with the legacy Courant controller when `courant`), plus `extra` sections.
fn piso_text(courant: bool, dt: f64, end: f64, extra: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 20\nny = 20\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\n\
         velocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"piso\"\n\
         convection = \"linear_upwind\"\ndt = {dt:.17e}\nend_time = {end:.17e}\n{}{extra}",
        if courant { "max_courant = 1e6\n" } else { "" }
    )
}

/// ADR 0145's prescribed-flow set-up (the ADR 0024 contact case of `cht_case.rs`, 32 cells).
fn frozen_text(dt: f64, end: f64) -> String {
    format!(
        r#"
title = "contact"
[mesh]
generator = "box_2d"
nx = 32
ny = 1
lx = 16.0
ly = 1.0
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = 2.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 400.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "piso"
convection = "upwind"
time_scheme = "bdf2"
frozen_flow = true
dt = {dt:.17e}
end_time = {end:.17e}
[solids.right]
zone = "box"
box_min = [8.0, -10.0, -10.0]
box_max = [20.0, 10.0, 10.0]
conductivity = 0.5
density = 2.0
specific_heat = 1.0
initial_temperature = 300.0
"#
    )
}

/// ADR 0145's variable-density set-up (the ADR 0009 closed-box heating case, `low_mach`).
fn vd_text(dt: f64, end: f64) -> String {
    const R: f64 = 287.0;
    const GAMMA: f64 = 1.4;
    const P0: f64 = 101_325.0;
    let cv = R / (GAMMA - 1.0);
    let rho0 = P0 / (R * 300.0);
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 6
ny = 6
lx = 0.1
ly = 0.1
[fluid]
density = "ideal_gas"
gas_constant = {R}
gamma = {GAMMA}
mode = "low_mach"
reference_pressure = {P0}
inner_iterations = 60
inner_tolerance = 1e-12
[energy]
enabled = true
relax = 1.0
heat_source = {q}
[initial]
temperature = 300.0
pressure = {P0}
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "pimple"
time_scheme = "bdf2"
dt = {dt:.17e}
end_time = {end:.17e}
alpha_u = 0.9
alpha_p = 1.0
momentum_rel_tol = 1e-10
momentum_max_iter = 2000
pressure_rel_tol = 1e-10
pressure_max_iter = 20000
"#,
        q = rho0 * cv * 100.0
    )
}

/// ADR 0145's VOF set-up (the ADR 0049 still pool of `vof_tree.rs`).
fn vof_text(dt: f64, end: f64) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 10
ny = 20
lx = 0.5
ly = 1.0
[physics]
gravity = [0.0, -9.81, 0.0]
[multiphase]
enabled = true
density_1 = 1000.0
density_2 = 1.0
viscosity_1 = 1.0e-3
viscosity_2 = 1.8e-5
[multiphase.initial]
shape = "half_space"
normal = [0.0, 1.0, 0.0]
point = [0.0, 0.5, 0.0]
[solver]
algorithm = "piso"
convection = "upwind"
n_correctors = 3
dt = {dt:.17e}
end_time = {end:.17e}
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
"#
    )
}

/// The settling box of `particles_case.rs` PT14 with the flow's end time `end` at step `dt`; the particles' own end time 1.0.
fn settling_text(dt: f64, end: f64) -> String {
    format!(
        r#"
title = "settling (ADR 0160 S4)"
[mesh]
generator = "box_2d"
nx = 6
ny = 12
lx = 1.0
ly = 2.0
[physics]
nu = 1.5e-5
rho = 1.2
gravity = [0.0, -9.81, 0.0]
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "piso"
dt = {dt:.17e}
end_time = {end:.17e}
[particles]
enabled = true
density = 2500.0
drag = "stokes"
integration = "analytic"
interpolation = "cell_centre"
end_time = 1.0
[particles.injection.p]
kind = "point"
position = [0.5, 1.9, 0.05]
rate = 1000.0
end_time = 0.001
diameter = 1e-4
"#
    )
}

fn case_text(kind: Kind, end: f64) -> String {
    let dt = kind.dt();
    match kind {
        Kind::Piso => piso_text(false, dt, end, ""),
        Kind::PisoCourant => piso_text(true, dt, end, ""),
        Kind::Frozen => frozen_text(dt, end),
        Kind::VarDensity => vd_text(dt, end),
        Kind::Vof => vof_text(dt, end),
    }
}

/// The f64 partial sums `t_1 .. t_n` of `n` steps of `dt`, added as the solvers add them.
fn sums(dt: f64, n: usize) -> Vec<f64> {
    let mut t = 0.0;
    (0..n)
        .map(|_| {
            t += dt;
            t
        })
        .collect()
}

/// Variant S's end time: `t_N + 1e-13`.
fn end_s(dt: f64) -> f64 {
    sums(dt, N)[N - 1] + SHORT
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn parse(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

/// A run stepped to its end: the simulation and the reported time after every step.
fn run_to_end(text: &str, cap: usize) -> (Simulation, Vec<f64>) {
    let mut sim = Simulation::new(&parse(text)).unwrap_or_else(|e| panic!("{e:?}"));
    let mut times = Vec::new();
    while !sim.is_done() {
        times.push(sim.step().time);
        assert!(times.len() <= cap, "the run does not end within {cap} steps");
    }
    (sim, times)
}

/// FNV-1a over the little-endian bytes of the bit pattern of every value (as `baseline_hashes.rs`).
fn fnv1a(values: &[f64], acc: &mut u64) {
    for x in values {
        for b in x.to_bits().to_le_bytes() {
            *acc ^= u64::from(b);
            *acc = acc.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

const FNV0: u64 = 0xcbf2_9ce4_8422_2325;

/// Hash of the final state: the fields `ux uy p T alpha` that the run has, then the face flux (ADR 0145 B2's hash).
fn state_hash(sim: &Simulation) -> u64 {
    let mut acc = FNV0;
    for f in ["ux", "uy", "p", "T", "alpha"] {
        if let Ok(v) = sim.field(f) {
            fnv1a(&v, &mut acc);
        }
    }
    fnv1a(sim.flux(), &mut acc);
    acc
}

/// Hashes of the monitor store: every `y`, and every `x` but each series' last; and each series' last `x`.
fn monitor_hashes(sim: &Simulation) -> (u64, u64, Vec<(String, f64)>) {
    let (mut hy, mut hx) = (FNV0, FNV0);
    let mut last = Vec::new();
    for n in sim.monitors.names() {
        let s = sim.monitors.get(n).unwrap();
        fnv1a(&s.y, &mut hy);
        if let Some((l, rest)) = s.x.split_last() {
            fnv1a(rest, &mut hx);
            last.push((n.to_string(), *l));
        }
    }
    (hy, hx, last)
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-w48z-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// (name, value) recorded on the unchanged tree (registration commit) by run R1 on Windows x86_64 and written here before any
/// code of this job (R2). Other platforms print (bit-identity is per platform, ADR 0137).
const PRE_FIX: &[(&str, u64)] = &[];

/// Compare recorded values: on the registered platform each must equal `PRE_FIX`; elsewhere they are printed only.
fn against_pre_fix(got: &[(String, u64)], problems: &mut Vec<String>) {
    for (name, v) in got {
        eprintln!("    (\"{name}\", 0x{v:016x}),");
    }
    if !cfg!(all(windows, target_arch = "x86_64")) {
        eprintln!("no recorded values for this platform (ADR 0137); printed above");
        return;
    }
    for (name, v) in got {
        match PRE_FIX.iter().find(|p| p.0 == name) {
            None => problems.push(format!("{name}: no pre-fix value recorded")),
            Some(&(_, w)) if w != *v => problems.push(format!("{name}: {v:016x} differs from the pre-fix {w:016x}")),
            Some(_) => {}
        }
    }
}

// ------------------------------------------------------------------------------------- S1: the five paths

#[test]
fn s1_fixed_step_paths_report_the_end_time_with_unchanged_states() {
    let mut problems = Vec::new();
    let mut got = Vec::new();
    for kind in KINDS {
        let dt = kind.dt();
        let end = end_s(dt);
        let (sim, times) = run_to_end(&case_text(kind, end), 100);
        let s = sums(dt, N);
        let (hy, hx, last) = monitor_hashes(&sim);
        eprintln!(
            "S1 {}: end {end:.17e}, {} steps, times {times:?}, sim.time() {:.17e}; series last x {:?}",
            kind.name(),
            times.len(),
            sim.time(),
            last.iter()
                .map(|(_, x)| *x)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        );
        if times.len() != N {
            problems.push(format!("S1 {}: {} steps (registered {N})", kind.name(), times.len()));
        } else {
            if bits(&times[..N - 1]) != bits(&s[..N - 1]) {
                problems.push(format!("S1 {}: times {times:?}, partial sums {s:?}", kind.name()));
            }
            if times[N - 1].to_bits() != end.to_bits() {
                problems.push(format!(
                    "S1 {}: final reported time {:.17e}, not the end time {end:.17e}",
                    kind.name(),
                    times[N - 1]
                ));
            }
        }
        if sim.time().to_bits() != end.to_bits() {
            problems.push(format!(
                "S1 {}: sim.time() {:.17e}, not {end:.17e}",
                kind.name(),
                sim.time()
            ));
        }
        let off: Vec<&(String, f64)> = last.iter().filter(|(_, x)| x.to_bits() != end.to_bits()).collect();
        if !off.is_empty() {
            problems.push(format!(
                "S1 {}: series whose last x is not the end time: {off:?}",
                kind.name()
            ));
        }
        got.push((format!("s1-{}-state", kind.name()), state_hash(&sim)));
        got.push((format!("s1-{}-monitor-y", kind.name()), hy));
        got.push((format!("s1-{}-monitor-x", kind.name()), hx));
    }
    against_pre_fix(&got, &mut problems);
    assert!(problems.is_empty(), "S1: {problems:#?}");
}

// ----------------------------------------------------------------------- S2: job 48S's arithmetic on the cavity

#[test]
fn s2_dt_3_over_2567_to_3_reports_3() {
    let dt = 3.0 / 2567.0;
    let (sim, times) = run_to_end(&piso_text(false, dt, 3.0, ""), 3000);
    let t = *times.last().unwrap();
    eprintln!(
        "S2: {} steps, final reported time {t:.17e}, sim.time() {:.17e}; f64 sum of 2567 steps {:.17e}",
        times.len(),
        sim.time(),
        sums(dt, 2567)[2566]
    );
    let mut problems = Vec::new();
    if times.len() != 2567 {
        problems.push(format!("{} steps, not 2567", times.len()));
    }
    if t.to_bits() != 3.0_f64.to_bits() || sim.time().to_bits() != 3.0_f64.to_bits() {
        problems.push(format!("final time {t:.17e} / {:.17e}, not 3", sim.time()));
    }
    against_pre_fix(&[("s2-state".into(), state_hash(&sim))], &mut problems);
    assert!(problems.is_empty(), "S2: {problems:#?}");
}

// ------------------------------------------------------------------------------------------ S3: statistics

#[test]
fn s3_statistics_are_unchanged() {
    let dt = Kind::Piso.dt();
    let end = end_s(dt);
    let stats = "[statistics]\nenabled = true\nstart_time = 0.0\nfields = [\"ux\", \"p\"]\n";
    let (sim, times) = run_to_end(&piso_text(false, dt, end, stats), 100);
    let mut acc = FNV0;
    let mut names = Vec::new();
    for f in ["ux", "p"] {
        for k in ["mean", "rms", "std", "min", "max"] {
            let n = format!("{f}_{k}");
            if let Ok(v) = sim.field(&n) {
                fnv1a(&v, &mut acc);
                names.push(n);
            }
        }
    }
    eprintln!(
        "S3: {} steps, final time {:.17e}, statistics fields {names:?}",
        times.len(),
        sim.time()
    );
    let mut problems = Vec::new();
    if names.len() < 4 {
        problems.push(format!("statistics fields {names:?}: the set-up gives fewer than four"));
    }
    if sim.time().to_bits() != end.to_bits() {
        problems.push(format!("final time {:.17e}, not {end:.17e}", sim.time()));
    }
    against_pre_fix(
        &[("s3-statistics".into(), acc), ("s3-state".into(), state_hash(&sim))],
        &mut problems,
    );
    assert!(problems.is_empty(), "S3: {problems:#?}");
}

// --------------------------------------------------------------------------------------------- S4: particles

#[test]
fn s4_transient_particle_tracking_is_unchanged() {
    let dt = 0.01;
    let end = end_s(dt);
    let c = parse(&settling_text(dt, end));
    let mut sim = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    let tr = particles::track_transient(&c, &mut sim).unwrap_or_else(|e| panic!("{e:?}"));
    let mut acc = FNV0;
    for p in &tr.particles {
        fnv1a(&p.x, &mut acc);
        fnv1a(&p.v, &mut acc);
    }
    fnv1a(&[tr.time], &mut acc);
    eprintln!(
        "S4: {} flow steps, flow time {:.17e}, {} parcels, tracker time {:.17e}",
        sim.iteration(),
        sim.time(),
        tr.particles.len(),
        tr.time
    );
    let mut problems = Vec::new();
    if sim.iteration() != N || tr.particles.is_empty() {
        problems.push(format!(
            "{} flow steps (registered {N}), {} parcels",
            sim.iteration(),
            tr.particles.len()
        ));
    }
    against_pre_fix(&[("s4-particles".into(), acc)], &mut problems);
    assert!(problems.is_empty(), "S4: {problems:#?}");
}

// ------------------------------------------------------------------------------------------ S5: continuation

fn drive(sim: Simulation, opts: &RunOptions) -> (Simulation, runctl::RunOutcome, Vec<Value>) {
    let m = Mutex::new(sim);
    let mut events = Vec::new();
    let o = runctl::drive(&m, opts, &|| false, &mut |e| events.push(e.clone()));
    (m.into_inner().unwrap(), o, events)
}

fn fields(s: &Simulation) -> (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>, u64) {
    (
        bits(&s.field("ux").unwrap()),
        bits(&s.field("uy").unwrap()),
        bits(&s.field("p").unwrap()),
        bits(s.flux()),
        s.time().to_bits(),
    )
}

#[test]
fn s5_a_run_continued_from_a_short_end_equals_the_single_run() {
    let dt = Kind::Piso.dt();
    let (end1, end2) = (end_s(dt), sums(dt, 2 * N)[2 * N - 1]);
    let dir = tmp("s5");
    let ck = dir.join("first.rcfd");
    let first = Simulation::new(&parse(&piso_text(false, dt, end1, ""))).unwrap();
    let (first, o1, _) = drive(
        first,
        &RunOptions {
            checkpoint: Some(ck.clone()),
            ..RunOptions::default()
        },
    );
    let mut second = Simulation::new(&parse(&piso_text(false, dt, end2, ""))).unwrap();
    second.load_restart(&ck).unwrap_or_else(|e| panic!("{e:?}"));
    let (chained, o2, _) = drive(second, &RunOptions::default());
    let (single, o3, _) = drive(
        Simulation::new(&parse(&piso_text(false, dt, end2, ""))).unwrap(),
        &RunOptions::default(),
    );
    eprintln!(
        "S5: first {} steps to {:.17e} (exit {}), chained {} steps to {:.17e} (exit {}), single {} steps to {:.17e} (exit {})",
        first.iteration(),
        first.time(),
        o1.exit_code,
        chained.iteration(),
        chained.time(),
        o2.exit_code,
        single.iteration(),
        single.time(),
        o3.exit_code
    );
    let _ = std::fs::remove_dir_all(&dir);
    let (a, b) = (fields(&chained), fields(&single));
    let mut problems = Vec::new();
    for (what, eq) in [
        ("ux", a.0 == b.0),
        ("uy", a.1 == b.1),
        ("p", a.2 == b.2),
        ("flux", a.3 == b.3),
        ("time", a.4 == b.4),
    ] {
        if !eq {
            problems.push(format!("{what} differs between the chained and the single run"));
        }
    }
    if o1.exit_code != 0 || o2.exit_code != 0 || o3.exit_code != 0 {
        problems.push(format!("exit codes {} {} {}", o1.exit_code, o2.exit_code, o3.exit_code));
    }
    against_pre_fix(&[("s5-chained-state".into(), state_hash(&chained))], &mut problems);
    assert!(problems.is_empty(), "S5: {problems:#?}");
}

// --------------------------------------------------------------------------- S6: events, outcome and the store

#[test]
fn s6_events_outcome_and_store_report_the_end_time() {
    let dt = Kind::Piso.dt();
    let end = end_s(dt);
    let out = tmp("s6");
    let store = "[output]\nstore_fields = [\"ux\"]\nstore_interval = 1\n";
    let sim = Simulation::new(&parse(&piso_text(false, dt, end, store))).unwrap();
    let (_sim, o, events) = drive(
        sim,
        &RunOptions {
            out_dir: Some(out.clone()),
            ..RunOptions::default()
        },
    );
    let last_iter = events.iter().rev().find(|e| e["event"] == "iteration").cloned();
    let finish = events.iter().rev().find(|e| e["event"] == "finish").cloned();
    let t_iter = last_iter.as_ref().and_then(|e| e["time"].as_f64()).unwrap_or(f64::NAN);
    let t_fin = finish.as_ref().and_then(|e| e["time"].as_f64()).unwrap_or(f64::NAN);
    let mut r = Reader::open(&out.join("fields")).expect("the run's field store opens");
    let snaps: Vec<(u64, f64)> = r.snapshots().iter().map(|s| (s.step, s.time)).collect();
    let mut acc = FNV0;
    for i in 0..snaps.len() {
        fnv1a(&r.read_field(i, "ux").unwrap(), &mut acc);
    }
    eprintln!(
        "S6: exit {} state {}; last iteration time {t_iter:.17e}, finish time {t_fin:.17e}, outcome time {:.17e}; store {snaps:?}",
        o.exit_code, o.state, o.time
    );
    let _ = std::fs::remove_dir_all(&out);
    let mut problems = Vec::new();
    for (what, t) in [("iteration", t_iter), ("finish", t_fin), ("outcome", o.time)] {
        if t.to_bits() != end.to_bits() {
            problems.push(format!("{what} time {t:.17e}, not {end:.17e}"));
        }
    }
    let store_ok = snaps.len() == N
        && snaps
            .last()
            .is_some_and(|s| s.0 == N as u64 && s.1.to_bits() == end.to_bits());
    if !store_ok {
        problems.push(format!("store snapshots {snaps:?}"));
    }
    against_pre_fix(&[("s6-store-ux".into(), acc)], &mut problems);
    assert!(problems.is_empty(), "S6: {problems:#?}");
}
