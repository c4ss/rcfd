//! F-320 (ADR 0226): wet-steam checks that isolate the nucleated-mass term (mutation MU3 of ADR 0217) and the frozen sound
//! speed at its two sites, the CFL step and the HLLC wave-speed estimates (MU6). Registered checks R1, L2, A1, A2, U1, U2 and
//! the negative controls NC3, NC4; the limits are those of ADR 0226 and are not changed after a run.
//!
//! References: ADR 0217's M1 table (values of the F-221 prototype, an implementation written before the library) and first
//! principles (the frozen sound speed is the isentropic derivative at fixed composition; an upwind flux whose left wave speed
//! is non-negative is the left state's physical flux, Toro ch. 10).

use rcfd_core::iapws::R_WATER;
use rcfd_core::vec3::Vec3;
use rcfd_fv::geometry::FvGeometry;
use rcfd_fv::wetsteam::{
    hllc, liquid_density, physical_flux, WetBc, WetCons, WetPrim, WetSettings, WetSteamModel, WetSteamSolver,
};
use rcfd_mesh::gen::box_2d;
use std::f64::consts::PI;
use std::sync::Arc;

/// Case 203 inlet stagnation state and Young's alpha (ADR 0217 M2, M5).
const P0: f64 = 35.87e3;
const T0: f64 = 368.3;
const ALPHA: f64 = 11.0;

fn model203() -> WetSteamModel {
    WetSteamModel::for_inlet(P0, T0, ALPHA).unwrap()
}

// ---- R1: rates decomposition at the M1 states --------------------------------------------------------------------------

/// A row of ADR 0217's M1 table: the state and the prototype's `rho_l`, `r*`, `J`, `dr/dt`.
struct M1Row {
    t: f64,
    p: f64,
    y: f64,
    r: f64,
    rho_l: f64,
    r_crit: f64,
    j: f64,
    dr_dt: f64,
}

const M1_ROWS: [M1Row; 3] = [
    M1Row {
        t: 285.0,
        p: 12000.0,
        y: 0.0,
        r: 0.0,
        rho_l: 9.9946874651e2,
        r_crit: 5.2173394580e-10,
        j: 2.0308632340e22,
        dr_dt: 0.0,
    },
    M1Row {
        t: 290.0,
        p: 10000.0,
        y: 0.02,
        r: 1.5e-8,
        rho_l: 9.9875642965e2,
        r_crit: 6.6372129998e-10,
        j: 7.6313126716e16,
        dr_dt: 5.7695442623e-4,
    },
    M1Row {
        t: 300.0,
        p: 9000.0,
        y: 0.04,
        r: 2.0e-8,
        rho_l: 9.9651426338e2,
        r_crit: 1.1124681327e-9,
        j: 3.4277088937e-8,
        dr_dt: 2.8906688854e-4,
    },
];

/// R1: `rates()` at the three M1 states against `J (4/3) pi r*^3 rho_l + 3 y (dr/dt)/r` and `J` from the prototype's values;
/// `dy/dt` within 5e-6 relative, `dn/dt` within 1e-6, the returned temperature and pressure within 1e-12.
#[test]
fn r1_rates_decomposition_matches_the_prototype_values() {
    let m = model203();
    for row in &M1_ROWS {
        let (t, p, y) = (row.t, row.p, row.y);
        let rho = p / ((1.0 - y) * R_WATER * t);
        let e = m.internal_energy(t, y);
        let (n, growth) = if row.r > 0.0 {
            (
                3.0 * y / (4.0 * PI * row.rho_l * row.r.powi(3)),
                3.0 * y * row.dr_dt / row.r,
            )
        } else {
            (0.0, 0.0)
        };
        let born = row.j * 4.0 / 3.0 * PI * row.r_crit.powi(3) * row.rho_l;
        let want = born + growth;
        let got = m.rates(rho, e, y, n).unwrap();
        let dy = (got.dy_dt / want - 1.0).abs();
        let dn = (got.dn_dt / row.j - 1.0).abs();
        let dtemp = (got.temperature / t - 1.0).abs();
        let dp = (got.pressure / p - 1.0).abs();
        println!(
            "R1 T {t} p {p} y {y}: n {n:.10e}; dy/dt {:.10e} (reference {want:.10e}: nucleated mass {born:.10e} + growth \
             {growth:.10e}), rel {dy:.2e}; dn/dt {:.10e} (J {:.10e}), rel {dn:.2e}; T rel {dtemp:.1e}, p rel {dp:.1e}",
            got.dy_dt, got.dn_dt, row.j
        );
        assert!(dy <= 5e-6, "R1 dy/dt at T {t}: {} vs {want}", got.dy_dt);
        assert!(dn <= 1e-6, "R1 dn/dt at T {t}: {} vs {}", got.dn_dt, row.j);
        assert!(dtemp <= 1e-12, "R1 temperature at T {t}");
        assert!(dp <= 1e-12, "R1 pressure at T {t}");
    }
}

// ---- L2: the dry nucleation box ------------------------------------------------------------------------------------------

const BOX_T: f64 = 285.0;
const BOX_P: f64 = 12.0e3;

/// The L1 box of ADR 0217 (2 x 2 cells of 1 mm, slip walls, at rest, point-implicit on), started dry at the M1 state 1.
fn dry_box() -> WetSteamSolver {
    let model = model203();
    let geom = FvGeometry::new(Arc::new(box_2d(2, 2, 2e-3, 2e-3, false, false).unwrap())).unwrap();
    let rho = model.density(BOX_P, BOX_T, 0.0);
    let mut s = WetSteamSolver::new(geom, model, WetSettings::default(), |_| WetPrim {
        rho,
        u: [0.0; 3],
        p: BOX_P,
        y: 0.0,
        n: 0.0,
    })
    .unwrap();
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        s.set_bc(p, WetBc::Slip).unwrap();
    }
    s
}

/// The isochoric ODE of the box (`rho`, `e` fixed) with the right-hand side written from `nucleation`, `liquid_density` and
/// `growth_rate` (the M6 formula, not `rates`), classic RK4: `(y, n, z)` at the requested times, `z` the nucleated-mass part
/// of `y`.
fn nucleation_reference(times: &[f64], dt: f64) -> Vec<(f64, f64, f64)> {
    let m = model203();
    let rho = m.density(BOX_P, BOX_T, 0.0);
    let e = m.internal_energy(BOX_T, 0.0);
    let f = |s: [f64; 3]| -> [f64; 3] {
        let (y, n) = (s[0], s[1]);
        let t = m.temperature(e, y);
        let p = (1.0 - y) * rho * R_WATER * t;
        let nuc = m.nucleation(t, p, y, rho).unwrap();
        let rl = liquid_density(t).unwrap();
        let born = if nuc.rate > 0.0 {
            nuc.rate * 4.0 / 3.0 * PI * nuc.r_crit.powi(3) * rl
        } else {
            0.0
        };
        let mut growth = 0.0;
        if y > 0.0 && n > 0.0 {
            let r = (3.0 * y / (4.0 * PI * rl * n)).cbrt();
            growth = n * 4.0 * PI * r * r * rl * m.growth_rate(t, p, y, rho, r).unwrap();
        }
        [born + growth, nuc.rate, born]
    };
    let mut s = [0.0f64; 3];
    let mut out = vec![];
    let mut k = 0usize;
    for &target in times {
        let steps = (target / dt).round() as usize;
        while k < steps {
            let k1 = f(s);
            let k2 = f(std::array::from_fn(|i| s[i] + 0.5 * dt * k1[i]));
            let k3 = f(std::array::from_fn(|i| s[i] + 0.5 * dt * k2[i]));
            let k4 = f(std::array::from_fn(|i| s[i] + dt * k3[i]));
            for i in 0..3 {
                s[i] += dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
            }
            k += 1;
        }
        out.push((s[0], s[1], s[2]));
    }
    out
}

/// `(y, n)` of the first cell and the relative spread of `rho E`, `rho y`, `rho n` over the cells (as ADR 0217 L1).
fn box_state(s: &WetSteamSolver) -> (f64, f64, f64) {
    let w = s.primitives().unwrap();
    let mut spread: f64 = 0.0;
    for k in [4usize, 5, 6] {
        let v: Vec<f64> = s.u.iter().map(|c| c[k]).collect();
        let (lo, hi) = v
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |a, &x| (a.0.min(x), a.1.max(x)));
        let scale = hi.abs().max(lo.abs()).max(1e-300);
        spread = spread.max((hi - lo) / scale);
    }
    (w[0].y, w[0].n, spread)
}

/// L2: the dry box (dt 1e-8 s) against the ODE written without `rates` at 2.5e-7, 5e-7, 1e-6 s: `y` and `n` within 1e-6
/// relative, cell spread at most 1e-12, no clips, and the reference's liquid at least 99 percent nucleated mass.
#[test]
fn l2_dry_nucleation_box_matches_an_ode_written_without_rates() {
    let times = [2.5e-7, 5e-7, 1e-6];
    let reference = nucleation_reference(&times, 1e-10);
    let mut s = dry_box();
    let dt = 1e-8;
    let mut k = 0usize;
    for (i, &target) in times.iter().enumerate() {
        let steps = (target / dt).round() as usize;
        while k < steps {
            s.step(dt).unwrap();
            k += 1;
        }
        let (y, n, spread) = box_state(&s);
        let (yr, nr, zr) = reference[i];
        let (dy, dn) = ((y / yr - 1.0).abs(), (n / nr - 1.0).abs());
        println!(
            "L2 t {target:.2e}: y {y:.10e} (reference {yr:.10e}, rel {dy:.2e}), n {n:.10e} (reference {nr:.10e}, rel {dn:.2e}), \
             nucleated share of the reference {:.10}, cell spread {spread:.1e}, clips {}",
            zr / yr,
            s.clips
        );
        assert!(spread <= 1e-12, "L2 spread {spread}");
        assert!(dy <= 1e-6, "L2 y at {target}: {y} vs {yr}");
        assert!(dn <= 1e-6, "L2 n at {target}: {n} vs {nr}");
        assert!(zr >= 0.99 * yr, "L2 relevance at {target}: nucleated {zr} of {yr}");
    }
    assert_eq!(s.clips, 0, "L2 clips");
}

// ---- A1, A2: the frozen sound speed and the time step ----------------------------------------------------------------------

/// `(T K, p Pa, y)` of A1 and A2.
const A_STATES: [(f64, f64, f64); 5] = [
    (300.0, 20e3, 0.0),
    (285.0, 12e3, 0.02),
    (300.0, 20e3, 0.05),
    (290.0, 10e3, 0.1),
    (330.0, 30e3, 0.2),
];

fn a_state(m: &WetSteamModel, (t, p, y): (f64, f64, f64), u: Vec3) -> WetPrim {
    WetPrim {
        rho: m.density(p, t, y),
        u,
        p,
        y,
        n: if y > 0.0 { 1e16 } else { 0.0 },
    }
}

/// The isentropic derivative at frozen composition of the library's equation of state (`to_cons`, `to_prim`), by a central
/// difference along the isentrope's tangent `de = (p/rho^2) drho`, `h = 1e-5`.
fn eos_sound_speed(m: &WetSteamModel, w: &WetPrim) -> f64 {
    let c = m.to_cons(&WetPrim { u: [0.0; 3], ..*w });
    let rho = c[0];
    let e = c[4] / rho;
    let (y, n) = (c[5] / rho, c[6] / rho);
    let h = 1e-5;
    let p_of = |r: f64, e: f64| m.to_prim(&[r, 0.0, 0.0, 0.0, r * e, r * y, r * n]).p;
    let dp = p_of(rho * (1.0 + h), e + h * w.p / rho) - p_of(rho * (1.0 - h), e - h * w.p / rho);
    (dp / (2.0 * h * rho)).sqrt()
}

/// A1: `sound_speed` equals the EOS's isentropic derivative within 1e-7 relative at the five states.
#[test]
fn a1_sound_speed_is_the_isentropic_derivative_of_the_eos() {
    let m = model203();
    for st in A_STATES {
        let w = a_state(&m, st, [0.0; 3]);
        let (c, ce) = (m.sound_speed(&w), eos_sound_speed(&m, &w));
        let vapour = (m.gamma_v() * w.p / w.rho).sqrt();
        let rel = (c / ce - 1.0).abs();
        println!(
            "A1 T {} p {} y {}: sound_speed {c:.10} m/s, EOS derivative {ce:.10} m/s, rel {rel:.2e}; vapour sound speed \
             {vapour:.6} m/s ({:+.3} %)",
            st.0,
            st.1,
            st.2,
            100.0 * (vapour / ce - 1.0)
        );
        assert!(rel <= 1e-7, "A1 at y {}: {c} vs {ce}", st.2);
    }
}

/// A2: `max_dt` of a uniform moving state equals `cfl dx dy/((|u_x| + c) dy + (|u_y| + c) dx)` with A1's `c`, within 1e-7.
#[test]
fn a2_time_step_uses_the_frozen_sound_speed() {
    let m = model203();
    let (lx, ly, nx, ny) = (3e-3, 1e-3, 3usize, 2usize);
    let (dx, dy) = (lx / nx as f64, ly / ny as f64);
    let u = [120.0, -40.0, 0.0];
    let settings = WetSettings {
        condensation: false,
        ..WetSettings::default()
    };
    for st in A_STATES {
        let w = a_state(&m, st, u);
        let geom = FvGeometry::new(Arc::new(box_2d(nx, ny, lx, ly, false, false).unwrap())).unwrap();
        let s = WetSteamSolver::new(geom, m, settings, |_| w).unwrap();
        let c = eos_sound_speed(&m, &w);
        let want = settings.cfl * dx * dy / ((u[0].abs() + c) * dy + (u[1].abs() + c) * dx);
        let got = s.max_dt().unwrap();
        let rel = (got / want - 1.0).abs();
        println!(
            "A2 T {} p {} y {}: max_dt {got:.12e} s, bound with the EOS sound speed {want:.12e} s, rel {rel:.2e}",
            st.0, st.1, st.2
        );
        assert!(rel <= 1e-7, "A2 at y {}: {got} vs {want}", st.2);
    }
}

// ---- U1, NC3: upwinding of frozen-supersonic face pairs --------------------------------------------------------------------

/// A state at frozen Mach number `mach` along `dir` (`c_f^2 = Gamma(y) p/rho`), plus `ut` along `tang`.
#[allow(clippy::too_many_arguments)]
fn moving(m: &WetSteamModel, t: f64, p: f64, y: f64, mach: f64, dir: Vec3, tang: Vec3, ut: f64) -> WetPrim {
    let rho = m.density(p, t, y);
    let c = (m.gamma_mix(y) * p / rho).sqrt();
    WetPrim {
        rho,
        u: std::array::from_fn(|i| mach * c * dir[i] + ut * tang[i]),
        p,
        y,
        n: 1e16,
    }
}

/// A U1 face: the two states, the unit normal and whether the left state is the upwind one.
struct Face {
    name: &'static str,
    l: WetPrim,
    r: WetPrim,
    n: Vec3,
    left_upwind: bool,
}

fn u1_faces(m: &WetSteamModel, mach: f64) -> [Face; 3] {
    let x = [1.0, 0.0, 0.0];
    let nn = [0.6, 0.8, 0.0];
    let tt = [-0.8, 0.6, 0.0];
    let z = [0.0; 3];
    [
        Face {
            name: "a",
            l: moving(m, 300.0, 20e3, 0.1, mach, x, z, 0.0),
            r: moving(m, 301.0, 20.1e3, 0.1, mach, x, z, 0.0),
            n: x,
            left_upwind: true,
        },
        Face {
            name: "b",
            l: moving(m, 300.0, 20e3, 0.1, -mach, x, z, 0.0),
            r: moving(m, 301.0, 20.1e3, 0.1, -mach, x, z, 0.0),
            n: x,
            left_upwind: false,
        },
        Face {
            name: "c",
            l: moving(m, 290.0, 10e3, 0.1, mach, nn, tt, 20.0),
            r: moving(m, 292.0, 10.2e3, 0.12, mach, nn, tt, 20.0),
            n: nn,
            left_upwind: true,
        },
    ]
}

/// `max_k |f_k - g_k| / max_k |g_k|`.
fn flux_gap(f: &WetCons, g: &WetCons) -> f64 {
    let scale = g.iter().fold(0.0f64, |a, x| a.max(x.abs()));
    f.iter().zip(g).fold(0.0f64, |a, (x, y)| a.max((x - y).abs())) / scale
}

fn upwind_flux(m: &WetSteamModel, face: &Face) -> (WetCons, WetCons) {
    let f = hllc(m, &face.l, &face.r, face.n);
    let up = physical_flux(m, if face.left_upwind { &face.l } else { &face.r }, face.n);
    (f, up)
}

/// U1: at frozen Mach 1.015 the HLLC flux is the upwind state's physical flux, bit for bit.
#[test]
fn u1_frozen_supersonic_faces_take_the_upwind_flux() {
    let m = model203();
    for face in u1_faces(&m, 1.015) {
        let (f, up) = upwind_flux(&m, &face);
        let bits = f.iter().zip(&up).all(|(a, b)| a.to_bits() == b.to_bits());
        println!(
            "U1 ({}): bit for bit {bits}, largest difference {:.3e} of the flux scale",
            face.name,
            flux_gap(&f, &up)
        );
        assert!(bits, "U1 ({}): {f:?} vs {up:?}", face.name);
    }
}

/// NC3: at frozen Mach 0.985 the HLLC flux differs from the upwind physical flux by more than 1e-9 of the flux scale.
#[test]
fn nc3_subsonic_faces_do_not_take_the_upwind_flux() {
    let m = model203();
    for face in u1_faces(&m, 0.985) {
        let (f, up) = upwind_flux(&m, &face);
        let gap = flux_gap(&f, &up);
        println!("NC3 ({}): largest difference {gap:.3e} of the flux scale", face.name);
        assert!(gap > 1e-9, "NC3 ({}) {gap}", face.name);
    }
}

// ---- U2, NC4: the supersonic Riemann problem -------------------------------------------------------------------------------

/// The U2 tube at frozen Mach number `mach` (100 steps of `max_dt`): the largest departure from the base state over the cells
/// with centre `x < 0.5 m`.
fn upstream_departure(mach: f64) -> f64 {
    let m = model203();
    let y = 0.1;
    let (tb, pb) = (300.0, 20e3);
    let rho = m.density(pb, tb, y);
    let ub = mach * (m.gamma_mix(y) * pb / rho).sqrt();
    let base = WetPrim {
        rho,
        u: [ub, 0.0, 0.0],
        p: pb,
        y,
        n: 0.0,
    };
    let right = WetPrim { p: pb * 1.002, ..base };
    let geom = FvGeometry::new(Arc::new(box_2d(200, 1, 1.0, 1.0 / 200.0, false, false).unwrap())).unwrap();
    let settings = WetSettings {
        condensation: false,
        ..WetSettings::default()
    };
    let mut s = WetSteamSolver::new(geom, m, settings, |x| if x[0] < 0.5 { base } else { right }).unwrap();
    s.set_bc("xmin", WetBc::Fixed(base)).unwrap();
    s.set_bc("xmax", WetBc::Extrapolate).unwrap();
    s.set_bc("ymin", WetBc::Slip).unwrap();
    s.set_bc("ymax", WetBc::Slip).unwrap();
    for _ in 0..100 {
        let dt = s.max_dt().unwrap();
        s.step(dt).unwrap();
    }
    let w = s.primitives().unwrap();
    let cc = s.geom.mesh.cell_centres();
    let (mut up, mut down): (f64, f64) = (0.0, 0.0);
    for (c, q) in w.iter().enumerate() {
        let d = (q.rho / rho - 1.0)
            .abs()
            .max((q.p / pb - 1.0).abs())
            .max((q.u[0] / ub - 1.0).abs())
            .max(q.u[1].abs() / ub)
            .max((q.y - y).abs());
        if cc[c][0] < 0.5 {
            up = up.max(d);
        } else {
            down = down.max(d);
        }
    }
    println!(
        "U2 frozen Mach {mach}: t {:.4e} s after {} steps; largest departure from the base state upstream (x < 0.5 m) \
         {up:.3e}, downstream {down:.3e}",
        s.time, s.steps
    );
    up
}

/// U2: in the frozen-supersonic tube (Mach 1.015) the cells upstream of the step keep the base state to 1e-12.
#[test]
fn u2_supersonic_riemann_problem_sends_nothing_upstream() {
    let up = upstream_departure(1.015);
    assert!(up <= 1e-12, "U2 {up}");
}

/// NC4: in the subsonic tube (Mach 0.985) the cells upstream of the step depart from the base state by more than 1e-10.
#[test]
fn nc4_subsonic_riemann_problem_sends_a_signal_upstream() {
    let up = upstream_departure(0.985);
    assert!(up > 1e-10, "NC4 {up}");
}
