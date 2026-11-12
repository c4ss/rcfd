//! ADR 0225 (F-300) part C, library checks of the smooth wall eddy viscosity of k-epsilon: W1 Spalding's `u+(y+)` and the
//! k-based wall eddy viscosity built on it (inverse, sign, limits, no kink at `y*_lam`), with the negative control W1N on the
//! standard (two-branch) function; W2 one steady sweep on a one-cell box whose k equation reduces to `k' = G_w k0 / eps_w`, so
//! that the sweep is seen to take the law in the wall-cell production and on the wall face. The limits are those registered in
//! ADR 0225 before this code ran.

use rcfd_fv::{FvGeometry, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::box_2d;
use rcfd_turb::kepsilon::WallFunction;
use rcfd_turb::wall::Roughness;
use rcfd_turb::{
    wall_cell_sources_law, KEpsilon, KEpsilonVariant, SpaldingLaw, WallConstants, WallNutLaw, WallTreatment,
};
use std::sync::Arc;

/// `k` that gives `y*` at wall distance `y` (the default `C_mu`).
fn k_for(wc: &WallConstants, nu: f64, y: f64, ystar: f64) -> f64 {
    (ystar * nu / (wc.cmu.powf(0.25) * y)).powi(2)
}

/// `g(y*) = nut(y*) / nu` of the W1 set-up (`nu = 1e-3`, `y = 1e-2`) for one law.
fn g(law: WallNutLaw, ystar: f64) -> f64 {
    let wc = WallConstants::default();
    let (nu, y) = (1e-3, 1e-2);
    let k = k_for(&wc, nu, y, ystar);
    match law {
        WallNutLaw::Standard => wc.nut_wall_from_k(nu, k, y) / nu,
        WallNutLaw::Spalding => wc.nut_wall_from_k_spalding(nu, k, y) / nu,
    }
}

/// The one-sided difference quotients at `y*_lam` with `h = 1e-3 y*_lam`: `(D-, D+)`.
fn quotients(law: WallNutLaw) -> (f64, f64) {
    let yl = WallConstants::default().y_plus_lam();
    let h = 1e-3 * yl;
    let (gm, g0, gp) = (g(law, yl - h), g(law, yl), g(law, yl + h));
    ((g0 - gm) / h, (gp - g0) / h)
}

/// W1: Spalding's inverse, the sign and limits of the wall eddy viscosity, and no kink at the intersection.
#[test]
fn w1_spalding_wall_nut_is_smooth_at_the_intersection() {
    let law = SpaldingLaw::default();
    let mut worst: f64 = 0.0;
    for i in 0..=60 {
        let ys = 10f64.powf(-2.0 + i as f64 / 10.0);
        let u = law.u_plus(ys);
        let r = (law.y_plus(u) - ys).abs() / ys;
        worst = worst.max(r);
        assert!(r <= 1e-12, "W1 (i): y* {ys:e}: |y+(u+(y*)) - y*| / y* = {r:e}");
        let gi = g(WallNutLaw::Spalding, ys);
        assert!(gi >= 0.0, "W1 (ii): g({ys:e}) = {gi:e} < 0");
    }
    println!("W1 (i): largest |y+(u+(y*)) - y*| / y* on the grid {worst:.3e} (limit 1e-12); g >= 0 on the grid");
    let (low, high) = (g(WallNutLaw::Spalding, 1e-2), g(WallNutLaw::Spalding, 1e3));
    let ratio = high / g(WallNutLaw::Standard, 1e3) - 1.0;
    println!("W1 (ii): g(1e-2) = {low:.3e} (limit 1e-6); g(1e3) / g_std(1e3) - 1 = {ratio:.3e} (limit 1e-2)");
    assert!(low <= 1e-6, "W1 (ii): g(1e-2) = {low:e}");
    assert!(ratio.abs() <= 1e-2, "W1 (ii): g(1e3) / g_std(1e3) - 1 = {ratio:e}");
    let (dm, dp) = quotients(WallNutLaw::Spalding);
    let jump = (dp - dm).abs() / dm.abs().max(dp.abs());
    println!(
        "W1 (iii): Spalding at y*_lam: D- {dm:.6e}, D+ {dp:.6e}, |D+ - D-| / max = {jump:.3e} (limit 1e-2); g(y*_lam) = {:.6e}",
        g(WallNutLaw::Spalding, WallConstants::default().y_plus_lam())
    );
    assert!(jump <= 1e-2, "W1 (iii): {jump:e}");
}

/// W1N: the standard function has the kink at `y*_lam` (W1 (iii) can fail).
#[test]
fn w1n_the_standard_wall_nut_has_a_kink_at_the_intersection() {
    let (dm, dp) = quotients(WallNutLaw::Standard);
    let jump = (dp - dm).abs() / dm.abs().max(dp.abs());
    println!("W1N: standard at y*_lam: D- {dm:.6e}, D+ {dp:.6e}, |D+ - D-| / max = {jump:.3e} (must be >= 0.5)");
    assert!(jump >= 0.5, "W1N did not fire: {jump:e}");
}

const NU: f64 = 0.01;
const Y: f64 = 0.5;

/// One steady sweep of the one-cell set-up OC at `y*` with `law`: (k0, k', wall-face eddy viscosity after the sweep).
fn oc_sweep(law: WallNutLaw, ystar: f64) -> (f64, f64, f64) {
    let mesh = box_2d(1, 1, 1.0, 1.0, false, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let wc = WallConstants::default();
    let k0 = k_for(&wc, NU, Y, ystar);
    let mut m = KEpsilon::new(KEpsilonVariant::Standard, geom.clone(), NU, &["ymin"], k0, 1.0).unwrap();
    assert_eq!(m.wall_treatment, WallTreatment::Smooth);
    assert_eq!(m.wall_function, WallFunction::Standard);
    m.relax_turb = 1.0;
    m.solver = SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-14,
        max_iter: 1000,
    };
    m.wall_nut_law = law;
    let u = VectorField::uniform("U", &geom, [1.0, 0.0, 0.0]);
    let phi = vec![0.0; geom.mesh.n_faces()];
    m.update(&u, &phi);
    // W2-0: the cell's wall face is the face of ymin, at y = 0.5
    let ymin: Vec<usize> = geom.mesh.patch("ymin").unwrap().faces().collect();
    assert_eq!(m.wall_nut.len(), 1, "W2-0: wall faces {:?}", m.wall_nut);
    let f = m.wall_nut[0].0;
    assert_eq!(ymin, vec![f], "W2-0: the wall face");
    let y = geom.dn[f - geom.n_internal];
    assert!((y - Y).abs() <= 1e-14, "W2-0: y = {y}");
    (k0, m.k.values[0], m.wall_nut[0].1)
}

/// W2: the sweep takes the law in the wall-cell production and on the wall face.
#[test]
fn w2_one_cell_sweep_uses_the_law_in_production_and_wall_face() {
    let wc = WallConstants::default();
    for ystar in [5.0, wc.y_plus_lam(), 30.0] {
        let mut kp = Vec::new();
        for law in [WallNutLaw::Standard, WallNutLaw::Spalding] {
            let (k0, k1, nutw) = oc_sweep(law, ystar);
            let (gw, ew) = wall_cell_sources_law(
                WallTreatment::Smooth,
                WallFunction::Standard,
                &Roughness::SMOOTH,
                &mut false,
                &wc,
                NU,
                k0,
                Y,
                1.0,
                law,
            );
            let want = gw * k0 / ew;
            let r = (k1 - want).abs() / k1;
            let nut_law = match law {
                WallNutLaw::Standard => wc.nut_wall_from_k(NU, k1, Y),
                WallNutLaw::Spalding => wc.nut_wall_from_k_spalding(NU, k1, Y),
            };
            println!(
                "W2 y* {ystar:.4} {law:?}: k0 {k0:.6e}, k' {k1:.6e}, |k' - G k0/eps| / k' = {r:.3e} (limit 1e-12); wall nut {nutw:.6e} = law {nut_law:.6e}: {}",
                nutw.to_bits() == nut_law.to_bits()
            );
            assert!(r <= 1e-12, "W2 y* {ystar} {law:?}: {r:e}");
            assert_eq!(
                nutw.to_bits(),
                nut_law.to_bits(),
                "W2 y* {ystar} {law:?}: wall nut {nutw:e} against {nut_law:e}"
            );
            kp.push(k1);
        }
        let d = (kp[1] - kp[0]).abs() / kp[0];
        println!("W2 y* {ystar:.4}: |k'_Spalding - k'_Standard| / k'_Standard = {d:.3e} (must be >= 1e-3)");
        assert!(d >= 1e-3, "W2 y* {ystar}: non-vacuity {d:e}");
    }
}
