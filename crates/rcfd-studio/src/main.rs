//! `rcfd-studio [PROJECT_DIR]`: the Studio window (ADR 0131).

use rcfd_studio::layout::default_layout;
use rcfd_studio::prefs::{config_dir, Loaded, Prefs};
use rcfd_studio::{Studio, StudioApp};
use std::path::PathBuf;

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
    let mut st = Studio::new(loaded.prefs, loaded.dock, dir.clone());
    // the Claude Code sidebar (ADR 0134): trust record and MCP config in the same per-user folder
    st.chat.bridge = rcfd_studio::chat::Bridge::from_env(dir);
    if let Some(m) = loaded.message {
        st.say(m);
    }
    let open = args.first().map(PathBuf::from);
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
            if let Some(p) = &open {
                app.st.open_project(p);
            }
            Ok(Box::new(app))
        }),
    )
}
