//! ADR 0052 checks M1-M5 (interfacial mass transfer wired into the VOF host). The limits are those registered in the ADR before
//! any run; none was changed afterwards.
//!
//! Energy limitation (ADR 0052 audit): the multiphase path has no energy equation, the temperature is a prescribed field and no
//! latent-heat feedback exists, so no Stefan problem and no relaxation of T to T_sat is claimed here.

use rcfd_fv::masstransfer::{HertzKnudsen, LeeModel, R_UNIVERSAL};
use rcfd_fv::vof::{indicator_fraction, MassTransfer, MassTransferModel, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const NX: usize = 8;
const NY: usize = 24;
const W: f64 = 0.1;
const HT: f64 = 0.3;
const T_SAT: f64 = 373.15;
const H_FG: f64 = 2.26e6;
const RHO: [f64; 2] = [1000.0, 0.6];

fn solver(mt: Option<MassTransfer>, tilt: bool, gravity: bool) -> VofSolver {
    let mesh = Arc::new(box_2d(NX, NY, W, HT, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    p_rgh.set_bc(&geom, "ymax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| {
        if tilt {
            x[1] <= 0.15 + 0.1 * (x[0] - 0.05)
        } else {
            x[1] <= 0.15
        }
    });
    let mut s = VofSettings::new(RHO, [1e-3, 1.2e-5]);
    if gravity {
        s.gravity = [0.0, -9.81, 0.0];
    }
    s.mass_transfer = mt;
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

fn lee(temperature: Vec<f64>, c: f64) -> MassTransfer {
    MassTransfer {
        liquid_phase: 1,
        model: MassTransferModel::Lee { c_evap: c, c_cond: c },
        t_sat: T_SAT,
        h_fg: H_FG,
        temperature,
    }
}

fn n_cells() -> usize {
    NX * NY
}

/// Superheated layer: the two cell rows just below the interface at y = 0.15.
fn layer_temperature(theta: f64) -> Vec<f64> {
    (0..n_cells())
        .map(|c| {
            let row = c / NX;
            if row == 10 || row == 11 {
                T_SAT * (1.0 + theta)
            } else {
                T_SAT
            }
        })
        .collect()
}

#[test]
fn m1_closed_form_source_alone() {
    let n = n_cells();
    let (c, theta) = (100.0, 0.02);
    // evaporation: alpha_l(t) = 0.6 exp(-c theta t), independent of dt
    for dt in [1e-3f64, 1e-2] {
        let mut s = solver(Some(lee(vec![T_SAT * (1.0 + theta); n], c)), false, false);
        s.transport.alpha.values = vec![0.6; n];
        let mut worst: f64 = 0.0;
        let steps_per: Vec<(usize, f64)> = [0.1, 0.5, 1.0]
            .iter()
            .map(|t| ((t / dt).round() as usize, *t))
            .collect();
        let mut done = 0;
        for (target, t) in steps_per {
            while done < target {
                let a0 = s.transport.alpha.values.clone();
                s.apply_mass_transfer(dt, &a0);
                done += 1;
            }
            let exact = 0.6 * (-c * theta * t).exp();
            for &a in &s.transport.alpha.values {
                worst = worst.max((a - exact).abs() / exact);
            }
        }
        println!("M1 evaporation dt {dt}: worst relative error {worst:.2e} (limit 1e-12)");
        assert!(worst <= 1e-12, "M1 evaporation dt {dt}: {worst}");
    }
    // condensation: 1 - alpha_l(t) = 0.6 exp(-c rho_v |theta| t / rho_l)
    let mut s = solver(Some(lee(vec![T_SAT * (1.0 - theta); n], c)), false, false);
    s.transport.alpha.values = vec![0.4; n];
    let dt = 1e-3;
    let mut worst: f64 = 0.0;
    for k in 1..=1000 {
        let a0 = s.transport.alpha.values.clone();
        s.apply_mass_transfer(dt, &a0);
        if k == 100 || k == 500 || k == 1000 {
            let exact = 0.6 * (-c * RHO[1] * theta * (k as f64 * dt) / RHO[0]).exp();
            for &a in &s.transport.alpha.values {
                worst = worst.max(((1.0 - a) - exact).abs() / exact);
            }
        }
    }
    println!("M1 condensation: worst relative error {worst:.2e} (limit 1e-12)");
    assert!(worst <= 1e-12, "M1 condensation {worst}");
    // phase 2 as the liquid: the complementary field gives the mirrored result
    let mut mt = lee(vec![T_SAT * (1.0 + theta); n], c);
    mt.liquid_phase = 2;
    let mut sw = VofSolver::new(
        s.geom.clone(),
        s.u.clone(),
        s.p_rgh.clone(),
        s.transport.alpha.clone(),
        {
            let mut st = s.settings.clone();
            st.rho = [RHO[1], RHO[0]];
            st.mass_transfer = Some(mt);
            st
        },
    )
    .unwrap();
    sw.transport.alpha.values = vec![0.4; n]; // alpha_l = 1 - 0.4 = 0.6
    for _ in 0..100 {
        let a0 = sw.transport.alpha.values.clone();
        sw.apply_mass_transfer(1e-3, &a0);
    }
    let exact_l = 0.6 * (-c * theta * 0.1f64).exp();
    let got_l = 1.0 - sw.transport.alpha.values[0];
    println!("M1 liquid_phase = 2: alpha_l {got_l:.15} exact {exact_l:.15}");
    assert!((got_l - exact_l).abs() / exact_l <= 1e-12);
    // small-dt limit equals the library Lee rate (the exponential integrator integrates the same source)
    let mut s = solver(Some(lee(vec![T_SAT * (1.0 + theta); n], c)), false, false);
    s.transport.alpha.values = vec![0.6; n];
    let a0 = s.transport.alpha.values.clone();
    let pc = s.apply_mass_transfer(1e-9, &a0);
    let lib = LeeModel { c_evap: c, c_cond: c }
        .rate(0.6, 0.4, RHO[0], RHO[1], T_SAT * (1.0 + theta), T_SAT)
        .unwrap();
    println!("M1 small-dt mdot_eff {:.9e} library Lee rate {lib:.9e}", pc.mdot[0]);
    assert!((pc.mdot[0] - lib).abs() / lib <= 1e-6);
}

#[test]
fn m2_phase_mass_conservation() {
    // per cell: rho_l d(V_l) + rho_v (d(V_v) + S_v V dt) = 0, from the alpha change and the continuity source independently
    let n = n_cells();
    let mut s = solver(Some(lee(vec![T_SAT * 1.03; n], 50.0)), false, false);
    s.transport.alpha.values = (0..n).map(|c| 0.1 + 0.8 * ((c * 7) % 10) as f64 / 10.0).collect();
    let dt = 1e-3;
    let vol = s.geom.mesh.cell_volumes().to_vec();
    let a0 = s.transport.alpha.values.clone();
    let pc = s.apply_mass_transfer(dt, &a0);
    let mut worst: f64 = 0.0;
    let mut tot = 0.0;
    for c in 0..n {
        let dvl = (s.transport.alpha.values[c] - a0[c]) * vol[c];
        let dvv = -dvl;
        let balance = RHO[0] * dvl + RHO[1] * (dvv + pc.vol_src[c] * dt);
        let scale = pc.mdot[c] * vol[c] * dt;
        tot += scale;
        worst = worst.max(balance.abs() / scale.abs().max(1e-300));
    }
    println!("M2 per-cell phase-mass balance: worst {worst:.2e} (limit 1e-12), total transferred {tot:.3e} kg");
    assert!(worst <= 1e-12, "M2 per cell {worst}");

    // coupled run: integrated liquid volume changes only by the delivered transfer (alpha = 0 at the open top)
    let mut s = solver(Some(lee(layer_temperature(0.02), 1.0)), false, true);
    let v0 = s.phase_volume();
    let (mut transferred, mut vol_src_cum, mut out_cum) = (0.0, 0.0, 0.0);
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    let mut rate_cum = 0.0;
    let dt = 1e-3;
    for _ in 0..200 {
        let i = s.step(dt).unwrap();
        transferred += i.transfer_volume;
        out_cum += dt * i.alpha_outflow;
        vol_src_cum += dt * i.volume_source;
        rate_cum += dt * i.transfer_rate;
        lo = lo.min(i.alpha_min);
        hi = hi.max(i.alpha_max);
    }
    let dv = s.phase_volume() - v0;
    let ident = (dv + out_cum - transferred).abs() / v0;
    let mass = (RHO[0] * transferred + rate_cum).abs() / rate_cum.abs();
    println!(
        "M2 coupled: dV {dv:.3e}, identity residual {ident:.2e} (limit 1e-12), liquid mass vs transferred {mass:.2e} (limit 1e-12), delivered {rate_cum:.4e} kg, volume source {vol_src_cum:.4e} m3, alpha [{lo:.2e}, {hi:.12}] (limit 1e-9)"
    );
    assert!(ident <= 1e-12, "M2 volume identity {ident}");
    assert!(mass <= 1e-12, "M2 mass {mass}");
    assert!(rate_cum > 0.0 && lo >= -1e-9 && hi <= 1.0 + 1e-9, "M4 bounds");
}

#[test]
fn m3_zero_transfer_at_saturation() {
    let n = n_cells();
    let mut a = solver(Some(lee(vec![T_SAT; n], 100.0)), true, true);
    let mut b = solver(None, true, true);
    for _ in 0..200 {
        a.step(1e-3).unwrap();
        b.step(1e-3).unwrap();
    }
    let same = |x: &[f64], y: &[f64]| x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits());
    let ua: Vec<f64> = a.u.values.iter().flat_map(|v| v.to_vec()).collect();
    let ub: Vec<f64> = b.u.values.iter().flat_map(|v| v.to_vec()).collect();
    let moving = ub.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    println!("M3 saturation: max |U| of the reference run {moving:.2e} (non-trivial flow)");
    assert!(moving > 1e-6, "the reference run must move");
    assert!(same(&a.transport.alpha.values, &b.transport.alpha.values), "alpha");
    assert!(same(&ua, &ub), "u");
    assert!(same(&a.p_rgh.values, &b.p_rgh.values), "p_rgh");
    assert!(same(&a.phi, &b.phi), "phi");
}

#[test]
fn m4_superheated_layer_continuity() {
    let mut s = solver(Some(lee(layer_temperature(0.02), 1.0)), false, true);
    let top = s.geom.mesh.patch("ymax").unwrap().faces();
    let (mut worst, mut worst_cell) = (0.0f64, 0usize);
    let mut flux_err: f64 = 0.0;
    for _ in 0..200 {
        let i = s.step(1e-3).unwrap();
        // net outflow through the pressure patch equals the volume source
        let out: f64 = top.clone().map(|f| s.phi[f]).sum();
        flux_err = flux_err.max((out - i.volume_source).abs() / i.volume_source.abs().max(1e-300));
        // cell-wise divergence of the corrected flux against S_v V
        let div = rcfd_fv::pressure::divergence(&s.geom, &s.phi);
        let vol = s.geom.mesh.cell_volumes();
        let [rl, rv] = RHO;
        for c in 0..div.len() {
            let want = s.mdot[c] * (1.0 / rv - 1.0 / rl) * vol[c];
            let e = (div[c] - want).abs();
            if e > worst {
                worst = e;
                worst_cell = c;
            }
        }
    }
    let q_scale = s.mdot.iter().fold(0.0f64, |m, v| m.max(v.abs())) * (1.0 / RHO[1]) * s.geom.mesh.cell_volumes()[0];
    println!(
        "M4 continuity: boundary flux vs volume source relative {flux_err:.2e} (limit 1e-6); largest cell |div phi - S_v V| {worst:.2e} m3/s at cell {worst_cell} (scale {q_scale:.2e})"
    );
    assert!(flux_err <= 1e-6, "M4 flux {flux_err}");
    assert!(worst <= 1e-6 * q_scale, "M4 cell {worst_cell}: {worst}");
}

#[test]
fn m5_hertz_knudsen_closed_form() {
    let n = n_cells();
    let t = T_SAT * 1.02;
    let (m_mol, h_ref) = (0.018, 101_325.0);
    let psat = h_ref * (H_FG * m_mol / R_UNIVERSAL * (1.0 / T_SAT - 1.0 / t)).exp();
    let model = |p_v: f64| MassTransfer {
        liquid_phase: 1,
        model: MassTransferModel::HertzKnudsen {
            accommodation: 1.0,
            molar_mass: m_mol,
            schrage: false,
            p_vapour: p_v,
            p_sat_ref: h_ref,
        },
        t_sat: T_SAT,
        h_fg: H_FG,
        temperature: vec![t; n],
    };
    let hk = HertzKnudsen {
        sigma: 1.0,
        molar_mass: m_mol,
        schrage: false,
    };
    let j = hk.flux(psat, t, 0.5 * psat, t).unwrap();
    let mut s = solver(Some(model(0.5 * psat)), false, false);
    let thickness = s.geom.mesh.total_volume() / (W * HT);
    let area = W * thickness;
    let a0 = s.transport.alpha.values.clone();
    let pc = s.apply_mass_transfer(1e-6, &a0);
    let rel = (pc.requested - j * area).abs() / (j * area);
    println!(
        "M5 HK: requested {:.6e} kg/s, j A = {:.6e} (relative {rel:.2e}, limit 1e-9); delivered {:.6e} (vapour-side share clipped: no liquid there)",
        pc.requested,
        j * area,
        pc.rate
    );
    assert!(rel <= 1e-9, "M5 {rel}");
    assert!(pc.rate > 0.0 && pc.rate <= pc.requested * (1.0 + 1e-12));
    // at saturation pressure the rate vanishes
    let mut s = solver(Some(model(psat)), false, false);
    let a0 = s.transport.alpha.values.clone();
    let pc = s.apply_mass_transfer(1e-6, &a0);
    println!(
        "M5 HK at p_v = p_sat(T): requested {:.2e} (limit {:.2e})",
        pc.requested,
        1e-12 * j * area
    );
    assert!(pc.requested.abs() <= 1e-12 * j * area);
}
