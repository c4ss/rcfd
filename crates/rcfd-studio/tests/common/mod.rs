//! Shared set-up of the ADR 0131 tests: temporary folders, the B1 fixture project, headless frames.
#![allow(dead_code)]

use rcfd_project::{Project, RunState, RunStatus};
use rcfd_studio::StudioApp;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The lid-driven cavity case (the `rcfd-mcp` template).
pub const CAVITY: &str = include_str!("../fixtures/cavity.toml");

/// Id of B1's half-created run folder (valid run id, only `case.snapshot`).
pub const HALF_RUN: &str = "r-20261028T120000-beef";

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

/// ADR 0131 amendment 1 item 4: 3 cases, 2 meshes, 8 runs (2 each queued, running, finished, failed) and one
/// half-created run folder.
pub fn b1_project(root: &Path) -> Project {
    let p = Project::create(root, "B1 fixture", "rcfd test").unwrap();
    std::fs::create_dir_all(root.join("cases/sub")).unwrap();
    std::fs::write(root.join("cases/a.toml"), CAVITY).unwrap();
    std::fs::write(root.join("cases/b.toml"), CAVITY.replace("nx = 16", "nx = 12")).unwrap();
    std::fs::write(root.join("cases/sub/c.toml"), CAVITY.replace("ny = 16", "ny = 20")).unwrap();
    std::fs::write(root.join("meshes/m1.msh"), b"mesh one").unwrap();
    std::fs::write(root.join("meshes/m2.msh"), b"mesh two").unwrap();
    let states = [
        RunState::Queued,
        RunState::Running,
        RunState::Finished,
        RunState::Failed,
    ];
    for i in 0..8u64 {
        let case = if i % 2 == 0 { "cases/a.toml" } else { "cases/b.toml" };
        let rec = p.create_run(case, None, "rcfd test", serde_json::json!({})).unwrap();
        let state = states[(i / 2) as usize];
        if state != RunState::Queued {
            let st = RunStatus {
                step: 10 * i,
                time: 0.5 * i as f64,
                reason: (state == RunState::Failed).then(|| "test failure".to_string()),
                ..RunStatus::new(state)
            };
            p.write_status(&rec.id, &st).unwrap();
        }
    }
    let half = root.join("runs").join(HALF_RUN);
    std::fs::create_dir_all(&half).unwrap();
    std::fs::write(half.join("case.snapshot"), CAVITY).unwrap();
    p
}

/// One headless frame of the app at 1280 x 800.
pub fn frame(app: &mut StudioApp, ctx: &egui::Context) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        ..Default::default()
    };
    ctx.run_ui(input, |ui| app.ui(ui))
}

/// Draw frames until `done` holds (draining the model each frame) or the timeout passes.
pub fn frames_until(
    app: &mut StudioApp,
    ctx: &egui::Context,
    timeout: Duration,
    mut done: impl FnMut(&StudioApp) -> bool,
) -> bool {
    let end = Instant::now() + timeout;
    loop {
        let _ = frame(app, ctx);
        if done(app) {
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
