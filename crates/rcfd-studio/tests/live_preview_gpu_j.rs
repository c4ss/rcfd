//! ADR 0144 (job 48J) GPU check, on every adapter with the SKIP rule of ADR 0125: J12, the viewport texture follows the
//! Mesh / Run toggle.

#[macro_use]
mod vp;
mod lj;
mod pv;

use rcfd_render::Field;
use rcfd_studio::preview::Status;
use rcfd_studio::view_toggle::Side;
use rcfd_studio::viewport::ViewGpu;
use rcfd_studio::StudioApp;
use std::sync::Arc;

fn diff(a: &[u8], b: &[u8]) -> usize {
    a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x != y).count()
}

/// The texture of the drawn app and `Renderer::render` of a fresh upload of the shown surface and frame under the
/// model's view (ADR 0138 G6's rule): pixels that differ, and the texture.
fn texture_vs_offscreen(app: &StudioApp) -> (usize, Vec<u8>) {
    let st = &app.st;
    let view = st.viewport.view().unwrap();
    let g = st.viewport.gpu().unwrap();
    let rd = g.renderer();
    let tex = vp::read_back(rd.device(), rd.queue(), g.texture().unwrap());
    let s = st.viewport.surface.clone().unwrap();
    let faces = st.viewport.frame.as_ref().unwrap().faces.clone();
    let fresh = rd.upload(&s, Field::Face(&faces)).unwrap();
    let want = rd.render(&fresh, &view).unwrap();
    (diff(&tex, &want.rgba), tex)
}

/// Frames until `ready` holds and one more image was rendered after the switch.
fn draw_until(app: &mut StudioApp, ctx: &egui::Context, what: &str, ready: impl Fn(&StudioApp) -> bool) {
    let r0 = app.st.viewport.gpu().map_or(0, |g| g.renders);
    for _ in 0..400 {
        let _ = lj::frame_with(app, ctx, Vec::new());
        if ready(app) && app.st.viewport.gpu().is_some_and(|g| g.renders > r0) {
            let _ = lj::frame_with(app, ctx, Vec::new());
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!(
        "{what}: not drawn: {:?} {:?}",
        app.st.viewport.status, app.st.viewport.error
    );
}

/// J12: a project with the cavity case (16 x 16) and a finished run storing the 8 x 8 cavity. In the drawn app: the case
/// opened (Mesh), the run selected (Run), the toggle to Mesh, the toggle to Run. Limit: after each switch the viewport
/// texture is byte-identical to `Renderer::render` of a fresh upload of what the model shows (the preview's surface at
/// value 0 with its patch styles, or the run's surface and field), on every adapter. Negative control: the textures
/// of the two sides differ (the comparison tells them apart).
#[test]
fn j12_the_texture_follows_the_toggle() {
    let adapters = adapters_or_skip!();
    let (root, id, _) = vp::store_project("j12", &[(1, 0.0), (2, 0.0)], vp::p_v4);
    for a in adapters {
        let name = vp::adapter_name(a);
        let mut st = lj::studio(&root, false);
        st.viewport.set_gpu(ViewGpu::from_adapter(a));
        assert_eq!(lj::open(&mut st, "cases/cavity.toml"), Status::Ready, "{name}");
        let mut app = StudioApp::new(st, None);
        let ctx = egui::Context::default();
        let mesh_ready = |a: &StudioApp| {
            a.st.viewport.run.is_none()
                && a.st
                    .viewport
                    .surface
                    .as_ref()
                    .is_some_and(|s| a.st.preview.shown.as_ref().is_some_and(|b| Arc::ptr_eq(s, &b.surface)))
        };
        let run_ready = |a: &StudioApp| {
            a.st.viewport.run.as_deref() == Some(id.as_str())
                && a.st.viewport.frame.is_some()
                && !a.st.viewport.pending()
        };
        let mut tex = Vec::new();
        let mut line = Vec::new();
        for (k, side) in [Side::Mesh, Side::Run, Side::Mesh, Side::Run].into_iter().enumerate() {
            match (k, side) {
                (0, _) => {}
                (1, _) => app.st.select_run(&id),
                (_, s) => app.st.set_view(s).unwrap(),
            }
            let what = format!("{name} step {k} ({side:?})");
            match side {
                Side::Mesh => draw_until(&mut app, &ctx, &what, mesh_ready),
                Side::Run => draw_until(&mut app, &ctx, &what, run_ready),
            }
            assert_eq!(app.st.view.side, side, "{what}");
            let (d, t) = texture_vs_offscreen(&app);
            assert_eq!(d, 0, "{what}: {d} pixels differ from the offscreen render");
            line.push(format!(
                "{side:?} {}x{} equal (uploads {})",
                app.st.viewport.size[0],
                app.st.viewport.size[1],
                app.st.viewport.gpu().unwrap().uploads
            ));
            tex.push(t);
        }
        let control = diff(&tex[0], &tex[1]);
        eprintln!(
            "J12 on {name}: {}; control: Mesh and Run textures differ in {control} pixels",
            line.join("; ")
        );
        assert!(control > 0, "{name}: control: the two sides drew the same texture");
    }
}
