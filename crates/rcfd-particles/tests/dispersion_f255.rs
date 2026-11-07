//! F-255 (ADR 0208): the closed form of the random walk's dispersion and the per-cell fields in the library. Registered
//! checks A (CF1 to CF3) and B (LF1, LF2, LT). The limits are those of ADR 0208, fixed before this code ran.

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_particles::*;
use std::sync::Arc;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// The moment system of ADR 0208 D3 (an independent derivation): `u` a Markov jump process of variance `sigma2` and
/// redraw rate `1/t_l`, `dv/dt = (u - v)/tau`, `dX/dt = v`, from `X = v = 0`; RK4 with `n` steps to `t`. Returns `<X^2>`.
fn moments(sigma2: f64, t_l: f64, tau: f64, t: f64, n: usize) -> f64 {
    let (a, b) = (1.0 / tau, 1.0 / t_l);
    // y = [<vu>, <v^2>, <Xu>, <Xv>, <X^2>]
    let f = |y: [f64; 5]| -> [f64; 5] {
        [
            a * (sigma2 - y[0]) - b * y[0],
            2.0 * a * (y[0] - y[1]),
            y[0] - b * y[2],
            y[1] + a * (y[2] - y[3]),
            2.0 * y[3],
        ]
    };
    let h = t / n as f64;
    let mut y = [0.0; 5];
    for _ in 0..n {
        let k1 = f(y);
        let k2 = f(std::array::from_fn(|i| y[i] + 0.5 * h * k1[i]));
        let k3 = f(std::array::from_fn(|i| y[i] + 0.5 * h * k2[i]));
        let k4 = f(std::array::from_fn(|i| y[i] + h * k3[i]));
        y = std::array::from_fn(|i| y[i] + h / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]));
    }
    y[4]
}

const TIMES: [f64; 4] = [0.25, 1.0, 4.0, 10.0];

/// CF1: the closed form against the moment system on the registered grid.
#[test]
fn cf1_closed_form_equals_the_moment_system() {
    let mut worst: f64 = 0.0;
    for tau in [1e-3f64, 0.5, 1.0, 1.0 + 1e-7, 2.0, 10.0] {
        for t in TIMES {
            let n = ((400.0 * t / tau.min(1.0)).ceil() as usize).max(4000);
            let c = drw_msd(1.0, 1.0, tau, t);
            let m = moments(1.0, 1.0, tau, t, n);
            let e = rel(c, m);
            println!("CF1 tau {tau:e} t {t}: closed {c:.12e} moments {m:.12e} rel {e:.2e} (n {n})");
            assert!(e <= 1e-9, "CF1 tau {tau} t {t}: {e:e}");
            worst = worst.max(e);
        }
    }
    println!("CF1 worst {worst:.2e}");
}

/// CF2: the tracer limit is Taylor's form; Taylor's form at long times.
#[test]
fn cf2_tracer_limit_is_taylor() {
    for t in TIMES {
        let e = rel(drw_msd(1.0, 1.0, 1e-9, t), taylor_msd(1.0, 1.0, t));
        println!("CF2 t {t}: drw_msd(tau 1e-9) against Taylor {e:.2e}");
        assert!(e <= 1e-7, "CF2 t {t}: {e:e}");
    }
    let e = rel(taylor_msd(1.0, 1.0, 50.0), 2.0 * (50.0 - 1.0));
    println!("CF2 Taylor at 50 T against 2 (50 - 1): {e:.2e}");
    assert!(e <= 1e-14, "CF2 long time: {e:e}");
}

/// CF3: the long-time diffusivity is `sigma^2 T` whatever `tau`.
#[test]
fn cf3_long_time_diffusivity_does_not_depend_on_tau() {
    for tau in [1e-3f64, 0.5, 2.0] {
        let k = (drw_msd(1.0, 1.0, tau, 101.0) - drw_msd(1.0, 1.0, tau, 100.0)) / 2.0;
        let e = rel(k, 1.0);
        println!("CF3 tau {tau}: diffusivity {k:.15} rel {e:.2e}");
        assert!(e <= 1e-9, "CF3 tau {tau}: {e:e}");
    }
}

/// The scenario of ADR 0192 R1 (`wall_film.rs`), every extension the tracker had then; `refresh(step, tracker)` is called
/// before every `advance` (steps from 1).
fn r1_scenario(refresh: &dyn Fn(usize, &mut Tracker)) -> Tracker {
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
    for step in 1..=150 {
        refresh(step, &mut tr);
        tr.advance(&flow, 0.01).unwrap();
    }
    tr
}

/// LF1: refusals and read access of `set_dispersion_fields`.
#[test]
fn lf1_dispersion_fields_refusals_and_access() {
    let mesh = Arc::new(box_mesh([2, 2, 2], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap());
    let n = mesh.n_cells();
    let mut tr = Tracker::new(mesh, Options::default(), &[]).unwrap();
    assert!(tr.dispersion_fields().is_none());
    let k = vec![0.5; n];
    let e = vec![0.2; n];
    let r0 = tr.set_dispersion_fields(&k, &e);
    println!("LF1 without set_dispersion: {r0:?}");
    assert!(r0.is_err(), "LF1 without the random walk");
    tr.set_dispersion(Dispersion::default(), k.clone(), e.clone(), 3)
        .unwrap();
    let r1 = tr.set_dispersion_fields(&k[1..], &e);
    let r2 = tr.set_dispersion_fields(&k, &e[1..]);
    let mut kn = k.clone();
    kn[3] = f64::NAN;
    let r3 = tr.set_dispersion_fields(&kn, &e);
    println!("LF1 refusals: {r1:?} / {r2:?} / {r3:?}");
    assert!(r1.is_err() && r2.is_err() && r3.is_err(), "LF1 refusals");
    let k2: Vec<f64> = (0..n).map(|i| 0.1 + 0.01 * i as f64).collect();
    let e2: Vec<f64> = (0..n).map(|i| 0.3 + 0.02 * i as f64).collect();
    tr.set_dispersion_fields(&k2, &e2).unwrap();
    let (kr, er) = tr.dispersion_fields().unwrap();
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(kr), bits(&k2), "LF1 k read back");
    assert_eq!(bits(er), bits(&e2), "LF1 epsilon read back");
}

/// LF2: replacing the fields by the values they hold changes nothing; replacing them by others changes the run from then
/// on only.
#[test]
fn lf2_refresh_has_no_side_effect() {
    let base = r1_scenario(&|_, _| {});
    let same = r1_scenario(&|_, tr| {
        let (k, e) = tr.dispersion_fields().unwrap();
        let (k, e) = (k.to_vec(), e.to_vec());
        tr.set_dispersion_fields(&k, &e).unwrap();
    });
    let fb = base.fingerprint();
    println!("LF2 fingerprint words {}", fb.len());
    assert_eq!(fb, same.fingerprint(), "LF2 the same arrays before every advance");
    let at75 = |doubled: bool| {
        let cut = std::cell::Cell::new(None::<Vec<u64>>);
        let tr = r1_scenario(&|step, tr| {
            if step == 76 {
                cut.set(Some(tr.fingerprint()));
                if doubled {
                    let (k, e) = tr.dispersion_fields().unwrap();
                    let k: Vec<f64> = k.iter().map(|x| 2.0 * x).collect();
                    let e = e.to_vec();
                    tr.set_dispersion_fields(&k, &e).unwrap();
                }
            }
        });
        (cut.take().unwrap(), tr.fingerprint())
    };
    let (b75, b150) = at75(false);
    let (d75, d150) = at75(true);
    assert_eq!(b75, d75, "LF2 equal after step 75");
    assert_eq!(b150, fb, "LF2 control");
    assert_ne!(d150, fb, "LF2 2 k from step 76 changes the run");
}

/// LT: the library form of CT3: a tracer cloud in the uniform fields of case T.
#[test]
fn lt_library_taylor() {
    let mesh = Arc::new(box_mesh([5, 5, 5], [400.0, 400.0, 400.0], [Grading::Uniform; 3]).unwrap());
    let n = mesh.n_cells();
    let opts = Options {
        drag: DragLaw::Stokes,
        scheme: Scheme::Analytic,
        interp: Interp::Gradient,
        gravity: [0.0; 3],
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh, opts, &[]).unwrap();
    let x0 = [200.0, 200.0, 200.0];
    tr.add_injector(InjectorSpec {
        name: "c".into(),
        kind: InjKind::Point(x0),
        rate: 1e8,
        t_start: 0.0,
        t_end: 2e-4,
        velocity: VelMode::Given([0.0; 3]),
        dist: SizeDist::Constant(1.8e-5),
        density: 1000.0,
        parcel_count: 1.0,
        seed: 1_000_003,
    })
    .unwrap();
    let (k, eps) = (1.5, 0.225);
    tr.set_dispersion(Dispersion::default(), vec![k; n], vec![eps; n], 20261111)
        .unwrap();
    let u = vec![[0.0; 3]; n];
    let grad = vec![[[0.0; 3]; 3]; n];
    let flow = Flow {
        u: &u,
        grad: Some(&grad),
        rho: 1.2,
        mu: 1.2 * 1.5e-5,
    };
    let sigma2 = 2.0 * k / 3.0;
    let t_l = 0.15 * k / eps;
    let tau = 1000.0 * 1.8e-5 * 1.8e-5 / (18.0 * 1.2 * 1.5e-5);
    let mut done = 0;
    for t_end in TIMES {
        while tr.time < t_end * (1.0 - 1e-12) {
            let h = 0.05f64.min(t_end - tr.time);
            tr.advance(&flow, h).unwrap();
        }
        let ps = &tr.particles;
        assert_eq!(ps.len(), 20_000, "LT N");
        assert!(ps.iter().all(|p| p.state == State::Active), "LT all active");
        let nf = ps.len() as f64;
        let v = ps
            .iter()
            .map(|p| (0..3).map(|c| (p.x[c] - x0[c]).powi(2)).sum::<f64>())
            .sum::<f64>()
            / (3.0 * nf);
        let c = ps.iter().map(|p| drw_msd(sigma2, t_l, tau, p.age)).sum::<f64>() / nf;
        let e = rel(v, c);
        println!("LT t {t_end}: V {v:.6e} C {c:.6e} rel {e:.4}");
        assert!(e <= 0.05, "LT t {t_end}: {e}");
        done += 1;
    }
    assert_eq!(done, 4);
}
