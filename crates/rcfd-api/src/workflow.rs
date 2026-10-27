//! Task-based workflow over the settings tree (F-120).
//!
//! A [`Workflow`] is an ordered list of [`Task`]s with dependencies. Each task wraps one step of
//! the set-up and run (check the mesh, validate, change a setting, initialise, solve, report,
//! export). The model is pure data: planning, dependency and staleness rules live here, execution
//! lives in the service (`workflow.*` methods) so every client (CLI, MCP, assistant) runs the same
//! tasks. Documents serialise to `{"format":"rcfd-workflow","version":1,...}`.

use serde_json::{json, Value};

/// Document format name.
pub const FORMAT: &str = "rcfd-workflow";
/// Document version.
pub const VERSION: u64 = 1;

/// Known task kinds with their parameters.
pub const KINDS: &[(&str, &str)] = &[
    (
        "check_mesh",
        "build the mesh and run the quality check; fails when the check fails",
    ),
    (
        "validate",
        "validate the settings and build the simulation; fails on any diagnostic",
    ),
    ("set", "params {path, value}: change one setting"),
    ("initialize", "build the simulation and initial fields"),
    (
        "solve",
        "params {max_steps?}: run to convergence, end time or the step cap",
    ),
    (
        "report",
        "params {format?: html|csv|json, path?}: evaluate reports and optionally write a file",
    ),
    ("export_vtk", "params {dir?, name?}: write the solution as VTK"),
];

/// Run state of one task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Never run.
    NotRun,
    /// Ran successfully and nothing it depends on changed since.
    Done,
    /// Ran and failed.
    Failed,
    /// Ran, but a setting or an upstream task changed afterwards: run again.
    Stale,
}

impl TaskState {
    /// Name used in JSON.
    pub fn name(self) -> &'static str {
        match self {
            TaskState::NotRun => "not_run",
            TaskState::Done => "done",
            TaskState::Failed => "failed",
            TaskState::Stale => "stale",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "not_run" => TaskState::NotRun,
            "done" => TaskState::Done,
            "failed" => TaskState::Failed,
            "stale" => TaskState::Stale,
            _ => return None,
        })
    }
}

/// One step of a workflow.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Unique id within the workflow.
    pub id: String,
    /// Display title.
    pub title: String,
    /// Kind, one of [`KINDS`].
    pub kind: String,
    /// Ids of the tasks that must be done first.
    pub deps: Vec<String>,
    /// Kind-specific parameters.
    pub params: Value,
    /// Current state.
    pub state: TaskState,
    /// Message of the last run.
    pub message: String,
    /// Result of the last run.
    pub result: Value,
}

impl Task {
    /// New task that has not run.
    pub fn new(id: &str, title: &str, kind: &str, deps: &[&str], params: Value) -> Self {
        Task {
            id: id.into(),
            title: title.into(),
            kind: kind.into(),
            deps: deps.iter().map(|s| s.to_string()).collect(),
            params,
            state: TaskState::NotRun,
            message: String::new(),
            result: Value::Null,
        }
    }

    /// JSON form.
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id, "title": self.title, "kind": self.kind, "deps": self.deps,
            "params": self.params, "state": self.state.name(), "message": self.message,
            "result": self.result,
        })
    }
}

/// An ordered, dependency-checked list of tasks.
#[derive(Debug, Clone, PartialEq)]
pub struct Workflow {
    /// Name.
    pub name: String,
    /// Tasks in display order.
    pub tasks: Vec<Task>,
}

/// Names accepted by [`Workflow::template`].
pub const TEMPLATES: &[(&str, &str)] = &[
    (
        "mesh_to_results",
        "check mesh, validate set-up, initialise, solve, report",
    ),
    ("channel_flow", "as mesh_to_results plus a VTK export of the solution"),
    ("empty", "no tasks"),
];

impl Workflow {
    /// A named template.
    pub fn template(name: &str) -> Option<Workflow> {
        let mut tasks = vec![
            Task::new("mesh_check", "Check the mesh", "check_mesh", &[], json!({})),
            Task::new("setup", "Validate the set-up", "validate", &["mesh_check"], json!({})),
            Task::new(
                "initialize",
                "Initialise the solution",
                "initialize",
                &["setup"],
                json!({}),
            ),
            Task::new("solve", "Solve", "solve", &["initialize"], json!({})),
            Task::new(
                "report",
                "Report results",
                "report",
                &["solve"],
                json!({"format": "json"}),
            ),
        ];
        match name {
            "mesh_to_results" => {}
            "channel_flow" => tasks.push(Task::new(
                "export_results",
                "Export the solution (VTK)",
                "export_vtk",
                &["solve"],
                json!({"dir": "results", "name": "solution"}),
            )),
            "empty" => tasks.clear(),
            _ => return None,
        }
        Some(Workflow {
            name: name.into(),
            tasks,
        })
    }

    /// Index of a task.
    pub fn index(&self, id: &str) -> Option<usize> {
        self.tasks.iter().position(|t| t.id == id)
    }

    /// Task by id.
    pub fn task(&self, id: &str) -> Option<&Task> {
        self.index(id).map(|i| &self.tasks[i])
    }

    /// Check ids, kinds, parameters, dependencies and that there is no cycle.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        for t in &self.tasks {
            if t.id.is_empty() || !t.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return Err(format!("task id '{}' must be letters, digits, '_' or '-'", t.id));
            }
            if !seen.insert(t.id.as_str()) {
                return Err(format!("duplicate task id '{}'", t.id));
            }
            if !KINDS.iter().any(|(k, _)| *k == t.kind) {
                return Err(format!(
                    "task '{}': unknown kind '{}' (known: {})",
                    t.id,
                    t.kind,
                    KINDS.iter().map(|k| k.0).collect::<Vec<_>>().join(", ")
                ));
            }
            if !t.params.is_object() {
                return Err(format!("task '{}': params must be an object", t.id));
            }
            if t.kind == "set" && (!t.params["path"].is_string() || t.params.get("value").is_none()) {
                return Err(format!("task '{}': a set task needs params path and value", t.id));
            }
        }
        for t in &self.tasks {
            for d in &t.deps {
                if d == &t.id {
                    return Err(format!("task '{}' depends on itself", t.id));
                }
                if self.index(d).is_none() {
                    return Err(format!("task '{}' depends on unknown task '{d}'", t.id));
                }
            }
        }
        // cycle check by repeated removal of tasks whose dependencies are gone
        let mut left: Vec<&Task> = self.tasks.iter().collect();
        let mut done: std::collections::HashSet<&str> = std::collections::HashSet::new();
        while !left.is_empty() {
            let before = left.len();
            left.retain(|t| {
                if t.deps.iter().all(|d| done.contains(d.as_str())) {
                    done.insert(t.id.as_str());
                    false
                } else {
                    true
                }
            });
            if left.len() == before {
                return Err(format!(
                    "dependency cycle among tasks: {}",
                    left.iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
        }
        Ok(())
    }

    /// Insert a task (after `after` when given, else at the end); the result must stay valid.
    pub fn insert(&mut self, task: Task, after: Option<&str>) -> Result<(), String> {
        let pos = match after {
            Some(a) => self.index(a).ok_or_else(|| format!("unknown task '{a}'"))? + 1,
            None => self.tasks.len(),
        };
        let backup = self.tasks.clone();
        self.tasks.insert(pos, task);
        if let Err(e) = self.validate() {
            self.tasks = backup;
            return Err(e);
        }
        Ok(())
    }

    /// Delete a task; refused while another task depends on it.
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        let i = self.index(id).ok_or_else(|| format!("unknown task '{id}'"))?;
        let users: Vec<&str> = self
            .tasks
            .iter()
            .filter(|t| t.deps.iter().any(|d| d == id))
            .map(|t| t.id.as_str())
            .collect();
        if !users.is_empty() {
            return Err(format!(
                "cannot delete '{id}': task(s) {} depend on it",
                users.join(", ")
            ));
        }
        self.tasks.remove(i);
        Ok(())
    }

    /// Ids of `id` and, when `with_prerequisites`, of everything it needs that is not done,
    /// in an order that respects the dependencies.
    pub fn plan(&self, id: &str, with_prerequisites: bool) -> Result<Vec<String>, String> {
        self.index(id).ok_or_else(|| format!("unknown task '{id}'"))?;
        let mut needed = std::collections::HashSet::new();
        needed.insert(id.to_string());
        if with_prerequisites {
            let mut stack = vec![id.to_string()];
            while let Some(x) = stack.pop() {
                let t = self.task(&x).ok_or_else(|| format!("unknown task '{x}'"))?;
                for d in &t.deps {
                    let dt = self.task(d).ok_or_else(|| format!("unknown task '{d}'"))?;
                    if dt.state != TaskState::Done && needed.insert(d.clone()) {
                        stack.push(d.clone());
                    }
                }
            }
        } else if let Some(bad) = self
            .task(id)
            .map(|t| {
                t.deps
                    .iter()
                    .find(|d| self.task(d).is_none_or(|x| x.state != TaskState::Done))
            })
            .and_then(|x| x.cloned())
        {
            return Err(format!("prerequisite task '{bad}' is not done"));
        }
        // display order is a valid topological order for the templates; sort robustly anyway
        let mut order: Vec<String> = Vec::new();
        let mut pending: Vec<&Task> = self.tasks.iter().filter(|t| needed.contains(&t.id)).collect();
        while !pending.is_empty() {
            let before = pending.len();
            pending.retain(|t| {
                if t.deps.iter().all(|d| order.contains(d) || !needed.contains(d)) {
                    order.push(t.id.clone());
                    false
                } else {
                    true
                }
            });
            if pending.len() == before {
                return Err("dependency cycle".into());
            }
        }
        Ok(order)
    }

    /// Mark the transitive dependants of `id` stale (those that were done).
    pub fn mark_dependants_stale(&mut self, id: &str) {
        let mut frontier = vec![id.to_string()];
        while let Some(x) = frontier.pop() {
            for t in self.tasks.iter_mut() {
                if t.deps.iter().any(|d| d == &x) {
                    if t.state == TaskState::Done {
                        t.state = TaskState::Stale;
                    }
                    frontier.push(t.id.clone());
                }
            }
        }
    }

    /// A setting changed outside the workflow: every done task becomes stale.
    pub fn invalidate_all(&mut self) {
        for t in self.tasks.iter_mut().filter(|t| t.state == TaskState::Done) {
            t.state = TaskState::Stale;
        }
    }

    /// Record the outcome of running `id`.
    pub fn finish(&mut self, id: &str, ok: bool, message: String, result: Value) {
        if let Some(i) = self.index(id) {
            self.tasks[i].state = if ok { TaskState::Done } else { TaskState::Failed };
            self.tasks[i].message = message;
            self.tasks[i].result = result;
            self.mark_dependants_stale(id);
        }
    }

    /// Short per-task summary.
    pub fn summary(&self) -> Value {
        let states: Vec<Value> = self
            .tasks
            .iter()
            .map(|t| json!({"id": t.id, "title": t.title, "kind": t.kind, "deps": t.deps, "state": t.state.name(), "message": t.message}))
            .collect();
        let next = self
            .tasks
            .iter()
            .find(|t| t.state != TaskState::Done)
            .map(|t| t.id.clone());
        json!({"name": self.name, "tasks": states, "next": next,
               "complete": self.tasks.iter().all(|t| t.state == TaskState::Done)})
    }

    /// Full document.
    pub fn to_json(&self) -> Value {
        json!({
            "format": FORMAT, "version": VERSION, "name": self.name,
            "tasks": self.tasks.iter().map(Task::to_json).collect::<Vec<_>>(),
        })
    }

    /// Parse and validate a document.
    pub fn from_json(v: &Value) -> Result<Workflow, String> {
        if v["format"] != FORMAT {
            return Err(format!("not an {FORMAT} document"));
        }
        if v["version"] != VERSION {
            return Err(format!("unsupported {FORMAT} version {}", v["version"]));
        }
        let mut tasks = Vec::new();
        for t in v["tasks"].as_array().ok_or("workflow has no 'tasks' array")? {
            tasks.push(task_from_json(t)?);
        }
        let w = Workflow {
            name: v["name"].as_str().unwrap_or("workflow").to_string(),
            tasks,
        };
        w.validate()?;
        Ok(w)
    }
}

/// Parse one task object (`id`, `kind` required; `title`, `deps`, `params` optional).
pub fn task_from_json(t: &Value) -> Result<Task, String> {
    let id = t["id"].as_str().ok_or("task needs a string 'id'")?;
    let kind = t["kind"]
        .as_str()
        .ok_or_else(|| format!("task '{id}' needs a string 'kind'"))?;
    let deps = match &t["deps"] {
        Value::Null => Vec::new(),
        Value::Array(a) => a
            .iter()
            .map(|d| {
                d.as_str()
                    .map(String::from)
                    .ok_or_else(|| format!("task '{id}': deps must be strings"))
            })
            .collect::<Result<_, _>>()?,
        _ => return Err(format!("task '{id}': deps must be an array")),
    };
    let state = match t["state"].as_str() {
        None => TaskState::NotRun,
        // a loaded document never claims results the session did not produce
        Some(s) => TaskState::parse(s)
            .map(|_| TaskState::NotRun)
            .ok_or_else(|| format!("task '{id}': unknown state '{s}'"))?,
    };
    Ok(Task {
        id: id.into(),
        title: t["title"].as_str().unwrap_or(id).into(),
        kind: kind.into(),
        deps,
        params: if t["params"].is_null() {
            json!({})
        } else {
            t["params"].clone()
        },
        state,
        message: String::new(),
        result: Value::Null,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_are_valid_and_ordered() {
        for (n, _) in TEMPLATES {
            let w = Workflow::template(n).unwrap();
            w.validate().unwrap();
        }
        let w = Workflow::template("channel_flow").unwrap();
        assert_eq!(
            w.plan("export_results", true).unwrap(),
            ["mesh_check", "setup", "initialize", "solve", "export_results"]
        );
        assert!(Workflow::template("nope").is_none());
    }

    #[test]
    fn plan_skips_done_prerequisites_and_refuses_without_them() {
        let mut w = Workflow::template("mesh_to_results").unwrap();
        assert!(w.plan("solve", false).is_err());
        w.finish("mesh_check", true, "ok".into(), Value::Null);
        w.finish("setup", true, "ok".into(), Value::Null);
        assert_eq!(w.plan("solve", true).unwrap(), ["initialize", "solve"]);
        w.finish("initialize", true, "ok".into(), Value::Null);
        assert_eq!(w.plan("solve", false).unwrap(), ["solve"]);
    }

    #[test]
    fn rerun_and_setting_changes_make_dependants_stale() {
        let mut w = Workflow::template("mesh_to_results").unwrap();
        for id in ["mesh_check", "setup", "initialize", "solve", "report"] {
            w.finish(id, true, String::new(), Value::Null);
        }
        assert!(w.summary()["complete"].as_bool().unwrap());
        w.finish("setup", true, String::new(), Value::Null);
        let st = |w: &Workflow, i: &str| w.task(i).unwrap().state;
        assert_eq!(st(&w, "mesh_check"), TaskState::Done);
        assert_eq!(st(&w, "initialize"), TaskState::Stale);
        assert_eq!(st(&w, "report"), TaskState::Stale);
        w.invalidate_all();
        assert_eq!(st(&w, "setup"), TaskState::Stale);
        assert_eq!(st(&w, "mesh_check"), TaskState::Stale);
        assert_eq!(w.summary()["next"], "mesh_check");
    }

    #[test]
    fn insert_delete_and_validation() {
        let mut w = Workflow::template("mesh_to_results").unwrap();
        let t = Task::new(
            "tweak",
            "Finer mesh",
            "set",
            &["mesh_check"],
            json!({"path": "mesh.nx", "value": 40}),
        );
        w.insert(t.clone(), Some("mesh_check")).unwrap();
        assert_eq!(w.tasks[1].id, "tweak");
        assert!(w.insert(t, None).unwrap_err().contains("duplicate"));
        assert!(w.delete("setup").unwrap_err().contains("initialize"));
        w.delete("tweak").unwrap();
        let bad = Task::new("x", "x", "solve", &["missing"], json!({}));
        assert!(w.insert(bad, None).unwrap_err().contains("unknown task"));
        let bad = Task::new("y", "y", "teleport", &[], json!({}));
        assert!(w.insert(bad, None).unwrap_err().contains("unknown kind"));
        let bad = Task::new("z", "z", "set", &[], json!({"path": "a"}));
        assert!(w.insert(bad, None).unwrap_err().contains("path and value"));
        assert_eq!(w.tasks.len(), 5, "failed inserts leave the workflow unchanged");
    }

    #[test]
    fn cycles_are_rejected() {
        let mut w = Workflow::template("mesh_to_results").unwrap();
        w.tasks[0].deps = vec!["report".into()];
        assert!(w.validate().unwrap_err().contains("cycle"));
    }

    #[test]
    fn document_round_trip_drops_run_state() {
        let mut w = Workflow::template("channel_flow").unwrap();
        w.finish("mesh_check", true, "ok".into(), json!({"cells": 4}));
        let doc = w.to_json();
        assert_eq!(doc["format"], FORMAT);
        let back = Workflow::from_json(&doc).unwrap();
        assert_eq!(back.tasks.len(), w.tasks.len());
        assert!(back.tasks.iter().all(|t| t.state == TaskState::NotRun));
        assert_eq!(back.tasks[5].id, "export_results");
        let mut bad = doc.clone();
        bad["version"] = json!(2);
        assert!(Workflow::from_json(&bad).is_err());
    }
}
