//! ADR 0154 (job 48T): the measuring tools of the cylinder O-grid spoke investigation. Both tests are ignored and driven by
//! environment variables; they read run stores written elsewhere (copies, never the user's project).
//!
//! * `w48t_dump_stores`: cell centres and every snapshot's cell fields as little-endian f64 for `scripts/w48t_spoke_metrics.py`.
//! * `w48t_display_matches_store` (check D1): the Studio's own path, a project whose run holds the store, the viewport model
//!   opened on it (`StoreView`, `face_values_from_cells`, `Field::Face`), its "Save PNG" image (`snapshot_image`) and its hover
//!   readout (`hover_at`, one pick pass per image) at every pixel, compared with the stored cell value of the face's owner cell.

#[macro_use]
mod vp;

use rcfd_project::RunState;
use rcfd_studio::viewport::{CameraState, Input, ViewGpu};
use rcfd_tsstore::Reader;
use std::io::Write;
use std::path::{Path, PathBuf};

fn write_f64(path: &Path, v: &[f64]) {
    let mut f =
        std::io::BufWriter::new(std::fs::File::create(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    for x in v {
        f.write_all(&x.to_le_bytes()).unwrap();
    }
}

fn specs(var: &str) -> Vec<String> {
    std::env::var(var)
        .unwrap_or_default()
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// `W48T_DUMP` = `<store dir>=<out prefix>;...`, optional `W48T_FIELDS` = `p,ux,uy` (default `p`). Writes `<prefix>.centres.f64`
/// (cells x 3), `<prefix>.<field>.f64` (snapshots x cells, the stored values widened to f64) and `<prefix>.meta.json` (cells,
/// steps, times, the reader's index source).
#[test]
#[ignore = "ADR 0154 tool: W48T_DUMP=<store dir>=<out prefix>;... (reads stores written elsewhere)"]
fn w48t_dump_stores() {
    let fields_var = std::env::var("W48T_FIELDS").unwrap_or_else(|_| "p".into());
    let fields: Vec<&str> = fields_var.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    let list = specs("W48T_DUMP");
    assert!(!list.is_empty(), "set W48T_DUMP=<store dir>=<out prefix>;...");
    for spec in list {
        let (dir, prefix) = spec.split_once('=').unwrap_or_else(|| panic!("bad spec {spec}"));
        let mut r = Reader::open(Path::new(dir)).unwrap_or_else(|e| panic!("{dir}: {e}"));
        let mesh = r.mesh().unwrap();
        let n = mesh.n_cells();
        let centres: Vec<f64> = mesh.cell_centres().iter().flat_map(|c| c.iter().copied()).collect();
        write_f64(Path::new(&format!("{prefix}.centres.f64")), &centres);
        let snaps: Vec<(u64, f64)> = r.snapshots().iter().map(|s| (s.step, s.time)).collect();
        for f in &fields {
            let mut all = Vec::with_capacity(n * snaps.len());
            for i in 0..snaps.len() {
                let v = r
                    .read_field(i, f)
                    .unwrap_or_else(|e| panic!("{dir}: snapshot {i} field {f}: {e}"));
                assert_eq!(v.len(), n);
                all.extend(v);
            }
            write_f64(Path::new(&format!("{prefix}.{f}.f64")), &all);
        }
        let meta = serde_json::json!({
            "store": dir, "cells": n, "fields": fields,
            "steps": snaps.iter().map(|s| s.0).collect::<Vec<_>>(),
            "times": snaps.iter().map(|s| s.1).collect::<Vec<_>>(),
            "index_source": format!("{:?}", r.index_source()),
        });
        std::fs::write(
            format!("{prefix}.meta.json"),
            serde_json::to_string_pretty(&meta).unwrap(),
        )
        .unwrap();
        eprintln!(
            "dumped {dir}: {n} cells, {} snapshots, fields {fields:?} -> {prefix}",
            snaps.len()
        );
    }
}

fn copy_store(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_file() {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    let i = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[i]
}

/// D1 of ADR 0154. `W48T_RENDER` = `<store dir>|<time>|<out prefix>;...`. For each: a project with one finished run whose
/// `fields/` is a copy of the store, the Studio model showing it at the slider position `<time>` (nearest snapshot, the
/// default; not linear), on every adapter: the Studio's 2D start camera on the whole surface (`domain`) and on the box
/// [-1.2, 1.2]^2 (`zoom`), 1000 x 800 pixels, light theme. Writes `<prefix>-<view>.png` and `<prefix>.json` (snapshot, range,
/// percentiles, covered pixels, mismatches). Asserts zero mismatches: every covered pixel's picked f32 equals the stored cell
/// value of the face's owner cell as f32, and the readout value equals it as f64.
#[test]
#[ignore = "ADR 0154 tool (check D1): W48T_RENDER=<store dir>|<time>|<out prefix>;... (reads stores written elsewhere)"]
fn w48t_display_matches_store() {
    let list = specs("W48T_RENDER");
    assert!(!list.is_empty(), "set W48T_RENDER=<store dir>|<time>|<out prefix>;...");
    let adapters = adapters_or_skip!();
    let (w, h) = (1000u32, 800u32);
    let mut report = Vec::new();
    for (k, spec) in list.iter().enumerate() {
        let parts: Vec<&str> = spec.split('|').collect();
        let [store, t, prefix] = parts[..] else {
            panic!("bad spec {spec}")
        };
        let t: f64 = t.parse().unwrap();
        let (root, id, dir) = vp::run_project(&format!("w48t-{k}"), RunState::Finished, (0, t));
        copy_store(Path::new(store), &dir);
        let mut st = vp::studio_showing(&root, &id);
        vp::slide(&mut st, t);
        let frame = st.viewport.frame.clone().unwrap();
        let surface = st.viewport.surface.clone().unwrap();
        let mut r = Reader::open(&dir).unwrap();
        let cells = r.read_field(frame.index, "p").unwrap();
        let mut sorted: Vec<f64> = frame.faces.iter().copied().collect();
        sorted.sort_by(f64::total_cmp);
        let (lo, hi) = surface.bounds();
        for a in adapters {
            st.viewport.set_gpu(ViewGpu::from_adapter(a));
            for (view, blo, bhi) in [("domain", lo, hi), ("zoom", [-1.2, -1.2, lo[2]], [1.2, 1.2, hi[2]])] {
                st.viewport.camera = Some(CameraState::start(blo, bhi, true));
                st.viewport.set_size(w, h, false);
                st.viewport.input(Input::Dolly(0.0));
                st.viewport.prepare();
                assert!(st.viewport.error.is_none(), "{:?}", st.viewport.error);
                let img = st.viewport.snapshot_image().unwrap();
                let png: PathBuf = format!("{prefix}-{view}.png").into();
                std::fs::write(&png, img.to_png().unwrap()).unwrap();
                let (mut covered, mut mismatch) = (0usize, 0usize);
                for y in 0..h {
                    for x in 0..w {
                        if let Some(hv) = st.viewport.hover_at(x, y) {
                            covered += 1;
                            let stored = cells[hv.cell.expect("a 2D face has an owner cell")];
                            if hv.picked.to_bits() != (stored as f32).to_bits()
                                || hv.value.to_bits() != stored.to_bits()
                            {
                                mismatch += 1;
                            }
                        }
                    }
                }
                let row = serde_json::json!({
                    "store": store, "time": t, "view": view, "adapter": vp::adapter_name(a), "png": png,
                    "snapshot": frame.index, "snapshot_time": frame.time, "snapshot_step": frame.step,
                    "drawn_range": st.viewport.drawn_range(), "faces": frame.faces.len(),
                    "p_percentiles": {"min": sorted[0], "p01": percentile(&sorted, 0.01), "p05": percentile(&sorted, 0.05),
                        "p50": percentile(&sorted, 0.5), "p95": percentile(&sorted, 0.95), "p99": percentile(&sorted, 0.99),
                        "max": sorted[sorted.len() - 1]},
                    "covered_pixels": covered, "mismatches": mismatch,
                });
                eprintln!("D1 {row}");
                report.push(row);
                assert!(covered > 0, "{spec} {view}: no covered pixel");
                assert_eq!(mismatch, 0, "{spec} {view}: shown values differ from the stored cells");
            }
        }
        std::fs::write(
            format!("{prefix}.json"),
            serde_json::to_string_pretty(&report[report.len() - 2 * adapters.len()..]).unwrap(),
        )
        .unwrap();
    }
}
