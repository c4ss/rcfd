//! F-253 (ADR 0192): the particle wall film. This first part holds the record R1 of ADR 0192: a tracker run without a
//! film, made on the base code (main 201ea17) before any code of F-253, whose fingerprint must stay the same.

use rcfd_core::hash::Fnv64;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_particles::*;
use std::sync::Arc;

/// ADR 0192 R1: FNV-1a 64 of the R1 scenario's fingerprint, coupling record and parcel temperatures on main 201ea17
/// (Windows; the analytic integrator and the heat step call `exp`, whose last bits may differ elsewhere, ADR 0137).
const R1: u64 = 0xe564_21d6_72c5_454f;

/// The R1 scenario: every extension the tracker has (two injections with Rosin-Rammler and uniform sizes, a cone and a
/// mass flow, dispersion, heat transfer, the exchange record) and every wall action (stick floor, escape ceiling,
/// reflecting walls with restitution 0.8), Schiller-Naumann drag and RK4 in a non-uniform flow, 150 steps of 0.01 s.
fn r1_hash() -> u64 {
    let mesh = Arc::new(box_mesh([6, 5, 4], [1.2, 1.0, 0.8], [Grading::Uniform; 3]).unwrap());
    let opts = Options {
        drag: DragLaw::SchillerNaumann,
        scheme: Scheme::Rk4,
        interp: Interp::Gradient,
        gravity: [0.0, 0.0, -9.81],
        default_wall: WallAction::Reflect(0.8),
        ..Options::default()
    };
    let overrides = [
        ("zmin".to_string(), WallAction::Stick),
        ("zmax".to_string(), WallAction::Escape),
    ];
    let mut tr = Tracker::new(mesh.clone(), opts, &overrides).unwrap();
    tr.add_injector(InjectorSpec {
        name: "a".into(),
        kind: InjKind::Point([0.3, 0.4, 0.5]),
        rate: 400.0,
        t_start: 0.0,
        t_end: 0.4,
        velocity: VelMode::Given([0.2, 0.0, 0.1]),
        dist: SizeDist::RosinRammler {
            d_bar: 1e-4,
            n: 2.5,
            d_min: Some(3e-5),
            d_max: Some(3e-4),
        },
        density: 1500.0,
        parcel_count: 10.0,
        seed: 11,
    })
    .unwrap();
    tr.add_injector(InjectorSpec {
        name: "b".into(),
        kind: InjKind::Surface("zmax".into()),
        rate: 150.0,
        t_start: 0.1,
        t_end: 0.5,
        velocity: VelMode::Fluid,
        dist: SizeDist::Uniform {
            d_min: 5e-5,
            d_max: 2e-4,
        },
        density: 1500.0,
        parcel_count: 1.0,
        seed: 12,
    })
    .unwrap();
    tr.set_injector_ext(
        "a",
        InjectorExt {
            cone: Some(ConeSpec {
                axis: [1.0, 0.2, 0.3],
                half_angle: 0.4,
                inner_half_angle: 0.1,
                speed: 0.5,
            }),
            mass_flow: Some(2e-6),
            temperature: Some(320.0),
        },
    )
    .unwrap();
    let n = mesh.n_cells();
    tr.set_dispersion(Dispersion::default(), vec![0.02; n], vec![0.05; n], 99).unwrap();
    tr.set_thermal(Thermal {
        cp_p: 900.0,
        k_f: 0.026,
        cp_f: 1005.0,
        t_fluid: mesh.cell_centres().iter().map(|c| 300.0 + 10.0 * c[0]).collect(),
    })
    .unwrap();
    tr.enable_coupling();
    let u: Vec<[f64; 3]> = mesh
        .cell_centres()
        .iter()
        .map(|c| [0.3 + 0.2 * c[1], -0.1 * c[0], 0.05 * c[2]])
        .collect();
    let grad = vec![[[0.0, 0.2, 0.0], [-0.1, 0.0, 0.0], [0.0, 0.0, 0.05]]; n];
    let flow = Flow {
        u: &u,
        grad: Some(&grad),
        rho: 1.2,
        mu: 1.8e-5,
    };
    for _ in 0..150 {
        tr.advance(&flow, 0.01).unwrap();
    }
    let counts = [State::Active, State::Deposited, State::Escaped, State::Stuck].map(|s| tr.count(s));
    println!("R1 set-up: injected {}, active/deposited/escaped/stuck {counts:?}", tr.injected());
    assert!(counts[0] > 0 && counts[1] > 0 && counts[2] > 0, "R1 set-up: {counts:?}");
    assert!(c_wall(&tr) > 0.0, "R1 set-up: no reflection");
    let mut h = Fnv64::new();
    for b in tr.fingerprint() {
        h.write_u64(b);
    }
    let c = tr.coupling().unwrap();
    for (i, e) in c.impulse.iter().enumerate() {
        for x in e {
            h.write_f64(*x);
        }
        h.write_f64(c.energy_mech[i]);
        h.write_f64(c.energy_heat[i]);
    }
    for x in c.wall_impulse {
        h.write_f64(x);
    }
    for i in 0..tr.particles.len() {
        h.write_f64(tr.temperature(i));
    }
    h.finish()
}

/// Size of the momentum the walls gave by reflection (zero when no parcel reflected).
fn c_wall(tr: &Tracker) -> f64 {
    let w = tr.coupling().unwrap().wall_impulse;
    w.iter().map(|x| x.abs()).sum()
}

/// ADR 0192 R1 (check D1): a tracker without a film gives the fingerprint recorded on the base code. Compared on Windows,
/// printed elsewhere (ADR 0137). `RCFD_F253_RECORD=1` prints the value only.
#[test]
fn d1_no_film_tracker_matches_the_base_record() {
    let h = r1_hash();
    println!("R1 {h:016x}");
    if std::env::var("RCFD_F253_RECORD").as_deref() == Ok("1") || !cfg!(windows) {
        return;
    }
    assert_eq!(h, R1, "R1 {h:016x} differs from the base record {R1:016x}");
}
