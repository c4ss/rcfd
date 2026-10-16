//! One-way transport of a bin population on a single-phase carrier flux (F-228, ADR 0041): checks T1-T3.
//! No interphase coupling: the carrier flux is prescribed and uniform. No `Symmetry` walls.

use rcfd_fv::bc::{FixedValue, Profile, ZeroGradient};
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::popbal::{BinGrid, BinModel, CoalKernel, PopBalTransport};
use rcfd_fv::scalar::ScalarTransport;
use rcfd_fv::{FvGeometry, ScalarField};
use rcfd_mesh::gen::{channel, Grading};
use std::sync::Arc;

struct Setup {
    geom: Arc<FvGeometry>,
    phi: Vec<f64>,
}

fn channel_setup(nx: usize) -> Setup {
    let mesh = Arc::new(channel([nx, 1, 1], [1.0, 0.1, 0.01], [Grading::Uniform; 3], true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = vec![[1.0, 0.0, 0.0]; geom.n_cells()];
    let ub = vec![[1.0, 0.0, 0.0]; geom.mesh.n_boundary_faces()];
    let phi = face_flux(&geom, &u, &ub);
    Setup { geom, phi }
}

fn bins(geom: &Arc<FvGeometry>, n: usize, inlet: &[f64]) -> Vec<ScalarTransport> {
    (0..n)
        .map(|i| {
            let mut f = ScalarField::uniform(&format!("N{i}"), geom, 0.0);
            f.set_bc(geom, "inlet", Box::new(FixedValue(Profile::scalar(inlet[i]))))
                .unwrap();
            f.set_bc(geom, "outlet", Box::new(ZeroGradient)).unwrap();
            f.set_bc(geom, "walls", Box::new(ZeroGradient)).unwrap();
            let mut t = ScalarTransport::new(geom.clone(), f, 0.0);
            t.convection = ConvScheme::LinearUpwind;
            t
        })
        .collect()
}

fn pulse(x: f64) -> f64 {
    (-((x - 0.25) / 0.08).powi(2)).exp()
}

#[test]
fn t1_pure_advection_of_independent_bins() {
    let nx = 400;
    let s = channel_setup(nx);
    let model = BinModel::new(BinGrid::linear(3, 1.0).unwrap(), None, None).unwrap();
    let mut pb = PopBalTransport::new(bins(&s.geom, 3, &[0.0, 0.0, 0.0]), model).unwrap();
    let cc = s.geom.mesh.cell_centres().to_vec();
    for c in 0..nx {
        pb.bins[0].field.values[c] = pulse(cc[c][0]);
        pb.bins[1].field.values[c] = 3.0 * pulse(cc[c][0]);
    }
    for b in &mut pb.bins {
        b.history = rcfd_fv::ddt::History::new(b.field.values.clone());
    }
    let dt = 0.4 / nx as f64;
    let steps = (0.3 / dt).round() as usize;
    for _ in 0..steps {
        pb.advance(&s.phi, dt, dt);
    }
    let t = steps as f64 * dt;
    let vol = s.geom.mesh.cell_volumes();
    let (mut num, mut den) = (0.0, 0.0);
    for c in 0..nx {
        let ex = pulse(cc[c][0] - t);
        num += vol[c] * (pb.bins[0].field.values[c] - ex).powi(2);
        den += vol[c] * ex * ex;
    }
    let l2 = (num / den).sqrt();
    let mut lin: f64 = 0.0;
    for c in 0..nx {
        let (a, b) = (pb.bins[0].field.values[c], pb.bins[1].field.values[c]);
        lin = lin.max((b - 3.0 * a).abs() / 3.0f64.max(a.abs()));
    }
    let bin3 = pb.bins[2].field.values.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    eprintln!("T1 relative L2 error {l2:.3e}, linearity {lin:.3e}, empty bin max {bin3:.3e}");
    assert!(l2 <= 0.06, "T1 L2 {l2}");
    assert!(lin <= 1e-12, "T1 linearity {lin}");
}

#[test]
fn t2_coagulating_plug_flow() {
    let nx = 200;
    let s = channel_setup(nx);
    let k = 4.0; // K N0 L / U = 4 with N0 = L = U = 1
    let model = BinModel::new(
        BinGrid::geometric(1.0, 4, 40).unwrap(),
        Some(CoalKernel::Constant(k)),
        None,
    )
    .unwrap();
    let nb = model.len();
    let mut inlet = vec![0.0; nb];
    inlet[0] = 1.0;
    let mut pb = PopBalTransport::new(bins(&s.geom, nb, &inlet), model).unwrap();
    let dt = 0.5 / nx as f64;
    let steps = (5.0 / dt).round() as usize;
    for _ in 0..steps {
        pb.advance(&s.phi, dt, dt);
    }
    let c = nx - 1;
    let xc = s.geom.mesh.cell_centres()[c][0];
    let n = pb.cell_state(c);
    let m0 = pb.model.moment(&n, 0.0);
    let exact = 1.0 / (1.0 + 0.5 * k * xc);
    let e0 = (m0 - exact).abs() / exact;
    let dv = (pb.model.volume(&n) - 1.0).abs();
    eprintln!("T2 outlet cell x {xc:.4}: m0 {m0:.6e} exact {exact:.6e} rel {e0:.3e}; |V/V_in - 1| {dv:.3e}");
    assert!(e0 <= 0.01, "T2 m0 {e0}");
    assert!(dv <= 1e-6, "T2 volume {dv}");
}

#[test]
fn t3_source_step_conserves_cell_volume() {
    let s = channel_setup(4);
    let model = BinModel::new(
        BinGrid::geometric(1.0, 3, 10).unwrap(),
        Some(CoalKernel::Brownian { kt_over_mu: 2.0 }),
        None,
    )
    .unwrap();
    let nb = model.len();
    let mut pb = PopBalTransport::new(bins(&s.geom, nb, &vec![0.0; nb]), model).unwrap();
    for (i, b) in pb.bins.iter_mut().enumerate() {
        for (c, v) in b.field.values.iter_mut().enumerate() {
            *v = 1.0 / (1.0 + (i * 3 + c * 7) as f64 % 5.0);
        }
    }
    let before: Vec<f64> = (0..4).map(|c| pb.model.volume(&pb.cell_state(c))).collect();
    pb.source_step(0.05, 0.005);
    let mut worst: f64 = 0.0;
    for c in 0..4 {
        let after = pb.model.volume(&pb.cell_state(c));
        worst = worst.max((after - before[c]).abs() / before[c]);
    }
    eprintln!("T3 worst relative volume change {worst:.3e}");
    assert!(worst <= 1e-12);
}
