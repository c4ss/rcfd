//! Finite-rate and turbulent-chemistry source terms for the species transport (F-054, ADR 0027): the `chemistry`
//! settings section.
//!
//! A CHEMKIN-format mechanism ([`rcfd_reactor::Mechanism`]) supplies the reactions and the NASA-7 thermodynamics; every
//! mechanism species is a `species.<name>` entry of the case. The flow is the constant-density incompressible one, so
//! the chemistry sees the constant density `chemistry.density` (which must equal `physics.rho`) and, with heat release, the
//! constant heat capacity `chemistry.heat_capacity` (which must equal `physics.cp`).
//!
//! * Transient runs: Lie operator splitting. After the flow step and the transport sweeps, every cell advances by `dt`
//!   with the stiff integrator of the 0D toolkit ([`rcfd_reactor::cell::CellModel`]); the result is written back to the
//!   species fields (and the temperature with heat release) and to their time-history level.
//! * Steady runs: source-term form. Per iteration and cell the rates are evaluated from the current state, with the
//!   diagonal implicit linearisation `c_i = max(0, -dS_i/dY_i)` as an implicit sink; a negative source additionally gets
//!   Patankar's coefficient `-S_i/Y_i` (the fixed point is unchanged, a species cannot be drained below zero).
//!
//! Closures: `finite_rate` (laminar), `eddy_dissipation` (Magnussen-Hjertager), `finite_rate_eddy_dissipation`
//! (minimum of the two), `edc` (Eddy Dissipation Concept). Constants and their verification status: ADR 0027 D2.

use crate::case::{Case, Diagnostic};
use crate::scalars::ScalarState;
use rcfd_core::settings::Schema;
use rcfd_fv::energy::EnergySolver;
use rcfd_reactor::cell::{edc_scales, relax_towards, CellModel, Closure, EdParams};
use rcfd_reactor::reactors::Tol;
use rcfd_reactor::Mechanism;

/// Settings section.
pub fn schema() -> Schema {
    Schema::object()
        .doc("Chemical reactions of the species (F-054, ADR 0027): a CHEMKIN-format mechanism whose species are the species.<name> entries; constant-density flow")
        .field(
            "model",
            Schema::enumeration(
                "off",
                &["off", "finite_rate", "eddy_dissipation", "finite_rate_eddy_dissipation", "edc"],
            )
            .doc("off: no reactions; finite_rate: Arrhenius rates of the mechanism (laminar); eddy_dissipation: mixing-limited rate A rho (eps/k) min(...) of the irreversible reactions (needs k_epsilon or k_omega_sst); finite_rate_eddy_dissipation: the smaller of the two; edc: Eddy Dissipation Concept with the fine-structure reactor of the mechanism"),
        )
        .field(
            "mechanism",
            Schema::string("").doc("CHEMKIN-format mechanism file (relative to the case); give this or mechanism_text"),
        )
        .field("mechanism_text", Schema::string("").doc("The mechanism as inline CHEMKIN text"))
        .field(
            "thermo",
            Schema::string("").doc("Separate CHEMKIN thermodynamic file (optional; the mechanism may carry its own THERMO section)"),
        )
        .field("thermo_text", Schema::string("").doc("The thermodynamic data as inline text"))
        .field(
            "density",
            Schema::float(-1.0).doc("Constant mass density [kg/m3] of the kinetics; must equal physics.rho (checked); -1 = unset, an error"),
        )
        .field(
            "heat_release",
            Schema::boolean(false).doc("Couple the chemical enthalpy release to the temperature (needs energy.enabled): dT/dt = -sum h_k omega_k / (rho cp)"),
        )
        .field(
            "heat_capacity",
            Schema::float(-1.0).doc("heat_release only: constant cp [J/kg/K] of the enthalpy balance; must equal physics.cp (checked); -1 = unset, an error with heat_release"),
        )
        .field(
            "temperature",
            Schema::float(-1.0).doc("Isothermal chemistry temperature [K] for runs without the energy equation; -1 = unset, an error then; not allowed with energy.enabled"),
        )
        .field(
            "minimum_temperature",
            Schema::float(0.0).doc("Cells colder than this [K] are not integrated (0 = every cell is integrated)"),
        )
        .field("rtol", Schema::float(1e-8).doc("Relative tolerance of the cell integrator"))
        .field("atol_species", Schema::float(1e-14).doc("Absolute tolerance of the mass fractions"))
        .field("atol_temperature", Schema::float(1e-8).doc("Absolute tolerance of the temperature [K]"))
        .field(
            "eddy_dissipation_a",
            Schema::float(4.0).doc("Eddy-dissipation rate constant A (4.0 is the value usually attributed to Magnussen 1977; unverified against the primary paper, a model parameter)"),
        )
        .field(
            "eddy_dissipation_b",
            Schema::float(0.5).doc("Eddy-dissipation product-limiter constant B (0.5, same status as A)"),
        )
        .field(
            "eddy_dissipation_product_limit",
            Schema::boolean(true).doc("Include the product-limited term B Y_P / sum(nu W); false is the reactant-only form (a project option; with it the rate is zero until products exist)"),
        )
        .field(
            "edc_c_xi",
            Schema::float(2.1377).doc("EDC length-scale constant C_xi (value printed in a secondary source, arXiv:2101.08764; primary not accessed)"),
        )
        .field(
            "edc_c_tau",
            Schema::float(0.4082).doc("EDC time-scale constant C_tau (same source)"),
        )
}

fn gf(c: &Case, p: &str) -> f64 {
    c.tree.get(p).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn gs(c: &Case, p: &str) -> String {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn gb(c: &Case, p: &str) -> bool {
    c.tree.get(p).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Is a chemistry model selected?
pub fn enabled(case: &Case) -> bool {
    gs(case, "chemistry.model") != "off"
}

fn diag(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// Read a text given as a file (confined to the sandbox when there is one) or inline.
fn text_of(case: &Case, file_key: &str, text_key: &str) -> Result<Option<(String, String)>, Diagnostic> {
    let file = gs(case, &format!("chemistry.{file_key}"));
    let inline = gs(case, &format!("chemistry.{text_key}"));
    if !file.is_empty() && !inline.is_empty() {
        return Err(diag(
            &format!("chemistry.{text_key}"),
            format!("give either {file_key} or {text_key}, not both"),
        ));
    }
    if !file.is_empty() {
        let path = case.base_dir.join(&file);
        if let Some(root) = &case.sandbox {
            let inside = !std::path::Path::new(&file)
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
                && path
                    .canonicalize()
                    .ok()
                    .zip(root.canonicalize().ok())
                    .map_or(true, |(p, r)| p.starts_with(r));
            if !inside {
                return Err(diag(
                    &format!("chemistry.{file_key}"),
                    format!("file '{file}' is outside the sandbox"),
                ));
            }
        }
        return match std::fs::read_to_string(&path) {
            Ok(t) => Ok(Some((format!("chemistry.{file_key}"), t))),
            Err(e) => Err(diag(
                &format!("chemistry.{file_key}"),
                format!("cannot read {}: {e}", path.display()),
            )),
        };
    }
    if !inline.is_empty() {
        return Ok(Some((format!("chemistry.{text_key}"), inline)));
    }
    Ok(None)
}

/// Read and parse the mechanism of the case. The error carries the key and, for syntax errors, the line.
fn load_mechanism(case: &Case) -> Result<(Mechanism, String), Diagnostic> {
    let Some((key, mech_text)) = text_of(case, "mechanism", "mechanism_text")? else {
        return Err(diag(
            "chemistry.mechanism",
            "a chemistry model needs a mechanism (chemistry.mechanism file or chemistry.mechanism_text)",
        ));
    };
    let thermo = text_of(case, "thermo", "thermo_text")?;
    Mechanism::from_chemkin(&mech_text, thermo.as_ref().map(|(_, t)| t.as_str()))
        .map(|m| (m, key.clone()))
        .map_err(|e| Diagnostic {
            path: key,
            // the case parser locates the key itself; the line inside the mechanism text goes into the message
            line: None,
            message: if e.line > 0 {
                format!("mechanism line {}: {}", e.line, e.msg)
            } else {
                e.msg
            },
        })
}

fn species_names(case: &Case) -> Vec<String> {
    case.tree.names("species").unwrap_or_default()
}

/// Settings checks.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !enabled(case) {
        return out;
    }
    let model = gs(case, "chemistry.model");
    let alg = gs(case, "solver.algorithm");
    if !matches!(alg.as_str(), "simple" | "simplec" | "coupled" | "piso" | "pimple") {
        out.push(diag(
            "chemistry.model",
            format!(
                "chemistry is available with the incompressible steady and transient algorithms, not with \"{alg}\""
            ),
        ));
    }
    if gb(case, "solver.frozen_flow") {
        out.push(diag(
            "chemistry.model",
            "chemistry needs the species transport, which is not iterated with solver.frozen_flow",
        ));
    }
    let names = species_names(case);
    if names.is_empty() {
        out.push(diag(
            "species",
            "chemistry needs a species.<name> entry for every species of the mechanism",
        ));
    }
    match load_mechanism(case) {
        Err(d) => out.push(d),
        Ok((mech, key)) => {
            for s in &mech.species {
                if !names.iter().any(|n| n.eq_ignore_ascii_case(&s.name)) {
                    out.push(diag(
                        &key,
                        format!(
                            "mechanism species '{}' has no species.{} entry in the case",
                            s.name, s.name
                        ),
                    ));
                }
            }
            for n in &names {
                if mech.species_index(n).is_none() {
                    out.push(diag(
                        &format!("species.{n}"),
                        format!("'{n}' is not a species of the mechanism"),
                    ));
                }
            }
            if matches!(model.as_str(), "eddy_dissipation") {
                for r in mech.reactions.iter().filter(|r| r.reversible) {
                    out.push(Diagnostic {
                        path: key.clone(),
                        line: None,
                        message: format!(
                            "mechanism line {}: the eddy-dissipation model needs irreversible reactions; '{}' is reversible (use '=>')",
                            r.line, r.equation
                        ),
                    });
                }
            }
        }
    }
    // density
    let (rho, phys_rho) = (gf(case, "chemistry.density"), gf(case, "physics.rho"));
    if rho <= 0.0 {
        out.push(diag(
            "chemistry.density",
            "set chemistry.density [kg/m3], the constant density of the kinetics; it must equal physics.rho",
        ));
    } else if (rho - phys_rho).abs() > 1e-12 * phys_rho.abs().max(1e-300) {
        out.push(diag(
            "chemistry.density",
            format!("chemistry.density = {rho} differs from physics.rho = {phys_rho}; the flow and the kinetics must use the same constant density"),
        ));
    }
    // temperature and heat release
    let energy = gb(case, "energy.enabled");
    let temp = gf(case, "chemistry.temperature");
    if energy {
        if temp > 0.0 {
            out.push(diag(
                "chemistry.temperature",
                "chemistry.temperature is for runs without the energy equation; with energy.enabled the cell temperature is used",
            ));
        }
    } else if temp <= 0.0 {
        out.push(diag(
            "chemistry.temperature",
            "without energy.enabled the chemistry is isothermal: set chemistry.temperature [K] (unset = -1)",
        ));
    }
    let heat = gb(case, "chemistry.heat_release");
    let cp = gf(case, "chemistry.heat_capacity");
    if heat {
        if !energy {
            out.push(diag(
                "chemistry.heat_release",
                "heat release needs energy.enabled = true",
            ));
        }
        let phys_cp = gf(case, "physics.cp");
        if cp <= 0.0 {
            out.push(diag(
                "chemistry.heat_capacity",
                "heat release needs chemistry.heat_capacity [J/kg/K], which must equal physics.cp",
            ));
        } else if (cp - phys_cp).abs() > 1e-12 * phys_cp.abs().max(1e-300) {
            out.push(diag(
                "chemistry.heat_capacity",
                format!("chemistry.heat_capacity = {cp} differs from physics.cp = {phys_cp}; the energy equation and the heat release must use the same heat capacity"),
            ));
        }
    } else if cp > 0.0 {
        out.push(diag(
            "chemistry.heat_capacity",
            "chemistry.heat_capacity is used only with chemistry.heat_release = true",
        ));
    }
    // turbulent closures
    let turbulent = matches!(
        model.as_str(),
        "eddy_dissipation" | "finite_rate_eddy_dissipation" | "edc"
    );
    if turbulent {
        let tm = gs(case, "turbulence.model");
        if !matches!(tm.as_str(), "k_epsilon" | "k_omega_sst") {
            out.push(diag(
                "chemistry.model",
                format!("the model \"{model}\" needs the turbulence fields k and epsilon (turbulence.model = \"k_epsilon\" or \"k_omega_sst\"), not \"{tm}\""),
            ));
        }
    }
    if model == "edc" && gf(case, "physics.nu") <= 0.0 {
        out.push(diag("physics.nu", "the EDC scales need a positive physics.nu"));
    }
    for (p, bad) in [
        (
            "chemistry.eddy_dissipation_a",
            gf(case, "chemistry.eddy_dissipation_a") <= 0.0,
        ),
        (
            "chemistry.eddy_dissipation_b",
            gf(case, "chemistry.eddy_dissipation_b") < 0.0,
        ),
        ("chemistry.edc_c_xi", gf(case, "chemistry.edc_c_xi") <= 0.0),
        ("chemistry.edc_c_tau", gf(case, "chemistry.edc_c_tau") <= 0.0),
        ("chemistry.rtol", gf(case, "chemistry.rtol") <= 0.0),
        ("chemistry.atol_species", gf(case, "chemistry.atol_species") < 0.0),
        (
            "chemistry.atol_temperature",
            gf(case, "chemistry.atol_temperature") < 0.0,
        ),
        (
            "chemistry.minimum_temperature",
            gf(case, "chemistry.minimum_temperature") < 0.0,
        ),
    ] {
        if bad {
            out.push(diag(
                p,
                "out of range (positive constants, non-negative tolerances and temperature floor)",
            ));
        }
    }
    out
}

/// Which closure of the mechanism rates is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    FiniteRate,
    EddyDissipation,
    Limited,
    Edc,
}

/// Sources of one steady iteration.
pub struct SteadyTerms {
    /// Per transported species: `(name, source, sink)` in mass-fraction units per second.
    pub species: Vec<(String, Vec<f64>, Vec<f64>)>,
    /// Temperature source (K/s) with heat release.
    pub heat: Option<Vec<f64>>,
}

/// Turbulence inputs of the closures that need them: `k`, `epsilon` cell values.
#[derive(Clone, Copy)]
pub struct Mixing<'a> {
    /// Turbulent kinetic energy.
    pub k: &'a [f64],
    /// Dissipation rate.
    pub eps: &'a [f64],
}

/// The configured chemistry.
pub struct ChemState {
    mech: Mechanism,
    kind: Kind,
    rho: f64,
    cp: Option<f64>,
    temperature: f64,
    min_temperature: f64,
    tol: Tol,
    a: f64,
    b: f64,
    product_limit: bool,
    c_xi: f64,
    c_tau: f64,
    nu: f64,
}

impl ChemState {
    /// Build from the case; `None` when the model is off. Settings errors are those of [`check`].
    pub fn from_case(case: &Case) -> Result<Option<ChemState>, Vec<Diagnostic>> {
        if !enabled(case) {
            return Ok(None);
        }
        let errs = check(case);
        if !errs.is_empty() {
            return Err(errs);
        }
        let (mech, _) = load_mechanism(case).map_err(|d| vec![d])?;
        let kind = match gs(case, "chemistry.model").as_str() {
            "eddy_dissipation" => Kind::EddyDissipation,
            "finite_rate_eddy_dissipation" => Kind::Limited,
            "edc" => Kind::Edc,
            _ => Kind::FiniteRate,
        };
        Ok(Some(ChemState {
            mech,
            kind,
            rho: gf(case, "chemistry.density"),
            cp: gb(case, "chemistry.heat_release").then(|| gf(case, "chemistry.heat_capacity")),
            temperature: gf(case, "chemistry.temperature"),
            min_temperature: gf(case, "chemistry.minimum_temperature"),
            tol: Tol {
                rtol: gf(case, "chemistry.rtol"),
                atol_t: gf(case, "chemistry.atol_temperature"),
                atol_y: gf(case, "chemistry.atol_species"),
            },
            a: gf(case, "chemistry.eddy_dissipation_a"),
            b: gf(case, "chemistry.eddy_dissipation_b"),
            product_limit: gb(case, "chemistry.eddy_dissipation_product_limit"),
            c_xi: gf(case, "chemistry.edc_c_xi"),
            c_tau: gf(case, "chemistry.edc_c_tau"),
            nu: gf(case, "physics.nu"),
        }))
    }

    /// Does the closure need `k` and `epsilon`?
    pub fn needs_turbulence(&self) -> bool {
        self.kind != Kind::FiniteRate
    }

    /// The mechanism.
    pub fn mechanism(&self) -> &Mechanism {
        &self.mech
    }

    fn cell_model(&self, eps_over_k: f64) -> CellModel<'_> {
        let ed = EdParams {
            a: self.a,
            b: self.b,
            eps_over_k,
            product_limit: self.product_limit,
        };
        CellModel {
            mech: &self.mech,
            rho: self.rho,
            cp: self.cp,
            closure: match self.kind {
                Kind::EddyDissipation => Closure::EddyDissipation(ed),
                Kind::Limited => Closure::KineticLimited(ed),
                _ => Closure::Kinetic,
            },
            tol: self.tol,
        }
    }

    fn eps_over_k(&self, mix: Option<Mixing>, c: usize) -> f64 {
        match mix {
            Some(m) if m.k[c] > 0.0 && m.eps[c] > 0.0 => m.eps[c] / m.k[c],
            _ => 0.0,
        }
    }

    /// Gather the full-species columns (mechanism order) and the temperature column.
    fn columns(&self, sc: &ScalarState, temp: Option<&[f64]>) -> Result<(Vec<Vec<f64>>, Vec<f64>), String> {
        let mut cols = Vec::with_capacity(self.mech.species.len());
        for s in &self.mech.species {
            let col = sc
                .field_ci(&s.name)
                .ok_or_else(|| format!("species '{}' is not in the case", s.name))?;
            cols.push(col);
        }
        let n = cols.first().map_or(0, Vec::len);
        let t = match temp {
            Some(t) => t.to_vec(),
            None => vec![self.temperature; n],
        };
        Ok((cols, t))
    }

    /// Transient operator-split step of size `dt`: chemistry advances every cell and the result is written back to the
    /// species (and the temperature of `energy` with heat release).
    pub fn step_split(
        &self,
        sc: &mut ScalarState,
        energy: Option<&mut EnergySolver>,
        mix: Option<Mixing>,
        dt: f64,
    ) -> Result<(), String> {
        let temp_in = energy.as_ref().map(|e| e.t.values.clone());
        let (mut cols, mut tcol) = self.columns(sc, temp_in.as_deref())?;
        let ns = self.mech.species.len();
        let n = tcol.len();
        let mut st = vec![0.0; ns + 1];
        for c in 0..n {
            if tcol[c] < self.min_temperature {
                continue;
            }
            st[0] = tcol[c];
            for k in 0..ns {
                st[1 + k] = cols[k][c];
            }
            let cell = self.cell_model(self.eps_over_k(mix, c));
            let fail = |e: rcfd_reactor::reactors::ReactorError| {
                format!("chemistry failed in cell {c} (T = {:.6e} K): {e}", tcol[c])
            };
            if self.kind == Kind::Edc {
                let Some(m) = mix else {
                    return Err("the EDC closure needs k and epsilon".into());
                };
                let Some((_, tau, rate)) = edc_scales(self.nu, m.k[c], m.eps[c], self.c_xi, self.c_tau) else {
                    continue;
                };
                let fs = cell.fine_structure(&st, tau).map_err(fail)?;
                for k in 0..ns {
                    cols[k][c] = relax_towards(st[1 + k], fs[1 + k], rate, dt);
                }
                if self.cp.is_some() {
                    tcol[c] = relax_towards(st[0], fs[0], rate, dt);
                }
            } else {
                cell.advance(&mut st, dt).map_err(fail)?;
                for k in 0..ns {
                    cols[k][c] = st[1 + k];
                }
                if self.cp.is_some() {
                    tcol[c] = st[0];
                }
            }
        }
        for (s, col) in self.mech.species.iter().zip(cols) {
            sc.set_species_ci(&s.name, col);
        }
        if self.cp.is_some() {
            if let Some(en) = energy {
                en.t.values.copy_from_slice(&tcol);
                en.history.old.copy_from_slice(&tcol);
            }
        }
        Ok(())
    }

    /// Source and implicit-sink terms of a steady iteration for the transported species, and the heat source.
    pub fn steady_terms(
        &self,
        sc: &ScalarState,
        temp: Option<&[f64]>,
        mix: Option<Mixing>,
    ) -> Result<SteadyTerms, String> {
        let (cols, tcol) = self.columns(sc, temp)?;
        let ns = self.mech.species.len();
        let n = tcol.len();
        let transported: Vec<bool> = self
            .mech
            .species
            .iter()
            .map(|s| sc.is_transported_ci(&s.name))
            .collect();
        let mut src = vec![vec![0.0; n]; ns];
        let mut snk = vec![vec![0.0; n]; ns];
        let mut heat = self.cp.map(|_| vec![0.0; n]);
        let mut st = vec![0.0; ns + 1];
        let mut f0 = vec![0.0; ns + 1];
        for c in 0..n {
            if tcol[c] < self.min_temperature {
                continue;
            }
            st[0] = tcol[c];
            for k in 0..ns {
                st[1 + k] = cols[k][c];
            }
            let cell = self.cell_model(self.eps_over_k(mix, c));
            if self.kind == Kind::Edc {
                let Some(m) = mix else {
                    return Err("the EDC closure needs k and epsilon".into());
                };
                let Some((_, tau, rate)) = edc_scales(self.nu, m.k[c], m.eps[c], self.c_xi, self.c_tau) else {
                    continue;
                };
                let fs = cell
                    .fine_structure(&st, tau)
                    .map_err(|e| format!("chemistry failed in cell {c} (T = {:.6e} K): {e}", tcol[c]))?;
                for k in 0..ns {
                    src[k][c] = rate * fs[1 + k];
                    snk[k][c] = rate;
                }
                if let Some(h) = heat.as_mut() {
                    h[c] = rate * (fs[0] - st[0]);
                }
            } else {
                cell.rhs(&st, &mut f0);
                let sink = cell.sink_coefficients(&st, &f0);
                for k in 0..ns {
                    // Patankar's rule for a negative source: the consumption rate over the lagged value is a sink
                    // coefficient, so that a species cannot be drained below zero (the fixed point is unchanged).
                    let y = st[1 + k];
                    let pat = if y > 1e-30 && f0[1 + k] < 0.0 {
                        -f0[1 + k] / y
                    } else {
                        0.0
                    };
                    let c_k = sink[k].max(pat);
                    src[k][c] = f0[1 + k] + c_k * y;
                    snk[k][c] = c_k;
                }
                if let Some(h) = heat.as_mut() {
                    h[c] = f0[0];
                }
            }
        }
        let species = self
            .mech
            .species
            .iter()
            .enumerate()
            .filter(|(k, _)| transported[*k])
            .map(|(k, s)| (s.name.clone(), std::mem::take(&mut src[k]), std::mem::take(&mut snk[k])))
            .collect();
        Ok(SteadyTerms { species, heat })
    }
}

/// Net molar rates of progress and production for prescribed fields, for tests: the state `[T, Y...]` of one cell.
pub fn cell_rates(chem: &ChemState, st: &[f64], eps_over_k: f64) -> (Vec<f64>, Vec<f64>) {
    let cell = chem.cell_model(eps_over_k);
    let mut wdot = vec![0.0; chem.mech.species.len()];
    let mut q = vec![0.0; chem.mech.reactions.len()];
    cell.production(st, &mut wdot, &mut q);
    (q, wdot)
}

/// `k` and `epsilon` cell values of the turbulence model (`epsilon = 0.09 k omega` for `k_omega_sst`, the SST beta*).
fn mixing_vectors(turb: Option<&crate::turb::TurbState>) -> Option<(Vec<f64>, Vec<f64>)> {
    let t = turb?;
    let k = t.scalar("k")?.values.clone();
    if let Some(e) = t.scalar("epsilon") {
        return Some((k, e.values.clone()));
    }
    let w = t.scalar("omega")?;
    let eps = k.iter().zip(&w.values).map(|(k, w)| 0.09 * k * w).collect();
    Some((k, eps))
}

/// One steady iteration: set the species chemistry terms and return the heat source (K/s) with heat release.
pub(crate) fn apply_steady(
    chem: &ChemState,
    scalars: Option<&mut ScalarState>,
    temperature: Option<&[f64]>,
    turb: Option<&crate::turb::TurbState>,
) -> Result<Option<Vec<f64>>, String> {
    let Some(sc) = scalars else {
        return Ok(None);
    };
    let mv = mixing_vectors(turb);
    if chem.needs_turbulence() && mv.is_none() {
        return Err("the chemistry closure needs the turbulence fields k and epsilon".into());
    }
    let mix = mv.as_ref().map(|(k, e)| Mixing { k, eps: e });
    let terms = chem.steady_terms(sc, temperature, mix)?;
    for (name, s, k) in terms.species {
        sc.set_chemistry_terms(&name, s, k);
    }
    Ok(terms.heat)
}

/// One transient operator-split chemistry step after the transport sweeps.
pub(crate) fn apply_split(
    chem: &ChemState,
    scalars: Option<&mut ScalarState>,
    energy: Option<&mut EnergySolver>,
    turb: Option<&crate::turb::TurbState>,
    dt: f64,
) -> Result<(), String> {
    let Some(sc) = scalars else {
        return Ok(());
    };
    let mv = mixing_vectors(turb);
    if chem.needs_turbulence() && mv.is_none() {
        return Err("the chemistry closure needs the turbulence fields k and epsilon".into());
    }
    let mix = mv.as_ref().map(|(k, e)| Mixing { k, eps: e });
    chem.step_split(sc, energy, mix, dt)
}
