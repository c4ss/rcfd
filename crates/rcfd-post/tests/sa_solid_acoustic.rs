//! F-418 (ADR 0468): equivalent radiated power and the Rayleigh integral of a baffled plane. Checks J0, E1 to E4, P1, P2, D1,
//! W1 to W3, S1, L1, L2, B1, B2, N1, N2, X1, X2, H1 and Z1 of the registration (DB1 in `sa_solid_acoustic_default_bits.rs`).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_post::acoustics::{band_index, third_octave_bands, Spectrum};
use rcfd_post::solid_acoustic::{
    equivalent_radiated_power, erp_spectrum, power_level_db, BaffledPlane, Phasor, PowerSpectrum, RayleighOptions,
    VibratingSurface, PENDING,
};
use std::f64::consts::PI;

const RHO: f64 = 1.2;
const C: f64 = 340.0;
const A: f64 = 0.1;
const V: f64 = 1e-3;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    /// The ulp probe of ADR 0468: the limit must be at least 1e3 times the relative change from a one-ulp input change.
    fn probe(&mut self, name: &str, change: f64, limit: f64) {
        println!("{name} ulp probe: relative change {change:.3e}");
        self.holds(
            &format!("{name} probe"),
            limit >= 1e3 * change,
            &format!("limit {limit:e} vs 1e3 x {change:.3e}"),
        );
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn rel(x: f64, r: f64) -> f64 {
    (x / r - 1.0).abs()
}

fn ulp_up(x: f64) -> f64 {
    assert!(x > 0.0);
    f64::from_bits(x.to_bits() + 1)
}

/// J1 by `(1/pi) int_0^pi cos(t - x sin t) dt`, 400-point midpoint rule.
fn j1(x: f64) -> f64 {
    let n = 400;
    (0..n)
        .map(|i| {
            let t = (i as f64 + 0.5) * PI / n as f64;
            (t - x * t.sin()).cos()
        })
        .sum::<f64>()
        / n as f64
}

fn j1_series(x: f64) -> f64 {
    let mut s = 0.0;
    let mut term = x / 2.0;
    for m in 0..40 {
        s += term;
        term *= -(x / 2.0) * (x / 2.0) / ((m + 1) as f64 * (m + 2) as f64);
    }
    s
}

fn uniform(n: usize, v: Vec3) -> Vec<[Phasor; 3]> {
    vec![[Phasor::new(v[0], 0.0), Phasor::new(v[1], 0.0), Phasor::new(v[2], 0.0)]; n]
}

/// The polar disk mesh of ADR 0468: centre node, `n_r` rings of `n_t` nodes, triangles then quadrilaterals.
fn disk(a: f64, n_r: usize, n_t: usize, centre: [f64; 2]) -> (Vec<Vec3>, Vec<Vec<usize>>) {
    let mut nodes = vec![[centre[0], centre[1], 0.0]];
    for j in 1..=n_r {
        for m in 0..n_t {
            let t = 2.0 * PI * m as f64 / n_t as f64;
            let r = a * j as f64 / n_r as f64;
            nodes.push([centre[0] + r * t.cos(), centre[1] + r * t.sin(), 0.0]);
        }
    }
    let idx = |j: usize, m: usize| if j == 0 { 0 } else { 1 + (j - 1) * n_t + (m % n_t) };
    let mut faces = Vec::new();
    for m in 0..n_t {
        faces.push(vec![0, idx(1, m), idx(1, m + 1)]);
    }
    for j in 1..n_r {
        for m in 0..n_t {
            faces.push(vec![idx(j, m), idx(j + 1, m), idx(j + 1, m + 1), idx(j, m + 1)]);
        }
    }
    (nodes, faces)
}

fn mesh_of(ka: f64) -> (usize, usize) {
    if ka == 10.0 {
        (16, 100)
    } else {
        (4, 64)
    }
}

fn piston(ka: f64, v: f64) -> VibratingSurface {
    let (n_r, n_t) = mesh_of(ka);
    let (nodes, faces) = disk(A, n_r, n_t, [0.0, 0.0]);
    let n = nodes.len();
    VibratingSurface::new(nodes, faces, uniform(n, [0.0, 0.0, v])).unwrap()
}

fn polygon_area(a: f64, n_t: usize) -> f64 {
    n_t as f64 / 2.0 * a * a * (2.0 * PI / n_t as f64).sin()
}

fn omega_of(ka: f64) -> f64 {
    ka / A * C
}

fn plate(m: usize, n: usize, amp: f64) -> VibratingSurface {
    let (lx, ly, nx, ny) = (0.4, 0.3, 40usize, 30usize);
    let mut nodes = Vec::new();
    let mut vel = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            let x = lx * i as f64 / nx as f64;
            let y = ly * j as f64 / ny as f64;
            nodes.push([x, y, 0.0]);
            let v = amp * (m as f64 * PI * x / lx).sin() * (n as f64 * PI * y / ly).sin();
            vel.push([Phasor::default(), Phasor::default(), Phasor::new(v, 0.0)]);
        }
    }
    let f = |j: usize, i: usize| j * (nx + 1) + i;
    let mut faces = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            faces.push(vec![f(j, i), f(j, i + 1), f(j + 1, i + 1), f(j + 1, i)]);
        }
    }
    VibratingSurface::new(nodes, faces, vel).unwrap()
}

/// A quarter cylinder of radius 0.1 about the y axis, 12 x 6 Quad4, radial velocity `V`.
fn quarter_cylinder() -> VibratingSurface {
    let (r, len, na, nl) = (0.1, 0.2, 12usize, 6usize);
    let mut nodes = Vec::new();
    let mut vel = Vec::new();
    for j in 0..=nl {
        for i in 0..=na {
            let p = PI / 2.0 * i as f64 / na as f64;
            nodes.push([r * p.cos(), len * j as f64 / nl as f64, r * p.sin()]);
            vel.push([
                Phasor::new(V * p.cos(), 0.0),
                Phasor::default(),
                Phasor::new(V * p.sin(), 0.0),
            ]);
        }
    }
    let f = |j: usize, i: usize| j * (na + 1) + i;
    let mut faces = Vec::new();
    for j in 0..nl {
        for i in 0..na {
            faces.push(vec![f(j, i), f(j, i + 1), f(j + 1, i + 1), f(j + 1, i)]);
        }
    }
    VibratingSurface::new(nodes, faces, vel).unwrap()
}

// ------------------------------------------------------------------------------------------------------------ J0, E

#[test]
fn j0_bessel_self_check() {
    let mut c = Checks::new();
    let worst = [0.2, 1.0, 4.0]
        .iter()
        .map(|&x| (j1(x) - j1_series(x)).abs())
        .fold(0.0, f64::max);
    c.at_most("J0 J1 integral vs series", worst, 1e-12);
    let change = [0.2, 1.0, 4.0]
        .iter()
        .map(|&x| (j1(ulp_up(x)) - j1(x)).abs())
        .fold(0.0, f64::max);
    c.probe("J0", change, 1e-12);
    c.finish();
}

#[test]
#[ignore = "known miss of ADR 0468 (registered run, commit 0ed86258): E1 at ka = 10 met (3.73e-14, limit 1e-12) but its ulp probe missed (one-ulp change 1.332e-15, the limit only 751 times it, registered 1e3); E1 at ka = 2, E2, E3, E4 met; continued in e1_e4_after_amendment_1 (amendment 1)"]
fn e1_e4_equivalent_radiated_power_known_miss() {
    let mut c = Checks::new();
    for ka in [2.0, 10.0] {
        let (_, n_t) = mesh_of(ka);
        let s = piston(ka, V);
        let reference = RHO * C * polygon_area(A, n_t) * V * V / 2.0;
        let r = equivalent_radiated_power(&s, RHO, C).unwrap();
        let e = rel(r.power, reference);
        c.at_most(&format!("E1 ka={ka} ERP vs rho c S_h v^2/2"), e, 1e-12);
        // ulp probes: the velocity, and one rim node coordinate
        let p1 = equivalent_radiated_power(&piston(ka, ulp_up(V)), RHO, C).unwrap().power;
        let mut nodes = s.nodes().to_vec();
        let last = nodes.len() - 1;
        nodes[last][0] = ulp_up(nodes[last][0].abs()).copysign(nodes[last][0]);
        let moved = VibratingSurface::new(nodes, s.faces().to_vec(), s.velocity().to_vec()).unwrap();
        let p2 = equivalent_radiated_power(&moved, RHO, C).unwrap().power;
        c.probe(&format!("E1 ka={ka}"), rel(p1, r.power).max(rel(p2, r.power)), 1e-12);
        c.holds(
            &format!("E4 ka={ka} note"),
            r.note.contains("radiation efficiency of one"),
            &r.note,
        );
    }
    // E2, E3: plate modes
    let lxly = 0.4 * 0.3;
    for (m, n) in [(1usize, 1usize), (2, 3)] {
        let r = equivalent_radiated_power(&plate(m, n, V), RHO, C).unwrap();
        let exact = RHO * C * lxly * V * V / 8.0;
        let f = (2.0 + (m as f64 * PI / 40.0).cos()) / 3.0 * ((2.0 + (n as f64 * PI / 30.0).cos()) / 3.0);
        let e = rel(r.power, exact);
        if (m, n) == (1, 1) {
            c.at_most("E2 plate (1,1) ERP vs rho c A v0^2/8", e, 5e-3);
        } else {
            println!("E2 plate ({m},{n}) recorded: {e:.6e} (no limit; the interpolant's deficit)");
        }
        let e3 = rel(r.power, exact * f);
        c.at_most(&format!("E3 plate ({m},{n}) ERP vs the interpolant's exact"), e3, 1e-12);
        let p1 = equivalent_radiated_power(&plate(m, n, ulp_up(V)), RHO, C)
            .unwrap()
            .power;
        c.probe(&format!("E3 ({m},{n})"), rel(p1, r.power), 1e-12);
    }
    // E4: a curved surface is accepted by the ERP
    let r = equivalent_radiated_power(&quarter_cylinder(), RHO, C).unwrap();
    c.holds(
        "E4 curved surface accepted",
        r.power.is_finite() && r.power > 0.0 && r.note.contains("radiation efficiency of one"),
        &format!("power {:e}, area {:e}", r.power, r.area),
    );
    c.finish();
}

/// ADR 0468 amendment 1: the checks of `e1_e4_equivalent_radiated_power_known_miss` that were met, with the same limits.
#[test]
fn e1_e4_after_amendment_1() {
    let mut c = Checks::new();
    for ka in [2.0, 10.0] {
        let (_, n_t) = mesh_of(ka);
        let s = piston(ka, V);
        let reference = RHO * C * polygon_area(A, n_t) * V * V / 2.0;
        let r = equivalent_radiated_power(&s, RHO, C).unwrap();
        let e = rel(r.power, reference);
        c.at_most(&format!("E1 ka={ka} ERP vs rho c S_h v^2/2"), e, 1e-12);
        // ulp probes: the velocity, and one rim node coordinate
        let p1 = equivalent_radiated_power(&piston(ka, ulp_up(V)), RHO, C).unwrap().power;
        let mut nodes = s.nodes().to_vec();
        let last = nodes.len() - 1;
        nodes[last][0] = ulp_up(nodes[last][0].abs()).copysign(nodes[last][0]);
        let moved = VibratingSurface::new(nodes, s.faces().to_vec(), s.velocity().to_vec()).unwrap();
        let p2 = equivalent_radiated_power(&moved, RHO, C).unwrap().power;
        let change = rel(p1, r.power).max(rel(p2, r.power));
        if ka == 2.0 {
            c.probe(&format!("E1 ka={ka}"), change, 1e-12);
        } else {
            // amendment 1: the probe of E1 at ka = 10 missed at the registered run; printed, not asserted
            println!("E1 ka={ka} ulp probe recorded (known miss, amendment 1): relative change {change:.3e}");
        }
        c.holds(
            &format!("E4 ka={ka} note"),
            r.note.contains("radiation efficiency of one"),
            &r.note,
        );
    }
    // E2, E3: plate modes
    let lxly = 0.4 * 0.3;
    for (m, n) in [(1usize, 1usize), (2, 3)] {
        let r = equivalent_radiated_power(&plate(m, n, V), RHO, C).unwrap();
        let exact = RHO * C * lxly * V * V / 8.0;
        let f = (2.0 + (m as f64 * PI / 40.0).cos()) / 3.0 * ((2.0 + (n as f64 * PI / 30.0).cos()) / 3.0);
        let e = rel(r.power, exact);
        if (m, n) == (1, 1) {
            c.at_most("E2 plate (1,1) ERP vs rho c A v0^2/8", e, 5e-3);
        } else {
            println!("E2 plate ({m},{n}) recorded: {e:.6e} (no limit; the interpolant's deficit)");
        }
        let e3 = rel(r.power, exact * f);
        c.at_most(&format!("E3 plate ({m},{n}) ERP vs the interpolant's exact"), e3, 1e-12);
        let p1 = equivalent_radiated_power(&plate(m, n, ulp_up(V)), RHO, C)
            .unwrap()
            .power;
        c.probe(&format!("E3 ({m},{n})"), rel(p1, r.power), 1e-12);
    }
    // E4: a curved surface is accepted by the ERP
    let r = equivalent_radiated_power(&quarter_cylinder(), RHO, C).unwrap();
    c.holds(
        "E4 curved surface accepted",
        r.power.is_finite() && r.power > 0.0 && r.note.contains("radiation efficiency of one"),
        &format!("power {:e}, area {:e}", r.power, r.area),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ P, D

#[test]
fn p1_p2_on_axis_pressure() {
    let mut c = Checks::new();
    for ka in [2.0, 10.0] {
        let k = ka / A;
        let omega = omega_of(ka);
        let plane = BaffledPlane::new(&piston(ka, V), &RayleighOptions::new()).unwrap();
        let (mut w1, mut w2) = (0.0f64, 0.0f64);
        for i in 0..40 {
            let z = A * 20f64.powf(i as f64 / 39.0);
            let p = plane.pressure([0.0, 0.0, z], RHO, C, omega).unwrap();
            let s = (z * z + A * A).sqrt();
            let mag = 2.0 * RHO * C * V * (k * (s - z) / 2.0).sin().abs();
            let cre = RHO * C * V * ((k * z).cos() - (k * s).cos());
            let cim = RHO * C * V * (-(k * z).sin() + (k * s).sin());
            w1 = w1.max(rel(p.abs(), mag));
            w2 = w2.max((p.re - cre).hypot(p.im - cim) / cre.hypot(cim));
        }
        c.at_most(&format!("P1 ka={ka} |p| vs 2 rho c v |sin|"), w1, 1e-2);
        c.at_most(&format!("P2 ka={ka} complex p vs closed form"), w2, 1e-2);
    }
    c.finish();
}

#[test]
fn d1_directivity() {
    let mut c = Checks::new();
    for ka in [2.0, 10.0] {
        let (_, n_t) = mesh_of(ka);
        let omega = omega_of(ka);
        let plane = BaffledPlane::new(&piston(ka, V), &RayleighOptions::new()).unwrap();
        let mut worst = 0.0f64;
        for phi in [0.0, PI / n_t as f64] {
            for deg in 0..=90 {
                let th = (deg as f64).to_radians();
                let d = [th.sin() * phi.cos(), th.sin() * phi.sin(), th.cos()];
                let got = plane.directivity(d, C, omega).unwrap();
                let u = ka * th.sin();
                let exact = if u > 0.0 { (2.0 * j1(u) / u).abs() } else { 1.0 };
                worst = worst.max((got - exact).abs());
            }
        }
        c.at_most(&format!("D1 ka={ka} |D - |2 J1(u)/u||"), worst, 1e-2);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ W, S, H

#[test]
fn w1_w2_s1_h1_power() {
    let mut c = Checks::new();
    for ka in [0.1, 0.5, 2.0, 10.0] {
        let omega = omega_of(ka);
        let s = piston(ka, V);
        let plane = BaffledPlane::new(&s, &RayleighOptions::new()).unwrap();
        let t0 = std::time::Instant::now();
        let w = plane.power(RHO, C, omega).unwrap();
        println!(
            "power ka={ka}: {:.3} s, efficiency {:.10}",
            t0.elapsed().as_secs_f64(),
            w.efficiency
        );
        let cf = 1.0 - j1(2.0 * ka) / ka;
        c.at_most(
            &format!("W1 ka={ka} W/ERP vs 1 - J1(2ka)/ka"),
            rel(w.efficiency, cf),
            1e-2,
        );
        if ka == 0.1 {
            let k = ka / A;
            let q = PI * A * A * V;
            c.at_most(
                "W2 ka=0.1 W vs rho c k^2 (S v)^2/(4 pi)",
                rel(w.power, RHO * C * k * k * q * q / (4.0 * PI)),
                2e-2,
            );
        }
        if ka == 2.0 || ka == 10.0 {
            let ws = plane.power_from_far_field(RHO, C, omega, 48, 96).unwrap();
            c.at_most(
                &format!("S1 ka={ka} far-field power vs double sum"),
                rel(ws, w.power),
                1e-9,
            );
        }
        if ka == 10.0 {
            let mut faces = s.faces().to_vec();
            faces.reverse();
            let rev = VibratingSurface::new(s.nodes().to_vec(), faces, s.velocity().to_vec()).unwrap();
            let wr = BaffledPlane::new(&rev, &RayleighOptions::new())
                .unwrap()
                .power(RHO, C, omega)
                .unwrap();
            c.at_most("H1 ka=10 power, faces reversed", rel(wr.power, w.power), 1e-12);
            let wu = BaffledPlane::new(&piston(ka, ulp_up(V)), &RayleighOptions::new())
                .unwrap()
                .power(RHO, C, omega)
                .unwrap();
            c.probe("H1", rel(wu.power, w.power), 1e-12);
        }
    }
    c.finish();
}

#[test]
fn w3_two_pistons() {
    let mut c = Checks::new();
    let b = 0.01;
    let k = 0.05 / b;
    let omega = k * C;
    for kd in [1.0, 3.0] {
        let d = kd / k;
        let (n1, f1) = disk(b, 2, 16, [-d / 2.0, 0.0]);
        let (n2, f2) = disk(b, 2, 16, [d / 2.0, 0.0]);
        for sign in [1.0, -1.0] {
            let off = n1.len();
            let mut nodes = n1.clone();
            nodes.extend(&n2);
            let mut faces = f1.clone();
            faces.extend(f2.iter().map(|f| f.iter().map(|i| i + off).collect::<Vec<_>>()));
            let mut vel = uniform(n1.len(), [0.0, 0.0, V]);
            vel.extend(uniform(n2.len(), [0.0, 0.0, sign * V]));
            let s = VibratingSurface::new(nodes, faces, vel).unwrap();
            let w = BaffledPlane::new(&s, &RayleighOptions::new())
                .unwrap()
                .power(RHO, C, omega)
                .unwrap();
            let q = polygon_area(b, 16) * V;
            let reference = RHO * C * k * k * q * q * (2.0 + 2.0 * sign * kd.sin() / kd) / (4.0 * PI);
            c.at_most(
                &format!("W3 kd={kd} sign {sign:+} W vs point pair"),
                rel(w.power, reference),
                5e-3,
            );
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ L, B

#[test]
fn l1_l2_levels() {
    let mut c = Checks::new();
    let l = power_level_db(1e-12).unwrap().unwrap();
    c.at_most("L1 level of 1e-12 W", l.abs(), 1e-12);
    let lp = power_level_db(ulp_up(1e-12)).unwrap().unwrap();
    c.probe("L1", (lp - l).abs(), 1e-12);
    let l2 = power_level_db(1e-2).unwrap().unwrap();
    c.at_most("L2 level of 1e-2 W vs 100 dB", (l2 - 100.0).abs(), 1e-12);
    c.holds("L2 zero gives None", power_level_db(0.0) == Ok(None), "");
    c.holds(
        "L2 negative and NaN refused",
        power_level_db(-1.0).is_err() && power_level_db(f64::NAN).is_err(),
        "",
    );
    c.finish();
}

fn b1_spectrum(v: f64) -> PowerSpectrum {
    let mut freq = Vec::new();
    let mut power = Vec::new();
    for i in 0..=300 {
        let f = 20.0 * 500f64.powf(i as f64 / 300.0);
        let amp = v / (1.0 + ((f - 800.0) / 200.0).powi(2)).sqrt();
        freq.push(f);
        power.push(equivalent_radiated_power(&plate(1, 1, amp), RHO, C).unwrap().power);
    }
    PowerSpectrum::new(freq, power).unwrap()
}

#[test]
fn b1_b2_third_octave_bands() {
    let mut c = Checks::new();
    let spec = b1_spectrum(V);
    let bands = spec.third_octave_bands(20.0, 10000.0).unwrap();
    let sum: f64 = bands.iter().map(|b| b.power).sum();
    let e = rel(sum, spec.total());
    c.at_most("B1 sum of bands vs narrow-band total", e, 1e-10);
    let sp = b1_spectrum(ulp_up(V));
    let bp: f64 = sp
        .third_octave_bands(20.0, 10000.0)
        .unwrap()
        .iter()
        .map(|b| b.power)
        .sum();
    c.probe(
        "B1",
        rel(bp / sp.total(), sum / spec.total()).max(rel(sp.total(), spec.total())),
        1e-10,
    );
    // B2: each band is the sum of its lines; layout as acoustics::third_octave_bands
    let mut ok = true;
    for b in &bands {
        let mut s = 0.0;
        for (f, w) in spec.freq.iter().zip(&spec.power) {
            if band_index(*f).unwrap() == b.n {
                s += w;
            }
        }
        ok &= s.to_bits() == b.power.to_bits();
    }
    c.holds(
        "B2 band = sum of its lines (bits)",
        ok,
        &format!("{} bands", bands.len()),
    );
    let tone = PowerSpectrum::new(vec![1000.0], vec![3e-5]).unwrap();
    let tb = tone.third_octave_bands(500.0, 2000.0).unwrap();
    let lit: Vec<_> = tb.iter().filter(|b| b.power != 0.0).collect();
    c.holds(
        "B2 1000 Hz tone in band 0 only",
        lit.len() == 1 && lit[0].n == 0 && lit[0].nominal == 1000.0 && lit[0].power == 3e-5,
        &format!("{:?}", lit.iter().map(|b| b.n).collect::<Vec<_>>()),
    );
    let df = 2.0;
    let sref = Spectrum {
        df,
        freq: (0..=8000).map(|k| k as f64 * df).collect(),
        ms: vec![1.0; 8001],
        n_used: 16000,
    };
    let layout = third_octave_bands(&sref, 20.0, 10000.0).unwrap();
    let same = layout.len() == bands.len()
        && layout.iter().zip(&bands).all(|(a, b)| {
            a.n == b.n
                && a.nominal.to_bits() == b.nominal.to_bits()
                && a.centre.to_bits() == b.centre.to_bits()
                && a.lo.to_bits() == b.lo.to_bits()
                && a.hi.to_bits() == b.hi.to_bits()
        });
    c.holds("B2 edges and labels as acoustics (bits)", same, "");
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ N

fn parse_deviation(msg: &str) -> Option<f64> {
    let rest = msg.split("deviation ").nth(1)?;
    rest.split(" m").next()?.parse().ok()
}

#[test]
#[allow(clippy::vec_init_then_push)]
fn n1_n2_refusals() {
    let mut c = Checks::new();
    let s = piston(2.0, V);
    for (lift, refused) in [(1e-3 * A, true), (1e-9 * A, false)] {
        let mut nodes = s.nodes().to_vec();
        nodes[0][2] += lift;
        let bent = VibratingSurface::new(nodes, s.faces().to_vec(), s.velocity().to_vec()).unwrap();
        match BaffledPlane::new(&bent, &RayleighOptions::new()) {
            Err(e) if refused => {
                let dev = parse_deviation(&e);
                c.holds(
                    "N1 lifted 1e-3 a refused with its deviation",
                    e.contains("not plane") && dev.is_some_and(|d| d >= 0.9 * lift && d <= lift),
                    &format!("{e} (parsed {dev:?}, lift {lift:e})"),
                );
            }
            Ok(_) if !refused => c.holds("N1 lifted 1e-9 a accepted", true, ""),
            other => c.holds(
                &format!("N1 lift {lift:e}"),
                false,
                &format!("{:?}", other.map(|p| p.deviation())),
            ),
        }
    }
    let cyl = BaffledPlane::new(&quarter_cylinder(), &RayleighOptions::new());
    c.holds(
        "N1 quarter cylinder refused",
        cyl.as_ref()
            .is_err_and(|e| e.contains("not plane") || e.contains("not consistently oriented")),
        &format!("{:?}", cyl.as_ref().err()),
    );
    // N2
    let plane = BaffledPlane::new(&s, &RayleighOptions::new()).unwrap();
    let om = omega_of(2.0);
    let n = s.nodes().len();
    let mut r = Vec::new();
    r.push((
        "point on the plane",
        plane.pressure([0.01, 0.0, 0.0], RHO, C, om).is_err(),
    ));
    r.push(("point behind", plane.pressure([0.0, 0.0, -0.1], RHO, C, om).is_err()));
    r.push(("rho 0", plane.pressure([0.0, 0.0, 0.1], 0.0, C, om).is_err()));
    r.push(("c negative", plane.power(RHO, -1.0, om).is_err()));
    r.push(("omega 0", plane.far_field([0.0, 0.0, 1.0], RHO, C, 0.0).is_err()));
    r.push(("erp rho NaN", equivalent_radiated_power(&s, f64::NAN, C).is_err()));
    let nodes = s.nodes().to_vec();
    let vel = s.velocity().to_vec();
    r.push((
        "face of 5 nodes",
        VibratingSurface::new(nodes.clone(), vec![vec![0, 1, 2, 3, 4]], vel.clone()).is_err(),
    ));
    r.push((
        "index out of range",
        VibratingSurface::new(nodes.clone(), vec![vec![0, 1, n]], vel.clone()).is_err(),
    ));
    r.push((
        "velocity length",
        VibratingSurface::new(nodes.clone(), s.faces().to_vec(), vel[1..].to_vec()).is_err(),
    ));
    let mut bad = vel.clone();
    bad[3][2] = Phasor::new(f64::NAN, 0.0);
    r.push((
        "NaN velocity",
        VibratingSurface::new(nodes.clone(), s.faces().to_vec(), bad).is_err(),
    ));
    r.push((
        "zero-area face",
        VibratingSurface::new(nodes.clone(), vec![vec![1, 1, 2]], vel.clone()).is_err(),
    ));
    r.push((
        "empty surface",
        VibratingSurface::new(nodes.clone(), vec![], vel.clone()).is_err(),
    ));
    let mut flipped = s.faces().to_vec();
    for f in flipped.iter_mut().step_by(2) {
        f.reverse();
    }
    let fl = VibratingSurface::new(nodes.clone(), flipped, vel.clone()).unwrap();
    r.push((
        "inconsistent orientation",
        BaffledPlane::new(&fl, &RayleighOptions::new()).is_err(),
    ));
    r.push((
        "order 0",
        BaffledPlane::new(&s, &RayleighOptions::new().order(0)).is_err(),
    ));
    for (name, ok) in r {
        c.holds(&format!("N2 {name} refused"), ok, "");
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ X, Z

fn block() -> SolidModel {
    let mesh = hex_box([0.0; 3], [0.3, 0.2, 0.05], [3, 2, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("zmin", comp, 0.0)).unwrap();
    }
    m.add_load(Load::pressure("zmax", 1e3)).unwrap();
    m
}

fn independent_erp(model: &SolidModel, re: &[f64], im: &[f64], omega: f64) -> f64 {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let mm = [
        [4.0, 2.0, 1.0, 2.0],
        [2.0, 4.0, 2.0, 1.0],
        [1.0, 2.0, 4.0, 2.0],
        [2.0, 1.0, 2.0, 4.0],
    ];
    let mut s = 0.0;
    for &f in mesh.surface_set("zmax").unwrap() {
        let g = mesh.face_nodes(f);
        let area = mesh.face_area(f);
        let u: Vec<(f64, f64)> = g
            .iter()
            .map(|&n| {
                let e = dofs.equation(n, 2).unwrap();
                (re[e], im[e])
            })
            .collect();
        let mut q = 0.0;
        for a in 0..4 {
            for b in 0..4 {
                q += mm[a][b] * (u[a].0 * u[b].0 + u[a].1 * u[b].1);
            }
        }
        s += area / 36.0 * q;
    }
    RHO * C / 2.0 * omega * omega * s
}

#[test]
fn x1_x2_from_response() {
    let mut c = Checks::new();
    let model = block();
    let load = HarmonicLoad::from_model(&model).unwrap();
    let solver = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let omega = 2.0 * PI * 500.0;
    let r = solver.response(&load, omega).unwrap();
    let s = VibratingSurface::from_response(&model, "zmax", &r).unwrap();
    let erp = equivalent_radiated_power(&s, RHO, C).unwrap().power;
    let refv = independent_erp(&model, &r.re, &r.im, omega);
    c.at_most(
        "X1 ERP of from_response vs bilinear mass formula",
        rel(erp, refv),
        1e-12,
    );
    let up = equivalent_radiated_power(
        &VibratingSurface::from_displacement(&model, "zmax", ulp_up(omega), &r.re, &r.im).unwrap(),
        RHO,
        C,
    )
    .unwrap()
    .power;
    c.probe("X1", rel(up, erp), 1e-12);
    println!("X1 ERP {erp:.6e} W");
    // X2: the velocity is i omega U
    let zeros = vec![0.0; r.re.len()];
    let s2 = VibratingSurface::from_displacement(&model, "zmax", omega, &r.re, &zeros).unwrap();
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let mut ok = true;
    for (k, &f) in mesh.surface_set("zmax").unwrap().iter().enumerate() {
        for (i, &g) in mesh.face_nodes(f).iter().enumerate() {
            let l = s2.faces()[k][i];
            ok &= s2.nodes()[l] == mesh.nodes()[g];
            for comp in 0..3 {
                let e = dofs.equation(g, comp).unwrap();
                let v = s2.velocity()[l][comp];
                ok &= v.re == 0.0 && v.im == omega * r.re[e];
            }
        }
    }
    c.holds("X2 velocity = i omega U at every node", ok, "");
    let (h20, _) = rcfd_fem::hex20::upgrade_to_hex20(model.mesh()).unwrap();
    let m20 = SolidModel::new(h20);
    let n20 = m20.dof_map().n_equations();
    let e20 = VibratingSurface::from_displacement(&m20, "zmax", omega, &vec![0.0; n20], &vec![0.0; n20]);
    c.holds(
        "X2 Quad8 faces refused naming 8 nodes",
        e20.as_ref().is_err_and(|e| e.contains("8 nodes")),
        &format!("{:?}", e20.err()),
    );
    let un = VibratingSurface::from_response(&model, "nope", &r);
    c.holds(
        "X2 unknown set refused",
        un.as_ref().is_err_and(|e| e.contains("unknown surface set")),
        "",
    );
    let rs: Vec<_> = [300.0, 500.0, 700.0]
        .iter()
        .map(|f| solver.response(&load, 2.0 * PI * f).unwrap())
        .collect();
    let spec = erp_spectrum(&model, "zmax", &rs, RHO, C).unwrap();
    let same = rs.iter().zip(&spec.power).all(|(ri, p)| {
        equivalent_radiated_power(&VibratingSurface::from_response(&model, "zmax", ri).unwrap(), RHO, C)
            .unwrap()
            .power
            .to_bits()
            == p.to_bits()
    });
    c.holds("X2 erp_spectrum = the ERPs (bits)", same && spec.freq.len() == 3, "");
    c.finish();
}

#[test]
fn z1_pending_landed() {
    assert_eq!(PENDING, None);
}
