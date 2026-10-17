//! ADR 0049 checks C1-C3 (static droplet with surface tension in zero gravity). The limits are those registered in the ADR
//! before any run; none was changed afterwards.

use rcfd_fv::vof::{indicator_fraction, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const N: usize = 64;
const R: f64 = 0.25;
const SIGMA: f64 = 1.0;
const MU: f64 = 0.02;

fn droplet() -> VofSolver {
    let mesh = Arc::new(box_2d(N, N, 1.0, 1.0, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| (x[0] - 0.5).hypot(x[1] - 0.5) <= R);
    let mut s = VofSettings::new([1.0, 1.0], [MU, MU]);
    s.sigma = SIGMA;
    s.n_smooth = 2;
    s.n_correctors = 3;
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

fn centroid(s: &VofSolver) -> [f64; 2] {
    let vol = s.geom.mesh.cell_volumes();
    let cc = s.geom.mesh.cell_centres();
    let a = &s.transport.alpha.values;
    let m: f64 = (0..a.len()).map(|c| a[c] * vol[c]).sum();
    [
        (0..a.len()).map(|c| a[c] * vol[c] * cc[c][0]).sum::<f64>() / m,
        (0..a.len()).map(|c| a[c] * vol[c] * cc[c][1]).sum::<f64>() / m,
    ]
}

#[test]
fn c_static_droplet() {
    let mut s = droplet();
    let dx = 1.0 / N as f64;
    let dt = 4e-4;
    assert!(
        dt <= s.capillary_dt(),
        "dt {dt} above the capillary limit {}",
        s.capillary_dt()
    );
    let v0 = s.phase_volume();
    let c0 = centroid(&s);
    let (mut ca_max, mut dv, mut lo, mut hi) = (0.0f64, 0.0f64, 0.0f64, 1.0f64);
    let t0 = std::time::Instant::now();
    for _ in 0..2500 {
        let i = s.step(dt).unwrap();
        let umax =
            s.u.values
                .iter()
                .map(|v| (v[0] * v[0] + v[1] * v[1]).sqrt())
                .fold(0.0, f64::max);
        ca_max = ca_max.max(MU * umax / SIGMA);
        dv = dv.max((s.phase_volume() - v0).abs() / v0);
        lo = lo.min(i.alpha_min);
        hi = hi.max(i.alpha_max);
    }
    // Laplace jump
    let cc = s.geom.mesh.cell_centres();
    let a = &s.transport.alpha.values;
    let (mut pin, mut nin, mut pout, mut nout) = (0.0, 0, 0.0, 0);
    for c in 0..a.len() {
        let r = (cc[c][0] - 0.5).hypot(cc[c][1] - 0.5);
        if a[c] > 0.99 && r < R - 3.0 * dx {
            pin += s.p[c];
            nin += 1;
        }
        if a[c] < 0.01 && r > R + 4.0 * dx {
            pout += s.p[c];
            nout += 1;
        }
    }
    let dp = pin / nin as f64 - pout / nout as f64;
    let err = (dp - SIGMA / R).abs() / (SIGMA / R);
    let c1 = centroid(&s);
    let drift = ((c1[0] - c0[0]).hypot(c1[1] - c0[1])) / dx;
    println!(
        "C1 dp {dp:.4} vs {:.4}: relative error {err:.3e} (limit 0.10); C2 Ca_max {ca_max:.3e} (limit 1e-2); C3 dV {dv:.2e} (limit 1e-12), centroid drift {drift:.3e} dx (limit 0.01); alpha in [{lo:.2e}, {:.8}] ({} / {} cells); {:.1} s",
        SIGMA / R,
        hi,
        nin,
        nout,
        t0.elapsed().as_secs_f64()
    );
    assert!(err <= 0.10, "C1 {err}");
    assert!(ca_max <= 1e-2, "C2 {ca_max}");
    assert!(dv <= 1e-12, "C3 volume {dv}");
    assert!(drift <= 0.01, "C3 centroid {drift}");
    assert!(lo >= -1e-9 && hi <= 1.0 + 1e-9, "Z1 bounds");
}
