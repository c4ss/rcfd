//! `rcfd-studio [PROJECT_DIR]`: the Studio window (ADR 0131).

use rcfd_studio::layout::default_layout;
use rcfd_studio::prefs::{config_dir, Loaded, Prefs};
use rcfd_studio::viewport::ViewGpu;
use rcfd_studio::{Studio, StudioApp};
use std::path::PathBuf;

/// Test hook (ADR 0132 integration, unregistered launch smoke): with `RCFD_STUDIO_SMOKE=<frames>` the window selects the
/// newest run once the project is open, draws that many frames, prints the adapter, the swap-chain format and the
/// viewport's state to stderr, and closes.
struct Smoke {
    app: StudioApp,
    frames: u64,
    left: u64,
}

impl eframe::App for Smoke {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if self.app.st.selected_run.is_none() {
            let newest = self
                .app
                .st
                .browser()
                .and_then(|b| b.runs.iter().map(|r| r.id.clone()).max());
            if let Some(id) = newest {
                self.app.st.select_run(&id);
            }
        }
        self.app.ui(ui);
        ui.ctx().request_repaint();
        self.left = self.left.saturating_sub(1);
        if self.left == 0 {
            let vp = &self.app.st.viewport;
            let rs = frame.wgpu_render_state();
            eprintln!(
                "rcfd-studio smoke: {} frames; adapter {:?}; window format {:?}; run {:?}; status {:?}; frame {:?}; \
                 gpu error {:?}; uploads {:?} scalar updates {:?} renders {:?}; texture {:?}; error {:?}",
                self.frames,
                rs.map(|r| {
                    let i = r.adapter.get_info();
                    format!("{} ({:?}, {:?})", i.name, i.backend, i.device_type)
                }),
                rs.map(|r| r.target_format),
                vp.run,
                vp.status,
                vp.frame.as_ref().map(|f| (f.field.clone(), f.index, f.step)),
                vp.gpu_error(),
                vp.gpu().map(|g| g.uploads),
                vp.gpu().map(|g| g.scalar_updates),
                vp.gpu().map(|g| g.renders),
                vp.gpu().and_then(|g| g.texture()).map(|t| (t.width(), t.height())),
                vp.error,
            );
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!(
            "usage: rcfd-studio [PROJECT_DIR]\n\nOpens the Studio window, and the project folder if one is given."
        );
        return Ok(());
    }
    if args.len() > 1 {
        eprintln!(
            "rcfd-studio: expected at most one argument (a project folder), got {}",
            args.len()
        );
        std::process::exit(2);
    }
    let dir = config_dir();
    let loaded = match &dir {
        Some(d) => Prefs::load(d),
        None => Loaded {
            prefs: Prefs::default(),
            dock: default_layout(),
            message: Some(
                "no configuration directory (APPDATA, XDG_CONFIG_HOME or HOME): preferences are not saved".into(),
            ),
        },
    };
    let mut st = Studio::new(loaded.prefs, loaded.dock, dir);
    if let Some(m) = loaded.message {
        st.say(m);
    }
    let open = args.first().map(PathBuf::from);
    let smoke: Option<u64> = std::env::var("RCFD_STUDIO_SMOKE")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n| n > 0);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("rcfd Studio")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rcfd Studio",
        options,
        Box::new(move |cc| {
            let mut app = StudioApp::new(st, Some(&cc.egui_ctx));
            // ADR 0132 W48-7 (a): the viewport renders on eframe's own device into a texture egui shows
            app.st.viewport.set_gpu(match cc.wgpu_render_state.as_ref() {
                Some(rs) => ViewGpu::from_render_state(rs),
                None => Err("viewport: eframe gave no wgpu render state (no adapter)".into()),
            });
            if let Some(p) = &open {
                app.st.open_project(p);
            }
            Ok(match smoke {
                Some(n) => Box::new(Smoke {
                    app,
                    frames: n,
                    left: n,
                }),
                None => Box::new(app),
            })
        }),
    )
}
