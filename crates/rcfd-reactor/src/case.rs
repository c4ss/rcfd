//! JSON case files for `rcfd reactor` (documented in docs/case-schema.md, section "Reactor cases"). Every problem is
//! reported with its JSON path, for example `reactor.temperature: must be a positive number (got -5)`.

use crate::equil::{equilibrium, Constraint};
use crate::mech::{Mechanism, R_GAS};
use crate::reactors::{
    ignition_delay_max_dtdt, psr_blowout, psr_steady, run_batch, run_plug_flow, BatchKind, Energy, Psr, Stop, Tol,
    Trajectory,
};
use serde_json::{json, Map, Value};
use std::path::Path;

/// Result of a reactor case.
pub struct CaseResult {
    /// Summary document.
    pub summary: Value,
    /// Trajectory table (CSV) when the reactor produces one.
    pub csv: Option<String>,
}

struct Rd<'a> {
    errs: Vec<String>,
    root: &'a Value,
}

impl<'a> Rd<'a> {
    fn obj(&mut self, path: &str, v: Option<&'a Value>, required: bool) -> Option<&'a Map<String, Value>> {
        match v {
            Some(Value::Object(m)) => Some(m),
            Some(_) => {
                self.errs.push(format!("{path}: must be an object"));
                None
            }
            None => {
                if required {
                    self.errs.push(format!("{path}: missing"));
                }
                None
            }
        }
    }

    fn num(&mut self, path: &str, m: &Map<String, Value>, key: &str, positive: bool, required: bool) -> Option<f64> {
        match m.get(key) {
            Some(v) => match v.as_f64() {
                Some(x) if !positive || x > 0.0 => Some(x),
                Some(x) => {
                    self.errs
                        .push(format!("{path}.{key}: must be a positive number (got {x})"));
                    None
                }
                None => {
                    self.errs.push(format!("{path}.{key}: must be a number"));
                    None
                }
            },
            None => {
                if required {
                    self.errs.push(format!("{path}.{key}: missing"));
                }
                None
            }
        }
    }

    fn string(
        &mut self,
        path: &str,
        m: &Map<String, Value>,
        key: &str,
        allowed: &[&str],
        default: Option<&str>,
    ) -> Option<String> {
        match m.get(key) {
            Some(Value::String(s)) if allowed.is_empty() || allowed.contains(&s.as_str()) => Some(s.clone()),
            Some(Value::String(s)) => {
                self.errs
                    .push(format!("{path}.{key}: '{s}' is not one of {}", allowed.join(", ")));
                None
            }
            Some(_) => {
                self.errs.push(format!("{path}.{key}: must be a string"));
                None
            }
            None => match default {
                Some(d) => Some(d.to_string()),
                None => {
                    self.errs.push(format!("{path}.{key}: missing"));
                    None
                }
            },
        }
    }
}

const REACTOR_KEYS: &[&str] = &[
    "type",
    "energy",
    "temperature",
    "pressure",
    "composition",
    "end_time",
    "ignition",
    "residence_time",
    "length",
    "velocity",
    "constraint",
    "blowout",
    "output_points",
    "inlet_temperature",
];

/// Run a reactor case. `base` is the directory that relative mechanism paths are resolved against.
pub fn run_case(doc: &Value, base: &Path) -> Result<CaseResult, Vec<String>> {
    let mut rd = Rd {
        errs: Vec::new(),
        root: doc,
    };
    let root = rd.root;
    let Some(top) = rd.obj("$", Some(root), true) else {
        return Err(rd.errs);
    };
    for k in top.keys() {
        if k != "mechanism" && k != "reactor" && k != "tolerances" {
            rd.errs
                .push(format!("$.{k}: unknown key (expected mechanism, reactor, tolerances)"));
        }
    }
    let mech_o = rd.obj("mechanism", top.get("mechanism"), true);
    let rea = rd.obj("reactor", top.get("reactor"), true);
    let tol_o = rd.obj("tolerances", top.get("tolerances"), false);
    let mut tol = Tol::default();
    if let Some(t) = tol_o {
        if let Some(v) = rd.num("tolerances", t, "rtol", true, false) {
            tol.rtol = v;
        }
        if let Some(v) = rd.num("tolerances", t, "atol_t", true, false) {
            tol.atol_t = v;
        }
        if let Some(v) = rd.num("tolerances", t, "atol_y", true, false) {
            tol.atol_y = v;
        }
    }
    let (Some(mo), Some(ro)) = (mech_o, rea) else {
        return Err(rd.errs);
    };
    let read_file = |rd: &mut Rd, key: &str, required: bool| -> Option<String> {
        let name = rd.string("mechanism", mo, key, &[], if required { None } else { Some("") })?;
        if name.is_empty() {
            return None;
        }
        let p = base.join(&name);
        match std::fs::read_to_string(&p) {
            Ok(s) => Some(s),
            Err(e) => {
                rd.errs
                    .push(format!("mechanism.{key}: cannot read '{}': {e}", p.display()));
                None
            }
        }
    };
    let chem = read_file(&mut rd, "chemkin", true);
    let thermo = read_file(&mut rd, "thermo", false);
    for k in ro.keys() {
        if !REACTOR_KEYS.contains(&k.as_str()) {
            rd.errs.push(format!("reactor.{k}: unknown key"));
        }
    }
    let rtype = rd.string(
        "reactor",
        ro,
        "type",
        &[
            "constant_pressure",
            "constant_volume",
            "psr",
            "plug_flow",
            "equilibrium",
        ],
        None,
    );
    let energy = rd.string("reactor", ro, "energy", &["adiabatic", "isothermal"], Some("adiabatic"));
    let temp = rd.num("reactor", ro, "temperature", true, true);
    let pres = rd.num("reactor", ro, "pressure", true, true);
    let comp_o = rd.obj("reactor.composition", ro.get("composition"), true);
    let mech = match chem {
        Some(c) => match Mechanism::from_chemkin(&c, thermo.as_deref()) {
            Ok(m) => Some(m),
            Err(e) => {
                rd.errs.push(format!("mechanism: {e}"));
                None
            }
        },
        None => None,
    };
    let mut x = None;
    if let (Some(co), Some(m)) = (comp_o, &mech) {
        let (key, by_mass) = if co.contains_key("mole") {
            ("mole", false)
        } else {
            ("mass", true)
        };
        if let Some(amounts) = rd.obj(&format!("reactor.composition.{key}"), co.get(key), true) {
            {
                let mut list = Vec::new();
                for (name, v) in amounts {
                    match v.as_f64() {
                        Some(a) if a >= 0.0 => {
                            if m.species_index(name).is_none() {
                                rd.errs
                                    .push(format!("reactor.composition.{key}.{name}: unknown species"));
                            }
                            list.push((name.clone(), a));
                        }
                        _ => rd.errs.push(format!(
                            "reactor.composition.{key}.{name}: must be a non-negative number"
                        )),
                    }
                }
                if rd.errs.is_empty() {
                    if by_mass {
                        let mut y = vec![0.0; m.species.len()];
                        for (n, a) in &list {
                            y[m.species_index(n).unwrap()] += a;
                        }
                        let s: f64 = y.iter().sum();
                        if s > 0.0 {
                            let yy: Vec<f64> = y.iter().map(|v| v / s).collect();
                            x = Some(m.y_to_x(&yy));
                        } else {
                            rd.errs.push("reactor.composition.mass: amounts sum to zero".into());
                        }
                    } else {
                        match crate::mole_fractions(m, &list) {
                            Ok(v) => x = Some(v),
                            Err(e) => rd.errs.push(format!("reactor.composition.mole: {e}")),
                        }
                    }
                }
            }
        }
    }
    let end_time = rd.num(
        "reactor",
        ro,
        "end_time",
        true,
        matches!(rtype.as_deref(), Some("constant_pressure" | "constant_volume")),
    );
    let tau = rd.num("reactor", ro, "residence_time", true, rtype.as_deref() == Some("psr"));
    let length = rd.num("reactor", ro, "length", true, rtype.as_deref() == Some("plug_flow"));
    let velocity = rd.num("reactor", ro, "velocity", true, rtype.as_deref() == Some("plug_flow"));
    let constraint = rd.string("reactor", ro, "constraint", &["TP", "HP", "UV"], Some("HP"));
    let t_in = rd.num("reactor", ro, "inlet_temperature", true, false);
    let npts = ro.get("output_points").and_then(Value::as_u64).unwrap_or(200).max(2) as usize;
    if !rd.errs.is_empty() {
        return Err(rd.errs);
    }
    let (mech, x, rtype, energy, temp, pres) = (
        mech.unwrap(),
        x.unwrap(),
        rtype.unwrap(),
        energy.unwrap(),
        temp.unwrap(),
        pres.unwrap(),
    );
    let energy = if energy == "adiabatic" {
        Energy::Adiabatic
    } else {
        Energy::Isothermal
    };
    let y0 = mech.x_to_y(&x);
    let mut state0 = vec![temp];
    state0.extend(&y0);
    let fail = |e: String| vec![format!("reactor: {e}")];
    let names: Vec<&str> = mech.species.iter().map(|s| s.name.as_str()).collect();
    let table = |tr: &Trajectory, tname: &str, extra: bool| -> String {
        let mut s = format!("{tname},T");
        for n in &names {
            s.push_str(&format!(",Y_{n}"));
        }
        if extra {
            s.push_str(",residence_time");
        }
        s.push('\n');
        let n = tr.t.len();
        let stride = (n / npts).max(1);
        for i in (0..n)
            .step_by(stride)
            .chain(if (n - 1) % stride != 0 { Some(n - 1) } else { None })
        {
            s.push_str(&format!("{:e},{:e}", tr.t[i], tr.state[i][0]));
            for k in 0..names.len() {
                s.push_str(&format!(",{:e}", tr.state[i][1 + k]));
            }
            if extra {
                s.push_str(&format!(",{:e}", tr.state[i][names.len() + 1]));
            }
            s.push('\n');
        }
        s
    };
    let top_species = |yv: &[f64]| -> Value {
        let xs = mech.y_to_x(yv);
        let mut idx: Vec<usize> = (0..xs.len()).collect();
        idx.sort_by(|&a, &b| xs[b].total_cmp(&xs[a]));
        let mut m = Map::new();
        for &k in idx.iter().take(12) {
            m.insert(names[k].to_string(), json!(xs[k]));
        }
        Value::Object(m)
    };
    match rtype.as_str() {
        "constant_pressure" | "constant_volume" => {
            let kind = if rtype == "constant_pressure" {
                BatchKind::ConstantPressure
            } else {
                BatchKind::ConstantVolume
            };
            let tend = end_time.unwrap();
            let tr = run_batch(&mech, kind, energy, pres, &state0, tend, tol, Stop::EndTime)
                .map_err(|e| fail(e.to_string()))?;
            let ign = tr.time_of_max_dtdt();
            let rise = match ro.get("ignition") {
                Some(Value::Object(m)) => m.get("temperature_rise").and_then(Value::as_f64).unwrap_or(400.0),
                _ => 400.0,
            };
            let last = tr.last();
            let summary = json!({
                "type": rtype,
                "ignition_delay_max_dTdt": if tr.temp(tr.t.len() - 1) - temp > rise { ign.map_or(Value::Null, |v| json!(v)) } else { Value::Null },
                "ignition_delay_temperature_rise": tr.time_of_rise(rise),
                "temperature_rise_threshold": rise,
                "final_temperature": last[0],
                "final_mole_fractions_top": top_species(&last[1..]),
                "steps": tr.t.len() - 1,
            });
            Ok(CaseResult {
                summary,
                csv: Some(table(&tr, "time", false)),
            })
        }
        "plug_flow" => {
            let tr = run_plug_flow(&mech, energy, pres, &state0, velocity.unwrap(), length.unwrap(), tol)
                .map_err(|e| fail(e.to_string()))?;
            let last = tr.last();
            let summary = json!({"type": rtype, "final_temperature": last[0], "residence_time": last[names.len() + 1],
                "final_mole_fractions_top": top_species(&last[1..=names.len()]), "steps": tr.t.len() - 1});
            Ok(CaseResult {
                summary,
                csv: Some(table(&tr, "x", true)),
            })
        }
        "psr" => {
            let tin = t_in.unwrap_or(temp);
            let spec = Psr {
                tau: tau.unwrap(),
                p: pres,
                t_in: tin,
                y_in: y0.clone(),
                energy,
            };
            let eq = equilibrium(&mech, &x, tin, pres, Constraint::HP).map_err(|e| fail(e.to_string()))?;
            let mut guess = vec![eq.t];
            guess.extend(&eq.y);
            if let Some(Value::Object(b)) = ro.get("blowout") {
                let tau_start = b.get("tau_start").and_then(Value::as_f64).unwrap_or(spec.tau);
                let bracket = b.get("bracket").and_then(Value::as_f64).unwrap_or(0.05);
                let t_burn = tin + 0.4 * (eq.t - tin);
                let bo = psr_blowout(&mech, &spec, tau_start, &guess, t_burn, bracket, tol)
                    .map_err(|e| fail(e.to_string()))?;
                let summary = json!({"type": rtype, "equilibrium_temperature": eq.t, "burning_threshold_temperature": t_burn,
                    "blowout_tau_burning": bo.tau_burning, "blowout_tau_extinct": bo.tau_extinct,
                    "temperature_at_tau_burning": bo.state[0]});
                return Ok(CaseResult { summary, csv: None });
            }
            let (st, ok) = psr_steady(&mech, &spec, &guess, tol).map_err(|e| fail(e.to_string()))?;
            let summary = json!({"type": rtype, "converged": ok, "temperature": st[0], "equilibrium_temperature": eq.t,
                "mole_fractions_top": top_species(&st[1..])});
            Ok(CaseResult { summary, csv: None })
        }
        _ => {
            let c = match constraint.as_deref().unwrap_or("HP") {
                "TP" => Constraint::TP,
                "UV" => Constraint::UV,
                _ => Constraint::HP,
            };
            let eq = equilibrium(&mech, &x, temp, pres, c).map_err(|e| fail(e.to_string()))?;
            let w = mech.mean_w(&eq.y);
            let summary = json!({"type": "equilibrium", "constraint": constraint, "temperature": eq.t, "pressure": eq.p,
                "density": eq.p * w / (R_GAS * eq.t), "mole_fractions_top": top_species(&eq.y), "iterations": eq.iterations});
            Ok(CaseResult { summary, csv: None })
        }
    }
}

/// Convenience used by tests and the CLI: run the ignition-delay helper on a parsed case state.
pub fn ignition_delay(mech: &Mechanism, kind: BatchKind, p: f64, y0: &[f64], t_end: f64, tol: Tol) -> Option<f64> {
    ignition_delay_max_dtdt(mech, kind, p, y0, t_end, tol).ok().flatten()
}
