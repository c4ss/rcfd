//! F-526 (ADR 0443): the iterative solve of a solid case file to its tolerance, the residual floor, and the node-set box bound.
//! Checks P1 to P7, Q1 to Q5 and the negative controls N1 to N3 of the registration.

use rcfd_api::solid_case::{run_solid_case_file, SolidCase, SolidOutcome, SolidRun};
use rcfd_fem::assemble::stiffness;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::SolidResult;
use std::path::{Path, PathBuf};

const EPS: f64 = f64::EPSILON;
const KEYS: [&str; 6] = [
    "block_jacobi",
    "ic0",
    "ic_fill",
    "amg",
    "amg_quadratic",
    "amg_quadratic_smoothed",
];

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

fn cases() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("solid_cases")
}

fn out_dir(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("sb_solid_case_iterative")
        .join(tag);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn text_of(name: &str) -> String {
    std::fs::read_to_string(cases().join(name)).unwrap()
}

fn run_text(text: &str, tag: &str) -> Result<SolidRun, String> {
    let case = SolidCase::parse_toml_in(text, &cases()).map_err(|e| format!("{e:?}"))?;
    let setup = case.build().map_err(|e| format!("{e:?}"))?;
    let _ = tag;
    rcfd_api::solid_case::run(setup).map_err(|e| e.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))
}

fn run_file(name: &str) -> SolidRun {
    run_solid_case_file(cases().join(name), Some(&out_dir(name))).unwrap_or_else(|e| panic!("{name}: {e:#?}"))
}

fn static_of(r: &SolidRun) -> &SolidResult {
    match &r.outcome {
        SolidOutcome::Static { result, .. } => result,
        SolidOutcome::Modal(_) => panic!("static expected"),
    }
}

/// Largest `|a - b| / max |b|`.
fn compare(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max((x - y).abs())) / scale
}

fn cg_text(base: &str, key: &str) -> String {
    assert_eq!(base.matches("method = \"direct\"").count(), 1);
    base.replace(
        "method = \"direct\"",
        &format!("method = \"cg\"\npreconditioner = \"{key}\"\ntolerance = 1e-10"),
    )
}

/// `(||K||_inf, ||u||_2, ||b||_2)` of a static run (ADR 0443 arithmetic 2).
fn norms(r: &SolidRun) -> (f64, f64, f64) {
    let k = stiffness(&r.setup.model).unwrap();
    let kn = (0..k.n)
        .map(|i| {
            k.val[k.row_ptr[i]..k.row_ptr[i + 1]]
                .iter()
                .map(|v| v.abs())
                .sum::<f64>()
        })
        .fold(0.0f64, f64::max);
    let u = static_of(r).displacement_values();
    let b = nodal_load_vector(&r.setup.model, &[]).unwrap();
    assert_eq!(u.len(), k.n);
    assert_eq!(b.len(), k.n);
    let n2 = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
    (kn, n2(u), n2(&b))
}

/// Prints the floor and returns `(phi, R)`.
fn floor(name: &str, r: &SolidRun) -> (f64, f64) {
    let (kn, un, bn) = norms(r);
    let ratio = bn / (kn * un);
    let phi = EPS * kn * un / bn;
    println!(
        "FLOOR {name}: ||K||_inf = {kn:.4e}, ||u||_2 = {un:.4e}, ||b||_2 = {bn:.4e}, R = ||b||/(||K|| ||u||) = {ratio:.4e}, \
         floor eps ||K|| ||u|| / ||b|| = {phi:.4e}"
    );
    (phi, ratio)
}

fn residual_of(r: &SolidRun) -> f64 {
    r.report["solver"]["relative_residual"].as_f64().unwrap()
}

#[test]
fn p1_to_p7_n1_n2_iterative_f381_and_floors() {
    let mut ch = Checks::new();
    let base = text_of("cantilever_f381_hex8.toml");
    let direct = run_file("cantilever_f381_hex8.toml");
    let dres = static_of(&direct);
    // P1
    let mesh = direct.setup.model.mesh();
    ch.holds(
        "P1 counts",
        mesh.nodes().len() == 1617
            && mesh.node_set("xmin").map(|s| s.len()) == Some(49)
            && dres.displacement_values().len() == 4851,
        &format!(
            "{} nodes, {} xmin nodes, {} values",
            mesh.nodes().len(),
            mesh.node_set("xmin").unwrap().len(),
            dres.displacement_values().len()
        ),
    );
    let b = nodal_load_vector(&direct.setup.model, &[]).unwrap();
    let fz: f64 = (0..mesh.nodes().len()).map(|n| b[3 * n + 2]).sum();
    ch.at_most("P1 |sum f_z / (-2.25e6) - 1|", (fz / -2.25e6 - 1.0).abs(), 1e-9);
    println!("P1 direct run: relative_residual {}", residual_of(&direct));
    // P2, P3, P6, P7
    let (phi_h, r_h) = floor("F-381 Hex8 cantilever", &direct);
    ch.at_most("P2 floor of the F-381 Hex8 model", phi_h, 5e-11);
    ch.at_least("P3 R of the F-381 Hex8 model", r_h, 4.4e-6);
    let tet = run_file("cantilever_tet10.toml");
    let (phi_t, r_t) = floor("ADR 0438 Tet10 cantilever", &tet);
    ch.at_least("P6 floor of the Tet10 model (lower)", phi_t, 6e-11);
    ch.at_most("P6 floor of the Tet10 model (upper)", phi_t, 6e-9);
    ch.at_least("P7 R_hex8 / R_tet10", r_h / r_t, 100.0);
    println!("measured stall of ADR 0438 on the Tet10 model: 5.7e-10 to 6.2e-10 of |b|");
    // P4, P5
    let du = dres.displacement_values().to_vec();
    let mut ic_fill_u = Vec::new();
    for key in KEYS {
        let t0 = std::time::Instant::now();
        match run_text(&cg_text(&base, key), key) {
            Err(e) => ch.holds(&format!("P4 {key} solve"), false, &e),
            Ok(r) => {
                println!(
                    "P4 {key}: solver {}, iterations {}, restarts {}, shift {}, {:.1} s",
                    r.report["solver"]["name"],
                    r.report["solver"]["iterations"],
                    r.report["solver"]["restarts"],
                    r.report["solver"]["shift"],
                    t0.elapsed().as_secs_f64()
                );
                ch.at_most(&format!("P4 {key} relative residual"), residual_of(&r), 1e-10);
                ch.at_most(
                    &format!("P5 {key} against the direct run"),
                    compare(static_of(&r).displacement_values(), &du),
                    1e-7,
                );
                if key == "ic_fill" {
                    ic_fill_u = static_of(&r).displacement_values().to_vec();
                }
            }
        }
    }
    // N1: the comparison can fail.
    let nu = base.replace("poisson_ratio = 0.3", "poisson_ratio = 0.3000000001");
    let other = run_text(&nu, "n1").unwrap();
    let d = compare(&ic_fill_u, static_of(&other).displacement_values());
    ch.holds(
        "N1 comparison sees nu + 1e-10",
        d > 0.0 && d < 1e-6,
        &format!("{d:.3e}"),
    );
    // N2: the Tet10 model at 1e-10 does not converge.
    let tt = text_of("cantilever_tet10.toml");
    match run_text(&cg_text(&tt, "amg_quadratic"), "n2") {
        Err(e) => ch.holds("N2 Tet10 amg_quadratic at 1e-10", e.contains("not converged"), &e),
        Ok(r) => ch.holds(
            "N2 Tet10 amg_quadratic at 1e-10",
            false,
            &format!("converged, residual {}", residual_of(&r)),
        ),
    }
    ch.finish();
}

fn mesh_nodes_of(case: &SolidCase) -> (Vec<[f64; 3]>, SolidCaseSets) {
    let setup = case.build().unwrap();
    let mesh = setup.model.mesh();
    let sets = SolidCaseSets(
        mesh.node_set_names()
            .map(|n| (n.to_string(), mesh.node_set(n).unwrap().to_vec()))
            .collect(),
    );
    (mesh.nodes().to_vec(), sets)
}

struct SolidCaseSets(Vec<(String, Vec<usize>)>);

impl SolidCaseSets {
    fn get(&self, name: &str) -> &[usize] {
        &self
            .0
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("set {name}"))
            .1
    }
}

fn f3(v: &toml::Value) -> [f64; 3] {
    let a = v.as_array().unwrap();
    let g = |i: usize| a[i].as_float().or_else(|| a[i].as_integer().map(|x| x as f64)).unwrap();
    [g(0), g(1), g(2)]
}

#[test]
fn q1_to_q4_n3_box_bounds_equal_to_coordinates() {
    let mut ch = Checks::new();
    let text = text_of("box_exact_bounds.toml");
    let case = SolidCase::parse_toml_in(&text, &cases()).unwrap();
    let (nodes, sets) = mesh_nodes_of(&case);
    let corner = [8.0f64, 0.0, 0.75];
    for name in ["corner_hi", "corner_lo", "point"] {
        let s = sets.get(name);
        let exact = s.len() == 1 && (0..3).all(|c| nodes[s[0]][c].to_bits() == corner[c].to_bits());
        ch.holds(
            &format!("Q {name} selects the node (8, 0, 0.75)"),
            exact,
            &format!(
                "{} node(s) {:?}",
                s.len(),
                s.iter().map(|&n| nodes[n]).collect::<Vec<_>>()
            ),
        );
    }
    let setup = case.build().unwrap();
    let xmax = setup.model.mesh().node_set("xmax").unwrap().to_vec();
    let face = sets.get("face_hi");
    ch.holds(
        "Q2 face_hi equals xmax",
        face == xmax.as_slice() && face.len() == 49,
        &format!("{} nodes", face.len()),
    );
    // N3: the upper x bound 1e-9 below the coordinate selects none of them.
    let below = text.replace("box_max = [8.0, 2.0, 2.0]", "box_max = [7.999999999, 2.0, 2.0]");
    assert_ne!(below, text);
    let case_n3 = SolidCase::parse_toml_in(&below, &cases()).unwrap();
    let (_, sets_n3) = mesh_nodes_of(&case_n3);
    let hit = sets_n3.get("face_hi").iter().filter(|n| xmax.contains(n)).count();
    ch.holds(
        "N3 bound 1e-9 below the coordinate",
        hit == 0,
        &format!("{hit} of 49 xmax nodes selected"),
    );
    ch.finish();
}

#[test]
fn q5_s_existing_case_files_selection_equals_an_independent_inclusive_filter() {
    let mut ch = Checks::new();
    let mut files: Vec<_> = std::fs::read_dir(cases())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    let (mut n_files, mut n_sets) = (0, 0);
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let raw: toml::Value = toml::from_str(&text).unwrap();
        let case = SolidCase::parse_toml_in(&text, &cases()).unwrap();
        let (nodes, sets) = mesh_nodes_of(&case);
        n_files += 1;
        let Some(table) = raw.get("node_sets").and_then(|t| t.as_table()) else {
            continue;
        };
        for (name, v) in table {
            let (lo, hi) = (f3(&v["box_min"]), f3(&v["box_max"]));
            let expect: Vec<usize> = (0..nodes.len())
                .filter(|&i| (0..3).all(|c| nodes[i][c] >= lo[c] && nodes[i][c] <= hi[c]))
                .collect();
            let got = sets.get(name);
            let f = path.file_name().unwrap().to_string_lossy();
            ch.holds(
                &format!("Q5 {f} set {name}"),
                !got.is_empty() && got == expect.as_slice(),
                &format!("{} node(s)", got.len()),
            );
            n_sets += 1;
        }
    }
    println!("Q5: {n_files} case files, {n_sets} node sets");
    ch.at_least("Q5 case files", n_files as f64, 9.0);
    ch.finish();
}
