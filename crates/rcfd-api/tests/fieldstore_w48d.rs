//! ADR 0133 R11: the opt-in `output.store_precision` key of the field store (wave 48D). Absent or `"f64"`: the bit-exact
//! store of ADR 0124, byte for byte; `"f32"`: every value rounded to the nearest single precision, within
//! `f32_error_bound`; the solver unchanged by the key. Negative control: the f32 store differs from the f64 store.

use rcfd_api::case::Case;
use rcfd_api::runctl::{self, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_tsstore::{f32_error_bound, Precision, Reader};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const NAMES: [&str; 4] = ["ux", "uy", "uz", "p"];

fn lid(extra: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 12\nny = 12\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"simplec\"\nconvection = \"upwind\"\nmax_iterations = 40\ntolerance_u = 1e-14\ntolerance_p = 1e-14\n[reports.lid_drag]\ntype = \"force\"\npatch = \"lid\"\ndirection = [1.0, 0.0, 0.0]\ncomponent = \"viscous\"\n[output]\nstore_interval = 7\n{extra}\n"
    )
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-w48d-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn drive(text: &str, out: &Path) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let m = Mutex::new(Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}")));
    let opts = RunOptions {
        out_dir: Some(out.to_path_buf()),
        ..Default::default()
    };
    let mut warnings = Vec::new();
    let o = runctl::drive(&m, &opts, &|| false, &mut |e| {
        if e["event"] == "warning" {
            warnings.push(e["message"].to_string());
        }
    });
    assert!(o.exit_code == 0 || o.exit_code == 5, "{o:?}");
    assert!(warnings.is_empty(), "{warnings:?}");
    m.into_inner().unwrap()
}

fn field_bits(sim: &Simulation) -> Vec<Vec<u64>> {
    NAMES
        .iter()
        .map(|n| sim.field(n).unwrap().iter().map(|x| x.to_bits()).collect())
        .collect()
}

fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn r11_store_precision_absent_is_f64_and_f32_rounds_within_the_bound() {
    let (d_abs, d_64, d_32) = (tmp("absent"), tmp("f64"), tmp("f32"));
    let s_abs = drive(&lid(""), &d_abs);
    let s_64 = drive(&lid("store_precision = \"f64\""), &d_64);
    let s_32 = drive(&lid("store_precision = \"f32\""), &d_32);
    // the solver does not see the key
    assert_eq!(field_bits(&s_abs), field_bits(&s_64));
    assert_eq!(field_bits(&s_abs), field_bits(&s_32));
    // absent: the store files byte-equal to "f64"
    let (fa, f64s) = (files(&d_abs.join("fields")), files(&d_64.join("fields")));
    assert_eq!(fa.len(), 2, "{:?}", fa.iter().map(|f| &f.0).collect::<Vec<_>>());
    assert!(
        fa == f64s,
        "the store without the key differs from store_precision = \"f64\""
    );
    let mut r64 = Reader::open(&d_64.join("fields")).unwrap();
    let mut r32 = Reader::open(&d_32.join("fields")).unwrap();
    assert_eq!(r64.snapshots().len(), r32.snapshots().len());
    let (mut n, mut differ, mut worst) = (0usize, 0usize, 0f64);
    for k in 0..r64.snapshots().len() {
        assert_eq!(r64.snapshots()[k].step, r32.snapshots()[k].step);
        for f in r32.fields(k).unwrap() {
            assert_eq!(f.precision, Precision::F32, "{}", f.name);
        }
        for f in r64.fields(k).unwrap() {
            assert_eq!(f.precision, Precision::F64, "{}", f.name);
        }
        for name in NAMES {
            let a = r64.read_field(k, name).unwrap();
            let b = r32.read_field(k, name).unwrap();
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(&b) {
                n += 1;
                assert_eq!(
                    y.to_bits(),
                    f64::from(*x as f32).to_bits(),
                    "{name} snapshot {k}: {x} -> {y}"
                );
                let e = (y - x).abs();
                assert!(e <= f32_error_bound(*x), "{name} snapshot {k}: {x} -> {y}, error {e}");
                if x.to_bits() != y.to_bits() {
                    differ += 1;
                    worst = worst.max(e / f32_error_bound(*x));
                }
            }
        }
    }
    let (b64, b32) = (
        std::fs::metadata(d_64.join("fields/store.rts")).unwrap().len(),
        std::fs::metadata(d_32.join("fields/store.rts")).unwrap().len(),
    );
    eprintln!(
        "R11: {} snapshots, {n} values, {differ} differ from f64 (worst error / bound {worst:.3}); store.rts {b64} bytes (f64), {b32} bytes (f32)",
        r64.snapshots().len()
    );
    // negative control: the precision took effect
    assert!(differ > 0, "the f32 store equals the f64 store");
    for d in [d_abs, d_64, d_32] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn r11_unknown_precision_is_refused_with_its_path() {
    let e = Case::parse_toml(&lid("store_precision = \"f16\""))
        .err()
        .expect("f16 is refused");
    assert!(e.iter().any(|d| d.path == "output.store_precision"), "{e:?}");
    assert!(Case::parse_toml(&lid("store_precision = \"f32\"")).is_ok());
}
