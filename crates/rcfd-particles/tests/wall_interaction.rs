//! F-254 (ADR 0194): particle wall interaction, erosion and rough-wall rebound, in the library. This file holds the
//! record R3 of a tracker run with the wall film of ADR 0192 (made on the base code main 54ec840 before any code of
//! F-254).

use rcfd_core::hash::Fnv64;
use rcfd_core::vec3::Vec3;
use rcfd_mesh::edit::{transform_mesh, Affine};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_particles::*;
use std::sync::Arc;

/// ADR 0194 R3: FNV-1a 64 of the R3 scenario's fingerprint on main 54ec840 (Windows; the analytic integrator calls `exp`,
/// whose last bits may differ elsewhere, ADR 0137).
const R3: u64 = 0xfe94_f34a_c9fc_5dbd;

/// The incline of ADR 0192 geometry G: `box_mesh([10, 4, 2], [0.05, 0.02, 0.01])` rotated by 30 degrees about y.
fn incline() -> Arc<Mesh> {
    let m = box_mesh([10, 4, 2], [0.05, 0.02, 0.01], [Grading::Uniform; 3]).unwrap();
    let a = Affine::rotation([0.0, 1.0, 0.0], 30f64.to_radians(), [0.0; 3]).unwrap();
    Arc::new(transform_mesh(&m, &a).unwrap())
}

/// Local (box) coordinates to world.
fn to_world(l: Vec3) -> Vec3 {
    let (s, c) = 30f64.to_radians().sin_cos();
    [c * l[0] + s * l[2], l[1], -s * l[0] + c * l[2]]
}

/// The R3 scenario: the wall film of ADR 0192 on the incline `zmin`, fed by a falling stream, and a second stream thrown
/// sideways at the reflecting walls (restitution 0.7, `xmin` sticks) before it lands on the film; Schiller-Naumann drag,
/// RK2, 300 steps of 0.01 s.
fn r3_hash() -> u64 {
    let mesh = incline();
    let opts = Options {
        drag: DragLaw::SchillerNaumann,
        scheme: Scheme::Rk2,
        interp: Interp::CellCentre,
        gravity: [0.0, 0.0, -9.81],
        default_wall: WallAction::Reflect(0.7),
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh.clone(), opts, &[("xmin".to_string(), WallAction::Stick)]).unwrap();
    tr.set_film(FilmSpec {
        patches: vec!["zmin".into()],
        density: 1000.0,
        viscosity: 1e-3,
        gravity: [0.0, 0.0, -9.81],
    })
    .unwrap();
    tr.add_injector(InjectorSpec {
        name: "fall".into(),
        kind: InjKind::Point(to_world([1.3e-3, 7.5e-3, 2e-3])),
        rate: 730.0,
        t_start: 0.0,
        t_end: 3.0,
        velocity: VelMode::Given([0.0; 3]),
        dist: SizeDist::Constant(1e-4),
        density: 1000.0,
        parcel_count: 1.0,
        seed: 3,
    })
    .unwrap();
    tr.add_injector(InjectorSpec {
        name: "throw".into(),
        kind: InjKind::Point(to_world([2e-2, 1e-2, 6e-3])),
        rate: 300.0,
        t_start: 0.1,
        t_end: 2.0,
        velocity: VelMode::Given(to_world([-1.5, 0.6, 0.3])),
        dist: SizeDist::Uniform {
            d_min: 5e-5,
            d_max: 2e-4,
        },
        density: 1000.0,
        parcel_count: 2.0,
        seed: 4,
    })
    .unwrap();
    let n = mesh.n_cells();
    let u: Vec<Vec3> = mesh
        .cell_centres()
        .iter()
        .map(|c| [0.05 + 2.0 * c[2], 0.0, 0.0])
        .collect();
    let flow = Flow {
        u: &u,
        grad: None,
        rho: 1.2,
        mu: 1.8e-5,
    };
    assert_eq!(u.len(), n);
    for _ in 0..300 {
        tr.advance(&flow, 0.01).unwrap();
    }
    let counts = [
        State::Active,
        State::Deposited,
        State::Escaped,
        State::Stuck,
        State::Film,
    ]
    .map(|s| tr.count(s));
    println!(
        "R3 set-up: injected {}, active/deposited/escaped/stuck/film {counts:?}, film held {:e}, drained {:e}",
        tr.injected(),
        tr.film().unwrap().held(),
        tr.film().unwrap().drained()
    );
    assert!(counts[4] > 0 && counts[1] > 0, "R3 set-up: {counts:?}");
    let mut h = Fnv64::new();
    for b in tr.fingerprint() {
        h.write_u64(b);
    }
    h.finish()
}

/// ADR 0194 R3 (check E1): a tracker with the wall film and no F-254 model gives the fingerprint recorded on the base
/// code. Compared on Windows, printed elsewhere (ADR 0137). `RCFD_F254_RECORD=1` prints the value only.
#[test]
fn e1_film_tracker_matches_the_base_record() {
    let h = r3_hash();
    println!("R3 {h:016x}");
    if std::env::var("RCFD_F254_RECORD").as_deref() == Ok("1") || !cfg!(windows) {
        return;
    }
    assert_eq!(h, R3, "R3 {h:016x} differs from the base record {R3:016x}");
}
