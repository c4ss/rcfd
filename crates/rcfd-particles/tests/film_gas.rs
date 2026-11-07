//! F-292 (ADR 0206): gas buoyancy and gas shear in the particle wall film. Registered checks A (gas buoyancy: GB1 to GB5,
//! GBN), B (gas shear: GS1 to GS6, GSN) and C (both, and the tracker: GC1, GC2, GT1, GT2, GD1). The limits are those of
//! ADR 0206, fixed before this code ran.

use rcfd_core::vec3::{norm, scale, Vec3};
use rcfd_mesh::edit::{transform_mesh, Affine};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_particles::*;
use std::sync::Arc;

/// The data of ADR 0206: incline angle [deg] of geometry G, liquid, design flow, column width and row length, gravity.
const THETA_DEG: f64 = 30.0;
const RHO_L: f64 = 1000.0;
const MU_L: f64 = 1e-3;
const GAMMA: f64 = 5e-4;
const W_COL: f64 = 5e-3;
const DX: f64 = 5e-3;
const G: f64 = 9.81;
/// Station row (local x centre 32.5 mm).
const STATION: usize = 6;
/// The Couette field of geometry H: shear rate [1/s] and gas viscosity [Pa s], so `tau = 0.1` Pa.
const S_RATE: f64 = 5555.555555555556;
const MU_G: f64 = 1.8e-5;
const TAU: f64 = 0.1;
/// The dense gas of A and the air of A and B [kg/m^3].
const RHO_DENSE: f64 = 250.0;
const RHO_AIR: f64 = 1.2;

fn theta() -> f64 {
    THETA_DEG.to_radians()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// Largest component difference of two vectors over the size of the reference.
fn rel_v(a: Vec3, b: Vec3) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f64::max) / norm(b)
}

/// The two geometries of ADR 0206: G, the incline of ADR 0192, and H, the same box not rotated (a floor).
#[derive(Clone, Copy, PartialEq)]
enum Geom {
    G,
    H,
}

/// Local (box) coordinates to world: for G a rotation by `theta` about y (local x goes down the slope); H is not rotated.
fn to_world(geom: Geom, l: Vec3) -> Vec3 {
    if geom == Geom::H {
        return l;
    }
    let (s, c) = theta().sin_cos();
    [c * l[0] + s * l[2], l[1], -s * l[0] + c * l[2]]
}

fn to_local(geom: Geom, w: Vec3) -> Vec3 {
    if geom == Geom::H {
        return w;
    }
    let (s, c) = theta().sin_cos();
    [c * w[0] - s * w[2], w[1], s * w[0] + c * w[2]]
}

fn mesh(geom: Geom) -> Arc<Mesh> {
    let m = box_mesh([10, 4, 2], [0.05, 0.02, 0.01], [Grading::Uniform; 3]).unwrap();
    if geom == Geom::H {
        return Arc::new(m);
    }
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
fn rows_cols(geom: Geom, film: &Film) -> Vec<(usize, usize)> {
    (0..film.len())
        .map(|k| {
            let l = to_local(geom, film.centre(k));
            ((l[0] / DX).floor() as usize, (l[1] / W_COL).floor() as usize)
        })
        .collect()
}

/// Unit vector down the slope of G.
fn down() -> Vec3 {
    to_world(Geom::G, [1.0, 0.0, 0.0])
}

fn g_t() -> f64 {
    G * theta().sin()
}

fn h_n() -> f64 {
    nusselt_thickness(RHO_L, MU_L, GAMMA, g_t())
}

fn h_b(rho_g: f64) -> f64 {
    buoyant_thickness(RHO_L, rho_g, MU_L, GAMMA, g_t())
}

fn h_s() -> f64 {
    shear_thickness(RHO_L, MU_L, GAMMA, TAU)
}

/// The Couette field of a geometry: H `(s z, 0, 0)`; G `s z_l t` along the slope (`z_l` the local height above `zmin`).
fn couette(geom: Geom, m: &Mesh) -> Vec<Vec3> {
    m.cell_centres()
        .iter()
        .map(|c| {
            let z = to_local(geom, *c)[2];
            match geom {
                Geom::H => [S_RATE * z, 0.0, 0.0],
                Geom::G => scale(down(), S_RATE * z),
            }
        })
        .collect()
}

fn film_with(geom: Geom, gas: FilmGas) -> Film {
    let mut f = Film::new(&mesh(geom), &film_spec()).unwrap();
    f.set_gas(gas);
    f
}

const BUOYANCY: FilmGas = FilmGas {
    buoyancy: true,
    shear: false,
};
const SHEAR: FilmGas = FilmGas {
    buoyancy: false,
    shear: true,
};
const BOTH: FilmGas = FilmGas {
    buoyancy: true,
    shear: true,
};

/// GB1: the closed forms of gas buoyancy.
#[test]
fn gb1_buoyancy_closed_forms() {
    let (mut e_gamma, mut e_ratio) = (0.0f64, 0.0f64);
    for rho_g in [RHO_DENSE, RHO_AIR] {
        let h = h_b(rho_g);
        let u = film_velocity(RHO_L - rho_g, MU_L, h, [g_t(), 0.0, 0.0]);
        e_gamma = e_gamma.max(rel(RHO_L * u[0] * h, GAMMA));
        e_ratio = e_ratio.max(rel(h / h_n(), (1.0 - rho_g / RHO_L).powf(-1.0 / 3.0)));
        println!("GB1 rho_g {rho_g}: h_B {h:e}, h_B/h_N {:.6}", h / h_n());
    }
    let same = buoyant_thickness(RHO_L, 0.0, MU_L, GAMMA, g_t()).to_bits() == h_n().to_bits();
    println!("GB1: Gamma round trip {e_gamma:e}, ratio {e_ratio:e}, rho_g = 0 equals h_N bit for bit: {same}");
    assert!(e_gamma <= 1e-14 && e_ratio <= 1e-14 && same);
}

/// One step of a film with every face at the mass `rho_l h A`: (rows 1 to 9 worst change, row-0 loss against
/// `Gamma W dt`, held + drained against the initial mass, sub-steps, opposed).
fn residual(geom: Geom, mut film: Film, h: f64) -> (f64, f64, f64, usize, u64) {
    for k in 0..film.len() {
        let m = RHO_L * h * film.area(k);
        film.add_mass(k, m);
    }
    let m0 = film.masses().to_vec();
    let total = film.held();
    let dt = 0.01;
    film.advance(dt);
    let rc = rows_cols(geom, &film);
    let (mut e_rows, mut e_row0) = (0.0f64, 0.0f64);
    for k in 0..film.len() {
        if rc[k].0 >= 1 {
            e_rows = e_rows.max(rel(film.masses()[k], m0[k]));
        } else {
            e_row0 = e_row0.max(rel(m0[k] - film.masses()[k], GAMMA * W_COL * dt));
        }
    }
    let e_total = rel(film.held() + film.drained(), total);
    (e_rows, e_row0, e_total, film.substeps(), film.opposed())
}

/// GB2: at the buoyant thickness and with no source, one step moves `Gamma W dt` out of row 0 and nothing else.
#[test]
fn gb2_buoyancy_residual() {
    let mut film = film_with(Geom::G, BUOYANCY);
    film.set_gas_state(RHO_DENSE, &[]).unwrap();
    let (e_rows, e_row0, e_total, sub, _) = residual(Geom::G, film, h_b(RHO_DENSE));
    println!("GB2: rows 1-9 {e_rows:e}, row 0 loss {e_row0:e}, held + drained {e_total:e}, sub-steps {sub}");
    assert_eq!(sub, 1);
    assert!(e_rows <= 1e-13 && e_row0 <= 1e-13 && e_total <= 1e-14);
}

/// From an empty film with a constant feed into row 0 for `steps` steps: (worst of rows 1 to 9 against `h`, its row,
/// ledger, smallest mass, sub-steps).
fn approach(geom: Geom, mut film: Film, steps: usize, h: f64) -> (f64, usize, f64, f64, usize) {
    let rc = rows_cols(geom, &film);
    let feed: Vec<usize> = (0..film.len()).filter(|&k| rc[k].0 == 0).collect();
    assert_eq!(feed.len(), 4);
    let (s, dt) = (GAMMA * W_COL, 0.01);
    for _ in 0..steps {
        for &k in &feed {
            film.add_mass(k, s * dt);
        }
        film.advance(dt);
    }
    let mut worst = (0.0f64, 0);
    for k in 0..film.len() {
        if rc[k].0 >= 1 {
            let e = rel(film.thickness(k), h);
            if e > worst.0 {
                worst = (e, rc[k].0);
            }
        }
    }
    let ledger = rel(film.held() + film.drained(), film.received());
    (worst.0, worst.1, ledger, film.min_mass(), film.substeps())
}

/// GB3: from an empty film with a constant feed, rows 1 to 9 reach the buoyant thickness at 20 s.
#[test]
fn gb3_buoyancy_approach() {
    let mut film = film_with(Geom::G, BUOYANCY);
    film.set_gas_state(RHO_DENSE, &[]).unwrap();
    let (worst, row, ledger, min, sub) = approach(Geom::G, film, 2000, h_b(RHO_DENSE));
    println!("GB3: worst rows 1-9 {worst:e} (row {row}), ledger {ledger:e}, min mass {min:e}, sub-steps {sub}");
    assert!(worst <= 1e-10 && ledger <= 1e-12);
    assert!(min >= 0.0);
    assert_eq!(sub, 2000);
}

/// Measurements of a parcel-fed run.
struct Fed {
    /// Window average and final thickness of each station face.
    station: Vec<(f64, f64)>,
    /// Final mass of each station face.
    station_mass: Vec<f64>,
    /// Worst ledger residual (relative to the injected mass) over all steps.
    ledger: f64,
    /// Worst absorbed against held + drained (relative) over all steps.
    film_ledger: f64,
    max_re: f64,
    /// Parcels deposited, escaped or stuck at the end.
    lost: usize,
    min_mass: f64,
    opposed: u64,
    injected: usize,
    absorbed: usize,
    fingerprint: Vec<u64>,
}

fn parcel_mass(p: &Particle) -> f64 {
    p.parcel * p.rho * std::f64::consts::PI * p.d.powi(3) / 6.0
}

/// A parcel-fed film: the case B of ADR 0192 on G (carrier at rest) or the floor of GS5 on H (the Couette field), four
/// point injectors from 0 to `t_end`, `steps` steps of 0.01 s, the window the last 200 steps. `gas` = `None` makes no
/// `set_film_gas` call.
fn fed_run(geom: Geom, gas: Option<FilmGas>, rho_g: f64, steps: usize, t_end: f64) -> Fed {
    let m = mesh(geom);
    let opts = Options {
        drag: DragLaw::None,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0, 0.0, -G],
        ..Options::default()
    };
    let mut tr = Tracker::new(m.clone(), opts, &[]).unwrap();
    tr.set_film(film_spec()).unwrap();
    if let Some(g) = gas {
        tr.set_film_gas(g).unwrap();
    }
    let x_inj = match geom {
        Geom::G => 2.5e-3 - 2e-3 * theta().tan(),
        Geom::H => 2.5e-3,
    };
    for j in 0..4 {
        let name = format!("c{j}");
        tr.add_injector(InjectorSpec {
            name: name.clone(),
            kind: InjKind::Point(to_world(geom, [x_inj, (j as f64 + 0.5) * W_COL, 2e-3])),
            rate: 730.0,
            t_start: 0.0,
            t_end,
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
    let u = match geom {
        Geom::G => vec![[0.0; 3]; m.n_cells()],
        Geom::H => couette(Geom::H, &m),
    };
    let flow = Flow {
        u: &u,
        grad: None,
        rho: rho_g,
        mu: MU_G,
    };
    let rc = rows_cols(geom, tr.film().unwrap());
    let station: Vec<usize> = (0..rc.len()).filter(|&k| rc[k].0 == STATION).collect();
    assert_eq!(station.len(), 4);
    let mut sum = vec![0.0; station.len()];
    let (mut ledger, mut film_ledger) = (0.0f64, 0.0f64);
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
    Fed {
        station: station
            .iter()
            .zip(&sum)
            .map(|(&k, s)| (s / 200.0, film.thickness(k)))
            .collect(),
        station_mass: station.iter().map(|&k| film.masses()[k]).collect(),
        ledger,
        film_ledger,
        max_re: film.max_reynolds(),
        lost: tr.count(State::Deposited) + tr.count(State::Escaped) + tr.count(State::Stuck),
        min_mass: film.min_mass(),
        opposed: film.opposed(),
        injected: tr.injected(),
        absorbed: tr.count(State::Film),
        fingerprint: tr.fingerprint(),
    }
}

/// GB4 (the acceptance): the parcel-fed incline under a dense gas reaches the buoyant thickness within 5 percent; the
/// ledger, the parcels, the film mass and `Re_f` as ADR 0192 B.
#[test]
fn gb4_acceptance_buoyant_film_on_an_incline() {
    let r = fed_run(Geom::G, Some(BUOYANCY), RHO_DENSE, 2000, 20.0);
    let h = h_b(RHO_DENSE);
    println!(
        "GB4: h_B {h:e}, injected {}, absorbed {}, lost {}",
        r.injected, r.absorbed, r.lost
    );
    for (i, (avg, last)) in r.station.iter().enumerate() {
        println!(
            "GB4 station face {i}: window average {:e}, at 20 s {:e} (relative)",
            rel(*avg, h),
            rel(*last, h)
        );
        assert!(rel(*avg, h) <= 0.05 && rel(*last, h) <= 0.05, "GB4 station face {i}");
    }
    println!(
        "GB4 ledger {:e}, largest face Re_f {:.4}, min mass {:e}",
        r.ledger, r.max_re, r.min_mass
    );
    assert!(r.ledger <= 1e-10, "GB4 ledger");
    assert!(r.lost == 0 && r.min_mass >= 0.0 && r.max_re <= 30.0, "GB4");
}

/// GB5 (consistency): the window average within 1e-6 of the buoyant thickness under the dense gas and under air, and the
/// absorbed mass equal to held + drained.
#[test]
fn gb5_consistency_of_the_buoyant_film() {
    let r = fed_run(Geom::G, Some(BUOYANCY), RHO_DENSE, 2000, 20.0);
    let a = fed_run(Geom::G, Some(BUOYANCY), RHO_AIR, 2000, 20.0);
    let worst = |f: &Fed, h: f64| f.station.iter().map(|(x, _)| rel(*x, h)).fold(0.0, f64::max);
    let (wd, wa) = (worst(&r, h_b(RHO_DENSE)), worst(&a, h_b(RHO_AIR)));
    println!(
        "GB5: dense gas window average {wd:e}, absorbed vs held + drained {:e}; air window average {wa:e} (h_B(1.2)/h_N - 1 = {:e})",
        r.film_ledger,
        h_b(RHO_AIR) / h_n() - 1.0
    );
    assert!(wd <= 1e-6 && r.film_ledger <= 1e-10 && wa <= 1e-6, "GB5");
}

/// GBN (negative control): without the buoyancy the station sits at the Nusselt thickness, more than 5 percent from the
/// buoyant one, so GB4 can fail.
#[test]
fn gbn_negative_control_without_buoyancy() {
    let r = fed_run(Geom::G, None, RHO_DENSE, 2000, 20.0);
    let (to_n, to_b) = r.station.iter().fold((0.0f64, f64::INFINITY), |(n, b), (x, _)| {
        (n.max(rel(*x, h_n())), b.min(rel(*x, h_b(RHO_DENSE))))
    });
    println!("GBN: window average against h_N {to_n:e} (worst), against h_B {to_b:e} (nearest)");
    assert!(to_n <= 1e-6 && to_b >= 0.05, "GBN");
}

/// GS1: the closed forms of the shear-driven film.
#[test]
fn gs1_shear_closed_forms() {
    let h = h_s();
    let u = shear_film_velocity(MU_L, h, [TAU, 0.0, 0.0]);
    let e = rel(RHO_L * h * norm(u), GAMMA);
    println!("GS1: h_S {h:e}, u_bar {:e}, Gamma round trip {e:e}", norm(u));
    assert!(e <= 1e-14);
}

/// GS2: the carrier shear of a Couette field next to the wall, with and without a part normal to the wall, on the floor
/// and on the incline.
#[test]
fn gs2_carrier_shear() {
    let film_h = film_with(Geom::H, SHEAR);
    let mh = mesh(Geom::H);
    let u = couette(Geom::H, &mh);
    let ea = (0..film_h.len())
        .map(|k| rel_v(film_h.carrier_shear(&u, MU_G)[k], [TAU, 0.0, 0.0]))
        .fold(0.0, f64::max);
    let un: Vec<Vec3> = u.iter().map(|v| [v[0], v[1], v[2] + 0.3]).collect();
    let tn = film_h.carrier_shear(&un, MU_G);
    let eb = tn.iter().map(|t| rel_v(*t, [TAU, 0.0, 0.0])).fold(0.0, f64::max);
    let film_g = film_with(Geom::G, SHEAR);
    let mg = mesh(Geom::G);
    let tg = film_g.carrier_shear(&couette(Geom::G, &mg), MU_G);
    let ec = tg.iter().map(|t| rel_v(*t, scale(down(), TAU))).fold(0.0, f64::max);
    println!("GS2: (a) floor {ea:e}, (b) with a normal part {eb:e}, (c) incline {ec:e}");
    assert!(ea <= 1e-14 && eb <= 1e-14 && ec <= 1e-14);
}

/// GS3: at the shear thickness and with no source, one step moves `Gamma W dt` out of row 0 and nothing else.
#[test]
fn gs3_shear_residual() {
    let mut film = film_with(Geom::H, SHEAR);
    let tau = vec![[TAU, 0.0, 0.0]; film.len()];
    film.set_gas_state(RHO_AIR, &tau).unwrap();
    let (e_rows, e_row0, e_total, sub, _) = residual(Geom::H, film, h_s());
    println!("GS3: rows 1-9 {e_rows:e}, row 0 loss {e_row0:e}, held + drained {e_total:e}, sub-steps {sub}");
    assert_eq!(sub, 1);
    assert!(e_rows <= 1e-13 && e_row0 <= 1e-13 && e_total <= 1e-14);
}

/// GS4: from an empty film with a constant feed, rows 1 to 9 reach the shear thickness at 40 s.
#[test]
fn gs4_shear_approach() {
    let mut film = film_with(Geom::H, SHEAR);
    let tau = vec![[TAU, 0.0, 0.0]; film.len()];
    film.set_gas_state(RHO_AIR, &tau).unwrap();
    let (worst, row, ledger, min, sub) = approach(Geom::H, film, 4000, h_s());
    println!("GS4: worst rows 1-9 {worst:e} (row {row}), ledger {ledger:e}, min mass {min:e}, sub-steps {sub}");
    assert!(worst <= 1e-10 && ledger <= 1e-12);
    assert!(min >= 0.0);
    assert_eq!(sub, 4000);
}

/// GS5 (the acceptance): the parcel-fed floor under the frozen Couette field reaches the shear thickness within 5 percent;
/// the ledger, the parcels, the film mass, `Re_f` and no opposed sub-step.
#[test]
fn gs5_acceptance_shear_driven_film_on_a_floor() {
    let r = fed_run(Geom::H, Some(SHEAR), RHO_AIR, 4000, 40.0);
    let h = h_s();
    println!(
        "GS5: h_S {h:e}, injected {}, absorbed {}, lost {}",
        r.injected, r.absorbed, r.lost
    );
    for (i, (avg, last)) in r.station.iter().enumerate() {
        println!(
            "GS5 station face {i}: window average {:e}, at 40 s {:e} (relative)",
            rel(*avg, h),
            rel(*last, h)
        );
        assert!(rel(*avg, h) <= 0.05 && rel(*last, h) <= 0.05, "GS5 station face {i}");
    }
    println!(
        "GS5 ledger {:e}, largest face Re_f {:.4}, min mass {:e}, opposed {}",
        r.ledger, r.max_re, r.min_mass, r.opposed
    );
    assert!(r.ledger <= 1e-10, "GS5 ledger");
    assert!(
        r.lost == 0 && r.min_mass >= 0.0 && r.max_re <= 30.0 && r.opposed == 0,
        "GS5"
    );
}

/// GS6 (consistency): the window average within 1e-6 of the shear thickness, and the absorbed mass equal to held + drained.
#[test]
fn gs6_consistency_of_the_shear_driven_film() {
    let r = fed_run(Geom::H, Some(SHEAR), RHO_AIR, 4000, 40.0);
    let worst = r.station.iter().map(|(a, _)| rel(*a, h_s())).fold(0.0, f64::max);
    println!(
        "GS6: worst window average {worst:e}; absorbed vs held + drained {:e}",
        r.film_ledger
    );
    assert!(worst <= 1e-6 && r.film_ledger <= 1e-10, "GS6");
}

/// GSN (negative control): without the shear the floor's film stays on the first row, so GS5 can fail.
#[test]
fn gsn_negative_control_without_shear() {
    let r = fed_run(Geom::H, None, RHO_AIR, 4000, 40.0);
    println!(
        "GSN: station masses at 40 s {:?}, absorbed {}",
        r.station_mass, r.absorbed
    );
    assert!(r.absorbed > 0, "GSN set-up: no parcel absorbed");
    assert!(r.station_mass.iter().all(|m| *m == 0.0), "GSN");
}

/// The positive root of `a h^3 + b h^2 - q` by bisection to the last bit.
fn bisect_root(a: f64, b: f64, q: f64) -> f64 {
    let f = |h: f64| a * h * h * h + b * h * h - q;
    let (mut lo, mut hi) = (0.0f64, 1e-3f64);
    assert!(f(hi) > 0.0);
    loop {
        let m = 0.5 * (lo + hi);
        if m == lo || m == hi {
            break;
        }
        if f(m) < 0.0 {
            lo = m;
        } else {
            hi = m;
        }
    }
    if f(lo).abs() <= f(hi).abs() {
        lo
    } else {
        hi
    }
}

/// GC1: both drives co-current on the incline: the root of the cubic of G3 (bisection against Cardano), and one step at it
/// moves `Gamma W dt` out of row 0 and nothing else, with no opposed sub-step.
#[test]
fn gc1_co_current_residual() {
    let tau_m = 0.05;
    let a = (RHO_L - RHO_AIR) * g_t() / (3.0 * MU_L);
    let b = tau_m / (2.0 * MU_L);
    let q = GAMMA / RHO_L;
    let hc = bisect_root(a, b, q);
    let p = -b * b / (3.0 * a * a);
    let r = 2.0 * b * b * b / (27.0 * a * a * a) - q / a;
    let sq = (r * r / 4.0 + p * p * p / 27.0).sqrt();
    let cardano = (-r / 2.0 + sq).cbrt() + (-r / 2.0 - sq).cbrt() - b / (3.0 * a);
    let e_root = rel(cardano, hc);
    let mut film = film_with(Geom::G, BOTH);
    let tau = vec![scale(down(), tau_m); film.len()];
    film.set_gas_state(RHO_AIR, &tau).unwrap();
    let (e_rows, e_row0, _, sub, opposed) = residual(Geom::G, film, hc);
    println!(
        "GC1: h_C {hc:e} (h_N {:e}), Cardano {e_root:e}; row 0 loss {e_row0:e}, rows 1-9 {e_rows:e}, sub-steps {sub}, opposed {opposed}",
        h_n()
    );
    assert!(
        e_root <= 1e-12 && e_row0 <= 1e-12 && e_rows <= 1e-13 && opposed == 0,
        "GC1"
    );
}

/// GC2: with the shear against the slope every face counts as opposed in every sub-step.
#[test]
fn gc2_counter_current_count() {
    let tau_m = 0.05;
    let a = (RHO_L - RHO_AIR) * g_t() / (3.0 * MU_L);
    let hc = bisect_root(a, tau_m / (2.0 * MU_L), GAMMA / RHO_L);
    let mut film = film_with(Geom::G, BOTH);
    let tau = vec![scale(down(), -tau_m); film.len()];
    film.set_gas_state(RHO_AIR, &tau).unwrap();
    let (_, _, _, sub, opposed) = residual(Geom::G, film, hc);
    println!("GC2: sub-steps {sub}, opposed {opposed}");
    assert_eq!(opposed, 40 * sub as u64, "GC2");
}

/// GT1: one `advance` gives the film the carrier density and the carrier shear of the flow, bit for bit.
#[test]
fn gt1_tracker_passes_the_gas_state() {
    let m = mesh(Geom::H);
    let opts = Options {
        drag: DragLaw::None,
        interp: Interp::CellCentre,
        gravity: [0.0, 0.0, -G],
        ..Options::default()
    };
    let mut tr = Tracker::new(m.clone(), opts, &[]).unwrap();
    tr.set_film(film_spec()).unwrap();
    tr.set_film_gas(BOTH).unwrap();
    let u = couette(Geom::H, &m);
    let flow = Flow {
        u: &u,
        grad: None,
        rho: RHO_AIR,
        mu: MU_G,
    };
    tr.advance(&flow, 0.01).unwrap();
    let film = tr.film().unwrap();
    let want = film.carrier_shear(&u, MU_G);
    let rho_ok = film.gas_density().to_bits() == RHO_AIR.to_bits();
    let tau_ok = (0..film.len()).all(|k| (0..3).all(|i| film.shear(k)[i].to_bits() == want[k][i].to_bits()));
    println!(
        "GT1: gas density {} (bits equal {rho_ok}), shear bits equal {tau_ok}",
        film.gas_density()
    );
    assert!(rho_ok && tau_ok, "GT1");
}

/// GT2: the refusals of the gas drives, and their controls.
#[test]
fn gt2_refusals() {
    let m = mesh(Geom::H);
    let mut tr = Tracker::new(m.clone(), Options::default(), &[]).unwrap();
    let no_film = tr.set_film_gas(BUOYANCY);
    tr.set_film(film_spec()).unwrap();
    tr.set_film_gas(BUOYANCY).unwrap();
    let u = vec![[0.0; 3]; m.n_cells()];
    let grad = vec![[[0.0; 3]; 3]; m.n_cells()];
    let dense = Flow {
        u: &u,
        grad: Some(&grad),
        rho: RHO_L,
        mu: MU_G,
    };
    let at_rho_l = tr.advance(&dense, 0.01);
    let mut f = film_with(Geom::H, BUOYANCY);
    let n = f.len();
    let nan_rho = f.set_gas_state(f64::NAN, &[]);
    let neg_rho = f.set_gas_state(-1.0, &[]);
    let below = f.set_gas_state(999.9, &[]);
    let mut s = film_with(Geom::H, SHEAR);
    let short = s.set_gas_state(RHO_AIR, &vec![[0.0; 3]; n - 1]);
    let mut bad = vec![[0.0; 3]; n];
    bad[3][1] = f64::NAN;
    let nan_tau = s.set_gas_state(RHO_AIR, &bad);
    let shear_only = s.set_gas_state(RHO_L, &vec![[0.0; 3]; n]);
    let errs = [
        ("set_film_gas without a film", no_film.is_err()),
        ("advance with rho_g = rho_l", at_rho_l.is_err()),
        ("NaN rho_g", nan_rho.is_err()),
        ("rho_g = -1", neg_rho.is_err()),
        ("tau one short", short.is_err()),
        ("NaN in tau", nan_tau.is_err()),
    ];
    let oks = [
        ("rho_g = 999.9 with buoyancy", below.is_ok()),
        ("rho_g = 1000 with the shear only", shear_only.is_ok()),
    ];
    println!("GT2: refusals {errs:?}; controls {oks:?}; {at_rho_l:?}");
    assert!(errs.iter().all(|e| e.1) && oks.iter().all(|o| o.1), "GT2");
}

/// GD1: an explicit default gas is the film of ADR 0192, bit for bit.
#[test]
fn gd1_default_gas_is_the_film_of_adr_0192() {
    let with = fed_run(Geom::G, Some(FilmGas::default()), RHO_AIR, 2000, 20.0);
    let without = fed_run(Geom::G, None, RHO_AIR, 2000, 20.0);
    println!(
        "GD1: fingerprints of {} and {} words, equal {}",
        with.fingerprint.len(),
        without.fingerprint.len(),
        with.fingerprint == without.fingerprint
    );
    assert!(with.fingerprint == without.fingerprint, "GD1");
}
