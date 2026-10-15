//! ADR 0026 ML1 and ML2: the mixing-length profiles and the model on a linear shear field.

mod common;

use common::sgs::{ctx, set_exact_bc};
use rcfd_fv::{FvGeometry, VectorField};
use rcfd_mesh::gen::box_2d;
use rcfd_turb::create_model;
use rcfd_turb::zeroeq::{mixing_length, nikuradse_l_over_r, MixingProfile};
use std::sync::Arc;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

#[test]
fn ml1_profiles() {
    assert!(rel(nikuradse_l_over_r(1.0), 0.14) <= 1e-12);
    assert!(rel(nikuradse_l_over_r(0.5), 0.11625) <= 1e-12);
    for xi in [0.1_f64, 0.37, 0.8] {
        let series = 0.4 * xi - 0.44 * xi.powi(2) + 0.24 * xi.powi(3) - 0.06 * xi.powi(4);
        assert!(rel(nikuradse_l_over_r(xi), series) <= 1e-12, "{xi}");
    }
    let xi = 1e-4;
    assert!((nikuradse_l_over_r(xi) / (0.4 * xi) - 1.0).abs() <= 1.2e-4);
    // beyond R the profile is clamped
    assert!(rel(nikuradse_l_over_r(2.5), 0.14) <= 1e-12);
    assert!(rel(mixing_length(MixingProfile::Linear, 0.41, 0.09, 1.0, 0.1), 0.041) <= 1e-12);
    assert!(rel(mixing_length(MixingProfile::Linear, 0.41, 0.09, 1.0, 0.5), 0.09) <= 1e-12);
    assert!(rel(mixing_length(MixingProfile::Nikuradse, 0.41, 0.09, 2.0, 2.0), 0.28) <= 1e-12);
}

#[test]
fn ml2_model_on_a_linear_shear_field() {
    let (nu, gamma) = (0.01, 4.0);
    let mesh = box_2d(8, 16, 1.0, 1.0, true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| [gamma * c[1], 0.0, 0.0]);
    set_exact_bc(&mut u, &geom, &["ymin", "ymax"], &|x| [gamma * x[1], 0.0, 0.0]);
    let cc = geom.mesh.cell_centres();
    let utau = (nu * gamma).sqrt();
    for (profile, damping, r) in [
        (MixingProfile::Linear, false, 1.0),
        (MixingProfile::Linear, true, 1.0),
        (MixingProfile::Nikuradse, false, 0.5),
        (MixingProfile::Nikuradse, true, 0.5),
    ] {
        let mut c = ctx(&geom, nu, &["ymin", "ymax"]);
        c.length_scale = r;
        c.mixing_profile = profile;
        c.wall_damping = damping;
        let mut m = create_model("mixing_length", &c).unwrap();
        m.update(&u, &[]);
        let mut worst = 0.0_f64;
        for i in 0..cc.len() {
            let d = cc[i][1].min(1.0 - cc[i][1]);
            let mut l = mixing_length(profile, 0.41, 0.09, r, d);
            if damping {
                let f = 1.0 - (-(d * utau / nu) / 26.0).exp();
                l = match profile {
                    MixingProfile::Linear => (0.41 * d * f).min(0.09 * r),
                    MixingProfile::Nikuradse => l * f,
                };
            }
            let want = l * l * gamma;
            worst = worst.max(rel(m.nut()[i], want));
        }
        println!("ML2 {profile:?} damping {damping}: max rel error {worst:.3e}");
        assert!(worst <= 1e-12, "{profile:?} {damping}: {worst}");
        assert_eq!(m.key(), "mixing_length");
        // wall faces carry nu only
        let nf = m.nu_face();
        for &f in &m.wall_faces() {
            assert_eq!(nf[f], nu);
        }
    }
}
