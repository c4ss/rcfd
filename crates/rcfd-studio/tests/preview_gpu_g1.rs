//! ADR 0138 (wave 48G1) GPU checks of the mesh preview of a case, on every adapter with the SKIP rule of ADR 0125: G2
//! (3D picking names the patch), G6 (the viewport texture equals an offscreen render; the 2D overlay lies on the image's
//! boundary), G10 (the frame that uploads a preview at the triangle limit).

#[macro_use]
mod vp;
mod pv;

use rcfd_api::case::Case;
use rcfd_render::{Field, PartStyle};
use rcfd_studio::preview::{self, Colouring, Status, FRONT_2D};
use rcfd_studio::viewport::{Input, ViewGpu};
use rcfd_studio::StudioApp;
use std::collections::BTreeMap;
use std::time::Instant;

fn diff(a: &[u8], b: &[u8]) -> usize {
    a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count()
}

/// The app showing the preview of `rel` on adapter `a`, drawn until the image is rendered.
fn app_showing(root: &std::path::Path, rel: &str, a: &rcfd_render::wgpu::Adapter) -> (StudioApp, egui::Context) {
    let mut st = pv::studio(root);
    st.viewport.set_gpu(ViewGpu::from_adapter(a));
    assert_eq!(pv::show_case(&mut st, rel), Status::Ready, "{rel}");
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    for _ in 0..200 {
        let _ = pv::timed_frame(&mut app, &ctx, Vec::new());
        if app.st.viewport.gpu().is_some_and(|g| g.renders >= 1) {
            return (app, ctx);
        }
    }
    panic!(
        "{rel}: no image on {}: {:?}",
        vp::adapter_name(a),
        app.st.viewport.error
    );
}

/// G2: 3D picking names the patch. The hex box from two opposite views: for every surface face whose centroid picks
/// that face, the readout names the patch that holds its volume-mesh face (from the mesh's patch ranges); all three
/// patches are seen; through the drawn app the readout of one face per patch is drawn. Negative control: the readout
/// through the reversed part list names a wrong patch at least once.
#[test]
fn g2_3d_picking_names_the_patch() {
    let root = pv::project("g2", &[]);
    let adapters = adapters_or_skip!();
    let mesh = rcfd_api::sim::build_mesh(&Case::load(root.join("cases/hex.toml")).unwrap()).unwrap();
    for a in adapters {
        let name = vp::adapter_name(a);
        let mut st = pv::studio(&root);
        st.viewport.set_gpu(ViewGpu::from_adapter(a));
        assert_eq!(pv::show_case(&mut st, "cases/hex.toml"), Status::Ready);
        let b = pv::built(&st);
        assert!(!b.two_d);
        st.viewport.set_size(800, 600, false);
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        let (mut checked, mut control) = (0usize, 0usize);
        for (yaw, pitch) in [(0.6, 0.4), (0.6 + std::f64::consts::PI, -0.4)] {
            {
                let c = st.viewport.camera.as_mut().unwrap();
                c.cam.yaw = yaw;
                c.cam.pitch = pitch;
            }
            st.viewport.input(Input::Fit);
            st.viewport.prepare();
            assert!(st.viewport.error.is_none(), "{name}: {:?}", st.viewport.error);
            for f in 0..b.surface.n_faces() {
                let Some((x, y)) = st.viewport.pixel_of(pv::centroid(&b.surface, f)) else {
                    continue;
                };
                let Some(h) = st.viewport.hover_at(x.floor() as u32, y.floor() as u32) else {
                    continue;
                };
                if h.face != f {
                    continue; // hidden or seen edge-on
                }
                let want = pv::patch_by_range(&mesh, b.surface.mesh_face[f].unwrap()).unwrap();
                let got = preview::readout(&b, Some((h.face, h.cell)), None).unwrap();
                assert!(
                    got.starts_with(&format!("patch {want} (")),
                    "{name}: face {f}: '{got}', want {want}"
                );
                let rev = b.part_patch[b.part_patch.len() - 1 - b.surface.face_part[f]];
                if b.patches[rev].name != want {
                    control += 1;
                }
                *seen.entry(want).or_default() += 1;
                checked += 1;
            }
        }
        assert_eq!(seen.len(), 3, "{name}: patches seen {seen:?}");
        assert!(
            control >= 1,
            "{name}: control: the reversed part list names every face right"
        );
        // through the drawn app: one visible face of each patch, the readout drawn
        let mut app = StudioApp::new(st, None);
        let ctx = egui::Context::default();
        let _ = pv::timed_frame(&mut app, &ctx, Vec::new());
        let _ = pv::timed_frame(&mut app, &ctx, Vec::new());
        let mut drawn_ok = Vec::new();
        for (yaw, pitch) in [(0.6, 0.4), (0.6 + std::f64::consts::PI, -0.4)] {
            {
                let c = app.st.viewport.camera.as_mut().unwrap();
                c.cam.yaw = yaw;
                c.cam.pitch = pitch;
            }
            app.st.viewport.input(Input::Fit);
            let _ = pv::timed_frame(&mut app, &ctx, Vec::new());
            for f in 0..b.surface.n_faces() {
                let want = pv::patch_by_range(&mesh, b.surface.mesh_face[f].unwrap()).unwrap();
                if drawn_ok.contains(&want) {
                    continue;
                }
                let vpm = &mut app.st.viewport;
                let Some((x, y)) = vpm.pixel_of(pv::centroid(&b.surface, f)) else {
                    continue;
                };
                if vpm.hover_at(x.floor() as u32, y.floor() as u32).map(|h| h.face) != Some(f) {
                    continue;
                }
                let (rect, size) = (vpm.image_rect.unwrap(), vpm.size);
                let pos = egui::pos2(
                    rect.min.x + (x / size[0] as f64) as f32 * rect.width(),
                    rect.min.y + (y / size[1] as f64) as f32 * rect.height(),
                );
                let _ = pv::timed_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
                let (drawn, _) = pv::timed_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
                let prefix = format!("patch {want} (");
                assert!(
                    drawn.iter().any(|t| t.starts_with(&prefix)),
                    "{name}: readout of {want} not drawn (hover {:?})",
                    app.st.viewport.hover
                );
                drawn_ok.push(want);
            }
        }
        assert_eq!(drawn_ok.len(), 3, "{name}: drawn readouts {drawn_ok:?}");
        eprintln!(
            "G2 on {name}: {checked} visible faces name their patch ({seen:?}); drawn readouts {drawn_ok:?}; control: \
             {control} wrong names through the reversed part list"
        );
    }
}

/// G6: the viewport texture of a preview equals `Renderer::render` of a fresh upload of the same surface (every face at
/// 0) under the model's view, byte for byte, for the hex box (3D, patch colours) and the cavity (2D). Negative control:
/// the render with other part styles differs. For the cavity, every boundary outline's midpoint lies on the image's
/// edge: 3 px inside is the front's colour, 3 px outside the background (control: 3 px outside is not the front's).
#[test]
fn g6_texture_equals_offscreen_and_overlay_on_the_edge() {
    let root = pv::project("g6", &[]);
    let adapters = adapters_or_skip!();
    for a in adapters {
        let name = vp::adapter_name(a);
        for rel in ["cases/hex.toml", "cases/cavity.toml"] {
            let (app, _ctx) = app_showing(&root, rel, a);
            let st = &app.st;
            let view = st.viewport.view().unwrap();
            let g = st.viewport.gpu().unwrap();
            let rd = g.renderer();
            let tex = vp::read_back(rd.device(), rd.queue(), g.texture().unwrap());
            let s = st.viewport.surface.clone().unwrap();
            let zeros = vec![0.0; s.n_faces()];
            let fresh = rd.upload(&s, Field::Face(&zeros)).unwrap();
            let want = rd.render(&fresh, &view).unwrap();
            let d = diff(&tex, &want.rgba);
            assert_eq!(d, 0, "{name} {rel}: {d} pixels differ from the offscreen render");
            let b = pv::built(st);
            let mut other = view.clone();
            other.parts = if b.two_d {
                Vec::new()
            } else {
                vec![PartStyle::Solid(preview::SINGLE); s.parts.len()]
            };
            assert_ne!(st.preview.colouring, Colouring::Single);
            let od = diff(&tex, &rd.render(&fresh, &other).unwrap().rgba);
            assert!(od > 0, "{name} {rel}: control: other part styles give the same image");
            let mut line = format!(
                "G6 on {name} {rel}: texture {}x{} equal to the offscreen render; control: {od} pixels differ",
                view.width, view.height
            );
            if b.two_d {
                let (w, h) = (view.width as usize, view.height as usize);
                let px = |x: f64, y: f64| -> [u8; 4] {
                    let (i, j) = (x.floor() as usize, y.floor() as usize);
                    assert!(i < w && j < h, "outside the image");
                    let k = 4 * (j * w + i);
                    [tex[k], tex[k + 1], tex[k + 2], tex[k + 3]]
                };
                let cam = st.viewport.camera.unwrap().cam;
                let size = [view.width, view.height];
                let (lo, hi) = s.bounds();
                let centre = [0, 1, 2].map(|d| 0.5 * (lo[d] + hi[d]));
                let c = preview::project_all(&cam, size, std::iter::once(centre))[0].unwrap();
                let front = [FRONT_2D[0], FRONT_2D[1], FRONT_2D[2], 255];
                let (mut ok, mut control) = (0usize, 0usize);
                for (_, seg) in b.outlines.iter() {
                    let mid = [0, 1, 2].map(|d| 0.5 * (seg[0][d] + seg[1][d]));
                    let p = preview::project_all(&cam, size, std::iter::once(mid))[0].unwrap();
                    let (ux, uy) = (p[0] - c[0], p[1] - c[1]);
                    let n = (ux * ux + uy * uy).sqrt();
                    let (ux, uy) = (ux / n, uy / n);
                    let inside = px(p[0] - 3.0 * ux, p[1] - 3.0 * uy);
                    let outside = px(p[0] + 3.0 * ux, p[1] + 3.0 * uy);
                    assert_eq!(inside, front, "{name}: inside the outline at {mid:?}");
                    assert_eq!(outside, view.background, "{name}: outside the outline at {mid:?}");
                    ok += 1;
                    if outside != front {
                        control += 1;
                    }
                }
                assert_eq!(ok, b.outlines.len());
                assert_eq!(control, ok, "control: 3 px outside is the front's colour");
                line.push_str(&format!(
                    "; {ok} of {} outline midpoints on the image's edge (control: {control} outside pixels not the front's)",
                    b.outlines.len()
                ));
            }
            eprintln!("{line}");
        }
    }
}

/// G10: the frame that uploads a preview at the triangle limit (a 500 x 250 box: 250,000 triangles) takes at most
/// 100 ms on `DiscreteGpu` adapters (reported on all). Negative control: the frame that uploads 1,000,000 triangles
/// (a 1000 x 500 box shown anyway) takes more than 100 ms on `DiscreteGpu` adapters.
#[test]
fn g10_upload_frame_at_the_triangle_limit() {
    if cfg!(debug_assertions) {
        eprintln!("SKIP g10 (crates/rcfd-studio/tests/preview_gpu_g1.rs): registered for release builds");
        return;
    }
    let root = pv::project(
        "g10",
        &[
            ("cases/cap.toml", pv::box_case(500, 250).as_bytes()),
            ("cases/over.toml", pv::box_case(1000, 500).as_bytes()),
        ],
    );
    let adapters = adapters_or_skip!();
    for a in adapters {
        let name = vp::adapter_name(a);
        let discrete = a.get_info().device_type == rcfd_render::wgpu::DeviceType::DiscreteGpu;
        for (rel, tri, control) in [("cases/cap.toml", 250_000, false), ("cases/over.toml", 1_000_000, true)] {
            let mut app = StudioApp::new(pv::studio(&root), None);
            app.st.viewport.set_gpu(ViewGpu::from_adapter(a));
            let ctx = egui::Context::default();
            for _ in 0..5 {
                let _ = pv::timed_frame(&mut app, &ctx, Vec::new());
            }
            app.st.open_case(rel);
            assert!(app.st.drain_until(pv::WAIT, pv::settled));
            if control {
                assert!(matches!(app.st.preview.status, Status::TooLarge { rebuild: false, .. }));
                app.st.preview.anyway();
            }
            assert!(app.st.drain_until(pv::WAIT, |s| s.viewport.surface.is_some()));
            assert_eq!(pv::built(&app.st).triangles, tri, "set-up");
            assert_eq!(app.st.viewport.gpu().unwrap().uploads, 0);
            let t = Instant::now();
            let (_, ms) = pv::timed_frame(&mut app, &ctx, Vec::new());
            let wall = t.elapsed().as_secs_f64() * 1e3;
            let g = app.st.viewport.gpu().unwrap();
            assert_eq!(
                g.uploads, 1,
                "{name} {rel}: the frame did not upload: {:?}",
                app.st.viewport.error
            );
            eprintln!(
                "G10{} on {name}: {rel}: {tri} triangles, upload frame {ms:.1} ms (wall {wall:.1} ms)",
                if control { " control" } else { "" }
            );
            if discrete {
                if control {
                    assert!(ms > 100.0, "{name}: control: {ms:.1} ms");
                } else {
                    assert!(ms <= 100.0, "{name}: {ms:.1} ms");
                }
            }
        }
    }
}
