//! F-332 (ADR 0243): the mass-spring-damper wall of the compressible solver (a frequency-dependent wall impedance in the
//! time domain) against closed forms. M the complex reflection coefficient of a tone at three frequencies around the
//! resonance (the acceptance), K the same measurement on walls whose reflection is known (calibration), E a wall
//! without damper (magnitude 1), F1 the free decay of the wall, S the stability of the coupling for a light mass and a
//! stiff spring, L1 the limit `m = k = 0` (bit-identical with the impedance wall of ADR 0233), B1 the defaults, W6 the
//! refusals, D1 the decomposition on synthetic data. Limits are the registered ones of ADR 0243.

use rcfd_fv::compressible::{to_prim, CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::compressible_bc::MsdWall;
use rcfd_fv::sponge::{Sponge, SpongeForm};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::box_2d;
use std::f64::consts::PI;
use std::sync::Arc;

const AIR: Gas = Gas { gamma: 1.4 };
const GAMMA: f64 = 1.4;
/// Ambient pressure: with `rho0 = 1` the sound speed and `rho0 c` are 1.
const P0: f64 = 1.0 / 1.4;
/// Perturbation amplitude of the waves.
const EPS: f64 = 1e-4 * P0;

/// A complex number `(re, im)`.
type C = (f64, f64);

fn c_mul(a: C, b: C) -> C {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

fn c_div(a: C, b: C) -> C {
    let d = b.0 * b.0 + b.1 * b.1;
    ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
}

fn c_abs(a: C) -> f64 {
    a.0.hypot(a.1)
}

fn c_arg(a: C) -> f64 {
    a.1.atan2(a.0)
}

/// `e^{i x}`.
fn c_exp_i(x: f64) -> C {
    (x.cos(), x.sin())
}

/// The difference of two angles on the circle, in `(-pi, pi]`.
fn angle_diff(a: f64, b: f64) -> f64 {
    let d = a - b;
    d - 2.0 * PI * (d / (2.0 * PI)).round()
}

fn ambient() -> Prim {
    Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: P0,
    }
}

/// The exact isentropic simple wave with the pressure `P0 + dp` travelling along `+x`.
fn simple_wave(dp: f64) -> Prim {
    let r = (P0 + dp) / P0;
    let a = r.powf((GAMMA - 1.0) / (2.0 * GAMMA));
    Prim {
        rho: r.powf(1.0 / GAMMA),
        u: [2.0 * (a - 1.0) / (GAMMA - 1.0), 0.0, 0.0],
        p: P0 + dp,
    }
}

fn solver(nx: usize, lx: f64, init: impl Fn([f64; 3]) -> Prim) -> CompressibleSolver {
    let mesh = Arc::new(box_2d(nx, 1, lx, lx / nx as f64, false, false).unwrap());
    let geom = FvGeometry::new(mesh).unwrap();
    let mut s = CompressibleSolver::new(geom, AIR, CompSettings::default(), init).unwrap();
    s.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
    s
}

fn pressure(s: &CompressibleSolver, cell: usize) -> f64 {
    to_prim(s.gas, &s.u[cell]).p
}

/// What closes the tube at `xmax`.
#[derive(Debug, Clone, Copy)]
enum Wall {
    Bc(CompBc),
    Msd(MsdWall),
}

fn close_tube(s: &mut CompressibleSolver, wall: Wall) {
    match wall {
        Wall::Bc(bc) => s.set_bc("xmax", bc).unwrap(),
        Wall::Msd(w) => s.set_msd_wall("xmax", Some(w)).unwrap(),
    }
}

/// The wall M of ADR 0243 D8 with the damper `r`: `m = 1/pi`, `k = 16 pi`, resonance at `omega0 = 4 pi`.
fn wall_m(r: f64) -> MsdWall {
    MsdWall {
        p: P0,
        r,
        m: 1.0 / PI,
        k: 16.0 * PI,
    }
}

/// The closed-form reflection coefficient `(Z - Z0)/(Z + Z0)`, `Z = r + i (omega m - k/omega)`, `Z0 = 1` (DR1).
fn reflection_cf(w: &MsdWall, omega: f64) -> C {
    let x = if w.k > 0.0 {
        omega * w.m - w.k / omega
    } else {
        omega * w.m
    };
    c_div((w.r - 1.0, x), (w.r + 1.0, x))
}

/// The least-squares decomposition of a tone into the incident and the reflected wave (ADR 0243 DR7).
struct ToneFit {
    omega: f64,
    /// Distance of every probe from the wall.
    s: Vec<f64>,
    /// Normal matrix of the basis `cos(omega t), sin(omega t), 1` over the samples.
    n: [[f64; 3]; 3],
    /// Right-hand side per probe.
    rhs: Vec<[f64; 3]>,
    samples: usize,
}

impl ToneFit {
    fn new(omega: f64, s: Vec<f64>) -> Self {
        let rhs = vec![[0.0; 3]; s.len()];
        Self {
            omega,
            s,
            n: [[0.0; 3]; 3],
            rhs,
            samples: 0,
        }
    }

    /// One sample time with the pressure perturbation of every probe.
    fn add(&mut self, t: f64, q: impl Iterator<Item = f64>) {
        let b = [(self.omega * t).cos(), (self.omega * t).sin(), 1.0];
        for i in 0..3 {
            for j in 0..3 {
                self.n[i][j] += b[i] * b[j];
            }
        }
        for (r, q) in self.rhs.iter_mut().zip(q) {
            for i in 0..3 {
                r[i] += b[i] * q;
            }
        }
        self.samples += 1;
    }

    /// The complex amplitude `P = a - i b` of every probe (`p' = Re[P e^{i omega t}]`).
    fn amplitudes(&self) -> Vec<C> {
        let m = &self.n;
        let det3 = |a: &[[f64; 3]; 3]| {
            a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
                + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
        };
        let d = det3(m);
        self.rhs
            .iter()
            .map(|r| {
                let col = |k: usize| {
                    let mut a = *m;
                    for i in 0..3 {
                        a[i][k] = r[i];
                    }
                    det3(&a) / d
                };
                (col(0), -col(1))
            })
            .collect()
    }

    /// `R = B/A` of the fit `P_j = A e^{i k s_j} + B e^{-i k s_j}` with the wavenumber `k_w`.
    fn reflection(&self, k_w: f64) -> C {
        let amp = self.amplitudes();
        // normal equations [[N, sum f^2], [sum e^2, N]] (A, B) = (sum f P, sum e P), e = e^{i k s}, f = conj(e)
        let n = self.s.len() as f64;
        let (mut see, mut sfp, mut sep) = ((0.0, 0.0), (0.0, 0.0), (0.0, 0.0));
        for (&s, &p) in self.s.iter().zip(&amp) {
            let e = c_exp_i(k_w * s);
            let f = (e.0, -e.1);
            let e2 = c_mul(e, e);
            see = (see.0 + e2.0, see.1 + e2.1);
            let (fp, ep) = (c_mul(f, p), c_mul(e, p));
            sfp = (sfp.0 + fp.0, sfp.1 + fp.1);
            sep = (sep.0 + ep.0, sep.1 + ep.1);
        }
        let sff = (see.0, -see.1);
        // Cramer: det = N^2 - sff see
        let prod = c_mul(sff, see);
        let det = (n * n - prod.0, -prod.1);
        let a_num = {
            let t = c_mul(sff, sep);
            (n * sfp.0 - t.0, n * sfp.1 - t.1)
        };
        let b_num = {
            let t = c_mul(see, sfp);
            (n * sep.0 - t.0, n * sep.1 - t.1)
        };
        c_div(c_div(b_num, det), c_div(a_num, det))
    }
}

/// D1 of ADR 0243: the decomposition recovers a known reflection coefficient from synthetic data.
#[test]
fn d1_decomposition_on_synthetic_data() {
    let a: C = c_mul((0.7, 0.0), c_exp_i(0.4));
    let mut worst = 0.0f64;
    for lambda in [0.625, 0.5, 0.4] {
        let omega = 2.0 * PI / lambda;
        let r_cf = reflection_cf(&wall_m(0.5), omega);
        let b = c_mul(r_cf, a);
        let s: Vec<f64> = (0..400).map(|j| 0.1 + 0.5 * (j as f64 + 0.5) / 400.0).collect();
        let mut fit = ToneFit::new(omega, s.clone());
        let mut t = 4.6;
        for n in 0..4000 {
            // uneven steps with the mean 5e-4
            t += 5e-4 * (1.0 + 0.3 * ((n as f64) * 0.7).sin());
            let rot = c_exp_i(omega * t);
            fit.add(
                t,
                s.iter().map(|&s| {
                    let e = c_exp_i(omega * s);
                    let p = c_mul(a, e);
                    let q = c_mul(b, (e.0, -e.1));
                    c_mul((p.0 + q.0, p.1 + q.1), rot).0 + 0.013
                }),
            );
        }
        let r = fit.reflection(omega);
        let e = c_abs((r.0 - r_cf.0, r.1 - r_cf.1));
        println!(
            "D1 lambda {lambda}: R = ({:+.12}, {:+.12}), deviation {e:.2e} (1e-9)",
            r.0, r.1
        );
        worst = worst.max(e);
    }
    assert!(worst <= 1e-9, "{worst}");
}

/// Set-up Z of ADR 0243: the complex reflection coefficient of a tone of the wavelength `lambda` at the wall `wall`, on
/// `nx` cells over the length 8.
fn tone_reflection(nx: usize, lambda: f64, wall: Wall) -> C {
    let len = 8.0;
    let ramp = |t: f64| (0.5 * PI * t).sin().powi(2);
    let mut s = solver(nx, len, |x| {
        let x = x[0];
        let e = if !(0.25..=7.5).contains(&x) {
            0.0
        } else if x < 0.75 {
            ramp((x - 0.25) / 0.5)
        } else if x <= 7.0 {
            1.0
        } else {
            ramp((7.5 - x) / 0.5)
        };
        simple_wave(EPS * e * (2.0 * PI * x / lambda).sin())
    });
    close_tube(&mut s, wall);
    let (cells, dist): (Vec<usize>, Vec<f64>) = {
        let cc = s.geom.mesh.cell_centres();
        (0..cc.len())
            .filter(|&c| (0.1..=0.6).contains(&(len - cc[c][0])))
            .map(|c| (c, len - cc[c][0]))
            .unzip()
    };
    let omega = 2.0 * PI / lambda;
    let mut fit = ToneFit::new(omega, dist);
    while s.time < 6.6 {
        let dt = s.max_dt();
        s.step(dt).unwrap();
        if (4.6..=6.6).contains(&s.time) {
            fit.add(s.time, cells.iter().map(|&c| pressure(&s, c) - P0));
        }
    }
    println!(
        "    tone lambda {lambda}, {nx} cells: {} steps, {} probes, {} samples",
        s.steps,
        cells.len(),
        fit.samples
    );
    fit.reflection(omega)
}

/// The cells of set-up Z.
const NX: usize = 6400;

/// Deviation of a measured reflection coefficient from a closed form: (relative magnitude, phase in rad).
fn deviation(r: C, r_cf: C) -> (f64, f64) {
    (
        (c_abs(r) / c_abs(r_cf) - 1.0).abs(),
        angle_diff(c_arg(r), c_arg(r_cf)).abs(),
    )
}

fn report(id: &str, r: C, r_cf: C, lim: (f64, f64)) -> (f64, f64) {
    let d = deviation(r, r_cf);
    println!(
        "{id}: |R| = {:.6}, arg R = {:+.6} rad; closed form {:.6}, {:+.6} rad; deviation {:.3e} (limit {:.3e}), {:.3e} rad (limit {:.4e})",
        c_abs(r),
        c_arg(r),
        c_abs(r_cf),
        c_arg(r_cf),
        d.0,
        lim.0,
        d.1,
        lim.1
    );
    d
}

/// The phase limit of the acceptance as read in ADR 0243: `0.02 min(1, |arg R_cf|)` rad.
fn phase_limit(r_cf: C) -> f64 {
    0.02 * c_arg(r_cf).abs().min(1.0)
}

/// K1 to K3 of ADR 0243: the measurement on the real impedance wall of ADR 0233 (`z = 0.5`, `R = -1/3`). Returns the
/// measured coefficient.
fn k_real_wall(id: &str, lambda: f64) -> C {
    let r = tone_reflection(NX, lambda, Wall::Bc(CompBc::Impedance { p: P0, z: 0.5 }));
    let d = report(id, r, (-1.0 / 3.0, 0.0), (0.01, 0.01));
    assert!(d.0 <= 0.01 && d.1 <= 0.01, "{id}: {d:?}");
    r
}

/// NC1 of ADR 0243: the damper alone fails the closed form of wall M away from the resonance.
fn nc1(id: &str, lambda: f64, r: C) {
    let d = deviation(r, reflection_cf(&wall_m(0.5), 2.0 * PI / lambda));
    println!(
        "NC1 ({id}): against the closed form of wall M: magnitude {:.3} (>= 0.5), phase {:.3} rad (>= 2)",
        d.0, d.1
    );
    assert!(d.0 >= 0.5 && d.1 >= 2.0, "NC1 {d:?}");
}

#[test]
#[ignore = "heavy: 6400 cells, 26400 steps, release"]
fn k1_calibration_real_wall_below() {
    let r = k_real_wall("K1", 0.625);
    nc1("K1", 0.625, r);
}

#[test]
#[ignore = "heavy: 6400 cells, 26400 steps, release"]
fn k2_calibration_real_wall_at_resonance_frequency() {
    k_real_wall("K2", 0.5);
}

#[test]
#[ignore = "heavy: 6400 cells, 26400 steps, release"]
fn k3_calibration_real_wall_above() {
    let r = k_real_wall("K3", 0.4);
    nc1("K3", 0.4, r);
}

#[test]
#[ignore = "heavy: 6400 cells, 26400 steps, release"]
fn k4_calibration_rigid_wall() {
    let r = tone_reflection(NX, 0.5, Wall::Bc(CompBc::Slip));
    let d = report("K4", r, (1.0, 0.0), (0.01, 0.01));
    assert!(d.0 <= 0.01 && d.1 <= 0.01, "K4: {d:?}");
}

/// M1 to M3 of ADR 0243 (the F-332 acceptance): wall M against the closed form within 2 percent in magnitude and phase.
fn m_acceptance(id: &str, lambda: f64) {
    let wall = wall_m(0.5);
    let r_cf = reflection_cf(&wall, 2.0 * PI / lambda);
    let lim = (0.02, phase_limit(r_cf));
    let r = tone_reflection(NX, lambda, Wall::Msd(wall));
    let d = report(id, r, r_cf, lim);
    assert!(d.0 <= lim.0 && d.1 <= lim.1, "{id}: {d:?}");
}

#[test]
#[ignore = "heavy: 6400 cells, 26400 steps, release"]
fn m1_msd_wall_below_the_resonance() {
    m_acceptance("M1", 0.625);
}

#[test]
#[ignore = "heavy: 6400 cells, 26400 steps, release"]
fn m2_msd_wall_at_the_resonance() {
    m_acceptance("M2", 0.5);
}

#[test]
#[ignore = "heavy: 6400 cells, 26400 steps, release"]
fn m3_msd_wall_above_the_resonance() {
    m_acceptance("M3", 0.4);
}

/// E1 to E3 of ADR 0243: a wall without damper reflects with the magnitude 1.
fn e_energy(id: &str, lambda: f64) {
    let wall = wall_m(0.0);
    let r_cf = reflection_cf(&wall, 2.0 * PI / lambda);
    let lim = (0.01, 0.02);
    let r = tone_reflection(NX, lambda, Wall::Msd(wall));
    let d = report(id, r, r_cf, lim);
    assert!(d.0 <= lim.0 && d.1 <= lim.1, "{id}: {d:?}");
}

#[test]
#[ignore = "heavy: 6400 cells, 26400 steps, release"]
fn e1_lossless_wall_below_the_resonance() {
    e_energy("E1", 0.625);
}

#[test]
#[ignore = "heavy: 6400 cells, 26400 steps, release"]
fn e2_lossless_wall_at_the_resonance() {
    e_energy("E2", 0.5);
}

#[test]
#[ignore = "heavy: 6400 cells, 26400 steps, release"]
fn e3_lossless_wall_above_the_resonance() {
    e_energy("E3", 0.4);
}

/// G of ADR 0243 (reported, no limit): wall M at the resonance on half the resolution.
#[test]
#[ignore = "heavy: 3200 cells, 13200 steps, release; reported only"]
fn g_msd_wall_at_the_resonance_on_the_coarser_grid() {
    let wall = wall_m(0.5);
    let r = tone_reflection(NX / 2, 0.5, Wall::Msd(wall));
    report("G", r, reflection_cf(&wall, 4.0 * PI), (f64::NAN, f64::NAN));
}

/// The pulse of ADR 0233 set-up P on `nx` cells over the length 1.
fn pulse_solver(nx: usize) -> CompressibleSolver {
    solver(nx, 1.0, |x| simple_wave(EPS * (-((x[0] - 0.5) / 0.05).powi(2)).exp()))
}

fn bits(s: &CompressibleSolver) -> Vec<u64> {
    s.u.iter().flat_map(|c| c.iter().map(|x| x.to_bits())).collect()
}

/// L1 of ADR 0243: without mass and spring the wall is the impedance wall of ADR 0233, bit for bit.
#[test]
fn l1_no_mass_no_spring_is_the_impedance_wall_bit_for_bit() {
    for z in [0.0, 1.0 / 3.0, 3.0] {
        let mut a = pulse_solver(400);
        close_tube(&mut a, Wall::Bc(CompBc::Impedance { p: P0, z }));
        let mut b = pulse_solver(400);
        close_tube(
            &mut b,
            Wall::Msd(MsdWall {
                p: P0,
                r: z,
                m: 0.0,
                k: 0.0,
            }),
        );
        for step in 1..=1400 {
            let (da, db) = (a.max_dt(), b.max_dt());
            assert_eq!(da.to_bits(), db.to_bits(), "z {z}: max_dt at step {step}");
            a.step(da).unwrap();
            b.step(db).unwrap();
            if step % 100 == 0 {
                assert!(bits(&a) == bits(&b), "z {z}: state differs after step {step}");
            }
        }
        // the pulse has been reflected (or absorbed) by then: the wall cell has seen it
        let moved = b.msd_wall_state("xmax").unwrap()[0][0];
        println!(
            "L1 z = {z:.4}: 1400 steps bit-identical, t = {:.4}, wall displacement {moved:.3e}",
            a.time
        );
        assert!(a.time > 0.6 && moved != 0.0);
        if z == 3.0 {
            // control: a spring changes the state
            let mut c = pulse_solver(400);
            close_tube(
                &mut c,
                Wall::Msd(MsdWall {
                    p: P0,
                    r: z,
                    m: 0.0,
                    k: 1.0,
                }),
            );
            for _ in 0..1400 {
                let dt = c.max_dt();
                c.step(dt).unwrap();
            }
            assert!(bits(&c) != bits(&a), "control: the spring changes nothing");
        }
    }
}

/// B1 of ADR 0243: a solver on which a wall was set and cleared again is bit-identical with one never touched.
#[test]
fn b1_a_cleared_wall_leaves_the_solver_bit_identical() {
    let mut a = pulse_solver(400);
    a.set_bc("xmax", CompBc::Slip).unwrap();
    let mut b = pulse_solver(400);
    b.set_bc("xmax", CompBc::Slip).unwrap();
    b.set_msd_wall("xmax", Some(wall_m(0.5))).unwrap();
    assert!(b.msd_wall("xmax").is_some() && b.msd_dt_limit().is_finite());
    b.set_msd_wall("xmax", None).unwrap();
    assert!(b.msd_wall("xmax").is_none() && b.msd_dt_limit() == f64::INFINITY);
    for step in 1..=200 {
        let (da, db) = (a.max_dt(), b.max_dt());
        assert_eq!(da.to_bits(), db.to_bits(), "max_dt at step {step}");
        a.step(da).unwrap();
        b.step(db).unwrap();
    }
    assert!(bits(&a) == bits(&b));
    println!("B1: 200 steps bit-identical after set and clear");
}

/// F1 of ADR 0243: the wall released from a displacement in a gas at rest follows the damped oscillator with the
/// radiation damping `rho c` (DR6).
#[test]
fn f1_free_decay_of_the_wall() {
    let wall = wall_m(0.5);
    let xi0 = 1e-6;
    let mut s = solver(400, 1.0, |_| ambient());
    s.set_msd_wall("xmax", Some(wall)).unwrap();
    s.set_msd_wall_state("xmax", &[[xi0, 0.0]]).unwrap();
    let a = (wall.r + 1.0) / (2.0 * wall.m);
    let w0sq = wall.k / wall.m;
    let wd = (w0sq - a * a).sqrt();
    let (mut e_xi, mut e_v) = (0.0f64, 0.0f64);
    while s.time < 1.0 {
        let dt = s.max_dt();
        s.step(dt).unwrap();
        let t = s.time;
        let [xi, v] = s.msd_wall_state("xmax").unwrap()[0];
        let decay = (-a * t).exp();
        let xi_cf = xi0 * decay * ((wd * t).cos() + a / wd * (wd * t).sin());
        let v_cf = -xi0 * w0sq / wd * decay * (wd * t).sin();
        e_xi = e_xi.max((xi - xi_cf).abs() / xi0);
        e_v = e_v.max((v - v_cf).abs() / (w0sq.sqrt() * xi0));
    }
    println!(
        "F1: alpha = {a:.4}, omega_d = {wd:.4}, {} steps: largest error of xi {e_xi:.3e}, of v {e_v:.3e} (limit 2e-3 each)",
        s.steps
    );
    assert!(e_xi <= 2e-3 && e_v <= 2e-3, "F1 {e_xi} {e_v}");
}

/// Set-up S of ADR 0243: the pulse reflection `A_ref/A_inc` at `x = 0.75` of the wall under test and of a reference
/// boundary, both stepped with the smaller of their `max_dt()`. Also returns whether the wall limit set the first step.
fn lockstep_reflection(wall: MsdWall, reference: CompBc) -> (f64, f64, bool) {
    let mut a = pulse_solver(400);
    close_tube(&mut a, Wall::Msd(wall));
    let mut b = pulse_solver(400);
    close_tube(&mut b, Wall::Bc(reference));
    let binding = a.max_dt() == a.msd_dt_limit() && a.max_dt() < b.max_dt();
    let probe = {
        let cc = a.geom.mesh.cell_centres();
        (0..cc.len())
            .filter(|&c| cc[c][0] > 0.75)
            .min_by(|&x, &y| cc[x][0].total_cmp(&cc[y][0]))
            .unwrap()
    };
    let mut peaks = [[0.0f64; 2]; 2];
    while a.time < 1.0 {
        let dt = a.max_dt().min(b.max_dt());
        a.step(dt).unwrap();
        b.step(dt).unwrap();
        let k = if a.time < 0.5 { 0 } else { 1 };
        for (pk, s) in peaks.iter_mut().zip([&a, &b]) {
            let q = pressure(s, probe) - P0;
            if q.abs() > pk[k].abs() {
                pk[k] = q;
            }
        }
    }
    println!(
        "    lockstep: {} steps, first step limited by the wall: {binding}",
        a.steps
    );
    (peaks[0][1] / peaks[0][0], peaks[1][1] / peaks[1][0], binding)
}

/// S1 of ADR 0243: a light mass, whose step limit is below the CFL step, runs stably and reflects like its damper.
#[test]
fn s1_light_mass_is_stable_at_the_wall_limit() {
    let (r, r_ref, binding) = lockstep_reflection(
        MsdWall {
            p: P0,
            r: 3.0,
            m: 4e-4,
            k: 0.0,
        },
        CompBc::Impedance { p: P0, z: 3.0 },
    );
    println!(
        "S1: R = {r:+.6}, reference (impedance 3) {r_ref:+.6}, difference {:.3e} (limit 5e-3)",
        (r - r_ref).abs()
    );
    assert!(binding, "S1: the wall limit does not set the step");
    assert!((r - r_ref).abs() <= 0.005, "S1 {r} {r_ref}");
}

/// S2 of ADR 0243: a stiff spring without mass, whose step limit is below the CFL step, runs stably and reflects like
/// a rigid wall.
#[test]
fn s2_stiff_spring_is_stable_at_the_wall_limit() {
    let (r, r_ref, binding) = lockstep_reflection(
        MsdWall {
            p: P0,
            r: 3.0,
            m: 0.0,
            k: 4e4,
        },
        CompBc::Slip,
    );
    println!(
        "S2: R = {r:+.6}, reference (rigid) {r_ref:+.6}, difference {:.3e} (limit 5e-3)",
        (r - r_ref).abs()
    );
    assert!(binding, "S2: the wall limit does not set the step");
    assert!((r - r_ref).abs() <= 0.005, "S2 {r} {r_ref}");
}

/// W6 of ADR 0243 (and NC2): what is refused.
#[test]
fn w6_refusals() {
    let mut s = pulse_solver(40);
    let good = MsdWall {
        p: P0,
        r: 3.0,
        m: 4e-4,
        k: 0.0,
    };
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        for w in [
            MsdWall { r: bad, ..good },
            MsdWall { m: bad, ..good },
            MsdWall { k: bad, ..good },
        ] {
            assert!(s.set_msd_wall("xmax", Some(w)).is_err(), "{w:?}");
        }
    }
    for p in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(s.set_msd_wall("xmax", Some(MsdWall { p, ..good })).is_err(), "p {p}");
    }
    assert!(s.set_msd_wall("nowhere", Some(good)).is_err());
    assert!(s.msd_wall("xmax").is_none() && s.msd_dt_limit() == f64::INFINITY);
    assert!(s.msd_wall_state("xmax").is_err() && s.set_msd_wall_state("xmax", &[[0.0, 0.0]]).is_err());
    // not together with a sponge, in either order
    let sponge = Sponge {
        reference: ambient(),
        rate: vec![0.0; s.u.len()],
        form: SpongeForm::Plain,
    };
    s.set_sponge(Some(sponge.clone())).unwrap();
    assert!(s.set_msd_wall("xmax", Some(good)).is_err(), "wall with a sponge");
    s.set_sponge(None).unwrap();
    s.set_msd_wall("xmax", Some(good)).unwrap();
    assert!(s.set_sponge(Some(sponge)).is_err(), "sponge with a wall");
    s.set_sponge(None).unwrap();
    // the state
    assert_eq!(s.msd_wall_state("xmax").unwrap(), vec![[0.0, 0.0]]);
    assert!(s.set_msd_wall_state("xmax", &[[0.0, 0.0], [0.0, 0.0]]).is_err());
    assert!(s.set_msd_wall_state("xmax", &[[f64::NAN, 0.0]]).is_err());
    assert!(s.set_msd_wall_state("xmin", &[[0.0, 0.0]]).is_err());
    assert!(s.msd_wall_state("xmin").is_err());
    // not on moving meshes
    assert!(s.step_ale(s.geom.clone(), 1e-4).is_err(), "step_ale with a wall");
    assert!(
        s.replace_mesh(s.geom.clone(), s.u.clone()).is_err(),
        "replace_mesh with a wall"
    );
    // the step limit: max_dt() is the wall limit, and a larger step is refused (NC2: the CFL step)
    let lim = s.msd_dt_limit();
    assert!((lim - 0.8 * 2.0 * 4e-4 / 4.0).abs() <= 1e-3 * lim, "{lim}");
    assert_eq!(s.max_dt(), lim);
    assert!(s.step(2.0 * lim).is_err(), "twice the wall limit");
    assert!(s.step(0.2 / 40.0).is_err(), "the CFL step");
    assert_eq!(s.steps, 0);
    s.step(lim).unwrap();
    // cleared: everything works again
    s.set_msd_wall("xmax", None).unwrap();
    s.set_msd_wall("xmax", None).unwrap();
    assert!(s.msd_dt_limit() == f64::INFINITY && s.max_dt() > 10.0 * lim);
    s.step_ale(s.geom.clone(), 1e-4).unwrap();
    s.replace_mesh(s.geom.clone(), s.u.clone()).unwrap();
}
