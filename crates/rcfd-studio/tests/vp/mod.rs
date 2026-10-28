//! Shared set-up of the ADR 0132 integration tests (wave 48C): store fixtures, adapters, readback, drawn text.
#![allow(dead_code)]

use rcfd_mesh::gen::cavity_2d;
use rcfd_mesh::Mesh;
use rcfd_project::{Project, RunState, RunStatus};
use rcfd_render::wgpu;
use rcfd_studio::Studio;
use rcfd_tsstore::{Location, Precision, WriteOptions, Writer};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

pub const WAIT: Duration = Duration::from_secs(30);

/// A fresh, empty temporary folder unique to this process and call.
pub fn temp(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir()
        .join("rcfd-studio-tests")
        .join(format!("{name}-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The 8 x 8 cavity of V4 (patches `lid`, `walls`, `frontAndBack`).
pub fn cavity8() -> Mesh {
    cavity_2d(8, 1.0).unwrap()
}

/// Write a store into `dir`: one snapshot per `(step, time)`, cell fields `p` = `p(c, k)` and `ux` = `cos(0.11 c) k`, F64.
pub fn write_store(dir: &Path, mesh: &Mesh, snaps: &[(u64, f64)], p: impl Fn(usize, usize) -> f64) {
    std::fs::create_dir_all(dir).unwrap();
    let mut w = Writer::create(dir, mesh, WriteOptions::default()).unwrap();
    let n = mesh.n_cells();
    for (k, &(step, time)) in snaps.iter().enumerate() {
        let pv: Vec<f64> = (0..n).map(|c| p(c, k)).collect();
        let ux: Vec<f64> = (0..n).map(|c| (0.11 * c as f64).cos() * k as f64).collect();
        let mut s = w.snapshot(step, time).unwrap();
        s.field("p", "unknown", Location::Cell, &pv, Precision::F64).unwrap();
        s.field("ux", "m/s", Location::Cell, &ux, Precision::F64).unwrap();
        s.commit().unwrap();
    }
    w.finish().unwrap();
}

/// V4's `p`.
pub fn p_v4(c: usize, k: usize) -> f64 {
    (0.37 * c as f64 + k as f64).sin() + 0.1 * k as f64
}

/// A project with one finished run whose store is at `runs/<id>/fields` (the ADR 0130 D3 contract). Returns the
/// canonical root, the run id and the store folder.
pub fn store_project(name: &str, snaps: &[(u64, f64)], p: impl Fn(usize, usize) -> f64) -> (PathBuf, String, PathBuf) {
    let root = temp(name).join("proj");
    let proj = Project::create(&root, name, "rcfd test").unwrap();
    std::fs::write(root.join("cases/cavity.toml"), include_str!("../fixtures/cavity.toml")).unwrap();
    let rec = proj.create_run("cases/cavity.toml", None, "rcfd test", serde_json::json!({})).unwrap();
    let st = RunStatus {
        step: snaps.last().map_or(0, |s| s.0),
        time: snaps.last().map_or(0.0, |s| s.1),
        ..RunStatus::new(RunState::Finished)
    };
    proj.write_status(&rec.id, &st).unwrap();
    let root = rcfd_project::paths::canonicalize(&root).unwrap();
    let dir = root.join("runs").join(&rec.id).join("fields");
    write_store(&dir, &cavity8(), snaps, p);
    (root, rec.id, dir)
}

/// A Studio (watcher and run worker off) with the project open and the run's store shown.
pub fn studio_showing(root: &Path, id: &str) -> Studio {
    let mut st = Studio::in_memory();
    st.poll = None;
    st.run_poll = None;
    st.open_project(root);
    assert!(st.drain_until(WAIT, |s| s.browser().is_some()), "the project did not open");
    st.select_run(id);
    assert!(
        st.drain_until(WAIT, |s| s.viewport.frame.is_some() && !s.viewport.pending()),
        "no frame: {:?}",
        st.viewport.status
    );
    st
}

/// Show snapshot `k` and wait for it.
pub fn show(st: &mut Studio, k: usize) {
    st.viewport.show_snapshot(k);
    assert!(st.drain_until(WAIT, |s| !s.viewport.pending()
        && s.viewport.frame.as_ref().is_some_and(|f| f.index == k)));
}

/// Move the slider and wait for the frame.
pub fn slide(st: &mut Studio, pos: f64) {
    st.viewport.set_pos(pos);
    assert!(st.drain_until(WAIT, |s| !s.viewport.pending()));
}

/// One headless frame of the app at 1280 x 800 points, with extra input events.
pub fn frame_with(app: &mut rcfd_studio::StudioApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| app.ui(ui))
}

/// One headless frame of the app at 1280 x 800 points.
pub fn frame(app: &mut rcfd_studio::StudioApp, ctx: &egui::Context) -> egui::FullOutput {
    frame_with(app, ctx, Vec::new())
}

/// The text of every drawn text shape.
pub fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

struct Unpark(std::thread::Thread);
impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Run a future on this thread (wgpu's native futures resolve during the call).
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::park();
    }
}

/// Every DX12 / Vulkan adapter (ADR 0125 step 0).
pub fn adapter_list() -> &'static [wgpu::Adapter] {
    static A: OnceLock<Vec<wgpu::Adapter>> = OnceLock::new();
    A.get_or_init(rcfd_render::adapters)
}

/// The adapters, or a located SKIP message (and `return`) when there is none. Test files declare `#[macro_use] mod vp;`.
#[allow(unused_macros)]
macro_rules! adapters_or_skip {
    () => {{
        let a = vp::adapter_list();
        if a.is_empty() {
            eprintln!(
                "SKIP {}:{}: no wgpu adapter (DX12 or Vulkan) on this machine",
                file!(),
                line!()
            );
            return;
        }
        a
    }};
}

pub fn adapter_name(a: &wgpu::Adapter) -> String {
    let i = a.get_info();
    format!("{} ({:?}, {:?})", i.name, i.backend, i.device_type)
}

/// Read a 4-byte-per-pixel texture back (rows unpadded).
pub fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, t: &wgpu::Texture) -> Vec<u8> {
    let (w, h) = (t.width(), t.height());
    let row = w as u64 * 4;
    let padded = row.div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: padded * h as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        t.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded as u32),
                rows_per_image: Some(h),
            },
        },
        t.size(),
    );
    queue.submit([enc.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let data = buf.slice(..).get_mapped_range().unwrap();
    let mut out = Vec::with_capacity((row * h as u64) as usize);
    for r in data.chunks(padded as usize) {
        out.extend_from_slice(&r[..row as usize]);
    }
    out
}
