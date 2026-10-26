//! ADR 0124 checks C1-C6 and C9-C12 of the time-series field store (small files; the 1M-cell timing and memory
//! checks C7/C8 are in `scale.rs`).

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_tsstore::*;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-tss-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// xorshift64* (deterministic test data).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn mesh_small() -> Mesh {
    box_mesh([3, 2, 2], [1.0, 0.5, 0.5], [Grading::Uniform; 3]).unwrap()
}

/// Deterministic values: random bit patterns (NaN payloads, infinities, subnormals, signed zeros) for "bits",
/// smooth values otherwise.
fn values(step: u64, f: &str, n: usize) -> Vec<f64> {
    if f == "bits" {
        let mut r = Rng(0x9E37_79B9_7F4A_7C15 ^ step);
        let mut v: Vec<f64> = (0..n).map(|_| f64::from_bits(r.next())).collect();
        let sp = [
            f64::NAN,
            f64::from_bits(0x7ff0_0000_0000_0001),
            -0.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            5e-324,
            -2.2e-308,
        ];
        for (x, s) in v.iter_mut().zip(sp) {
            *x = s;
        }
        return v;
    }
    (0..n)
        .map(|i| (i as f64 * 0.37 + step as f64).sin() * 1e3 + f.len() as f64)
        .collect()
}

const FIELDS: [(&str, &str, Location, Precision); 3] = [
    ("bits", "Pa", Location::Cell, Precision::F64),
    ("T", "K", Location::Point, Precision::F64),
    ("q", "unknown", Location::Cell, Precision::F32),
];

fn count(m: &Mesh, loc: Location) -> usize {
    match loc {
        Location::Cell => m.n_cells(),
        Location::Point => m.n_points(),
    }
}

/// The small reference store: 3 fields in blocks of 5 values, `n` snapshots at steps 10, 20, ... and times
/// 0.5, 1.0, ...; returns the file bytes.
fn write_small(dir: &Path, codec: Codec, n: u64) -> Vec<u8> {
    let mesh = mesh_small();
    let mut w = Writer::create(dir, &mesh, WriteOptions { block_values: 5, codec }).unwrap();
    for k in 1..=n {
        let mut s = w.snapshot(10 * k, 0.5 * k as f64).unwrap();
        for (name, unit, loc, p) in FIELDS {
            s.field(name, unit, loc, &values(10 * k, name, count(&mesh, loc)), p)
                .unwrap();
        }
        s.commit().unwrap();
    }
    w.finish().unwrap();
    std::fs::read(dir.join(STORE_FILE)).unwrap()
}

fn bits_eq(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// Every f64 field of every snapshot equals what `write_small` wrote, bit for bit.
fn small_data_ok(r: &mut Reader) -> bool {
    let mesh = mesh_small();
    for i in 0..r.snapshots().len() {
        let step = r.snapshots()[i].step;
        for (name, _, loc, p) in FIELDS {
            let Ok(got) = r.read_field(i, name) else { return false };
            let want = values(step, name, count(&mesh, loc));
            let ok = match p {
                Precision::F64 => bits_eq(&got, &want),
                Precision::F32 => got.iter().zip(&want).all(|(g, w)| (g - w).abs() <= f32_error_bound(*w)),
            };
            if !ok {
                return false;
            }
        }
    }
    true
}

#[test]
fn c1_lossless_round_trip_is_bit_exact() {
    for codec in [Codec::Raw, Codec::ShuffleDeflate] {
        let dir = tmp(&format!("c1-{codec:?}"));
        write_small(&dir, codec, 4);
        let mut r = Reader::open(&dir).unwrap();
        assert_eq!(r.index_source(), &IndexSource::Footer);
        assert_eq!(
            r.snapshots().iter().map(|s| s.step).collect::<Vec<_>>(),
            [10, 20, 30, 40]
        );
        let mesh = mesh_small();
        let back = r.mesh().unwrap();
        assert_eq!(back.content_hash(), mesh.content_hash());
        assert_eq!(back.topology(), mesh.topology());
        assert_eq!(r.mesh_hash(), mesh.content_hash());
        assert!(small_data_ok(&mut r));
        let info = r.fields(2).unwrap();
        assert_eq!(
            info.iter()
                .map(|f| (f.name.as_str(), f.unit.as_str(), f.location, f.codec))
                .collect::<Vec<_>>(),
            FIELDS.iter().map(|f| (f.0, f.1, f.2, codec)).collect::<Vec<_>>()
        );
        // partial reads: unsorted indices with repeats, across block boundaries
        let idx = [11usize, 0, 5, 4, 5, 9, 1];
        let full = r.read_field(1, "bits").unwrap();
        let sub = r.read_subset(1, "bits", &idx).unwrap();
        assert!(bits_eq(&sub, &idx.iter().map(|&k| full[k]).collect::<Vec<_>>()));
        assert!(r.read_subset(1, "bits", &[12]).is_err());
        // negative controls: the comparison is not vacuous (other snapshot differs) and f32 storage is not bit-exact
        assert!(!bits_eq(&r.read_field(2, "bits").unwrap(), &values(20, "bits", 12)));
        assert!(!bits_eq(&r.read_field(0, "q").unwrap(), &values(10, "q", 12)));
        let idx_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(INDEX_FILE)).unwrap()).unwrap();
        assert_eq!(idx_json["snapshots"].as_array().unwrap().len(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn c2_single_precision_is_within_its_registered_bound() {
    let mesh = box_mesh([20, 20, 10], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let n = mesh.n_cells();
    let mut r = Rng(7);
    let mut v: Vec<f64> = (0..n)
        .map(|_| {
            let e = (-46.0 + 84.0 * r.unit()).floor(); // 1e-46 (below the f32 subnormals) .. 3.3e38 (below f32::MAX)
            let s = if r.next() & 1 == 0 { 1.0 } else { -1.0 };
            s * (1.0 + 2.3 * r.unit()) * 10f64.powf(e)
        })
        .collect();
    v[0] = 0.0;
    v[1] = f64::NAN;
    v[2] = f64::INFINITY;
    v[3] = 1.0e-40; // f32 subnormal range
    for codec in [Codec::Raw, Codec::ShuffleDeflate] {
        let dir = tmp(&format!("c2-{codec:?}"));
        let mut w = Writer::create(
            &dir,
            &mesh,
            WriteOptions {
                block_values: 1000,
                codec,
            },
        )
        .unwrap();
        let mut s = w.snapshot(1, 0.0).unwrap();
        s.field("x", "unknown", Location::Cell, &v, Precision::F32).unwrap();
        // values beyond the f32 range are refused, not turned into infinities
        assert!(s
            .field("big", "unknown", Location::Cell, &vec![1e39; n], Precision::F32)
            .is_err());
        s.commit().unwrap();
        w.finish().unwrap();
        let mut rd = Reader::open(&dir).unwrap();
        let got = rd.read_field(0, "x").unwrap();
        let f = &rd.fields(0).unwrap()[0];
        let mut worst = 0.0f64;
        for (g, x) in got.iter().zip(&v) {
            if x.is_finite() {
                let ratio = (g - x).abs() / f.error_bound(*x);
                worst = worst.max(ratio);
            } else {
                assert!(g.is_nan() == x.is_nan() && (g.is_nan() || g == x));
            }
        }
        println!("C2 {codec:?}: worst |error| / bound = {worst:.4}");
        assert!(worst <= 1.0, "bound exceeded: {worst}");
        // negative control: half the bound is exceeded somewhere, so the bound is not vacuous
        assert!(worst > 0.5, "half bound never reached: {worst}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn c3_shuffle_deflate_shrinks_a_smooth_field() {
    let mesh = box_mesh([40, 40, 10], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let n = mesh.n_cells();
    let field: Vec<f64> = (0..n)
        .map(|i| 101325.0 + 50.0 * ((i % 40) as f64 * 0.1).sin() * ((i / 40) as f64 * 0.05).cos())
        .collect();
    let mut sizes = Vec::new();
    for codec in [Codec::Raw, Codec::ShuffleDeflate] {
        let dir = tmp(&format!("c3-{codec:?}"));
        let mut w = Writer::create(
            &dir,
            &mesh,
            WriteOptions {
                block_values: 4096,
                codec,
            },
        )
        .unwrap();
        let base = std::fs::metadata(dir.join(STORE_FILE)).unwrap().len();
        let mut s = w.snapshot(1, 0.0).unwrap();
        s.field("p", "Pa", Location::Cell, &field, Precision::F64).unwrap();
        s.commit().unwrap();
        w.finish().unwrap();
        sizes.push(std::fs::metadata(dir.join(STORE_FILE)).unwrap().len() - base);
        let mut r = Reader::open(&dir).unwrap();
        assert!(bits_eq(&r.read_field(0, "p").unwrap(), &field));
        let _ = std::fs::remove_dir_all(&dir);
    }
    let ratio = sizes[1] as f64 / sizes[0] as f64;
    println!(
        "C3 snapshot bytes raw {} deflate {} ratio {ratio:.3}",
        sizes[0], sizes[1]
    );
    assert!(ratio <= 0.9, "ratio {ratio}");
}

#[test]
fn c4_truncation_at_every_offset_keeps_every_complete_snapshot() {
    let src = tmp("c4-src");
    let full = write_small(&src, Codec::ShuffleDeflate, 3);
    let mut r = Reader::open(&src).unwrap();
    let ends: Vec<u64> = r.snapshots().iter().map(|s| s.end_offset()).collect();
    let mesh_end = r.fields(0).unwrap().iter().map(|f| f.blocks[0]).min().unwrap();
    let dir = tmp("c4");
    std::fs::create_dir_all(&dir).unwrap();
    let mut appended = 0;
    for l in 0..=full.len() {
        let _ = std::fs::remove_file(dir.join(INDEX_FILE));
        std::fs::write(dir.join(STORE_FILE), &full[..l]).unwrap();
        let opened = Reader::open(&dir);
        if (l as u64) < mesh_end {
            assert!(
                opened.is_err(),
                "length {l}: a store without a complete mesh must not open"
            );
            continue;
        }
        let mut r = opened.unwrap_or_else(|e| panic!("length {l}: {e}"));
        let want = ends.iter().filter(|&&e| e <= l as u64).count();
        assert_eq!(r.snapshots().len(), want, "length {l}");
        assert_eq!(
            matches!(r.index_source(), IndexSource::Footer),
            l == full.len(),
            "length {l}"
        );
        r.verify().unwrap_or_else(|e| panic!("length {l}: {e}"));
        assert!(small_data_ok(&mut r), "length {l}");
        drop(r);
        // a continued writer cuts the incomplete tail and adds a snapshot
        let mesh = mesh_small();
        let mut w = Writer::open_append(
            &dir,
            WriteOptions {
                block_values: 5,
                codec: Codec::Raw,
            },
        )
        .unwrap();
        let mut s = w.snapshot(1000, 100.0).unwrap();
        s.field(
            "bits",
            "Pa",
            Location::Cell,
            &values(1000, "bits", mesh.n_cells()),
            Precision::F64,
        )
        .unwrap();
        s.commit().unwrap();
        w.finish().unwrap();
        let mut r = Reader::open(&dir).unwrap();
        assert_eq!(r.index_source(), &IndexSource::Footer);
        assert_eq!(r.snapshots().len(), want + 1, "length {l}");
        assert!(bits_eq(&r.read_field(want, "bits").unwrap(), &values(1000, "bits", 12)));
        for i in 0..want {
            assert!(bits_eq(
                &r.read_field(i, "bits").unwrap(),
                &values(10 * (i as u64 + 1), "bits", 12)
            ));
        }
        r.verify().unwrap();
        appended += 1;
    }
    println!("C4 file {} bytes, mesh frame ends at {mesh_end}, snapshots end at {ends:?}, {appended} truncations re-opened and appended", full.len());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&src);
}

const C5_ENV: &str = "RCFD_TSS_C5_DIR";

fn c5_mesh() -> Mesh {
    box_mesh([10, 10, 10], [1.0; 3], [Grading::Uniform; 3]).unwrap()
}

/// Child of C5: three complete snapshots, then one field of the fourth, flushed, then waits to be killed.
#[test]
fn c5_child_body() {
    let Ok(dir) = std::env::var(C5_ENV) else { return };
    let mesh = c5_mesh();
    let n = mesh.n_cells();
    let mut w = Writer::create(
        Path::new(&dir),
        &mesh,
        WriteOptions {
            block_values: 128,
            codec: Codec::Raw,
        },
    )
    .unwrap();
    for k in 1..=3u64 {
        let mut s = w.snapshot(k, k as f64).unwrap();
        for f in ["a", "bits", "c"] {
            s.field(f, "unknown", Location::Cell, &values(k, f, n), Precision::F64)
                .unwrap();
        }
        s.commit().unwrap();
    }
    let mut s = w.snapshot(4, 4.0).unwrap();
    s.field("a", "unknown", Location::Cell, &values(4, "a", n), Precision::F64)
        .unwrap();
    s.flush().unwrap();
    println!("C5 MID");
    use std::io::Write as _;
    std::io::stdout().flush().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(120));
    panic!("C5 child was not killed");
}

#[test]
fn c5_writer_killed_mid_snapshot_leaves_earlier_snapshots_readable() {
    let dir = tmp("c5");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["c5_child_body", "--exact", "--nocapture", "--test-threads=1"])
        .env(C5_ENV, &dir)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.stdout.take().unwrap();
    let mut seen = false;
    for line in BufReader::new(out).lines() {
        if line.unwrap().contains("C5 MID") {
            seen = true;
            break;
        }
    }
    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(
        seen && !status.success(),
        "child did not reach the middle of snapshot 4 ({status})"
    );
    let n = c5_mesh().n_cells();
    let check = |r: &mut Reader, k: usize| {
        for i in 0..k {
            for f in ["a", "bits", "c"] {
                assert!(bits_eq(&r.read_field(i, f).unwrap(), &values(i as u64 + 1, f, n)));
            }
        }
    };
    let mut r = Reader::open(&dir).unwrap();
    // negative control: there is no footer and the interrupted snapshot left bytes behind
    let IndexSource::Scanned { tail_bytes, reason } = r.index_source().clone() else {
        panic!("a killed writer cannot have written a footer")
    };
    println!(
        "C5 recovered {} snapshots, tail {tail_bytes} bytes ({reason})",
        r.snapshots().len()
    );
    assert!(tail_bytes > 0);
    assert_eq!(r.snapshots().len(), 3);
    r.verify().unwrap();
    check(&mut r, 3);
    drop(r);
    let mut w = Writer::open_append(&dir, WriteOptions::default()).unwrap();
    let mut s = w.snapshot(4, 4.0).unwrap();
    for f in ["a", "bits", "c"] {
        s.field(f, "unknown", Location::Cell, &values(4, f, n), Precision::F64)
            .unwrap();
    }
    s.commit().unwrap();
    w.finish().unwrap();
    let mut r = Reader::open(&dir).unwrap();
    assert_eq!(r.index_source(), &IndexSource::Footer);
    check(&mut r, 4);
    let _ = std::fs::remove_dir_all(&dir);
}

const C5B_ENV: &str = "RCFD_TSS_C5B_DIR";

/// Child of C5b: writes snapshots until killed.
#[test]
fn c5b_child_body() {
    let Ok(dir) = std::env::var(C5B_ENV) else { return };
    let mesh = c5_mesh();
    let n = mesh.n_cells();
    let mut w = Writer::create(
        Path::new(&dir),
        &mesh,
        WriteOptions {
            block_values: 64,
            codec: Codec::ShuffleDeflate,
        },
    )
    .unwrap();
    // until killed (the parent kills after a few snapshots)
    for k in 1u64..=u64::MAX {
        let mut s = w.snapshot(k, k as f64 * 0.1).unwrap();
        for f in ["a", "bits", "c", "d"] {
            s.field(f, "unknown", Location::Cell, &values(k, f, n), Precision::F64)
                .unwrap();
        }
        s.commit().unwrap();
    }
}

#[test]
fn c5b_writer_killed_at_a_random_moment_leaves_earlier_snapshots_readable() {
    let n = c5_mesh().n_cells();
    for round in 0..3 {
        let dir = tmp(&format!("c5b-{round}"));
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["c5b_child_body", "--exact", "--test-threads=1"])
            .env(C5B_ENV, &dir)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let t0 = std::time::Instant::now();
        loop {
            if let Ok(r) = Reader::open(&dir) {
                if r.snapshots().len() >= 5 + 7 * round {
                    break;
                }
            }
            assert!(t0.elapsed().as_secs() < 120, "child too slow");
        }
        child.kill().unwrap();
        child.wait().unwrap();
        let mut r = Reader::open(&dir).unwrap();
        let k = r.snapshots().len();
        assert!(k >= 5 + 7 * round);
        r.verify().unwrap();
        for i in 0..k {
            for f in ["a", "bits", "c", "d"] {
                assert!(bits_eq(&r.read_field(i, f).unwrap(), &values(i as u64 + 1, f, n)));
            }
        }
        println!(
            "C5b round {round}: {k} snapshots recovered, index {:?}",
            r.index_source()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Byte offsets of the small store that no checksum covers (ADR 0124 C6): none.
const UNCHECKED: &[usize] = &[];

#[test]
fn c6_every_flipped_byte_is_detected() {
    for codec in [Codec::Raw, Codec::ShuffleDeflate] {
        let src = tmp(&format!("c6-src-{codec:?}"));
        let good = write_small(&src, codec, 3);
        let dir = tmp(&format!("c6-{codec:?}"));
        std::fs::create_dir_all(&dir).unwrap();
        // control: the intact file opens from its footer, verifies and holds the written data
        std::fs::write(dir.join(STORE_FILE), &good).unwrap();
        let mut r = Reader::open(&dir).unwrap();
        assert_eq!(r.index_source(), &IndexSource::Footer);
        r.verify().unwrap();
        assert!(small_data_ok(&mut r));
        drop(r);
        let (mut by_error, mut by_flag, mut undetected) = (0, 0, Vec::new());
        for mask in [0xffu8, 0x01] {
            for i in 0..good.len() {
                let mut b = good.clone();
                b[i] ^= mask;
                std::fs::write(dir.join(STORE_FILE), &b).unwrap();
                let Ok(mut r) = Reader::open(&dir) else {
                    by_error += 1;
                    continue;
                };
                let scanned = matches!(r.index_source(), IndexSource::Scanned { .. });
                match r.verify() {
                    Err(_) => by_error += 1,
                    Ok(()) if scanned => {
                        // flagged fallback: the data must still be exactly what was written
                        assert!(
                            small_data_ok(&mut r) && r.snapshots().len() == 3,
                            "byte {i}: flagged but wrong data"
                        );
                        by_flag += 1;
                    }
                    Ok(()) => undetected.push(i),
                }
            }
        }
        println!(
            "C6 {codec:?}: {} bytes x 2 masks, {by_error} rejected by a checksum or structure check, {by_flag} flagged (footer/trailer: index rebuilt by scan), undetected {undetected:?}",
            good.len()
        );
        assert_eq!(undetected, UNCHECKED);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&src);
    }
}

#[test]
fn c6b_magic_and_version_are_checked() {
    let dir = tmp("c6b");
    let good = write_small(&dir, Codec::Raw, 1);
    let mut b = good.clone();
    b[8..12].copy_from_slice(&2u32.to_le_bytes());
    let s = rcfd_core::hash::fnv64(&b[..12]);
    b[12..20].copy_from_slice(&s.to_le_bytes());
    std::fs::write(dir.join(STORE_FILE), &b).unwrap();
    let e = Reader::open(&dir).err().unwrap().to_string();
    assert!(e.contains("unsupported store version 2"), "{e}");
    let mut b = good.clone();
    b[0] = b'X';
    std::fs::write(dir.join(STORE_FILE), &b).unwrap();
    assert!(Reader::open(&dir).err().unwrap().to_string().contains("magic"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// ADR 0124 C9 bound for linear interpolation of a field linear in time.
fn c9_bound(a: f64, b: f64, t: f64, v0: f64, v1: f64) -> f64 {
    8.0 * f64::EPSILON * (a.abs() + (b * t).abs() + v0.abs() + v1.abs())
}

#[test]
fn c9_linear_interpolation_is_exact_for_fields_linear_in_time() {
    let mesh = mesh_small();
    let n = mesh.n_cells();
    let mut rng = Rng(99);
    // A: dyadic coefficients and times (exact arithmetic), B: random reals and irregular times
    let dy_a: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let dy_b: Vec<f64> = (0..n).map(|i| (i as f64 - 5.0) * 0.25).collect();
    let ra: Vec<f64> = (0..n).map(|_| 2e3 * rng.unit() - 1e3).collect();
    let rb: Vec<f64> = (0..n).map(|_| 2e3 * rng.unit() - 1e3).collect();
    let mut rt = vec![0.0];
    for _ in 0..6 {
        let last = *rt.last().unwrap();
        rt.push(last + 0.01 + rng.unit());
    }
    let lin = |a: &[f64], b: &[f64], t: f64| a.iter().zip(b).map(|(a, b)| a + b * t).collect::<Vec<f64>>();
    let quad = |t: f64| (0..n).map(|i| i as f64 + t * t).collect::<Vec<f64>>();
    for (case, times) in [
        ("dyadic", (0..5).map(|k| 0.5 * k as f64).collect::<Vec<_>>()),
        ("random", rt.clone()),
    ] {
        let dir = tmp(&format!("c9-{case}"));
        let (a, b) = if case == "dyadic" { (&dy_a, &dy_b) } else { (&ra, &rb) };
        let mut w = Writer::create(&dir, &mesh, WriteOptions::default()).unwrap();
        for (k, &t) in times.iter().enumerate() {
            let mut s = w.snapshot(k as u64, t).unwrap();
            s.field("lin", "unknown", Location::Cell, &lin(a, b, t), Precision::F64)
                .unwrap();
            s.field("quad", "unknown", Location::Cell, &quad(t), Precision::F64)
                .unwrap();
            s.commit().unwrap();
        }
        w.finish().unwrap();
        let mut r = Reader::open(&dir).unwrap();
        let (t_first, t_last) = (times[0], *times.last().unwrap());
        let mut worst = 0.0f64;
        let (mut quad_violations, mut nearest_violations, mut queries) = (0, 0, 0);
        for q in 0..200 {
            let t = if case == "dyadic" {
                t_first + (t_last - t_first) * (q as f64 / 256.0)
            } else {
                t_first + (t_last - t_first) * rng.unit()
            };
            let s = r.field_at_time("lin", t, Mode::Linear).unwrap();
            let i0 = r.by_time(t, Pick::Floor).unwrap();
            assert_eq!(s.interpolated, times[i0] != t);
            let want = lin(a, b, t);
            let (v0, v1) = (
                r.read_field(s.lower, "lin").unwrap(),
                r.read_field(s.upper, "lin").unwrap(),
            );
            for c in 0..n {
                let err = (s.values[c] - want[c]).abs();
                if case == "dyadic" {
                    assert_eq!(err, 0.0, "dyadic case must be exact (t = {t})");
                }
                worst = worst.max(err / c9_bound(a[c], b[c], t, v0[c], v1[c]).max(f64::MIN_POSITIVE));
            }
            if s.interpolated {
                queries += 1;
                let sq = r.field_at_time("quad", t, Mode::Linear).unwrap();
                let wq = quad(t);
                if (0..n).any(|c| (sq.values[c] - wq[c]).abs() > c9_bound(c as f64, 0.0, t, wq[c], wq[c])) {
                    quad_violations += 1;
                }
                let sn = r.field_at_time("lin", t, Mode::Pick(Pick::Nearest)).unwrap();
                if (0..n).any(|c| (sn.values[c] - want[c]).abs() > c9_bound(a[c], b[c], t, v0[c], v1[c])) {
                    nearest_violations += 1;
                }
            }
        }
        // at stored times: stored values, bit for bit, not flagged
        for (k, &t) in times.iter().enumerate() {
            let s = r.field_at_time("lin", t, Mode::Linear).unwrap();
            assert!(!s.interpolated && s.lower == k);
            assert!(bits_eq(&s.values, &r.read_field(k, "lin").unwrap()));
        }
        assert!(r.field_at_time("lin", t_last + 1.0, Mode::Linear).is_err());
        println!("C9 {case}: worst error / bound = {worst:.3e}; {queries} interpolated queries, quadratic-field violations {quad_violations}, nearest-pick violations {nearest_violations}");
        assert!(worst <= 1.0);
        // negative controls: the bound rejects a field quadratic in time and a nearest-snapshot read
        assert_eq!(quad_violations, queries);
        assert_eq!(nearest_violations, queries);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn c10_retention_keeps_every_kth_snapshot_bit_exact() {
    for (n, k, want) in [
        (10u64, 3usize, vec![10u64, 40, 70, 100]),
        (11, 3, vec![10, 40, 70, 100, 110]),
        (4, 1, vec![10, 20, 30, 40]),
    ] {
        let src = tmp(&format!("c10-src-{n}-{k}"));
        let dst = tmp(&format!("c10-dst-{n}-{k}"));
        write_small(&src, Codec::ShuffleDeflate, n);
        let kept = retain_every(&src, &dst, k).unwrap();
        assert_eq!(kept.iter().map(|s| s.step).collect::<Vec<_>>(), want);
        let mut r = Reader::open(&dst).unwrap();
        assert_eq!(r.index_source(), &IndexSource::Footer);
        r.verify().unwrap();
        assert!(small_data_ok(&mut r));
        let mut s = Reader::open(&src).unwrap();
        for (i, step) in want.iter().enumerate() {
            let j = s.by_step(*step, Pick::Floor).unwrap();
            for (name, ..) in FIELDS {
                assert!(bits_eq(
                    &r.read_field(i, name).unwrap(),
                    &s.read_field(j, name).unwrap()
                ));
            }
        }
        // negative control: a dropped snapshot is gone
        if k > 1 {
            assert_eq!(r.by_step(20, Pick::Floor).map(|i| r.snapshots()[i].step), Some(10));
        }
        assert!(
            std::fs::metadata(dst.join(STORE_FILE)).unwrap().len()
                < std::fs::metadata(src.join(STORE_FILE)).unwrap().len()
                || k == 1
        );
        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
    }
    assert!(retain_every(Path::new("."), &tmp("c10-zero"), 0).is_err());
}

#[test]
fn c11_lookup_by_step_and_time_and_writer_preconditions() {
    let dir = tmp("c11");
    let mesh = mesh_small();
    let n = mesh.n_cells();
    let mut w = Writer::create(&dir, &mesh, WriteOptions::default()).unwrap();
    for (step, t) in [(1u64, 0.0), (2, 1.0), (3, 1.0), (4, 2.0)] {
        let mut s = w.snapshot(step, t).unwrap();
        s.field("p", "unknown", Location::Cell, &vec![step as f64; n], Precision::F64)
            .unwrap();
        s.commit().unwrap();
    }
    // preconditions (negative controls of the writer)
    assert!(w.snapshot(4, 3.0).is_err(), "step must increase");
    assert!(w.snapshot(5, 1.5).is_err(), "time must not decrease");
    assert!(w.snapshot(5, f64::NAN).is_err());
    {
        let mut s = w.snapshot(5, 3.0).unwrap();
        assert!(s
            .field("p", "unknown", Location::Cell, &vec![0.0; n + 1], Precision::F64)
            .is_err());
        assert!(s
            .field("p", "unknown", Location::Point, &vec![0.0; n], Precision::F64)
            .is_err());
        s.field("p", "unknown", Location::Cell, &vec![0.0; n], Precision::F64)
            .unwrap();
        assert!(
            s.field("p", "unknown", Location::Cell, &vec![0.0; n], Precision::F64)
                .is_err(),
            "duplicate name"
        );
        // dropped without commit: never visible
    }
    w.finish().unwrap();
    assert!(
        Writer::create(&dir, &mesh, WriteOptions::default()).is_err(),
        "an existing store is not replaced"
    );
    let r = Reader::open(&dir).unwrap();
    assert_eq!(r.snapshots().len(), 4);
    use Pick::*;
    assert_eq!(r.by_time(1.0, Floor), Some(2));
    assert_eq!(r.by_time(0.4, Nearest), Some(0));
    assert_eq!(r.by_time(0.5, Nearest), Some(0));
    assert_eq!(r.by_time(0.6, Nearest), Some(1));
    assert_eq!(r.by_time(-1.0, Floor), None);
    assert_eq!(r.by_time(-1.0, Nearest), Some(0));
    assert_eq!(r.by_time(9.0, Floor), Some(3));
    assert_eq!(r.by_time(f64::NAN, Floor), None);
    assert_eq!(r.by_step(3, Floor), Some(2));
    assert_eq!(r.by_step(0, Floor), None);
    assert_eq!(r.by_step(0, Nearest), Some(0));
    assert_eq!(r.by_step(100, Nearest), Some(3));
    let mut r = r;
    let s = r.field_at_time("p", 1.5, Mode::Linear).unwrap();
    assert!(s.interpolated && s.lower == 2 && s.upper == 3 && s.values[0] == 3.5);
    let s = r.field_at_time("p", 1.0, Mode::Linear).unwrap();
    assert!(!s.interpolated && s.lower == 2 && s.values[0] == 3.0);
    assert!(r.field_at_time("nope", 1.0, Mode::Linear).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn c12_live_reader_picks_up_new_snapshots() {
    let dir = tmp("c12");
    let mesh = mesh_small();
    let n = mesh.n_cells();
    let mut w = Writer::create(&dir, &mesh, WriteOptions::default()).unwrap();
    let put = |w: &mut Writer, k: u64| {
        let mut s = w.snapshot(k, k as f64).unwrap();
        s.field("p", "unknown", Location::Cell, &values(k, "p", n), Precision::F64)
            .unwrap();
        s.commit().unwrap();
    };
    put(&mut w, 1);
    put(&mut w, 2);
    let mut r = Reader::open(&dir).unwrap();
    assert!(matches!(r.index_source(), IndexSource::Scanned { tail_bytes: 0, .. }));
    assert_eq!(r.snapshots().len(), 2);
    put(&mut w, 3);
    assert_eq!(r.refresh().unwrap(), 1);
    assert!(bits_eq(&r.read_field(2, "p").unwrap(), &values(3, "p", n)));
    assert_eq!(r.refresh().unwrap(), 0);
    w.finish().unwrap();
    let r = Reader::open(&dir).unwrap();
    assert_eq!(r.index_source(), &IndexSource::Footer);
    let _ = std::fs::remove_dir_all(&dir);
}
