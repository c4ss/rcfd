//! F-471 (ADR 0748, amendment 1): D1 of ADR 0373 (F-476, kept by F-834 in `shipped_store_f834.rs`) restated. The first
//! hash of `cases/axial_fan_cgrid` moved with the corrected order of the coarsest dense LU (ADR 0743); the maintainer
//! approved the move on 2026-11-20. The retired test is an ignored record in `shipped_store_f834.rs`, its table untouched.
//! Here: the ten cases under the default against the table with that one value changed (S2), and `axial_fan_cgrid` under
//! the old order, asked for with `LinearOptions::amg_coarse_lu_interleaved` on the steady solver, against the two
//! registered values of ADR 0373 (S3). `planned` and the body of the run are copies of the retired test.

mod f834;

use rcfd_api::case::Case;
use rcfd_api::chain::apply_set;
use rcfd_api::runctl::{drive, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_workspace::plan::{parse_case, Plan};
use std::path::Path;
use std::sync::Mutex;

const FAN: &str = "cases/axial_fan_cgrid/axial_fan_cgrid.toml";
/// The two hashes of `axial_fan_cgrid` of ADR 0373 D1 (store.rts, index.json), before ADR 0743.
const FAN_OLD: (u64, u64) = (0xe4539468a88f2746, 0x5234e433bd8358f4);

/// (case, FNV-1a 64 of store.rts, of index.json): the table of ADR 0373 D1 with the first hash of `axial_fan_cgrid` as
/// measured under the default of ADR 0743 (gate run 44599067147; S0 of ADR 0748 addendum 1). The nineteen other values
/// are those of the retired table.
const NEW: [(&str, u64, u64); 10] = [
    (
        "cases/axial_fan_cgrid/axial_fan_cgrid.toml",
        0x03d18f072e9caca5,
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

/// Three steps of `case` with the runner's folders under `dir`, the old order of the coarsest dense LU asked for on the
/// steady solver when `old_order`; the hashes of store.rts and index.json.
fn store_hashes(case: &Case, dir: &Path, old_order: bool) -> (u64, u64) {
    let mut sim = Simulation::new(case).unwrap_or_else(|d| panic!("{d:?}"));
    if old_order {
        let solver = sim.simple_solver_mut().expect("a steady SIMPLE run");
        solver.settings.linear.amg_coarse_lu_interleaved = true;
    }
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
    (f834::fnv(&store), f834::fnv(&index))
}

/// ADR 0748 S2: D1 of ADR 0373 under the default of ADR 0743, the ten cases.
#[test]
fn d1_restated_under_the_default() {
    let tmp = f834::temp("adr0748-s2");
    let mut measured = Vec::new();
    for (i, (rel, path)) in f834::shipped("toml").iter().enumerate() {
        let (case, _) = planned(rel, path);
        if case.tree.get("mesh").ok().is_some_and(|m| m.get("adapt").is_some()) {
            continue;
        }
        let (hs, hi) = store_hashes(&case, &tmp.join(format!("run{i}")), false);
        eprintln!("S2 (\"{rel}\", 0x{hs:016x}, 0x{hi:016x}),");
        measured.push((rel.clone(), hs, hi));
    }
    let new: Vec<(String, u64, u64)> = NEW.iter().map(|(r, s, i)| (r.to_string(), *s, *i)).collect();
    assert_eq!(measured, new);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// ADR 0748 S3: `axial_fan_cgrid` under the old order gives the two values of ADR 0373 D1.
#[test]
fn d1_axial_fan_cgrid_under_the_old_order() {
    let tmp = f834::temp("adr0748-s3");
    let (rel, path) = f834::shipped("toml").into_iter().find(|(r, _)| r == FAN).expect(FAN);
    let (case, _) = planned(&rel, &path);
    let (hs, hi) = store_hashes(&case, &tmp.join("run"), true);
    eprintln!("S3 (\"{rel}\", 0x{hs:016x}, 0x{hi:016x}), old order");
    assert_eq!((hs, hi), FAN_OLD, "S3: axial_fan_cgrid under the old order");
    let _ = std::fs::remove_dir_all(&tmp);
}
