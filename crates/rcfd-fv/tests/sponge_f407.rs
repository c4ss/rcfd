//! F-407 (ADR 0344): the matched sponge on a mesh that moves along the layer and the plain sponge on a rotating mesh.
//! X30, X45 limited plane pulses through a matched layer on a mesh translating along it (heavy); R a plain radial layer on
//! a rotating O-grid (heavy); I1 the grazing instability of the attached memory in one dimension; G3 the transport of the
//! memories of the lab frame against a closed form; U1L, U2r uniform states; B3 bit identities; S5 the frame setting.
//! Limits are the registered ones of ADR 0344.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::compressible::{to_prim, CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::compressible_bc::TotalInlet;
use rcfd_fv::sponge::{
    rate_max_for_reflection, MatchedOptions, Sponge, SpongeForm, SpongeFrame, SpongeLayer, SpongeMemories,
};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{box_2d, cylinder_ogrid, warp, Grading};
use rcfd_mesh::motion::moved_mesh;
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;

const AIR: Gas = Gas { gamma: 1.4 };
const GAMMA: f64 = 1.4;
/// Ambient pressure: with `rho0 = 1` the sound speed is 1.
const P0: f64 = 1.0 / 1.4;
/// Perturbation amplitude (ADR 0237).
const EPS: f64 = 1e-4 * P0;
/// Width of the pulses.
const W: f64 = 0.04;
/// Cell size of the reflection checks.
const DX: f64 = 0.005;
/// The layer of ADR 0237 D7.
const LAYER: f64 = 0.2;
const EXPONENT: f64 = 2.0;
const R0: f64 = 1e-6;
/// The acceptance limit of F-407.
const LIMIT: f64 = 0.01;
/// The bound of the boundedness checks (ADR 0344 reading 5).
const BOUND: f64 = 1.5;
/// The mesh velocity along the layer (ADR 0344 reading 4).
const V_ALONG: Vec3 = [0.0, 0.2, 0.0];
/// Round-off (ADR 0265 U1).
const ROUND_OFF: f64 = 1e-12;

fn geom(m: Mesh) -> FvGeometry {
    FvGeometry::new(Arc::new(m)).unwrap()
}

fn ambient() -> Prim {
    Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: P0,
    }
}

fn pressure(s: &CompressibleSolver, cell: usize) -> f64 {
    to_prim(s.gas, &s.u[cell]).p
}

/// The largest `|p - p0| / eps` over the cells (`B` of ADR 0344).
fn b_all(s: &CompressibleSolver) -> f64 {
    (0..s.u.len()).fold(0.0f64, |m, c| m.max((pressure(s, c) - P0).abs() / EPS))
}

/// The isentropic simple wave of ADR 0237 with the pressure perturbation `dp` along the unit vector `d`.
fn simple_wave(dp: f64, d: Vec3) -> Prim {
    let a = ((P0 + dp) / P0).powf((GAMMA - 1.0) / (2.0 * GAMMA));
    let v = 2.0 * (a - 1.0) / (GAMMA - 1.0);
    Prim {
        rho: ((P0 + dp) / P0).powf(1.0 / GAMMA),
        u: vec3::scale(d, v),
        p: P0 + dp,
    }
}

/// The zero-mean pulse of ADR 0344 DR6 (peak 1).
fn g1(s: f64) -> f64 {
    (s / W) * (-(s / W).powi(2)).exp() / 0.428882
}

fn moved(mesh0: &Mesh, f: impl Fn(Vec3) -> Vec3) -> FvGeometry {
    let points: Vec<Vec3> = mesh0.points().iter().map(|&p| f(p)).collect();
    geom(moved_mesh(mesh0, points).unwrap())
}

fn registered_layer(front: f64) -> SpongeLayer {
    SpongeLayer {
        origin: [front, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        thickness: LAYER,
        rate_max: rate_max_for_reflection(LAYER, 1.0, R0, EXPONENT).unwrap(),
        exponent: EXPONENT,
    }
}

const THOMPSON: CompBc = CompBc::NonReflecting { p: P0, sigma: 0.0 };

/// A run of a lockstep group: the grid at `t = 0`, the solver, whether it still steps, and the largest `B`.
struct Run {
    mesh0: Mesh,
    s: CompressibleSolver,
    alive: bool,
    stopped_at: Option<f64>,
    bmax: f64,
}

impl Run {
    fn new(mesh0: Mesh, s: CompressibleSolver) -> Self {
        Run {
            mesh0,
            s,
            alive: true,
            stopped_at: None,
            bmax: 0.0,
        }
    }
}

/// The motion of a group.
#[derive(Clone, Copy)]
enum Motion {
    Static,
    Translate(Vec3),
    Rotate(f64),
}

fn place(p: Vec3, motion: Motion, t: f64) -> Vec3 {
    match motion {
        Motion::Static => p,
        Motion::Translate(v) => vec3::add(p, vec3::scale(v, t)),
        Motion::Rotate(omega) => {
            let (c, s) = ((omega * t).cos(), (omega * t).sin());
            [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]]
        }
    }
}

/// Advance the live runs by `dt` in parallel threads; a run whose step fails, whose state is not finite or whose `B`
/// exceeds 100 is stopped (ADR 0344 DR6). `B` is updated.
fn advance(runs: &mut [&mut Run], dt: f64, motion: Motion) {
    std::thread::scope(|scope| {
        for r in runs.iter_mut() {
            if !r.alive {
                continue;
            }
            scope.spawn(move || {
                let t_new = r.s.time + dt;
                let ok = match motion {
                    Motion::Static => r.s.step(dt).is_ok(),
                    _ => {
                        let g = moved(&r.mesh0, |p| place(p, motion, t_new));
                        r.s.step_ale(g, dt).is_ok()
                    }
                };
                let b = if ok { b_all(&r.s) } else { f64::NAN };
                if b.is_finite() {
                    r.bmax = r.bmax.max(b);
                }
                if !ok || !b.is_finite() || b > 100.0 {
                    r.alive = false;
                    r.stopped_at = Some(t_new);
                    if b.is_finite() {
                        r.bmax = r.bmax.max(b);
                    } else {
                        r.bmax = f64::INFINITY;
                    }
                }
            });
        }
    });
}

fn max_dt(s: &CompressibleSolver, motion: Motion) -> f64 {
    match motion {
        Motion::Static => s.max_dt(),
        _ => s.max_dt_ale(),
    }
}

// ---------------------------------------------------------------------------------------------------------------- X

/// One row of the table of set-up X (ADR 0344 DR6).
struct Packet {
    theta_deg: f64,
    xc: f64,
    yc: f64,
    xi: f64,
    nx: usize,
    ny: usize,
    nx_ref: usize,
    xp: f64,
    probe_y: [f64; 2],
    t_end: f64,
}

const X30: Packet = Packet {
    theta_deg: 30.0,
    xc: 0.59,
    yc: 0.825,
    xi: 1.15,
    nx: 270,
    ny: 417,
    nx_ref: 390,
    xp: 1.0475,
    probe_y: [0.838, 1.338],
    t_end: 1.55,
};

const X45: Packet = Packet {
    theta_deg: 45.0,
    xc: 0.73,
    yc: 0.73,
    xi: 1.43,
    nx: 326,
    ny: 494,
    nx_ref: 446,
    xp: 1.3275,
    probe_y: [1.125, 1.625],
    t_end: 2.05,
};

/// The sponge and frame of a run of set-up X.
#[derive(Clone, Copy, PartialEq, Debug)]
enum XRun {
    Reference,
    Lab,
    Attached,
    Control,
    Static,
}

fn packet_run(pk: &Packet, kind: XRun) -> Run {
    let th = pk.theta_deg.to_radians();
    let (c, s) = (th.cos(), th.sin());
    let nx = if kind == XRun::Reference { pk.nx_ref } else { pk.nx };
    let mesh0 = box_2d(nx, pk.ny, nx as f64 * DX, pk.ny as f64 * DX, false, false).unwrap();
    let (xc, yc) = (pk.xc, pk.yc);
    let init = move |x: Vec3| {
        let (dx, dy) = (x[0] - xc, x[1] - yc);
        let xi = dx * c + dy * s;
        let eta = -dx * s + dy * c;
        simple_wave(EPS * g1(xi) * (-(eta / 0.4).powi(4)).exp(), [c, s, 0.0])
    };
    let mut sv = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), init).unwrap();
    for p in ["xmin", "ymin", "ymax"] {
        sv.set_bc(p, CompBc::Fixed(ambient())).unwrap();
    }
    sv.set_bc("xmax", THOMPSON).unwrap();
    match kind {
        XRun::Lab | XRun::Attached | XRun::Static => {
            let sp = Sponge::planar(&sv.geom, ambient(), &registered_layer(pk.xi), true).unwrap();
            sv.set_sponge(Some(sp)).unwrap();
        }
        XRun::Reference | XRun::Control => {}
    }
    match kind {
        XRun::Lab => sv.set_sponge_frame(SpongeFrame::AttachedWithLabMemory),
        XRun::Attached => sv.set_sponge_frame(SpongeFrame::AttachedToMesh),
        _ => {}
    }
    Run::new(mesh0, sv)
}

/// The probe cells of a run of set-up X: the column at `xp` with the centre at `t = 0` in `probe_y`, sorted by `y`.
fn packet_probe(r: &Run, pk: &Packet) -> Vec<usize> {
    let cc = r.mesh0.cell_centres();
    let mut v: Vec<usize> = (0..cc.len())
        .filter(|&c| (cc[c][0] - pk.xp).abs() < 0.2 * DX && cc[c][1] >= pk.probe_y[0] && cc[c][1] <= pk.probe_y[1])
        .collect();
    v.sort_by(|&a, &b| cc[a][1].total_cmp(&cc[b][1]));
    assert!(!v.is_empty());
    v
}

/// The measure of ADR 0237 for the runs `tests` against `reference`, all in lockstep (`dt` from the runs flagged in
/// `sets_dt` and the reference) until `t_end`. Returns `R` per test run.
fn lockstep_measure(
    reference: &mut Run,
    tests: &mut [Run],
    sets_dt: &[bool],
    probes: &[Vec<usize>],
    probe_ref: &[usize],
    t_end: f64,
    motion: Motion,
) -> Vec<f64> {
    let mut num = vec![0.0f64; tests.len()];
    let mut den = 0.0f64;
    while reference.s.time < t_end {
        let dt = tests
            .iter()
            .zip(sets_dt)
            .filter(|(r, f)| **f && r.alive)
            .map(|(r, _)| max_dt(&r.s, motion))
            .fold(max_dt(&reference.s, motion), f64::min);
        {
            let mut all: Vec<&mut Run> = tests.iter_mut().collect();
            all.push(reference);
            advance(&mut all, dt, motion);
        }
        assert!(reference.alive, "the reference run stopped");
        let p_ref: Vec<f64> = probe_ref.iter().map(|&c| pressure(&reference.s, c)).collect();
        den = p_ref.iter().fold(den, |m, p| m.max(p - P0));
        for (i, r) in tests.iter().enumerate() {
            if r.alive {
                for (k, &c) in probes[i].iter().enumerate() {
                    num[i] = num[i].max((pressure(&r.s, c) - p_ref[k]).abs());
                }
            }
        }
    }
    println!(
        "  {} steps to t = {:.4}; incident peak at the probe / eps = {:.5}",
        reference.s.steps,
        reference.s.time,
        den / EPS
    );
    num.iter().map(|n| n / den).collect()
}

/// Set-up X of ADR 0344 at one angle: the moving group (reference, L, A, NC) to `t_end`, L and A on to `t = 4.0`, then
/// the static group (reference, S). Returns (LR, LB, AR, AB, A's stop, NC, SR).
fn packet_case(pk: &Packet) -> (f64, f64, f64, f64, Option<f64>, f64, f64) {
    let motion = Motion::Translate(V_ALONG);
    let mut reference = packet_run(pk, XRun::Reference);
    let mut tests = vec![
        packet_run(pk, XRun::Lab),
        packet_run(pk, XRun::Attached),
        packet_run(pk, XRun::Control),
    ];
    let probes: Vec<Vec<usize>> = tests.iter().map(|r| packet_probe(r, pk)).collect();
    let probe_ref = packet_probe(&reference, pk);
    assert_eq!(probe_ref.len(), probes[0].len());
    println!(
        "X{} moving group: test {} cells, reference {} cells, {} probe cells",
        pk.theta_deg,
        tests[0].s.u.len(),
        reference.s.u.len(),
        probe_ref.len()
    );
    let r = lockstep_measure(
        &mut reference,
        &mut tests,
        &[true, false, true],
        &probes,
        &probe_ref,
        pk.t_end,
        motion,
    );
    drop(reference);
    let _control = tests.pop();
    // L and A on to 20 transit times of the layer
    let t_long = 20.0 * LAYER;
    while tests[0].s.time < t_long {
        let dt = max_dt(&tests[0].s, motion);
        let mut all: Vec<&mut Run> = tests.iter_mut().collect();
        advance(&mut all, dt, motion);
        assert!(tests[0].alive, "run L stopped at {:?}", tests[0].stopped_at);
    }
    let (lb, ab, a_stop) = (tests[0].bmax, tests[1].bmax, tests[1].stopped_at);
    println!(
        "  L to t = {:.4}: B max {lb:.4}, final {:.3e}; A: B max {ab:.4e}, stopped at {a_stop:?}, final {:.3e}",
        tests[0].s.time,
        b_all(&tests[0].s),
        b_all(&tests[1].s)
    );
    drop(tests);
    // the static group
    let mut reference = packet_run(pk, XRun::Reference);
    let mut st = vec![packet_run(pk, XRun::Static)];
    let probes_s = vec![packet_probe(&st[0], pk)];
    let probe_ref = packet_probe(&reference, pk);
    let rs = lockstep_measure(
        &mut reference,
        &mut st,
        &[true],
        &probes_s,
        &probe_ref,
        pk.t_end,
        Motion::Static,
    );
    (r[0], lb, r[1], ab, a_stop, r[2], rs[0])
}

fn report_packet(name: &str, pk: &Packet, nc_range: [f64; 2]) {
    let (lr, lb, ar, ab, a_stop, nc, sr) = packet_case(pk);
    println!(
        "{name}: LR = {lr:.3e} (limit {LIMIT:e}); LB = {lb:.4} (<= {BOUND}); SR = {sr:.3e} (limit {LIMIT:e}); NC = {nc:.4} ({nc_range:?}); AR = {ar:.3e}, AB = {ab:.4e}, A stopped at {a_stop:?} (records; predicted B > 1.5 or a failed step before t = 4)"
    );
    assert!(lr <= LIMIT, "LR {name} {lr}");
    assert!(lb.is_finite() && lb <= BOUND, "LB {name} {lb}");
    assert!(sr <= LIMIT, "SR {name} {sr}");
    assert!((nc_range[0]..=nc_range[1]).contains(&nc), "NC {name} {nc}");
}

/// X30 of ADR 0344: LR30, LB30, SR30, NC30, AR30, AB30.
#[test]
#[ignore = "heavy: limited plane pulse at 30 degrees, runs of 113 000 and 163 000 cells on moving grids to t = 4; run in the release profile"]
fn x30_limited_plane_pulse_on_a_mesh_translating_along_the_layer() {
    report_packet("X30", &X30, [0.03, 0.12]);
}

/// X45 of ADR 0344: LR45, LB45, SR45, NC45, AR45, AB45.
#[test]
#[ignore = "heavy: limited plane pulse at 45 degrees, runs of 161 000 and 220 000 cells on moving grids to t = 4; run in the release profile"]
fn x45_limited_plane_pulse_on_a_mesh_translating_along_the_layer() {
    report_packet("X45", &X45, [0.08, 0.25]);
}

// ---------------------------------------------------------------------------------------------------------------- R

/// The warped O-grid of set-up R (ADR 0344 DR7): angles `phi -> phi + 0.06 sin(5 phi)`, radii kept.
fn ogrid(n_theta: usize, n_r: usize, r_in: f64, r_out: f64) -> Mesh {
    let m = cylinder_ogrid(n_theta, n_r, r_in, r_out, Grading::Uniform, DX).unwrap();
    warp(&m, |p| {
        let (r, phi) = ((p[0] * p[0] + p[1] * p[1]).sqrt(), p[1].atan2(p[0]));
        let f = phi + 0.06 * (5.0 * phi).sin();
        [r * f.cos(), r * f.sin(), p[2]]
    })
    .unwrap()
}

/// The plain radial layer of set-up R: `rate_max ((r - front)/thickness)^2` beyond `front`, at the cell centres.
fn radial_plain(g: &FvGeometry, front: f64, thickness: f64, rate_max: f64) -> Sponge {
    let rate = g
        .mesh
        .cell_centres()
        .iter()
        .map(|x| {
            let d = ((x[0] * x[0] + x[1] * x[1]).sqrt() - front).max(0.0).min(thickness);
            rate_max * (d / thickness).powi(2)
        })
        .collect();
    Sponge {
        reference: ambient(),
        rate,
        form: SpongeForm::Plain,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum RRun {
    Reference,
    Plain,
    Control,
}

const R_FRONT: f64 = 2.0;
const OMEGA: f64 = 0.1;

fn radial_run(kind: RRun) -> Run {
    let (n_r, r_out) = if kind == RRun::Reference {
        (240, 2.6)
    } else {
        (160, 2.2)
    };
    let mesh0 = ogrid(256, n_r, 1.4, r_out);
    let init = |x: Vec3| {
        let r = (x[0] * x[0] + x[1] * x[1]).sqrt();
        simple_wave(EPS * g1(r - 1.7), [x[0] / r, x[1] / r, 0.0])
    };
    let mut sv = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), init).unwrap();
    sv.set_bc("cylinder", CompBc::Slip).unwrap();
    match kind {
        RRun::Control => sv.set_bc("farfield", CompBc::PressureOutlet { p: P0 }).unwrap(),
        _ => sv.set_bc("farfield", THOMPSON).unwrap(),
    }
    if kind == RRun::Plain {
        let rate_max = rate_max_for_reflection(LAYER, 1.0, R0, EXPONENT).unwrap();
        let sp = radial_plain(&sv.geom, R_FRONT, LAYER, rate_max);
        sv.set_sponge(Some(sp)).unwrap();
    }
    sv.set_sponge_frame(SpongeFrame::AttachedToMesh);
    Run::new(mesh0, sv)
}

/// The probe ring of set-up R (radial cell 99, centre radius about 1.8975), matched between grids by the centre.
fn radial_probes(test: &Run, reference: &Run) -> (Vec<usize>, Vec<usize>) {
    let (ct, cr) = (test.mesh0.cell_centres(), reference.mesh0.cell_centres());
    let rad = |x: Vec3| (x[0] * x[0] + x[1] * x[1]).sqrt();
    let lo = 1.4 + 99.0 * DX;
    let pt: Vec<usize> = (0..ct.len())
        .filter(|&c| rad(ct[c]) > lo && rad(ct[c]) < lo + DX)
        .collect();
    let pr: Vec<usize> = pt
        .iter()
        .map(|&c| {
            (0..cr.len())
                .find(|&k| vec3::norm(vec3::sub(cr[k], ct[c])) < 1e-9)
                .expect("probe cell in the reference")
        })
        .collect();
    (pt, pr)
}

/// RR, RS and NCR of ADR 0344: a plain radial layer on a rotating O-grid, an outgoing cylindrical pulse.
#[test]
#[ignore = "heavy: runs of 41 000 and 61 000 cells on rotating grids; run in the release profile"]
fn r_plain_radial_layer_on_a_rotating_mesh() {
    let t_end = 1.15;
    let motion = Motion::Rotate(OMEGA);
    let mut reference = radial_run(RRun::Reference);
    let mut tests = vec![radial_run(RRun::Plain), radial_run(RRun::Control)];
    let (pt, pr) = radial_probes(&tests[0], &reference);
    println!(
        "R rotating group: test {} cells, reference {} cells, {} probe cells",
        tests[0].s.u.len(),
        reference.s.u.len(),
        pt.len()
    );
    let probes = vec![pt.clone(), pt.clone()];
    let r = lockstep_measure(&mut reference, &mut tests, &[true, true], &probes, &pr, t_end, motion);
    let mut reference = radial_run(RRun::Reference);
    let mut st = vec![radial_run(RRun::Plain)];
    st[0].s.set_sponge_frame(SpongeFrame::StaticMesh);
    let rs = lockstep_measure(&mut reference, &mut st, &[true], &[pt], &pr, t_end, Motion::Static);
    println!(
        "R: RR = {:.3e}, RS = {:.3e} (limit {LIMIT:e} each); NCR = {:.4} (>= 0.5)",
        r[0], rs[0], r[1]
    );
    assert!(r[0] <= LIMIT, "RR {}", r[0]);
    assert!(rs[0] <= LIMIT, "RS {}", rs[0]);
    assert!(r[1] >= 0.5, "NCR {}", r[1]);
}

// ---------------------------------------------------------------------------------------------------------------- I1

/// Set-up I of ADR 0344 DR8: the column along the layer, the uniform rate 50, the matched form, a Gaussian simple wave
/// running along `+y`.
fn column(frame: SpongeFrame, form: SpongeForm) -> Run {
    let mesh0 = box_2d(1, 400, DX, 2.0, false, false).unwrap();
    let init = |x: Vec3| simple_wave(EPS * (-((x[1] - 0.4) / W).powi(2)).exp(), [0.0, 1.0, 0.0]);
    let mut sv = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), init).unwrap();
    sv.set_bc("xmin", CompBc::Slip).unwrap();
    sv.set_bc("xmax", CompBc::Slip).unwrap();
    sv.set_bc("ymin", CompBc::Fixed(ambient())).unwrap();
    sv.set_bc("ymax", CompBc::Fixed(ambient())).unwrap();
    let n = sv.u.len();
    sv.set_sponge(Some(Sponge {
        reference: ambient(),
        rate: vec![50.0; n],
        form,
    }))
    .unwrap();
    sv.set_sponge_frame(frame);
    Run::new(mesh0, sv)
}

const MATCHED_X: SpongeForm = SpongeForm::Matched {
    normal: [1.0, 0.0, 0.0],
};

/// I1a, I1l and I1s of ADR 0344: the attached memory grows a grazing pulse on a mesh moving along the layer; the memory
/// of the lab frame keeps it bounded, as at rest.
#[test]
fn i1_grazing_instability_of_the_attached_memory() {
    let mut a = column(SpongeFrame::AttachedToMesh, MATCHED_X);
    let mut l = column(SpongeFrame::AttachedWithLabMemory, MATCHED_X);
    let mut s = column(SpongeFrame::StaticMesh, MATCHED_X);
    let motion = Motion::Translate(V_ALONG);
    let (mut b_l, mut b_s) = (0.0f64, 0.0f64);
    let mut at_15 = None;
    while at_15.is_none() {
        let dt = l.s.max_dt_ale().min(s.s.max_dt());
        advance(&mut [&mut a, &mut l], dt, motion);
        advance(&mut [&mut s], dt, Motion::Static);
        assert!(l.alive && s.alive);
        let t = l.s.time;
        if t >= 0.5 {
            b_l = b_l.max(b_all(&l.s));
            b_s = b_s.max(b_all(&s.s));
        }
        if t >= 1.5 {
            at_15 = Some((t, b_all(&a.s), b_all(&l.s), b_all(&s.s)));
        }
    }
    let (t, ba, bl, bs) = at_15.unwrap();
    println!(
        "I1 at t = {t:.4}: I1a attached B = {ba:.3} (>= 5; model 21.3, stopped at {:?}); I1l lab largest B over [0.5, 1.5] = {b_l:.4} (<= 0.5; model 0.34); I1s static {b_s:.4} (<= 0.5); |B_l - B_s| at t = {t:.4}: {:.2e} (<= 0.05)",
        a.stopped_at,
        (bl - bs).abs()
    );
    assert!(ba >= 5.0 || a.stopped_at.is_some(), "I1a {ba}");
    assert!(b_l <= 0.5, "I1l {b_l}");
    assert!(b_s <= 0.5, "I1s {b_s}");
    assert!((bl - bs).abs() <= 0.05, "I1s difference {bl} {bs}");
}

// ---------------------------------------------------------------------------------------------------------------- G3

const BETA: f64 = 1e-3;
const GAMMA_M: f64 = 2e-3;

/// Set-up G3 of ADR 0344: returns the solver after 50 steps of `dt = 0.01` translating at `(0.2, 0, 0)`.
fn g3_run(form: SpongeForm, frame: SpongeFrame) -> (CompressibleSolver, Mesh) {
    let mesh0 = box_2d(16, 8, 1.6, 0.8, false, false).unwrap();
    let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), |_| ambient()).unwrap();
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        s.set_bc(p, CompBc::Fixed(ambient())).unwrap();
    }
    let n = s.u.len();
    s.set_sponge(Some(Sponge {
        reference: ambient(),
        rate: vec![2.0; n],
        form,
    }))
    .unwrap();
    s.set_sponge_frame(frame);
    let cc = mesh0.cell_centres().to_vec();
    let q: Vec<f64> = cc.iter().map(|x| BETA * x[0]).collect();
    if matches!(form, SpongeForm::MatchedShifted(_)) {
        s.set_sponge_memories(&SpongeMemories {
            q,
            r: vec![0.0; n],
            m: cc.iter().map(|x| [GAMMA_M * x[0], 0.0, 0.0]).collect(),
        })
        .unwrap();
    } else {
        s.set_sponge_memory(&q).unwrap();
    }
    for k in 1..=50 {
        let t_new = k as f64 * 0.01;
        s.step_ale(moved(&mesh0, |p| vec3::add(p, [0.2 * t_new, 0.0, 0.0])), 0.01)
            .unwrap();
    }
    (s, mesh0)
}

/// The largest relative drift of the state from the reference at rest (ADR 0265 U1 scales).
fn drift(s: &CompressibleSolver) -> f64 {
    let uref = ambient().to_cons(AIR);
    let scale = [1.0, 1.0, 1.0, 1.0, uref[4]];
    s.u.iter().fold(0.0f64, |m, c| {
        (0..5).fold(m, |m, k| m.max((c[k] - uref[k]).abs() / scale[k]))
    })
}

/// The cells off the first and last columns of the G3 grid.
fn g3_interior(mesh0: &Mesh) -> Vec<usize> {
    let cc = mesh0.cell_centres();
    (0..cc.len()).filter(|&c| cc[c][0] > 0.1 && cc[c][0] < 1.5).collect()
}

/// G3m, G3s and G3c of ADR 0344: the memories of the lab frame on a mesh translating normal to the layer against the
/// closed form; the attached memory as the control.
#[test]
fn g3_lab_memory_transport_against_the_closed_form() {
    let shifted = SpongeForm::MatchedShifted(MatchedOptions {
        normal: [1.0, 0.0, 0.0],
        shift: 0.0,
        along: 0.0,
        second: None,
    });
    let check = |s: &CompressibleSolver, mesh0: &Mesh| -> (f64, f64, f64) {
        let cc = s.geom.mesh.cell_centres();
        let mem = s.sponge_memories();
        let t = s.time;
        let (mut eq, mut er, mut em) = (0.0f64, 0.0f64, 0.0f64);
        for c in g3_interior(mesh0) {
            eq = eq.max((mem.q[c] - BETA * cc[c][0]).abs() / (1.6 * BETA));
            if !mem.r.is_empty() {
                er = er.max((mem.r[c] - BETA * cc[c][0] * t).abs() / (1.6 * BETA * t));
                let want = [GAMMA_M * cc[c][0], 0.0, 0.0];
                em = em.max(vec3::norm(vec3::sub(mem.m[c], want)) / (1.6 * GAMMA_M));
            }
        }
        (eq, er, em)
    };
    let (sm, mesh0) = g3_run(MATCHED_X, SpongeFrame::AttachedWithLabMemory);
    let (g3m, _, _) = check(&sm, &mesh0);
    let (ss, _) = g3_run(shifted, SpongeFrame::AttachedWithLabMemory);
    let (g3q, g3r, g3mm) = check(&ss, &mesh0);
    let (sc, _) = g3_run(MATCHED_X, SpongeFrame::AttachedToMesh);
    let (g3c, _, _) = check(&sc, &mesh0);
    let (dm, ds) = (drift(&sm), drift(&ss));
    println!(
        "G3 t = {:.2}: G3m q {g3m:.2e}, drift {dm:.2e}; G3s q {g3q:.2e}, r {g3r:.2e}, m {g3mm:.2e}, drift {ds:.2e} (limit {ROUND_OFF:e} each); G3c attached q {g3c:.4} (>= 0.05)",
        sm.time
    );
    assert!(g3m <= ROUND_OFF && dm <= ROUND_OFF, "G3m {g3m} {dm}");
    assert!(
        g3q <= ROUND_OFF && g3r <= ROUND_OFF && g3mm <= ROUND_OFF && ds <= ROUND_OFF,
        "G3s {g3q} {g3r} {g3mm} {ds}"
    );
    assert!(g3c >= 0.05, "G3c {g3c}");
}

// ---------------------------------------------------------------------------------------------------------------- U

/// The motion of set-up U of ADR 0207 T2 at time `t` (copied from `compressible_ale_f258.rs`).
fn u_motion(p: Vec3, t: f64) -> Vec3 {
    let (s1, s2) = ((2.0 * PI * t).sin(), (4.0 * PI * t).sin());
    let bump = 0.01 * s1 * (PI * p[0] / 1.2).sin() * (PI * p[1] / 0.8).sin();
    let q = [p[0] + bump, p[1] + bump];
    let (cx, cy) = (0.6, 0.4);
    let d = [(1.0 + 0.03 * s1) * (q[0] - cx), (1.0 + 0.02 * s2) * (q[1] - cy)];
    let th = 0.05 * s1;
    let (c, s) = (th.cos(), th.sin());
    [
        cx + c * d[0] - s * d[1] + 0.02 * s1,
        cy + s * d[0] + c * d[1] + 0.01 * s2,
        p[2],
    ]
}

fn ended(time: f64, end: f64) -> bool {
    time >= end - 1e-12 * end.max(1.0)
}

fn step_to(s: &CompressibleSolver, end: f64) -> f64 {
    s.max_dt_ale().min((end - s.time).max(1e-300))
}

fn u1_layers() -> (SpongeLayer, SpongeLayer) {
    let layer = SpongeLayer {
        origin: [0.6, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        thickness: 0.6,
        rate_max: 20.0,
        exponent: 2.0,
    };
    let second = SpongeLayer {
        origin: [0.0, 0.4, 0.0],
        normal: [0.0, 1.0, 0.0],
        thickness: 0.4,
        ..layer
    };
    (layer, second)
}

/// U1L of ADR 0344: U1m and U1s of ADR 0265 with the memories of the lab frame.
#[test]
fn u1l_uniform_reference_state_stays_with_the_lab_memory() {
    let mesh0 = box_2d(12, 8, 1.2, 0.8, false, false).unwrap();
    let (layer, second) = u1_layers();
    let reference = Prim {
        rho: 1.2,
        u: [0.0; 3],
        p: 0.9,
    };
    for name in ["U1Lm", "U1Ls"] {
        let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), |_| reference).unwrap();
        for p in ["xmin", "xmax", "ymin", "ymax"] {
            s.set_bc(p, CompBc::Fixed(reference)).unwrap();
        }
        let sp = if name == "U1Lm" {
            Sponge::planar(&s.geom, reference, &layer, true).unwrap()
        } else {
            Sponge::corner(&s.geom, reference, &layer, &second, 0.25, 0.05).unwrap()
        };
        s.set_sponge(Some(sp)).unwrap();
        s.set_sponge_frame(SpongeFrame::AttachedWithLabMemory);
        let uref = reference.to_cons(AIR);
        let a = reference.sound_speed(AIR);
        let scale = [
            reference.rho,
            reference.rho * a,
            reference.rho * a,
            reference.rho * a,
            uref[4],
        ];
        let (mut dr, mut mq, mut mr, mut mm) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        while !ended(s.time, 2.0) {
            let dt = step_to(&s, 2.0);
            let t_new = s.time + dt;
            s.step_ale(moved(&mesh0, |p| u_motion(p, t_new)), dt).unwrap();
            for c in &s.u {
                for k in 0..5 {
                    dr = dr.max((c[k] - uref[k]).abs() / scale[k]);
                }
            }
            let mem = s.sponge_memories();
            let t = s.time;
            mq = mem.q.iter().fold(mq, |m, q| m.max(q.abs() / (reference.p * t)));
            mr = mem.r.iter().fold(mr, |m, r| m.max(r.abs() / (reference.p * t * t)));
            mm = mem
                .m
                .iter()
                .fold(mm, |m, v| m.max(vec3::norm(*v) / (reference.rho * a * t)));
        }
        println!(
            "{name}: {} steps to t = {:.4}; drift {dr:.2e}, memories q {mq:.2e}, r {mr:.2e}, m {mm:.2e} (limit {ROUND_OFF:e} each)",
            s.steps, s.time
        );
        assert!(s.steps >= 100, "{name} steps");
        assert!(
            dr <= ROUND_OFF && mq <= ROUND_OFF && mr <= ROUND_OFF && mm <= ROUND_OFF,
            "{name}"
        );
    }
}

/// U2r of ADR 0344: a uniform reference state stays with a plain radial layer on the rotating warped O-grid.
#[test]
fn u2r_uniform_reference_state_stays_on_a_rotating_mesh() {
    let mesh0 = ogrid(64, 40, 1.4, 1.6);
    let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), |_| ambient()).unwrap();
    s.set_bc("farfield", CompBc::Fixed(ambient())).unwrap();
    let sp = radial_plain(&s.geom, 1.5, 0.1, 20.0);
    assert!(sp.rate.iter().any(|r| *r > 0.0));
    s.set_sponge(Some(sp)).unwrap();
    s.set_sponge_frame(SpongeFrame::AttachedToMesh);
    let mut dr = 0.0f64;
    let mut wmax = 0.0f64;
    for _ in 0..100 {
        let dt = s.max_dt_ale();
        let t_new = s.time + dt;
        s.step_ale(moved(&mesh0, |p| place(p, Motion::Rotate(OMEGA), t_new)), dt)
            .unwrap();
        dr = dr.max(drift(&s));
        wmax = s.mesh_speeds().unwrap().iter().fold(wmax, |m, w| m.max(w.abs()));
    }
    println!(
        "U2r: 100 steps to t = {:.4} on the rotating O-grid ({} cells, largest |w_n| {wmax:.3}): drift {dr:.2e} (limit {ROUND_OFF:e})",
        s.time,
        s.u.len()
    );
    assert!(wmax > 0.1, "U2r the mesh rotates");
    assert!(dr <= ROUND_OFF, "U2r {dr}");
}

// ---------------------------------------------------------------------------------------------------------------- B3

/// B3a of ADR 0344: with a plain layer the lab frame gives the bits of the attached frame on set-up U.
#[test]
fn b3a_plain_layer_lab_frame_is_bit_identical_to_the_attached_frame() {
    let mesh0 = box_2d(12, 8, 1.2, 0.8, false, false).unwrap();
    let u0 = [0.3, 0.1, 0.0];
    let q0 = Prim {
        rho: 1.0,
        u: u0,
        p: 1.0,
    };
    let rest = Prim { u: [0.0; 3], ..q0 };
    let (layer, _) = u1_layers();
    let run = |frame: SpongeFrame| -> (Vec<[u64; 5]>, Vec<u64>) {
        let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), |_| q0).unwrap();
        let t0 = 1.0 + 0.2 * vec3::dot(u0, u0) / GAMMA;
        s.set_bc(
            "xmin",
            CompBc::TotalInlet(TotalInlet {
                p0: t0.powf(3.5),
                t0,
                r_gas: 1.0,
                dir: vec3::unit(u0),
            }),
        )
        .unwrap();
        s.set_bc("xmax", CompBc::PressureOutlet { p: 1.0 }).unwrap();
        s.set_bc("ymin", CompBc::FarField(q0)).unwrap();
        s.set_bc("ymax", CompBc::NonReflecting { p: 1.0, sigma: 0.5 }).unwrap();
        let sp = Sponge::planar(&s.geom, rest, &layer, false).unwrap();
        s.set_sponge(Some(sp)).unwrap();
        s.set_sponge_frame(frame);
        for _ in 0..30 {
            let dt = step_to(&s, 2.0);
            let t_new = s.time + dt;
            s.step_ale(moved(&mesh0, |p| u_motion(p, t_new)), dt).unwrap();
        }
        (
            s.u.iter().map(|c| c.map(f64::to_bits)).collect(),
            s.mesh_speeds().unwrap().iter().map(|w| w.to_bits()).collect(),
        )
    };
    let a = run(SpongeFrame::AttachedToMesh);
    let l = run(SpongeFrame::AttachedWithLabMemory);
    println!(
        "B3a: plain layer, 30 steps of set-up U: lab frame and attached frame bit-identical: {}",
        a == l
    );
    assert!(a == l, "B3a");
}

/// B3b of ADR 0344: on a mesh that does not move the lab frame gives the bits of the attached frame (matched, shifted).
#[test]
fn b3b_lab_frame_without_motion_is_bit_identical_to_the_attached_frame() {
    let shifted = SpongeForm::MatchedShifted(MatchedOptions {
        normal: [1.0, 0.0, 0.0],
        shift: 0.25,
        along: 0.05,
        second: None,
    });
    for form in [MATCHED_X, shifted] {
        let run = |frame: SpongeFrame| {
            let mut r = column(frame, form);
            for _ in 0..40 {
                let dt = r.s.max_dt_ale();
                let g = moved(&r.mesh0, |p| p);
                r.s.step_ale(g, dt).unwrap();
            }
            let mem = r.s.sponge_memories();
            let bits = |x: &[f64]| x.iter().map(|v| v.to_bits()).collect::<Vec<u64>>();
            (
                r.s.u.iter().map(|c| c.map(f64::to_bits)).collect::<Vec<_>>(),
                bits(&mem.q),
                bits(&mem.r),
                mem.m.iter().map(|v| v.map(f64::to_bits)).collect::<Vec<_>>(),
                r.s.u.iter().any(|c| c[2] != 0.0),
            )
        };
        let a = run(SpongeFrame::AttachedToMesh);
        let l = run(SpongeFrame::AttachedWithLabMemory);
        println!("B3b {form:?}: bit-identical {}", a == l);
        assert!(a.4, "B3b the state evolves");
        assert!(a == l, "B3b {form:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------- S5

/// S5 of ADR 0344: the frame setting.
#[test]
fn s5_lab_frame_setting() {
    let mesh0 = box_2d(8, 2, 1.0, 0.25, false, false).unwrap();
    let mut s = CompressibleSolver::new(geom(mesh0.clone()), AIR, CompSettings::default(), |_| ambient()).unwrap();
    assert_eq!(s.sponge_frame(), SpongeFrame::StaticMesh);
    assert_eq!(SpongeFrame::default(), SpongeFrame::StaticMesh);
    s.set_sponge_frame(SpongeFrame::AttachedWithLabMemory);
    let layer = SpongeLayer {
        origin: [0.5, 0.0, 0.0],
        normal: [1.0, 0.0, 0.0],
        thickness: 0.5,
        rate_max: 10.0,
        exponent: 2.0,
    };
    let shift = |k: f64| move |x: Vec3| vec3::add(x, [0.0, 1e-5 * k, 0.0]);
    let forms = [
        Sponge::planar(&s.geom, ambient(), &layer, false).unwrap(),
        Sponge::planar(&s.geom, ambient(), &layer, true).unwrap(),
        Sponge::planar_shifted(&s.geom, ambient(), &layer, 0.25, 0.05).unwrap(),
    ];
    for (k, sp) in forms.into_iter().enumerate() {
        s.set_sponge(None).unwrap();
        s.set_sponge(Some(sp)).unwrap();
        assert_eq!(s.sponge_frame(), SpongeFrame::AttachedWithLabMemory, "S5 kept");
        s.step_ale(moved(&mesh0, shift(k as f64 + 1.0)), 1e-3).unwrap();
        let (u, time) = (s.u.clone(), s.time);
        assert!(s.replace_mesh(s.geom.clone(), s.u.clone()).is_err(), "S5 replace_mesh");
        assert!(s.u == u && s.time == time && s.sponge().is_some());
    }
    assert_eq!(s.steps, 3);
    println!("S5: lab frame kept across set_sponge, step_ale accepts plain, matched, shifted; replace_mesh refused");
}
