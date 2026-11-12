//! ADR 0245 (F-344): the step-size-dependent remainder of the library's BDF2 step on a mesh at rest.
//!
//! Set-up R (ADR 0241 H16): the mesh of SMc at rest, the exact Taylor-Green velocity less its net flux fixed on the four
//! sides, `PimpleSolver::step` with BDF2 and three outer iterations. `d_N` is the difference of the velocities of the
//! runs in `N` and in 800 steps over the decay. The helpers of the set-up are copies of those of `overset_f338.rs`.
//!
//! Slow tests are ignored: `cargo test --release -p rcfd-fv --test bdf2_remainder_f344 -- --ignored --nocapture`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::move_mesh;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, cavity_2d, rectilinear_2d};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;

const DEPTH: f64 = 0.1;
const NU_TG: f64 = 0.1;
const T_END: f64 = 0.5;
/// Half the side of the square of SMc.
const HALF: f64 = 0.6;
/// The centre of the square at rest (`tg_pose(0)` of ADR 0219: no rotation, this translation).
const CENTRE: [f64; 2] = [PI / 2.0 - 0.3, PI / 2.0 - 0.2];

/// Solver controls of every set-up (ADR 0219 PS).
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};

/// ADR 0219 PS.
fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    s
}

/// Records the failed checks and prints every check.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    /// `value <= limit`.
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.4e} (limit <= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    /// `value >= limit`.
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:.4e} (limit >= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} < {limit:e}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

fn tg_u(x: Vec3, t: f64) -> Vec3 {
    let e = (-2.0 * NU_TG * t).exp();
    [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
}

fn tg_p(x: Vec3, t: f64) -> f64 {
    -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
}

/// The exact velocity at `t` on the four sides (the initial conditions of SMc).
fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
    for name in ["xmin", "xmax", "ymin", "ymax"] {
        let v: Vec<Vec3> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| tg_u(geom.mesh.face_centres()[f], t))
            .collect();
        u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
}

/// The exact velocity at `t` on the four sides, less its net flux (SMc of ADR 0232 amendment 2).
fn sm_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
    let sides = ["xmin", "xmax", "ymin", "ymax"];
    let sf = geom.mesh.face_area_vectors();
    let fc = geom.mesh.face_centres();
    let (mut q, mut a) = (0.0, 0.0);
    for name in sides {
        for f in geom.mesh.patch(name).unwrap().faces() {
            q += vec3::dot(tg_u(fc[f], t), sf[f]);
            a += vec3::norm(sf[f]);
        }
    }
    for name in sides {
        let v: Vec<Vec3> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| vec3::sub(tg_u(fc[f], t), vec3::scale(vec3::unit(sf[f]), q / a)))
            .collect();
        u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
}

/// One run of the set-up R.
#[derive(Clone)]
struct Cfg {
    /// Cells per side (40 is SMc).
    n: usize,
    /// The step sizes; they sum to `T_END`.
    dts: Vec<f64>,
    n_outer: usize,
    n_correctors: usize,
    ddt_correction: bool,
    /// The corrective C of ADR 0245 amendment 1 (`PimpleSettings::rhie_chow_step_consistent`).
    consistent: bool,
}

impl Cfg {
    /// R: `n` cells per side, `steps` equal steps, BDF2, three outer iterations, two correctors.
    fn r(n: usize, steps: usize) -> Self {
        Cfg {
            n,
            dts: vec![T_END / steps as f64; steps],
            n_outer: 3,
            n_correctors: 2,
            ddt_correction: true,
            consistent: false,
        }
    }
}

/// The state of a run at `t = 0.5`.
struct Out {
    geom: Arc<FvGeometry>,
    u: Vec<Vec3>,
    /// The pressure with its mean removed.
    p: Vec<f64>,
    /// The face defect `(phi - interp(u) . S) / |S|` of the internal faces.
    defect: Vec<f64>,
    /// The largest continuity of the steps.
    continuity: f64,
}

fn run(cfg: &Cfg) -> Out {
    let cs = uniform(-HALF, 2.0 * HALF, cfg.n);
    let mesh0 = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let pts: Vec<Vec3> = mesh0
        .points()
        .iter()
        .map(|q| [q[0] + CENTRE[0], q[1] + CENTRE[1], q[2]])
        .collect();
    let geom = Arc::new(FvGeometry::new(Arc::new(moved_mesh(&mesh0, pts).unwrap())).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut set = settings(NU_TG);
    set.time_scheme = TimeScheme::Bdf2;
    set.n_outer = cfg.n_outer;
    set.n_correctors = cfg.n_correctors;
    set.ddt_correction = cfg.ddt_correction;
    set.rhie_chow_step_consistent = cfg.consistent;
    let mut s = PimpleSolver::new(geom.clone(), u, p, set);
    let equal = cfg.dts.iter().all(|d| *d == cfg.dts[0]);
    let mut continuity = 0.0f64;
    let mut t = 0.0;
    for (k, &dt) in cfg.dts.iter().enumerate() {
        // equal steps: the times of ADR 0241; otherwise the sum, the last step landing on the end time
        t = if equal {
            (k + 1) as f64 * dt
        } else if k + 1 == cfg.dts.len() {
            T_END
        } else {
            t + dt
        };
        sm_conditions(&geom, &mut s.u, t);
        let info = s.step(dt);
        continuity = continuity.max(info.continuity);
    }
    assert!(s.diverged.is_none(), "the run stopped: {:?}", cfg.dts.len());
    let ub = s.u.boundary_values(&geom);
    let flux = face_flux(&geom, &s.u.values, &ub);
    let sf = geom.mesh.face_area_vectors();
    let defect = (0..geom.n_internal)
        .map(|f| (s.phi[f] - flux[f]) / vec3::norm(sf[f]))
        .collect();
    let mean = s.p.values.iter().sum::<f64>() / s.p.values.len() as f64;
    Out {
        geom,
        u: s.u.values.clone(),
        p: s.p.values.iter().map(|x| x - mean).collect(),
        defect,
        continuity,
    }
}

fn decay() -> f64 {
    (-2.0 * NU_TG * T_END).exp()
}

/// The largest difference of two velocity fields over the decay, and its cell.
fn distance_at(a: &Out, b: &Out) -> (f64, usize) {
    a.u.iter()
        .zip(&b.u)
        .map(|(p, q)| vec3::norm(vec3::sub(*p, *q)) / decay())
        .enumerate()
        .fold((0.0, 0), |m, (c, x)| if x > m.0 { (x, c) } else { m })
}

fn distance(a: &Out, b: &Out) -> f64 {
    distance_at(a, b).0
}

/// The largest difference of two pressure fields (means removed) over the decay of the pressure.
fn distance_p(a: &Out, b: &Out) -> f64 {
    let e = (-4.0 * NU_TG * T_END).exp();
    a.p.iter().zip(&b.p).map(|(p, q)| (p - q).abs() / e).fold(0.0, f64::max)
}

/// The layer of a cell: 0 next to a side, counted inwards.
fn layer(o: &Out, c: usize) -> usize {
    let x = o.geom.mesh.cell_centres()[c];
    let h = 2.0 * HALF / ((o.u.len() as f64).sqrt().round());
    let d = (HALF - (x[0] - CENTRE[0]).abs()).min(HALF - (x[1] - CENTRE[1]).abs());
    (d / h).floor() as usize
}

/// The largest difference of the face defects of two runs over the decay, on the faces with a cell in the two layers
/// next to a side (near) and on the other internal faces (inner), each with its face.
fn defect_distance(a: &Out, b: &Out) -> ((f64, usize), (f64, usize)) {
    let (own, nb) = (a.geom.mesh.owner(), a.geom.mesh.neighbour());
    let (mut near, mut inner) = ((0.0, 0), (0.0, 0));
    for f in 0..a.geom.n_internal {
        let x = (a.defect[f] - b.defect[f]).abs() / decay();
        let m = if layer(a, own[f]).min(layer(a, nb[f])) < 2 {
            &mut near
        } else {
            &mut inner
        };
        if x > m.0 {
            *m = (x, f);
        }
    }
    (near, inner)
}

/// The largest error of the velocity against the exact Taylor-Green field, over the decay.
fn exact_error(o: &Out) -> f64 {
    let cc = o.geom.mesh.cell_centres();
    o.u.iter()
        .enumerate()
        .map(|(c, v)| vec3::norm(vec3::sub(*v, tg_u(cc[c], T_END))) / decay())
        .fold(0.0, f64::max)
}

/// ADR 0245 S0 and S1: the set-up R gives the numbers of ADR 0241 H16.
#[test]
fn s0_rest_remainder_is_that_of_adr_0241() {
    let (a, b, r) = (run(&Cfg::r(40, 200)), run(&Cfg::r(40, 400)), run(&Cfg::r(40, 800)));
    let (d200, d400) = (distance(&a, &r), distance(&b, &r));
    let mut c = Checks::new();
    c.at_least("S0 (set-up) max |d_200| of R", d200, 5.0e-6);
    c.at_most("S0 (set-up) max |d_200| of R", d200, 5.3e-6);
    c.at_least("S0 (set-up) max |d_200| / max |d_400| of R", d200 / d400, 2.2);
    c.at_most("S0 (set-up) max |d_200| / max |d_400| of R", d200 / d400, 2.4);
    c.at_most(
        "S1 continuity, every step of the three runs",
        a.continuity.max(b.continuity).max(r.continuity),
        1e-9,
    );
    c.finish();
}

/// The periodic Taylor-Green box of the library's unit tests with the settings of this item: the velocities and the
/// amplitude at `t = 0.5` over the exact one.
fn periodic_run(steps: usize) -> (Vec<Vec3>, f64) {
    let l = 2.0 * PI;
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(64, 64, l, l, true, true).unwrap())).unwrap());
    let init = |c: Vec3| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0];
    let u = VectorField::from_fn("U", &geom, init);
    let p = ScalarField::from_fn("p", &geom, |c| 0.25 * ((2.0 * c[0]).cos() + (2.0 * c[1]).cos()));
    let mut set = settings(NU_TG);
    set.time_scheme = TimeScheme::Bdf2;
    set.n_outer = 3;
    let mut s = PimpleSolver::new(geom.clone(), u, p, set);
    let dt = T_END / steps as f64;
    for _ in 0..steps {
        s.step(dt);
    }
    let cc = geom.mesh.cell_centres();
    let (mut num, mut den) = (0.0, 0.0);
    for (c, v) in s.u.values.iter().enumerate() {
        let e = init(cc[c]);
        num += vec3::dot(*v, e);
        den += vec3::dot(e, e);
    }
    (s.u.values.clone(), num / den / decay())
}

/// ADR 0245, the numbers of H1 to H6 and H8 and the printed series. Printed; the hypotheses are decided in the ADR.
#[test]
#[ignore = "diagnosis of ADR 0245 (about forty runs of the set-up R, about fifteen minutes): printed only, run once"]
fn diagnosis_of_the_remainder_at_rest() {
    let mut cont = 0.0f64;
    // H1, H4: the order between consecutive pairs
    let ns = [100usize, 200, 400, 800, 1600];
    let runs: Vec<Out> = ns.iter().map(|&n| run(&Cfg::r(40, n))).collect();
    for o in &runs {
        cont = cont.max(o.continuity);
    }
    let delta: Vec<f64> = (0..4).map(|i| distance(&runs[i], &runs[i + 1])).collect();
    let delta_p: Vec<f64> = (0..4).map(|i| distance_p(&runs[i], &runs[i + 1])).collect();
    for i in 0..4 {
        println!(
            "H1 delta_{} = max |u_{} - u_{}| {:.4e}; H4 pressure {:.4e}",
            ns[i],
            ns[i],
            ns[i + 1],
            delta[i],
            delta_p[i]
        );
    }
    for i in 0..3 {
        println!(
            "H1 q_{} {:.4}; H4 pressure q_{} {:.4}",
            ns[i],
            (delta[i] / delta[i + 1]).log2(),
            ns[i],
            (delta_p[i] / delta_p[i + 1]).log2()
        );
    }
    let (d200, c200) = distance_at(&runs[1], &runs[3]);
    let d400 = distance(&runs[2], &runs[3]);
    let x = runs[1].geom.mesh.cell_centres()[c200];
    println!(
        "R: max |d_200| {d200:.4e} in cell {c200} at ({:.3}, {:.3}), layer {}; max |d_400| {d400:.4e}; ratio {:.4}",
        x[0],
        x[1],
        layer(&runs[1], c200),
        d200 / d400
    );
    println!(
        "R against the run in 1600 steps: max |u_200 - u_1600| {:.4e}, max |u_400 - u_1600| {:.4e}, max |u_800 - u_1600| {:.4e}",
        distance(&runs[1], &runs[4]),
        distance(&runs[2], &runs[4]),
        distance(&runs[3], &runs[4])
    );
    println!(
        "R error against the exact field: 200 steps {:.6e}, 800 steps {:.6e}, 1600 steps {:.6e}",
        exact_error(&runs[1]),
        exact_error(&runs[3]),
        exact_error(&runs[4])
    );
    // H5, H6: the face defect
    for (i, n) in [(1usize, 200), (2, 400)] {
        let (near, inner) = defect_distance(&runs[i], &runs[3]);
        let fc = runs[i].geom.mesh.face_centres();
        println!(
            "H5/H6 D_{n}: near {:.4e} (face at ({:.3}, {:.3})), inner {:.4e} (face at ({:.3}, {:.3})), near / inner {:.3}",
            near.0,
            fc[near.1][0],
            fc[near.1][1],
            inner.0,
            fc[inner.1][0],
            fc[inner.1][1],
            near.0 / inner.0
        );
    }
    let size = runs[3].defect.iter().fold(0.0f64, |m, d| m.max(d.abs())) / decay();
    println!("H5 the largest face defect itself (800 steps): {size:.4e}");
    // H2: a common start
    let common = |n: usize| -> Cfg {
        let mut dts = vec![0.000625; 160];
        dts.extend(vec![0.4 / n as f64; n]);
        Cfg { dts, ..Cfg::r(40, 1) }
    };
    let (a, b, r) = (run(&common(160)), run(&common(320)), run(&common(640)));
    cont = cont.max(a.continuity).max(b.continuity).max(r.continuity);
    let (s160, s320) = (distance(&a, &r), distance(&b, &r));
    println!(
        "H2 common start: max |d'_160| {s160:.4e}, max |d'_320| {s320:.4e}, R_start {:.4}",
        s160 / s320
    );
    // H3: six outer iterations; four correctors
    let six = run(&Cfg {
        n_outer: 6,
        ..Cfg::r(40, 200)
    });
    let four = run(&Cfg {
        n_correctors: 4,
        ..Cfg::r(40, 200)
    });
    cont = cont.max(six.continuity).max(four.continuity);
    println!(
        "H3 e_out = max |u_3 - u_6| {:.4e}; e_corr = max |u_2c - u_4c| {:.4e}",
        distance(&runs[1], &six),
        distance(&runs[1], &four)
    );
    // printed: no carry-over
    let off = |n: usize| {
        run(&Cfg {
            ddt_correction: false,
            ..Cfg::r(40, n)
        })
    };
    let (a, b, r) = (off(200), off(400), off(800));
    let (o200, o400) = (distance(&a, &r), distance(&b, &r));
    println!(
        "printed, ddt_correction off: max |d_200| {o200:.4e}, max |d_400| {o400:.4e}, ratio {:.4}",
        o200 / o400
    );
    // H8: the order in h at a fixed step size, and at a fixed nu dt / h^2
    let mut fixed_dt = Vec::new();
    let mut fixed_x = Vec::new();
    for (n, base) in [(20usize, 50usize), (40, 200), (80, 800)] {
        let (a, b, r) = (run(&Cfg::r(n, 200)), run(&Cfg::r(n, 400)), run(&Cfg::r(n, 800)));
        cont = cont.max(a.continuity).max(b.continuity).max(r.continuity);
        let (e200, e400) = (distance(&a, &r), distance(&b, &r));
        println!(
            "H8 R_{n} at dt fixed: max |d_200| {e200:.4e}, max |d_400| {e400:.4e}, ratio {:.4}",
            e200 / e400
        );
        fixed_dt.push(e200);
        let (a, b, r) = if n == 40 {
            (run(&Cfg::r(n, 200)), run(&Cfg::r(n, 400)), run(&Cfg::r(n, 800)))
        } else {
            (
                run(&Cfg::r(n, base)),
                run(&Cfg::r(n, 2 * base)),
                run(&Cfg::r(n, 4 * base)),
            )
        };
        cont = cont.max(a.continuity).max(b.continuity).max(r.continuity);
        let (e1, e2) = (distance(&a, &r), distance(&b, &r));
        println!(
            "H8 R_{n} at nu dt / h^2 fixed ({base}, {}, {} steps): max |d| {e1:.4e}, {e2:.4e}, ratio {:.4}",
            2 * base,
            4 * base,
            e1 / e2
        );
        fixed_x.push(e1);
    }
    println!(
        "H8 order in h at dt fixed: 20 to 40 {:.3}, 40 to 80 {:.3}; at nu dt / h^2 fixed: 20 to 40 {:.3}, 40 to 80 {:.3}",
        (fixed_dt[0] / fixed_dt[1]).log2(),
        (fixed_dt[1] / fixed_dt[2]).log2(),
        (fixed_x[0] / fixed_x[1]).log2(),
        (fixed_x[1] / fixed_x[2]).log2()
    );
    // printed: the periodic box
    let ps = [50usize, 100, 200, 400];
    let pr: Vec<(Vec<Vec3>, f64)> = ps.iter().map(|&n| periodic_run(n)).collect();
    let pd: Vec<f64> = (0..3)
        .map(|i| {
            pr[i]
                .0
                .iter()
                .zip(&pr[i + 1].0)
                .map(|(p, q)| vec3::norm(vec3::sub(*p, *q)) / decay())
                .fold(0.0, f64::max)
        })
        .collect();
    for i in 0..4 {
        println!(
            "printed, periodic box, {} steps: amplitude over exact - 1 {:.6e}",
            ps[i],
            pr[i].1 - 1.0
        );
    }
    println!(
        "printed, periodic box: delta_50 {:.4e}, delta_100 {:.4e}, delta_200 {:.4e}; q_50 {:.4}, q_100 {:.4}; amplitude differences {:.4e}, {:.4e}, {:.4e}",
        pd[0],
        pd[1],
        pd[2],
        (pd[0] / pd[1]).log2(),
        (pd[1] / pd[2]).log2(),
        pr[0].1 - pr[1].1,
        pr[1].1 - pr[2].1,
        pr[2].1 - pr[3].1
    );
    println!("S1 continuity, every step of the runs of R: {cont:.3e} (limit <= 1e-9)");
    assert!(cont <= 1e-9, "S1: continuity {cont:e}");
}

// ---- the corrective C of amendment 1 -----------------------------------------------------------------------------------

/// R with the corrective C.
fn rc(n: usize, steps: usize) -> Cfg {
    Cfg {
        consistent: true,
        ..Cfg::r(n, steps)
    }
}

/// ADR 0245 amendment 1: A1, H7, C1, C2, A3 and C4 on the set-up R.
#[test]
fn a1_rest_remainder_with_the_consistent_face_coefficient() {
    let runs: Vec<Out> = [100usize, 200, 400, 800].iter().map(|&n| run(&rc(40, n))).collect();
    let (d200, d400) = (distance(&runs[1], &runs[3]), distance(&runs[2], &runs[3]));
    let (near, inner) = defect_distance(&runs[1], &runs[3]);
    let q200 = (distance(&runs[1], &runs[2]) / distance(&runs[2], &runs[3])).log2();
    println!(
        "R with C: max |d_200| {d200:.4e}, max |d_400| {d400:.4e}; delta_100 {:.4e}, delta_200 {:.4e}, delta_400 {:.4e}; q_100 {:.4}",
        distance(&runs[0], &runs[1]),
        distance(&runs[1], &runs[2]),
        distance(&runs[2], &runs[3]),
        (distance(&runs[0], &runs[1]) / distance(&runs[1], &runs[2])).log2()
    );
    let default = run(&Cfg::r(40, 800));
    let (ec, ed) = (exact_error(&runs[3]), exact_error(&default));
    println!("R in 800 steps, error against the exact field: with C {ec:.6e}, default {ed:.6e}");
    let mut c = Checks::new();
    c.at_least("A1 (acceptance) R with C, max |d_200| / max |d_400|", d200 / d400, 4.0);
    c.at_most("H7 R with C, max |d_200|", d200, 1.7e-6);
    c.at_most("C1 R with C, D_200 of the face defect", near.0.max(inner.0), 4e-7);
    c.at_least("C2 R with C, q_200", q200, 1.7);
    c.at_most(
        "A3 (no harm) error against the exact field, C over default",
        ec / ed,
        1.05,
    );
    c.at_most(
        "C4 continuity, every step of the runs with C",
        runs.iter().fold(0.0, |m, o| m.max(o.continuity)),
        1e-9,
    );
    c.finish();
}

/// The lid-driven cavity of C3 from rest to `t = 20` in steps of `dt`: the cell velocities, the largest change of the
/// velocity over the last step, the largest continuity.
fn cavity_run(dt: f64, consistent: bool) -> (Vec<Vec3>, f64, f64) {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(20, 1.0).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut set = settings(0.1);
    set.time_scheme = TimeScheme::Bdf2;
    set.n_outer = 3;
    set.rhie_chow_step_consistent = consistent;
    let mut s = PimpleSolver::new(geom, u, p, set);
    let steps = (20.0 / dt).round() as usize;
    let (mut change, mut continuity) = (0.0f64, 0.0f64);
    for k in 0..steps {
        let before = s.u.values.clone();
        let info = s.step(dt);
        continuity = continuity.max(info.continuity);
        if k + 1 == steps {
            change = field_distance(&s.u.values, &before);
        }
    }
    assert!(s.diverged.is_none(), "the cavity run stopped");
    (s.u.values.clone(), change, continuity)
}

fn field_distance(a: &[Vec3], b: &[Vec3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(p, q)| vec3::norm(vec3::sub(*p, *q)))
        .fold(0.0, f64::max)
}

/// ADR 0245 amendment 1, C3 and C3n: the steady solution of the cavity does not depend on the step with C, and does
/// without.
#[test]
fn c3_steady_cavity_does_not_depend_on_the_step() {
    let mut c = Checks::new();
    let (a, ca, conta) = cavity_run(0.04, true);
    let (b, cb, contb) = cavity_run(0.01, true);
    println!("C3 change over the last step: {ca:.3e} (dt 0.04), {cb:.3e} (dt 0.01)");
    c.at_most(
        "C3 steady cavity with C, steps 0.04 against 0.01",
        field_distance(&a, &b),
        1e-9,
    );
    c.at_most("C4 continuity of the cavity runs with C", conta.max(contb), 1e-9);
    let (a, ca, _) = cavity_run(0.04, false);
    let (b, cb, _) = cavity_run(0.01, false);
    println!("C3n change over the last step: {ca:.3e} (dt 0.04), {cb:.3e} (dt 0.01)");
    c.at_least(
        "C3n (negative control) steady cavity without C, steps 0.04 against 0.01",
        field_distance(&a, &b),
        1e-6,
    );
    c.finish();
}

// ---- the overset step (helpers copied from overset_f338.rs) -------------------------------------------------------------

fn tg_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
    }
}

fn tg_background(level: usize) -> Mesh {
    let bs = uniform(0.0, PI, 40 * level);
    rectilinear_2d(&bs, &bs, DEPTH).unwrap()
}

fn box_overset(bg: Mesh, half: f64, n: usize, hole: f64) -> Overset {
    let cs = uniform(-half, 2.0 * half, n);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
    };
    Overset::new(composite, spec).unwrap()
}

/// The settings of the item with C: PS, C on; BDF2 and three outer iterations are set on the solver.
fn settings_c() -> PimpleSettings {
    let mut set = settings(NU_TG);
    set.rhie_chow_step_consistent = true;
    set
}

/// ADR 0245 amendment 1, E1 and E1n: with every cell calculated on a mesh at rest the overset step with C is the
/// library's step with C (TGB_1 of ADR 0241 Q0b, 40 steps of 0.0025, BDF2, three outer iterations).
#[test]
fn e1_overset_at_rest_equals_the_library_with_the_consistent_coefficient() {
    let dt = 0.0025;
    let library = |consistent: bool| -> PimpleSolver {
        let geom = Arc::new(FvGeometry::new(Arc::new(tg_background(1))).unwrap());
        let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
        tg_conditions(&geom, &mut u, 0.0);
        let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
        let mut set = settings(NU_TG);
        set.time_scheme = TimeScheme::Bdf2;
        set.n_outer = 3;
        set.rhie_chow_step_consistent = consistent;
        let mut s = PimpleSolver::new(geom.clone(), u, p, set);
        for k in 1..=40 {
            tg_conditions(&geom, &mut s.u, k as f64 * dt);
            s.step(dt);
        }
        s
    };
    let a = library(true);
    let geom = Arc::new(FvGeometry::new(Arc::new(tg_background(1))).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut b = OversetSolver::new(geom, u, p, settings_c(), MassMode::Conservative, None).unwrap();
    b.set_time_scheme(TimeScheme::Bdf2).unwrap();
    b.set_outer_iterations(3, 1.0, 1.0).unwrap();
    for k in 1..=40 {
        let st = move_mesh(&b.geom, b.geom.mesh.points().to_vec(), dt).unwrap();
        tg_conditions(&st.geom, &mut b.u, k as f64 * dt);
        b.advance_mesh(dt, st, Vec::new()).unwrap();
    }
    let du = field_distance(&a.u.values, &b.u.values);
    let dp =
        a.p.values
            .iter()
            .zip(&b.p.values)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
    let phi_max = a.phi.iter().map(|x| x.abs()).fold(0.0, f64::max);
    let dphi = a.phi.iter().zip(&b.phi).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / phi_max;
    println!(
        "E1 bit for bit: {}",
        a.u.values == b.u.values && a.p.values == b.p.values && a.phi == b.phi
    );
    let mut c = Checks::new();
    c.at_most("E1 overset at rest against the library, both with C, u", du, 1e-12);
    c.at_most("E1 p", dp, 1e-12);
    c.at_most("E1 phi", dphi, 1e-12);
    c.at_least(
        "E1n (negative control) the library with C against the library without, u",
        field_distance(&a.u.values, &library(false).u.values),
        1e-9,
    );
    c.finish();
}

/// TG_2 of ADR 0219 with BDF2, three outer iterations and C, `steps` steps to `t = 0.5`: the solver, the error `e` of
/// ADR 0219 over the non-hole cells and the largest continuity.
fn tg2_run(steps: usize) -> (OversetSolver, f64, f64) {
    let t0 = std::time::Instant::now();
    let dt = T_END / steps as f64;
    let ov = box_overset(tg_background(2), 0.6, 40, 0.25);
    let pose = tg_pose(0.0);
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let coupling = Some(OversetCoupling { overset: ov, pose });
    let mut s = OversetSolver::new(geom, u, p, settings_c(), MassMode::Conservative, coupling).unwrap();
    s.set_time_scheme(TimeScheme::Bdf2).unwrap();
    s.set_outer_iterations(3, 1.0, 1.0).unwrap();
    let mut cont = 0.0f64;
    for k in 1..=steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let info = s.step(dt, tg_pose(t), &[]).unwrap();
        cont = cont.max(info.continuity);
    }
    let e = (0..s.geom.n_cells())
        .filter(|&c| s.conn.kind[c] != CellKind::Hole)
        .map(|c| vec3::norm(vec3::sub(s.u.values[c], tg_u(s.geom.mesh.cell_centres()[c], T_END))) / decay())
        .fold(0.0, f64::max);
    println!(
        "TG_2 {steps} steps with C ({:.1} s): e {e:.4e}, continuity {cont:.2e}",
        t0.elapsed().as_secs_f64()
    );
    (s, e, cont)
}

/// The largest difference of the velocities of two runs of TG_2 over the non-hole cells, over the decay, with its cell.
fn tg2_difference(tag: &str, a: &OversetSolver, b: &OversetSolver) -> f64 {
    assert!(a.conn.kind == b.conn.kind, "the two runs end at different kinds");
    let mut best = (0.0, 0);
    for c in 0..a.geom.n_cells() {
        if a.conn.kind[c] != CellKind::Hole {
            let d = vec3::norm(vec3::sub(a.u.values[c], b.u.values[c])) / decay();
            if d > best.0 {
                best = (d, c);
            }
        }
    }
    let x = b.geom.mesh.cell_centres()[best.1];
    println!(
        "{tag}: max |d| {:.4e} in cell {} ({:?}, {}) at ({:.3}, {:.3})",
        best.0,
        best.1,
        b.conn.kind[best.1],
        if best.1 < b.conn.n_background_cells {
            "background"
        } else {
            "component"
        },
        x[0],
        x[1]
    );
    best.0
}

/// The numbers of A2, A2b and A2c: `max |d_200|`, `max |d_400|`, `e` in 800 steps, the largest continuity.
fn tg2_numbers() -> (f64, f64, f64, f64) {
    let (a, _, ca) = tg2_run(200);
    let (b, _, cb) = tg2_run(400);
    let (r, e, cr) = tg2_run(800);
    (
        tg2_difference("TG_2 with C, d_200", &a, &r),
        tg2_difference("TG_2 with C, d_400", &b, &r),
        e,
        ca.max(cb).max(cr),
    )
}

/// ADR 0245 amendment 1, A2, A2b and A2c: D2 of ADR 0241 on TG_2 with C.
#[test]
#[ignore = "slow (three level-2 overset runs, about 6 minutes): run once for the addendum"]
fn a2_tg2_ratio_with_the_consistent_face_coefficient() {
    let (d200, d400, e, cont) = tg2_numbers();
    let mut c = Checks::new();
    c.at_least(
        "A2 (acceptance) TG_2 with C, max |d_200| / max |d_400|",
        d200 / d400,
        4.0,
    );
    c.at_most("A2b TG_2 with C, max |d_200|", d200, 2.125e-5);
    c.at_most("A2c continuity of the calculated cells, every step", cont, 1e-9);
    c.at_most(
        "A2c e(TG_2) in 800 steps with C over the default's 3.179e-4",
        e / 3.179e-4,
        1.05,
    );
    c.finish();
}
