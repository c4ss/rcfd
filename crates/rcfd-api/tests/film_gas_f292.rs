//! F-292 (ADR 0206): gas buoyancy and gas shear in the particle wall film, in the case tree and the run path. This file
//! starts with the records R6 and R7 of ADR 0206 (check D2, D3), made on main 8db45fb before any code of F-292; the
//! registered checks CT1 to CT5 and the parts of D2 and D3 that need the code follow them. The limits are those of ADR
//! 0206, fixed before the code ran.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_api::{particle_run, particles};
use rcfd_core::vec3::Vec3;
use rcfd_particles::{
    DragLaw, FilmGas, FilmSpec, Flow, InjKind, InjectorExt, InjectorSpec, Interp, Options, Scheme, SizeDist, Tracker,
    VelMode, WallAction,
};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Case K of ADR 0192 C2 as corrected by its amendment 1 (with `[energy]` and `[initial]`): a box_2d incline (gravity 9.81
/// at 30 degrees from -y), frozen flow at rest, the film on `ymin`, one point injection that lands on the centre of the
/// first `ymin` face.
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
gravity = [4.905, -8.495709211125343, 0.0]
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
dt = 0.01
[particles]
enabled = true
density = 1000.0
drag = "none"
integration = "analytic"
end_time = 20.0
dt = 0.01
[particles.film]
enabled = true
patches = ["ymin"]
viscosity = 1.0e-3
[particles.injection.f]
kind = "point"
position = [1.3452994616207485e-3, 2.0e-3, 2.5e-3]
rate = 730.0
diameter = 1.0e-4
mass_flow = 2.5e-6
"#;

fn parse(t: &str) -> Case {
    Case::parse_toml(t).unwrap_or_else(|e| panic!("{e:?}"))
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f292_{tag}_{}", std::process::id()));
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

/// ADR 0206 R6: FNV-1a 64 of the names and bytes of the particle output files of case K through the run path on main
/// 8db45fb (Windows; the flow and the particles call `exp`, whose last bits may differ elsewhere, ADR 0137).
const R6: u64 = 0x2a6c_2d99_f94d_77dc;

/// ADR 0206 R7: the settings content hash of case K on main 8db45fb (a settings hash has no platform value).
const R7: u64 = 0x9517_5a46_b994_1edc;

/// ADR 0206 R6 (check D2): a film run without the gas keys writes the same particle files as on the base code. Compared
/// on Windows, printed elsewhere (ADR 0137). `RCFD_F292_RECORD=1` prints the value only.
#[test]
fn d2_film_run_outputs_match_the_base_record() {
    let dir = tmp("d2");
    run_path(K_CASE, &dir);
    let (h, names) = particle_files_hash(&dir);
    println!("R6 {h:016x} over {names:?}");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(names.iter().any(|n| n == "particle_film.csv"), "set-up: {names:?}");
    if std::env::var("RCFD_F292_RECORD").as_deref() == Ok("1") || !cfg!(windows) {
        return;
    }
    assert_eq!(h, R6, "R6 {h:016x} differs from the base record {R6:016x}");
}

/// ADR 0206 R7 (check D3): the settings hash of case K is that of the base code. `RCFD_F292_RECORD=1` prints the value
/// only.
#[test]
fn d3_film_case_settings_hash_matches_the_base_record() {
    let h = parse(K_CASE).tree.content_hash();
    println!("R7 {h:016x}");
    if std::env::var("RCFD_F292_RECORD").as_deref() == Ok("1") {
        return;
    }
    assert_eq!(h, R7, "R7 {h:016x} differs from the base record {R7:016x}");
}

// ---- After the code: CT1 to CT5 and the rest of D2, D3 (ADR 0206) ----

/// Case KS of ADR 0206: the floor case, a frozen Couette field `u = (s y, 0, 0)` over the `ymin` floor (gravity normal to
/// it), the film on `ymin` with the gas shear, one point injection that lands on the centre of the first `ymin` face.
const KS_CASE: &str = r#"
title = "wall film KS"
[mesh]
generator = "box_2d"
nx = 10
ny = 2
lx = 0.05
ly = 0.01
[physics]
nu = 1.5e-5
rho = 1.2
gravity = [0.0, -9.81, 0.0]
[energy]
enabled = true
[initial]
temperature = 300.0
velocity_expression_x = "5555.555555555556 [m/s] * y / 1 [m]"
[boundaries.xmin]
velocity = "fixed"
velocity_expression_x = "5555.555555555556 [m/s] * y / 1 [m]"
[boundaries.xmax]
velocity = "fixed"
velocity_expression_x = "5555.555555555556 [m/s] * y / 1 [m]"
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
end_time = 40.0
dt = 0.01
[particles.film]
enabled = true
patches = ["ymin"]
viscosity = 1.0e-3
gas_shear = true
[particles.injection.f]
kind = "point"
position = [2.5e-3, 2.0e-3, 2.5e-3]
rate = 730.0
diameter = 1.0e-4
mass_flow = 2.5e-6
"#;

/// Case KB of ADR 0206: K under a dense gas (250 kg/m^3) with the gas buoyancy.
fn kb_text() -> String {
    K_CASE
        .replace("rho = 1.2", "rho = 250.0")
        .replace("viscosity = 1.0e-3", "viscosity = 1.0e-3\ngas_buoyancy = true")
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

fn rel_v(a: Vec3, b: Vec3) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f64::max) / rcfd_core::vec3::norm(b)
}

/// The mass flow of the injection per unit width (over the 5 mm depth).
const GAMMA: f64 = 2.5e-6 / 5e-3;

/// The buoyant thickness of G1 on the 30-degree incline of K.
fn h_b(rho_g: f64) -> f64 {
    (3.0 * 1e-3 * GAMMA / (1000.0 * (1000.0 - rho_g) * 9.81 * 30f64.to_radians().sin())).cbrt()
}

/// The shear thickness of G2 for `tau = 0.1` Pa.
fn h_s() -> f64 {
    (2.0 * 1e-3 * GAMMA / (1000.0 * 0.1)).sqrt()
}

/// CT1: every invalid input of ADR 0206 S2 is located at its path; the controls have no diagnostic.
#[test]
fn ct1_located_errors_of_the_gas_keys() {
    let both = K_CASE.replace(
        "viscosity = 1.0e-3",
        "viscosity = 1.0e-3\ngas_buoyancy = true\ngas_shear = true",
    );
    let both_false = K_CASE.replace(
        "viscosity = 1.0e-3",
        "viscosity = 1.0e-3\ngas_buoyancy = false\ngas_shear = false",
    );
    let ks_off = KS_CASE.replace("[particles.film]\nenabled = true", "[particles.film]\nenabled = false");
    let controls: Vec<(&str, String)> = vec![
        ("K", K_CASE.to_string()),
        ("KB", kb_text()),
        ("KS", KS_CASE.to_string()),
        ("K with both keys true", both),
        ("K with both keys false", both_false),
        ("KS with the film off", ks_off),
    ];
    for (name, t) in &controls {
        let e = errors(t);
        println!("CT1 control {name}: {e:?}");
        assert!(e.is_empty(), "CT1 control {name}: {e:?}");
    }
    let ymin = "[boundaries.ymin]\nvelocity = \"fixed\"";
    let b = "particles.film.gas_buoyancy";
    let s = "particles.film.gas_shear";
    let cases: Vec<(String, &str)> = vec![
        (kb_text().replace("rho = 250.0", "rho = 1000.0"), b),
        (kb_text().replace("rho = 250.0", "rho = 1200.0"), b),
        (format!("{KS_CASE}[turbulence]\nmodel = \"k_epsilon\"\n"), s),
        (format!("{KS_CASE}[dynamic_mesh]\nenabled = true\n"), s),
        (format!("{KS_CASE}[sources.rot]\ntype = \"rotating_frame\"\n"), s),
        (
            KS_CASE.replace(ymin, "[boundaries.ymin]\nvelocity = \"moving_wall\""),
            s,
        ),
        (
            KS_CASE.replace(ymin, &format!("{ymin}\nvelocity_value = [0.1, 0.0, 0.0]")),
            s,
        ),
        (
            KS_CASE.replace(ymin, &format!("{ymin}\nvelocity_expression_x = \"0.1\"")),
            s,
        ),
        (KS_CASE.replace(ymin, &format!("{ymin}\nvelocity_udf = \"w\"")), s),
    ];
    for (i, (text, path)) in cases.iter().enumerate() {
        let e = errors(text);
        println!(
            "CT1 input {i}: expected at {path}; got {:?}",
            e.iter().map(|d| (&d.path, &d.message)).collect::<Vec<_>>()
        );
        assert!(
            e.iter().any(|d| d.path == *path),
            "CT1 input {i}: no diagnostic at {path}: {e:?}"
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

/// A tracker built by hand from the inputs of KB or KS: the options, the injector (seed 1 * 1_000_003 + 0), its extension,
/// the film and its gas drives that `build_tracker` makes, run on the simulation's cell velocities to `end`.
fn by_hand(sim: &Simulation, rho: f64, g: Vec3, position: Vec3, end: f64, gas: FilmGas) -> Tracker {
    let mesh = sim.geom.mesh.clone();
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
        kind: InjKind::Point(position),
        rate: 730.0,
        t_start: 0.0,
        t_end: end,
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
    c.set_film_gas(gas).unwrap();
    let n = mesh.n_cells();
    let (u, grad) = (sim.velocity().values.clone(), vec![[[0.0; 3]; 3]; n]);
    let flow = Flow {
        u: &u,
        grad: Some(&grad),
        rho,
        mu: rho * 1.5e-5,
    };
    while c.time < end * (1.0 - 1e-12) {
        let h = 0.01f64.min(end - c.time);
        c.advance(&flow, h).unwrap();
    }
    c
}

/// CT2, CT3 for one text: `track_frozen`, the run path's `steady_run` and the tracker by hand give the same bits; the
/// row-6 face is within 5 percent and within 1e-6 of `h` at the end time.
fn ct2_ct3(name: &str, text: &str, by: impl Fn(&Simulation) -> Tracker, h: f64) {
    let case = parse(text);
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let a = particles::track_frozen(&case, &sim).unwrap_or_else(|e| panic!("{e:?}"));
    let b = particle_run::steady_run(&sim, None).unwrap_or_else(|e| panic!("{e:?}"));
    let c = by(&sim);
    let (sa, sb, sc) = (state_of(&a), state_of(&b.tracker), state_of(&c));
    println!(
        "CT2 {name}: fingerprint lengths {} {} {}; equal a=b {}, a=c {}",
        sa.0.len(),
        sb.0.len(),
        sc.0.len(),
        sa == sb,
        sa == sc
    );
    assert!(sa == sb && sa == sc, "CT2 {name}: the three trackers differ");
    let film = a.film().unwrap();
    let k6 = (0..film.len())
        .find(|&k| (film.centre(k)[0] - 0.0325).abs() < 1e-9)
        .expect("the row-6 face");
    let e = rel(film.thickness(k6), h);
    println!(
        "CT3 {name}: row-6 thickness {:e} against {h:e}, relative {e:e}; injected {}, absorbed {}, largest Re_f {:.4}, gas {:?}, opposed {}",
        film.thickness(k6),
        a.injected(),
        a.count(rcfd_particles::State::Film),
        film.max_reynolds(),
        film.gas(),
        film.opposed()
    );
    assert!(e <= 0.05 && e <= 1e-6, "CT3 {name}");
}

/// CT2, CT3 on KB: the buoyant film in the tree equals the library and meets the buoyant thickness.
#[test]
fn ct2_ct3_kb_tree_equals_library_and_meets_the_buoyant_thickness() {
    let gas = FilmGas {
        buoyancy: true,
        shear: false,
    };
    let g = [4.905, -8.495709211125343, 0.0];
    let by = |sim: &Simulation| by_hand(sim, 250.0, g, [1.3452994616207485e-3, 2.0e-3, 2.5e-3], 20.0, gas);
    ct2_ct3("KB", &kb_text(), by, h_b(250.0));
}

/// CT2, CT3 on KS: the shear-driven film in the tree equals the library and meets the shear thickness.
#[test]
fn ct2_ct3_ks_tree_equals_library_and_meets_the_shear_thickness() {
    let gas = FilmGas {
        buoyancy: false,
        shear: true,
    };
    let by = |sim: &Simulation| by_hand(sim, 1.2, [0.0, -9.81, 0.0], [2.5e-3, 2.0e-3, 2.5e-3], 40.0, gas);
    ct2_ct3("KS", KS_CASE, by, h_s());
}

/// CT4: the shear the tree's film takes in KS is the wall shear of the post-processing and the 0.1 Pa of the Couette field.
#[test]
fn ct4_tree_shear_equals_the_wall_shear_of_the_post_processing() {
    let case = parse(KS_CASE);
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let tr = particles::track_frozen(&case, &sim).unwrap_or_else(|e| panic!("{e:?}"));
    let film = tr.film().unwrap();
    let u = sim.velocity();
    let bv = u.boundary_values(&sim.geom);
    let ws = rcfd_post::derived::wall_shear(&sim.geom, "ymin", &u.values, &bv, 1.2, 1.5e-5).unwrap();
    assert_eq!(ws.len(), film.len());
    let big = ws.iter().map(|t| rcfd_core::vec3::norm(*t)).fold(0.0, f64::max);
    let mut e_post = 0.0f64;
    let mut bits = true;
    let mut e_nominal = 0.0f64;
    for (k, w) in ws.iter().enumerate() {
        let t = film.shear(k);
        for i in 0..3 {
            e_post = e_post.max((t[i] - w[i]).abs() / big);
            bits &= t[i].to_bits() == w[i].to_bits();
        }
        e_nominal = e_nominal.max(rel_v(t, [0.1, 0.0, 0.0]));
    }
    println!("CT4: against wall_shear {e_post:e} (bits equal {bits}), against (0.1, 0, 0) {e_nominal:e}");
    assert!(e_post <= 1e-15 && e_nominal <= 1e-12, "CT4");
}

/// CT5: the gas fields of the report and the film CSV of the run path, for KB and KS.
#[test]
fn ct5_report_and_film_csv_with_the_gas_drives() {
    let base = "face,patch,x,y,z,area,thickness,mass,ux,uy,uz";
    for (name, text) in [("KB", kb_text()), ("KS", KS_CASE.to_string())] {
        let dir = tmp(&format!("ct5_{name}"));
        let sim = run_path(&text, &dir);
        let run = particle_run::steady_run(&sim, None).unwrap_or_else(|e| panic!("{e:?}"));
        let film = run.tracker.film().unwrap();
        let rep: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("particle_report.json")).unwrap()).unwrap();
        let g = &rep["film"]["gas"];
        println!("CT5 {name}: report film.gas {g}");
        assert_eq!(g["buoyancy"].as_bool(), Some(film.gas().buoyancy), "CT5 {name}");
        assert_eq!(g["shear"].as_bool(), Some(film.gas().shear), "CT5 {name}");
        assert_eq!(g["gas_density"].as_f64(), Some(film.gas_density()), "CT5 {name}");
        assert_eq!(g["max_shear"].as_f64(), Some(film.max_shear()), "CT5 {name}");
        assert_eq!(g["opposed"].as_u64(), Some(film.opposed()), "CT5 {name}");
        let csv = std::fs::read_to_string(dir.join("particle_film.csv")).unwrap();
        let mut lines = csv.lines();
        let header = lines.next().unwrap();
        let rows: Vec<&str> = lines.collect();
        assert_eq!(rows.len(), film.len(), "CT5 {name} rows");
        if film.gas().shear {
            assert_eq!(header, format!("{base},tx,ty,tz"), "CT5 {name} header");
            for (k, r) in rows.iter().enumerate() {
                let c: Vec<&str> = r.split(',').collect();
                let t: Vec<f64> = c[11..14].iter().map(|x| x.parse().unwrap()).collect();
                assert_eq!(t, film.shear(k).to_vec(), "CT5 {name} row {k}");
            }
        } else {
            assert_eq!(header, base, "CT5 {name} header");
        }
        println!("CT5 {name}: header {header}, {} rows", rows.len());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// ADR 0206 D2 after the code: K with both keys written false writes the files of R6 (compared on Windows, printed
/// elsewhere).
#[test]
fn d2b_film_run_with_the_keys_false_matches_the_base_record() {
    let dir = tmp("d2b");
    let text = K_CASE.replace(
        "viscosity = 1.0e-3",
        "viscosity = 1.0e-3\ngas_buoyancy = false\ngas_shear = false",
    );
    run_path(&text, &dir);
    let (h, names) = particle_files_hash(&dir);
    println!("D2b {h:016x} over {names:?}");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(names.iter().any(|n| n == "particle_film.csv"), "set-up: {names:?}");
    if !cfg!(windows) {
        return;
    }
    assert_eq!(h, R6, "D2b {h:016x} differs from R6 {R6:016x}");
}

/// ADR 0206 D3 after the code: K with both keys written false has the hash R7; each key set to true changes it.
#[test]
fn d3b_settings_hash_with_the_gas_keys() {
    let with = |extra: &str| {
        parse(&K_CASE.replace("viscosity = 1.0e-3", &format!("viscosity = 1.0e-3\n{extra}")))
            .tree
            .content_hash()
    };
    let (f, s, b) = (
        with("gas_buoyancy = false\ngas_shear = false"),
        with("gas_shear = true"),
        with("gas_buoyancy = true"),
    );
    println!("D3b: both false {f:016x}, gas_shear {s:016x}, gas_buoyancy {b:016x} (R7 {R7:016x})");
    assert!(f == R7 && s != R7 && b != R7, "D3b");
}
