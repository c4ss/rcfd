//! ADR 0145 (job 48K): a fixed-step transient run whose summed steps stop short of `solver.end_time` by a rounding
//! remainder ends on a whole step, instead of adding a remainder step of a few picoseconds whose pressure is spoiled
//! (ADR 0142 question 6). Checks C1, L1-L6, W1 and B2 as registered in ADR 0145 before any run; none was changed
//! afterwards. Every test prints what it measured before it asserts, so a run on the unfixed tree still reports.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions, RunOutcome};
use rcfd_api::sim::Simulation;
use rcfd_tsstore::Reader;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;

// ------------------------------------------------------------------------------------------------ set-ups

/// The five fixed-step paths of ADR 0145 (Context 1 and the set-up table).
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

    /// The registered step of the path's set-up.
    fn dt(self) -> f64 {
        match self {
            Kind::Piso | Kind::PisoCourant => 0.01,
            Kind::Frozen => 0.0025,
            Kind::VarDensity => 0.05,
            Kind::Vof => 1e-3,
        }
    }

    /// The paths that solve a pressure raise the D4 warning; prescribed (frozen) flow solves none.
    fn warns(self) -> bool {
        self != Kind::Frozen
    }
}

/// Whole steps before the end time in every landing set-up (registered n = 3).
const N: usize = 3;

/// Registered variants: `end = t_n + f dt` (E exact, T absorbed below 1e-6, J kept above it, N a large remainder).
const VARIANTS: [(&str, f64); 4] = [("E", 0.0), ("T", 0.9e-6), ("J", 1.1e-6), ("N", 0.3)];

/// PISO only (W1, B2): either side of the registered warning fraction 0.05.
const WARN_VARIANTS: [(&str, f64); 2] = [("W-", 0.049), ("W+", 0.051)];

/// The baseline cavity of `baseline_hashes.rs` as a PISO run (with the legacy Courant controller when `courant`).
fn piso_text(courant: bool, dt: f64, end: f64) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 20\nny = 20\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\n\
         velocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"piso\"\n\
         convection = \"linear_upwind\"\ndt = {dt:.17e}\nend_time = {end:.17e}\n{}",
        if courant { "max_courant = 1e6\n" } else { "" }
    )
}

/// The ADR 0024 contact case of `cht_case.rs` (prescribed flow, energy only) with 32 cells.
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

/// The ADR 0009 closed-box heating case of `transient_variable_density.rs` (`low_mach`).
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

/// The ADR 0049 still pool of `vof_tree.rs`.
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

fn case_text(kind: Kind, end: f64) -> String {
    let dt = kind.dt();
    match kind {
        Kind::Piso => piso_text(false, dt, end),
        Kind::PisoCourant => piso_text(true, dt, end),
        Kind::Frozen => frozen_text(dt, end),
        Kind::VarDensity => vd_text(dt, end),
        Kind::Vof => vof_text(dt, end),
    }
}

/// The f64 partial sums `t_1 .. t_n` of `n` steps of `dt`, added as the solvers add them (`time += dt`).
fn sums(dt: f64, n: usize) -> Vec<f64> {
    let mut t = 0.0;
    (0..n)
        .map(|_| {
            t += dt;
            t
        })
        .collect()
}

/// The end time of a variant: `t_n + f dt` (exactly `t_n` for f = 0).
fn end_of(dt: f64, f: f64) -> f64 {
    let t_n = sums(dt, N)[N - 1];
    if f == 0.0 {
        t_n
    } else {
        t_n + f * dt
    }
}

/// The step times a variant must give (registered L limits): E the sums; T the first n - 1 sums and the end time (the
/// last whole step stretched); J, N and the W variants the n sums and the end time (today's remainder step).
fn expected_times(dt: f64, tag: &str, end: f64) -> Vec<f64> {
    let s = sums(dt, N);
    match tag {
        "E" => s,
        "T" => s[..N - 1].iter().copied().chain([end]).collect(),
        _ => s.into_iter().chain([end]).collect(),
    }
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// A run stepped to its end: the simulation and the time after every step (from the step summaries).
fn run_to_end(text: &str) -> (Simulation, Vec<f64>) {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let mut times = Vec::new();
    while !sim.is_done() {
        times.push(sim.step().time);
        assert!(times.len() <= 100, "the run does not end: {times:?}");
    }
    (sim, times)
}

/// The `short_last_step` warnings among `take_runtime_warnings`.
fn short_last_step(sim: &mut Simulation) -> Vec<Value> {
    sim.take_runtime_warnings()
        .into_iter()
        .filter(|w| w["kind"] == "short_last_step")
        .collect()
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

/// Hash of the final state: the fields `ux uy p T alpha` that the run has, then the face flux.
fn state_hash(sim: &Simulation) -> u64 {
    let mut acc = 0xcbf2_9ce4_8422_2325_u64;
    for f in ["ux", "uy", "p", "T", "alpha"] {
        if let Ok(v) = sim.field(f) {
            fnv1a(&v, &mut acc);
        }
    }
    fnv1a(sim.flux(), &mut acc);
    acc
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-w48k-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

// ---------------------------------------------------------------------------------------- C1: the reported defect

/// C1 (e): the last value of `y` is an ordinary next value of the 200 before it: within their range widened by 10 % on
/// each side, and its change from the previous value at most 5 times their largest step-to-step change.
fn ordinary_last(y: &[f64]) -> (bool, String) {
    let n = y.len();
    if n < 202 {
        return (false, format!("only {n} values"));
    }
    let w = &y[n - 201..n - 1];
    let (lo, hi) = w
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
    let range = hi - lo;
    let big = w.windows(2).map(|p| (p[1] - p[0]).abs()).fold(0.0, f64::max);
    let last = y[n - 1];
    let jump = (last - y[n - 2]).abs();
    let ok = last >= lo - 0.1 * range && last <= hi + 0.1 * range && jump <= 5.0 * big;
    (
        ok,
        format!("last {last:.6e}, window [{lo:.6e}, {hi:.6e}], change {jump:.3e} (limit 5 x {big:.3e})"),
    )
}

#[test]
fn c1_cylinder_dt_0_05_to_100_ends_on_a_whole_step() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../example-projects/wind-tunnel/cases/cylinder-re100.toml"
    );
    let mut case = Case::parse_toml(&std::fs::read_to_string(path).expect("example case readable"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    // ADR 0142 smoke row 2: the same cylinder with dt 0.05 and end time 100
    case.tree.set("solver.dt", json!(0.05)).unwrap();
    case.tree.set("solver.end_time", json!(100.0)).unwrap();
    let out = tmp("c1");
    let m = Mutex::new(Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}")));
    let opts = RunOptions {
        out_dir: Some(out.clone()),
        ..Default::default()
    };
    let mut warnings: Vec<Value> = Vec::new();
    let t0 = std::time::Instant::now();
    let o = runctl::drive(&m, &opts, &|| false, &mut |e| {
        if e["event"] == "warning" {
            warnings.push(e.clone());
        }
    });
    let wall = t0.elapsed().as_secs_f64();
    let sim = m.into_inner().unwrap();
    let series = |n: &str| sim.monitors.get(n).map(|s| s.y.clone()).unwrap_or_default();
    let (dts, cd, cl) = (series("dt"), series("cd"), series("cl"));
    let t1999 = sums(0.05, 1999)[1998];
    let (cd_ok, cd_text) = ordinary_last(&cd);
    let (cl_ok, cl_text) = ordinary_last(&cl);
    let mut r = Reader::open(&out.join("fields")).expect("the run's field store opens");
    let snaps: Vec<(u64, f64)> = r.snapshots().iter().map(|s| (s.step, s.time)).collect();
    let k = snaps.len();
    let mut p_max = |i: usize| {
        r.read_field(i, "p")
            .expect("p stored")
            .iter()
            .fold(0.0_f64, |a, x| a.max(x.abs()))
    };
    let (p_last, p_prev) = if k >= 2 {
        (p_max(k - 1), p_max(k - 2))
    } else {
        (f64::NAN, f64::NAN)
    };
    let short: Vec<&Value> = warnings.iter().filter(|w| w["kind"] == "short_last_step").collect();
    eprintln!(
        "C1: exit {} state {} reason {:?}; {} steps, final time {:.17e}; last step {:.6e} (step 2000 must be 100 - t_1999 = {:.17e}); \
         cd: {cd_text}; cl: {cl_text}; store {k} snapshots, last {:?}, max|p| last {p_last:.6e}, before it {p_prev:.6e}; \
         {} warnings ({} short_last_step); wall {wall:.1} s",
        o.exit_code,
        o.state,
        o.reason,
        sim.iteration(),
        sim.time(),
        dts.last().copied().unwrap_or(f64::NAN),
        100.0 - t1999,
        snaps.last(),
        warnings.len(),
        short.len()
    );
    let mut problems = Vec::new();
    if !(o.exit_code == 0 && o.state == "finished" && o.reason == "end time 100 reached") {
        problems.push(format!("(a) outcome {o:?}"));
    }
    if sim.iteration() != 2000 || dts.len() != 2000 {
        problems.push(format!(
            "(b) {} steps ({} dt values), not 2000",
            sim.iteration(),
            dts.len()
        ));
    }
    if sim.time().to_bits() != 100.0_f64.to_bits() {
        problems.push(format!("(c) final time {:.17e}, not 100", sim.time()));
    }
    let whole = dts.len() == 2000
        && dts[..1999].iter().all(|d| d.to_bits() == 0.05_f64.to_bits())
        && dts[1999].to_bits() == (100.0 - t1999).to_bits();
    if !whole {
        problems.push(format!(
            "(d) steps: first {:?}, last {:?}",
            dts.first(),
            dts.iter().rev().take(2).collect::<Vec<_>>()
        ));
    }
    if !cd_ok {
        problems.push(format!("(e) cd {cd_text}"));
    }
    if !cl_ok {
        problems.push(format!("(e) cl {cl_text}"));
    }
    let store_ok = k == 100
        && snaps
            .last()
            .is_some_and(|s| s.0 == 2000 && s.1.to_bits() == 100.0_f64.to_bits())
        && p_last <= 2.0 * p_prev;
    if !store_ok {
        problems.push(format!(
            "(f) store: {k} snapshots, last {:?}, max|p| {p_last:.6e} against {p_prev:.6e} before it",
            snaps.last()
        ));
    }
    if !short.is_empty() {
        problems.push(format!("(g) short_last_step warnings {short:?}"));
    }
    let _ = std::fs::remove_dir_all(&out);
    assert!(problems.is_empty(), "C1: {problems:#?}");
}

// --------------------------------------------------------------------------------- L1-L6: landing on every path

fn landing(kind: Kind) {
    let dt = kind.dt();
    let mut problems = Vec::new();
    for (tag, f) in VARIANTS {
        let end = end_of(dt, f);
        let (mut sim, times) = run_to_end(&case_text(kind, end));
        let want = expected_times(dt, tag, end);
        let warn = short_last_step(&mut sim);
        let last_step = match times.len() {
            0 => f64::NAN,
            1 => times[0],
            n => times[n - 1] - times[n - 2],
        };
        eprintln!(
            "L {} {tag}: end {end:.17e}, {} steps (registered {}), times {:?}, last step {last_step:.6e} ({:.3e} dt), \
             {} short_last_step warnings {:?}; stop {:?}",
            kind.name(),
            times.len(),
            want.len(),
            times,
            last_step / dt,
            warn.len(),
            warn.iter().map(|w| w["fraction"].clone()).collect::<Vec<_>>(),
            sim.stop_reason()
        );
        if bits(&times) != bits(&want) {
            problems.push(format!(
                "L {} {tag}: step times {times:?}, registered {want:?}",
                kind.name()
            ));
        }
        let want_warning = kind.warns() && tag == "J";
        if want_warning {
            let fraction = warn.first().and_then(|w| w["fraction"].as_f64()).unwrap_or(f64::NAN);
            let fraction_ok = (fraction / f - 1.0).abs() <= 1e-6;
            if warn.len() != 1 || !fraction_ok {
                problems.push(format!(
                    "L6 {} {tag}: {} short_last_step warnings, fraction {fraction:e} (registered one, {f:e} within 1e-6)",
                    kind.name(),
                    warn.len()
                ));
            }
        } else if !warn.is_empty() {
            problems.push(format!("L6 {} {tag}: unexpected warnings {warn:?}", kind.name()));
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn l1_piso_lands_on_the_end_time() {
    landing(Kind::Piso);
}

#[test]
fn l2_piso_courant_lands_on_the_end_time() {
    landing(Kind::PisoCourant);
}

#[test]
fn l3_frozen_flow_lands_on_the_end_time() {
    landing(Kind::Frozen);
}

#[test]
fn l4_variable_density_lands_on_the_end_time() {
    landing(Kind::VarDensity);
}

#[test]
fn l5_vof_lands_on_the_end_time() {
    landing(Kind::Vof);
}

// ------------------------------------------------------------------------------------- W1: the warning as an event

fn drive(text: &str) -> (RunOutcome, Vec<Value>) {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let m = Mutex::new(Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}")));
    let mut warnings = Vec::new();
    let o = runctl::drive(&m, &RunOptions::default(), &|| false, &mut |e| {
        if e["event"] == "warning" {
            warnings.push(e.clone());
        }
    });
    (o, warnings)
}

#[test]
fn w1_short_last_step_warning_is_a_run_event() {
    let dt = Kind::Piso.dt();
    let mut problems = Vec::new();
    for (tag, f) in VARIANTS.into_iter().chain(WARN_VARIANTS) {
        let end = end_of(dt, f);
        let (o, warnings) = drive(&piso_text(false, dt, end));
        let short: Vec<&Value> = warnings.iter().filter(|w| w["kind"] == "short_last_step").collect();
        eprintln!(
            "W1 {tag}: exit {} state {}, {} steps; warning events {:?}",
            o.exit_code,
            o.state,
            o.iterations,
            warnings.iter().map(|w| w["message"].clone()).collect::<Vec<_>>()
        );
        let want = matches!(tag, "J" | "W-");
        if o.exit_code != 0 || short.len() != usize::from(want) {
            problems.push(format!(
                "W1 {tag}: exit {}, {} short_last_step events (registered {})",
                o.exit_code,
                short.len(),
                usize::from(want)
            ));
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

// ------------------------------------------------------------------------------ B2: bit-identity against the unfixed tree

/// (variant, steps, hash) of the final state on the unfixed tree, recorded by run R1 of ADR 0145 on Windows x86_64 and
/// written here before any fix code (R2). Other platforms print (bit-identity is per platform, ADR 0137).
const PRE_FIX: &[(&str, usize, u64)] = &[
    ("piso-E", 3, 0x311aed3df1158964),
    ("piso-T", 4, 0x01f98d890f4d0ebd),
    ("piso-J", 4, 0x40f7c7404c3ae2f8),
    ("piso-N", 4, 0x1f630542a2b74fcf),
    ("piso-W-", 4, 0x1fe8a48c457d1e20),
    ("piso-W+", 4, 0x8780c099cb836647),
    ("piso_courant-E", 3, 0x311aed3df1158964),
    ("piso_courant-T", 4, 0x01f98d890f4d0ebd),
    ("piso_courant-J", 4, 0x40f7c7404c3ae2f8),
    ("piso_courant-N", 4, 0x1f630542a2b74fcf),
    ("frozen-E", 3, 0x63676cfc5953f9d8),
    ("frozen-T", 4, 0x3bc1517691771c5e),
    ("frozen-J", 4, 0x80176db8fb42f4ad),
    ("frozen-N", 4, 0x46252b23e5531c31),
    ("variable_density-E", 3, 0xf3b7338710ebdffe),
    ("variable_density-T", 4, 0x738dbd15049f2063),
    ("variable_density-J", 4, 0x62911882de788a97),
    ("variable_density-N", 4, 0x8aefb8f9adb16758),
    ("vof-E", 3, 0x75ea9b9e2ea73dd7),
    ("vof-T", 4, 0xff4cd23857524e32),
    ("vof-J", 4, 0x70e5191d8baaa2af),
    ("vof-N", 4, 0x3c97d09f38ec2228),
];

#[test]
fn b2_final_states_equal_the_pre_fix_hashes_where_no_step_is_absorbed() {
    let mut got: Vec<(String, usize, u64)> = Vec::new();
    for kind in KINDS {
        let extra: &[(&str, f64)] = if kind == Kind::Piso { &WARN_VARIANTS } else { &[] };
        for &(tag, f) in VARIANTS.iter().chain(extra) {
            let end = end_of(kind.dt(), f);
            let (sim, times) = run_to_end(&case_text(kind, end));
            got.push((format!("{}-{tag}", kind.name()), times.len(), state_hash(&sim)));
        }
    }
    for (name, steps, hash) in &got {
        eprintln!("    (\"{name}\", {steps}, 0x{hash:016x}),");
    }
    let registered = cfg!(all(windows, target_arch = "x86_64"));
    let mut problems = Vec::new();
    if registered {
        for (name, steps, hash) in &got {
            match PRE_FIX.iter().find(|p| p.0 == name) {
                None => problems.push(format!("{name}: no pre-fix hash recorded")),
                Some(&(_, s0, h0)) if name.ends_with("-T") => {
                    if (*steps, *hash) == (s0, h0) {
                        problems.push(format!(
                            "{name}: ({steps}, {hash:016x}) equals the pre-fix state: the absorbed remainder is not seen"
                        ));
                    }
                }
                Some(&(_, s0, h0)) => {
                    if (*steps, *hash) != (s0, h0) {
                        problems.push(format!(
                            "{name}: ({steps}, {hash:016x}) differs from the pre-fix ({s0}, {h0:016x})"
                        ));
                    }
                }
            }
        }
    } else {
        eprintln!("B2: no recorded hashes for this platform (ADR 0137); the values are printed above");
    }
    assert!(problems.is_empty(), "B2: {problems:#?}");
}
