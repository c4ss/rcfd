//! F-323 (ADR 0230): the vapour mass, heat and momentum of evaporating and breaking parcels in the exchange record.
//! The limits are those registered in ADR 0230 before this file was written. Every reference is computed by this file's own
//! code, not by the library's functions. Run in release.

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_particles::*;
use std::f64::consts::PI;
use std::sync::Arc;

type V3 = [f64; 3];

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

fn vnorm(a: V3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn vsum(v: &[V3]) -> V3 {
    let mut s = [0.0; 3];
    for a in v {
        for k in 0..3 {
            s[k] += a[k];
        }
    }
    s
}

fn total(v: &[f64]) -> f64 {
    v.iter().sum()
}

/// Molar gas constant `N_A k` (CODATA, exact).
const RU: f64 = 6.022_140_76e23 * 1.380_649e-23;

/// Data E of ADR 0224.
const RHO_L: f64 = 700.0;
const C_L: f64 = 2200.0;
const LV: f64 = 3.2e5;
const TB: f64 = 371.5;
const PB: f64 = 101_325.0;
const P: f64 = 101_325.0;
const WF: f64 = 0.1;
const WB: f64 = 0.029;
const RHO: f64 = 0.5;
const MU: f64 = 3e-5;
const LAM: f64 = 0.05;
const CPM: f64 = 1100.0;
const CPV: f64 = 2500.0;
const DIFF: f64 = 3e-5;
const DMIN: f64 = 1e-7;
const D0: f64 = 5e-5;
const N0: f64 = 1000.0;

fn spec_e() -> EvaporationSpec {
    EvaporationSpec {
        latent_heat: LV,
        boiling_temperature: TB,
        boiling_pressure: PB,
        pressure: P,
        vapour_molar_mass: WF,
        gas_molar_mass: WB,
        diffusivity: DIFF,
        vapour_cp: CPV,
        min_diameter: DMIN,
        max_step_fraction: 0.1,
    }
}

// ---- this file's own model (ADR 0224 E2 to E4)

fn x_of(t: f64) -> f64 {
    PB / P * (LV * WF / RU * (1.0 / TB - 1.0 / t)).exp()
}

fn y_i(t: f64) -> f64 {
    let x = x_of(t);
    x * WF / (x * WF + (1.0 - x) * WB)
}

fn b_m(t: f64, ys: f64) -> f64 {
    (y_i(t) - ys) / (1.0 - y_i(t))
}

fn nu_sh(re: f64) -> (f64, f64) {
    let pr = MU * CPM / LAM;
    let sc = MU / (RHO * DIFF);
    (2.0 + 0.6 * re.sqrt() * pr.cbrt(), 2.0 + 0.6 * re.sqrt() * sc.cbrt())
}

/// Wet bulb with Bird's correction by bisection (200 halvings on `[250, T_b - 1e-9]`): `c_p,vap (T_s - T) = B_T L_v`.
fn wet_bulb(ts: f64, ys: f64, re: f64) -> f64 {
    let (nu, sh) = nu_sh(re);
    let phi = CPV * RHO * DIFF * sh / (LAM * nu);
    let g = |t: f64| CPV * (ts - t) - ((phi * (1.0 + b_m(t, ys)).ln()).exp() - 1.0) * LV;
    let (mut lo, mut hi) = (250.0, TB - 1e-9);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if g(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

fn liquid(tr: &Tracker) -> f64 {
    tr.particles
        .iter()
        .map(|p| p.parcel * (PI * p.rho / 6.0) * p.d * p.d * p.d)
        .sum()
}

/// What was injected: mass, momentum, energy (`(1/2) m v^2 + m c_l T`).
#[derive(Clone, Copy)]
struct Injected {
    m: f64,
    p: V3,
    e: f64,
}

/// The residuals of the identities IM, IP and IE of ADR 0230 (each relative to the largest term of its identity) and the
/// terms of IP and IE for the record. `c_l` is the liquid specific heat (`None` without heat transfer).
struct Ident {
    rm: f64,
    rp: f64,
    re: f64,
    p_terms: [f64; 7],
    e_terms: [f64; 9],
    /// Residual of IP with the vapour momentum left out (NC2).
    rp_no_vapour: f64,
}

fn identities(tr: &Tracker, inj: Injected, c_l: Option<f64>) -> Ident {
    let cpl = tr.coupling().unwrap();
    let m_liq = liquid(tr);
    let s_vm = total(&cpl.vapour_mass);
    let rm_scale = m_liq.abs().max(s_vm.abs()).max(inj.m.abs());
    let rm = if rm_scale > 0.0 {
        (m_liq + s_vm - inj.m).abs() / rm_scale
    } else {
        0.0
    };
    let mut pp = [0.0; 3];
    let mut ep = 0.0;
    for (i, p) in tr.particles.iter().enumerate() {
        let m = p.parcel * (PI * p.rho / 6.0) * p.d * p.d * p.d;
        for k in 0..3 {
            pp[k] += m * p.v[k];
        }
        ep += 0.5 * m * (p.v[0] * p.v[0] + p.v[1] * p.v[1] + p.v[2] * p.v[2]);
        if let Some(c) = c_l {
            ep += m * c * tr.temperature(i);
        }
    }
    let sd = vsum(&cpl.impulse);
    let sv = vsum(&cpl.vapour_impulse);
    let (g, w, b) = (cpl.gravity_impulse, cpl.wall_impulse, cpl.breakup_impulse);
    let mut res = [0.0; 3];
    let mut res_nv = [0.0; 3];
    for k in 0..3 {
        res_nv[k] = pp[k] + sd[k] - g[k] - w[k] - b[k] - inj.p[k];
        res[k] = res_nv[k] + sv[k];
    }
    let p_terms = [
        vnorm(pp),
        vnorm(sd),
        vnorm(sv),
        vnorm(g),
        vnorm(w),
        vnorm(b),
        vnorm(inj.p),
    ];
    let p_scale = p_terms.iter().cloned().fold(0.0, f64::max);
    let (rp, rp_no_vapour) = if p_scale > 0.0 {
        (vnorm(res) / p_scale, vnorm(res_nv) / p_scale)
    } else {
        (0.0, 0.0)
    };
    let e_terms = [
        ep,
        total(&cpl.energy_mech),
        total(&cpl.energy_heat),
        total(&cpl.vapour_kinetic),
        total(&cpl.vapour_enthalpy),
        cpl.gravity_work,
        cpl.wall_energy,
        cpl.breakup_energy,
        inj.e,
    ];
    let e_res = e_terms[0] + e_terms[1] + e_terms[2] + e_terms[3] + e_terms[4]
        - e_terms[5]
        - e_terms[6]
        - e_terms[7]
        - e_terms[8];
    let e_scale = e_terms.iter().map(|x| x.abs()).fold(0.0, f64::max);
    let re = if e_scale > 0.0 { e_res.abs() / e_scale } else { 0.0 };
    Ident {
        rm,
        rp,
        re,
        p_terms,
        e_terms,
        rp_no_vapour,
    }
}

// ---- set-ups

struct Run {
    tr: Tracker,
    mesh: Arc<Mesh>,
    u: Vec<V3>,
}

impl Run {
    fn step(&mut self, dt: f64) {
        let flow = Flow {
            u: &self.u,
            grad: None,
            rho: RHO,
            mu: MU,
        };
        self.tr.advance(&flow, dt).unwrap();
    }
}

/// One parcel of `n` droplets of data E on a box of `cells` cells of 0.01 m along x, injected at `x` with the velocity `v`
/// and the temperature `t_inj`, released at `1/rate`; the gas at rest at `ts` with no vapour; the droplet exchange on.
#[allow(clippy::too_many_arguments)]
fn single(
    cells: usize,
    x: V3,
    v: V3,
    n: f64,
    t_inj: f64,
    ts: f64,
    drag: DragLaw,
    g: V3,
    wall: WallAction,
    rate: f64,
) -> Run {
    let mesh = Arc::new(box_mesh([cells, 1, 1], [0.01 * cells as f64, 0.01, 0.01], [Grading::Uniform; 3]).unwrap());
    let opts = Options {
        drag,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: g,
        default_wall: wall,
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh.clone(), opts, &[]).unwrap();
    tr.add_injector(InjectorSpec {
        name: "d".into(),
        kind: InjKind::Point(x),
        rate,
        t_start: 0.0,
        t_end: 1.0 / rate,
        velocity: VelMode::Given(v),
        dist: SizeDist::Constant(D0),
        density: RHO_L,
        parcel_count: n,
        seed: 1,
    })
    .unwrap();
    tr.set_injector_ext(
        "d",
        InjectorExt {
            cone: None,
            mass_flow: None,
            temperature: Some(t_inj),
        },
    )
    .unwrap();
    tr.set_thermal(Thermal {
        cp_p: C_L,
        k_f: LAM,
        cp_f: CPM,
        t_fluid: vec![ts; cells],
    })
    .unwrap();
    tr.set_evaporation(spec_e(), vec![0.0; cells]).unwrap();
    tr.enable_droplet_exchange();
    let u = vec![[0.0; 3]; cells];
    Run { tr, mesh, u }
}

fn at_rest(t_inj: f64) -> Run {
    single(
        1,
        [0.005; 3],
        [0.0; 3],
        N0,
        t_inj,
        800.0,
        DragLaw::Stokes,
        [0.0; 3],
        WallAction::Reflect(1.0),
        1e4,
    )
}

fn m0() -> f64 {
    N0 * (PI * RHO_L / 6.0) * D0 * D0 * D0
}

fn zero_bits(x: f64) -> bool {
    x.to_bits() == 0
}

fn zero_bits3(v: V3) -> bool {
    v.iter().all(|x| x.to_bits() == 0)
}

/// DX1 (mass and heat at the wet bulb) with NC1.
#[test]
fn dx1_droplet_at_its_wet_bulb_draws_the_latent_heat_from_the_gas() {
    let twb = wet_bulb(800.0, 0.0, 0.0);
    let mut r = at_rest(twb);
    let inj = Injected {
        m: m0(),
        p: [0.0; 3],
        e: m0() * C_L * twb,
    };
    let (mut wb, mut wc, mut wd, mut we) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut steps = 0;
    for step in 1..=200 {
        r.step(1e-4);
        let tr = &r.tr;
        let cpl = tr.coupling().unwrap();
        let p = &tr.particles[0];
        let lost = N0 * (PI * RHO_L / 6.0) * (D0 * D0 * D0 - p.d * p.d * p.d);
        // (f)
        assert!(
            zero_bits3(cpl.impulse[0]) && zero_bits3(cpl.vapour_impulse[0]),
            "DX1 f step {step}: impulses"
        );
        assert!(
            zero_bits(cpl.energy_mech[0]) && zero_bits(cpl.vapour_kinetic[0]),
            "DX1 f step {step}: mechanical"
        );
        assert!(
            zero_bits3(cpl.gravity_impulse) && zero_bits(cpl.gravity_work),
            "DX1 f step {step}: gravity"
        );
        assert!(
            zero_bits3(cpl.wall_impulse) && zero_bits(cpl.wall_energy),
            "DX1 f step {step}: wall"
        );
        assert!(
            zero_bits3(cpl.breakup_impulse) && zero_bits(cpl.breakup_energy),
            "DX1 f step {step}: breakup"
        );
        // (a)
        assert_eq!(
            cpl.vapour_mass[0].to_bits(),
            tr.evaporation().unwrap().source()[0].to_bits(),
            "DX1 a step {step}"
        );
        if lost > 0.0 {
            let s = total(&cpl.vapour_mass);
            let b = rel(s, lost);
            let c = rel(-total(&cpl.energy_heat), LV * s);
            let d = rel(total(&cpl.vapour_enthalpy), (C_L * twb + LV) * s);
            let e = identities(tr, inj, Some(C_L)).re;
            wb = wb.max(b);
            wc = wc.max(c);
            wd = wd.max(d);
            we = we.max(e);
            assert!(b <= 1e-10, "DX1 b step {step}: {b:e}");
            assert!(c <= 1e-10, "DX1 c step {step}: {c:e}");
            assert!(d <= 1e-10, "DX1 d step {step}: {d:e}");
            assert!(e <= 1e-10, "DX1 e step {step}: {e:e}");
        }
        steps = step;
        if p.state != State::Active {
            break;
        }
    }
    let tr = &r.tr;
    let cpl = tr.coupling().unwrap();
    assert_eq!(tr.particles[0].state, State::Evaporated, "DX1 completion");
    let end = rel(total(&cpl.vapour_mass), m0());
    let heat = -total(&cpl.energy_heat);
    let nc1 = rel(heat, (C_L * twb + LV) * m0());
    println!(
        "DX1: T_wb {twb:.10} K, {steps} steps; worst b {wb:.2e}, c {wc:.2e}, d {wd:.2e}, e {we:.2e}; at completion vapour against M0 {end:.2e}; heat from the gas {heat:.10e} J against L_v M0 {:.10e}; NC1 {nc1:.4} off",
        LV * m0()
    );
    assert!(end <= 1e-10, "DX1 completion mass: {end:e}");
    assert!(nc1 >= 0.5, "NC1 must be at least 50 percent off: {nc1}");
}

/// DX2 (heat-up).
#[test]
fn dx2_cold_droplet_conserves_mass_and_energy_through_its_heat_up() {
    let twb = wet_bulb(800.0, 0.0, 0.0);
    let mut r = at_rest(300.0);
    let inj = Injected {
        m: m0(),
        p: [0.0; 3],
        e: m0() * C_L * 300.0,
    };
    let (mut wm, mut we) = (0.0f64, 0.0f64);
    let mut steps = 0;
    for step in 1..=200 {
        r.step(1e-4);
        let id = identities(&r.tr, inj, Some(C_L));
        wm = wm.max(id.rm);
        we = we.max(id.re);
        assert!(id.rm <= 1e-10, "DX2 IM step {step}: {:e}", id.rm);
        assert!(id.re <= 1e-10, "DX2 IE step {step}: {:e}", id.re);
        steps = step;
        if r.tr.particles[0].state != State::Active {
            break;
        }
    }
    assert_eq!(r.tr.particles[0].state, State::Evaporated, "DX2 completion");
    let cpl = r.tr.coupling().unwrap();
    let excess = -total(&cpl.energy_heat) - LV * m0();
    let bound = m0() * C_L * (twb - 300.0);
    println!(
        "DX2: {steps} steps; worst IM {wm:.2e}, IE {we:.2e}; heat that warmed the liquid {excess:.6e} J, bound {bound:.6e} J ({:.4} of it)",
        excess / bound
    );
    assert!(
        excess > 0.0 && excess <= bound,
        "DX2 excess {excess:e} not in (0, {bound:e}]"
    );
}

/// `e(h)` of DX3: the heat of the first sub-step of a droplet at 300 K against the model's `Q h`.
fn heat_error(h: f64) -> f64 {
    let mut r = single(
        1,
        [0.005; 3],
        [0.0; 3],
        N0,
        300.0,
        800.0,
        DragLaw::Stokes,
        [0.0; 3],
        WallAction::Reflect(1.0),
        1.0 / h,
    );
    r.step(h);
    assert_eq!(r.tr.particles.len(), 1, "DX3 release");
    assert_eq!(r.tr.particles[0].age, 0.0, "DX3 released at the end of the first step");
    r.step(h);
    let heat = -total(&r.tr.coupling().unwrap().energy_heat);
    let beta = CPV * RHO * DIFF / LAM * (1.0 + b_m(300.0, 0.0)).ln();
    let g = beta / (beta.exp() - 1.0);
    let q = N0 * PI * D0 * LAM * 2.0 * g * (800.0 - 300.0);
    heat / (q * h) - 1.0
}

/// DX3 (the heat against the model's `Q`, first order).
#[test]
fn dx3_heat_of_a_sub_step_is_first_order_consistent_with_the_model() {
    let e1 = heat_error(1e-6);
    let e2 = heat_error(2e-6);
    let ratio = e2 / e1;
    println!("DX3: e(1e-6) {e1:.6e}, e(2e-6) {e2:.6e}, ratio {ratio:.6}");
    assert!(e1.abs() <= 1e-3, "DX3 e(1e-6): {e1:e}");
    assert!((1.9..=2.1).contains(&ratio), "DX3 ratio {ratio}");
}

/// DX4 (momentum and kinetic energy of the vapour, many cells) and SR1 (sources).
#[test]
fn dx4_vapour_carries_its_momentum_and_kinetic_energy() {
    let v = [20.0, 0.0, 0.0];
    let re0 = RHO * D0 * 20.0 / MU;
    let twb = wet_bulb(800.0, 0.0, re0);
    let mut r = single(
        8,
        [0.005; 3],
        v,
        N0,
        twb,
        800.0,
        DragLaw::None,
        [0.0; 3],
        WallAction::Reflect(1.0),
        1e5,
    );
    let inj = Injected {
        m: m0(),
        p: [m0() * 20.0, 0.0, 0.0],
        e: 0.5 * m0() * 400.0 + m0() * C_L * twb,
    };
    let (mut wp, mut wk, mut wm, mut wip, mut wie) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for step in 1..=300 {
        r.step(1e-5);
        let tr = &r.tr;
        let cpl = tr.coupling().unwrap();
        let p = &tr.particles[0];
        for k in 0..3 {
            assert_eq!(p.v[k].to_bits(), v[k].to_bits(), "DX4 velocity step {step}");
        }
        let src = tr.evaporation().unwrap().source();
        for c in 0..8 {
            assert!(
                zero_bits3(cpl.impulse[c]) && zero_bits(cpl.energy_mech[c]),
                "DX4 drag step {step} cell {c}"
            );
            assert_eq!(
                cpl.vapour_mass[c].to_bits(),
                src[c].to_bits(),
                "DX4 vapour mass step {step} cell {c}"
            );
        }
        let lost = m0() - p.parcel * (PI * RHO_L / 6.0) * p.d * p.d * p.d;
        if lost > 0.0 {
            let sv = vsum(&cpl.vapour_impulse);
            let ep = vnorm([sv[0] - lost * 20.0, sv[1], sv[2]]) / (lost * 20.0);
            let ek = rel(total(&cpl.vapour_kinetic), 200.0 * lost);
            wp = wp.max(ep);
            wk = wk.max(ek);
            assert!(ep <= 1e-10, "DX4 vapour momentum step {step}: {ep:e}");
            assert!(ek <= 1e-10, "DX4 vapour kinetic energy step {step}: {ek:e}");
        }
        let id = identities(tr, inj, Some(C_L));
        wm = wm.max(id.rm);
        wip = wip.max(id.rp);
        wie = wie.max(id.re);
        assert!(id.rm <= 1e-10, "DX4 IM step {step}: {:e}", id.rm);
        assert!(id.rp <= 1e-10, "DX4 IP step {step}: {:e}", id.rp);
        assert!(id.re <= 1e-10, "DX4 IE step {step}: {:e}", id.re);
    }
    let tr = &r.tr;
    let cpl = tr.coupling().unwrap();
    let cells = cpl.vapour_mass.iter().filter(|m| **m > 0.0).count();
    let p = &tr.particles[0];
    println!(
        "DX4: Re0 {re0:.4}, T_wb {twb:.8} K; worst vapour momentum {wp:.2e}, kinetic {wk:.2e}, IM {wm:.2e}, IP {wip:.2e}, IE {wie:.2e}; cells with vapour {cells}; x {:.6}, d/d0 {:.6}, state {:?}",
        p.x[0],
        p.d / D0,
        p.state
    );
    assert!(cells >= 5, "DX4 cells with vapour: {cells}");
    assert_eq!(p.state, State::Active, "DX4 the parcel is active");

    // SR1
    let t = 3e-3;
    let vol = r.mesh.cell_volumes();
    let s = cpl.droplet_sources(vol, t);
    let mut worst = 0.0f64;
    let mut check = |got: f64, want: f64, what: &str, c: usize| {
        if want == 0.0 {
            assert_eq!(got, 0.0, "SR1 {what} cell {c}: zero");
        } else {
            let e = rel(got, want);
            worst = worst.max(e);
            assert!(e <= 1e-12, "SR1 {what} cell {c}: {e:e}");
        }
    };
    for c in 0..8 {
        let f = vol[c] * t;
        check(s.mass[c] * f, cpl.vapour_mass[c], "mass", c);
        for k in 0..3 {
            check(
                s.momentum[c][k] * f,
                cpl.impulse[c][k] + cpl.vapour_impulse[c][k],
                "momentum",
                c,
            );
        }
        check(
            s.energy[c] * f,
            cpl.energy_mech[c] + cpl.energy_heat[c] + cpl.vapour_kinetic[c] + cpl.vapour_enthalpy[c],
            "energy",
            c,
        );
    }
    println!("SR1: worst {worst:.2e}");
}

/// DX5 (walls and gravity).
#[test]
fn dx5_identities_close_with_walls_and_gravity() {
    let v = [3.0, 0.0, -20.0];
    let mut r = single(
        1,
        [0.005; 3],
        v,
        N0,
        300.0,
        800.0,
        DragLaw::None,
        [0.0, 0.0, -9.81],
        WallAction::Reflect(0.8),
        1e5,
    );
    let inj = Injected {
        m: m0(),
        p: [m0() * 3.0, 0.0, -m0() * 20.0],
        e: 0.5 * m0() * 409.0 + m0() * C_L * 300.0,
    };
    let (mut wm, mut wp, mut we) = (0.0f64, 0.0f64, 0.0f64);
    for step in 1..=100 {
        r.step(1e-5);
        let id = identities(&r.tr, inj, Some(C_L));
        wm = wm.max(id.rm);
        wp = wp.max(id.rp);
        we = we.max(id.re);
        assert!(id.rm <= 1e-10, "DX5 IM step {step}: {:e}", id.rm);
        assert!(id.rp <= 1e-10, "DX5 IP step {step}: {:e}", id.rp);
        assert!(id.re <= 1e-10, "DX5 IE step {step}: {:e}", id.re);
    }
    let cpl = r.tr.coupling().unwrap();
    let p = &r.tr.particles[0];
    println!(
        "DX5: worst IM {wm:.2e}, IP {wp:.2e}, IE {we:.2e}; wall impulse {:?}, wall energy {:.6e} J, gravity impulse z {:.6e}, gravity work {:.6e} J; v {:?}, d/d0 {:.6}, state {:?}",
        cpl.wall_impulse,
        cpl.wall_energy,
        cpl.gravity_impulse[2],
        cpl.gravity_work,
        p.v,
        p.d / D0,
        p.state
    );
    assert!(cpl.wall_impulse[2] > 0.0, "DX5 the floor was hit");
    assert!(cpl.wall_energy < 0.0, "DX5 the reflection lost kinetic energy");
    assert!(cpl.gravity_impulse[2].abs() > 0.0, "DX5 gravity acted");
}

/// BX1 (breakup alone): the BU4 set-up of ADR 0224 at We = 1000.
#[test]
fn bx1_breakup_books_its_momentum_and_energy_without_a_carrier_counterpart() {
    let (sigma, rho_g, d0, r0, n0) = (0.02, 1.0, 2e-4, 1e-4, 100.0);
    let we: f64 = 1000.0;
    let mesh = Arc::new(box_mesh([1, 1, 1], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap());
    let opts = Options {
        drag: DragLaw::None,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0; 3],
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh, opts, &[]).unwrap();
    tr.add_injector(InjectorSpec {
        name: "d".into(),
        kind: InjKind::Point([0.5; 3]),
        rate: 1e5,
        t_start: 0.0,
        t_end: 1e-5,
        velocity: VelMode::Given([0.0; 3]),
        dist: SizeDist::Constant(d0),
        density: RHO_L,
        parcel_count: n0,
        seed: 3,
    })
    .unwrap();
    tr.set_breakup(BreakupSpec::tab(sigma, 0.0, 7)).unwrap();
    tr.enable_droplet_exchange();
    let speed = (we * sigma / (rho_g * r0)).sqrt();
    let u = vec![[speed, 0.0, 0.0]];
    let flow = Flow {
        u: &u,
        grad: None,
        rho: rho_g,
        mu: 1.8e-5,
    };
    for _ in 0..60 {
        tr.advance(&flow, 1e-5).unwrap();
    }
    let p = &tr.particles[0];
    let cpl = tr.coupling().unwrap();
    let m = p.parcel * (PI * p.rho / 6.0) * p.d * p.d * p.d;
    let m_0 = n0 * (PI * RHO_L / 6.0) * d0 * d0 * d0;
    let bi = cpl.breakup_impulse;
    let e_imp = vnorm([bi[0] - m * p.v[0], bi[1] - m * p.v[1], bi[2] - m * p.v[2]]) / (m * vnorm(p.v));
    let w = we / 12.0;
    let om0 = (8.0 * sigma / (RHO_L * r0 * r0 * r0)).sqrt();
    let yd_b = w * om0 * (2.0 / w - 1.0 / (w * w)).sqrt();
    let e_abs = rel(vnorm(bi), m_0 * 0.5 * r0 * yd_b);
    let e_en = rel(cpl.breakup_energy, 0.5 * m * vnorm(p.v) * vnorm(p.v));
    println!(
        "BX1: breakups {}, breakup impulse {bi:?} N s against m v: {e_imp:.2e}; its norm against M0 C_b r0 yd_b: {e_abs:.2e}; breakup energy {:.8e} J: {e_en:.2e}; mass {:.2e}",
        tr.breakups(0).0,
        cpl.breakup_energy,
        rel(m, m_0)
    );
    assert_eq!(tr.breakups(0).0, 1, "BX1 one breakup");
    assert!(e_imp <= 1e-12, "BX1 impulse against m v: {e_imp:e}");
    assert!(e_abs <= 1e-10, "BX1 impulse norm: {e_abs:e}");
    assert!(e_en <= 1e-12, "BX1 energy: {e_en:e}");
    assert!(
        zero_bits3(cpl.impulse[0]) && zero_bits(cpl.energy_mech[0]),
        "BX1 the carrier received nothing"
    );
    assert!(
        zero_bits(cpl.vapour_mass[0])
            && zero_bits3(cpl.vapour_impulse[0])
            && zero_bits(cpl.vapour_kinetic[0])
            && zero_bits(cpl.vapour_enthalpy[0]),
        "BX1 vapour fields"
    );
}

// ---- the spray (scenario SP of ADR 0224 C)

#[derive(Clone, Copy, PartialEq)]
enum Record {
    None,
    Plain,
    Droplet,
}

fn sp_injectors() -> [InjectorSpec; 2] {
    [
        InjectorSpec {
            name: "a".into(),
            kind: InjKind::Point([0.005, 0.012, 0.012]),
            rate: 2e4,
            t_start: 0.0,
            t_end: 0.004,
            velocity: VelMode::Given([0.0; 3]),
            dist: SizeDist::Constant(4e-4),
            density: RHO_L,
            parcel_count: 20.0,
            seed: 21,
        },
        InjectorSpec {
            name: "b".into(),
            kind: InjKind::Point([0.005, 0.008, 0.008]),
            rate: 2e4,
            t_start: 0.001,
            t_end: 0.005,
            velocity: VelMode::Given([0.0; 3]),
            dist: SizeDist::Constant(3e-5),
            density: RHO_L,
            parcel_count: 1000.0,
            seed: 22,
        },
    ]
}

/// The tracker of scenario SP; `models` switches evaporation and breakup.
fn sp_tracker(models: bool, record: Record) -> (Tracker, Vec<V3>) {
    let mesh = Arc::new(box_mesh([8, 2, 2], [0.08, 0.02, 0.02], [Grading::Uniform; 3]).unwrap());
    let opts = Options {
        drag: DragLaw::SchillerNaumann,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0, 0.0, -9.81],
        default_wall: WallAction::Reflect(0.8),
        ..Options::default()
    };
    let overrides = [
        ("xmax".to_string(), WallAction::Escape),
        ("zmin".to_string(), WallAction::Stick),
    ];
    let mut tr = Tracker::new(mesh.clone(), opts, &overrides).unwrap();
    for (spec, t) in sp_injectors().iter().zip([300.0, 320.0]) {
        tr.add_injector(spec.clone()).unwrap();
        tr.set_injector_ext(
            &spec.name,
            InjectorExt {
                cone: None,
                mass_flow: None,
                temperature: Some(t),
            },
        )
        .unwrap();
    }
    let xc: Vec<f64> = mesh.cell_centres().iter().map(|c| c[0]).collect();
    tr.set_thermal(Thermal {
        cp_p: C_L,
        k_f: LAM,
        cp_f: CPM,
        t_fluid: xc.iter().map(|x| 700.0 + 1250.0 * x).collect(),
    })
    .unwrap();
    if models {
        tr.set_evaporation(spec_e(), xc.iter().map(|x| 0.05 * x / 0.08).collect())
            .unwrap();
        tr.set_breakup(BreakupSpec::tab(0.01, 4e-4, 31)).unwrap();
    }
    match record {
        Record::None => {}
        Record::Plain => tr.enable_coupling(),
        Record::Droplet => tr.enable_droplet_exchange(),
    }
    let u = vec![[30.0, 0.0, -2.0]; mesh.n_cells()];
    (tr, u)
}

fn sp_flow(u: &[V3]) -> Flow<'_> {
    Flow {
        u,
        grad: None,
        rho: RHO,
        mu: MU,
    }
}

/// DX6 (acceptance), DX7 (observation only) and NC2.
#[test]
fn dx6_spray_with_evaporation_and_breakup_conserves_mass_momentum_and_energy() {
    let (mut tr, u) = sp_tracker(true, Record::Droplet);
    let inj = sp_injectors();
    let m_a = 20.0 * (PI * RHO_L / 6.0) * 4e-4f64.powi(3);
    let m_b = 1000.0 * (PI * RHO_L / 6.0) * 3e-5f64.powi(3);
    let (mut wm, mut wp, mut we) = (0.0f64, 0.0f64, 0.0f64);
    let mut last = None;
    for step in 1..=300 {
        tr.advance(&sp_flow(&u), 1e-4).unwrap();
        let t = step as f64 * 1e-4;
        let (na, nb) = (inj[0].emitted_by(t) as f64, inj[1].emitted_by(t) as f64);
        let injected = Injected {
            m: na * m_a + nb * m_b,
            p: [0.0; 3],
            e: na * m_a * C_L * 300.0 + nb * m_b * C_L * 320.0,
        };
        let id = identities(&tr, injected, Some(C_L));
        wm = wm.max(id.rm);
        wp = wp.max(id.rp);
        we = we.max(id.re);
        assert!(id.rm <= 1e-10, "DX6 IM step {step}: {:e}", id.rm);
        assert!(id.rp <= 1e-10, "DX6 IP step {step}: {:e}", id.rp);
        assert!(id.re <= 1e-10, "DX6 IE step {step}: {:e}", id.re);
        last = Some(id);
    }
    let id = last.unwrap();
    let cpl = tr.coupling().unwrap();
    let counts = [
        State::Active,
        State::Deposited,
        State::Escaped,
        State::Stuck,
        State::Film,
        State::Evaporated,
    ]
    .map(|s| tr.count(s));
    let events = tr.breakup().unwrap().events();
    let bi = vnorm(cpl.breakup_impulse);
    println!(
        "DX6: {} parcels; worst IM {wm:.2e}, IP {wp:.2e}, IE {we:.2e}; breakup events {events}; active/deposited/escaped/stuck/film/evaporated {counts:?}; vapour {:.6e} kg",
        tr.particles.len(),
        total(&cpl.vapour_mass)
    );
    println!(
        "DX6 IP terms [parcels, drag, vapour, gravity, wall, breakup, injected] {:?} N s; breakup impulse over the drag impulse {:.4e}",
        id.p_terms,
        bi / id.p_terms[1]
    );
    println!(
        "DX6 IE terms [parcels, mech, heat, vapour kinetic, vapour enthalpy, gravity work, wall, breakup, injected] {:?} J",
        id.e_terms
    );
    println!("NC2: IP without the vapour momentum {:.4e}", id.rp_no_vapour);
    assert!(events >= 80, "DX6 breakup events {events}");
    assert!(bi > 0.0, "DX6 breakup impulse");
    assert!(counts[5] >= 1, "DX6 one parcel evaporated: {counts:?}");
    assert!(counts[2] >= 1, "DX6 one parcel escaped: {counts:?}");
    assert!(
        id.rp_no_vapour >= 1e-3,
        "NC2 must be at least 1e-3: {:e}",
        id.rp_no_vapour
    );

    // DX7
    let (mut plain, u2) = sp_tracker(true, Record::None);
    for _ in 0..300 {
        plain.advance(&sp_flow(&u2), 1e-4).unwrap();
    }
    assert_eq!(
        tr.fingerprint(),
        plain.fingerprint(),
        "DX7 the record changes the parcels"
    );
    println!("DX7: {} fingerprint words equal", plain.fingerprint().len());
}

/// CL1 (closed loop) with NC3.
#[test]
fn cl1_closed_box_reaches_the_adiabatic_saturation_state() {
    let n = 20000.0;
    let mut r = single(
        1,
        [0.005; 3],
        [0.0; 3],
        n,
        300.0,
        500.0,
        DragLaw::Stokes,
        [0.0; 3],
        WallAction::Reflect(1.0),
        1e4,
    );
    let m_air = 5e-7;
    let m_l0 = n * (PI * RHO_L / 6.0) * D0 * D0 * D0;
    let mut m_v = 0.0;
    let mut h_g = m_air * CPM * 500.0;
    let h0 = h_g + m_l0 * C_L * 300.0;
    let (mut wm, mut wh) = (0.0f64, 0.0f64);
    let (mut t_g, mut y_s) = (500.0, 0.0);
    for step in 1..=2000 {
        r.step(1e-4);
        {
            let cpl = r.tr.coupling().unwrap();
            m_v += total(&cpl.vapour_mass);
            h_g += total(&cpl.energy_heat) + total(&cpl.vapour_enthalpy);
        }
        r.tr.coupling_mut().unwrap().reset();
        t_g = (h_g - m_v * LV) / (m_air * CPM + m_v * C_L);
        y_s = m_v / (m_air + m_v);
        r.tr.thermal_mut().unwrap().t_fluid[0] = t_g;
        r.tr.set_evaporation_vapour(&[y_s]).unwrap();
        let m_liq = liquid(&r.tr);
        let em = rel(m_liq + m_v, m_l0);
        let eh = rel(h_g + m_liq * C_L * r.tr.temperature(0), h0);
        wm = wm.max(em);
        wh = wh.max(eh);
        assert!(em <= 1e-10, "CL1 mass step {step}: {em:e}");
        assert!(eh <= 1e-10, "CL1 enthalpy step {step}: {eh:e}");
    }
    let res = |t: f64| (m_air * CPM + m_l0 * C_L) * t + m_air * y_i(t) / (1.0 - y_i(t)) * LV - h0;
    let (mut lo, mut hi) = (280.0, 360.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if res(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t_eq = 0.5 * (lo + hi);
    let p = &r.tr.particles[0];
    let t_p = r.tr.temperature(0);
    let nc3 = (t_eq - h0 / (m_air * CPM + m_l0 * C_L)).abs();
    println!(
        "CL1: M0 {m_l0:.10e} kg, H(0) {h0:.10} J, T_eq {t_eq:.10} K, Y_i(T_eq) {:.10}; worst mass {wm:.2e}, enthalpy {wh:.2e}; T_g - T_eq {:.3e} K, T_p - T_eq {:.3e} K, Y_s - Y_eq {:.3e}; M_v {m_v:.6e} kg, d/d0 {:.6}, state {:?}; NC3 {nc3:.3} K",
        y_i(t_eq),
        t_g - t_eq,
        t_p - t_eq,
        y_s - y_i(t_eq),
        p.d / D0,
        p.state
    );
    assert!((t_g - t_eq).abs() <= 1e-6, "CL1 gas temperature: {:e}", t_g - t_eq);
    assert!((t_p - t_eq).abs() <= 1e-6, "CL1 droplet temperature: {:e}", t_p - t_eq);
    assert!(
        (y_s - y_i(t_eq)).abs() <= 1e-8,
        "CL1 vapour fraction: {:e}",
        y_s - y_i(t_eq)
    );
    assert!(p.state == State::Active && p.d < D0, "CL1 liquid is left");
    assert!(nc3 >= 10.0, "NC3 must be at least 10 K: {nc3}");
}

/// RF3: the refusals and the controls of the two records.
#[test]
fn rf3_records_and_droplet_models() {
    let mesh = Arc::new(box_mesh([1, 1, 1], [0.01, 0.01, 0.01], [Grading::Uniform; 3]).unwrap());
    let u = vec![[0.0; 3]];
    let flow = Flow {
        u: &u,
        grad: None,
        rho: RHO,
        mu: MU,
    };
    let opts = Options {
        interp: Interp::CellCentre,
        ..Options::default()
    };
    let th = Thermal {
        cp_p: C_L,
        k_f: LAM,
        cp_f: CPM,
        t_fluid: vec![800.0],
    };
    // (evaporation, breakup, the calls that start the record: false = enable_coupling, true = enable_droplet_exchange)
    let build = |evap: bool, bu: bool, records: &[bool]| {
        let mut tr = Tracker::new(mesh.clone(), opts.clone(), &[]).unwrap();
        if evap {
            tr.set_thermal(th.clone()).unwrap();
            tr.set_evaporation(spec_e(), vec![0.0]).unwrap();
        }
        if bu {
            tr.set_breakup(BreakupSpec::tab(0.02, 1e-3, 7)).unwrap();
        }
        for droplet in records {
            if *droplet {
                tr.enable_droplet_exchange();
            } else {
                tr.enable_coupling();
            }
        }
        tr
    };
    let refused: [(&str, bool, bool, &[bool]); 3] = [
        ("evaporation with enable_coupling", true, false, &[false]),
        ("breakup with enable_coupling", false, true, &[false]),
        (
            "evaporation with enable_droplet_exchange then enable_coupling",
            true,
            false,
            &[true, false],
        ),
    ];
    for (what, evap, bu, records) in refused {
        let mut tr = build(evap, bu, records);
        let r = tr.advance(&flow, 1e-4);
        println!("RF3 {what}: {r:?}");
        assert!(r.is_err(), "RF3 {what} must be refused");
    }
    let accepted: [(&str, bool, bool); 4] = [
        ("evaporation", true, false),
        ("breakup", false, true),
        ("both", true, true),
        ("neither", false, false),
    ];
    for (what, evap, bu) in accepted {
        let mut tr = build(evap, bu, &[true]);
        let r = tr.advance(&flow, 1e-4);
        assert!(r.is_ok(), "RF3 {what} with enable_droplet_exchange: {r:?}");
    }
    let rx = ReactionSpec {
        solid_molar_mass: 0.012,
        solid_coefficient: 1.0,
        gas: vec![
            GasSpecies {
                name: "O".into(),
                molar_mass: 0.032,
                coefficient: -1.0,
            },
            GasSpecies {
                name: "P".into(),
                molar_mass: 0.044,
                coefficient: 1.0,
            },
        ],
        oxidiser: 0,
        rate: 0.1,
        reacting_fraction: 0.875,
    };
    let mut tr = build(false, false, &[true]);
    let r = tr.set_reaction(rx.clone(), vec![0.2]);
    println!("RF3 reaction after enable_droplet_exchange: {r:?}");
    assert!(r.is_err(), "RF3 the reaction must be refused with the record");
    let mut tr = build(false, false, &[]);
    assert!(
        tr.set_reaction(rx, vec![0.2]).is_ok(),
        "RF3 control: the reaction without a record"
    );
}

/// IN1 (inert): without evaporation and breakup the two records are the same.
#[test]
fn in1_record_without_droplet_models_is_the_record_of_adr_0057() {
    let run = |record: Record| {
        let (mut tr, u) = sp_tracker(false, record);
        for _ in 0..300 {
            tr.advance(&sp_flow(&u), 1e-4).unwrap();
        }
        tr
    };
    let (a, b) = (run(Record::Plain), run(Record::Droplet));
    assert_eq!(a.fingerprint(), b.fingerprint(), "IN1 fingerprints");
    let (ca, cb) = (a.coupling().unwrap(), b.coupling().unwrap());
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<u64>>();
    let bits3 = |v: &[V3]| v.iter().flat_map(|x| x.map(f64::to_bits)).collect::<Vec<u64>>();
    assert_eq!(bits3(&ca.impulse), bits3(&cb.impulse), "IN1 impulse");
    assert_eq!(bits(&ca.energy_mech), bits(&cb.energy_mech), "IN1 energy_mech");
    assert_eq!(bits(&ca.energy_heat), bits(&cb.energy_heat), "IN1 energy_heat");
    assert_eq!(bits(&ca.wall_impulse), bits(&cb.wall_impulse), "IN1 wall_impulse");
    assert_eq!(
        bits(&ca.gravity_impulse),
        bits(&cb.gravity_impulse),
        "IN1 gravity_impulse"
    );
    assert_eq!(ca.gravity_work.to_bits(), cb.gravity_work.to_bits(), "IN1 gravity_work");
    assert_eq!(ca.wall_energy.to_bits(), cb.wall_energy.to_bits(), "IN1 wall_energy");
    for c in [ca, cb] {
        assert!(c.vapour_mass.iter().all(|x| zero_bits(*x)), "IN1 vapour mass");
        assert!(c.vapour_impulse.iter().all(|x| zero_bits3(*x)), "IN1 vapour impulse");
        assert!(c.vapour_kinetic.iter().all(|x| zero_bits(*x)), "IN1 vapour kinetic");
        assert!(c.vapour_enthalpy.iter().all(|x| zero_bits(*x)), "IN1 vapour enthalpy");
        assert!(
            zero_bits3(c.breakup_impulse) && zero_bits(c.breakup_energy),
            "IN1 breakup"
        );
    }
    let nonzero = ca.impulse.iter().filter(|x| vnorm(**x) > 0.0).count();
    println!(
        "IN1: {} parcels, {} fingerprint words, {nonzero} cells with a drag impulse, wall impulse {:?}, heat {:.6e} J",
        a.particles.len(),
        a.fingerprint().len(),
        ca.wall_impulse,
        total(&ca.energy_heat)
    );
    assert!(nonzero > 0, "IN1 the record is not empty");
}
