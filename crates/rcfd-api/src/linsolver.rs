//! The `linear_solver` section (ADR 0038): selects the device of the pressure linear solve. `device = "cpu"` (the
//! default and the unset value) leaves the CPU path bit-identical to before this section existed.

use crate::case::{Case, Diagnostic};
use crate::sim::{get_i, get_s};
use rcfd_core::settings::Schema;
use rcfd_linalg::{GpuPrecond, LinearDevice, LinearOptions};

/// Algorithms whose pressure equation goes through the segregated pressure solve that supports a device.
const SUPPORTED_ALGORITHMS: [&str; 4] = ["simple", "simplec", "piso", "pimple"];

fn d(path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// Schema of the `linear_solver` section.
pub fn schema() -> Schema {
    Schema::object()
        .doc("Linear-solver device for the pressure equation (ADR 0038). cpu is the unchanged default path")
        .field(
            "device",
            Schema::enumeration("cpu", &["cpu", "cuda", "auto"]).doc("cpu (default), cuda (CUDA device; a located error if none is usable) or auto (the device when one is usable and the system has at least 100000 unknowns, otherwise the CPU path). With a device the pressure CG uses the device preconditioner below instead of DIC/AMG; only for simple, simplec, piso and pimple with constant density"),
        )
        .field(
            "preconditioner",
            Schema::enumeration("jacobi", &["jacobi", "chebyshev"]).doc("Device preconditioner: jacobi (inverse diagonal) or chebyshev (polynomial of the Jacobi-scaled operator)"),
        )
        .field(
            "chebyshev_degree",
            Schema::int(0).range(0.0, 32.0).doc("Chebyshev terms (degree - 1 matrix products per application); 0 = unset = 4"),
        )
}

/// The linear-solver options of a case.
pub fn options(case: &Case) -> LinearOptions {
    LinearOptions {
        device: LinearDevice::parse(&get_s(case, "linear_solver.device")).unwrap_or_default(),
        precond: match get_s(case, "linear_solver.preconditioner").as_str() {
            "chebyshev" => GpuPrecond::Chebyshev,
            _ => GpuPrecond::Jacobi,
        },
        chebyshev_degree: get_i(case, "linear_solver.chebyshev_degree").max(0) as usize,
    }
}

/// Static checks of the section.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let o = options(case);
    if o.device == LinearDevice::Cpu {
        return out;
    }
    let alg = get_s(case, "solver.algorithm");
    if !SUPPORTED_ALGORITHMS.contains(&alg.as_str()) {
        out.push(d(
            "linear_solver.device",
            format!(
                "device = \"{}\" is supported for solver.algorithm simple, simplec, piso and pimple, not \"{alg}\"",
                o.device.as_str()
            ),
        ));
    }
    if get_s(case, "fluid.density") != "constant" {
        out.push(d(
            "linear_solver.device",
            format!(
                "device = \"{}\" is supported for constant density only, not fluid.density = \"{}\"",
                o.device.as_str(),
                get_s(case, "fluid.density")
            ),
        ));
    }
    if o.device == LinearDevice::Cuda {
        if let Err(e) = rcfd_gpu::resolve(LinearDevice::Cuda, 0) {
            out.push(d("linear_solver.device", e.message));
        }
    }
    out
}
