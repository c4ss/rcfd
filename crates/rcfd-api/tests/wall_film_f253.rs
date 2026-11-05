//! F-253 (ADR 0192): the particle wall film in the case tree and the run path. Registered checks C1 to C4, D2 (the record
//! R2 of the output files of a particle run without a film, made on the base code main 201ea17 before any code of F-253)
//! and D4. The limits are those of ADR 0192, fixed before this code ran.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_api::{particle_run, particles};
use rcfd_particles::{
    DragLaw, FilmSpec, Flow, InjKind, InjectorExt, InjectorSpec, Interp, Options, Scheme, SizeDist, State, Tracker,
    VelMode, WallAction,
};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Case K of ADR 0192 C2: a box_2d incline (gravity 9.81 at 30 degrees from -y), frozen flow at rest, the film on `ymin`,
/// one point injection that lands on the centre of the first `ymin` face. `{particles}` and `{film}` take extra lines of
/// the `[particles]` and `[particles.film]` tables; `{physics}` replaces the gravity line.
const K_CASE: &str = r#"
title = "wall film K"
[mesh]
generator = "box_2d"
nx = 10
ny = 2
lx = 0.05
ly = 0.01
[physics]
nu = 1.5e-5
rho = 1.2
{physics}
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simple"
frozen_flow = true
max_iterations = 2
dt = 0.01
[particles]
enabled = true
density = 1000.0
drag = "none"
integration = "analytic"
end_time = 20.0
dt = 0.01
{particles}
[particles.film]
enabled = true
patches = ["ymin"]
viscosity = 1.0e-3
{film}
[particles.injection.f]
kind = "point"
position = [1.3452994616207485e-3, 2.0e-3, 2.5e-3]
rate = 730.0
diameter = 1.0e-4
mass_flow = 2.5e-6
"#;

const K_GRAVITY: &str = "gravity = [4.905, -8.495709211125343, 0.0]";

/// ADR 0192 amendment 1: `Simulation::new` refuses frozen flow without the energy equation, so the corrected case K
/// gains these two tables (as the PT14 case of ADR 0028 has).
const K_ENERGY: &str = "[energy]\nenabled = true\n[initial]\ntemperature = 300.0\n[boundaries.xmin]";

/// Case K (`fixed`: corrected by amendment 1, else as registered) with replacements: (placeholder, text) pairs;
/// placeholders not given are filled with their defaults.
fn k_text_of(fixed: bool, subs: &[(&str, &str)]) -> String {
    let mut t = if fixed {
        K_CASE.replace("[boundaries.xmin]", K_ENERGY)
    } else {
        K_CASE.to_string()
    };
    for (k, v) in subs {
        t = t.replace(k, v);
    }
    t.replace("{physics}", K_GRAVITY)
        .replace("{particles}", "")
        .replace("{film}", "")
}

fn parse(t: &str) -> Case {
    Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"))
}

/// Diagnostics of a text: parse errors, then the semantic errors, then the errors of `Simulation::new`.
fn errors(t: &str) -> Vec<Diagnostic> {
    let c = match Case::parse_toml(t) {
        Err(e) => return e,
        Ok(c) => c,
    };
    let e = c.semantic_errors();
    if !e.is_empty() {
        return e;
    }
    match Simulation::new(&c) {
        Err(e) => e,
        Ok(_) => Vec::new(),
    }
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// The reference of the acceptance, `(3 mu Gamma / (rho^2 g sin(theta)))^(1/3)`, with `Gamma` the injected mass flow
/// over the 5 mm depth of the box.
fn h_acc() -> f64 {
    let gamma = 2.5e-6 / 5e-3;
    (3.0 * 1e-3 * gamma / (1000.0 * 1000.0 * 9.81 * 30f64.to_radians().sin())).cbrt()
}

/// C1 on the case K as registered: stopped at its precondition (amendment 1); kept as the record.
#[test]
#[ignore = "ADR 0192 amendment 1: case K as registered is refused (frozen flow needs energy.enabled); c1b is the corrected test"]
fn c1_located_errors_of_the_film_keys_k_setup_defect() {
    c1(false);
}

/// C1 (corrected case K, ADR 0192 amendment 1).
#[test]
fn c1b_located_errors_of_the_film_keys() {
    c1(true);
}

/// C1: every invalid input of ADR 0192 S2 is located at its path; the valid case K has no diagnostic.
fn c1(fixed: bool) {
    let k_text = |subs: &[(&str, &str)]| k_text_of(fixed, subs);
    let ok = errors(&k_text(&[]));
    assert!(ok.is_empty(), "valid K: {ok:?}");
    let cases: Vec<(String, &str)> = vec![
        (
            k_text(&[]).replace("enabled = true\ndensity", "enabled = false\ndensity"),
            "particles.film.enabled",
        ),
        (k_text(&[("{particles}", "two_way = true")]), "particles.film.enabled"),
        (
            k_text(&[("{particles}", "heat_transfer = true")]),
            "particles.film.enabled",
        ),
        (
            k_text(&[]).replace("patches = [\"ymin\"]", "patches = []"),
            "particles.film.patches",
        ),
        (
            k_text(&[]).replace("patches = [\"ymin\"]", "patches = [\"ymin\", \"ymin\"]"),
            "particles.film.patches",
        ),
        (
            k_text(&[]).replace(
                "[particles.film]",
                "[particles.walls.ymin]\ninteraction = \"stick\"\n[particles.film]",
            ),
            "particles.film.patches",
        ),
        (
            k_text(&[]).replace("viscosity = 1.0e-3", ""),
            "particles.film.viscosity",
        ),
        (
            k_text(&[]).replace("viscosity = 1.0e-3", "viscosity = 0.0"),
            "particles.film.viscosity",
        ),
        (k_text(&[("{particles}", "gravity = false")]), "particles.gravity"),
        (k_text(&[("{physics}", "gravity = [0.0, 0.0, 0.0]")]), "physics.gravity"),
        (
            k_text(&[]).replace("patches = [\"ymin\"]", "patches = [\"nope\"]"),
            "particles.film.patches",
        ),
        (
            k_text(&[]).replace("patches = [\"ymin\"]", "patches = [\"frontAndBack\"]"),
            "particles.film.patches",
        ),
    ];
    for (i, (text, path)) in cases.iter().enumerate() {
        let e = errors(text);
        println!(
            "C1 input {i}: expected at {path}; got {:?}",
            e.iter().map(|d| (&d.path, &d.message)).collect::<Vec<_>>()
        );
        assert!(
            e.iter().any(|d| d.path == *path),
            "C1 input {i}: no diagnostic at {path}: {e:?}"
        );
    }
}

/// The face masses, drained mass and fingerprint of a tracker.
fn state_of(tr: &Tracker) -> (Vec<u64>, Vec<u64>, u64) {
    let f = tr.film().expect("film");
    (
        tr.fingerprint(),
        f.masses().iter().map(|m| m.to_bits()).collect(),
        f.drained().to_bits(),
    )
}

/// C2, C3 on the case K as registered: stopped at the set-up (amendment 1); kept as the record.
#[test]
#[ignore = "ADR 0192 amendment 1: case K as registered is refused (frozen flow needs energy.enabled); c2b_c3b is the corrected test"]
fn c2_c3_tree_equals_library_and_meets_the_nusselt_thickness_k_setup_defect() {
    c2_c3(false);
}

/// C2, C3 (corrected case K, ADR 0192 amendment 1).
#[test]
fn c2b_c3b_tree_equals_library_and_meets_the_nusselt_thickness() {
    c2_c3(true);
}

/// C2, C3: case K through `track_frozen`, through the run path and as a tracker built by hand from the same inputs gives the
/// same bits; the row-6 face is within 5 percent of the Nusselt thickness at 20 s.
fn c2_c3(fixed: bool) {
    let case = parse(&k_text_of(fixed, &[]));
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let a = particles::track_frozen(&case, &sim).unwrap_or_else(|e| panic!("{e:?}"));
    let b = particle_run::steady_run(&sim, None).unwrap_or_else(|e| panic!("{e:?}"));
    // by hand: the options, injector (seed 1 * 1_000_003 + 0), extension and film build_tracker makes from K
    let mesh = sim.geom.mesh.clone();
    let g = [4.905, -8.495709211125343, 0.0];
    let opts = Options {
        drag: DragLaw::None,
        scheme: Scheme::Analytic,
        interp: Interp::Gradient,
        gravity: g,
        default_wall: WallAction::Reflect(1.0),
        ..Options::default()
    };
    let mut c = Tracker::new(mesh.clone(), opts, &[]).unwrap();
    c.add_injector(InjectorSpec {
        name: "f".into(),
        kind: InjKind::Point([1.3452994616207485e-3, 2.0e-3, 2.5e-3]),
        rate: 730.0,
        t_start: 0.0,
        t_end: 20.0,
        velocity: VelMode::Given([0.0; 3]),
        dist: SizeDist::Constant(1e-4),
        density: 1000.0,
        parcel_count: 1.0,
        seed: 1_000_003,
    })
    .unwrap();
    c.set_injector_ext(
        "f",
        InjectorExt {
            mass_flow: Some(2.5e-6),
            ..InjectorExt::default()
        },
    )
    .unwrap();
    c.set_film(FilmSpec {
        patches: vec!["ymin".into()],
        density: 1000.0,
        viscosity: 1e-3,
        gravity: g,
    })
    .unwrap();
    let n = mesh.n_cells();
    let (u, grad) = (vec![[0.0; 3]; n], vec![[[0.0; 3]; 3]; n]);
    let flow = Flow {
        u: &u,
        grad: Some(&grad),
        rho: 1.2,
        mu: 1.2 * 1.5e-5,
    };
    while c.time < 20.0 * (1.0 - 1e-12) {
        let h = 0.01f64.min(20.0 - c.time);
        c.advance(&flow, h).unwrap();
    }
    let (sa, sb, sc) = (state_of(&a), state_of(&b.tracker), state_of(&c));
    println!(
        "C2: fingerprint lengths {} {} {}; equal a=b {}, a=c {}",
        sa.0.len(),
        sb.0.len(),
        sc.0.len(),
        sa == sb,
        sa == sc
    );
    assert!(sa == sb && sa == sc, "C2: the three trackers differ");
    let film = a.film().unwrap();
    let k6 = (0..film.len())
        .find(|&k| (film.centre(k)[0] - 0.0325).abs() < 1e-9)
        .expect("the row-6 face");
    let e = rel(film.thickness(k6), h_acc());
    println!(
        "C3: row-6 thickness {:e}, h_acc {:e}, relative {e:e}; injected {}, absorbed {}, largest Re_f {:.4}",
        film.thickness(k6),
        h_acc(),
        a.injected(),
        a.count(State::Film),
        film.max_reynolds()
    );
    assert!(e <= 0.05, "C3");
}

/// C4 on the case K as registered: stopped at the set-up (amendment 1); kept as the record.
#[test]
#[ignore = "ADR 0192 amendment 1: case K as registered is refused (frozen flow needs energy.enabled); c4b is the corrected test"]
fn c4_report_and_film_csv_k_setup_defect() {
    c4(false);
}

/// C4 (corrected case K, ADR 0192 amendment 1).
#[test]
fn c4b_report_and_film_csv() {
    c4(true);
}

/// C4: the film fields of the report and the film CSV of the run path.
fn c4(fixed: bool) {
    let dir = tmp(if fixed { "c4b" } else { "c4" });
    let sim = run_path(&k_text_of(fixed, &[]), &dir);
    let run = particle_run::steady_run(&sim, None).unwrap_or_else(|e| panic!("{e:?}"));
    let tr = &run.tracker;
    let film = tr.film().unwrap();
    let rep: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("particle_report.json")).unwrap()).unwrap();
    let f = &rep["film"];
    println!("C4 report film {f}, counts {}", rep["counts"]);
    assert_eq!(f["faces"].as_u64(), Some(film.len() as u64));
    assert_eq!(f["held"].as_f64(), Some(film.held()));
    assert_eq!(f["drained"].as_f64(), Some(film.drained()));
    assert_eq!(f["absorbed"].as_f64(), Some(film.received()));
    assert_eq!(f["max_thickness"].as_f64(), Some(film.max_thickness()));
    assert_eq!(f["max_reynolds"].as_f64(), Some(film.max_reynolds()));
    assert_eq!(rep["counts"]["film"].as_u64(), Some(tr.count(State::Film) as u64));
    let csv = std::fs::read_to_string(dir.join("particle_film.csv")).unwrap();
    let rows: Vec<&str> = csv.lines().skip(1).collect();
    assert_eq!(rows.len(), film.len());
    let mut worst = 0.0f64;
    for (k, r) in rows.iter().enumerate() {
        let c: Vec<&str> = r.split(',').collect();
        assert_eq!(c[0].parse::<usize>().unwrap(), film.faces()[k]);
        assert_eq!(c[1], "ymin");
        let (area, h, m): (f64, f64, f64) = (c[5].parse().unwrap(), c[6].parse().unwrap(), c[7].parse().unwrap());
        if m > 0.0 {
            worst = worst.max(rel(h, m / (1000.0 * area)));
        }
    }
    println!("C4 csv: {} rows, thickness against M/(rho A) {worst:e}", rows.len());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(worst <= 1e-15);
}

/// D4: a case without `particles.film` has no `film` key in its data; writing the table with `enabled = false` only leaves
/// the settings hash unchanged.
#[test]
fn d4_absent_film_leaves_data_and_hash_unchanged() {
    let without = parse(R2_CASE);
    let data = without.tree.to_json();
    assert!(data["particles"].get("film").is_none(), "D4: film in the data");
    let with = parse(&format!("{R2_CASE}\n[particles.film]\nenabled = false\n"));
    println!(
        "D4: hash without {:016x}, with an inert [particles.film] {:016x}",
        without.tree.content_hash(),
        with.tree.content_hash()
    );
    assert!(
        with.tree.to_json()["particles"].get("film").is_some(),
        "D4 set-up: the table was not read"
    );
    assert_eq!(without.tree.content_hash(), with.tree.content_hash(), "D4");
}

/// ADR 0192 R2: FNV-1a 64 of the names and bytes of the output files of the R2 run on main 201ea17 (Windows; the flow
/// and the particles call `exp`, whose last bits may differ elsewhere, ADR 0137).
const R2: u64 = 0xce9d_f10b_3a88_5c96;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f253_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The R2 case: the 20 x 20 lid-driven box of `particle_run.rs` (piso, 40 steps of 0.01 s) with a point injection of
/// fluid-velocity parcels, reflecting walls, the track log and numbered VTP files.
const R2_CASE: &str = r#"
title = "particle run R2"
[mesh]
generator = "box_2d"
nx = 20
ny = 20
lx = 1.0
ly = 1.0
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[solver]
algorithm = "piso"
dt = 0.01
end_time = 0.4
[particles]
enabled = true
density = 1000.0
drag = "schiller_naumann"
integration = "analytic"
gravity = false
end_time = 0.4
dt = 0.01
tracks = true
track_every = 3
output_every = 10
[particles.injection.p]
kind = "point"
position = [0.3, 0.7, 0.0]
rate = 50.0
end_time = 0.2
velocity_mode = "fluid"
diameter = 1.0e-3
"#;

/// The run path of the library (`drive` plus `write_results`) into `dir`.
fn run_path(t: &str, dir: &Path) -> Simulation {
    let case = Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"));
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

/// FNV-1a 64 of the particle output files of `dir` (names sorted, each name then its bytes).
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

/// ADR 0192 R2 (check D2): a particle run without a film writes the same files as on the base code. Compared on
/// Windows, printed elsewhere (ADR 0137). `RCFD_F253_RECORD=1` prints the value only.
#[test]
fn d2_no_film_run_outputs_match_the_base_record() {
    let dir = tmp("d2");
    run_path(R2_CASE, &dir);
    let (h, names) = particle_files_hash(&dir);
    println!("R2 {h:016x} over {names:?}");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(names.len() >= 4, "set-up: {names:?}");
    if std::env::var("RCFD_F253_RECORD").as_deref() == Ok("1") || !cfg!(windows) {
        return;
    }
    assert_eq!(h, R2, "R2 {h:016x} differs from the base record {R2:016x}");
}
