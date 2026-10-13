//! Multicomponent species transport in a mixing layer (F-053).
//!
//! Checks fixed before running: two streams (upper half and lower half of a 2-D channel) meet at
//! the inlet and mix by diffusion in a uniform flow; three species with different diffusivities,
//! the third one closed by difference.
//!
//! * conservation: for each transported species the net discrete boundary outflow (convection plus
//!   diffusion, from the boundary coefficients of the matrix) differs from zero by less than 1e-12
//!   of the sum of absolute boundary fluxes; the mass fractions sum to one in every cell to 1e-14;
//! * accuracy: the transported profiles at x = L/2 match the similarity solution
//!   `Y = Yb + (Yt - Yb)/2 (1 + erf((y - H/2) / (2 sqrt(D x / U))))` within 2 percent of the inlet jump
//!   of that species (the layer does not reach the walls).

use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::{FixedValue, Profile, ZeroGradient};
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::scalar::{ScalarTransport, SpeciesSet};
use rcfd_fv::{FvGeometry, ScalarField};
use rcfd_mesh::gen::{channel, Grading};
use std::sync::Arc;

const L: f64 = 1.0;
const H: f64 = 0.5;
const U: f64 = 1.0;

fn erf(x: f64) -> f64 {
    // Abramowitz and Stegun 7.1.26 is only 1.5e-7 accurate; use the series / continued fraction
    // via the complementary error function of high accuracy (W. J. Cody style rational is long),
    // here a Taylor series for |x| < 3 and the asymptotic limit beyond.
    if x.abs() > 4.0 {
        return x.signum();
    }
    let mut term = x;
    let mut sum = x;
    let mut n = 0.0;
    while term.abs() > 1e-17 {
        n += 1.0;
        term *= -x * x / n;
        sum += term / (2.0 * n + 1.0);
    }
    2.0 / std::f64::consts::PI.sqrt() * sum
}

struct Mixing {
    geom: Arc<FvGeometry>,
    set: SpeciesSet,
    phi: Vec<f64>,
}

/// (top fraction, bottom fraction, diffusivity) of the two transported species.
const SPECIES: [(f64, f64, f64); 2] = [(0.7, 0.0, 1e-3), (0.3, 0.2, 2e-3)];

fn setup(nx: usize, ny: usize) -> Mixing {
    let mesh = Arc::new(channel([nx, ny, 1], [L, H, 0.01], [Grading::Uniform; 3], true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let inlet = geom.mesh.patch("inlet").unwrap();
    let fc = geom.mesh.face_centres();
    let mut transported = Vec::new();
    for (k, &(top, bottom, d)) in SPECIES.iter().enumerate() {
        let vals: Vec<Vec3> = inlet
            .faces()
            .map(|f| [if fc[f][1] > 0.5 * H { top } else { bottom }, 0.0, 0.0])
            .collect();
        let mut y = ScalarField::uniform(&format!("Y{k}"), &geom, bottom);
        y.set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
        y.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
        y.set_bc(&geom, "walls", Box::new(ZeroGradient)).unwrap();
        let mut t = ScalarTransport::new(geom.clone(), y, d);
        t.convection = ConvScheme::LinearUpwind;
        transported.push(t);
    }
    let u = vec![[U, 0.0, 0.0]; geom.n_cells()];
    let ub = vec![[U, 0.0, 0.0]; geom.mesh.n_boundary_faces()];
    let phi = face_flux(&geom, &u, &ub);
    Mixing {
        geom,
        set: SpeciesSet {
            transported,
            names: vec!["A".into(), "B".into(), "C".into()],
        },
        phi,
    }
}

#[test]
fn mixing_layer_conserves_every_species_and_matches_the_similarity_solution() {
    let mut m = setup(200, 100);
    let mut sweeps = 0;
    loop {
        let res = m.set.iterate(&m.phi);
        sweeps += 1;
        if res.iter().all(|&r| r < 1e-12) || sweeps >= 300 {
            eprintln!("{sweeps} sweeps, residuals {res:?}");
            assert!(
                res.iter().all(|&r| r < 1e-12),
                "deferred-correction iteration did not converge"
            );
            break;
        }
    }
    for (s, name) in m.set.transported.iter().zip(&m.set.names) {
        let imb = s.imbalance(&m.phi);
        let scale = s.flux_scale(&m.phi);
        eprintln!(
            "{name}: imbalance {imb:.3e}, scale {scale:.3e}, relative {:.3e}",
            imb / scale
        );
        for f in s.boundary_flux(&m.phi) {
            eprintln!("   {:<7} conv {:+.6e} diff {:+.6e}", f.patch, f.convective, f.diffusive);
        }
        assert!(imb.abs() < 1e-12 * scale, "{name}: {imb} / {scale}");
    }
    let c = m.set.balance_fractions();
    let n = m.geom.n_cells();
    let sum_err = (0..n)
        .map(|i| {
            let s: f64 = m.set.transported.iter().map(|t| t.field.values[i]).sum::<f64>() + c[i];
            (s - 1.0).abs()
        })
        .fold(0.0, f64::max);
    assert!(sum_err < 1e-14, "sum of fractions off by {sum_err}");
    let bal = m.set.balance_outflow(&m.phi);
    eprintln!("balance species net outflow {bal:.3e}");
    assert!(bal.abs() < 1e-12 * m.set.transported[0].flux_scale(&m.phi));

    // profile at x = L/2
    let cc = m.geom.mesh.cell_centres();
    let dx = L / 200.0;
    for (k, &(top, bottom, d)) in SPECIES.iter().enumerate() {
        let jump = (top - bottom).abs();
        let mut worst: f64 = 0.0;
        for i in 0..n {
            if (cc[i][0] - (0.5 * L - 0.5 * dx)).abs() < 1e-9 {
                let x = cc[i][0];
                let exact =
                    bottom + (top - bottom) * 0.5 * (1.0 + erf((cc[i][1] - 0.5 * H) / (2.0 * (d * x / U).sqrt())));
                worst = worst.max((m.set.transported[k].field.values[i] - exact).abs() / jump);
            }
        }
        eprintln!("species {k}: max profile error {:.3}% of the jump", 100.0 * worst);
        assert!(worst < 0.02, "species {k}: {worst}");
    }
}
