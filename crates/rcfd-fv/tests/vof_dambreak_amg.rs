//! ADR 0052 A4: the 120 x 40 dam break of ADR 0049 D with the AMG-preconditioned pressure solve (the 27A run broke down at step
//! 120, root cause in `rcfd-linalg::amg`, ADR 0052 addendum 1). Limits registered before the fix: no NaN, every pressure corrector
//! converges below the cap with at most 60 iterations, mean iterations at most half of the plain CG run, wall time not above plain
//! CG, D1/D2/D3 as in `vof_dambreak.rs`, and the front at t* = 3 within one cell of the plain-CG run.
//!
//! F-854 (ADR 0749; ADR 0052 amendment 1): the two runs advance in turn, one step each, and the wall time of a run is the sum
//! of the times of its own steps, so a load of the machine that comes during the test falls on both. The limit is unchanged.

use rcfd_fv::vof::{indicator_fraction, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const A: f64 = 0.05715;
const G: f64 = 9.81;
const NX: usize = 120;
const NY: usize = 40;

fn dam(amg: bool) -> VofSolver {
    let mesh = Arc::new(box_2d(NX, NY, 12.0 * A, 4.0 * A, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| x[0] <= A && x[1] <= 2.0 * A);
    let mut s = VofSettings::new([1000.0, 1.0], [1e-3, 1.8e-5]);
    s.gravity = [0.0, -G, 0.0];
    s.n_correctors = 3;
    s.amg_pressure = amg;
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

fn front(s: &VofSolver) -> f64 {
    let dx = 12.0 * A / NX as f64;
    let a = &s.transport.alpha.values;
    let mut i = NX - 1;
    while i > 0 && a[i] <= 0.5 {
        i -= 1;
    }
    let xc = (i as f64 + 0.5) * dx;
    if i + 1 < NX && a[i] > a[i + 1] {
        xc + dx * (a[i] - 0.5) / (a[i] - a[i + 1])
    } else {
        xc
    }
}

struct Run {
    track: Vec<(f64, f64)>,
    mean_iters: f64,
    max_iters_step: usize,
    dv: f64,
    lo: f64,
    hi: f64,
    co: f64,
    secs: f64,
    finite: bool,
}

/// A run in progress: the solver, its start volume, the pressure iterations so far and the record.
struct Live {
    s: VofSolver,
    v0: f64,
    total: usize,
    r: Run,
}

fn t_unit() -> f64 {
    (2.0 * A / G).sqrt()
}

fn start(amg: bool) -> Live {
    let s = dam(amg);
    let v0 = s.phase_volume();
    let r = Run {
        track: vec![(0.0, front(&s))],
        mean_iters: 0.0,
        max_iters_step: 0,
        dv: 0.0,
        lo: 0.0,
        hi: 1.0,
        co: 0.0,
        secs: 0.0,
        finite: true,
    };
    Live { s, v0, total: 0, r }
}

impl Live {
    fn running(&self) -> bool {
        self.s.time < 3.6 * t_unit()
    }

    /// One step with its bookkeeping; its wall time is added to `secs`.
    fn advance(&mut self) {
        let t0 = std::time::Instant::now();
        let (s, r) = (&mut self.s, &mut self.r);
        let i = s.step(1.5e-3).unwrap();
        self.total += i.p_iters;
        r.max_iters_step = r.max_iters_step.max(i.p_iters);
        r.dv = r.dv.max((s.phase_volume() - self.v0).abs() / self.v0);
        r.lo = r.lo.min(i.alpha_min);
        r.hi = r.hi.max(i.alpha_max);
        r.co = r.co.max(i.courant);
        r.finite &=
            s.p_rgh.values.iter().all(|v| v.is_finite()) && s.transport.alpha.values.iter().all(|v| v.is_finite());
        r.track.push((s.time / t_unit(), front(s)));
        r.secs += t0.elapsed().as_secs_f64();
    }

    fn finish(mut self) -> Run {
        self.r.mean_iters = self.total as f64 / (3 * (self.r.track.len() - 1)) as f64;
        self.r
    }
}

/// The plain-CG run and the AMG run, advanced in turn: one step of each per pair, the order within a pair alternating
/// (ADR 0052 amendment 1). The two solvers share nothing, so each run is the run it would be alone.
fn run_both() -> (Run, Run) {
    let (mut cg, mut am) = (start(false), start(true));
    let mut pair = 0usize;
    while cg.running() || am.running() {
        for amg_turn in [pair % 2 == 1, pair % 2 == 0] {
            let live = if amg_turn { &mut am } else { &mut cg };
            if live.running() {
                live.advance();
            }
        }
        pair += 1;
    }
    (cg.finish(), am.finish())
}

fn at(track: &[(f64, f64)], ts: f64) -> f64 {
    let k = track.iter().position(|p| p.0 >= ts).unwrap();
    let (p0, p1) = (track[k - 1], track[k]);
    p0.1 + (p1.1 - p0.1) * (ts - p0.0) / (p1.0 - p0.0)
}

#[test]
fn a4_dam_break_with_amg() {
    let (cg, am) = run_both();
    let h = 2.0 * A;
    let dx = 12.0 * A / NX as f64;
    let v13 = ((at(&am.track, 3.0) - at(&am.track, 1.0)) / h) / 2.0;
    let e = |ts: f64| {
        let pred = 1.69 * (ts - 1.0);
        (((at(&am.track, ts) - at(&am.track, 1.0)) / h) - pred).abs() / pred
    };
    let monotone = am
        .track
        .windows(2)
        .filter(|w| w[0].0 >= 0.5)
        .all(|w| w[1].1 >= w[0].1 - 1e-12);
    let dfront = (at(&am.track, 3.0) - at(&cg.track, 3.0)).abs();
    println!(
        "A4 AMG: finite {} mean p-iters/corrector {:.1} (CG {:.1}), max p-iters/step {} (cap 3 x 60), wall {:.1} s (CG {:.1} s); D1 v {v13:.3}, D2 {:.3} {:.3}, monotone {monotone}, D3 dV {:.2e} alpha [{:.2e}, {:.9}] Co {:.2}; front(AMG) - front(CG) at t*=3: {:.2e} m ({:.2} dx)",
        am.finite,
        am.mean_iters,
        cg.mean_iters,
        am.max_iters_step,
        am.secs,
        cg.secs,
        e(2.0),
        e(3.0),
        am.dv,
        am.lo,
        am.hi,
        am.co,
        dfront,
        dfront / dx
    );
    assert!(am.finite, "NaN/inf in the AMG run");
    assert!(
        am.max_iters_step <= 3 * 60,
        "A4 iterations per corrector: {}",
        am.max_iters_step
    );
    assert!(am.mean_iters <= 0.5 * cg.mean_iters, "A4 mean iterations");
    assert!(am.secs <= cg.secs, "A4 wall time");
    assert!((1.69 * 0.85..=1.69 * 1.15).contains(&v13) && v13 < 2.0, "D1 {v13}");
    assert!(e(2.0) <= 0.20 && e(3.0) <= 0.20 && monotone, "D2");
    assert!(am.dv <= 1e-12, "D3 volume {}", am.dv);
    assert!(am.lo >= -1e-9 && am.hi <= 1.0 + 1e-9, "Z1 bounds");
    assert!(am.co <= 1.5, "D3 Courant");
    assert!(dfront <= dx, "front vs plain CG {dfront}");
}
