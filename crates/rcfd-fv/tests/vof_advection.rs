//! ADR 0049 checks A1-A4 (pure advection of a sharp interface with a prescribed, discretely solenoidal face flux). The limits are
//! those registered in the ADR before any run; none was changed afterwards.

use rcfd_fv::vof::{indicator_fraction, AlphaTransport};
use rcfd_fv::{FvGeometry, ScalarField};
use rcfd_mesh::gen::box_2d;
use std::f64::consts::PI;
use std::sync::Arc;

struct Setup {
    geom: Arc<FvGeometry>,
    depth: f64,
}

fn setup(nx: usize, ny: usize, lx: f64, ly: f64) -> Setup {
    let mesh = Arc::new(box_2d(nx, ny, lx, ly, false, false).unwrap());
    Setup {
        geom: Arc::new(FvGeometry::new(mesh).unwrap()),
        depth: lx / nx as f64,
    }
}

/// Face flux from a stream function `u = d psi/dy`, `v = -d psi/dx`, exact node differences (discretely divergence free).
fn psi_flux(s: &Setup, psi: &dyn Fn(f64, f64) -> f64) -> Vec<f64> {
    let mesh = &s.geom.mesh;
    let sf = mesh.face_area_vectors();
    let cf = mesh.face_centres();
    (0..mesh.n_faces())
        .map(|f| {
            if s.geom.empty[f] {
                return 0.0;
            }
            let a = (sf[f][0] * sf[f][0] + sf[f][1] * sf[f][1]).sqrt();
            let h = a / s.depth;
            if sf[f][0].abs() > sf[f][1].abs() {
                sf[f][0].signum() * (psi(cf[f][0], cf[f][1] + 0.5 * h) - psi(cf[f][0], cf[f][1] - 0.5 * h)) * s.depth
            } else {
                -sf[f][1].signum() * (psi(cf[f][0] + 0.5 * h, cf[f][1]) - psi(cf[f][0] - 0.5 * h, cf[f][1])) * s.depth
            }
        })
        .collect()
}

fn make(s: &Setup, inside: impl Fn([f64; 3]) -> bool, c_alpha: f64) -> AlphaTransport {
    let mut a = ScalarField::uniform("alpha", &s.geom, 0.0);
    a.values = indicator_fraction(&s.geom, 10, inside);
    AlphaTransport::new(s.geom.clone(), a, c_alpha, 0.25)
}

fn l1(s: &Setup, a: &[f64], b: &[f64]) -> f64 {
    let vol = s.geom.mesh.cell_volumes();
    let num: f64 = (0..a.len()).map(|c| (a[c] - b[c]).abs() * vol[c]).sum();
    let den: f64 = (0..a.len()).map(|c| b[c] * vol[c]).sum();
    num / den
}

fn mixed(a: &[f64]) -> usize {
    a.iter().filter(|&&x| x > 0.01 && x < 0.99).count()
}

fn bounds(a: &[f64]) -> (f64, f64) {
    a.iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| (l.min(x), h.max(x)))
}

#[test]
fn a1_slug_one_dimensional() {
    let n = 200;
    let s = setup(n, 1, 1.0, 1.0 / n as f64);
    let flux = psi_flux(&s, &|_x, y| y); // u = 1
    for (c_alpha, limited) in [(1.0, true), (0.0, false)] {
        let mut t = make(&s, |x| x[0] >= 0.1 && x[0] <= 0.3, c_alpha);
        let v0 = t.phase_volume();
        let dt = 0.004; // flux Courant 0.8
        let (mut lo, mut hi, mut dv) = (0.0f64, 1.0f64, 0.0f64);
        for _ in 0..100 {
            t.advance(&flux, dt).unwrap();
            let (l, h) = bounds(&t.alpha.values);
            lo = lo.min(l);
            hi = hi.max(h);
            dv = dv.max((t.phase_volume() - v0).abs() / v0);
        }
        let exact = indicator_fraction(&s.geom, 10, |x| x[0] >= 0.5 && x[0] <= 0.7);
        let e = l1(&s, &t.alpha.values, &exact);
        let m = mixed(&t.alpha.values);
        println!(
            "A1 c_alpha {c_alpha}: L1 {e:.4e} (limit 0.05) mixed {m} (limit 8) min {lo:.3e} max-1 {:.3e} dV {dv:.2e}",
            hi - 1.0
        );
        assert!(lo >= -1e-12 && hi <= 1.0 + 1e-12, "bounds {lo} {hi}");
        assert!(dv <= 1e-12, "volume {dv}");
        if limited {
            assert!(e <= 0.05, "L1 {e}");
            assert!(m <= 8, "mixed {m}");
        }
    }
}

#[test]
fn a2_translated_disk() {
    let n = 64;
    let s = setup(n, n, 1.0, 1.0);
    let flux = psi_flux(&s, &|x, y| y - 0.5 * x); // u = (1, 0.5)
    let r = 0.15;
    let disk = |c: [f64; 2]| move |x: [f64; 3]| (x[0] - c[0]).hypot(x[1] - c[1]) <= r;
    let mut t = make(&s, disk([0.3, 0.3]), 1.0);
    let v0 = t.phase_volume();
    let dt = 0.3 / 60.0;
    let (mut lo, mut hi, mut dv) = (0.0f64, 1.0f64, 0.0f64);
    for _ in 0..60 {
        t.advance(&flux, dt).unwrap();
        let (l, h) = bounds(&t.alpha.values);
        lo = lo.min(l);
        hi = hi.max(h);
        dv = dv.max((t.phase_volume() - v0).abs() / v0);
    }
    let exact = indicator_fraction(&s.geom, 10, disk([0.6, 0.45]));
    let e = l1(&s, &t.alpha.values, &exact);
    let dx = 1.0 / n as f64;
    let vol_mixed: f64 = t
        .alpha
        .values
        .iter()
        .zip(s.geom.mesh.cell_volumes())
        .filter(|(a, _)| **a > 0.01 && **a < 0.99)
        .map(|(_, v)| v)
        .sum();
    let width = vol_mixed / s.depth / (2.0 * PI * r) / dx;
    println!(
        "A2 L1 {e:.4e} (limit 0.10) width {width:.3} cells (limit 3.0) min {lo:.3e} max-1 {:.3e} dV {dv:.2e}",
        hi - 1.0
    );
    assert!(lo >= -1e-12 && hi <= 1.0 + 1e-12);
    assert!(dv <= 1e-12);
    assert!(e <= 0.10, "L1 {e}");
    assert!(width <= 3.0, "width {width}");
}

fn zalesak(x: [f64; 3]) -> bool {
    let in_disk = (x[0] - 50.0).hypot(x[1] - 75.0) <= 15.0;
    let in_slot = x[0] >= 47.5 && x[0] <= 52.5 && x[1] <= 85.0;
    in_disk && !in_slot
}

/// Result of the Zalesak rotation: (L1 vs initial, slot maximum, min alpha, max alpha, relative volume drift, relative
/// drift after adding the accumulated outflow through the domain boundary faces).
fn zalesak_run() -> (f64, f64, f64, f64, f64, f64) {
    let n = 100;
    let s = setup(n, n, 100.0, 100.0);
    let w = 2.0 * PI / 628.0;
    let flux = psi_flux(&s, &|x, y| -0.5 * w * ((x - 50.0).powi(2) + (y - 50.0).powi(2)));
    let mut t = make(&s, zalesak, 1.0);
    let a0 = t.alpha.values.clone();
    let v0 = t.phase_volume();
    let (mut lo, mut hi, mut dv, mut dv_bal, mut out) = (0.0f64, 1.0f64, 0.0f64, 0.0f64, 0.0f64);
    for _ in 0..628 {
        let info = t.advance(&flux, 1.0).unwrap();
        out += info.alpha_phi[s.geom.n_internal..].iter().sum::<f64>();
        let (l, h) = bounds(&t.alpha.values);
        lo = lo.min(l);
        hi = hi.max(h);
        dv = dv.max((t.phase_volume() - v0).abs() / v0);
        dv_bal = dv_bal.max((t.phase_volume() + out - v0).abs() / v0);
    }
    let e = l1(&s, &t.alpha.values, &a0);
    // cells on the slot axis between y = 62 and 70 (the cells straddling the slot axis x = 50)
    let cc = s.geom.mesh.cell_centres();
    let slot_max = (0..cc.len())
        .filter(|&c| (cc[c][0] - 50.0).abs() < 1.0 && cc[c][1] > 62.0 && cc[c][1] < 70.0)
        .map(|c| t.alpha.values[c])
        .fold(0.0, f64::max);
    println!("A3 outflow through the boundary {out:.3e}");
    (e, slot_max, lo, hi, dv, dv_bal)
}

/// Registered A3 as written (drift of the phase volume itself `<= 1e-12`). It misses: the rotation of the registered set-up is
/// not tangential to the square domain boundary, so the numerical tails of alpha leave through the boundary faces (see the
/// ADR addendum); the limit is kept and the check moved to this ignored test.
#[test]
#[ignore = "known miss: phase-volume drift 2.3e-10 > 1e-12 from tail outflow through the open boundary (ADR 0049 addendum)"]
fn a3_zalesak_slotted_disk_known_miss() {
    let (e, slot_max, lo, hi, dv, _) = zalesak_run();
    println!(
        "A3 L1 {e:.4e} (limit 0.20) slot max alpha {slot_max:.3} (limit 0.5) min {lo:.3e} max-1 {:.3e} dV {dv:.2e}",
        hi - 1.0
    );
    assert!(lo >= -1e-12 && hi <= 1.0 + 1e-12);
    assert!(dv <= 1e-12);
    assert!(e <= 0.20, "L1 {e}");
    assert!(slot_max <= 0.5, "slot fill {slot_max}");
}

/// Corrected A3 (set-up defect: the domain is not closed): the same run, the conservation statement is the exact flux balance
/// `V + (accumulated boundary outflow) = V_0`, limit `1e-12` as registered for conservation.
#[test]
fn a3b_zalesak_slotted_disk_corrected() {
    let (e, slot_max, lo, hi, dv, dv_bal) = zalesak_run();
    println!(
        "A3b L1 {e:.4e} (limit 0.20) slot max alpha {slot_max:.3} (limit 0.5) min {lo:.3e} max-1 {:.3e} drift {dv:.2e} balance {dv_bal:.2e} (limit 1e-12)",
        hi - 1.0
    );
    assert!(lo >= -1e-12 && hi <= 1.0 + 1e-12);
    assert!(dv_bal <= 1e-12, "balance {dv_bal}");
    assert!(e <= 0.20, "L1 {e}");
    assert!(slot_max <= 0.5, "slot fill {slot_max}");
}

#[test]
fn a4_single_vortex_reversal() {
    let n = 128;
    let s = setup(n, n, 1.0, 1.0);
    let disk = |x: [f64; 3]| (x[0] - 0.5).hypot(x[1] - 0.75) <= 0.15;
    let mut t = make(&s, disk, 1.0);
    let a0 = t.alpha.values.clone();
    let v0 = t.phase_volume();
    let dt = 0.01;
    let (mut lo, mut hi, mut dv) = (0.0f64, 1.0f64, 0.0f64);
    for k in 0..200 {
        let tm = (k as f64 + 0.5) * dt;
        let c = (PI * tm / 2.0).cos();
        let flux = psi_flux(&s, &|x, y| (PI * x).sin().powi(2) * (PI * y).sin().powi(2) * c / PI);
        t.advance(&flux, dt).unwrap();
        let (l, h) = bounds(&t.alpha.values);
        lo = lo.min(l);
        hi = hi.max(h);
        dv = dv.max((t.phase_volume() - v0).abs() / v0);
    }
    let e = l1(&s, &t.alpha.values, &a0);
    println!(
        "A4 L1 {e:.4e} (limit 0.25) min {lo:.3e} max-1 {:.3e} dV {dv:.2e}",
        hi - 1.0
    );
    assert!(lo >= -1e-12 && hi <= 1.0 + 1e-12);
    assert!(dv <= 1e-12);
    assert!(e <= 0.25, "L1 {e}");
}
