//! Project tools (ADR 0135): tools on one project folder (`rcfd-mcp --project <dir>`), its run records, stored
//! fields and the run service. They are listed only when the server was given a project; without one the tool list
//! is unchanged (F1).
//!
//! The tools are the commands and queries of the project's workspace (`rcfd_workspace`, ADR 0173 W2, ADR 0185 E3): this
//! module holds their MCP descriptors (names, descriptions, input schemas, access classes) and wraps each answer of
//! [`rcfd_workspace::Workspace::call`] as an MCP result. Paths, limits, error codes and the write rule are the
//! workspace's. The server's journal lines name the actor `mcp` of this process (ADR 0188 J3); the write tools take an
//! optional `reason`, and `read_feed` lists the project's change feed (J7).

use crate::policy::Access;
use crate::{obj, ok_result, Tool};
use rcfd_api::service::RpcError;
use rcfd_workspace::journal::{Actor, MCP};
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

/// The optional `reason` of a write tool (ADR 0188 J4): kept with the change in the project journal. The workspace
/// enforces the length (the argument check here does not know `maxLength`).
fn reason_schema() -> Value {
    json!({"type": "string", "maxLength": rcfd_workspace::journal::MAX_REASON, "description": "why this change is made, in a short sentence (at most 500 characters): kept with it in the project journal (.rcfd/edits.jsonl), shown in the Studio and by read_feed"})
}

/// The run-time settings of `start_run` and `case_facts` (ADR 0737 P2): the engine's time-step rule of ADR 0153 and 0158.
fn run_time_schema() -> Value {
    json!({"type": "object", "description": "set the run's time step: solver.end_time = simulate_s (default the case's), a first step for the courant target (default 0.5) through the thinnest cell, then the solver's Courant controller up to max_dt (default simulate_s/100) (mode adaptive, the default), equal steps (fixed) or the case's own (case); adaptive_correctors (default true) adds solver.corrector_control=adaptive where the solver accepts it. Entries go before the store plan's and set's.",
        "properties": {
            "simulate_s": {"type": "number", "exclusiveMinimum": 0},
            "courant": {"type": "number", "exclusiveMinimum": 0},
            "max_dt": {"type": "number", "exclusiveMinimum": 0},
            "mode": {"type": "string", "enum": ["adaptive", "fixed", "case"]},
            "adaptive_correctors": {"type": "boolean"}
        },
        "additionalProperties": false})
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
            name: "derive_picture",
            description: "A derived picture of a stored snapshot of a run (default the last) as a GLB file in runs/<id>/derived/ (metres; POSITION, _CELL the cell of each vertex, _VALUE, _TRIANGLE): kind iso (the surface where a cell field equals level, marching tetrahedra; colour_by another field), streamlines (RK4 from seeds through the velocity ux, uy, uz, linear over tets), slice (the cells cut by an axis plane, a value per triangle; vectors adds each triangle's stored vector) or cut_surface (the Studio's picture: the boundary surface on the kept side of an axis plane, with vectors its arrows). Fields are the stored cell fields and speed. The same arguments give the same file, which is reused; rcfd post derive makes it without a project.",
            access: Access::Read,
            schema: || {
                let point = json!({"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3});
                obj(
                    json!({
                        "run": run_schema(),
                        "kind": {"type": "string", "enum": ["iso", "streamlines", "slice", "cut_surface"]},
                        "snapshot": {"type": "integer", "minimum": 0, "description": "snapshot index (or step or time, the nearest)"},
                        "step": {"type": "integer", "minimum": 0},
                        "time": {"type": "number"},
                        "field": {"type": "string", "description": "a stored cell field or speed (iso, slice; cut_surface colours by it)"},
                        "level": {"type": "number", "description": "iso: the value of the surface, inside the field's range"},
                        "colour_by": {"type": "string", "description": "iso: the field coloured on the surface (default the level)"},
                        "seeds": {"type": "object", "description": "streamlines: {rake: {p0, p1, n}}, {points: [[x, y, z], ...]} or {patch: name, n}; at most 1000 seeds", "properties": {
                            "rake": {"type": "object", "properties": {"p0": point.clone(), "p1": point.clone(), "n": {"type": "integer", "minimum": 1}}},
                            "points": {"type": "array", "items": point},
                            "patch": {"type": "string"},
                            "n": {"type": "integer", "minimum": 1}}},
                        "direction": {"type": "string", "enum": ["forward", "backward", "both"]},
                        "max_steps": {"type": "integer", "minimum": 1, "maximum": 10000, "description": "streamlines: steps per line (default 2000)"},
                        "max_length": {"type": "number", "description": "streamlines: arc length per line, m (default 4 x the bounding box diagonal)"},
                        "min_speed": {"type": "number", "description": "streamlines: a line stops below this speed (default 1e-9 x the largest)"},
                        "step_fraction": {"type": "number", "description": "streamlines: the RK4 step as a part of the cell length (default 0.25)"},
                        "axis": {"type": "string", "enum": ["x", "y", "z"], "description": "slice, cut_surface: the plane's normal axis"},
                        "at": {"type": "number", "description": "slice, cut_surface: the plane's coordinate, m"},
                        "keep": {"type": "string", "enum": ["below", "above"], "description": "cut_surface: the side kept (default below)"},
                        "vectors": {"type": "boolean", "description": "slice, cut_surface: add the vectors (arrows for cut_surface)"}
                    }),
                    &["run", "kind"],
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
            name: "read_feed",
            description: "The project's change feed: who changed what and why. Each entry is a line of the project journal (an edit with its actor, command, changed settings with old and new values, and reason; a case created, a mesh imported, a run submitted, paused, resumed or cancelled), a run's state change, warning or stop, or a file changed outside the workspace (external, path only). Call without since to get the current cursor; with since, the entries numbered after it, oldest first (limit, default 50; next to continue).",
            access: Access::Read,
            schema: || {
                obj(
                    json!({"since": {"type": "integer", "minimum": 0}, "limit": {"type": "integer", "minimum": 1, "maximum": 200},
                        "from_seq": {"type": "integer", "minimum": 1, "description": "instead of since: replay the project journal from this seq on (1 for all of it), also what happened before this server started; entries carry no n; next_seq continues, and since = cursor continues live"}}),
                    &[],
                )
            },
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
            name: "hash_project_file",
            description: "The hash of a project file or folder, by the rule of run.json mesh_hash: a file is fnv64 of its bytes; a folder (an OpenFOAM mesh) is fnv64 over the sorted lines <relative path>\\0<file hash>\\n of every file below it. Symbolic links are refused; at most 4 GiB and 100000 files (error 1003 names the limit). Compare with run.json mesh_hash to see whether a run's mesh changed since.",
            access: Access::Read,
            schema: || obj(json!({"path": {"type": "string", "description": "project-relative file or folder"}}), &["path"]),
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
                        "dry_run": {"type": "boolean"},
                        "reason": reason_schema()
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
                        "base_hash": {"type": "string"},
                        "reason": reason_schema()
                    }),
                    &["path", "text"],
                )
            },
        },
        Tool {
            name: "new_case",
            description: "Create a new case file cases/<name>.toml (or .json) in the project from exactly one source: a shipped example (byte for byte, with the data files it reads), a built-in template, TOML text, a settings object (written as JSON) or a copy of a project case. The case is validated before anything is written and no file is ever replaced. Returns its path and hash (the base_hash for edit_case).",
            access: Access::WriteFiles,
            schema: || {
                let mut s = new_case_schema();
                s["properties"]["reason"] = reason_schema();
                s
            },
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
                        "name": {"type": "string", "description": "name in meshes/ (default the source's name; a file keeps its extension)"},
                        "reason": reason_schema()
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
            name: "runner_config",
            description: "The run service's budgets: .rcfd/runner-config.json (exists, hash: the base_hash for set_runner_config, values, problem: why the runner would refuse the file), the runner's defaults on this machine, and in_use: the values the runner running now started with. The runner reads the file when it starts (applies: next_start).",
            access: Access::Read,
            schema: || obj(json!({}), &[]),
        },
        Tool {
            name: "set_runner_config",
            description: "Change the run service's budgets in .rcfd/runner-config.json (max_runs, cpu_threads, mem_budget_mb, mem_base_mb, mem_kb_per_cell, poll_ms, adopt_probe_ms, cancel_grace_ms): a number sets a key, null removes it (the runner's default). Checked as the runner checks the file (error 1003 at the key) and written atomically only if the file still has base_hash (null: there is no file yet; else error 1200 with the current hash). A file the runner cannot read is replaced only with replace: true and every key given. Applies at the next runner start; the runner is not restarted. Returns the budgets as runner_config does.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "set": {"type": "object", "description": "{key: number, or null for the runner's default}"},
                        "base_hash": {"type": ["string", "null"], "description": "hash from runner_config; null when there is no file"},
                        "replace": {"type": "boolean", "description": "write exactly set (every key) over a file the runner cannot read"},
                        "reason": reason_schema()
                    }),
                    &["set", "base_hash"],
                )
            },
        },
        Tool {
            name: "case_facts",
            description: "What a project case's time step depends on, without running it (ADR 0737): the kind of run, solver.dt and solver.end_time (and whether the file sets them), its own step control, cells, 2D, bounds, the thinnest and thickest cell (h_min, h_max, m), the fastest speed at the start (u0, m/s, and where), the pressure correctors; run_time: the plan of a run with the run_time settings of start_run (default: automatic step at Courant 0.5, adaptive correctors), its --set entries, first step, step and Courant estimates and a warning above Courant 2; memory: the runner's estimate with the project's budgets. Error 1003 when the case does not build.",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "case": {"type": "string", "description": "project-relative case file"},
                        "run_time": run_time_schema()
                    }),
                    &["case"],
                )
            },
        },
        Tool {
            name: "refine_case_mesh",
            description: "Refine x2 (ADR 0737): double every cell count of the case's generator mesh (mesh.nx, mesh.ny, mesh.nz and the second block's _right counts), written through edit_case's rule (only if the file still has base_hash, else error 1200; comments kept). Refused with the reason, nothing written: 1009 for a mesh file; 1004 for a generator counted by other settings (axial_fan_sector) and a count over its maximum. Returns the edits, cells before and after, the runner's memory estimate after, and edit_case's result.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "case": {"type": "string", "description": "project-relative case file"},
                        "factor": {"type": "integer", "enum": [2], "description": "only 2"},
                        "base_hash": {"type": "string", "description": "hash from read_project_file"},
                        "reason": reason_schema()
                    }),
                    &["case", "factor", "base_hash"],
                )
            },
        },
        Tool {
            name: "list_sites",
            description: "The run sites of the user's sites file (ADR 0739): name, kind, host, engine and work folders, budgets, whether MCP clients may name the site (assistant) and whether the user confirmed it. start_run with site queues a run on one with assistant true. Nothing is started and nothing is sent.",
            access: Access::Read,
            schema: || obj(json!({}), &[]),
        },
        Tool {
            name: "start_run",
            description: "Queue a run of a project case with the project's run service and start the service if none is alive. set holds rcfd run --set overrides (path=value). Returns the run id.",
            access: Access::Execute,
            schema: || {
                obj(
                    json!({
                        "new_case": {"description": "instead of case: a new case made first, with the arguments of new_case, then run", "type": "object", "properties": new_case_schema()["properties"], "required": ["name"], "additionalProperties": false},
                        "store": {"type": "string", "enum": ["plan", "none"], "description": "plan: the Studio's start plan stores the run's fields (about 100 snapshots, f32) unless the case or set gives the store; none: only what the case asks for. Default plan; none is the way back to a run that stores only what its case asks for"},
                        "reason": reason_schema(),
                        "case": {"type": "string", "description": "project-relative case file"},
                        "set": {"type": "array", "items": {"type": "string"}, "description": "overrides path=value, in order"},
                        "priority": {"type": "integer"},
                        "threads": {"type": "integer", "minimum": 1, "maximum": 256},
                        "mem_mb": {"type": "integer", "minimum": 1},
                        "run_time": run_time_schema(),
                        "dry_run": {"type": "boolean", "description": "return what the call would do (case, set, store and run-time plans, the check, memory, whether a runner would start) and write nothing (ADR 0737)"},
                        "continue_from": {"type": "string", "description": "a run id of this project: the run starts from that ended run's output/restart.rcfd"},
                        "max_steps": {"type": "integer", "minimum": 1, "description": "at most this many steps"},
                        "from_run": {"type": "string", "description": "instead of case or new_case: rerun that run id from its case.snapshot with its run.json settings (priority, threads and mem_mb given override them); the new run.json has rerun_of"},
                        "site": {"type": "string", "description": "a site of list_sites with assistant true: the run goes there (its inputs are uploaded, its folder here is kept up to date from the site); any other site is refused before anything is sent"}
                    }),
                    &[],
                )
            },
        },
        Tool {
            name: "pause_run",
            description: "Hold a run: a queued run is not launched, a running one stops between two steps.",
            access: Access::Execute,
            schema: || obj(json!({"run": run_schema(), "reason": reason_schema()}), &["run"]),
        },
        Tool {
            name: "resume_run",
            description: "Release a held run.",
            access: Access::Execute,
            schema: || obj(json!({"run": run_schema(), "reason": reason_schema()}), &["run"]),
        },
        Tool {
            name: "cancel_run",
            description: "Cancel a run; a running solver stops cleanly and writes its results.",
            access: Access::Execute,
            schema: || obj(json!({"run": run_schema(), "reason": reason_schema()}), &["run"]),
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
        // ADR 0736 P1 (F-829 E-829a): the project trash, after the F-275 list
        Tool {
            name: "list_trash",
            description: "The project trash (.rcfd/trash/), newest first: each item's token, kind (run, results or case), original path, parts, when and by which command it was trashed, the reason, the hash of a case file and its bytes; bytes is the whole trash. Paged (offset, limit; next_offset when more). Nothing empties the trash by itself.",
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
            name: "trash_run",
            description: "Move a finished, failed or cancelled run's folder runs/<id>/ whole to the project trash, from where restore_trash brings it back. Refused (1002) while the run is queued, running or paused, and (1003, data.runs) while a run that has not ended continues from it (continue_from). Returns the trash token.",
            access: Access::WriteFiles,
            schema: || obj(json!({"run": run_schema(), "reason": reason_schema()}), &["run"]),
        },
        Tool {
            name: "trash_results",
            description: "Move a finished, failed or cancelled run's stored fields (runs/<id>/fields/) and/or its output (runs/<id>/output/) to the project trash as one item; the run's record, status, events and residuals stay. Refused as trash_run, and (1202) when a part is absent. Returns the trash token.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "run": run_schema(),
                        "parts": {"type": "array", "items": {"type": "string", "enum": ["fields", "output"]}, "minItems": 1, "description": "what to move"},
                        "reason": reason_schema()
                    }),
                    &["run", "parts"],
                )
            },
        },
        Tool {
            name: "trash_case",
            description: "Move a case file under cases/ to the project trash, only if it still has base_hash (else error 1200 with the current hash), under its edit lock. Runs keep their own case.snapshot, so no run changes. Returns the trash token.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "path": case_schema(),
                        "base_hash": {"type": "string", "description": "hash from read_project_file"},
                        "reason": reason_schema()
                    }),
                    &["path", "base_hash"],
                )
            },
        },
        Tool {
            name: "restore_trash",
            description: "Move a trashed item back to its original path. Refused (1200, data.path) when something occupies that path now; then nothing moves.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "token": {"type": "string", "description": "trash token (t-<yyyymmddThhmmss>-<4 hex>, see list_trash)"},
                        "reason": reason_schema()
                    }),
                    &["token"],
                )
            },
        },
        Tool {
            name: "empty_trash",
            description: "Remove items of the project trash for good: the tokens given (each must be in the trash, else 1202 and nothing is removed), or every item; with older_than_days, only items trashed longer ago. Cannot be undone. Class delete: no allow-list includes it, so the client asks for every call.",
            access: Access::Delete,
            schema: || {
                obj(
                    json!({
                        "tokens": {"type": "array", "items": {"type": "string"}, "description": "trash tokens (default: every item)"},
                        "older_than_days": {"type": "number", "minimum": 0.0},
                        "reason": reason_schema()
                    }),
                    &[],
                )
            },
        },
    ]
}

/// Whether `name` can be the actor client of `--client`: 1 to 32 of `a-z`, `0-9`, `_` and `-`.
pub fn valid_client(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
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
    /// `rcfd-runner` next to the current executable or named by `RCFD_RUNNER_BIN`. Journal lines name the actor `mcp` of
    /// this process.
    pub fn open(dir: &Path) -> Result<Self, String> {
        Self::open_as(dir, MCP)
    }

    /// As [`ProjectTools::open`], with journal lines naming the actor `client` of this process instead of `mcp`
    /// (`rcfd-mcp --client <name>`, ADR 0801 Addendum 23: the Studio starts its own servers with `--client studio`, so
    /// the feed tells its writes from those of an MCP client such as Claude Code). `client` is 1 to 32 of `a-z`, `0-9`,
    /// `_` and `-`; anything else is refused with the reason.
    pub fn open_as(dir: &Path, client: &str) -> Result<Self, String> {
        if !valid_client(client) {
            return Err(format!(
                "--client {client:?}: a client name is 1 to 32 of a-z, 0-9, '_' and '-'"
            ));
        }
        Ok(ProjectTools {
            ws: Workspace::open(dir)?.with_actor(Actor::this_process(client)),
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
