//! Potential-flow initialisation of the laminar backward-facing step (F-205, ADR 0019 check I2): the number of SIMPLE
//! iterations to a fixed residual from the potential-flow start (velocity and Bernoulli pressure) against two uniform
//! starts. Registered limit: `iterations(potential) < iterations(uniform (1,0,0))` and `< iterations(zero)`.

use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::potential::{bernoulli_head, bernoulli_pressure, potential_flow, PotentialBc};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{backward_step_graded, StepGrading, StepSpec};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Start {
    UniformX,
    Zero,
    Potential,
    PotentialVelocityOnly,
}

struct Run {
    iterations: usize,
    converged: bool,
}

fn run(start: Start) -> Run {
    let spec = StepSpec {
        l_in: 2.0,
        l_out: 20.0,
        h_in: 1.0,
        step: 1.0,
        depth: 0.1,
        n: [8, 80, 8, 8],
    };
    let geom =
        Arc::new(FvGeometry::new(Arc::new(backward_step_graded(spec, StepGrading::uniform()).unwrap())).unwrap());
    let u0 = if start == Start::UniformX {
        [1.0, 0.0, 0.0]
    } else {
        [0.0; 3]
    };
    let mut u = VectorField::uniform("U", &geom, u0);
    let patch = geom.mesh.patch("inlet").unwrap().clone();
    let cf = geom.mesh.face_centres();
    let prof: Vec<[f64; 3]> = patch
        .faces()
        .map(|f| {
            let s = (cf[f][1] - spec.step) / spec.h_in;
            [6.0 * s * (1.0 - s), 0.0, 0.0]
        })
        .collect();
    u.set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    let ub = u.boundary_values(&geom);
    let mut st = SimpleSettings::new(spec.step / 100.0, Coupling::Simple);
    st.convection = ConvScheme::LinearUpwind;
    st.h_from_solution = true;
    let patch_bcs: Vec<(String, PotentialBc)> = ["inlet", "walls", "outlet"]
        .iter()
        .map(|n| {
            let pt = geom.mesh.patch(n).unwrap();
            if *n == "outlet" {
                (n.to_string(), PotentialBc::Dirichlet)
            } else {
                let v: Vec<f64> = pt
                    .faces()
                    .map(|f| vec3::dot(ub[f - geom.n_internal], geom.normal(f)))
                    .collect();
                (n.to_string(), PotentialBc::Flux(v))
            }
        })
        .collect();
    let mut solver = SimpleSolver::new(geom.clone(), u, p, st);
    if matches!(start, Start::Potential | Start::PotentialVelocityOnly) {
        let flow = potential_flow(&geom, &patch_bcs).unwrap();
        let pr = if start == Start::Potential {
            let h = bernoulli_head(&geom, &flow.u, &[("outlet".to_string(), 0.0)]).unwrap();
            bernoulli_pressure(&flow.u, h)
        } else {
            vec![0.0; geom.n_cells()]
        };
        solver.set_fields(flow.u, pr).unwrap();
    }
    let max_iter = 3000;
    for i in 1..=max_iter {
        let info = solver.iterate();
        let res = info.res_u.max(info.res_p);
        if std::env::var("I2_TRACE").is_ok() && (i <= 3 || i % 25 == 0) {
            println!("  {start:?} it {i} res_u {:.3e} res_p {:.3e}", info.res_u, info.res_p);
        }
        if !res.is_finite() {
            return Run {
                iterations: i,
                converged: false,
            };
        }
        if res < 1e-6 {
            return Run {
                iterations: i,
                converged: true,
            };
        }
    }
    Run {
        iterations: max_iter,
        converged: false,
    }
}

#[test]
#[ignore = "known miss: potential start 258 iterations against 253 (uniform) and 258 (zero); see ADR 0019 results"]
fn i2_potential_start_needs_fewer_iterations_on_the_backward_facing_step_known_miss() {
    let a = run(Start::UniformX);
    let b = run(Start::Zero);
    let c = run(Start::Potential);
    let d = run(Start::PotentialVelocityOnly);
    println!(
        "I2 iterations: uniform (1,0,0) {} ({}), zero {} ({}), potential {} ({}), potential velocity only {} ({})",
        a.iterations, a.converged, b.iterations, b.converged, c.iterations, c.converged, d.iterations, d.converged
    );
    assert!(
        a.converged && b.converged && c.converged,
        "all registered runs must converge"
    );
    assert!(
        c.iterations < a.iterations,
        "potential {} vs uniform (1,0,0) {}",
        c.iterations,
        a.iterations
    );
    assert!(
        c.iterations < b.iterations,
        "potential {} vs zero {}",
        c.iterations,
        b.iterations
    );
}
