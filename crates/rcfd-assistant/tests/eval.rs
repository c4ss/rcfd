//! F-094: deterministic assistant workflow eval with the mock provider.
//!
//! Five scripted scenarios drive the real agent loop, tool layer, approval gate and solver service. The
//! "model" is a script (some replies are computed from earlier tool results, as a model would), so this
//! measures the tool layer, approvals, diagnostics and journals, not the competence of any real model.
//! The pass criteria C1..C8 are registered in docs/roadmap.json (F-094 acceptance) and must not be edited
//! after results are seen. Run `cargo test -p rcfd-assistant --test eval -- --nocapture` for the report.

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_assistant::config::{build_provider, AssistantConfig, KeyStore, MemoryKeyStore, ProviderConfig, ProviderKind};
use rcfd_assistant::http::{FakeTransport, Transport};
use rcfd_assistant::mock::MockProvider;
use rcfd_assistant::tools::ToolOutput;
use rcfd_assistant::*;
use rcfd_mcp::policy::{Access, PERMISSION_DENIED};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------- scripts

fn calls(text: &str, c: Vec<(&str, &str, Value)>) -> ChatResponse {
    ChatResponse::tool_calls(text, c)
}

fn calls_owned(text: &str, c: Vec<(String, String, Value)>) -> ChatResponse {
    let mut blocks = vec![];
    if !text.is_empty() {
        blocks.push(Block::Text(text.to_string()));
    }
    for (id, name, input) in c {
        blocks.push(Block::ToolUse { id, name, input });
    }
    ChatResponse {
        blocks,
        stop: StopReason::ToolUse,
        usage: Usage::default(),
    }
}

/// Parsed JSON of the tool results in the newest user message (Null where not JSON).
fn last_results(req: &ChatRequest) -> Vec<Value> {
    req.messages
        .last()
        .map(|m| {
            m.blocks
                .iter()
                .filter_map(|b| match b {
                    Block::ToolResult { content, .. } => Some(serde_json::from_str(content).unwrap_or(Value::Null)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn find_key<'a>(rs: &'a [Value], key: &str) -> Option<&'a Value> {
    rs.iter().find(|r| r.get(key).is_some())
}

fn sets(prefix: &str, items: Vec<(String, Value)>) -> Vec<(String, String, Value)> {
    items
        .into_iter()
        .enumerate()
        .map(|(i, (p, v))| {
            (
                format!("{prefix}{i}"),
                "set_setting".to_string(),
                json!({"path": p, "value": v}),
            )
        })
        .collect()
}

const CAVITY_BROKEN: &str = "title = \"cavity, stretched mesh\"\n[mesh]\ngenerator = \"box_2d\"\nnx = 400\nny = 2\nlx = 1.0\nly = 1.0\n[physics]\nnu = 0.01\n[boundaries.ymax]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.xmin]\nvelocity = \"fixed\"\n[boundaries.xmax]\nvelocity = \"fixed\"\n[boundaries.ymin]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"simplec\"\nmax_iterations = 60\n";

const CHANNEL_NO_WALLS: &str = "title = \"k-epsilon channel\"\n[mesh]\ngenerator = \"channel_2d\"\nnx = 24\nny = 12\nlx = 3.0\nly = 0.2\ndepth = 0.1\n[physics]\nnu = 1e-5\n[solver]\nalgorithm = \"simplec\"\nconvection = \"upwind\"\nalpha_u = 0.7\nalpha_p = 0.3\nmax_iterations = 30\n[turbulence]\nmodel = \"k_epsilon\"\nvariant = \"rng\"\nk_initial = 0.01\nepsilon_initial = 0.01\n";

const NOZZLE: &str = "title = \"nozzle\"\n[mesh]\ngenerator = \"nozzle_2d\"\nnx = 40\nny = 8\nlx = 2.0\nly = 0.1\nthroat_height = 0.05\nexit_height = 0.1\n[solver]\nalgorithm = \"compressible\"\nend_time = 0.002\n[compressible]\nflux = \"hllc\"\n[compressible.initial.left]\ndensity = 1.15\nvelocity = [60.0, 0.0, 0.0]\npressure = 98000.0\n";

fn s_cavity(m: &MockProvider) {
    m.push(calls(
        "Starting from the lid-driven cavity template.",
        vec![("a1", "create_case", json!({"template": "lid_driven_cavity"}))],
    ));
    m.push(calls(
        "Re = U L / nu with a unit lid speed and unit cavity: Re 400 needs nu = 0.0025.",
        vec![
            ("a2", "set_setting", json!({"path": "physics.nu", "value": 0.0025})),
            (
                "a3",
                "set_setting",
                json!({"path": "solver.max_iterations", "value": 300}),
            ),
        ],
    ));
    m.push(calls(
        "Checking the case and mesh.",
        vec![("a4", "validate_case", json!({})), ("a5", "assess_mesh", json!({}))],
    ));
    m.push(calls(
        "Running.",
        vec![("a6", "run", json!({"max_steps": 300, "wait_seconds": 120}))],
    ));
    m.push(calls(
        "Reading the residuals.",
        vec![("a7", "explain_residuals", json!({}))],
    ));
    m.push_fn(|req| {
        let v = last_results(req);
        ChatResponse::text(format!(
            "Cavity at Re 400 ran; verdict: {}.",
            v.first().and_then(|x| x["verdict"].as_str()).unwrap_or("unknown")
        ))
    });
}

fn setup_cavity_diverging(tb: &Toolbox) {
    user(tb, "create_case", json!({"template": "lid_driven_cavity"}));
    for (k, v) in [
        ("solver.algorithm", json!("simple")),
        ("solver.alpha_u", json!(1.0)),
        ("solver.alpha_p", json!(1.0)),
        ("solver.convection", json!("linear_upwind")),
        ("physics.nu", json!(0.0025)),
        ("solver.max_iterations", json!(150)),
    ] {
        user(tb, "set_setting", json!({"path": k, "value": v}));
    }
    user(tb, "run", json!({"max_steps": 150, "wait_seconds": 120}));
}

fn s_diverge(m: &MockProvider) {
    m.push(calls(
        "Looking at the residual history.",
        vec![("b1", "explain_residuals", json!({}))],
    ));
    m.push_fn(|req| {
        let v = last_results(req);
        let changes = v
            .first()
            .and_then(|x| x["suggested_changes"].as_array().cloned())
            .unwrap_or_default();
        let items = changes
            .iter()
            .map(|c| (c["path"].as_str().unwrap_or("").to_string(), c["value"].clone()))
            .collect();
        calls_owned("Applying the suggested stabilisation.", sets("b2_", items))
    });
    m.push(calls(
        "Re-running with the new settings.",
        vec![("b3", "run", json!({"max_steps": 150, "wait_seconds": 120}))],
    ));
    m.push(calls("Checking again.", vec![("b4", "explain_residuals", json!({}))]));
    m.push(calls("Validating.", vec![("b5", "validate_case", json!({}))]));
    m.push_fn(|req| {
        let v = last_results(req);
        ChatResponse::text(format!(
            "After the change the verdict is {}.",
            v.first().and_then(|x| x["verdict"].as_str()).unwrap_or("unknown")
        ))
    });
}

fn s_channel(m: &MockProvider) {
    // First attempt omits the wall entry and is rejected by validation (a tool error, no state change).
    m.push(calls(
        "A k-epsilon channel with inlet, outlet and walls.",
        vec![("c1", "create_case", json!({"toml": CHANNEL_NO_WALLS}))],
    ));
    let fixed = format!("{CHANNEL_NO_WALLS}[boundaries.walls]\nvelocity = \"fixed\"\n");
    m.push_fn(move |_| {
        calls(
            "The case needs a wall patch for k-epsilon; adding it.",
            vec![("c2", "create_case", json!({"toml": fixed.clone()}))],
        )
    });
    m.push(calls(
        "What is missing?",
        vec![
            ("c3", "validate_case", json!({})),
            ("c4", "suggest_boundary_conditions", json!({})),
        ],
    ));
    m.push_fn(|req| {
        let v = last_results(req);
        let mut items = vec![];
        if let Some(sug) = find_key(&v, "suggestions").and_then(|s| s["suggestions"].as_array()) {
            for s in sug.iter().filter(|s| s["status"] == "missing") {
                for e in s["settings"].as_array().into_iter().flatten() {
                    items.push((e["path"].as_str().unwrap_or("").to_string(), e["value"].clone()));
                }
            }
        }
        calls_owned("Applying the proposed boundary conditions.", sets("c5_", items))
    });
    m.push(calls("Validating.", vec![("c6", "validate_case", json!({}))]));
    m.push(calls(
        "A short run to see that it starts.",
        vec![("c7", "run", json!({"max_steps": 30, "wait_seconds": 120}))],
    ));
    m.push_fn(|_| ChatResponse::text("Boundary conditions chosen and the case runs."));
}

fn s_nozzle(m: &MockProvider) {
    m.push(calls(
        "A converging-diverging nozzle with the compressible solver.",
        vec![("d1", "create_case", json!({"toml": NOZZLE}))],
    ));
    m.push(calls(
        "Which patches lack conditions?",
        vec![
            ("d2", "validate_case", json!({})),
            ("d3", "suggest_boundary_conditions", json!({})),
        ],
    ));
    m.push_fn(|req| {
        // Name-based suggestions cannot know xmin is the inlet: the model sets the characteristic
        // conditions itself and takes the slip walls from the suggestions.
        let v = last_results(req);
        let mut items: Vec<(String, Value)> = vec![
            ("boundaries.xmin.compressible_bc".into(), json!("total_inlet")),
            ("boundaries.xmin.total_pressure_value".into(), json!(101325.0)),
            ("boundaries.xmin.temperature_value".into(), json!(300.0)),
            ("boundaries.xmin.flow_direction".into(), json!([1.0, 0.0, 0.0])),
            ("boundaries.xmax.compressible_bc".into(), json!("pressure_outlet")),
            ("boundaries.xmax.pressure_value".into(), json!(98000.0)),
        ];
        if let Some(sug) = find_key(&v, "suggestions").and_then(|s| s["suggestions"].as_array()) {
            for s in sug
                .iter()
                .filter(|s| matches!(s["patch"].as_str(), Some("ymin" | "ymax")))
            {
                for e in s["settings"].as_array().into_iter().flatten() {
                    items.push((e["path"].as_str().unwrap_or("").to_string(), e["value"].clone()));
                }
            }
        }
        calls_owned(
            "Total-pressure inlet at 101325 Pa / 300 K, 98 kPa outlet, slip walls.",
            sets("d4_", items),
        )
    });
    m.push(calls("Validating.", vec![("d5", "validate_case", json!({}))]));
    m.push(calls(
        "Running.",
        vec![("d6", "run", json!({"max_steps": 200, "wait_seconds": 120}))],
    ));
    m.push_fn(|_| ChatResponse::text("Nozzle set up with a total-pressure inlet and run."));
}

fn setup_stretched_mesh(tb: &Toolbox) {
    user(tb, "create_case", json!({"toml": CAVITY_BROKEN}));
}

fn s_mesh(m: &MockProvider) {
    m.push(calls("Checking the mesh.", vec![("e1", "assess_mesh", json!({}))]));
    m.push_fn(|req| {
        let v = last_results(req);
        let verdict = v.first().and_then(|x| x["verdict"].as_str()).unwrap_or("");
        let items = if verdict == "good" {
            vec![]
        } else {
            vec![("mesh.nx".to_string(), json!(24)), ("mesh.ny".to_string(), json!(24))]
        };
        calls_owned(
            "The aspect ratio is too high; using a uniform 24x24 mesh.",
            sets("e2_", items),
        )
    });
    m.push(calls("Re-checking.", vec![("e3", "assess_mesh", json!({}))]));
    m.push(calls("Validating.", vec![("e4", "validate_case", json!({}))]));
    m.push(calls(
        "Running.",
        vec![("e5", "run", json!({"max_steps": 60, "wait_seconds": 120}))],
    ));
    m.push_fn(|_| ChatResponse::text("Mesh fixed and the case runs."));
}

fn s_malformed(m: &MockProvider) {
    m.push(calls(
        "",
        vec![("m1", "create_case", json!({"template": "lid_driven_cavity"}))],
    ));
    m.push(calls(
        "",
        vec![
            ("m2", "frobnicate", json!({})),
            ("m3", "set_setting", json!({ INVALID_ARGS_KEY: "{path: " })),
            ("m4", "set_setting", json!({"path": 5, "value": 1})),
            ("m5", "set_setting", json!({"value": 1})),
            ("m6", "get_setting", json!({"pth": "mesh.nx"})),
        ],
    ));
    m.push(ChatResponse::text(
        "I could not make those calls; no changes were made.",
    ));
}

struct Scenario {
    name: &'static str,
    prompt: &'static str,
    setup: Option<fn(&Toolbox)>,
    script: fn(&MockProvider),
    check: fn(&Outcome) -> Vec<String>,
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "cavity_re400",
            prompt: "Set up a lid-driven cavity at Re 400, check it and run it.",
            setup: None,
            script: s_cavity,
            check: check_cavity,
        },
        Scenario {
            name: "diagnose_divergence",
            prompt: "My cavity run has diverging residuals. Diagnose it, fix it and re-run.",
            setup: Some(setup_cavity_diverging),
            script: s_diverge,
            check: check_diverge,
        },
        Scenario {
            name: "channel_k_epsilon_bcs",
            prompt: "Pick boundary conditions for a channel with k-epsilon and make sure it runs.",
            setup: None,
            script: s_channel,
            check: check_channel,
        },
        Scenario {
            name: "compressible_nozzle",
            prompt: "Set up a compressible nozzle with a total-pressure inlet and run it.",
            setup: None,
            script: s_nozzle,
            check: check_nozzle,
        },
        Scenario {
            name: "mesh_triage",
            prompt: "Check my mesh and fix whatever is wrong with it, then run.",
            setup: Some(setup_stretched_mesh),
            script: s_mesh,
            check: check_mesh,
        },
    ]
}

// ---------------------------------------------------------------- harness

/// The user acting directly (not through the agent): grant and execute.
fn user(tb: &Toolbox, name: &str, args: Value) -> ToolOutput {
    tb.grant(name, &args);
    let o = tb.execute(name, &args);
    assert!(!o.is_error, "setup call {name} failed: {}", o.text);
    o
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Policy {
    ApproveAll,
    DenyAll,
    /// Approve `create_case` only, deny everything else.
    ApproveCreateOnly,
}

#[derive(Clone, Debug)]
enum Ev {
    Approval {
        id: String,
        name: String,
        read_access: bool,
    },
    Result {
        id: String,
        name: String,
        args: Value,
        is_error: bool,
        denied: bool,
        code: Option<i64>,
        text: String,
    },
}

impl Ev {
    fn json(&self) -> Value {
        match self {
            Ev::Approval { id, name, read_access } => {
                json!({"event": "approval_requested", "id": id, "tool": name, "read_access": read_access})
            }
            Ev::Result {
                id,
                name,
                args,
                is_error,
                denied,
                code,
                text,
            } => json!({"event": "tool_result", "id": id, "tool": name, "args": args, "is_error": is_error,
                        "denied": denied, "code": code, "text": text}),
        }
    }
}

struct Outcome {
    svc: Arc<Service>,
    tb: Arc<Toolbox>,
    events: Vec<Ev>,
    transcript: Value,
    turn: Result<String, AgentError>,
    /// Tool calls the model issued, in order.
    calls: Vec<(String, Value)>,
    /// Service state captured when the first call was denied (or at the first create_case result).
    snap_at_mark: Option<Value>,
    initial: Value,
    final_state: Value,
}

/// Sessions and their settings trees.
fn state(svc: &Arc<Service>) -> Value {
    let list = svc.call("session.list", &json!({})).unwrap();
    let mut out = vec![];
    for s in list["sessions"].as_array().unwrap() {
        let id = s["session"].as_str().unwrap();
        let v = svc.call("settings.get", &json!({"session": id})).unwrap();
        out.push(json!({"session": id, "settings": v["value"]}));
    }
    json!(out)
}

fn run_scenario(sc: &Scenario, policy: Policy) -> Outcome {
    let svc = Service::new(ServiceConfig::default());
    let tb = Arc::new(Toolbox::new(svc.clone(), "cases", ApprovalMode::ApproveWrites));
    if let Some(s) = sc.setup {
        s(&tb);
    }
    let mock = Arc::new(MockProvider::empty());
    (sc.script)(&mock);
    let mut agent = Agent::new(mock, tb.clone(), "eval-model");
    let initial = state(&svc);
    let mut events: Vec<Ev> = vec![];
    let mut mark: Option<Value> = None;
    let turn = {
        let svc_d = svc.clone();
        let tb_d = tb.clone();
        let svc_s = svc.clone();
        let mark_cell = std::cell::RefCell::new(&mut mark);
        let events_ref = std::cell::RefCell::new(&mut events);
        agent.run_turn(
            sc.prompt,
            &mut |p| {
                let allow = match policy {
                    Policy::ApproveAll => true,
                    Policy::DenyAll => false,
                    Policy::ApproveCreateOnly => p.name == "create_case",
                };
                let _ = &tb_d;
                if !allow && mark_cell.borrow().is_none() {
                    **mark_cell.borrow_mut() = Some(state(&svc_d));
                }
                if allow {
                    Decision::Approve
                } else {
                    Decision::Deny("eval: refused".into())
                }
            },
            &mut |e| match e {
                AgentEvent::ApprovalRequested(p) => {
                    events_ref.borrow_mut().push(Ev::Approval {
                        id: p.id,
                        name: p.name,
                        read_access: p.access == Access::Read,
                    });
                }
                AgentEvent::ToolResult {
                    id,
                    name,
                    args,
                    output,
                    denied,
                } => {
                    let code = output.structured["error"]["code"].as_i64();
                    events_ref.borrow_mut().push(Ev::Result {
                        id,
                        name,
                        args,
                        is_error: output.is_error,
                        denied,
                        code,
                        text: output.text,
                    });
                    let _ = &svc_s;
                }
                AgentEvent::Text(_) => {}
            },
        )
    };
    let calls: Vec<(String, Value)> = agent
        .history()
        .iter()
        .flat_map(|m| &m.blocks)
        .filter_map(|b| match b {
            Block::ToolUse { name, input, .. } => Some((name.clone(), input.clone())),
            _ => None,
        })
        .collect();
    let final_state = state(&svc);
    Outcome {
        svc,
        tb,
        events,
        transcript: agent.transcript(),
        turn,
        calls,
        snap_at_mark: mark,
        initial,
        final_state,
    }
}

fn tool_access(o: &Outcome, name: &str) -> Option<Access> {
    o.tb.info(name).map(|i| i.access)
}

fn setting(o: &Outcome, path: &str) -> Value {
    let sid = o.tb.session().expect("session");
    o.svc
        .call("settings.get", &json!({"session": sid, "path": path}))
        .map(|v| v["value"].clone())
        .unwrap_or(Value::Null)
}

fn results<'a>(o: &'a Outcome, tool: &'a str) -> impl Iterator<Item = &'a Ev> + 'a {
    o.events
        .iter()
        .filter(move |e| matches!(e, Ev::Result { name, .. } if name == tool))
}

fn result_json(e: &Ev) -> Value {
    match e {
        Ev::Result { text, .. } => serde_json::from_str(text).unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

// ---- scenario-specific checks (part of C1)

fn check_cavity(o: &Outcome) -> Vec<String> {
    let mut f = vec![];
    let nu = setting(o, "physics.nu").as_f64().unwrap_or(0.0);
    if (1.0 / nu - 400.0).abs() > 1e-6 {
        f.push(format!("Re = 1/nu = {} (want 400)", 1.0 / nu));
    }
    f
}

fn check_diverge(o: &Outcome) -> Vec<String> {
    let mut f = vec![];
    let verdicts: Vec<String> = results(o, "explain_residuals")
        .map(|e| result_json(e)["verdict"].as_str().unwrap_or("?").to_string())
        .collect();
    if verdicts.first().map(String::as_str) != Some("diverging") {
        f.push(format!("first verdict {verdicts:?}, want diverging"));
    }
    if verdicts.len() != 2 || verdicts[1] == "diverging" {
        f.push(format!(
            "verdicts {verdicts:?}: the repair did not remove the divergence"
        ));
    }
    f
}

fn check_channel(o: &Outcome) -> Vec<String> {
    let mut f = vec![];
    let creates: Vec<&Ev> = results(o, "create_case").collect();
    if !(creates.len() == 2 && matches!(creates[0], Ev::Result { is_error: true, .. })) {
        f.push("expected a rejected first create_case and an accepted second".into());
    }
    for k in ["turbulence_spec", "turbulence_intensity", "turbulence_length"] {
        if setting(o, &format!("boundaries.inlet.{k}")).is_null() {
            f.push(format!("inlet {k} not set"));
        }
    }
    if setting(o, "boundaries.outlet.pressure").as_str() != Some("fixed") {
        f.push("outlet pressure not fixed".into());
    }
    f
}

fn check_nozzle(o: &Outcome) -> Vec<String> {
    let mut f = vec![];
    if setting(o, "boundaries.xmin.compressible_bc").as_str() != Some("total_inlet") {
        f.push("xmin is not a total_inlet".into());
    }
    if setting(o, "boundaries.xmin.total_pressure_value").as_f64() != Some(101325.0) {
        f.push("total pressure not 101325".into());
    }
    f
}

fn check_mesh(o: &Outcome) -> Vec<String> {
    let mut f = vec![];
    let verdicts: Vec<String> = results(o, "assess_mesh")
        .map(|e| result_json(e)["verdict"].as_str().unwrap_or("?").to_string())
        .collect();
    if verdicts != ["usable with care", "good"] {
        f.push(format!(
            "assess_mesh verdicts {verdicts:?}, want [usable with care, good]"
        ));
    }
    f
}

// ---- criteria

/// C1: validates, no patch without BC, the run finishes with finite residuals; plus scenario checks.
fn c1(sc: &Scenario, o: &Outcome) -> Vec<String> {
    let mut f = vec![];
    if let Err(e) = &o.turn {
        return vec![format!("turn failed: {e}")];
    }
    let v = o.tb.execute("validate_case", &json!({}));
    if v.is_error || v.structured["ok"] != true {
        f.push(format!("validate_case not ok: {}", v.text));
    }
    let missing = v.structured["warnings"]
        .as_array()
        .map(|w| {
            w.iter()
                .filter(|x| {
                    x["message"]
                        .as_str()
                        .is_some_and(|m| m.contains("has no boundary condition"))
                })
                .count()
        })
        .unwrap_or(0);
    if missing > 0 {
        f.push(format!("{missing} patches without a boundary entry"));
    }
    let sid = o.tb.session().unwrap_or_default();
    match o.svc.call("run.status", &json!({"session": sid})) {
        Ok(st) => {
            // Registered C1: the run "reaches its step limit or convergence". The step limit is the
            // max_steps of the last run call (transient solvers are not "done" at a step limit).
            let limit = o
                .calls
                .iter()
                .rev()
                .find(|(n, _)| n == "run")
                .and_then(|(_, a)| a["max_steps"].as_u64())
                .unwrap_or(u64::MAX);
            let it = st["simulation"]["iteration"].as_u64().unwrap_or(0);
            if st["simulation"]["done"] != true && it < limit {
                f.push(format!(
                    "run neither done nor at its step limit {limit}: {}",
                    st["simulation"]
                ));
            }
        }
        Err(e) => f.push(format!("run.status: {}", e.message)),
    }
    match o.svc.call("monitors.get", &json!({"session": sid})) {
        Ok(m) => {
            let series = m["series"].as_object().cloned().unwrap_or_default();
            if series.is_empty() {
                f.push("no monitor series".into());
            }
            for (k, s) in &series {
                let last = s["y"].as_array().and_then(|a| a.last()).and_then(Value::as_f64);
                if !last.is_some_and(f64::is_finite) {
                    f.push(format!("series {k} last value {last:?} not finite"));
                }
            }
        }
        Err(e) => f.push(format!("monitors.get: {}", e.message)),
    }
    f.extend((sc.check)(o));
    f
}

/// C2: approval accounting from the event log.
fn c2(o: &Outcome) -> Vec<String> {
    let mut f = vec![];
    for (i, e) in o.events.iter().enumerate() {
        match e {
            Ev::Approval { id, name, read_access } => {
                if *read_access {
                    f.push(format!("approval requested for read tool {name}"));
                }
                let n = o
                    .events
                    .iter()
                    .filter(|x| matches!(x, Ev::Approval { id: j, .. } if j == id))
                    .count();
                if n != 1 {
                    f.push(format!("{n} approval requests for call {id}"));
                }
                let r = o.events[i + 1..]
                    .iter()
                    .filter(|x| matches!(x, Ev::Result { id: j, .. } if j == id))
                    .count();
                if r != 1 {
                    f.push(format!("call {id} has {r} results after its approval"));
                }
            }
            Ev::Result {
                id,
                name,
                is_error,
                denied,
                code,
                ..
            } => {
                let writes = tool_access(o, name).is_some_and(|a| a != Access::Read);
                if writes && !denied {
                    let approved_before = o.events[..i]
                        .iter()
                        .any(|x| matches!(x, Ev::Approval { id: j, .. } if j == id));
                    // A write call refused before reaching approval (bad arguments) never ran.
                    let refused_early = *is_error && matches!(code, Some(-32602 | -32601));
                    if !approved_before && !refused_early {
                        f.push(format!("write tool {name} ({id}) ran without an approval request"));
                    }
                }
            }
        }
    }
    f
}

/// C3: denied variants leave state unchanged and report code 1100.
fn c3(o: &Outcome, policy: Policy) -> Vec<String> {
    let mut f = vec![];
    if let Err(e) = &o.turn {
        return vec![format!("turn failed: {e}")];
    }
    let denied: Vec<&Ev> = o
        .events
        .iter()
        .filter(|e| matches!(e, Ev::Result { denied: true, .. }))
        .collect();
    if denied.is_empty() {
        f.push("nothing was denied".into());
    }
    for e in &denied {
        if let Ev::Result {
            name, is_error, code, ..
        } = e
        {
            if !(*is_error && *code == Some(PERMISSION_DENIED)) {
                f.push(format!("denied {name} gave code {code:?}"));
            }
        }
    }
    let reference = match policy {
        Policy::DenyAll => &o.initial,
        _ => o.snap_at_mark.as_ref().unwrap_or(&Value::Null),
    };
    if *reference != o.final_state {
        f.push("state after the turn differs from the state at the first denial".into());
    }
    if results(o, "run").any(|e| {
        matches!(
            e,
            Ev::Result {
                is_error: false,
                denied: false,
                ..
            }
        )
    }) {
        f.push("a run was started".into());
    }
    f
}

/// C4: malformed calls.
fn c4() -> Vec<String> {
    let mut f = vec![];
    let svc = Service::new(ServiceConfig::default());
    let tb = Arc::new(Toolbox::new(svc.clone(), "cases", ApprovalMode::ApproveWrites));
    let mock = Arc::new(MockProvider::empty());
    s_malformed(&mock);
    let mut agent = Agent::new(mock, tb.clone(), "eval-model");
    let mut results_ev: Vec<(String, bool, String)> = vec![];
    let mut approvals: Vec<String> = vec![];
    let mut at_create: Option<Value> = None;
    let turn = agent.run_turn(
        "do something odd",
        &mut |p| {
            approvals.push(p.name.clone());
            Decision::Approve
        },
        &mut |e| {
            if let AgentEvent::ToolResult { id, output, name, .. } = e {
                if name == "create_case" {
                    at_create = Some(state(&svc));
                }
                results_ev.push((id, output.is_error, output.text));
            }
        },
    );
    if let Err(e) = turn {
        return vec![format!("turn failed: {e}")];
    }
    let bad: Vec<_> = results_ev.iter().filter(|r| r.0 != "m1").collect();
    if bad.len() != 5 || bad.iter().any(|r| !r.1) {
        f.push(format!("expected 5 error results, got {bad:?}"));
    }
    let want = [
        ("m2", "unknown tool"),
        ("m3", "not a valid JSON object"),
        ("m4", "invalid arguments"),
        ("m5", "invalid arguments"),
        ("m6", "invalid arguments"),
    ];
    for (id, needle) in want {
        match results_ev.iter().find(|r| r.0 == id) {
            Some(r) if r.1 && r.2.contains(needle) => {}
            other => f.push(format!("{id}: {other:?} lacks '{needle}'")),
        }
    }
    if approvals != ["create_case"] {
        f.push(format!("approvals {approvals:?}, want only create_case"));
    }
    // Sixth: a valid write call while the gate is closed.
    let after = state(&svc);
    let o = tb.execute("set_setting", &json!({"path": "mesh.nx", "value": 5}));
    if !(o.is_error && o.structured["error"]["code"] == PERMISSION_DENIED) {
        f.push(format!("gate-closed write was not refused: {}", o.text));
    }
    if at_create.as_ref() != Some(&after) || state(&svc) != after {
        f.push("state changed after malformed or ungranted calls".into());
    }
    f
}

const CANARY_ENV: &str = "sk-eval-canary-env-7f3a91c2d4e85b60";
const CANARY_STORE: &str = "ak-eval-canary-store-19c0be77a2d4";

/// C5: secrets. `journals` are the journal texts written for the scenarios.
fn c5(journals: &[String]) -> Vec<String> {
    let mut f = vec![];
    let var = "RCFD_EVAL_F094_CANARY";
    std::env::set_var(var, CANARY_ENV);
    let store = MemoryKeyStore::new();
    store.set("claude-like", &Secret::new(CANARY_STORE)).unwrap();
    let mk = |name: &str, kind, env: Option<&str>| ProviderConfig {
        name: name.into(),
        kind,
        base_url: if kind == ProviderKind::Anthropic {
            String::new()
        } else {
            "http://localhost:9/v1".into()
        },
        model: "m".into(),
        key_env: env.map(String::from),
        max_tokens: 64,
        timeout_secs: 5,
        local: false,
    };
    let cfgs = [
        (
            mk("openai-like", ProviderKind::OpenAiCompatible, Some(var)),
            CANARY_ENV,
            "authorization",
        ),
        (
            mk("claude-like", ProviderKind::Anthropic, None),
            CANARY_STORE,
            "x-api-key",
        ),
    ];
    let mut corpus: Vec<(String, String)> = vec![];
    for (cfg, canary, header) in &cfgs {
        let t = Arc::new(FakeTransport::new());
        t.push(
            401,
            format!(r#"{{"error":{{"message":"Incorrect API key provided: {canary}. Check your key."}}}}"#),
        );
        let provider = build_provider(cfg, &store, Some(t.clone() as Arc<dyn Transport>)).unwrap();
        let svc = Service::new(ServiceConfig::default());
        let tb = Arc::new(Toolbox::new(svc, "cases", ApprovalMode::ApproveWrites));
        let mut agent = Agent::new(Arc::from(provider), tb, "m");
        agent.submit("hello").unwrap();
        match agent.step(&mut |_| {}) {
            Err(e) => {
                corpus.push((format!("{} AgentError Display", cfg.name), e.to_string()));
                corpus.push((format!("{} AgentError Debug", cfg.name), format!("{e:?}")));
            }
            Ok(s) => f.push(format!("{}: expected a provider error, got {s:?}", cfg.name)),
        }
        corpus.push((format!("{} transcript", cfg.name), agent.transcript().to_string()));
        // Positive control: the key really was sent (header) and really was echoed by the server.
        let req = t.last_request().unwrap();
        let hv = req.header(header).unwrap_or("");
        if !hv.contains(canary) {
            f.push(format!("{}: control failed, key not in the {header} header", cfg.name));
        }
        if req.url.contains(canary) || req.body.as_deref().unwrap_or("").contains(canary) {
            f.push(format!("{}: key present in request URL or body", cfg.name));
        }
        corpus.push((format!("{} config Debug", cfg.name), format!("{cfg:?}")));
        corpus.push((
            format!("{} AssistantConfig Debug", cfg.name),
            format!(
                "{:?}",
                AssistantConfig {
                    providers: vec![cfg.clone()],
                    active: Some(cfg.name.clone())
                }
            ),
        ));
        corpus.push((
            format!("{} Secret Debug", cfg.name),
            format!("{:?}", Secret::new(*canary)),
        ));
        // A successful streamed turn (OpenAI-compatible only) must not carry the key either.
        if cfg.kind == ProviderKind::OpenAiCompatible {
            t.push(
                200,
                "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n",
            );
            match agent.step(&mut |_| {}) {
                Ok(Step::Done(_)) => {}
                other => f.push(format!("{}: streamed retry did not finish: {other:?}", cfg.name)),
            }
            corpus.push((
                format!("{} transcript after retry", cfg.name),
                agent.transcript().to_string(),
            ));
            for (i, r) in t.requests.lock().unwrap().iter().enumerate() {
                if r.url.contains(canary) || r.body.as_deref().unwrap_or("").contains(canary) {
                    f.push(format!("{}: key in request {i} URL/body", cfg.name));
                }
            }
        }
    }
    for (i, j) in journals.iter().enumerate() {
        corpus.push((format!("journal {i}"), j.clone()));
    }
    for (what, text) in &corpus {
        for c in [CANARY_ENV, CANARY_STORE] {
            if text.contains(c) {
                f.push(format!("canary leaked into {what}"));
            }
        }
    }
    std::env::remove_var(var);
    f
}

/// Journal for one run: transcript plus event log.
fn journal(sc: &Scenario, policy: Policy, o: &Outcome) -> Value {
    json!({"format": "rcfd-assistant-eval-journal", "version": 1, "scenario": sc.name,
           "policy": format!("{policy:?}"), "prompt": sc.prompt,
           "transcript": o.transcript, "events": o.events.iter().map(Ev::json).collect::<Vec<_>>()})
}

/// C6: parse a journal back; every tool_use has exactly one tool_result.
fn c6_check(text: &str) -> Vec<String> {
    let mut f = vec![];
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return vec![format!("journal does not parse: {e}")],
    };
    let mut uses: Vec<String> = vec![];
    let mut res: Vec<String> = vec![];
    for m in v["transcript"]["messages"].as_array().into_iter().flatten() {
        for b in m["content"].as_array().into_iter().flatten() {
            match b["type"].as_str() {
                Some("tool_use") => uses.push(b["id"].as_str().unwrap_or("").into()),
                Some("tool_result") => res.push(b["id"].as_str().unwrap_or("").into()),
                _ => {}
            }
        }
    }
    uses.sort();
    res.sort();
    if uses.is_empty() {
        f.push("journal has no tool calls".into());
    }
    if uses != res {
        f.push(format!("tool_use ids {uses:?} != tool_result ids {res:?}"));
    }
    if v["events"].as_array().is_none_or(|e| e.is_empty()) {
        f.push("journal has no events".into());
    }
    f
}

#[test]
fn eval_suite_meets_the_registered_criteria() {
    let t0 = Instant::now();
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("f094-journals");
    std::fs::create_dir_all(&dir).unwrap();
    let mut miss: Vec<(String, String)> = vec![];
    let mut journals: Vec<String> = vec![];
    let mut report: Vec<String> = vec![];

    for sc in scenarios() {
        // C1, C2, C6, C7 on the approve-all run.
        let a = run_scenario(&sc, Policy::ApproveAll);
        let b = run_scenario(&sc, Policy::ApproveAll);
        for m in c1(&sc, &a) {
            miss.push(("C1".into(), format!("{}: {m}", sc.name)));
        }
        for m in c2(&a) {
            miss.push(("C2".into(), format!("{}: {m}", sc.name)));
        }
        if a.calls != b.calls || a.final_state != b.final_state {
            miss.push(("C7".into(), format!("{}: two runs differ", sc.name)));
        }
        let approvals = a.events.iter().filter(|e| matches!(e, Ev::Approval { .. })).count();
        let reads = a
            .events
            .iter()
            .filter(|e| matches!(e, Ev::Result { name, .. } if tool_access(&a, name) == Some(Access::Read)))
            .count();
        report.push(format!(
            "{:<24} calls={:<3} approvals={:<2} read-calls={:<2}",
            sc.name,
            a.calls.len(),
            approvals,
            reads
        ));
        for (policy, o) in [(Policy::ApproveAll, &a)] {
            let text = journal(&sc, policy, o).to_string();
            let path = dir.join(format!("{}.{:?}.journal.json", sc.name, policy));
            std::fs::write(&path, &text).unwrap();
            let back = std::fs::read_to_string(&path).unwrap();
            for m in c6_check(&back) {
                miss.push(("C6".into(), format!("{}: {m}", sc.name)));
            }
            journals.push(back);
        }
        // C3 on both denial variants (journals are written for these as well).
        for policy in [Policy::DenyAll, Policy::ApproveCreateOnly] {
            let d = run_scenario(&sc, policy);
            for m in c3(&d, policy) {
                miss.push(("C3".into(), format!("{} {policy:?}: {m}", sc.name)));
            }
            for m in c2(&d) {
                miss.push(("C2".into(), format!("{} {policy:?}: {m}", sc.name)));
            }
            let text = journal(&sc, policy, &d).to_string();
            std::fs::write(dir.join(format!("{}.{:?}.journal.json", sc.name, policy)), &text).unwrap();
            if !d.transcript["messages"].as_array().is_some_and(|m| !m.is_empty()) {
                miss.push(("C6".into(), format!("{} {policy:?}: empty transcript", sc.name)));
            }
            journals.push(text);
        }
    }
    for m in c4() {
        miss.push(("C4".into(), m));
    }
    for m in c5(&journals) {
        miss.push(("C5".into(), m));
    }
    let elapsed = t0.elapsed().as_secs_f64();
    if elapsed >= 60.0 {
        miss.push(("C8".into(), format!("suite took {elapsed:.1} s")));
    }

    println!("---- F-094 eval report ----");
    for r in &report {
        println!("{r}");
    }
    println!("journals written to {}", dir.display());
    println!("suite wall time {elapsed:.2} s");
    for c in ["C1", "C2", "C3", "C4", "C5", "C6", "C7", "C8"] {
        let n = miss.iter().filter(|m| m.0 == c).count();
        println!(
            "{c}: {}",
            if n == 0 {
                "PASS".to_string()
            } else {
                format!("{n} MISS(ES)")
            }
        );
    }
    for (c, m) in &miss {
        println!("  miss {c}: {m}");
    }
    assert!(miss.is_empty(), "criteria missed: {miss:#?}");
}
