//! F-834: the start-plan record of the 22 unedited shipped cases (ADR 0152, job 48R, DV6), checked against the workspace's
//! start plan (`rcfd_workspace::plan`, what `start_run` applies) now that the egui Studio is gone (F-449).
//!
//! The record is `tests/fixtures/plans_6867e40.txt`, restored byte for byte from git history (blob
//! 9fc695324a8af58d8c0fb50f52211047f046f925, made on 6867e40 by the ignored recorder
//! `crates/rcfd-studio/tests/plans_record_r.rs::dv6_record_on_6867e40`; never record it again). Each block is the
//! `Debug` text of the Studio's `StartPlan` and of its `set()`. The workspace plan computes part of what that text
//! holds: the `--set` entries (the `set:` line, compared whole), the store proposal (`store:`: snapshot spacing and the
//! f32 entry), `note`, `max_iterations` and the cell count of the mesh generator's table; the engine's reading of the
//! case gives `algorithm`, `end_time`, `dt`, `dynamic_mesh` and whether the case sets the precision itself. Those are
//! compared; the rest of the text is the Studio's own (its dialog's priority, threads, memory and extra entries; its
//! `CaseExtra`: output keys, the fixed-step flag, chain, two-D, mesh bytes and how it counted cells) and has no engine
//! counterpart.

mod f834;

use rcfd_api::case::Case;
use rcfd_workspace::plan::{parse_case, Every, Plan};
use rcfd_workspace::templates::LID_DRIVEN_CAVITY;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The git blob id of the record (`git hash-object`), so that a changed fixture fails here and not only in review.
const RECORD_BLOB: &str = "9fc695324a8af58d8c0fb50f52211047f046f925";

/// One case of the record (`dv::Entry` of the deleted Studio tests).
struct Entry {
    label: String,
    rel: String,
    text: String,
    base: PathBuf,
}

/// The unedited cases of DV6 (`dv::unedited`): the files under `cases/`, the cases of `example-projects/*/cases/`, and
/// the two cavity texts of the Studio's tests (`common::RUN_CASE`, and `common::CAVITY`, which was
/// `tests/fixtures/cavity.toml`, byte for byte the workspace's lid-driven cavity template).
fn unedited() -> Vec<Entry> {
    let root = f834::repo();
    let entry = |(rel, p): (String, PathBuf)| Entry {
        label: rel.clone(),
        rel,
        text: std::fs::read_to_string(&p).unwrap(),
        base: p.parent().unwrap().to_path_buf(),
    };
    let mut v: Vec<Entry> = f834::files_under(&root.join("cases"), "toml")
        .into_iter()
        .map(entry)
        .collect();
    let mut projects: Vec<PathBuf> = std::fs::read_dir(root.join("example-projects"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("cases").is_dir())
        .collect();
    projects.sort();
    for p in projects {
        v.extend(f834::files_under(&p.join("cases"), "toml").into_iter().map(entry));
    }
    for (label, text) in [
        ("common::RUN_CASE", f834::RUN_CASE),
        ("common::CAVITY", LID_DRIVEN_CAVITY),
    ] {
        v.push(Entry {
            label: label.into(),
            rel: "cases/cavity.toml".into(),
            text: text.into(),
            base: PathBuf::from("."),
        });
    }
    v
}

/// DV6's control (`dv::saved_control`): the 30-iteration cavity with `physics.nu` 0.02 as `rcfd-api`'s writer saves it
/// (`Case::to_toml`, every key spelled out, the store keys at their defaults).
fn saved_control() -> Entry {
    let mut c = Case::parse_toml(&f834::cavity(30)).unwrap();
    c.set("physics.nu", serde_json::json!(0.02)).unwrap();
    Entry {
        label: "saved control: g4::cavity(30), physics.nu 0.02, Case::to_toml".into(),
        rel: "cases/s.toml".into(),
        text: c.to_toml(),
        base: PathBuf::from("."),
    }
}

/// The blocks of a record (each starts with a line `== <label>`), as (label, the block's text) (`dv::blocks`).
fn blocks(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.split_inclusive('\n') {
        match line.strip_prefix("== ") {
            Some(label) => out.push((label.trim_end().to_string(), line.to_string())),
            None => {
                if let Some((_, b)) = out.last_mut() {
                    b.push_str(line);
                }
            }
        }
    }
    out
}

/// The `git hash-object` id of `bytes` (SHA-1 of `blob <len>\0` and the bytes), without a dependency.
fn git_blob_id(bytes: &[u8]) -> String {
    let mut msg = format!("blob {}\0", bytes.len()).into_bytes();
    msg.extend_from_slice(bytes);
    let mut h: [u32; 5] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476, 0xc3d2_e1f0];
    let bits = (msg.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5a82_7999),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

/// The pieces of a block the engine computes, in the order the record writes them: the plan's line in fragments, and
/// the whole `set:` line.
fn engine_view(e: &Entry) -> (Vec<String>, String) {
    let case = parse_case(&e.text, &e.base, None).unwrap_or_else(|d| panic!("{}: {d:?}", e.label));
    let plan = Plan::of(&case);
    let get = |p: &str| case.get(p).unwrap_or(Value::Null);
    let f = |p: &str| get(p).as_f64().unwrap_or(0.0);
    let store = match plan.every {
        Some(every) => format!(
            "store: Some(StorePlan {{ on: true, every: {every:?}, f32: {} }})",
            !plan.sets_precision
        ),
        None => "store: None".to_string(),
    };
    let pieces = vec![
        format!("== {}\n", e.label),
        format!("plan: Ok(StartPlan {{ case: {:?}, facts: CaseFacts {{ ", e.rel),
        format!(
            "algorithm: {:?}, ",
            get("solver.algorithm").as_str().unwrap_or_default()
        ),
        format!("max_iterations: {}, ", plan.max_iterations),
        format!("end_time: {:?}, ", f("solver.end_time")),
        format!("cells: {:?}, ", plan.cells),
        format!("sets_precision: {}, ", plan.sets_precision),
        format!(
            "dynamic_mesh: {}, ",
            get("dynamic_mesh.enabled").as_bool() == Some(true)
        ),
        format!("dt: {:?}, ", f("solver.dt")),
        format!("{store}, "),
        format!("note: {:?}, ", plan.note),
    ];
    let set = format!("set: {:?}\n", Ok::<_, String>(plan.entries()));
    (pieces, set)
}

/// Where the pieces are not found in `block` in order (empty when every one is).
fn missing(block: &str, pieces: &[String]) -> Vec<String> {
    let mut at = 0;
    let mut out = Vec::new();
    for p in pieces {
        match block[at..].find(p.as_str()) {
            Some(i) => at += i + p.len(),
            None => out.push(p.clone()),
        }
    }
    out
}

/// The `set:` line of a block.
fn set_line(block: &str) -> String {
    block
        .lines()
        .find(|l| l.starts_with("set: "))
        .map(|l| format!("{l}\n"))
        .unwrap_or_default()
}

/// Replaces `crates/rcfd-studio/tests/saved_store_r.rs::dv6_unedited_plans_are_bit_identical` (DV6 (a) and its
/// negative control; the record is that of `crates/rcfd-studio/tests/plans_record_r.rs::dv6_record_on_6867e40`): the
/// workspace's plans of the 22 unedited cases agree with the record made on 6867e40 in every part the engine computes,
/// the `set:` line byte for byte; the saved control's plan differs from the record (ADR 0152: store keys at their
/// schema defaults are not the case's own), while the parts that rule does not touch still agree. DV6 (b), the Studio's
/// route before ADR 0152 (`PlanRoute::default_store_keys_set`), was Studio code with no engine counterpart and is not
/// ported. ADR 0157: a case added after 6867e40 has no recorded plan (the four left out, by name), and a recorded case
/// that is gone fails the label comparison.
#[test]
fn dv6_the_workspace_plans_of_the_unedited_cases_agree_with_the_6867e40_record() {
    let bytes = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plans_6867e40.txt")).unwrap();
    assert_eq!(
        git_blob_id(&bytes),
        RECORD_BLOB,
        "tests/fixtures/plans_6867e40.txt is not the record of 6867e40 (never re-record it)"
    );
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains('\r'), "the record is LF");
    let record = blocks(&text);
    let labels: Vec<&str> = record.iter().map(|(l, _)| l.as_str()).collect();
    let (entries, added): (Vec<Entry>, Vec<Entry>) =
        unedited().into_iter().partition(|e| labels.contains(&e.label.as_str()));
    let added: Vec<&str> = added.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(
        added,
        [
            "cases/cavity/adapt_re100.toml",
            "example-projects/axial-fan/cases/fan01-passage-upwind.toml",
            "example-projects/wind-tunnel/cases/naca0012-aoa2-rotated.toml",
            "example-projects/wind-tunnel/cases/naca0012-aoa4-rotated.toml",
        ],
        "the cases added after 6867e40 (no recorded plan)"
    );
    assert_eq!(entries.len(), 22, "the unedited cases");
    let control = saved_control();
    let mut want: Vec<&str> = entries.iter().map(|e| e.label.as_str()).collect();
    want.push(control.label.as_str());
    assert_eq!(labels, want, "the record's cases");
    let recorded = |label: &str| record.iter().find(|(l, _)| l == label).map(|(_, b)| b.clone()).unwrap();
    for e in &entries {
        let block = recorded(&e.label);
        let (pieces, set) = engine_view(e);
        let miss = missing(&block, &pieces);
        assert!(
            miss.is_empty(),
            "{}: not in the record: {miss:?}\nrecord:\n{block}",
            e.label
        );
        assert_eq!(
            set_line(&block),
            set,
            "{}: the set line differs from the record",
            e.label
        );
        eprintln!("DV6 {}: agrees with the record; {}", e.label, set.trim_end());
    }
    // negative control: the saved control spells the store keys out at their defaults; the record (the route of
    // 6867e40) gives it no store, the workspace plan does
    let tree: Value = toml::from_str(&control.text).unwrap();
    let o = &tree["output"];
    assert!(
        o["store_interval"].as_u64() == Some(0)
            && o["store_time_interval"].as_f64() == Some(0.0)
            && o["store_precision"] == "f64"
            && o["store_fields"] == serde_json::json!([]),
        "set-up: the saved control does not spell the store keys out: {o}"
    );
    let block = recorded(&control.label);
    let (pieces, set) = engine_view(&control);
    eprintln!("DV6 control: the workspace {}the record {}", set, set_line(&block));
    assert_ne!(set_line(&block), set, "control: the workspace plan equals the record");
    assert_eq!(set_line(&block), "set: Ok([])\n");
    assert_eq!(
        set,
        "set: Ok([\"output.store_interval=1\", \"output.store_precision=f32\"])\n"
    );
    let miss = missing(&block, &pieces);
    assert_eq!(
        miss,
        [
            "sets_precision: false, ",
            "store: Some(StorePlan { on: true, every: Steps(1), f32: true }), ",
            "note: None, "
        ],
        "control: only the parts of ADR 0152's rule differ"
    );
    assert_eq!(
        Plan::of(&parse_case(&control.text, &control.base, None).unwrap()).every,
        Some(Every::Steps(1))
    );
}
