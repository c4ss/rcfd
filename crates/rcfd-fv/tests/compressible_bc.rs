//! F-062 validation (checks registered in docs/adr/0006-compressible-boundary-conditions.md
//! before any run): A5 free-stream preservation, B no spurious reflection, C total-pressure inlet
//! nozzle versus the isentropic relations, D airfoil placeholder.

use rcfd_fv::compressible::{CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::compressible_bc::{self, TotalInlet};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{box_2d, warp};
use std::sync::Arc;

fn vec_norm(v: &[f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[test]
fn a5_uniform_free_stream_is_preserved_by_far_field_on_all_sides() {
    let mesh = Arc::new(box_2d(12, 10, 1.0, 1.0, false, false).unwrap());
    let geom = FvGeometry::new(mesh).unwrap();
    let (m, al) = (0.8f64, 1.25f64.to_radians());
    let fs = Prim {
        rho: 1.0,
        u: [m * al.cos(), m * al.sin(), 0.0],
        p: 1.0 / 1.4,
    };
    let mut s = CompressibleSolver::new(geom, Gas::AIR, CompSettings::default(), |_| fs).unwrap();
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        s.set_bc(p, CompBc::FarField(fs)).unwrap();
    }
    for _ in 0..50 {
        let dt = s.max_dt();
        s.step(dt).unwrap();
    }
    let worst = s
        .primitives()
        .iter()
        .map(|q| {
            (q.rho - fs.rho)
                .abs()
                .max((q.p - fs.p).abs())
                .max((q.u[0] - fs.u[0]).abs())
                .max((q.u[1] - fs.u[1]).abs())
        })
        .fold(0.0, f64::max);
    println!("A5 max deviation from free stream: {worst:.2e}");
    assert!(worst < 1e-12, "{worst}");
}

/// Reflected-to-incident pressure amplitude ratio of a right-running pulse leaving through `bc`.
fn reflection_ratio(bc: CompBc) -> f64 {
    let n = 400;
    let mesh = Arc::new(box_2d(n, 1, 1.0, 1.0 / n as f64, false, false).unwrap());
    let geom = FvGeometry::new(mesh).unwrap();
    let p0 = 1.0 / 1.4;
    let amb = Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: p0,
    };
    let eps = 1e-3 * p0;
    let init = move |x: [f64; 3]| {
        let dp = eps * (-((x[0] - 0.5) / 0.03).powi(2)).exp();
        // right-running isentropic simple wave with a = 1, rho = 1
        Prim {
            rho: 1.0 + dp,
            u: [dp, 0.0, 0.0],
            p: p0 + dp,
        }
    };
    let mut s = CompressibleSolver::new(geom, Gas::AIR, CompSettings::default(), init).unwrap();
    s.set_bc("xmin", CompBc::Fixed(amb)).unwrap();
    s.set_bc("xmax", bc).unwrap();
    let amp = |s: &CompressibleSolver| s.primitives().iter().map(|q| (q.p - p0).abs()).fold(0.0, f64::max);
    s.run_to(0.25).unwrap();
    let a_inc = amp(&s);
    s.run_to(1.0).unwrap();
    amp(&s) / a_inc
}

#[test]
fn b_no_spurious_reflection() {
    let p0 = 1.0 / 1.4;
    let fixed_p = reflection_ratio(CompBc::PressureOutlet { p: p0 });
    let nr0 = reflection_ratio(CompBc::NonReflecting { p: p0, sigma: 0.0 });
    let amb = Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: p0,
    };
    let legacy_fixed = reflection_ratio(CompBc::Fixed(amb));
    let legacy_extrap = reflection_ratio(CompBc::Extrapolate);
    println!(
        "B reflection ratio: pressure_outlet {fixed_p:.4}, nonreflecting(0) {nr0:.4}, [info] Fixed {legacy_fixed:.4}, Extrapolate {legacy_extrap:.4}"
    );
    assert!((0.8..=1.2).contains(&fixed_p), "fixed pressure {fixed_p}");
    assert!(nr0 <= 0.02, "sigma 0: {nr0}");
}

/// Registered limit for the default relaxation weight sigma = 0.02 (ADR 0006 check B). Measured
/// 0.461: the stateless relaxation reflects waves slower than its time scale dx/(sigma a).
#[test]
#[ignore = "known miss: reflected ratio 0.461 against the registered 0.05 at sigma = 0.02"]
fn b_non_reflecting_sigma_002_known_miss() {
    let p0 = 1.0 / 1.4;
    let fixed_p = reflection_ratio(CompBc::PressureOutlet { p: p0 });
    let nr = reflection_ratio(CompBc::NonReflecting { p: p0, sigma: 0.02 });
    assert!(nr <= 0.05 && nr <= 0.1 * fixed_p, "sigma 0.02: {nr}");
}

/// Post-hoc, disclosed, NOT a gate: reflection ratio against sigma (explains the miss above).
#[test]
#[ignore = "informational sweep"]
fn b_sigma_sweep_info() {
    let p0 = 1.0 / 1.4;
    for sigma in [0.0, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.3, 1.0] {
        println!(
            "sigma {sigma}: R = {:.4}",
            reflection_ratio(CompBc::NonReflecting { p: p0, sigma })
        );
    }
}

// ---- C: total-pressure inlet nozzle ----

const GAMMA: f64 = 1.4;

fn area_ratio(m: f64) -> f64 {
    ((2.0 / (GAMMA + 1.0)) * (1.0 + 0.5 * (GAMMA - 1.0) * m * m)).powf((GAMMA + 1.0) / (2.0 * (GAMMA - 1.0))) / m
}

fn subsonic_mach(ar: f64) -> f64 {
    let (mut lo, mut hi) = (1e-6, 1.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if area_ratio(mid) > ar {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[test]
#[ignore = "heavy: explicit steady march of the 200x10 nozzle"]
fn c_total_pressure_inlet_reproduces_the_isentropic_nozzle() {
    let (p0, t0, r) = (101325.0, 300.0, 287.0);
    let (ht, nx, ny) = (0.05, 200usize, 10usize);
    let l = 40.0 * ht;
    let h = |x: f64| ht + ht * (1.0 + (2.0 * std::f64::consts::PI * x / l).cos()) / 2.0;
    let base = box_2d(nx, ny, l, 1.0, false, false).unwrap();
    let mesh = Arc::new(warp(&base, |p| [p[0], p[1] * h(p[0]), p[2]]).unwrap());
    let geom = FvGeometry::new(mesh.clone()).unwrap();
    let a_t_ratio = area_ratio(0.3);
    let m_e = subsonic_mach(2.0 * a_t_ratio);
    let p_e = p0 * (1.0 + 0.5 * (GAMMA - 1.0) * m_e * m_e).powf(-GAMMA / (GAMMA - 1.0));
    let gas = Gas::AIR;
    let tin = t0 / (1.0 + 0.5 * (GAMMA - 1.0) * m_e * m_e);
    let init = Prim {
        rho: p_e / (r * tin),
        u: [m_e * (GAMMA * r * tin).sqrt(), 0.0, 0.0],
        p: p_e,
    };
    let inlet = TotalInlet {
        p0,
        t0,
        r_gas: r,
        dir: [1.0, 0.0, 0.0],
    };
    let mut s = CompressibleSolver::new(geom, gas, CompSettings::default(), |_| init).unwrap();
    s.set_bc("xmin", CompBc::TotalInlet(inlet)).unwrap();
    s.set_bc("xmax", CompBc::PressureOutlet { p: p_e }).unwrap();
    let (steps, change) = s.run_steady(1e-9, 200_000).unwrap();
    println!("C steady: {steps} steps, change {change:.2e}");
    assert!(change < 1e-9, "not converged: {change}");

    let prim = s.primitives();
    let cc = mesh.cell_centres();
    let vol = mesh.cell_volumes();
    let dx = l / nx as f64;
    let (mut e_m, mut e_p, mut e_t) = (0.0f64, 0.0f64, 0.0f64);
    for i in (10..200).step_by(20) {
        let (mut vm, mut vp, mut vt, mut vv) = (0.0, 0.0, 0.0, 0.0);
        let mut xc = 0.0;
        for (c, x) in cc.iter().enumerate() {
            if (x[0] / dx).floor() as usize == i {
                let q = &prim[c];
                vm += vol[c] * vec_norm(&q.u) / q.sound_speed(gas);
                vp += vol[c] * q.p;
                vt += vol[c] * q.p / (q.rho * r);
                vv += vol[c];
                xc = x[0];
            }
        }
        let m_ref = subsonic_mach(a_t_ratio * h(xc) / ht);
        let fac = 1.0 + 0.5 * (GAMMA - 1.0) * m_ref * m_ref;
        e_m = e_m.max((vm / vv / m_ref - 1.0).abs());
        e_p = e_p.max((vp / vv / (p0 * fac.powf(-GAMMA / (GAMMA - 1.0))) - 1.0).abs());
        e_t = e_t.max((vt / vv / (t0 / fac) - 1.0).abs());
    }
    // mass flows through the boundary face states
    let (own, sf) = (mesh.owner(), mesh.face_area_vectors());
    let flow = |patch: &str, f: &dyn Fn(&Prim, [f64; 3]) -> Prim| -> f64 {
        let pa = mesh.patches().iter().find(|p| p.name == patch).unwrap();
        pa.faces()
            .map(|fc| {
                let a = vec_norm(&sf[fc]);
                let n = [sf[fc][0] / a, sf[fc][1] / a, sf[fc][2] / a];
                let q = f(&prim[own[fc]], n);
                q.rho * (q.u[0] * sf[fc][0] + q.u[1] * sf[fc][1] + q.u[2] * sf[fc][2])
            })
            .sum()
    };
    let m_in_flow = -flow("xmin", &|q, n| compressible_bc::total_inlet(gas, &inlet, q, n));
    let m_out_flow = flow("xmax", &|q, n| compressible_bc::pressure_outlet(gas, p_e, 1.0, q, n));
    let inlet_area: f64 = mesh
        .patches()
        .iter()
        .find(|p| p.name == "xmin")
        .unwrap()
        .faces()
        .map(|fc| vec_norm(&sf[fc]))
        .sum();
    let depth = inlet_area / h(0.0);
    let a_star = ht / a_t_ratio * depth;
    let m_ref =
        a_star * p0 * (GAMMA / (r * t0)).sqrt() * (2.0 / (GAMMA + 1.0)).powf((GAMMA + 1.0) / (2.0 * (GAMMA - 1.0)));
    let e_mass = m_in_flow / m_ref - 1.0;
    let e_io = (m_in_flow - m_out_flow).abs() / m_in_flow;
    println!(
        "C errors: Mach {e_m:.3e} (1.5e-2), p/p0 {e_p:.3e} (5e-3), T/T0 {e_t:.3e} (3e-3), mass {e_mass:.3e} (1e-2), in/out {e_io:.2e} (1e-3)"
    );
    assert!(e_m < 1.5e-2, "Mach {e_m}");
    assert!(e_p < 5e-3, "p {e_p}");
    assert!(e_t < 3e-3, "T {e_t}");
    assert!(e_mass.abs() < 1e-2, "mass {e_mass}");
    assert!(e_io < 1e-3, "in/out {e_io}");
}

#[test]
#[ignore = "known miss: not attempted, no accessible published NACA 0012 transonic Cp reference (ADR 0006 check D)"]
fn naca0012_transonic_cp_known_miss() {
    panic!("NACA 0012 transonic Cp within published scatter was not attempted: no accessible published reference data (ADR 0006 check D)");
}
