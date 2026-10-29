//! `rcfd-gui` entry point.

use rcfd_gui::assistant_setup::AssistantSetup;
use rcfd_gui::controller::Controller;
use rcfd_gui::state::AppState;
use rcfd_gui::theme::Theme;
use rcfd_gui::RcfdApp;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let mut args = std::env::args().skip(1);
    let mut root: Option<PathBuf> = None;
    let mut open: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => root = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                println!("usage: rcfd-gui [--root DIR] [CASE.toml]");
                return Ok(());
            }
            other => open = Some(PathBuf::from(other)),
        }
    }
    let cases = std::env::temp_dir().join("rcfd-gui-cases");
    let mut ctrl = Controller::new(root, cases);
    let result = match &open {
        Some(p) => ctrl.open_path(p),
        None => ctrl.new_from_template(),
    };
    let mut st = AppState::new(ctrl, AssistantSetup::load_default(), Theme::Dark);
    if let Err(e) = result {
        st.ctrl.message = Some(e);
    }
    st.prefs_path = rcfd_assistant::config::config_dir().map(|d| d.join("gui-prefs.json"));
    eframe::run_native(
        "rcfd",
        rcfd_gui::ui::native_options(),
        Box::new(move |_cc| Ok(Box::new(RcfdApp::new(st)))),
    )
}
