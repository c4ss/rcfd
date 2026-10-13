//! Generalised-Newtonian viscosity laws and temperature shifts (F-110).
//!
//! The apparent dynamic viscosity depends on the shear rate `g = sqrt(2 S:S)` (S the strain-rate
//! tensor of the resolved velocity) and optionally on temperature:
//!
//! * power law `mu = K g^(n-1)`
//! * Carreau `mu = mu_inf + (mu_0 - mu_inf) [1 + (lambda g)^2]^((n-1)/2)`
//! * Carreau-Yasuda `mu = mu_inf + (mu_0 - mu_inf) [1 + (lambda g)^a]^((n-1)/a)`
//! * Cross `mu = mu_inf + (mu_0 - mu_inf) / [1 + (lambda g)^m]`
//! * Herschel-Bulkley `mu = K g^(n-1) + tau_0 [1 - exp(-m_p g)] / g` (Papanastasiou regularisation of the
//!   yield stress, `m_p` the regularisation time); Bingham is the case `n = 1` with `K` the plastic viscosity
//! * temperature shift (time-temperature superposition) `mu(g, T) = a_T mu_ref(a_T g)`, with the Arrhenius
//!   factor `a_T = exp(E (1/T - 1/T_ref))`, the WLF factor `log10 a_T = -C1 (T - T_ref) / (C2 + T - T_ref)`
//!   or a table `a_T(T)` (F-105 lookup tables)
//!
//! The forms are the textbook ones (Bird, Armstrong and Hassager 1987; Carreau 1972; Yasuda 1981; Cross
//! 1965; Herschel and Bulkley 1926; Papanastasiou 1987; Williams, Landel and Ferry 1955); see
//! docs/reference-data.md for the verification status. Viscosity is clamped to
//! `[min_viscosity, max_viscosity]`. The solver uses it as a kinematic face viscosity `mu / rho` in the
//! diffusion term `div(nu grad u)` (the transposed-gradient term of a variable viscosity is not included).

use crate::case::{Case, Diagnostic};
use rcfd_core::table::Table;
use rcfd_fv::{FvGeometry, VectorField};
use std::sync::Arc;

/// Smallest shear rate used in the laws (1/s), so `g^(n-1)` stays finite.
const SHEAR_FLOOR: f64 = 1e-10;

/// Viscosity law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    /// Constant viscosity (only useful with a temperature shift).
    Newtonian,
    /// Ostwald-de Waele power law.
    PowerLaw,
    /// Carreau.
    Carreau,
    /// Carreau-Yasuda.
    CarreauYasuda,
    /// Cross.
    Cross,
    /// Herschel-Bulkley with regularised yield stress.
    HerschelBulkley,
    /// Bingham (Herschel-Bulkley with n = 1).
    Bingham,
}

/// Temperature dependence.
#[derive(Debug, Clone)]
pub enum Shift {
    /// None.
    None,
    /// `a_T = exp(E (1/T - 1/T_ref))`, `E` in kelvin.
    Arrhenius {
        /// Activation temperature `E_a / R` (K).
        e: f64,
        /// Reference temperature (K).
        t_ref: f64,
    },
    /// Williams-Landel-Ferry.
    Wlf {
        /// First constant.
        c1: f64,
        /// Second constant (K).
        c2: f64,
        /// Reference temperature (K).
        t_ref: f64,
    },
    /// `a_T` read from a lookup table (`tables.<name>`, input column `T`, output column `a_T`).
    Table {
        /// The table.
        table: Arc<Table>,
        /// Index of the `a_T` output.
        out: usize,
    },
}

/// A configured viscosity law.
#[derive(Debug, Clone)]
pub struct Rheology {
    /// The law.
    pub model: Model,
    /// Consistency index `K` (Pa s^n) / plastic viscosity.
    pub k: f64,
    /// Power-law index `n`.
    pub n: f64,
    /// Zero-shear viscosity (Pa s); for `Newtonian` the reference viscosity.
    pub mu0: f64,
    /// Infinite-shear viscosity (Pa s).
    pub mu_inf: f64,
    /// Relaxation (time constant) `lambda` (s).
    pub lambda: f64,
    /// Yasuda exponent.
    pub a: f64,
    /// Cross exponent.
    pub m: f64,
    /// Yield stress (Pa).
    pub tau0: f64,
    /// Regularisation time of the yield stress (s).
    pub reg: f64,
    /// Lower clamp of the viscosity (Pa s).
    pub mu_min: f64,
    /// Upper clamp of the viscosity (Pa s).
    pub mu_max: f64,
    /// Temperature shift.
    pub shift: Shift,
    /// Under-relaxation of the viscosity between iterations (1 = none).
    pub relax: f64,
    /// Density (kg/m^3).
    pub rho: f64,
    prev: Option<Vec<f64>>,
}

fn f(c: &Case, p: &str) -> f64 {
    c.tree.get(p).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn s(c: &Case, p: &str) -> String {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// Is a non-trivial viscosity law requested?
pub fn enabled(case: &Case) -> bool {
    s(case, "rheology.model") != "newtonian" || s(case, "rheology.shift") != "none"
}

impl Rheology {
    /// Build from the `rheology` settings; `None` when the fluid is plain Newtonian.
    pub fn from_case(case: &Case) -> Option<Rheology> {
        if !enabled(case) {
            return None;
        }
        let model = match s(case, "rheology.model").as_str() {
            "power_law" => Model::PowerLaw,
            "carreau" => Model::Carreau,
            "carreau_yasuda" => Model::CarreauYasuda,
            "cross" => Model::Cross,
            "herschel_bulkley" => Model::HerschelBulkley,
            "bingham" => Model::Bingham,
            _ => Model::Newtonian,
        };
        let rho = f(case, "physics.rho");
        let t_ref = f(case, "rheology.reference_temperature");
        let shift = match s(case, "rheology.shift").as_str() {
            "arrhenius" => Shift::Arrhenius {
                e: f(case, "rheology.activation_temperature"),
                t_ref,
            },
            "wlf" => Shift::Wlf {
                c1: f(case, "rheology.wlf_c1"),
                c2: f(case, "rheology.wlf_c2"),
                t_ref,
            },
            "table" => {
                let name = s(case, "rheology.shift_table");
                let table = crate::exprs::load_tables(case).0.get(&name).cloned();
                match table.and_then(|t| t.output_index("a_T").map(|out| (t, out))) {
                    Some((table, out)) => Shift::Table { table, out },
                    None => Shift::None,
                }
            }
            _ => Shift::None,
        };
        Some(Rheology {
            model,
            k: f(case, "rheology.consistency_index"),
            n: if model == Model::Bingham {
                1.0
            } else {
                f(case, "rheology.power_index")
            },
            mu0: if model == Model::Newtonian {
                f(case, "physics.nu") * rho
            } else {
                f(case, "rheology.zero_shear_viscosity")
            },
            mu_inf: f(case, "rheology.infinite_shear_viscosity"),
            lambda: f(case, "rheology.time_constant"),
            a: f(case, "rheology.yasuda_exponent"),
            m: f(case, "rheology.cross_exponent"),
            tau0: f(case, "rheology.yield_stress"),
            reg: f(case, "rheology.regularisation_time"),
            mu_min: f(case, "rheology.min_viscosity"),
            mu_max: f(case, "rheology.max_viscosity"),
            shift,
            relax: f(case, "rheology.relax"),
            rho,
            prev: None,
        })
    }

    /// Temperature shift factor `a_T` (1 without a shift).
    pub fn shift_factor(&self, t: f64) -> f64 {
        match &self.shift {
            Shift::None => 1.0,
            Shift::Arrhenius { e, t_ref } => (e * (1.0 / t - 1.0 / t_ref)).exp(),
            Shift::Wlf { c1, c2, t_ref } => 10f64.powf(-c1 * (t - t_ref) / (c2 + t - t_ref)),
            Shift::Table { table, out } => table.eval(*out, &[t]),
        }
    }

    /// Reference-temperature viscosity (Pa s) at shear rate `g` (1/s), before clamping.
    pub fn law(&self, g: f64) -> f64 {
        let g = g.max(SHEAR_FLOOR);
        match self.model {
            Model::Newtonian => self.mu0,
            Model::PowerLaw => self.k * g.powf(self.n - 1.0),
            Model::Carreau => {
                self.mu_inf + (self.mu0 - self.mu_inf) * (1.0 + (self.lambda * g).powi(2)).powf((self.n - 1.0) / 2.0)
            }
            Model::CarreauYasuda => {
                self.mu_inf
                    + (self.mu0 - self.mu_inf) * (1.0 + (self.lambda * g).powf(self.a)).powf((self.n - 1.0) / self.a)
            }
            Model::Cross => self.mu_inf + (self.mu0 - self.mu_inf) / (1.0 + (self.lambda * g).powf(self.m)),
            Model::HerschelBulkley | Model::Bingham => {
                self.k * g.powf(self.n - 1.0) + self.tau0 * (1.0 - (-self.reg * g).exp()) / g
            }
        }
    }

    /// Apparent dynamic viscosity (Pa s) at shear rate `g` and temperature `t`, clamped; NaN when a shift
    /// table does not cover `t`.
    pub fn viscosity(&self, g: f64, t: f64) -> f64 {
        let at = self.shift_factor(t);
        (at * self.law(at * g)).clamp(self.mu_min, self.mu_max)
    }

    /// Kinematic viscosity at every face from the cell velocity field (and cell temperatures), relaxed
    /// against the previous call. Cell values are interpolated to the faces; boundary faces use the owner.
    pub fn nu_faces(
        &mut self,
        geom: &FvGeometry,
        u: &VectorField,
        temp: Option<&[f64]>,
        t_default: f64,
    ) -> Result<Vec<f64>, String> {
        let shear = rcfd_turb::eddy::strain_rate(geom, u);
        let t_ref = match &self.shift {
            Shift::Arrhenius { t_ref, .. } | Shift::Wlf { t_ref, .. } => *t_ref,
            Shift::Table { .. } | Shift::None => t_default,
        };
        let mu_cell: Vec<f64> = (0..shear.len())
            .map(|c| self.viscosity(shear[c], temp.map_or(t_ref, |t| t[c])))
            .collect();
        if let Some(c) = mu_cell.iter().position(|m| !m.is_finite()) {
            let t = temp.map_or(t_ref, |t| t[c]);
            return Err(format!(
                "viscosity law: no finite viscosity at T = {t} K in cell {c} (the table rheology.shift_table does not cover it; extend it or set its extrapolation to clamp)"
            ));
        }
        let mesh = &geom.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let mut nu: Vec<f64> = (0..mesh.n_faces())
            .map(|fc| {
                let mu = if fc < geom.n_internal {
                    let w = geom.weights[fc];
                    w * mu_cell[own[fc]] + (1.0 - w) * mu_cell[nei[fc]]
                } else {
                    mu_cell[own[fc]]
                };
                mu / self.rho
            })
            .collect();
        if let Some(p) = &self.prev {
            if self.relax < 1.0 && p.len() == nu.len() {
                for (a, b) in nu.iter_mut().zip(p) {
                    *a = b + self.relax * (*a - b);
                }
            }
        }
        self.prev = Some(nu.clone());
        Ok(nu)
    }

    /// Forget the relaxation history (after a restart or a field change).
    pub fn reset(&mut self) {
        self.prev = None;
    }
}

/// Check the `rheology` settings against the rest of the case.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !enabled(case) {
        return out;
    }
    let bad = |path: &str, message: String| Diagnostic {
        path: path.into(),
        line: None,
        message,
    };
    let model = s(case, "rheology.model");
    let (k, n) = (f(case, "rheology.consistency_index"), f(case, "rheology.power_index"));
    let (mu0, mu_inf) = (
        f(case, "rheology.zero_shear_viscosity"),
        f(case, "rheology.infinite_shear_viscosity"),
    );
    let lambda = f(case, "rheology.time_constant");
    if matches!(model.as_str(), "power_law" | "herschel_bulkley" | "bingham") && k <= 0.0 {
        out.push(bad(
            "rheology.consistency_index",
            format!("the {model} law needs a positive consistency index"),
        ));
    }
    if matches!(model.as_str(), "carreau" | "carreau_yasuda" | "cross") {
        if mu0 <= 0.0 || mu_inf < 0.0 || mu_inf > mu0 {
            out.push(bad(
                "rheology.zero_shear_viscosity",
                "needs 0 <= infinite_shear_viscosity <= zero_shear_viscosity and a positive zero-shear viscosity"
                    .into(),
            ));
        }
        if lambda <= 0.0 {
            out.push(bad("rheology.time_constant", "must be positive".into()));
        }
    }
    if model == "carreau_yasuda" && f(case, "rheology.yasuda_exponent") <= 0.0 {
        out.push(bad("rheology.yasuda_exponent", "must be positive".into()));
    }
    if model == "cross" && f(case, "rheology.cross_exponent") <= 0.0 {
        out.push(bad("rheology.cross_exponent", "must be positive".into()));
    }
    if matches!(model.as_str(), "herschel_bulkley" | "bingham") {
        if f(case, "rheology.yield_stress") <= 0.0 {
            out.push(bad(
                "rheology.yield_stress",
                format!("the {model} law needs a positive yield stress"),
            ));
        }
        if f(case, "rheology.regularisation_time") <= 0.0 {
            out.push(bad("rheology.regularisation_time", "must be positive".into()));
        }
    }
    if n <= 0.0 {
        out.push(bad("rheology.power_index", "must be positive".into()));
    }
    let (lo, hi) = (f(case, "rheology.min_viscosity"), f(case, "rheology.max_viscosity"));
    if hi <= lo || hi <= 0.0 {
        out.push(bad(
            "rheology.max_viscosity",
            "must be positive and larger than min_viscosity".into(),
        ));
    }
    let shift = s(case, "rheology.shift");
    if shift != "none" {
        if !case
            .tree
            .get("energy.enabled")
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            out.push(bad(
                "rheology.shift",
                "a temperature shift needs energy.enabled = true".into(),
            ));
        }
        if matches!(model.as_str(), "herschel_bulkley" | "bingham") {
            out.push(bad(
                "rheology.shift",
                "a temperature shift is not defined for the yield-stress laws".into(),
            ));
        }
        if f(case, "rheology.reference_temperature") <= 0.0 {
            out.push(bad(
                "rheology.reference_temperature",
                "must be positive (kelvin)".into(),
            ));
        }
        if shift == "wlf" && f(case, "rheology.wlf_c2") <= 0.0 {
            out.push(bad("rheology.wlf_c2", "must be positive".into()));
        }
        if shift == "table" {
            let name = s(case, "rheology.shift_table");
            match crate::exprs::load_tables(case).0.get(&name) {
                None => out.push(bad(
                    "rheology.shift_table",
                    format!("no usable table '{name}' (define tables.{name} with input T and output a_T)"),
                )),
                Some(t) => {
                    if t.inputs.len() != 1 || t.inputs[0].name != "T" || t.output_index("a_T").is_none() {
                        out.push(bad(
                            "rheology.shift_table",
                            format!("table '{name}' must have the single input column T and an output column a_T"),
                        ));
                    }
                }
            }
        }
    }
    let alg = s(case, "solver.algorithm");
    if alg == "compressible" {
        out.push(bad(
            "rheology.model",
            "viscosity laws are not available with the compressible solver".into(),
        ));
    }
    if crate::turb::enabled(case) {
        out.push(bad(
            "rheology.model",
            "viscosity laws cannot be combined with a turbulence model".into(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(model: Model) -> Rheology {
        Rheology {
            model,
            k: 0.5,
            n: 0.6,
            mu0: 1.0,
            mu_inf: 0.001,
            lambda: 2.0,
            a: 2.0,
            m: 2.0,
            tau0: 3.0,
            reg: 1000.0,
            mu_min: 0.0,
            mu_max: 1e12,
            shift: Shift::None,
            relax: 1.0,
            rho: 1.0,
            prev: None,
        }
    }

    #[test]
    fn power_law_values() {
        let r = base(Model::PowerLaw);
        assert!((r.viscosity(100.0, 300.0) - 0.5 * 100f64.powf(-0.4)).abs() < 1e-14);
        let mut c = base(Model::PowerLaw);
        c.mu_max = 0.2;
        assert_eq!(c.viscosity(1e-6, 300.0), 0.2);
    }

    #[test]
    fn carreau_family_recovers_newtonian_limits() {
        for m in [Model::Carreau, Model::CarreauYasuda, Model::Cross] {
            let r = base(m);
            // low shear: mu_0 to within 1e-6 relative
            let lo = r.viscosity(1e-4, 300.0);
            assert!(((lo - r.mu0) / r.mu0).abs() < 1e-6, "{m:?} low {lo}");
            // high shear: the infinite-shear plateau; the deviation falls like (lambda g)^(n-1) (Carreau
            // family) or (lambda g)^-m (Cross), checked at g = 1e14
            let hi = r.viscosity(1e14, 300.0);
            assert!(((hi - r.mu_inf) / r.mu_inf).abs() < 3e-3, "{m:?} high {hi}");
        }
    }

    #[test]
    fn carreau_closed_form_and_yasuda_reduces_to_carreau() {
        let c = base(Model::Carreau);
        let g = 3.7;
        let exact = 0.001 + (1.0 - 0.001) * (1.0 + (2.0f64 * g).powi(2)).powf(-0.2);
        assert!((c.viscosity(g, 300.0) - exact).abs() < 1e-14);
        // a = 2 makes Carreau-Yasuda identical to Carreau
        let y = base(Model::CarreauYasuda);
        assert!((y.viscosity(g, 300.0) - c.viscosity(g, 300.0)).abs() < 1e-14);
    }

    #[test]
    fn yield_stress_laws() {
        let hb = base(Model::HerschelBulkley);
        // at high shear the regularised yield term tends to tau0/g
        let g = 50.0;
        assert!((hb.viscosity(g, 300.0) - (0.5 * g.powf(-0.4) + 3.0 / g)).abs() < 1e-12);
        // at vanishing shear the apparent viscosity is bounded by tau0 * m_p (+ the power-law term)
        let low = hb.viscosity(1e-8, 300.0);
        assert!(
            low > 1000.0 && low <= 3.0 * 1000.0 + 0.5 * 1e-10f64.powf(-0.4) + 1.0,
            "{low}"
        );
        // Bingham: n = 1, K plastic viscosity
        let mut b = base(Model::Bingham);
        b.n = 1.0;
        assert!((b.viscosity(g, 300.0) - (0.5 + 3.0 / g)).abs() < 1e-12);
    }

    #[test]
    fn temperature_shifts() {
        let mut r = base(Model::Newtonian);
        r.shift = Shift::Arrhenius {
            e: 2000.0,
            t_ref: 300.0,
        };
        assert!((r.viscosity(1.0, 300.0) - 1.0).abs() < 1e-14);
        let t = 320.0;
        assert!((r.viscosity(1.0, t) - (2000.0f64 * (1.0 / t - 1.0 / 300.0)).exp()).abs() < 1e-14);
        assert!(r.viscosity(1.0, 350.0) < r.viscosity(1.0, 320.0));
        r.shift = Shift::Wlf {
            c1: 17.44,
            c2: 51.6,
            t_ref: 300.0,
        };
        assert!((r.viscosity(1.0, 300.0) - 1.0).abs() < 1e-14);
        assert!(r.viscosity(1.0, 310.0) < 1.0 && r.viscosity(1.0, 290.0) > 1.0);
        // power law under a shift: mu(g, T) = a_T K (a_T g)^(n-1) = K a_T^n g^(n-1)
        let mut p = base(Model::PowerLaw);
        p.shift = Shift::Arrhenius {
            e: 1500.0,
            t_ref: 300.0,
        };
        let at = p.shift_factor(330.0);
        assert!((p.viscosity(7.0, 330.0) - 0.5 * at.powf(0.6) * 7f64.powf(-0.4)).abs() < 1e-12);
    }
}
