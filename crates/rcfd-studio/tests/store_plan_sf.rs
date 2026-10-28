//! ADR 0140 SF1 (every shipped case plans a store), SF3 (cell counts) and SF9 (the start dialog's lines, warnings and
//! fixes, pure logic and a drawn frame).

mod common;

use rcfd_api::case::Case;
use rcfd_studio::runs::{PlanRoute, StartPlan};
use rcfd_studio::store_plan::{self, Fix, Warning};
use rcfd_studio::ui::StartDialog;
use rcfd_studio::{Studio, StudioApp};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn repo() -> PathBuf {
    rcfd_project::paths::canonicalize(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

/// Files under `cases/` with extension `ext`, as (`cases/...` with `/`, absolute path), sorted.
fn shipped(ext: &str) -> Vec<(String, PathBuf)> {
    let root = repo();
    let mut out = Vec::new();
    let mut stack = vec![root.join("cases")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().map(|e| e.unwrap()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                out.push((rel, p));
            }
        }
    }
    out.sort();
    out
}

fn plan_of(rel: &str, path: &Path) -> StartPlan {
    let text = std::fs::read_to_string(path).unwrap();
    StartPlan::new(rel, &text, path.parent().unwrap()).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The registered `set()` of every shipped case (SF1), before `output.store_precision=f32`.
const EXPECTED: [(&str, &str); 10] = [
    ("cases/axial_fan_cgrid/axial_fan_cgrid.toml", "output.store_interval=60"),
    (
        "cases/axial_fan_sector/axial_fan_sector.toml",
        "output.store_interval=60",
    ),
    ("cases/channel/kepsilon_channel.toml", "output.store_interval=80"),
    ("cases/channel/kepsilon_inlet.toml", "output.store_interval=4"),
    ("cases/channel/profile_inlet.toml", "output.store_interval=20"),
    ("cases/channel/sst_channel.toml", "output.store_interval=90"),
    ("cases/packed_bed/packed_bed.toml", "output.store_interval=17"),
    (
        "cases/rotating_channel/rotating_channel.toml",
        "output.store_interval=17",
    ),
    ("cases/sod/sod.toml", "output.store_time_interval=0.002"),
    ("cases/wedge/wedge.toml", "output.store_time_interval=0.03"),
];

/// The 48D rule (ADR 0133, before ADR 0140 D2): the time rule only for piso and pimple.
fn rule_48d(f: &rcfd_studio::runs::CaseFacts) -> String {
    if matches!(f.algorithm.as_str(), "piso" | "pimple") && f.end_time > 0.0 {
        format!("output.store_time_interval={}", f.end_time / 100.0)
    } else {
        format!("output.store_interval={}", f.max_iterations.div_ceil(100).max(1))
    }
}

#[test]
fn sf1_every_shipped_case_plans_a_store() {
    let cases = shipped("toml");
    let names: Vec<&str> = cases.iter().map(|(r, _)| r.as_str()).collect();
    assert_eq!(names, EXPECTED.map(|(r, _)| r), "the shipped case files");
    for ((rel, path), (_, want)) in cases.iter().zip(EXPECTED) {
        let plan = plan_of(rel, path);
        eprintln!(
            "SF1 {rel}: set {:?}; {:?}; {:?}",
            plan.set(),
            store_plan::store_line(&plan),
            store_plan::case_lines(&plan)
        );
        assert!(plan.store.as_ref().is_some_and(|s| s.on), "{rel}: store not on");
        assert_eq!(plan.set(), [want, "output.store_precision=f32"], "{rel}");
        assert!(plan.note.is_none(), "{rel}: {:?}", plan.note);
    }
    let descriptors = shipped("json");
    assert_eq!(descriptors.len(), 6, "{descriptors:?}");
    for (rel, path) in &descriptors {
        let text = std::fs::read_to_string(path).unwrap();
        let err = StartPlan::new(rel, &text, path.parent().unwrap()).unwrap_err();
        eprintln!("SF1 {rel}: refused: {err}");
        // ADR 0149 (registered change): the plan reads JSON as `rcfd run` does and refuses a benchmark descriptor with
        // ADR 0147's one line (before: the TOML parser's `<rel>: not TOML: ...`)
        assert_eq!(
            Some(err.clone()),
            rcfd_studio::preview::descriptor_message(rel, &text),
            "{err}"
        );
    }
    // negative control: the 48D rule gives sod the step rule
    let (rel, path) = &cases[8];
    let old = rule_48d(&plan_of(rel, path).facts);
    eprintln!("SF1 control: 48D rule for {rel}: {old}");
    assert_eq!(old, "output.store_interval=20");
    assert_ne!(old, EXPECTED[8].1);
}

#[test]
fn sf3_cell_counts() {
    let mut fan = Vec::new();
    for (rel, path) in shipped("toml") {
        let case = Case::load(&path).unwrap();
        let n = rcfd_api::sim::build_mesh(&case).unwrap().n_cells() as u64;
        let plan = plan_of(&rel, &path);
        match plan.facts.cells {
            Some(c) => {
                eprintln!("SF3 {rel}: table {c}, mesh {n}");
                assert_eq!(c, n, "{rel}");
            }
            None => {
                eprintln!("SF3 {rel}: no table count, mesh {n}");
                fan.push((rel, path, n));
            }
        }
    }
    let fan_names: Vec<&str> = fan.iter().map(|(r, _, _)| r.as_str()).collect();
    assert_eq!(
        fan_names,
        [
            "cases/axial_fan_cgrid/axial_fan_cgrid.toml",
            "cases/axial_fan_sector/axial_fan_sector.toml"
        ]
    );
    // the Studio model counts them in the background (run worker, no runner)
    let root = common::temp("sf3").join("proj");
    rcfd_project::Project::create(&root, "sf3", "rcfd test").unwrap();
    for (rel, path, _) in &fan {
        let name = Path::new(rel).file_name().unwrap();
        std::fs::copy(path, root.join("cases").join(name)).unwrap();
    }
    let mut st = Studio::in_memory();
    st.poll = None;
    st.open_project(&root);
    assert!(st.drain_until(Duration::from_secs(30), |s| s.browser().is_some()
        && s.runs.last_tick.is_some()));
    for (rel, _, n) in &fan {
        let local = format!("cases/{}", Path::new(rel).file_name().unwrap().to_string_lossy());
        st.plan_start(&local).unwrap();
        let t0 = std::time::Instant::now();
        assert!(
            st.drain_until(Duration::from_secs(60), |s| matches!(&s.runs.plan, Some(Ok(p))
                if p.case == local && p.facts.cells.is_some())),
            "{local}: no cell count within 60 s: {:?}",
            st.runs
                .plan
                .as_ref()
                .map(|p| p.as_ref().map(|p| p.facts.extra.cells_state.clone()))
        );
        let Some(Ok(p)) = &st.runs.plan else { unreachable!() };
        eprintln!(
            "SF3 {local}: counted {:?} cells in {:.2} s, {:?}; {:?}",
            p.facts.cells,
            t0.elapsed().as_secs_f64(),
            p.facts.extra.cells_state,
            store_plan::size_line(p)
        );
        assert_eq!(p.facts.cells, Some(*n));
        // negative control: nx * ny of the fan case (it has no nx, ny) is not the count
        let c = Case::parse_toml(&std::fs::read_to_string(root.join(&local)).unwrap()).unwrap();
        let g = |k: &str| c.get(k).ok().and_then(|v| v.as_u64()).unwrap_or(0);
        let nxny = g("mesh.nx") * g("mesh.ny");
        eprintln!("SF3 control {local}: nx * ny = {nxny}");
        assert_ne!(nxny, *n);
    }
}

fn texts_of(w: &[Warning]) -> Vec<String> {
    w.iter().map(|w| w.text.clone()).collect()
}

const OFF: &str = "so the Viewport will have nothing to show for this run.";

/// R10 (e)'s transient cavity.
fn piso_cavity() -> String {
    "[mesh]\ngenerator = \"cavity_2d\"\nnx = 12\nny = 12\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"piso\"\nmax_iterations = 400\nend_time = 0.3\ndt = 0.01\n".into()
}

const BIG: &str = r#"[mesh]
generator = "box_2d"
nx = 1000
ny = 1000
[physics]
nu = 0.01
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
max_iterations = 2000
"#;

fn cavity_plan(text: &str) -> StartPlan {
    StartPlan::new("cases/cavity.toml", text, Path::new(".")).unwrap()
}

#[test]
fn sf9_start_dialog_lines_warnings_and_fixes() {
    // (a) the default
    let a = cavity_plan(common::RUN_CASE);
    eprintln!(
        "SF9 (a): {:?} {:?} {:?} estimate {:?}",
        store_plan::store_line(&a),
        store_plan::size_line(&a),
        texts_of(&store_plan::warnings(&a)),
        store_plan::estimate(&a)
    );
    assert!(a.store.as_ref().is_some_and(|s| s.on));
    assert_eq!(
        store_plan::store_line(&a).unwrap(),
        "Stores ux, uy, uz, p every 4 iterations: at most 100 snapshots (fewer if the run converges earlier), f32."
    );
    assert_eq!(
        store_plan::size_line(&a).unwrap(),
        "Estimated size 342 kB (144 cells), in runs/<run id>/fields/ where the Viewport reads it."
    );
    assert_eq!(store_plan::estimate(&a), Some(342_016));
    assert!(
        store_plan::warnings(&a).is_empty(),
        "control: the default has no warning"
    );
    // (b) switched off, and the fix
    let mut b = a.clone();
    b.store.as_mut().unwrap().on = false;
    let w = store_plan::warnings(&b);
    eprintln!("SF9 (b): {:?}", w);
    assert_eq!(
        w,
        [Warning {
            text: format!("Fields will not be stored, {OFF}"),
            fix: Some(("Store fields".into(), Fix::StoreOn))
        }]
    );
    store_plan::apply_fix(&mut b, Fix::StoreOn);
    assert_eq!(b.set(), a.set());
    // (c) the case with `store_interval = 0`. ADR 0152 (a registered change): the 0 is the schema default, which a case
    // saved by the Studio spells out, so it is not the case's own: the plan of (a), no warning, no case line
    let c_text = format!("{}\n[output]\nstore_interval = 0\n", common::RUN_CASE);
    let c_new = cavity_plan(&c_text);
    eprintln!(
        "SF9 (c), ADR 0152: set {:?}, warnings {:?}, case lines {:?}",
        c_new.set(),
        store_plan::warnings(&c_new),
        store_plan::case_lines(&c_new)
    );
    assert!(c_new.store.as_ref().is_some_and(|s| s.on));
    assert_eq!(c_new.set(), ["output.store_interval=4", "output.store_precision=f32"]);
    assert!(store_plan::warnings(&c_new).is_empty());
    assert!(store_plan::case_lines(&c_new).is_empty());
    // the registered (c), with the route before ADR 0152: the case turns the store off
    let before = PlanRoute {
        default_store_keys_set: true,
        ..PlanRoute::default()
    };
    let mut c = StartPlan::new_route("cases/cavity.toml", &c_text, Path::new("."), before).unwrap();
    assert!(c.set().is_empty(), "{:?}", c.set());
    let w = store_plan::warnings(&c);
    eprintln!("SF9 (c): {:?}", w);
    assert_eq!(
        w,
        [Warning {
            text: format!("The case turns the field store off (output.store_interval = 0), {OFF}"),
            fix: Some(("Store fields for this run".into(), Fix::StoreForCase))
        }]
    );
    store_plan::apply_fix(&mut c, Fix::StoreForCase);
    assert_eq!(c.set(), ["output.store_interval=4", "output.store_precision=f32"]);
    assert!(store_plan::case_lines(&c)
        .contains(&"Overrides the case's output.store_interval = 0 for this run.".to_string()));
    assert!(store_plan::warnings(&c).is_empty());
    // (d) huge
    let mut d = cavity_plan(BIG);
    let w = store_plan::warnings(&d);
    eprintln!(
        "SF9 (d): cells {:?}, estimate {:?}, {:?}",
        d.facts.cells,
        store_plan::estimate(&d),
        w
    );
    assert_eq!(d.facts.cells, Some(1_000_000));
    assert_eq!(d.set(), ["output.store_interval=20", "output.store_precision=f32"]);
    assert_eq!(store_plan::estimate(&d), Some(1_864_313_600));
    assert_eq!(
        w,
        [Warning {
            text: "The store is estimated at 1.86 GB, more than 1.00 GB: check that the disk has room, or store fewer \
                   snapshots."
                .into(),
            fix: Some(("Store fewer snapshots (about 45)".into(), Fix::FitHuge))
        }]
    );
    store_plan::apply_fix(&mut d, Fix::FitHuge);
    eprintln!(
        "SF9 (d) fixed: set {:?}, estimate {:?}",
        d.set(),
        store_plan::estimate(&d)
    );
    assert_eq!(d.set(), ["output.store_interval=45", "output.store_precision=f32"]);
    assert_eq!(store_plan::estimate(&d), Some(984_141_615));
    assert!(store_plan::estimate(&d).unwrap() <= store_plan::HUGE_BYTES);
    // (e) an override raises the count above 100
    let mut e = a.clone();
    e.extra_set = vec!["solver.max_iterations=40000".into()];
    let w = store_plan::warnings(&e);
    eprintln!("SF9 (e): {:?}", w);
    assert_eq!(
        w,
        [Warning {
            text: "10000 snapshots will be stored, more than the 100 the Studio proposes.".into(),
            fix: Some(("Store at most 100 snapshots".into(), Fix::AtMost100))
        }]
    );
    store_plan::apply_fix(&mut e, Fix::AtMost100);
    assert_eq!(
        e.set(),
        [
            "output.store_interval=400",
            "output.store_precision=f32",
            "solver.max_iterations=40000"
        ]
    );
    // (f) an override turns the store off
    let mut f = a.clone();
    f.extra_set = vec!["output.store_interval=0".into()];
    let w = store_plan::warnings(&f);
    eprintln!("SF9 (f): {:?}", w);
    assert_eq!(
        w,
        [Warning {
            text: format!("The override output.store_interval=0 turns the field store off, {OFF}"),
            fix: None
        }]
    );
    // (g) R10 (e)'s piso cavity: every step stored
    let g = cavity_plan(&piso_cavity());
    assert_eq!(
        store_plan::store_line(&g).unwrap(),
        "Stores ux, uy, uz, p every 0.003 s of simulated time: 30 snapshots, f32."
    );
    // (h) sod
    let sod = repo().join("cases/sod/sod.toml");
    let h = plan_of("cases/sod/sod.toml", &sod);
    assert_eq!(
        store_plan::store_line(&h).unwrap(),
        "Stores ux, uy, uz, p every 0.002 s of simulated time: at most 100 snapshots (the time step follows the CFL \
         limit), f32."
    );
}

/// The text of every drawn text shape.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

#[test]
fn sf9_start_dialog_drawn() {
    let root = common::temp("sf9-frame").join("proj");
    rcfd_project::Project::create(&root, "sf9", "rcfd test").unwrap();
    std::fs::write(root.join("cases/cavity.toml"), common::RUN_CASE).unwrap();
    let mut st = Studio::in_memory();
    st.poll = None;
    st.open_project(&root);
    assert!(st.drain_until(Duration::from_secs(30), |s| s.browser().is_some()
        && s.runs.last_tick.is_some()));
    let mut app = StudioApp::new(st, None);
    app.ui_state.start_dialog = Some(StartDialog {
        case: "cases/cavity.toml".into(),
        ..Default::default()
    });
    let ctx = egui::Context::default();
    let store_line =
        "Stores ux, uy, uz, p every 4 iterations: at most 100 snapshots (fewer if the run converges earlier), f32.";
    let size_line = "Estimated size 342 kB (144 cells), in runs/<run id>/fields/ where the Viewport reads it.";
    let first = texts(&common::frame(&mut app, &ctx).shapes);
    eprintln!(
        "SF9 (i) control: first frame has the store line: {}",
        first.iter().any(|t| t == store_line)
    );
    assert!(
        !first.iter().any(|t| t == store_line),
        "control: drawn before the plan arrived"
    );
    let mut drawn = Vec::new();
    assert!(common::frames_until(&mut app, &ctx, Duration::from_secs(30), |a| {
        matches!(&a.st.runs.plan, Some(Ok(_)))
    }));
    for _ in 0..3 {
        drawn = texts(&common::frame(&mut app, &ctx).shapes);
    }
    let Some(Ok(p)) = &app.st.runs.plan else { unreachable!() };
    assert!(p.store.as_ref().is_some_and(|s| s.on), "the switch is on");
    for want in ["Store fields for scrubbing (ux, uy, uz, p)", store_line, size_line] {
        assert!(drawn.iter().any(|t| t == want), "not drawn: {want}\n{drawn:?}");
    }
    eprintln!("SF9 (i): checkbox label, store line and size line drawn; switch on");
    if let Some(Ok(p)) = app.st.runs.plan.as_mut() {
        p.store.as_mut().unwrap().on = false;
    }
    let mut off = Vec::new();
    for _ in 0..3 {
        off = texts(&common::frame(&mut app, &ctx).shapes);
    }
    for want in [format!("Fields will not be stored, {OFF}"), "Store fields".to_string()] {
        assert!(off.contains(&want), "not drawn: {want}\n{off:?}");
    }
    assert!(!off.iter().any(|t| t == store_line));
    eprintln!("SF9 (i): switched off: warning and \"Store fields\" drawn, no store line");
}
