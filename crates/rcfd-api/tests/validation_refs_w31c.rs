//! Reference-reading checks of wave 31C (ADR 0064).
//!
//! Two kinds of checks, none of which runs a solver:
//!  1. the in-repo reference tables equal the tables read from public copies (the `w31c_*` CSV files of
//!     `validation-data/reference`), to the printed precision;
//!  2. cheap analytic checks of correlations against the numbers printed in the sources that were read
//!     (Williamson 1989 fit, Dean's channel relation, Colebrook-White equivalent forms, Graetz eigenvalues of the
//!     circular tube, the Gartling Re = 800 step lengths, NACA 0012 force data).

mod validation_common;

use validation_common::*;

fn w31c(name: &str) -> Vec<Vec<f64>> {
    read_numeric(&reference_dir().join(name))
}

/// First `n` columns of a CSV that carries a free-text note column, parsed as numbers (a note may hold commas).
fn w31c_cols(name: &str, n: usize) -> Vec<Vec<f64>> {
    read_rows(&reference_dir().join(name))
        .iter()
        .map(|r| r[..n].iter().map(|v| v.parse().unwrap()).collect())
        .collect()
}

fn lookup(rows: &[Vec<f64>], key: f64) -> &Vec<f64> {
    rows.iter()
        .find(|r| (r[0] - key).abs() < 1e-9)
        .unwrap_or_else(|| panic!("no row {key}"))
}

// ---------------------------------------------------------------------------------------------------------------
// 1. In-repo tables against the read tables
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn ghia_u_table_equals_read_table_for_all_three_re() {
    let ours = read_numeric(&cavity_dir().join("ghia1982_u_centerline.csv"));
    let read = w31c("w31c_ghia1982_u_centerline_read.csv");
    assert_eq!(ours.len(), read.len(), "row count");
    for a in &ours {
        let b = lookup(&read, a[0]);
        for col in 1..4 {
            assert!(
                (a[col] - b[col]).abs() < 1e-12,
                "u Re col {col} at y {}: in-repo {} read {}",
                a[0],
                a[col],
                b[col]
            );
        }
    }
}

#[test]
fn ghia_v_re100_and_re1000_u_botella_table_equal_read_tables() {
    let ours = read_numeric(&cavity_dir().join("ghia1982_v_centerline_Re100.csv"));
    let read = w31c("w31c_ghia1982_v_centerline_read.csv");
    assert_eq!(ours.len(), read.len(), "row count");
    for r in &ours {
        let b = lookup(&read, r[0]);
        assert!((r[1] - b[1]).abs() < 1e-12, "v Re 100 at x {}", r[0]);
    }
    let u = w31c("w31c_ghia1982_u_centerline_read.csv");
    let c34 = w31c("cavity_re1000_ghia_botella_u.csv");
    for r in &c34 {
        let b = lookup(&u, r[0]);
        assert!((r[1] - b[3]).abs() < 1e-12, "Ghia Re 1000 column at y {}", r[0]);
    }
}

#[test]
fn ghia_u_and_v_tables_carry_the_documented_boundary_and_misprint_values() {
    let v = w31c("w31c_ghia1982_v_centerline_read.csv");
    // the known misprint of the Re 400 v table is stored as printed (it is flagged in ADR 0064)
    assert_eq!(lookup(&v, 0.9063)[2], -0.23827);
    // the cells on which the secondary copies disagree hold the value of two of the three copies
    assert_eq!(lookup(&v, 0.5)[2], 0.05186);
    assert_eq!(lookup(&v, 0.9063)[3], -0.5155);
    for r in &v {
        if r[0] == 0.0 || r[0] == 1.0 {
            assert!(r[1..].iter().all(|&x| x == 0.0), "wall v must be zero");
        }
    }
}

#[test]
fn cylinder_literature_table_carries_the_williamson_experiment_value() {
    // wave 30B's CSV lists "Williamson (experiment)" with St = 0.165 (Williamson 1996 review, read in arXiv:2303.09262);
    // the 1989 fit read here gives 0.1643 at Re = 100.
    let fit: Vec<f64> = read_rows(&reference_dir().join("w31c_cylinder_williamson1989_st_re_fit.csv"))
        .iter()
        .map(|r| r[1].parse().unwrap())
        .collect();
    let (a, b, c) = (fit[0], fit[1], fit[2]);
    assert_eq!((a, b, c), (-3.3265, 0.1816, 1.6e-4));
    let st = |re: f64| a / re + b + c * re;
    let st100 = st(100.0);
    assert!((st100 - 0.164335).abs() < 1e-9, "St(100) = {st100}");
    assert!(
        (st100 - 0.165).abs() / 0.165 < 0.005,
        "differs from 0.165 by {:.2} %",
        100.0 * (st100 - 0.165) / 0.165
    );
    // fit range of the study: Re 49 to 178, monotone increasing there
    assert!(st(49.0) < st(100.0) && st(100.0) < st(178.0));
}

// ---------------------------------------------------------------------------------------------------------------
// 2. Backward-facing step: Gartling Re = 800 (normalisation check)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn gartling_re800_lengths_in_step_heights_match_the_value_used_in_the_repo() {
    let rows = read_rows(&reference_dir().join("w31c_bfs_gartling1990_re800_mfix.csv"));
    let step_height = 0.5; // inlet profile 24 y (0.5 - y), channel height 1
    for r in &rows {
        let (h_ch, h_step): (f64, f64) = (r[1].parse().unwrap(), r[2].parse().unwrap());
        assert!(
            (h_ch / step_height - h_step).abs() < 0.011,
            "{}: {h_ch}/0.5 = {} vs printed {h_step}",
            r[0],
            h_ch / step_height
        );
    }
    // the lower-wall reattachment length is the 12.20 step heights quoted in arXiv:2507.16509 Table 2
    let l1: f64 = rows[0][2].parse().unwrap();
    assert_eq!(l1, 12.20);
    // upper bubble length is reattachment minus separation
    let (xs, xr, lu): (f64, f64, f64) = (
        rows[1][1].parse().unwrap(),
        rows[2][1].parse().unwrap(),
        rows[3][1].parse().unwrap(),
    );
    assert!((xr - xs - lu).abs() < 0.011);
    // Re_D = V 2h / nu with V the mean inlet velocity of the inlet channel of height h equals Re_H = V H / nu
    // with H = 2h for the expansion ratio 2 (Armaly's definition D = 2 h_inlet is read in the paper)
    let (v, nu, h_inlet): (f64, f64, f64) = (1.0, 1.0 / 800.0, 0.5);
    assert!(((v * 2.0 * h_inlet / nu) - 800.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------------------------------------------------
// 3. Pipe and channel friction
// ---------------------------------------------------------------------------------------------------------------

/// Colebrook-White, 1/sqrt(f) = -2 log10( eps/(3.7 D) + 2.51/(Re sqrt f) ), fixed-point iteration.
fn colebrook(re: f64, rel_rough: f64) -> f64 {
    let mut x = 1.0 / 0.02f64.sqrt();
    for _ in 0..200 {
        x = -2.0 * (rel_rough / 3.7 + 2.51 * x / re).log10();
    }
    1.0 / (x * x)
}

#[test]
fn colebrook_solver_satisfies_the_three_printed_equivalent_forms() {
    for &(re, rr) in &[(5e3, 1e-4), (1e5, 1e-3), (1e6, 0.0), (1e7, 5e-2)] {
        let f = colebrook(re, rr);
        let rhs1 = -2.0 * (rr / 3.7 + 2.51 / (re * f.sqrt())).log10();
        // form with hydraulic radius, eps / (14.8 R_h) with R_h = D/4
        let rhs2 = -2.0 * (rr / (14.8 / 4.0) + 2.51 / (re * f.sqrt())).log10();
        // form with 1.1364 = 2 log10(3.7) and 9.287 = 2.51 * 3.7
        let k = 2.0 * 3.7f64.log10();
        assert!((k - 1.1364).abs() < 5e-5);
        let rhs3 = if rr > 0.0 {
            k - 2.0 * (rr + 9.287 / (re * f.sqrt())).log10()
        } else {
            rhs1
        };
        for r in [rhs1, rhs2, rhs3] {
            assert!((r - 1.0 / f.sqrt()).abs() < 1e-9, "Re {re} rr {rr}");
        }
    }
}

#[test]
fn colebrook_limits_smooth_prandtl_form_and_rough_asymptote() {
    // smooth limit is the Prandtl form 1/sqrt(f) = 2 log10(Re sqrt f) - 2 log10(2.51); 2 log10(2.51) = 0.7993
    let (re, f) = (1e5, colebrook(1e5, 0.0));
    let kp = 1.930 * (re * f.sqrt()).log10() - 0.537; // Karman-Prandtl form with adjusted constants (read)
    assert!(
        (kp - 1.0 / f.sqrt()).abs() / (1.0 / f.sqrt()) < 0.01,
        "adjusted Karman-Prandtl vs Colebrook smooth at Re 1e5"
    );
    // fully rough asymptote: f -> 1 / (2 log10(3.7 D / eps))^2
    let f_rough = colebrook(1e12, 0.01);
    let asym = 1.0 / (2.0 * (3.7f64 / 0.01).log10()).powi(2);
    assert!((f_rough - asym).abs() / asym < 1e-3);
    // laminar and Blasius forms as printed: f = 64/Re, f = 0.3164 Re^-1/4 (smooth, up to Re 1e5);
    // Moody states +-5 % for smooth pipes: Blasius against Colebrook smooth over 4000 <= Re <= 1e5
    for &re in &[4e3f64, 1e4, 3e4, 1e5] {
        let blasius = 0.3164 * re.powf(-0.25);
        let cb = colebrook(re, 0.0);
        assert!(
            (blasius - cb).abs() / cb < 0.05,
            "Re {re}: Blasius {blasius} Colebrook {cb}"
        );
    }
    // Haaland and Swamee-Jain are approximations of Colebrook (no accuracy number printed on the page read);
    // sanity: within 3 % over the Re and roughness ranges below (measured, not a published bound)
    for &re in &[1e4, 1e5, 1e6, 1e7] {
        for &rr in &[1e-5, 1e-3, 1e-2] {
            let cb = colebrook(re, rr);
            let haal = (1.0 / (-1.8 * ((rr / 3.7).powf(1.11) + 6.9 / re).log10())).powi(2);
            let sj = 0.25 / (rr / 3.7 + 5.74 / re.powf(0.9)).log10().powi(2);
            assert!((haal - cb).abs() / cb < 0.03, "Haaland Re {re} rr {rr}");
            assert!((sj - cb).abs() / cb < 0.03, "Swamee-Jain Re {re} rr {rr}");
        }
    }
}

#[test]
fn dean_channel_relation_reproduces_the_printed_table() {
    let rows = w31c_cols("w31c_dean1978_channel_friction_table.csv", 3);
    let alpha = 0.0614;
    for r in &rows {
        let (re_tau, re_b) = (r[0], r[1]);
        // Re_tau = sqrt(alpha/2) Re_b^(7/8) = 0.1752 Re_b^(7/8)
        let c = (alpha / 2.0f64).sqrt();
        assert!((c - 0.1752).abs() < 5e-5);
        let pred = c * re_b.powf(0.875);
        assert!(
            (pred - re_tau).abs() / re_tau < 0.005,
            "Re_tau {re_tau}: relation gives {pred}"
        );
        // U_b / u_tau = sqrt(2 / Cf), Cf = alpha Re_b^-1/4
        let ub = (2.0 / (alpha * re_b.powf(-0.25))).sqrt();
        let printed = r[2];
        let tol = if re_tau == 100.0 { 0.03 } else { 0.005 };
        assert!(
            (ub - printed).abs() / printed < tol,
            "Re_tau {re_tau}: U_b+ {ub} printed {printed}"
        );
    }
    // alpha = 0.0614 is the half-width version of 0.073 Re_(2h)^-1/4: 0.073 / 2^(1/4) (search-snippet value of the
    // 0.073 form, derived here, not read in Dean's paper)
    assert!((0.073 / 2f64.powf(0.25) - alpha).abs() < 5e-5);
}

// ---------------------------------------------------------------------------------------------------------------
// 4. Graetz problem of the circular tube: eigenvalues
// ---------------------------------------------------------------------------------------------------------------

/// phi(1; lambda) of phi'' + phi'/Y + lambda^2 (1 - Y^2) phi = 0, phi(0) = 1, phi'(0) = 0, by the even power series
/// phi = sum a_k Y^(2k), (2k+2)^2 a_(k+1) = -lambda^2 (a_k - a_(k-1)).
fn phi_at_wall(lambda: f64) -> f64 {
    let l2 = lambda * lambda;
    let (mut prev, mut cur) = (0.0, 1.0);
    let mut sum = 1.0;
    for k in 0..400 {
        let next = -l2 * (cur - prev) / ((2 * k + 2) as f64).powi(2);
        sum += next;
        prev = cur;
        cur = next;
    }
    sum
}

fn first_eigenvalues(n: usize) -> Vec<f64> {
    let mut out = Vec::new();
    let (mut x, step) = (0.5, 0.05);
    let mut f0 = phi_at_wall(x);
    while out.len() < n {
        let x1 = x + step;
        let f1 = phi_at_wall(x1);
        if f0 * f1 < 0.0 {
            let (mut a, mut b, mut fa) = (x, x1, f0);
            for _ in 0..200 {
                let m = 0.5 * (a + b);
                let fm = phi_at_wall(m);
                if fa * fm <= 0.0 {
                    b = m;
                } else {
                    a = m;
                    fa = fm;
                }
            }
            out.push(0.5 * (a + b));
        }
        x = x1;
        f0 = f1;
    }
    out
}

#[test]
fn graetz_tube_eigenvalues_match_the_printed_values_and_nu_inf() {
    let rows = w31c("graetz_tube_cwt_eigenvalues.csv");
    let ours = first_eigenvalues(rows.len());
    for (r, l) in rows.iter().zip(&ours) {
        // printed to 5 significant digits
        assert!(
            (l - r[1]).abs() / r[1] < 2e-4,
            "lambda_{}: computed {l} printed {}",
            r[0],
            r[1]
        );
        assert!((l * l - r[2]).abs() / r[2] < 4e-4, "lambda^2 {}", r[0]);
    }
    // Nu_inf = lambda_1^2 / 2 with Nu = 2 h R / k; printed as 3.656 (the exact value is 3.6568)
    let nu_inf = 0.5 * ours[0] * ours[0];
    assert!((nu_inf - 3.656).abs() < 1.5e-3, "Nu_inf = {nu_inf}");
    // BYU table of wave 30C prints 3.66 for the same limit
    assert!((nu_inf - 3.66).abs() < 5e-3);
}

// ---------------------------------------------------------------------------------------------------------------
// 5. NACA 0012 experiment subsets
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn naca0012_ladson_80grit_data_have_the_expected_lift_slope_and_drag_floor() {
    let rows = w31c("w31c_naca0012_ladson1988_80grit_cl_cd.csv");
    assert_eq!(rows.len(), 17);
    // linear range alpha in [-4.1, 6.1] deg: least-squares slope per degree
    let lin: Vec<&Vec<f64>> = rows.iter().filter(|r| r[0] > -4.5 && r[0] < 6.5).collect();
    let n = lin.len() as f64;
    let (sx, sy): (f64, f64) = lin.iter().fold((0.0, 0.0), |s, r| (s.0 + r[0], s.1 + r[1]));
    let (mx, my) = (sx / n, sy / n);
    let slope = lin.iter().map(|r| (r[0] - mx) * (r[1] - my)).sum::<f64>()
        / lin.iter().map(|r| (r[0] - mx).powi(2)).sum::<f64>();
    // thin-airfoil value 2 pi per radian is 0.1097 per degree; measured slope of the tripped data
    assert!(slope > 0.105 && slope < 0.1097, "slope {slope} per degree");
    // symmetric section: Cl(alpha = 0) ~ 0 within the experimental offset
    assert!(lin.iter().find(|r| r[0].abs() < 0.1).unwrap()[1].abs() < 0.02);
    // minimum drag coefficient of the tripped section about 0.008
    let cdmin = rows.iter().map(|r| r[2]).fold(f64::INFINITY, f64::min);
    assert!(cdmin > 0.0079 && cdmin < 0.0082, "Cd min {cdmin}");
    // stall: the last two rows have a sharp drag rise
    assert!(rows[15][2] > 0.1 && rows[16][2] > 0.1);
    let clmax = rows.iter().map(|r| r[1]).fold(f64::MIN, f64::max);
    assert!((clmax - 1.6116).abs() < 1e-9);
}

#[test]
fn naca0012_gregory_cp_alpha0_upper_surface_basic_shape() {
    let rows = w31c("w31c_naca0012_gregory1970_cp_upper_alpha0.csv");
    assert_eq!(rows.len(), 25);
    assert_eq!(rows[0][0], 0.0);
    assert_eq!(rows[0][1], 1.0, "stagnation point Cp = 1");
    let cpmin = rows.iter().map(|r| r[1]).fold(f64::INFINITY, f64::min);
    // suction peak of the order -0.42 at x/c ~ 0.1 to 0.13 (digitised data)
    assert!(cpmin > -0.43 && cpmin < -0.41, "Cp min {cpmin}");
    let at_min = rows.iter().find(|r| r[1] == cpmin).unwrap()[0];
    assert!(at_min > 0.09 && at_min < 0.13, "x/c of min {at_min}");
    // the upper-surface x/c coordinates increase monotonically
    assert!(rows.windows(2).all(|w| w[1][0] > w[0][0]));
}
