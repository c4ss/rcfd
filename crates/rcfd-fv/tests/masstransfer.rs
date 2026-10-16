//! Checks M1-M9 of ADR 0040: interfacial mass-transfer closures against hand values, invariants and closed forms.

use rcfd_core::iapws::saturation_pressure;
use rcfd_fv::masstransfer::*;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

#[test]
fn m1_lee_exact_values_and_signs() {
    let m = LeeModel {
        c_evap: 0.1,
        c_cond: 0.2,
    };
    let r = m.rate(0.6, 0.4, 958.0, 0.6, 380.0, 373.15).unwrap();
    assert!(rel(r, 0.1 * 0.6 * 958.0 * (380.0 - 373.15) / 373.15) <= 1e-14, "{r}");
    let c = m.rate(0.7, 0.3, 958.0, 0.6, 360.0, 373.15).unwrap();
    assert!(rel(c, -0.2 * 0.3 * 0.6 * (373.15 - 360.0) / 373.15) <= 1e-14, "{c}");
    assert_eq!(m.rate(0.5, 0.5, 958.0, 0.6, 373.15, 373.15).unwrap(), 0.0);
    for i in 0..200 {
        let t = 343.15 + 0.3 * i as f64;
        let r = m.rate(0.5, 0.5, 958.0, 0.6, t, 373.15).unwrap();
        assert_eq!(r > 0.0, t > 373.15, "t = {t}");
        assert_eq!(r < 0.0, t < 373.15, "t = {t}");
    }
    assert_eq!(m.rate(0.0, 1.0, 958.0, 0.6, 400.0, 373.15).unwrap(), 0.0);
    assert_eq!(m.rate(1.0, 0.0, 958.0, 0.6, 350.0, 373.15).unwrap(), 0.0);
    let r1 = m.rate(0.6, 0.4, 958.0, 0.6, 383.15, 373.15).unwrap();
    let r2 = m.rate(0.6, 0.4, 958.0, 0.6, 393.15, 373.15).unwrap();
    assert!(rel(r2, 2.0 * r1) <= 1e-14);
}

#[test]
fn m2_conservation_all_models() {
    let lee = LeeModel {
        c_evap: 0.1,
        c_cond: 0.3,
    };
    let hks = HertzKnudsen {
        sigma: 0.7,
        molar_mass: 0.018015,
        schrage: true,
    };
    let zgb = PressureDriven::default();
    let mut rng = Lcg(7);
    for _ in 0..2000 {
        let h_fg = 2.0e6 + 5.0e5 * rng.next();
        let a_l = rng.next();
        let a_v = 1.0 - a_l;
        let t = 330.0 + 80.0 * rng.next();
        let t_sat = 373.0;
        let s = lee.source(a_l, a_v, 958.0, 0.6, t, t_sat, h_fg).unwrap();
        assert_eq!(s.liquid + s.vapour, 0.0);
        assert_eq!(s.energy, -s.rate() * h_fg);
        assert_eq!(s.rate() > 0.0, t > t_sat);

        let p_v = 5.0e4 + 1.0e5 * rng.next();
        let psat = 1.0e5;
        let s = hks.source(psat, t, p_v, t, 50.0 + 100.0 * rng.next(), h_fg).unwrap();
        assert_eq!(s.liquid + s.vapour, 0.0);
        assert_eq!(s.energy, -s.rate() * h_fg);
        assert_eq!(s.rate() > 0.0, p_v < psat);

        let p = 1.0e3 + 4.0e3 * rng.next();
        let s = zgb.source(a_v, 998.0, 0.023, p, 2339.0, h_fg).unwrap();
        assert_eq!(s.liquid + s.vapour, 0.0);
        assert_eq!(s.energy, -s.rate() * h_fg);
        assert_eq!(s.rate() > 0.0, p < 2339.0);
    }
}

#[test]
fn m3_hertz_knudsen_schrage() {
    let (mm, tl, ps, pv, ai) = (0.018015, 373.15, 101325.0, 95000.0, 100.0);
    let pre = (mm / (2.0 * std::f64::consts::PI * 8.314462618)).sqrt();
    let s = HertzKnudsen {
        sigma: 0.8,
        molar_mass: mm,
        schrage: true,
    };
    let hand = (2.0 * 0.8 / (2.0 - 0.8)) * pre * (101325.0 - 95000.0) / 373.15f64.sqrt() * 100.0;
    assert!(rel(s.rate(ps, tl, pv, tl, ai).unwrap(), hand) <= 1e-14);
    let p = HertzKnudsen { schrage: false, ..s };
    assert!(
        rel(
            p.rate(ps, tl, pv, tl, ai).unwrap(),
            0.8 * pre * (101325.0 - 95000.0) / 373.15f64.sqrt() * 100.0
        ) <= 1e-14
    );
    assert_eq!(s.flux(ps, tl, ps, tl).unwrap(), 0.0);
    assert!(s.flux(ps, tl, 1.1 * ps, tl).unwrap() < 0.0);
    // maximum one-way flux equals the kinetic-theory expression 0.25 n m vbar
    let one = HertzKnudsen {
        sigma: 1.0,
        molar_mass: mm,
        schrage: false,
    };
    let j = one.flux(ps, tl, 0.0, tl).unwrap();
    let kinetic = 0.25 * (ps * mm / (8.314462618 * tl)) * (8.0 * 8.314462618 * tl / (std::f64::consts::PI * mm)).sqrt();
    assert!(rel(j, kinetic) <= 1e-13, "{j} vs {kinetic}");
    // temperature jump with the IF97 saturation pressure: hotter liquid evaporates into saturated vapour at the cooler temperature
    let (t_v, t_l) = (373.15, 374.15);
    let p_v = saturation_pressure(t_v).unwrap();
    let j = s.flux(saturation_pressure(t_l).unwrap(), t_l, p_v, t_v).unwrap();
    assert!(j > 0.0);
    let j0 = s.flux(saturation_pressure(t_v).unwrap(), t_v, p_v, t_v).unwrap();
    assert_eq!(j0, 0.0);
}

#[test]
fn m4_pressure_driven() {
    let z = PressureDriven {
        r_bubble: 1e-6,
        alpha_nuc: 5e-4,
        f_vap: 50.0,
        f_cond: 0.01,
    };
    let (rl, rv, pv) = (998.0, 0.023, 2339.0);
    let vap = z.rate(0.1, rl, rv, 0.9 * pv, pv).unwrap();
    let hand = 50.0 * 3.0 * 5e-4 * (1.0 - 0.1) * 0.023 / 1e-6 * (2.0 / 3.0 * (pv - 0.9 * pv) / 998.0f64).sqrt();
    assert!(rel(vap, hand) <= 1e-14, "{vap} {hand}");
    let con = z.rate(0.1, rl, rv, 1.2 * pv, pv).unwrap();
    let hand = -0.01 * 3.0 * 0.1 * 0.023 / 1e-6 * (2.0 / 3.0 * (1.2 * pv - pv) / 998.0f64).sqrt();
    assert!(rel(con, hand) <= 1e-14, "{con} {hand}");
    assert_eq!(z.rate(0.1, rl, rv, pv, pv).unwrap(), 0.0);
    let a = z.rate(0.1, rl, rv, pv - 100.0, pv).unwrap();
    let b = z.rate(0.1, rl, rv, pv - 400.0, pv).unwrap();
    assert!(rel(b, 2.0 * a) <= 1e-14);
    // bubble dynamics reading
    let alpha = 0.1;
    let n = 3.0 * alpha / (4.0 * std::f64::consts::PI * 1e-18);
    let dr = PressureDriven::wall_speed(pv, 1.2 * pv, rl);
    let bubble = -rv * n * 4.0 * std::f64::consts::PI * 1e-12 * dr * 0.01;
    assert!(rel(con, bubble) <= 1e-13, "{con} {bubble}");
    assert_eq!(z.rate(1.0, rl, rv, 0.5 * pv, pv).unwrap(), 0.0);
    assert_eq!(z.rate(0.0, rl, rv, 1.5 * pv, pv).unwrap(), 0.0);
}

#[test]
fn m5_raoult() {
    let x = [0.3, 0.7];
    let ps = [1.0e5, 4.0e4];
    let pb = bubble_pressure(&x, &ps).unwrap();
    assert!(rel(pb, 5.8e4) <= 1e-15, "{pb}");
    let y = raoult_vapour_fractions(&x, &ps, pb).unwrap();
    assert!(rel(y[0], 0.3 * 1e5 / 5.8e4) <= 1e-15 && rel(y[1], 0.7 * 4e4 / 5.8e4) <= 1e-15);
    assert!((y.iter().sum::<f64>() - 1.0).abs() <= 1e-15);
    assert!(raoult_vapour_fractions(&x, &ps, 0.9 * pb).unwrap().iter().sum::<f64>() > 1.0);
    assert!(raoult_vapour_fractions(&x, &ps, 1.1 * pb).unwrap().iter().sum::<f64>() < 1.0);
    let xs = [[0.2, 0.3, 0.5], [0.8, 0.1, 0.1], [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]];
    let ps3 = [2.0e5, 7.0e4, 1.2e4];
    for x in xs {
        let pb = bubble_pressure(&x, &ps3).unwrap();
        let s: f64 = raoult_vapour_fractions(&x, &ps3, pb).unwrap().iter().sum();
        assert!((s - 1.0).abs() <= 1e-14, "{s}");
    }
    let y = raoult_vapour_fractions(&[1.0, 0.0], &ps, 2.0e5).unwrap();
    assert_eq!(y[0], 1.0e5 / 2.0e5);
    // dew point round trip
    let yv = [0.6, 0.4];
    let pd = dew_pressure(&yv, &ps).unwrap();
    let xd: Vec<f64> = yv.iter().zip(&ps).map(|(y, p)| y * pd / p).collect();
    assert!((xd.iter().sum::<f64>() - 1.0).abs() <= 1e-14);
    assert!((bubble_pressure(&xd, &ps).unwrap() - pd).abs() <= 1e-14 * pd);
    assert!(raoult_vapour_fractions(&[0.5, -0.1, 0.6], &ps3, 1e5)
        .unwrap_err()
        .contains("x[1]"));
    assert!(bubble_pressure(&[0.5, 0.4], &ps).unwrap_err().contains("sum"));
    assert!(bubble_pressure(&x, &[0.0, 1.0]).unwrap_err().contains("psat[0]"));
    let e = bubble_pressure(&x, &ps3).unwrap_err();
    assert!(e.contains('2') && e.contains('3'), "{e}");
    assert!(raoult_vapour_fractions(&x, &ps, 0.0).unwrap_err().contains('p'));
}

#[test]
fn m6_henry_and_transfer() {
    let h = Henry {
        h0: 4.3e9,
        t0: 298.15,
        c: 1700.0,
    };
    assert_eq!(h.at(298.15).unwrap(), 4.3e9);
    let (u1, u2) = (1.0 / 330.0 - 5e-7, 1.0 / 330.0 + 5e-7);
    let slope = ((h.at(1.0 / u1).unwrap()).ln() - (h.at(1.0 / u2).unwrap()).ln()) / (u1 - u2);
    assert!(rel(slope, -1700.0) <= 1e-6, "{slope}");
    let mut last = 0.0;
    for i in 0..100 {
        let v = h.at(280.0 + i as f64).unwrap();
        assert!(v > last);
        last = v;
    }
    let flat = Henry { c: 0.0, ..h };
    for t in [250.0, 300.0, 400.0] {
        assert_eq!(flat.at(t).unwrap(), 4.3e9);
    }
    let x = h.liquid_fraction(0.21, 1.0e5, 298.15).unwrap();
    assert!(rel(x, 0.21 * 1.0e5 / 4.3e9) <= 1e-14);
    assert!(rel(h.gas_fraction(x, 1.0e5, 298.15).unwrap(), 0.21) <= 1e-14);
    // ideal solution: Henry with H = psat is Raoult
    let ps = 7.0e4;
    let hi = Henry {
        h0: ps,
        t0: 350.0,
        c: 0.0,
    };
    let y_h = hi.gas_fraction(0.4, 1.0e5, 350.0).unwrap();
    let y_r = raoult_vapour_fractions(&[0.4, 0.6], &[ps, 5.0e4], 1.0e5).unwrap()[0];
    assert!((y_h - y_r).abs() <= 1e-15);
    // mass fractions
    let mm = [0.018, 0.044, 0.028];
    let xm = [0.2, 0.5, 0.3];
    let w = mole_to_mass_fractions(&xm, &mm).unwrap();
    assert!((w.iter().sum::<f64>() - 1.0).abs() <= 1e-14);
    let back = mass_to_mole_fractions(&w, &mm).unwrap();
    for i in 0..3 {
        assert!((back[i] - xm[i]).abs() <= 1e-14);
    }
    // transfer
    assert_eq!(species_transfer_rate(1e-3, 100.0, 1.2, 0.05, 0.05).unwrap(), 0.0);
    let r = species_transfer_rate(1e-3, 100.0, 1.2, 0.08, 0.05).unwrap();
    assert!(r > 0.0 && rel(r, 1e-3 * 100.0 * 1.2 * 0.03) <= 1e-12);
    assert!(species_transfer_rate(1e-3, 100.0, 1.2, 0.02, 0.05).unwrap() < 0.0);
}

fn water_film() -> NusseltFilm {
    NusseltFilm {
        rho_l: 958.4,
        rho_v: 0.5977,
        mu_l: 2.82e-4,
        k_l: 0.679,
        h_fg: 2.257e6,
        g: 9.81,
        rohsenow: false,
        cp_l: 4216.0,
    }
}

#[test]
fn m7_nusselt_film() {
    let f = water_film();
    let (ts, tw) = (373.15, 363.15);
    let delta = f.thickness(0.5, ts, tw).unwrap();
    let hand = (4.0 * 2.82e-4 * 0.679 * 10.0 * 0.5 / (9.81 * 958.4 * (958.4 - 0.5977) * 2.257e6f64)).powf(0.25);
    assert!(rel(delta, hand) <= 1e-14, "{delta} {hand}");
    let d16 = f.thickness(0.5, ts, ts - 160.0).unwrap();
    assert!(rel(d16, 2.0 * delta) <= 1e-14);
    let flux = f.wall_mass_flux(0.5, ts, tw).unwrap();
    assert!(rel(flux * 2.257e6, 0.679 * 10.0 / delta) <= 1e-14);
    // Gamma(x) = int_0^x m'' dx' with x = u^4 (integrand 4 u^3 m''(u^4) is a cubic in u^... smooth), Simpson
    let x_end: f64 = 0.5;
    let u_end = x_end.powf(0.25);
    let n = 2000;
    let hh = u_end / n as f64;
    let g = |u: f64| {
        if u == 0.0 {
            0.0
        } else {
            4.0 * u * u * u * f.wall_mass_flux(u.powi(4), ts, tw).unwrap()
        }
    };
    let mut s = g(0.0) + g(u_end);
    for i in 1..n {
        s += g(i as f64 * hh) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    let integral = s * hh / 3.0;
    let gamma = f.film_mass_flow(x_end, ts, tw).unwrap();
    assert!(rel(integral, gamma) <= 1e-12, "{integral} {gamma}");
    let hl = f.mean_h(0.5, ts, tw).unwrap();
    assert!(rel(hl, 4.0 / 3.0 * f.local_h(0.5, ts, tw).unwrap()) <= 1e-14);
    let coef = 4.0 / 3.0 * 0.25f64.powf(0.25);
    assert!((coef - 0.943).abs() / 0.943 <= 3e-4, "{coef}");
    let closed =
        coef * (9.81 * 958.4 * (958.4 - 0.5977) * 0.679f64.powi(3) * 2.257e6 / (2.82e-4 * 10.0 * 0.5)).powf(0.25);
    assert!(rel(hl, closed) <= 1e-12, "{hl} {closed}");
    println!(
        "m7: delta(0.5) = {delta:e} m, h_L = {hl:.1} W/m2K, Re_film = {:.2}",
        f.film_reynolds(0.5, ts, tw).unwrap()
    );
    assert_eq!(f.wall_mass_flux(0.5, ts, ts).unwrap(), 0.0);
    assert_eq!(f.wall_mass_flux(0.5, ts, ts + 5.0).unwrap(), 0.0);
    assert!(f.thickness(0.5, ts, ts + 1.0).is_err());
    let s = wall_cell_source(flux, 0.01, 1e-4, 2.257e6).unwrap();
    assert_eq!(s.rate(), -(flux * 0.01 / 1e-4));
    assert!(s.liquid > 0.0 && s.vapour < 0.0 && s.liquid + s.vapour == 0.0);
    assert_eq!(s.energy, -s.rate() * 2.257e6);
    // Rohsenow correction raises the effective latent heat and lowers the thickness
    let fr = NusseltFilm { rohsenow: true, ..f };
    assert!(fr.thickness(0.5, ts, tw).unwrap() < delta);
}

#[test]
fn m8_relaxation_cell() {
    let p = LeeCellParams {
        c: 0.1,
        t_sat: 373.15,
        heat_capacity: 2.0e6,
        h_fg: 2.257e6,
        frozen: true,
    };
    let (t0, ml) = (400.0, 500.0);
    let lambda = p.lambda(ml);
    let t_end = 3.0 / lambda;
    let n = 200;
    let s = p
        .integrate(
            LeeCell {
                m_liquid: ml,
                m_vapour: 0.0,
                t: t0,
            },
            t_end / n as f64,
            n,
        )
        .unwrap();
    let exact = p.analytic_frozen(t0, ml, t_end);
    let err = (s.t - exact).abs() / (t0 - p.t_sat);
    println!("m8 frozen: lambda = {lambda:e}, max err = {err:e}");
    assert!(err <= 1e-9, "{err}");
    // depleting: invariants and monotone relaxation
    let q = LeeCellParams { frozen: false, ..p };
    let mut st = LeeCell {
        m_liquid: ml,
        m_vapour: 1.0,
        t: t0,
    };
    let inv0 = q.heat_capacity * st.t + q.h_fg * st.m_vapour;
    let mass0 = st.m_liquid + st.m_vapour;
    let dt = 0.1 / lambda;
    let mut last = st.t - q.t_sat;
    for _ in 0..200 {
        st = q.integrate(st, dt, 1).unwrap();
        let th = st.t - q.t_sat;
        assert!(th >= 0.0 && th <= last + 1e-12, "{th} {last}");
        last = th;
        assert!(st.m_liquid > 0.0);
        let inv = q.heat_capacity * st.t + q.h_fg * st.m_vapour;
        assert!((inv - inv0).abs() <= 1e-12 * inv0.abs());
        assert!((st.m_liquid + st.m_vapour - mass0).abs() <= 1e-12 * mass0);
    }
    assert!(last < 0.3 * (t0 - q.t_sat));
    // condensing side
    let mut sc = LeeCell {
        m_liquid: 1.0,
        m_vapour: 50.0,
        t: 350.0,
    };
    let qc = LeeCellParams { ..q };
    let mut prev = qc.t_sat - sc.t;
    for _ in 0..100 {
        sc = qc.integrate(sc, 0.05, 1).unwrap();
        let d = qc.t_sat - sc.t;
        assert!(d >= 0.0 && d <= prev + 1e-12, "{d} {prev}");
        prev = d;
    }
    assert!(sc.m_liquid > 1.0 && sc.m_vapour < 50.0);
    assert!(qc
        .integrate(
            LeeCell {
                m_liquid: -1.0,
                m_vapour: 0.0,
                t: 300.0
            },
            0.1,
            1
        )
        .unwrap_err()
        .contains("m_liquid"));
}

#[test]
fn m9_located_input_errors() {
    let lee = LeeModel {
        c_evap: 0.1,
        c_cond: 0.1,
    };
    assert!(lee
        .rate(1.5, 0.0, 958.0, 0.6, 380.0, 373.0)
        .unwrap_err()
        .contains("alpha_l"));
    assert!(lee
        .rate(0.5, 0.5, 0.0, 0.6, 380.0, 373.0)
        .unwrap_err()
        .contains("rho_l"));
    assert!(lee
        .rate(0.5, 0.5, 958.0, 0.6, 380.0, 0.0)
        .unwrap_err()
        .contains("t_sat"));
    assert!(lee
        .rate(0.5, 0.5, 958.0, 0.6, f64::NAN, 373.0)
        .unwrap_err()
        .contains('t'));
    let neg = LeeModel {
        c_evap: -1.0,
        c_cond: 0.1,
    };
    assert!(neg
        .rate(0.5, 0.5, 958.0, 0.6, 380.0, 373.0)
        .unwrap_err()
        .contains("c_evap"));
    for sigma in [0.0, 1.5, f64::NAN, -0.1] {
        let h = HertzKnudsen {
            sigma,
            molar_mass: 0.018,
            schrage: true,
        };
        assert!(h.flux(1e5, 373.0, 1e5, 373.0).unwrap_err().contains("sigma"));
    }
    let h = HertzKnudsen {
        sigma: 1.0,
        molar_mass: 0.018,
        schrage: false,
    };
    assert!(h.flux(1e5, 373.0, -1.0, 373.0).unwrap_err().contains("p_v"));
    assert!(h.source(1e5, 373.0, 9e4, 373.0, 1.0, 0.0).unwrap_err().contains("h_fg"));
    assert!(PressureDriven::default()
        .rate(0.1, 998.0, 0.02, f64::NAN, 2339.0)
        .unwrap_err()
        .contains('p'));
    assert!(Henry {
        h0: 0.0,
        t0: 298.0,
        c: 0.0
    }
    .at(300.0)
    .unwrap_err()
    .contains("h0"));
}
