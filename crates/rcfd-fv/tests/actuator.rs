//! Checks A1 and A2 of ADR 0036: momentum-theory closed forms and the discrete invariants of the actuator-disk source.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::actuator::*;
use rcfd_fv::simple::BodyForce;
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{box_mesh, Grading};
use std::sync::Arc;

#[test]
fn a1_momentum_theory_closed_forms() {
    let (t, rho, a) = (100.0, 1.2, 2.0);
    for u0 in [0.0, 0.5, 2.0, 5.0, 20.0] {
        let ud = axial_disk_velocity(u0, t, rho, a);
        let uw = wake_velocity(u0, ud);
        let resid = (2.0 * rho * a * ud * (ud - u0) - t).abs() / t;
        assert!(resid <= 1e-13, "u0 = {u0}: {resid}");
        let m = rho * a * ud;
        let p = disk_power(t, ud);
        let p_energy = 0.5 * m * (uw * uw - u0 * u0);
        assert!((p - p_energy).abs() <= 1e-13 * p, "energy theorem u0 = {u0}");
        if u0 > 0.0 {
            let eta = propulsive_efficiency(u0, ud);
            assert!((eta - 2.0 * u0 / (u0 + uw)).abs() <= 1e-14, "efficiency u0 = {u0}");
        }
    }
    let vh = hover_induced_velocity(t, rho, a);
    let ud = axial_disk_velocity(0.0, t, rho, a);
    assert!((ud - vh).abs() <= 1e-14 * vh);
    assert!((wake_velocity(0.0, ud) - 2.0 * ud).abs() <= 1e-14 * ud);
    let p = disk_power(t, ud);
    assert!((p - t.powf(1.5) / (2.0 * rho * a).sqrt()).abs() <= 1e-13 * p);
    assert!((betz_cp(1.0 / 3.0) - 16.0 / 27.0).abs() <= 1e-14);
    let n = 1_000_000;
    let mut best = (0.0, 0.0);
    for i in 0..=n {
        let x = 0.5 * i as f64 / n as f64;
        let v = betz_cp(x);
        if v > best.0 {
            best = (v, x);
        }
    }
    assert!((best.1 - 1.0 / 3.0).abs() <= 1e-5, "argmax {}", best.1);
    assert!((betz_ct(0.5) - 1.0).abs() <= 1e-15);
}

fn setup() -> (Arc<FvGeometry>, Vec<bool>) {
    let mesh = box_mesh(
        [24, 24, 24],
        [1.0, 1.0, 1.0],
        [Grading::Expansion(2.0), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let zone: Vec<bool> = geom
        .mesh
        .cell_centres()
        .iter()
        .map(|c| (c[0] - 0.5).abs() <= 0.1 && (c[1] - 0.5).powi(2) + (c[2] - 0.5).powi(2) <= 0.09)
        .collect();
    (geom, zone)
}

#[test]
fn a2_discrete_invariants() {
    let (geom, zone) = setup();
    let n_zone = zone.iter().filter(|&&z| z).count();
    let vol = geom.mesh.cell_volumes();
    let cc = geom.mesh.cell_centres();
    let v_sel: f64 = (0..zone.len()).filter(|&c| zone[c]).map(|c| vol[c]).sum();
    let v_ideal = std::f64::consts::PI * 0.09 * 0.2;
    eprintln!(
        "A2: {n_zone} cells, selected volume {v_sel:.5e} vs pi R^2 t {v_ideal:.5e} ({:+.2} %) [reported]",
        100.0 * (v_sel / v_ideal - 1.0)
    );
    assert!(n_zone > 100);
    let centre: Vec3 = [0.5, 0.5, 0.5];
    let (t, q, rho) = (3.7, 0.7, 1.3);
    for profile in [DiskProfile::Uniform, DiskProfile::LinearR] {
        let disk = ActuatorDisk {
            thrust: t,
            direction: [1.0, 0.0, 0.0],
            profile,
            centre,
            axis: [1.0, 0.0, 0.0],
            torque: q,
            density: rho,
            smooth: false,
        };
        let mut bf = BodyForce::uniform(&geom, [0.0; 3]);
        disk.add_force(&geom, &zone, &mut bf).unwrap();
        let mut f = [0.0; 3];
        let mut tq = 0.0;
        for c in 0..zone.len() {
            let a = bf.cell[c];
            if !zone[c] {
                assert_eq!(a, [0.0; 3], "outside the zone");
            }
            let fc = vec3::scale(a, rho * vol[c]);
            f = vec3::add(f, fc);
            tq += vec3::dot(vec3::cross(vec3::sub(cc[c], centre), fc), [1.0, 0.0, 0.0]);
        }
        eprintln!("A2 {profile:?}: force {f:?}, torque {tq:.15}");
        assert!((f[0] - t).abs() <= 1e-12 * t, "axial");
        assert!(f[1].abs() <= 1e-12 * t && f[2].abs() <= 1e-12 * t, "transverse {f:?}");
        assert!(tq > 0.0 && (tq - q).abs() <= 1e-12 * q, "torque {tq}");
        if profile == DiskProfile::LinearR {
            // two cells in the same x layer
            let cells: Vec<usize> = (0..zone.len())
                .filter(|&c| zone[c] && (cc[c][0] - cc[(0..zone.len()).find(|&k| zone[k]).unwrap()][0]).abs() < 1e-12)
                .collect();
            let r = |c: usize| ((cc[c][1] - 0.5).powi(2) + (cc[c][2] - 0.5).powi(2)).sqrt();
            let (c1, c2) = (cells[0], cells[cells.len() / 2]);
            assert!(r(c1) > 0.0 && r(c2) > 0.0);
            let ratio = bf.cell[c1][0] / bf.cell[c2][0];
            assert!(
                (ratio - r(c1) / r(c2)).abs() <= 1e-13 * ratio.abs(),
                "ratio {ratio} vs {}",
                r(c1) / r(c2)
            );
        }
    }
    // no swirl: the force is purely axial
    let disk = ActuatorDisk {
        thrust: t,
        direction: [1.0, 0.0, 0.0],
        profile: DiskProfile::Uniform,
        centre,
        axis: [1.0, 0.0, 0.0],
        torque: 0.0,
        density: rho,
        smooth: false,
    };
    let f = disk.cell_forces(&geom, &zone).unwrap();
    assert!(f.iter().all(|v| v[1] == 0.0 && v[2] == 0.0));
}

/// A2b (added after the A4 diagnosis, ADR 0036 addendum): the smoothed distribution keeps the totals exactly and spreads the force
/// over at most one extra cell layer.
#[test]
fn a2b_smoothed_distribution_keeps_totals() {
    let (geom, zone) = setup();
    let vol = geom.mesh.cell_volumes();
    let cc = geom.mesh.cell_centres();
    let centre: Vec3 = [0.5, 0.5, 0.5];
    let (t, q, rho) = (3.7, 0.7, 1.3);
    for profile in [DiskProfile::Uniform, DiskProfile::LinearR] {
        let disk = ActuatorDisk {
            thrust: t,
            direction: [1.0, 0.0, 0.0],
            profile,
            centre,
            axis: [1.0, 0.0, 0.0],
            torque: q,
            density: rho,
            smooth: true,
        };
        let mut bf = BodyForce::uniform(&geom, [0.0; 3]);
        disk.add_force(&geom, &zone, &mut bf).unwrap();
        let mut f = [0.0; 3];
        let mut tq = 0.0;
        let mut outside = 0usize;
        for c in 0..zone.len() {
            let fc = vec3::scale(bf.cell[c], rho * vol[c]);
            f = vec3::add(f, fc);
            tq += vec3::dot(vec3::cross(vec3::sub(cc[c], centre), fc), [1.0, 0.0, 0.0]);
            if !zone[c] && vec3::norm(bf.cell[c]) > 0.0 {
                outside += 1;
            }
        }
        eprintln!("A2b {profile:?}: force {f:?}, torque {tq:.15}, {outside} cells outside the zone carry force");
        assert!((f[0] - t).abs() <= 1e-9 * t, "axial {f:?}");
        assert!((tq - q).abs() <= 1e-9 * q, "torque {tq}");
        assert!(outside > 0);
    }
}
