//! F-253 (ADR 0192): the particle wall film. Registered checks A (the model), B (the acceptance case) and D1 (the record R1
//! of a tracker run without a film, made on the base code main 201ea17 before any code of F-253). The limits are those of
//! ADR 0192, fixed before this code ran.

use rcfd_core::hash::Fnv64;
use rcfd_core::vec3::{dot, norm, Vec3};
use rcfd_mesh::edit::{transform_mesh, Affine};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_particles::*;
use std::sync::Arc;

/// Geometry G of ADR 0192: incline angle [deg], liquid, design flow, column width and row length, gravity.
const THETA_DEG: f64 = 30.0;
const RHO_L: f64 = 1000.0;
const MU_L: f64 = 1e-3;
const GAMMA: f64 = 5e-4;
const W_COL: f64 = 5e-3;
const DX: f64 = 5e-3;
const G: f64 = 9.81;
/// Station row of B (local x centre 32.5 mm).
const STATION: usize = 6;

fn theta() -> f64 {
    THETA_DEG.to_radians()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// Local (box) coordinates to world: rotation by `theta` about y (right-handed), so local x goes down the slope.
fn to_world(l: Vec3) -> Vec3 {
    let (s, c) = theta().sin_cos();
    [c * l[0] + s * l[2], l[1], -s * l[0] + c * l[2]]
}

fn to_local(w: Vec3) -> Vec3 {
    let (s, c) = theta().sin_cos();
    [c * w[0] - s * w[2], w[1], s * w[0] + c * w[2]]
}

/// Geometry G: `box_mesh([10, 4, 2], [0.05, 0.02, 0.01])` rotated by 30 degrees about the y axis through the origin.
fn incline() -> Arc<Mesh> {
    let m = box_mesh([10, 4, 2], [0.05, 0.02, 0.01], [Grading::Uniform; 3]).unwrap();
    let a = Affine::rotation([0.0, 1.0, 0.0], theta(), [0.0; 3]).unwrap();
    Arc::new(transform_mesh(&m, &a).unwrap())
}

fn film_spec() -> FilmSpec {
    FilmSpec {
        patches: vec!["zmin".into()],
        density: RHO_L,
        viscosity: MU_L,
        gravity: [0.0, 0.0, -G],
    }
}

/// (row, column) of every film face, from the local coordinates of its centre.
fn rows_cols(film: &Film) -> Vec<(usize, usize)> {
    (0..film.len())
        .map(|k| {
            let l = to_local(film.centre(k));
            ((l[0] / DX).floor() as usize, (l[1] / W_COL).floor() as usize)
        })
        .collect()
}

/// The reference of the acceptance, `(3 mu Gamma / (rho^2 g sin(theta)))^(1/3)`.
fn h_acc(gamma: f64) -> f64 {
    (3.0 * MU_L * gamma / (RHO_L * RHO_L * G * theta().sin())).cbrt()
}

/// A1: the closed forms, the tangential gravity of the faces and the direction of the film velocity.
#[test]
fn a1_closed_forms_and_tangential_gravity() {
    let mesh = incline();
    let film = Film::new(&mesh, &film_spec()).unwrap();
    let gt = G * theta().sin();
    let h = nusselt_thickness(RHO_L, MU_L, GAMMA, gt);
    let u = film_velocity(RHO_L, MU_L, h, [gt, 0.0, 0.0]);
    let e_gamma = rel(RHO_L * u[0] * h, GAMMA);
    let down = to_world([1.0, 0.0, 0.0]);
    let across = to_world([0.0, 1.0, 0.0]);
    let (mut e_gt, mut e_across) = (0.0f64, 0.0f64);
    for k in 0..film.len() {
        let g = film.g_t(k);
        e_gt = e_gt.max(rel(norm(g), gt));
        e_across = e_across.max(dot(g, across).abs() / norm(g));
        assert!(dot(g, down) > 0.0, "face {k}: g_t does not point down the slope");
    }
    println!("A1: h_N {h:e}, Gamma round trip {e_gamma:e}, |g_t| {e_gt:e}, across-slope part {e_across:e}");
    assert_eq!(film.len(), 40);
    assert!(e_gamma <= 1e-14 && e_gt <= 1e-14 && e_across <= 1e-14);
}

/// A2: with every face at the Nusselt mass and no source, one step leaves rows 1 to 9 unchanged and moves `Phi dt` out of
/// row 0.
#[test]
fn a2_residual_at_the_nusselt_state() {
    let mesh = incline();
    let mut film = Film::new(&mesh, &film_spec()).unwrap();
    let gt = G * theta().sin();
    let h = nusselt_thickness(RHO_L, MU_L, GAMMA, gt);
    for k in 0..film.len() {
        let m = RHO_L * h * film.area(k);
        film.add_mass(k, m);
    }
    let m0 = film.masses().to_vec();
    let total = film.held();
    let dt = 0.01;
    film.advance(dt);
    let rc = rows_cols(&film);
    let (mut e_rows, mut e_row0) = (0.0f64, 0.0f64);
    for k in 0..film.len() {
        if rc[k].0 >= 1 {
            e_rows = e_rows.max(rel(film.masses()[k], m0[k]));
        } else {
            let ubar = RHO_L * h * h * gt / (3.0 * MU_L);
            e_row0 = e_row0.max(rel(m0[k] - film.masses()[k], RHO_L * h * ubar * W_COL * dt));
        }
    }
    let e_total = rel(film.held() + film.drained(), total);
    println!(
        "A2: rows 1-9 {e_rows:e}, row 0 loss {e_row0:e}, held + drained {e_total:e}, sub-steps {}",
        film.substeps()
    );
    assert_eq!(film.substeps(), 1);
    assert!(e_rows <= 1e-13 && e_row0 <= 1e-13 && e_total <= 1e-14);
}

/// A3: from an empty film with a constant feed into row 0, rows 1 to 9 reach the Nusselt thickness at 20 s.
#[test]
fn a3_approach_to_the_nusselt_thickness() {
    let mesh = incline();
    let mut film = Film::new(&mesh, &film_spec()).unwrap();
    let rc = rows_cols(&film);
    let feed: Vec<usize> = (0..film.len()).filter(|&k| rc[k].0 == 0).collect();
    assert_eq!(feed.len(), 4);
    let (s, dt) = (GAMMA * W_COL, 0.01);
    for _ in 0..2000 {
        for &k in &feed {
            film.add_mass(k, s * dt);
        }
        film.advance(dt);
    }
    let h = h_acc(GAMMA);
    let mut worst = (0.0f64, 0);
    for k in 0..film.len() {
        if rc[k].0 >= 1 {
            let e = rel(film.thickness(k), h);
            if e > worst.0 {
                worst = (e, rc[k].0);
            }
        }
    }
    let e_ledger = rel(film.held() + film.drained(), film.received());
    println!(
        "A3: worst rows 1-9 {:e} (row {}), ledger {e_ledger:e}, min mass {:e}, sub-steps {}",
        worst.0,
        worst.1,
        film.min_mass(),
        film.substeps()
    );
    assert!(worst.0 <= 1e-10 && e_ledger <= 1e-12);
    assert!(film.min_mass() >= 0.0);
    assert_eq!(film.substeps(), 2000);
}

/// Measurements of the acceptance case B.
struct BRun {
    /// Window average and final thickness of each station face.
    station: Vec<(f64, f64)>,
    /// Worst ledger residual (relative to the injected mass) over all steps.
    ledger: f64,
    /// Worst absorbed against held + drained (relative) over all steps.
    film_ledger: f64,
    /// Largest face Reynolds number at the end.
    max_re: f64,
    /// Parcels deposited, escaped or stuck at the end.
    lost: usize,
    min_mass: f64,
    injected: usize,
    absorbed: usize,
}

fn parcel_mass(p: &Particle) -> f64 {
    p.parcel * p.rho * std::f64::consts::PI * p.d.powi(3) / 6.0
}

/// The acceptance case B of ADR 0192: four point injectors feed the film of the incline for 20 s.
fn b_run() -> BRun {
    let mesh = incline();
    let opts = Options {
        drag: DragLaw::None,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0, 0.0, -G],
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh.clone(), opts, &[]).unwrap();
    tr.set_film(film_spec()).unwrap();
    let x_inj = 2.5e-3 - 2e-3 * theta().tan();
    for j in 0..4 {
        let name = format!("c{j}");
        tr.add_injector(InjectorSpec {
            name: name.clone(),
            kind: InjKind::Point(to_world([x_inj, (j as f64 + 0.5) * W_COL, 2e-3])),
            rate: 730.0,
            t_start: 0.0,
            t_end: 20.0,
            velocity: VelMode::Given([0.0; 3]),
            dist: SizeDist::Constant(1e-4),
            density: RHO_L,
            parcel_count: 1.0,
            seed: j as u64,
        })
        .unwrap();
        tr.set_injector_ext(
            &name,
            InjectorExt {
                mass_flow: Some(GAMMA * W_COL),
                ..InjectorExt::default()
            },
        )
        .unwrap();
    }
    let u = vec![[0.0; 3]; mesh.n_cells()];
    let flow = Flow {
        u: &u,
        grad: None,
        rho: 1.2,
        mu: 1.8e-5,
    };
    let rc = rows_cols(tr.film().unwrap());
    let station: Vec<usize> = (0..rc.len()).filter(|&k| rc[k].0 == STATION).collect();
    assert_eq!(station.len(), 4);
    let mut sum = vec![0.0; station.len()];
    let (mut ledger, mut film_ledger) = (0.0f64, 0.0f64);
    let steps = 2000;
    for step in 0..steps {
        tr.advance(&flow, 0.01).unwrap();
        let film = tr.film().unwrap();
        let mut by_state = [0.0f64; 5];
        for p in &tr.particles {
            by_state[p.state as usize] += parcel_mass(p);
        }
        let injected: f64 = by_state.iter().sum();
        if injected > 0.0 {
            let found = by_state[0] + by_state[1] + by_state[2] + by_state[3] + film.held() + film.drained();
            ledger = ledger.max(rel(found, injected));
        }
        if by_state[4] > 0.0 {
            film_ledger = film_ledger.max(rel(film.held() + film.drained(), by_state[4]));
        }
        if step >= steps - 200 {
            for (s, &k) in sum.iter_mut().zip(&station) {
                *s += film.thickness(k);
            }
        }
    }
    let film = tr.film().unwrap();
    BRun {
        station: station
            .iter()
            .zip(&sum)
            .map(|(&k, s)| (s / 200.0, film.thickness(k)))
            .collect(),
        ledger,
        film_ledger,
        max_re: film.max_reynolds(),
        lost: tr.count(State::Deposited) + tr.count(State::Escaped) + tr.count(State::Stuck),
        min_mass: film.min_mass(),
        injected: tr.injected(),
        absorbed: tr.count(State::Film),
    }
}

/// B1, B3, B5, B6 (the acceptance): the station thickness within 5 percent of the Nusselt thickness, the mass ledger to
/// 1e-10, the film Reynolds number in the laminar range, no parcel lost elsewhere, no negative film mass.
#[test]
fn b1_acceptance_parcel_fed_film_on_an_incline() {
    let r = b_run();
    // Gamma = injected mass flow of a column / column width
    let mdot = GAMMA * W_COL;
    let h = h_acc(mdot / W_COL);
    let re = 4.0 * GAMMA / MU_L;
    let re_c = (10.0 / 3.0) / theta().tan();
    println!(
        "B: h_acc {h:e}, injected {}, absorbed {}, lost {}",
        r.injected, r.absorbed, r.lost
    );
    for (i, (avg, last)) in r.station.iter().enumerate() {
        println!(
            "B1 station face {i}: window average {:e}, at 20 s {:e} (relative)",
            rel(*avg, h),
            rel(*last, h)
        );
        assert!(rel(*avg, h) <= 0.05 && rel(*last, h) <= 0.05, "B1 station face {i}");
    }
    println!(
        "B3 ledger {:e}; B5 Re_f {re} (laminar range (0, 30], long-wave threshold {re_c:.4}), largest face Re_f {:.4}; B6 min mass {:e}",
        r.ledger, r.max_re, r.min_mass
    );
    assert!(r.ledger <= 1e-10, "B3");
    assert!(re > 0.0 && re <= 30.0 && re < re_c && r.max_re <= 30.0, "B5");
    assert!(r.lost == 0 && r.min_mass >= 0.0, "B6");
}

/// B2, B4 (consistency): the window average of the station within 1e-6 of the Nusselt thickness, and the absorbed mass
/// equal to held + drained to 1e-10.
#[test]
fn b2_consistency_of_the_parcel_fed_film() {
    let r = b_run();
    let h = h_acc(GAMMA);
    let worst = r.station.iter().map(|(a, _)| rel(*a, h)).fold(0.0, f64::max);
    println!(
        "B2 worst window average {worst:e}; B4 absorbed vs held + drained {:e}",
        r.film_ledger
    );
    assert!(worst <= 1e-6, "B2");
    assert!(r.film_ledger <= 1e-10, "B4");
}

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
    tr.set_dispersion(Dispersion::default(), vec![0.02; n], vec![0.05; n], 99)
        .unwrap();
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
    println!(
        "R1 set-up: injected {}, active/deposited/escaped/stuck {counts:?}",
        tr.injected()
    );
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
