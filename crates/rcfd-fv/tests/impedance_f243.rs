//! F-243 (ADR 0233): the impedance boundary and the quiet boundaries of the compressible solver against closed forms.
//! I, Q the tube with a pulse (reflection coefficient `(z - rho c)/(z + rho c)`, quiet boundaries below 1 percent, pressure
//! anchoring), T the tube with a tone (standing-wave ratio), O oblique incidence in two dimensions (heavy), A6, A7 the
//! moving-face state and the refusals, NC the negative controls. Limits are the registered ones of ADR 0233.

use rcfd_fv::compressible::{characteristic_face_state, to_prim, CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::compressible_bc;
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const AIR: Gas = Gas { gamma: 1.4 };
const GAMMA: f64 = 1.4;
/// Ambient pressure: with `rho0 = 1` the sound speed and `rho0 c` are 1.
const P0: f64 = 1.0 / 1.4;
/// Perturbation amplitude of the waves.
const EPS: f64 = 1e-4 * P0;
/// The registered limit of I, Q1, Q2 and Q3.
const TUBE_LIMIT: f64 = 0.01;

fn ambient() -> Prim {
    Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: P0,
    }
}

/// The exact isentropic simple wave with the pressure `P0 + dp` travelling along the unit vector `d`.
fn simple_wave(dp: f64, d: [f64; 3]) -> Prim {
    let r = (P0 + dp) / P0;
    let a = r.powf((GAMMA - 1.0) / (2.0 * GAMMA));
    let v = 2.0 * (a - 1.0) / (GAMMA - 1.0);
    Prim {
        rho: r.powf(1.0 / GAMMA),
        u: [v * d[0], v * d[1], v * d[2]],
        p: P0 + dp,
    }
}

fn solver(nx: usize, ny: usize, lx: f64, ly: f64, init: impl Fn([f64; 3]) -> Prim) -> CompressibleSolver {
    let mesh = Arc::new(box_2d(nx, ny, lx, ly, false, false).unwrap());
    let geom = FvGeometry::new(mesh).unwrap();
    CompressibleSolver::new(geom, AIR, CompSettings::default(), init).unwrap()
}

fn pressure(s: &CompressibleSolver, cell: usize) -> f64 {
    to_prim(s.gas, &s.u[cell]).p
}

/// Set-up P of ADR 0233: the signed ratio of the reflected to the incident pulse at the probe `x = 0.75`.
fn tube_reflection(bc: CompBc) -> f64 {
    let n = 1600;
    let mut s = solver(n, 1, 1.0, 1.0 / n as f64, |x| {
        simple_wave(EPS * (-((x[0] - 0.5) / 0.05).powi(2)).exp(), [1.0, 0.0, 0.0])
    });
    s.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
    s.set_bc("xmax", bc).unwrap();
    let probe = {
        let cc = s.geom.mesh.cell_centres();
        (0..cc.len())
            .filter(|&c| cc[c][0] > 0.75)
            .min_by(|&a, &b| cc[a][0].total_cmp(&cc[b][0]))
            .unwrap()
    };
    let (mut inc, mut refl) = (0.0f64, 0.0f64);
    while s.time < 1.0 {
        let dt = s.max_dt();
        s.step(dt).unwrap();
        let q = pressure(&s, probe) - P0;
        if s.time < 0.5 {
            if q.abs() > inc.abs() {
                inc = q;
            }
        } else if s.time <= 1.0 && q.abs() > refl.abs() {
            refl = q;
        }
    }
    refl / inc
}

fn impedance(z: f64) -> CompBc {
    CompBc::Impedance { p: P0, z }
}

/// I1 to I5 of ADR 0233: the impedance tube with a pulse against `R = (z - rho c)/(z + rho c)`.
#[test]
fn i_impedance_tube_reflection_matches_the_closed_form() {
    let mut worst = 0.0f64;
    for (id, z) in [("I1", 0.0), ("I2", 1.0 / 3.0), ("I3", 1.0), ("I4", 3.0), ("I5", 19.0)] {
        let r_cf = (z - 1.0) / (z + 1.0);
        let r = tube_reflection(impedance(z));
        println!(
            "{id} z = {z:.4}: R = {r:+.6}, closed form {r_cf:+.6}, deviation {:.2e} (limit {TUBE_LIMIT:e})",
            (r - r_cf).abs()
        );
        worst = worst.max((r - r_cf).abs());
    }
    assert!(worst <= TUBE_LIMIT, "worst deviation {worst}");
}

/// Q1 and Q2 of ADR 0233 (the F-243 acceptance): both quiet boundaries reflect a pulse by less than 1 percent.
#[test]
fn q_quiet_boundaries_reflect_less_than_one_percent() {
    let thompson = tube_reflection(CompBc::NonReflecting { p: P0, sigma: 0.0 });
    let matched = tube_reflection(impedance(1.0));
    println!("Q1 NonReflecting sigma 0: R = {thompson:+.3e}; Q2 matched impedance: R = {matched:+.3e} (limit {TUBE_LIMIT:e})");
    assert!(thompson.abs() <= TUBE_LIMIT, "Q1 {thompson}");
    assert!(matched.abs() <= TUBE_LIMIT, "Q2 {matched}");
}

/// NC1 and NC2 of ADR 0233: the rigid wall and the pressure outlet reflect the pulse fully, so they fail the quiet limit.
#[test]
fn nc1_nc2_reflecting_boundaries_fail_the_quiet_limit() {
    let slip = tube_reflection(CompBc::Slip);
    let outlet = tube_reflection(CompBc::PressureOutlet { p: P0 });
    println!("NC1 Slip: R = {slip:+.6} (>= 0.9); NC2 PressureOutlet: R = {outlet:+.6} (<= -0.9)");
    assert!(slip >= 0.9, "NC1 {slip}");
    assert!(outlet <= -0.9, "NC2 {outlet}");
    assert!(slip.abs() > TUBE_LIMIT && outlet.abs() > TUBE_LIMIT);
}

/// Set-up Q3 of ADR 0233: the largest pressure offset left at `t = 1.5`, as a fraction of the initial uniform offset.
fn anchoring(bc: CompBc) -> f64 {
    let n = 400;
    let off = 1e-3 * P0;
    let mut s = solver(n, 1, 1.0, 1.0 / n as f64, |_| Prim {
        u: [0.0; 3],
        ..simple_wave(off, [1.0, 0.0, 0.0])
    });
    s.set_bc("xmin", bc).unwrap();
    s.set_bc("xmax", bc).unwrap();
    while s.time < 1.5 {
        let dt = s.max_dt();
        s.step(dt).unwrap();
    }
    s.primitives().iter().map(|q| (q.p - P0).abs()).fold(0.0, f64::max) / off
}

/// Q3 and NC3 of ADR 0233: the matched impedance brings a uniform overpressure back to the ambient pressure, the Thompson
/// boundary keeps it.
#[test]
fn q3_matched_impedance_holds_the_ambient_pressure() {
    let matched = anchoring(impedance(1.0));
    let thompson = anchoring(CompBc::NonReflecting { p: P0, sigma: 0.0 });
    println!("Q3 matched impedance: D = {matched:.3e} (limit {TUBE_LIMIT:e}); NC3 NonReflecting sigma 0: D = {thompson:.6} (>= 0.99)");
    assert!(matched <= TUBE_LIMIT, "Q3 {matched}");
    assert!(thompson >= 0.99, "NC3 {thompson}");
}

/// Set-up T of ADR 0233: the pressure envelope within 1.5 wavelengths of the wall. Returns (largest / smallest, envelope of
/// the wall cell / largest, envelope of the wall cell / smallest).
fn standing_wave(z: f64) -> (f64, f64, f64) {
    let (n, len, lambda) = (2400, 6.0, 0.5);
    let ramp = |t: f64| (0.5 * std::f64::consts::PI * t).sin().powi(2);
    let mut s = solver(n, 1, len, len / n as f64, |x| {
        let x = x[0];
        let e = if !(1.0..=5.5).contains(&x) {
            0.0
        } else if x < 1.5 {
            ramp((x - 1.0) / 0.5)
        } else if x <= 5.0 {
            1.0
        } else {
            ramp((5.5 - x) / 0.5)
        };
        simple_wave(
            EPS * e * (2.0 * std::f64::consts::PI * x / lambda).sin(),
            [1.0, 0.0, 0.0],
        )
    });
    s.set_bc("xmin", CompBc::Fixed(ambient())).unwrap();
    s.set_bc("xmax", impedance(z)).unwrap();
    let cells: Vec<usize> = {
        let cc = s.geom.mesh.cell_centres();
        let mut v: Vec<usize> = (0..cc.len()).filter(|&c| cc[c][0] >= 5.25).collect();
        v.sort_by(|&a, &b| cc[a][0].total_cmp(&cc[b][0]));
        v
    };
    let mut env = vec![0.0f64; cells.len()];
    while s.time < 3.75 {
        let dt = s.max_dt();
        s.step(dt).unwrap();
        if s.time >= 1.75 && s.time <= 3.75 {
            for (e, &c) in env.iter_mut().zip(&cells) {
                *e = e.max((pressure(&s, c) - P0).abs());
            }
        }
    }
    let largest = env.iter().copied().fold(0.0, f64::max);
    let smallest = env.iter().copied().fold(f64::INFINITY, f64::min);
    let wall = *env.last().unwrap();
    (largest / smallest, wall / largest, wall / smallest)
}

/// T1 and T2 of ADR 0233: the standing-wave ratio of a tone is `z/(rho c)` or its inverse, with the pressure maximum at a
/// stiff wall and the minimum at a soft one.
#[test]
fn t_standing_wave_ratio_matches_the_closed_form() {
    let (swr1, wall_to_max, _) = standing_wave(3.0);
    let (swr2, _, wall_to_min) = standing_wave(1.0 / 3.0);
    println!("T1 z = 3: SWR = {swr1:.5} (3 within 2 percent), wall cell / largest = {wall_to_max:.5} (>= 0.98)");
    println!("T2 z = 1/3: SWR = {swr2:.5} (3 within 2 percent), wall cell / smallest = {wall_to_min:.5} (<= 1.02)");
    assert!((swr1 / 3.0 - 1.0).abs() <= 0.02, "T1 {swr1}");
    assert!(wall_to_max >= 0.98, "T1 wall {wall_to_max}");
    assert!((swr2 / 3.0 - 1.0).abs() <= 0.02, "T2 {swr2}");
    assert!(wall_to_min <= 1.02, "T2 wall {wall_to_min}");
}

/// A6 of ADR 0233: on a moving face the impedance relation reads the normal velocity relative to the face.
#[test]
fn a6_impedance_on_a_moving_face() {
    let n = [0.6, 0.8, 0.0];
    let int = Prim {
        rho: 1.1,
        u: [0.3, 0.2, 0.1],
        p: 0.8,
    };
    let (p, z) = (0.75, 2.5);
    let bc = CompBc::Impedance { p, z };
    assert_eq!(
        characteristic_face_state(AIR, &bc, &int, n, 0.0).unwrap(),
        compressible_bc::impedance_wall(AIR, p, z, &int, n)
    );
    for wn in [0.3, -0.2] {
        let got = characteristic_face_state(AIR, &bc, &int, n, wn).unwrap();
        let rel = Prim {
            u: [int.u[0] - wn * n[0], int.u[1] - wn * n[1], int.u[2] - wn * n[2]],
            ..int
        };
        let st = compressible_bc::impedance_wall(AIR, p, z, &rel, n);
        let want = Prim {
            u: [st.u[0] + wn * n[0], st.u[1] + wn * n[1], st.u[2] + wn * n[2]],
            ..st
        };
        let d = (got.rho - want.rho)
            .abs()
            .max((got.p - want.p).abs())
            .max((0..3).map(|k| (got.u[k] - want.u[k]).abs()).fold(0.0, f64::max));
        let un = got.u[0] * n[0] + got.u[1] * n[1] + got.u[2] * n[2];
        let relation = (got.p - p - z * (un - wn)).abs() / int.p;
        println!("A6 wn = {wn}: difference {d:.2e} (1e-14), relation in the face frame {relation:.2e} (1e-12)");
        assert!(d <= 1e-14, "{d}");
        assert!(relation <= 1e-12, "{relation}");
    }
}

/// A7 of ADR 0233: `set_bc` refuses an impedance that is negative or not finite and a pressure that is not positive.
#[test]
fn a7_set_bc_refuses_an_invalid_impedance() {
    let mut s = solver(4, 1, 1.0, 0.25, |_| ambient());
    for (p, z) in [
        (P0, -1.0),
        (P0, f64::NAN),
        (P0, f64::INFINITY),
        (0.0, 1.0),
        (-1.0, 1.0),
        (f64::NAN, 1.0),
    ] {
        let before = s.bcs.clone();
        assert!(s.set_bc("xmax", CompBc::Impedance { p, z }).is_err(), "p {p}, z {z}");
        assert_eq!(s.bcs, before);
    }
    s.set_bc("xmax", CompBc::Impedance { p: P0, z: 0.0 }).unwrap();
    s.set_bc("xmax", CompBc::Impedance { p: P0, z: 1.0 }).unwrap();
}

/// Set-up O of ADR 0233: the largest pressure rise in the wall cell at `W = (0.8, 0.8)` for a plane pulse arriving at
/// `theta_deg` from the normal of `xmax`.
fn oblique_wall_peak(bc: CompBc, theta_deg: f64) -> f64 {
    let th = theta_deg.to_radians();
    let d = [th.cos(), th.sin(), 0.0];
    let mut s = solver(160, 320, 0.8, 1.6, |x| {
        let xi = (x[0] - 0.8) * d[0] + (x[1] - 0.8) * d[1];
        simple_wave(EPS * (-((xi + 0.5) / 0.04).powi(2)).exp(), d)
    });
    for patch in ["xmin", "ymin", "ymax"] {
        s.set_bc(patch, CompBc::Extrapolate).unwrap();
    }
    s.set_bc("xmax", bc).unwrap();
    let probe = {
        let cc = s.geom.mesh.cell_centres();
        (0..cc.len())
            .filter(|&c| cc[c][0] > 0.8 - 0.005 && cc[c][1] > 0.8)
            .min_by(|&a, &b| cc[a][1].total_cmp(&cc[b][1]))
            .unwrap()
    };
    let mut peak = f64::NEG_INFINITY;
    while s.time < 0.65 {
        let dt = s.max_dt();
        s.step(dt).unwrap();
        peak = peak.max(pressure(&s, probe) - P0);
    }
    peak
}

/// The registered limit of O1 to O3.
const OBLIQUE_LIMIT: f64 = 0.03;

/// The O runs of one angle: (O0 `P_rigid/(2 eps)`, R of `z = 3`, R of `z = 1`, R of `NonReflecting sigma = 0`), each R
/// calibrated by the rigid wall, and the closed forms DR2, DR2, DR3.
fn oblique(theta_deg: f64) -> ([f64; 4], [f64; 3]) {
    let c = theta_deg.to_radians().cos();
    let rigid = oblique_wall_peak(CompBc::Slip, theta_deg);
    let r = |bc: CompBc| 2.0 * oblique_wall_peak(bc, theta_deg) / rigid - 1.0;
    let got = [
        rigid / (2.0 * EPS),
        r(impedance(3.0)),
        r(impedance(1.0)),
        r(CompBc::NonReflecting { p: P0, sigma: 0.0 }),
    ];
    let cf = [
        (3.0 * c - 1.0) / (3.0 * c + 1.0),
        (c - 1.0) / (c + 1.0),
        (1.0 - c) / (1.0 + c),
    ];
    println!(
        "O theta = {theta_deg}: O0 P_rigid/(2 eps) = {:.5} ([0.90, 1.02])",
        got[0]
    );
    for (k, id) in ["O1 z = 3", "O2 z = 1", "O3 NonReflecting sigma 0"].iter().enumerate() {
        println!(
            "O theta = {theta_deg}: {id}: R = {:+.5}, closed form {:+.5}, deviation {:.2e} (limit {OBLIQUE_LIMIT:e})",
            got[k + 1],
            cf[k],
            (got[k + 1] - cf[k]).abs()
        );
    }
    (got, cf)
}

fn assert_oblique(got: [f64; 4], cf: [f64; 3]) {
    assert!((0.90..=1.02).contains(&got[0]), "O0 {}", got[0]);
    for k in 0..3 {
        assert!(
            (got[k + 1] - cf[k]).abs() <= OBLIQUE_LIMIT,
            "O{} {} against {}",
            k + 1,
            got[k + 1],
            cf[k]
        );
    }
}

/// O0 to O3 of ADR 0233 at 30 degrees.
#[test]
#[ignore = "heavy: four 160x320 runs; run in the release profile"]
fn o_oblique_incidence_30_degrees() {
    let (got, cf) = oblique(30.0);
    assert_oblique(got, cf);
}

/// O0 to O3 and NC4 of ADR 0233 at 45 degrees.
#[test]
#[ignore = "heavy: four 160x320 runs; run in the release profile"]
fn o_oblique_incidence_45_degrees() {
    let (got, cf) = oblique(45.0);
    assert_oblique(got, cf);
    // NC4: neither quiet boundary is quiet at 45 degrees
    assert!(
        got[2].abs() >= 0.10 && got[3].abs() >= 0.10,
        "NC4 {} {}",
        got[2],
        got[3]
    );
}
