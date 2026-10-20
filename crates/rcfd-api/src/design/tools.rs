//! `design.*` service methods (ADR 0067): study definition, variants, operating maps, evaluation, recording, comparison and
//! proposal of candidates. The state of a study lives in its directory: `study.json` (definition) and `evaluations.jsonl`
//! (the store of [`super::store`]); every method is a function of the arguments and those files, so the methods are
//! stateless and survive a restart of the server.
//!
//! All paths go through [`Service::sandbox_path`]; names are restricted to `[A-Za-z0-9_.-]` without a leading dot.

use super::case_eval::{CaseEvaluator, CaseMapEvaluator, RunLimits};
use super::opmap::{self, Axis, MapOptions, MapPoint, OnFailure, PointEvaluator, PointResult};
use super::optim::Stop;
use super::store::{CondResult, Store};
use super::study::{
    Cmp, Condition, ConstraintDef, Doe, Evaluator, Method, ObjectiveDef, ParamDef, Proposal, Robust, Sense, Study,
    StudyDef, Target,
};
use crate::case::Case;
use crate::service::{code, Rpc, RpcError, Service};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// `format` member of `study.json`.
pub const STUDY_FORMAT: &str = "rcfd-design-study";

fn bad(msg: impl Into<String>) -> RpcError {
    RpcError::params(msg)
}

fn limit(msg: impl Into<String>) -> RpcError {
    RpcError::new(code::LIMIT, msg)
}

/// Whether `s` is an allowed name: `[A-Za-z0-9_.-]`, at least one character, no leading dot.
pub fn name_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with('.')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn check_name(what: &str, s: &str) -> Rpc<()> {
    if name_ok(s) {
        Ok(())
    } else {
        Err(bad(format!(
            "{what} '{s}': names use letters, digits, '_', '.' and '-' (at most 64) and do not start with a dot"
        )))
    }
}

fn hash_text(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn f64_of(v: &Value, what: &str) -> Rpc<f64> {
    match v.as_f64() {
        Some(x) if x.is_finite() => Ok(x),
        _ => Err(bad(format!("{what}: expected a finite number"))),
    }
}

fn str_arg<'a>(p: &'a Value, key: &str) -> Rpc<&'a str> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| bad(format!("missing string parameter '{key}'")))
}

fn opt_str<'a>(p: &'a Value, key: &str) -> Rpc<Option<&'a str>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(bad(format!("parameter '{key}' must be a string"))),
    }
}

fn opt_usize(p: &Value, key: &str) -> Rpc<Option<usize>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .map(|x| Some(x as usize))
            .ok_or_else(|| bad(format!("parameter '{key}' must be a non-negative integer"))),
    }
}

fn opt_bool(p: &Value, key: &str, default: bool) -> Rpc<bool> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_bool()
            .ok_or_else(|| bad(format!("parameter '{key}' must be true or false"))),
    }
}

fn write_text(path: &Path, text: &str) -> Rpc<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)
            .map_err(|e| RpcError::new(code::INTERNAL, format!("cannot create {}: {e}", d.display())))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|e| RpcError::new(code::INTERNAL, format!("cannot write {}: {e}", path.display())))
}

fn load_case(svc: &Service, path: &Path) -> Rpc<Case> {
    let case = Case::load_with(path, svc.config().root.as_deref()).map_err(|d| {
        bad(format!(
            "base case {}: {}",
            path.display(),
            d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        ))
    })?;
    Ok(case)
}

// ---------------------------------------------------------------- study definition

/// Settings of a study beyond [`StudyDef`].
#[derive(Debug, Clone)]
pub struct StudyMeta {
    /// Base case file.
    pub base_path: PathBuf,
    /// Fingerprint of the base case when the study was created.
    pub base_fingerprint: String,
    /// Sampler or optimiser.
    pub method: Method,
    /// Chain the conditions of a candidate.
    pub chain: bool,
    /// Step cap of one case run.
    pub max_steps: Option<usize>,
}

fn parse_robust(o: &Value, at: &str) -> Rpc<Robust> {
    let kind = o.get("robust").and_then(Value::as_str).unwrap_or("mean");
    Ok(match kind {
        "mean" => Robust::Mean,
        "worst" => Robust::Worst,
        "mean_plus_k_sigma" => Robust::MeanPlusKSigma(f64_of(o.get("k").unwrap_or(&Value::Null), &format!("{at}.k"))?),
        "weighted" => {
            let w = o.get("weights").and_then(Value::as_array).ok_or_else(|| {
                bad(format!(
                    "{at}.weights: a weighted objective needs one weight per condition"
                ))
            })?;
            Robust::Weighted(
                w.iter()
                    .enumerate()
                    .map(|(i, v)| f64_of(v, &format!("{at}.weights[{i}]")))
                    .collect::<Rpc<_>>()?,
            )
        }
        other => {
            return Err(bad(format!(
                "{at}.robust: '{other}' is not mean, worst, mean_plus_k_sigma or weighted"
            )))
        }
    })
}

fn parse_method(m: &Value) -> Rpc<Method> {
    let kind = m
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("method.type: missing"))?;
    let seed = m.get("seed").map_or(Ok(1), |v| {
        v.as_u64()
            .ok_or_else(|| bad("method.seed: expected a non-negative integer"))
    })?;
    let usize_of = |k: &str, default: usize| -> Rpc<usize> {
        match m.get(k) {
            None | Some(Value::Null) => Ok(default),
            Some(v) => v
                .as_u64()
                .map(|x| x as usize)
                .ok_or_else(|| bad(format!("method.{k}: expected a non-negative integer"))),
        }
    };
    Ok(match kind {
        "nelder_mead" => Method::NelderMead {
            budget: usize_of("budget", 100)?,
            restarts: usize_of("restarts", 0)?,
            seed,
        },
        "differential_evolution" => Method::Differential {
            budget: usize_of("budget", 500)?,
            pop: match m.get("pop") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_u64().ok_or_else(|| bad("method.pop: expected a non-negative integer"))? as usize),
            },
            seed,
        },
        "factorial" => Method::Doe(Doe::Factorial {
            levels: m
                .get("levels")
                .and_then(Value::as_array)
                .ok_or_else(|| bad("method.levels: a factorial needs one level count per parameter"))?
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    v.as_u64()
                        .filter(|x| *x >= 1)
                        .map(|x| x as usize)
                        .ok_or_else(|| bad(format!("method.levels[{i}]: expected an integer >= 1")))
                })
                .collect::<Rpc<_>>()?,
        }),
        "lhs" => Method::Doe(Doe::Lhs {
            n: usize_of("n", 0)?,
            seed,
        }),
        "sobol" => Method::Doe(Doe::Sobol { n: usize_of("n", 0)? }),
        other => {
            return Err(bad(format!(
                "method.type: '{other}' is not nelder_mead, differential_evolution, factorial, lhs or sobol (CMA-ES is not available)"
            )))
        }
    })
}

fn method_json(m: &Method) -> Value {
    match m {
        Method::NelderMead { budget, restarts, seed } => {
            json!({"type": "nelder_mead", "budget": budget, "restarts": restarts, "seed": seed})
        }
        Method::Differential { budget, pop, seed } => {
            json!({"type": "differential_evolution", "budget": budget, "pop": pop, "seed": seed})
        }
        Method::Doe(Doe::Factorial { levels }) => json!({"type": "factorial", "levels": levels}),
        Method::Doe(Doe::Lhs { n, seed }) => json!({"type": "lhs", "n": n, "seed": seed}),
        Method::Doe(Doe::Sobol { n }) => json!({"type": "sobol", "n": n}),
    }
}

/// Parse the study definition (`study.json` contents or the arguments of `design.create_study`).
pub fn parse_study(v: &Value, base_path: PathBuf, base_fingerprint: String) -> Rpc<(StudyDef, StudyMeta)> {
    let name = v.get("name").and_then(Value::as_str).unwrap_or("study").to_string();
    check_name("study name", &name)?;
    let params = v
        .get("parameters")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("parameters: a study needs a list of parameters"))?;
    let mut pdefs = Vec::new();
    for (i, p) in params.iter().enumerate() {
        let at = format!("parameters[{i}]");
        let pname = p
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| bad(format!("{at}.name: missing")))?;
        check_name(&format!("{at}.name"), pname)?;
        let mut targets = Vec::new();
        if let Some(path) = p.get("path").and_then(Value::as_str) {
            targets.push(Target {
                path: path.to_string(),
                scale: 1.0,
                offset: 0.0,
            });
        }
        if let Some(ts) = p.get("targets").and_then(Value::as_array) {
            for (k, t) in ts.iter().enumerate() {
                let path = t
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad(format!("{at}.targets[{k}].path: missing")))?;
                targets.push(Target {
                    path: path.to_string(),
                    scale: t
                        .get("scale")
                        .map_or(Ok(1.0), |x| f64_of(x, &format!("{at}.targets[{k}].scale")))?,
                    offset: t
                        .get("offset")
                        .map_or(Ok(0.0), |x| f64_of(x, &format!("{at}.targets[{k}].offset")))?,
                });
            }
        }
        if targets.is_empty() {
            return Err(bad(format!(
                "parameter '{pname}': give 'path' or 'targets' (the settings path the value is written to)"
            )));
        }
        pdefs.push(ParamDef {
            name: pname.to_string(),
            lo: f64_of(p.get("lo").unwrap_or(&Value::Null), &format!("parameter '{pname}'.lo"))?,
            hi: f64_of(p.get("hi").unwrap_or(&Value::Null), &format!("parameter '{pname}'.hi"))?,
            init: match p.get("init") {
                None | Some(Value::Null) => None,
                Some(x) => Some(f64_of(x, &format!("parameter '{pname}'.init"))?),
            },
            targets,
        });
    }
    let conditions: Vec<Condition> = match v.get("conditions") {
        None | Some(Value::Null) => vec![Condition {
            label: "base".into(),
            set: vec![],
        }],
        Some(c) => {
            let list = c.as_array().ok_or_else(|| bad("conditions: expected a list"))?;
            let mut out = Vec::new();
            for (i, c) in list.iter().enumerate() {
                let label = c
                    .get("label")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad(format!("conditions[{i}].label: missing")))?;
                check_name(&format!("conditions[{i}].label"), label)?;
                let set = match c.get("set") {
                    None | Some(Value::Null) => vec![],
                    Some(Value::Object(o)) => o.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                    Some(_) => {
                        return Err(bad(format!(
                            "conditions[{i}].set: expected an object of settings paths"
                        )))
                    }
                };
                out.push(Condition {
                    label: label.to_string(),
                    set,
                });
            }
            out
        }
    };
    let mut objectives = Vec::new();
    for (i, o) in v
        .get("objectives")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("objectives: a study needs at least one objective"))?
        .iter()
        .enumerate()
    {
        let at = format!("objectives[{i}]");
        objectives.push(ObjectiveDef {
            metric: o
                .get("metric")
                .and_then(Value::as_str)
                .ok_or_else(|| bad(format!("{at}.metric: missing")))?
                .to_string(),
            sense: match o.get("sense").and_then(Value::as_str).unwrap_or("min") {
                "min" => Sense::Min,
                "max" => Sense::Max,
                other => return Err(bad(format!("{at}.sense: '{other}' is not min or max"))),
            },
            weight: o
                .get("weight")
                .map_or(Ok(1.0), |x| f64_of(x, &format!("{at}.weight")))?,
            robust: parse_robust(o, &at)?,
        });
    }
    let mut constraints = Vec::new();
    if let Some(cs) = v.get("constraints").and_then(Value::as_array) {
        for (i, c) in cs.iter().enumerate() {
            let at = format!("constraints[{i}]");
            let limit = f64_of(c.get("limit").unwrap_or(&Value::Null), &format!("{at}.limit"))?;
            constraints.push(ConstraintDef {
                metric: c
                    .get("metric")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad(format!("{at}.metric: missing")))?
                    .to_string(),
                cmp: match c.get("cmp").and_then(Value::as_str).unwrap_or("<=") {
                    "<=" => Cmp::Le,
                    ">=" => Cmp::Ge,
                    other => return Err(bad(format!("{at}.cmp: '{other}' is not <= or >="))),
                },
                limit,
                scale: c
                    .get("scale")
                    .map_or(Ok(limit.abs().max(1.0)), |x| f64_of(x, &format!("{at}.scale")))?,
            });
        }
    }
    let method = parse_method(
        v.get("method")
            .ok_or_else(|| bad("method: missing (nelder_mead, differential_evolution, factorial, lhs or sobol)"))?,
    )?;
    let workers = match v.get("workers") {
        None | Some(Value::Null) => 1,
        Some(x) => x
            .as_u64()
            .filter(|w| *w >= 1)
            .ok_or_else(|| bad("workers: expected an integer >= 1"))? as usize,
    };
    let def = StudyDef {
        name,
        params: pdefs,
        conditions,
        objectives,
        constraints,
        workers,
    };
    def.validate().map_err(bad)?;
    let max_steps = opt_usize(v, "max_steps")?;
    let chain = opt_bool(v, "chain", true)?;
    Ok((
        def,
        StudyMeta {
            base_path,
            base_fingerprint,
            method,
            chain,
            max_steps,
        },
    ))
}

fn study_json(def: &StudyDef, meta: &StudyMeta, base_name: &str) -> Value {
    json!({
        "format": STUDY_FORMAT, "version": 1, "name": def.name,
        "base": {"path": base_name, "fingerprint": meta.base_fingerprint},
        "parameters": def.params.iter().map(|p| json!({
            "name": p.name, "lo": p.lo, "hi": p.hi, "init": p.init,
            "targets": p.targets.iter().map(|t| json!({"path": t.path, "scale": t.scale, "offset": t.offset})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "conditions": def.conditions.iter().map(|c| json!({
            "label": c.label, "set": c.set.iter().cloned().collect::<Map<String, Value>>(),
        })).collect::<Vec<_>>(),
        "objectives": def.objectives.iter().map(|o| {
            let mut j = json!({"metric": o.metric, "sense": match o.sense { Sense::Min => "min", Sense::Max => "max" }, "weight": o.weight});
            match &o.robust {
                Robust::Mean => j["robust"] = json!("mean"),
                Robust::Worst => j["robust"] = json!("worst"),
                Robust::MeanPlusKSigma(k) => { j["robust"] = json!("mean_plus_k_sigma"); j["k"] = json!(k); }
                Robust::Weighted(w) => { j["robust"] = json!("weighted"); j["weights"] = json!(w); }
            }
            j
        }).collect::<Vec<_>>(),
        "constraints": def.constraints.iter().map(|c| json!({
            "metric": c.metric, "cmp": match c.cmp { Cmp::Le => "<=", Cmp::Ge => ">=" }, "limit": c.limit, "scale": c.scale,
        })).collect::<Vec<_>>(),
        "method": method_json(&meta.method), "workers": def.workers, "chain": meta.chain, "max_steps": meta.max_steps,
    })
}

/// Signature of what determines the metrics of a candidate (parameters with targets, conditions, base case).
fn signature(def: &StudyDef, meta: &StudyMeta) -> String {
    let j = study_json(def, meta, "");
    let key = json!({
        "base": meta.base_fingerprint, "parameters": j["parameters"], "conditions": j["conditions"],
        "chain": meta.chain, "max_steps": meta.max_steps,
    });
    hash_text(&key.to_string())
}

struct Loaded {
    dir: PathBuf,
    def: StudyDef,
    meta: StudyMeta,
    base: Case,
    sig: String,
}

fn load_study(svc: &Service, p: &Value) -> Rpc<Loaded> {
    let dir = svc.sandbox_path(str_arg(p, "dir")?)?;
    let file = dir.join("study.json");
    let text = std::fs::read_to_string(&file).map_err(|e| {
        bad(format!(
            "dir: no study at {} ({e}); create one with design.create_study",
            dir.display()
        ))
    })?;
    let v: Value = serde_json::from_str(&text).map_err(|e| bad(format!("{}: not JSON: {e}", file.display())))?;
    if v.get("format").and_then(Value::as_str) != Some(STUDY_FORMAT) {
        return Err(bad(format!("{} is not a design study", file.display())));
    }
    let base_rel = v["base"]["path"]
        .as_str()
        .ok_or_else(|| bad("study.json: base.path missing"))?;
    let base_path = if Path::new(base_rel).is_absolute() {
        PathBuf::from(base_rel)
    } else {
        dir.join(base_rel)
    };
    let base_path = svc.sandbox_path(&base_path.to_string_lossy())?;
    let fp = v["base"]["fingerprint"].as_str().unwrap_or("").to_string();
    let (def, meta) = parse_study(&v, base_path.clone(), fp.clone())?;
    let base = load_case(svc, &base_path)?;
    let now = crate::study::fingerprint(&base);
    if now != fp {
        return Err(bad(format!(
            "base case {} changed since the study was created (fingerprint {now}, study {fp}); create a new study",
            base_path.display()
        )));
    }
    let sig = signature(&def, &meta);
    Ok(Loaded {
        dir,
        def,
        meta,
        base,
        sig,
    })
}

impl Loaded {
    fn store(&self, repair: bool) -> Rpc<Store> {
        let path = self.dir.join("evaluations.jsonl");
        let r = if repair {
            Store::open(&path, &self.sig)
        } else {
            Store::open_read(&path, &self.sig)
        };
        r.map_err(bad)
    }

    fn limits(&self, svc: &Service) -> RunLimits {
        let cfg = svc.config();
        RunLimits {
            max_cells: cfg.max_cells,
            max_steps: Some(
                self.meta
                    .max_steps
                    .unwrap_or(cfg.max_steps_per_call)
                    .min(cfg.max_steps_per_call),
            ),
        }
    }

    fn evaluator(&self, svc: &Service) -> CaseEvaluator {
        CaseEvaluator::new(self.base.clone(), self.limits(svc), self.meta.chain)
    }

    fn x_from(&self, p: &Value) -> Rpc<Vec<f64>> {
        let params = p
            .get("params")
            .and_then(Value::as_object)
            .ok_or_else(|| bad("params: expected an object {parameter name: value}"))?;
        for k in params.keys() {
            if !self.def.params.iter().any(|q| q.name == *k) {
                return Err(bad(format!("params: unknown parameter '{k}'")));
            }
        }
        self.def.vector(&|n| params.get(n).and_then(Value::as_f64)).map_err(bad)
    }
}

// ---------------------------------------------------------------- record output

fn params_obj(def: &StudyDef, x: &[f64]) -> Value {
    Value::Object(
        def.params
            .iter()
            .zip(x)
            .map(|(p, v)| (p.name.clone(), json!(v)))
            .collect(),
    )
}

fn record_json(study: &Study<'_>, pos: usize) -> Value {
    let rec = &study.store.records()[pos];
    let fit = study.fitness_of(rec);
    json!({
        "index": rec.index,
        "params": params_obj(&study.def, &study.x_of(rec)),
        "source": rec.source, "ok": rec.ok(),
        "conditions": rec.conditions.iter().map(CondResult::to_json).collect::<Vec<_>>(),
        "objectives": study.objective_vector(rec),
        "f": if fit.f.is_finite() { json!(fit.f) } else { Value::Null },
        "violation": if fit.violation.is_finite() { json!(fit.violation) } else { Value::Null },
        "feasible": rec.ok() && fit.feasible(),
        "message": rec.message,
    })
}

struct NoEval;
impl Evaluator for NoEval {
    fn evaluate(&self, def: &StudyDef, _x: &[f64]) -> Vec<CondResult> {
        def.conditions
            .iter()
            .map(|c| CondResult::failure(&c.label, "failed", "no evaluator in this call"))
            .collect()
    }
}

fn summary(study: &Study<'_>) -> Value {
    let recs = study.store.records();
    let usable = recs.iter().filter(|r| r.ok()).count();
    let best = study.best();
    json!({
        "name": study.def.name,
        "evaluations": recs.len(), "usable": usable, "failed": recs.len() - usable,
        "best": best.map(|i| record_json(study, i)),
        "pareto": study.pareto().iter().map(|&i| recs[i].index).collect::<Vec<_>>(),
    })
}

fn stop_error(e: Stop) -> RpcError {
    match e {
        Stop::Error(m) => bad(m),
        Stop::Interrupted => RpcError::new(code::INTERNAL, "interrupted"),
        Stop::Missing(_) => RpcError::new(code::INTERNAL, "internal error: missing evaluation"),
    }
}

// ---------------------------------------------------------------- methods

/// Dispatch a `design.*` method.
pub fn call(svc: &Arc<Service>, method: &str, p: &Value) -> Rpc {
    match method {
        "design.create_study" => create_study(svc, p),
        "design.create_variant" => create_variant(svc, p),
        "design.run_map" => run_map(svc, p),
        "design.run_study" => run_study(svc, p),
        "design.evaluate" => evaluate(svc, p),
        "design.record" => record(svc, p),
        "design.get_metrics" => get_metrics(svc, p),
        "design.compare" => compare(svc, p),
        "design.propose" => propose(svc, p),
        "design.list_study" => list_study(svc, p),
        other => Err(RpcError::new(
            code::METHOD_NOT_FOUND,
            format!("unknown method '{other}'"),
        )),
    }
}

fn create_study(svc: &Service, p: &Value) -> Rpc {
    let dir = svc.sandbox_path(str_arg(p, "dir")?)?;
    // base case: a file, or the settings of a session (copied into the study directory)
    let (base_path, base_name, base) = if let Some(b) = opt_str(p, "base")? {
        let bp = svc.sandbox_path(b)?;
        let case = load_case(svc, &bp)?;
        // stored absolute: a relative name would later be read against the study directory (ADR 0089 F1)
        let abs = std::env::current_dir()
            .map(|d| d.join(&bp))
            .unwrap_or_else(|_| bp.clone());
        (bp, abs.to_string_lossy().to_string(), case)
    } else if p.get("session").is_some() {
        let case = svc.session_case(p)?;
        let bp = dir.join("base.toml");
        (bp, "base.toml".to_string(), case)
    } else {
        return Err(bad("give 'base' (a case file inside the sandbox) or 'session'"));
    };
    let fp = crate::study::fingerprint(&base);
    let (def, meta) = parse_study(p, base_path.clone(), fp)?;
    // dry-run the parameter targets and conditions on the base case so that bad paths are found now
    let ev = CaseEvaluator::new(base.clone(), RunLimits::default(), true);
    let mid: Vec<f64> = def
        .params
        .iter()
        .map(|q| q.init.unwrap_or(0.5 * (q.lo + q.hi)))
        .collect();
    for c in &def.conditions {
        ev.build_case(&def, &mid, Some(c))
            .map_err(|e| bad(format!("study definition: {e}")))?;
    }
    if let Method::Doe(d) = &meta.method {
        let tmp = NoEval;
        let st = Study::new(def.clone(), Store::memory("x"), &tmp).map_err(bad)?;
        st.doe_points(d).map_err(|e| bad(format!("method: {e}")))?;
    }
    if dir.join("study.json").exists() && !opt_bool(p, "overwrite", false)? {
        return Err(bad(format!(
            "{} already has a study; pass overwrite = true to replace its definition (the stored evaluations are kept only when the signature matches)",
            dir.display()
        )));
    }
    if base_name == "base.toml" {
        write_text(&base_path, &base.to_toml())?;
    }
    let text = serde_json::to_string_pretty(&study_json(&def, &meta, &base_name)).unwrap_or_default() + "\n";
    write_text(&dir.join("study.json"), &text)?;
    Ok(json!({
        "dir": dir.display().to_string(), "study": dir.join("study.json").display().to_string(),
        "name": def.name, "parameters": def.params.len(), "conditions": def.conditions.len(),
        "signature": signature(&def, &meta), "base_fingerprint": meta.base_fingerprint,
    }))
}

fn json_to_pairs(v: &Value, what: &str) -> Rpc<Vec<(String, Value)>> {
    match v {
        Value::Null => Ok(vec![]),
        Value::Object(o) => Ok(o.iter().map(|(k, x)| (k.clone(), x.clone())).collect()),
        _ => Err(bad(format!("{what}: expected an object {{settings path: value}}"))),
    }
}

fn create_variant(svc: &Service, p: &Value) -> Rpc {
    let out = svc.sandbox_path(str_arg(p, "path")?)?;
    if let Some(n) = opt_str(p, "name")? {
        check_name("name", n)?;
    }
    let overrides = json_to_pairs(p.get("overrides").unwrap_or(&Value::Null), "overrides")?;
    let mut case;
    let mut applied = 0usize;
    if p.get("dir").is_some() {
        let l = load_study(svc, p)?;
        let x = if p.get("params").is_some() {
            l.x_from(p)?
        } else {
            l.def
                .params
                .iter()
                .map(|q| q.init.unwrap_or(0.5 * (q.lo + q.hi)))
                .collect()
        };
        let cond = match opt_str(p, "condition")? {
            Some(label) => Some(
                l.def
                    .conditions
                    .iter()
                    .find(|c| c.label == label)
                    .ok_or_else(|| bad(format!("condition '{label}': not in the study")))?,
            ),
            None => None,
        };
        case = l.evaluator(svc).build_case(&l.def, &x, cond).map_err(bad)?;
        applied += l.def.params.iter().map(|q| q.targets.len()).sum::<usize>() + cond.map_or(0, |c| c.set.len());
    } else {
        let base = svc.sandbox_path(str_arg(p, "base")?)?;
        case = load_case(svc, &base)?;
    }
    for (path, v) in &overrides {
        crate::study::apply_value(&mut case, path, v).map_err(|e| bad(format!("overrides: {path}: {e}")))?;
        applied += 1;
    }
    let sem = case.semantic_errors();
    if !sem.is_empty() {
        return Err(RpcError::validation(sem));
    }
    if out.exists() && !opt_bool(p, "overwrite", false)? {
        return Err(bad(format!(
            "{} exists; pass overwrite = true to replace it",
            out.display()
        )));
    }
    if let Some(d) = out.parent() {
        std::fs::create_dir_all(d)
            .map_err(|e| RpcError::new(code::INTERNAL, format!("cannot create {}: {e}", d.display())))?;
    }
    case.save(&out)
        .map_err(|e| RpcError::new(code::INTERNAL, format!("cannot write {}: {e}", out.display())))?;
    Ok(
        json!({"path": out.display().to_string(), "fingerprint": crate::study::fingerprint(&case), "settings_applied": applied}),
    )
}

/// Counts case runs of a map evaluator.
struct Counting<'a> {
    inner: CaseMapEvaluator,
    runs: &'a std::sync::atomic::AtomicUsize,
}

impl PointEvaluator for Counting<'_> {
    type State = super::case_eval::Fields;
    fn run(&self, point: &MapPoint, warm: Option<&Self::State>) -> PointResult<Self::State> {
        self.runs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.run(point, warm)
    }
}

fn parse_axes(p: &Value) -> Rpc<Vec<MapPoint>> {
    if let Some(pts) = p.get("points") {
        let list = pts
            .as_array()
            .ok_or_else(|| bad("points: expected a list of {settings path: value} objects"))?;
        let sets = list
            .iter()
            .enumerate()
            .map(|(i, o)| json_to_pairs(o, &format!("points[{i}]")))
            .collect::<Rpc<Vec<_>>>()?;
        return opmap::list_points(sets).map_err(bad);
    }
    let axes = p
        .get("axes")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("give 'axes' (a list of {path, values} or {path, start, stop, count}) or 'points'"))?;
    let mut out = Vec::new();
    for (i, a) in axes.iter().enumerate() {
        let path = a
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| bad(format!("axes[{i}].path: missing")))?;
        let values: Vec<Value> = if let Some(v) = a.get("values").and_then(Value::as_array) {
            v.clone()
        } else if a.get("start").is_some() {
            let start = f64_of(&a["start"], &format!("axes[{i}].start"))?;
            let stop = f64_of(a.get("stop").unwrap_or(&Value::Null), &format!("axes[{i}].stop"))?;
            let n = a
                .get("count")
                .and_then(Value::as_u64)
                .filter(|n| *n >= 1)
                .ok_or_else(|| bad(format!("axes[{i}].count: expected an integer >= 1")))? as usize;
            if n > opmap::MAX_MAP_POINTS {
                return Err(bad(format!("axes[{i}].count: too large")));
            }
            (0..n)
                .map(|k| {
                    json!(if n == 1 {
                        start
                    } else if k + 1 == n {
                        stop
                    } else {
                        start + (stop - start) * k as f64 / (n - 1) as f64
                    })
                })
                .collect()
        } else {
            return Err(bad(format!(
                "axis '{path}': give 'values' or 'start', 'stop' and 'count'"
            )));
        };
        out.push(Axis {
            path: path.to_string(),
            values,
        });
    }
    opmap::grid_points(&out, opt_bool(p, "serpentine", false)?).map_err(bad)
}

fn run_map(svc: &Service, p: &Value) -> Rpc {
    let out_dir = svc.sandbox_path(str_arg(p, "out_dir")?)?;
    let points = parse_axes(p)?;
    let cap = svc.config().max_design_evaluations;
    if points.len() > cap {
        return Err(limit(format!(
            "the map has {} points, the limit max_design_evaluations is {cap}",
            points.len()
        )));
    }
    let (mut base, base_desc) = if p.get("dir").is_some() {
        let l = load_study(svc, p)?;
        let x = if p.get("params").is_some() {
            l.x_from(p)?
        } else {
            l.def
                .params
                .iter()
                .map(|q| q.init.unwrap_or(0.5 * (q.lo + q.hi)))
                .collect()
        };
        let cond = match opt_str(p, "condition")? {
            Some(label) => Some(
                l.def
                    .conditions
                    .iter()
                    .find(|c| c.label == label)
                    .ok_or_else(|| bad(format!("condition '{label}': not in the study")))?,
            ),
            None => None,
        };
        let c = l.evaluator(svc).build_case(&l.def, &x, cond).map_err(bad)?;
        (c, format!("study {} {}", l.dir.display(), params_obj(&l.def, &x)))
    } else {
        let b = svc.sandbox_path(str_arg(p, "base")?)?;
        (load_case(svc, &b)?, b.display().to_string())
    };
    if let Some(ov) = p.get("overrides") {
        for (path, v) in json_to_pairs(ov, "overrides")? {
            crate::study::apply_value(&mut base, &path, &v).map_err(|e| bad(format!("overrides: {path}: {e}")))?;
        }
    }
    let cfg = svc.config();
    let max_steps = opt_usize(p, "max_steps")?
        .unwrap_or(cfg.max_steps_per_call)
        .min(cfg.max_steps_per_call);
    let opts = MapOptions {
        on_failure: match opt_str(p, "on_failure")?.unwrap_or("continue") {
            "continue" => OnFailure::Continue,
            "stop" => OnFailure::Stop,
            other => return Err(bad(format!("on_failure: '{other}' is not continue or stop"))),
        },
        retry_cold: opt_bool(p, "retry_cold", true)?,
        cold: opt_bool(p, "cold", false)?,
        out_dir: Some(out_dir.clone()),
    };
    let sig = hash_text(
        &json!({
            "base": crate::study::fingerprint(&base),
            "points": points.iter().map(|q| json!({"set": q.set.iter().cloned().collect::<Map<String, Value>>(), "label": q.label})).collect::<Vec<_>>(),
            "on_failure": format!("{:?}", opts.on_failure), "retry_cold": opts.retry_cold, "cold": opts.cold, "max_steps": max_steps,
        })
        .to_string(),
    );
    let table_path = out_dir.join("map.json");
    let meta_path = out_dir.join("map.meta.json");
    if let (Ok(meta), Ok(table)) = (
        std::fs::read_to_string(&meta_path),
        std::fs::read_to_string(&table_path),
    ) {
        let same = serde_json::from_str::<Value>(&meta)
            .ok()
            .is_some_and(|m| m["signature"] == json!(sig));
        if same {
            if let Ok(t) = serde_json::from_str::<Value>(&table) {
                if t["state"] != json!("running") {
                    return Ok(json!({
                        "cached": true, "evaluations_run": 0, "state": t["state"], "planned": points.len(),
                        "points": t["points"], "csv": out_dir.join("map.csv").display().to_string(),
                        "json": table_path.display().to_string(), "base": base_desc,
                    }));
                }
            }
        }
    }
    let runs = std::sync::atomic::AtomicUsize::new(0);
    let ev = Counting {
        inner: CaseMapEvaluator {
            base,
            limits: RunLimits {
                max_cells: cfg.max_cells,
                max_steps: Some(max_steps),
            },
        },
        runs: &runs,
    };
    let outcome = opmap::run_and_write(&points, &ev, &opts).map_err(|e| RpcError::new(code::INTERNAL, e))?;
    write_text(&meta_path, &json!({"signature": sig}).to_string())?;
    Ok(json!({
        "cached": false, "evaluations_run": runs.load(std::sync::atomic::Ordering::SeqCst), "state": outcome.state,
        "planned": outcome.planned, "converged": outcome.converged(),
        "points": outcome.rows.iter().map(opmap::row_json).collect::<Vec<_>>(),
        "csv": out_dir.join("map.csv").display().to_string(), "json": table_path.display().to_string(), "base": base_desc,
    }))
}

fn run_study(svc: &Service, p: &Value) -> Rpc {
    let l = load_study(svc, p)?;
    let cap = svc.config().max_design_evaluations;
    let allowance = opt_usize(p, "max_evaluations")?.unwrap_or(cap).min(cap);
    let ev = l.evaluator(svc);
    let store = l.store(true)?;
    let mut study = Study::new(l.def.clone(), store, &ev).map_err(bad)?;
    study.allowance = Some(allowance);
    let state = match study.run(&l.meta.method) {
        Ok(_) => "complete",
        Err(Stop::Interrupted) => "interrupted",
        Err(e) => return Err(stop_error(e)),
    };
    let mut out = summary(&study);
    out["state"] = json!(state);
    out["evaluations_run"] = json!(study.evaluated);
    out["limit"] = json!(allowance);
    Ok(out)
}

fn evaluate(svc: &Service, p: &Value) -> Rpc {
    let l = load_study(svc, p)?;
    let x = l.x_from(p)?;
    let ev = l.evaluator(svc);
    let store = l.store(true)?;
    let mut study = Study::new(l.def.clone(), store, &ev).map_err(bad)?;
    let cap = svc.config().max_design_evaluations;
    study.allowance = Some(cap);
    let pos = study.eval_points(&[x], "evaluated").map_err(stop_error)?;
    let mut out = record_json(&study, pos[0]);
    out["evaluations_run"] = json!(study.evaluated);
    out["cached"] = json!(study.evaluated == 0);
    Ok(out)
}

fn record(svc: &Service, p: &Value) -> Rpc {
    let l = load_study(svc, p)?;
    let x = l.x_from(p)?;
    let mut conditions = Vec::new();
    let one = |label: &str, o: &Value, at: &str| -> Rpc<CondResult> {
        let m = o
            .get("metrics")
            .and_then(Value::as_object)
            .ok_or_else(|| bad(format!("{at}.metrics: expected an object {{metric name: number}}")))?;
        let mut metrics = BTreeMap::new();
        for (k, v) in m {
            if k.is_empty() {
                return Err(bad(format!("{at}.metrics: empty metric name")));
            }
            let x = v
                .as_f64()
                .filter(|x| x.is_finite())
                .ok_or_else(|| bad(format!("{at}.metrics.{k}: expected a finite number")))?;
            metrics.insert(k.clone(), x);
        }
        let ok = opt_bool(o, "ok", true)?;
        Ok(if ok {
            let mut r = CondResult::success(label, metrics);
            r.status = "recorded".into();
            r
        } else {
            CondResult::failure(
                label,
                "failed",
                o.get("message").and_then(Value::as_str).unwrap_or("recorded as failed"),
            )
        })
    };
    if let Some(cs) = p.get("conditions").and_then(Value::as_array) {
        if cs.len() != l.def.conditions.len() {
            return Err(bad(format!(
                "conditions: {} results for {} study conditions",
                cs.len(),
                l.def.conditions.len()
            )));
        }
        for (i, (c, def)) in cs.iter().zip(&l.def.conditions).enumerate() {
            if let Some(lab) = c.get("label").and_then(Value::as_str) {
                if lab != def.label {
                    return Err(bad(format!(
                        "conditions[{i}].label: expected '{}', got '{lab}'",
                        def.label
                    )));
                }
            }
            conditions.push(one(&def.label, c, &format!("conditions[{i}]"))?);
        }
    } else if l.def.conditions.len() == 1 {
        conditions.push(one(&l.def.conditions[0].label, p, "arguments")?);
    } else {
        return Err(bad(format!(
            "conditions: the study has {} operating conditions; give 'conditions' with one {{label, metrics}} each",
            l.def.conditions.len()
        )));
    }
    let ev = NoEval;
    let store = l.store(true)?;
    let mut study = Study::new(l.def.clone(), store, &ev).map_err(bad)?;
    let note = opt_str(p, "note")?.unwrap_or("");
    let idx = study.record(&x, conditions, note).map_err(bad)?;
    let mut out = record_json(&study, idx);
    out["recorded"] = json!(true);
    Ok(out)
}

fn find_record(study: &Study<'_>, l: &Loaded, p: &Value) -> Rpc<usize> {
    if let Some(i) = opt_usize(p, "index")? {
        return if i < study.store.records().len() {
            Ok(i)
        } else {
            Err(bad(format!(
                "index {i}: the study has {} records",
                study.store.records().len()
            )))
        };
    }
    let x = l.x_from(p)?;
    let key = super::store::key_of(&x);
    study
        .store
        .records()
        .iter()
        .position(|r| r.key == key)
        .ok_or_else(|| bad("params: no stored evaluation for these exact parameter values"))
}

fn get_metrics(svc: &Service, p: &Value) -> Rpc {
    if let Some(m) = opt_str(p, "map_dir")? {
        let dir = svc.sandbox_path(m)?;
        let file = dir.join("map.json");
        let text =
            std::fs::read_to_string(&file).map_err(|e| bad(format!("map_dir: cannot read {}: {e}", file.display())))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| bad(format!("{}: not JSON: {e}", file.display())))?;
        return Ok(v);
    }
    let l = load_study(svc, p)?;
    let ev = NoEval;
    let store = l.store(false)?;
    let study = Study::new(l.def.clone(), store, &ev).map_err(bad)?;
    let pos = find_record(&study, &l, p)?;
    Ok(record_json(&study, pos))
}

fn compare(svc: &Service, p: &Value) -> Rpc {
    let l = load_study(svc, p)?;
    let ev = NoEval;
    let store = l.store(false)?;
    let study = Study::new(l.def.clone(), store, &ev).map_err(bad)?;
    let mut positions = Vec::new();
    if let Some(idx) = p.get("indices").and_then(Value::as_array) {
        for (k, v) in idx.iter().enumerate() {
            let i = v
                .as_u64()
                .ok_or_else(|| bad(format!("indices[{k}]: expected a non-negative integer")))?
                as usize;
            if i >= study.store.records().len() {
                return Err(bad(format!("indices[{k}]: record {i} does not exist")));
            }
            positions.push(i);
        }
    } else if let Some(list) = p.get("candidates").and_then(Value::as_array) {
        for (k, c) in list.iter().enumerate() {
            let q = json!({"dir": p["dir"], "params": c});
            positions.push(find_record(&study, &l, &q).map_err(|e| bad(format!("candidates[{k}]: {}", e.message)))?);
        }
    } else {
        return Err(bad(
            "give 'indices' (record numbers) or 'candidates' (parameter objects)",
        ));
    }
    if positions.len() < 2 {
        return Err(bad("compare at least two records"));
    }
    let recs = study.store.records();
    let metric_names: Vec<String> = {
        let mut s = std::collections::BTreeSet::new();
        for &i in &positions {
            for c in &recs[i].conditions {
                s.extend(c.metrics.keys().cloned());
            }
        }
        s.into_iter().collect()
    };
    let mut rows = Vec::new();
    for &i in &positions {
        rows.push(record_json(&study, i));
    }
    // exact differences of every metric of every condition relative to the first record
    let first = &recs[positions[0]];
    let mut diffs = Vec::new();
    for &i in &positions[1..] {
        let mut d = Map::new();
        for (c0, c1) in first.conditions.iter().zip(&recs[i].conditions) {
            for m in &metric_names {
                if let (Some(a), Some(b)) = (c0.metrics.get(m), c1.metrics.get(m)) {
                    d.insert(format!("{}.{m}", c0.label), json!(b - a));
                }
            }
        }
        diffs.push(json!({"index": recs[i].index, "minus_first": d}));
    }
    // ranking by the scalar objective with the feasibility rules
    let mut order: Vec<usize> = (0..positions.len()).collect();
    let fits: Vec<_> = positions.iter().map(|&i| study.fitness_of(&recs[i])).collect();
    order.sort_by(|&a, &b| {
        if fits[a].better(&fits[b]) {
            std::cmp::Ordering::Less
        } else if fits[b].better(&fits[a]) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
    Ok(json!({
        "records": rows, "differences": diffs,
        "ranking": order.iter().map(|&k| recs[positions[k]].index).collect::<Vec<_>>(),
        "metrics": metric_names,
    }))
}

fn propose(svc: &Service, p: &Value) -> Rpc {
    let l = load_study(svc, p)?;
    let ev = NoEval;
    let store = l.store(false)?;
    let mut study = Study::new(l.def.clone(), store, &ev).map_err(bad)?;
    let out = match study.propose(&l.meta.method).map_err(bad)? {
        Proposal::Complete => json!({"complete": true, "candidates": [], "evaluations": study.store.records().len()}),
        Proposal::Next(points) => json!({
            "complete": false,
            "candidates": points.iter().map(|x| params_obj(&l.def, x)).collect::<Vec<_>>(),
            "evaluations": study.store.records().len(),
            "method": method_json(&l.meta.method),
        }),
    };
    Ok(out)
}

fn list_study(svc: &Service, p: &Value) -> Rpc {
    let l = load_study(svc, p)?;
    let ev = NoEval;
    let store = l.store(false)?;
    let study = Study::new(l.def.clone(), store, &ev).map_err(bad)?;
    let mut out = summary(&study);
    let text = std::fs::read_to_string(l.dir.join("study.json")).unwrap_or_default();
    out["definition"] = serde_json::from_str(&text).unwrap_or(Value::Null);
    out["signature"] = json!(l.sig);
    Ok(out)
}
