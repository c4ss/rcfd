//! MCP server exposing the rcfd solver service as tools (F-090) with a
//! permission model (F-091).
//!
//! Transport: newline-delimited JSON-RPC 2.0 on stdio (`rcfd-mcp` binary) or
//! any reader/writer pair. Only `std` threads and `serde_json` are used.
//!
//! Error semantics: malformed requests, unknown tools and arguments that do not
//! match a tool's `inputSchema` are JSON-RPC errors (-32601 / -32602). Failures
//! while executing a valid call (validation problems, wrong run state, limits,
//! denied permission) are tool results with `isError: true` whose text is a
//! JSON object `{"error": {"code", "message", "data"?}}`.

pub mod policy;

use policy::{Access, Policy, ToolCall};
use rcfd_api::service::{RpcError, Service};
use serde_json::{json, Map, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Protocol revision offered when the client does not state one.
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-06-18";

/// Built-in starting points for `create_case`.
pub const TEMPLATES: &[(&str, &str)] = &[("lid_driven_cavity", LID_DRIVEN_CAVITY)];

/// Lid-driven cavity at Re = 100 (unit lid speed, unit side, nu = 0.01).
pub const LID_DRIVEN_CAVITY: &str = r#"title = "lid-driven cavity Re=100"
[mesh]
generator = "cavity_2d"
nx = 16
ny = 16
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 600
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
"#;

struct Tool {
    name: &'static str,
    description: &'static str,
    access: Access,
    schema: fn() -> Value,
}

fn sess() -> Value {
    json!({"type": "string", "description": "Session id; defaults to the session made by the last create_case"})
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
}

fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "list_cases",
            description: "List the benchmark and validation cases shipped with rcfd and the built-in case templates.",
            access: Access::Read,
            schema: || obj(json!({"dir": {"type": "string", "description": "cases directory (default: cases)"}}), &[]),
        },
        Tool {
            name: "create_case",
            description: "Create a simulation session from a template, TOML text, a settings object or a case file. Returns the session id and makes it the default session.",
            access: Access::Mutate,
            schema: || {
                obj(
                    json!({
                        "template": {"type": "string", "description": "built-in template name (see list_cases)"},
                        "toml": {"type": "string", "description": "case file text"},
                        "settings": {"type": "object", "description": "case settings as JSON"},
                        "path": {"type": "string", "description": "path of a .toml/.json case file inside the sandbox"}
                    }),
                    &[],
                )
            },
        },
        Tool {
            name: "set_setting",
            description: "Set one setting by dotted path, e.g. solver.alpha_u or boundaries.lid.velocity_value. Named entries (boundaries.<patch>, reports.<name>) are created on demand. Refused while a run is active.",
            access: Access::Mutate,
            schema: || obj(json!({"session": sess(), "path": {"type": "string"}, "value": {}}), &["path", "value"]),
        },
        Tool {
            name: "get_setting",
            description: "Read one setting by dotted path, or the whole settings tree when path is omitted.",
            access: Access::Read,
            schema: || obj(json!({"session": sess(), "path": {"type": "string"}}), &[]),
        },
        Tool {
            name: "load_mesh",
            description: "Use a mesh file (Gmsh .msh or an OpenFOAM polyMesh directory) instead of a generator.",
            access: Access::Mutate,
            schema: || {
                obj(
                    json!({"session": sess(), "path": {"type": "string"}, "scale": {"type": "number", "minimum": 0.0, "description": "coordinate scale factor"}}),
                    &["path"],
                )
            },
        },
        Tool {
            name: "check_mesh",
            description: "Build the mesh and return its quality report (non-orthogonality, skewness, aspect ratio, errors, warnings) and patch list.",
            access: Access::Read,
            schema: || obj(json!({"session": sess()}), &[]),
        },
        Tool {
            name: "run",
            description: "Start the solver and wait up to wait_seconds for it to finish. Returns the run state; a run that is still going keeps running in the background (poll with status).",
            access: Access::Execute,
            schema: || {
                obj(
                    json!({
                        "session": sess(),
                        "max_steps": {"type": "integer", "minimum": 1, "description": "stop after this many iterations/steps"},
                        "wait_seconds": {"type": "number", "minimum": 0.0, "description": "how long to block (default 30)"}
                    }),
                    &[],
                )
            },
        },
        Tool {
            name: "status",
            description: "Run state, iteration, time, latest residuals and report values.",
            access: Access::Read,
            schema: || obj(json!({"session": sess()}), &[]),
        },
        Tool {
            name: "get_residuals",
            description: "Monitor histories (res_u, res_p, continuity and report values) as x/y arrays; use since to fetch only newer samples.",
            access: Access::Read,
            schema: || {
                obj(
                    json!({"session": sess(), "names": {"type": "array", "items": {"type": "string"}}, "since": {"type": "number"}}),
                    &[],
                )
            },
        },
        Tool {
            name: "get_field_stats",
            description: "Min, max, mean and volume average of a field (ux, uy, uz, p, speed, ...).",
            access: Access::Read,
            schema: || obj(json!({"session": sess(), "name": {"type": "string"}}), &["name"]),
        },
        Tool {
            name: "export_vtk",
            description: "Write the current solution as a VTK .vtu file inside the sandbox and return its path.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({"session": sess(), "dir": {"type": "string"}, "name": {"type": "string"}}),
                    &[],
                )
            },
        },
        Tool {
            name: "render_image",
            description: "Render a field as a PNG image (optionally with velocity vectors) and return it as an image content block.",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "session": sess(),
                        "field": {"type": "string", "description": "field name, default speed"},
                        "width": {"type": "integer", "minimum": 64, "maximum": 4096},
                        "vectors": {"type": "boolean"}
                    }),
                    &[],
                )
            },
        },
    ]
}

/// Check `value` against the small JSON Schema subset used by the tools
/// (`type`, `properties`, `required`, `additionalProperties`, `items`, `enum`,
/// `minimum`, `maximum`).
pub fn validate_args(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    if let Some(t) = schema.get("type").and_then(Value::as_str) {
        let ok = match t {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "integer" => value.is_i64() || value.is_u64(),
            "number" => value.is_number(),
            _ => true,
        };
        if !ok {
            return Err(format!("'{path}' must be of type {t}"));
        }
    }
    if let Some(e) = schema.get("enum").and_then(Value::as_array) {
        if !e.contains(value) {
            return Err(format!("'{path}' must be one of {e:?}"));
        }
    }
    if let Some(x) = value.as_f64() {
        if schema.get("minimum").and_then(Value::as_f64).is_some_and(|m| x < m) {
            return Err(format!("'{path}' is below the minimum {}", schema["minimum"]));
        }
        if schema.get("maximum").and_then(Value::as_f64).is_some_and(|m| x > m) {
            return Err(format!("'{path}' is above the maximum {}", schema["maximum"]));
        }
    }
    if let (Some(props), Some(map)) = (schema.get("properties").and_then(Value::as_object), value.as_object()) {
        for r in schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !map.contains_key(r) {
                return Err(format!("missing required argument '{r}'"));
            }
        }
        for (k, v) in map {
            match props.get(k) {
                Some(s) => validate_args(s, v, k)?,
                None if schema.get("additionalProperties") == Some(&json!(false)) => {
                    return Err(format!("unknown argument '{k}'"));
                }
                None => {}
            }
        }
    }
    if let (Some(items), Some(arr)) = (schema.get("items"), value.as_array()) {
        for (i, v) in arr.iter().enumerate() {
            validate_args(items, v, &format!("{path}[{i}]"))?;
        }
    }
    Ok(())
}

/// MCP server state.
pub struct McpServer {
    svc: Arc<Service>,
    policy: Policy,
    current: Mutex<Option<String>>,
    cases_dir: PathBuf,
}

fn error_result(e: &RpcError) -> Value {
    let mut err = json!({"code": e.code, "message": e.message});
    if let Some(d) = &e.data {
        err["data"] = d.clone();
    }
    let body = json!({"error": err});
    json!({"content": [{"type": "text", "text": body.to_string()}], "structuredContent": body, "isError": true})
}

fn ok_result(v: Value) -> Value {
    json!({"content": [{"type": "text", "text": v.to_string()}], "structuredContent": v, "isError": false})
}

impl McpServer {
    /// Server over `svc` with the given permission policy.
    pub fn new(svc: Arc<Service>, policy: Policy) -> Self {
        McpServer {
            svc,
            policy,
            current: Mutex::new(None),
            cases_dir: PathBuf::from("cases"),
        }
    }

    /// Directory searched by `list_cases` by default.
    pub fn with_cases_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cases_dir = dir.into();
        self
    }

    /// The solver service this server drives.
    pub fn service(&self) -> &Arc<Service> {
        &self.svc
    }

    /// Session used by tools that omit `session`, if any.
    pub fn current_session(&self) -> Option<String> {
        self.current.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Make `id` the default session (used by hosts that create sessions themselves).
    pub fn set_current_session(&self, id: Option<String>) {
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = id;
    }

    /// `tools/list` payload.
    pub fn tool_list(&self) -> Value {
        let list: Vec<Value> = tools()
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": (t.schema)(),
                    "annotations": {
                        "readOnlyHint": t.access == Access::Read,
                        "destructiveHint": false,
                        "idempotentHint": matches!(t.access, Access::Read),
                        "openWorldHint": false,
                        "title": t.name,
                    },
                    "_meta": {"rcfd/access": t.access.name(), "rcfd/needsApproval": t.access != Access::Read}
                })
            })
            .collect();
        json!({"tools": list})
    }

    fn session_id(&self, args: &Value) -> Result<String, RpcError> {
        if let Some(s) = args.get("session").and_then(Value::as_str) {
            return Ok(s.to_string());
        }
        self.current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| RpcError {
                code: 1001,
                message: "no session yet; call create_case first".into(),
                data: None,
            })
    }

    /// Execute a tool; the `Err` side is a JSON-RPC level error.
    pub fn call_tool(&self, name: &str, args: &Value) -> Result<Value, RpcError> {
        let all = tools();
        let Some(tool) = all.iter().find(|t| t.name == name) else {
            return Err(RpcError {
                code: -32602,
                message: format!("unknown tool '{name}'"),
                data: None,
            });
        };
        let empty = json!({});
        let args = if args.is_null() { &empty } else { args };
        validate_args(&(tool.schema)(), args, "arguments").map_err(|m| RpcError {
            code: -32602,
            message: format!("invalid arguments for {name}: {m}"),
            data: None,
        })?;
        let call = ToolCall {
            tool: tool.name,
            access: tool.access,
            args,
        };
        if let Err(denied) = self.policy.authorize(&call) {
            return Ok(error_result(&denied));
        }
        Ok(match self.dispatch(tool.name, args) {
            Ok(v) => v,
            Err(e) => error_result(&e),
        })
    }

    fn dispatch(&self, name: &str, args: &Value) -> Result<Value, RpcError> {
        let svc = &self.svc;
        let with = |extra: Value| -> Result<Value, RpcError> {
            let mut m = extra.as_object().cloned().unwrap_or_default();
            m.insert("session".into(), json!(self.session_id(args)?));
            Ok(Value::Object(m))
        };
        match name {
            "list_cases" => {
                let dir = args
                    .get("dir")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| self.cases_dir.clone());
                let dir = svc.sandbox_path(&dir.display().to_string())?;
                let cases = rcfd_io::cases::Registry::load(&dir)
                    .map(|r| {
                        r.list()
                            .iter()
                            .map(|c| json!({"id": c.id, "title": c.title, "roadmap": c.roadmap}))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let templates: Vec<&str> = TEMPLATES.iter().map(|t| t.0).collect();
                Ok(ok_result(json!({"cases": cases, "templates": templates})))
            }
            "create_case" => {
                let mut p = Map::new();
                if let Some(t) = args.get("template").and_then(Value::as_str) {
                    let Some((_, text)) = TEMPLATES.iter().find(|x| x.0 == t) else {
                        return Err(RpcError {
                            code: -32602,
                            message: format!("unknown template '{t}'"),
                            data: Some(json!({"templates": TEMPLATES.iter().map(|x| x.0).collect::<Vec<_>>()})),
                        });
                    };
                    p.insert("toml".into(), json!(text));
                }
                for k in ["toml", "settings", "path"] {
                    if let Some(v) = args.get(k) {
                        p.insert(k.into(), v.clone());
                    }
                }
                let r = svc.call("session.create", &Value::Object(p))?;
                let id = r["session"].as_str().unwrap_or_default().to_string();
                *self.current.lock().unwrap_or_else(|e| e.into_inner()) = Some(id.clone());
                Ok(ok_result(json!({"session": id})))
            }
            "set_setting" => {
                let p = with(json!({"path": args["path"], "value": args["value"]}))?;
                Ok(ok_result(svc.call("settings.set", &p)?))
            }
            "get_setting" => {
                let p = with(json!({"path": args.get("path").cloned().unwrap_or(Value::Null)}))?;
                Ok(ok_result(svc.call("settings.get", &p)?))
            }
            "load_mesh" => {
                let path = args["path"].as_str().unwrap_or_default();
                svc.sandbox_path(path)?;
                for (k, v) in [("mesh.source", json!("file")), ("mesh.file", json!(path))]
                    .into_iter()
                    .chain(args.get("scale").map(|s| ("mesh.scale", s.clone())))
                {
                    svc.call("settings.set", &with(json!({"path": k, "value": v}))?)?;
                }
                let chk = svc.call("mesh.check", &with(json!({}))?)?;
                Ok(ok_result(chk))
            }
            "check_mesh" => Ok(ok_result(svc.call("mesh.check", &with(json!({}))?)?)),
            "run" => {
                let mut p = json!({});
                if let Some(m) = args.get("max_steps") {
                    p["max_steps"] = m.clone();
                }
                let p = with(p)?;
                svc.call("run.start", &p)?;
                let wait = args.get("wait_seconds").and_then(Value::as_f64).unwrap_or(30.0);
                let st = svc.call("run.wait", &with(json!({"timeout_ms": (wait * 1000.0) as u64}))?)?;
                Ok(ok_result(st))
            }
            "status" => {
                let mut st = svc.call("run.status", &with(json!({}))?)?;
                if let Ok(r) = svc.call("reports.get", &with(json!({}))?) {
                    st["reports"] = r["reports"].clone();
                }
                Ok(ok_result(st))
            }
            "get_residuals" => {
                let mut p = Map::new();
                for k in ["names", "since"] {
                    if let Some(v) = args.get(k) {
                        p.insert(k.into(), v.clone());
                    }
                }
                Ok(ok_result(svc.call("monitors.get", &with(Value::Object(p))?)?))
            }
            "get_field_stats" => Ok(ok_result(
                svc.call("field.stats", &with(json!({"name": args["name"]}))?)?,
            )),
            "export_vtk" => {
                let mut p = Map::new();
                for k in ["dir", "name"] {
                    if let Some(v) = args.get(k) {
                        p.insert(k.into(), v.clone());
                    }
                }
                Ok(ok_result(svc.call("field.export_vtk", &with(Value::Object(p))?)?))
            }
            "render_image" => {
                let mut p = Map::new();
                for k in ["field", "width", "vectors"] {
                    if let Some(v) = args.get(k) {
                        p.insert(k.into(), v.clone());
                    }
                }
                let r = svc.call("render.png", &with(Value::Object(p))?)?;
                let meta = json!({"field": r["field"], "min": r["min"], "max": r["max"], "width": r["width"], "height": r["height"]});
                Ok(json!({
                    "content": [
                        json!({"type": "image", "data": r["data_base64"], "mimeType": "image/png"}),
                        {"type": "text", "text": meta.to_string()}
                    ],
                    "structuredContent": meta,
                    "isError": false
                }))
            }
            other => Err(RpcError {
                code: -32601,
                message: format!("tool '{other}' has no handler"),
                data: None,
            }),
        }
    }

    /// Handle one JSON-RPC message; `None` for notifications.
    pub fn handle(&self, req: &Value) -> Option<Value> {
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str);
        let params = req.get("params").cloned().unwrap_or(Value::Null);
        let fail =
            |code: i64, msg: String| json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": msg}});
        let Some(method) = method else {
            return id.as_ref().map(|_| fail(-32600, "not a JSON-RPC request".into()));
        };
        id.as_ref()?;
        let result = match method {
            "initialize" => json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(DEFAULT_PROTOCOL_VERSION),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "rcfd", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "Clean-room open source CFD solver. create_case, adjust settings, check_mesh, run, then inspect residuals, fields and images."
            }),
            "ping" => json!({}),
            "tools/list" => self.tool_list(),
            "tools/call" => {
                let Some(name) = params.get("name").and_then(Value::as_str) else {
                    return Some(fail(-32602, "tools/call needs a tool name".into()));
                };
                match self.call_tool(name, params.get("arguments").unwrap_or(&Value::Null)) {
                    Ok(v) => v,
                    Err(e) => return Some(fail(e.code, e.message)),
                }
            }
            other => return Some(fail(-32601, format!("unknown method '{other}'"))),
        };
        Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }

    /// Serve newline-delimited JSON-RPC until the reader closes.
    pub fn serve(&self, reader: impl std::io::Read, mut writer: impl Write) -> std::io::Result<()> {
        for line in std::io::BufReader::new(reader).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let resp = match serde_json::from_str::<Value>(&line) {
                Ok(req) => self.handle(&req),
                Err(e) => Some(
                    json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}}),
                ),
            };
            if let Some(r) = resp {
                writeln!(writer, "{r}")?;
                writer.flush()?;
            }
        }
        self.svc.shutdown();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
