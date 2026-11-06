//! F-254 (ADR 0194): particle wall interaction, erosion and rough-wall rebound, in the case tree and the run path.
//! Registered checks D1 to D4, E2 (the record R4 of the particle output files of a steady run with per-patch wall actions,
//! made on the base code main 54ec840 before any code of F-254) and E4. The limits are those of ADR 0194, fixed before
//! this code ran.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_api::{particle_run, particles};
use rcfd_particles::{
    DragLaw, ErosionSpec, Flow, InjKind, InjectorSpec, Interp, Options, RoughSpec, Scheme, SizeDist, State, Tracker,
    VelMode, WallAction,
};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The streams of ADR 0194 B in the plane `z = 0.005`: (name, angle [deg], target x, rate [1/s]).
const STREAMS: [(&str, f64, f64, f64); 3] = [
    ("s1", 15.0, 0.045, 1000.0),
    ("s2", 35.0, 0.035, 2000.0),
    ("s3", 90.0, 0.015, 500.0),
];
const U: f64 = 20.0;
const D: f64 = 2e-4;
const RHO: f64 = 2650.0;
const K: f64 = 2.0e-9;
const N: f64 = 2.6;
const RHO_T: f64 = 7800.0;
const Z: f64 = 0.005;

/// Start point and velocity of a stream: `L = 0.03` m before its target on `ymin`.
fn stream(alpha_deg: f64, x_t: f64) -> ([f64; 3], [f64; 3]) {
    let a = alpha_deg.to_radians();
    let dir = [a.cos(), -a.sin(), 0.0];
    let l = 0.03;
    ([x_t - l * dir[0], -l * dir[1], Z], [U * dir[0], U * dir[1], 0.0])
}

/// Case D of ADR 0194: the plate `ymin` of a `box_2d` hit by the streams of B, frozen flow at rest, erosion on `ymin`.
/// `{particles}`, `{erosion}` and `{rough}` take extra lines; `{walls}` comes before the escape entries.
const D_CASE: &str = r#"
title = "wall interaction D"
[mesh]
generator = "box_2d"
nx = 6
ny = 4
lx = 0.06
ly = 0.04
[physics]
nu = 1.5e-5
rho = 1.2
[energy]
enabled = true
[initial]
temperature = 300.0
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
dt = 0.001
[particles]
enabled = true
density = 2650.0
drag = "none"
integration = "analytic"
gravity = false
end_time = 0.6
dt = 0.001
{particles}
{walls}
[particles.walls.xmin]
interaction = "escape"
[particles.walls.xmax]
interaction = "escape"
[particles.walls.ymax]
interaction = "escape"
[particles.erosion]
enabled = true
patches = ["ymin"]
k = 2.0e-9
n = 2.6
density = 7800.0
{erosion}
{rough}
"#;

/// The rough wall of D2.
const ROUGH: &str = "[particles.rough_wall]\nenabled = true\npatches = [\"ymin\"]\nangle_std = 8.0\nseed = 254";

/// Case D with replacements: (placeholder, text) pairs; placeholders not given are emptied. The injections follow.
fn d_text(subs: &[(&str, &str)]) -> String {
    let mut t = D_CASE.to_string();
    for (k, v) in subs {
        t = t.replace(k, v);
    }
    for k in ["{particles}", "{walls}", "{erosion}", "{rough}"] {
        t = t.replace(k, "");
    }
    for (name, a, x, r) in STREAMS {
        let (p, v) = stream(a, x);
        t.push_str(&format!(
            "[particles.injection.{name}]\nkind = \"point\"\nposition = [{:?}, {:?}, {:?}]\nrate = {r:?}\nend_time = 0.5\nvelocity = [{:?}, {:?}, {:?}]\ndiameter = {D:?}\n",
            p[0], p[1], p[2], v[0], v[1], v[2]
        ));
    }
    t
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

/// DNV RP O501 eq. 7.2 as printed, with `alpha` in degrees and the constants of Table 7-1 typed here again.
fn f_dnv_deg(alpha_deg: f64) -> f64 {
    let a = [9.370, 42.295, 110.864, 175.804, 170.137, 98.398, 31.211, 4.170];
    let x = alpha_deg * PI / 180.0;
    (0..8)
        .map(|i| {
            let s = if i % 2 == 0 { 1.0 } else { -1.0 };
            s * a[i] * x.powi(i as i32 + 1)
        })
        .sum()
}

/// D1: every invalid input of ADR 0194 S2 is located at its path; case D, with and without the rough wall, has no
/// diagnostic.
#[test]
fn d1_located_errors_of_the_erosion_and_rough_wall_keys() {
    for t in [d_text(&[]), d_text(&[("{rough}", ROUGH)])] {
        let e = errors(&t);
        assert!(e.is_empty(), "valid D: {e:?}");
    }
    let r = |subs: &[(&str, &str)]| {
        let mut v = vec![("{rough}", ROUGH)];
        v.extend_from_slice(subs);
        d_text(&v)
    };
    let film = "[particles.film]\nenabled = true\npatches = [\"ymin\"]\nviscosity = 1.0e-3";
    let cases: Vec<(String, &str)> = vec![
        (
            r(&[]).replace("enabled = true\ndensity = 2650.0", "enabled = false\ndensity = 2650.0"),
            "particles.erosion.enabled",
        ),
        (
            r(&[]).replace("enabled = true\ndensity = 2650.0", "enabled = false\ndensity = 2650.0"),
            "particles.rough_wall.enabled",
        ),
        (
            r(&[]).replace("patches = [\"ymin\"]\nk =", "patches = []\nk ="),
            "particles.erosion.patches",
        ),
        (
            r(&[]).replace("patches = [\"ymin\"]\nk =", "patches = [\"ymin\", \"ymin\"]\nk ="),
            "particles.erosion.patches",
        ),
        (r(&[("{walls}", film)]), "particles.erosion.patches"),
        (r(&[]).replace("k = 2.0e-9\n", ""), "particles.erosion.k"),
        (r(&[]).replace("n = 2.6\n", "n = 0.0\n"), "particles.erosion.n"),
        (r(&[]).replace("density = 7800.0\n", ""), "particles.erosion.density"),
        (
            r(&[]).replace("patches = [\"ymin\"]\nk =", "patches = [\"nope\"]\nk ="),
            "particles.erosion.patches",
        ),
        (
            r(&[]).replace("patches = [\"ymin\"]\nk =", "patches = [\"frontAndBack\"]\nk ="),
            "particles.erosion.patches",
        ),
        (
            r(&[]).replace("patches = [\"ymin\"]\nangle_std", "patches = []\nangle_std"),
            "particles.rough_wall.patches",
        ),
        (
            r(&[]).replace(
                "patches = [\"ymin\"]\nangle_std",
                "patches = [\"ymin\", \"ymin\"]\nangle_std",
            ),
            "particles.rough_wall.patches",
        ),
        (r(&[("{walls}", film)]), "particles.rough_wall.patches"),
        (
            r(&[]).replace("angle_std = 8.0\n", ""),
            "particles.rough_wall.angle_std",
        ),
        (
            r(&[]).replace("angle_std = 8.0\n", "angle_std = 31.0\n"),
            "particles.rough_wall.angle_std",
        ),
        (
            r(&[("{walls}", "[particles.walls.ymin]\ninteraction = \"stick\"")]),
            "particles.rough_wall.patches",
        ),
        (
            r(&[("{walls}", "[particles.walls.ymin]\ninteraction = \"escape\"")]),
            "particles.rough_wall.patches",
        ),
        (
            r(&[("{particles}", "wall_interaction = \"stick\"")]),
            "particles.rough_wall.patches",
        ),
        (
            r(&[("{particles}", "restitution = 0.0")]),
            "particles.rough_wall.patches",
        ),
        (
            r(&[]).replace("patches = [\"ymin\"]\nangle_std", "patches = [\"nope\"]\nangle_std"),
            "particles.rough_wall.patches",
        ),
        (
            r(&[]).replace(
                "patches = [\"ymin\"]\nangle_std",
                "patches = [\"frontAndBack\"]\nangle_std",
            ),
            "particles.rough_wall.patches",
        ),
    ];
    for (i, (text, path)) in cases.iter().enumerate() {
        let e = errors(text);
        println!(
            "D1 input {i}: expected at {path}; got {:?}",
            e.iter().map(|d| (&d.path, &d.message)).collect::<Vec<_>>()
        );
        assert!(
            e.iter().any(|d| d.path == *path),
            "D1 input {i}: no diagnostic at {path}: {e:?}"
        );
    }
}

/// D2: case D with the rough wall through `track_frozen`, through the run path and as a tracker built by hand from the
/// same inputs gives the same bits.
#[test]
fn d2_tree_equals_library() {
    let case = parse(&d_text(&[("{rough}", ROUGH)]));
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let a = particles::track_frozen(&case, &sim).unwrap_or_else(|e| panic!("{e:?}"));
    let b = particle_run::steady_run(&sim, None).unwrap_or_else(|e| panic!("{e:?}"));
    // by hand: the options, overrides, injectors (seed 1 * 1_000_003 + k), erosion and rough wall build_tracker makes
    let mesh = sim.geom.mesh.clone();
    let opts = Options {
        drag: DragLaw::None,
        scheme: Scheme::Analytic,
        interp: Interp::Gradient,
        gravity: [0.0; 3],
        default_wall: WallAction::Reflect(1.0),
        ..Options::default()
    };
    let overrides: Vec<(String, WallAction)> = ["xmin", "xmax", "ymax"]
        .iter()
        .map(|p| (p.to_string(), WallAction::Escape))
        .collect();
    let mut c = Tracker::new(mesh.clone(), opts, &overrides).unwrap();
    for (k, (name, a, x, r)) in STREAMS.iter().enumerate() {
        let (p, v) = stream(*a, *x);
        c.add_injector(InjectorSpec {
            name: name.to_string(),
            kind: InjKind::Point(p),
            rate: *r,
            t_start: 0.0,
            t_end: 0.5,
            velocity: VelMode::Given(v),
            dist: SizeDist::Constant(D),
            density: RHO,
            parcel_count: 1.0,
            seed: 1_000_003 + k as u64,
        })
        .unwrap();
    }
    c.set_erosion(ErosionSpec {
        patches: vec!["ymin".into()],
        k: K,
        n: N,
        density: RHO_T,
    })
    .unwrap();
    c.set_rough_wall(RoughSpec {
        patches: vec!["ymin".into()],
        angle_std: 8f64.to_radians(),
        seed: 254,
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
    while c.time < 0.6 * (1.0 - 1e-12) {
        let h = 0.001f64.min(0.6 - c.time);
        c.advance(&flow, h).unwrap();
    }
    let (fa, fb, fc) = (a.fingerprint(), b.tracker.fingerprint(), c.fingerprint());
    println!(
        "D2: fingerprint lengths {} {} {}; equal a=b {}, a=c {}; injected {}, escaped {}; rough counts {:?}",
        fa.len(),
        fb.len(),
        fc.len(),
        fa == fb,
        fa == fc,
        a.injected(),
        a.count(State::Escaped),
        a.rough_wall().unwrap().counts()
    );
    assert!(a.rough_wall().unwrap().counts().collisions > 0, "D2 set-up");
    assert!(fa == fb && fa == fc, "D2: the three trackers differ");
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f254_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The rows of `particle_erosion.csv` as numbers (face, x, y, z, area, hits, impact_mass, eroded, rate, depth_rate) and
/// the patch names.
fn erosion_rows(dir: &Path) -> Vec<(Vec<f64>, String)> {
    let csv = std::fs::read_to_string(dir.join("particle_erosion.csv")).unwrap();
    let mut lines = csv.lines();
    assert_eq!(
        lines.next(),
        Some("face,patch,x,y,z,area,hits,impact_mass,eroded,rate,depth_rate")
    );
    lines
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            let mut v = vec![c[0].parse::<f64>().unwrap()];
            v.extend(c[2..].iter().map(|x| x.parse::<f64>().unwrap()));
            (v, c[1].to_string())
        })
        .collect()
}

/// D3 (the acceptance in the tree): the `rate` column of `particle_erosion.csv` for the three target faces against
/// `G K U^n F(alpha)`.
#[test]
fn d3_tree_erosion_rate_equals_the_impact_flux_expression() {
    let dir = tmp("d3");
    run_path(&d_text(&[]), &dir);
    let rep: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("particle_report.json")).unwrap()).unwrap();
    let rows = erosion_rows(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let t = rep["time"].as_f64().unwrap();
    let m = RHO * PI * D.powi(3) / 6.0;
    let mut worst = 0.0f64;
    for (_, a, x, r) in STREAMS {
        let row = rows
            .iter()
            .find(|(v, _)| (v[1] - x).abs() < 1e-9 && v[2].abs() < 1e-12 && (v[3] - Z).abs() < 1e-9)
            .expect("the target face");
        let g = (r * 0.5).floor() * m / (1e-4 * t);
        let expected = g * K * U.powf(N) * f_dnv_deg(a);
        let e = rel(row.0[8], expected);
        println!(
            "D3: {a} deg, face {}: rate {:e}, closed form {expected:e}, relative {e:e}; hits {}",
            row.0[0], row.0[8], row.0[5]
        );
        worst = worst.max(e);
    }
    println!("D3: report counts {}, time {t}", rep["counts"]);
    assert!(worst <= 1e-10, "D3");
}

/// D4: the report objects and the erosion CSV of the run path against the tracker.
#[test]
fn d4_report_and_erosion_csv() {
    let dir = tmp("d4");
    let sim = run_path(&d_text(&[("{rough}", ROUGH)]), &dir);
    let run = particle_run::steady_run(&sim, None).unwrap_or_else(|e| panic!("{e:?}"));
    let tr = &run.tracker;
    let er = tr.erosion().unwrap();
    let rep: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("particle_report.json")).unwrap()).unwrap();
    let rows = erosion_rows(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let (e, rw) = (&rep["erosion"], &rep["rough_wall"]);
    println!("D4 report erosion {e}, rough_wall {rw}, counts {}", rep["counts"]);
    let t = tr.time;
    assert_eq!(rep["time"].as_f64(), Some(t));
    assert_eq!(e["faces"].as_u64(), Some(er.len() as u64));
    assert_eq!(e["hits"].as_u64(), Some(er.total_hits()));
    assert_eq!(e["impact_mass"].as_f64(), Some(er.total_impact_mass()));
    assert_eq!(e["eroded"].as_f64(), Some(er.total_eroded()));
    assert_eq!(e["max_rate"].as_f64(), Some(er.max_rate(t)));
    assert_eq!(e["max_depth_rate"].as_f64(), Some(er.max_depth_rate(t)));
    let c = tr.rough_wall().unwrap().counts();
    assert_eq!(rw["collisions"].as_u64(), Some(c.collisions));
    assert_eq!(rw["redraws"].as_u64(), Some(c.redraws));
    assert_eq!(rw["fallbacks"].as_u64(), Some(c.fallbacks));
    assert_eq!(rw["normal_incidence"].as_u64(), Some(c.normal_incidence));
    assert_eq!(rw["grazing"].as_u64(), Some(c.grazing));
    assert_eq!(rows.len(), er.len());
    assert_eq!(rows.len(), 6, "one row per ymin face");
    let mut worst = 0.0f64;
    for (k, (v, patch)) in rows.iter().enumerate() {
        assert_eq!(v[0] as usize, er.faces()[k]);
        assert_eq!(patch, "ymin");
        let (area, eroded, rate) = (v[4], v[7], v[8]);
        if eroded > 0.0 {
            worst = worst.max(rel(rate, eroded / (area * t)));
        }
    }
    println!("D4 csv: {} rows, rate against eroded/(area time) {worst:e}", rows.len());
    assert!(worst <= 1e-15, "D4");
}

/// E4: a case without the two tables has neither key in its data; writing each with `enabled = false` only keeps the
/// settings hash.
#[test]
fn e4_absent_tables_leave_data_and_hash_unchanged() {
    let without = parse(R4_CASE);
    let data = without.tree.to_json();
    assert!(data["particles"].get("erosion").is_none(), "E4: erosion in the data");
    assert!(
        data["particles"].get("rough_wall").is_none(),
        "E4: rough_wall in the data"
    );
    for table in ["erosion", "rough_wall"] {
        let with = parse(&format!("{R4_CASE}\n[particles.{table}]\nenabled = false\n"));
        println!(
            "E4 {table}: hash without {:016x}, with an inert table {:016x}",
            without.tree.content_hash(),
            with.tree.content_hash()
        );
        assert!(
            with.tree.to_json()["particles"].get(table).is_some(),
            "E4 set-up: the table was not read"
        );
        assert_eq!(without.tree.content_hash(), with.tree.content_hash(), "E4 {table}");
    }
}

/// ADR 0194 R4: FNV-1a 64 of the names and bytes of the particle output files of the R4 run on main 54ec840 (Windows;
/// the flow and the particles call `exp`, whose last bits may differ elsewhere, ADR 0137).
const R4: u64 = 0x5514_d633_2741_d3a4;

/// The R4 case: a steady run on a prescribed shear flow in air (`solver.frozen_flow`, `u = 2 y` [1/s] as in the Couette
/// case of `particles_case.rs`, box 12 x 12), particles tracked once on that field by the run path, with gravity, per-patch
/// wall actions (`ymin` sticks, `ymax` reflects with restitution 0.5, `xmax` lets them escape, `xmin` reflects with 0.9),
/// two injections (Rosin-Rammler sizes with a given velocity; fluid-velocity parcels), the track log and numbered VTP
/// files.
const R4_CASE: &str = r#"
title = "particle run R4"
[mesh]
generator = "box_2d"
nx = 12
ny = 12
lx = 1.0
ly = 1.0
[physics]
nu = 1.5e-5
rho = 1.2
gravity = [0.0, -9.81, 0.0]
[energy]
enabled = true
[initial]
temperature = 300.0
velocity_expression_x = "2 [1/s] * y"
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
velocity_value = [2.0, 0.0, 0.0]
[solver]
algorithm = "simple"
frozen_flow = true
max_iterations = 2
dt = 0.01
[particles]
enabled = true
density = 1500.0
drag = "schiller_naumann"
integration = "rk4"
end_time = 1.0
dt = 0.01
restitution = 0.9
tracks = true
track_every = 5
output_every = 25
[particles.walls.ymin]
interaction = "stick"
[particles.walls.ymax]
interaction = "reflect"
restitution = 0.5
[particles.walls.xmax]
interaction = "escape"
[particles.injection.a]
kind = "point"
position = [0.5, 0.8, 0.04]
rate = 120.0
end_time = 0.6
velocity = [0.8, 2.5, 0.0]
distribution = "rosin_rammler"
rr_d_bar = 2.0e-4
rr_n = 3.0
[particles.injection.b]
kind = "point"
position = [0.2, 0.3, 0.04]
rate = 60.0
velocity_mode = "fluid"
diameter = 1.5e-4
parcel_count = 10.0
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

/// ADR 0194 R4 (check E2): a steady particle run with per-patch wall actions and no F-254 model writes the same files as on
/// the base code. Compared on Windows, printed elsewhere (ADR 0137). `RCFD_F254_RECORD=1` prints the value only.
#[test]
fn e2_wall_actions_run_outputs_match_the_base_record() {
    let dir = tmp("e2");
    run_path(R4_CASE, &dir);
    let (h, names) = particle_files_hash(&dir);
    let report = std::fs::read_to_string(dir.join("particle_report.json")).unwrap_or_default();
    println!("R4 {h:016x} over {names:?}\nR4 report {report}");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(names.len() >= 4, "set-up: {names:?}");
    let rep: serde_json::Value = serde_json::from_str(&report).unwrap();
    let c = &rep["counts"];
    assert!(
        c["deposited"].as_u64().unwrap_or(0) > 0 && c["escaped"].as_u64().unwrap_or(0) > 0,
        "R4 set-up: {c}"
    );
    if std::env::var("RCFD_F254_RECORD").as_deref() == Ok("1") || !cfg!(windows) {
        return;
    }
    assert_eq!(h, R4, "R4 {h:016x} differs from the base record {R4:016x}");
}
