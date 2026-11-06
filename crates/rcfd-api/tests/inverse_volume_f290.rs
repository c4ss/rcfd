//! F-290 (ADR 0203): the value `inverse_volume` of `dynamic_mesh.smoothing.method` runs ADR 0090 D4's `InverseVolume(k)`
//! displacement-diffusion stiffness, with `k` from `dynamic_mesh.smoothing.exponent`. The limits are those registered in
//! ADR 0203.
//!
//! P-I is ADR 0033's piston on a graded mesh (the moving-wall case), P-V ADR 0191's compressible smoothed piston on the same
//! grading. "The library points" of a step are the smoothed points computed here with `rcfd_fv::smoothing` from the tree's
//! initial mesh, and "the library run" of P-V is ADR 0191's loop with library calls only. V1 to V3: the tree equals the
//! library bit for bit; Q1 to Q3 and T: the inverse-volume stiffness preserves or improves the smallest cell, the largest
//! volume ratio and the largest aspect ratio against the default method; D0, D1: cases without the value are unchanged;
//! L: located errors.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::expr::{Compiled, Expr};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::compressible::{CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::FvGeometry;
use rcfd_linalg::SolveControl;
use rcfd_mesh::motion::{mat_vec, moved_mesh, patch_points, rotation_matrix};
use rcfd_mesh::Mesh;
use std::sync::Arc;

/// Steps of P-I (dt 0.005 to t = 0.5).
const STEPS: usize = 100;
/// V1b: exponent 2 must move a point by more than this against exponent 1 (m).
const EXPONENT_TEETH: f64 = 1e-9;
/// T: the inverse-volume mesh must differ from the laplacian one by more than this at the end (m).
const METHOD_TEETH: f64 = 1e-6;
const GAMMA: f64 = 1.4;
/// D0: the content hashes printed before the change (ADR 0203 table).
const D0: [&str; 5] = [
    "536d14dc8993d75b",
    "bd8183148b7a2d8c",
    "2812b71792835d54",
    "836d0833059d0d2b",
    "27a988665a8f9dae",
];

/// ADR 0033's piston of `crates/rcfd-api/tests/dynamic_mesh.rs` (`piston("", "")`), unchanged.
const PISTON_T1: &str = r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 0.5

[dynamic_mesh]
enabled = true
moving_patches = ["xmin"]
deformation = "smoothing"
displacement_expression_x = "1 [m/s^2] * t^2"
fixed_patches = ["xmax"]

"#;

/// P-I of ADR 0203: ADR 0033's piston on a graded mesh (`mesh.grading = 8`, small cells at both ends in x), with the lines
/// `smoothing` in `[dynamic_mesh.smoothing]` (none: the table is left out).
fn p_i(smoothing: &str) -> String {
    let table = if smoothing.is_empty() {
        String::new()
    } else {
        format!("[dynamic_mesh.smoothing]\n{smoothing}")
    };
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.2
grading = 8.0
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 0.5
[dynamic_mesh]
enabled = true
moving_patches = ["xmin"]
deformation = "smoothing"
displacement_expression_x = "1 [m/s^2] * t^2"
fixed_patches = ["xmax"]
{table}"#
    )
}

/// P-V of ADR 0203: ADR 0191's P-B (a compressible smoothed piston with a static pressure outlet), on the graded mesh
/// (`graded`) or ADR 0191's uniform one, with the lines `smoothing` in `[dynamic_mesh.smoothing]`.
fn p_v(graded: bool, smoothing: &str) -> String {
    let grading = if graded { "grading = 8.0\n" } else { "" };
    let table = if smoothing.is_empty() {
        String::new()
    } else {
        format!("[dynamic_mesh.smoothing]\n{smoothing}")
    };
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.1
{grading}
[solver]
algorithm = "compressible"
end_time = 1.0

[compressible]
flux = "hllc"
reconstruction = "tvd_van_leer"
rk_order = 3
cfl = 0.4
gamma = 1.4

[compressible.initial.left]
density = 1.0
pressure = 1.0
velocity = []

[boundaries.xmin]
compressible_bc = "pressure_outlet"
pressure_value = 1.0

[dynamic_mesh]
enabled = true
deformation = "smoothing"
moving_patches = ["xmax"]
fixed_patches = ["xmin"]
displacement_expression_x = "-0.2 [m] * (1 - cos(pi * t / 2 [s]))"
{table}"#
    )
}

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|d| panic!("{d:?}\n{text}"))
}

/// D0: the settings content hashes of cases that do not use the new value, against the values printed before the change.
#[test]
fn d0_hashes_of_cases_without_the_value_are_unchanged() {
    let texts: [(&str, String); 5] = [
        ("dynamic_mesh.rs piston", PISTON_T1.to_string()),
        ("P-I laplacian (no smoothing table)", p_i("")),
        (
            "P-I inverse_distance, exponent 2",
            p_i("method = \"inverse_distance\"\nexponent = 2.0\n"),
        ),
        ("P-V laplacian (no smoothing table)", p_v(true, "")),
        ("ADR 0191 P-B", p_v(false, "")),
    ];
    for ((what, text), want) in texts.iter().zip(D0) {
        let got = format!("{:016x}", case(text).tree.content_hash());
        println!("D0 {what}: {got} (recorded {want})");
        assert_eq!(got, want, "D0 {what}");
    }
}

const IV: &str = "method = \"inverse_volume\"\n";

/// The expression `src` compiled in the tree's context of `case`.
fn compile(case: &Case, src: &str) -> Compiled {
    let ctx = rcfd_api::exprs::spatial_context(case);
    Expr::parse(src).unwrap().compile(&ctx).unwrap()
}

/// The prescribed pose as the tree applies it (no rotation): `origin + R(z, 0) (p0 - origin) + (d_x(t), 0, 0)`.
struct Pose {
    dx: Compiled,
}

impl Pose {
    fn map(&self, p0: Vec3, t: f64) -> Vec3 {
        let slots = [0.0, 0.0, 0.0, t];
        let d = [self.dx.eval(&slots), 0.0, 0.0];
        let (origin, r) = (vec3::ZERO, rotation_matrix([0.0, 0.0, 1.0], 0.0));
        vec3::add(vec3::add(origin, mat_vec(&r, vec3::sub(p0, origin))), d)
    }
}

/// The library side of a smoothed motion: the initial mesh and geometry, the patch classes, the moving patch and the pose.
struct Smoother {
    mesh0: Mesh,
    geom0: FvGeometry,
    classes: Vec<PatchClass>,
    moving: &'static str,
    pose: Pose,
}

impl Smoother {
    fn new(mesh0: &Mesh, moving: &'static str, fixed: &str, pose: Pose) -> Self {
        let classes = mesh0
            .patches()
            .iter()
            .map(|p| {
                if p.name == moving {
                    PatchClass::Prescribed
                } else if p.name == fixed {
                    PatchClass::Fixed
                } else {
                    PatchClass::Slip
                }
            })
            .collect();
        Smoother {
            mesh0: mesh0.clone(),
            geom0: FvGeometry::new(Arc::new(mesh0.clone())).unwrap(),
            classes,
            moving,
            pose,
        }
    }

    /// The library points at time `t`.
    fn points(&self, method: SmoothingMethod, t: f64) -> Vec<Vec3> {
        // the tree's control of the displacement solve: the default tolerance 1e-12
        let control = SolveControl {
            tol_abs: 1e-16,
            tol_rel: 1e-12,
            max_iter: 5000,
        };
        let p0 = self.mesh0.points();
        let mut presc = vec![vec3::ZERO; p0.len()];
        for q in patch_points(&self.mesh0, self.moving).unwrap() {
            presc[q] = vec3::sub(self.pose.map(p0[q], t), p0[q]);
        }
        let sm = smooth_displacement(&self.geom0, &self.classes, &presc, method, &control).unwrap();
        p0.iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect()
    }
}

/// A tree run of a moving-mesh case: the initial mesh and, after every step, its time and mesh.
fn tree_meshes(text: &str) -> (Mesh, Vec<(f64, Arc<Mesh>)>) {
    let c = case(text);
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    let mut s = Simulation::new(&c).unwrap_or_else(|d| panic!("{d:?}"));
    let mesh0 = (*s.geom.mesh).clone();
    let mut out = Vec::new();
    loop {
        let sum = s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        out.push((sum.time, s.geom.mesh.clone()));
        if sum.done {
            break;
        }
    }
    (mesh0, out)
}

/// P-I's library side on the tree's initial mesh.
fn p_i_smoother(mesh0: &Mesh, text: &str) -> Smoother {
    let pose = Pose {
        dx: compile(&case(text), "1 [m/s^2] * t^2"),
    };
    Smoother::new(mesh0, "xmin", "xmax", pose)
}

/// Points of every step that differ in a bit from `want(t)`.
fn differing(run: &[(f64, Arc<Mesh>)], want: impl Fn(f64) -> Vec<Vec3>) -> usize {
    run.iter()
        .map(|(t, m)| {
            let w = want(*t);
            m.points()
                .iter()
                .zip(&w)
                .filter(|(a, b)| (0..3).any(|k| a[k].to_bits() != b[k].to_bits()))
                .count()
        })
        .sum()
}

fn max_distance(a: &Mesh, b: &Mesh) -> f64 {
    a.points()
        .iter()
        .zip(b.points())
        .map(|(p, q)| vec3::norm(vec3::sub(*p, *q)))
        .fold(0.0, f64::max)
}

/// (min_volume, max_volume_ratio, max_aspect_ratio) of a mesh.
fn quality(m: &Mesh) -> [f64; 3] {
    let q = rcfd_mesh::quality::check_mesh(m);
    [q.min_volume, q.max_volume_ratio, q.max_aspect_ratio]
}

/// V1, V1b, Q1 to Q3 and T on P-I.
#[test]
fn v1_q_t_inverse_volume_piston_through_the_tree() {
    let text1 = p_i(IV);
    let (mesh0, iv1) = tree_meshes(&text1);
    let (_, iv2) = tree_meshes(&p_i("method = \"inverse_volume\"\nexponent = 2.0\n"));
    let (_, lap) = tree_meshes(&p_i(""));
    assert_eq!(iv1.len(), STEPS, "P-I steps");
    assert_eq!(iv2.len(), STEPS, "P-I exponent 2 steps");
    assert_eq!(lap.len(), STEPS, "P-I laplacian steps");
    let sm = p_i_smoother(&mesh0, &text1);
    // V1, V1b
    let d1 = differing(&iv1, |t| sm.points(SmoothingMethod::InverseVolume(1.0), t));
    let d2 = differing(&iv2, |t| sm.points(SmoothingMethod::InverseVolume(2.0), t));
    let e12 = max_distance(&iv1[STEPS - 1].1, &iv2[STEPS - 1].1);
    println!(
        "V1: {d1} of {} step points differ in a bit (limit 0); V1b: {d2} (limit 0), exponent 2 against 1 at the end {e12:.3e} m (limit > {EXPONENT_TEETH:e})",
        STEPS * mesh0.n_points()
    );
    // Q1 to Q3 after every step (both runs at the same times)
    let q0 = quality(&mesh0);
    println!(
        "initial mesh: min volume {:.6e}, max volume ratio {:.6}, max aspect ratio {:.6}",
        q0[0], q0[1], q0[2]
    );
    let (mut q1, mut q2, mut q3) = (f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut bad = Vec::new();
    for (i, ((ta, a), (tb, b))) in iv1.iter().zip(&lap).enumerate() {
        assert_eq!(ta.to_bits(), tb.to_bits(), "step {i}: the runs are at the same time");
        let (qa, qb) = (quality(a), quality(b));
        let ratio = qa[0] / qb[0];
        q1 = q1.min(ratio);
        q2 = q2.max(qa[1] - qb[1]);
        q3 = q3.max(qa[2] - qb[2]);
        if !(ratio >= 1.0 && qa[1] <= qb[1] && qa[2] <= qb[2]) {
            bad.push(i + 1);
        }
    }
    let (qa, qb) = (quality(&iv1[STEPS - 1].1), quality(&lap[STEPS - 1].1));
    println!(
        "last step: inverse_volume min volume {:.6e}, max volume ratio {:.6}, max aspect ratio {:.6}; laplacian {:.6e}, {:.6}, {:.6}",
        qa[0], qa[1], qa[2], qb[0], qb[1], qb[2]
    );
    println!(
        "Q1 smallest min-volume ratio inverse_volume / laplacian over the steps {q1:.6} (limit >= 1); Q2 largest volume-ratio excess {q2:.3e} (limit <= 0); Q3 largest aspect-ratio excess {q3:.3e} (limit <= 0); steps that miss: {bad:?}"
    );
    let t = max_distance(&iv1[STEPS - 1].1, &lap[STEPS - 1].1);
    println!("T: inverse_volume against laplacian at the end {t:.3e} m (limit > {METHOD_TEETH:e})");
    assert_eq!(d1, 0, "V1");
    assert_eq!(d2, 0, "V1b");
    assert!(e12 > EXPONENT_TEETH, "V1b exponent teeth {e12}");
    assert!(bad.is_empty(), "Q1 to Q3 missed at steps {bad:?}");
    assert!(t > METHOD_TEETH, "T {t}");
}

/// V3: exponent 0 gives `V^-0 = 1`, the laplacian stiffness, bit for bit.
#[test]
fn v3_exponent_zero_is_the_laplacian_bit_for_bit() {
    let (_, iv0) = tree_meshes(&p_i("method = \"inverse_volume\"\nexponent = 0.0\n"));
    let (_, lap) = tree_meshes(&p_i(""));
    assert_eq!(iv0.len(), lap.len(), "steps");
    let differ: usize = iv0
        .iter()
        .zip(&lap)
        .map(|((_, a), (_, b))| {
            a.points()
                .iter()
                .zip(b.points())
                .filter(|(p, q)| (0..3).any(|k| p[k].to_bits() != q[k].to_bits()))
                .count()
        })
        .sum();
    println!(
        "V3: {differ} step points differ in a bit over {} steps (limit 0)",
        iv0.len()
    );
    assert_eq!(differ, 0, "V3");
}

/// The tree's end test of a transient compressible run.
fn ended(time: f64, end: f64) -> bool {
    time >= end - 1e-12 * end.max(1.0)
}

/// A finished compressible run: steps, cell count after every step, the final primitives and time.
struct Run {
    cells: Vec<usize>,
    prim: Vec<Prim>,
    time: f64,
}

fn tree_compressible(text: &str) -> (Simulation, Run) {
    let c = case(text);
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    let mut s = Simulation::new(&c).unwrap_or_else(|d| panic!("{d:?}"));
    let mut cells = Vec::new();
    loop {
        let sum = s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        cells.push(s.geom.n_cells());
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
        cells,
        prim,
        time: s.solver_time(),
    };
    (s, run)
}

/// ADR 0191's library run of P-B with the stiffness `method`: `step_ale` onto `moved_mesh` of the smoothed points.
fn lib_compressible(sm: &Smoother, method: SmoothingMethod, end: f64) -> Run {
    let rest = Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: 1.0,
    };
    let mut s = CompressibleSolver::new(sm.geom0.clone(), Gas { gamma: GAMMA }, CompSettings::default(), |_| {
        rest
    })
    .unwrap();
    s.set_bc("xmin", CompBc::PressureOutlet { p: 1.0 }).unwrap();
    let mut cells = Vec::new();
    while !ended(s.time, end) {
        let dt = s.max_dt().min((end - s.time).max(1e-300));
        let t_new = s.time + dt;
        let g = FvGeometry::new(Arc::new(moved_mesh(&sm.mesh0, sm.points(method, t_new)).unwrap())).unwrap();
        s.step_ale(g, dt).unwrap();
        cells.push(s.geom.n_cells());
    }
    Run {
        cells,
        prim: s.primitives(),
        time: s.time,
    }
}

/// V2: P-V with `inverse_volume` through the tree against its library run; P-V's quality against laplacian is reported.
#[test]
fn v2_compressible_piston_with_inverse_volume_equals_the_library_run() {
    let text = p_v(true, IV);
    let (s, tree) = tree_compressible(&text);
    let mesh0 = (*Simulation::new(&case(&text)).unwrap().geom.mesh).clone();
    let pose = Pose {
        dx: compile(&case(&text), "-0.2 [m] * (1 - cos(pi * t / 2 [s]))"),
    };
    let sm = Smoother::new(&mesh0, "xmax", "xmin", pose);
    let lib = lib_compressible(&sm, SmoothingMethod::InverseVolume(1.0), 1.0);
    let mut differ = 0;
    for (a, b) in tree.prim.iter().zip(&lib.prim) {
        let fields = [
            (a.rho, b.rho),
            (a.p, b.p),
            (a.u[0], b.u[0]),
            (a.u[1], b.u[1]),
            (a.u[2], b.u[2]),
        ];
        differ += fields.iter().filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    }
    println!(
        "V2 P-V: {} tree steps, {} library steps; {differ} of {} values differ in a bit (limit 0); end time {} / {}",
        tree.cells.len(),
        lib.cells.len(),
        5 * tree.prim.len(),
        tree.time,
        lib.time
    );
    let (lap, lap_run) = tree_compressible(&p_v(true, ""));
    let (qa, qb) = (quality(&s.geom.mesh), quality(&lap.geom.mesh));
    println!("P-V laplacian: {} steps", lap_run.cells.len());
    println!(
        "P-V at its end (no limit): inverse_volume min volume {:.6e}, max volume ratio {:.6}, max aspect ratio {:.6}; laplacian {:.6e}, {:.6}, {:.6}",
        qa[0], qa[1], qa[2], qb[0], qb[1], qb[2]
    );
    assert_eq!(tree.cells, lib.cells, "V2 steps and cell counts");
    assert_eq!(tree.time.to_bits(), lib.time.to_bits(), "V2 end time");
    assert_eq!(differ, 0, "V2");
}

/// D1: `method = "laplacian"` written changes neither the settings hash nor a bit of the run.
#[test]
fn d1_laplacian_written_changes_nothing() {
    let (a, b) = (p_i(""), p_i("method = \"laplacian\"\n"));
    let (ha, hb) = (case(&a).tree.content_hash(), case(&b).tree.content_hash());
    let run = |text: &str| {
        let mut s = Simulation::new(&case(text)).unwrap();
        s.run(10 * STEPS);
        assert!(s.failure().is_none(), "{:?}", s.failure());
        (s.geom.mesh.points().to_vec(), s.field("ux").unwrap())
    };
    let ((pa, ua), (pb, ub)) = (run(&a), run(&b));
    let points = pa
        .iter()
        .zip(&pb)
        .filter(|(p, q)| (0..3).any(|k| p[k].to_bits() != q[k].to_bits()))
        .count();
    let ux = ua.iter().zip(&ub).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    println!("D1: settings hash {ha:016x} / {hb:016x}; {points} points and {ux} ux values differ in a bit (limit 0)");
    assert_eq!(ha, hb, "D1 hash");
    assert_eq!(points, 0, "D1 points");
    assert_eq!(ux, 0, "D1 ux");
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

/// L: located errors of the smoothing keys with `inverse_volume`; the controls give none.
#[test]
fn l_located_errors() {
    for (lines, path) in [
        ("method = \"inverse_volumes\"\n", "dynamic_mesh.smoothing.method"),
        (
            "method = \"inverse_volume\"\nexponent = -1.0\n",
            "dynamic_mesh.smoothing.exponent",
        ),
        (
            "method = \"inverse_volume\"\nexponent = 11.0\n",
            "dynamic_mesh.smoothing.exponent",
        ),
        (
            "method = \"inverse_volume\"\nsubsteps = 2\n",
            "dynamic_mesh.smoothing.substeps",
        ),
    ] {
        let p = paths(&p_i(lines));
        println!("L {lines:?}: {p:?} (want {path})");
        assert!(p.iter().any(|x| x == path), "L {lines:?}: got {p:?}");
    }
    for text in [
        p_i("method = \"inverse_volume\"\nexponent = 0.0\n"),
        p_i(IV),
        p_i("method = \"inverse_volume\"\nexponent = 10.0\n"),
        p_v(true, IV),
    ] {
        let p = paths(&text);
        println!("L control: {p:?} (want none)");
        assert!(p.is_empty(), "L control: {p:?}\n{text}");
    }
}
