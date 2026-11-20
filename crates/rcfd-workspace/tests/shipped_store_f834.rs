//! F-834: the field stores of the shipped cases, three steps in process with the workspace's start plan applied (its
//! entries, what `start_run` passes as `--set`) and the folders the runner passes: the F-476 hashes (ADR 0373 D1) and
//! the readable store of ADR 0140 SF2. The Studio's start dialog planned the same entries (`store_plan_f834.rs`, SF1 and
//! C-P), so the stores are the ones its tests measured.

mod f834;

use rcfd_api::case::Case;
use rcfd_api::chain::apply_set;
use rcfd_api::runctl::{drive, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_workspace::plan::{parse_case, Plan};
use serde_json::Value;
use std::path::Path;
use std::sync::Mutex;

/// (case, FNV-1a 64 of store.rts, of index.json), measured on main ca949c78 before any code of F-476 (ADR 0373 D1);
/// the table of `crates/rcfd-studio/tests/shipped_store_hashes_f476.rs`, unchanged.
const BASE: [(&str, u64, u64); 10] = [
    (
        "cases/axial_fan_cgrid/axial_fan_cgrid.toml",
        0xe4539468a88f2746,
        0x5234e433bd8358f4,
    ),
    (
        "cases/axial_fan_sector/axial_fan_sector.toml",
        0x8d9e1d34c706cee0,
        0x03e63f21ee68fa28,
    ),
    (
        "cases/channel/kepsilon_channel.toml",
        0x269c3babb3a87b12,
        0x984cf3730b698718,
    ),
    (
        "cases/channel/kepsilon_inlet.toml",
        0x106d318127515386,
        0x9026a73a5c545e72,
    ),
    (
        "cases/channel/profile_inlet.toml",
        0xacb39f1e9f45233c,
        0x545ea65c717d922e,
    ),
    ("cases/channel/sst_channel.toml", 0xf5aaa2a8677a7f08, 0x2d45ac9c983ecf6b),
    (
        "cases/packed_bed/packed_bed.toml",
        0x4106958b724ca8d9,
        0x20c63fec68d21438,
    ),
    (
        "cases/rotating_channel/rotating_channel.toml",
        0xd2d9ac194bca796b,
        0x0eaa679efd2d13c7,
    ),
    ("cases/sod/sod.toml", 0x83e78cebd8f8cc65, 0xd237488983de765c),
    ("cases/wedge/wedge.toml", 0x051f5460fa632813, 0xfdbea4878f589da5),
];

/// The shipped case at `path` with the workspace plan's entries applied, and the entries.
fn planned(rel: &str, path: &Path) -> (Case, Vec<String>) {
    let text = std::fs::read_to_string(path).unwrap();
    let plan = Plan::of(&parse_case(&text, path.parent().unwrap(), None).unwrap_or_else(|d| panic!("{rel}: {d:?}")));
    let mut case = Case::load(path).unwrap_or_else(|d| panic!("{rel}: {d:?}"));
    let set = plan.entries();
    for s in &set {
        apply_set(&mut case, s).unwrap_or_else(|e| panic!("{rel}: {s}: {e:?}"));
    }
    (case, set)
}

/// Three steps of `case` with the runner's folders under `dir`; the status events.
fn three_steps(case: &Case, dir: &Path) -> Vec<Value> {
    let sim = Simulation::new(case).unwrap_or_else(|d| panic!("{d:?}"));
    let opts = RunOptions {
        max_steps: Some(3),
        out_dir: Some(dir.join("output")),
        store_dir: Some(dir.join("fields")),
        status_monitors: true,
        ..Default::default()
    };
    let mut events = Vec::new();
    let o = drive(&Mutex::new(sim), &opts, &|| false, &mut |e| events.push(e.clone()));
    eprintln!("  drive: {} after {} steps", o.state, o.iterations);
    events
}

fn store_warnings(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e["event"] == "warning")
        .filter_map(|e| e["message"].as_str().map(String::from))
        .filter(|m| m.contains("field store"))
        .collect()
}

/// Replaces `crates/rcfd-studio/tests/shipped_store_hashes_f476.rs::d1_the_store_of_every_non_adaptive_shipped_case_is_unchanged`
/// (ADR 0373 D1, F-476): the field store of every shipped case without `[mesh.adapt]` is byte for byte the store it
/// wrote before F-476, against the recorded table, unchanged.
#[test]
#[ignore = "retired record (ADR 0748, F-471): the first hash of axial_fan_cgrid moved with the default of ADR 0743, approved by the maintainer on 2026-11-20; restated in shipped_store_adr0748.rs"]
fn d1_the_store_of_every_non_adaptive_shipped_case_is_unchanged() {
    let tmp = f834::temp("f476-d1");
    let mut measured = Vec::new();
    for (i, (rel, path)) in f834::shipped("toml").iter().enumerate() {
        let (case, _) = planned(rel, path);
        if case.tree.get("mesh").ok().is_some_and(|m| m.get("adapt").is_some()) {
            continue;
        }
        let dir = tmp.join(format!("run{i}"));
        let sim = Simulation::new(&case).unwrap_or_else(|d| panic!("{d:?}"));
        let opts = RunOptions {
            max_steps: Some(3),
            out_dir: Some(dir.join("output")),
            store_dir: Some(dir.join("fields")),
            status_monitors: true,
            ..Default::default()
        };
        drive(&Mutex::new(sim), &opts, &|| false, &mut |_| {});
        let store = std::fs::read(dir.join("fields").join(rcfd_tsstore::STORE_FILE)).unwrap();
        let index = std::fs::read(dir.join("fields").join(rcfd_tsstore::INDEX_FILE)).unwrap();
        let (hs, hi) = (f834::fnv(&store), f834::fnv(&index));
        eprintln!("D1 (\"{rel}\", 0x{hs:016x}, 0x{hi:016x}),");
        measured.push((rel.clone(), hs, hi));
    }
    let base: Vec<(String, u64, u64)> = BASE.iter().map(|(r, s, i)| (r.to_string(), *s, *i)).collect();
    assert_eq!(measured, base);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Replaces `crates/rcfd-studio/tests/shipped_store_sf2.rs::sf2_every_shipped_case_writes_a_readable_store` (ADR 0140
/// SF2): every shipped case, with its plan's entries, writes a store that opens: at least one snapshot, the fields ux,
/// uy, uz, p in f32 in every snapshot, the last p readable over the store's own mesh with finite values (what the
/// workspace's `field_summary` and `render_field` read; the Studio's viewport reader `StoreView` read the same). Negative
/// control: a store naming a field that does not exist is refused by the solver.
#[test]
fn sf2_every_shipped_case_writes_a_readable_store() {
    let cases = f834::shipped("toml");
    assert_eq!(cases.len(), 11);
    let tmp = f834::temp("sf2");
    let mut ok = 0;
    for (i, (rel, path)) in cases.iter().enumerate() {
        let (case, set) = planned(rel, path);
        let dir = tmp.join(format!("run{i}"));
        eprintln!("SF2 {rel}: set {set:?}");
        let events = three_steps(&case, &dir);
        let warns = store_warnings(&events);
        assert!(warns.is_empty(), "{rel}: {warns:?}");
        let store = dir.join("fields");
        let mut r = rcfd_tsstore::Reader::open(&store).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let n = r.snapshots().len();
        let steps: Vec<u64> = r.snapshots().iter().map(|s| s.step).collect();
        let mut names = Vec::new();
        let mut precisions = std::collections::BTreeSet::new();
        for k in 0..n {
            let f = r.fields(k).unwrap();
            names = f.iter().map(|x| x.name.clone()).collect::<Vec<_>>();
            for x in &f {
                precisions.insert(format!("{:?}", x.precision));
            }
        }
        assert!(n >= 1, "{rel}: no snapshot");
        let cells = r
            .mesh()
            .unwrap_or_else(|e| panic!("{rel}: the store's mesh: {e}"))
            .n_cells();
        let p = r.read_field(n - 1, "p").unwrap();
        eprintln!(
            "SF2 {rel}: {n} snapshot(s) at steps {steps:?}, fields {names:?}, precisions {precisions:?}, last p over \
             {} of {cells} cells",
            p.len()
        );
        assert_eq!(names, ["ux", "uy", "uz", "p"], "{rel}");
        assert_eq!(precisions.into_iter().collect::<Vec<_>>(), ["F32"], "{rel}");
        assert_eq!(p.len(), cells, "{rel}");
        assert!(p.iter().all(|x| x.is_finite()), "{rel}: p not finite");
        ok += 1;
    }
    eprintln!("SF2: {ok} of {} shipped cases stored a readable store", cases.len());
    assert_eq!(ok, 11);
    // negative control: a field that does not exist
    let cav = tmp.join("control.toml");
    std::fs::write(&cav, f834::RUN_CASE).unwrap();
    let mut case = Case::load(&cav).unwrap();
    for s in [
        "output.store_interval=4",
        "output.store_precision=f32",
        "output.store_fields=[\"nosuch\"]",
    ] {
        apply_set(&mut case, s).unwrap();
    }
    let dir = tmp.join("control");
    let warns = store_warnings(&three_steps(&case, &dir));
    eprintln!(
        "SF2 control: {warns:?}; store file: {}",
        dir.join("fields/store.rts").exists()
    );
    assert!(warns.iter().any(|w| w.contains("field store disabled")));
    assert!(!dir.join("fields").join(rcfd_tsstore::STORE_FILE).exists());
    let _ = std::fs::remove_dir_all(&tmp);
}
