//! Set-up of the ADR 0134 panel checks (Amendment 3): a project folder, the replay double `fake-claude` of
//! `rcfd-claude` (never the real CLI), its recorded fixtures read in place, a Claude Code home and a Studio config
//! folder outside the project.
#![allow(dead_code)]

use rcfd_claude::Claude;
use rcfd_project::Project;
use rcfd_studio::chat::{Bridge, TurnState};
use rcfd_studio::layout::{self, Panel};
use rcfd_studio::{Studio, StudioApp};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const S1: &str = "00000000-0000-4000-8000-000000000001";
pub const S2: &str = "00000000-0000-4000-8000-000000000002";
/// Open in the listing, no transcript yet (ADR 0126 D7).
pub const S3: &str = "11111111-2222-4333-8444-555555555555";
/// Transcript of another folder whose directory name collides (ADR 0126 D7).
pub const S4: &str = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";

pub const WAIT: Duration = Duration::from_secs(30);

/// `fake-claude` of `rcfd-claude`, built for this profile next to the `rcfd-studio` binary (Amendment 3 item 1). A
/// missing double fails loudly.
pub fn fake_claude() -> PathBuf {
    let studio = PathBuf::from(env!("CARGO_BIN_EXE_rcfd-studio"));
    let fake = studio.with_file_name(format!("fake-claude{}", std::env::consts::EXE_SUFFIX));
    assert!(
        fake.is_file(),
        "{} missing: cargo build -p rcfd-claude --bin fake-claude with this profile",
        fake.display()
    );
    fake
}

/// A recorded fixture of ADR 0126 (read in place).
pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rcfd-claude/tests/fixtures/v2.1.310")
        .join(name)
}

/// The fixture text with the scrubbed project folder replaced by `project` (JSON-escaped).
pub fn retarget(name: &str, project: &Path) -> String {
    let esc = serde_json::to_string(&project.to_string_lossy()).unwrap();
    std::fs::read_to_string(fixture(name))
        .unwrap()
        .replace("C:\\\\proj\\\\demo", &esc[1..esc.len() - 1])
}

/// A fresh temporary folder.
pub fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join("rcfd-studio-tests")
        .join(format!("chat-{name}-{}", rcfd_claude::new_uuid()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// One test's folders.
pub struct ChatSetup {
    pub root: PathBuf,
    /// The project folder, canonical (the root `rcfd-project` reports).
    pub project: PathBuf,
    /// Claude Code home of the fake.
    pub home: PathBuf,
    /// The Studio's config folder (outside the project).
    pub config: PathBuf,
}

impl ChatSetup {
    /// A project `my-proj` made by `Project::create`, and the fake's configuration in it: `stream` (a fixture, or a text
    /// written as is when it starts with `{`), `agents` (the listing; `None`: the listing fails), `extra` keys.
    pub fn new(name: &str, stream: Option<&str>, agents: Option<Value>, extra: Value) -> ChatSetup {
        let root = temp(name);
        let project = root.join("my-proj");
        Project::create(&project, "chat test", "rcfd test").unwrap();
        let project = rcfd_project::paths::canonicalize(&project).unwrap();
        let s = ChatSetup {
            home: root.join("home"),
            config: root.join("studio-config"),
            root,
            project,
        };
        s.configure(stream, agents, extra);
        s
    }

    /// Rewrite the fake's configuration.
    pub fn configure(&self, stream: Option<&str>, agents: Option<Value>, extra: Value) {
        let mut cfg = json!({"agents_fail": agents.is_none()});
        if let Some(st) = stream {
            let p = self.root.join("stream.jsonl");
            let text = if st.starts_with('{') {
                st.to_string()
            } else {
                retarget(st, &self.project)
            };
            std::fs::write(&p, text).unwrap();
            cfg["stream"] = p.to_string_lossy().into();
        }
        if let Some(a) = agents {
            let p = self.root.join("agents.json");
            std::fs::write(&p, a.to_string()).unwrap();
            cfg["agents"] = p.to_string_lossy().into();
        }
        for (k, v) in extra.as_object().unwrap() {
            cfg[k] = v.clone();
        }
        std::fs::write(self.project.join(".fake-claude.json"), cfg.to_string()).unwrap();
    }

    /// The transcript folder of the project in the fake home.
    pub fn transcript_dir(&self) -> PathBuf {
        let d = self
            .home
            .join("projects")
            .join(rcfd_claude::discover::project_dir_name(&self.project));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The bridge of the tests: the fake, its home, the config folder, an `rcfd-mcp` path (never started: the fake
    /// starts no MCP server).
    pub fn bridge(&self) -> Bridge {
        Bridge {
            claude: Claude::new(fake_claude(), &self.home),
            config_dir: self.config.clone(),
            rcfd_mcp: self.root.join(format!("rcfd-mcp{}", std::env::consts::EXE_SUFFIX)),
        }
    }

    /// A Studio (watcher and run worker off) with the chat on the fake, the project open, the chat's first folder
    /// check, allow-list read, MCP config and discovery received.
    pub fn studio(&self) -> Studio {
        self.studio_with(|_| {})
    }

    /// As [`ChatSetup::studio`], with `f` applied to the model before the project opens.
    pub fn studio_with(&self, f: impl FnOnce(&mut Studio)) -> Studio {
        let mut st = Studio::in_memory();
        st.poll = None;
        st.run_poll = None;
        st.chat.bridge = Ok(self.bridge());
        f(&mut st);
        st.open_project(&self.project);
        assert!(
            st.drain_until(WAIT, |s| s.browser().is_some()
                && s.chat.mcp.is_some()
                && s.chat.allow.is_some()
                && s.chat.discoveries >= 1
                && !matches!(s.chat.folder, rcfd_studio::chat::Folder::Checking)),
            "the chat did not start"
        );
        st
    }

    /// Every call the fake recorded.
    pub fn calls(&self) -> Vec<Value> {
        std::fs::read_to_string(self.project.join(".fake-claude-calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    /// Argument lists of the chat (`-p`) processes the fake recorded (not `agents --json`).
    pub fn chat_argvs(&self) -> Vec<Vec<String>> {
        self.calls()
            .into_iter()
            .map(|c| serde_json::from_value::<Vec<String>>(c["args"].clone()).unwrap())
            .filter(|a| a.first().map(String::as_str) == Some("-p"))
            .collect()
    }
}

/// A listing of `claude agents --json`.
pub fn listing(entries: &[(&str, u32, &Path)]) -> Value {
    Value::Array(
        entries
            .iter()
            .map(|(id, pid, cwd)| json!({"pid": pid, "cwd": cwd.to_string_lossy(), "kind": "interactive", "sessionId": id, "name": format!("name-{}", &id[..4]), "status": "busy"}))
            .collect(),
    )
}

/// A pid that has exited.
pub fn dead_pid() -> u32 {
    let mut c = std::process::Command::new(fake_claude())
        .current_dir(std::env::temp_dir())
        .arg("agents")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let pid = c.id();
    c.wait().unwrap();
    pid
}

/// Drain until the turn is no longer live.
pub fn finish_turn(st: &mut Studio) -> TurnState {
    assert!(
        st.drain_until(WAIT, |s| s.chat.turn.as_ref().is_some_and(|t| !t.state.is_live())),
        "the turn did not end: {:?}",
        st.chat.turn.as_ref().map(|t| t.state.clone())
    );
    st.chat.turn.as_ref().unwrap().state.clone()
}

/// The app with the Chat panel as the visible tab of its area (the case editor hidden).
pub fn app(st: Studio) -> StudioApp {
    let mut app = StudioApp::new(st, None);
    layout::hide(&mut app.st.dock, Panel::CaseEditor);
    assert!(layout::is_open(&app.st.dock, Panel::Chat));
    app
}

/// One headless frame at 1280 x 800.
pub fn frame(app: &mut StudioApp, ctx: &egui::Context) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        ..Default::default()
    };
    ctx.run_ui(input, |ui| app.ui(ui))
}

/// Every text drawn in a frame, one per line.
pub fn drawn_text(out: &egui::FullOutput) -> String {
    fn walk(s: &egui::Shape, acc: &mut String) {
        match s {
            egui::Shape::Text(t) => {
                acc.push_str(t.galley.text());
                acc.push('\n');
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, acc)),
            _ => {}
        }
    }
    let mut acc = String::new();
    for c in &out.shapes {
        walk(&c.shape, &mut acc);
    }
    acc
}

/// Draw a few frames (layout settles) and return the text of the last.
pub fn shown(app: &mut StudioApp, ctx: &egui::Context) -> String {
    let mut out = frame(app, ctx);
    for _ in 0..3 {
        out = frame(app, ctx);
    }
    drawn_text(&out)
}

/// Wait until `f` holds, polling every 20 ms; the elapsed time, or `None` after `timeout`.
pub fn wait_for(timeout: Duration, mut f: impl FnMut() -> bool) -> Option<Duration> {
    let t0 = Instant::now();
    loop {
        if f() {
            return Some(t0.elapsed());
        }
        if t0.elapsed() > timeout {
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
