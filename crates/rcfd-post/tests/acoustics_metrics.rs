//! ADR 0066 checks M1-M5 and W6a: SPL/OASPL, A-weighting, spectra and 1/3-octave bands, Bessel functions, sphere
//! quadrature, located errors. Limits are those registered in the ADR (not changed after a run).

use rcfd_post::acoustics::*;
use rcfd_post::rotor::bessel_j;
use std::f64::consts::PI;

fn sine(a: f64, f: f64, fs: f64, n: usize, phase: f64) -> Vec<f64> {
    (0..n)
        .map(|i| a * (2.0 * PI * f * i as f64 / fs + phase).sin())
        .collect()
}

#[test]
fn m1_spl_and_oaspl() {
    assert!(spl_db(20e-6).unwrap().abs() <= 1e-12);
    let one = spl_db(1.0).unwrap();
    println!("M1 SPL(1 Pa) = {one:.12}");
    assert!((one - 93.97940008672).abs() <= 1e-9);
    assert!((spl_db(2.0).unwrap() - spl_db(1.0).unwrap() - 6.02059991328).abs() <= 1e-12);
    // OASPL of a sine over whole periods
    let (fs, n, a) = (65536.0, 65536usize, 3.7);
    let x = sine(a, 1000.0, fs, n, 0.4);
    let exact = 20.0 * (a / (2f64.sqrt() * 20e-6)).log10();
    let got = oaspl_time(&x).unwrap();
    println!("M1 OASPL time {got:.12} exact {exact:.12}");
    assert!((got - exact).abs() <= 1e-9);
    // from the Parseval spectrum (rectangular window, tone on a bin)
    let spec = power_spectrum(&x, 1.0 / fs, Window::Rectangular).unwrap();
    let from_spec = spec.oaspl().unwrap().unwrap();
    println!("M1 OASPL spectrum {from_spec:.12}");
    assert!((from_spec - got).abs() <= 1e-9);
}

#[test]
fn m2_a_weighting() {
    assert!(a_weighting_db(1000.0).unwrap().abs() <= 1e-12);
    // three values read on metricgate.com (printed to 0.01 dB)
    for (f, v) in [(63.0, -26.22), (125.0, -16.19), (2000.0, 1.20)] {
        let d = (a_weighting_db(f).unwrap() - v).abs();
        println!("M2 {f} Hz: {} vs {v} (diff {d:.4})", a_weighting_db(f).unwrap());
        assert!(d <= 0.01, "M2 {f} Hz: diff {d}");
    }
    // table recalled from memory (UNVERIFIED), at the exact base-10 frequencies 10^(k/10), k = 10..43
    let table = [
        -70.4, -63.4, -56.7, -50.5, -44.7, -39.4, -34.6, -30.2, -26.2, -22.5, -19.1, -16.1, -13.4, -10.9, -8.6, -6.6,
        -4.8, -3.2, -1.9, -0.8, 0.0, 0.6, 1.0, 1.2, 1.3, 1.2, 1.0, 0.5, -0.1, -1.1, -2.5, -4.3, -6.6, -9.3,
    ];
    assert_eq!(table.len(), 34);
    let mut worst = 0.0_f64;
    for (i, v) in table.iter().enumerate() {
        let f = 10f64.powf((10 + i) as f64 / 10.0);
        let d = (a_weighting_db(f).unwrap() - v).abs();
        worst = worst.max(d);
        println!(
            "M2 table f = {f:.3}: {:.3} vs {v} (diff {d:.3})",
            a_weighting_db(f).unwrap()
        );
    }
    println!("M2 worst difference to the recalled table {worst:.3} dB");
    assert!(worst <= 0.1, "M2 recalled table: worst {worst}");
    // finite everywhere
    for i in 0..=1000 {
        let f = 0.1 * 10f64.powf(7.0 * i as f64 / 1000.0);
        assert!(a_weighting_db(f).unwrap().is_finite());
    }
}

#[test]
fn m3_bands_tone_lands_in_the_right_band_and_energy_is_preserved() {
    let (fs, n) = (65536.0, 65536usize);
    for (f, label) in [
        (100.0, 100.0),
        (1000.0, 1000.0),
        (1250.0, 1250.0),
        (5000.0, 5000.0),
        (1122.0, 1000.0),
        (1123.0, 1250.0),
    ] {
        let x = sine(2.0, f, fs, n, 0.3);
        let spec = power_spectrum(&x, 1.0 / fs, Window::Rectangular).unwrap();
        let bands = third_octave_bands(&spec, 20.0, 20000.0).unwrap();
        let total: f64 = bands.iter().map(|b| b.ms).sum();
        let hit: Vec<&Band> = bands.iter().filter(|b| b.ms > 1e-6 * total).collect();
        println!(
            "M3 tone {f} Hz -> bands {:?}",
            hit.iter().map(|b| b.nominal).collect::<Vec<_>>()
        );
        assert_eq!(hit.len(), 1, "tone {f}");
        assert_eq!(hit[0].nominal, label, "tone {f}");
        let e = (total / 2.0 - 1.0).abs(); // A^2/2 = 2
        assert!(e <= 1e-9, "tone {f}: energy {e:.3e}");
    }
    // edges
    let spec = power_spectrum(&sine(1.0, 1000.0, fs, n, 0.0), 1.0 / fs, Window::Rectangular).unwrap();
    let bands = third_octave_bands(&spec, 20.0, 20000.0).unwrap();
    let b1000 = bands.iter().find(|b| b.nominal == 1000.0).unwrap();
    assert!((b1000.centre - 1000.0).abs() < 1e-9 && (b1000.hi - 1122.018454).abs() < 1e-5);
    // off-bin tone, Hann window
    let x = sine(2.0, 1003.7, fs, n, 0.9);
    let spec = power_spectrum(&x, 1.0 / fs, Window::Hann).unwrap();
    let bands = third_octave_bands(&spec, 20.0, 20000.0).unwrap();
    let total: f64 = bands.iter().map(|b| b.ms).sum();
    let e = (total / 2.0 - 1.0).abs();
    println!("M3 Hann off-bin tone: band-sum energy error {e:.3e}");
    assert!(e <= 1e-6);
}

#[test]
fn m3_parseval_and_nominal_labels() {
    let mut state = 12345u64;
    let x: Vec<f64> = (0..5000)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 11) as f64 / (1u64 << 53) as f64) - 0.5
        })
        .collect();
    for win in [Window::Rectangular, Window::Hann] {
        let spec = power_spectrum(&x, 1e-4, win).unwrap();
        let n = spec.n_used;
        assert_eq!(n, 4096);
        let mean = x[..n].iter().sum::<f64>() / n as f64;
        let w: Vec<f64> = (0..n)
            .map(|i| match win {
                Window::Rectangular => 1.0,
                Window::Hann => 0.5 * (1.0 - (2.0 * PI * i as f64 / n as f64).cos()),
            })
            .collect();
        let num: f64 = (0..n).map(|i| ((x[i] - mean) * w[i]).powi(2)).sum();
        let den: f64 = w.iter().map(|v| v * v).sum();
        let e = (spec.total_ms() / (num / den) - 1.0).abs();
        println!("M3 Parseval {win:?}: {e:.3e}");
        assert!(e <= 1e-12);
    }
    let labels: Vec<f64> = (-17..=13).map(nominal_frequency).collect();
    assert_eq!(labels[0], 20.0);
    assert_eq!(labels[17], 1000.0);
    assert_eq!(*labels.last().unwrap(), 20000.0);
    assert!(labels.contains(&31.5) && labels.contains(&3150.0) && labels.contains(&630.0));
    assert_eq!(band_index(1000.0).unwrap(), 0);
}

#[test]
fn m3_empty_bands_have_no_level_and_everything_is_finite() {
    let x = vec![0.0; 1024];
    let rep = spectrum_report(&x, 1e-4, Window::Hann, 20.0, 5000.0).unwrap();
    assert!(rep.oaspl.is_none() && rep.oaspl_a.is_none());
    assert!(rep.bands.iter().all(|b| b.level().is_none() && b.level_a().is_none()));
    let doc = rep.to_json();
    assert!(doc["third_octave"][3]["level_db"].is_null());
    let csv = rep.bands_csv();
    assert!(csv.lines().nth(1).unwrap().ends_with(",,"), "{csv}");
    // a real signal: all numbers finite
    let x = sine(1.0, 800.0, 20000.0, 4096, 0.2);
    let rep = spectrum_report(&x, 1.0 / 20000.0, Window::Hann, 20.0, 8000.0).unwrap();
    assert!(rep.spectrum.ms.iter().all(|v| v.is_finite() && *v >= 0.0));
    assert!(rep
        .bands
        .iter()
        .all(|b| b.ms.is_finite() && b.level().map_or(true, |l| l.is_finite())));
    let dba = rep.oaspl_a.unwrap();
    assert!(dba.is_finite());
}

#[test]
fn m4_bessel_against_trapezoid_rule() {
    let nodes = 4000;
    let mut worst = 0.0_f64;
    for &x in &[0.1, 1.0, 5.0, 20.0, 50.0] {
        for n in 0..=40usize {
            let t: f64 = (0..nodes)
                .map(|i| {
                    let tau = 2.0 * PI * i as f64 / nodes as f64;
                    (n as f64 * tau - x * tau.sin()).cos()
                })
                .sum::<f64>()
                / nodes as f64;
            let d = (bessel_j(n, x).unwrap() - t).abs();
            worst = worst.max(d);
        }
    }
    println!("M4 worst |J_n - trapezoid| = {worst:.3e}");
    assert!(worst <= 1e-13);
    assert_eq!(bessel_j(0, 0.0).unwrap(), 1.0);
    assert_eq!(bessel_j(3, 0.0).unwrap(), 0.0);
}

#[test]
fn w6a_sphere_quadrature_and_power_identity() {
    let q = sphere_quadrature(8, 16).unwrap();
    let total: f64 = q.iter().map(|p| p.1).sum();
    let nz2: f64 = q.iter().map(|p| p.1 * p.0[2] * p.0[2]).sum();
    println!(
        "W6a quadrature: {:.3e} {:.3e}",
        (total / (4.0 * PI) - 1.0).abs(),
        (nz2 / (4.0 * PI / 3.0) - 1.0).abs()
    );
    assert!((total / (4.0 * PI) - 1.0).abs() <= 1e-13);
    assert!((nz2 / (4.0 * PI / 3.0) - 1.0).abs() <= 1e-13);
    // uniform P cos(wt) on the sphere: W = 4 pi R^2 P^2/(2 rho0 c)
    let (rho0, c0, r, p) = (1.2, 340.0, 7.0, 0.8);
    let w: Vec<f64> = q.iter().map(|x| x.1).collect();
    let series: Vec<Vec<f64>> = q
        .iter()
        .map(|_| (0..128).map(|i| p * (2.0 * PI * i as f64 / 64.0).cos()).collect())
        .collect();
    let power = sound_power_sphere(r, &w, &series, rho0, c0).unwrap();
    let exact = 4.0 * PI * r * r * p * p / (2.0 * rho0 * c0);
    let e = (power / exact - 1.0).abs();
    println!("W6a power identity: {e:.3e}");
    assert!(e <= 1e-10);
}

#[test]
fn m5_located_errors() {
    let e = spl_db(-1.0).unwrap_err().to_string();
    assert!(e.contains("p_rms"), "{e}");
    assert!(spl_db(f64::NAN).unwrap_err().to_string().contains("p_rms"));
    assert!(spl_db(0.0).unwrap_err().to_string().contains("p_rms"));
    let e = power_spectrum(&[1.0; 5], 1e-3, Window::Hann).unwrap_err().to_string();
    assert!(e.contains("samples"), "{e}");
    let e = power_spectrum(&[1.0; 16], 0.0, Window::Hann).unwrap_err().to_string();
    assert!(e.contains("dt"), "{e}");
    let mut x = vec![0.5; 64];
    x[17] = f64::NAN;
    let e = power_spectrum(&x, 1e-3, Window::Hann).unwrap_err().to_string();
    assert!(e.contains("sample 17"), "{e}");
    let bad = Spectrum {
        df: 1.0,
        freq: vec![0.0, 2.0, 1.0],
        ms: vec![0.0; 3],
        n_used: 4,
    };
    let e = third_octave_bands(&bad, 20.0, 2000.0).unwrap_err().to_string();
    assert!(e.contains("freq"), "{e}");
    assert!(a_weighting_db(0.0).unwrap_err().to_string().contains("f:"));
    assert!(a_weighting_db(f64::NAN).is_err());
    let e = sound_power_sphere(1.0, &[1.0], &[vec![1.0, f64::NAN]], 1.2, 340.0)
        .unwrap_err()
        .to_string();
    assert!(e.contains("series[0]") && e.contains("sample 1"), "{e}");
}
