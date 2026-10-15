//! Near-wall options of the turbulence models in the settings tree (F-203, F-043, ADR 0015):
//! `turbulence.wall_function` (`standard`, `scalable`, `enhanced`), `turbulence.roughness_height`,
//! `turbulence.roughness_constant`, `turbulence.wall_omega` (`automatic`, `low_re`) and `turbulence.sst_variant`
//! (`sst`, `bsl`).
//!
//! [`check`] validates the keys (called from `Case::validate`); [`WallOptions::from_case`] reads them and
//! [`apply_kepsilon`] / [`apply_sst`] put them on a library model. The turbulence model of a running simulation is a
//! `Box<dyn TurbulenceModel>` built in `turb.rs`; `TurbState::new` hands the options to it through
//! `TurbulenceModel::apply_wall_options` (see [`BUILD_HOOK_INSTALLED`]).

use crate::case::{Case, Diagnostic};
use crate::sim::{get_f, get_s};
use rcfd_turb::kepsilon::WallFunction;
use rcfd_turb::sst::{SstVariant, SstWallMode};

/// Whether the simulation build path applies [`WallOptions`] to the model it constructs: it does, through
/// `TurbulenceModel::apply_wall_options`, called once in `TurbState::new` after `create_model`.
pub const BUILD_HOOK_INSTALLED: bool = true;

pub use rcfd_turb::model::{apply_kepsilon, apply_sst, WallOptions};

/// Reading the wall options from a case (the struct itself lives in `rcfd-turb`, which `rcfd-api` depends on).
pub trait WallOptionsFromCase {
    /// Read the options (schema defaults when absent).
    fn from_case(case: &Case) -> Self;
}

impl WallOptionsFromCase for WallOptions {
    fn from_case(case: &Case) -> Self {
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
    out
}
