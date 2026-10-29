//! ADR 0142 (wave 48H): the finished example projects are fresh; ADR 0150 (job 48P): in both committed trees.
//!
//! The examples were made by `scripts/make_example_projects.py` (the real solver through the real run service). These
//! tests fail when a committed example goes stale: a case changed after its run, a run record or field store that the
//! readers refuse, a ledger that no longer matches its runs, notes naming files that are gone, or a folder that grew past
//! its registered size cap. The crate already depends on `rcfd-project`, `rcfd-api`, `rcfd-tsstore` and `rcfd-io`, so the
//! tests add no edge to the crate graph. Negative controls tamper a temporary copy.
//!
//! ADR 0150: the projects exist twice. `example-projects/` holds runs built on Windows (ADR 0142); `example-projects-linux/`
//! holds the same inputs with runs built on x86_64 Linux by the same script (in Docker, Ubuntu 24.04). Mesh generators
//! call the C library's libm, whose last bits differ per platform (ADR 0137), so a store's mesh equals the mesh a platform
//! builds from the run's case only in that platform's tree. Every check runs on both trees on every platform, except that
//! one comparison (S5's mesh hash), which is asserted for the tree of this platform and printed for the other. T1-T3
//! check that the trees share their inputs, that each was built on its platform, and that the other platform's stores
//! do not fit this one.

use rcfd_api::case::Case;
use rcfd_api::design::ledger;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_project::records::{hash_bytes, ResidualLine, RunState};
use rcfd_project::{jsonl, Project};
use rcfd_tsstore::{IndexSource, Precision, Reader};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Registered caps (ADR 0142 D7, per tree since ADR 0150): bytes of every file of a project folder (committed runs only),
/// and of a whole tree.
const CAP_TOTAL: u64 = 40_000_000;
const CAP_PROJECTS: [(&str, u64); 2] = [("wind-tunnel", 35_000_000), ("quick-start", 6_000_000)];
const MAX_SNAPSHOTS: usize = 100;
/// The aerofoil cases name a mesh file; every other case uses a generator (ADR 0142 D3, E1 known miss). The two rotated
/// ones set the angle by `[mesh.transform]` (ADR 0155 D9).
const MESH_FILE_CASES: [&str; 4] = [
    "cases/naca0012-aoa4.toml",
    "cases/naca0012-aoa2.toml",
    "cases/naca0012-aoa4-rotated.toml",
    "cases/naca0012-aoa2-rotated.toml",
];
/// The committed trees (ADR 0150): (folder, platform its runs were built on). The Windows tree holds the inputs.
const TREES: [(&str, &str); 2] = [("example-projects", "windows"), ("example-projects-linux", "linux")];
/// The generated run table of a project's `notes/README.md` (the only part of an input a rebuild writes).
const RUNS_BEGIN: &str = "<!-- generated: runs (scripts/make_example_projects.py) -->";
const RUNS_END: &str = "<!-- end of generated runs -->";

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// The tree whose runs were built on this platform (ADR 0150): Windows, or x86_64 Linux; `None` elsewhere (S5's mesh
/// hashes are then printed, not compared, as ADR 0137's per-platform values do).
fn native_tree() -> Option<&'static str> {
    if cfg!(windows) {
        Some(TREES[0].0)
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some(TREES[1].0)
    } else {
        None
    }
}

/// The tree of this platform, or the Windows tree where none was built (the controls tamper a copy of it).
fn examples() -> PathBuf {
    repo().join(native_tree().unwrap_or(TREES[0].0))
}

fn read_json(p: &Path) -> Value {
    let t = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    serde_json::from_str(&t).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The committed runs of each project, from `example-projects/manifest.json`: project -> [(run id, case)].
fn manifest(root: &Path) -> BTreeMap<String, Vec<(String, String)>> {
    let m = read_json(&root.join("manifest.json"));
    let mut out = BTreeMap::new();
    for (name, p) in m["projects"].as_object().expect("projects") {
        let runs = p["runs"]
            .as_array()
            .expect("runs")
            .iter()
            .map(|r| {
                (
                    r["run"].as_str().expect("run id").to_string(),
                    r["case"].as_str().expect("case").to_string(),
                )
            })
            .collect();
        out.insert(name.clone(), runs);
    }
    out
}

fn tree_size(p: &Path) -> u64 {
    if p.is_file() {
        return std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    }
    std::fs::read_dir(p)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| tree_size(&e.path())).sum())
        .unwrap_or(0)
}

/// Bytes of a project folder counting only the committed runs (runs made later by a user are left out, ADR 0142 D6).
fn project_size(dir: &Path, runs: &[(String, String)]) -> u64 {
    let mut total = 0;
    for e in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
        if e.file_name() == "runs" {
            total += runs
                .iter()
                .map(|(id, _)| tree_size(&dir.join("runs").join(id)))
                .sum::<u64>();
        } else {
            total += tree_size(&e.path());
        }
    }
    total
}

/// Problems of one project (empty: fresh). Shared by the checks and their negative controls.
fn project_problems(dir: &Path, runs: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    let p = match Project::open(dir) {
        Ok(p) => p,
        Err(e) => return vec![format!("open: {e}")],
    };
    if let Some(v) = p.migrated_from() {
        out.push(format!("rcfd-project.json: schema_version {v}, migrated in memory"));
    }
    let committed: Vec<&str> = runs.iter().map(|(id, _)| id.as_str()).collect();
    let in_committed = |path: &Path| {
        committed
            .iter()
            .any(|id| path.starts_with(dir.join("runs").join(id)) || path.starts_with(p.root().join("runs").join(id)))
    };
    let foreign_run = |path: &Path| path.starts_with(p.root().join("runs")) && !in_committed(path);
    for i in p.validate().expect("validate") {
        if !foreign_run(&i.path) {
            out.push(format!("validate: {}: {}", i.path.display(), i.message));
        }
    }
    let listed = p.runs().expect("runs");
    for s in &listed.skipped {
        if !foreign_run(&s.path) {
            out.push(format!("skipped run {}: {}", s.path.display(), s.message));
        }
    }
    for (id, case) in runs {
        match listed.runs.iter().find(|r| &r.id == id) {
            None => out.push(format!("run {id} of {case} is not listed")),
            Some(r) => {
                if r.state != RunState::Finished {
                    out.push(format!("run {id}: state {:?}", r.state));
                }
                if &r.case != case {
                    out.push(format!("run {id}: case {} in run.json, {case} in the manifest", r.case));
                }
            }
        }
        let run = dir.join("runs").join(id);
        let snap = std::fs::read(run.join("case.snapshot")).unwrap_or_default();
        match std::fs::read(dir.join(case)) {
            Err(e) => out.push(format!("{case}: {e}")),
            Ok(b) if b != snap => out.push(format!(
                "{case} changed since run {id} (case {}, snapshot {})",
                hash_bytes(&b),
                hash_bytes(&snap)
            )),
            Ok(_) => {}
        }
        match Reader::open(&run.join("fields")) {
            Err(e) => out.push(format!("run {id}: field store: {e}")),
            Ok(mut r) => {
                let idx = read_json(&run.join("fields").join("index.json"));
                let n_idx = idx["snapshots"].as_array().map_or(0, Vec::len);
                if r.snapshots().len() != n_idx {
                    out.push(format!(
                        "run {id}: store has {} snapshots, index.json {n_idx}",
                        r.snapshots().len()
                    ));
                }
                if *r.index_source() != IndexSource::Footer {
                    out.push(format!(
                        "run {id}: store index {:?} (no valid footer)",
                        r.index_source()
                    ));
                }
                if Some(r.n_cells() as u64) != idx["n_cells"].as_u64()
                    || Some(r.n_points() as u64) != idx["n_points"].as_u64()
                {
                    out.push(format!("run {id}: store and index.json disagree on the mesh size"));
                }
                if let Err(e) = r.verify() {
                    out.push(format!("run {id}: store verify: {e}"));
                }
            }
        }
    }
    out
}

/// (run id, case) of the committed runs of one project.
type Committed = Vec<(String, String)>;

/// One example project of one tree.
struct Example {
    /// The tree's folder name (`example-projects`, `example-projects-linux`).
    tree: &'static str,
    name: String,
    dir: PathBuf,
    runs: Committed,
}

impl Example {
    fn label(&self) -> String {
        format!("{}/{}", self.tree, self.name)
    }
}

/// The projects of one tree, from its `manifest.json`.
fn tree_projects(tree: &'static str) -> Vec<Example> {
    let root = repo().join(tree);
    manifest(&root)
        .into_iter()
        .map(|(name, runs)| Example {
            tree,
            dir: root.join(&name),
            name,
            runs,
        })
        .collect()
}

/// Every example project of both trees.
fn all_projects() -> Vec<Example> {
    TREES.into_iter().flat_map(|(t, _)| tree_projects(t)).collect()
}

fn case_files(dir: &Path) -> Vec<String> {
    let p = Project::open(dir).expect("open");
    p.cases().expect("cases")
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), &to).unwrap();
        }
    }
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_w48h_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

// ------------------------------------------------------------------------------------------------ S1, S2: records

/// S1: every project opens with `rcfd-project`'s reader, schema version current, 0 validate issues, 0 skipped runs,
/// every committed run listed as finished with its case, the case unchanged since the run, every field store opens
/// with its footer, verifies, and agrees with its `index.json`.
#[test]
fn s1_projects_open_validate_and_list_every_committed_run() {
    for (tree, _) in TREES {
        let ps = tree_projects(tree);
        assert_eq!(ps.len(), 2, "{tree}: two example projects");
        for ex in &ps {
            let probs = project_problems(&ex.dir, &ex.runs);
            println!("S1 {}: {} runs, {} problems", ex.label(), ex.runs.len(), probs.len());
            assert!(probs.is_empty(), "{}: {probs:#?}", ex.label());
        }
    }
}

/// S1 controls on a copy (of this platform's tree): (a) a changed byte in a run's case.snapshot (hash mismatch), (b) the
/// last snapshot of a store cut off, (c) schema_version 0 (stale) and 2 (newer), (d) a changed case file: each is
/// reported.
#[test]
fn s1_controls_tampered_copies_are_reported() {
    let ex = all_projects()
        .into_iter()
        .find(|e| examples().ends_with(e.tree) && e.name == "quick-start")
        .expect("quick-start");
    let (dir, runs) = (ex.dir, ex.runs);
    let (id, case) = runs[0].clone();
    let fresh = |tag: &str| {
        let d = tmp(tag);
        copy_dir(&dir, &d);
        d
    };
    // (a) bad hash
    let d = fresh("hash");
    let snap = d.join("runs").join(&id).join("case.snapshot");
    let mut b = std::fs::read(&snap).unwrap();
    b[0] ^= 0x20;
    std::fs::write(&snap, &b).unwrap();
    let probs = project_problems(&d, &runs);
    println!("S1 control (a) bad hash: {probs:?}");
    assert!(
        probs
            .iter()
            .any(|p| p.contains("case.snapshot") && p.contains("does not match")),
        "{probs:#?}"
    );
    let _ = std::fs::remove_dir_all(&d);
    // (b) missing snapshot: the store cut at the end of its second-last snapshot (no footer, one snapshot fewer)
    let d = fresh("snap");
    let fields = d.join("runs").join(&id).join("fields");
    let cut = {
        let r = Reader::open(&fields).unwrap();
        let s = r.snapshots();
        s[s.len() - 2].end_offset()
    };
    let store = fields.join(rcfd_tsstore::STORE_FILE);
    let b = std::fs::read(&store).unwrap();
    std::fs::write(&store, &b[..cut as usize]).unwrap();
    let probs = project_problems(&d, &runs);
    println!("S1 control (b) missing snapshot: {probs:?}");
    assert!(probs.iter().any(|p| p.contains("snapshots, index.json")), "{probs:#?}");
    let _ = std::fs::remove_dir_all(&d);
    // (c) stale and newer schema versions
    for v in [0, 2] {
        let d = fresh(&format!("schema{v}"));
        let mpath = d.join("rcfd-project.json");
        let mut m = read_json(&mpath);
        m["schema_version"] = json!(v);
        std::fs::write(&mpath, serde_json::to_vec_pretty(&m).unwrap()).unwrap();
        let probs = project_problems(&d, &runs);
        println!("S1 control (c) schema_version {v}: {probs:?}");
        // 0: the migration hook refuses the manifest (it has no version-0 fields); 2: newer than this library
        assert!(
            probs
                .iter()
                .any(|p| p.contains("schema_version") || p.contains("migration 0 -> 1")),
            "{probs:#?}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
    // (d) a case edited after its run
    let d = fresh("case");
    let cpath = d.join(&case);
    let mut t = std::fs::read_to_string(&cpath).unwrap();
    t.push_str("# edited after the run\n");
    std::fs::write(&cpath, t).unwrap();
    let probs = project_problems(&d, &runs);
    println!("S1 control (d) changed case: {probs:?}");
    assert!(probs.iter().any(|p| p.contains("changed since run")), "{probs:#?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// S2: runtime state and pruned files are not committed: `.rcfd/` holds only its `.gitignore`; no run folder holds the
/// solver stream, a VTU file or a checkpoint.
#[test]
fn s2_no_runtime_state_or_pruned_files() {
    for ex in all_projects() {
        let name = ex.label();
        let dot: Vec<String> = std::fs::read_dir(ex.dir.join(".rcfd"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(dot, vec![".gitignore".to_string()], "{name}/.rcfd");
        for (id, _) in &ex.runs {
            let run = ex.dir.join("runs").join(id);
            assert!(!run.join("solver-status.jsonl").exists(), "{name}/{id}");
            for e in std::fs::read_dir(run.join("output")).unwrap() {
                let f = e.unwrap().file_name().to_string_lossy().into_owned();
                assert!(!f.ends_with(".vtu") && !f.ends_with(".rcfd"), "{name}/{id}/output/{f}");
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ S3: cases

/// S3: every case file validates through rcfd-api with the case's folder as base (`Case::load`, then `rcfd check`'s
/// `runctl::check_only`: exit 0 and no error-level set-up rule), and its mesh builds.
#[test]
fn s3_every_case_validates_through_rcfd_api() {
    for (tree, _) in TREES {
        let mut n = 0;
        for ex in tree_projects(tree) {
            let (name, dir) = (ex.label(), ex.dir);
            for rel in case_files(&dir) {
                let case = Case::load(dir.join(&rel)).unwrap_or_else(|d| panic!("{name}/{rel}: {d:?}"));
                let (code, ev) = rcfd_api::runctl::check_only(&case);
                let errors = ev["rules"]
                    .as_array()
                    .map_or(0, |r| r.iter().filter(|x| x["severity"] == "error").count());
                println!(
                    "S3 {name}/{rel}: exit {code}, cells {}, error rules {errors}",
                    ev["cells"]
                );
                assert!(code == 0 && errors == 0, "{name}/{rel}: exit {code}, {}", ev);
                n += 1;
            }
        }
        // ten of ADR 0142 and the two rotated aerofoil cases of ADR 0155, in each tree
        assert_eq!(n, 12, "{tree}: twelve case files");
    }
}

/// A default service, `session.create` with the text (and `base_dir` when given), `settings.validate`. Returns
/// (opened, valid, diagnostics).
fn text_validate(text: &str, base_dir: Option<&Path>) -> (bool, bool, String) {
    let svc = Service::new(ServiceConfig::default());
    let mut p = json!({ "toml": text });
    if let Some(d) = base_dir {
        p["base_dir"] = json!(d.to_string_lossy());
    }
    let Ok(s) = svc.call("session.create", &p) else {
        return (false, false, String::new());
    };
    let v = svc
        .call("settings.validate", &json!({"session": s["session"]}))
        .expect("settings.validate answers");
    (true, v["ok"] == true, v["diagnostics"].to_string())
}

/// The Studio case editor's calls (`crates/rcfd-studio/src/case_edit.rs`): a default service, `session.create` with the
/// text and the case's folder as `base_dir` (ADR 0144: relative paths resolve against the case's folder),
/// `settings.validate`. Returns (opened, valid, diagnostics).
fn editor_validate(text: &str, case_dir: &Path) -> (bool, bool, String) {
    text_validate(text, Some(case_dir))
}

/// The folder of case `rel` of project folder `dir`.
fn case_dir(dir: &Path, rel: &str) -> PathBuf {
    dir.join(rel).parent().unwrap().to_path_buf()
}

/// E1: the eight generator cases open and validate through the Studio editor's calls.
#[test]
fn e1_generator_cases_validate_through_the_editor_calls() {
    for (tree, _) in TREES {
        let mut n = 0;
        for ex in tree_projects(tree) {
            let (name, dir) = (ex.label(), ex.dir);
            for rel in case_files(&dir)
                .into_iter()
                .filter(|r| !MESH_FILE_CASES.contains(&r.as_str()))
            {
                let (open, ok, diag) =
                    editor_validate(&std::fs::read_to_string(dir.join(&rel)).unwrap(), &case_dir(&dir, &rel));
                println!("E1 {name}/{rel}: open {open}, valid {ok} {diag}");
                assert!(open && ok, "{name}/{rel}: {diag}");
                n += 1;
            }
        }
        assert_eq!(n, 8, "{tree}");
    }
}

/// E1 for the two aerofoil cases (registered limit: every case validates through the editor's calls). They name
/// `../meshes/naca0012-ogrid`; the editor resolved it against the process's working folder until ADR 0144 (this test
/// was the ignored known miss `e1_mesh_file_cases_validate_through_the_editor_calls_known_miss`); its calls now give the
/// case's folder (`base_dir`), as `rcfd run` and the run service do.
#[test]
fn e1_mesh_file_cases_validate_through_the_editor_calls() {
    for ex in all_projects() {
        let (name, dir) = (ex.label(), ex.dir);
        for rel in case_files(&dir)
            .into_iter()
            .filter(|r| MESH_FILE_CASES.contains(&r.as_str()))
        {
            let (open, ok, diag) =
                editor_validate(&std::fs::read_to_string(dir.join(&rel)).unwrap(), &case_dir(&dir, &rel));
            println!("E1 {name}/{rel}: open {open}, valid {ok} {diag}");
            assert!(open && ok, "{name}/{rel}: {diag}");
        }
    }
}

/// E2 (contrast to E1): the same aerofoil cases through `session.create` with the file's path (base: the case's
/// folder, as `rcfd run` and the run service resolve it) validate; through the text call without a base folder (the
/// service's rule for text, and the editor's call before ADR 0144) they open but fail at `mesh.file`.
#[test]
fn e2_mesh_file_cases_validate_when_resolved_against_their_folder() {
    for (tree, _) in TREES {
        let dir = repo().join(tree).join("wind-tunnel");
        for rel in MESH_FILE_CASES {
            let path = dir.join(rel).canonicalize().unwrap();
            let svc = Service::new(ServiceConfig::default());
            let s = svc
                .call("session.create", &json!({"path": path.display().to_string()}))
                .unwrap_or_else(|e| panic!("{tree}/{rel}: {}", e.message));
            let v = svc
                .call("settings.validate", &json!({"session": s["session"]}))
                .unwrap();
            println!("E2 {tree}/{rel} by path: valid {} {}", v["ok"], v["diagnostics"]);
            assert_eq!(v["ok"], true, "{tree}/{rel}: {}", v["diagnostics"]);
            let (open, ok, diag) = text_validate(&std::fs::read_to_string(dir.join(rel)).unwrap(), None);
            println!("E2 {tree}/{rel} by text: open {open}, valid {ok} {diag}");
            assert!(open && !ok && diag.contains("mesh.file"), "{tree}/{rel}: {diag}");
        }
    }
}

/// S4: the aerofoil cases agree with themselves: the angle of the initial velocity equals the far-field velocity's, the
/// lift and drag directions are normal and parallel to it, and it equals the ledger's `alpha` (each tree's ledger). A
/// case that rotates its mesh (ADR 0155) has the flow along +x (lift +y, drag +x) and its angle of attack is
/// `-rotate_z_deg` about the quarter chord.
#[test]
fn s4_aerofoil_angle_of_attack_is_consistent() {
    for (tree, _) in TREES {
        s4_tree(tree);
    }
}

fn s4_tree(tree: &str) {
    let dir = repo().join(tree).join("wind-tunnel");
    let entries = ledger_entries(&dir);
    for rel in MESH_FILE_CASES {
        let case = Case::load(dir.join(rel)).unwrap();
        let v = |p: &str| -> Vec<f64> {
            serde_json::from_value(case.get(p).unwrap_or_else(|e| panic!("{p}: {e}"))).unwrap()
        };
        let u0 = v("initial.velocity");
        let uf = v("boundaries.farfield.velocity_value");
        let lift = v("reports.cl.direction");
        let drag = v("reports.cd.direction");
        let mesh_turn = case.get("mesh.transform.rotate_z_deg").unwrap().as_f64().unwrap();
        let alpha = if mesh_turn != 0.0 {
            // ADR 0155: the mesh turns by -alpha about the quarter chord; the flow and the forces stay on the world axes
            assert_eq!(u0, [1.0, 0.0, 0.0], "{rel}: initial velocity");
            assert_eq!(uf, [1.0, 0.0, 0.0], "{rel}: far field");
            assert_eq!(lift, [0.0, 1.0, 0.0], "{rel}: lift direction");
            assert_eq!(drag, [1.0, 0.0, 0.0], "{rel}: drag direction");
            assert_eq!(
                v("mesh.transform.rotate_origin"),
                [0.25, 0.0, 0.0],
                "{rel}: quarter chord"
            );
            (-mesh_turn).to_radians()
        } else {
            let alpha = u0[1].atan2(u0[0]);
            let (s, c) = alpha.sin_cos();
            let close = |a: &[f64], b: [f64; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 1e-15);
            assert!(
                close(&uf, [u0[0], u0[1], u0[2]]),
                "{rel}: far field {uf:?} vs initial {u0:?}"
            );
            assert!(close(&lift, [-s, c, 0.0]), "{rel}: lift direction {lift:?}");
            assert!(close(&drag, [c, s, 0.0]), "{rel}: drag direction {drag:?}");
            alpha
        };
        let stem = Path::new(rel).file_stem().unwrap().to_string_lossy().into_owned();
        let e = entries.iter().find(|e| e.variant == stem).expect("ledger entry");
        let a = e.param("alpha").expect("alpha").value;
        println!(
            "S4 {tree}/{rel}: alpha {} deg from the case, {a} in the ledger",
            alpha.to_degrees()
        );
        assert!(
            (alpha.to_degrees() - a).abs() <= 1e-12,
            "{tree}/{rel}: {} vs {a}",
            alpha.to_degrees()
        );
    }
}

// ------------------------------------------------------------------------------------------------ S5, S6: runs

/// The case of a run (its `case.snapshot`, base: the case's folder), and the content hash and cell count of the mesh
/// this platform builds from it.
fn run_case_and_mesh(dir: &Path, id: &str, case_rel: &str) -> (Case, u64, usize) {
    let text = std::fs::read_to_string(dir.join("runs").join(id).join("case.snapshot")).unwrap();
    let base = dir.join(case_rel).parent().unwrap().to_path_buf();
    let case = Case::parse_toml_in(&text, &base).unwrap_or_else(|d| panic!("{id}: {d:?}"));
    let mesh = rcfd_api::sim::build_mesh(&case).unwrap_or_else(|d| panic!("{id}: {d:?}"));
    (case, mesh.content_hash(), mesh.n_cells())
}

/// S5: per committed run: the store's mesh hash equals the content hash of the mesh built from `case.snapshot` (base:
/// the case's folder) and its cell count; 1 to 100 snapshots, each holding exactly the case's `output.store_fields` in
/// f32; `residuals.jsonl` reads with no torn tail, steps 1..N with N the step of `status.json`; a transient run ends at
/// its `solver.end_time`; `metrics.json` `reports` equal the last residual line's monitors bit for bit. ADR 0150: the
/// mesh hash is compared for the tree of this platform only (the other tree's stores were built with the other
/// platform's libm; T3 shows they differ); for the other tree it is printed. Every other limit holds for both trees.
#[test]
fn s5_run_stores_residuals_and_metrics_match_their_case() {
    let native = native_tree();
    for ex in all_projects() {
        let (name, dir, runs) = (ex.label(), &ex.dir, &ex.runs);
        for (id, case_rel) in runs {
            let run = dir.join("runs").join(id);
            let (case, mesh_hash, n_cells) = run_case_and_mesh(dir, id, case_rel);
            let mut r = Reader::open(&run.join("fields")).unwrap();
            if native == Some(ex.tree) {
                assert_eq!(r.mesh_hash(), mesh_hash, "{name}/{id}: store mesh hash");
            } else {
                println!(
                    "S5 {name}/{id}: store mesh hash {}, the mesh built here {mesh_hash} (not compared: the stores of {} were built on another platform, ADR 0150)",
                    r.mesh_hash(),
                    ex.tree
                );
            }
            assert_eq!(r.n_cells(), n_cells, "{name}/{id}: cells");
            let n = r.snapshots().len();
            assert!((1..=MAX_SNAPSHOTS).contains(&n), "{name}/{id}: {n} snapshots");
            let want: Vec<String> = serde_json::from_value(case.get("output.store_fields").unwrap()).unwrap();
            for i in 0..n {
                let f = r.fields(i).unwrap();
                let names: Vec<String> = f.iter().map(|x| x.name.clone()).collect();
                assert_eq!(names, want, "{name}/{id}: snapshot {i}");
                assert!(
                    f.iter().all(|x| x.precision == Precision::F32),
                    "{name}/{id}: snapshot {i} not f32"
                );
            }
            let lines = jsonl::read::<ResidualLine>(&run.join("residuals.jsonl")).unwrap();
            assert!(lines.torn.is_none(), "{name}/{id}: torn residual line");
            let steps: Vec<u64> = lines.items.iter().map(|l| l.step).collect();
            let st = read_json(&run.join("status.json"));
            let last = *steps.last().unwrap();
            assert!(steps.iter().copied().eq(1..=last), "{name}/{id}: steps not 1..{last}");
            assert_eq!(Some(last), st["step"].as_u64(), "{name}/{id}: status step");
            let algorithm = case.get("solver.algorithm").unwrap();
            let end = lines.items.last().unwrap();
            if algorithm == "piso" {
                let t_end = case.get("solver.end_time").unwrap().as_f64().unwrap();
                assert_eq!(end.time, t_end, "{name}/{id}: transient end time");
            }
            let met = read_json(&run.join("metrics.json"));
            let reports = met["reports"].as_object().expect("reports");
            assert_eq!(reports.len(), end.monitors.len(), "{name}/{id}: report names");
            for (k, v) in reports {
                assert_eq!(
                    v.as_f64().unwrap().to_bits(),
                    end.monitors[k].to_bits(),
                    "{name}/{id}: report {k}"
                );
            }
            println!(
                "S5 {name}/{id} {case_rel}: {n_cells} cells, {n} snapshots {want:?} f32, {last} steps, end time {}, {} reports",
                end.time,
                reports.len()
            );
        }
    }
}

/// T3 (ADR 0150), the standing control of S5's per-tree comparison: on Windows and on x86_64 Linux, the stores of the
/// other platform's tree do not fit this platform: the store mesh hash of both cylinder runs (the `cylinder_ogrid`
/// generator with its graded spacing calls libm) differs from the mesh built here from their case. Every run's pair is
/// printed. Elsewhere (no tree of its own) nothing is compared.
#[test]
fn t3_the_other_platforms_cylinder_stores_differ_from_the_mesh_built_here() {
    let Some(native) = native_tree() else {
        println!("T3: no tree was built on this platform; nothing compared");
        return;
    };
    let mut cylinders = 0;
    for (tree, _) in TREES.into_iter().filter(|(t, _)| *t != native) {
        for ex in tree_projects(tree) {
            for (id, case_rel) in &ex.runs {
                let (_, built, _) = run_case_and_mesh(&ex.dir, id, case_rel);
                let stored = Reader::open(&ex.dir.join("runs").join(id).join("fields"))
                    .unwrap()
                    .mesh_hash();
                println!(
                    "T3 {}/{id} {case_rel}: stored {stored}, built here {built}: {}",
                    ex.label(),
                    if stored == built { "equal" } else { "differ" }
                );
                if case_rel.contains("cylinder") {
                    assert_ne!(
                        stored,
                        built,
                        "{}/{id}: the other platform's store fits here",
                        ex.label()
                    );
                    cylinders += 1;
                }
            }
        }
    }
    assert_eq!(cylinders, 2, "two cylinder runs in the other tree");
}

fn ledger_entries(dir: &Path) -> Vec<ledger::Entry> {
    let text = std::fs::read_to_string(dir.join("designs").join("variants.ledger.jsonl")).unwrap();
    let c = ledger::parse_text(&text);
    assert!(
        c.skipped.is_empty(),
        "{}: skipped ledger lines {:?}",
        dir.display(),
        c.skipped
    );
    c.entries
}

/// Strouhal number, mean C_D, C_L amplitude and the number of periods, over all whole periods of C_L between the first
/// and the last upward crossing through its mean in the second half of the run (ADR 0142 amendment 1), as
/// `scripts/make_example_projects.py` computes them.
fn shedding(monitors: &Value) -> (f64, f64, f64, usize) {
    let s = &monitors["series"];
    let col = |n: &str, k: &str| -> Vec<f64> { serde_json::from_value(s[n][k].clone()).unwrap() };
    let (t, cl, cd) = (col("cl", "x"), col("cl", "y"), col("cd", "y"));
    let half = t[t.len() - 1] / 2.0;
    let idx: Vec<usize> = (0..t.len()).filter(|&i| t[i] >= half).collect();
    let mean = idx.iter().map(|&i| cl[i]).sum::<f64>() / idx.len() as f64;
    let mut cross = Vec::new();
    for w in idx.windows(2) {
        let (a, b) = (w[0], w[1]);
        if cl[a] - mean < 0.0 && 0.0 <= cl[b] - mean {
            let wgt = (mean - cl[a]) / (cl[b] - cl[a]);
            cross.push(t[a] + wgt * (t[b] - t[a]));
        }
    }
    assert!(cross.len() >= 2, "{} crossings", cross.len());
    let periods = cross.len() - 1;
    let (ta, tb) = (cross[0], cross[cross.len() - 1]);
    let win: Vec<usize> = idx.iter().copied().filter(|&i| ta <= t[i] && t[i] <= tb).collect();
    let mut num = 0.0;
    for w in win.windows(2) {
        num += 0.5 * (cd[w[0]] + cd[w[1]]) * (t[w[1]] - t[w[0]]);
    }
    let cd_mean = num / (t[win[win.len() - 1]] - t[win[0]]);
    let (lo, hi) = win.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &i| {
        (lo.min(cl[i]), hi.max(cl[i]))
    });
    (periods as f64 / (tb - ta), cd_mean, 0.5 * (hi - lo), periods)
}

/// Sign changes of ux along the cell row nearest to `y` (the stored mesh's own cell centres), linear interpolation.
fn row_sign_changes(mesh_centres: &[[f64; 3]], ux: &[f64], y: f64) -> Vec<f64> {
    let mut row: Vec<(f64, f64)> = mesh_centres
        .iter()
        .zip(ux)
        .filter(|(c, _)| (c[1] - y).abs() < 1e-9)
        .map(|(c, u)| (c[0], *u))
        .collect();
    row.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    for w in row.windows(2) {
        let ((x0, a), (x1, b)) = (w[0], w[1]);
        if (a < 0.0 && 0.0 <= b) || (a >= 0.0 && 0.0 > b) {
            out.push(x0 + (-a / (b - a)) * (x1 - x0));
        }
    }
    out
}

/// S6: the design ledger of each project reads with rcfd-api's ledger reader (0 skipped lines, every entry valid), has
/// one entry per committed run with the run's case, snapshot hash and store mesh hash; every metric that is a report
/// equals `metrics.json` bit for bit; the derived metrics are recomputed here from the run's own files (shedding from
/// `output/monitors.json`, reattachment from the last stored `ux` on the stored mesh's cell centres) within 1e-9.
#[test]
fn s6_ledger_matches_the_runs() {
    for ex in all_projects() {
        let (name, dir, runs) = (ex.label(), ex.dir, ex.runs);
        let entries = ledger_entries(&dir);
        assert_eq!(entries.len(), runs.len(), "{name}: one entry per run");
        for (id, case_rel) in &runs {
            let e = entries
                .iter()
                .find(|e| e.settings["run"] == json!(id))
                .unwrap_or_else(|| panic!("{name}: no ledger entry for {id}"));
            e.validate().unwrap_or_else(|m| panic!("{name}/{id}: {m}"));
            let run = dir.join("runs").join(id);
            let rec = read_json(&run.join("run.json"));
            assert_eq!(e.settings["case"], json!(case_rel), "{name}/{id}");
            assert_eq!(
                e.settings["case_snapshot_hash"], rec["case_snapshot_hash"],
                "{name}/{id}"
            );
            let mut r = Reader::open(&run.join("fields")).unwrap();
            assert_eq!(e.geometry_hash, format!("fnv64:{:016x}", r.mesh_hash()), "{name}/{id}");
            let met = read_json(&run.join("metrics.json"));
            let mut derived = Vec::new();
            for q in &e.metrics {
                match met["reports"].get(&q.name) {
                    Some(v) => assert_eq!(
                        v.as_f64().unwrap().to_bits(),
                        q.value.to_bits(),
                        "{name}/{id}: {}",
                        q.name
                    ),
                    None => derived.push(q),
                }
            }
            let mut checked = Vec::new();
            if e.metric("strouhal").is_some() {
                let (st, cd, amp, periods) = shedding(&read_json(&run.join("output").join("monitors.json")));
                assert_eq!(
                    e.metric("periods").map(|q| q.value),
                    Some(periods as f64),
                    "{name}/{id}: periods"
                );
                checked.push(format!("periods {periods}"));
                for (k, v) in [("strouhal", st), ("cd_mean", cd), ("cl_amplitude", amp)] {
                    let q = e.metric(k).unwrap().value;
                    assert!((q - v).abs() <= 1e-9 * v.abs(), "{name}/{id}: {k} {q} vs {v}");
                    checked.push(format!("{k} {v:.5}"));
                }
            }
            if e.metric("x_reattach").is_some() {
                let mesh = r.mesh().unwrap();
                let n = r.snapshots().len();
                let ux = r.read_field(n - 1, "ux").unwrap();
                let cc: Vec<[f64; 3]> = mesh.cell_centres().to_vec();
                let (ylo, yhi) = cc.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), c| {
                    (a.min(c[1]), b.max(c[1]))
                });
                let floor = row_sign_changes(&cc, &ux, ylo);
                let top = row_sign_changes(&cc, &ux, yhi);
                let mut pairs = vec![("x_reattach", *floor.last().unwrap())];
                if top.len() >= 2 {
                    pairs.push(("x_top_separation", top[0]));
                    pairs.push(("x_top_reattach", top[1]));
                }
                for (k, v) in pairs {
                    let q = e.metric(k).unwrap_or_else(|| panic!("{name}/{id}: {k} missing")).value;
                    assert!((q - v).abs() <= 1e-9 * v.abs(), "{name}/{id}: {k} {q} vs {v}");
                    checked.push(format!("{k} {v:.4}"));
                }
            }
            assert_eq!(
                checked.len(),
                derived.len(),
                "{name}/{id}: unchecked derived metrics {derived:?}"
            );
            println!("S6 {name}/{id}: {} metrics, derived {checked:?}", e.metrics.len());
        }
    }
}

// ------------------------------------------------------------------------------------------------ G1-G3: the runs

/// Ledger metric `name` of the entry whose variant is `variant`.
fn metric(entries: &[ledger::Entry], variant: &str, name: &str) -> f64 {
    let e = entries
        .iter()
        .find(|e| e.variant == variant)
        .unwrap_or_else(|| panic!("no ledger entry {variant}"));
    e.metric(name)
        .unwrap_or_else(|| panic!("{variant}: no metric {name}"))
        .value
}

/// G1, G2: every committed run finished with the registered solver state and step; each run's wall time (launched to
/// final state, the ledger's `wall_s`) at most 180 s, the sum at most 20 min (per tree: each tree is one rebuild).
/// ADR 0169: the aerofoil runs regenerated under the far-field default of ADR 0169 meet the residual targets (409 and
/// 230 iterations, exit 0), so their G1 part is the known miss below (limit intact); every other run's G1 and every
/// run's G2 stay here.
#[test]
fn g1_g2_runs_finished_as_registered_within_their_time() {
    for (tree, _) in TREES {
        g1_g2_tree(tree, false);
    }
}

/// KNOWN MISS (ADR 0169 interim note and addendum): G1's aerofoil expectation, solver state `not_converged` at iteration
/// 2000 with exit code 5 (ADR 0142), is missed by the runs regenerated under ADR 0169's far-field default, which meet the
/// residual targets at iteration 409 (4 degrees) and 230 (2 degrees), exit 0. The limit is kept as registered.
#[test]
#[ignore = "KNOWN MISS (ADR 0169): the regenerated aerofoil runs converge at 409 / 230 iterations (exit 0), G1 expects not_converged at 2000, exit 5"]
fn g1_aerofoil_runs_stop_not_converged_at_2000_known_miss() {
    for (tree, _) in TREES {
        g1_g2_tree(tree, true);
    }
}

/// G1 / G2 of one tree. `aerofoils`: false checks G1 of every run except the aerofoils and G2 of every run; true checks
/// only the aerofoils' G1 (the known miss above).
fn g1_g2_tree(tree: &'static str, aerofoils: bool) {
    let mut total = 0.0;
    for ex in tree_projects(tree) {
        let (name, dir, runs) = (ex.label(), ex.dir, ex.runs);
        let entries = ledger_entries(&dir);
        for (id, case_rel) in &runs {
            let naca = case_rel.contains("naca0012");
            let met = read_json(&dir.join("runs").join(id).join("metrics.json"));
            let fin = &met["solver_finish"];
            let (state, steps, exit) = (
                fin["state"].as_str().unwrap_or(""),
                met["steps"].as_u64().unwrap(),
                met["exit_code"].as_i64(),
            );
            let want = if case_rel.contains("cylinder") {
                ("finished", Some(1280), Some(0))
            } else if case_rel.contains("naca0012") {
                ("not_converged", Some(2000), Some(5))
            } else {
                ("converged", None, Some(0))
            };
            println!(
                "G1 {name}/{id} {case_rel}: state {}, solver {state}, steps {steps}, exit {exit:?}",
                met["state"]
            );
            assert_eq!(met["state"], "finished", "{name}/{id}");
            if aerofoils != naca {
                if !aerofoils {
                    // G2 of the aerofoil runs (their G1 is the known miss)
                    let e = entries.iter().find(|e| e.settings["run"] == json!(id)).expect("entry");
                    let wall = e.wall_s.expect("wall_s");
                    println!("G2 {name}/{id}: {wall:.1} s");
                    assert!(wall <= 180.0, "{name}/{id}: {wall} s");
                    total += wall;
                }
                continue;
            }
            assert_eq!(state, want.0, "{name}/{id}: solver state");
            if let Some(n) = want.1 {
                assert_eq!(steps, n, "{name}/{id}: steps");
            }
            assert_eq!(exit, want.2, "{name}/{id}: exit code");
            if case_rel.contains("cylinder") {
                assert_eq!(met["time"].as_f64(), Some(80.0), "{name}/{id}: end time");
            }
            let e = entries.iter().find(|e| e.settings["run"] == json!(id)).expect("entry");
            let wall = e.wall_s.expect("wall_s");
            println!("G2 {name}/{id}: {wall:.1} s");
            assert!(wall <= 180.0, "{name}/{id}: {wall} s");
            total += wall;
        }
    }
    println!("G2 {tree} total: {total:.1} s");
    assert!(total <= 1200.0, "{tree}: {total} s");
}

/// The G3 comparisons on the given values: (lower, higher) pairs that must be strictly ordered.
fn g3_ordered(pairs: &[(&str, f64, f64)]) -> Vec<String> {
    pairs
        .iter()
        .filter(|(_, lo, hi)| lo.partial_cmp(hi) != Some(std::cmp::Ordering::Less))
        .map(|(what, lo, hi)| format!("{what}: {lo} is not below {hi}"))
        .collect()
}

/// G3, crossing count (registered limit: at least 7 upward crossings of `cl` through its mean in the second half of each
/// cylinder run). Known miss (ADR 0142 amendment 1): the Re 100 run has 6, because about 6 periods fit into the second
/// half of an 80 D/U run. The limit is kept. Every cylinder run of both trees is counted and printed before the limit is
/// asserted (ADR 0150); the Linux tree's Re 100 run crosses 6 times too, the same known miss (ADR 0150 L3).
#[test]
#[ignore = "known miss (ADR 0142 amendment 1): the Re 100 cylinder run crosses 6 times in its second half, limit 7"]
fn g3_cylinders_cross_at_least_7_times_known_miss() {
    let mut counts = Vec::new();
    for (tree, _) in TREES {
        g3_crossings_tree(tree, &mut counts);
    }
    for (what, ups) in &counts {
        assert!(*ups >= 7, "{what}: {ups} crossings");
    }
}

fn g3_crossings_tree(tree: &str, counts: &mut Vec<(String, usize)>) {
    let root = repo().join(tree);
    for (id, case_rel) in &manifest(&root)["wind-tunnel"] {
        if case_rel.contains("cylinder") {
            let m = read_json(
                &root
                    .join("wind-tunnel")
                    .join("runs")
                    .join(id)
                    .join("output")
                    .join("monitors.json"),
            );
            let t: Vec<f64> = serde_json::from_value(m["series"]["cl"]["x"].clone()).unwrap();
            let cl: Vec<f64> = serde_json::from_value(m["series"]["cl"]["y"].clone()).unwrap();
            let t_half = t.last().copied().unwrap_or(0.0) / 2.0;
            let half: Vec<f64> = t
                .iter()
                .zip(&cl)
                .filter(|(ti, _)| **ti >= t_half)
                .map(|(_, c)| *c)
                .collect();
            let mean = half.iter().sum::<f64>() / half.len() as f64;
            let ups = half
                .windows(2)
                .filter(|w| w[0] - mean < 0.0 && 0.0 <= w[1] - mean)
                .count();
            println!("G3 {tree}/{case_rel}: {ups} upward crossings of cl in the second half");
            counts.push((format!("{tree}/{case_rel}"), ups));
        }
    }
}

/// G3: the examples show their flow (sanity, not validation): the cylinders' `cl` amplitude above 0.1 and St rising with
/// Re; 0 < `cl`(2 deg) < `cl`(4 deg); 0 < `x_reattach`(400) < `x_reattach`(800) < 15; 1 < `nu_hot`(1e3) < `nu_hot`(1e4);
/// `lid_drag` < 0. Control: the four variant comparisons with the two runs swapped fail. (The crossing count is the
/// known miss above.) Each tree's runs (ADR 0150: the limits apply to the Linux runs unchanged).
#[test]
fn g3_examples_show_their_flow_and_swapped_variants_fail() {
    for (tree, _) in TREES {
        g3_flow_tree(tree);
    }
}

fn g3_flow_tree(tree: &str) {
    let root = repo().join(tree);
    let wt = ledger_entries(&root.join("wind-tunnel"));
    let qs = ledger_entries(&root.join("quick-start"));
    let v = |e: &[ledger::Entry], var: &str, n: &str| metric(e, var, n);
    let pairs = [
        (
            "cylinder Re 100 cl_amplitude above 0.1",
            0.1,
            v(&wt, "cylinder-re100", "cl_amplitude"),
        ),
        (
            "cylinder Re 200 cl_amplitude above 0.1",
            0.1,
            v(&wt, "cylinder-re200", "cl_amplitude"),
        ),
        (
            "strouhal Re 100 < Re 200",
            v(&wt, "cylinder-re100", "strouhal"),
            v(&wt, "cylinder-re200", "strouhal"),
        ),
        ("0 < cl(2 deg)", 0.0, v(&wt, "naca0012-aoa2", "cl")),
        (
            "cl(2 deg) < cl(4 deg)",
            v(&wt, "naca0012-aoa2", "cl"),
            v(&wt, "naca0012-aoa4", "cl"),
        ),
        ("0 < x_reattach(400)", 0.0, v(&wt, "step-re400", "x_reattach")),
        (
            "x_reattach(400) < x_reattach(800)",
            v(&wt, "step-re400", "x_reattach"),
            v(&wt, "step-re800", "x_reattach"),
        ),
        ("x_reattach(800) < 15", v(&wt, "step-re800", "x_reattach"), 15.0),
        ("1 < nu_hot(1e3)", 1.0, v(&qs, "heated-cavity-ra1e3", "nu_hot")),
        (
            "nu_hot(1e3) < nu_hot(1e4)",
            v(&qs, "heated-cavity-ra1e3", "nu_hot"),
            v(&qs, "heated-cavity-ra1e4", "nu_hot"),
        ),
        ("lid_drag(100) < 0", v(&qs, "cavity-re100", "lid_drag"), 0.0),
        ("lid_drag(400) < 0", v(&qs, "cavity-re400", "lid_drag"), 0.0),
    ];
    for (what, lo, hi) in &pairs {
        println!("G3 {tree} {what}: {lo} < {hi}");
    }
    let probs = g3_ordered(&pairs);
    assert!(probs.is_empty(), "{tree}: {probs:#?}");
    // control: the variant comparisons with the two runs swapped
    let swapped: Vec<(&str, f64, f64)> = pairs
        .iter()
        .filter(|(w, _, _)| {
            w.contains(" < ")
                && !w.starts_with("0 <")
                && !w.starts_with("1 <")
                && !w.contains("< 0")
                && !w.contains("< 15")
        })
        .map(|(w, lo, hi)| (*w, *hi, *lo))
        .collect();
    let failed = g3_ordered(&swapped);
    println!(
        "G3 {tree} control: {} of {} swapped comparisons fail",
        failed.len(),
        swapped.len()
    );
    assert_eq!(swapped.len(), 4);
    assert_eq!(failed.len(), swapped.len(), "{tree}: {failed:#?}");
}

// ------------------------------------------------------------------------------------------------ S7, S8, S9

/// Project-relative paths that a notes file names in backticks (`cases/a.toml`, `../../docs/x.md`).
fn named_paths(text: &str) -> Vec<String> {
    let prefixes = ["cases/", "meshes/", "runs/", "designs/", "notes/", "../../"];
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(|s| s.trim_end_matches(['.', ',', ';', ':']).to_string())
        .filter(|s| prefixes.iter().any(|p| s.starts_with(p)) && !s.contains(['<', '*', ' ']))
        .collect()
}

fn notes_problems(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir.join("notes")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "md") {
            let text = std::fs::read_to_string(&p).unwrap();
            for n in named_paths(&text) {
                if !dir.join(&n).exists() {
                    out.push(format!("{} names `{n}`, which does not exist", p.display()));
                }
            }
        }
    }
    out
}

/// S7: every path the notes name in backticks exists (inside the project, or in the repository through `../../`), and
/// the generated run table names every committed run.
#[test]
fn s7_notes_name_existing_files() {
    for ex in all_projects() {
        let (name, dir, runs) = (ex.label(), ex.dir, ex.runs);
        let probs = notes_problems(&dir);
        assert!(probs.is_empty(), "{name}: {probs:#?}");
        let readme = std::fs::read_to_string(dir.join("notes").join("README.md")).unwrap();
        for (id, _) in &runs {
            assert!(
                readme.contains(&format!("`runs/{id}`")),
                "{name}: README does not name run {id}"
            );
        }
        println!("S7 {name}: {} paths named, all exist", {
            let mut n = 0;
            for e in std::fs::read_dir(dir.join("notes")).unwrap() {
                n += named_paths(&std::fs::read_to_string(e.unwrap().path()).unwrap()).len();
            }
            n
        });
    }
}

/// S7 control: a note naming a missing file is reported.
#[test]
fn s7_control_a_missing_file_is_reported() {
    let d = tmp("notes");
    copy_dir(&examples().join("quick-start"), &d);
    let p = d.join("notes").join("README.md");
    let mut t = std::fs::read_to_string(&p).unwrap();
    t.push_str("\nSee `cases/no-such-case.toml`.\n");
    std::fs::write(&p, t).unwrap();
    let probs = notes_problems(&d);
    println!("S7 control: {probs:?}");
    assert!(
        probs.iter().any(|x| x.contains("cases/no-such-case.toml")),
        "{probs:#?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// S8: sizes stay under the registered caps (committed runs only). ADR 0150: the caps apply to each tree on its own
/// (each project under its cap, each whole tree under 40 MB); the two trees together are not capped.
#[test]
fn s8_sizes_under_the_registered_caps() {
    for (tree, _) in TREES {
        let mut total = 0;
        for ex in tree_projects(tree) {
            let size = project_size(&ex.dir, &ex.runs);
            let cap = CAP_PROJECTS.iter().find(|(n, _)| *n == ex.name).expect("cap").1;
            println!("S8 {}: {size} bytes (cap {cap})", ex.label());
            assert!(size <= cap, "{}: {size} > {cap}", ex.label());
            total += size;
        }
        let root = repo().join(tree);
        let extra = tree_size(&root.join("manifest.json"))
            + tree_size(&root.join("README.md"))
            + tree_size(&root.join(".gitattributes"));
        total += extra;
        println!("S8 {tree} total: {total} bytes (cap {CAP_TOTAL})");
        assert!(total <= CAP_TOTAL, "{tree}: {total} > {CAP_TOTAL}");
    }
}

/// S9: the aerofoil mesh is the recipe's: it reads as a polyMesh with 6144 cells and the patches `airfoil`, `farfield`,
/// `frontAndBack`; every point of the `airfoil` patch lies on the closed-trailing-edge NACA 0012 surface (|y| = y_t(x)
/// within 1e-12) and every point of `farfield` on the circle of radius 20 about (0.5, 0) within 1e-12. Each tree's
/// mesh (ADR 0150: each tree builds its own from the recipe).
#[test]
fn s9_aerofoil_mesh_matches_its_recipe() {
    for (tree, _) in TREES {
        s9_tree(tree);
    }
}

fn s9_tree(tree: &str) {
    let m = rcfd_io::openfoam::read_dir(
        repo()
            .join(tree)
            .join("wind-tunnel")
            .join("meshes")
            .join("naca0012-ogrid"),
    )
    .unwrap();
    assert_eq!(m.n_cells(), 6144);
    let yt = |x: f64| 0.6 * (0.2969 * x.sqrt() - 0.1260 * x - 0.3516 * x * x + 0.2843 * x.powi(3) - 0.1036 * x.powi(4));
    let pts = m.points();
    let mut worst = (0.0f64, 0.0f64);
    for (patch, which) in [("airfoil", 0), ("farfield", 1)] {
        let p = m.patch(patch).unwrap_or_else(|| panic!("patch {patch}"));
        for f in p.faces() {
            for &i in m.face_points(f) {
                let [x, y, _] = pts[i];
                let err = if which == 0 {
                    (y.abs() - yt(x.clamp(0.0, 1.0))).abs()
                } else {
                    ((x - 0.5).hypot(y) - 20.0).abs()
                };
                if which == 0 {
                    worst.0 = worst.0.max(err);
                } else {
                    worst.1 = worst.1.max(err);
                }
            }
        }
    }
    assert!(m.patch("frontAndBack").is_some());
    println!(
        "S9 {tree}: 6144 cells; airfoil points off the surface by at most {:.3e}, far field off the circle by {:.3e}",
        worst.0, worst.1
    );
    assert!(worst.0 <= 1e-12 && worst.1 <= 1e-12, "{tree}: {worst:?}");
}

// ------------------------------------------------------------------------------------------------ T1, T2: the trees

/// The project-relative path (with `/`) of every file under `dir`.
fn files_under(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, d: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(d).unwrap().filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel: Vec<String> = p
                    .strip_prefix(root)
                    .unwrap()
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                out.push(rel.join("/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// A file a rebuild writes (ADR 0150): under `runs/` or `.rcfd/`, the ledger, or in a mesh folder built from a recipe
/// (`meshes/<name>/` next to `meshes/<name>.recipe.toml`). Every other file of a project folder is an input.
fn generated(dir: &Path, rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    parts[0] == "runs"
        || parts[0] == ".rcfd"
        || rel == "designs/variants.ledger.jsonl"
        || (parts.len() >= 3
            && parts[0] == "meshes"
            && dir.join("meshes").join(format!("{}.recipe.toml", parts[1])).is_file())
}

/// The bytes the trees share of an input: a notes file without its generated run table.
fn comparable(rel: &str, data: Vec<u8>) -> Vec<u8> {
    if rel.starts_with("notes/") {
        if let Ok(t) = std::str::from_utf8(&data) {
            if let (Some(a), Some(b)) = (t.find(RUNS_BEGIN), t.find(RUNS_END)) {
                if a < b {
                    return format!("{}{}", &t[..a], &t[b + RUNS_END.len()..]).into_bytes();
                }
            }
        }
    }
    data
}

/// Inputs of project folder `other` that differ from those of `win` (the Windows tree's, which a Linux rebuild copies).
fn inputs_problems(win: &Path, other: &Path) -> Vec<String> {
    let inputs = |d: &Path| -> Vec<String> { files_under(d).into_iter().filter(|r| !generated(d, r)).collect() };
    let (a, b) = (inputs(win), inputs(other));
    let mut out = Vec::new();
    for r in a.iter().filter(|r| !b.contains(r)) {
        out.push(format!("{r}: missing in {}", other.display()));
    }
    for r in b.iter().filter(|r| !a.contains(r)) {
        out.push(format!(
            "{r}: in {} but not an input of {}",
            other.display(),
            win.display()
        ));
    }
    for r in a.iter().filter(|r| b.contains(r)) {
        let read = |d: &Path| comparable(r, std::fs::read(d.join(r)).unwrap());
        if read(win) != read(other) {
            out.push(format!(
                "{r}: differs between {} and {}",
                win.display(),
                other.display()
            ));
        }
    }
    out
}

/// T1 (ADR 0150): the Linux tree holds the Windows tree's inputs: every project has the same input files (case files,
/// mesh recipe, notes, `rcfd-project.json`, `.rcfd`-free) byte for byte, the notes outside their generated run table;
/// the trees' `.gitattributes` are equal.
#[test]
fn t1_the_trees_share_their_inputs() {
    let (win, lin) = (repo().join(TREES[0].0), repo().join(TREES[1].0));
    assert_eq!(
        std::fs::read(win.join(".gitattributes")).unwrap(),
        std::fs::read(lin.join(".gitattributes")).unwrap(),
        ".gitattributes"
    );
    let mut n = 0;
    for ex in tree_projects(TREES[0].0) {
        let other = lin.join(&ex.name);
        let probs = inputs_problems(&ex.dir, &other);
        let count = files_under(&ex.dir).iter().filter(|r| !generated(&ex.dir, r)).count();
        println!("T1 {}: {count} inputs, {} differ", ex.name, probs.len());
        assert!(probs.is_empty(), "{}: {probs:#?}", ex.name);
        n += 1;
    }
    assert_eq!(n, 2);
}

/// T1 control on copies of the Linux tree's quick-start: (a) a case edited, (b) an extra case, (c) a notes line outside
/// the run table changed: each is reported; (d) a change inside the run table only is not.
#[test]
fn t1_control_inputs_that_differ_are_reported() {
    let win = repo().join(TREES[0].0).join("quick-start");
    let lin = repo().join(TREES[1].0).join("quick-start");
    let case = manifest(&repo().join(TREES[1].0))["quick-start"][0].1.clone();
    let edit = |tag: &str, f: &dyn Fn(&Path)| -> Vec<String> {
        let d = tmp(&format!("t1{tag}"));
        copy_dir(&lin, &d);
        f(&d);
        let probs = inputs_problems(&win, &d);
        println!("T1 control ({tag}): {probs:?}");
        let _ = std::fs::remove_dir_all(&d);
        probs
    };
    let a = edit("a", &|d| {
        let mut t = std::fs::read_to_string(d.join(&case)).unwrap();
        t.push_str("# edited in the Linux tree\n");
        std::fs::write(d.join(&case), t).unwrap();
    });
    assert!(a.iter().any(|p| p.starts_with(&format!("{case}: differs"))), "{a:#?}");
    let b = edit("b", &|d| {
        std::fs::write(d.join("cases").join("extra.toml"), "# extra\n").unwrap()
    });
    assert!(b.iter().any(|p| p.starts_with("cases/extra.toml: in ")), "{b:#?}");
    let readme = |d: &Path| d.join("notes").join("README.md");
    let c = edit("c", &|d| {
        let t = std::fs::read_to_string(readme(d)).unwrap();
        std::fs::write(readme(d), format!("{t}\nA line only the Linux tree has.\n")).unwrap();
    });
    assert!(c.iter().any(|p| p.starts_with("notes/README.md: differs")), "{c:#?}");
    let dd = edit("d", &|d| {
        let t = std::fs::read_to_string(readme(d)).unwrap();
        let at = t.find(RUNS_END).unwrap();
        std::fs::write(readme(d), format!("{}| a row in the table |\n{}", &t[..at], &t[at..])).unwrap();
    });
    assert!(dd.is_empty(), "{dd:#?}");
}

/// Signs that the tree `tree` (folder name) at `root` was not built on `platform` (ADR 0150 T2): the manifest's
/// platform, and the absolute case paths every `run.json` records.
fn provenance_problems(tree: &str, platform: &str, root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let m = read_json(&root.join("manifest.json"));
    match m.get("platform") {
        // ADR 0142's tree predates the field; it was built on Windows (its addendum)
        None if platform == "windows" => {}
        None => out.push("manifest.json: no platform".to_string()),
        Some(p) => {
            if p["os"] != platform || p["arch"] != "x86_64" {
                out.push(format!("manifest.json: built on {} {}", p["os"], p["arch"]));
            }
            if platform == "linux" && !p["libc"].as_str().unwrap_or("").starts_with("glibc") {
                out.push(format!("manifest.json: libc {}", p["libc"]));
            }
        }
    }
    for (pname, runs) in manifest(root) {
        for (id, _) in runs {
            let rec = read_json(&root.join(&pname).join("runs").join(&id).join("run.json"));
            for key in ["case_dir", "case_source"] {
                let v = rec[key].as_str().unwrap_or("");
                let ok = if platform == "linux" {
                    v.starts_with('/') && v.contains(&format!("/{tree}/{pname}/cases"))
                } else {
                    let b = v.as_bytes();
                    b.len() > 3
                        && b[0].is_ascii_alphabetic()
                        && &b[1..3] == b":\\"
                        && v.contains(&format!("\\{tree}\\{pname}\\cases"))
                };
                if !ok {
                    out.push(format!("{pname}/runs/{id}/run.json: {key} {v:?}"));
                }
            }
        }
    }
    out
}

/// T2 (ADR 0150): each tree was built on its platform: the Linux tree's manifest records os `linux`, arch `x86_64` and
/// a glibc (the Windows tree's predates the field); every committed run's `run.json` names its case by an absolute path
/// of that platform inside that tree's project folder (`/.../example-projects-linux/<project>/cases`,
/// `X:\...\example-projects\<project>\cases`), so no run was copied in from the other tree.
#[test]
fn t2_each_tree_was_built_on_its_platform() {
    for (tree, platform) in TREES {
        let probs = provenance_problems(tree, platform, &repo().join(tree));
        let m = read_json(&repo().join(tree).join("manifest.json"));
        println!("T2 {tree}: platform {}, {} problems", m["platform"], probs.len());
        assert!(probs.is_empty(), "{tree}: {probs:#?}");
    }
}

/// T2 control on a copy of the Linux tree's manifest and quick-start: (a) the manifest's os `windows`, (b) a run's
/// `case_dir` a Windows path: each is reported.
#[test]
fn t2_control_a_foreign_build_is_reported() {
    let (tree, platform) = TREES[1];
    let src = repo().join(tree);
    let probe = |tag: &str, f: &dyn Fn(&Path)| -> Vec<String> {
        let root = tmp(&format!("t2{tag}")).join(tree);
        copy_dir(&src.join("quick-start"), &root.join("quick-start"));
        let mut m = read_json(&src.join("manifest.json"));
        m["projects"].as_object_mut().unwrap().retain(|k, _| k == "quick-start");
        std::fs::write(root.join("manifest.json"), serde_json::to_vec_pretty(&m).unwrap()).unwrap();
        f(&root);
        let probs = provenance_problems(tree, platform, &root);
        println!("T2 control ({tag}): {probs:?}");
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
        probs
    };
    let clean = probe("0", &|_| {});
    assert!(clean.is_empty(), "the unchanged copy: {clean:#?}");
    let a = probe("a", &|r| {
        let mut m = read_json(&r.join("manifest.json"));
        m["platform"]["os"] = json!("windows");
        std::fs::write(r.join("manifest.json"), serde_json::to_vec_pretty(&m).unwrap()).unwrap();
    });
    assert!(a.iter().any(|p| p.contains("built on \"windows\"")), "{a:#?}");
    let b = probe("b", &|r| {
        let id = manifest(r)["quick-start"][0].0.clone();
        let p = r.join("quick-start").join("runs").join(&id).join("run.json");
        let mut rec = read_json(&p);
        rec["case_dir"] = json!(format!("E:\\x\\{tree}\\quick-start\\cases"));
        std::fs::write(&p, serde_json::to_vec_pretty(&rec).unwrap()).unwrap();
    });
    assert!(b.iter().any(|p| p.contains("case_dir")), "{b:#?}");
}
