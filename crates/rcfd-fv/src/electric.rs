//! Steady electric conduction `div(sigma grad phi) = 0` and Joule heating (F-213, ADR 0035 D2).
//!
//! The potential is solved with the shared scalar sweep (zero convective flux, steady) on the face conductivity
//! `sigma_f`, the distance-weighted harmonic mean of the cell values (zero when either cell is an insulator). Insulating
//! cells (`sigma = 0`) are held at `phi = 0` and decouple. The Joule heat is the face dissipation of the discrete
//! operator, so that on an orthogonal mesh the total heat equals the electric power `-sum_b F_b phi_b` exactly. With the
//! gradient options the `_opt` functions use the currents of the solved operator, and the Joule heat the full face power, so
//! that for a converged potential the total equals the power on any mesh (ADR 0221).

use crate::cht::{conductive_face_flows, conductive_face_flows_opt};
use crate::convection::ConvScheme;
use crate::diffusion::GradientOptions;
use crate::field::ScalarField;
use crate::geometry::FvGeometry;
use crate::scalar::{sweep_ext, SweepExtras, SweepInputs};
use rcfd_linalg::SolveControl;

/// Conductivity of a linear-resistivity conductor `sigma_0 / (1 + a (T - T_ref))`; `None` when the denominator is not
/// positive (the resistivity would be zero or negative).
pub fn sigma_of_t(sigma0: f64, a: f64, t_ref: f64, t: f64) -> Option<f64> {
    let d = 1.0 + a * (t - t_ref);
    (d > 0.0 && d.is_finite()).then(|| sigma0 / d)
}

/// Face conductivity (S/m): distance-weighted harmonic mean on internal faces, the owner value on boundary faces; zero as
/// soon as one of the two cells does not conduct.
pub fn face_conductivity(geom: &FvGeometry, sigma: &[f64]) -> Vec<f64> {
    let own = geom.mesh.owner();
    let nei = geom.mesh.neighbour();
    (0..geom.mesh.n_faces())
        .map(|f| {
            if f < geom.n_internal {
                let (so, sn) = (sigma[own[f]], sigma[nei[f]]);
                if so <= 0.0 || sn <= 0.0 {
                    0.0
                } else {
                    let w = geom.weights[f];
                    1.0 / ((1.0 - w) / so + w / sn)
                }
            } else {
                sigma[own[f]]
            }
        })
        .collect()
}

/// Solve the potential in place for the cell conductivities `sigma` (boundary conditions already set on `phi`). Repeats
/// the sweep (needed only for the non-orthogonal correction) until the largest change falls below `1e-12` of the potential
/// range or `max_sweeps` is reached. Returns the face conductivity used and the number of sweeps.
pub fn solve(
    geom: &FvGeometry,
    phi: &mut ScalarField,
    sigma: &[f64],
    ctl: &SolveControl,
    max_sweeps: usize,
) -> (Vec<f64>, usize) {
    solve_with_source(geom, phi, sigma, None, false, ctl, max_sweeps)
}

/// [`solve`] with the gradient options on the explicit non-orthogonal flux of the potential's diffusion term (ADR 0204 K3, case
/// value `solver.gradient_equations.electric_potential`); `None` is [`solve`].
pub fn solve_opt(
    geom: &FvGeometry,
    phi: &mut ScalarField,
    sigma: &[f64],
    ctl: &SolveControl,
    max_sweeps: usize,
    gradient: Option<GradientOptions>,
) -> (Vec<f64>, usize) {
    solve_with_source_opt(geom, phi, sigma, None, false, ctl, max_sweeps, gradient)
}

/// [`solve`] with a volumetric source: `-div(sigma_f grad phi) = source` per cell (A/m^3; ADR 0039 D1: the divergence of the
/// motional current `sigma u x B`). With `pin_reference` the first conducting cell is held at `phi = 0`, for a problem
/// without any fixed-potential boundary (the potential is then defined up to a constant).
pub fn solve_with_source(
    geom: &FvGeometry,
    phi: &mut ScalarField,
    sigma: &[f64],
    source: Option<&[f64]>,
    pin_reference: bool,
    ctl: &SolveControl,
    max_sweeps: usize,
) -> (Vec<f64>, usize) {
    solve_with_source_opt(geom, phi, sigma, source, pin_reference, ctl, max_sweeps, None)
}

/// [`solve_with_source`] with the gradient options of [`solve_opt`] (ADR 0204 K3); `None` is [`solve_with_source`]. The sweep
/// always computes the cell gradient (`LinearUpwind` on a zero flux), so the options act on every non-orthogonal face.
#[allow(clippy::too_many_arguments)]
pub fn solve_with_source_opt(
    geom: &FvGeometry,
    phi: &mut ScalarField,
    sigma: &[f64],
    source: Option<&[f64]>,
    pin_reference: bool,
    ctl: &SolveControl,
    max_sweeps: usize,
    gradient: Option<GradientOptions>,
) -> (Vec<f64>, usize) {
    let sf = face_conductivity(geom, sigma);
    let mut fixed: Vec<Option<f64>> = sigma.iter().map(|&s| (s <= 0.0).then_some(0.0)).collect();
    if pin_reference {
        if let Some(c) = sigma.iter().position(|&s| s > 0.0) {
            fixed[c] = Some(0.0);
        }
    }
    let any_fixed = fixed.iter().any(|f| f.is_some());
    let zero = vec![0.0; geom.mesh.n_faces()];
    let mut sweeps = 0;
    for _ in 0..max_sweeps.max(1) {
        sweeps += 1;
        let before = phi.values.clone();
        sweep_ext(
            geom,
            phi,
            &SweepInputs {
                phi: &zero,
                gamma: &sf,
                scheme: ConvScheme::LinearUpwind,
                relax: 1.0,
                source,
                solver: ctl,
                ddt: None,
            },
            &SweepExtras {
                fixed: any_fixed.then_some(fixed.as_slice()),
                gradient,
                ..Default::default()
            },
        );
        let range = phi.values.iter().fold(0.0_f64, |m, v| m.max(v.abs())).max(1e-300);
        let change = before
            .iter()
            .zip(&phi.values)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        if change <= 1e-12 * range {
            break;
        }
    }
    (sf, sweeps)
}

/// Current leaving each cell through every face, `-sigma_f A dphi/dn` (A), the same operator as the solver.
pub fn face_currents(geom: &FvGeometry, phi: &ScalarField, sigma_face: &[f64], non_orthogonal: bool) -> Vec<f64> {
    conductive_face_flows(geom, phi, sigma_face, non_orthogonal)
}

/// Joule heat per cell, W/m^3: the dissipation `G (dphi)^2` of the internal faces shared half and half between their two
/// cells, and `F_b (phi_P - phi_b)` of each boundary face in its owner cell (`F_b` the current leaving the domain).
pub fn joule_heat(geom: &FvGeometry, phi: &ScalarField, sigma_face: &[f64], non_orthogonal: bool) -> Vec<f64> {
    let own = geom.mesh.owner();
    let nei = geom.mesh.neighbour();
    let vol = geom.mesh.cell_volumes();
    let mut q = vec![0.0; geom.n_cells()];
    for f in 0..geom.n_internal {
        let d = phi.values[own[f]] - phi.values[nei[f]];
        let p = 0.5 * sigma_face[f] * geom.lap_coeff[f] * d * d;
        q[own[f]] += p;
        q[nei[f]] += p;
    }
    let flows = face_currents(geom, phi, sigma_face, non_orthogonal);
    let bc = phi.boundary_coeffs(geom);
    for f in geom.n_internal..geom.mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let co = &bc[f - geom.n_internal];
        let pp = phi.values[own[f]];
        q[own[f]] += flows[f] * (pp - co.value(pp));
    }
    for (x, v) in q.iter_mut().zip(vol) {
        *x /= v;
    }
    q
}

/// Total electric power delivered by the boundaries, `-sum_b F_b phi_b` (W), the quantity the Joule heat must add up to.
pub fn boundary_power(geom: &FvGeometry, phi: &ScalarField, sigma_face: &[f64], non_orthogonal: bool) -> f64 {
    let flows = face_currents(geom, phi, sigma_face, non_orthogonal);
    power_of_flows(geom, phi, &flows)
}

/// [`face_currents`] of a potential solved with the gradient options `opts` ([`solve_opt`]): the face fluxes of the solved
/// operator ([`crate::cht::conductive_face_flows_opt`], ADR 0221 D1). `None` is [`face_currents`].
pub fn face_currents_opt(
    geom: &FvGeometry,
    phi: &ScalarField,
    sigma_face: &[f64],
    non_orthogonal: bool,
    opts: Option<GradientOptions>,
) -> Vec<f64> {
    conductive_face_flows_opt(geom, phi, sigma_face, non_orthogonal, opts)
}

/// [`joule_heat`] of a potential solved with the gradient options (ADR 0221 D1). `None` is [`joule_heat`]. With `Some`, `F` the
/// currents of [`face_currents_opt`]: each internal face gives half of its power `F_f (phi_o - phi_n)` to each of its cells
/// and each boundary face `F_b (phi_P - phi_b)` to its owner, so the total is the boundary power plus `sum_c phi_c net_c`
/// (`net_c` the current leaving cell `c`), exactly; for a converged potential it equals [`boundary_power_opt`]. The face power is
/// not a square, so on a strongly skewed mesh a cell value can be negative. `Some(GradientOptions::default())` is
/// [`joule_heat`], bit for bit (ADR 0221 amendment 2).
pub fn joule_heat_opt(
    geom: &FvGeometry,
    phi: &ScalarField,
    sigma_face: &[f64],
    non_orthogonal: bool,
    opts: Option<GradientOptions>,
) -> Vec<f64> {
    if opts.unwrap_or_default() == GradientOptions::default() {
        return joule_heat(geom, phi, sigma_face, non_orthogonal);
    }
    let own = geom.mesh.owner();
    let nei = geom.mesh.neighbour();
    let vol = geom.mesh.cell_volumes();
    let flows = face_currents_opt(geom, phi, sigma_face, non_orthogonal, opts);
    let mut q = vec![0.0; geom.n_cells()];
    for f in 0..geom.n_internal {
        let p = 0.5 * flows[f] * (phi.values[own[f]] - phi.values[nei[f]]);
        q[own[f]] += p;
        q[nei[f]] += p;
    }
    let bc = phi.boundary_coeffs(geom);
    for f in geom.n_internal..geom.mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let co = &bc[f - geom.n_internal];
        let pp = phi.values[own[f]];
        q[own[f]] += flows[f] * (pp - co.value(pp));
    }
    for (x, v) in q.iter_mut().zip(vol) {
        *x /= v;
    }
    q
}

/// [`boundary_power`] with the currents of [`face_currents_opt`] (ADR 0221 D1); `None` and `Some(GradientOptions::default())` are
/// [`boundary_power`].
pub fn boundary_power_opt(
    geom: &FvGeometry,
    phi: &ScalarField,
    sigma_face: &[f64],
    non_orthogonal: bool,
    opts: Option<GradientOptions>,
) -> f64 {
    if opts.unwrap_or_default() == GradientOptions::default() {
        return boundary_power(geom, phi, sigma_face, non_orthogonal);
    }
    let flows = face_currents_opt(geom, phi, sigma_face, non_orthogonal, opts);
    power_of_flows(geom, phi, &flows)
}

/// `-sum_b F_b phi_b` over the non-empty boundary faces.
fn power_of_flows(geom: &FvGeometry, phi: &ScalarField, flows: &[f64]) -> f64 {
    let bc = phi.boundary_coeffs(geom);
    let own = geom.mesh.owner();
    let mut p = 0.0;
    for f in geom.n_internal..geom.mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let co = &bc[f - geom.n_internal];
        p -= flows[f] * co.value(phi.values[own[f]]);
    }
    p
}
