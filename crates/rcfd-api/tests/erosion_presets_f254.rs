//! ADR 0197 (F-254 follow-up): the named steel preset of the erosion constants in the case tree and the run path.
//! Registered checks G3 to G8: the preset equals the same constants written explicitly, bit for bit; explicit keys
//! override it; located errors; the records in the report and `particle_erosion.csv`; the provenance warning in the docs;
//! the settings hashes of the record R5, made on main 2f60baf before any code of ADR 0197. The limits are those of
//! ADR 0197, fixed before this code ran.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_api::{particle_run, particles};
use rcfd_particles::{Tracker, PRESET_PROVENANCE};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const NAME: &str = "dnv_o501_2007_steel";
/// The erosion lines of case D as ADR 0194 writes them.
const EXPLICIT: &str = "k = 2.0e-9\nn = 2.6\ndensity = 7800.0\n";
/// The preset line.
const PRESET: &str = "material = \"dnv_o501_2007_steel\"\n";

/// Case D of ADR 0194 (the plate `ymin` of a `box_2d` hit by three streams) up to the erosion table, whose lines after
/// `patches` follow; then the injections of ADR 0194 D, written as there.
const D_HEAD: &str = r#"
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
[particles.walls.xmin]
interaction = "escape"
[particles.walls.xmax]
interaction = "escape"
[particles.walls.ymax]
interaction = "escape"
[particles.erosion]
enabled = true
patches = ["ymin"]
"#;

/// The streams of ADR 0194 B in the plane `z = 0.005`: (name, angle [deg], target x, rate [1/s]).
const STREAMS: [(&str, f64, f64, f64); 3] = [
    ("s1", 15.0, 0.045, 1000.0),
    ("s2", 35.0, 0.035, 2000.0),
    ("s3", 90.0, 0.015, 500.0),
];
const U: f64 = 20.0;
const D: f64 = 2e-4;
const Z: f64 = 0.005;

/// Start point and velocity of a stream: `L = 0.03` m before its target on `ymin`.
fn stream(alpha_deg: f64, x_t: f64) -> ([f64; 3], [f64; 3]) {
    let a = alpha_deg.to_radians();
    let dir = [a.cos(), -a.sin(), 0.0];
    let l = 0.03;
    ([x_t - l * dir[0], -l * dir[1], Z], [U * dir[0], U * dir[1], 0.0])
}

/// Case D with the erosion lines `erosion` (after `patches`) and the injections, as ADR 0194's `d_text` writes them.
fn d_case(erosion: &str) -> String {
    let mut t = format!("{D_HEAD}{erosion}");
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

/// `particles::track_frozen` of a case text.
fn frozen(t: &str) -> Tracker {
    let case = parse(t);
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    particles::track_frozen(&case, &sim).unwrap_or_else(|e| panic!("{e:?}"))
}

/// The report of `particle_run::steady_run` of a case text.
fn report(t: &str) -> Value {
    let case = parse(t);
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    particle_run::steady_run(&sim, None)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .report(false)
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_adr0197_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The run path of the library (`drive` plus `write_results`) into `dir`.
fn run_path(t: &str, dir: &Path) {
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
}

/// The run path of a case text: (`particle_erosion.csv`, `particle_report.json`, `particles.vtp`).
fn run_files(t: &str, tag: &str) -> (String, Value, Vec<u8>) {
    let dir = tmp(tag);
    run_path(t, &dir);
    let csv = std::fs::read_to_string(dir.join("particle_erosion.csv")).unwrap();
    let rep = serde_json::from_str(&std::fs::read_to_string(dir.join("particle_report.json")).unwrap()).unwrap();
    let vtp = std::fs::read(dir.join("particles.vtp")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    (csv, rep, vtp)
}

/// The text after the first `n` lines.
fn after_lines(s: &str, n: usize) -> &str {
    let mut rest = s;
    for _ in 0..n {
        rest = &rest[rest.find('\n').map_or(rest.len(), |i| i + 1)..];
    }
    rest
}

/// G3: case D with the preset instead of the three constants equals case D, bit for bit, through `track_frozen` and the
/// run path.
#[test]
fn g3_tree_preset_equals_explicit_bit_for_bit() {
    let (tp, te) = (d_case(PRESET), d_case(EXPLICIT));
    let (fp, fe) = (frozen(&tp).fingerprint(), frozen(&te).fingerprint());
    let (csv_p, mut rep_p, vtp_p) = run_files(&tp, "g3p");
    let (csv_e, rep_e, vtp_e) = run_files(&te, "g3e");
    let material = rep_p["erosion"].as_object_mut().unwrap().remove("material");
    println!(
        "G3: fingerprint words {} and {}, equal {}; csv rows {} and {}, equal after the preamble {}; vtp equal {}; report equal without erosion.material {}; material {material:?}",
        fp.len(),
        fe.len(),
        fp == fe,
        csv_p.lines().count(),
        csv_e.lines().count(),
        after_lines(&csv_p, 3) == csv_e,
        vtp_p == vtp_e,
        rep_p == rep_e
    );
    assert!(rep_e["erosion"]["hits"].as_u64().unwrap_or(0) > 0, "G3 set-up");
    assert!(material.is_some(), "G3 set-up: the preset run records the preset");
    assert!(fp == fe, "G3 (i) fingerprints");
    assert_eq!(after_lines(&csv_p, 3), csv_e, "G3 (ii) csv");
    assert!(vtp_p == vtp_e, "G3 (ii) vtp");
    assert_eq!(rep_p, rep_e, "G3 (ii) report");
}

/// G4: explicit keys override the preset: (a) the density, (b) k and n, (c) all three.
#[test]
fn g4_explicit_keys_override_the_preset() {
    let preset_alone = frozen(&d_case(PRESET)).fingerprint();
    let sets: [(&str, [f64; 3], [&str; 3]); 3] = [
        (
            "density = 2700.0\n",
            [2.0e-9, 2.6, 2700.0],
            ["preset", "preset", "case"],
        ),
        (
            "k = 3.0e-9\nn = 2.4\n",
            [3.0e-9, 2.4, 7800.0],
            ["case", "case", "preset"],
        ),
        (
            "k = 3.0e-9\nn = 2.4\ndensity = 2700.0\n",
            [3.0e-9, 2.4, 2700.0],
            ["case", "case", "case"],
        ),
    ];
    for (i, (lines, want, from)) in sets.iter().enumerate() {
        let tp = d_case(&format!("{PRESET}{lines}"));
        let te = d_case(&format!(
            "k = {:?}\nn = {:?}\ndensity = {:?}\n",
            want[0], want[1], want[2]
        ));
        let (a, b) = (frozen(&tp), frozen(&te));
        let er = a.erosion().unwrap();
        let (fa, fb) = (a.fingerprint(), b.fingerprint());
        let m = &report(&tp)["erosion"]["material"];
        println!(
            "G4 ({}): k {:e}, n {}, density {}; fingerprint equal to the explicit case {}, to the preset alone {}; report material {m}",
            ["a", "b", "c"][i],
            er.k(),
            er.n(),
            er.density(),
            fa == fb,
            fa == preset_alone
        );
        assert!(
            er.k() == want[0] && er.n() == want[1] && er.density() == want[2],
            "G4 {i}: values"
        );
        assert!(fa == fb, "G4 {i}: fingerprint against the explicit case");
        if i == 1 {
            assert!(fa != preset_alone, "G4 (b): the override has no effect");
        }
        assert_eq!(m["preset"], NAME, "G4 {i}: preset");
        for (k, f) in ["k", "n", "density"].iter().zip(from) {
            assert_eq!(m["from"][k], *f, "G4 {i}: from.{k}");
        }
    }
}

/// G5: located errors of the preset key and of a partial override; the controls have none.
#[test]
fn g5_located_errors_of_the_preset() {
    for lines in [
        "",
        "density = 2700.0\n",
        "k = 3.0e-9\nn = 2.4\n",
        "k = 3.0e-9\nn = 2.4\ndensity = 2700.0\n",
    ] {
        let e = errors(&d_case(&format!("{PRESET}{lines}")));
        assert!(e.is_empty(), "G5 control {lines:?}: {e:?}");
    }
    let cases = [
        ("material = \"steel\"\n", "particles.erosion.material"),
        ("material = \"\"\n", "particles.erosion.material"),
        ("k = 3.0e-9\n", "particles.erosion.n"),
        ("n = 2.4\n", "particles.erosion.k"),
        ("k = 0.0\nn = 2.4\n", "particles.erosion.k"),
        ("k = 3.0e-9\nn = 0.0\n", "particles.erosion.n"),
        ("density = -2.0\n", "particles.erosion.density"),
    ];
    for (i, (lines, path)) in cases.iter().enumerate() {
        let text = if lines.starts_with("material") {
            d_case(lines)
        } else {
            d_case(&format!("{PRESET}{lines}"))
        };
        let e = errors(&text);
        println!(
            "G5 input {i} {lines:?}: expected at {path}; got {:?}",
            e.iter().map(|d| (&d.path, &d.message)).collect::<Vec<_>>()
        );
        assert!(
            e.iter().any(|d| d.path == *path),
            "G5 input {i}: no diagnostic at {path}: {e:?}"
        );
    }
}

/// G6: the records of the preset in the report and the CSV; none without it; the provenance in the schema doc.
#[test]
fn g6_records_of_the_preset() {
    let header = "face,patch,x,y,z,area,hits,impact_mass,eroded,rate,depth_rate";
    let l1 = "# erosion preset dnv_o501_2007_steel: steel grades, DNV RP O501 Rev 4.2-2007 Table 7-2";
    let l3 = format!("# {PRESET_PROVENANCE}");
    let tp = d_case(PRESET);
    let tr = frozen(&tp);
    let er = tr.erosion().unwrap();
    let (csv, rep, _) = run_files(&tp, "g6p");
    let m = &rep["erosion"]["material"];
    println!(
        "G6 report material {m}\nG6 csv head:\n{}",
        csv.lines().take(4).collect::<Vec<_>>().join("\n")
    );
    assert_eq!(m["preset"], NAME);
    assert_eq!(m["row"], "steel grades");
    assert_eq!(m["source"], "DNV RP O501 Rev 4.2-2007 Table 7-2");
    assert_eq!(m["k"].as_f64(), Some(er.k()));
    assert_eq!(m["n"].as_f64(), Some(er.n()));
    assert_eq!(m["density"].as_f64(), Some(er.density()));
    for k in ["k", "n", "density"] {
        assert_eq!(m["from"][k], "preset", "G6 from.{k}");
    }
    assert_eq!(m["provenance"], PRESET_PROVENANCE);
    let head: Vec<&str> = csv.lines().take(4).collect();
    assert_eq!(
        head,
        vec![
            l1,
            "# values used: k = 2e-9 (preset), n = 2.6 (preset), density = 7800.0 (preset)",
            l3.as_str(),
            header
        ],
        "G6 csv preamble"
    );
    // G4 (a) through the run path
    let (csv_a, _, _) = run_files(&d_case(&format!("{PRESET}density = 2700.0\n")), "g6a");
    let second = csv_a.lines().nth(1).unwrap_or_default();
    println!("G6 (a) second line: {second}");
    assert_eq!(
        second,
        "# values used: k = 2e-9 (preset), n = 2.6 (preset), density = 2700.0 (case)"
    );
    // without the key
    let (csv_e, rep_e, _) = run_files(&d_case(EXPLICIT), "g6e");
    assert!(
        rep_e["erosion"].get("material").is_none(),
        "G6: material without a preset"
    );
    assert_eq!(
        csv_e.lines().next(),
        Some(header),
        "G6: the header first without a preset"
    );
    let case = parse(&tp);
    let doc = &case.tree.schema_at("particles.erosion.material").unwrap().doc;
    println!("G6 schema doc: {doc}");
    assert!(doc.contains(PRESET_PROVENANCE), "G6 schema doc");
}

/// G7: the preset name and the provenance warning in the docs (white space collapsed).
#[test]
fn g7_docs_carry_the_provenance_warning() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    for f in [
        "particles.md",
        "case-schema.md",
        "reference-data.md",
        "adr/0197-f254-named-steel-preset-of-the-erosion-constants.md",
    ] {
        let text = std::fs::read_to_string(root.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for phrase in [
            NAME,
            "has not been checked against the maintainer's edition",
            "indicative, not certified",
        ] {
            println!("G7 {f}: '{phrase}' {}", flat.contains(phrase));
            assert!(flat.contains(phrase), "G7: {f} lacks '{phrase}'");
        }
    }
}

/// The R4 case of ADR 0194 (`wall_interaction_f254.rs`), copied as it is.
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

/// R5 of ADR 0197, recorded on main 2f60baf before any code of ADR 0197: the settings hash of case D.
const R5_D: u64 = 0x03a6_467e_53b5_0ec2;
/// R5: the settings hash of the R4 case, with or without an inert erosion table.
const R5_R4: u64 = 0x4b83_0c0b_b797_6b35;

/// G8 (iii): the settings hashes of R5 are unchanged, and writing `material = "none"` keeps them.
#[test]
fn g8_settings_hashes_without_a_preset_are_unchanged() {
    let h = |t: &str| parse(t).tree.content_hash();
    let inert = format!("{R4_CASE}\n[particles.erosion]\nenabled = false\n");
    let inert_none = format!("{inert}material = \"none\"\n");
    let d_none = d_case(&format!("{EXPLICIT}material = \"none\"\n"));
    let got = [
        ("case D", h(&d_case(EXPLICIT)), R5_D),
        ("case D with material = none", h(&d_none), R5_D),
        ("R4 case", h(R4_CASE), R5_R4),
        ("R4 case with an inert erosion table", h(&inert), R5_R4),
        ("R4 case with an inert table and material = none", h(&inert_none), R5_R4),
    ];
    for (what, v, want) in got {
        println!("G8 {what}: {v:016x} (R5 {want:016x})");
    }
    let data = parse(&d_case(EXPLICIT)).tree.to_json();
    println!(
        "G8 case D data: particles.erosion.material = {}",
        data["particles"]["erosion"]["material"]
    );
    assert!(
        parse(&inert_none).tree.to_json()["particles"].get("erosion").is_some(),
        "G8 set-up: the table was not read"
    );
    for (what, v, want) in got {
        assert_eq!(v, want, "G8 {what}");
    }
}
