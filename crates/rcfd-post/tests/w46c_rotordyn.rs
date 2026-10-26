//! Wave 46C (ADR 0120): rotor dynamics of a small fan rotor on two bearings (E1, E2, E3 interface, E4 hook).
//! Every check compares the general two-bearing solver with a closed form; every negative control is a deliberately wrong model
//! that must miss the same limit by at least 10x.

use rcfd_io::nastran::Deck;
use rcfd_post::rotordyn::*;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// Jeffcott set-up: m = 0.035 kg, k = 4e5 N/m, zeta = 0.05, e = 1e-5 m (unbalance 1 g at 0.35 mm).
const M: f64 = 0.035;
const K: f64 = 4.0e5;
const ZETA: f64 = 0.05;
const E: f64 = 1.0e-5;

fn jeff(zeta: f64) -> Rotor {
    Rotor::jeffcott(M, K, 2.0 * zeta * (K * M).sqrt())
}

fn wn() -> f64 {
    (K / M).sqrt()
}

/// Unbalance of the Jeffcott set-up: U = m e, in the centre-of-mass plane.
fn unb(spin: f64) -> HarmonicLoad {
    unbalance_load(
        &[PointUnbalance {
            mass: 1e-3,
            radius: M * E / 1e-3,
            angle: 0.0,
            z: 0.0,
        }],
        spin,
    )
    .unwrap()
}

/// |X| / e of the unbalance response.
fn amp_ratio(r: &Rotor, spin: f64) -> f64 {
    let s = response(r, spin, &[unb(spin)]).unwrap()[0];
    ellipse_major(s.disp[0], s.disp[1]) / E
}

/// Asymmetric rigid rotor of the R checks (distinct translation and tilt frequencies).
fn asym(spin_c: f64) -> Rotor {
    Rotor {
        mass: 0.035,
        it: 1.2e-5,
        ip: 2.0e-5,
        bearings: [
            Bearing {
                z: -0.012,
                k: 2.0e5,
                c: spin_c,
                eta: 0.0,
            },
            Bearing {
                z: 0.006,
                k: 3.0e5,
                c: spin_c,
                eta: 0.0,
            },
        ],
    }
}

fn sums(r: &Rotor) -> (f64, f64, f64) {
    let mut s = (0.0, 0.0, 0.0);
    for b in &r.bearings {
        s.0 += b.k;
        s.1 += b.z * b.k;
        s.2 += b.z * b.z * b.k;
    }
    s
}

/// Closest root to `target` (relative distance).
fn closest(roots: &[C64], target: f64) -> (f64, C64) {
    roots
        .iter()
        .map(|l| ((l.re - target).hypot(l.im) / target.abs(), *l))
        .fold((f64::INFINITY, C64::default()), |a, b| if b.0 < a.0 { b } else { a })
}

// ---------------------------------------------------------------- J1: Jeffcott critical speed sqrt(k/m)

#[test]
#[ignore = "DEFECTIVE SET-UP (ADR 0120 amendment 1): the negative control leaves the rotor on one bearing, critical_speeds is empty and [0] panics; replaced by w46c_j1b"]
fn w46c_j1_jeffcott_critical_speed_defective_control() {
    let r = jeff(0.0);
    let cs = critical_speeds(&r).unwrap();
    let nf = natural_frequencies(&r, 0.0).unwrap();
    let (d_nf, _) = closest(&nf, wn());
    let (d_nfb, _) = closest(&nf, -wn());
    eprintln!(
        "W46C J1 critical {:?} wn {} rel {:.3e}; natural +/- rel {d_nf:.3e} {d_nfb:.3e}",
        cs,
        wn(),
        rel(cs[0], wn())
    );
    assert_eq!(cs.len(), 1);
    assert!(rel(cs[0], wn()) <= 1e-9 && d_nf <= 1e-9 && d_nfb <= 1e-9);
    // negative control: only one bearing's stiffness counted
    let mut wrong = r;
    wrong.bearings[1].k = 1e-30;
    let w = critical_speeds(&wrong).unwrap()[0];
    eprintln!("W46C J1 control rel {:.3e}", rel(w, wn()));
    assert!(rel(w, wn()) >= 1e-8);
}

/// J1 corrected (ADR 0120 amendment 1): the control counts one bearing's stiffness (k/4 on each, total k/2), kc stays 0.
#[test]
fn w46c_j1b_jeffcott_critical_speed() {
    let r = jeff(0.0);
    let cs = critical_speeds(&r).unwrap();
    let nf = natural_frequencies(&r, 0.0).unwrap();
    let (d_nf, _) = closest(&nf, wn());
    let (d_nfb, _) = closest(&nf, -wn());
    eprintln!(
        "W46C J1b critical {:?} wn {} rel {:.3e}; natural +/- rel {d_nf:.3e} {d_nfb:.3e}",
        cs,
        wn(),
        rel(cs[0], wn())
    );
    assert_eq!(cs.len(), 1);
    assert!(rel(cs[0], wn()) <= 1e-9 && d_nf <= 1e-9 && d_nfb <= 1e-9);
    let w = critical_speeds(&Rotor::jeffcott(M, 0.5 * K, 0.0)).unwrap()[0];
    eprintln!("W46C J1b control rel {:.3e}", rel(w, wn()));
    assert!(rel(w, wn()) >= 1e-8);
}

// ---------------------------------------------------------------- J2: resonance amplitude 1/(2 zeta) at spin = wn

#[test]
fn w46c_j2_resonance_amplitude() {
    let a = amp_ratio(&jeff(ZETA), wn());
    let want = 1.0 / (2.0 * ZETA);
    eprintln!("W46C J2 |X|/e at r=1 {a} want {want} rel {:.3e}", rel(a, want));
    assert!(rel(a, want) <= 1e-9);
    // negative control: the damping of one bearing only
    let mut wrong = jeff(ZETA);
    wrong.bearings[1].c = 0.0;
    let aw = amp_ratio(&wrong, wn());
    eprintln!("W46C J2 control rel {:.3e}", rel(aw, want));
    assert!(rel(aw, want) >= 1e-8);
}

// ---------------------------------------------------------------- J3: supercritical self-centring

#[test]
fn w46c_j3_supercritical_limit() {
    let r = jeff(ZETA);
    // (a) |X|/e -> 1: at r = 1e5 the analytic deviation is (1 - 2 zeta^2) / r^2 ~ 1e-10
    let a = amp_ratio(&r, 1e5 * wn());
    // (b) centre of mass |X + e| / e equals the closed form |(1 + 2 i zeta r) / (1 - r^2 + 2 i zeta r)| at r = 1e3
    let rr = 1e3;
    let s = response(&r, rr * wn(), &[unb(rr * wn())]).unwrap()[0];
    let cg = (s.disp[0] + C64::new(E, 0.0)).abs() / E;
    let want = C64::new(1.0, 2.0 * ZETA * rr).abs() / C64::new(1.0 - rr * rr, 2.0 * ZETA * rr).abs();
    eprintln!(
        "W46C J3 |X|/e(r=1e5) - 1 = {:.3e}; cg {cg:.6e} want {want:.6e} rel {:.3e}",
        a - 1.0,
        rel(cg, want)
    );
    assert!((a - 1.0).abs() <= 1e-9 && rel(cg, want) <= 1e-9);
    // negative control: quasi-static model X = U spin^2 / k (no inertia)
    let qs = M * E * (1e5 * wn()).powi(2) / K / E;
    eprintln!("W46C J3 control |X|/e = {qs:.3e}");
    assert!((qs - 1.0).abs() >= 1e-8);
}

// ---------------------------------------------------------------- J4: low-speed transmitted load = static unbalance load

#[test]
fn w46c_j4_low_speed_transmitted_load() {
    let r = asym(5.0);
    let spin = 1e-5 * critical_speeds(&r).unwrap()[0];
    let l = unbalance_load(
        &[
            PointUnbalance {
                mass: 1e-3,
                radius: 1e-3,
                angle: 0.3,
                z: 0.004,
            },
            PointUnbalance {
                mass: 5e-4,
                radius: 2e-3,
                angle: 2.0,
                z: -0.009,
            },
        ],
        spin,
    )
    .unwrap();
    let s = response(&r, spin, &[l]).unwrap()[0];
    let df = ((s.mount_force[0] - l.force[0]).abs()).hypot((s.mount_force[1] - l.force[1]).abs())
        / l.force[0].abs().hypot(l.force[1].abs());
    let dm = ((s.mount_moment[0] - l.moment[0]).abs()).hypot((s.mount_moment[1] - l.moment[1]).abs())
        / l.moment[0].abs().hypot(l.moment[1].abs());
    eprintln!("W46C J4 force rel {df:.3e} moment rel {dm:.3e}");
    assert!(df <= 1e-9 && dm <= 1e-9);
    // negative control: force on the rotor reported instead of force on the housing (sign)
    let dfw = ((-s.mount_force[0] - l.force[0]).abs()).hypot((-s.mount_force[1] - l.force[1]).abs())
        / l.force[0].abs().hypot(l.force[1].abs());
    eprintln!("W46C J4 control rel {dfw:.3e}");
    assert!(dfw >= 1e-8);
}

// ---------------------------------------------------------------- R1: rigid rotor on two bearings, closed-form 2x2 eigenproblem

#[test]
fn w46c_r1_two_bearing_natural_frequencies() {
    let r = asym(0.0);
    let (kt, kc, kr) = sums(&r);
    let (a, d) = (kt / r.mass, kr / r.it);
    let disc = ((a - d) * (a - d) + 4.0 * kc * kc / (r.mass * r.it)).sqrt();
    let want = [(0.5 * (a + d - disc)).sqrt(), (0.5 * (a + d + disc)).sqrt()];
    let nf = natural_frequencies(&r, 0.0).unwrap();
    let mut worst: f64 = 0.0;
    for w in want {
        for t in [w, -w] {
            worst = worst.max(closest(&nf, t).0);
        }
    }
    eprintln!("W46C R1 want {want:?} roots {nf:?} worst rel {worst:.3e}");
    assert_eq!(nf.len(), 4);
    assert!(worst <= 1e-9);
    // negative control: translation-tilt coupling kc dropped
    let wrong = [a.sqrt(), d.sqrt()];
    let mut wc: f64 = 0.0;
    for (w, x) in want.iter().zip(wrong) {
        wc = wc.max(rel(x, *w));
    }
    eprintln!("W46C R1 control rel {wc:.3e}");
    assert!(wc >= 1e-8);
}

// ---------------------------------------------------------------- R2: gyroscopic forward / backward whirl of a thin disc

fn disc_rotor(ip: f64) -> Rotor {
    let b = |z| Bearing {
        z,
        k: 2.0e5,
        c: 0.0,
        eta: 0.0,
    };
    Rotor {
        mass: 0.035,
        it: 1.0e-5,
        ip,
        bearings: [b(-0.01), b(0.01)],
    }
}

#[test]
fn w46c_r2_gyroscopic_whirl_splitting() {
    let spin = 1500.0;
    let r = disc_rotor(2.0e-5); // thin disc: Ip = 2 It
    let (kt, _, kr) = sums(&r);
    let root = |ip: f64| (ip * ip * spin * spin + 4.0 * r.it * kr).sqrt();
    let fwd = (r.ip * spin + root(r.ip)) / (2.0 * r.it);
    let bwd = (r.ip * spin - root(r.ip)) / (2.0 * r.it);
    let tr = (kt / r.mass).sqrt();
    let nf = natural_frequencies(&r, spin).unwrap();
    let (df, lf) = closest(&nf, fwd);
    let (db, lb) = closest(&nf, bwd);
    let (dt, _) = closest(&nf, tr);
    let (dt2, _) = closest(&nf, -tr);
    eprintln!(
        "W46C R2 fwd {fwd} got {lf:?} rel {df:.3e}; bwd {bwd} got {lb:?} rel {db:.3e}; transl rel {dt:.3e} {dt2:.3e}"
    );
    assert!(df <= 1e-9 && db <= 1e-9 && dt <= 1e-9 && dt2 <= 1e-9);
    // thin disc: no synchronous tilt critical speed, only the translation one
    let cs = critical_speeds(&r).unwrap();
    eprintln!("W46C R2 critical speeds {cs:?} (translation {tr})");
    assert!(cs.len() == 1 && rel(cs[0], tr) <= 1e-9);
    // negative control: gyroscopic coupling omitted (Ip = 0 in the model)
    let nfw = natural_frequencies(&disc_rotor(0.0), spin).unwrap();
    let (dfw, _) = closest(&nfw, fwd);
    eprintln!("W46C R2 control fwd rel {dfw:.3e}");
    assert!(dfw >= 1e-8);
}

// ---------------------------------------------------------------- E: energy balance, work in per cycle = dissipation

fn energy_rotor() -> Rotor {
    Rotor {
        mass: 0.035,
        it: 1.2e-5,
        ip: 2.0e-5,
        bearings: [
            Bearing {
                z: -0.012,
                k: 2.0e5,
                c: 6.0,
                eta: 0.03,
            },
            Bearing {
                z: 0.006,
                k: 3.0e5,
                c: 3.0,
                eta: 0.05,
            },
        ],
    }
}

/// (work in, dissipated) over one period by N-point trapezoid integration of the real time signals (exact for these
/// trigonometric polynomials of degree 2).
fn cycle_energy(r: &Rotor, l: &HarmonicLoad, s: &HarmonicResponse) -> (f64, f64) {
    let n = 64;
    let w = l.omega;
    let dt = 2.0 * std::f64::consts::PI / w / n as f64;
    let sig = |p: C64, t: f64| (C64::polar(1.0, w * t) * p).re;
    let vel = |p: C64, t: f64| (C64::polar(1.0, w * t) * C64::I.scale(w) * p).re;
    let gen = [l.force[0], l.force[1], l.moment[0], l.moment[1]];
    let (mut win, mut wd) = (0.0, 0.0);
    for i in 0..n {
        let t = i as f64 * dt;
        for k in 0..4 {
            win += sig(gen[k], t) * vel(s.disp[k], t) * dt;
        }
        for b in &r.bearings {
            // bearing displacement u = (x + z theta_y, y - z theta_x)
            let u = [s.disp[0] + s.disp[3].scale(b.z), s.disp[1] - s.disp[2].scale(b.z)];
            for uk in u {
                // viscous c u' and hysteretic eta k u (phase-advanced by 90 degrees at omega > 0)
                let f = b.c * vel(uk, t) + sig(C64::I.scale(b.eta * b.k) * uk, t);
                wd += f * vel(uk, t) * dt;
            }
        }
    }
    (win, wd)
}

#[test]
fn w46c_e_energy_balance() {
    let r = energy_rotor();
    let spin = 300.0;
    let general = HarmonicLoad {
        omega: 700.0,
        force: [C64::new(0.3, -0.1), C64::new(-0.05, 0.2), C64::new(1.0, 0.0)],
        moment: [C64::new(0.002, 0.001), C64::new(-0.0015, 0.0007), C64::default()],
    };
    let ub = unbalance_load(
        &[PointUnbalance {
            mass: 1e-3,
            radius: 1e-3,
            angle: 0.0,
            z: 0.003,
        }],
        spin,
    )
    .unwrap();
    let mut worst: f64 = 0.0;
    for l in [general, ub] {
        let s = response(&r, spin, &[l]).unwrap()[0];
        let (win, wd) = cycle_energy(&r, &l, &s);
        eprintln!(
            "W46C E omega {} work in {win:.6e} dissipated {wd:.6e} rel {:.3e}",
            l.omega,
            rel(wd, win)
        );
        worst = worst.max(rel(wd, win));
    }
    assert!(worst <= 1e-9);
    // negative control: the solver omits the hysteretic damping, the dissipation is evaluated with it
    let mut wrong = r;
    wrong.bearings[0].eta = 0.0;
    wrong.bearings[1].eta = 0.0;
    let s = response(&wrong, spin, &[general]).unwrap()[0];
    let (win, wd) = cycle_energy(&r, &general, &s);
    eprintln!("W46C E control rel {:.3e}", rel(wd, win));
    assert!(rel(wd, win) >= 1e-8);
}

// ---------------------------------------------------------------- B: balance grade conversion

#[test]
fn w46c_b_balance_grade_exact() {
    // binary-exact case: e = 2^-10 m, spin = 256 rad/s -> G = 250 mm/s; U_per(250, 2 kg, 256) = 2^-9 kg m
    let g = balance_grade(2f64.powi(-10), 256.0).unwrap();
    let u = permissible_unbalance(250.0, 2.0, 256.0).unwrap();
    // general round trip G -> U_per -> e -> G at 3000 rpm
    let spin = 2.0 * std::f64::consts::PI * 3000.0 / 60.0;
    let u2 = permissible_unbalance(6.3, 0.035, spin).unwrap();
    let g2 = balance_grade(u2 / 0.035, spin).unwrap();
    eprintln!("W46C B G {g} U {u:e} round trip rel {:.3e}", rel(g2, 6.3));
    assert!(g == 250.0 && u == 2f64.powi(-9));
    assert!(rel(g2, 6.3) <= 4.0 * f64::EPSILON);
    // negative control: speed in rpm instead of rad/s
    let wrong = balance_grade(u2 / 0.035, 3000.0).unwrap();
    eprintln!("W46C B control rel {:.3e}", rel(wrong, 6.3));
    assert!(rel(wrong, 6.3) >= 40.0 * f64::EPSILON);
}

// ---------------------------------------------------------------- U: static and couple unbalance load

#[test]
fn w46c_u_static_and_couple_unbalance() {
    let spin = 300.0;
    let (mu, ru, a) = (2e-4, 0.05, 0.01);
    let st = unbalance_load(
        &[PointUnbalance {
            mass: mu,
            radius: ru,
            angle: 0.7,
            z: 0.0,
        }],
        spin,
    )
    .unwrap();
    let cp = unbalance_load(
        &[
            PointUnbalance {
                mass: mu,
                radius: ru,
                angle: 0.7,
                z: a,
            },
            PointUnbalance {
                mass: mu,
                radius: ru,
                angle: 0.7 + std::f64::consts::PI,
                z: -a,
            },
        ],
        spin,
    )
    .unwrap();
    let f = mu * ru * spin * spin;
    let m2 = 2.0 * a * f;
    let e1 = rel(st.force[0].abs(), f)
        .max(rel(st.force[1].abs(), f))
        .max(st.moment[0].abs().max(st.moment[1].abs()) / (f * a));
    let e2 = (cp.force[0].abs().max(cp.force[1].abs()) / f)
        .max(rel(cp.moment[0].abs(), m2))
        .max(rel(cp.moment[1].abs(), m2));
    // direction: the force leads by the unbalance angle and Fy lags Fx by 90 degrees (forward rotation)
    let e3 = (st.force[0] - C64::polar(f, 0.7))
        .abs()
        .max((st.force[1] - C64::polar(f, 0.7 - 0.5 * std::f64::consts::PI)).abs())
        / f;
    eprintln!("W46C U static {e1:.3e} couple {e2:.3e} direction {e3:.3e}");
    assert!(e1 <= 1e-12 && e2 <= 1e-12 && e3 <= 1e-12);
    // negative control: spin to the first power instead of squared
    let wrong = mu * ru * spin;
    assert!(rel(wrong, f) >= 1e-11);
}

// ---------------------------------------------------------------- X: E3 harmonics -> mount spectrum -> F-237 load deck

#[test]
fn w46c_x_harmonic_spectrum_and_load_deck() {
    let r = energy_rotor();
    let spin = 300.0;
    let loads: Vec<HarmonicLoad> = (1..=3)
        .map(|h| HarmonicLoad {
            omega: 7.0 * spin * h as f64,
            force: [
                C64::new(0.01 / h as f64, 0.0),
                C64::new(0.0, 0.004),
                C64::new(0.05, 0.01),
            ],
            moment: [C64::new(1e-4, 0.0), C64::default(), C64::new(2e-4, -1e-4)],
        })
        .collect();
    let res = response(&r, spin, &loads).unwrap();
    let deck = load_deck(&r, &res, 0, 100).unwrap();
    let back = Deck::parse(&deck.to_string_checked().unwrap()).unwrap();
    assert!(back.forces == deck.forces && back.moments == deck.moments && back.grids == deck.grids);
    let z = |g: i64| back.grids.iter().find(|p| p.0 == g).unwrap().1[2];
    let mut worst: f64 = 0.0;
    for (n, s) in res.iter().enumerate() {
        for part in 0..2 {
            let sid = 100 + 2 * n as i64 + part;
            let pick = |c: C64| if part == 0 { c.re } else { c.im };
            let (mut f, mut m) = ([0.0; 3], [0.0; 3]);
            for (s2, g, v) in &back.forces {
                if *s2 == sid {
                    for k in 0..3 {
                        f[k] += v[k];
                    }
                    m[0] += -z(*g) * v[1];
                    m[1] += z(*g) * v[0];
                }
            }
            for (s2, _, v) in &back.moments {
                if *s2 == sid {
                    for k in 0..3 {
                        m[k] += v[k];
                    }
                }
            }
            let scale_f = s.mount_force.iter().map(|c| c.abs()).fold(0.0, f64::max);
            let scale_m = s.mount_moment.iter().map(|c| c.abs()).fold(0.0, f64::max);
            for k in 0..3 {
                worst = worst.max((f[k] - pick(s.mount_force[k])).abs() / scale_f);
                worst = worst.max((m[k] - pick(s.mount_moment[k])).abs() / scale_m);
            }
        }
        eprintln!(
            "W46C X omega {} bearing peaks {:.4e} {:.4e} N, mount |F| peak {:.4e} N",
            s.omega,
            ellipse_major(s.bearing_force[0][0], s.bearing_force[0][1]),
            ellipse_major(s.bearing_force[1][0], s.bearing_force[1][1]),
            ellipse_major(s.mount_force[0], s.mount_force[1])
        );
    }
    eprintln!("W46C X deck resultant worst rel {worst:.3e}");
    assert!(worst <= 1e-12);
    // negative control: an export that drops the passed-through axial force
    let mut wrong = deck.clone();
    for f in wrong.forces.iter_mut() {
        f.2[2] = 0.0;
    }
    let fz: f64 = wrong.forces.iter().filter(|f| f.0 == 100).map(|f| f.2[2]).sum();
    assert!((fz - res[0].mount_force[2].re).abs() / res[0].mount_force[2].abs() >= 1e-11);
}

// ---------------------------------------------------------------- I: invalid inputs are located

#[test]
fn w46c_i_invalid_inputs_located() {
    let good = energy_rotor();
    let l = unbalance_load(
        &[PointUnbalance {
            mass: 1e-3,
            radius: 1e-3,
            angle: 0.0,
            z: 0.0,
        }],
        100.0,
    )
    .unwrap();
    let mut cases: Vec<(String, &str)> = Vec::new();
    let mut push = |f: &dyn Fn(&mut Rotor), want: &'static str| {
        let mut r = good;
        f(&mut r);
        cases.push((response(&r, 100.0, &[l]).unwrap_err(), want));
    };
    push(&|r| r.mass = -1.0, "mass");
    push(&|r| r.it = f64::NAN, "it");
    push(&|r| r.bearings[1].k = 0.0, "bearings[1].k");
    push(&|r| r.bearings[0].c = f64::NAN, "bearings[0].c");
    push(&|r| r.bearings[1].eta = -0.1, "bearings[1].eta");
    push(
        &|r| r.bearings[1].z = r.bearings[0].z,
        "bearings[0].z and bearings[1].z",
    );
    cases.push((response(&good, f64::NAN, &[l]).unwrap_err(), "spin"));
    let mut l0 = l;
    l0.omega = 0.0;
    cases.push((response(&good, 100.0, &[l, l0]).unwrap_err(), "loads[1].omega"));
    let undamped = Rotor::jeffcott(M, K, 0.0);
    cases.push((
        response(&undamped, wn(), &[unb(wn())]).unwrap_err(),
        "singular at omega",
    ));
    let bad_u = [PointUnbalance {
        mass: -1.0,
        radius: 1e-3,
        angle: 0.0,
        z: 0.0,
    }];
    cases.push((unbalance_load(&bad_u, 100.0).unwrap_err(), "unbalance[0]"));
    cases.push((permissible_unbalance(0.0, 1.0, 100.0).unwrap_err(), "grade"));
    cases.push((balance_grade(1e-5, -1.0).unwrap_err(), "spin"));
    cases.push((load_deck(&good, &[], 2, 1).unwrap_err(), "thrust_bearing"));
    let deck_txt = "BEGIN BULK\nGRID,1,,0.0,0.0,0.0\nFORCE,1,1,2,1.0,0.0,0.0,1.0\nENDDATA\n";
    cases.push((Deck::parse(deck_txt).unwrap_err().to_string(), "line 3"));
    for (msg, want) in &cases {
        eprintln!("W46C I {want}: {msg}");
        assert!(msg.contains(want), "{msg} does not name {want}");
    }
    // negative control: the valid inputs are accepted
    assert!(response(&good, 100.0, &[l]).is_ok() && unbalance_load(&[], 100.0).is_ok());
}

// ---------------------------------------------------------------- W: worked example, 120 mm fan (REPORTED, assumed values)

#[test]
fn w46c_w_worked_example_120mm_fan() {
    // ASSUMED, not measured: 35 g impeller with magnet ring, disc-like inertia, two ball bearings 16 mm apart in a plastic
    // bearing tube (effective radial stiffness 2e5 N/m each including the tube), light damping, 7 blades, 2000 rpm.
    let r = Rotor {
        mass: 0.035,
        it: 1.2e-5,
        ip: 2.0e-5,
        bearings: [
            Bearing {
                z: -0.010,
                k: 2.0e5,
                c: 4.0,
                eta: 0.02,
            },
            Bearing {
                z: 0.006,
                k: 2.0e5,
                c: 4.0,
                eta: 0.02,
            },
        ],
    };
    let spin = 2000.0 * 2.0 * std::f64::consts::PI / 60.0;
    let u_per = permissible_unbalance(6.3, r.mass, spin).unwrap();
    eprintln!("W46C W U_per(G 6.3, 35 g, 2000 rpm) = {:.4} g mm", u_per * 1e6);
    // residual: 1.0 g mm static at the CG plane plus a 0.5 g mm couple at +-10 mm
    let ub = [
        PointUnbalance {
            mass: 1e-4,
            radius: 0.010,
            angle: 0.0,
            z: 0.0,
        },
        PointUnbalance {
            mass: 5e-5,
            radius: 0.010,
            angle: 1.0,
            z: 0.010,
        },
        PointUnbalance {
            mass: 5e-5,
            radius: 0.010,
            angle: 1.0 + std::f64::consts::PI,
            z: -0.010,
        },
    ];
    let g = balance_grade(1.0e-6 / r.mass, spin).unwrap();
    eprintln!("W46C W static grade G = {g:.3} mm/s");
    eprintln!(
        "W46C W critical speeds {:?} rpm",
        critical_speeds(&r)
            .unwrap()
            .iter()
            .map(|w| w * 60.0 / (2.0 * std::f64::consts::PI))
            .collect::<Vec<_>>()
    );
    eprintln!(
        "W46C W natural frequencies at 2000 rpm [Hz] {:?}",
        natural_frequencies(&r, spin)
            .unwrap()
            .iter()
            .map(|l| l.re / (2.0 * std::f64::consts::PI))
            .collect::<Vec<_>>()
    );
    for rpm in [600.0, 1000.0, 2000.0, 3000.0, 4800.0] {
        let w = rpm * 2.0 * std::f64::consts::PI / 60.0;
        let s = response(&r, w, &[unbalance_load(&ub, w).unwrap()]).unwrap()[0];
        eprintln!(
            "W46C W {rpm} rpm: CG orbit {:.4e} um, bearing forces {:.4e} {:.4e} N, mount |F| {:.4e} N |M| {:.4e} N m",
            ellipse_major(s.disp[0], s.disp[1]) * 1e6,
            ellipse_major(s.bearing_force[0][0], s.bearing_force[0][1]),
            ellipse_major(s.bearing_force[1][0], s.bearing_force[1][1]),
            ellipse_major(s.mount_force[0], s.mount_force[1]),
            ellipse_major(s.mount_moment[0], s.mount_moment[1])
        );
    }
    // assumed blade-passing loads: 5 mN lateral at BPF, 2 mN at 2 BPF (fixed x direction), axial 20 mN and 8 mN
    let bpf = 7.0 * spin;
    let loads = [
        HarmonicLoad {
            omega: bpf,
            force: [C64::new(5e-3, 0.0), C64::default(), C64::new(2e-2, 0.0)],
            moment: [C64::default(); 3],
        },
        HarmonicLoad {
            omega: 2.0 * bpf,
            force: [C64::new(2e-3, 0.0), C64::default(), C64::new(8e-3, 0.0)],
            moment: [C64::default(); 3],
        },
    ];
    for s in response(&r, spin, &loads).unwrap() {
        eprintln!(
            "W46C W {:.1} Hz: bearing forces {:.4e} {:.4e} N, mount lateral {:.4e} N axial {:.4e} N",
            s.omega / (2.0 * std::f64::consts::PI),
            ellipse_major(s.bearing_force[0][0], s.bearing_force[0][1]),
            ellipse_major(s.bearing_force[1][0], s.bearing_force[1][1]),
            ellipse_major(s.mount_force[0], s.mount_force[1]),
            s.mount_force[2].abs()
        );
    }
}

// ---------------------------------------------------------------- smoke (rule d): every set-up constructs and solves

#[test]
#[ignore = "smoke for registration (rule d)"]
fn smoke_w46c() {
    for r in [
        jeff(0.0),
        jeff(ZETA),
        asym(0.0),
        asym(5.0),
        disc_rotor(2.0e-5),
        energy_rotor(),
    ] {
        let nf = natural_frequencies(&r, 300.0).unwrap();
        let cs = critical_speeds(&r).unwrap();
        let ok = response(
            &r,
            1000.0,
            &[unbalance_load(
                &[PointUnbalance {
                    mass: 1e-3,
                    radius: 1e-3,
                    angle: 0.0,
                    z: 0.0,
                }],
                1000.0,
            )
            .unwrap()],
        )
        .is_ok();
        eprintln!(
            "SMOKE roots {} finite {} criticals {} response ok {ok}",
            nf.len(),
            nf.iter().all(|l| l.re.is_finite()),
            cs.len()
        );
    }
}
