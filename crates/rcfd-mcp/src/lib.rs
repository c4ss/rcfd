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
            name: "list_workflow_tasks",
            description: "List the tasks of the session's set-up/run workflow with their state (not_run, done, failed, stale), dependencies and the next task to run; also lists the available templates and task kinds.",
            access: Access::Read,
            schema: || obj(json!({"session": sess()}), &[]),
        },
        Tool {
            name: "create_workflow",
            description: "Create the session's workflow from a template (mesh_to_results, channel_flow, empty), from a workflow document, or by loading a workflow file from the sandbox.",
            access: Access::Mutate,
            schema: || {
                obj(
                    json!({
                        "session": sess(),
                        "template": {"type": "string", "description": "mesh_to_results (default), channel_flow or empty"},
                        "workflow": {"type": "object", "description": "an rcfd-workflow document"},
                        "path": {"type": "string", "description": "workflow JSON file inside the sandbox"},
                        "name": {"type": "string"}
                    }),
                    &[],
                )
            },
        },
        Tool {
            name: "edit_workflow",
            description: "Insert a task into, or delete a task from, the session's workflow. Deleting is refused while other tasks depend on the task; inserting validates ids, kinds, dependencies and cycles.",
            access: Access::Mutate,
            schema: || {
                obj(
                    json!({
                        "session": sess(),
                        "action": {"type": "string", "enum": ["insert", "delete"]},
                        "task": {"type": "object", "description": "for insert: {id, kind, title?, deps?, params?}"},
                        "after": {"type": "string", "description": "for insert: id of the task to insert after (default: at the end)"},
                        "id": {"type": "string", "description": "for delete: task id"}
                    }),
                    &["action"],
                )
            },
        },
        Tool {
            name: "run_workflow_task",
            description: "Run one workflow task (and, by default, the prerequisite tasks that are not done), or all outstanding tasks when task is omitted. Stops at the first failing task and reports its message. Solve tasks run synchronously under the service limits.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "session": sess(),
                        "task": {"type": "string"},
                        "with_prerequisites": {"type": "boolean"},
                        "max_steps": {"type": "integer", "minimum": 1}
                    }),
                    &[],
                )
            },
        },
        Tool {
            name: "check_case_headless",
            description: "Check a case without running it (the --check-only launch): settings, simulation set-up and mesh quality. Give a session, or a case file path inside the sandbox. Returns the exit code the CLI would use (0 ok, 3 invalid, 4 bad mesh).",
            access: Access::Read,
            schema: || obj(json!({"session": sess(), "path": {"type": "string"}}), &[]),
        },
        Tool {
            name: "run_headless",
            description: "Run the session under the run-control contract: step and wall-clock limits, stop file, continue from a checkpoint, periodic/final checkpoints and a machine-readable status stream written to a file. Blocks until the run ends; returns the outcome (state, exit code, iterations) and the last status events.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "session": sess(),
                        "max_steps": {"type": "integer", "minimum": 1},
                        "max_elapsed_s": {"type": "number", "minimum": 0.0},
                        "stop_file": {"type": "string"},
                        "continue_from": {"type": "string", "description": "checkpoint file (steady SIMPLE/SIMPLEC without turbulence)"},
                        "interpolate_case": {"type": "string", "description": "case file of the source mesh; with interpolate_checkpoint, initialise from that solution (F-115)"},
                        "interpolate_checkpoint": {"type": "string", "description": "checkpoint written for the source case"},
                        "interpolate_mode": {"type": "string", "enum": ["nonconservative", "conservative"]},
                        "checkpoint": {"type": "string"},
                        "checkpoint_every": {"type": "integer", "minimum": 1},
                        "status_path": {"type": "string", "description": "write the JSON-lines status stream here"},
                        "status_every": {"type": "integer", "minimum": 1},
                        "out_dir": {"type": "string", "description": "write solution.vtu, monitors and report.html here at the end"}
                    }),
                    &[],
                )
            },
        },
        Tool {
            name: "read_status_stream",
            description: "Summarise a status stream file (JSON lines, format rcfd-status) written by `rcfd run --status-json/--status-file` or run_headless: final state, exit code, iterations, last residuals, checkpoints and warnings.",
            access: Access::Read,
            schema: || obj(json!({"path": {"type": "string"}}), &["path"]),
        },
        Tool {
            name: "design_create_study",
            description: "Define a design study in a directory inside the sandbox (writes study.json): named parameters with bounds mapped to settings paths, operating conditions, objectives with robust mode (mean, worst, mean_plus_k_sigma, weighted), constraints and a sampler or optimiser (factorial, lhs, sobol, nelder_mead, differential_evolution). The base case is a case file or the settings of a session.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "dir": {"type": "string", "description": "study directory (study.json and evaluations.jsonl live here)"},
                        "base": {"type": "string", "description": "base case file inside the sandbox"},
                        "session": sess(),
                        "name": {"type": "string"},
                        "parameters": {"type": "array", "items": {"type": "object"}, "description": "{name, lo, hi, init?, path | targets:[{path, scale?, offset?}]}"},
                        "conditions": {"type": "array", "items": {"type": "object"}, "description": "{label, set:{settings path: value}}; default one condition 'base'"},
                        "objectives": {"type": "array", "items": {"type": "object"}, "description": "{metric, sense: min|max, weight?, robust?, k?, weights?}"},
                        "constraints": {"type": "array", "items": {"type": "object"}, "description": "{metric, cmp: <=|>=, limit, scale?}"},
                        "method": {"type": "object", "description": "{type, budget?, restarts?, seed?, pop?, levels?, n?}"},
                        "workers": {"type": "integer", "minimum": 1},
                        "chain": {"type": "boolean", "description": "start each condition from the previous converged one (default true)"},
                        "max_steps": {"type": "integer", "minimum": 1},
                        "overwrite": {"type": "boolean"}
                    }),
                    &["dir", "parameters", "objectives", "method"],
                )
            },
        },
        Tool {
            name: "design_create_variant",
            description: "Write a variant case file: a base case file, or a study with parameter values, plus settings overrides, validated through the settings tree. Returns the file path and the settings fingerprint. Refuses to replace an existing file unless overwrite is true.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "path": {"type": "string", "description": "variant case file to write (.toml or .json)"},
                        "base": {"type": "string"},
                        "dir": {"type": "string", "description": "study directory (instead of base)"},
                        "params": {"type": "object", "description": "parameter values of the study"},
                        "condition": {"type": "string"},
                        "overrides": {"type": "object", "description": "{settings path: value}"},
                        "name": {"type": "string"},
                        "overwrite": {"type": "boolean"}
                    }),
                    &["path"],
                )
            },
        },
        Tool {
            name: "design_run_operating_map",
            description: "Run an operating map: points over settings values (grid of axes, or an explicit list), each restarting from the previous converged solution; writes map.csv and map.json to out_dir and returns per-point status, restart kind, iterations and all report values. A repeated identical call returns the stored table without running. Bounded by max_design_evaluations.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "out_dir": {"type": "string"},
                        "base": {"type": "string", "description": "case file inside the sandbox"},
                        "dir": {"type": "string", "description": "study directory (instead of base) with params for a variant"},
                        "params": {"type": "object"},
                        "condition": {"type": "string"},
                        "overrides": {"type": "object"},
                        "axes": {"type": "array", "items": {"type": "object"}, "description": "{path, values} or {path, start, stop, count}; first axis slowest"},
                        "points": {"type": "array", "items": {"type": "object"}, "description": "explicit points, each {settings path: value}"},
                        "serpentine": {"type": "boolean"},
                        "on_failure": {"type": "string", "enum": ["continue", "stop"]},
                        "retry_cold": {"type": "boolean"},
                        "cold": {"type": "boolean"},
                        "max_steps": {"type": "integer", "minimum": 1}
                    }),
                    &["out_dir"],
                )
            },
        },
        Tool {
            name: "design_run_study",
            description: "Run the study's sampler or optimiser with the case solver, appending every evaluation to the study store; stops after max_evaluations new evaluations (bounded by max_design_evaluations) and continues from the store on the next call. Returns the best record and the Pareto front.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({"dir": {"type": "string"}, "max_evaluations": {"type": "integer", "minimum": 1}}),
                    &["dir"],
                )
            },
        },
        Tool {
            name: "design_evaluate",
            description: "Evaluate one candidate of a study (all its operating conditions) with the case solver and store the result; a candidate already in the store is returned without running.",
            access: Access::WriteFiles,
            schema: || obj(json!({"dir": {"type": "string"}, "params": {"type": "object", "description": "{parameter name: value}"}}), &["dir", "params"]),
        },
        Tool {
            name: "design_record",
            description: "Record metrics computed elsewhere (for example by running the case through the other tools) for a candidate of a study. Metric values must be finite numbers; give metrics for a single-condition study or conditions [{label, metrics}] for several.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "dir": {"type": "string"},
                        "params": {"type": "object"},
                        "metrics": {"type": "object"},
                        "conditions": {"type": "array", "items": {"type": "object"}},
                        "ok": {"type": "boolean"},
                        "note": {"type": "string"}
                    }),
                    &["dir", "params"],
                )
            },
        },
        Tool {
            name: "design_get_metrics",
            description: "Metrics of one stored evaluation of a study (by record index or exact parameter values), or the table of an operating map (map_dir).",
            access: Access::Read,
            schema: || {
                obj(
                    json!({"dir": {"type": "string"}, "index": {"type": "integer", "minimum": 0}, "params": {"type": "object"}, "map_dir": {"type": "string"}}),
                    &[],
                )
            },
        },
        Tool {
            name: "design_compare_variants",
            description: "Compare stored evaluations of a study side by side: parameters, metrics per condition, exact metric differences against the first record and a ranking by the study objective (feasible first).",
            access: Access::Read,
            schema: || {
                obj(
                    json!({"dir": {"type": "string"}, "indices": {"type": "array", "items": {"type": "integer", "minimum": 0}}, "candidates": {"type": "array", "items": {"type": "object"}}}),
                    &["dir"],
                )
            },
        },
        Tool {
            name: "design_propose",
            description: "The next candidate(s) the study's sampler or optimiser asks for, given the evaluations stored so far. Deterministic and free of side effects; evaluate or record them, then call again. complete is true when the method has nothing more to ask.",
            access: Access::Read,
            schema: || obj(json!({"dir": {"type": "string"}}), &["dir"]),
        },
        Tool {
            name: "design_list_study",
            description: "Definition, evaluation counts, best feasible record and Pareto front indices of a study.",
            access: Access::Read,
            schema: || obj(json!({"dir": {"type": "string"}}), &["dir"]),
        },
        // ---- wave 46D (ADR 0121): design ledger
        Tool {
            name: "design_ledger_append",
            description: "Append one evaluated variant to a design ledger (a JSON-lines file in the sandbox, created on first use): named parameters and metrics with units, evaluator id and version, geometry/mesh hash, settings, verdict (ok, diverged, constraint_rejected, not_evaluated; a located reason is required unless ok), fidelity tier. The solver version and a timestamp are stamped. Returns the entry index.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "ledger": {"type": "string", "description": "ledger file inside the sandbox, e.g. fan/ledger.jsonl"},
                        "study": {"type": "string"},
                        "variant": {"type": "string"},
                        "kind": {"type": "string", "description": "evaluation (default) or a summary kind"},
                        "params": {"type": "array", "items": {"type": "object"}, "description": "[{name, value, unit}] (unit \"1\" for dimensionless)"},
                        "metrics": {"type": "array", "items": {"type": "object"}, "description": "[{name, value, unit}]; value may be null when the verdict is not ok"},
                        "evaluator": {"type": "string"},
                        "evaluator_version": {"type": "string"},
                        "solver_version": {"type": "string", "description": "default: this server's rcfd version"},
                        "geometry_hash": {"type": "string"},
                        "settings": {"type": "object", "description": "what is needed to re-run (case settings, overrides, operating point)"},
                        "verdict": {"type": "string", "enum": ["ok", "diverged", "constraint_rejected", "not_evaluated"]},
                        "reason": {"type": "string"},
                        "fidelity": {"type": "string", "enum": ["analytic", "reduced_order", "cfd"]},
                        "wall_s": {"type": "number", "minimum": 0.0},
                        "seed": {"type": "integer", "minimum": 0},
                        "provenance": {"type": "object"}
                    }),
                    &["ledger", "params", "metrics", "evaluator", "evaluator_version", "verdict", "fidelity"],
                )
            },
        },
        Tool {
            name: "design_ledger_query",
            description: "Read a design ledger: entries filtered by kind, verdict, study, evaluator and fidelity (paged), plus optionally the Pareto-optimal entries for [{metric, sense}], the best entry and running best-so-far trace for {metric, sense}, and the history of one parameter. Pareto and best use ok evaluation entries of one evaluator and tier with consistent units (filter first when the ledger mixes them). Torn or foreign lines are reported in skipped_lines.",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "ledger": {"type": "string"},
                        "kind": {"type": "string"},
                        "verdict": {"type": "string", "enum": ["ok", "diverged", "constraint_rejected", "not_evaluated"]},
                        "study": {"type": "string"},
                        "evaluator": {"type": "string"},
                        "fidelity": {"type": "string", "enum": ["analytic", "reduced_order", "cfd"]},
                        "offset": {"type": "integer", "minimum": 0},
                        "limit": {"type": "integer", "minimum": 0, "maximum": 1000},
                        "pareto": {"type": "array", "items": {"type": "object"}, "description": "[{metric, sense: min|max}]"},
                        "best": {"type": "object", "description": "{metric, sense: min|max}"},
                        "history": {"type": "string", "description": "parameter name"}
                    }),
                    &["ledger"],
                )
            },
        },
        Tool {
            name: "design_ledger_export",
            description: "Export the (filtered) entries of a design ledger to a CSV file (one column per parameter and metric with its unit; numbers round-trip exactly) or a JSON file inside the sandbox.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "ledger": {"type": "string"},
                        "out": {"type": "string"},
                        "format": {"type": "string", "enum": ["csv", "json"]},
                        "kind": {"type": "string"},
                        "verdict": {"type": "string", "enum": ["ok", "diverged", "constraint_rejected", "not_evaluated"]},
                        "study": {"type": "string"},
                        "evaluator": {"type": "string"},
                        "fidelity": {"type": "string", "enum": ["analytic", "reduced_order", "cfd"]}
                    }),
                    &["ledger", "out"],
                )
            },
        },
        Tool {
            name: "design_ledger_rerun_check",
            description: "Compare the metrics of a re-run (done with the entry's evaluator, settings and parameters) with ledger entry index: per metric recorded, new, absolute difference and whether it is within atol + rtol |recorded| (defaults 0: bit-identical); units, missing metrics and the verdict are checked too.",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "ledger": {"type": "string"},
                        "index": {"type": "integer", "minimum": 0},
                        "metrics": {"type": "array", "items": {"type": "object"}, "description": "[{name, value, unit}] of the re-run"},
                        "verdict": {"type": "string", "enum": ["ok", "diverged", "constraint_rejected", "not_evaluated"]},
                        "rtol": {"type": "number", "minimum": 0.0},
                        "atol": {"type": "number", "minimum": 0.0}
                    }),
                    &["ledger", "index", "metrics"],
                )
            },
        },
        // ---- end wave 46D
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
            "list_workflow_tasks" => Ok(ok_result(svc.call("workflow.list", &with(json!({}))?)?)),
            "create_workflow" => {
                if let Some(path) = args.get("path") {
                    return Ok(ok_result(svc.call("workflow.load", &with(json!({"path": path}))?)?));
                }
                let mut p = Map::new();
                for k in ["template", "workflow", "name"] {
                    if let Some(v) = args.get(k) {
                        p.insert(k.into(), v.clone());
                    }
                }
                Ok(ok_result(svc.call("workflow.create", &with(Value::Object(p))?)?))
            }
            "edit_workflow" => match args["action"].as_str() {
                Some("insert") => {
                    let mut p = Map::new();
                    for k in ["task", "after"] {
                        if let Some(v) = args.get(k) {
                            p.insert(k.into(), v.clone());
                        }
                    }
                    Ok(ok_result(svc.call("workflow.insert", &with(Value::Object(p))?)?))
                }
                Some("delete") => Ok(ok_result(svc.call(
                    "workflow.delete",
                    &with(json!({"id": args.get("id").cloned().unwrap_or(Value::Null)}))?,
                )?)),
                _ => Err(RpcError {
                    code: -32602,
                    message: "action must be insert or delete".into(),
                    data: None,
                }),
            },
            "run_workflow_task" => {
                let mut p = Map::new();
                for k in ["task", "with_prerequisites", "max_steps"] {
                    if let Some(v) = args.get(k) {
                        p.insert(k.into(), v.clone());
                    }
                }
                Ok(ok_result(svc.call("workflow.run", &with(Value::Object(p))?)?))
            }
            "check_case_headless" => {
                let p = match args.get("path") {
                    Some(path) => json!({"path": path}),
                    None => with(json!({}))?,
                };
                Ok(ok_result(svc.call("run.check", &p)?))
            }
            "run_headless" => {
                let mut p = Map::new();
                for k in [
                    "max_steps",
                    "max_elapsed_s",
                    "stop_file",
                    "continue_from",
                    "interpolate_case",
                    "interpolate_checkpoint",
                    "interpolate_mode",
                    "checkpoint",
                    "checkpoint_every",
                    "status_path",
                    "status_every",
                    "out_dir",
                ] {
                    if let Some(v) = args.get(k) {
                        p.insert(k.into(), v.clone());
                    }
                }
                Ok(ok_result(svc.call("run.headless", &with(Value::Object(p))?)?))
            }
            "read_status_stream" => Ok(ok_result(svc.call("status.read", &json!({"path": args["path"]}))?)),
            "design_create_study"
            | "design_create_variant"
            | "design_run_operating_map"
            | "design_run_study"
            | "design_evaluate"
            | "design_record"
            | "design_get_metrics"
            | "design_compare_variants"
            | "design_propose"
            | "design_list_study"
            // wave 46D (ADR 0121)
            | "design_ledger_append"
            | "design_ledger_query"
            | "design_ledger_export"
            | "design_ledger_rerun_check" => {
                let method = match name {
                    "design_create_study" => "design.create_study",
                    "design_create_variant" => "design.create_variant",
                    "design_run_operating_map" => "design.run_map",
                    "design_run_study" => "design.run_study",
                    "design_evaluate" => "design.evaluate",
                    "design_record" => "design.record",
                    "design_get_metrics" => "design.get_metrics",
                    "design_compare_variants" => "design.compare",
                    "design_propose" => "design.propose",
                    "design_ledger_append" => "design.ledger_append",
                    "design_ledger_query" => "design.ledger_query",
                    "design_ledger_export" => "design.ledger_export",
                    "design_ledger_rerun_check" => "design.ledger_rerun_check",
                    _ => "design.list_study",
                };
                let mut p = args.clone();
                // a study can be made from the settings of the current session
                if name == "design_create_study" && args.get("base").is_none() && args.get("session").is_none() {
                    if let Some(id) = self.current_session() {
                        p["session"] = json!(id);
                    }
                }
                Ok(ok_result(svc.call(method, &p)?))
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
mod design_tests;
#[cfg(test)]
mod tests;
