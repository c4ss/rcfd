//! F-259 (ADR 0191): the case tree runs the ADR 0090 library work, the compressible ALE step and the planar layering of a
//! piston, through `dynamic_mesh`. The limits are those registered in ADR 0191; the CLI checks X1 and X2 are in
//! `crates/rcfd-cli/tests/layering_ale_f259.rs`.
//!
//! "The library run" of a set-up is a loop written here with library calls only, on the tree's schedule (the step `dt` of
//! the current mesh clipped to the end time, the motion at the end of the step). The motion is the case's expression text
//! compiled with `rcfd_core::expr` in the tree's context, so both sides place the piston on the same bits.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::expr::{Compiled, Expr};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::compressible::{CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::FvGeometry;
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::box_2d;
use rcfd_mesh::layering::{remap, LayerStack};
use rcfd_mesh::motion::{mat_vec, moved_mesh, patch_points, rotation_matrix};
use rcfd_mesh::Mesh;
use std::path::PathBuf;
use std::sync::Arc;

/// Equality of a tree run and a library run (ADR 0191): relative to the largest density and pressure, and to `c0`.
const EQ: f64 = 1e-12;
/// ADR 0090 A1, A2: `p_bar V^gamma` against its initial value.
const ISENTROPE: f64 = 2e-2;
/// ADR 0090 B2: mass drift of every step.
const MASS_DRIFT: f64 = 1e-12;
/// ADR 0090 B3: the mesh volume against `0.01 x_p(t)`.
const VOLUME: f64 = 1e-13;
/// ADR 0090 B4: removals and additions each.
const MIN_EVENTS: usize = 5;
/// ADR 0090 C1: a uniform state on a moving mesh.
const GCL: f64 = 1e-12;
const GAMMA: f64 = 1.4;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// The solver keys every set-up shares (ADR 0191: HLLC, van Leer, SSP-RK3, CFL 0.4, gamma 1.4) and the initial state.
fn solver(end: f64, velocity: &str) -> String {
    format!(
        r#"
[solver]
algorithm = "compressible"
end_time = {end:?}

[compressible]
flux = "hllc"
reconstruction = "tvd_van_leer"
rk_order = 3
cfl = 0.4
gamma = 1.4

[compressible.initial.left]
density = 1.0
pressure = 1.0
velocity = {velocity}
"#
    )
}

/// P-T of ADR 0191 (ADR 0090's piston P through the tree), with the end time and the stroke period `period`;
/// `dm` is added to `[dynamic_mesh]` and `tables` after it.
fn p_t(end: f64, period: f64, dm: &str, tables: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_3d"
nx = 40
ny = 2
nz = 2
lx = 1.0
ly = 0.1
lz = 0.1
{}
[dynamic_mesh]
enabled = true
deformation = "layering"
moving_patches = ["xmax"]
displacement_expression_x = "-0.3 [m] * (1 - cos(pi * t / {period:?} [s]))"
{dm}
{tables}
"#,
        solver(end, "[]")
    )
}

/// P-B of ADR 0191: smoothing with a static characteristic boundary.
fn p_b(dm: &str, tables: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.1
{}
[boundaries.xmin]
compressible_bc = "pressure_outlet"
pressure_value = 1.0

[dynamic_mesh]
enabled = true
deformation = "smoothing"
moving_patches = ["xmax"]
fixed_patches = ["xmin"]
displacement_expression_x = "-0.2 [m] * (1 - cos(pi * t / 2 [s]))"
{dm}
{tables}
"#,
        solver(1.0, "[]")
    )
}

const U0: Vec3 = [0.3, 0.1, 0.0];

/// P-C of ADR 0191: a rigid motion with uniform flow and fixed states on every boundary.
fn p_c(dm: &str, tables: &str) -> String {
    let mut b = String::new();
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        b.push_str(&format!(
            "[boundaries.{p}]\ncompressible_bc = \"fixed\"\ndensity_value = 1.0\nvelocity_value = [0.3, 0.1, 0.0]\npressure_value = 1.0\n\n"
        ));
    }
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 4
lx = 1.0
ly = 0.2
{}
{b}
[dynamic_mesh]
enabled = true
deformation = "rigid"
displacement_expression_x = "0.05 [m] * sin(2 * pi * t / 1 [s])"
displacement_expression_y = "0.02 [m] * sin(2 * pi * t / 0.5 [s])"
rotation_angle_expression = "0.1 * sin(2 * pi * t / 1 [s])"
rotation_origin = [0.5, 0.1, 0.0]
{dm}
{tables}
"#,
        solver(2.0, "[0.3, 0.1, 0.0]")
    )
}

fn parse(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|d| panic!("{d:?}\n{text}"))
}

/// Paths of the diagnostics of a case text: parse errors, else the static checks, else those of `Simulation::new`.
fn paths(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.iter().map(|x| x.path.clone()).collect(),
        Ok(c) => {
            let mut d: Vec<String> = c.semantic_errors().iter().map(|x| x.path.clone()).collect();
            if d.is_empty() {
                if let Err(e) = Simulation::new(&c) {
                    d = e.iter().map(|x| x.path.clone()).collect();
                }
            }
            d
        }
    }
}

/// The expression `src` compiled in the tree's context of `case`.
fn compile(case: &Case, src: &str) -> Compiled {
    let ctx = rcfd_api::exprs::spatial_context(case);
    Expr::parse(src).unwrap().compile(&ctx).unwrap()
}

fn at(c: &Compiled, t: f64) -> f64 {
    c.eval(&[0.0, 0.0, 0.0, t])
}

/// The tree's end test of a transient compressible run.
fn ended(time: f64, end: f64) -> bool {
    time >= end - 1e-12 * end.max(1.0)
}

/// The tree's step: the stable step of the current mesh, clipped to the end time.
fn step_of(s: &CompressibleSolver, end: f64) -> f64 {
    s.max_dt().min((end - s.time).max(1e-300))
}

fn gas() -> Gas {
    Gas { gamma: GAMMA }
}

fn rest() -> Prim {
    Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: 1.0,
    }
}

/// A finished run: steps, the cell count after every step, the final primitives and the final time.
struct Run {
    steps: usize,
    cells: Vec<usize>,
    prim: Vec<Prim>,
    time: f64,
}

/// A tree run to its end, with a hook after every step.
fn tree_run(text: &str, mut each: impl FnMut(&Simulation, &rcfd_api::sim::StepSummary)) -> (Simulation, Run) {
    let c = parse(text);
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    let mut s = Simulation::new(&c).unwrap_or_else(|d| panic!("{d:?}"));
    let mut cells = Vec::new();
    loop {
        let sum = s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        cells.push(s.geom.n_cells());
        each(&s, &sum);
        if sum.done {
            break;
        }
    }
    let f = |n: &str| s.field(n).unwrap();
    let (rho, ux, uy, uz, p) = (f("rho"), f("ux"), f("uy"), f("uz"), f("p"));
    let prim = (0..rho.len())
        .map(|i| Prim {
            rho: rho[i],
            u: [ux[i], uy[i], uz[i]],
            p: p[i],
        })
        .collect();
    let run = Run {
        steps: cells.len(),
        cells,
        prim,
        time: s.solver_time(),
    };
    (s, run)
}

/// Equality of ADR 0191: (rho, p, u) differences relative to the largest library density, pressure and to `c0`.
fn equality(tree: &Run, lib: &Run) -> [f64; 3] {
    assert_eq!(tree.steps, lib.steps, "steps");
    assert_eq!(tree.cells, lib.cells, "cell count after every step");
    let max = |f: &dyn Fn(&Prim) -> f64| lib.prim.iter().map(f).fold(0.0f64, f64::max);
    let (rho_max, p_max, c0) = (max(&|p| p.rho), max(&|p| p.p), GAMMA.sqrt());
    let mut e = [0.0f64; 3];
    for (a, b) in tree.prim.iter().zip(&lib.prim) {
        e[0] = e[0].max((a.rho - b.rho).abs() / rho_max);
        e[1] = e[1].max((a.p - b.p).abs() / p_max);
        for k in 0..3 {
            e[2] = e[2].max((a.u[k] - b.u[k]).abs() / c0);
        }
    }
    assert!(
        (tree.time - lib.time).abs() <= EQ * lib.time,
        "time {} {}",
        tree.time,
        lib.time
    );
    e
}

fn finish(s: &CompressibleSolver, steps: usize, cells: Vec<usize>) -> Run {
    Run {
        steps,
        cells,
        prim: s.primitives(),
        time: s.time,
    }
}

/// The library run of a layered piston: `LayerStack`, `step_ale`, `remap`, `replace_mesh`.
fn lib_layered(nx: usize, cross: [usize; 2], size: [f64; 2], end: f64, dx: &Compiled) -> Run {
    let geom = |s: &LayerStack| FvGeometry::new(Arc::new(s.mesh().unwrap())).unwrap();
    let mut stack = LayerStack::uniform(nx, 1.0, cross, size, 0.4, 1.6).unwrap();
    let mut s = CompressibleSolver::new(geom(&stack), gas(), CompSettings::default(), |_| rest()).unwrap();
    let mut cells = Vec::new();
    while !ended(s.time, end) {
        let dt = step_of(&s, end);
        let t_new = s.time + dt;
        stack.set_piston(1.0 + at(dx, t_new)).unwrap();
        s.step_ale(geom(&stack), dt).unwrap();
        if let Some(ch) = stack.adjust() {
            let g = geom(&stack);
            let u = remap(&ch, &s.u, s.geom.mesh.cell_volumes(), g.mesh.cell_volumes());
            s.replace_mesh(g, u).unwrap();
        }
        cells.push(s.geom.n_cells());
    }
    finish(&s, cells.len(), cells)
}

/// A prescribed pose as the tree applies it: `origin + R(axis, angle) (p0 - origin) + d`.
struct Pose {
    disp: [Option<Compiled>; 3],
    angle: Option<Compiled>,
    origin: Vec3,
    axis: Vec3,
}

impl Pose {
    fn map(&self, p0: Vec3, t: f64) -> Vec3 {
        let d = [0, 1, 2].map(|i| self.disp[i].as_ref().map_or(0.0, |c| at(c, t)));
        let a = self.angle.as_ref().map_or(0.0, |c| at(c, t));
        let r = rotation_matrix(self.axis, a);
        vec3::add(vec3::add(self.origin, mat_vec(&r, vec3::sub(p0, self.origin))), d)
    }
}

/// The library run of a rigid (`classes = None`) or smoothed motion of `mesh0`: `step_ale` onto `moved_mesh`.
fn lib_moving(
    mesh0: &Mesh,
    pose: &Pose,
    classes: Option<(&[PatchClass], &str)>,
    init: Prim,
    bcs: &[(&str, CompBc)],
    end: f64,
) -> (CompressibleSolver, Run) {
    let geom0 = FvGeometry::new(Arc::new(mesh0.clone())).unwrap();
    let mut s = CompressibleSolver::new(geom0.clone(), gas(), CompSettings::default(), |_| init).unwrap();
    for (p, bc) in bcs {
        s.set_bc(p, *bc).unwrap();
    }
    // the tree's control of the displacement solve: the default tolerance 1e-12
    let control = SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-12,
        max_iter: 5000,
    };
    let p0 = mesh0.points();
    let mut cells = Vec::new();
    while !ended(s.time, end) {
        let dt = step_of(&s, end);
        let t_new = s.time + dt;
        let points: Vec<Vec3> = match classes {
            None => p0.iter().map(|&p| pose.map(p, t_new)).collect(),
            Some((cls, moving)) => {
                let mut presc = vec![vec3::ZERO; p0.len()];
                for q in patch_points(mesh0, moving).unwrap() {
                    presc[q] = vec3::sub(pose.map(p0[q], t_new), p0[q]);
                }
                let sm = smooth_displacement(&geom0, cls, &presc, SmoothingMethod::Laplacian, &control).unwrap();
                p0.iter()
                    .zip(&sm.displacement)
                    .map(|(p, d)| vec3::add(*p, *d))
                    .collect()
            }
        };
        let g = FvGeometry::new(Arc::new(moved_mesh(mesh0, points).unwrap())).unwrap();
        s.step_ale(g, dt).unwrap();
        cells.push(s.geom.n_cells());
    }
    let run = finish(&s, cells.len(), cells);
    (s, run)
}

/// `p_bar V^gamma` of a tree state (`p_bar` the volume mean of `p`).
fn isentrope(s: &Simulation) -> f64 {
    let v = s.geom.mesh.cell_volumes();
    let vol: f64 = v.iter().sum();
    let pv: f64 = s.field("p").unwrap().iter().zip(v).map(|(p, v)| p * v).sum();
    pv / vol * vol.powf(GAMMA)
}

/// K1 of ADR 0191: the mesh `box_3d` builds is the layer stack's, bit for bit.
#[test]
fn k1_box_3d_is_the_layer_stack_mesh() {
    for (text, n, cross) in [
        (p_t(32.0, 16.0, "", ""), 40, [2, 2]),
        (
            p_t(4.0, 4.0, "", "").replace("nx = 40\nny = 2\nnz = 2", "nx = 20\nny = 1\nnz = 1"),
            20,
            [1, 1],
        ),
    ] {
        let m = rcfd_api::sim::build_mesh(&parse(&text)).unwrap();
        let st = LayerStack::uniform(n, 1.0, cross, [0.1, 0.1], 0.4, 1.6)
            .unwrap()
            .mesh()
            .unwrap();
        let bits = |m: &Mesh| -> Vec<u64> { m.points().iter().flat_map(|p| p.map(f64::to_bits)).collect() };
        println!(
            "K1 nx {n}: content hash {:016x} / {:016x}, {} cells, patches {:?}",
            m.content_hash(),
            st.content_hash(),
            m.n_cells(),
            m.patches().iter().map(|p| p.name.clone()).collect::<Vec<_>>()
        );
        assert_eq!(m.content_hash(), st.content_hash(), "K1 content hash nx {n}");
        assert_eq!(bits(&m), bits(&st), "K1 points nx {n}");
    }
}

/// A-eq, A-2, A-B2 to A-B5 of ADR 0191: P-T through the tree against its library run, and ADR 0090's limits on the tree run.
#[test]
fn a_layered_piston_through_the_tree_equals_the_library_run() {
    let clock = std::time::Instant::now();
    let text = p_t(32.0, 16.0, "", "");
    let case = parse(&text);
    let dx = compile(&case, "-0.3 [m] * (1 - cos(pi * t / 16.0 [s]))");
    let s0 = Simulation::new(&case).unwrap();
    let inv0 = isentrope(&s0);
    let (mut drift, mut b3) = (0.0f64, 0.0f64);
    let mut prev_cells = s0.geom.n_cells();
    let (mut removed, mut added, mut b5) = (0usize, 0usize, 0usize);
    let mut last_event = 0i64;
    let (s, tree) = tree_run(&text, |s, sum| {
        drift = drift.max(sum.continuity);
        let vol: f64 = s.geom.mesh.cell_volumes().iter().sum();
        let x_p = 1.0 + at(&dx, s.solver_time());
        b3 = b3.max((vol / (0.01 * x_p) - 1.0).abs());
        let n = s.geom.n_cells() as i64;
        let event = (n - prev_cells as i64).signum();
        match event {
            -1 => removed += 1,
            1 => added += 1,
            _ => {}
        }
        if event != 0 && last_event == -event {
            b5 += 1;
        }
        last_event = event;
        prev_cells = n as usize;
    });
    let a2 = isentrope(&s) / inv0 - 1.0;
    let t_tree = clock.elapsed().as_secs_f64();
    let lib = lib_layered(40, [2, 2], [0.1, 0.1], 32.0, &dx);
    let e = equality(&tree, &lib);
    println!(
        "A-eq P-T: {} steps, final cells {}, rho {:.2e}, p {:.2e}, u {:.2e} (limit {EQ:e}); tree {t_tree:.1} s, both {:.1} s",
        tree.steps,
        tree.cells.last().unwrap(),
        e[0],
        e[1],
        e[2],
        clock.elapsed().as_secs_f64()
    );
    println!("A-2 p_bar V^gamma / (p0 V0^gamma) - 1 at t = 32: {a2:.3e} (limit {ISENTROPE:e})");
    println!(
        "A-B2 max mass drift {drift:.2e} (limit {MASS_DRIFT:e}); A-B3 max volume error {b3:.2e} (limit {VOLUME:e})"
    );
    println!("A-B4 removals {removed}, additions {added} (each >= {MIN_EVENTS}); A-B5 opposite events in consecutive steps {b5}");
    assert!(e.iter().all(|x| *x <= EQ), "A-eq {e:?}");
    assert!(a2.abs() <= ISENTROPE, "A-2 {a2}");
    assert!(drift <= MASS_DRIFT, "A-B2 {drift}");
    assert!(b3 <= VOLUME, "A-B3 {b3}");
    assert!(removed >= MIN_EVENTS && added >= MIN_EVENTS, "A-B4 {removed} {added}");
    assert_eq!(b5, 0, "A-B5");
}

/// A-1 of ADR 0191: P-T1 (P-T to t = 16, full compression) through the tree.
#[test]
fn a1_layered_piston_isentrope_at_full_compression() {
    let text = p_t(16.0, 16.0, "", "");
    let s0 = Simulation::new(&parse(&text)).unwrap();
    let inv0 = isentrope(&s0);
    let (s, run) = tree_run(&text, |_, _| {});
    let vol: f64 = s.geom.mesh.cell_volumes().iter().sum();
    let a1 = isentrope(&s) / inv0 - 1.0;
    println!(
        "A-1 p_bar V^gamma / (p0 V0^gamma) - 1 at t = 16: {a1:.3e} (limit {ISENTROPE:e}); V/V0 {:.6}, {} steps, {} cells",
        vol / 0.01,
        run.steps,
        run.cells.last().unwrap()
    );
    assert!(a1.abs() <= ISENTROPE, "A-1 {a1}");
}

/// B-eq and B-0 of ADR 0191: P-B (smoothing, a static pressure outlet) through the tree against its library run.
#[test]
fn b_smoothing_with_a_static_outlet_equals_the_library_run() {
    let text = p_b("", "");
    let case = parse(&text);
    let (s, tree) = tree_run(&text, |_, _| {});
    let mesh0 = box_2d(20, 2, 1.0, 0.1, false, false).unwrap();
    let classes: Vec<PatchClass> = mesh0
        .patches()
        .iter()
        .map(|p| match p.name.as_str() {
            "xmax" => PatchClass::Prescribed,
            "xmin" => PatchClass::Fixed,
            _ => PatchClass::Slip,
        })
        .collect();
    let pose = Pose {
        disp: [Some(compile(&case, "-0.2 [m] * (1 - cos(pi * t / 2 [s]))")), None, None],
        angle: None,
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
    };
    let (_, lib) = lib_moving(
        &mesh0,
        &pose,
        Some((&classes, "xmax")),
        rest(),
        &[("xmin", CompBc::PressureOutlet { p: 1.0 })],
        1.0,
    );
    let e = equality(&tree, &lib);
    let xmin = |m: &Mesh| -> Vec<u64> {
        patch_points(m, "xmin")
            .unwrap()
            .iter()
            .flat_map(|&q| m.points()[q].map(f64::to_bits))
            .collect()
    };
    let b0 = xmin(&s.geom.mesh) == xmin(&mesh0);
    let x_p = s.geom.mesh.points().iter().map(|p| p[0]).fold(0.0f64, f64::max);
    println!(
        "B-eq P-B: {} steps, rho {:.2e}, p {:.2e}, u {:.2e} (limit {EQ:e}); B-0 xmin points unchanged: {b0}; piston at {x_p:.6}",
        tree.steps, e[0], e[1], e[2]
    );
    assert!(e.iter().all(|x| *x <= EQ), "B-eq {e:?}");
    assert!(b0, "B-0");
}

/// C-eq and C-gcl of ADR 0191: P-C (a rigid translation and rotation with uniform flow) through the tree.
#[test]
fn c_rigid_motion_keeps_uniform_flow_and_equals_the_library_run() {
    let text = p_c("", "");
    let case = parse(&text);
    let (_, tree) = tree_run(&text, |_, _| {});
    let mesh0 = box_2d(20, 4, 1.0, 0.2, false, false).unwrap();
    let pose = Pose {
        disp: [
            Some(compile(&case, "0.05 [m] * sin(2 * pi * t / 1 [s])")),
            Some(compile(&case, "0.02 [m] * sin(2 * pi * t / 0.5 [s])")),
            None,
        ],
        angle: Some(compile(&case, "0.1 * sin(2 * pi * t / 1 [s])")),
        origin: [0.5, 0.1, 0.0],
        axis: [0.0, 0.0, 1.0],
    };
    let state = Prim {
        rho: 1.0,
        u: U0,
        p: 1.0,
    };
    let bcs: Vec<(&str, CompBc)> = ["xmin", "xmax", "ymin", "ymax"]
        .iter()
        .map(|p| (*p, CompBc::Fixed(state)))
        .collect();
    let (_, lib) = lib_moving(&mesh0, &pose, None, state, &bcs, 2.0);
    let e = equality(&tree, &lib);
    let mut gcl = 0.0f64;
    for p in &tree.prim {
        gcl = gcl
            .max((p.rho - 1.0).abs())
            .max(vec3::norm(vec3::sub(p.u, U0)) / vec3::norm(U0))
            .max((p.p - 1.0).abs());
    }
    println!(
        "C-eq P-C: {} steps, rho {:.2e}, p {:.2e}, u {:.2e} (limit {EQ:e}); C-gcl {gcl:.2e} (limit {GCL:e})",
        tree.steps, e[0], e[1], e[2]
    );
    assert!(e.iter().all(|x| *x <= EQ), "C-eq {e:?}");
    assert!(gcl <= GCL, "C-gcl {gcl}");
}

/// L of ADR 0191: E1 to E13 give a diagnostic at the registered path; the three set-ups give none.
#[test]
#[ignore = "record of ADR 0191 L: its three E5 inputs assert the refusal of a characteristic condition on a moving patch, which ADR 0257 D1 lifts (ADR 0207 Q1 (a)); the new behaviour and the other inputs are asserted by l2_located_errors of tree_wiring_f259.rs"]
fn l_located_errors() {
    let has = |text: String, path: &str| {
        let p = paths(&text);
        assert!(p.iter().any(|x| x == path), "{path}: got {p:?}\n{text}");
        println!("L {path}: {p:?}");
    };
    for text in [p_t(32.0, 16.0, "", ""), p_b("", ""), p_c("", "")] {
        assert_eq!(paths(&text), Vec::<String>::new(), "control\n{text}");
    }
    let pt = || p_t(32.0, 16.0, "", "");
    // E1
    has(
        pt().replace("gamma = 1.4", "gamma = 1.4\nmode = \"steady\""),
        "dynamic_mesh.enabled",
    );
    // E2 to E4
    has(p_b("motion = \"six_dof\"", ""), "dynamic_mesh.motion");
    has(p_b("follow_patches = [\"xmax\"]", ""), "dynamic_mesh.follow_patches");
    has(p_c("rest_patches = [\"ymin\"]", ""), "dynamic_mesh.rest_patches");
    // E5: a characteristic condition on a patch that moves, under each deformation; xmin of a layered piston is static
    has(
        p_b("", "").replace("fixed_patches = [\"xmin\"]", "fixed_patches = []"),
        "boundaries.xmin.compressible_bc",
    );
    has(
        p_c("", "").replace(
            "[boundaries.xmax]\ncompressible_bc = \"fixed\"",
            "[boundaries.xmax]\ncompressible_bc = \"pressure_outlet\"",
        ),
        "boundaries.xmax.compressible_bc",
    );
    has(
        p_t(
            32.0,
            16.0,
            "",
            "[boundaries.ymin]\ncompressible_bc = \"nonreflecting\"\npressure_value = 1.0",
        ),
        "boundaries.ymin.compressible_bc",
    );
    assert_eq!(
        paths(&p_t(
            32.0,
            16.0,
            "",
            "[boundaries.xmin]\ncompressible_bc = \"pressure_outlet\"\npressure_value = 1.0"
        )),
        Vec::<String>::new(),
        "E5 control: xmin of a layered piston"
    );
    // E6 to E12
    has(
        pt().replace("algorithm = \"compressible\"", "algorithm = \"piso\"\ndt = 0.01"),
        "dynamic_mesh.deformation",
    );
    has(
        pt().replace("generator = \"box_3d\"", "generator = \"box_2d\""),
        "mesh.generator",
    );
    has(
        pt().replace("generator = \"box_3d\"", "source = \"file\"\nfile = \"none.msh\""),
        "mesh.source",
    );
    has(
        pt().replace("moving_patches = [\"xmax\"]", "moving_patches = [\"xmin\"]"),
        "dynamic_mesh.moving_patches",
    );
    for k in [
        "displacement_expression_y = \"0.01 [m]\"",
        "displacement_expression_z = \"0.01 [m]\"",
        "rotation_angle_expression = \"0.1\"",
    ] {
        has(
            p_t(32.0, 16.0, k, ""),
            &format!("dynamic_mesh.{}", k.split(' ').next().unwrap()),
        );
    }
    has(pt().replace("nx = 40", "nx = 1"), "mesh.nx");
    has(
        p_t(32.0, 16.0, "fixed_patches = [\"xmin\"]", ""),
        "dynamic_mesh.fixed_patches",
    );
    has(
        p_t(32.0, 16.0, "", "[mesh.transform]\nrotate_z_deg = 10.0"),
        "mesh.transform.rotate_z_deg",
    );
    // E13
    has(pt().replace("lz = 0.1", "lz = 0.1\ngrading = 2.0"), "mesh.grading");
    has(
        pt().replace("lz = 0.1", "lz = 0.1\nperiodic_x = true"),
        "mesh.periodic_x",
    );
    has(
        pt().replace("lz = 0.1", "lz = 0.1\nperiodic_y = true"),
        "mesh.periodic_y",
    );
}

/// R1 of ADR 0191: a piston driven through the last static plane stops the run with a located `dynamic_mesh:` failure.
#[test]
fn r1_piston_through_the_last_static_plane_is_a_located_failure() {
    let text = p_t(32.0, 16.0, "", "")
        .replace("nx = 40\nny = 2\nnz = 2", "nx = 4\nny = 1\nnz = 1")
        .replace(
            "-0.3 [m] * (1 - cos(pi * t / 16.0 [s]))",
            "-0.9 [m] * (1 - exp(-t / 1e-6 [s]))",
        );
    let mut s = Simulation::new(&parse(&text)).unwrap();
    let n = s.run(100);
    let f = s.failure().expect("the run must fail");
    println!("R1 after {n} steps: {f}");
    assert!(f.starts_with("dynamic_mesh:") && f.contains("layering"), "{f}");
}

/// FNV-1a 64 of the bits of the final rho, ux, uy, uz and p of every cell, the final time and the step count of a
/// compressible case run to its end through the tree.
fn state_hash(text: &str, base: &std::path::Path) -> (String, usize, f64) {
    let c = Case::parse_toml_in(text, base).expect("case parses");
    let mut s = Simulation::new(&c).expect("simulation builds");
    let steps = s.run(1_000_000);
    let mut bytes = Vec::new();
    for f in ["rho", "ux", "uy", "uz", "p"] {
        for v in s.field(f).expect("field") {
            bytes.extend_from_slice(&v.to_bits().to_le_bytes());
        }
    }
    bytes.extend_from_slice(&s.time().to_bits().to_le_bytes());
    bytes.extend_from_slice(&(steps as u64).to_le_bytes());
    (format!("{:016x}", rcfd_core::hash::fnv64(&bytes)), steps, s.time())
}

/// D0 of ADR 0191: the fixed-mesh compressible path is bit-identical to main 201ea17 (values recorded there before any
/// code change, on Windows; elsewhere printed, ADR 0137).
#[test]
fn d0_fixed_mesh_compressible_runs_are_bit_identical() {
    let sod = std::fs::read_to_string(repo().join("cases/sod/sod.toml")).unwrap();
    let wedge = std::fs::read_to_string(repo().join("cases/wedge/wedge.toml")).unwrap();
    let outlet = sod.replace(
        "[boundaries.xmax]\ncompressible_bc = \"fixed\"\ndensity_value = 0.125\npressure_value = 0.1",
        "[boundaries.xmax]\ncompressible_bc = \"pressure_outlet\"\npressure_value = 0.1",
    );
    assert_ne!(outlet, sod, "set-up: the outlet variant must differ from sod");
    let mut got = Vec::new();
    for (name, text, dir) in [
        ("sod", &sod, "cases/sod"),
        ("wedge", &wedge, "cases/wedge"),
        ("sod_pressure_outlet", &outlet, "cases/sod"),
    ] {
        let (h, steps, t) = state_hash(text, &repo().join(dir));
        println!("D0 {name}: {h} ({steps} steps, t = {t})");
        got.push(h);
    }
    let want = ["919c6d4caef875de", "3c87a648a9da72e0", "220cf1723c972add"];
    if cfg!(windows) {
        assert_eq!(got, want, "D0");
    }
}
