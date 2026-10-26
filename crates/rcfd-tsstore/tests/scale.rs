//! ADR 0124 checks C7 (random-access time bounds) and C8 (memory cap for scrubbing) on a 1M-cell mesh with 20
//! fields, release builds only. The store is about 0.7 GB on disk and is deleted afterwards.

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_tsstore::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const N_FIELDS: usize = 20;
const TIMES: [f64; 3] = [0.0, 0.5, 1.0];

/// Registered limits (ADR 0124, C7), seconds, medians of 5 repetitions in release.
const OPEN_FOOTER_S: f64 = 0.05;
const OPEN_SCAN_S: f64 = 0.5;
const NEAREST_FIELD_S: f64 = 0.1;
const LINEAR_FIELD_S: f64 = 0.2;
const SUBSET_1000_S: f64 = 0.1;
const MANY_OPEN_FOOTER_S: f64 = 0.05;
const MANY_OPEN_SCAN_S: f64 = 0.5;
const MANY_RANDOM_READ_S: f64 = 0.005;
/// Registered memory cap of a scrubbing reader process (ADR 0124, C8), bytes.
const SCRUB_CAP_BYTES: u64 = 64 << 20;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-tss-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn value(field: usize, snap: usize, i: usize) -> f64 {
    (i as f64 * 1e-3 + field as f64).sin() * (1.0 + snap as f64) + 1e5
}

fn median(f: &mut dyn FnMut() -> f64) -> f64 {
    let mut v: Vec<f64> = (0..5).map(|_| f()).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v[2]
}

fn timed(f: &mut dyn FnMut()) -> f64 {
    let t = Instant::now();
    f();
    t.elapsed().as_secs_f64()
}

/// Peak working set (Windows) or high-water resident set (Linux) of this process, bytes.
fn peak_memory() -> Option<u64> {
    if cfg!(windows) {
        let out = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {}).PeakWorkingSet64", std::process::id()),
            ])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    } else {
        let s = std::fs::read_to_string("/proc/self/status").ok()?;
        let l = s.lines().find(|l| l.starts_with("VmHWM:"))?;
        l.split_whitespace().nth(1)?.parse::<u64>().ok().map(|k| k * 1024)
    }
}

const C8_ENV: &str = "RCFD_TSS_C8_DIR";

/// Child of C8: `scrub` samples one field at 33 times (linear in time), `naive` holds a whole snapshot.
#[test]
fn c8_child_body() {
    let Ok(dir) = std::env::var(C8_ENV) else { return };
    let mode = std::env::var("RCFD_TSS_C8_MODE").unwrap();
    let mut r = Reader::open(Path::new(&dir)).unwrap();
    let mut check = 0.0;
    if mode == "scrub" {
        for k in 0..=32 {
            let t = k as f64 / 32.0;
            let s = r.field_at_time("f07", t, Mode::Linear).unwrap();
            check += s.values[12345];
        }
    } else {
        let all: Vec<Vec<f64>> = (0..N_FIELDS)
            .map(|f| r.read_field(1, &format!("f{f:02}")).unwrap())
            .collect();
        check += all.iter().map(|v| v[12345]).sum::<f64>();
    }
    println!("C8 PEAK {} CHECK {check}", peak_memory().unwrap_or(0));
}

fn child_peak(dir: &Path, mode: &str) -> u64 {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["c8_child_body", "--exact", "--nocapture", "--test-threads=1"])
        .env(C8_ENV, dir)
        .env("RCFD_TSS_C8_MODE", mode)
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "child {mode} failed: {s} {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // the harness may print "test c8_child_body ... " on the same line
    let l = s.split("C8 PEAK ").nth(1).expect("child printed its peak");
    l.split_whitespace().next().unwrap().parse().unwrap()
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "1M cells x 20 fields: timing and memory criteria are for release builds"
)]
fn c7_c8_scrubbing_a_1m_cell_store_with_20_fields() {
    let dir = tmp("c7");
    let t_mesh = Instant::now();
    let mesh = box_mesh([100, 100, 100], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let n = mesh.n_cells();
    assert_eq!(n, 1_000_000);
    let mesh_s = t_mesh.elapsed().as_secs_f64();
    let t_write = Instant::now();
    let mut w = Writer::create(&dir, &mesh, WriteOptions::default()).unwrap();
    drop(mesh);
    let mut buf = vec![0.0; n];
    for (s, &t) in TIMES.iter().enumerate() {
        let mut sw = w.snapshot(s as u64, t).unwrap();
        for f in 0..N_FIELDS {
            for (i, x) in buf.iter_mut().enumerate() {
                *x = value(f, s, i);
            }
            sw.field(&format!("f{f:02}"), "unknown", Location::Cell, &buf, Precision::F64)
                .unwrap();
        }
        sw.commit().unwrap();
    }
    w.finish().unwrap();
    let write_s = t_write.elapsed().as_secs_f64();
    let bytes = std::fs::metadata(dir.join(STORE_FILE)).unwrap().len();
    println!(
        "C7 set-up: mesh {mesh_s:.2} s, write {write_s:.2} s, store {:.1} MB",
        bytes as f64 / 1e6
    );

    let open_footer =
        median(&mut || timed(&mut || assert_eq!(Reader::open(&dir).unwrap().index_source(), &IndexSource::Footer)));
    let mut r = Reader::open(&dir).unwrap();
    let mut k = 0;
    let nearest = median(&mut || {
        k += 1;
        let t = [0.1, 0.4, 0.9, 0.6, 0.2][k % 5];
        timed(&mut || {
            let s = r.field_at_time("f07", t, Mode::Pick(Pick::Nearest)).unwrap();
            assert_eq!(s.values[777].to_bits(), value(7, s.lower, 777).to_bits());
        })
    });
    let linear = median(&mut || {
        k += 1;
        let t = 0.13 + 0.17 * (k % 5) as f64;
        timed(&mut || {
            let s = r.field_at_time("f13", t, Mode::Linear).unwrap();
            assert!(s.interpolated);
        })
    });
    let idx: Vec<usize> = (0..1000).map(|j| (j * 7919 + k * 13) % n).collect();
    let subset = median(&mut || {
        k += 1;
        timed(&mut || {
            let v = r.read_subset(k % 3, "f19", &idx).unwrap();
            assert_eq!(v[5].to_bits(), value(19, k % 3, idx[5]).to_bits());
        })
    });
    drop(r);

    // C8: memory of a separate scrubbing process (the test harness shares this one)
    let scrub_peak = child_peak(&dir, "scrub");
    let naive_peak = child_peak(&dir, "naive");
    println!(
        "C8 peak memory: scrub {:.1} MB, naive whole-snapshot reader {:.1} MB, cap {:.1} MB",
        scrub_peak as f64 / 1048576.0,
        naive_peak as f64 / 1048576.0,
        SCRUB_CAP_BYTES as f64 / 1048576.0
    );

    // footer removed: the index is rebuilt by scanning
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(dir.join(STORE_FILE))
        .unwrap();
    f.set_len(bytes - TRAILER_LEN).unwrap();
    drop(f);
    let open_scan = median(&mut || {
        timed(&mut || {
            let r = Reader::open(&dir).unwrap();
            assert!(matches!(r.index_source(), IndexSource::Scanned { .. }) && r.snapshots().len() == 3);
        })
    });
    let _ = std::fs::remove_dir_all(&dir);

    // many snapshots: 2000 snapshots of one 1000-cell field
    let dir = tmp("c7-many");
    let small = box_mesh([10, 10, 10], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let mut w = Writer::create(&dir, &small, WriteOptions::default()).unwrap();
    for s in 0..2000usize {
        let v: Vec<f64> = (0..1000).map(|i| value(0, s, i)).collect();
        let mut sw = w.snapshot(s as u64, s as f64 * 1e-3).unwrap();
        sw.field("p", "unknown", Location::Cell, &v, Precision::F64).unwrap();
        sw.commit().unwrap();
    }
    w.finish().unwrap();
    let many_footer = median(&mut || timed(&mut || assert_eq!(Reader::open(&dir).unwrap().snapshots().len(), 2000)));
    let mut r = Reader::open(&dir).unwrap();
    let many_read = median(&mut || {
        let mut seed = k as u64 * 0x9E37_79B9 + 1;
        let t = Instant::now();
        for _ in 0..100 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let s = (seed >> 33) as usize % 2000;
            let i = if s % 2 == 0 {
                r.by_time(s as f64 * 1e-3, Pick::Nearest)
            } else {
                r.by_step(s as u64, Pick::Floor)
            }
            .unwrap();
            assert_eq!(i, s);
            assert_eq!(r.read_field(i, "p").unwrap()[3].to_bits(), value(0, s, 3).to_bits());
        }
        k += 1;
        t.elapsed().as_secs_f64() / 100.0
    });
    drop(r);
    let len = std::fs::metadata(dir.join(STORE_FILE)).unwrap().len();
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(dir.join(STORE_FILE))
        .unwrap();
    f.set_len(len - TRAILER_LEN).unwrap();
    drop(f);
    let many_scan = median(&mut || timed(&mut || assert_eq!(Reader::open(&dir).unwrap().snapshots().len(), 2000)));
    let _ = std::fs::remove_dir_all(&dir);

    let rows = [
        ("open with footer (1M x 20 x 3)", open_footer, OPEN_FOOTER_S),
        ("open by scan (1M x 20 x 3)", open_scan, OPEN_SCAN_S),
        ("nearest snapshot, one 1M field", nearest, NEAREST_FIELD_S),
        ("linear in time, one 1M field", linear, LINEAR_FIELD_S),
        ("1000-cell subset", subset, SUBSET_1000_S),
        ("open with footer (2000 snapshots)", many_footer, MANY_OPEN_FOOTER_S),
        ("open by scan (2000 snapshots)", many_scan, MANY_OPEN_SCAN_S),
        ("random lookup + 1000-cell field read", many_read, MANY_RANDOM_READ_S),
    ];
    for (what, got, lim) in rows {
        println!("C7 {what}: {:.4} s (limit {lim} s)", got);
    }
    for (what, got, lim) in rows {
        assert!(got <= lim, "{what}: {got} s > {lim} s");
    }
    assert!(scrub_peak > 0, "peak memory could not be measured on this platform");
    assert!(scrub_peak <= SCRUB_CAP_BYTES, "scrub peak {scrub_peak} > cap");
    // negative control: holding one whole snapshot exceeds the cap, so the measurement discriminates
    assert!(
        naive_peak > SCRUB_CAP_BYTES,
        "naive peak {naive_peak} not above the cap"
    );
}
