//! ADR 0373 D1 (F-476): the field store of every shipped case without `[mesh.adapt]` is byte for byte the store it wrote
//! before F-476. SF2's set-up (the start plan's `set()`, three steps of `runctl::drive` with the runner's folders); FNV-1a 64
//! of `store.rts` and `index.json` per case against the table measured with this file on an archive copy of main ca949c78.

mod common;

use rcfd_api::case::Case;
use rcfd_api::chain::apply_set;
use rcfd_api::runctl::{drive, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_studio::runs::StartPlan;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// (case, FNV-1a 64 of store.rts, of index.json), measured on main ca949c78 before any code of F-476 (ADR 0373 D1).
const BASE: [(&str, u64, u64); 10] = [
    ("cases/axial_fan_cgrid/axial_fan_cgrid.toml", 0xe4539468a88f2746, 0x5234e433bd8358f4),
    ("cases/axial_fan_sector/axial_fan_sector.toml", 0x8d9e1d34c706cee0, 0x03e63f21ee68fa28),
    ("cases/channel/kepsilon_channel.toml", 0x269c3babb3a87b12, 0x984cf3730b698718),
    ("cases/channel/kepsilon_inlet.toml", 0x106d318127515386, 0x9026a73a5c545e72),
    ("cases/channel/profile_inlet.toml", 0xacb39f1e9f45233c, 0x545ea65c717d922e),
    ("cases/channel/sst_channel.toml", 0xf5aaa2a8677a7f08, 0x2d45ac9c983ecf6b),
    ("cases/packed_bed/packed_bed.toml", 0x4106958b724ca8d9, 0x20c63fec68d21438),
    ("cases/rotating_channel/rotating_channel.toml", 0xd2d9ac194bca796b, 0x0eaa679efd2d13c7),
    ("cases/sod/sod.toml", 0x83e78cebd8f8cc65, 0xd237488983de765c),
    ("cases/wedge/wedge.toml", 0x051f5460fa632813, 0xfdbea4878f589da5),
];

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn shipped() -> Vec<(String, PathBuf)> {
    let root = rcfd_project::paths::canonicalize(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let mut out = Vec::new();
    let mut stack = vec![root.join("cases")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().map(|e| e.unwrap()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "toml") {
                let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                out.push((rel, p));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn d1_the_store_of_every_non_adaptive_shipped_case_is_unchanged() {
    let tmp = common::temp("f476-d1");
    let mut measured = Vec::new();
    for (i, (rel, path)) in shipped().iter().enumerate() {
        let text = std::fs::read_to_string(path).unwrap();
        let mut case = Case::load(path).unwrap_or_else(|d| panic!("{rel}: {d:?}"));
        if case.tree.get("mesh").ok().is_some_and(|m| m.get("adapt").is_some()) {
            continue;
        }
        let plan = StartPlan::new(rel, &text, path.parent().unwrap()).unwrap();
        for s in plan.set() {
            apply_set(&mut case, &s).unwrap_or_else(|e| panic!("{rel}: {s}: {e:?}"));
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
        let (hs, hi) = (fnv(&store), fnv(&index));
        eprintln!("D1 (\"{rel}\", 0x{hs:016x}, 0x{hi:016x}),");
        measured.push((rel.clone(), hs, hi));
    }
    let base: Vec<(String, u64, u64)> = BASE.iter().map(|(r, s, i)| (r.to_string(), *s, *i)).collect();
    assert_eq!(measured, base);
    let _ = std::fs::remove_dir_all(&tmp);
}
