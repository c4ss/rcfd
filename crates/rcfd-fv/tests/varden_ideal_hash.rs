//! ADR 0007 check (R4): the ideal-gas path of the variable-density kernel is bit-identical before and after the
//! real-gas equation of state was added. The hash (FNV-1a over the f64 bit patterns of u, p, T, rho and the mass
//! flux after 30 iterations of a 40 x 6 subsonic nozzle, SIMPLEC, compressible mode, stagnation inlet) was recorded
//! on the tree before any real-gas edit.

use rcfd_core::hash::Fnv64;
use rcfd_fv::bc::{FixedValue, Symmetry, ZeroGradient};
use rcfd_fv::simple::Coupling;
use rcfd_fv::varden::{DensityMode, IdealGas, Property, StagnationInlet, VarDensitySolver, VdSettings};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d, warp};
use std::sync::Arc;

const REGISTERED: u64 = 0xbbedca02b6f39a6d;

fn run() -> u64 {
    let (r, gamma, p0, t0, h_t, l) = (287.0, 1.4, 101_325.0, 300.0, 0.05, 2.0);
    let base = box_2d(40, 6, l, 1.0, false, false).unwrap();
    let mesh = warp(&base, |p| {
        let s = p[0] / l;
        let h = h_t + (2.0 * h_t - h_t) * (1.0 + (2.0 * std::f64::consts::PI * s).cos()) / 2.0;
        [p[0], p[1] * h, p[2]]
    })
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let p_out = 0.97 * p0;
    let mut u = VectorField::uniform("U", &geom, [30.0, 0.0, 0.0]);
    u.set_bc(&geom, "xmin", Box::new(ZeroGradient)).unwrap();
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    u.set_bc(&geom, "ymin", Box::new(Symmetry)).unwrap();
    u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.5 * (p0 + p_out));
    p.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(p0))).unwrap();
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(p_out))).unwrap();
    p.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    p.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut t = ScalarField::uniform("T", &geom, t0);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(t0))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let gas = IdealGas {
        r,
        cp: gamma * r / (gamma - 1.0),
    };
    let mut s = VdSettings::new(
        gas,
        Property::Constant(1.8e-5),
        Property::Constant(0.03),
        Coupling::Simplec,
    );
    s.mode = DensityMode::Compressible;
    s.inlets = vec![StagnationInlet {
        patch: "xmin".into(),
        p0,
        t0,
    }];
    let mut sol = VarDensitySolver::new(geom, u, p, t, s).unwrap();
    for _ in 0..30 {
        sol.iterate();
    }
    let mut h = Fnv64::new();
    for v in &sol.u.values {
        for c in v {
            h.write_f64(*c);
        }
    }
    for x in sol.p.values.iter().chain(&sol.t.values).chain(&sol.rho).chain(&sol.phi) {
        h.write_f64(*x);
    }
    h.finish()
}

#[test]
fn ideal_gas_nozzle_matches_registered_hash() {
    let got = run();
    assert_eq!(got, REGISTERED, "got {got:#018x}");
}
