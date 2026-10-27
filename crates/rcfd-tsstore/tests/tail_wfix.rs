//! ADR 0128 finding 7 (check C13): a zero-filled or garbage tail after complete snapshots (a crash that extended the
//! file, a torn write seen by a live reader) leaves the complete snapshots readable; the writer stays strict.

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_tsstore::*;
use std::path::{Path, PathBuf};

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-tss-wfix-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Deterministic field values of snapshot `step` (the same pattern for every field).
fn values(step: u64, n: usize) -> Vec<f64> {
    (0..n).map(|i| (i as f64 * 0.37 + step as f64).sin() * 1e3).collect()
}

const FIELDS: [&str; 2] = ["p", "T"];

/// A finished store of 3 snapshots (steps 10, 20, 30) on a 12-cell mesh; returns its bytes.
fn write3(dir: &Path) -> Vec<u8> {
    let mesh = box_mesh([3, 2, 2], [1.0, 0.5, 0.5], [Grading::Uniform; 3]).unwrap();
    let mut w = Writer::create(
        dir,
        &mesh,
        WriteOptions {
            block_values: 5,
            codec: Codec::Raw,
        },
    )
    .unwrap();
    for k in 1..=3u64 {
        let mut s = w.snapshot(10 * k, 0.5 * k as f64).unwrap();
        for f in FIELDS {
            s.field(f, "unknown", Location::Cell, &values(10 * k, 12), Precision::F64)
                .unwrap();
        }
        s.commit().unwrap();
    }
    w.finish().unwrap();
    std::fs::read(dir.join(STORE_FILE)).unwrap()
}

/// Every field of every snapshot reads back bit-exactly.
fn data_ok(r: &mut Reader) -> bool {
    (0..r.snapshots().len()).all(|i| {
        let step = r.snapshots()[i].step;
        FIELDS.iter().all(|f| {
            r.read_field(i, f)
                .is_ok_and(|v| v.iter().zip(values(step, 12)).all(|(a, b)| a.to_bits() == b.to_bits()))
        })
    })
}

fn tails() -> Vec<(&'static str, Vec<u8>)> {
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    let garbage = (0..4096)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect();
    vec![("zero", vec![0u8; 4096]), ("garbage", garbage)]
}

/// Opens `bytes` as a store: (snapshots, tail bytes, verify ok, data ok), or the open error.
fn open(dir: &Path, bytes: &[u8]) -> Result<(usize, u64, bool, bool), String> {
    std::fs::write(dir.join(STORE_FILE), bytes).unwrap();
    let mut r = Reader::open(dir).map_err(|e| e.to_string())?;
    let tail = match r.index_source() {
        IndexSource::Scanned { tail_bytes, .. } => *tail_bytes,
        IndexSource::Footer => u64::MAX,
    };
    let v = r.verify().is_ok();
    Ok((r.snapshots().len(), tail, v, data_ok(&mut r)))
}

#[test]
fn c13_zero_and_garbage_tails_after_complete_snapshots() {
    let src = tmp("c13-src");
    let full = write3(&src);
    let r = Reader::open(&src).unwrap();
    let ends: Vec<usize> = r.snapshots().iter().map(|s| s.end_offset() as usize).collect();
    drop(r);
    let dir = tmp("c13");
    // the first data block of snapshot 1 starts right after the mesh frame
    let mesh_end = {
        let mut r = Reader::open(&src).unwrap();
        r.fields(0).unwrap().iter().map(|f| f.blocks[0]).min().unwrap() as usize
    };
    let mut lines = Vec::new();
    let mut bad = Vec::new();
    for (name, tail) in tails() {
        // unfinished store cut after snapshot n, then the tail
        for (n, cut) in [(0, mesh_end), (1, ends[0]), (3, ends[2])] {
            let mut b = full[..cut].to_vec();
            b.extend_from_slice(&tail);
            let got = open(&dir, &b);
            lines.push(format!("{name} after {n} (unfinished): {got:?}"));
            if got != Ok((n, 4096, true, true)) {
                bad.push(format!("{name}/{n}"));
            }
        }
        // finished store (footer and trailer) with the tail appended: the footer no longer ends the file
        let mut b = full.clone();
        b.extend_from_slice(&tail);
        let got = open(&dir, &b);
        let want = (full.len() - ends[2] + 4096) as u64;
        lines.push(format!("{name} after the trailer: {got:?} (want tail {want})"));
        if got != Ok((3, want, true, true)) {
            bad.push(format!("{name}/finished"));
        }
        // the writer stays strict on such a file
        let strict = Writer::open_append(&dir, WriteOptions::default()).is_err();
        lines.push(format!("{name}: open_append refuses: {strict}"));
        if !strict {
            bad.push(format!("{name}/open_append"));
        }
        // a live reader: opened after snapshot 1, the rest and the tail arrive, refresh picks up both snapshots
        std::fs::write(dir.join(STORE_FILE), &full[..ends[0]]).unwrap();
        let mut r = Reader::open(&dir).unwrap();
        let mut b = full[..ends[2]].to_vec();
        b.extend_from_slice(&tail);
        std::fs::write(dir.join(STORE_FILE), &b).unwrap();
        let added = r.refresh().map_err(|e| e.to_string());
        let tail_now = match r.index_source() {
            IndexSource::Scanned { tail_bytes, .. } => *tail_bytes,
            IndexSource::Footer => u64::MAX,
        };
        let ok = data_ok(&mut r);
        lines.push(format!("{name}: refresh {added:?}, tail {tail_now}, data ok {ok}"));
        if added != Ok(2) || tail_now != 4096 || !ok || r.snapshots().len() != 3 {
            bad.push(format!("{name}/refresh"));
        }
    }
    for l in &lines {
        println!("C13 {l}");
    }
    assert!(bad.is_empty(), "failed: {bad:?}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&src);
}
