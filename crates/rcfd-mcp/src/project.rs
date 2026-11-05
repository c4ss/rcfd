//! Project tools (ADR 0135): tools on one project folder (`rcfd-mcp --project <dir>`), its run records, stored
//! fields and the run service. They are listed only when the server was given a project; without one the tool list
//! is unchanged (F1).
//!
//! The tools are the commands and queries of the project's workspace (`rcfd_workspace`, ADR 0173 W2, ADR 0185 E3): this
//! module holds their MCP descriptors (names, descriptions, input schemas, access classes) and wraps each answer of
//! [`rcfd_workspace::Workspace::call`] as an MCP result. Paths, limits, error codes and the write rule are the
//! workspace's.

use crate::policy::Access;
use crate::{obj, ok_result, Tool};
use rcfd_api::service::RpcError;
use rcfd_workspace::{Answer, Workspace};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub use rcfd_workspace::{
    decimate_minmax, residual_series, CONFLICT, FILE_PAGE, LOCK_BUSY, MAX_WAIT_S, NOT_FOUND, TEXT_BUDGET,
};

const STATES: [&str; 6] = ["queued", "running", "paused", "finished", "failed", "cancelled"];

fn run_schema() -> Value {
    json!({"type": "string", "description": "run id (r-<yyyymmddThhmmss>-<4 hex>, see list_runs)"})
}

fn state_schema() -> Value {
    json!({"type": "string", "enum": STATES})
}

/// A project case argument of the mesh tools (ADR 0187).
fn case_schema() -> Value {
    json!({"type": "string", "description": "project-relative case file (.toml or .json), e.g. cases/cavity.toml"})
}

/// The arguments of `new_case` (ADR 0186 G3), also `start_run`'s `new_case` object.
fn new_case_schema() -> Value {
    let examples: Vec<&str> = rcfd_workspace::examples::EXAMPLES.iter().map(|e| e.source).collect();
    let templates: Vec<&str> = rcfd_workspace::templates::TEMPLATES.iter().map(|t| t.0).collect();
    obj(
        json!({
            "name": {"type": "string", "description": "case name: letters, digits, '-' and '_', at most 64 characters, not taken in cases/ in any letter case; the file is cases/<name>.toml (.json for settings or a .json source)"},
            "example": {"type": "string", "enum": examples, "description": "a shipped example, written byte for byte with the data files it reads"},
            "template": {"type": "string", "enum": templates, "description": "a built-in template"},
            "toml": {"type": "string", "description": "case file text (TOML)"},
            "settings": {"type": "object", "description": "case settings as JSON, written as cases/<name>.json"},
            "copy": {"type": "string", "description": "project-relative case file to copy byte for byte"}
        }),
        &["name"],
    )
}

fn snapshot_props(extra: Value) -> Value {
    let mut p = json!({
        "run": run_schema(),
        "field": {"type": "string", "description": "stored field name (default store: ux, uy, uz, p)"},
        "step": {"type": "integer", "minimum": 0, "description": "snapshot by step (with pick nearest or floor)"},
        "time": {"type": "number", "description": "snapshot by simulated time"},
        "pick": {"type": "string", "enum": ["nearest", "floor", "linear"], "description": "how a step or time is matched (default nearest; linear interpolates in time)"}
    });
    for (k, v) in extra.as_object().into_iter().flatten() {
        p[k] = v.clone();
    }
    p
}

/// The project tools, in `tools/list` order.
pub(crate) fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "project_info",
            description: "The project folder: manifest, counts of cases, meshes, designs, notes and runs (by state), and the problems validate finds.",
            access: Access::Read,
            schema: || obj(json!({}), &[]),
        },
        Tool {
            name: "list_runs",
            description: "Runs of the project, oldest first, with state, step, time and numeric metrics; run folders that cannot be read are listed as skipped with the reason. Filter by state or case; paged (offset, limit; next_offset when more).",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "state": state_schema(),
                        "case": {"type": "string", "description": "project-relative case path, e.g. cases/cavity.toml"},
                        "offset": {"type": "integer", "minimum": 0},
                        "limit": {"type": "integer", "minimum": 1, "maximum": 100}
                    }),
                    &[],
                )
            },
        },
        Tool {
            name: "run_status",
            description: "One run: its record (case, created, solver version, settings), status (state, step, time, reason, exit code), the last residual line and the last events.",
            access: Access::Read,
            schema: || obj(json!({"run": run_schema(), "events": {"type": "integer", "minimum": 0, "maximum": 50}}), &["run"]),
        },
        Tool {
            name: "read_metrics",
            description: "metrics.json (final report values) of one or more runs (at most 50).",
            access: Access::Read,
            schema: || obj(json!({"runs": {"type": "array", "items": {"type": "string"}}}), &["runs"]),
        },
        Tool {
            name: "read_residuals",
            description: "Residual and monitor series of a run (residuals.<equation>, monitors.<name>, and, for runs with adaptive pressure correctors, correctors = the count of the step's final outer iteration and corrector_cap = the estimates so far that ended at the cap) against the step, each decimated to at most max_points (default 200) keeping first, last and every bucket's minimum and maximum. A result over the size budget is written to runs/<id>/mcp/ and returned as a path.",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "run": run_schema(),
                        "names": {"type": "array", "items": {"type": "string"}, "description": "series to return (default all)"},
                        "max_points": {"type": "integer", "minimum": 2, "maximum": 2000},
                        "since_step": {"type": "integer", "minimum": 0}
                    }),
                    &["run"],
                )
            },
        },
        Tool {
            name: "field_summary",
            description: "Minimum, maximum and mean (cell-volume weighted for cell fields) of a field stored in a run's field store (runs/<id>/fields), at a step or time (nearest, floor or linear in time); default the last snapshot.",
            access: Access::Read,
            schema: || obj(snapshot_props(json!({})), &["run", "field"]),
        },
        Tool {
            name: "render_field",
            description: "PNG image of a stored cell field of a 2D case at a step or time (default the last snapshot), drawn by the CPU renderer of render_image. An image over the size budget is written to runs/<id>/renders/ and returned as a path.",
            access: Access::Read,
            schema: || {
                obj(
                    snapshot_props(json!({"width": {"type": "integer", "minimum": 64, "maximum": 4096}})),
                    &["run", "field"],
                )
            },
        },
        Tool {
            name: "compare_runs",
            description: "Metrics of 2 to 10 runs side by side with the differences to the first run, and each run's state and residual history length.",
            access: Access::Read,
            schema: || obj(json!({"runs": {"type": "array", "items": {"type": "string"}}}), &["runs"]),
        },
        Tool {
            name: "list_changes",
            description: "Project-relative paths created, changed or deleted since a change token. Call without since to get the current token; .rcfd/ is not watched.",
            access: Access::Read,
            schema: || obj(json!({"since": {"type": "integer", "minimum": 0}}), &[]),
        },
        Tool {
            name: "read_project_file",
            description: "Text of a project file (a case, mesh recipe or note) with the hash of the whole file, which is the base_hash for edit_case and write_note. Read in pages of at most 32000 bytes (offset, limit; next_offset when more).",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "path": {"type": "string", "description": "project-relative path"},
                        "offset": {"type": "integer", "minimum": 0},
                        "limit": {"type": "integer", "minimum": 1, "maximum": 32000}
                    }),
                    &["path"],
                )
            },
        },
        Tool {
            name: "edit_case",
            description: "Set dotted settings (e.g. physics.nu, solver.max_iterations) in a .toml or .json case file. The edited case is validated as set_setting does; the file is replaced atomically only if valid and only if it still has base_hash (else error 1200 with the current hash). The changed lines are edited in place (comments kept) when possible. dry_run validates without writing.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "path": {"type": "string", "description": "project-relative case file (.toml or .json)"},
                        "base_hash": {"type": "string", "description": "hash from read_project_file"},
                        "set": {"type": "object", "description": "{dotted setting path: value}"},
                        "dry_run": {"type": "boolean"}
                    }),
                    &["path", "base_hash", "set"],
                )
            },
        },
        Tool {
            name: "write_note",
            description: "Create or replace a text file under notes/. Without base_hash the file must not exist yet; with it the file is replaced only if it still has that hash (else error 1200 with the current hash).",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "path": {"type": "string", "description": "project-relative path under notes/"},
                        "text": {"type": "string"},
                        "base_hash": {"type": "string"}
                    }),
                    &["path", "text"],
                )
            },
        },
        Tool {
            name: "new_case",
            description: "Create a new case file cases/<name>.toml (or .json) in the project from exactly one source: a shipped example (byte for byte, with the data files it reads), a built-in template, TOML text, a settings object (written as JSON) or a copy of a project case. The case is validated before anything is written and no file is ever replaced. Returns its path and hash (the base_hash for edit_case).",
            access: Access::WriteFiles,
            schema: new_case_schema,
        },
        Tool {
            name: "check_case_mesh",
            description: "Build a project case's mesh as its settings describe it (a generator, or mesh.file with mesh.scale and mesh.transform) and return the quality report of rcfd check-mesh and the boundary patches, whose names boundaries.* must use. The mesh file must be in the project folder. Change mesh settings with edit_case, which checks the edited mesh too.",
            access: Access::Read,
            schema: || obj(json!({"case": case_schema()}), &["case"]),
        },
        Tool {
            name: "preview_mesh",
            description: "PNG image of a project case's mesh coloured by a cell-quality measure (default non_orthogonality, the largest of the cell's faces, degrees), drawn by the CPU renderer of render_field: the cells cut by a plane, by default the middle of the thinnest direction (a 2D case face on). Colours run from the ideal value up to the largest value. An image over the size budget is drawn narrower.",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "case": case_schema(),
                        "measure": {"type": "string", "enum": rcfd_workspace::meshes::MEASURES},
                        "width": {"type": "integer", "minimum": 64, "maximum": 4096},
                        "axis": {"type": "string", "enum": ["x", "y", "z"], "description": "normal of the drawn plane"},
                        "slice": {"type": "number", "description": "coordinate of the plane along axis (default the middle)"}
                    }),
                    &["case"],
                )
            },
        },
        Tool {
            name: "import_mesh",
            description: "Bring a mesh that is in the project folder into meshes/: a Gmsh or Fluent .msh/.cas file, or an OpenFOAM polyMesh folder. It is read with the solver's readers first (cells, faces, patches, zones and bounds are returned), then copied to meshes/<name> without replacing anything. Point a case at it with edit_case: mesh.source = \"file\" and mesh.file = the returned mesh_file.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "path": {"type": "string", "description": "project-relative mesh file or OpenFOAM folder"},
                        "name": {"type": "string", "description": "name in meshes/ (default the source's name; a file keeps its extension)"}
                    }),
                    &["path"],
                )
            },
        },
        Tool {
            name: "list_meshes",
            description: "Meshes under meshes/ (files, and OpenFOAM folders as one entry) with their size and the project cases whose mesh.file names each one; also case mesh files that are not in meshes/ and cases that could not be read. Paged (offset, limit; next_offset when more).",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "offset": {"type": "integer", "minimum": 0},
                        "limit": {"type": "integer", "minimum": 1, "maximum": 100}
                    }),
                    &[],
                )
            },
        },
        Tool {
            name: "start_run",
            description: "Queue a run of a project case with the project's run service and start the service if none is alive. set holds rcfd run --set overrides (path=value). Returns the run id.",
            access: Access::Execute,
            schema: || {
                obj(
                    json!({
                        "new_case": {"description": "instead of case: a new case made first, with the arguments of new_case, then run", "type": "object", "properties": new_case_schema()["properties"], "required": ["name"], "additionalProperties": false},
                        "store": {"type": "string", "enum": ["plan", "none"], "description": "plan: the Studio's start plan stores the run's fields (about 100 snapshots, f32) unless the case or set gives the store; none: only what the case asks for. Default plan with new_case, none with case"},
                        "case": {"type": "string", "description": "project-relative case file"},
                        "set": {"type": "array", "items": {"type": "string"}, "description": "overrides path=value, in order"},
                        "priority": {"type": "integer"},
                        "threads": {"type": "integer", "minimum": 1, "maximum": 256},
                        "mem_mb": {"type": "integer", "minimum": 1}
                    }),
                    &[],
                )
            },
        },
        Tool {
            name: "pause_run",
            description: "Hold a run: a queued run is not launched, a running one stops between two steps.",
            access: Access::Execute,
            schema: || obj(json!({"run": run_schema()}), &["run"]),
        },
        Tool {
            name: "resume_run",
            description: "Release a held run.",
            access: Access::Execute,
            schema: || obj(json!({"run": run_schema()}), &["run"]),
        },
        Tool {
            name: "cancel_run",
            description: "Cancel a run; a running solver stops cleanly and writes its results.",
            access: Access::Execute,
            schema: || obj(json!({"run": run_schema()}), &["run"]),
        },
        Tool {
            name: "wait_run",
            description: "Wait until a run's state differs from from (default: its state now) or the run is finished, failed or cancelled, for at most timeout_s seconds (default 30, at most 60). Call again to keep waiting.",
            access: Access::Read,
            schema: || {
                obj(
                    json!({"run": run_schema(), "timeout_s": {"type": "number", "minimum": 0.0, "maximum": MAX_WAIT_S}, "from": state_schema()}),
                    &["run"],
                )
            },
        },
    ]
}

/// The project tools' state: the project's workspace (its project, run service and change watcher).
pub struct ProjectTools {
    ws: Workspace,
}

impl std::fmt::Debug for ProjectTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectTools").field("root", &self.ws.root()).finish()
    }
}

impl ProjectTools {
    /// Open the project folder `dir` (a located error when it is not a project). Runs are started with the
    /// `rcfd-runner` next to the current executable or named by `RCFD_RUNNER_BIN`.
    pub fn open(dir: &Path) -> Result<Self, String> {
        Ok(ProjectTools {
            ws: Workspace::open(dir)?,
        })
    }

    /// Start runs with the runner executable `exe` instead.
    pub fn with_runner_exe(mut self, exe: impl Into<PathBuf>) -> Self {
        self.ws = self.ws.with_runner_exe(exe);
        self
    }

    /// Canonical project root.
    pub fn root(&self) -> &Path {
        self.ws.root()
    }

    /// The project's workspace.
    pub fn workspace(&self) -> &Workspace {
        &self.ws
    }

    /// Execute project tool `name`; `None` if it is not one.
    pub(crate) fn call(&self, name: &str, args: &Value) -> Option<Result<Value, RpcError>> {
        Some(self.ws.call(name, args)?.map(|a| match a {
            Answer::Value(v) => ok_result(v),
            Answer::Image { png_base64, meta } => json!({
                "content": [
                    {"type": "image", "data": png_base64, "mimeType": "image/png"},
                    {"type": "text", "text": meta.to_string()}
                ],
                "structuredContent": meta,
                "isError": false
            }),
        }))
    }
}
