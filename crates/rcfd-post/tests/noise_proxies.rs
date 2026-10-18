//! ADR 0066 checks P1-P3 (identities and scaling exponents only; the proxies are NOT validated against measured
//! noise) and the proxy part of M5.

use rcfd_post::acoustics::{sound_power_sphere, sphere_quadrature};
use rcfd_post::noise_proxies::*;
use std::f64::consts::PI;

#[test]
fn p1_proudman_arithmetic_and_scaling() {
    let (k, eps, rho0, c0, alpha) = (3.0, 120.0, 1.2, 340.0, 0.1);
    let p = proudman_power_density(k, eps, rho0, c0, alpha).unwrap();
    let exact = alpha * rho0 * eps * ((2.0 * k).sqrt() / c0).powi(5);
    assert!((p / exact - 1.0).abs() <= 1e-12);
    let r = |a: f64, b: f64, c: f64| proudman_power_density(a, b, rho0, c, alpha).unwrap() / p;
    assert!((r(4.0 * k, eps, c0) / 32.0 - 1.0).abs() <= 1e-12);
    assert!((r(k, 2.0 * eps, c0) / 2.0 - 1.0).abs() <= 1e-12);
    assert!((r(k, eps, 2.0 * c0) * 32.0 - 1.0).abs() <= 1e-12);
    let l = power_density_level_db(p).unwrap();
    assert!((l - 10.0 * (p / 1e-12).log10()).abs() <= 1e-12);
    assert_eq!(proudman_power_density(0.0, eps, rho0, c0, alpha).unwrap(), 0.0);
    assert!(power_density_level_db(0.0).is_none());
    let w = proudman_total_power(&[(k, eps, 2.0), (k, eps, 3.0)], rho0, c0, alpha).unwrap();
    assert!((w / (5.0 * p) - 1.0).abs() <= 1e-12);
    let (u, l) = turbulence_scales(3.0, 120.0).unwrap();
    assert!((u - 2f64.sqrt()).abs() <= 1e-12 && (l - 3f64.powf(1.5) / 120.0).abs() <= 1e-12);
}

#[test]
fn p2_curle_proxy_power_matches_sphere_integral_of_far_field() {
    let (rho0, c0) = (1.2, 340.0);
    let faces: Vec<DipoleFace> = (0..7)
        .map(|i| DipoleFace {
            area: 0.01 * (1 + i) as f64,
            normal: [0.0, 0.0, 1.0],
            dpdt_rms: 500.0 + 40.0 * i as f64,
            corr_area: 1e-4 * (1 + i % 3) as f64,
        })
        .collect();
    let w = curle_power(&faces, rho0, c0).unwrap();
    let r = 12.0;
    let q = sphere_quadrature(8, 16).unwrap();
    let mut sum = 0.0;
    for (dir, wq) in &q {
        sum += wq * r * r * curle_far_field_ms(&faces, c0, r, *dir).unwrap() / (rho0 * c0);
    }
    let e = (sum / w - 1.0).abs();
    println!("P2 sphere integral vs closed form: {e:.3e}");
    assert!(e <= 1e-10);
    // and through the series-based power routine: a synthetic series with the exact mean square
    let weights: Vec<f64> = q.iter().map(|x| x.1).collect();
    let series: Vec<Vec<f64>> = q
        .iter()
        .map(|(dir, _)| {
            let ms = curle_far_field_ms(&faces, c0, r, *dir).unwrap();
            (0..64)
                .map(|i| (2.0 * ms).sqrt() * (2.0 * PI * i as f64 / 64.0).sin())
                .collect()
        })
        .collect();
    let w2 = sound_power_sphere(r, &weights, &series, rho0, c0).unwrap();
    assert!((w2 / w - 1.0).abs() <= 1e-10);
    // flat plate dipole: no radiation in the plane of the plate
    assert!(curle_far_field_ms(&faces, c0, r, [1.0, 0.0, 0.0]).unwrap() < 1e-30);
    assert!((dpdt_from_frequency(10.0, 100.0).unwrap() - 2.0 * PI * 1000.0).abs() <= 1e-9);
}

#[test]
fn p3_trailing_edge_scaling() {
    let base = trailing_edge_power(1.0, 1.2, 10.0, 0.3, 0.01, 340.0).unwrap();
    let r = |k: f64, u: f64, c: f64| trailing_edge_power(k, 1.2, u, 0.3, 0.01, c).unwrap() / base;
    assert!((r(1.0, 20.0, 340.0) / 32.0 - 1.0).abs() <= 1e-12);
    assert!((r(1.0, 10.0, 680.0) * 4.0 - 1.0).abs() <= 1e-12);
    assert!((r(2.5, 10.0, 340.0) / 2.5 - 1.0).abs() <= 1e-12);
}

#[test]
fn m5_proxy_located_errors() {
    assert!(proudman_power_density(-1.0, 1.0, 1.2, 340.0, 0.1)
        .unwrap_err()
        .to_string()
        .contains("k:"));
    assert!(proudman_power_density(1.0, f64::NAN, 1.2, 340.0, 0.1)
        .unwrap_err()
        .to_string()
        .contains("eps"));
    let bad = vec![DipoleFace {
        area: -1.0,
        normal: [0.0, 0.0, 1.0],
        dpdt_rms: 1.0,
        corr_area: 1.0,
    }];
    assert!(curle_power(&bad, 1.2, 340.0)
        .unwrap_err()
        .to_string()
        .contains("faces[0].area"));
    let bad = vec![DipoleFace {
        area: 1.0,
        normal: [0.0, 0.0, 2.0],
        dpdt_rms: 1.0,
        corr_area: 1.0,
    }];
    assert!(curle_power(&bad, 1.2, 340.0)
        .unwrap_err()
        .to_string()
        .contains("normal"));
    assert!(trailing_edge_power(1.0, 1.2, -1.0, 1.0, 1.0, 340.0).is_err());
}
