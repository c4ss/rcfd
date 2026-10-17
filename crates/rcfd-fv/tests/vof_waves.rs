//! ADR 0052 checks W1-W5 (wave inlet and relaxation-zone beach in the VOF host, 2-D numerical wave tank). The limits are those
//! registered in the ADR before any run; none was changed afterwards.
//!
//! Tank: depth 0.30 m, T = 1 s, H = 0.06 m (Airy), 4.5 lambda long, 0.45 m high, dx = lambda/32, dy = 0.01 m, dt = 2.5e-3 s,
//! 10 periods, beach (relaxation to still water) on [3 lambda, 4.5 lambda].

use rcfd_fv::vof::{indicator_fraction, RelaxTarget, RelaxZone, VofSettings, VofSolver, WaveInlet, WaveSettings};
use rcfd_fv::waves::{Blend, Wave};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const G: f64 = 9.81;
const DEPTH: f64 = 0.30;
const PERIOD: f64 = 1.0;
const HEIGHT: f64 = 0.06;
const LY: f64 = 0.45;
const NY: usize = 45;
const DT: f64 = 2.5e-3;

struct Tank {
    s: VofSolver,
    lambda: f64,
    k: f64,
    nx: usize,
    lx: f64,
}

fn tank(height: f64, beach: bool, t_ramp: f64, n_correctors: usize) -> Tank {
    let omega = 2.0 * std::f64::consts::PI / PERIOD;
    let wave = Wave::new(0.5 * height, omega, DEPTH, G, 0.0, 1).unwrap();
    let lambda = 2.0 * std::f64::consts::PI / wave.k;
    let nx = (4.5f64 * 32.0).round() as usize;
    let lx = 4.5 * lambda;
    let mesh = Arc::new(box_2d(nx, NY, lx, LY, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    p_rgh.set_bc(&geom, "ymax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| x[1] <= DEPTH);
    let mut s = VofSettings::new([1000.0, 1.0], [1e-3, 1.8e-5]);
    s.gravity = [0.0, -G, 0.0];
    s.x_ref = [0.0, LY, 0.0];
    s.n_correctors = n_correctors;
    s.amg_pressure = true;
    s.waves = Some(WaveSettings {
        level: DEPTH,
        inlet: Some(WaveInlet {
            patch: "xmin".into(),
            wave,
            t_ramp,
        }),
        zones: if beach {
            vec![RelaxZone {
                x_inner: 3.0 * lambda,
                x_outer: 4.5 * lambda,
                blend: Blend::exponential(),
                target: RelaxTarget::StillWater,
            }]
        } else {
            vec![]
        },
    });
    let s = VofSolver::new(geom, u, p_rgh, alpha, s).unwrap();
    Tank {
        s,
        lambda,
        k: wave.k,
        nx,
        lx,
    }
}

/// Free-surface elevation (above the still level) at the column nearest to `x`: the column integral of alpha.
fn eta_at(t: &Tank, x: f64) -> f64 {
    let dx = t.lx / t.nx as f64;
    let i = ((x / dx - 0.5).round() as usize).min(t.nx - 1);
    let a = &t.s.transport.alpha.values;
    let dy = LY / NY as f64;
    (0..NY).map(|j| a[j * t.nx + i] * dy).sum::<f64>() - DEPTH
}

#[derive(Clone, Copy)]
struct Cx(f64, f64);
impl Cx {
    fn mul(self, o: Cx) -> Cx {
        Cx(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn sub(self, o: Cx) -> Cx {
        Cx(self.0 - o.0, self.1 - o.1)
    }
    fn div(self, o: Cx) -> Cx {
        let d = o.0 * o.0 + o.1 * o.1;
        Cx((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
    fn abs(self) -> f64 {
        self.0.hypot(self.1)
    }
    fn exp_i(a: f64) -> Cx {
        Cx(a.cos(), a.sin())
    }
}

/// Complex first-harmonic amplitude `B` of `eta = Re[B e^{-i omega t}]` over `samples` (integer number of periods).
fn harmonic(samples: &[f64], t0: f64, omega: f64) -> Cx {
    let (mut re, mut im) = (0.0, 0.0);
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    for (i, &f) in samples.iter().enumerate() {
        let t = t0 + (i as f64 + 1.0) * DT;
        re += (f - mean) * (omega * t).cos();
        im += (f - mean) * (omega * t).sin();
    }
    let n = samples.len() as f64;
    Cx(2.0 * re / n, 2.0 * im / n)
}

/// Goda and Suzuki (1976) two-gauge separation: incident and reflected amplitudes for gauges at `x1`, `x2 = x1 + delta`.
fn reflection(b1: Cx, b2: Cx, k: f64, x1: f64, x2: f64) -> (f64, f64) {
    let e = |x: f64| Cx::exp_i(k * x);
    let en = |x: f64| Cx::exp_i(-k * x);
    // [e^{ikx1} e^{-ikx1}; e^{ikx2} e^{-ikx2}] [A_I; A_R] = [B1; B2]
    let det = e(x1).mul(en(x2)).sub(en(x1).mul(e(x2)));
    let a_i = b1.mul(en(x2)).sub(b2.mul(en(x1))).div(det);
    let a_r = e(x1).mul(b2).sub(e(x2).mul(b1)).div(det);
    (a_i.abs(), a_r.abs())
}

/// Zero up-crossing spacing: mean period of `samples` (mean removed), linear interpolation.
fn mean_period(samples: &[f64]) -> f64 {
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let mut ups: Vec<f64> = Vec::new();
    for i in 1..samples.len() {
        let (a, b) = (samples[i - 1] - mean, samples[i] - mean);
        if a < 0.0 && b >= 0.0 {
            ups.push((i as f64 - 1.0 + (-a) / (b - a)) * DT);
        }
    }
    (ups[ups.len() - 1] - ups[0]) / (ups.len() - 1) as f64
}

struct Record {
    g1: Vec<f64>,
    g2: Vec<f64>,
    r1: Vec<f64>,
    r2: Vec<f64>,
    dv_identity: f64,
    level_drift: f64,
    alpha_lo: f64,
    alpha_hi: f64,
    secs: f64,
}

fn run_tank(beach: bool) -> (Record, Tank) {
    run_tank_periods(beach, 10)
}

fn run_tank_periods(beach: bool, periods: usize) -> (Record, Tank) {
    let mut t = tank(HEIGHT, beach, 2.0 * PERIOD, 3);
    let (x_g1, x_g2) = (0.5 * t.lambda, 1.5 * t.lambda);
    let (x_r1, x_r2) = (2.0 * t.lambda, 2.25 * t.lambda);
    let n_steps = (periods as f64 * PERIOD / DT).round() as usize;
    let v0 = t.s.phase_volume();
    let mut rec = Record {
        g1: vec![],
        g2: vec![],
        r1: vec![],
        r2: vec![],
        dv_identity: 0.0,
        level_drift: 0.0,
        alpha_lo: 0.0,
        alpha_hi: 1.0,
        secs: 0.0,
    };
    let (mut out, mut relax_cum) = (0.0, 0.0);
    let t0 = std::time::Instant::now();
    for n in 0..n_steps {
        let i = t.s.step(DT).unwrap();
        out += DT * i.alpha_outflow;
        relax_cum += i.relax_volume;
        rec.alpha_lo = rec.alpha_lo.min(i.alpha_min);
        rec.alpha_hi = rec.alpha_hi.max(i.alpha_max);
        if n >= 5 * (PERIOD / DT) as usize {
            rec.g1.push(eta_at(&t, x_g1));
            rec.g2.push(eta_at(&t, x_g2));
            rec.r1.push(eta_at(&t, x_r1));
            rec.r2.push(eta_at(&t, x_r2));
        }
    }
    rec.secs = t0.elapsed().as_secs_f64();
    let dv = t.s.phase_volume() - v0;
    rec.dv_identity = (dv + out - relax_cum).abs() / v0;
    let thick = t.s.geom.mesh.total_volume() / (t.lx * LY);
    rec.level_drift = dv / (t.lx * thick);
    (rec, t)
}

#[test]
fn w1_w3_generation_and_beach() {
    let omega = 2.0 * std::f64::consts::PI / PERIOD;
    let (rec, t) = run_tank(true);
    // window t in [6T, 10T]: the samples stored from step 2000 on are t in (5T, 10T]; take the last 4 periods
    let per = (PERIOD / DT).round() as usize;
    let skip = per; // 5T..6T dropped
    let w1 = &rec.g1[skip..];
    let w2 = &rec.g2[skip..];
    let t_start = 6.0 * PERIOD;
    let h1 = 2.0 * harmonic(w1, t_start, omega).abs();
    let h2 = 2.0 * harmonic(w2, t_start, omega).abs();
    let (p1, p2) = (mean_period(w1), mean_period(w2));
    // reflection window t in [7T, 10T]
    let skip_r = 2 * per;
    let (b1, b2) = (
        harmonic(&rec.r1[skip_r..], 7.0 * PERIOD, omega),
        harmonic(&rec.r2[skip_r..], 7.0 * PERIOD, omega),
    );
    let (ai, ar) = reflection(b1, b2, t.k, 2.0 * t.lambda, 2.25 * t.lambda);
    let kr = ar / ai;
    println!(
        "W1 H(G1 0.5 lambda) = {h1:.4} m ({:.1} percent of 0.06; limit 12), H(G2 1.5 lambda) = {h2:.4} m ({:.1} percent; limit 25), period G1 {p1:.4} G2 {p2:.4} s (limit 2 percent); W2 K_r = {kr:.4} (incident {ai:.4}, reflected {ar:.4}; limit 0.15); W3 volume identity {:.2e} (limit 1e-10), level drift {:.2e} m (limit {:.1e}), alpha [{:.2e}, {:.9}]; {:.0} s, lambda {:.4} m",
        100.0 * (h1 / HEIGHT - 1.0),
        100.0 * (h2 / HEIGHT - 1.0),
        rec.dv_identity,
        rec.level_drift,
        0.1 * HEIGHT,
        rec.alpha_lo,
        rec.alpha_hi,
        rec.secs,
        t.lambda
    );
    assert!((h1 / HEIGHT - 1.0).abs() <= 0.12, "W1 G1 height {h1}");
    assert!((h2 / HEIGHT - 1.0).abs() <= 0.25, "W1 G2 height {h2}");
    assert!(
        (p1 / PERIOD - 1.0).abs() <= 0.02 && (p2 / PERIOD - 1.0).abs() <= 0.02,
        "W1 period {p1} {p2}"
    );
    assert!(kr <= 0.15, "W2 K_r {kr}");
    assert!(rec.dv_identity <= 1e-10, "W3 identity {}", rec.dv_identity);
    assert!(rec.level_drift.abs() <= 0.1 * HEIGHT, "W3 drift {}", rec.level_drift);
    assert!(rec.alpha_lo >= -1e-9 && rec.alpha_hi <= 1.0 + 1e-9, "W3 bounds");
}

/// KNOWN MISS, SET-UP DEFECT (ADR 0052 addendum 2): the registered control with the registered 10-period run. The reflection
/// window [7T, 10T] ends before the wall-reflected wave group has returned to the gauges (group speed about 0.9 m/s: the front
/// needs about 10 s to reach the wall at 4.5 lambda and come back to 2 lambda), so the measured K_r = 0.061 is "no reflected wave
/// yet", not a reflection coefficient. The same defect makes the beach K_r of `w1_w3_generation_and_beach` vacuous. The limit
/// (control K_r >= 0.5) is unchanged; the corrected tests `w2b_*` below use a 20-period run and the window [16T, 20T].
#[test]
#[ignore = "known miss, set-up defect: window before the reflected front arrives; see w2b_control_long_window"]
fn w2_control_reflecting_end_wall_known_miss() {
    let omega = 2.0 * std::f64::consts::PI / PERIOD;
    let (rec, t) = run_tank(false);
    let per = (PERIOD / DT).round() as usize;
    let skip_r = 2 * per;
    let (b1, b2) = (
        harmonic(&rec.r1[skip_r..], 7.0 * PERIOD, omega),
        harmonic(&rec.r2[skip_r..], 7.0 * PERIOD, omega),
    );
    let (ai, ar) = reflection(b1, b2, t.k, 2.0 * t.lambda, 2.25 * t.lambda);
    let kr = ar / ai;
    println!(
        "W2 control (no beach): K_r = {kr:.4} (incident {ai:.4}, reflected {ar:.4}; limit >= 0.5); {:.0} s",
        rec.secs
    );
    assert!(kr >= 0.5, "W2 control K_r {kr}");
}

/// Max |U| and max |alpha - alpha_0| over 200 steps of still water with the beach and a zero-height inlet.
fn still_water(n_correctors: usize, beach: bool) -> (f64, f64) {
    let mut t = tank(0.0, beach, 0.01, n_correctors);
    let a0 = t.s.transport.alpha.values.clone();
    let (mut worst_u, mut worst_a) = (0.0f64, 0.0f64);
    for _ in 0..200 {
        t.s.step(DT).unwrap();
        worst_u =
            t.s.u
                .values
                .iter()
                .map(|v| v[0].abs().max(v[1].abs()))
                .fold(worst_u, f64::max);
        worst_a =
            t.s.transport
                .alpha
                .values
                .iter()
                .zip(&a0)
                .map(|(a, b)| (a - b).abs())
                .fold(worst_a, f64::max);
    }
    (worst_u, worst_a)
}

/// KNOWN MISS (ADR 0052 addendum 2): the registered W4 with the registered 3 PISO correctors. The same noise is present WITHOUT the
/// zone (it is the base splitting error of the thin, nearly inviscid air layer, `(mu_air dt / (rho_air dy^2))^n_correctors`), so
/// the zone is not its cause; the limits stay as registered.
#[test]
#[ignore = "known miss: base PISO splitting noise 1.3e-8 > 1e-10 with 3 correctors (see w4b for the corrected set-up)"]
fn w4_still_water_with_zone_known_miss() {
    let (worst_u, worst_a) = still_water(3, true);
    println!(
        "W4 (3 correctors): max |U| {worst_u:.2e} (limit 1e-10), max |alpha - alpha_0| {worst_a:.2e} (limit 1e-12)"
    );
    assert!(worst_u <= 1e-10, "W4 U {worst_u}");
    assert!(worst_a <= 1e-12, "W4 alpha {worst_a}");
}

/// Corrected set-up (the registered limits, 4 PISO correctors): no spurious current from the zone.
#[test]
fn w4b_still_water_with_zone_four_correctors() {
    let (worst_u, worst_a) = still_water(4, true);
    let (base_u, base_a) = still_water(4, false);
    println!("W4b (4 correctors) with beach: max |U| {worst_u:.2e} (limit 1e-10), max |alpha - alpha_0| {worst_a:.2e} (limit 1e-12); without beach: {base_u:.2e}, {base_a:.2e}");
    assert!(worst_u <= 1e-10, "W4b U {worst_u}");
    assert!(worst_a <= 1e-12, "W4b alpha {worst_a}");
}

/// Diagnosis record: with the registered 3 correctors the noise is the same without the zone (so it is not the zone).
#[test]
fn w4_noise_is_independent_of_the_zone() {
    let (with_zone, _) = still_water(3, true);
    let (without, _) = still_water(3, false);
    println!("W4 3 correctors: max |U| with beach {with_zone:.3e}, without {without:.3e}");
    assert!(
        (with_zone - without).abs() <= 1e-3 * without,
        "zone changes the noise level"
    );
}

#[test]
fn w5_inlet_values_reproduce_the_wave() {
    let omega = 2.0 * std::f64::consts::PI / PERIOD;
    let mut t = tank(HEIGHT, false, 0.01, 3);
    let wave = Wave::new(0.5 * HEIGHT, omega, DEPTH, G, 0.0, 1).unwrap();
    let geom = t.s.geom.clone();
    let faces = geom.mesh.patch("xmin").unwrap().faces();
    let dy = LY / NY as f64;
    let mut worst_eta: f64 = 0.0;
    let mut worst_q: f64 = 0.0;
    let mut checked = 0;
    for _ in 0..400 {
        t.s.step(DT).unwrap();
        let ph = (t.s.time / PERIOD).fract();
        // three phases: crest-ish, zero down-crossing, trough
        if [0.0f64, 0.25, 0.5, 0.75, 1.0]
            .iter()
            .any(|p| (ph - p).abs() < 0.5 * DT / PERIOD)
            && t.s.time > 0.1
        {
            let ub = t.s.u.boundary_values(&geom);
            let ab = t.s.transport.alpha.boundary_values(&geom);
            let q_num: f64 = faces.clone().map(|f| ub[f - geom.n_internal][0] * dy).sum();
            let eta_num: f64 = faces.clone().map(|f| ab[f - geom.n_internal] * dy).sum::<f64>() - DEPTH;
            let eta = wave.eta(0.0, t.s.time);
            // exact column flux int_{-h}^{eta} u dz by fine midpoint quadrature
            let nq = 20000;
            let h = (eta + DEPTH) / nq as f64;
            let q: f64 = (0..nq)
                .map(|i| wave.velocity(0.0, -DEPTH + (i as f64 + 0.5) * h, t.s.time).0 * h)
                .sum();
            worst_eta = worst_eta.max((eta_num - eta).abs());
            let qs = (omega * HEIGHT / 2.0) * 1.0 / wave.k; // flux amplitude scale a omega / k
            worst_q = worst_q.max((q_num - q).abs() / qs);
            checked += 1;
            println!(
                "W5 t = {:.4}: eta {eta:.5} (inlet {eta_num:.5}), column flux {q:.5} (inlet {q_num:.5})",
                t.s.time
            );
        }
    }
    println!("W5 worst |eta_inlet - eta| {worst_eta:.2e} m, worst flux error {worst_q:.3} of the amplitude scale (limit 0.03); {checked} samples");
    assert!(checked >= 3);
    assert!(worst_eta <= 1e-12, "W5 eta {worst_eta}");
    assert!(worst_q <= 0.03, "W5 flux {worst_q}");
}

/// Reflection coefficient of a `periods`-long run from the last 4 periods (Goda-Suzuki, gauges at 2.0 and 2.25 lambda).
fn long_window_kr(beach: bool) -> (f64, f64, f64, f64, f64) {
    let omega = 2.0 * std::f64::consts::PI / PERIOD;
    let periods = 20usize;
    let (rec, t) = run_tank_periods(beach, periods);
    let per = (PERIOD / DT).round() as usize;
    // records start at t = 5T
    let skip = (periods - 4 - 5) * per;
    let t_start = (periods - 4) as f64 * PERIOD;
    let (b1, b2) = (
        harmonic(&rec.r1[skip..], t_start, omega),
        harmonic(&rec.r2[skip..], t_start, omega),
    );
    let (ai, ar) = reflection(b1, b2, t.k, 2.0 * t.lambda, 2.25 * t.lambda);
    (ar / ai, ai, ar, rec.dv_identity, rec.secs)
}

/// Corrected W2 control (same limit as the registered one, K_r >= 0.5): 20 periods, window [16T, 20T].
#[test]
fn w2b_control_long_window() {
    let (kr, ai, ar, _, secs) = long_window_kr(false);
    println!("W2b control (no beach, 20 periods): K_r = {kr:.4} (incident {ai:.4}, reflected {ar:.4}; limit >= 0.5); {secs:.0} s");
    assert!(kr >= 0.5, "W2b control K_r {kr}");
}

/// Corrected W2 beach (same limit as the registered one, K_r <= 0.15): 20 periods, window [16T, 20T].
#[test]
fn w2b_beach_long_window() {
    let (kr, ai, ar, dv, secs) = long_window_kr(true);
    println!(
        "W2b beach (20 periods): K_r = {kr:.4} (incident {ai:.4}, reflected {ar:.4}; limit <= 0.15); volume identity {dv:.2e}; {secs:.0} s"
    );
    assert!(kr <= 0.15, "W2b beach K_r {kr}");
    assert!(dv <= 1e-10, "W3b identity {dv}");
}
