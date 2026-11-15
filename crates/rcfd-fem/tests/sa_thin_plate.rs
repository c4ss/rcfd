//! F-641 (ADR 0572): smoothed aggregation for thin plates and shells. The checks of the registration in its order: K0 (the
//! reference), KA1, KA2 (aggregation), KC1 to KC3 and KV (coarse operator and the test-side two-grid), KS1 to KS3 (smoother), M1
//! (the mechanism, plain MITC3), FX1 to FX3 (the vertex-patch smoother on the ADR 0552 models), SY1 (symmetry), RF1 (refusal),
//! DF and B2 (defaults and the digests of the base). Every limit is the registered one; every estimate is that of
//! `f641-scratch/explore1.log`, `deep2.log`, `tscan.log`, `s0.log`, `plain.log` and `digest-base.log`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::{assemble_with, stiffness};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed, ReducedSystem};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::shell3::{self, Shell3Options};
use rcfd_fem::solve::{
    smoothed_aggregation_structural, solve_static_report, solve_static_with, CgPreconditioner, StaticOptions,
    StaticSolver,
};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::amg_elastic::{
    Aggregation, ElasticAmg, ElasticAmgError, ElasticAmgSettings, Smoother, QUADRATIC_RATIO,
};
use rcfd_linalg::vecops::norm2;
use rcfd_linalg::{cg, CsrMatrix, Preconditioner, SolveControl};

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit && value.is_finite();
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit && value.is_finite();
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

/// NaN-propagating maximum of `|v|`.
fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| {
        if m.is_nan() || x.is_nan() {
            f64::NAN
        } else {
            m.max(x.abs())
        }
    })
}

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
/// ADR 0552 arithmetic 2: the true relative residual of the CG and its iteration cap.
const TAU: f64 = 1e-9;
const MAX_IT: usize = 5000;

fn s0() -> ElasticAmgSettings {
    ElasticAmgSettings::default()
}

/// ADR 0552 arithmetic 1: the forced hierarchy.
fn s1() -> ElasticAmgSettings {
    ElasticAmgSettings {
        max_coarse: 60,
        ..ElasticAmgSettings::default()
    }
}

/// ADR 0572 D2: the settings with the vertex-patch smoother.
fn vp(s: ElasticAmgSettings) -> ElasticAmgSettings {
    ElasticAmgSettings {
        smoother: Smoother::VertexPatch,
        ..s
    }
}

fn structural(settings: ElasticAmgSettings) -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::SmoothedAggregationStructural(settings),
        tolerance: TAU,
        max_iterations: MAX_IT,
    })
}

// ------------------------------------------------------------------------------- the models of ADR 0552 D3 (copied)

fn grid(a: f64, b: f64, nx: usize, ny: usize) -> (Vec<Vec3>, Vec<[usize; 4]>) {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([a * i as f64 / nx as f64, b * j as f64 / ny as f64, 0.0]);
        }
    }
    let mut el = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            el.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    (nodes, el)
}

fn shell_model(nodes: Vec<Vec3>, cells: &[[usize; 4]], tri: bool, t: f64, nu: f64) -> SolidModel {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    if tri {
        let conn: Vec<usize> = cells
            .iter()
            .flat_map(|q| [q[0], q[1], q[2], q[0], q[2], q[3]])
            .collect();
        mesh.add_block(ElementKind::Shell3, conn).unwrap();
        mesh.add_element_set("all", (0..2 * cells.len()).collect()).unwrap();
    } else {
        mesh.add_block(ElementKind::Shell4, cells.iter().flatten().copied().collect())
            .unwrap();
        mesh.add_element_set("all", (0..cells.len()).collect()).unwrap();
    }
    mesh.set_block_property(0, BlockProperty::shell(t).unwrap()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, nu, RHO, 0.0).unwrap())
        .unwrap();
    m
}

struct Case {
    name: &'static str,
    model: SolidModel,
    prescribed: Vec<Prescribed>,
    forces: Vec<NodalForceAt>,
}

/// P4 / P3: the simply supported plate (Shell4 16 x 16, Shell3 32 x 32), thickness `t`.
fn ss_plate(name: &'static str, tri: bool, t: f64) -> Case {
    let n = if tri { 32 } else { 16 };
    let (nodes, el) = grid(1.0, 1.0, n, n);
    let mut prescribed = Vec::new();
    for (k, x) in nodes.iter().enumerate() {
        for comp in [0, 1, 5] {
            prescribed.push(Prescribed::new(k, comp, 0.0));
        }
        let onx = x[0] == 0.0 || x[0] == 1.0;
        let ony = x[1] == 0.0 || x[1] == 1.0;
        if onx || ony {
            prescribed.push(Prescribed::new(k, 2, 0.0));
        }
        if onx {
            prescribed.push(Prescribed::new(k, 3, 0.0));
        }
        if ony {
            prescribed.push(Prescribed::new(k, 4, 0.0));
        }
    }
    let mut model = shell_model(nodes, &el, tri, t, 0.3);
    model.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    Case {
        name,
        model,
        prescribed,
        forces: Vec::new(),
    }
}

/// C4 / C3: the cantilever strip.
fn strip(name: &'static str, tri: bool) -> Case {
    let n = 15;
    let (nodes, el) = grid(1.0, 0.1, n, 1);
    let model = shell_model(nodes, &el, tri, 0.01, 0.0);
    let mut prescribed = Vec::new();
    for k in [0, n + 1] {
        for comp in 0..6 {
            prescribed.push(Prescribed::new(k, comp, 0.0));
        }
    }
    let forces = [n, 2 * n + 1]
        .iter()
        .map(|&k| NodalForceAt::new(k, [0.0, 0.0, 0.5]))
        .collect();
    Case {
        name,
        model,
        prescribed,
        forces,
    }
}

/// BM: 40 Beam2 from the origin to (2, 1, 0.6), node 0 clamped, the tip force (120, -250, 370) N.
fn beam() -> Case {
    let ne = 40;
    let end = [2.0, 1.0, 0.6];
    let nodes: Vec<Vec3> = (0..=ne)
        .map(|i| {
            let s = i as f64 / ne as f64;
            [end[0] * s, end[1] * s, end[2] * s]
        })
        .collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, (0..ne).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    let section = BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, [0.0, 0.0, 1.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(2.1e11, 0.3, RHO, 0.0).unwrap())
        .unwrap();
    Case {
        name: "BM",
        model,
        prescribed: (0..6).map(|c| Prescribed::new(0, c, 0.0)).collect(),
        forces: vec![NodalForceAt::new(ne, [120.0, -250.0, 370.0])],
    }
}

fn p4a() -> Case {
    ss_plate("P4a", false, 0.01)
}

fn p4b() -> Case {
    ss_plate("P4b", false, 0.001)
}

/// The models of FX2 in the ADR 0552 order with ADR 0552's measured S0 and S1 counts.
fn cases() -> Vec<(Case, usize, usize)> {
    vec![
        (p4a(), 1, 85),
        (p4b(), 1, 531),
        (strip("C4", false), 1, 50),
        (ss_plate("P3a", true, 0.01), 32, 50),
        (ss_plate("P3b", true, 0.001), 38, 59),
        (strip("C3", true), 1, 23),
        (beam(), 1, 21),
    ]
}

// ---------------------------------------------------------------------------------------------------------- harness

fn system(case: &Case) -> ReducedSystem {
    let k = stiffness(&case.model).unwrap();
    system_of(case, k)
}

fn system_of(case: &Case, k: CsrMatrix) -> ReducedSystem {
    let f = nodal_load_vector(&case.model, &case.forces).unwrap();
    let fixed = prescribed_equations(&case.model, &case.prescribed).unwrap();
    reduce(&k, &f, &fixed).unwrap()
}

/// The library hierarchy of `SmoothedAggregationStructural(settings)` on the reduced system.
fn hierarchy(case: &Case, sys: &ReducedSystem, settings: &ElasticAmgSettings) -> ElasticAmg {
    smoothed_aggregation_structural(
        &sys.matrix,
        sys.free(),
        &case.model.dof_map(),
        case.model.mesh().nodes(),
        settings,
    )
    .unwrap()
}

/// The restarted CG of `solve::preconditioned_cg` (ADR 0282 amendment 2), copied: the total iterations, the restarts and the
/// true relative residual reached, also when it stays above `TAU`.
fn control_cg(a: &CsrMatrix, b: &[f64], m: &dyn Preconditioner) -> (usize, usize, f64) {
    let target = TAU * norm2(b);
    let mut x = vec![0.0; b.len()];
    let mut used = 0;
    let mut restarts = 0;
    let mut r = vec![0.0; b.len()];
    loop {
        let control = SolveControl {
            tol_abs: target,
            tol_rel: 0.0,
            max_iter: MAX_IT - used,
        };
        let s = if restarts == 0 {
            cg(a, b, &mut x, m, &control)
        } else {
            let mut delta = vec![0.0; b.len()];
            let s = cg(a, &r, &mut delta, m, &control);
            for (xi, di) in x.iter_mut().zip(&delta) {
                *xi += di;
            }
            s
        };
        used += s.iterations;
        a.residual(b, &x, &mut r);
        let tr = norm2(&r);
        if tr <= target || !s.converged || restarts == 3 || used >= MAX_IT {
            return (used, restarts, tr / norm2(b));
        }
        restarts += 1;
    }
}

/// The control count of the library hierarchy of `settings` on `case`.
fn library_count(case: &Case, settings: &ElasticAmgSettings) -> usize {
    let sys = system(case);
    let amg = hierarchy(case, &sys, settings);
    let (its, rs, rr) = control_cg(&sys.matrix, &sys.rhs, &amg);
    println!(
        "  {} {:?}: {its} iterations, {rs} restarts, residual {rr:.3e}, {} levels",
        case.name,
        settings,
        amg.n_levels()
    );
    its
}

/// Dense inverse of an SPD matrix by Cholesky.
fn spd_inverse(a: &[f64], n: usize) -> Vec<f64> {
    let mut l = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..=i {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= l[i * n + k] * l[j * n + k];
            }
            if i == j {
                assert!(s > 0.0, "not positive definite at {i}");
                l[i * n + i] = s.sqrt();
            } else {
                l[i * n + j] = s / l[j * n + j];
            }
        }
    }
    let mut inv = vec![0.0; n * n];
    for c in 0..n {
        let mut y = vec![0.0; n];
        for i in 0..n {
            let mut s = if i == c { 1.0 } else { 0.0 };
            for k in 0..i {
                s -= l[i * n + k] * y[k];
            }
            y[i] = s / l[i * n + i];
        }
        for i in (0..n).rev() {
            let mut s = y[i];
            for k in i + 1..n {
                s -= l[k * n + i] * inv[k * n + c];
            }
            inv[i * n + c] = s / l[i * n + i];
        }
    }
    inv
}

/// The dense restriction of `a` to `rows` (increasing), row-major.
fn principal(a: &CsrMatrix, rows: &[usize]) -> Vec<f64> {
    let m = rows.len();
    let mut d = vec![0.0; m * m];
    for (i, &r) in rows.iter().enumerate() {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            if let Ok(j) = rows.binary_search(&a.col[k]) {
                d[i * m + j] = a.val[k];
            }
        }
    }
    d
}

/// The fine smoother of the test-side two-grid: blocks (node blocks: block Gauss-Seidel) or vertex patches (ADR 0572 D2),
/// each a row list and the inverse of the matrix on it; one multiplicative sweep visits them in order (forward) or in
/// reverse (backward). For node blocks this is the library's block Gauss-Seidel.
struct Subspaces(Vec<(Vec<usize>, Vec<f64>)>);

impl Subspaces {
    fn blocks(a: &CsrMatrix, bp: &[usize]) -> Subspaces {
        Subspaces(
            bp.windows(2)
                .map(|w| {
                    let rows: Vec<usize> = (w[0]..w[1]).collect();
                    let inv = spd_inverse(&principal(a, &rows), rows.len());
                    (rows, inv)
                })
                .collect(),
        )
    }
    fn patches(a: &CsrMatrix, bp: &[usize]) -> Subspaces {
        let mut of = vec![0; a.n];
        for (b, w) in bp.windows(2).enumerate() {
            of[w[0]..w[1]].fill(b);
        }
        Subspaces(
            bp.windows(2)
                .map(|w| {
                    let mut blocks: Vec<usize> = (w[0]..w[1])
                        .flat_map(|r| a.col[a.row_ptr[r]..a.row_ptr[r + 1]].iter().map(|&c| of[c]))
                        .collect();
                    blocks.sort_unstable();
                    blocks.dedup();
                    let rows: Vec<usize> = blocks.iter().flat_map(|&c| bp[c]..bp[c + 1]).collect();
                    let inv = spd_inverse(&principal(a, &rows), rows.len());
                    (rows, inv)
                })
                .collect(),
        )
    }
    fn sweep(&self, a: &CsrMatrix, b: &[f64], x: &mut [f64], forward: bool) {
        let n = self.0.len();
        for s in 0..n {
            let (rows, inv) = &self.0[if forward { s } else { n - 1 - s }];
            let res: Vec<f64> = rows
                .iter()
                .map(|&r| {
                    let mut v = b[r];
                    for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                        v -= a.val[k] * x[a.col[k]];
                    }
                    v
                })
                .collect();
            let m = rows.len();
            for i in 0..m {
                x[rows[i]] += (0..m).map(|j| inv[i * m + j] * res[j]).sum::<f64>();
            }
        }
    }
}

/// The test-side two-grid of ADR 0572: one pre-sweep, the coarse correction with `P^T A P` solved exactly, one post-sweep.
struct TwoGrid {
    a: CsrMatrix,
    smoother: Subspaces,
    p: CsrMatrix,
    r: CsrMatrix,
    coarse_inv: Vec<f64>,
}

impl TwoGrid {
    fn new(a: &CsrMatrix, smoother: Subspaces, p: CsrMatrix) -> TwoGrid {
        let r = p.transpose();
        let ac = r.matmul(&a.matmul(&p));
        let nc = ac.n;
        let rows: Vec<usize> = (0..nc).collect();
        let coarse_inv = spd_inverse(&principal(&ac, &rows), nc);
        TwoGrid {
            a: a.clone(),
            smoother,
            p,
            r,
            coarse_inv,
        }
    }
}

impl Preconditioner for TwoGrid {
    fn apply(&self, b: &[f64], x: &mut [f64]) {
        let n = b.len();
        x.iter_mut().for_each(|v| *v = 0.0);
        self.smoother.sweep(&self.a, b, x, true);
        let mut res = vec![0.0; n];
        self.a.residual(b, x, &mut res);
        let nc = self.p.ncols;
        let mut rc = vec![0.0; nc];
        self.r.matvec(&res, &mut rc);
        let xc: Vec<f64> = (0..nc)
            .map(|i| (0..nc).map(|j| self.coarse_inv[i * nc + j] * rc[j]).sum())
            .collect();
        let mut corr = vec![0.0; n];
        self.p.matvec(&xc, &mut corr);
        x.iter_mut().zip(&corr).for_each(|(xi, c)| *xi += c);
        self.smoother.sweep(&self.a, b, x, false);
    }
}

/// `P - omega D^{-1} A P`, `D` the node blocks.
fn smooth_once(a: &CsrMatrix, bp: &[usize], p: &CsrMatrix, omega: f64) -> CsrMatrix {
    let ap = a.matmul(p);
    let nc = p.ncols;
    let dense = |m: &CsrMatrix, r: usize| {
        let mut row = vec![0.0; nc];
        for k in m.row_ptr[r]..m.row_ptr[r + 1] {
            row[m.col[k]] = m.val[k];
        }
        row
    };
    let mut row_ptr = vec![0usize];
    let (mut cols, mut vals) = (Vec::new(), Vec::new());
    for w in bp.windows(2) {
        let rows: Vec<usize> = (w[0]..w[1]).collect();
        let bs = rows.len();
        let dinv = spd_inverse(&principal(a, &rows), bs);
        let apr: Vec<Vec<f64>> = rows.iter().map(|&r| dense(&ap, r)).collect();
        for (i, &r) in rows.iter().enumerate() {
            let mut row = dense(p, r);
            for (c, v) in row.iter_mut().enumerate() {
                let s: f64 = (0..bs).map(|j| dinv[i * bs + j] * apr[j][c]).sum();
                *v -= omega * s;
            }
            for (c, &v) in row.iter().enumerate() {
                if v != 0.0 {
                    cols.push(c);
                    vals.push(v);
                }
            }
            row_ptr.push(cols.len());
        }
    }
    CsrMatrix::rect(a.n, nc, row_ptr, cols, vals)
}

#[derive(Clone, Copy)]
enum TwoGridKind {
    /// The library's smoothed prolongator, block Gauss-Seidel (KV).
    Library,
    /// The tentative prolongator, block Gauss-Seidel (KC2).
    Tentative,
    /// The smoothed prolongator smoothed once more, block Gauss-Seidel (KC3).
    SmoothedTwice,
    /// The library's smoothed prolongator, vertex patches (KS1).
    Patches,
}

/// The count of the test-side two-grid of `kind` on `case`, from the library hierarchy of S1 with two levels.
fn two_grid_count(case: &Case, kind: TwoGridKind) -> usize {
    let sys = system(case);
    let settings = ElasticAmgSettings { max_levels: 2, ..s1() };
    let amg = hierarchy(case, &sys, &settings);
    assert_eq!(amg.n_levels(), 2);
    let a = amg.matrix(0);
    let bp = amg.block_ptr(0);
    let p_lib = amg.prolongator(0).unwrap().clone();
    let (p, smoother) = match kind {
        TwoGridKind::Library => (p_lib, Subspaces::blocks(a, bp)),
        TwoGridKind::Tentative => (amg.tentative(0).unwrap().clone(), Subspaces::blocks(a, bp)),
        TwoGridKind::SmoothedTwice => (
            smooth_once(a, bp, &p_lib, amg.omega(0).unwrap()),
            Subspaces::blocks(a, bp),
        ),
        TwoGridKind::Patches => (p_lib, Subspaces::patches(a, bp)),
    };
    let tg = TwoGrid::new(a, smoother, p);
    let (its, rs, rr) = control_cg(&sys.matrix, &sys.rhs, &tg);
    println!(
        "  {} two-grid: {its} iterations, {rs} restarts, residual {rr:.3e}",
        case.name
    );
    its
}

// ------------------------------------------------------------------------------------------- K0, KA, KC, KV, KS: the cause

/// FNV-1a 64 of the little-endian bits (ADR 0572 arithmetic 5).
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in v {
        for byte in x.to_bits().to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

#[test]
fn cause_controls_on_p4b() {
    let mut c = Checks::new();
    // K0: the reference by the library path.
    let b = p4b();
    let (rb, pb) = solve_static_report(&b.model, &b.prescribed, &b.forces, &structural(s1())).unwrap();
    let d = digest(rb.displacement_values());
    c.holds(
        "K0 P4b S1: 531 iterations, 2 restarts, digest d8339a8d90774479",
        pb.iterations == 531 && pb.restarts == 2 && d == 0xd833_9a8d_9077_4479,
        &format!(
            "{} iterations, {} restarts, digest {d:016x}",
            pb.iterations, pb.restarts
        ),
    );
    let a = p4a();
    let (_, pa) = solve_static_report(&a.model, &a.prescribed, &a.forces, &structural(s1())).unwrap();
    c.holds(
        "K0 P4a S1: 85 iterations",
        pa.iterations == 85,
        &format!("{} iterations", pa.iterations),
    );

    let excluded = |c: &mut Checks, id: &str, nb: usize, na: usize| {
        let g = nb as f64 / na as f64;
        println!("{id}: P4b {nb}, P4a {na}, G = {g:.3}");
        c.at_least(&format!("{id} P4b count"), nb as f64, 236.0);
        c.at_least(&format!("{id} G"), g, 3.5);
    };
    let cause = |c: &mut Checks, id: &str, nb: usize, na: usize| {
        let g = nb as f64 / na as f64;
        println!("{id}: P4b {nb}, P4a {na}, G = {g:.3}");
        c.at_most(&format!("{id} P4b count"), nb as f64, 118.0);
        c.at_most(&format!("{id} G"), g, 2.5);
    };

    // KA1, KA2: aggregation.
    let geo = ElasticAmgSettings {
        aggregation: Aggregation::Geometric { ratio: QUADRATIC_RATIO },
        ..s1()
    };
    excluded(
        &mut c,
        "KA1 geometric aggregation",
        library_count(&b, &geo),
        library_count(&a, &geo),
    );
    let th = ElasticAmgSettings { theta: 0.08, ..s1() };
    excluded(&mut c, "KA2 theta 0.08", library_count(&b, &th), library_count(&a, &th));

    // KC1 to KC3, KV: coarse operator.
    let two = ElasticAmgSettings { max_levels: 2, ..s1() };
    let (kc1b, kc1a) = (library_count(&b, &two), library_count(&a, &two));
    excluded(&mut c, "KC1 exact level-1 solve", kc1b, kc1a);
    excluded(
        &mut c,
        "KC2 tentative prolongator",
        two_grid_count(&b, TwoGridKind::Tentative),
        two_grid_count(&a, TwoGridKind::Tentative),
    );
    excluded(
        &mut c,
        "KC3 prolongator smoothed twice",
        two_grid_count(&b, TwoGridKind::SmoothedTwice),
        two_grid_count(&a, TwoGridKind::SmoothedTwice),
    );
    for (case, lib) in [(&b, kc1b), (&a, kc1a)] {
        let tg = two_grid_count(case, TwoGridKind::Library);
        c.at_most(
            &format!("KV {} test-side two-grid against KC1 ({tg} / {lib})", case.name),
            (tg as f64 - lib as f64).abs() / lib as f64,
            0.15,
        );
    }

    // KS1, KS2, KS3: smoother.
    cause(
        &mut c,
        "KS1 two-grid with vertex patches",
        two_grid_count(&b, TwoGridKind::Patches),
        two_grid_count(&a, TwoGridKind::Patches),
    );
    let (_, vb) = solve_static_report(&b.model, &b.prescribed, &b.forces, &structural(vp(s1()))).unwrap();
    let (_, va) = solve_static_report(&a.model, &a.prescribed, &a.forces, &structural(vp(s1()))).unwrap();
    cause(&mut c, "KS2 VP(S1) library path", vb.iterations, va.iterations);
    let sw4 = ElasticAmgSettings {
        pre_sweeps: 4,
        post_sweeps: 4,
        ..s1()
    };
    let (nb, na) = (library_count(&b, &sw4), library_count(&a, &sw4));
    println!("KS3: P4b {nb}, P4a {na}");
    c.at_least("KS3 four node sweeps: G", nb as f64 / na as f64, 3.5);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------------ M1

#[test]
fn m1_plain_mitc3_is_hard_for_the_node_smoother() {
    let mut c = Checks::new();
    let t = 0.001;
    for (alpha, id) in [
        (0.0, "alpha = 0 (plain MITC3)"),
        (shell3::SHEAR_STABILISATION, "default alpha"),
    ] {
        let case = ss_plate("P3b", true, t);
        let opts = Shell3Options::default().stabilisation(alpha);
        let k = assemble_with(&case.model, None, |ctx| {
            shell3::element_stiffness_with(ctx.coords, t, ctx.material, &opts)
        })
        .unwrap();
        let sys = system_of(&case, k);
        let amg = hierarchy(&case, &sys, &s1());
        let (its, rs, rr) = control_cg(&sys.matrix, &sys.rhs, &amg);
        println!("M1 P3b {id}: {its} iterations, {rs} restarts, residual {rr:.3e}");
        if alpha == 0.0 {
            c.at_least(&format!("M1 P3b {id}: count"), its as f64, 236.0);
        } else {
            c.at_most(&format!("M1 P3b {id}: count"), its as f64, 89.0);
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------- FX1 to FX3

fn relative_errors(u: &[f64], ud: &[f64]) -> (f64, f64) {
    let d: Vec<f64> = u.iter().zip(ud).map(|(a, b)| a - b).collect();
    (norm2(&d) / norm2(ud), max_abs(&d) / max_abs(ud))
}

#[test]
fn fx_vertex_patches_on_the_adr_0552_models() {
    let mut c = Checks::new();
    let mut s1_counts = Vec::new();
    for (case, cap0, cap1) in cases() {
        let Case {
            name,
            model,
            prescribed,
            forces,
        } = case;
        let direct = solve_static_with(&model, &prescribed, &forces, &StaticOptions::default()).unwrap();
        let ud = direct.displacement_values();
        for (sid, settings, cap) in [("VP(S0)", vp(s0()), cap0), ("VP(S1)", vp(s1()), cap1)] {
            match solve_static_report(&model, &prescribed, &forces, &structural(settings)) {
                Ok((res, rep)) => {
                    let (e2, einf) = relative_errors(res.displacement_values(), ud);
                    println!(
                        "{name} {sid}: {} iterations, {} restarts, residual {:.3e}",
                        rep.iterations, rep.restarts, rep.relative_residual
                    );
                    c.at_most(&format!("FX2 {name} {sid}: |u - u_d|_2 / |u_d|_2"), e2, 1e-8);
                    c.at_most(&format!("FX2 {name} {sid}: |u - u_d|_inf / |u_d|_inf"), einf, 1e-8);
                    c.at_most(
                        &format!("FX2 {name} {sid}: iterations (ADR 0552 count)"),
                        rep.iterations as f64,
                        cap as f64,
                    );
                    if sid == "VP(S1)" {
                        s1_counts.push((name, rep.iterations));
                    }
                }
                Err(e) => c.holds(&format!("FX2 {name} {sid}: solve Ok"), false, &format!("{e:?}")),
            }
        }
    }
    let count = |n: &str| s1_counts.iter().find(|(m, _)| *m == n).map(|&(_, k)| k as f64);
    match (count("P4b"), count("P3b")) {
        (Some(b4), Some(b3)) => {
            c.at_most("FX1 P4b VP(S1) against 2 x 59", b4, 118.0);
            c.at_most(
                &format!("FX1 P4b VP(S1) against 2 x P3b VP(S1) = {}", 2.0 * b3),
                b4,
                2.0 * b3,
            );
        }
        _ => c.holds("FX1 P4b and P3b solved", false, "missing"),
    }
    // FX3: thickness scan of the Shell4 plate.
    for (t, cap) in [(0.1, 11.0), (0.01, 29.0), (0.003, 39.0), (0.001, 47.0)] {
        let case = ss_plate("P4", false, t);
        match solve_static_report(&case.model, &case.prescribed, &case.forces, &structural(vp(s1()))) {
            Ok((_, rep)) => c.at_most(&format!("FX3 P4 t/a = {t}: iterations"), rep.iterations as f64, cap),
            Err(e) => c.holds(&format!("FX3 P4 t/a = {t}: solve Ok"), false, &format!("{e:?}")),
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------- SY1, RF1

#[test]
fn sy1_vertex_patch_cycle_is_symmetric() {
    let mut c = Checks::new();
    let case = p4b();
    let sys = system(&case);
    let amg = hierarchy(&case, &sys, &vp(s1()));
    let n = sys.rhs.len();
    let x: Vec<f64> = (0..n).map(|i| (0.37 * i as f64 + 0.1).sin()).collect();
    let y: Vec<f64> = (0..n).map(|i| (1.3 * i as f64).cos() + 0.2).collect();
    let (mut mx, mut my) = (vec![0.0; n], vec![0.0; n]);
    amg.apply(&x, &mut mx);
    amg.apply(&y, &mut my);
    let dot = |u: &[f64], v: &[f64]| u.iter().zip(v).map(|(a, b)| a * b).sum::<f64>();
    let ratio = (dot(&mx, &y) - dot(&x, &my)).abs() / (norm2(&mx) * norm2(&y));
    c.at_most("SY1 |(Mx, y) - (x, My)| / (|Mx| |y|)", ratio, 1e-11);
    c.finish();
}

#[test]
fn rf1_non_positive_patch_is_refused() {
    let mut c = Checks::new();
    let n = 20;
    let mut t = Vec::new();
    for i in 0..n {
        let d = if i == 9 || i == 10 { 1.0 } else { 2.0 };
        t.push((i, i, d));
        if i + 1 < n {
            let o = if i == 9 { 2.0 } else { -1.0 };
            t.push((i, i + 1, o));
            t.push((i + 1, i, o));
        }
    }
    let a = CsrMatrix::from_triplets(n, t);
    let bp: Vec<usize> = (0..=n).collect();
    let nn = vec![vec![1.0; n]];
    let base = ElasticAmgSettings {
        max_coarse: 2,
        ..ElasticAmgSettings::default()
    };
    let r = ElasticAmg::new(&a, &bp, &nn, &vp(base));
    c.holds(
        "RF1 VertexPatch: PatchNotPositive at level 0",
        matches!(r, Err(ElasticAmgError::PatchNotPositive { level: 0, .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    let g = ElasticAmg::new(&a, &bp, &nn, &base);
    c.holds(
        "RF1 BlockGaussSeidel: builds",
        g.is_ok(),
        &format!("{:?}", g.as_ref().err()),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------- DF, B2

/// The Hex8 cantilever of ADR 0552 D3.
fn solid_cantilever() -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [4.0, 1.0, 1.0], [16, 4, 4]).unwrap());
    m.add_material("all", Material::isotropic_elastic(E, 0.3, 0.0, 0.0).unwrap())
        .unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1.0e6])).unwrap();
    m
}

#[test]
fn df_b2_defaults_and_base_digests() {
    let mut c = Checks::new();
    c.holds(
        "DF default, quadratic, quadratic_smoothed: BlockGaussSeidel",
        ElasticAmgSettings::default().smoother == Smoother::BlockGaussSeidel
            && ElasticAmgSettings::quadratic().smoother == Smoother::BlockGaussSeidel
            && ElasticAmgSettings::quadratic_smoothed().smoother == Smoother::BlockGaussSeidel,
        &format!("{:?}", ElasticAmgSettings::default().smoother),
    );
    let m = solid_cantilever();
    let opts = |p: CgPreconditioner| {
        StaticOptions::with_solver(StaticSolver::PreconditionedCg {
            preconditioner: p,
            tolerance: 1e-10,
            max_iterations: 1000,
        })
    };
    for (name, p, want, its) in [
        (
            "SmoothedAggregation(default)",
            CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default()),
            0x29a8_78b3_5b6d_6b25u64,
            15,
        ),
        (
            "SmoothedAggregation(quadratic)",
            CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::quadratic()),
            0x29a8_78b3_5b6d_6b25,
            15,
        ),
        (
            "SmoothedAggregation(quadratic_smoothed)",
            CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::quadratic_smoothed()),
            0x949d_365b_da15_6212,
            12,
        ),
        (
            "SmoothedAggregationStructural(default)",
            CgPreconditioner::SmoothedAggregationStructural(ElasticAmgSettings::default()),
            0x29a8_78b3_5b6d_6b25,
            15,
        ),
    ] {
        let (r, rep) = solve_static_report(&m, &[], &[], &opts(p)).unwrap();
        let d = digest(r.displacement_values());
        c.holds(
            &format!("B2 Hex8 {name}: digest {want:016x}, {its} iterations, 0 restarts"),
            d == want && rep.iterations == its && rep.restarts == 0,
            &format!("{d:016x}, {} iterations, {} restarts", rep.iterations, rep.restarts),
        );
    }
    c.finish();
}
