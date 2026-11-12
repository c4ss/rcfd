//! ADR 0350 amendment 1 (F-447), control column C: what `rcfd-studio` takes for the equivalents of P4 to P8 on the data
//! sets of the Electron spike, release profile, offscreen at 1280 x 800 on the first adapter whose name holds "RTX" (else
//! the first). No window, no egui pass, no presentation: the Studio's data and draw path only.
//!
//! An ignored heavy record, run once by hand with the data folder of `f447_bench gen` in `F447_DATA`:
//! `cargo test -p rcfd-studio --release --test f447_control -- --ignored --nocapture`.

#[macro_use]
mod vp;

use rcfd_project::RunState;
use rcfd_render::{wgpu, SurfaceMesh};
use rcfd_studio::viewport::{Input, ViewGpu};
use rcfd_studio::Studio;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const W: u32 = 1280;
const H: u32 = 800;

fn stats(name: &str, ms: &[f64]) {
    let mut s = ms.to_vec();
    s.sort_by(f64::total_cmp);
    let rank = |q: f64| s[((q * s.len() as f64).ceil() as usize).clamp(1, s.len()) - 1];
    let total: f64 = ms.iter().sum();
    eprintln!(
        "F447-C {name}: n {} p50 {:.3} ms p99 {:.3} ms max {:.3} ms total {:.1} ms rate {:.2} per s",
        s.len(),
        rank(0.5),
        rank(0.99),
        s[s.len() - 1],
        total,
        1e3 * s.len() as f64 / total
    );
}

fn adapter() -> Option<&'static wgpu::Adapter> {
    let all = vp::adapter_list();
    all.iter().find(|a| a.get_info().name.contains("RTX")).or(all.first())
}

fn wait_gpu(st: &Studio) {
    let gpu = st.viewport.gpu().expect("a GPU");
    gpu.renderer()
        .device()
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll");
}

/// A project with one finished run whose store is a copy of `<data>/project/runs/<name>/fields/store.rts`.
fn project_of(data: &Path, name: &str) -> (PathBuf, String) {
    let (root, id, dir) = vp::run_project(&format!("f447-{name}"), RunState::Finished, (20, 0.0));
    std::fs::create_dir_all(&dir).unwrap();
    let from = data
        .join("project")
        .join("runs")
        .join(name)
        .join("fields")
        .join("store.rts");
    std::fs::copy(&from, dir.join("store.rts")).unwrap_or_else(|e| panic!("{}: {e}", from.display()));
    (root, id)
}

fn studio(root: &Path, a: &wgpu::Adapter) -> Studio {
    let mut st = Studio::in_memory();
    st.poll = None;
    st.run_poll = None;
    st.open_project(root);
    assert!(
        st.drain_until(vp::WAIT, |s| s.browser().is_some()),
        "the project did not open"
    );
    st.viewport.set_gpu(ViewGpu::from_adapter(a));
    st.viewport.set_size(W, H, false);
    st
}

/// `select_run` to the first prepared image.
fn open_run(st: &mut Studio, id: &str) -> f64 {
    let t = Instant::now();
    st.select_run(id);
    assert!(
        st.drain_until(3 * vp::WAIT, |s| s.viewport.frame.is_some() && !s.viewport.pending()),
        "no frame: {:?}",
        st.viewport.status
    );
    st.viewport.prepare();
    assert!(st.viewport.error.is_none(), "{:?}", st.viewport.error);
    wait_gpu(st);
    t.elapsed().as_secs_f64() * 1e3
}

/// `show_snapshot(k)` to the prepared image.
fn show(st: &mut Studio, k: usize) -> f64 {
    let t = Instant::now();
    vp::show(st, k);
    st.viewport.prepare();
    wait_gpu(st);
    t.elapsed().as_secs_f64() * 1e3
}

/// The sheet z = 0.25 sin(3 pi x) cos(3 pi y) over the unit square, 1000 x 500 quadrilaterals cut in two, three own
/// corners per triangle (ADR 0350 amendment 1, A1.2).
fn soup() -> SurfaceMesh {
    let (nx, ny) = (1000usize, 500usize);
    let z = |x: f64, y: f64| 0.25 * (3.0 * std::f64::consts::PI * x).sin() * (3.0 * std::f64::consts::PI * y).cos();
    let p = |i: usize, j: usize| {
        let (x, y) = (i as f64 / nx as f64, j as f64 / ny as f64);
        [x, y, z(x, y)]
    };
    let mut s = SurfaceMesh {
        face_offsets: vec![0],
        parts: vec!["soup".to_string()],
        ..Default::default()
    };
    for j in 0..ny {
        for i in 0..nx {
            for tri in [
                [(i, j), (i + 1, j), (i + 1, j + 1)],
                [(i, j), (i + 1, j + 1), (i, j + 1)],
            ] {
                for (a, b) in tri {
                    s.face_points.push(s.points.len());
                    s.point_id.push(s.points.len());
                    s.points.push(p(a, b));
                }
                s.face_offsets.push(s.face_points.len());
                s.face_part.push(0);
                s.mesh_face.push(None);
                s.cell.push(None);
            }
        }
    }
    s
}

#[test]
#[ignore = "heavy record of ADR 0350 (F-447): needs the data sets in F447_DATA and a GPU; run once by hand"]
fn f447_control_record() {
    let Ok(data) = std::env::var("F447_DATA") else {
        eprintln!("SKIP f447_control_record: F447_DATA is not set");
        return;
    };
    let data = PathBuf::from(data);
    let Some(a) = adapter() else {
        eprintln!("SKIP f447_control_record: no wgpu adapter");
        return;
    };
    eprintln!("F447-C adapter: {}", vp::adapter_name(a));

    // C7 (five openings of the 1e6 run), then C4 and C5 on the last one
    let (root, id) = project_of(&data, "box-1e6-raw");
    let mut c7 = Vec::new();
    let mut st = studio(&root, a);
    for rep in 0..5 {
        if rep > 0 {
            st = studio(&root, a);
        }
        c7.push(open_run(&mut st, &id));
    }
    eprintln!("F447-C C7 first {:.1} ms, all {:?}", c7[0], c7);
    stats("C7 open 1e6 run to first image", &c7);
    let f = st.viewport.frame.clone().unwrap();
    eprintln!(
        "F447-C shown: field {} snapshot {} of {}, {} faces, {} cells",
        f.field,
        f.index,
        st.viewport.snaps.len(),
        f.faces.len(),
        st.viewport.cells.unwrap_or(0)
    );
    assert_eq!(st.viewport.snaps.len(), 20);
    for i in 0..20 {
        show(&mut st, i % 20);
    }
    let c4: Vec<f64> = (0..200).map(|i| show(&mut st, i % 20)).collect();
    stats("C4 snapshot to prepared image, 1e6", &c4);
    for k in 0..20 {
        show(&mut st, k);
    }
    let c5: Vec<f64> = (0..200).map(|i| show(&mut st, i % 20)).collect();
    stats("C5 scrubbing 10 passes, 1e6", &c5);
    drop(st);

    // C6
    let (root, id) = project_of(&data, "box-4e6-raw");
    let mut st = studio(&root, a);
    let first = open_run(&mut st, &id);
    eprintln!("F447-C C6 open 4e6 run to first image: {first:.1} ms");
    for k in 0..20 {
        show(&mut st, k);
    }
    let c6: Vec<f64> = (0..100).map(|i| show(&mut st, i % 20)).collect();
    stats("C6 scrubbing 5 passes, 4e6", &c6);
    drop(st);

    // C8
    let mut st = Studio::in_memory();
    st.poll = None;
    st.run_poll = None;
    let s = soup();
    assert_eq!(s.n_faces(), 1_000_000);
    st.viewport.show_surface(Arc::new(s), false, Vec::new());
    st.viewport.set_gpu(ViewGpu::from_adapter(a));
    st.viewport.set_size(W, H, false);
    st.viewport.prepare();
    assert!(st.viewport.error.is_none(), "{:?}", st.viewport.error);
    wait_gpu(&st);
    let frame = |st: &mut Studio| {
        let t = Instant::now();
        st.viewport.input(Input::Orbit(2.0, 0.0));
        st.viewport.prepare();
        wait_gpu(st);
        t.elapsed().as_secs_f64() * 1e3
    };
    for _ in 0..120 {
        frame(&mut st);
    }
    let c8: Vec<f64> = (0..1200).map(|_| frame(&mut st)).collect();
    assert!(st.viewport.error.is_none(), "{:?}", st.viewport.error);
    stats("C8 orbit frame of the 1e6-triangle soup", &c8);
}
