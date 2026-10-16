//! Lorentz-force library checks (F-214, ADR 0039 checks M1, M2 and the Hartmann closed forms). The limits are the ones
//! registered in `docs/adr/0039-wave23a-mhd-heat-exchanger.md` before any run.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::lorentz::{hartmann, Lorentz, PatchElectric, PotentialMode};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

fn geometry(nx: usize, ny: usize, lx: f64, ly: f64) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(box_2d(nx, ny, lx, ly, false, false).unwrap())).unwrap())
}

fn zone_all(geom: &FvGeometry) -> Vec<bool> {
    vec![true; geom.n_cells()]
}

#[test]
fn m1_lorentz_algebra() {
    let geom = geometry(5, 4, 1.0, 1.0);
    let n = geom.n_cells();
    let mut u = VectorField::uniform("u", &geom, [0.0; 3]);
    for (c, v) in u.values.iter_mut().enumerate() {
        let x = c as f64;
        *v = [0.3 + 0.1 * x.sin(), -0.2 + 0.05 * x.cos(), 0.1 * (1.7 * x).sin()];
    }
    let cases: [(Vec3, Vec3, f64, f64); 3] = [
        ([0.0, 2.0, 0.0], [0.0; 3], 3.0, 1.2),
        ([0.7, -1.1, 0.4], [0.5, 0.2, -0.9], 5.5e5, 7000.0),
        ([0.0, 0.0, 0.3], [-1.0, 0.0, 0.4], 0.8, 1.0),
    ];
    let mut phi = ScalarField::uniform("phi", &geom, 0.0);
    for (b, e, sigma, rho) in cases {
        let l = Lorentz {
            b,
            sigma,
            e_ext: e,
            rho,
            mode: PotentialMode::None,
            patches: vec![],
            zone: zone_all(&geom),
        };
        let r = l.evaluate(&geom, &mut phi, &u);
        let s = sigma * vec3::dot(b, b) / rho;
        let (mut worst_hand, mut worst_split, mut worst_perp, mut worst_work) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
        let mut integral = [0.0; 3];
        let vol = geom.mesh.cell_volumes();
        for c in 0..n {
            let uc = u.values[c];
            let j = vec3::scale(vec3::add(e, vec3::cross(uc, b)), sigma);
            let f = vec3::cross(j, b);
            let scale = vec3::norm(f).max(1e-300);
            worst_hand = worst_hand.max(vec3::norm(vec3::sub(r.force[c], f)) / scale);
            // split identity: accel - s u = J x B / rho
            let split = vec3::sub(r.accel[c], vec3::scale(uc, r.sink[c]));
            worst_split = worst_split.max(vec3::norm(vec3::sub(split, vec3::scale(f, 1.0 / rho))) / (scale / rho));
            assert_eq!(r.sink[c], s);
            worst_perp =
                worst_perp.max(vec3::dot(r.force[c], b).abs() / (vec3::norm(r.force[c]) * vec3::norm(b)).max(1e-300));
            if vec3::norm(e) == 0.0 {
                // work identity u . (J x B) = -|J|^2 / sigma
                let w = vec3::dot(uc, r.force[c]);
                let jj = -vec3::dot(r.current[c], r.current[c]) / sigma;
                worst_work = worst_work.max((w - jj).abs() / jj.abs().max(1e-300));
            }
            vec3::axpy(&mut integral, vol[c], r.force[c]);
        }
        let v: f64 = vol.iter().sum();
        let mut want = [0.0; 3];
        for c in 0..n {
            vec3::axpy(
                &mut want,
                vol[c],
                vec3::cross(vec3::scale(vec3::add(e, vec3::cross(u.values[c], b)), sigma), b),
            );
        }
        let int_err = vec3::norm(vec3::sub(integral, want)) / vec3::norm(want).max(1e-300);
        eprintln!(
            "M1 B={b:?} E={e:?}: hand {worst_hand:.2e} (limit 1e-13), split {worst_split:.2e} (1e-13), F.B {worst_perp:.2e} (1e-12), work {worst_work:.2e} (1e-12), integral {int_err:.2e}, V {v}"
        );
        assert!(worst_hand <= 1e-13);
        assert!(worst_split <= 1e-13);
        assert!(worst_perp <= 1e-12);
        assert!(worst_work <= 1e-12);
        assert!(int_err <= 1e-12);
    }
}

#[test]
fn m1_zone_restricts_current_and_force() {
    let geom = geometry(6, 3, 1.0, 1.0);
    let cc = geom.mesh.cell_centres().to_vec();
    let zone: Vec<bool> = cc.iter().map(|c| c[0] < 0.5).collect();
    let u = VectorField::uniform("u", &geom, [1.0, 0.0, 0.0]);
    let mut phi = ScalarField::uniform("phi", &geom, 0.0);
    let l = Lorentz {
        b: [0.0, 1.0, 0.0],
        sigma: 2.0,
        e_ext: [0.0; 3],
        rho: 1.0,
        mode: PotentialMode::None,
        patches: vec![],
        zone: zone.clone(),
    };
    let r = l.evaluate(&geom, &mut phi, &u);
    for c in 0..geom.n_cells() {
        if zone[c] {
            assert!((r.force[c][0] + 2.0).abs() < 1e-13); // -sigma B^2 u
        } else {
            assert_eq!(r.force[c], [0.0; 3]);
            assert_eq!(r.sink[c], 0.0);
        }
    }
}

#[test]
fn hartmann_closed_forms_are_consistent() {
    // Ha -> 0 limit of the flow ratio is 1; the profile reduces to the parabola; the mean of the profile is the stated mean
    assert!((hartmann::flow_ratio(1e-3) - 1.0).abs() < 1e-5);
    let (sigma, b0, a, ha) = (1.0, 20.0, 0.5, 10.0);
    let g = 400.0;
    let n = 20000;
    let mut mean = 0.0;
    for i in 0..n {
        let y = -a + (i as f64 + 0.5) * 2.0 * a / n as f64;
        mean += hartmann::u_short(g, sigma, b0, ha, a, y) / n as f64;
    }
    assert!((mean / hartmann::mean_short(g, sigma, b0, ha) - 1.0).abs() < 1e-7);
    // insulated: mean U reproduced, force balance G = sigma B0^2 U (tanh/Ha)/(1-tanh/Ha) means wall shear = G a per half
    let u_mean = 1.0;
    let mut m2 = 0.0;
    for i in 0..n {
        let y = -a + (i as f64 + 0.5) * 2.0 * a / n as f64;
        m2 += hartmann::u_insulated(u_mean, ha, a, y) / n as f64;
    }
    assert!((m2 - u_mean).abs() < 1e-7);
}

/// M2: electrodes on the plates of a channel, `B` normal to the plane of the flow, imposed parabolic velocity.
fn electrode_run(dv: f64, ny: usize) -> (f64, f64, f64, f64, f64) {
    let (sigma, b0, a, u0) = (1.0_f64, 3.0_f64, 0.5_f64, 2.0_f64);
    let geom = geometry(4, ny, 1.0, 2.0 * a);
    let mut u = VectorField::uniform("u", &geom, [0.0; 3]);
    let cc = geom.mesh.cell_centres().to_vec();
    for (c, v) in u.values.iter_mut().enumerate() {
        let y = cc[c][1] - a;
        *v = [u0 * (1.0 - (y / a) * (y / a)), 0.0, 0.0];
    }
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    let mut phi = ScalarField::uniform("phi", &geom, 0.0);
    let l = Lorentz {
        b: [0.0, 0.0, b0],
        sigma,
        e_ext: [0.0; 3],
        rho: 1.0,
        mode: PotentialMode::Solve,
        patches: vec![
            ("ymin".into(), PatchElectric::Potential(0.0)),
            ("ymax".into(), PatchElectric::Potential(dv)),
        ],
        zone: zone_all(&geom),
    };
    let r = l.evaluate(&geom, &mut phi, &u);
    let ubar = 2.0 * u0 / 3.0;
    let j_exact = -sigma * (dv / (2.0 * a) + b0 * ubar);
    let j_mean = r.current.iter().map(|j| j[1]).sum::<f64>() / geom.n_cells() as f64;
    let spread = r.current.iter().map(|j| (j[1] - j_mean).abs()).fold(0.0, f64::max);
    let vol = geom.mesh.cell_volumes();
    let v: f64 = vol.iter().sum();
    let fx: f64 = r.force.iter().zip(vol).map(|(f, v)| f[0] * v).sum();
    let _ = sigma;
    (j_mean, j_exact, spread / j_mean.abs().max(1e-300), fx, b0 * v * j_mean)
}

#[test]
fn m2_electrode_current_matches_the_closed_form() {
    let (sigma, b0, a, u0) = (1.0_f64, 3.0_f64, 0.5_f64, 2.0_f64);
    let ubar = 2.0 * u0 / 3.0;
    // short circuit
    let (j, jx, spread, fx, want) = electrode_run(0.0, 40);
    eprintln!(
        "M2 short circuit: j {j:.6} vs {jx:.6} (rel {:.2e}, limit 5e-3), spread {spread:.2e} (limit 1e-9), force integral rel {:.2e} (limit 1e-9)",
        (j / jx - 1.0).abs(),
        ((fx - want) / want).abs()
    );
    assert!((j / jx - 1.0).abs() <= 5e-3);
    assert!(spread <= 1e-9);
    assert!(((fx - want) / want).abs() <= 1e-9);
    // open circuit: electrodes at the motional potential difference
    let dv = -2.0 * a * b0 * ubar;
    let (j2, _, _, _, _) = electrode_run(dv, 40);
    eprintln!(
        "M2 open circuit: |j|/(sigma B Ubar) {:.2e} (limit 5e-3)",
        j2.abs() / (sigma * b0 * ubar)
    );
    assert!(j2.abs() / (sigma * b0 * ubar) <= 5e-3);
}
