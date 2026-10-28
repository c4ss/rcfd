//! ADR 0139 (wave 48G item 2): the case starter. CS1-CS13 as registered there, with ADR 0139 amendment 2 (written with
//! ADR 0147's registration: `channel/profile_inlet.toml` offered with its data file); CS14 is the rest of the crate's
//! tests, clippy and fmt. Release build, `--test-threads 1`.

mod common;

use common::{frame, temp, CAVITY};
use rcfd_api::case::Case;
use rcfd_mesh::PatchKind;
use rcfd_project::records::hash_bytes;
use rcfd_project::Project;
use rcfd_studio::case_edit::{CaseEditor, EditorState};
use rcfd_studio::case_starter::{self, check_name, is_reference, Example, EXAMPLES, EXCLUDED};
use rcfd_studio::case_starter_ui;
use rcfd_studio::schema_model::value_at;
use rcfd_studio::{Studio, StudioApp};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Longest wait for the project worker, as ADR 0131's tests.
const WAIT: Duration = Duration::from_secs(30);

fn repo_cases() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases")
}

/// The example's folder in the repository's `cases/`, where its data files are (ADR 0139 amendment 2: the editor opens a
/// case with its folder, ADR 0144 amendment 3).
fn folder_of(e: &Example) -> PathBuf {
    repo_cases().join(Path::new(e.source).parent().unwrap())
}

/// Offered examples (ADR 0139 amendment 2: 10, `channel/profile_inlet.toml` with its data file).
const OFFERED: usize = 10;

fn example(source: &str) -> &'static Example {
    EXAMPLES
        .iter()
        .find(|e| e.source == source)
        .unwrap_or_else(|| panic!("{source} is not offered"))
}

/// A Studio on a fresh project (`cases/` empty unless `seed` writes files), opened and drained. `watch`: watcher on.
fn studio_on_new_project(name: &str, watch: bool, seed: impl FnOnce(&Path)) -> (PathBuf, Studio) {
    let root = temp(name);
    Project::create(&root, name, "rcfd test").unwrap();
    seed(&root);
    let mut st = Studio::in_memory();
    st.run_poll = None; // no run worker: these tests start no runs
    if !watch {
        st.poll = None;
    }
    st.open_project(&root);
    assert!(
        st.drain_until(WAIT, |s| s.browser().is_some()),
        "{:?}",
        st.last_message()
    );
    (root, st)
}

fn tmp_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect()
}

// ---------------------------------------------------------------- CS1

#[test]
fn cs1_every_offered_example_opens_and_validates_in_the_editors_service() {
    let st = Studio::in_memory();
    let mut passed = 0;
    for e in EXAMPLES {
        let rel = format!("cases/{}.{}", e.stem(), e.ext());
        let t = Instant::now();
        // amendment 2: with its folder, as the Studio's editor opens a case (its data files are there)
        let mut ed = CaseEditor::from_bytes_in(st.svc.clone(), &rel, &folder_of(e), e.bytes())
            .unwrap_or_else(|m| panic!("{}: does not open: {m}", e.source));
        let ok = ed.validate();
        eprintln!(
            "cs1 {}: open ok, validate {ok}, {} diagnostics, {:.3} s",
            e.source,
            ed.diagnostics.len(),
            t.elapsed().as_secs_f64()
        );
        assert!(ok && ed.diagnostics.is_empty(), "{}: {:?}", e.source, ed.diagnostics);
        passed += 1;
    }
    assert_eq!(passed, OFFERED);
}

#[test]
fn cs1_control_broken_examples_fail_the_same_function() {
    let st = Studio::in_memory();
    let sod = example("sod/sod.toml").text;
    assert!(sod.contains("nx = 100\n"));
    let bad = sod.replace("nx = 100\n", "nx = \"abc\"\n");
    let m = CaseEditor::from_bytes(st.svc.clone(), "cases/sod.toml", bad.as_bytes())
        .err()
        .expect("a string for mesh.nx must not open");
    eprintln!("cs1 control (open): {m}");
    assert!(m.contains("mesh.nx"), "{m}");
    // ADR 0139 amendment 1: a laminar case without reports, so the text opens and only validate() can refuse it
    let channel = example("rotating_channel/rotating_channel.toml").text;
    let ymax = "[boundaries.ymax]\nvelocity = \"fixed\"\n";
    assert!(channel.contains(ymax));
    let cut = channel.replace(ymax, "");
    let mut ed = CaseEditor::from_bytes(st.svc.clone(), "cases/rotating_channel.toml", cut.as_bytes())
        .expect("opens without the ymax table");
    let ok = ed.validate();
    eprintln!("cs1 control (validate): {ok} {:?}", ed.diagnostics);
    assert!(!ok);
    assert!(
        ed.diagnostics.iter().any(|d| d.starts_with("boundaries.ymax:")),
        "{:?}",
        ed.diagnostics
    );
}

// ---------------------------------------------------------------- CS2

#[test]
fn cs2_created_files_are_byte_equal_to_the_shipped_examples() {
    let (root, mut st) = studio_on_new_project("cs2", true, |_| {});
    for e in EXAMPLES {
        let rel = st.create_case_from_example(e, e.stem()).unwrap();
        assert!(
            st.drain_until(WAIT, |s| s.browser().is_some_and(|b| b.cases.contains(&rel))),
            "{rel} not listed: {:?}",
            st.last_message()
        );
        let written = std::fs::read(root.join(&rel)).unwrap();
        let shipped = std::fs::read(repo_cases().join(e.source)).unwrap();
        assert_eq!(written, e.bytes(), "{rel}: differs from the embedded text");
        assert_eq!(written, shipped, "{rel}: differs from cases/{}", e.source);
        eprintln!(
            "cs2 {rel}: {} bytes, {} = cases/{}",
            written.len(),
            hash_bytes(&written),
            e.source
        );
        // amendment 2: its data files next to it, byte-equal too
        for c in e.companions {
            let crel = format!("cases/{}", c.name());
            let got = std::fs::read(root.join(&crel)).unwrap_or_else(|err| panic!("{crel}: {err}"));
            assert_eq!(got, c.bytes(), "{crel}: differs from the embedded text");
            assert_eq!(
                got,
                std::fs::read(repo_cases().join(c.source)).unwrap(),
                "{crel}: differs from cases/{}",
                c.source
            );
            eprintln!(
                "cs2 {crel}: {} bytes, {} = cases/{}",
                got.len(),
                hash_bytes(&got),
                c.source
            );
        }
    }
    let companions: usize = EXAMPLES.iter().map(|e| e.companions.len()).sum();
    assert_eq!(st.browser().unwrap().cases.len(), OFFERED + companions);
}

#[test]
fn cs2_control_the_editors_save_bytes_are_not_byte_equal() {
    let st = Studio::in_memory();
    let mut differ = 0;
    for e in EXAMPLES {
        let rel = format!("cases/{}.toml", e.stem());
        let mut ed = CaseEditor::from_bytes_in(st.svc.clone(), &rel, &folder_of(e), e.bytes()).unwrap();
        let saved = ed.to_bytes().unwrap();
        eprintln!(
            "cs2 control {}: source {} bytes, editor save {} bytes",
            e.source,
            e.bytes().len(),
            saved.len()
        );
        if saved != e.bytes() {
            differ += 1;
        }
    }
    assert_eq!(differ, OFFERED);
}

// ---------------------------------------------------------------- CS3

fn cs3_listing() -> Vec<String> {
    [
        "cases/Wedge.toml",
        "cases/old.json",
        "cases/sod.toml",
        "cases/sub/deep.toml",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

#[test]
fn cs3_name_rule_refuses_and_accepts() {
    let cases = cs3_listing();
    let long65 = "a".repeat(65);
    let nul = "a\0b".to_string();
    let mut refused: Vec<(&str, &str)> = vec![
        ("", "needs a name"),
        (" ", "not allowed"),
        (".", "not allowed"),
        ("..", "not allowed"),
        ("a/b", "path separator"),
        ("a\\b", "path separator"),
        ("a:b", "not allowed"),
        ("a*b", "not allowed"),
        ("a?b", "not allowed"),
        ("a<b", "not allowed"),
        ("a>b", "not allowed"),
        ("a|b", "not allowed"),
        ("a\"b", "not allowed"),
        (nul.as_str(), "not allowed"),
        ("x.", "not allowed"),
        ("x ", "not allowed"),
        ("my case", "not allowed"),
        ("é", "not allowed"),
        ("-x", "start the name"),
        ("_x", "start the name"),
        ("foo.toml", "extension"),
        ("foo.TOML", "extension"),
        ("foo.json", "extension"),
        (long65.as_str(), "at most 64"),
        ("sod", "already exists"),
        ("SOD", "already exists"),
        ("wedge", "already exists"),
        ("old", "already exists"),
    ];
    for r in [
        "CON", "con", "Con", "PRN", "AUX", "NUL", "COM0", "COM1", "com9", "LPT0", "LPT1", "lpt9",
    ] {
        refused.push((r, "reserved"));
    }
    for (name, reason) in &refused {
        let m = check_name(name, "toml", &cases).expect_err(&format!("{name:?} must be refused"));
        assert!(m.contains(reason), "{name:?}: {m}");
        eprintln!("cs3 refused {name:?}: {m}");
    }
    let long64 = "b".repeat(64);
    let accepted = [
        "x",
        "sod2",
        "my-case_2",
        "deep",
        "CONX",
        "COM10",
        "LPT",
        long64.as_str(),
    ];
    for name in accepted {
        assert_eq!(
            check_name(name, "toml", &cases),
            Ok(format!("cases/{name}.toml")),
            "{name:?}"
        );
    }
    eprintln!("cs3: {} refused, {} accepted", refused.len(), accepted.len());
}

#[test]
fn cs3_control_project_resolve_alone_lets_these_names_through() {
    let root = temp("cs3c");
    let p = Project::create(&root, "cs3c", "rcfd test").unwrap();
    let long65 = "a".repeat(65);
    for name in ["x.", "my case", "a/b", "-x", long65.as_str()] {
        assert!(check_name(name, "toml", &[]).is_err());
        let rel = format!("cases/{name}.toml");
        let r = p.resolve(&rel);
        eprintln!("cs3 control: resolve({rel:?}) -> {r:?}");
        assert!(r.is_ok(), "{rel}: {r:?}");
    }
    for name in ["CON", "LPT9"] {
        let rel = format!("cases/{name}.toml");
        eprintln!("cs3 control (reported): resolve({rel:?}) -> {:?}", p.resolve(&rel));
    }
}

// ---------------------------------------------------------------- CS4

#[test]
fn cs4_an_existing_file_is_never_replaced() {
    let mine: &[u8] = b"# my own case, not the example\ntitle = \"mine\"\n";
    let (root, mut st) = studio_on_new_project("cs4", false, |r| {
        std::fs::write(r.join("cases/sod.toml"), mine).unwrap();
    });
    let sod = example("sod/sod.toml");
    let path = root.join("cases/sod.toml");
    let before = hash_bytes(mine);
    // (a) the model refuses the name; nothing is sent
    let n = st.messages.len();
    let m = st.create_case_from_example(sod, "sod").unwrap_err();
    eprintln!("cs4 (a): {m}");
    assert!(m.contains("already exists"), "{m}");
    assert!(!st.drain_until(Duration::from_millis(1000), |s| s.messages.len() > n));
    assert!(st.pending_case.is_none());
    // (b) the worker's write, without the name check
    let p = Project::open(&root).unwrap();
    let m = case_starter::create(&p, "cases/sod.toml", sod.bytes()).unwrap_err();
    eprintln!("cs4 (b): {m}");
    assert!(m.contains("sod.toml") && m.contains("already exists"), "{m}");
    // (c) the browser does not list the file yet (watcher off, written after the open): the worker refuses
    let late = root.join("cases/late.toml");
    std::fs::write(&late, mine).unwrap();
    assert!(!st.browser().unwrap().cases.contains(&"cases/late.toml".to_string()));
    let rel = st.create_case_from_example(sod, "late").unwrap();
    assert_eq!(rel, "cases/late.toml");
    assert!(
        st.drain_until(WAIT, |s| s
            .last_message()
            .is_some_and(|m| m.contains("late.toml") && m.contains("already exists"))),
        "{:?}",
        st.last_message()
    );
    eprintln!("cs4 (c): {}", st.last_message().unwrap());
    assert!(st.editor.is_none() && st.pending_case.is_none());
    for f in [&path, &late] {
        assert_eq!(
            hash_bytes(&std::fs::read(f).unwrap()),
            before,
            "{} changed",
            f.display()
        );
    }
    assert!(
        tmp_files(&root.join("cases")).is_empty(),
        "{:?}",
        tmp_files(&root.join("cases"))
    );
}

#[test]
fn cs4_control_the_replacing_write_changes_the_hash() {
    let root = temp("cs4c");
    Project::create(&root, "cs4c", "rcfd test").unwrap();
    let path = root.join("cases/sod.toml");
    std::fs::write(&path, b"# mine\n").unwrap();
    let before = hash_bytes(&std::fs::read(&path).unwrap());
    rcfd_project::atomic::atomic_write(&path, example("sod/sod.toml").bytes()).unwrap();
    let after = hash_bytes(&std::fs::read(&path).unwrap());
    eprintln!("cs4 control: {before} -> {after}");
    assert_ne!(before, after);
}

// ---------------------------------------------------------------- CS5

fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            walk(base, &p, out);
        } else {
            let rel = p.strip_prefix(base).unwrap();
            let parts: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            out.push(parts.join("/"));
        }
    }
}

/// ADR 0139 CS5: every file under `dir` is an offered example with equal bytes, an `EXCLUDED` path, or under a
/// `reference` folder; every offered and every excluded path exists. Amendment 2: or a data file of an offered example
/// (a companion) with equal bytes; every companion exists.
fn sync_problems(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    files.sort();
    let mut out = Vec::new();
    let companion = |f: &str| EXAMPLES.iter().flat_map(|e| e.companions).find(|c| c.source == f);
    for f in &files {
        if let Some(e) = EXAMPLES.iter().find(|e| e.source == f) {
            if std::fs::read(dir.join(f)).unwrap() != e.bytes() {
                out.push(format!("{f}: the embedded copy differs from the file"));
            }
        } else if let Some(c) = companion(f) {
            if std::fs::read(dir.join(f)).unwrap() != c.bytes() {
                out.push(format!("{f}: the embedded copy differs from the file"));
            }
        } else if !EXCLUDED.iter().any(|(p, _)| p == f) && !is_reference(f) {
            out.push(format!("{f}: shipped but neither offered nor excluded"));
        }
    }
    for e in EXAMPLES {
        if !files.iter().any(|f| f == e.source) {
            out.push(format!("{}: offered but not in cases/", e.source));
        }
        for c in e.companions {
            if !files.iter().any(|f| f == c.source) {
                out.push(format!("{}: data file of {} but not in cases/", c.source, e.source));
            }
        }
    }
    for (p, _) in EXCLUDED {
        if !files.iter().any(|f| f == p) {
            out.push(format!("{p}: excluded but not in cases/ (stale exclusion)"));
        }
    }
    out
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let dest = to.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_dir(&p, &dest);
        } else {
            std::fs::copy(&p, &dest).unwrap();
        }
    }
}

#[test]
fn cs5_the_list_is_in_step_with_cases() {
    let problems = sync_problems(&repo_cases());
    let mut files = Vec::new();
    walk(&repo_cases(), &repo_cases(), &mut files);
    eprintln!(
        "cs5: {} files in cases/, {} offered, {} excluded, {} reference; problems {problems:?}",
        files.len(),
        EXAMPLES.len(),
        EXCLUDED.len(),
        files.iter().filter(|f| is_reference(f)).count()
    );
    assert!(problems.is_empty(), "{problems:#?}");
    // the lists themselves: no path both offered and excluded, no duplicate, every reason given
    for (p, why) in EXCLUDED {
        assert!(!why.trim().is_empty(), "{p}: no reason");
        assert!(EXAMPLES.iter().all(|e| e.source != *p), "{p}: offered and excluded");
        assert_eq!(EXCLUDED.iter().filter(|(q, _)| q == p).count(), 1, "{p} twice");
    }
    for e in EXAMPLES {
        assert_eq!(
            EXAMPLES.iter().filter(|x| x.source == e.source).count(),
            1,
            "{} twice",
            e.source
        );
        // amendment 2: a data file is in its example's folder and neither offered nor excluded itself
        for c in e.companions {
            assert_eq!(
                Path::new(c.source).parent(),
                Path::new(e.source).parent(),
                "{}: not in the folder of {}",
                c.source,
                e.source
            );
            assert!(
                EXAMPLES.iter().all(|x| x.source != c.source) && EXCLUDED.iter().all(|(p, _)| *p != c.source),
                "{}: a data file also offered or excluded",
                c.source
            );
        }
    }
}

/// One CS5 control: its name, the change made to a copy of `cases/`, the file the one problem must name.
type Drift = (&'static str, Box<dyn Fn(&Path)>, &'static str);

#[test]
fn cs5_controls_each_kind_of_drift_is_one_problem() {
    let base = temp("cs5c");
    let cases: [Drift; 6] = [
        (
            "added",
            Box::new(|d: &Path| {
                std::fs::create_dir_all(d.join("extra")).unwrap();
                std::fs::write(d.join("extra/extra.toml"), "title = \"x\"\n").unwrap();
            }),
            "extra/extra.toml",
        ),
        (
            "deleted",
            Box::new(|d: &Path| std::fs::remove_file(d.join("sod/sod.toml")).unwrap()),
            "sod/sod.toml",
        ),
        // amendment 2: the stale exclusion is a descriptor now (the table became a data file of an offered example)
        (
            "stale",
            Box::new(|d: &Path| std::fs::remove_file(d.join("cavity/case.json")).unwrap()),
            "cavity/case.json",
        ),
        (
            "data file deleted",
            Box::new(|d: &Path| std::fs::remove_file(d.join("channel/profile_inlet.csv")).unwrap()),
            "channel/profile_inlet.csv",
        ),
        (
            "data file changed",
            Box::new(|d: &Path| {
                let p = d.join("channel/profile_inlet.csv");
                let mut b = std::fs::read(&p).unwrap();
                b[0] ^= 0x20;
                std::fs::write(&p, b).unwrap();
            }),
            "channel/profile_inlet.csv",
        ),
        (
            "changed",
            Box::new(|d: &Path| {
                let p = d.join("wedge/wedge.toml");
                let mut b = std::fs::read(&p).unwrap();
                b[0] ^= 0x20;
                std::fs::write(&p, b).unwrap();
            }),
            "wedge/wedge.toml",
        ),
    ];
    for (what, change, file) in cases {
        let dir = base.join(what);
        copy_dir(&repo_cases(), &dir);
        assert!(sync_problems(&dir).is_empty(), "{what}: the copy itself has problems");
        change(&dir);
        let problems = sync_problems(&dir);
        eprintln!("cs5 control {what}: {problems:?}");
        assert_eq!(problems.len(), 1, "{what}: {problems:?}");
        assert!(problems[0].starts_with(file), "{what}: {problems:?}");
    }
}

// ---------------------------------------------------------------- CS6

#[test]
fn cs6_the_excluded_case_files_do_not_open_in_the_editor() {
    let st = Studio::in_memory();
    let mut n = 0;
    for (p, why) in EXCLUDED {
        let ext = p.rsplit('.').next().unwrap();
        if ext != "toml" && ext != "json" {
            continue;
        }
        let bytes = std::fs::read(repo_cases().join(p)).unwrap();
        let m = CaseEditor::from_bytes(st.svc.clone(), &format!("cases/x.{ext}"), &bytes)
            .err()
            .unwrap_or_else(|| panic!("{p} opens in the editor; its exclusion reason no longer holds: {why}"));
        let short: String = m.chars().take(160).collect();
        eprintln!("cs6 {p}: {short}...");
        n += 1;
    }
    // amendment 2: the 6 descriptors (channel/profile_inlet.toml is offered with its data file)
    assert_eq!(n, 6);
}

// ---------------------------------------------------------------- CS7

/// ADR 0139 CS7's comparison: the curated cell count and dimension against the mesh build (amendment 2: the text parsed
/// with its folder, where its data files are).
fn facts_match(e: &Example) -> Result<(), String> {
    let c = Case::parse_toml_in(e.text, &folder_of(e)).map_err(|d| format!("{}: {d:?}", e.source))?;
    let m = rcfd_api::sim::build_mesh(&c).map_err(|d| format!("{}: {d:?}", e.source))?;
    let dim = if m.patches().iter().any(|p| p.kind == PatchKind::Empty) {
        "2D"
    } else {
        "3D"
    };
    if m.n_cells() as u64 != e.cells {
        return Err(format!(
            "{}: {} cells listed, the mesh has {}",
            e.source,
            e.cells,
            m.n_cells()
        ));
    }
    if dim != e.dim {
        return Err(format!("{}: listed {}, the mesh is {dim}", e.source, e.dim));
    }
    Ok(())
}

/// The run length ADR 0149's amendment 3 of ADR 0139 registers for an example (its text parsed with its folder): a
/// transient one runs `<algorithm>, to simulated time T s`, a steady one `<algorithm>, at most N iterations`.
fn run_length(e: &Example) -> String {
    let c = Case::parse_toml_in(e.text, &folder_of(e)).unwrap();
    let text = |k: &str| {
        c.get(k)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    };
    let alg = text("solver.algorithm");
    let transient =
        matches!(alg.as_str(), "piso" | "pimple") || (alg == "compressible" && text("compressible.mode") != "steady");
    if transient {
        let t = c.get("solver.end_time").unwrap().as_f64().unwrap();
        format!("{alg}, to simulated time {t} s")
    } else {
        let n = c.get("solver.max_iterations").unwrap().as_u64().unwrap();
        format!("{alg}, at most {n} iterations")
    }
}

#[test]
fn cs7_curated_cells_and_dimension_match_the_mesh() {
    let mut transient = 0;
    for (e, d) in EXAMPLES.iter().zip(case_starter::build_descriptions()) {
        facts_match(e).unwrap();
        let file: toml::Value = toml::from_str(e.text).unwrap();
        let title = file["title"].as_str().unwrap();
        assert_eq!(d.title, title, "{}", e.source);
        let line = format!("{}: {}", d.title, d.line);
        assert!(line.contains(title) && line.contains(e.dim) && line.contains(&format!("{} cells", e.cells)));
        // ADR 0139 amendment 3 (ADR 0149): the run length in the Run page's words, no bare `t`
        assert_eq!(d.run, run_length(e), "{}", e.source);
        assert!(line.contains(&format!("{}.", d.run)), "{}: {line}", e.source);
        assert!(!line.contains(" t = "), "{}: a bare t: {line}", e.source);
        transient += usize::from(d.run.contains("simulated time"));
        eprintln!("cs7 {line}");
    }
    assert!(transient > 0, "set-up: no transient example");
    eprintln!("cs7: {transient} transient examples");
}

/// ADR 0139 amendment 3's control: the words before (`describe_with(.., true)`) give the transient examples a bare
/// `t = `, which the limit refuses.
#[test]
fn cs7_control_the_words_before_say_t() {
    let mut bare = 0;
    for e in EXAMPLES.iter() {
        let d = e.describe_with(true);
        if d.run.contains(" t = ") {
            bare += 1;
            assert_ne!(d.run, run_length(e), "{}", e.source);
            eprintln!("cs7 control {}: {}", e.source, d.run);
        }
    }
    assert!(bare > 0, "control: no bare t");
}

#[test]
fn cs7_control_a_wrong_count_or_dimension_fails_the_comparison() {
    let sod = *example("sod/sod.toml");
    let more = Example {
        cells: sod.cells + 1,
        ..sod
    };
    let other = Example { dim: "3D", ..sod };
    let (a, b) = (facts_match(&more), facts_match(&other));
    eprintln!("cs7 control: {a:?} / {b:?}");
    assert!(a.is_err() && b.is_err());
}

// ---------------------------------------------------------------- CS8

#[test]
fn cs8_the_browser_lists_and_the_editor_opens_the_new_case_with_the_watcher_off() {
    let (_root, mut st) = studio_on_new_project("cs8", false, |_| {});
    assert!(st.browser().unwrap().cases.is_empty());
    let sod = example("sod/sod.toml");
    let t0 = Instant::now();
    let rel = st.create_case_from_example(sod, "first").unwrap();
    assert_eq!(rel, "cases/first.toml");
    let ok = st.drain_until(WAIT, |s| s.editor.as_ref().is_some_and(|e| e.rel == rel));
    let dt = t0.elapsed();
    assert!(ok, "{:?}", st.last_message());
    assert_eq!(st.browser().unwrap().cases, vec![rel.clone()]);
    let ed = st.editor.as_ref().unwrap();
    assert_eq!(ed.state(), EditorState::Clean);
    assert_eq!(ed.base_hash, hash_bytes(sod.bytes()));
    assert!(
        st.messages.iter().any(|m| m == "created cases/first.toml"),
        "{:?}",
        st.messages
    );
    eprintln!("cs8: listed and opened {:.3} s after the call", dt.as_secs_f64());
}

#[test]
fn cs8_control_a_file_written_by_hand_is_not_listed_with_the_watcher_off() {
    let (root, mut st) = studio_on_new_project("cs8c", false, |_| {});
    std::fs::write(root.join("cases/direct.toml"), CAVITY).unwrap();
    let listed = st.drain_until(Duration::from_secs(2), |s| !s.browser().unwrap().cases.is_empty());
    eprintln!("cs8 control: listed after 2 s: {listed}");
    assert!(!listed);
}

// ---------------------------------------------------------------- CS9

fn cs9_project(name: &str) -> (PathBuf, Studio) {
    let (root, mut st) = studio_on_new_project(name, false, |r| {
        std::fs::write(r.join("cases/a.toml"), CAVITY).unwrap();
    });
    st.open_case("cases/a.toml");
    assert!(st.drain_until(WAIT, |s| s.editor.is_some()));
    (root, st)
}

#[test]
fn cs9_unsaved_edits_are_kept() {
    let (root, mut st) = cs9_project("cs9");
    let schema = st.schema.clone();
    st.editor.as_mut().unwrap().edit_text(&schema, "mesh.nx", "12").unwrap();
    assert_eq!(st.editor_state(), Some(EditorState::Dirty));
    let wedge = example("wedge/wedge.toml");
    let rel = st.create_case_from_example(wedge, "w").unwrap();
    assert!(st.drain_until(WAIT, |s| s
        .messages
        .iter()
        .any(|m| m.starts_with("created cases/w.toml"))));
    // anything that would follow the creation has arrived well within this
    assert!(
        !st.drain_until(Duration::from_millis(500), |s| s.editor.as_ref().unwrap().rel
            != "cases/a.toml")
    );
    assert!(st.browser().unwrap().cases.contains(&rel));
    let ed = st.editor.as_ref().unwrap();
    assert_eq!(ed.rel, "cases/a.toml");
    assert_eq!(ed.state(), EditorState::Dirty);
    assert_eq!(value_at(&ed.settings, "mesh.nx"), Some(&json!(12)));
    let m = st
        .messages
        .iter()
        .find(|m| m.starts_with("created cases/w.toml"))
        .unwrap();
    eprintln!("cs9: {m}");
    assert!(m.contains("not opened"), "{m}");
    assert_eq!(std::fs::read(root.join(&rel)).unwrap(), wedge.bytes());
}

#[test]
fn cs9_control_a_clean_editor_switches_to_the_new_case() {
    let (_root, mut st) = cs9_project("cs9c");
    assert_eq!(st.editor_state(), Some(EditorState::Clean));
    let rel = st.create_case_from_example(example("wedge/wedge.toml"), "w").unwrap();
    let switched = st.drain_until(WAIT, |s| s.editor.as_ref().is_some_and(|e| e.rel == rel));
    eprintln!("cs9 control: switched {switched}, {:?}", st.last_message());
    assert!(switched);
}

// ---------------------------------------------------------------- CS10

fn diagnostic_paths(text: &str) -> (bool, Vec<String>) {
    let st = Studio::in_memory();
    let mut ed = CaseEditor::from_bytes(st.svc.clone(), "cases/e.toml", text.as_bytes()).unwrap();
    let ok = ed.validate();
    let mut paths: Vec<String> = ed
        .diagnostics
        .iter()
        .map(|d| d.split(':').next().unwrap_or_default().to_string())
        .collect();
    paths.sort();
    eprintln!("cs10 {text:?}: valid {ok}, {:?}", ed.diagnostics);
    (ok, paths)
}

#[test]
fn cs10_the_empty_case_does_not_validate_so_no_empty_starter_is_offered() {
    for text in ["", "title = \"empty case\"\n"] {
        let (ok, paths) = diagnostic_paths(text);
        assert!(
            !ok,
            "the all-defaults case validates now: ADR 0139 D7's condition for a \"New empty case\" holds; offer it"
        );
        assert_eq!(paths, ["boundaries.lid", "boundaries.walls"]);
    }
}

#[test]
fn cs10_control_the_two_boundary_tables_make_it_validate() {
    let (ok, paths) =
        diagnostic_paths("[boundaries.lid]\nvelocity = \"fixed\"\n[boundaries.walls]\nvelocity = \"fixed\"\n");
    assert!(ok, "{paths:?}");
}

// ---------------------------------------------------------------- CS11

/// ADR 0139 CS11's rule over `(source, bytes)` pairs of the offered examples and (amendment 2) of their data files:
/// all of it counts to the size; an offered example is a `.toml` case file; nothing is reference data.
fn embed_problems(list: &[(&str, usize)], data: &[(&str, usize)]) -> Vec<String> {
    let mut out = Vec::new();
    let total: usize = list.iter().chain(data).map(|x| x.1).sum();
    if total > 32_768 {
        out.push(format!("{total} bytes embedded, more than 32768"));
    }
    for (s, _) in list {
        if !s.ends_with(".toml") {
            out.push(format!("{s}: not a .toml case file"));
        }
    }
    for (s, _) in list.iter().chain(data) {
        if is_reference(s) {
            out.push(format!("{s}: reference data"));
        }
    }
    out
}

/// The embedded data files of the offered examples.
fn embedded_data() -> Vec<(&'static str, usize)> {
    EXAMPLES
        .iter()
        .flat_map(|e| e.companions)
        .map(|c| (c.source, c.text.len()))
        .collect()
}

#[test]
fn cs11_embedded_size_and_content() {
    let list: Vec<(&str, usize)> = EXAMPLES.iter().map(|e| (e.source, e.text.len())).collect();
    let data = embedded_data();
    let total: usize = list.iter().chain(&data).map(|x| x.1).sum();
    let problems = embed_problems(&list, &data);
    eprintln!(
        "cs11: {} examples and {} data files, {total} bytes embedded; problems {problems:?}",
        list.len(),
        data.len()
    );
    assert!(problems.is_empty());
}

#[test]
fn cs11_control_reference_data_and_size_fail_the_rule() {
    let mut list: Vec<(&str, usize)> = EXAMPLES.iter().map(|e| (e.source, e.text.len())).collect();
    list.push(("cavity/reference/ghia1982_u_centerline.csv", 541));
    let a = embed_problems(&list, &embedded_data());
    let b = embed_problems(&[("a.toml", 32_000), ("b.toml", 769)], &[]);
    eprintln!("cs11 control: {a:?} / {b:?}");
    assert_eq!(a.len(), 2);
    assert_eq!(b.len(), 1);
}

// ---------------------------------------------------------------- CS12

#[test]
fn cs12_the_dialog_draws_every_example_in_headless_frames() {
    let (_root, st) = studio_on_new_project("cs12", false, |_| {});
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    let _ = frame(&mut app, &ctx);
    assert!(app.ui_state.starter.is_none());
    case_starter_ui::request_open(&ctx);
    let mut shapes = usize::MAX;
    for i in 0..60 {
        if i % 2 == 1 {
            app.st.theme = app.st.theme.toggled();
        }
        let out = frame(&mut app, &ctx);
        let d = app.ui_state.starter.as_ref().expect("the dialog is open");
        assert_eq!(d.rows_drawn, OFFERED, "frame {i}");
        shapes = shapes.min(out.shapes.len());
    }
    let d = app.ui_state.starter.as_ref().unwrap();
    eprintln!(
        "cs12: 60 frames, {OFFERED} rows each, at least {shapes} shapes per frame, proposed name {:?}",
        d.name
    );
    assert_eq!(d.name, EXAMPLES[0].stem());
}

/// Not registered (found in review after the registered runs, ADR 0139 addendum): a request made after the dialogs were
/// drawn, as the Project panel's button makes it, asks for the next frame at once, and that frame shows the dialog.
#[test]
fn panel_button_request_repaints_and_opens_the_dialog_next_frame() {
    let (_root, st) = studio_on_new_project("cs12b", false, |_| {});
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        ..Default::default()
    };
    // egui asks for more frames while it settles the layout; draw until a frame asks for none (at most 20)
    let mut quiet = frame(&mut app, &ctx);
    let mut settled = 1;
    while quiet.viewport_output[&egui::ViewportId::ROOT].repaint_delay.is_zero() && settled < 20 {
        quiet = frame(&mut app, &ctx);
        settled += 1;
    }
    eprintln!("panel request: {settled} frames until a frame asked for no repaint");
    let out = ctx.run_ui(input(), |ui| {
        app.ui(ui);
        case_starter_ui::request_open(ui.ctx()); // after dialogs(), as from the Project panel
    });
    let delay = |o: &egui::FullOutput| o.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    eprintln!(
        "panel request: repaint delay {:?} (a frame without a request: {:?})",
        delay(&out),
        delay(&quiet)
    );
    assert!(app.ui_state.starter.is_none());
    assert_eq!(delay(&out), Duration::ZERO);
    let _ = frame(&mut app, &ctx);
    assert_eq!(app.ui_state.starter.as_ref().map(|d| d.rows_drawn), Some(OFFERED));
}

// ---------------------------------------------------------------- CS13

#[test]
fn cs13_building_the_description_list_is_cheap() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP cs13: the limit is for release builds");
        return;
    }
    let mut worst = Duration::ZERO;
    for _ in 0..5 {
        let t = Instant::now();
        let d = case_starter::build_descriptions();
        worst = worst.max(t.elapsed());
        assert_eq!(d.len(), OFFERED);
    }
    eprintln!("cs13: largest of 5 builds {:.3} ms", worst.as_secs_f64() * 1e3);
    assert!(worst <= Duration::from_millis(100));
}

#[test]
fn cs13_control_deriving_the_mesh_facts_through_validation_exceeds_the_limit() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP cs13 control: the limit is for release builds");
        return;
    }
    let st = Studio::in_memory();
    let t = Instant::now();
    let d = case_starter::build_descriptions();
    for e in EXAMPLES {
        let mut ed = CaseEditor::from_bytes_in(st.svc.clone(), "cases/x.toml", &folder_of(e), e.bytes()).unwrap();
        assert!(ed.validate());
    }
    let dt = t.elapsed();
    eprintln!(
        "cs13 control: {} descriptions plus {OFFERED} validations {:.3} ms",
        d.len(),
        dt.as_secs_f64() * 1e3
    );
    assert!(dt > Duration::from_millis(100));
}
