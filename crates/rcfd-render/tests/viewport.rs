//! Registered GPU checks of the library part of ADR 0132 (wave 48C): V1 (window path equals offscreen), V6
//! (scalar-only update) and V7 (scrub cost), plus API-boundary tests of the new calls. Every check runs on every
//! DX12 / Vulkan adapter of the machine (WARP included); with no adapter the test prints a located SKIP line and
//! passes. V2 (egui composite) and V3-V5, V8, V9 (viewport model) belong to the integration part.

use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Instant;

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_render::wgpu;
use rcfd_render::{adapters, Camera, Field, GpuMesh, PartStyle, Projection, Renderer, Shading, SurfaceMesh, View};

struct Unpark(std::thread::Thread);
impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
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

/// One adapter: the offscreen renderer of ADR 0125 (`from_adapter`, its own device) and a renderer built with
/// `from_device` on a device the test owns, as the Studio will own eframe's.
struct Gpu {
    offscreen: Renderer,
    shared: Renderer,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn gpu_list() -> &'static [Gpu] {
    static G: OnceLock<Vec<Gpu>> = OnceLock::new();
    G.get_or_init(|| {
        adapters()
            .iter()
            .filter_map(|a| {
                let offscreen = Renderer::from_adapter(a).ok()?;
                let (device, queue) = block_on(a.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
                let shared = Renderer::from_device(device.clone(), queue.clone(), a.get_info()).ok()?;
                Some(Gpu {
                    offscreen,
                    shared,
                    device,
                    queue,
                })
            })
            .collect()
    })
}

/// The adapters, or a located SKIP message (and `return`) when there is none.
macro_rules! gpus {
    () => {{
        let g = gpu_list();
        if g.is_empty() {
            eprintln!(
                "SKIP {}:{}: no wgpu adapter (DX12 or Vulkan) on this machine",
                file!(),
                line!()
            );
            return;
        }
        g
    }};
}

fn name(r: &Renderer) -> String {
    let i = r.info();
    format!("{} ({:?}, {:?})", i.name, i.backend, i.device_type)
}

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*s >> 11) as f64 / (1u64 << 53) as f64
}

/// A texture the window path draws into: `Rgba8Unorm`, readable by a copy (the test) and by a sampler (egui).
fn colour_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("viewport"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

/// Read a 4-byte-per-pixel texture back (rows unpadded), written in the test, independent of the library's readback.
fn read_back(g: &Gpu, t: &wgpu::Texture) -> Vec<u8> {
    let (w, h) = (t.width(), t.height());
    let row = w as u64 * 4;
    let padded = row.div_ceil(256) * 256;
    let buf = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: padded * h as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = g.device.create_command_encoder(&Default::default());
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
    g.queue.submit([enc.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let data = buf.slice(..).get_mapped_range().unwrap();
    let mut out = Vec::with_capacity((row * h as u64) as usize);
    for r in data.chunks(padded as usize) {
        out.extend_from_slice(&r[..row as usize]);
    }
    out
}

/// Draw with `render_into` into a fresh texture of the view's size and read it back.
fn window_path(g: &Gpu, m: &GpuMesh, v: &View) -> Vec<u8> {
    let t = colour_texture(&g.device, v.width, v.height);
    g.shared.render_into(m, v, &t.create_view(&Default::default())).unwrap();
    read_back(g, &t)
}

// ---------------------------------------------------------------- the G2 scenes of ADR 0125 (tests/render.rs)

fn cube() -> SurfaceMesh {
    let m = box_mesh([4, 4, 4], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap();
    SurfaceMesh::from_mesh(&m, &[]).unwrap()
}

fn cube_view(s: &SurfaceMesh, yaw_deg: f64) -> View {
    let (lo, hi) = s.bounds();
    let cam = Camera::fit(
        lo,
        hi,
        yaw_deg.to_radians(),
        25f64.to_radians(),
        Projection::Perspective { fovy: 0.8 },
    );
    View::new(cam, 320, 240)
}

struct Scene {
    label: &'static str,
    shading: Shading,
    vertex: bool,
    parts: Vec<PartStyle>,
}

fn scenes() -> Vec<Scene> {
    vec![
        Scene {
            label: "a face unlit",
            shading: Shading::Unlit,
            vertex: false,
            parts: vec![],
        },
        Scene {
            label: "b face flat",
            shading: Shading::Flat,
            vertex: false,
            parts: vec![],
        },
        Scene {
            label: "c vertex smooth",
            shading: Shading::Smooth,
            vertex: true,
            parts: vec![],
        },
        Scene {
            label: "d vertex unlit, xmax solid, ymax hidden",
            shading: Shading::Unlit,
            vertex: true,
            // patch order of box_mesh: xmin, xmax, ymin, ymax, zmin, zmax
            parts: vec![
                PartStyle::Field,
                PartStyle::Solid([255, 0, 0]),
                PartStyle::Field,
                PartStyle::Hidden,
            ],
        },
    ]
}

fn scene_view(s: &SurfaceMesh, sc: &Scene, yaw_deg: f64) -> View {
    let mut v = cube_view(s, yaw_deg);
    v.shading = sc.shading;
    v.parts = sc.parts.clone();
    v
}

fn cube_fields(s: &SurfaceMesh) -> (Vec<f64>, Vec<f64>) {
    let mut seed = 11u64;
    let faces = (0..s.n_faces()).map(|_| lcg(&mut seed)).collect();
    let verts = s.points.iter().map(|p| p[0] + 2.0 * p[1] * p[1] - p[2]).collect();
    (faces, verts)
}

// V1: each G2 scene drawn with `from_device` + `render_into`, the texture read back in the test, is byte-identical to
// `Renderer::render` on the same adapter. Negative control: the scene at yaw 37 degrees differs.
#[test]
fn v1_window_path_equals_offscreen_render() {
    let gpus = gpus!();
    let s = cube();
    let (fv, vv) = cube_fields(&s);
    for sc in scenes() {
        let field = if sc.vertex {
            Field::Vertex(&vv)
        } else {
            Field::Face(&fv)
        };
        let v = scene_view(&s, &sc, 35.0);
        let nv = scene_view(&s, &sc, 37.0);
        for g in gpus {
            let reference = g.offscreen.render(&g.offscreen.upload(&s, field).unwrap(), &v).unwrap();
            let m = g.shared.upload(&s, field).unwrap();
            let got = window_path(g, &m, &v);
            let control = window_path(g, &m, &nv);
            let differ = |a: &[u8]| {
                a.chunks(4)
                    .zip(reference.rgba.chunks(4))
                    .filter(|(x, y)| x != y)
                    .count()
            };
            eprintln!(
                "V1 {} [{}]: pixels differing from render(): {}; negative control (yaw 37): {}",
                sc.label,
                name(&g.shared),
                differ(&got),
                differ(&control)
            );
            assert!(got == reference.rgba, "V1: render_into differs from render");
            assert!(control != reference.rgba, "V1 negative control does not discriminate");
        }
    }
}

// V6: G2 scene (c) uploaded, then `update_scalar` with the field of scene (a), rendered with scene (a)'s view: byte-
// identical to a fresh upload of scene (a). Also through `render_into` and the pick pass. Negative control: drawing
// after the update without rebinding the scalar buffer leaves the old image (differs from the fresh upload).
#[test]
fn v6_scalar_only_update_equals_fresh_upload() {
    let gpus = gpus!();
    let s = cube();
    let (fv, vv) = cube_fields(&s);
    let sc = scenes();
    let (va, vc) = (scene_view(&s, &sc[0], 35.0), scene_view(&s, &sc[2], 35.0));
    for g in gpus {
        let r = &g.shared;
        let fresh = r.upload(&s, Field::Face(&fv)).unwrap();
        let want = r.render(&fresh, &va).unwrap();
        let want_pick = r.pick(&fresh, &va).unwrap();
        let mut m = r.upload(&s, Field::Vertex(&vv)).unwrap();
        let before = r.render(&m, &va).unwrap();
        let before_c = r.render(&m, &vc).unwrap();
        r.update_scalar(&mut m, Field::Face(&fv)).unwrap();
        let got = r.render(&m, &va).unwrap();
        let got_window = window_path(g, &m, &va);
        let got_pick = r.pick(&m, &va).unwrap();
        let stale = r.render_ignoring_scalar_buffer(&m, &va).unwrap();
        // back to the field of scene (c): the image of the original upload under scene (c)'s view
        r.update_scalar(&mut m, Field::Vertex(&vv)).unwrap();
        let back_c = r.render(&m, &vc).unwrap();
        let differ = |a: &[u8]| a.chunks(4).zip(want.rgba.chunks(4)).filter(|(x, y)| x != y).count();
        eprintln!(
            "V6 [{}]: after update {} pixels differ from a fresh upload (render_into {}); negative control (no rebind) {}; before the update {}; back to (c) equal: {}",
            name(r),
            differ(&got.rgba),
            differ(&got_window),
            differ(&stale.rgba),
            differ(&before.rgba),
            back_c.rgba == before_c.rgba
        );
        assert!(got.rgba == want.rgba, "V6: update_scalar differs from a fresh upload");
        assert!(got_window == want.rgba, "V6: render_into after update_scalar differs");
        assert!(
            got_pick.ids == want_pick.ids && got_pick.values == want_pick.values,
            "V6: pick after update_scalar differs"
        );
        assert!(
            back_c.rgba == before_c.rgba,
            "V6: a second update differs from the original upload"
        );
        assert!(stale.rgba != want.rgba, "V6 negative control does not discriminate");
        assert!(before.rgba != want.rgba, "V6: the two fields give the same image");
    }
}

// API boundary of the new calls (not a registered check): wrong targets for `render_into` and wrong fields for
// `update_scalar` are located errors; a failed update leaves the mesh as it was.
#[test]
fn render_into_and_update_scalar_reject_bad_input() {
    let gpus = gpus!();
    let s = cube();
    let (fv, vv) = cube_fields(&s);
    for g in gpus {
        let r = &g.shared;
        let v = cube_view(&s, 35.0);
        let mut m = r.upload(&s, Field::Face(&fv)).unwrap();
        let tex = |w, h, format, usage| {
            g.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let ra = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let rgba = wgpu::TextureFormat::Rgba8Unorm;
        for (t, what) in [
            (tex(321, 240, rgba, ra), "size"),
            (tex(320, 240, wgpu::TextureFormat::Bgra8Unorm, ra), "format"),
            (tex(320, 240, wgpu::TextureFormat::Rgba8UnormSrgb, ra), "format"),
            (tex(320, 240, rgba, wgpu::TextureUsages::TEXTURE_BINDING), "usage"),
        ] {
            let e = r.render_into(&m, &v, &t.create_view(&Default::default())).err();
            eprintln!("render_into {what} [{}]: {e:?}", name(r));
            assert!(e.is_some_and(|e| e.to_string().contains(what)), "{what}");
        }
        assert!(r
            .render_into(&m, &v, &tex(320, 240, rgba, ra).create_view(&Default::default()))
            .is_ok());
        let before = r.render(&m, &v).unwrap();
        assert!(r.update_scalar(&mut m, Field::Face(&vv)).is_err(), "wrong length");
        let mut nan = fv.clone();
        nan[3] = f64::NAN;
        assert!(r.update_scalar(&mut m, Field::Face(&nan)).is_err(), "NaN");
        assert!(r.update_scalar(&mut m, Field::Vertex(&fv)).is_err(), "wrong length");
        assert!(
            r.render(&m, &v).unwrap().rgba == before.rgba,
            "a failed update changed the mesh"
        );
        // no field: grey, as an upload without a field
        r.update_scalar(&mut m, Field::None).unwrap();
        let none = r.render(&r.upload(&s, Field::None).unwrap(), &v).unwrap();
        assert!(r.render(&m, &v).unwrap().rgba == none.rgba, "Field::None");
    }
}

// ---------------------------------------------------------------- V7

/// Registered limit (ADR 0132 V7): median step time on `DiscreteGpu` adapters, seconds.
const STEP_LIMIT_S: f64 = 0.100;
/// The store of ADR 0124 C7: 20 f64 fields, 3 snapshots at these times, values from C7's formula.
const N_FIELDS: usize = 20;
const TIMES: [f64; 3] = [0.0, 0.5, 1.0];
const FIELD: &str = "f07";
const STEPS: usize = 20;
const CONTROL_STEPS: usize = 5;

fn value(field: usize, snap: usize, i: usize) -> f64 {
    (i as f64 * 1e-3 + field as f64).sin() * (1.0 + snap as f64) + 1e5
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// Write the C7-layout store of `mesh` into a fresh folder under the test target's temporary directory.
fn write_store(tag: &str, mesh: &rcfd_mesh::Mesh) -> std::path::PathBuf {
    use rcfd_tsstore::{Location, Precision, WriteOptions, Writer};
    let dir =
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("rcfd-render-v7-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut w = Writer::create(&dir, mesh, WriteOptions::default()).unwrap();
    let mut buf = vec![0.0; mesh.n_cells()];
    for (k, &t) in TIMES.iter().enumerate() {
        let mut sw = w.snapshot(k as u64, t).unwrap();
        for f in 0..N_FIELDS {
            for (i, x) in buf.iter_mut().enumerate() {
                *x = value(f, k, i);
            }
            sw.field(&format!("f{f:02}"), "unknown", Location::Cell, &buf, Precision::F64)
                .unwrap();
        }
        sw.commit().unwrap();
    }
    w.finish().unwrap();
    dir
}

/// One V7 arm: the store of `mesh`, the surface of `patches`, every adapter. Returns per adapter the name, the
/// type and the median step time (and, with `control`, the median of the same step with a full re-upload).
fn v7_arm(
    gpus: &[Gpu],
    tag: &str,
    mesh: rcfd_mesh::Mesh,
    patches: &[&str],
    triangles: usize,
    control: bool,
) -> Vec<(String, wgpu::DeviceType, f64, Option<f64>)> {
    let t_setup = Instant::now();
    let dir = write_store(tag, &mesh);
    drop(mesh);
    let mut reader = rcfd_tsstore::Reader::open(&dir).unwrap();
    let s = {
        let m = reader.mesh().unwrap();
        SurfaceMesh::from_mesh(&m, patches).unwrap()
    };
    assert_eq!(s.triangles().count(), triangles, "{tag}");
    let (lo, hi) = s.bounds();
    let mut v = View::new(
        Camera::fit(lo, hi, 0.4, 0.6, Projection::Perspective { fovy: 0.8 }),
        1920,
        1080,
    );
    v.shading = Shading::Flat;
    eprintln!(
        "V7 {tag} set-up: {} faces, {triangles} triangles, store written and surface built in {:.1} s",
        s.n_faces(),
        t_setup.elapsed().as_secs_f64()
    );
    let mut out = Vec::new();
    for g in gpus {
        let r = &g.shared;
        let tex = colour_texture(&g.device, v.width, v.height);
        let tv = tex.create_view(&Default::default());
        let first = s.face_values_from_cells(&reader.read_field(0, FIELD).unwrap()).unwrap();
        let mut m = r.upload(&s, Field::Face(&first)).unwrap();
        let mut k = 0usize;
        // one step: store read + face mapping + scalar update (or a full re-upload) + render into the texture,
        // timed up to the end of the GPU work
        let mut step = |m: &mut GpuMesh, reupload: bool| -> f64 {
            k = (k + 1) % TIMES.len();
            let t = Instant::now();
            let cells = reader.read_field(k, FIELD).unwrap();
            let faces = s.face_values_from_cells(&cells).unwrap();
            if reupload {
                *m = r.upload(&s, Field::Face(&faces)).unwrap();
            } else {
                r.update_scalar(m, Field::Face(&faces)).unwrap();
            }
            r.render_into(m, &v, &tv).unwrap();
            g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let dt = t.elapsed().as_secs_f64();
            assert_eq!(cells[777].to_bits(), value(7, k, 777).to_bits());
            dt
        };
        step(&mut m, false); // warm-up: the first update creates the scalar buffer
        let med = median((0..STEPS).map(|_| step(&mut m, false)).collect());
        let ctl = control.then(|| median((0..CONTROL_STEPS).map(|_| step(&mut m, true)).collect()));
        let i = r.info();
        eprintln!(
            "V7 {tag} [{}]: median step {:.1} ms over {STEPS} steps{}",
            name(r),
            med * 1e3,
            ctl.map_or(String::new(), |c| format!(
                "; negative control (full re-upload) median {:.1} ms over {CONTROL_STEPS}",
                c * 1e3
            ))
        );
        out.push((i.name.clone(), i.device_type, med, ctl));
    }
    let _ = std::fs::remove_dir_all(&dir);
    out
}

// V7: scrub cost, release. The 100^3 box store of ADR 0124 C7 (6 patches, 60 000 faces, 120 000 triangles) and the
// 1 000 000-triangle surface of ADR 0125 T1 (500 x 500 x 1 box, zmin + zmax) with a store of the same layout. One step
// = store read + face mapping + `update_scalar` + `render_into` a 1920 x 1080 texture, timed to the end of the GPU
// work. Median of 20 steps <= 100 ms on `DiscreteGpu` adapters, reported on every adapter. Negative control: the same
// step with a full re-upload on the 1M-triangle surface exceeds 100 ms.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "1M-cell store and 1M-triangle surface: the V7 time limit is for release builds"
)]
fn v7_scrub_step_cost() {
    let gpus = gpus!();
    let boxm = box_mesh([100, 100, 100], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    assert_eq!(boxm.n_cells(), 1_000_000);
    let a = v7_arm(gpus, "box100", boxm, &[], 120_000, false);
    let t1 = box_mesh([500, 500, 1], [1.0, 1.0, 0.01], [Grading::Uniform; 3]).unwrap();
    let b = v7_arm(gpus, "t1", t1, &["zmin", "zmax"], 1_000_000, true);
    for (adapter, ty, med, ctl) in a.iter().chain(&b) {
        if *ty == wgpu::DeviceType::DiscreteGpu {
            assert!(
                *med <= STEP_LIMIT_S,
                "V7 {adapter}: median step {med} s > {STEP_LIMIT_S} s"
            );
            if let Some(c) = ctl {
                assert!(
                    *c > STEP_LIMIT_S,
                    "V7 negative control does not discriminate on {adapter}: {c} s"
                );
            }
        }
    }
}
