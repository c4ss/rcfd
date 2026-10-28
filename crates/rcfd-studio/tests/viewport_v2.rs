//! ADR 0132 V2: egui-wgpu draws the viewport texture into an offscreen target of a window format (`Rgba8Unorm`,
//! `Bgra8Unorm`) with eframe's renderer options; inside the image every channel is within 1 of the texture. Negative
//! control (integration amendment 1): the ramp sampled through an `Rgba8UnormSrgb` view (decoded, written unconverted)
//! differs by more than 1 in at least half of the mid-grey columns.

#[macro_use]
mod vp;

use eframe::egui_wgpu;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_render::{wgpu, Camera, Projection, SurfaceMesh};
use rcfd_studio::viewport::{min_max, CameraState, Frame, ViewGpu, Viewport};
use std::sync::Arc;

const ORIGIN: f32 = 16.0;

/// The Studio's viewport texture of the cube (G2 scene (b): per-face field, flat, yaw 35 degrees, 320 x 240), drawn by
/// the Studio code (`Viewport::prepare`) on a `ViewGpu` made with `from_device`.
fn cube_viewport(device: &wgpu::Device, queue: &wgpu::Queue, info: wgpu::AdapterInfo) -> Viewport {
    let m = box_mesh([4, 4, 4], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap();
    let s = SurfaceMesh::from_mesh(&m, &[]).unwrap();
    let (lo, hi) = s.bounds();
    let faces: Vec<f64> = (0..s.n_faces()).map(|f| (0.7 * f as f64).sin()).collect();
    let mut vp = Viewport::default();
    vp.set_gpu(ViewGpu::from_device(device.clone(), queue.clone(), info));
    vp.camera = Some(CameraState {
        cam: Camera::fit(
            lo,
            hi,
            35f64.to_radians(),
            25f64.to_radians(),
            Projection::Perspective { fovy: 0.8 },
        ),
        lo,
        hi,
    });
    vp.frame = Some(Frame {
        field: "p".into(),
        index: 0,
        upper: 0,
        weight: 0.0,
        interpolated: false,
        step: 0.0,
        time: 0.0,
        range: min_max(&faces),
        faces: Arc::new(faces),
    });
    vp.surface = Some(Arc::new(s));
    vp.set_size(320, 240, false);
    vp.prepare();
    assert!(vp.error.is_none(), "{:?}", vp.error);
    vp
}

/// A 256 x 16 grey ramp (column x holds (x, x, x, 255)) in a texture of the Studio's descriptor, optionally viewable as
/// `Rgba8UnormSrgb`.
fn ramp(device: &wgpu::Device, queue: &wgpu::Queue, srgb_view: bool) -> wgpu::Texture {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ramp"),
        size: wgpu::Extent3d {
            width: 256,
            height: 16,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: if srgb_view {
            &[wgpu::TextureFormat::Rgba8UnormSrgb]
        } else {
            &[]
        },
    });
    let mut data = Vec::with_capacity(256 * 16 * 4);
    for _ in 0..16 {
        for x in 0..256u32 {
            data.extend_from_slice(&[x as u8, x as u8, x as u8, 255]);
        }
    }
    queue.write_texture(
        t.as_image_copy(),
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(256 * 4),
            rows_per_image: Some(16),
        },
        t.size(),
    );
    t
}

/// Draw `view` (a texture of `size` pixels) with egui-wgpu into a `format` target at `ppp`; the target's bytes as RGBA
/// and the image's pixel origin.
fn composite(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &wgpu::TextureView,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    ppp: f32,
) -> (Vec<u8>, (u32, u32), (u32, u32)) {
    let mut r = egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions::default());
    let id = r.register_native_texture(device, view, wgpu::FilterMode::Nearest);
    let ctx = egui::Context::default();
    let screen = egui::vec2(400.0, 300.0);
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)),
        ..Default::default()
    };
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .native_pixels_per_point = Some(ppp);
    let rect = egui::Rect::from_min_size(
        egui::pos2(ORIGIN, ORIGIN),
        egui::vec2(size.0 as f32 / ppp, size.1 as f32 / ppp),
    );
    let out = ctx.run_ui(input, |ui| {
        ui.ctx().layer_painter(egui::LayerId::background()).image(
            id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    });
    assert_eq!(out.pixels_per_point, ppp);
    let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
    for (tid, deltas) in &out.textures_delta.set {
        for delta in deltas {
            r.update_texture(device, queue, *tid, delta);
        }
    }
    let (w, h) = ((screen.x * ppp) as u32, (screen.y * ppp) as u32);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("window"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let tv = target.create_view(&Default::default());
    let sd = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [w, h],
        pixels_per_point: ppp,
    };
    let mut enc = device.create_command_encoder(&Default::default());
    let mut cmds = r.update_buffers(device, queue, &mut enc, &prims, &sd);
    {
        let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &tv,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let mut pass = pass.forget_lifetime();
        r.render(&mut pass, &prims, &sd);
    }
    cmds.push(enc.finish());
    queue.submit(cmds);
    let mut bytes = vp::read_back(device, queue, &target);
    if format == wgpu::TextureFormat::Bgra8Unorm {
        for p in bytes.chunks_mut(4) {
            p.swap(0, 2);
        }
    }
    let o = (ORIGIN * ppp) as u32;
    (bytes, (w, h), (o, o))
}

/// Largest channel difference inside the image, and the count of pixels in `cols` differing by more than 1.
fn compare(
    tex: &[u8],
    size: (u32, u32),
    out: &[u8],
    out_w: u32,
    origin: (u32, u32),
    cols: (u32, u32),
) -> (u8, usize, usize) {
    let mut worst = 0u8;
    let (mut over, mut n) = (0usize, 0usize);
    for y in 0..size.1 {
        for x in 0..size.0 {
            let a = &tex[((y * size.0 + x) * 4) as usize..][..4];
            let b = &out[(((origin.1 + y) * out_w + origin.0 + x) * 4) as usize..][..4];
            let d = a.iter().zip(b).map(|(p, q)| p.abs_diff(*q)).max().unwrap();
            worst = worst.max(d);
            if (cols.0..=cols.1).contains(&x) {
                n += 1;
                over += usize::from(d > 1);
            }
        }
    }
    (worst, over, n)
}

#[test]
fn v2_egui_composite_of_the_viewport_texture() {
    let adapters = adapters_or_skip!();
    for a in adapters {
        let (device, queue) = vp::block_on(a.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let cube = cube_viewport(&device, &queue, a.get_info());
        let cube_tex = cube.gpu().unwrap().texture().unwrap();
        let cube_bytes = vp::read_back(&device, &queue, cube_tex);
        let ramp_tex = ramp(&device, &queue, false);
        let ramp_bytes = vp::read_back(&device, &queue, &ramp_tex);
        let ctrl_tex = ramp(&device, &queue, true);
        let ctrl_view = ctrl_tex.create_view(&wgpu::TextureViewDescriptor {
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            ..Default::default()
        });
        for format in [wgpu::TextureFormat::Rgba8Unorm, wgpu::TextureFormat::Bgra8Unorm] {
            for ppp in [1.0f32, 2.0] {
                let mut line = format!("V2 on {} {format:?} ppp {ppp}:", vp::adapter_name(a));
                for (label, tex, bytes) in [("cube", cube_tex, &cube_bytes), ("ramp", &ramp_tex, &ramp_bytes)] {
                    let size = (tex.width(), tex.height());
                    let (out, (w, _), o) = composite(
                        &device,
                        &queue,
                        &tex.create_view(&Default::default()),
                        size,
                        format,
                        ppp,
                    );
                    let (worst, _, _) = compare(bytes, size, &out, w, o, (0, 0));
                    assert!(worst <= 1, "{line} {label}: max channel difference {worst}");
                    line.push_str(&format!(" {label} max diff {worst};"));
                }
                // negative control: the ramp decoded as sRGB and written unconverted
                let (out, (w, _), o) = composite(&device, &queue, &ctrl_view, (256, 16), format, ppp);
                let (worst, over, n) = compare(&ramp_bytes, (256, 16), &out, w, o, (64, 191));
                assert!(
                    2 * over >= n,
                    "{line} control: only {over} of {n} mid-grey pixels differ by more than 1"
                );
                line.push_str(&format!(
                    " control: {over} of {n} mid-grey pixels differ by > 1 (max {worst})"
                ));
                eprintln!("{line}");
            }
        }
    }
}
