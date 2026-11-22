//! ADR 0769 (F-883): a time-mean static pressure on the FAN-01 casing behind the rotor, read from the wall-pressure data of
//! the published record, and the identity of ADR 0765 evaluated with it. The data are `kulite_pressure_in_Pa` of
//! `mic_kulite/mic_kulite_n1ug_1_4.mat` in `mic_kulite.tar` of the Zenodo record doi:10.5281/zenodo.23943494 (v6, CC BY 4.0),
//! "wall pressure, differential to the chamber pressure, mean value included" (its description, Appendix A.2), transcribed
//! in `docs/investigations/f883-fan01-wall-pressure/kulite_means.csv` and recorded in `docs/reference-data.d/0769-F-883.part`.
//! The station, the mean, the casing-to-span correction, the band and the rule were registered in ADR 0769 before the data
//! were fetched. The result (ADR 0769, addendum 1): `Pi_m` = 93.96 Pa (+-7.50), the rotor's share 32.26 Pa +-29.19 Pa of the
//! missing 52.47 Pa; by the rule of ADR 0765 it cannot be told which part is the larger. No solve is run here.

const RHO: f64 = 1.2; // ASSUMED (ADR 0765): the density of the measurement is in no source that was read (F-877)
const R_WALL: f64 = 0.250;
const P_TS: f64 = 126.46;
const D_MISSING: f64 = 126.46 - 73.99;
/// B1 at x = 90 mm and the band of it (ADR 0765, addendum 1 and analysis.txt: U_num + S + T + u_cov).
const PI_S: f64 = 61.709;
const U_S: f64 = 21.683;
/// Pointwise e_m - q_m and q_m (ADR 0765).
const EM_QM: f64 = 120.819;
const QM: f64 = 99.196;

/// Per row 0 to 14 of the transcription: mean, standard deviation about it, root of the mean square [Pa].
const MEAN_STD_RMS: [[f64; 3]; 15] = [
    [0.003769, 0.158932, 0.158976],
    [-49.825314, 11.129496, 51.053184],
    [-47.796352, 15.581131, 50.271890],
    [-44.819944, 22.531784, 50.164816],
    [-42.580378, 33.073215, 53.915917],
    [-39.492848, 47.189575, 61.534877],
    [-42.047095, 76.452151, 87.251874],
    [-53.222132, 125.613404, 136.423321],
    [-68.146319, 184.067547, 196.277310],
    [26.495310, 77.569669, 81.969842],
    [92.824127, 39.549604, 100.898413],
    [104.812954, 27.883282, 108.458438],
    [112.765461, 19.909527, 114.509556],
    [116.535283, 14.388703, 117.420215],
    [117.298852, 11.501407, 117.861372],
];
/// Batch means (30 of 48 000 samples) of the rows that the checks and the reading use [Pa].
const B01: [f64; 30] = [
    -51.522276, -49.113803, -50.170411, -49.568741, -49.398361, -49.933430, -51.094611, -50.663579, -49.505950,
    -50.283789, -50.423518, -48.294295, -47.643087, -50.740036, -51.509445, -51.113277, -48.770497, -48.655431,
    -46.879696, -48.186900, -50.022961, -50.570556, -49.973512, -48.647850, -52.787542, -49.645344, -50.217536,
    -50.168078, -49.846461, -50.077839,
];
const B02: [f64; 30] = [
    -49.142469, -47.219789, -48.100762, -47.282631, -47.726921, -47.853477, -48.694663, -48.266953, -47.727076,
    -48.298401, -48.038332, -46.600806, -45.835487, -48.265900, -48.721661, -49.002580, -46.750991, -46.773937,
    -45.120236, -46.704461, -48.238072, -48.446266, -48.160351, -46.528576, -50.062393, -47.954401, -48.450254,
    -48.081918, -48.263853, -48.111344,
];
const B03: [f64; 30] = [
    -45.944854, -44.225582, -45.056877, -44.121639, -44.927909, -44.837111, -45.610033, -45.047287, -44.902630,
    -45.639721, -44.760935, -43.949117, -42.994318, -44.899952, -45.234612, -45.923635, -43.394258, -43.800899,
    -42.188906, -44.125665, -45.653005, -45.433502, -45.489363, -43.571148, -46.787553, -45.263688, -45.578871,
    -45.049228, -45.405684, -45.275004,
];
const B04: [f64; 30] = [
    -43.752149, -42.046138, -42.326584, -41.613645, -43.036185, -42.709124, -43.390720, -42.509907, -42.746468,
    -43.418745, -42.262695, -41.826616, -40.934257, -42.376873, -42.561883, -43.272952, -40.913879, -41.897033,
    -39.956126, -42.154488, -43.661178, -42.775238, -43.330020, -41.579450, -44.414811, -43.257511, -43.779740,
    -42.880803, -43.649523, -42.995422,
];
const B05: [f64; 30] = [
    -40.000903, -39.289518, -39.129587, -38.397953, -40.488654, -39.428062, -39.631151, -39.017985, -39.992129,
    -40.193935, -38.472668, -39.324816, -38.175886, -38.936372, -38.726572, -39.970512, -37.910174, -38.883409,
    -37.478461, -39.886309, -40.736450, -39.027074, -40.383002, -38.645851, -40.617527, -40.649651, -41.070343,
    -39.834503, -40.911461, -40.081154,
];
const B12: [f64; 30] = [
    110.068279, 111.670558, 112.279998, 114.266237, 113.680552, 110.365324, 110.634172, 112.566480, 113.260017,
    113.407827, 114.829146, 116.135544, 116.220501, 113.558450, 111.291034, 111.110648, 113.064175, 111.784799,
    115.383069, 112.974835, 111.520586, 111.235672, 112.030475, 113.377778, 109.951166, 111.452158, 112.601124,
    113.206489, 112.715633, 113.948248,
];
const B13: [f64; 30] = [
    114.325583, 116.387451, 116.301335, 117.859439, 118.385786, 114.956469, 114.877556, 116.267274, 117.466442,
    115.430885, 118.645133, 118.571973, 118.396318, 116.803413, 114.003975, 114.505573, 117.384541, 117.128992,
    118.882790, 117.899050, 115.370636, 115.262845, 115.914165, 116.312311, 114.152918, 115.371495, 116.455027,
    116.137153, 116.426880, 117.532206,
];

/// The rows the batch means above belong to.
const BATCH_ROWS: [(usize, &[f64; 30]); 7] = [
    (1, &B01),
    (2, &B02),
    (3, &B03),
    (4, &B04),
    (5, &B05),
    (12, &B12),
    (13, &B13),
];

fn repo_file(rel: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The data lines of a transcription as (header, rows of cells); comment lines start with `#`.
fn table(rel: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let text = repo_file(rel);
    let mut lines = text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty());
    let head = lines.next().unwrap().split(',').map(str::to_string).collect();
    (
        head,
        lines.map(|l| l.split(',').map(str::to_string).collect()).collect(),
    )
}

fn col(head: &[String], name: &str) -> usize {
    head.iter()
        .position(|h| h == name)
        .unwrap_or_else(|| panic!("column {name}"))
}

/// Twice the standard error of a mean from its batch means (ADR 0769, "The statistical uncertainty of a mean").
fn two_se(b: &[f64]) -> f64 {
    let n = b.len() as f64;
    let m = b.iter().sum::<f64>() / n;
    2.0 * (b.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0) / n).sqrt()
}

fn trapezoid(r: &[f64], y: &[f64]) -> f64 {
    (0..r.len() - 1)
        .map(|j| 0.5 * (y[j] + y[j + 1]) * (r[j + 1] - r[j]))
        .sum()
}

/// The casing-to-span correction of ADR 0769: `G(r) = int_r^R rho (<c_t^2> - <c_r^2>) / r' dr' + rho <c_r^2>(r)`, the
/// integrand falling linearly to zero from the last radius to the wall; returns (`Gbar` weighted by `ux 2 pi r`, end piece).
fn correction(r: &[f64], ux: &[f64], ct2: &[f64], cr2: &[f64]) -> (f64, f64) {
    let n = r.len();
    let f: Vec<f64> = (0..n).map(|i| RHO * (ct2[i] - cr2[i]) / r[i]).collect();
    let end = 0.5 * f[n - 1] * (R_WALL - r[n - 1]);
    let g: Vec<f64> = (0..n)
        .map(|i| trapezoid(&r[i..], &f[i..]) + end + RHO * cr2[i])
        .collect();
    let w: Vec<f64> = (0..n).map(|i| ux[i] * 2.0 * std::f64::consts::PI * r[i]).collect();
    let gw: Vec<f64> = (0..n).map(|i| g[i] * w[i]).collect();
    (trapezoid(r, &gw) / trapezoid(r, &w), end)
}

/// The LDA line at x = 90 mm of ADR 0765: radii [m], `ux_mean`, and `<c_t^2>`, `<c_r^2>` with or without the rms.
fn lda_90(full: bool) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let (h, rows) = table("docs/investigations/f878-fan01-lda-profiles/lda_profiles.csv");
    let v = |row: &Vec<String>, name: &str| -> f64 { row[col(&h, name)].parse().unwrap() };
    let rows: Vec<_> = rows.into_iter().filter(|row| row[col(&h, "x_mm")] == "90").collect();
    assert_eq!(rows.len(), 26);
    let sq = |row: &Vec<String>, m: &str, s: &str| v(row, m).powi(2) + if full { v(row, s).powi(2) } else { 0.0 };
    (
        rows.iter().map(|row| v(row, "radius_norm") * R_WALL).collect(),
        rows.iter().map(|row| v(row, "ux_mean")).collect(),
        rows.iter().map(|row| sq(row, "uphi_mean", "uphi_rms")).collect(),
        rows.iter().map(|row| sq(row, "ur_mean", "ur_rms")).collect(),
    )
}

#[derive(Debug, PartialEq)]
enum Verdict {
    Installation,
    Rotor,
    CannotBeTold,
}

#[test]
fn f883_transcription_constants() {
    let (h, rows) = table("docs/investigations/f883-fan01-wall-pressure/kulite_means.csv");
    assert_eq!(rows.len(), 15);
    let v = |i: usize, name: &str| -> f64 { rows[i][col(&h, name)].parse().unwrap() };
    for (i, c) in MEAN_STD_RMS.iter().enumerate() {
        assert_eq!(rows[i][col(&h, "row")], i.to_string());
        assert_eq!(rows[i][col(&h, "n")], "1464320");
        let x = if i == 0 {
            String::new()
        } else {
            (10 * i as i64 - 33).to_string()
        };
        assert_eq!(rows[i][col(&h, "x_mm")], x, "row {i}: x = 10 n - 33 mm");
        for (k, name) in ["mean", "std", "rms"].iter().enumerate() {
            assert!(
                (c[k] - v(i, name)).abs() < 5e-7,
                "row {i} {name}: {} against {}",
                c[k],
                v(i, name)
            );
        }
    }
    for (i, b) in BATCH_ROWS {
        for (k, x) in b.iter().enumerate() {
            let name = format!("b{:02}", k + 1);
            assert!((x - v(i, &name)).abs() < 5e-7, "row {i} {name}");
        }
    }
}

#[test]
fn f883_radial_equilibrium_closed_form() {
    // <c_t^2> = (a + k) r, <c_r^2> = k r, ux = b / r: the integrand is rho a, G is linear in r and the weight is constant,
    // so the trapezoid rule is exact and Gbar is G at the middle of the measured span.
    let (r, _, _, _) = lda_90(true);
    let (a, k, b) = (310.0, 47.0, 2.3);
    let ct2: Vec<f64> = r.iter().map(|x| (a + k) * x).collect();
    let cr2: Vec<f64> = r.iter().map(|x| k * x).collect();
    let ux: Vec<f64> = r.iter().map(|x| b / x).collect();
    let (gbar, end) = correction(&r, &ux, &ct2, &cr2);
    let (r1, rn) = (r[0], r[r.len() - 1]);
    let rm = 0.5 * (r1 + rn);
    let want_end = 0.5 * RHO * a * (R_WALL - rn);
    let want = RHO * a * (rn - rm) + want_end + RHO * k * rm;
    assert!(
        ((end - want_end) / want_end).abs() <= 1e-9,
        "end piece {end} against {want_end}"
    );
    assert!(((gbar - want) / want).abs() <= 1e-9, "Gbar {gbar} against {want}");
}

#[test]
fn f883_reading_and_share() {
    let mean = |i: usize| MEAN_STD_RMS[i][0];
    let batches = |i: usize| BATCH_ROWS.iter().find(|(j, _)| *j == i).unwrap().1;
    // K1: Appendix B, item 1 of the description (the reading of "rms" that meets it is the standard deviation)
    let std: Vec<f64> = (1..15).map(|i| MEAN_STD_RMS[i][1]).collect();
    let round = |x: f64, d: i32| (x * 10f64.powi(d)).round() / 10f64.powi(d);
    assert!(mean(0).abs() < 0.5 && round(MEAN_STD_RMS[0][1], 2) == 0.16, "K1, row 0");
    let (lo, hi) = (
        std.iter().cloned().fold(f64::MAX, f64::min),
        std.iter().cloned().fold(f64::MIN, f64::max),
    );
    assert!(
        round(lo, 0) == 11.0 && round(hi, 0) == 184.0,
        "K1, rows 1 to 14: {lo} {hi}"
    );
    // K2: ahead of the blades the wall pressure is at most the chamber's
    for i in 1..=5 {
        assert!(mean(i) <= two_se(batches(i)), "K2, row {i}: {}", mean(i));
    }
    // K3: referred to the chamber, not to the ambient
    assert!(mean(1) > -P_TS, "K3: {}", mean(1));
    // the station x = 90 mm between rows 12 (87 mm) and 13 (97 mm)
    let pw = 0.7 * mean(12) + 0.3 * mean(13);
    let comb: Vec<f64> = (0..30).map(|k| 0.7 * B12[k] + 0.3 * B13[k]).collect();
    let u_stat = two_se(&comb);
    let u_x = 0.13 * (mean(13) - mean(12)).abs();
    let (r, ux, ct2, cr2) = lda_90(true);
    let (gbar, u_gap) = correction(&r, &ux, &ct2, &cr2);
    let (_, _, ct2_0, cr2_0) = lda_90(false);
    let (gbar0, _) = correction(&r, &ux, &ct2_0, &cr2_0);
    let u_read = (gbar - gbar0).abs();
    let pi_m = pw - gbar;
    let u_m = u_stat + u_x + u_gap + u_read; // u_meas = 0: the record states no uncertainty of the wall pressure
    let share = pi_m - PI_S;
    let u = u_m + U_S;
    let verdict = if share + u < D_MISSING / 2.0 {
        Verdict::Installation
    } else if share - u > D_MISSING / 2.0 {
        Verdict::Rotor
    } else {
        Verdict::CannotBeTold
    };
    let consistent = P_TS - QM - u_m <= pi_m && pi_m <= EM_QM + u_m;
    let got = [
        ("p_w(90)", pw, 113.8964),
        ("u_stat", u_stat, 0.5674),
        ("u_x", u_x, 0.4901),
        ("Gbar", gbar, 19.9318),
        ("Gbar_0", gbar0, 14.1979),
        ("u_gap", u_gap, 0.7124),
        ("u_read", u_read, 5.7339),
        ("Pi_m", pi_m, 93.9646),
        ("U_m", u_m, 7.5038),
        ("R", share, 32.2556),
        ("I", D_MISSING - share, 20.2144),
        ("U", u, 29.1868),
        ("L_m", EM_QM - pi_m, 26.8544),
        ("Rec_m", P_TS - pi_m, 32.4954),
    ];
    for (name, value, recorded) in got {
        println!("F883 {name} {value:.4} (recorded {recorded})");
        assert!(
            (value - recorded).abs() <= 0.005,
            "{name}: {value} against the recorded {recorded}"
        );
    }
    assert_eq!(verdict, Verdict::CannotBeTold);
    assert!(
        consistent,
        "B1: Pi_m {pi_m} outside the bounds of ADR 0765 with U_m {u_m}"
    );
}
