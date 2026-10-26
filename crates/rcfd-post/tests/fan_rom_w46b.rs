//! Wave 46B (ADR 0119): reduced-order axial-fan tier. Registered checks E1-E2, Z1-Z2, L1-L3, F1, R1-R3, M1, G1, the reported
//! FAN-01 comparison V1, and the rule-d smoke.

use rcfd_mesh::fan::SectionFamily;
use rcfd_post::fan_rom::*;
use rcfd_post::rotor::Rotor;
use std::f64::consts::PI;

// ---------------------------------------------------------------- set-ups (every struct literal of the blade lives here)

const Q_D: f64 = 0.045;
const DP_IDEAL: f64 = 45.0;

fn op() -> Operating {
    Operating {
        rpm: 3000.0,
        rho: 1.2,
        nu: 1.5e-5,
    }
}

fn section(radius: f64, chord: f64, stagger: f64, camber: f64, camber_pos: f64) -> RomSection {
    RomSection {
        radius,
        chord,
        stagger,
        camber,
        camber_pos,
    }
}

/// 120 mm class axial fan: 7 blades, hub 30 mm, tip 58 mm, gap 1 mm, chord 30 mm, NACA camber 0.04 at 0.5 (stagger set by the design).
fn base_fan() -> RomFan {
    RomFan {
        n_blades: 7,
        r_hub: 0.030,
        r_tip: 0.058,
        tip_gap: 0.001,
        family: SectionFamily::Naca4,
        sections: vec![
            section(0.030, 0.030, 0.0, 0.04, 0.5),
            section(0.058, 0.030, 0.0, 0.04, 0.5),
        ],
    }
}

fn k_vortex() -> f64 {
    DP_IDEAL / op().rho / op().omega()
}

/// Free-vortex design at Q_D for `model`.
fn designed(model: &RomModel) -> FanRom {
    let f = free_vortex_design(&base_fan(), model, &op(), Q_D, k_vortex()).unwrap();
    FanRom::new(f, *model).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

/// FAN-01 CAD sections and characteristic from `validation-data/reference` (ADR 0075).
fn fan01() -> (RomFan, Vec<[f64; 3]>) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation-data/reference");
    let rows = |name: &str| -> Vec<Vec<String>> {
        std::fs::read_to_string(root.join(name))
            .unwrap()
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .skip(1)
            .map(|l| l.split(',').map(|s| s.trim().to_string()).collect())
            .collect()
    };
    let num = |s: &String| s.parse::<f64>().unwrap();
    let sections: Vec<RomSection> = rows("w34d_fan01_cad_sections.csv")
        .iter()
        .map(|r| {
            section(
                num(&r[0]) * 1e-3,
                num(&r[1]) * 1e-3,
                num(&r[2]).to_radians(),
                num(&r[3]),
                num(&r[4]),
            )
        })
        .collect();
    let fan = RomFan {
        n_blades: 9,
        r_hub: 0.124,
        r_tip: 0.2475,
        tip_gap: 0.0025,
        family: SectionFamily::Naca4,
        sections,
    };
    let data = rows("w34d_fan01_characteristic_n1ug.csv")
        .iter()
        .map(|r| [num(&r[0]), num(&r[1]), num(&r[2])])
        .collect();
    (fan, data)
}

// ---------------------------------------------------------------- smoke (rule d), run before registration

#[test]
#[ignore = "rule-d smoke of ADR 0119, printed values only"]
fn smoke_w46b() {
    let m = RomModel::default();
    let rom = designed(&m);
    let p = rom.point(&op(), Q_D).unwrap();
    let e = p
        .strips
        .iter()
        .map(|s| rel(s.dc_theta, k_vortex() / s.r))
        .fold(0.0, f64::max);
    println!("SMOKE design: max rel |dc - K/r| {e:.2e}; dp_t {:.3} dp_s {:.3} P {:.4} eta_t {:.4} eta_s {:.4} tip {:.5} drag {:.5} re_out {}",
        p.dp_t, p.dp_s, p.power, p.eta_t, p.eta_s, p.tip_loss, p.drag_loss, p.re_outside);
    for s in p.strips.iter().step_by(8) {
        println!(
            "SMOKE strip r {:.4} U {:.2} dc {:.3} beta {:.2} ae {:.3} deg cl {:.3} cd {:.4} Re {:.0}",
            s.r,
            s.u,
            s.dc_theta,
            s.beta_inf.to_degrees(),
            s.alpha_e.to_degrees(),
            s.cl,
            s.cd,
            s.re
        );
    }
    for j in 1..=24 {
        let q = 0.004 * j as f64;
        match rom.point(&op(), q) {
            Ok(p) => println!(
                "SMOKE curve q {q:.3} dp_t {:8.3} dp_s {:8.3} P {:.4} eta_t {:.4} eta_s {:.4} stalled {} re_out {}",
                p.dp_t,
                p.dp_s,
                p.power,
                p.eta_t,
                p.eta_s,
                p.stalled.len(),
                p.re_outside
            ),
            Err(e) => println!("SMOKE curve q {q:.3} error {e}"),
        }
    }
    let k = p.dp_s / (Q_D * Q_D);
    let r = rom.operating_point(&op(), &SystemCurve { k, l: 0.0 }, 1e-3, 0.1, 100);
    println!("SMOKE op K {k:.1}: {:?}", r.as_ref().map(|x| (x.0, x.1.dp_s)));
    let r = rom.operating_point(&op(), &SystemCurve { k: 20.0 * k, l: 0.0 }, 1e-3, 0.1, 100);
    println!("SMOKE op 20K: {:?}", r.as_ref().map(|x| x.0));
    let r = rom.operating_point(&op(), &SystemCurve { k: 1e3 * k, l: 0.0 }, 1e-3, 0.1, 100);
    println!("SMOKE op 1000K: {:?}", r.as_ref().map(|x| x.0));
    let t = rom.tone(&op(), &p, 343.0, 1, 1.0, PI / 2.0).unwrap();
    let t60 = rom.tone(&op(), &p, 343.0, 1, 1.0, PI / 3.0).unwrap();
    println!("SMOKE tone BPF 1 m 90 deg {t:.3e} Pa, 60 deg {t60:.3e} Pa");
    let one = Rotor {
        blades: 7,
        omega: op().omega(),
        r_tip: 0.058,
        thrust: p.thrust,
        torque: p.torque,
        c0: 343.0,
    };
    println!(
        "SMOKE compact rotor at r_tip: {:.3e} Pa",
        one.harmonic_amplitude(1, 1.0, PI / 2.0).unwrap()
    );
    let ledger = p.power - (p.q * p.dp_t + p.drag_loss + p.tip_loss);
    println!("SMOKE ledger residual {:.2e} of P {:.4}", ledger, p.power);
    let m2 = RomModel { n_strips: 80, ..m };
    let p2 = FanRom::new(rom.fan.clone(), m2)
        .unwrap()
        .point(&op(), 0.6 * Q_D)
        .unwrap();
    let p1 = rom.point(&op(), 0.6 * Q_D).unwrap();
    println!(
        "SMOKE strips 40 vs 80 at 0.6 Q_D: dp_t {:.4} {:.4} P {:.5} {:.5}",
        p1.dp_t, p2.dp_t, p1.power, p2.power
    );
    for (name, mm) in [("default", m), ("no stall", RomModel { cl_max: 100.0, ..m })] {
        let r = FanRom::new(rom.fan.clone(), mm).unwrap();
        for j in 0..=12 {
            let q = Q_D * (0.70 + 0.05 * j as f64);
            let p = r.point(&op(), q).unwrap();
            println!(
                "SMOKE fine {name} q/Q_D {:.2} dp_t {:8.3} dp_s {:8.3} eta_t {:.4} eta_s {:.4} stalled {}",
                q / Q_D,
                p.dp_t,
                p.dp_s,
                p.eta_t,
                p.eta_s,
                p.stalled.len()
            );
        }
    }
    let ns = FanRom::new(rom.fan.clone(), RomModel { cl_max: 100.0, ..m }).unwrap();
    for f in [0.5, 0.55, 1.25, 1.3] {
        let p = ns.point(&op(), f * Q_D).unwrap();
        println!("SMOKE no-stall ends q/Q_D {f}: dp_t {:.3} eta_t {:.4}", p.dp_t, p.eta_t);
    }
    let p75 = rom.point(&op(), 0.75 * Q_D).unwrap();
    let k75 = p75.dp_s / (0.75 * Q_D).powi(2);
    let r = rom.operating_point(&op(), &SystemCurve { k: k75, l: 0.0 }, 1e-3, 0.1, 100);
    println!("SMOKE op through 0.75 Q_D: {:?}", r.as_ref().map(|x| x.0));
    let (f1, data) = fan01();
    let r1 = FanRom::new(f1, m);
    println!(
        "SMOKE FAN-01 construction: {} sections, {} data rows, ok {}",
        r1.as_ref().map(|r| r.fan.sections.len()).unwrap_or(0),
        data.len(),
        r1.is_ok()
    );
}

// ---------------------------------------------------------------- registered checks (ADR 0119)

fn dq(s: &Strip) -> f64 {
    s.cx * 2.0 * PI * s.r * s.dr
}

/// Mass-averaged Euler total pressure `rho sum U dc dQ / Q`.
fn euler_dp(p: &FanPoint, rho: f64) -> f64 {
    p.strips.iter().map(|s| rho * s.u * s.dc_theta * dq(s)).sum::<f64>() / p.q
}

#[test]
fn e1_euler_work_identity() {
    let rom = designed(&RomModel::default());
    let (mut worst, mut nc) = (0.0_f64, f64::INFINITY);
    for f in [0.6, 1.0, 1.2] {
        let p = rom.point(&op(), f * Q_D).unwrap();
        for s in &p.strips {
            let c1 = s.cx * s.cx;
            let c2 = c1 + s.dc_theta * s.dc_theta;
            let w1 = c1 + s.u * s.u;
            let y = s.u * s.dc_theta;
            let w2 = c1 + (s.u - s.dc_theta).powi(2);
            let w2_wrong = c1 + (s.u + s.dc_theta).powi(2);
            worst = worst.max((y - (0.5 * (c2 - c1) + 0.5 * (w1 - w2))).abs() / (s.u * s.u));
            nc = nc.min((y - (0.5 * (c2 - c1) + 0.5 * (w1 - w2_wrong))).abs() / (s.u * s.u));
        }
    }
    println!("W46B E1: worst {worst:.2e} (limit 1e-12); NC-E1 smallest {nc:.2e} (must be >= 1e-11)");
    assert!(nc >= 1e-11, "NC-E1 does not discriminate: {nc:e}");
    assert!(worst <= 1e-12, "{worst:e}");
}

#[test]
fn e2_free_vortex_closed_form() {
    let o = op();
    let k = k_vortex();
    let p = designed(&RomModel::default()).point(&o, Q_D).unwrap();
    let e_dc = p.strips.iter().map(|s| rel(s.dc_theta, k / s.r)).fold(0.0, f64::max);
    let e_m = rel(p.torque, o.rho * Q_D * k);
    let e_p = rel(p.power, o.rho * Q_D * o.omega() * k);
    let e_y = rel(euler_dp(&p, o.rho) / o.rho, o.omega() * k);
    let wrong = FanRom::new(designed(&RomModel::lossless()).fan, RomModel::default()).unwrap();
    let pw = wrong.point(&o, Q_D).unwrap();
    let nc = pw.strips.iter().map(|s| rel(s.dc_theta, k / s.r)).fold(0.0, f64::max);
    println!(
        "W46B E2: dc {e_dc:.2e}, torque {e_m:.2e}, power {e_p:.2e}, work {e_y:.2e} (limit 1e-12); NC-E2 {nc:.2e} (must be >= 1e-11)"
    );
    assert!(nc >= 1e-11, "NC-E2 does not discriminate: {nc:e}");
    for e in [e_dc, e_m, e_p, e_y] {
        assert!(e <= 1e-12, "{e:e}");
    }
}

fn z1(flows: &[f64]) {
    let o = op();
    let rom = designed(&RomModel::lossless());
    let (mut e_dp, mut e_eta, mut nc) = (0.0_f64, 0.0_f64, f64::INFINITY);
    let mut e45 = f64::NAN;
    for &f in flows {
        let p = rom.point(&o, f * Q_D).unwrap();
        let eu = euler_dp(&p, o.rho);
        e_dp = e_dp.max(rel(p.dp_t, eu));
        e_eta = e_eta.max((p.eta_t - 1.0).abs());
        let no_swirl = p.strips.iter().map(|s| s.f_x / s.pitch * dq(s)).sum::<f64>() / p.q;
        nc = nc.min(rel(no_swirl, eu));
        if f == 1.0 {
            e45 = rel(p.dp_t, DP_IDEAL);
        }
        println!("W46B Z1 q/Q_D {f}: stalled strips {}", p.stalled.len());
    }
    println!(
        "W46B Z1 {flows:?}: dp_t vs Euler {e_dp:.2e}, |eta_t - 1| {e_eta:.2e}, dp_t(Q_D) vs 45 Pa {e45:.2e} (limit 1e-12); NC-Z1 {nc:.2e}"
    );
    assert!(nc >= 1e-11, "NC-Z1 does not discriminate: {nc:e}");
    for e in [e_dp, e_eta, e45] {
        assert!(e <= 1e-12, "{e:e}");
    }
}

#[test]
#[ignore = "ADR 0119 amendment 1: registered Z1 set-up defective (0.7 Q_D has stalled strips whose post-stall drag is a loss)"]
fn z1_zero_loss_known_miss() {
    z1(&[0.7, 1.0, 1.2]);
}

#[test]
fn z1b_zero_loss_unstalled() {
    let rom = designed(&RomModel::lossless());
    for f in [0.85, 1.0, 1.2] {
        assert!(
            rom.point(&op(), f * Q_D).unwrap().stalled.is_empty(),
            "stalled strips at {f} Q_D"
        );
    }
    z1(&[0.85, 1.0, 1.2]);
}

/// C with sections at the strip radii, stagger `atan2(Omega r, c_x)` at Q_D.
fn zero_lift(camber: f64, model: RomModel) -> FanRom {
    let base = FanRom::new(base_fan(), model).unwrap();
    let cx = Q_D / base.area();
    let sections = base
        .strip_radii()
        .iter()
        .map(|&r| section(r, 0.030, (op().omega() * r).atan2(cx), camber, 0.5))
        .collect();
    FanRom::new(RomFan { sections, ..base_fan() }, model).unwrap()
}

#[test]
fn z2_zero_lift() {
    let o = op();
    let ut = o.omega() * 0.058;
    let p = zero_lift(0.0, RomModel::lossless()).point(&o, Q_D).unwrap();
    let e_dc = p.strips.iter().map(|s| s.dc_theta.abs() / s.u).fold(0.0, f64::max);
    let e_dp = p.dp_t.abs() / (o.rho * ut * ut);
    let e_p = p.power.abs() / (o.rho * Q_D * ut * ut);
    let d = zero_lift(0.0, RomModel::default()).point(&o, Q_D).unwrap();
    let nc = zero_lift(0.02, RomModel::lossless()).point(&o, Q_D).unwrap().dp_t.abs() / (o.rho * ut * ut);
    println!(
        "W46B Z2: |dc|/U {e_dc:.2e}, |dp_t| {e_dp:.2e}, |P| {e_p:.2e} (limit 1e-12); default model dp_t {:.4e} Pa, P {:.4e} W; NC-Z2 {nc:.2e}",
        d.dp_t, d.power
    );
    assert!(nc >= 1e-11, "NC-Z2 does not discriminate: {nc:e}");
    for e in [e_dc, e_dp, e_p] {
        assert!(e <= 1e-12, "{e:e}");
    }
    assert!(
        d.dp_t < 0.0 && d.power > 0.0,
        "drag only: dp_t {} P {}",
        d.dp_t,
        d.power
    );
}

#[test]
fn l1_power_ledger() {
    let o = op();
    let rom = designed(&RomModel::default());
    let (mut worst, mut nc) = (0.0_f64, [f64::INFINITY; 4]);
    for f in [0.5, 0.75, 1.0, 1.2] {
        let p = rom.point(&o, f * Q_D).unwrap();
        let led = |dp_t: f64, drag: f64, tip: f64| (p.power - (p.q * dp_t + drag + tip)).abs() / p.power;
        worst = worst.max(led(p.dp_t, p.drag_loss, p.tip_loss));
        // dp_t from a strip dp0 recomputed with a wrong model
        let recomputed =
            |dp0: &dyn Fn(&Strip) -> f64| (p.strips.iter().map(|s| dp0(s) * dq(s)).sum::<f64>() - p.tip_loss) / p.q;
        let q_dyn = |s: &Strip| 0.5 * o.rho * s.w_inf * s.w_inf * s.chord;
        let drag_flip = recomputed(&|s: &Strip| {
            q_dyn(s) * (s.cl * s.beta_inf.sin() + s.cd * s.beta_inf.cos()) / s.pitch + 0.5 * o.rho * s.dc_theta.powi(2)
        });
        let no_swirl = recomputed(&|s: &Strip| s.f_x / s.pitch);
        for (i, v) in [
            led(p.dp_t, p.drag_loss, 0.0),
            led(p.dp_t, 0.0, p.tip_loss),
            led(drag_flip, p.drag_loss, p.tip_loss),
            led(no_swirl, p.drag_loss, p.tip_loss),
        ]
        .into_iter()
        .enumerate()
        {
            nc[i] = nc[i].min(v);
        }
        println!(
            "W46B L1 q/Q_D {f}: stalled {}, residual {:.2e}",
            p.stalled.len(),
            led(p.dp_t, p.drag_loss, p.tip_loss)
        );
    }
    println!(
        "W46B L1: worst {worst:.2e} (limit 1e-12); NC (a) tip {:.2e} (b) drag {:.2e} (c) drag sign {:.2e} (d) swirl {:.2e}",
        nc[0], nc[1], nc[2], nc[3]
    );
    for (i, v) in nc.iter().enumerate() {
        assert!(*v >= 1e-11, "NC-L1 {i} does not discriminate: {v:e}");
    }
    assert!(worst <= 1e-12, "{worst:e}");
}

fn tip_formula(p: &FanPoint, gap: f64, w: f64, rho: f64) -> f64 {
    let t = p.strips.last().unwrap();
    let dp = 0.5 * rho * w * w * t.cl.abs();
    7.0 * (PI / (PI + 2.0)) * gap * t.chord * dp * (2.0 * dp / rho).sqrt()
}

#[test]
fn l2_tip_loss() {
    let o = op();
    let rom = designed(&RomModel::default());
    let with_gap = |g: f64| {
        let mut f = rom.fan.clone();
        f.tip_gap = g;
        FanRom::new(f, rom.model).unwrap().point(&o, Q_D).unwrap()
    };
    let (p0, p1, p2) = (with_gap(0.0), with_gap(0.001), with_gap(0.002));
    let t = p1.strips.last().unwrap();
    let e_f = rel(p1.tip_loss, tip_formula(&p1, 0.001, t.w_inf, o.rho));
    let e_2 = rel(p2.tip_loss, 2.0 * p1.tip_loss);
    let w1 = (t.cx * t.cx + t.u * t.u).sqrt();
    let nc = rel(tip_formula(&p1, 0.001, w1, o.rho), p1.tip_loss);
    println!(
        "W46B L2: formula {e_f:.2e}, doubling {e_2:.2e} (limit 1e-12); tip loss {:.5} W; eta_t gap 0/1/2 mm {:.5} {:.5} {:.5}; NC-L2 {nc:.2e}",
        p1.tip_loss, p0.eta_t, p1.eta_t, p2.eta_t
    );
    assert!(nc >= 1e-11, "NC-L2 does not discriminate: {nc:e}");
    assert!(e_f <= 1e-12 && e_2 <= 1e-12, "{e_f:e} {e_2:e}");
    assert!(p0.tip_loss == 0.0 && p0.eta_t > p1.eta_t && p1.eta_t > p2.eta_t);
}

#[test]
fn l3_drag_terms() {
    let o = op();
    let p = designed(&RomModel::default()).point(&o, Q_D).unwrap();
    assert!(p.stalled.is_empty());
    let h = 0.058 - 0.030;
    let sum = |annulus: f64| -> f64 {
        p.strips
            .iter()
            .map(|s| {
                let cd = 2.656 / s.re.sqrt() + annulus * s.pitch / h + 0.018 * s.cl * s.cl;
                7.0 * 0.5 * o.rho * s.w_inf.powi(3) * s.chord * cd * s.dr
            })
            .sum()
    };
    let e = rel(p.drag_loss, sum(0.020));
    let nc = rel(sum(0.0), p.drag_loss);
    println!(
        "W46B L3: drag loss {:.5} W, error {e:.2e} (limit 1e-12); NC-L3 {nc:.2e}",
        p.drag_loss
    );
    assert!(nc >= 1e-11, "NC-L3 does not discriminate: {nc:e}");
    assert!(e <= 1e-12, "{e:e}");
}

#[test]
fn f1_fan_laws() {
    let rom = designed(&RomModel::default());
    let o1 = op();
    let o2 = Operating {
        rpm: 4500.0,
        rho: 1.0,
        nu: 2.25e-5,
    };
    let o2_fixed_nu = Operating { nu: o1.nu, ..o2 };
    let (n, rr) = (1.5, 1.0 / 1.2);
    let (mut worst, mut nc, mut re_dev) = (0.0_f64, f64::INFINITY, 0.0_f64);
    for f in [0.6, 1.0, 1.2] {
        let p1 = rom.point(&o1, f * Q_D).unwrap();
        let p2 = rom.point(&o2, 1.5 * f * Q_D).unwrap();
        let (q, dp_t, pw) = fan_law(p1.q, p1.dp_t, p1.power, n, rr);
        let (_, dp_s, _) = fan_law(p1.q, p1.dp_s, p1.power, n, rr);
        for e in [
            rel(p2.q, q),
            rel(p2.dp_t, dp_t),
            rel(p2.dp_s, dp_s),
            rel(p2.power, pw),
            (p2.eta_t - p1.eta_t).abs(),
        ] {
            worst = worst.max(e);
        }
        nc = nc.min(rel(p1.power * rr * n * n, p2.power));
        let p3 = rom.point(&o2_fixed_nu, 1.5 * f * Q_D).unwrap();
        re_dev = re_dev.max(rel(p3.dp_t, dp_t)).max(rel(p3.power, pw));
        println!(
            "W46B F1 q/Q_D {f}: nu held fixed: dp_t {:.4} vs fan law {:.4} Pa, P {:.4} vs {:.4} W, eta_t {:.4} vs {:.4}",
            p3.dp_t, dp_t, p3.power, pw, p3.eta_t, p1.eta_t
        );
    }
    println!("W46B F1: worst {worst:.2e} (limit 1e-12); NC-F1 {nc:.2e}; Reynolds deviation with nu fixed (reported) {re_dev:.3e}");
    assert!(nc >= 1e-11, "NC-F1 does not discriminate: {nc:e}");
    assert!(worst <= 1e-12, "{worst:e}");
}

fn k_r1b(rom: &FanRom) -> f64 {
    rom.point(&op(), Q_D).unwrap().dp_s / (Q_D * Q_D)
}

#[test]
fn r1_operating_point_analytic_pair() {
    let fan = |q: f64| -> Result<f64, String> { Ok(40.0 - 1e4 * q * q) };
    let sys = SystemCurve { k: 5e3, l: 200.0 };
    let o = operating_point(&fan, &sys, 1e-3, 0.1, 100).unwrap();
    let exact = (-200.0 + (200.0_f64 * 200.0 + 4.0 * 1.5e4 * 40.0).sqrt()) / (2.0 * 1.5e4);
    let no_lin = (40.0_f64 / 1.5e4).sqrt();
    let (e, nc) = (rel(o.q, exact), rel(no_lin, o.q));
    let rom = designed(&RomModel::default());
    let k = k_r1b(&rom);
    let (ob, pb) = rom
        .operating_point(&op(), &SystemCurve { k, l: 0.0 }, 1e-3, 0.1, 100)
        .unwrap();
    let (res, dq_) = ((pb.dp_s - k * ob.q * ob.q).abs(), rel(ob.q, Q_D));
    println!(
        "W46B R1: q* {:.15e} vs {exact:.15e}, error {e:.2e} (limit 1e-12), crossings {}; NC-R1 {nc:.2e}; R1b residual {res:.2e} Pa, |q - Q_D|/Q_D {dq_:.2e}",
        o.q, o.crossings
    );
    assert!(nc >= 1e-11, "NC-R1 does not discriminate: {nc:e}");
    assert!(e <= 1e-12 && o.crossings == 1, "{e:e}");
    assert!(res <= 1e-10 && dq_ <= 1e-9, "{res:e} {dq_:e}");
}

#[test]
fn r2_no_intersection_is_located() {
    let rom = designed(&RomModel::default());
    let k = k_r1b(&rom);
    let e = rom
        .operating_point(&op(), &SystemCurve { k: 1e3 * k, l: 0.0 }, 1e-3, 0.1, 100)
        .unwrap_err();
    println!("W46B R2: {e}");
    assert!(
        rom.operating_point(&op(), &SystemCurve { k, l: 0.0 }, 1e-3, 0.1, 100)
            .is_ok(),
        "NC-R2"
    );
    assert!(
        e.contains("no intersection") && e.contains("[1.000000e-3, 1.000000e-1]"),
        "{e}"
    );
}

#[test]
fn r3_stall_is_located() {
    let rom = designed(&RomModel::default());
    let k = k_r1b(&rom);
    let e = rom
        .operating_point(&op(), &SystemCurve { k: 20.0 * k, l: 0.0 }, 1e-3, 0.1, 100)
        .unwrap_err();
    let p75 = rom.point(&op(), 0.75 * Q_D).unwrap();
    let k75 = p75.dp_s / (0.75 * Q_D).powi(2);
    let eb = rom
        .operating_point(&op(), &SystemCurve { k: k75, l: 0.0 }, 1e-3, 0.1, 100)
        .unwrap_err();
    println!("W46B R3: {e}\nW46B R3b: {eb}");
    assert!(
        rom.operating_point(&op(), &SystemCurve { k, l: 0.0 }, 1e-3, 0.1, 100)
            .is_ok(),
        "NC-R3"
    );
    assert!(e.contains("stall"), "{e}");
    let r0 = format!("{:.4}", p75.strips[p75.stalled[0]].r);
    assert!(eb.contains("stalled") && eb.contains(&r0), "{eb}");
}

/// (dp_t and dp_s strictly decreasing, interior eta_t maximum by >= 1e-3, the margin) on C at q/Q_D = 0.50 .. 1.30.
fn m1_holds(model: RomModel) -> (bool, bool, f64) {
    let rom = FanRom::new(designed(&RomModel::default()).fan, model).unwrap();
    let ps: Vec<FanPoint> = (0..=16)
        .map(|j| rom.point(&op(), Q_D * (0.5 + 0.05 * j as f64)).unwrap())
        .collect();
    let dec = ps.windows(2).all(|w| w[1].dp_t < w[0].dp_t && w[1].dp_s < w[0].dp_s);
    let imax = (0..ps.len()).fold(0, |a, i| if ps[i].eta_t > ps[a].eta_t { i } else { a });
    let margin = ps[imax].eta_t - ps[0].eta_t.max(ps[16].eta_t);
    (dec, imax > 0 && imax < 16 && margin >= 1e-3, margin)
}

#[test]
fn m1_monotone_curve_and_efficiency_maximum() {
    let (dec, peak, margin) = m1_holds(RomModel {
        cl_max: 100.0,
        ..RomModel::default()
    });
    let (_, nc_peak, nc_margin) = m1_holds(RomModel {
        cl_max: 100.0,
        ..RomModel::lossless()
    });
    println!(
        "W46B M1: decreasing {dec}, interior eta_t maximum {peak} (margin {margin:.4}); NC-M1 peak {nc_peak} (margin {nc_margin:.2e})"
    );
    assert!(!nc_peak, "NC-M1 does not discriminate");
    assert!(dec && peak);
}

#[test]
fn g1_gutin_closed_form() {
    let o = op();
    let rom = designed(&RomModel::default());
    let p = rom.point(&o, Q_D).unwrap();
    let r = 0.8 * 0.058;
    let at_r: Vec<(f64, f64, f64)> = p
        .strips
        .iter()
        .map(|s| (r, 7.0 * s.f_x * s.dr, 7.0 * s.f_theta * s.r * s.dr))
        .collect();
    let thrust: f64 = p
        .strips
        .iter()
        .map(|s| {
            let d = 0.5 * o.rho * s.w_inf * s.w_inf * s.chord * s.cd;
            (o.rho * s.u * s.dc_theta - d * s.w_inf / (s.pitch * s.cx) - 0.5 * o.rho * s.dc_theta.powi(2))
                * 2.0
                * PI
                * s.r
                * s.dr
        })
        .sum();
    let torque: f64 = p.strips.iter().map(|s| o.rho * s.r * s.dc_theta * dq(s)).sum();
    let rot = |torque: f64| Rotor {
        blades: 7,
        omega: o.omega(),
        r_tip: r,
        thrust,
        torque,
        c0: 343.0,
    };
    let (mut worst, mut nc) = (0.0_f64, f64::INFINITY);
    let th = PI / 3.0;
    for m in [1, 2] {
        let sum = tonal_harmonic(7, o.omega(), 343.0, &at_r, m, 1.0, th).unwrap();
        worst = worst.max(rel(sum, rot(torque).harmonic_amplitude(m, 1.0, th).unwrap()));
        nc = nc.min(rel(rot(-torque).harmonic_amplitude(m, 1.0, th).unwrap(), sum));
    }
    let full: Vec<f64> = [PI / 2.0, PI / 3.0]
        .iter()
        .map(|&t| rom.tone(&o, &p, 343.0, 1, 1.0, t).unwrap())
        .collect();
    let compact = tonal_harmonic(7, o.omega(), 343.0, &at_r, 1, 1.0, PI / 2.0).unwrap();
    let spl = |a: f64| rcfd_post::acoustics::spl_db(a / 2f64.sqrt()).unwrap();
    println!(
        "W46B G1: worst {worst:.2e} (limit 1e-12); NC-G1 {nc:.2e}; reported BPF1 at 1 m: 90 deg {:.3e} Pa ({:.1} dB), 60 deg {:.3e} Pa ({:.1} dB), compact at 0.8 r_tip 90 deg {compact:.3e} Pa; thrust {thrust:.6} N vs {:.6}, torque {torque:.7} vs {:.7} N m",
        full[0],
        spl(full[0]),
        full[1],
        spl(full[1]),
        p.thrust,
        p.torque
    );
    assert!(nc >= 1e-11, "NC-G1 does not discriminate: {nc:e}");
    assert!(worst <= 1e-12, "{worst:e}");
}

#[test]
fn d1_harmonic_amplitude_bit_identical() {
    let rt = Rotor {
        blades: 5,
        omega: 300.0,
        r_tip: 0.05,
        thrust: 0.4,
        torque: 0.01,
        c0: 343.0,
    };
    for m in 1..=3 {
        for th in [0.3, PI / 2.0, 2.5] {
            let n = m * rt.blades;
            let j = rcfd_post::rotor::bessel_j(n, n as f64 * rt.tip_mach() * f64::sin(th).abs()).unwrap();
            let load = rt.thrust * th.cos() + rt.torque * rt.c0 / (rt.omega * rt.r_tip * rt.r_tip);
            let old = n as f64 * rt.omega / (2.0 * PI * rt.c0 * 2.0) * j.abs() * load.abs();
            assert_eq!(rt.harmonic_amplitude(m, 2.0, th).unwrap().to_bits(), old.to_bits());
        }
    }
}

#[test]
#[ignore = "ADR 0119 V1: reported FAN-01 comparison, run once"]
fn v1_fan01_reported() {
    let (fan, data) = fan01();
    let rom = FanRom::new(fan, RomModel::default()).unwrap();
    let o = Operating {
        rpm: 1486.0,
        rho: 1.2,
        nu: 1.5e-5,
    };
    let (mut s2, mut sp2, mut onset) = (0.0, 0.0, 0.0_f64);
    let (mut re_min, mut re_max) = (f64::INFINITY, 0.0_f64);
    for d in &data {
        let p = rom.point(&o, d[0]).unwrap();
        let eta_m = d[0] * d[1] / d[2];
        println!(
            "W46B V1 q {:.4}: dp_s {:8.2} vs {:8.2} Pa ({:+8.2}), P {:7.2} vs {:7.2} W ({:+6.1} %), eta_s {:.3} vs {:.3}, dp_t {:8.2}, stalled {}",
            d[0],
            p.dp_s,
            d[1],
            p.dp_s - d[1],
            p.power,
            d[2],
            100.0 * (p.power / d[2] - 1.0),
            p.eta_s,
            eta_m,
            p.dp_t,
            p.stalled.len()
        );
        s2 += (p.dp_s - d[1]).powi(2);
        sp2 += (p.power / d[2] - 1.0).powi(2);
        if !p.stalled.is_empty() {
            onset = onset.max(d[0]);
        }
        for s in &p.strips {
            re_min = re_min.min(s.re);
            re_max = re_max.max(s.re);
        }
    }
    let n = data.len() as f64;
    println!(
        "W46B V1: rms dp_s difference {:.2} Pa, rms power difference {:.1} %, ROM stall onset (largest measured q with a stalled strip) {onset:.4} m^3/s (measured about 1.1), chord Re {re_min:.3e}..{re_max:.3e}",
        (s2 / n).sqrt(),
        100.0 * (sp2 / n).sqrt()
    );
}

// ---------------------------------------------------------------- amendment 2 (ADR 0119)

#[test]
fn s1_static_pressure_deductions() {
    let o = op();
    let rom = designed(&RomModel::default());
    let (mut worst, mut nc_swirl, mut nc_axial) = (0.0_f64, f64::INFINITY, f64::INFINITY);
    for f in [0.5, 0.75, 1.0, 1.2] {
        let p = rom.point(&o, f * Q_D).unwrap();
        let cx = p.strips[0].cx;
        let axial = 0.5 * o.rho * cx * cx;
        let swirl = p
            .strips
            .iter()
            .map(|s| 0.5 * o.rho * s.dc_theta * s.dc_theta * dq(s))
            .sum::<f64>()
            / p.q;
        let d = p.dp_t - p.dp_s;
        worst = worst.max(rel(d, axial + swirl));
        nc_swirl = nc_swirl.min(rel(axial, d));
        nc_axial = nc_axial.min(rel(swirl, d));
    }
    println!("W46B S1: worst {worst:.2e} (limit 1e-12); NC swirl omitted {nc_swirl:.2e}, axial omitted {nc_axial:.2e}");
    assert!(nc_swirl >= 1e-11 && nc_axial >= 1e-11, "NC-S1 does not discriminate");
    assert!(worst <= 1e-12, "{worst:e}");
}

/// The only `rcfd_mesh::fan::Station` literal of this file (the schema is being extended in parallel).
fn station(radius: f64, chord: f64, stagger: f64) -> rcfd_mesh::fan::Station {
    rcfd_mesh::fan::Station {
        radius,
        chord,
        stagger,
        camber: 0.04,
        camber_pos: 0.4,
        thickness: 0.1,
        sweep: 0.0,
        lean: 0.0,
    }
}

#[test]
fn p1_from_axial_fan() {
    use rcfd_mesh::fan::AxialFan;
    let mk = |n: usize| {
        AxialFan::linear(
            n,
            0.03,
            0.058,
            0.0015,
            0.1,
            0.02,
            SectionFamily::Naca4,
            station(0.0, 0.03, 0.6),
            station(0.0, 0.025, 1.1),
        )
    };
    let f = mk(7);
    let r = RomFan::from_axial_fan(&f).unwrap();
    assert_eq!(
        (r.n_blades, r.r_hub, r.r_tip, r.tip_gap, r.family),
        (7, 0.03, 0.058, 0.0015, SectionFamily::Naca4)
    );
    assert_eq!(r.sections.len(), f.stations.len());
    for (a, b) in r.sections.iter().zip(&f.stations) {
        assert_eq!(
            (a.radius, a.chord, a.stagger, a.camber, a.camber_pos),
            (b.radius, b.chord, b.stagger, b.camber, b.camber_pos)
        );
    }
    let e = RomFan::from_axial_fan(&mk(1)).unwrap_err();
    println!("W46B P1: invalid fan -> {e}");
    assert!(e.contains("n_blades"), "{e}");
}
