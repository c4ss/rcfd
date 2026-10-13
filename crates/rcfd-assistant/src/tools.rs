//! The assistant's tool layer: the MCP tool set executed in-process, plus the
//! setup-workflow tools, behind a server-side approval gate.
//!
//! Mutating tools (everything except `Access::Read`) are enforced by an
//! [`rcfd_mcp::policy::Policy`] hook that consults a shared [`Gate`]: a call
//! runs only if the mode allows writes *and* a one-shot grant for exactly that
//! tool and argument set was issued. The UI cannot bypass this by mistake.

use crate::workflow;
use rcfd_api::service::{RpcError, Service};
use rcfd_mcp::policy::{Access, Policy};
use rcfd_mcp::{validate_args, McpServer};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// How mutating tools are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalMode {
    /// Only read tools are offered and allowed.
    ReadOnly,
    /// Mutating tools run after the user approves each call.
    ApproveWrites,
}

/// Shared approval state read by the policy hook.
pub struct Gate {
    mode: Mutex<ApprovalMode>,
    grants: Mutex<Vec<String>>,
}

fn key(tool: &str, args: &Value) -> String {
    format!("{tool}:{args}")
}

impl Gate {
    fn new(mode: ApprovalMode) -> Self {
        Gate {
            mode: Mutex::new(mode),
            grants: Mutex::new(vec![]),
        }
    }

    fn allows(&self, tool: &str, args: &Value) -> bool {
        if *self.mode.lock().unwrap() == ApprovalMode::ReadOnly {
            return false;
        }
        let k = key(tool, args);
        let mut g = self.grants.lock().unwrap();
        match g.iter().position(|x| *x == k) {
            Some(i) => {
                g.remove(i);
                true
            }
            None => false,
        }
    }
}

/// A tool as offered to the model.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Argument schema.
    pub schema: Value,
    /// Access class.
    pub access: Access,
}

/// Result of running a tool.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutput {
    /// Text for the model (size-bounded).
    pub text: String,
    /// True when the call failed or was refused.
    pub is_error: bool,
    /// PNG (base64) produced by `render_image`, kept for the UI and not sent to the model.
    pub image_png_base64: Option<String>,
    /// Full structured result.
    pub structured: Value,
}

impl ToolOutput {
    fn error(code: i64, message: impl Into<String>) -> Self {
        let message = message.into();
        let body = json!({"error": {"code": code, "message": message}});
        ToolOutput {
            text: body.to_string(),
            is_error: true,
            image_png_base64: None,
            structured: body,
        }
    }
}

/// Largest tool result text sent to a model, in characters.
pub const MAX_RESULT_CHARS: usize = 12_000;
/// Longest monitor series sent to a model, in samples per series.
pub const MAX_SERIES_POINTS: usize = 120;

fn access_from(s: &str) -> Access {
    match s {
        "read" => Access::Read,
        "execute" => Access::Execute,
        "write_files" => Access::WriteFiles,
        _ => Access::Mutate,
    }
}

fn obj(props: Value) -> Value {
    json!({"type": "object", "properties": props, "required": [], "additionalProperties": false})
}

fn extras() -> Vec<ToolInfo> {
    let sess = json!({"session": {"type": "string", "description": "Session id; defaults to the current case"}});
    vec![
        ToolInfo {
            name: "assess_mesh".into(),
            description: "Build the mesh and judge its quality: non-orthogonality, skewness, aspect ratio, volume ratio, with a verdict and concrete advice per finding.".into(),
            schema: obj(sess.clone()),
            access: Access::Read,
        },
        ToolInfo {
            name: "suggest_boundary_conditions".into(),
            description: "Propose boundary conditions for mesh patches that have none, from patch names and kinds. Returns set_setting calls; it changes nothing.".into(),
            schema: obj(sess.clone()),
            access: Access::Read,
        },
        ToolInfo {
            name: "validate_case".into(),
            description: "Check the case: schema/semantic errors, boundary entries that match no patch, patches without conditions, and a Reynolds-number plausibility warning.".into(),
            schema: obj(sess.clone()),
            access: Access::Read,
        },
        ToolInfo {
            name: "explain_residuals".into(),
            description: "Diagnose the residual history of the current run (diverging, oscillating, stagnating, converging) and suggest setting changes. Does not apply them.".into(),
            schema: obj(sess),
            access: Access::Read,
        },
    ]
}

/// MCP tools plus workflow tools, executed in-process with approval enforcement.
pub struct Toolbox {
    mcp: McpServer,
    svc: Arc<Service>,
    gate: Arc<Gate>,
    infos: Vec<ToolInfo>,
}

impl Toolbox {
    /// Toolbox over `svc`; `cases_dir` is searched by `list_cases`.
    pub fn new(svc: Arc<Service>, cases_dir: impl Into<PathBuf>, mode: ApprovalMode) -> Self {
        let gate = Arc::new(Gate::new(mode));
        let g = gate.clone();
        let policy = Policy::with_hook(move |c| g.allows(c.tool, c.args));
        let mcp = McpServer::new(svc.clone(), policy).with_cases_dir(cases_dir);
        let mut infos: Vec<ToolInfo> = mcp.tool_list()["tools"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| ToolInfo {
                name: t["name"].as_str().unwrap_or_default().to_string(),
                description: t["description"].as_str().unwrap_or_default().to_string(),
                schema: t["inputSchema"].clone(),
                access: access_from(t["_meta"]["rcfd/access"].as_str().unwrap_or("mutate")),
            })
            .collect();
        infos.extend(extras());
        Toolbox { mcp, svc, gate, infos }
    }

    /// Current approval mode.
    pub fn mode(&self) -> ApprovalMode {
        *self.gate.mode.lock().unwrap()
    }

    /// Switch approval mode.
    pub fn set_mode(&self, mode: ApprovalMode) {
        *self.gate.mode.lock().unwrap() = mode;
        self.gate.grants.lock().unwrap().clear();
    }

    /// Tools that may be offered in the current mode.
    pub fn available(&self) -> Vec<ToolInfo> {
        let ro = self.mode() == ApprovalMode::ReadOnly;
        self.infos
            .iter()
            .filter(|t| !ro || t.access == Access::Read)
            .cloned()
            .collect()
    }

    /// Look up a tool (regardless of mode).
    pub fn info(&self, name: &str) -> Option<&ToolInfo> {
        self.infos.iter().find(|t| t.name == name)
    }

    /// Check arguments against the tool's schema.
    pub fn validate(&self, name: &str, args: &Value) -> Result<(), String> {
        let info = self.info(name).ok_or_else(|| format!("unknown tool '{name}'"))?;
        let empty = json!({});
        validate_args(&info.schema, if args.is_null() { &empty } else { args }, "arguments")
    }

    /// Session used when a tool omits `session`.
    pub fn session(&self) -> Option<String> {
        self.mcp.current_session()
    }

    /// Make `id` the default session.
    pub fn set_session(&self, id: Option<String>) {
        self.mcp.set_current_session(id);
    }

    /// The solver service.
    pub fn service(&self) -> &Arc<Service> {
        &self.svc
    }

    /// Issue a one-shot grant for exactly this call (the user approved it).
    pub fn grant(&self, name: &str, args: &Value) {
        let empty = json!({});
        let args = if args.is_null() { &empty } else { args };
        self.gate.grants.lock().unwrap().push(key(name, args));
    }

    /// Run a tool. Mutating tools are refused unless granted.
    pub fn execute(&self, name: &str, args: &Value) -> ToolOutput {
        if let Err(m) = self.validate(name, args) {
            return ToolOutput::error(-32602, format!("invalid arguments for {name}: {m}"));
        }
        let empty = json!({});
        let args = if args.is_null() { &empty } else { args };
        if extras().iter().any(|e| e.name == name) {
            return self.run_extra(name, args);
        }
        match self.mcp.call_tool(name, args) {
            Ok(v) => self.convert(name, &v),
            Err(e) => ToolOutput::error(e.code, e.message),
        }
    }

    fn convert(&self, name: &str, v: &Value) -> ToolOutput {
        let is_error = v["isError"].as_bool().unwrap_or(false);
        let structured = v["structuredContent"].clone();
        let mut image = None;
        let mut text = String::new();
        for c in v["content"].as_array().into_iter().flatten() {
            match c["type"].as_str() {
                Some("image") => image = c["data"].as_str().map(String::from),
                Some("text") => text = c["text"].as_str().unwrap_or_default().to_string(),
                _ => {}
            }
        }
        if image.is_some() {
            text =
                format!("Rendered an image (shown to the user in the sidebar, not visible to you). Metadata: {text}");
        } else if name == "get_residuals" && !is_error {
            text = thin_series(&structured).to_string();
        }
        ToolOutput {
            text: truncate(&text),
            is_error,
            image_png_base64: image,
            structured,
        }
    }

    fn sid(&self, args: &Value) -> Result<String, RpcError> {
        args.get("session")
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| self.session())
            .ok_or(RpcError {
                code: 1001,
                message: "no case yet; call create_case first".into(),
                data: None,
            })
    }

    fn run_extra(&self, name: &str, args: &Value) -> ToolOutput {
        let go = || -> Result<Value, RpcError> {
            let sid = self.sid(args)?;
            let p = json!({"session": sid});
            match name {
                "assess_mesh" => Ok(workflow::assess_mesh(&self.svc.call("mesh.check", &p)?)),
                "suggest_boundary_conditions" => {
                    let mc = self.svc.call("mesh.check", &p)?;
                    let st = self.svc.call("settings.get", &p)?;
                    Ok(workflow::suggest_boundary_conditions_for(
                        &mc["patches"],
                        &st["value"]["boundaries"],
                        &st["value"],
                    ))
                }
                "validate_case" => {
                    let v = self.svc.call("settings.validate", &p)?;
                    let mc = self.svc.call("mesh.check", &p).ok();
                    let st = self.svc.call("settings.get", &p)?;
                    Ok(workflow::validate_case(&v, mc.as_ref(), &st["value"]))
                }
                _ => {
                    // No simulation yet (code 1002) means an empty history, not a failure.
                    let m = self
                        .svc
                        .call("monitors.get", &p)
                        .unwrap_or_else(|_| json!({"series": {}}));
                    let st = self.svc.call("settings.get", &p)?;
                    let series: Vec<(String, Vec<f64>)> = m["series"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(k, s)| {
                            let y = s["y"]
                                .as_array()
                                .map(|a| a.iter().map(|v| v.as_f64().unwrap_or(f64::NAN)).collect())
                                .unwrap_or_default();
                            (k.clone(), y)
                        })
                        .collect();
                    Ok(workflow::explain_residuals(&series, &st["value"]))
                }
            }
        };
        match go() {
            Ok(v) => ToolOutput {
                text: truncate(&v.to_string()),
                is_error: false,
                image_png_base64: None,
                structured: v,
            },
            Err(e) => ToolOutput::error(e.code, e.message),
        }
    }
}

fn truncate(s: &str) -> String {
    if s.chars().count() <= MAX_RESULT_CHARS {
        return s.to_string();
    }
    let t: String = s.chars().take(MAX_RESULT_CHARS).collect();
    format!("{t}... [truncated: result exceeded {MAX_RESULT_CHARS} characters]")
}

/// Keep at most [`MAX_SERIES_POINTS`] samples per series (evenly spaced, always the last).
pub fn thin_series(m: &Value) -> Value {
    let mut out = m.clone();
    if let Some(series) = out["series"].as_object_mut() {
        for s in series.values_mut() {
            let (Some(x), Some(y)) = (s["x"].as_array().cloned(), s["y"].as_array().cloned()) else {
                continue;
            };
            let n = x.len().min(y.len());
            if n <= MAX_SERIES_POINTS {
                continue;
            }
            let stride = n.div_ceil(MAX_SERIES_POINTS);
            let mut idx: Vec<usize> = (0..n).step_by(stride).collect();
            if idx.last() != Some(&(n - 1)) {
                idx.push(n - 1);
            }
            s["x"] = Value::Array(idx.iter().map(|&i| x[i].clone()).collect());
            s["y"] = Value::Array(idx.iter().map(|&i| y[i].clone()).collect());
            s["thinned_from"] = json!(n);
        }
    }
    out
}
