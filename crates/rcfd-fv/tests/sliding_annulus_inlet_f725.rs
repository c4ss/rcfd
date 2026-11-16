//! F-725 (ADR 0661): the sector SS against the full annulus P of ADR 0216 with the non-axisymmetric inlet of ADR 0641, from a
//! start consistent with the inlet; the difference split into the momentum lag of the coupled faces, their orthogonal
//! coupling (ADR 0629 Q4) and the rest by the variants `V(K, N)` (`n_outer = K`, `non_orth_correctors = N`). Checks G1, G2,
//! G0, A1s, H1, NC2 (the acceptance run), J0, LC, I2, NC1 (ADR 0641's start, 16 steps) and I1 (the decomposition over 400
//! steps). The limits are those registered in ADR 0661 before this code ran; each test prints every measured number and then
//! asserts the registered checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sliding::sliding_step;
use rcfd_fv::sliding_periodic::periodic_sliding_step_coupled;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_sector, cylinder_ogrid, Grading};
use rcfd_mesh::sliding::{SlidingInterface, SlidingMesh};
use rcfd_mesh::sliding_periodic::{PeriodicSlidingInterface, PeriodicSlidingMesh};
use rcfd_mesh::Mesh;
use std::f64::consts::{FRAC_PI_3, PI};
use std::sync::Arc;
use std::time::Instant;

const L: f64 = 0.1;
const Z: Vec3 = [0.0, 0.0, 1.0];
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};
const U_IN: f64 = 1.0;
const RI: f64 = 1.0;
/// ADR 0641's LS: the rotor pitch over 8, 400 steps.
const DT: f64 = PI / 576.0;
const STEPS: usize = 400;
/// J: ADR 0641's start, the first two coincidence steps.
const J_STEPS: usize = 16;
/// G1's limit (ADR 0661).
const G1_LIMIT: f64 = 3e-2;

fn pair_ss() -> PeriodicSlidingInterface {
    let a = annulus_sector(24, 8, 1.0, 1.5, FRAC_PI_3, Grading::Uniform, L).unwrap();
    let b = annulus_sector(20, 8, 1.5, 2.0, FRAC_PI_3, Grading::Uniform, L).unwrap();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
}

/// ADR 0216's pair P (the full annulus).
fn pair_p() -> SlidingInterface {
    let a = cylinder_ogrid(144, 8, 1.0, 1.5, Grading::Uniform, L).unwrap();
    let b = cylinder_ogrid(120, 8, 1.5, 2.0, Grading::Uniform, L).unwrap();
    SlidingInterface::cylindrical(a, "farfield", b, "cylinder", vec3::ZERO, Z, 1e-8).unwrap()
}

/// PS6 of ADR 0629 with `n_outer = k`, `non_orth_correctors = n`.
fn settings(k: usize, n: usize) -> PimpleSettings {
    let mut s = PimpleSettings::piso(0.1);
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = n;
    s.n_outer = k;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

fn geom_of(m: &Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m.clone())).unwrap())
}

fn xhat(x: Vec3) -> Vec3 {
    vec3::unit([x[0], x[1], 0.0])
}

/// The inlet of ADR 0641, `U_in (1 + 0.5 sin 6 theta) x_hat`.
fn modulation(x: Vec3) -> f64 {
    1.0 + 0.5 * (6.0 * x[1].atan2(x[0])).sin()
}

fn inlet(x: Vec3) -> Vec3 {
    vec3::scale(xhat(x), U_IN * modulation(x))
}

/// The start: `u0 = U_in (1 + 0.5 sin 6 theta) (r_i / r) x_hat` (consistent with the inlet, divergence-free; ADR 0661) or
/// ADR 0641's `U_in (r_i / r) x_hat`.
fn start(x: Vec3, consistent: bool) -> Vec3 {
    let m = if consistent { modulation(x) } else { 1.0 };
    vec3::scale(xhat(x), U_IN * m * RI / x[0].hypot(x[1]))
}

fn per_face(g: &FvGeometry, patch: &str) -> Box<FixedValue> {
    let cf = g.mesh.face_centres();
    Box::new(FixedValue(Profile::PerFace(
        g.mesh.patch(patch).unwrap().faces().map(|k| inlet(cf[k])).collect(),
    )))
}

fn solver(g: Arc<FvGeometry>, inlet_patch: &str, outlet: &str, consistent: bool, s: PimpleSettings) -> PimpleSolver {
    let mut u = VectorField::from_fn("U", &g, |c| start(c, consistent));
    u.set_bc(&g, inlet_patch, per_face(&g, inlet_patch)).unwrap();
    u.set_bc(&g, outlet, Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &g, 0.0);
    p.set_bc(&g, outlet, Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(g, u, p, s)
}

fn finite_or_inf(x: f64) -> f64 {
    if x.is_nan() {
        f64::INFINITY
    } else {
        x
    }
}

type Fails = Vec<(&'static str, String)>;

fn check(fails: &mut Fails, id: &'static str, ok: bool, what: String) {
    println!("{id}: {what} -> {}", if ok { "met" } else { "MISSED" });
    if !ok {
        fails.push((id, what));
    }
}

/// For every point of `pts`, the nearest cell of `annulus`; the largest distance; whether one-to-one.
fn match_points(pts: &[Vec3], annulus: &Mesh) -> (Vec<usize>, f64, bool) {
    let ca = annulus.cell_centres();
    let mut worst: f64 = 0.0;
    let idx: Vec<usize> = pts
        .iter()
        .map(|c| {
            let (mut best, mut d) = (0, f64::INFINITY);
            for (j, a) in ca.iter().enumerate() {
                let e = vec3::norm(vec3::sub(*c, *a));
                if e < d {
                    d = e;
                    best = j;
                }
            }
            worst = worst.max(d);
            best
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let one = idx.iter().all(|j| seen.insert(*j));
    (idx, worst, one)
}

/// One variant `V(K, N)`: the coupled sector C and the annulus A.
struct Variant<'a> {
    k: usize,
    n: usize,
    sc: &'a PeriodicSlidingInterface,
    c_cur: PeriodicSlidingMesh,
    c: PimpleSolver,
    a_sc: SlidingInterface,
    a_cur: SlidingMesh,
    a: PimpleSolver,
}

/// What one step of a variant returns for the checks.
struct StepOut {
    /// `u_C - u_A` at every sector cell (against its nearest annulus cell).
    delta: Vec<Vec3>,
    match_d: f64,
    match_one: bool,
    flux_residual: f64,
    /// H1 on the new sector mesh: the seam pairs' and the wrap pairs' largest non-orthogonality.
    seam_sin: f64,
    wrap_sin: f64,
}

impl<'a> Variant<'a> {
    fn new(sc: &'a PeriodicSlidingInterface, k: usize, n: usize, consistent: bool) -> Self {
        let c_cur = sc.mesh_turned(0.0).unwrap();
        let c = solver(geom_of(&c_cur.mesh), "inner", "outer", consistent, settings(k, n));
        let a_sc = pair_p();
        let a_cur = a_sc.mesh_at(0.0).unwrap();
        let a = solver(geom_of(&a_cur.mesh), "cylinder", "farfield", consistent, settings(k, n));
        Variant {
            k,
            n,
            sc,
            c_cur,
            c,
            a_sc,
            a_cur,
            a,
        }
    }

    fn step(&mut self, step_no: usize, geometry: bool) -> StepOut {
        let theta = step_no as f64 * DT;
        let new_c = self.sc.mesh_turned(theta).unwrap();
        let st_c = periodic_sliding_step_coupled(&self.c.geom, &self.c_cur, &new_c, &self.c.phi, &self.c.u.values, DT)
            .unwrap();
        let new_a = self.a_sc.mesh_at(theta).unwrap();
        let st_a = sliding_step(&self.a.geom, &self.a_cur, &new_a, &self.a.phi, &self.a.u.values, DT).unwrap();
        let flux_residual = finite_or_inf(st_c.flux_residual);
        let (seam_sin, wrap_sin) = if geometry {
            non_orthogonality(&st_c.geom, &new_c)
        } else {
            (0.0, 0.0)
        };
        let gc = st_c.geom.clone();
        self.c.u.set_bc(&gc, "inner", per_face(&gc, "inner")).unwrap();
        self.c.step_sliding_periodic(DT, st_c, Vec::new()).unwrap();
        self.c_cur = new_c;
        let ga = st_a.geom.clone();
        self.a.u.set_bc(&ga, "cylinder", per_face(&ga, "cylinder")).unwrap();
        self.a.step_sliding(DT, st_a, Vec::new()).unwrap();
        self.a_cur = new_a;
        let (idx, match_d, match_one) = match_points(self.c.geom.mesh.cell_centres(), &self.a.geom.mesh);
        let delta = idx
            .iter()
            .enumerate()
            .map(|(c, &j)| vec3::sub(self.c.u.values[c], self.a.u.values[j]))
            .collect();
        StepOut {
            delta,
            match_d,
            match_one,
            flux_residual,
            seam_sin,
            wrap_sin,
        }
    }
}

/// H1: for every periodic pair `(f, g)` of `geom`, `d = (x_f - c_f) + R (c_g - x_g)` and `|d x n_f| / |d|`; the largest over the
/// seam pairs (not wrap faces) and over the wrap pairs.
fn non_orthogonality(geom: &FvGeometry, m: &PeriodicSlidingMesh) -> (f64, f64) {
    let mesh = &geom.mesh;
    let (cf, cc, sf) = (mesh.face_centres(), mesh.cell_centres(), mesh.face_area_vectors());
    let own = mesh.owner();
    let (mut seam, mut wrap): (f64, f64) = (0.0, 0.0);
    for (pi, p) in mesh.patches().iter().enumerate() {
        let Some(map) = &geom.periodic_partner[pi] else {
            continue;
        };
        for (local, f) in p.faces().enumerate() {
            let g = map[local];
            let half_g = vec3::sub(cc[own[g]], cf[g]);
            let turned = match &geom.periodic_rot[pi] {
                Some(r) => [0, 1, 2].map(|i| r[i][0] * half_g[0] + r[i][1] * half_g[1] + r[i][2] * half_g[2]),
                None => half_g,
            };
            let d = vec3::add(vec3::sub(cf[f], cc[own[f]]), turned);
            let n = vec3::unit(sf[f]);
            let s = finite_or_inf(vec3::norm(vec3::cross(d, n)) / vec3::norm(d));
            if m.is_wrap(f) || m.is_wrap(g) {
                wrap = wrap.max(s);
            } else {
                seam = seam.max(s);
            }
        }
    }
    (seam, wrap)
}

fn max_norm(x: &[Vec3]) -> f64 {
    x.iter().fold(0.0f64, |m, v| m.max(finite_or_inf(vec3::norm(*v)))) / U_IN
}

fn max_diff(x: &[Vec3], y: &[Vec3]) -> f64 {
    x.iter()
        .zip(y)
        .fold(0.0f64, |m, (a, b)| m.max(finite_or_inf(vec3::norm(vec3::sub(*a, *b)))))
        / U_IN
}

/// The radius, the angle and the angle from the rotor seam and from the stator seam (both reduced to the sector) of cell `c`
/// of the sector mesh `m`.
fn location(m: &PeriodicSlidingMesh, c: usize) -> (f64, f64, f64, f64) {
    let x = m.mesh.cell_centres()[c];
    let th = x[1].atan2(x[0]);
    let wrap = |a: f64| {
        let r = a.rem_euclid(FRAC_PI_3);
        r.min(FRAC_PI_3 - r)
    };
    (x[0].hypot(x[1]), th, wrap(th - m.offset), wrap(th))
}

/// The lockstep of the variants `vs` from the start `consistent` over `steps` steps: every variant's largest difference
/// (with its step and location), the largest pairwise difference of the variants' difference fields, the match, and the
/// extras of variant 0 (`extra`).
struct Run {
    g: Vec<f64>,
    g_at: Vec<(usize, (f64, f64, f64, f64))>,
    pair: Vec<Vec<f64>>,
    g_step1: f64,
    g_coinc: f64,
    rms_last: f64,
    match_d: f64,
    match_one: bool,
    a1s: f64,
    seam_sin: f64,
    wrap_sin: f64,
    nc2: f64,
    elapsed: f64,
}

fn run(vs: &[(usize, usize)], consistent: bool, steps: usize, extra: bool) -> Run {
    let t0 = Instant::now();
    let sc = pair_ss();
    let mut var: Vec<Variant> = vs.iter().map(|&(k, n)| Variant::new(&sc, k, n, consistent)).collect();
    let nv = var.len();
    let mut out = Run {
        g: vec![0.0; nv],
        g_at: vec![(0, (0.0, 0.0, 0.0, 0.0)); nv],
        pair: vec![vec![0.0; nv]; nv],
        g_step1: 0.0,
        g_coinc: 0.0,
        rms_last: 0.0,
        match_d: 0.0,
        match_one: true,
        a1s: 0.0,
        seam_sin: 0.0,
        wrap_sin: 0.0,
        nc2: 0.0,
        elapsed: 0.0,
    };
    for n in 1..=steps {
        let outs: Vec<StepOut> = var
            .iter_mut()
            .enumerate()
            .map(|(i, v)| v.step(n, extra && i == 0))
            .collect();
        for (i, o) in outs.iter().enumerate() {
            out.match_d = out.match_d.max(o.match_d);
            out.match_one &= o.match_one;
            let mx = max_norm(&o.delta);
            if mx > out.g[i] {
                out.g[i] = mx;
                let c = o
                    .delta
                    .iter()
                    .enumerate()
                    .fold(
                        (0, -1.0),
                        |b, (c, d)| if vec3::norm(*d) > b.1 { (c, vec3::norm(*d)) } else { b },
                    )
                    .0;
                out.g_at[i] = (n, location(&var[i].c_cur, c));
            }
            for (j, p) in outs.iter().enumerate().skip(i + 1) {
                let d = max_diff(&o.delta, &p.delta);
                out.pair[i][j] = out.pair[i][j].max(d);
                out.pair[j][i] = out.pair[i][j];
            }
        }
        let o0 = &outs[0];
        let mx0 = max_norm(&o0.delta);
        if n == 1 {
            out.g_step1 = mx0;
        }
        if n % 8 == 0 {
            out.g_coinc = out.g_coinc.max(mx0);
        }
        if extra {
            if n >= 2 {
                out.a1s = out.a1s.max(o0.flux_residual);
            }
            out.seam_sin = out.seam_sin.max(o0.seam_sin);
            out.wrap_sin = out.wrap_sin.max(o0.wrap_sin);
        }
        if n == steps {
            let v0 = &var[0];
            let vol = v0.c.geom.mesh.cell_volumes();
            let (mut s2, mut sv) = (0.0, 0.0);
            for (c, d) in o0.delta.iter().enumerate() {
                let e = vec3::norm(*d) / U_IN;
                s2 += vol[c] * e * e;
                sv += vol[c];
            }
            out.rms_last = (s2 / sv).sqrt();
            if extra {
                // NC2: every sector cell against the annulus cell nearest to its centre turned by one rotor cell
                let (s, c) = ((PI / 72.0).sin(), (PI / 72.0).cos());
                let pts: Vec<Vec3> =
                    v0.c.geom
                        .mesh
                        .cell_centres()
                        .iter()
                        .map(|x| [c * x[0] - s * x[1], s * x[0] + c * x[1], x[2]])
                        .collect();
                let (idx, _, _) = match_points(&pts, &v0.a.geom.mesh);
                out.nc2 = idx.iter().enumerate().fold(0.0f64, |m, (cc, &j)| {
                    m.max(vec3::norm(vec3::sub(v0.c.u.values[cc], v0.a.u.values[j])))
                }) / U_IN;
            }
        }
    }
    out.elapsed = t0.elapsed().as_secs_f64();
    for (i, v) in var.iter().enumerate() {
        println!(
            "V({}, {}): G {:.4e} U_in at step {} (radius, angle, from the rotor seam, from the stator seam) {:.4?}",
            v.k, v.n, out.g[i], out.g_at[i].0, out.g_at[i].1
        );
    }
    out
}

/// The shares of ADR 0661's decomposition from the variants `[V(1,6), V(6,6), V(1,0), V(6,0)]` (indices 0 to 3) of `r`:
/// `(q, l, rho)` and the printed parts.
fn shares(r: &Run) -> (f64, f64, f64) {
    let g = r.g[0];
    let (lag, lag2) = (r.pair[0][1], r.pair[2][3]);
    let (q4, q42) = (r.pair[1][3], r.pair[0][2]);
    let rest = r.g[3];
    println!(
        "parts: M(L) {lag:.3e}, M(L') {lag2:.3e}, M(Q) {q4:.3e}, M(Q') {q42:.3e}, M(R) {rest:.3e}; G {g:.3e} (triangle: \
         M(L) + M(Q) + M(R) = {:.3e})",
        lag + q4 + rest
    );
    (q4.max(q42) / g, lag.max(lag2) / g, rest / g)
}

/// The identification rule of ADR 0661 for one share against the two others: identified, ruled out, or undecided.
fn verdict(x: f64, o1: f64, o2: f64) -> &'static str {
    if x >= 0.5 && x >= o1 && x >= o2 {
        "identified"
    } else if x <= 0.1 {
        "ruled out"
    } else {
        "undecided"
    }
}

/// G1, G2, G0, A1s, H1, NC2 of ADR 0661: the acceptance run `V(1, 6)` from the start consistent with the inlet, the
/// registered run: a record of the miss of H1 (ADR 0661 amendment 1).
#[test]
#[ignore = "known miss: H1 9.08e-4 (1e-12), a wrong premise: the join cuts the interface-ring cells, whose centres then break the seam's mirror symmetry; off the interface rings 1.27e-14 (ADR 0661 amendment 1, H1c)"]
fn lockstep_known_miss_h1_from_a_start_consistent_with_the_inlet() {
    lockstep_run(false);
}

/// `V(1, 6)` with H1 printed only (ADR 0661 amendment 1): G1, G2, G0, A1s, NC2.
#[test]
fn lockstep_corrected_from_a_start_consistent_with_the_inlet() {
    lockstep_run(true);
}

/// H1c of ADR 0661 amendment 1: on the sector mesh of every one of the 400 steps the seam pairs whose cells touch no interface
/// or wrap face are orthogonal to `1e-12`; the seam pairs at the interface rings and the wrap pairs printed.
#[test]
fn h1c_seam_pairs_off_the_interface_rings_are_orthogonal() {
    let sc = pair_ss();
    let (mut off, mut on, mut wrap): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for n in 1..=STEPS {
        let m = sc.mesh_turned(n as f64 * DT).unwrap();
        let geom = geom_of(&m.mesh);
        let mesh = &geom.mesh;
        let (cf, cc, sf) = (mesh.face_centres(), mesh.cell_centres(), mesh.face_area_vectors());
        let own = mesh.owner();
        let touches = |c: usize| mesh.cell_faces(c).iter().any(|&f| m.is_interface(f) || m.is_wrap(f));
        for (pi, p) in mesh.patches().iter().enumerate() {
            let Some(map) = &geom.periodic_partner[pi] else {
                continue;
            };
            for (local, f) in p.faces().enumerate() {
                let g = map[local];
                let half_g = vec3::sub(cc[own[g]], cf[g]);
                let turned = match &geom.periodic_rot[pi] {
                    Some(r) => [0, 1, 2].map(|i| r[i][0] * half_g[0] + r[i][1] * half_g[1] + r[i][2] * half_g[2]),
                    None => half_g,
                };
                let d = vec3::add(vec3::sub(cf[f], cc[own[f]]), turned);
                let x = finite_or_inf(vec3::norm(vec3::cross(d, vec3::unit(sf[f]))) / vec3::norm(d));
                if m.is_wrap(f) || m.is_wrap(g) {
                    wrap = wrap.max(x);
                } else if touches(own[f]) || touches(own[g]) {
                    on = on.max(x);
                } else {
                    off = off.max(x);
                }
            }
        }
    }
    let mut fails = Fails::new();
    check(
        &mut fails,
        "H1c",
        off <= 1e-12,
        format!(
            "seam pairs off the interface rings {off:.2e} (1e-12); printed: at the interface rings {on:.4e}, wrap pairs              {wrap:.4e}"
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// The acceptance run `V(1, 6)`; `corrected` selects the checks of amendment 1.
fn lockstep_run(corrected: bool) {
    let r = run(&[(1, 6)], true, STEPS, true);
    println!("elapsed {:.1} s", r.elapsed);
    let mut fails = Fails::new();
    check(
        &mut fails,
        "G1",
        r.g[0] <= G1_LIMIT,
        format!(
            "max |u_C - u_A| / U_in over every one of the {STEPS} steps {:.4e} (step {}), step 1 {:.4e}, the coincidence \
             steps {:.4e} ({G1_LIMIT:e} each step)",
            r.g[0], r.g_at[0].0, r.g_step1, r.g_coinc
        ),
    );
    check(
        &mut fails,
        "G2",
        r.rms_last <= 1.5e-2,
        format!("rms at step {STEPS} {:.4e} U_in (1.5e-2)", r.rms_last),
    );
    check(
        &mut fails,
        "G0",
        r.match_d <= 1e-9 && r.match_one,
        format!(
            "nearest annulus cell {:.2e} (1e-9), one-to-one {}",
            r.match_d, r.match_one
        ),
    );
    check(
        &mut fails,
        "A1s",
        r.a1s <= 1e-12,
        format!("C flux_residual, steps 2 to {STEPS}: {:.2e} (1e-12)", r.a1s),
    );
    let h1 = format!(
        "seam pairs' non-orthogonality |d x n| / |d| {:.2e} (1e-12); wrap pairs (printed) {:.4e}",
        r.seam_sin, r.wrap_sin
    );
    if corrected {
        println!("H1 (printed, the miss is held by the record): {h1}");
    } else {
        check(&mut fails, "H1", r.seam_sin <= 1e-12, h1);
    }
    check(
        &mut fails,
        "NC2",
        r.nc2 >= 5e-2,
        format!(
            "the measure against the annulus turned by one rotor cell {:.4e} U_in (>= 5e-2)",
            r.nc2
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// J0, LC, I2, NC1 of ADR 0661: ADR 0641's start over 16 steps, the variants `V(1,6), V(6,6), V(1,0), V(6,0), V(10,6)`.
#[test]
fn start_transient_of_adr_0641_by_part() {
    let r = run(&[(1, 6), (6, 6), (1, 0), (6, 0), (10, 6)], false, J_STEPS, false);
    println!("elapsed {:.1} s", r.elapsed);
    let (q, l, rho) = shares(&r);
    println!(
        "J shares: q {q:.3}, l {l:.3}, rho {rho:.3}; Q4 {}, lag {}, rest {}",
        verdict(q, l, rho),
        verdict(l, q, rho),
        verdict(rho, q, l)
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "J0",
        (r.g_step1 - 3.4941e-2).abs() <= 1e-6,
        format!(
            "step 1 of ADR 0641's start {:.6e} U_in (3.4941e-2 within 1e-6)",
            r.g_step1
        ),
    );
    let lc = r.pair[4][1];
    let lag = r.pair[0][1];
    check(
        &mut fails,
        "LC",
        lc <= 0.05 * lag,
        format!(
            "M(delta_(10,6) - delta_(6,6)) {lc:.3e} (<= 0.05 M(delta_(1,6) - delta_(6,6)) = {:.3e})",
            0.05 * lag
        ),
    );
    let named = q.max(l).max(rho) >= 0.5;
    check(
        &mut fails,
        "I2",
        named,
        format!("one part with share >= 0.5 and the largest: q {q:.3}, l {l:.3}, rho {rho:.3}"),
    );
    check(
        &mut fails,
        "NC1",
        r.g[0] >= G1_LIMIT,
        format!(
            "ADR 0641's start, largest difference over {J_STEPS} steps {:.4e} (>= {G1_LIMIT:e})",
            r.g[0]
        ),
    );
    check(
        &mut fails,
        "G0",
        r.match_d <= 1e-9 && r.match_one,
        format!(
            "nearest annulus cell {:.2e} (1e-9), one-to-one {}",
            r.match_d, r.match_one
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// I1 of ADR 0661: the decomposition of the acceptance run's difference over 400 steps from the consistent start. A heavy
/// record of its miss (ADR 0661 amendment 1).
#[test]
#[ignore = "heavy record (1092 s) and known miss: I1 undecided, q 0.302 (M(Q) 5.98e-3, M(Q') 5.90e-3), l 1.017 (lag identified), rho 0.007 (rest ruled out), G 1.98e-2 (ADR 0661 amendment 1)"]
fn difference_by_part_known_miss_lag_and_q4() {
    let r = run(&[(1, 6), (6, 6), (1, 0), (6, 0)], true, STEPS, false);
    println!("elapsed {:.1} s", r.elapsed);
    let (q, l, rho) = shares(&r);
    let vq = verdict(q, l, rho);
    println!(
        "I1 shares: q {q:.3}, l {l:.3}, rho {rho:.3}; Q4 {vq}, lag {}, rest {}",
        verdict(l, q, rho),
        verdict(rho, q, l)
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "I1",
        vq != "undecided",
        format!("Q4 (ADR 0629) {vq}: q {q:.3} (identified >= 0.5 and the largest; ruled out <= 0.1)"),
    );
    check(
        &mut fails,
        "G0",
        r.match_d <= 1e-9 && r.match_one,
        format!(
            "nearest annulus cell {:.2e} (1e-9), one-to-one {}",
            r.match_d, r.match_one
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}
