//! ADR 0156 (job 48V): `solver.corrector_control`, its hash-neutral default, the located errors (A9), the status stream (N5,
//! A10), the pre-run warning SV301 (B3's library part) and the tools of B2a (no warning on the shipped and literal cases), B2c
//! (classification of the census B2b) and the store dumper of the C runs. The cylinder checks (A1-A5, A8, B0, B1) are in
//! `w48t_cylinder_spokes.rs`, next to ADR 0154's harness.
//!
//! Every test prints what it measured before it asserts.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// The committed example case (`cylinder_ogrid` 64 x 48, PISO, dt 1/16) as text.
fn example_text() -> String {
    std::fs::read_to_string(repo_root().join("example-projects/wind-tunnel/cases/cylinder-re100.toml"))
        .expect("example case readable")
}

/// The example case with `line` added to its `[solver]` table.
fn example_with(line: &str) -> String {
    example_text().replacen("[solver]\n", &format!("[solver]\n{line}\n"), 1)
}

fn parse(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

fn with_sets(mut case: Case, sets: &[(&str, Value)]) -> Case {
    for (k, v) in sets {
        case.tree.set(k, v.clone()).unwrap_or_else(|e| panic!("{k}: {e:?}"));
    }
    case
}

// ------------------------------------------------------------------------------------------------ the key and its hash (D1, D6)

#[test]
fn w48v_key_defaults_to_fixed_and_its_default_leaves_the_case_hash_unchanged() {
    let c0 = parse(&example_text());
    assert_eq!(c0.get("solver.corrector_control").unwrap(), json!("fixed"));
    let h0 = c0.tree.content_hash();
    let explicit = parse(&example_with("corrector_control = \"fixed\""));
    let adaptive = parse(&example_with("corrector_control = \"adaptive\""));
    let set = with_sets(parse(&example_text()), &[("solver.corrector_control", json!("fixed"))]);
    println!(
        "hash: default {h0:016x}, written fixed {:016x}, set fixed {:016x}, adaptive {:016x}",
        explicit.tree.content_hash(),
        set.tree.content_hash(),
        adaptive.tree.content_hash()
    );
    assert_eq!(explicit.tree.to_json(), c0.tree.to_json());
    assert_eq!(explicit.tree.content_hash(), h0);
    assert_eq!(set.tree.content_hash(), h0);
    assert_ne!(adaptive.tree.content_hash(), h0);
    assert!(Case::parse_toml(&example_with("corrector_control = \"converged\"")).is_err());
    let js = rcfd_api::schema::case_schema().to_json_schema();
    assert_eq!(
        js["properties"]["solver"]["properties"]["corrector_control"]["enum"],
        json!(["fixed", "adaptive"])
    );
}

// ------------------------------------------------------------------------------------------------ A9 located errors (D5)

const PISTON: &str = r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 0.5
corrector_control = "adaptive"
[dynamic_mesh]
enabled = true
moving_patches = ["xmin"]
deformation = "smoothing"
displacement_expression_x = "1 [m/s^2] * t^2"
fixed_patches = ["xmax"]
"#;

/// The closed-box heating case of `end_time_landing_w48k.rs` (`vd_text`, ADR 0009 `low_mach`) at dt 0.01, with the key.
const LOW_MACH: &str = r#"
[mesh]
generator = "box_2d"
nx = 6
ny = 6
lx = 0.1
ly = 0.1
[fluid]
density = "ideal_gas"
gas_constant = 287.0
gamma = 1.4
mode = "low_mach"
reference_pressure = 101325.0
inner_iterations = 60
inner_tolerance = 1e-12
[energy]
enabled = true
relax = 1.0
heat_source = 84437.5
[initial]
temperature = 300.0
pressure = 101325.0
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
dt = 0.01
end_time = 0.1
alpha_u = 0.9
alpha_p = 1.0
momentum_rel_tol = 1e-10
momentum_max_iter = 2000
pressure_rel_tol = 1e-10
pressure_max_iter = 20000
corrector_control = "adaptive"
"#;

/// The five A9 arms; `simplec` is the arm's text (amendment 2 corrects it).
fn a9_arms(simplec: String) -> [(&'static str, String); 5] {
    let ad = "corrector_control = \"adaptive\"";
    [
        ("simplec", simplec),
        (
            "fractional_step",
            example_with(&format!("{ad}\ntime_advance = \"fractional_step\"")),
        ),
        ("dynamic mesh", PISTON.to_string()),
        (
            "error_tolerance",
            example_with(&format!("{ad}\nerror_tolerance = 1e-3")),
        ),
        ("variable density (low Mach pimple)", LOW_MACH.to_string()),
    ]
}

/// A9 as registered. KNOWN MISS by a set-up defect (ADR 0156 amendment 2): its simplec arm is refused by the case parser before
/// a simulation is built (the example's `output.store_time_interval` is a key of transient runs), so the test stops there; the
/// corrected test is `w48v_a9b_...`.
#[test]
#[ignore = "known miss (ADR 0156 amendment 2): set-up defect, the simplec arm keeps the example's transient store key and is refused by the parser; corrected in w48v_a9b_"]
fn w48v_a9_adaptive_is_refused_where_the_kernel_has_no_piso_correctors_known_miss() {
    let simplec =
        example_with("corrector_control = \"adaptive\"\n").replace("algorithm = \"piso\"", "algorithm = \"simplec\"");
    a9_check(a9_arms(simplec));
}

/// A9 corrected (ADR 0156 amendment 2): the simplec arm with `store_interval = 50` in place of the transient store key.
#[test]
fn w48v_a9b_adaptive_is_refused_where_the_kernel_has_no_piso_correctors() {
    let simplec = example_with("corrector_control = \"adaptive\"\n")
        .replace("algorithm = \"piso\"", "algorithm = \"simplec\"")
        .replace("store_time_interval = 1.0", "store_interval = 50");
    a9_check(a9_arms(simplec));
}

fn a9_check(texts: [(&'static str, String); 5]) {
    for (name, text) in texts {
        let r = Simulation::new(&parse(&text));
        let msg = match &r {
            Ok(_) => "built".to_string(),
            Err(d) => d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
        };
        println!("A9 {name}: {msg}");
        let Err(d) = r else { panic!("A9 {name}: built") };
        assert!(
            d.iter().any(|x| x.path == "solver.corrector_control"),
            "A9 {name}: {msg}"
        );
    }
    // the same cases without the key build (the refusal is the key's)
    assert!(Simulation::new(&parse(&PISTON.replace("corrector_control = \"adaptive\"\n", ""))).is_ok());
    assert!(Simulation::new(&parse(&LOW_MACH.replace("corrector_control = \"adaptive\"\n", ""))).is_ok());
}

// ------------------------------------------------------------------------------------------------ N5, A10: the status stream

fn drive(case: Case, steps: usize) -> Vec<Value> {
    let sim = Mutex::new(Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}")));
    let opts = RunOptions {
        max_steps: Some(steps),
        status_every: 1,
        ..RunOptions::default()
    };
    let mut events = Vec::new();
    runctl::drive(&sim, &opts, &|| false, &mut |e| events.push(e.clone()));
    events
}

#[test]
fn w48v_n5_a10_correctors_in_the_status_stream_only_when_adaptive() {
    let fixed = drive(parse(&example_text()), 3);
    for e in &fixed {
        assert!(
            e.get("correctors").is_none() && e.get("corrector_cap").is_none(),
            "N5: a fixed run's event has a corrector key: {e}"
        );
    }
    let adaptive = drive(parse(&example_with("corrector_control = \"adaptive\"")), 3);
    let its: Vec<&Value> = adaptive.iter().filter(|e| e["event"] == "iteration").collect();
    let fin = adaptive.iter().find(|e| e["event"] == "finish").expect("finish event");
    println!(
        "A10: iteration correctors {:?}; finish correctors {}",
        its.iter().map(|e| e["correctors"].clone()).collect::<Vec<_>>(),
        fin["correctors"]
    );
    assert_eq!(its.len(), 3);
    for e in &its {
        assert_eq!(e["correctors"], 2, "{e}");
        assert_eq!(e["corrector_cap"], 0, "{e}");
    }
    let c = &fin["correctors"];
    assert_eq!(c["mode"], "adaptive");
    assert_eq!(c["steps"], 3);
    assert!(c["estimates"].as_u64().unwrap() >= 1);
    assert_eq!(c["cap_bound"], 0);
    assert_eq!(c["log"][0][0], 1, "first estimate at step 1: {c}");
}

/// A11 (ADR 0156 amendment 3): the cap's reports end to end. K1's mesh (64 x 256) with `adaptive` at dt 1/16, 1/4 and 1, three
/// steps each through `runctl::drive`; on the first arm whose first estimate is cap-bound: a `corrector_cap` warning event
/// (`correctors` 10, `rho` > 0.5), `corrector_cap >= 1` in every later `iteration` event, `finish.correctors.cap_bound >= 1`.
#[test]
fn w48v_a11_the_cap_is_reported_end_to_end() {
    for dt in [1.0 / 16.0, 0.25, 1.0] {
        let case = with_sets(
            parse(&example_with("corrector_control = \"adaptive\"")),
            &[
                ("mesh.nx", json!(64)),
                ("mesh.ny", json!(256)),
                ("solver.dt", json!(dt)),
            ],
        );
        let ev = drive(case, 3);
        let fin = ev.iter().find(|e| e["event"] == "finish").expect("finish event");
        let c = &fin["correctors"];
        println!("A11 dt {dt}: state {}, correctors {c}", fin["state"]);
        if c["log"][0][3] != true {
            continue;
        }
        let w = ev
            .iter()
            .position(|e| e["event"] == "warning" && e["kind"] == "corrector_cap")
            .expect("A11: a corrector_cap warning event");
        println!("A11 warning: {}", ev[w]["message"]);
        assert_eq!(ev[w]["correctors"], 10, "{}", ev[w]);
        assert!(ev[w]["rho"].as_f64().unwrap() > 0.5, "{}", ev[w]);
        let later: Vec<&Value> = ev[w..].iter().filter(|e| e["event"] == "iteration").collect();
        assert!(!later.is_empty(), "A11: no iteration event after the warning");
        for e in later {
            assert!(e["corrector_cap"].as_u64().unwrap() >= 1, "{e}");
        }
        assert!(c["cap_bound"].as_u64().unwrap() >= 1, "{c}");
        return;
    }
    panic!("A11: no arm bound the cap (reported as not exercised)");
}

// ------------------------------------------------------------------------------------------------ B3 (library): check_only

fn sv301(ev: &Value) -> Vec<Value> {
    ev["rules"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["id"] == "SV301")
        .cloned()
        .collect()
}

#[test]
fn w48v_b3_check_only_gives_sv301_on_the_fine_wall_layer_and_not_on_the_committed_case() {
    let fine = with_sets(
        parse(&example_text()),
        &[
            ("mesh.nx", json!(64)),
            ("mesh.ny", json!(256)),
            ("solver.dt", json!(0.015625)),
        ],
    );
    let (code, ev) = runctl::check_only(&fine);
    let r = sv301(&ev);
    println!("B3 64 x 256: exit {code}, {r:?}");
    assert_eq!(code, runctl::exit::OK);
    assert_eq!(r.len(), 1);
    let r = &r[0];
    assert_eq!(r["severity"], "warning");
    assert_eq!(r["category"], "solver");
    assert_eq!(r["path"], "solver.corrector_control");
    assert_eq!(
        r["fix"],
        json!({"path": "solver.corrector_control", "value": "adaptive"})
    );
    let m = r["message"].as_str().unwrap();
    assert!(
        m.contains("solver.corrector_control = \"adaptive\"") && m.contains("n_correctors = 2"),
        "{m}"
    );
    let (code, ev) = runctl::check_only(&parse(&example_text()));
    println!("B3 64 x 48: exit {code}, SV301 {:?}", sv301(&ev));
    assert_eq!(code, runctl::exit::OK);
    assert!(sv301(&ev).is_empty());
    // with the remedy applied there is nothing to warn about
    let (_, ev) = runctl::check_only(&with_sets(fine, &[("solver.corrector_control", json!("adaptive"))]));
    assert!(sv301(&ev).is_empty());
}

// ------------------------------------------------------------------------------------------------ B2a: the shipped and literal cases

fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, ext, out);
        } else if p.extension().is_some_and(|x| x == ext) {
            out.push(p);
        }
    }
}

/// Fenced toml blocks of a markdown file (as ADR 0031's E2).
fn fenced_toml(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for l in text.lines() {
        if l.trim_start().starts_with("```") {
            match cur.take() {
                Some(b) => out.push(b),
                None if l.trim() == "```toml" => cur = Some(String::new()),
                None => cur = None,
            }
        } else if let Some(b) = cur.as_mut() {
            b.push_str(l);
            b.push('\n');
        }
    }
    out
}

/// String literals of a Rust source (as ADR 0031's E2): raw strings and ordinary strings.
fn rust_literals(src: &str) -> Vec<String> {
    let b: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == '/' && b.get(i + 1) == Some(&'/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == 'r' && (i == 0 || !(b[i - 1].is_alphanumeric() || b[i - 1] == '_')) {
            let mut j = i + 1;
            let mut hashes = 0;
            while b.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if b.get(j) == Some(&'"') {
                let start = j + 1;
                let mut k = start;
                while k < b.len() && !(b[k] == '"' && (0..hashes).all(|h| b.get(k + 1 + h) == Some(&'#'))) {
                    k += 1;
                }
                out.push(b[start..k.min(b.len())].iter().collect());
                i = k + 1 + hashes;
                continue;
            }
        }
        if c == '"' {
            let mut s = String::new();
            let mut k = i + 1;
            while k < b.len() && b[k] != '"' {
                if b[k] == '\\' {
                    k += 1;
                    match b.get(k) {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some('\n') => {
                            while b.get(k + 1).is_some_and(|c| c.is_whitespace()) {
                                k += 1;
                            }
                        }
                        Some(o) => s.push(*o),
                        None => {}
                    }
                } else {
                    s.push(b[k]);
                }
                k += 1;
            }
            out.push(s);
            i = k + 1;
            continue;
        }
        if c == '\'' {
            if b.get(i + 2) == Some(&'\'') {
                i += 3;
                continue;
            }
            if b.get(i + 1) == Some(&'\\') && b.get(i + 3) == Some(&'\'') {
                i += 4;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Why the preflight does not apply to `case` (`None` when it does), read from its settings.
fn not_applicable(case: &Case) -> Option<String> {
    let s = |k: &str| {
        case.get(k)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    };
    let alg = s("solver.algorithm");
    if !matches!(alg.as_str(), "piso" | "pimple") {
        return Some(format!("algorithm {alg}"));
    }
    if s("solver.time_advance") == "fractional_step" {
        return Some("fractional_step".into());
    }
    if case
        .get("solver.n_correctors")
        .ok()
        .and_then(|v| v.as_i64())
        .unwrap_or(2)
        < 2
    {
        return Some("n_correctors 1".into());
    }
    if s("solver.corrector_control") == "adaptive" {
        return Some("adaptive".into());
    }
    if s("solver.pressure_lag_check") == "off" {
        return Some("pressure_lag_check off".into());
    }
    if case.get("dynamic_mesh.enabled").ok().and_then(|v| v.as_bool()) == Some(true) {
        return Some("dynamic mesh".into());
    }
    None
}

/// B2a (ADR 0156): the preflight on ADR 0031's E2 collection and the example projects' cases: zero SV301 warnings.
#[test]
#[ignore = "ADR 0156 B2a (prints the list of cases checked)"]
fn w48v_b2a_no_sv301_on_the_shipped_and_literal_cases() {
    let root = repo_root();
    let mut texts: Vec<(String, String, PathBuf)> = Vec::new();
    let mut files = Vec::new();
    walk(&root.join("cases"), "toml", &mut files);
    for tree in ["example-projects", "example-projects-linux"] {
        if let Ok(rd) = std::fs::read_dir(root.join(tree)) {
            for e in rd.flatten() {
                walk(&e.path().join("cases"), "toml", &mut files);
            }
        }
    }
    files.sort();
    for p in &files {
        let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        texts.push((
            rel,
            std::fs::read_to_string(p).unwrap(),
            p.parent().unwrap().to_path_buf(),
        ));
    }
    for md in ["docs/case-schema.md", "docs/run-control.md"] {
        let t = std::fs::read_to_string(root.join(md)).unwrap();
        for (i, b) in fenced_toml(&t).into_iter().enumerate() {
            texts.push((format!("{md}#{i}"), b, root.join("docs")));
        }
    }
    let mut tests = Vec::new();
    walk(&root.join("crates"), "rs", &mut tests);
    tests.retain(|p| p.components().any(|c| c.as_os_str() == "tests"));
    tests.sort();
    for p in &tests {
        let src = std::fs::read_to_string(p).unwrap();
        for (i, lit) in rust_literals(&src).into_iter().enumerate() {
            if lit.contains("[mesh]") || lit.contains("generator") {
                let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                texts.push((format!("{rel}#{i}"), lit, p.parent().unwrap().to_path_buf()));
            }
        }
    }
    let mut seen = BTreeSet::new();
    let (mut applied, mut warned, mut skipped) = (Vec::new(), Vec::new(), Vec::new());
    for (origin, text, dir) in texts {
        if !seen.insert(text.clone()) {
            continue;
        }
        if text.contains("{}") || text.contains("{n}") || text.contains("{nx}") || text.contains("{{") {
            skipped.push(format!("{origin}: template"));
            continue;
        }
        let Ok(case) = Case::parse_toml_in(&text, &dir) else {
            skipped.push(format!("{origin}: not a valid case"));
            continue;
        };
        if let Some(why) = not_applicable(&case) {
            skipped.push(format!("{origin}: {why}"));
            continue;
        }
        let t0 = std::time::Instant::now();
        let sim = match Simulation::new(&case) {
            Ok(s) => s,
            Err(d) => {
                skipped.push(format!("{origin}: simulation not built ({})", d[0]));
                continue;
            }
        };
        match sim.pressure_lag_preflight() {
            Some(p) => {
                let line = format!(
                    "{origin}: rho {:.4} ({} cells, {:.1} s)",
                    p.rho,
                    sim.geom.n_cells(),
                    t0.elapsed().as_secs_f64()
                );
                if rcfd_api::rules::lag_preflight_rule(&p).is_some() {
                    warned.push(line.clone());
                }
                applied.push(line);
            }
            None => skipped.push(format!(
                "{origin}: not applicable (engine: variable density, VOF or compressible)"
            )),
        }
    }
    println!("B2a: {} distinct case texts with the preflight:", applied.len());
    for l in &applied {
        println!("  {l}");
    }
    println!("B2a: {} skipped:", skipped.len());
    for l in &skipped {
        println!("  {l}");
    }
    println!("B2a: SV301 warnings: {warned:#?}");
    assert!(
        applied.len() >= 3,
        "under-powered: {} cases with the preflight",
        applied.len()
    );
    assert!(warned.is_empty(), "B2a: SV301 on {warned:#?}");
}

// ------------------------------------------------------------------------------------------------ B2c: classify the census

/// The ADR 0073 start vector of the lag estimate (cosines over the bounding box plus 0.1 of a fixed pseudo-random part),
/// normalised to a largest magnitude of 1.
fn start_vector(sim: &Simulation) -> Vec<f64> {
    let cc = sim.geom.mesh.cell_centres();
    let active = sim.geom.active;
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for x in cc {
        for k in 0..3 {
            lo[k] = lo[k].min(x[k]);
            hi[k] = hi[k].max(x[k]);
        }
    }
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let v: Vec<f64> = cc
        .iter()
        .map(|x| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let r = (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
            let mut v = 1.0;
            for k in 0..3 {
                if active[k] && hi[k] > lo[k] {
                    v *= (std::f64::consts::PI * (x[k] - lo[k]) / (hi[k] - lo[k])).cos();
                }
            }
            v + 0.1 * r
        })
        .collect();
    let m = v.iter().fold(0.0f64, |a, x| a.max(x.abs())).max(1e-300);
    v.into_iter().map(|x| x / m).collect()
}

/// Volume-weighted L2 norm of `a - b` with its volume mean removed.
fn diff_norm(a: &[f64], b: &[f64], vol: &[f64]) -> f64 {
    let tv: f64 = vol.iter().sum();
    let mean = a.iter().zip(b).zip(vol).map(|((x, y), v)| (x - y) * v).sum::<f64>() / tv;
    a.iter()
        .zip(b)
        .zip(vol)
        .map(|((x, y), v)| (x - y - mean).powi(2) * v)
        .sum::<f64>()
        .sqrt()
}

/// B2c (ADR 0156): every distinct set-up of the census B2b (`W48V_B2B_LOG` = its JSON-lines log) with rho >= 1.3, run 60 steps
/// with the fixed coupling from its initial state with and without p0 += 1e-6 x the ADR 0073 start vector: g = (|dp(60)| /
/// |dp(30)|)^(1/30). True positive when g > 1.05 or the perturbed run is stopped, false when g < 0.95; zero false positives.
#[test]
#[ignore = "ADR 0156 B2c: W48V_B2B_LOG=<census log> (classifies the census's SV301 set-ups)"]
fn w48v_b2c_classify_the_census_warnings() {
    let log = std::env::var("W48V_B2B_LOG").expect("W48V_B2B_LOG names the census log");
    let text = std::fs::read_to_string(&log).unwrap();
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    let mut false_pos = Vec::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let rho = v["rho"].as_f64().unwrap_or(0.0);
        if rho < rcfd_api::rules::SV301_LEVEL || !seen.insert(v["settings"].to_string()) {
            continue;
        }
        let base = PathBuf::from(v["base_dir"].as_str().unwrap_or("."));
        let label = format!(
            "{} (rho {rho:.3}, {} cells)",
            v["thread"].as_str().unwrap_or("?"),
            v["cells"]
        );
        let case = match Case::from_json_in(&v["settings"], &base) {
            Ok(c) => c,
            Err(d) => {
                rows.push(format!("{label}: not rebuilt ({})", d[0]));
                continue;
            }
        };
        let (Ok(mut a), Ok(mut b)) = (Simulation::new(&case), Simulation::new(&case)) else {
            rows.push(format!("{label}: simulation not built"));
            continue;
        };
        let dp: Vec<f64> = start_vector(&b).iter().map(|x| 1e-6 * x).collect();
        assert!(b.perturb_pressure(&dp), "{label}: not the transient solver");
        let vol = a.geom.mesh.cell_volumes().to_vec();
        let (mut n30, mut n60, mut stopped) = (f64::NAN, f64::NAN, None);
        for k in 1..=60 {
            a.step();
            b.step();
            if b.diverged() || a.diverged() {
                stopped = Some((k, a.diverged(), b.diverged()));
                break;
            }
            let d = diff_norm(&b.field("p").unwrap(), &a.field("p").unwrap(), &vol);
            if k == 30 {
                n30 = d;
            }
            if k == 60 {
                n60 = d;
            }
        }
        let class = match stopped {
            Some((k, base_stop, pert_stop)) => {
                format!("true positive (stopped at step {k}: unperturbed {base_stop}, perturbed {pert_stop})")
            }
            None => {
                let g = (n60 / n30).powf(1.0 / 30.0);
                if g > 1.05 {
                    format!("true positive (g {g:.4}, |dp| {n30:.3e} -> {n60:.3e})")
                } else if g < 0.95 {
                    false_pos.push(label.clone());
                    format!("FALSE positive (g {g:.4}, |dp| {n30:.3e} -> {n60:.3e})")
                } else {
                    format!("undetermined (g {g:.4}, |dp| {n30:.3e} -> {n60:.3e})")
                }
            }
        };
        rows.push(format!("{label}: {class}"));
    }
    println!(
        "B2c: {} distinct set-ups with rho >= {}:",
        rows.len(),
        rcfd_api::rules::SV301_LEVEL
    );
    for r in &rows {
        println!("  {r}");
    }
    assert!(false_pos.is_empty(), "B2c: false positives {false_pos:#?}");
}

// ------------------------------------------------------------------------------------------------ store dumper for the C runs

fn write_f64(path: &Path, v: &[f64]) {
    let mut b = Vec::with_capacity(v.len() * 8);
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}

/// The store dumper of ADR 0154 (`w48t_dump_stores` of `crates/rcfd-studio/tests/w48t_cylinder_spokes.rs`), here so that the
/// Studio is not built: `W48V_DUMP` = `<store dir>=<out prefix>;...`, optional `W48V_FIELDS` (default `p`). Writes
/// `<prefix>.centres.f64`, `<prefix>.<field>.f64` (snapshots x cells, widened to f64) and `<prefix>.meta.json`, the formats
/// `scripts/w48t_spoke_metrics.py` reads.
#[test]
#[ignore = "ADR 0156 tool: W48V_DUMP=<store dir>=<out prefix>;... (reads stores written elsewhere)"]
fn w48v_dump_stores() {
    let fields_var = std::env::var("W48V_FIELDS").unwrap_or_else(|_| "p".into());
    let fields: Vec<&str> = fields_var.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    let list = std::env::var("W48V_DUMP").expect("W48V_DUMP=<store dir>=<out prefix>;...");
    for spec in list.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (dir, prefix) = spec.split_once('=').unwrap_or_else(|| panic!("bad spec {spec}"));
        let mut r = rcfd_tsstore::Reader::open(Path::new(dir)).unwrap_or_else(|e| panic!("{dir}: {e}"));
        let mesh = r.mesh().unwrap();
        let n = mesh.n_cells();
        let centres: Vec<f64> = mesh.cell_centres().iter().flat_map(|c| c.iter().copied()).collect();
        write_f64(Path::new(&format!("{prefix}.centres.f64")), &centres);
        let snaps: Vec<(u64, f64)> = r.snapshots().iter().map(|s| (s.step, s.time)).collect();
        for f in &fields {
            let mut all = Vec::with_capacity(n * snaps.len());
            for i in 0..snaps.len() {
                let v = r
                    .read_field(i, f)
                    .unwrap_or_else(|e| panic!("{dir}: snapshot {i} field {f}: {e}"));
                assert_eq!(v.len(), n);
                all.extend(v);
            }
            write_f64(Path::new(&format!("{prefix}.{f}.f64")), &all);
        }
        let meta = json!({
            "store": dir, "cells": n, "fields": fields,
            "steps": snaps.iter().map(|s| s.0).collect::<Vec<_>>(),
            "times": snaps.iter().map(|s| s.1).collect::<Vec<_>>(),
            "index_source": format!("{:?}", r.index_source()),
        });
        std::fs::write(
            format!("{prefix}.meta.json"),
            serde_json::to_string_pretty(&meta).unwrap(),
        )
        .unwrap();
        eprintln!(
            "dumped {dir}: {n} cells, {} snapshots, fields {fields:?} -> {prefix}",
            snaps.len()
        );
    }
}
