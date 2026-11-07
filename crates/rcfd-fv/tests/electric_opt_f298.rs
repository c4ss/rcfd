//! F-298 (ADR 0221) part (a), library: the electric current and the Joule heat of a potential solved with the gradient options.
//! Limits are those registered in
//! docs/adr/0221-f298-registration-electric-current-and-joule-heat-with-the-gradient-options-and-the-fixed-pressure-flux-correction.md
//! before this file was written.
//!
//! Set-up KE: ADR 0016's skewed mesh K, a varying conductivity, fixed potentials on two patches, a current patch and an arbitrary
//! potential with local extrema. P: K reaches 25 degrees. L1: the currents of `face_currents_opt` are the face fluxes of the
//! potential's operator with the same options (per cell to 1e-12 of the flux scale); L1-T: the unchanged currents miss that.
//! L2: the total of `joule_heat_opt` equals `boundary_power_opt` plus `sum phi_c net_c` (to 1e-12 of the power scale); L2-T: the
//! default formula misses that identity with its own currents. L3: `None` (and the default options for the currents and the power)
//! give the bits of the existing functions; L3b (amendment 2): the default options for the Joule heat too. L4: without a
//! gradient the new Joule formula is the old one to round-off.

use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::{FixedGradient, Profile};
use rcfd_fv::diffusion::{laplacian_opt, BoundaryGradient, GradientOptions};
use rcfd_fv::electric::{
    boundary_power, boundary_power_opt, face_conductivity, face_currents, face_currents_opt, joule_heat, joule_heat_opt,
};
use rcfd_fv::gradient::{least_squares, Limiter};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField};
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use std::sync::Arc;

/// L1 limit, relative to the flux scale `S`.
const IDENTITY: f64 = 1e-12;
/// L2 limit, relative to the power scale `W`.
const POWER: f64 = 1e-12;
/// L1-T and L2-T: the smallest miss of the unchanged quantities.
const TEETH: f64 = 1e-9;
/// L4 limit, relative to the largest cell heat.
const ROUND_OFF: f64 = 1e-12;

/// ADR 0016 mesh K at `n`: `x' = x + 0.7 y + 0.3 z`, `y' = y + 0.3 z`.
fn mesh_k(n: usize) -> Arc<FvGeometry> {
    let m = warp(&box_mesh([n; 3], [1.0; 3], [Grading::Uniform; 3]).unwrap(), |p| {
        [p[0] + 0.7 * p[1] + 0.3 * p[2], p[1] + 0.3 * p[2], p[2]]
    })
    .unwrap();
    Arc::new(FvGeometry::new(Arc::new(m)).unwrap())
}

/// ADR 0190's option sets O1 to O6 and C2.
fn sets() -> Vec<(&'static str, GradientOptions)> {
    let d = GradientOptions::default();
    let venk = |k| Some(Limiter::Venkatakrishnan(k));
    vec![
        (
            "O1",
            GradientOptions {
                face_correction: true,
                ..d
            },
        ),
        (
            "O2",
            GradientOptions {
                boundary: BoundaryGradient::Corrected,
                ..d
            },
        ),
        (
            "O3",
            GradientOptions {
                boundary: BoundaryGradient::Dropped,
                ..d
            },
        ),
        (
            "O4",
            GradientOptions {
                secondary_limiter: Some(Limiter::BarthJespersen),
                ..d
            },
        ),
        (
            "O5",
            GradientOptions {
                secondary_limiter: venk(5.0),
                ..d
            },
        ),
        (
            "O6",
            GradientOptions {
                secondary_limiter: venk(0.5),
                ..d
            },
        ),
        (
            "C2",
            GradientOptions {
                face_correction: true,
                secondary_limiter: venk(5.0),
                boundary: BoundaryGradient::Corrected,
            },
        ),
    ]
}

fn f(p: Vec3) -> f64 {
    p[0] + (1.3 * p[0] + 0.4).sin() * (1.1 * p[1]).cos() * (0.9 * p[2] + 0.2).cos()
}

/// Set-up KE: the geometry, the potential (boundary conditions and cell values) and the face conductivity.
fn setup() -> (Arc<FvGeometry>, ScalarField, Vec<f64>) {
    let geom = mesh_k(8);
    let q = rcfd_mesh::quality::check_mesh(&geom.mesh);
    println!(
        "P: mesh K (n = 8) max non-orthogonality {:.3} degrees (at least 25)",
        q.max_non_orthogonality
    );
    assert!(q.max_non_orthogonality >= 25.0, "P set-up: {}", q.max_non_orthogonality);
    let cc = geom.mesh.cell_centres();
    let fc = geom.mesh.face_centres();
    let sigma: Vec<f64> = cc
        .iter()
        .map(|x| 1.0 + 0.5 * (1.7 * x[0] + 0.3).sin() * (0.8 * x[1] - 0.2 * x[2]).cos())
        .collect();
    let sf = face_conductivity(&geom, &sigma);
    let mut phi = ScalarField::uniform("phi", &geom, 0.0);
    for name in ["xmin", "xmax"] {
        let vals = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|face| [f(fc[face]), 0.0, 0.0])
            .collect();
        phi.set_bc(&geom, name, Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
    }
    phi.set_bc(&geom, "ymin", Box::new(FixedGradient(Profile::scalar(0.3))))
        .unwrap();
    for (c, v) in phi.values.iter_mut().enumerate() {
        *v = f(cc[c]) + if c % 2 == 0 { 0.05 } else { -0.05 };
    }
    (geom, phi, sf)
}

/// The current leaving every cell through its faces.
fn net(geom: &FvGeometry, flows: &[f64]) -> Vec<f64> {
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let mut out = vec![0.0; geom.n_cells()];
    for f in 0..geom.mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        out[own[f]] += flows[f];
        if f < geom.n_internal {
            out[nei[f]] -= flows[f];
        }
    }
    out
}

/// `S = max_c sum_{f in c} |F_f|`.
fn flux_scale(geom: &FvGeometry, flows: &[f64]) -> f64 {
    (0..geom.n_cells())
        .map(|c| {
            geom.mesh
                .cell_faces(c)
                .iter()
                .filter(|&&f| !geom.empty[f])
                .map(|&f| flows[f].abs())
                .sum::<f64>()
        })
        .fold(0.0, f64::max)
}

/// `W = sum_int |F_f| (|phi_o| + |phi_n|) + sum_b |F_b| (|phi_P| + |phi_b|)`.
fn power_scale(geom: &FvGeometry, phi: &ScalarField, flows: &[f64]) -> f64 {
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let bv = phi.boundary_values(geom);
    let mut w = 0.0;
    for f in 0..geom.mesh.n_faces() {
        if geom.empty[f] {
            continue;
        }
        let other = if f < geom.n_internal {
            phi.values[nei[f]].abs()
        } else {
            bv[f - geom.n_internal].abs()
        };
        w += flows[f].abs() * (phi.values[own[f]].abs() + other);
    }
    w
}

/// L1's per-cell miss `|net_c - ((A phi)_c - b_c)|` of the currents `flows` against the operator with `op`.
fn identity(geom: &FvGeometry, phi: &ScalarField, sf: &[f64], op: Option<GradientOptions>, flows: &[f64]) -> f64 {
    let bv = phi.boundary_values(geom);
    let grad = least_squares(geom, &phi.values, &bv);
    let m = laplacian_opt(geom, sf, &phi.boundary_coeffs(geom), Some(&grad), &phi.values, &bv, op);
    let r = m.residual(&phi.values); // b - A phi
    net(geom, flows)
        .iter()
        .zip(&r)
        .map(|(n, r)| (n + r).abs())
        .fold(0.0, f64::max)
}

/// L2's quantity `|sum q V - P - sum phi_c net_c|` for the cell heat `q`, the power `p` and the currents `flows`.
fn power_miss(geom: &FvGeometry, phi: &ScalarField, q: &[f64], p: f64, flows: &[f64]) -> f64 {
    let vol = geom.mesh.cell_volumes();
    let total: f64 = q.iter().zip(vol).map(|(q, v)| q * v).sum();
    let pn: f64 = phi.values.iter().zip(net(geom, flows)).map(|(a, b)| a * b).sum();
    (total - p - pn).abs()
}

/// L1, L1-T, L2 and L2-T on KE.
#[test]
fn l1_l2_currents_and_joule_heat_follow_the_solved_operator() {
    let (geom, phi, sf) = setup();
    let mut missed = Vec::new();
    // L1 for the default
    let f0 = face_currents_opt(&geom, &phi, &sf, true, None);
    let s0 = flux_scale(&geom, &f0);
    let m0 = identity(&geom, &phi, &sf, None, &f0);
    println!(
        "L1 default: max miss {m0:.3e}, S = {s0:.4}, ratio {:.3e} (limit {IDENTITY:e})",
        m0 / s0
    );
    if m0 > IDENTITY * s0 {
        missed.push(format!("L1 default: {:e}", m0 / s0));
    }
    // L2-T: the default formula with its own currents
    let q0 = joule_heat(&geom, &phi, &sf, true);
    let p0 = boundary_power(&geom, &phi, &sf, true);
    let fo = face_currents(&geom, &phi, &sf, true);
    let w0 = power_scale(&geom, &phi, &fo);
    let t0 = power_miss(&geom, &phi, &q0, p0, &fo);
    println!(
        "L2-T default: |sum qV - P - sum phi net| = {t0:.3e}, W = {w0:.4}, ratio {:.3e} (limit > {TEETH:e})",
        t0 / w0
    );
    if t0 <= TEETH * w0 {
        missed.push(format!("L2-T default: {:e}", t0 / w0));
    }
    for (name, o) in sets() {
        let fl = face_currents_opt(&geom, &phi, &sf, true, Some(o));
        let s = flux_scale(&geom, &fl);
        let m = identity(&geom, &phi, &sf, Some(o), &fl);
        let mt = identity(&geom, &phi, &sf, Some(o), &fo);
        println!(
            "L1 {name}: ratio {:.3e} (limit {IDENTITY:e}); L1-T {name}: the unchanged currents miss by {:.3e} of S (limit > {TEETH:e})",
            m / s,
            mt / s
        );
        if m > IDENTITY * s {
            missed.push(format!("L1 {name}: {:e}", m / s));
        }
        if mt <= TEETH * s {
            missed.push(format!("L1-T {name}: {:e}", mt / s));
        }
        let q = joule_heat_opt(&geom, &phi, &sf, true, Some(o));
        let p = boundary_power_opt(&geom, &phi, &sf, true, Some(o));
        let w = power_scale(&geom, &phi, &fl);
        let pm = power_miss(&geom, &phi, &q, p, &fl);
        let qmin = q.iter().copied().fold(f64::INFINITY, f64::min);
        println!(
            "L2 {name}: |sum qV - P - sum phi net| = {pm:.3e}, W = {w:.4}, ratio {:.3e} (limit {POWER:e}); P = {p:.6e}; smallest cell heat {qmin:.3e}",
            pm / w
        );
        if pm > POWER * w {
            missed.push(format!("L2 {name}: {:e}", pm / w));
        }
    }
    assert!(missed.is_empty(), "{missed:?}");
}

/// L3: `None` and (for the currents and the power) the default options give the bits of the existing functions; L4: without a
/// gradient the new Joule formula is the old one to round-off.
#[test]
fn l3_l4_no_options_change_no_bit() {
    let (geom, phi, sf) = setup();
    let bits = |a: &[f64], b: &[f64]| a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    let mut missed = Vec::new();
    for non_orth in [true, false] {
        let base_f = face_currents(&geom, &phi, &sf, non_orth);
        let base_q = joule_heat(&geom, &phi, &sf, non_orth);
        let base_p = boundary_power(&geom, &phi, &sf, non_orth);
        for (label, o) in [("None", None), ("default", Some(GradientOptions::default()))] {
            let n = bits(&base_f, &face_currents_opt(&geom, &phi, &sf, non_orth, o));
            let pw = boundary_power_opt(&geom, &phi, &sf, non_orth, o);
            println!(
                "L3 currents {label}, gradient {non_orth}: {n} of {} faces differ in a bit; power bits equal {}",
                base_f.len(),
                pw.to_bits() == base_p.to_bits()
            );
            if n != 0 || pw.to_bits() != base_p.to_bits() {
                missed.push(format!("L3 {label} {non_orth}"));
            }
        }
        let nq = bits(&base_q, &joule_heat_opt(&geom, &phi, &sf, non_orth, None));
        println!(
            "L3 joule heat None, gradient {non_orth}: {nq} of {} cells differ in a bit",
            base_q.len()
        );
        if nq != 0 {
            missed.push(format!("L3 joule {non_orth}"));
        }
        // L3b (ADR 0221 amendment 2): the default options are `joule_heat` too
        let nd = bits(
            &base_q,
            &joule_heat_opt(&geom, &phi, &sf, non_orth, Some(GradientOptions::default())),
        );
        println!(
            "L3b joule heat default options, gradient {non_orth}: {nd} of {} cells differ in a bit",
            base_q.len()
        );
        if nd != 0 {
            missed.push(format!("L3b joule {non_orth}"));
        }
    }
    let (_, c2) = sets().pop().unwrap();
    let base = face_currents(&geom, &phi, &sf, true);
    let moved = face_currents_opt(&geom, &phi, &sf, true, Some(c2))
        .iter()
        .zip(&base)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!("L3 control: C2 with a gradient moves a face current by {moved:.3e}");
    if moved <= 0.0 {
        missed.push("L3 control".into());
    }
    let q_old = joule_heat(&geom, &phi, &sf, false);
    let q_new = joule_heat_opt(&geom, &phi, &sf, false, Some(c2));
    let scale = q_old.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    let d = q_old.iter().zip(&q_new).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    println!(
        "L4: without a gradient the formulas differ by {d:.3e}, ratio {:.3e} of the largest heat (limit {ROUND_OFF:e})",
        d / scale
    );
    if d > ROUND_OFF * scale {
        missed.push(format!("L4: {:e}", d / scale));
    }
    assert!(missed.is_empty(), "{missed:?}");
}
