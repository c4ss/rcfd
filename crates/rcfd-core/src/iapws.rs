//! Water and steam from the IAPWS Industrial Formulation 1997 (IAPWS-IF97), with the IAPWS viscosity and
//! thermal-conductivity correlations (F-111).
//!
//! Sources (public IAPWS releases, no third-party licensed data):
//!
//! * IAPWS R7-97(2012), "Revised Release on the IAPWS Industrial Formulation 1997 for the Thermodynamic
//!   Properties of Water and Steam": region 1 (compressed liquid, Eq. 7, Table 2), region 2 (vapour, Eqs. 15-17,
//!   Tables 10 and 11), the B23 boundary (Eq. 5, Table 1), the saturation-pressure equation (Eq. 30, Table 34) and the
//!   property relations of Tables 3 and 12. Coefficients were transcribed from the release and are checked by the
//!   release's own computer-program verification values (Tables 5, 15 and 35), see the tests.
//! * IAPWS R12-08, "Release on the IAPWS Formulation 2008 for the Viscosity of Ordinary Water Substance"
//!   (Eqs. 10-12, Tables 1, 2, with the critical enhancement set to 1, the industrial form, and the density from
//!   IF97).
//! * IAPWS R15-11, "Release on the IAPWS Formulation 2011 for the Thermal Conductivity of Ordinary Water
//!   Substance" (Eqs. 15-17, Tables 1 and 2, without the critical enhancement).
//!
//! Not implemented: region 3 (near-critical, 623.15 K and above at high pressure), region 5 (above 1073.15 K),
//! the backward equations T(p,h), T(p,s), and the critical enhancement of the transport properties. A state in
//! an unsupported region returns an error; the viscosity and conductivity are not meant for the near-critical
//! window (T within a few kelvin of 647.096 K, density 250-400 kg/m^3).

use crate::error::{Error, Result};

/// Specific gas constant of water, J/(kg K) (R7-97 Eq. 1).
pub const R_WATER: f64 = 461.526;
/// Critical temperature (K).
pub const T_CRITICAL: f64 = 647.096;
/// Critical density (kg/m^3).
pub const RHO_CRITICAL: f64 = 322.0;

fn err<T>(reason: impl Into<String>) -> Result<T> {
    Err(Error::InvalidValue {
        path: "water state".into(),
        reason: reason.into(),
    })
}

/// Thermodynamic state of water from IF97.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterState {
    /// IF97 region (1 liquid, 2 vapour).
    pub region: u8,
    /// Density (kg/m^3).
    pub density: f64,
    /// Specific enthalpy (J/kg).
    pub h: f64,
    /// Specific internal energy (J/kg).
    pub u: f64,
    /// Specific entropy (J/(kg K)).
    pub s: f64,
    /// Isobaric specific heat (J/(kg K)).
    pub cp: f64,
    /// Speed of sound (m/s).
    pub w: f64,
}

// ---- region 1 (Table 2) -------------------------------------------------------------------------------------

const R1_I: [i32; 34] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 8, 8, 21, 23, 29, 30, 31, 32,
];
const R1_J: [i32; 34] = [
    -2, -1, 0, 1, 2, 3, 4, 5, -9, -7, -1, 0, 1, 3, -3, 0, 1, 3, 17, -4, 0, 6, -5, -2, 10, -8, -11, -6, -29, -31, -38,
    -39, -40, -41,
];
const R1_N: [f64; 34] = [
    0.14632971213167,
    -0.84548187169114,
    -0.37563603672040e1,
    0.33855169168385e1,
    -0.95791963387872,
    0.15772038513228,
    -0.16616417199501e-1,
    0.81214629983568e-3,
    0.28319080123804e-3,
    -0.60706301565874e-3,
    -0.18990068218419e-1,
    -0.32529748770505e-1,
    -0.21841717175414e-1,
    -0.52838357969930e-4,
    -0.47184321073267e-3,
    -0.30001780793026e-3,
    0.47661393906987e-4,
    -0.44141845330846e-5,
    -0.72694996297594e-15,
    -0.31679644845054e-4,
    -0.28270797985312e-5,
    -0.85205128120103e-9,
    -0.22425281908000e-5,
    -0.65171222895601e-6,
    -0.14341729937924e-12,
    -0.40516996860117e-6,
    -0.12734301741641e-8,
    -0.17424871230634e-9,
    -0.68762131295531e-18,
    0.14478307828521e-19,
    0.26335781662795e-22,
    -0.11947622640071e-22,
    0.18228094581404e-23,
    -0.93537087292458e-25,
];

/// Dimensionless Gibbs energy of region 1 and its derivatives `[g, g_pi, g_pipi, g_tau, g_tautau, g_pitau]`.
fn gibbs1(pi: f64, tau: f64) -> [f64; 6] {
    let (a, b) = (7.1 - pi, tau - 1.222);
    let mut g = [0.0; 6];
    for k in 0..34 {
        let (i, j, n) = (R1_I[k], R1_J[k], R1_N[k]);
        let (fi, fj) = (i as f64, j as f64);
        g[0] += n * a.powi(i) * b.powi(j);
        g[1] -= n * fi * a.powi(i - 1) * b.powi(j);
        g[2] += n * fi * (fi - 1.0) * a.powi(i - 2) * b.powi(j);
        g[3] += n * a.powi(i) * fj * b.powi(j - 1);
        g[4] += n * a.powi(i) * fj * (fj - 1.0) * b.powi(j - 2);
        g[5] -= n * fi * a.powi(i - 1) * fj * b.powi(j - 1);
    }
    g
}

fn region1(t: f64, p: f64) -> WaterState {
    let (pi, tau) = (p / 16.53e6, 1386.0 / t);
    let g = gibbs1(pi, tau);
    let rt = R_WATER * t;
    let v = pi * g[1] * rt / p;
    let w2 = rt * g[1] * g[1] / ((g[1] - tau * g[5]).powi(2) / (tau * tau * g[4]) - g[2]);
    WaterState {
        region: 1,
        density: 1.0 / v,
        h: tau * g[3] * rt,
        u: (tau * g[3] - pi * g[1]) * rt,
        s: (tau * g[3] - g[0]) * R_WATER,
        cp: -tau * tau * g[4] * R_WATER,
        w: w2.sqrt(),
    }
}

// ---- region 2 (Tables 10 and 11) ----------------------------------------------------------------------------

const R2_J0: [i32; 9] = [0, 1, -5, -4, -3, -2, -1, 2, 3];
const R2_N0: [f64; 9] = [
    -0.96927686500217e1,
    0.10086655968018e2,
    -0.56087911283020e-2,
    0.71452738081455e-1,
    -0.40710498223928,
    0.14240819171444e1,
    -0.43839511319450e1,
    -0.28408632460772,
    0.21268463753307e-1,
];
const R2_I: [i32; 43] = [
    1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 4, 4, 4, 5, 6, 6, 6, 7, 7, 7, 8, 8, 9, 10, 10, 10, 16, 16, 18, 20, 20,
    20, 21, 22, 23, 24, 24, 24,
];
const R2_J: [i32; 43] = [
    0, 1, 2, 3, 6, 1, 2, 4, 7, 36, 0, 1, 3, 6, 35, 1, 2, 3, 7, 3, 16, 35, 0, 11, 25, 8, 36, 13, 4, 10, 14, 29, 50, 57,
    20, 35, 48, 21, 53, 39, 26, 40, 58,
];
const R2_N: [f64; 43] = [
    -0.17731742473213e-2,
    -0.17834862292358e-1,
    -0.45996013696365e-1,
    -0.57581259083432e-1,
    -0.50325278727930e-1,
    -0.33032641670203e-4,
    -0.18948987516315e-3,
    -0.39392777243355e-2,
    -0.43797295650573e-1,
    -0.26674547914087e-4,
    0.20481737692309e-7,
    0.43870667284435e-6,
    -0.32277677238570e-4,
    -0.15033924542148e-2,
    -0.40668253562649e-1,
    -0.78847309559367e-9,
    0.12790717852285e-7,
    0.48225372718507e-6,
    0.22922076337661e-5,
    -0.16714766451061e-10,
    -0.21171472321355e-2,
    -0.23895741934104e2,
    -0.59059564324270e-17,
    -0.12621808899101e-5,
    -0.38946842435739e-1,
    0.11256211360459e-10,
    -0.82311340897998e1,
    0.19809712802088e-7,
    0.10406965210174e-18,
    -0.10234747095929e-12,
    -0.10018179379511e-8,
    -0.80882908646985e-10,
    0.10693031879409,
    -0.33662250574171,
    0.89185845355421e-24,
    0.30629316876232e-12,
    -0.42002467698208e-5,
    -0.59056029685639e-25,
    0.37826947613457e-5,
    -0.12768608934681e-14,
    0.73087610595061e-28,
    0.55414715350778e-16,
    -0.94369707241210e-6,
];

/// Ideal-gas part `[g0, g0_pi, g0_pipi, g0_tau, g0_tautau]` and residual part `[gr, gr_pi, gr_pipi, gr_tau,
/// gr_tautau, gr_pitau]` of region 2.
fn gibbs2(pi: f64, tau: f64) -> ([f64; 5], [f64; 6]) {
    let mut g0 = [pi.ln(), 1.0 / pi, -1.0 / (pi * pi), 0.0, 0.0];
    for k in 0..9 {
        let (j, n) = (R2_J0[k] as f64, R2_N0[k]);
        g0[0] += n * tau.powi(R2_J0[k]);
        g0[3] += n * j * tau.powi(R2_J0[k] - 1);
        g0[4] += n * j * (j - 1.0) * tau.powi(R2_J0[k] - 2);
    }
    let b = tau - 0.5;
    let mut gr = [0.0; 6];
    for k in 0..43 {
        let (i, j, n) = (R2_I[k], R2_J[k], R2_N[k]);
        let (fi, fj) = (i as f64, j as f64);
        gr[0] += n * pi.powi(i) * b.powi(j);
        gr[1] += n * fi * pi.powi(i - 1) * b.powi(j);
        gr[2] += n * fi * (fi - 1.0) * pi.powi(i - 2) * b.powi(j);
        gr[3] += n * pi.powi(i) * fj * b.powi(j - 1);
        gr[4] += n * pi.powi(i) * fj * (fj - 1.0) * b.powi(j - 2);
        gr[5] += n * fi * pi.powi(i - 1) * fj * b.powi(j - 1);
    }
    (g0, gr)
}

fn region2(t: f64, p: f64) -> WaterState {
    let (pi, tau) = (p / 1.0e6, 540.0 / t);
    let (g0, gr) = gibbs2(pi, tau);
    let rt = R_WATER * t;
    let gp = g0[1] + gr[1];
    let gt = g0[3] + gr[3];
    let gtt = g0[4] + gr[4];
    let v = pi * gp * rt / p;
    let w2 = rt * (1.0 + 2.0 * pi * gr[1] + pi * pi * gr[1] * gr[1])
        / ((1.0 - pi * pi * gr[2]) + (1.0 + pi * gr[1] - tau * pi * gr[5]).powi(2) / (tau * tau * gtt));
    WaterState {
        region: 2,
        density: 1.0 / v,
        h: tau * gt * rt,
        u: (tau * gt - pi * gp) * rt,
        s: (tau * gt - (g0[0] + gr[0])) * R_WATER,
        cp: -tau * tau * gtt * R_WATER,
        w: w2.sqrt(),
    }
}

// ---- boundaries and saturation -------------------------------------------------------------------------------

const SAT_N: [f64; 10] = [
    0.11670521452767e4,
    -0.72421316703206e6,
    -0.17073846940092e2,
    0.12020824702470e5,
    -0.32325550322333e7,
    0.14915108613530e2,
    -0.48232657361591e4,
    0.40511340542057e6,
    -0.23855557567849,
    0.65017534844798e3,
];

/// Saturation pressure (Pa) from the IF97 Eq. 30, valid 273.15 K to the critical temperature.
pub fn saturation_pressure(t: f64) -> Result<f64> {
    if !(273.15..=T_CRITICAL).contains(&t) {
        return err(format!(
            "saturation pressure needs 273.15 K <= T <= {T_CRITICAL} K, got {t}"
        ));
    }
    let n = &SAT_N;
    let th = t + n[8] / (t - n[9]);
    let a = th * th + n[0] * th + n[1];
    let b = n[2] * th * th + n[3] * th + n[4];
    let c = n[5] * th * th + n[6] * th + n[7];
    let x = 2.0 * c / (-b + (b * b - 4.0 * a * c).sqrt());
    Ok(x.powi(4) * 1.0e6)
}

/// Pressure (Pa) of the B23 boundary between regions 2 and 3 at temperature `t` (R7-97 Eq. 5, 623.15-863.15 K).
pub fn boundary_b23_pressure(t: f64) -> f64 {
    (0.34805185628969e3 - 0.11671859879975e1 * t + 0.10192970039326e-2 * t * t) * 1.0e6
}

/// Which IF97 region contains the state, or an error when it is outside the implemented regions.
pub fn region(t: f64, p: f64) -> Result<u8> {
    if !(t.is_finite() && p.is_finite()) || p <= 0.0 {
        return err("temperature and pressure must be finite and positive");
    }
    if p > 100.0e6 {
        return err(format!("pressure {p} Pa above the IF97 limit of 100 MPa"));
    }
    if t < 273.15 {
        return err(format!("temperature {t} K below the IF97 limit of 273.15 K"));
    }
    if t <= 623.15 {
        return Ok(if p > saturation_pressure(t)? { 1 } else { 2 });
    }
    if t <= 863.15 {
        return if p <= boundary_b23_pressure(t) {
            Ok(2)
        } else {
            err("state in IF97 region 3 (near-critical dense fluid), which is not implemented")
        };
    }
    if t <= 1073.15 {
        return Ok(2);
    }
    err(format!(
        "temperature {t} K above 1073.15 K (IF97 region 5 is not implemented)"
    ))
}

/// Thermodynamic state at temperature `t` (K) and pressure `p` (Pa).
pub fn water_state(t: f64, p: f64) -> Result<WaterState> {
    Ok(match region(t, p)? {
        1 => region1(t, p),
        _ => region2(t, p),
    })
}

// ---- transport properties ------------------------------------------------------------------------------------

const MU_H: [f64; 4] = [1.67752, 2.20462, 0.6366564, -0.241605];
/// `(i, j, H_ij)` of R12-08 Table 2; all other coefficients are zero.
const MU_HIJ: [(i32, i32, f64); 21] = [
    (0, 0, 5.20094e-1),
    (1, 0, 8.50895e-2),
    (2, 0, -1.08374),
    (3, 0, -2.89555e-1),
    (0, 1, 2.22531e-1),
    (1, 1, 9.99115e-1),
    (2, 1, 1.88797),
    (3, 1, 1.26613),
    (5, 1, 1.20573e-1),
    (0, 2, -2.81378e-1),
    (1, 2, -9.06851e-1),
    (2, 2, -7.72479e-1),
    (3, 2, -4.89837e-1),
    (4, 2, -2.57040e-1),
    (0, 3, 1.61913e-1),
    (1, 3, 2.57399e-1),
    (0, 4, -3.25372e-2),
    (3, 4, 6.98452e-2),
    (4, 5, 8.72102e-3),
    (3, 6, -4.35673e-3),
    (5, 6, -5.93264e-4),
];

/// Dynamic viscosity (Pa s) at temperature `t` (K) and density `rho` (kg/m^3), IAPWS R12-08 with the critical
/// enhancement omitted (industrial form).
pub fn viscosity(t: f64, rho: f64) -> f64 {
    let (tb, rb) = (t / T_CRITICAL, rho / RHO_CRITICAL);
    let denom: f64 = MU_H.iter().enumerate().map(|(i, h)| h / tb.powi(i as i32)).sum();
    let mu0 = 100.0 * tb.sqrt() / denom;
    let s: f64 = MU_HIJ
        .iter()
        .map(|&(i, j, h)| (1.0 / tb - 1.0).powi(i) * h * (rb - 1.0).powi(j))
        .sum();
    mu0 * (rb * s).exp() * 1.0e-6
}

const LAM_L: [f64; 5] = [2.443221e-3, 1.323095e-2, 6.770357e-3, -3.454586e-3, 4.096266e-4];
const LAM_LIJ: [[f64; 6]; 5] = [
    [
        1.60397357,
        -0.646013523,
        0.111443906,
        0.102997357,
        -0.0504123634,
        0.00609859258,
    ],
    [
        2.33771842,
        -2.78843778,
        1.53616167,
        -0.463045512,
        0.0832827019,
        -0.00719201245,
    ],
    [
        2.19650529,
        -4.54580785,
        3.55777244,
        -1.40944978,
        0.275418278,
        -0.0205938816,
    ],
    [-1.21051378, 1.60812989, -0.621178141, 0.0716373224, 0.0, 0.0],
    [-2.7203370, 4.57586331, -3.18369245, 1.1168348, -0.19268305, 0.012913842],
];

/// Thermal conductivity (W/(m K)) at temperature `t` (K) and density `rho` (kg/m^3), IAPWS R15-11 without the
/// critical enhancement (accurate outside the window near the critical point).
pub fn conductivity(t: f64, rho: f64) -> f64 {
    let (tb, rb) = (t / T_CRITICAL, rho / RHO_CRITICAL);
    let denom: f64 = LAM_L.iter().enumerate().map(|(k, l)| l / tb.powi(k as i32)).sum();
    let l0 = tb.sqrt() / denom;
    let mut s = 0.0;
    for (i, row) in LAM_LIJ.iter().enumerate() {
        let inner: f64 = row.iter().enumerate().map(|(j, l)| l * (rb - 1.0).powi(j as i32)).sum();
        s += (1.0 / tb - 1.0).powi(i as i32) * inner;
    }
    1.0e-3 * l0 * (rb * s).exp()
}

/// Transport and thermodynamic properties of water at `(t, p)` bundled for a flow solver.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterProperties {
    /// IF97 state.
    pub state: WaterState,
    /// Dynamic viscosity (Pa s).
    pub viscosity: f64,
    /// Thermal conductivity (W/(m K)).
    pub conductivity: f64,
}

/// All properties at `(t, p)`.
pub fn water_properties(t: f64, p: f64) -> Result<WaterProperties> {
    let state = water_state(t, p)?;
    Ok(WaterProperties {
        state,
        viscosity: viscosity(t, state.density),
        conductivity: conductivity(t, state.density),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * b.abs()
    }

    // R7-97 Table 5 (region 1), Table 15 (region 2), Table 35 (saturation): the release's verification values.
    // Tolerance 1e-8 relative (the tables give nine digits).
    #[test]
    fn region_1_matches_the_release_verification_table() {
        let cases = [
            (
                300.0,
                3.0e6,
                0.100215168e-2,
                0.115331273e3,
                0.112324818e3,
                0.392294792,
                0.417301218e1,
                0.150773921e4,
            ),
            (
                300.0,
                80.0e6,
                0.971180894e-3,
                0.184142828e3,
                0.106448356e3,
                0.368563852,
                0.401008987e1,
                0.163469054e4,
            ),
            (
                500.0,
                3.0e6,
                0.120241800e-2,
                0.975542239e3,
                0.971934985e3,
                0.258041912e1,
                0.465580682e1,
                0.124071337e4,
            ),
        ];
        for (t, p, v, h, u, s, cp, w) in cases {
            let st = water_state(t, p).unwrap();
            assert_eq!(st.region, 1);
            assert!(close(1.0 / st.density, v, 1e-8), "v {} {v}", 1.0 / st.density);
            assert!(close(st.h / 1e3, h, 1e-8), "h {}", st.h);
            assert!(close(st.u / 1e3, u, 1e-8), "u {}", st.u);
            assert!(close(st.s / 1e3, s, 1e-8), "s {}", st.s);
            assert!(close(st.cp / 1e3, cp, 1e-8), "cp {}", st.cp);
            assert!(close(st.w, w, 1e-8), "w {}", st.w);
        }
    }

    #[test]
    fn region_2_matches_the_release_verification_table() {
        let cases = [
            (
                300.0,
                0.0035e6,
                0.394913866e2,
                0.254991145e4,
                0.241169160e4,
                0.852238967e1,
                0.191300162e1,
                0.427920172e3,
            ),
            (
                700.0,
                0.0035e6,
                0.923015898e2,
                0.333568375e4,
                0.301262819e4,
                0.101749996e2,
                0.208141274e1,
                0.644289068e3,
            ),
            (
                700.0,
                30.0e6,
                0.542946619e-2,
                0.263149474e4,
                0.246861076e4,
                0.517540298e1,
                0.103505092e2,
                0.480386523e3,
            ),
        ];
        for (t, p, v, h, u, s, cp, w) in cases {
            let st = water_state(t, p).unwrap();
            assert_eq!(st.region, 2);
            assert!(close(1.0 / st.density, v, 1e-8), "v {} {v}", 1.0 / st.density);
            assert!(close(st.h / 1e3, h, 1e-8), "h {}", st.h);
            assert!(close(st.u / 1e3, u, 1e-8), "u {}", st.u);
            assert!(close(st.s / 1e3, s, 1e-8), "s {}", st.s);
            assert!(close(st.cp / 1e3, cp, 1e-8), "cp {}", st.cp);
            assert!(close(st.w, w, 1e-8), "w {}", st.w);
        }
    }

    #[test]
    fn saturation_pressure_and_b23_match_the_release() {
        for (t, p) in [(300.0, 0.353658941e-2), (500.0, 0.263889776e1), (600.0, 0.123443146e2)] {
            let ps = saturation_pressure(t).unwrap() / 1e6;
            assert!(close(ps, p, 1e-8), "{t}: {ps} vs {p}");
        }
        // B23 (Eq. 5) passes through 623.15 K, 16.5291643 MPa (Table 2 of the release)
        assert!(close(boundary_b23_pressure(623.15) / 1e6, 0.165291643e2, 1e-8));
        // the release states that Eq. 30 reproduces the normal boiling point: 101.325 kPa near 373.124 K
        assert!(close(saturation_pressure(373.124).unwrap(), 101325.0, 1e-4));
    }

    // R12-08 Table 4 (critical enhancement 1) and R15-11 Table 4 (lambda_2 = 0); the tables carry 7 to 9 digits, so the tolerance
    // is 1e-7 relative (rounding of the last printed digit).
    #[test]
    fn transport_correlations_match_the_release_verification_tables() {
        let mu = [
            (298.15, 998.0, 889.735100),
            (298.15, 1200.0, 1437.649467),
            (373.15, 1000.0, 307.883622),
            (433.15, 1.0, 14.538324),
            (433.15, 1000.0, 217.685358),
            (873.15, 1.0, 32.619287),
            (873.15, 100.0, 35.802262),
            (873.15, 600.0, 77.430195),
            (1173.15, 1.0, 44.217245),
            (1173.15, 100.0, 47.640433),
            (1173.15, 400.0, 64.154608),
        ];
        for (t, rho, v) in mu {
            let got = viscosity(t, rho) * 1e6;
            assert!(close(got, v, 1e-7), "mu({t},{rho}) = {got} vs {v}");
        }
        let lam = [
            (298.15, 0.0, 18.4341883),
            (298.15, 998.0, 607.712868),
            (298.15, 1200.0, 799.038144),
            (873.15, 0.0, 79.1034659),
        ];
        for (t, rho, v) in lam {
            let got = conductivity(t, rho) * 1e3;
            assert!(close(got, v, 1e-7), "lambda({t},{rho}) = {got} vs {v}");
        }
    }

    #[test]
    fn unsupported_states_are_rejected() {
        assert!(water_state(650.0, 40.0e6).is_err()); // region 3
        assert!(water_state(1200.0, 1.0e5).is_err()); // region 5
        assert!(water_state(250.0, 1.0e5).is_err());
        assert!(water_state(300.0, 2.0e8).is_err());
    }
}
