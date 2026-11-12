//! F-367 (ADR 0360) NF: fingerprints of the runs that the item must leave bit for bit. Written against the API of main 2ad9dcb9
//! only and committed, with the fingerprints it printed on that library, before any library edit of F-367; the final tree must
//! print the same. F1 to F4: the default on the periodic set-ups of ADR 0360 (PC, PN, PD, PJ); F5: PC with ADR 0223 E2 through
//! the older fields; F6, F7: ADR 0258's DN and DJ ducts with X1 and X3 through the value.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::lumped::FaceJump;
use rcfd_fv::pimple::{ConsistentCorrectors, PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, Symmetry, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d_graded, warp, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

const NU: f64 = 0.01;
const U0: f64 = 0.1;
const V0: f64 = 0.02;
const LP: f64 = 0.3;
const DT: f64 = 0.002;
const SHEAR: f64 = 0.5;
const R0: f64 = 0.2;

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

/// ADR 0211's `fb_settings` (copied).
fn fb_settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.momentum_solver = tight(5000);
    s.pressure_solver = tight(20000);
    s.amg_pressure = true;
    s
}

fn fnv(u: &[Vec3], p: &[f64], phi: &[f64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |x: f64| {
        for b in x.to_bits().to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for v in u {
        for x in v {
            eat(*x);
        }
    }
    for x in p.iter().chain(phi) {
        eat(*x);
    }
    h
}

/// The grid of ADR 0360: PC (`periodic`) or ADR 0258's duct, sheared or not.
fn geom(periodic: bool, sheared: bool) -> Arc<FvGeometry> {
    let g0 = box_2d_graded(
        20,
        12,
        LP,
        0.1,
        [Grading::Uniform, Grading::Symmetric(3.0)],
        periodic,
        false,
    )
    .unwrap();
    let mesh = if sheared {
        warp(&g0, |p| [p[0] + SHEAR * p[1], p[1], p[2]]).unwrap()
    } else {
        g0
    };
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

fn profile(geom: &FvGeometry, patch: &str) -> Profile {
    let fc = geom.mesh.face_centres();
    let p = geom.mesh.patches().iter().find(|p| p.name == patch).expect("patch");
    Profile::PerFace(
        p.faces()
            .map(|f| [U0, V0 * (2.0 * PI * fc[f][0] / LP).sin(), 0.0])
            .collect(),
    )
}

/// The jump of ADR 0258 DJ / ADR 0360 PJ on the internal faces of the plane `x - shear y = 0.15` below `y = 0.05`.
fn jump(geom: &FvGeometry, sheared: bool) -> FaceJump {
    let mesh = &geom.mesh;
    let (fc, sf) = (mesh.face_centres(), mesh.face_area_vectors());
    let shear = if sheared { SHEAR } else { 0.0 };
    let mut j = FaceJump::zeros(mesh.n_faces());
    let mut n = 0;
    for f in 0..geom.n_internal {
        let x = fc[f][0] - shear * fc[f][1];
        if (x - 0.15).abs() < 1e-9 && fc[f][1] < 0.05 {
            j.rise[f] = R0 * sf[f][0].signum();
            j.resist[f] = 0.2 * R0 / (U0 * vec3::norm(sf[f]));
            n += 1;
        }
    }
    assert_eq!(n, 6, "the jump faces");
    j
}

/// The periodic channel of ADR 0360: `moving_bottom` = PD's conditions (`ymin` the profile, `ymax` symmetry), else PC's (`ymin`
/// at rest, `ymax` the profile).
fn channel(sheared: bool, moving_bottom: bool, with_jump: bool, settings: PimpleSettings) -> PimpleSolver {
    let geom = geom(true, sheared);
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    if moving_bottom {
        u.set_bc(&geom, "ymin", Box::new(FixedValue(profile(&geom, "ymin"))))
            .unwrap();
        u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    } else {
        u.set_bc(&geom, "ymin", Box::new(FixedValue::vector(vec3::ZERO)))
            .unwrap();
        u.set_bc(&geom, "ymax", Box::new(FixedValue(profile(&geom, "ymax"))))
            .unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut flow = PimpleSolver::new(geom.clone(), u, p, settings);
    if with_jump {
        flow.face_jump = Some(jump(&geom, sheared));
    }
    flow
}

/// ADR 0258's duct (DN sheared, or DJ with its jump).
fn duct(sheared: bool, with_jump: bool, settings: PimpleSettings) -> PimpleSolver {
    let geom = geom(false, sheared);
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([U0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut flow = PimpleSolver::new(geom.clone(), u, p, settings);
    if with_jump {
        flow.face_jump = Some(jump(&geom, sheared));
    }
    flow
}

fn run(mut f: PimpleSolver) -> u64 {
    for _ in 0..20 {
        f.step(DT);
        assert!(f.diverged.is_none(), "stopped: {:?}", f.diverged);
    }
    fnv(&f.u.values, &f.p.values, &f.phi)
}

/// The fingerprints printed on main 2ad9dcb9 (before any library edit of F-367).
const EXPECTED: [(&str, u64); 7] = [
    ("F1 PC default", 0x20a96720b36e66f4),
    ("F2 PN default", 0x87b89a15150aae8d),
    ("F3 PD default", 0x192c8678593963e8),
    ("F4 PJ default", 0x7cab22bfe229e4c8),
    ("F5 PC with E2 (older fields)", 0x0d0c0f402413e6b6),
    ("F6 DN duct with X1 (value)", 0xd019fe264ea1caac),
    ("F7 DJ duct with X3 (value)", 0xb2c59bfdb3b53236),
];

/// ADR 0360 NF.
#[test]
fn nf_fingerprints_of_the_runs_left_bit_for_bit() {
    let non_orth = || {
        let mut s = fb_settings();
        s.non_orth_correctors = 2;
        s
    };
    let diag = || {
        let mut s = fb_settings();
        s.component_diagonal = true;
        s
    };
    let e2 = || {
        let mut s = fb_settings();
        s.consistent_correctors = true;
        s.consistent_periodic = true;
        s
    };
    let x1 = || {
        let mut s = non_orth();
        s.consistent = ConsistentCorrectors {
            enabled: true,
            non_orthogonal: true,
            ..Default::default()
        };
        s
    };
    let x3 = || {
        let mut s = fb_settings();
        s.consistent = ConsistentCorrectors {
            enabled: true,
            face_jump: true,
            ..Default::default()
        };
        s
    };
    let got = [
        run(channel(false, false, false, fb_settings())),
        run(channel(true, false, false, non_orth())),
        run(channel(false, true, false, diag())),
        run(channel(false, false, true, fb_settings())),
        run(channel(false, false, false, e2())),
        run(duct(true, false, x1())),
        run(duct(false, true, x3())),
    ];
    for ((name, _), have) in EXPECTED.iter().zip(got) {
        println!("NF {name}: {have:#018x}");
    }
    for ((name, want), have) in EXPECTED.iter().zip(got) {
        assert_eq!(have, *want, "NF {name}");
    }
}
