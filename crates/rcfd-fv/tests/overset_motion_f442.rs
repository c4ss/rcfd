//! ADR 0359 (F-442): the step-size-dependent remainder made by the motion of the component through the overset coupling.
//!
//! TG_2 and TG_1 are the set-ups of ADR 0245 A2 and H10 (BDF2, three outer iterations, `rhie_chow_step_consistent`). The
//! bilinear stencil B (diagnosis only, built here from `OversetSolver::prepare` and handed to `advance`) interpolates every
//! fringe cell from the four cell centres of the other part around its centre: a continuous function of the pose, where
//! the library's stencil is chosen by the donor cell and jumps when the fringe centre crosses a face of the other part.
//! The helpers of the set-up are copies of those of `bdf2_remainder_f344.rs`.
//!
//! Slow tests are ignored: `cargo test --release -p rcfd-fv --test overset_motion_f442 -- --ignored --nocapture`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::rectilinear_2d;
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{
    compose, pose_inverse_apply, CellKind, Connectivity, DonorInterpolation, Overset, OversetSpec, Stencil,
};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;

const DEPTH: f64 = 0.1;
const NU_TG: f64 = 0.1;
const T_END: f64 = 0.5;
/// Half the side of the component square.
const HALF: f64 = 0.6;

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
    /// `lo <= value <= hi`.
    fn within(&mut self, id: &str, value: f64, lo: f64, hi: f64) {
        let ok = (lo..=hi).contains(&value);
        println!(
            "{id}: {value:.4e} (limit in [{lo:.3e}, {hi:.3e}]) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} not in [{lo:e}, {hi:e}]"));
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

/// The exact velocity at `t` on the four sides.
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

fn decay() -> f64 {
    (-2.0 * NU_TG * T_END).exp()
}

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

/// The overset of TG_`level`; `hole` false: no background cell is a hole (and so none is a fringe cell).
fn tg_overset(level: usize, hole: bool) -> Overset {
    let cs = uniform(-HALF, 2.0 * HALF, 20 * level);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&tg_background(level), &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| hole && x[0].abs().max(x[1].abs()) < 0.25),
    };
    Overset::new(composite, spec).unwrap()
}

// ---- the bilinear stencil B (ADR 0359, diagnosis) -------------------------------------------------------------------

/// A uniform rectangular part: the cell of every index pair, from the reference centres.
struct Grid {
    x0: f64,
    h: f64,
    n: usize,
    cell: Vec<usize>,
}

impl Grid {
    fn new(centres: &[Vec3], cells: std::ops::Range<usize>, x0: f64, h: f64, n: usize) -> Self {
        let mut cell = vec![usize::MAX; n * n];
        for c in cells {
            let i = ((centres[c][0] - x0) / h - 0.5).round() as usize;
            let j = ((centres[c][1] - x0) / h - 0.5).round() as usize;
            assert!(i < n && j < n && cell[j * n + i] == usize::MAX, "B: not a uniform grid");
            cell[j * n + i] = c;
        }
        assert!(cell.iter().all(|&c| c != usize::MAX), "B: not a uniform grid");
        Grid { x0, h, n, cell }
    }

    /// The four cells around `x` and their bilinear weights.
    fn bilinear(&self, x: Vec3) -> (Vec<usize>, Vec<f64>) {
        let fx = (x[0] - self.x0) / self.h - 0.5;
        let fy = (x[1] - self.x0) / self.h - 0.5;
        let (i0, j0) = (fx.floor(), fy.floor());
        assert!(
            i0 >= 0.0 && j0 >= 0.0 && (i0 as usize) + 1 < self.n && (j0 as usize) + 1 < self.n,
            "B: the point ({}, {}) has no dual cell (defective set-up)",
            x[0],
            x[1]
        );
        let (s, t) = (fx - i0, fy - j0);
        let (i, j) = (i0 as usize, j0 as usize);
        let at = |a: usize, b: usize| self.cell[b * self.n + a];
        (
            vec![at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1)],
            vec![(1.0 - s) * (1.0 - t), s * (1.0 - t), (1.0 - s) * t, s * t],
        )
    }
}

/// The two grids of TG_`level`.
struct Parts {
    background: Grid,
    component: Grid,
    nb: usize,
}

impl Parts {
    fn new(ov: &Overset, level: usize) -> Self {
        let mesh = &ov.composite.mesh;
        let centres = mesh.cell_centres();
        let nb = ov.composite.n_background_cells;
        Parts {
            background: Grid::new(centres, 0..nb, 0.0, PI / (40 * level) as f64, 40 * level),
            component: Grid::new(
                centres,
                nb..mesh.n_cells(),
                -HALF,
                2.0 * HALF / (20 * level) as f64,
                20 * level,
            ),
            nb,
        }
    }

    /// Replace the stencil of every fringe cell of `conn` by B at `pose`; `centres` are those of the moved mesh.
    fn apply(&self, conn: &mut Connectivity, centres: &[Vec3], pose: &Pose) {
        let kind = conn.kind.clone();
        for (c, st) in conn.stencils.iter_mut() {
            let (cells, weights) = if *c < self.nb {
                self.component.bilinear(pose_inverse_apply(pose, centres[*c]))
            } else {
                self.background.bilinear(centres[*c])
            };
            assert!(
                cells.iter().all(|&k| kind[k] == CellKind::Calculated),
                "B: a stencil cell of the fringe cell {c} is not calculated (defective set-up)"
            );
            *st = Stencil { cells, weights };
        }
    }
}

/// The number of fringe cells, present at both kinds, whose donor (first stencil cell) changed.
fn donor_switches(a: &Connectivity, b: &Connectivity) -> usize {
    let mut first = vec![usize::MAX; a.kind.len()];
    for (c, s) in &a.stencils {
        first[*c] = s.cells[0];
    }
    b.stencils
        .iter()
        .filter(|(c, s)| first[*c] != usize::MAX && first[*c] != s.cells[0])
        .count()
}

// ---- the runs ------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct Cfg {
    level: usize,
    steps: usize,
    hole: bool,
    bilinear: bool,
    n_outer: usize,
}

struct Run {
    s: OversetSolver,
    e: f64,
    continuity: f64,
    switches: usize,
    changed: usize,
}

/// TG_`level` with BDF2, `n_outer` outer iterations and C, the component moving by `tg_pose(t)`.
fn run(cfg: Cfg) -> Run {
    let t0 = std::time::Instant::now();
    let dt = T_END / cfg.steps as f64;
    let ov = tg_overset(cfg.level, cfg.hole);
    let parts = Parts::new(&ov, cfg.level);
    let pose = tg_pose(0.0);
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let coupling = Some(OversetCoupling { overset: ov, pose });
    let mut set = settings(NU_TG);
    set.rhie_chow_step_consistent = true;
    let mut s = OversetSolver::new(geom, u, p, set, MassMode::Conservative, coupling).unwrap();
    s.set_time_scheme(TimeScheme::Bdf2).unwrap();
    s.set_outer_iterations(cfg.n_outer, 1.0, 1.0).unwrap();
    let (mut continuity, mut switches, mut changed) = (0.0f64, 0, 0);
    let mut last: Option<Connectivity> = None;
    for k in 1..=cfg.steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let pz = tg_pose(t);
        let mut st = s.prepare(dt, pz, &[]).unwrap();
        if let Some(l) = &last {
            switches += donor_switches(l, &st.conn);
        }
        last = Some(st.conn.clone());
        if cfg.bilinear {
            let centres = st.geom.mesh.cell_centres().to_vec();
            parts.apply(&mut st.conn, &centres, &pz);
        }
        let info = s.advance(dt, st).unwrap();
        continuity = continuity.max(info.continuity);
        changed += info.n_changed;
    }
    let e = (0..s.geom.n_cells())
        .filter(|&c| s.conn.kind[c] != CellKind::Hole)
        .map(|c| vec3::norm(vec3::sub(s.u.values[c], tg_u(s.geom.mesh.cell_centres()[c], T_END))) / decay())
        .fold(0.0, f64::max);
    println!(
        "{cfg:?} ({:.1} s): e {e:.4e}, continuity {continuity:.2e}, donor switches {switches}, kind changes {changed}",
        t0.elapsed().as_secs_f64()
    );
    Run {
        s,
        e,
        continuity,
        switches,
        changed,
    }
}

/// The largest difference of the velocities of two runs over the non-hole cells, over the decay, with its cell.
fn difference(tag: &str, a: &OversetSolver, b: &OversetSolver) -> f64 {
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

/// The series of a level in `n`, `2 n` and `4 n` steps (`n` = 200 at level 2, 100 at level 1): `max |d_n|`, `max |d_2n|`,
/// the error `e` of the finest run and the largest continuity of the three.
fn series(tag: &str, level: usize, hole: bool, bilinear: bool) -> (f64, f64, f64, f64) {
    let n = if level == 2 { 200 } else { 100 };
    let cfg = |steps| Cfg {
        level,
        steps,
        hole,
        bilinear,
        n_outer: 3,
    };
    let (a, b, r) = (run(cfg(n)), run(cfg(2 * n)), run(cfg(4 * n)));
    let d1 = difference(&format!("{tag}, d_{n}"), &a.s, &r.s);
    let d2 = difference(&format!("{tag}, d_{}", 2 * n), &b.s, &r.s);
    println!(
        "{tag}: max |d_{n}| {d1:.4e}, max |d_{}| {d2:.4e}, ratio {:.4}; switches {} {} {}, kind changes {} {} {}",
        2 * n,
        d1 / d2,
        a.switches,
        b.switches,
        r.switches,
        a.changed,
        b.changed,
        r.changed
    );
    (d1, d2, r.e, a.continuity.max(b.continuity).max(r.continuity))
}

// ---- S2: B reproduces a linear field -------------------------------------------------------------------------------

#[test]
fn s2_bilinear_stencil_reproduces_a_linear_field() {
    let lin = |x: Vec3| 1.0 + 2.0 * x[0] - 3.0 * x[1];
    let mut c = Checks::new();
    for t in [0.0, 0.3] {
        let ov = tg_overset(2, true);
        let parts = Parts::new(&ov, 2);
        let pose = tg_pose(t);
        let moved = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
        let mut conn = ov.connectivity(&moved, &pose).unwrap();
        let centres = moved.cell_centres().to_vec();
        parts.apply(&mut conn, &centres, &pose);
        let err = conn
            .stencils
            .iter()
            .map(|(f, s)| {
                let v: f64 = s.cells.iter().zip(&s.weights).map(|(&k, &w)| w * lin(centres[k])).sum();
                (v - lin(centres[*f])).abs()
            })
            .fold(0.0, f64::max);
        println!("S2 at tg_pose({t}): {} fringe cells", conn.stencils.len());
        c.at_most(&format!("S2 B against a linear field at tg_pose({t})"), err, 1e-12);
    }
    c.finish();
}

// ---- F0: the fingerprint of the defaults ---------------------------------------------------------------------------

/// FNV-1a over the bits of a sequence of numbers.
fn fnv(values: impl Iterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in values {
        for b in v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn state_fingerprint(s: &OversetSolver) -> u64 {
    let u = s.u.values.iter().flat_map(|v| v.iter().map(|x| x.to_bits()));
    let p = s.p.values.iter().map(|x| x.to_bits());
    let phi = s.phi.iter().map(|x| x.to_bits());
    fnv(u.chain(p).chain(phi))
}

fn conn_fingerprint(c: &Connectivity) -> u64 {
    let kinds = c.kind.iter().map(|k| *k as u64);
    let st = c.stencils.iter().flat_map(|(f, s)| {
        std::iter::once(*f as u64)
            .chain(s.cells.iter().map(|&k| k as u64))
            .chain(s.weights.iter().map(|w| w.to_bits()))
    });
    fnv(kinds.chain(st))
}

/// TG_1, 20 steps of 0.005, the component moving: default settings (implicit Euler, one outer iteration), or BDF2, three
/// outer iterations and C.
fn f0_run(bdf2_c: bool) -> OversetSolver {
    let dt = 0.005;
    let ov = tg_overset(1, true);
    let pose = tg_pose(0.0);
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let coupling = Some(OversetCoupling { overset: ov, pose });
    let mut set = settings(NU_TG);
    set.rhie_chow_step_consistent = bdf2_c;
    let mut s = OversetSolver::new(geom, u, p, set, MassMode::Conservative, coupling).unwrap();
    if bdf2_c {
        s.set_time_scheme(TimeScheme::Bdf2).unwrap();
        s.set_outer_iterations(3, 1.0, 1.0).unwrap();
    }
    for k in 1..=20 {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        s.step(dt, tg_pose(t), &[]).unwrap();
    }
    s
}

/// The four fingerprints of F0: TG_1 default, TG_1 with BDF2 and C, the connectivity of TG_2 at `tg_pose(0.3)` with the
/// linear and with the quadratic stencil.
fn f0_fingerprints() -> [u64; 4] {
    let a = state_fingerprint(&f0_run(false));
    let b = state_fingerprint(&f0_run(true));
    let mut ov = tg_overset(2, true);
    let pose = tg_pose(0.3);
    let moved = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
    let c = conn_fingerprint(&ov.connectivity(&moved, &pose).unwrap());
    ov.set_donor_interpolation(DonorInterpolation::Quadratic);
    let d = conn_fingerprint(&ov.connectivity(&moved, &pose).unwrap());
    let f = [a, b, c, d];
    println!(
        "F0 fingerprints: {:#018x} {:#018x} {:#018x} {:#018x}",
        f[0], f[1], f[2], f[3]
    );
    f
}

/// ADR 0359 F0, the record: printed on the tree before any library change and on the final tree.
#[test]
fn f0_defaults_fingerprint_printed() {
    f0_fingerprints();
}

/// ADR 0359 F0: the fingerprints printed by `f0_defaults_fingerprint_printed` on the tree before any library change of
/// F-442 (commit 9bb0bff3, log `f442-scratch/fast-base.log`); the final tree must give the same.
#[test]
fn f0_defaults_fingerprint_equals_the_base_tree() {
    let base: [u64; 4] = [
        0x3dee_ab3c_fda1_d8d0,
        0x4758_f6c2_1597_0201,
        0xc931_bf94_74a9_7f49,
        0xac84_b084_2670_29d0,
    ];
    assert_eq!(f0_fingerprints(), base, "F0: a default result changed");
}

// ---- S0 and the diagnosis ------------------------------------------------------------------------------------------

/// ADR 0359 S0 and S1: TG_2 with C and the library's stencils is the run of ADR 0245 A2.
#[test]
#[ignore = "slow (three level-2 overset runs, about 6 minutes): run once for the addendum"]
fn s0_tg2_remainder_is_that_of_adr_0245() {
    let (d200, d400, e, cont) = series("S0 TG_2 with C", 2, true, false);
    let mut c = Checks::new();
    c.within("S0 max |d_200|", d200, 2.0e-5, 2.1e-5);
    c.within("S0 ratio", d200 / d400, 2.40, 2.50);
    c.at_most("S1 continuity", cont, 1e-9);
    println!("S0 e(TG_2) in 800 steps {e:.4e}");
    c.finish();
}

/// ADR 0359 H1 (printed; decided in the ADR) and S1: TG_2 with C and B.
#[test]
#[ignore = "diagnosis (three level-2 overset runs, about 6 minutes): run once"]
fn h1_tg2_with_the_bilinear_stencil() {
    let (d200, d400, e, cont) = series("H1 TG_2 with C and B", 2, true, true);
    println!(
        "H1: b = max |d_200| {d200:.4e}, ratio {:.4}; e(TG_2) in 800 steps {e:.4e}",
        d200 / d400
    );
    let mut c = Checks::new();
    c.at_most("S1 continuity", cont, 1e-9);
    c.finish();
}

/// ADR 0359 H2 (printed; decided in the ADR) and S1: TG_2 without a hole, with and without B.
#[test]
#[ignore = "diagnosis (six level-2 overset runs, about 12 minutes): run once"]
fn h2_tg2_without_a_hole() {
    let (d200, d400, _, c1) = series("H2 TG_2 without a hole, C and B", 2, false, true);
    println!(
        "H2 without a hole with B: max |d_200| {d200:.4e}, ratio {:.4}",
        d200 / d400
    );
    let (e200, e400, _, c2) = series("H2 TG_2 without a hole, C, library stencils", 2, false, false);
    println!(
        "H2 without a hole, library stencils: max |d_200| {e200:.4e}, ratio {:.4}",
        e200 / e400
    );
    let mut c = Checks::new();
    c.at_most("S1 continuity", c1.max(c2), 1e-9);
    c.finish();
}

/// ADR 0359 H1a, H3 (printed; decided in the ADR) and S1: level 1.
#[test]
#[ignore = "diagnosis (level-1 overset runs, about 4 minutes): run once"]
fn h1a_h3_level_1() {
    let (d100, d200, _, c1) = series("H1a TG_1 with C and B", 1, true, true);
    println!("H1a: max |d_100| {d100:.4e}, ratio {:.4}", d100 / d200);
    let (f100, f200, _, c2) = series("TG_1 without a hole, C and B (printed)", 1, false, true);
    println!(
        "TG_1 without a hole with B: max |d_100| {f100:.4e}, ratio {:.4}",
        f100 / f200
    );
    let cfg = |n_outer| Cfg {
        level: 1,
        steps: 100,
        hole: true,
        bilinear: false,
        n_outer,
    };
    let (r3, r6) = (run(cfg(3)), run(cfg(6)));
    let e_out = difference("H3 TG_1 three against six outer iterations, 100 steps", &r3.s, &r6.s);
    println!("H3: e_out {e_out:.4e}");
    let mut c = Checks::new();
    c.at_most("S1 continuity", c1.max(c2).max(r3.continuity).max(r6.continuity), 1e-9);
    c.finish();
}

// ---- amendment 1: H4 and H5 ----------------------------------------------------------------------------------------

/// A run of TG_2 (hole, C, three outer iterations) with B on the background fringe cells (`b_bg`) and on the component
/// fringe cells (`b_comp`), and every cell whose kind changed in some step.
struct Run2 {
    s: OversetSolver,
    continuity: f64,
    changed_cells: Vec<bool>,
}

fn run2(steps: usize, b_bg: bool, b_comp: bool) -> Run2 {
    let t0 = std::time::Instant::now();
    let dt = T_END / steps as f64;
    let ov = tg_overset(2, true);
    let parts = Parts::new(&ov, 2);
    let pose = tg_pose(0.0);
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&pose)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let coupling = Some(OversetCoupling { overset: ov, pose });
    let mut set = settings(NU_TG);
    set.rhie_chow_step_consistent = true;
    let mut s = OversetSolver::new(geom, u, p, set, MassMode::Conservative, coupling).unwrap();
    s.set_time_scheme(TimeScheme::Bdf2).unwrap();
    s.set_outer_iterations(3, 1.0, 1.0).unwrap();
    let mut continuity = 0.0f64;
    let mut changed_cells = vec![false; s.geom.n_cells()];
    for k in 1..=steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let pz = tg_pose(t);
        let mut st = s.prepare(dt, pz, &[]).unwrap();
        for (c, ch) in changed_cells.iter_mut().enumerate() {
            *ch |= st.conn.kind[c] != s.conn.kind[c];
        }
        if b_bg || b_comp {
            let centres = st.geom.mesh.cell_centres().to_vec();
            let mut b = st.conn.clone();
            parts.apply(&mut b, &centres, &pz);
            for ((c, own), (_, bil)) in st.conn.stencils.iter_mut().zip(b.stencils) {
                if (*c < parts.nb && b_bg) || (*c >= parts.nb && b_comp) {
                    *own = bil;
                }
            }
        }
        let info = s.advance(dt, st).unwrap();
        continuity = continuity.max(info.continuity);
    }
    println!(
        "TG_2 {steps} steps, B on the background fringe {b_bg}, on the component fringe {b_comp} ({:.1} s): continuity {continuity:.2e}",
        t0.elapsed().as_secs_f64()
    );
    Run2 {
        s,
        continuity,
        changed_cells,
    }
}

/// The largest difference over the decay over the non-hole cells that `pick` selects.
fn difference_where(a: &OversetSolver, b: &OversetSolver, pick: impl Fn(usize) -> bool) -> f64 {
    assert!(a.conn.kind == b.conn.kind, "the two runs end at different kinds");
    (0..a.geom.n_cells())
        .filter(|&c| a.conn.kind[c] != CellKind::Hole && pick(c))
        .map(|c| vec3::norm(vec3::sub(a.u.values[c], b.u.values[c])) / decay())
        .fold(0.0, f64::max)
}

/// ADR 0359 amendment 1, H4 (printed; decided in the ADR) and S1: B on the component's fringe only, and (printed) on the
/// background fringe only.
#[test]
#[ignore = "diagnosis of amendment 1 (six level-2 overset runs, about 12 minutes): run once"]
fn h4_tg2_bilinear_on_one_fringe() {
    let mut cont = 0.0f64;
    for (b_bg, b_comp, tag) in [
        (false, true, "H4 B on the component fringe only"),
        (true, false, "B on the background fringe only (printed)"),
    ] {
        let (a, b, r) = (
            run2(200, b_bg, b_comp),
            run2(400, b_bg, b_comp),
            run2(800, b_bg, b_comp),
        );
        let nb = a.s.conn.n_background_cells;
        let k200 = difference_where(&a.s, &r.s, |c| c >= nb);
        let k400 = difference_where(&b.s, &r.s, |c| c >= nb);
        let g200 = difference_where(&a.s, &r.s, |c| c < nb);
        let g400 = difference_where(&b.s, &r.s, |c| c < nb);
        println!(
            "{tag}: component cells max |d_200| {k200:.4e}, ratio {:.4}; background cells max |d_200| {g200:.4e}, ratio {:.4}",
            k200 / k400,
            g200 / g400
        );
        cont = cont.max(a.continuity).max(b.continuity).max(r.continuity);
    }
    let mut c = Checks::new();
    c.at_most("S1 continuity", cont, 1e-9);
    c.finish();
}

/// ADR 0359 amendment 1, H5 (printed; decided in the ADR) and S1: the runs of H1 with the cells split by their distance to
/// the cells that changed kind.
#[test]
#[ignore = "diagnosis of amendment 1 (three level-2 overset runs, about 6 minutes): run once"]
fn h5_tg2_bilinear_near_the_kind_changes() {
    let (a, b, r) = (run2(200, true, true), run2(400, true, true), run2(800, true, true));
    let changed: Vec<bool> = (0..a.changed_cells.len())
        .map(|c| a.changed_cells[c] || b.changed_cells[c] || r.changed_cells[c])
        .collect();
    let mesh = &r.s.geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut near = changed.clone();
    for _ in 0..2 {
        let mut next = near.clone();
        for f in 0..mesh.n_internal_faces() {
            if near[own[f]] || near[nei[f]] {
                next[own[f]] = true;
                next[nei[f]] = true;
            }
        }
        near = next;
    }
    let calc = |c: usize| r.s.conn.kind[c] == CellKind::Calculated;
    let n200 = difference_where(&a.s, &r.s, |c| calc(c) && near[c]);
    let f200 = difference_where(&a.s, &r.s, |c| calc(c) && !near[c]);
    let n400 = difference_where(&b.s, &r.s, |c| calc(c) && near[c]);
    let f400 = difference_where(&b.s, &r.s, |c| calc(c) && !near[c]);
    println!(
        "H5: {} cells changed kind; near max |d_200| {n200:.4e} (ratio {:.4}), far {f200:.4e} (ratio {:.4}); n / f = {:.3}",
        changed.iter().filter(|&&x| x).count(),
        n200 / n400,
        f200 / f400,
        n200 / f200
    );
    let mut c = Checks::new();
    c.at_most("S1 continuity", a.continuity.max(b.continuity).max(r.continuity), 1e-9);
    c.finish();
}
