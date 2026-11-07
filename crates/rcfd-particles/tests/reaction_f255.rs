//! F-255 (ADR 0208): the surface reaction of the parcels (library). Registered checks D (RX1 to RX6). The limits are those
//! of ADR 0208, fixed before this code ran.

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_particles::*;
use std::f64::consts::PI;
use std::sync::Arc;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// The reaction of the checks: `S + G_O -> G_P` with round molar masses (test inputs, not data), `f_r = 0.875`.
fn spec(rate: f64) -> ReactionSpec {
    ReactionSpec {
        solid_molar_mass: 0.012,
        solid_coefficient: 1.0,
        gas: vec![
            GasSpecies {
                name: "O".into(),
                molar_mass: 0.032,
                coefficient: -1.0,
            },
            GasSpecies {
                name: "P".into(),
                molar_mass: 0.044,
                coefficient: 1.0,
            },
        ],
        oxidiser: 0,
        rate,
        reacting_fraction: 0.875,
    }
}

fn total(v: &[f64]) -> f64 {
    v.iter().sum()
}

/// The RX1 set-up: one parcel released at rest at 1e-3 s at the centre of a unit cell, carrier at rest.
fn rx1_tracker() -> Tracker {
    let mesh = Arc::new(box_mesh([1, 1, 1], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap());
    let opts = Options {
        drag: DragLaw::Stokes,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0; 3],
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh, opts, &[]).unwrap();
    tr.add_injector(InjectorSpec {
        name: "p".into(),
        kind: InjKind::Point([0.5, 0.5, 0.5]),
        rate: 1000.0,
        t_start: 0.0,
        t_end: 1e-3,
        velocity: VelMode::Given([0.0; 3]),
        dist: SizeDist::Constant(1e-4),
        density: 1500.0,
        parcel_count: 1e6,
        seed: 1,
    })
    .unwrap();
    tr.set_reaction(spec(0.01), vec![0.23]).unwrap();
    tr
}

const RX1_SLOPE: f64 = 1.38e-6;

fn still() -> (Vec<[f64; 3]>, f64, f64) {
    (vec![[0.0; 3]], 1.2, 1.8e-5)
}

/// RX1: the closed form of a parcel at rest in a uniform oxidiser.
#[test]
fn rx1_closed_form_shrinking_parcel() {
    let mut tr = rx1_tracker();
    let (u, rho, mu) = still();
    let flow = Flow {
        u: &u,
        grad: None,
        rho,
        mu,
    };
    let d0 = 1e-4;
    let omega = tr.reaction().unwrap().rate_density(rho, 0);
    let slope = 2.0 * 0.012 * omega / 1500.0;
    println!(
        "RX1 omega {omega:e} dd/dt {slope:e} (registered {RX1_SLOPE:e}), t_b {:.4} s",
        5e-5 / slope
    );
    assert!(rel(slope, RX1_SLOPE) <= 1e-12, "RX1 set-up slope");
    for step in 1..=400 {
        tr.advance(&flow, 0.1).unwrap();
        if [100, 200, 300].contains(&step) {
            let p = &tr.particles[0];
            let e = rel(p.d, d0 - RX1_SLOPE * p.age);
            println!("RX1 step {step} age {:.6}: d {:.15e} rel {e:.2e}", p.age, p.d);
            assert!(e <= 1e-12, "RX1 step {step}: {e:e}");
        }
    }
    assert_eq!(tr.particles.len(), 1, "RX1 one parcel");
    let p = &tr.particles[0];
    let rx = tr.reaction().unwrap();
    let m0 = 1500.0 * PI * d0 * d0 * d0 / 6.0;
    let e_d = rel(p.d, d0 / 2.0);
    let e_s = rel(rx.solid_reacted(), 1e6 * 0.875 * m0);
    let e_o = rel(total(rx.sources(0)), -0.032 * rx.extent());
    let e_p = rel(total(rx.sources(1)), 0.044 * rx.extent());
    println!(
        "RX1 at 40 s: reacting mass {:e}, d {:.15e} rel {e_d:.2e}, solid {e_s:.2e}, O {e_o:.2e}, P {e_p:.2e}",
        tr.reacting_mass(0),
        p.d
    );
    assert_eq!(tr.reacting_mass(0), 0.0, "RX1 burnt out");
    assert!(e_d <= 1e-14, "RX1 inert diameter: {e_d:e}");
    assert!(e_s <= 1e-12, "RX1 solid reacted: {e_s:e}");
    assert!(e_o <= 1e-12 && e_p <= 1e-12, "RX1 sources: {e_o:e} {e_p:e}");
}

/// RX5: a new oxidiser field doubles the shrink rate from the next step on.
#[test]
fn rx5_oxidiser_refresh() {
    let mut tr = rx1_tracker();
    let (u, rho, mu) = still();
    let flow = Flow {
        u: &u,
        grad: None,
        rho,
        mu,
    };
    let (mut d100, mut a100) = (0.0, 0.0);
    for step in 1..=200 {
        tr.advance(&flow, 0.1).unwrap();
        let p = &tr.particles[0];
        if step == 100 {
            d100 = p.d;
            a100 = p.age;
            tr.set_reaction_oxidiser(&[0.46]).unwrap();
        } else if step == 150 || step == 200 {
            let e = rel(p.d, d100 - 2.0 * RX1_SLOPE * (p.age - a100));
            println!("RX5 step {step} age {:.6}: d {:.15e} rel {e:.2e}", p.age, p.d);
            assert!(e <= 1e-12, "RX5 step {step}: {e:e}");
        }
    }
}

/// The RX2 scenario: the ADR 0192 R1 scenario without heat transfer and exchange record, constant diameters, with the
/// reaction when `y_scale` is given (`Y_o = y_scale (0.05 + 0.2 x_c / 1.2)`). `check(step, tracker)` runs after every
/// advance.
fn rx2_scenario(y_scale: Option<f64>, check: &mut dyn FnMut(usize, &Tracker)) -> Tracker {
    rx2_scenario_with(y_scale, false, check)
}

/// [`rx2_scenario`], with the third injector `c` of ADR 0208 amendment 1 when `third` (RX2b, RX3b).
fn rx2_scenario_with(y_scale: Option<f64>, third: bool, check: &mut dyn FnMut(usize, &Tracker)) -> Tracker {
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
        dist: SizeDist::Constant(1e-4),
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
        dist: SizeDist::Constant(6e-5),
        density: 1400.0,
        parcel_count: 1.0,
        seed: 12,
    })
    .unwrap();
    if third {
        // ADR 0208 amendment 1: half a cell above the sticking floor, at rest
        tr.add_injector(InjectorSpec {
            name: "c".into(),
            kind: InjKind::Point([0.6, 0.5, 0.05]),
            rate: 200.0,
            t_start: 0.0,
            t_end: 0.4,
            velocity: VelMode::Given([0.0; 3]),
            dist: SizeDist::Constant(1e-4),
            density: 1300.0,
            parcel_count: 10.0,
            seed: 13,
        })
        .unwrap();
    }
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
            temperature: None,
        },
    )
    .unwrap();
    let n = mesh.n_cells();
    tr.set_dispersion(Dispersion::default(), vec![0.02; n], vec![0.05; n], 99)
        .unwrap();
    if let Some(s) = y_scale {
        let y: Vec<f64> = mesh
            .cell_centres()
            .iter()
            .map(|c| s * (0.05 + 0.2 * c[0] / 1.2))
            .collect();
        tr.set_reaction(spec(1.0), y).unwrap();
    }
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
    for step in 1..=300 {
        tr.advance(&flow, 0.01).unwrap();
        check(step, &tr);
    }
    tr
}

/// Injected diameter of a parcel of the RX2 scenario (by its density; 1300 is the injector `c` of amendment 1).
fn d0_of(p: &Particle) -> f64 {
    if p.rho == 1500.0 || p.rho == 1300.0 {
        1e-4
    } else {
        6e-5
    }
}

/// RX2 (acceptance): per-species conservation at the end of every step; RX3 at the end. ADR 0208 amendment 1: the record
/// of the first run, whose set-up deposits no parcel (RX3's last condition); the corrected test is
/// `rx2b_rx3b_species_conservation_with_deposition`.
#[test]
#[ignore = "ADR 0208 amendment 1: set-up defect (no parcel deposited); RX2 met at every step (1.0e-14); corrected test rx2b_rx3b_species_conservation_with_deposition"]
fn rx2_rx3_species_conservation_setup_defect() {
    let mut worst = [0.0f64; 4];
    let mut checks = 0;
    let tr = rx2_scenario(Some(1.0), &mut |step, tr| {
        let rx = tr.reaction().unwrap();
        let dm_p: f64 = tr
            .particles
            .iter()
            .map(|p| {
                let d0 = d0_of(p);
                p.parcel * (PI * p.rho / 6.0) * (d0 * d0 * d0 - p.d * p.d * p.d)
            })
            .sum();
        if dm_p == 0.0 {
            return;
        }
        let xi_p = dm_p / 0.012;
        let s_o = total(rx.sources(0));
        let s_p = total(rx.sources(1));
        let e = [
            rel(rx.solid_reacted(), dm_p),
            rel(s_o, -0.032 * xi_p),
            rel(s_p, 0.044 * xi_p),
            rel(s_o + s_p, dm_p),
        ];
        for k in 0..4 {
            worst[k] = worst[k].max(e[k]);
        }
        checks += 1;
        assert!(e.iter().all(|x| *x <= 1e-10), "RX2 step {step}: {e:?}");
    });
    println!(
        "RX2 {checks} steps checked; worst solid {:.2e}, O {:.2e}, P {:.2e}, total {:.2e}",
        worst[0], worst[1], worst[2], worst[3]
    );
    assert_eq!(checks, 300, "RX2 every step");
    let n = tr.particles.len();
    let mut burnt = 0;
    let mut partly = 0;
    for (i, p) in tr.particles.iter().enumerate() {
        let m_r = tr.reacting_mass(i);
        let d0 = d0_of(p);
        let d_i = d0 * (1.0 - 0.875f64).cbrt();
        assert!(m_r >= 0.0, "RX3 parcel {i}: reacting mass {m_r:e}");
        assert!(
            p.d >= d_i * (1.0 - 1e-14) && p.d <= d0,
            "RX3 parcel {i}: d {:e} not in [{d_i:e}, {d0:e}]",
            p.d
        );
        if m_r == 0.0 {
            burnt += 1;
        } else if p.d < d0 {
            partly += 1;
        }
    }
    let counts = [State::Active, State::Deposited, State::Escaped, State::Stuck].map(|s| tr.count(s));
    println!("RX3 {n} parcels: burnt out {burnt}, partly reacted {partly}; active/deposited/escaped/stuck {counts:?}");
    assert!(
        10 * burnt >= n && 10 * partly >= n,
        "RX3 regimes: burnt {burnt} partly {partly} of {n}"
    );
    assert!(counts[1] > 0 && counts[2] > 0, "RX3 deposited and escaped: {counts:?}");
}

/// RX2b and RX3b (ADR 0208 amendment 1): RX2 and RX3 on the scenario with the third injector `c`.
#[test]
fn rx2b_rx3b_species_conservation_with_deposition() {
    let mut worst = [0.0f64; 4];
    let mut checks = 0;
    let tr = rx2_scenario_with(Some(1.0), true, &mut |step, tr| {
        let rx = tr.reaction().unwrap();
        let dm_p: f64 = tr
            .particles
            .iter()
            .map(|p| {
                let d0 = d0_of(p);
                p.parcel * (PI * p.rho / 6.0) * (d0 * d0 * d0 - p.d * p.d * p.d)
            })
            .sum();
        if dm_p == 0.0 {
            return;
        }
        let xi_p = dm_p / 0.012;
        let s_o = total(rx.sources(0));
        let s_p = total(rx.sources(1));
        let e = [
            rel(rx.solid_reacted(), dm_p),
            rel(s_o, -0.032 * xi_p),
            rel(s_p, 0.044 * xi_p),
            rel(s_o + s_p, dm_p),
        ];
        for k in 0..4 {
            worst[k] = worst[k].max(e[k]);
        }
        checks += 1;
        assert!(e.iter().all(|x| *x <= 1e-10), "RX2b step {step}: {e:?}");
    });
    println!(
        "RX2b {checks} steps checked; worst solid {:.2e}, O {:.2e}, P {:.2e}, total {:.2e}",
        worst[0], worst[1], worst[2], worst[3]
    );
    assert_eq!(checks, 300, "RX2b every step");
    let n = tr.particles.len();
    let mut burnt = 0;
    let mut partly = 0;
    for (i, p) in tr.particles.iter().enumerate() {
        let m_r = tr.reacting_mass(i);
        let d0 = d0_of(p);
        let d_i = d0 * (1.0 - 0.875f64).cbrt();
        assert!(m_r >= 0.0, "RX3b parcel {i}: reacting mass {m_r:e}");
        assert!(
            p.d >= d_i * (1.0 - 1e-14) && p.d <= d0,
            "RX3b parcel {i}: d {:e} not in [{d_i:e}, {d0:e}]",
            p.d
        );
        if m_r == 0.0 {
            burnt += 1;
        } else if p.d < d0 {
            partly += 1;
        }
    }
    let counts = [State::Active, State::Deposited, State::Escaped, State::Stuck].map(|s| tr.count(s));
    println!("RX3b {n} parcels: burnt out {burnt}, partly reacted {partly}; active/deposited/escaped/stuck {counts:?}");
    assert!(
        10 * burnt >= n && 10 * partly >= n,
        "RX3b regimes: burnt {burnt} partly {partly} of {n}"
    );
    assert!(counts[1] > 0 && counts[2] > 0, "RX3b deposited and escaped: {counts:?}");
}

/// RX4: with no oxidiser the reaction changes no parcel.
#[test]
fn rx4_inert_at_zero_oxidiser() {
    let without = rx2_scenario(None, &mut |_, _| {}).fingerprint();
    let with = rx2_scenario(Some(0.0), &mut |_, _| {}).fingerprint();
    println!("RX4 fingerprint words: without {}, with {}", without.len(), with.len());
    assert!(with.len() > without.len(), "RX4 the reaction part is appended");
    assert_eq!(&with[..without.len()], &without[..], "RX4 parcels unchanged");
}

/// RX6: refusals and the control.
#[test]
fn rx6_refusals() {
    let mesh = Arc::new(box_mesh([1, 1, 1], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap());
    let fresh = || Tracker::new(mesh.clone(), Options::default(), &[]).unwrap();
    let y = vec![0.23];
    let mut bad: Vec<(&str, ReactionSpec, Vec<f64>)> = Vec::new();
    let with = |f: &dyn Fn(&mut ReactionSpec)| {
        let mut s = spec(0.01);
        f(&mut s);
        s
    };
    bad.push(("rate 0", with(&|s| s.rate = 0.0), y.clone()));
    bad.push(("f_r 1", with(&|s| s.reacting_fraction = 1.0), y.clone()));
    bad.push(("f_r 0", with(&|s| s.reacting_fraction = 0.0), y.clone()));
    bad.push(("no gas", with(&|s| s.gas.clear()), y.clone()));
    bad.push(("coefficient 0", with(&|s| s.gas[1].coefficient = 0.0), y.clone()));
    bad.push(("molar mass < 0", with(&|s| s.gas[0].molar_mass = -0.032), y.clone()));
    bad.push(("oxidiser index 2", with(&|s| s.oxidiser = 2), y.clone()));
    bad.push(("oxidiser P", with(&|s| s.oxidiser = 1), y.clone()));
    bad.push(("unbalanced", with(&|s| s.gas[1].molar_mass = 0.045), y.clone()));
    bad.push(("y short", spec(0.01), vec![]));
    bad.push(("y 1.5", spec(0.01), vec![1.5]));
    for (what, s, y) in bad {
        let r = fresh().set_reaction(s, y);
        println!("RX6 {what}: {r:?}");
        assert!(r.is_err(), "RX6 {what}");
    }
    let th = || Thermal {
        cp_p: 900.0,
        k_f: 0.026,
        cp_f: 1005.0,
        t_fluid: vec![300.0],
    };
    let mut t = fresh();
    t.set_reaction(spec(0.01), y.clone()).unwrap();
    let second = t.set_reaction(spec(0.01), y.clone());
    let thermal_after = t.set_thermal(th());
    let mut t2 = fresh();
    t2.set_thermal(th()).unwrap();
    let after_thermal = t2.set_reaction(spec(0.01), y.clone());
    let mut t3 = fresh();
    t3.set_reaction(spec(0.01), y.clone()).unwrap();
    t3.enable_coupling();
    let u = vec![[0.0; 3]];
    let grad = vec![[[0.0; 3]; 3]];
    let flow = Flow {
        u: &u,
        grad: Some(&grad),
        rho: 1.2,
        mu: 1.8e-5,
    };
    let adv = t3.advance(&flow, 0.1);
    let no_rx = fresh().set_reaction_oxidiser(&[0.1]);
    println!("RX6 second {second:?}; thermal after {thermal_after:?}; after thermal {after_thermal:?}; advance with the record {adv:?}; oxidiser without {no_rx:?}");
    assert!(second.is_err() && thermal_after.is_err() && after_thermal.is_err() && adv.is_err() && no_rx.is_err());
    assert!(fresh().set_reaction(spec(0.01), y).is_ok(), "RX6 control");
}
