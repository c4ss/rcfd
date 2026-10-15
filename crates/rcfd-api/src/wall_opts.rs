//! Near-wall options of the turbulence models in the settings tree (F-203, F-043, ADR 0015):
//! `turbulence.wall_function` (`standard`, `scalable`, `enhanced`), `turbulence.roughness_height`,
//! `turbulence.roughness_constant`, `turbulence.wall_omega` (`automatic`, `low_re`) and `turbulence.sst_variant`
//! (`sst`, `bsl`).
//!
//! [`check`] validates the keys (called from `Case::validate`); [`WallOptions::from_case`] reads them and
//! [`apply_kepsilon`] / [`apply_sst`] put them on a library model. The turbulence model of a running simulation is a
//! `Box<dyn TurbulenceModel>` built in `turb.rs`; that trait has no way to reach the concrete `KEpsilon` /
//! `KOmegaSst`, so the build path calls [`apply_kepsilon`] / [`apply_sst`] only once `turb.rs` and `model.rs`
//! provide a hook (see [`BUILD_HOOK_INSTALLED`]). Until then [`check`] rejects every non-default value with a located
//! error instead of silently ignoring it.

use crate::case::{Case, Diagnostic};
use crate::sim::{get_f, get_s};
use rcfd_turb::kepsilon::WallFunction;
use rcfd_turb::sst::{SstVariant, SstWallMode};
use rcfd_turb::wall::Roughness;
use rcfd_turb::{KEpsilon, KOmegaSst};

/// Whether the simulation build path applies [`WallOptions`] to the model it constructs. It needs a trait method on
/// `rcfd_turb::TurbulenceModel` (for example `fn apply_wall_options(&mut self, o: &WallOptions) -> Result<(), String>`
/// implemented by `KEpsilonModel` and `SstModel` with [`apply_kepsilon`] / [`apply_sst`]) and one call in
/// `TurbState::new` after `create_model`. `false` while those two files are owned by the plugin work.
pub const BUILD_HOOK_INSTALLED: bool = false;

/// The wall options of a case.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallOptions {
    /// k-epsilon wall treatment.
    pub wall_function: WallFunction,
    /// Sand-grain roughness height (0: smooth).
    pub roughness_height: f64,
    /// Roughness constant `C_s` (0: derived from the smooth log law, the Nikuradse value).
    pub roughness_constant: f64,
    /// SST wall omega treatment.
    pub wall_omega: SstWallMode,
    /// Menter variant.
    pub sst_variant: SstVariant,
}

impl WallOptions {
    /// Read the options (schema defaults when absent).
    pub fn from_case(case: &Case) -> Self {
        WallOptions {
            wall_function: match get_s(case, "turbulence.wall_function").as_str() {
                "scalable" => WallFunction::Scalable,
                "enhanced" => WallFunction::Enhanced,
                _ => WallFunction::Standard,
            },
            roughness_height: get_f(case, "turbulence.roughness_height"),
            roughness_constant: get_f(case, "turbulence.roughness_constant"),
            wall_omega: match get_s(case, "turbulence.wall_omega").as_str() {
                "low_re" => SstWallMode::LowRe,
                _ => SstWallMode::Automatic,
            },
            sst_variant: match get_s(case, "turbulence.sst_variant").as_str() {
                "bsl" => SstVariant::Bsl,
                _ => SstVariant::Sst,
            },
        }
    }

    /// Roughness for a log law with the given constants.
    pub fn roughness(&self, kappa: f64, e: f64) -> Roughness {
        if self.roughness_height > 0.0 {
            if self.roughness_constant > 0.0 {
                Roughness {
                    ks: self.roughness_height,
                    cs: self.roughness_constant,
                }
            } else {
                Roughness::new(self.roughness_height, kappa, e)
            }
        } else {
            Roughness::SMOOTH
        }
    }
}

/// Put the options on a k-epsilon model (uses its own log-law constants for the derived roughness constant).
pub fn apply_kepsilon(o: &WallOptions, m: &mut KEpsilon) {
    m.wall_function = o.wall_function;
    m.roughness = o.roughness(m.wc.kappa, m.wc.e);
}

/// Put the options on a k-omega SST model.
pub fn apply_sst(o: &WallOptions, m: &mut KOmegaSst) -> rcfd_core::Result<()> {
    m.variant = o.sst_variant;
    m.roughness = o.roughness(m.law.kappa, m.law.e);
    m.set_wall_mode(o.wall_omega)
}

fn diag(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.to_string(),
        line: None,
        message: message.into(),
    }
}

/// Validation of the wall options (located errors).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let model = get_s(case, "turbulence.model");
    let (ke, sst) = (model == "k_epsilon", model == "k_omega_sst");
    let o = WallOptions::from_case(case);
    let non_default = |k: &str, label: &str, default: &str, out: &mut Vec<Diagnostic>, ok: bool, only: &str| {
        let v = get_s(case, &format!("turbulence.{k}"));
        if v != default && !ok {
            out.push(diag(
                &format!("turbulence.{k}"),
                format!("turbulence.{k} = \"{v}\" ({label}) applies to {only} only, not to {model}"),
            ));
        }
    };
    non_default("wall_function", "wall function", "standard", &mut out, ke, "k_epsilon");
    non_default(
        "wall_omega",
        "SST wall omega",
        "automatic",
        &mut out,
        sst,
        "k_omega_sst",
    );
    non_default("sst_variant", "SST variant", "sst", &mut out, sst, "k_omega_sst");
    if o.roughness_height < 0.0 || !o.roughness_height.is_finite() {
        out.push(diag(
            "turbulence.roughness_height",
            format!("turbulence.roughness_height = {} must be >= 0", o.roughness_height),
        ));
    }
    if o.roughness_constant < 0.0 || !o.roughness_constant.is_finite() {
        out.push(diag(
            "turbulence.roughness_constant",
            format!(
                "turbulence.roughness_constant = {} must be > 0 (0 derives the Nikuradse value)",
                o.roughness_constant
            ),
        ));
    }
    if o.roughness_height > 0.0 {
        if !(ke || sst) {
            out.push(diag(
                "turbulence.roughness_height",
                format!("wall roughness is available for k_epsilon and k_omega_sst only, not for {model}"),
            ));
        } else if ke && o.wall_function == WallFunction::Enhanced {
            out.push(diag(
                "turbulence.roughness_height",
                "wall roughness is not available with turbulence.wall_function = \"enhanced\" (the blended law has no rough form here); use \"standard\" or \"scalable\"",
            ));
        } else if sst && o.wall_omega == SstWallMode::LowRe {
            out.push(diag(
                "turbulence.roughness_height",
                "wall roughness needs turbulence.wall_omega = \"automatic\" (the low-Re wall condition has no rough form here)",
            ));
        }
    } else if o.roughness_constant > 0.0 {
        out.push(diag(
            "turbulence.roughness_constant",
            "turbulence.roughness_constant has no effect without turbulence.roughness_height > 0",
        ));
    }
    if !BUILD_HOOK_INSTALLED {
        let pending = [
            ("wall_function", o.wall_function != WallFunction::Standard),
            ("roughness_height", o.roughness_height > 0.0),
            ("wall_omega", o.wall_omega != SstWallMode::Automatic),
            ("sst_variant", o.sst_variant != SstVariant::Sst),
        ];
        for (k, set) in pending {
            if set && !out.iter().any(|d| d.path == format!("turbulence.{k}")) {
                out.push(diag(
                    &format!("turbulence.{k}"),
                    format!(
                        "turbulence.{k} is implemented in the rcfd-turb library but the simulation build path does not apply it yet (docs/adr/0015-turbulence-finishing.md, check C5); the value is rejected rather than ignored"
                    ),
                ));
            }
        }
    }
    out
}
