//! F-659 (ADR 0577): the vertex-patch smoother of ADR 0572 on a mesh sequence of thin plates. The checks of the registration:
//! FL and BD (the floor probe and the admissible thicknesses), LV (forced hierarchy), IT and ER (counts and errors), GX and NC
//! (the growth exponent in h/t, patch and node smoother), HX (the growth in n at t/a 0.1). Every limit is the registered one;
//! every estimate is that of `f659-scratch/scan-node.log` and `scan-quad64.log`. The harness is copied from `sa_thin_plate.rs`
//! (F-641), which is not edited.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed, ReducedSystem};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::solve::{
    smoothed_aggregation_structural, solve_static_report, solve_static_with, CgPreconditioner, StaticOptions,
    StaticSolver,
};
use rcfd_fem::{BlockProperty, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::amg_elastic::{ElasticAmg, ElasticAmgSettings, Smoother};
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
/// ADR 0577 D2: the target of the floor probe and the admissibility bound `tau / 4`.
const PROBE_TARGET: f64 = 1e-14;
const FLOOR_LIMIT: f64 = 2.5e-10;

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

// --------------------------------------------------------------------------------- the plate of ADR 0552 D3 (copied)

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
    model: SolidModel,
    prescribed: Vec<Prescribed>,
}

/// The simply supported unit plate of ADR 0552 D3 on an `n x n` mesh (Shell4 quadrilaterals or Shell3 triangles), thickness
/// `t`.
fn ss_plate(tri: bool, n: usize, t: f64) -> Case {
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
    Case { model, prescribed }
}

// ---------------------------------------------------------------------------------------------------------- harness

fn system(case: &Case) -> ReducedSystem {
    let k = stiffness(&case.model).unwrap();
    let f = nodal_load_vector(&case.model, &[]).unwrap();
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

/// The restarted CG of `solve::preconditioned_cg` (ADR 0282 amendment 2), copied from `sa_thin_plate.rs` with the target as
/// a parameter: the total iterations, the restarts and the true relative residual reached, also when it stays above it.
fn control_cg(a: &CsrMatrix, b: &[f64], m: &dyn Preconditioner, tau: f64) -> (usize, usize, f64) {
    let target = tau * norm2(b);
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

/// The floor probe of ADR 0577 D2: the true relative residual the restarted CG with the `VP(S1)` hierarchy reaches at the
/// target `1e-14`.
fn floor_probe(case: &Case, sys: &ReducedSystem) -> f64 {
    let amg = hierarchy(case, sys, &vp(s1()));
    control_cg(&sys.matrix, &sys.rhs, &amg, PROBE_TARGET).2
}

fn relative_errors(u: &[f64], ud: &[f64]) -> (f64, f64) {
    let d: Vec<f64> = u.iter().zip(ud).map(|(a, b)| a - b).collect();
    (norm2(&d) / norm2(ud), max_abs(&d) / max_abs(ud))
}

// ------------------------------------------------------------------------------------------- the registered sequence

/// One mesh of the sequence: the element, `n`, the admissible thicknesses (ADR 0577 arithmetic 2, thickest first) and the IT
/// limits `ceil(1.5 x estimate)`.
struct Mesh {
    tri: bool,
    n: usize,
    ts: &'static [f64],
    limits: &'static [usize],
}

const T_ALL: &[f64] = &[0.1, 0.03, 0.01, 0.003, 0.001];

const SHELL4: [Mesh; 4] = [
    Mesh {
        tri: false,
        n: 8,
        ts: T_ALL,
        limits: &[9, 14, 17, 21, 21],
    },
    Mesh {
        tri: false,
        n: 16,
        ts: &[0.1, 0.03, 0.01, 0.003],
        limits: &[11, 18, 29, 39],
    },
    Mesh {
        tri: false,
        n: 32,
        ts: &[0.1, 0.03, 0.01],
        limits: &[12, 21, 36],
    },
    Mesh {
        tri: false,
        n: 64,
        ts: &[0.1, 0.03, 0.01],
        limits: &[14, 20, 36],
    },
];

const SHELL3: [Mesh; 4] = [
    Mesh {
        tri: true,
        n: 8,
        ts: T_ALL,
        limits: &[12, 14, 14, 14, 14],
    },
    Mesh {
        tri: true,
        n: 16,
        ts: T_ALL,
        limits: &[15, 21, 24, 24, 24],
    },
    Mesh {
        tri: true,
        n: 32,
        ts: T_ALL,
        limits: &[17, 27, 33, 38, 38],
    },
    Mesh {
        tri: true,
        n: 64,
        ts: &[0.1, 0.03, 0.01, 0.003],
        limits: &[20, 30, 48, 62],
    },
];

/// The control count of the node smoother `S1` (block Gauss-Seidel) on the plate `(tri, n, t)`.
fn node_count(element: &str, tri: bool, n: usize, t: f64) -> f64 {
    let case = ss_plate(tri, n, t);
    let sys = system(&case);
    let amg = hierarchy(&case, &sys, &s1());
    let (its, rs, rr) = control_cg(&sys.matrix, &sys.rhs, &amg, TAU);
    println!("  node smoother {element} n {n} t/a {t}: {its} iterations, {rs} restarts, residual {rr:.3e}");
    its as f64
}

/// FL, LV, IT, ER on every case of `meshes`; GX and NC per mesh; HX across the meshes.
fn run_sequence(element: &str, meshes: &[Mesh]) {
    let mut c = Checks::new();
    let mut thick = Vec::new();
    for mesh in meshes {
        let n = mesh.n;
        let mut counts = Vec::new();
        for (&t, &limit) in mesh.ts.iter().zip(mesh.limits) {
            let id = format!("{element} n {n} t/a {t} (h/t {:.4})", 1.0 / (n as f64 * t));
            let case = ss_plate(mesh.tri, n, t);
            let sys = system(&case);
            let levels = hierarchy(&case, &sys, &vp(s1())).n_levels();
            c.at_least(&format!("LV {id}: levels"), levels as f64, 2.0);
            c.at_most(&format!("FL {id}: floor probe"), floor_probe(&case, &sys), FLOOR_LIMIT);
            let direct = solve_static_with(&case.model, &case.prescribed, &[], &StaticOptions::default()).unwrap();
            match solve_static_report(&case.model, &case.prescribed, &[], &structural(vp(s1()))) {
                Ok((res, rep)) => {
                    println!(
                        "{id}: {} iterations, {} restarts, residual {:.3e}",
                        rep.iterations, rep.restarts, rep.relative_residual
                    );
                    c.at_most(&format!("IT {id}: iterations"), rep.iterations as f64, limit as f64);
                    c.holds(
                        &format!("IT {id}: no restart"),
                        rep.restarts == 0,
                        &format!("{} restarts", rep.restarts),
                    );
                    let (e2, einf) = relative_errors(res.displacement_values(), direct.displacement_values());
                    c.at_most(&format!("ER {id}: |u - u_d|_2 / |u_d|_2"), e2, 1e-8);
                    c.at_most(&format!("ER {id}: |u - u_d|_inf / |u_d|_inf"), einf, 1e-8);
                    counts.push(rep.iterations);
                }
                Err(e) => c.holds(&format!("IT {id}: solve Ok"), false, &format!("{e:?}")),
            }
        }
        if counts.len() != mesh.ts.len() {
            c.holds(&format!("GX {element} n {n}: every count"), false, "a solve failed");
            continue;
        }
        let (t0, tn) = (mesh.ts[0], mesh.ts[mesh.ts.len() - 1]);
        let ln_h = (t0 / tn).ln();
        let p_patch = (counts[counts.len() - 1] as f64 / counts[0] as f64).ln() / ln_h;
        let p_node = (node_count(element, mesh.tri, n, tn) / node_count(element, mesh.tri, n, t0)).ln() / ln_h;
        println!(
            "{element} n {n}: H = {:.2}, counts {counts:?}, p_n patch {p_patch:.4}, p_n node {p_node:.4}",
            t0 / tn
        );
        c.at_most(&format!("GX {element} n {n}: p_n patch"), p_patch, 0.6);
        c.holds(
            &format!("NC {element} n {n}: p_n patch < p_n node"),
            p_patch < p_node,
            &format!("{p_patch:.4} against {p_node:.4}"),
        );
        if !mesh.tri {
            c.at_least(&format!("NC {element} n {n}: p_n node"), p_node, 0.5);
        }
        thick.push(counts[0]);
    }
    if thick.len() == meshes.len() {
        c.at_most(
            &format!("HX {element}: count(n 64, t 0.1) / count(n 8, t 0.1) ({thick:?})"),
            thick[thick.len() - 1] as f64 / thick[0] as f64,
            2.4,
        );
    }
    c.finish();
}

#[test]
fn shell4_mesh_sequence() {
    run_sequence("Shell4", &SHELL4);
}

#[test]
fn shell3_mesh_sequence() {
    run_sequence("Shell3", &SHELL3);
}

#[test]
fn bd_next_thinner_thickness_is_floor_limited() {
    let mut c = Checks::new();
    for (tri, n, t) in [
        (false, 16, 0.001),
        (false, 32, 0.003),
        (false, 64, 0.003),
        (true, 64, 0.001),
    ] {
        let case = ss_plate(tri, n, t);
        let sys = system(&case);
        let probe = floor_probe(&case, &sys);
        let element = if tri { "Shell3" } else { "Shell4" };
        c.holds(
            &format!("BD {element} n {n} t/a {t}: floor probe > {FLOOR_LIMIT:e}"),
            probe > FLOOR_LIMIT && probe.is_finite(),
            &format!("{probe:.4e}"),
        );
    }
    c.finish();
}
