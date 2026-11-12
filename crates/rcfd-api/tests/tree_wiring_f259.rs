//! F-259 (ADR 0257): the case tree runs the library work of F-257 (ADR 0213: prism layers, layering on the piso / pimple
//! path) and of F-258 (ADR 0207: characteristic conditions on moving faces, the mesh-speed time step). The limits are
//! those registered in ADR 0257; the CLI checks X3 and X4 are in `crates/rcfd-cli/tests/tree_wiring_f259.rs`.
//!
//! "The library run" of a set-up is a loop written here with library calls only, on the tree's schedule, with the motion
//! evaluated from the case's expression text compiled in the tree's context (ADR 0191). The helpers that ADR 0191's test
//! file `layering_ale_f259.rs` has are repeated here (that file is not edited beyond the ignored record of its test L).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::expr::{Compiled, Expr};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::compressible::{CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::compressible_bc::TotalInlet;
use rcfd_fv::pimple::PimpleSolver;
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::box_2d;
use rcfd_mesh::layering::{remap, remap_face_flux, ExtrudedStack, LayerStack, Section};
use rcfd_mesh::motion::{mat_vec, moved_mesh, rotation_matrix};
use rcfd_mesh::Mesh;
use std::sync::Arc;

/// Equality of a tree run and a library run (ADR 0257, ADR 0191).
const EQ: f64 = 1e-12;
/// U-gcl: a uniform state with moving characteristic faces (ADR 0207 U1).
const GCL: f64 = 1e-12;
/// U-set: steps of T-U and the largest normal mesh speed on each patch.
const U_STEPS: usize = 50;
const U_SPEED: f64 = 0.02;
/// DT-n: the step counts with and without the mesh-speed step differ by this fraction at least.
const DT_CONTROL: f64 = 2e-3;
/// DT-m, P-4: mass drift of every step.
const MASS_DRIFT: f64 = 1e-12;
/// P-2: `p_bar V^gamma` against its initial value.
const ISENTROPE: f64 = 2e-2;
/// P-6, I-4, X-4: events.
const MIN_EVENTS: usize = 5;
/// I-1: continuity of every step.
const CONTINUITY: f64 = 1e-9;
/// I-3: the mesh volume against `0.04 x_p(t)`.
const VOLUME: f64 = 1e-13;
const GAMMA: f64 = 1.4;
const U0: Vec3 = [0.3, 0.1, 0.0];

/// The compressible keys every set-up shares (ADR 0191) and the initial state.
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
gas_constant = 1.0

[compressible.initial.left]
density = 1.0
pressure = 1.0
velocity = {velocity}
"#
    )
}

/// A layered compressible piston on `box_3d` (ADR 0191's P-T and P-X): `n = [nx, ny, nz]`, the end time and the stroke
/// period; `tables` is added at the end.
fn piston(n: [usize; 3], end: f64, period: f64, tables: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_3d"
nx = {}
ny = {}
nz = {}
lx = 1.0
ly = 0.1
lz = 0.1
{}
[dynamic_mesh]
enabled = true
deformation = "layering"
moving_patches = ["xmax"]
displacement_expression_x = "-0.3 [m] * (1 - cos(pi * t / {period:?} [s]))"

{tables}
"#,
        n[0],
        n[1],
        n[2],
        solver(end, "[]")
    )
}

const TRIANGLES: &str = "[mesh.box_3d]\nsection = \"triangles\"\n";
const RECTANGLES: &str = "[mesh.box_3d]\nsection = \"rectangles\"\n";
const MESH_SPEED: &str = "[dynamic_mesh.compressible]\ntime_step = \"mesh_speed\"\n";
const ABSOLUTE: &str = "[dynamic_mesh.compressible]\ntime_step = \"absolute\"\n";

/// P-X of ADR 0191 (T-D0), with `tables`.
fn p_x(tables: &str) -> String {
    piston([20, 1, 1], 4.0, 4.0, tables)
}

/// T-P of ADR 0257: ADR 0191's P-T on prism layers.
fn t_p() -> String {
    piston([40, 2, 2], 32.0, 16.0, TRIANGLES)
}

/// T-U of ADR 0257: uniform flow, the four characteristic conditions, a rigid translation and rotation.
fn t_u() -> String {
    let t0 = 1.0 + 0.2 * vec3::dot(U0, U0) / GAMMA;
    let p0 = t0.powf(3.5);
    let mach = vec3::norm(U0) / GAMMA.sqrt();
    let angle = (U0[1]).atan2(U0[0]).to_degrees();
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 12
ny = 8
lx = 1.2
ly = 0.8
{}
[boundaries.xmin]
compressible_bc = "total_inlet"
total_pressure_value = {p0:?}
temperature_value = {t0:?}
flow_direction = [0.3, 0.1, 0.0]

[boundaries.xmax]
compressible_bc = "pressure_outlet"
pressure_value = 1.0

[boundaries.ymin]
compressible_bc = "farfield"
pressure_value = 1.0
temperature_value = 1.0
mach_value = {mach:?}
flow_angle = {angle:?}

[boundaries.ymax]
compressible_bc = "nonreflecting"
pressure_value = 1.0
reflection_value = 0.5

[dynamic_mesh]
enabled = true
deformation = "rigid"
displacement_expression_x = "0.05 [m] * sin(2 * pi * t / 1 [s])"
displacement_expression_y = "0.02 [m] * sin(2 * pi * t / 0.5 [s])"
rotation_angle_expression = "0.1 * sin(2 * pi * t / 1 [s])"
rotation_origin = [0.6, 0.4, 0.0]
"#,
        solver(1.0, "[0.3, 0.1, 0.0]")
    )
}

/// T-I (`sign = -1`, ADR 0213's G5L) and T-X (`sign = +1`, G5X) of ADR 0257 through the tree; `n = [nx, ny, nz]`,
/// `disp` the displacement expression, `tables` added at the end.
fn channel(n: [usize; 3], disp: &str, section: &str, tables: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_3d"
nx = {}
ny = {}
nz = {}
lx = 1.0
ly = 0.2
lz = 0.2

[physics]
nu = 0.01
rho = 1.0

[boundaries.xmax]
velocity = "fixed"

[boundaries.xmin]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0

[boundaries.ymin]
velocity = "zero_gradient"

[boundaries.ymax]
velocity = "zero_gradient"

[boundaries.zmin]
velocity = "zero_gradient"

[boundaries.zmax]
velocity = "zero_gradient"

[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 0.6
momentum_rel_tol = 1e-13
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-16
momentum_max_iter = 5000
pressure_max_iter = 5000

[dynamic_mesh]
enabled = true
deformation = "layering"
moving_patches = ["xmax"]
displacement_expression_x = "{disp}"

{section}
{tables}
"#,
        n[0], n[1], n[2]
    )
}

const PUSH: &str = "-1 [m/s^2] * t^2";
const PULL: &str = "1 [m/s^2] * t^2";

fn t_i() -> String {
    channel([20, 2, 2], PUSH, RECTANGLES, "")
}

fn t_x() -> String {
    channel([20, 2, 2], PULL, RECTANGLES, "")
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

/// The tree's compressible step: the stable step (absolute, or the mesh-speed one) clipped to the end time.
fn step_of(s: &CompressibleSolver, end: f64, mesh_speed: bool) -> f64 {
    let dt = if mesh_speed { s.max_dt_ale() } else { s.max_dt() };
    dt.min((end - s.time).max(1e-300))
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

/// A finished compressible run: steps, the cell count after every step, the final primitives and the final time.
struct Run {
    steps: usize,
    cells: Vec<usize>,
    prim: Vec<Prim>,
    time: f64,
}

/// A compressible tree run to its end, with a hook after every step.
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
    let prim = prims(&s);
    let run = Run {
        steps: cells.len(),
        cells,
        prim,
        time: s.solver_time(),
    };
    (s, run)
}

fn prims(s: &Simulation) -> Vec<Prim> {
    let f = |n: &str| s.field(n).unwrap();
    let (rho, ux, uy, uz, p) = (f("rho"), f("ux"), f("uy"), f("uz"), f("p"));
    (0..rho.len())
        .map(|i| Prim {
            rho: rho[i],
            u: [ux[i], uy[i], uz[i]],
            p: p[i],
        })
        .collect()
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

fn finish(s: &CompressibleSolver, cells: Vec<usize>) -> Run {
    Run {
        steps: cells.len(),
        cells,
        prim: s.primitives(),
        time: s.time,
    }
}

/// The library run of a layered piston on ADR 0090's box stack (ADR 0191's loop), with the absolute or the mesh-speed step.
fn lib_layered_box(nx: usize, cross: [usize; 2], end: f64, dx: &Compiled, mesh_speed: bool) -> Run {
    let geom = |s: &LayerStack| FvGeometry::new(Arc::new(s.mesh().unwrap())).unwrap();
    let mut stack = LayerStack::uniform(nx, 1.0, cross, [0.1, 0.1], 0.4, 1.6).unwrap();
    let mut s = CompressibleSolver::new(geom(&stack), gas(), CompSettings::default(), |_| rest()).unwrap();
    let mut cells = Vec::new();
    while !ended(s.time, end) {
        let dt = step_of(&s, end, mesh_speed);
        stack.set_piston(1.0 + at(dx, s.time + dt)).unwrap();
        s.step_ale(geom(&stack), dt).unwrap();
        if let Some(ch) = stack.adjust() {
            let g = geom(&stack);
            let u = remap(&ch, &s.u, s.geom.mesh.cell_volumes(), g.mesh.cell_volumes());
            s.replace_mesh(g, u).unwrap();
        }
        cells.push(s.geom.n_cells());
    }
    finish(&s, cells)
}

/// The library run of a layered piston on ADR 0213's extruded stack, with the absolute step.
fn lib_layered_extruded(mut stack: ExtrudedStack, end: f64, dx: &Compiled) -> Run {
    let geom = |s: &ExtrudedStack| FvGeometry::new(Arc::new(s.mesh().unwrap())).unwrap();
    let mut s = CompressibleSolver::new(geom(&stack), gas(), CompSettings::default(), |_| rest()).unwrap();
    let mut cells = Vec::new();
    while !ended(s.time, end) {
        let dt = step_of(&s, end, false);
        stack.set_piston(1.0 + at(dx, s.time + dt)).unwrap();
        s.step_ale(geom(&stack), dt).unwrap();
        if let Some(ch) = stack.adjust() {
            let g = geom(&stack);
            let u = remap(&ch, &s.u, s.geom.mesh.cell_volumes(), g.mesh.cell_volumes());
            s.replace_mesh(g, u).unwrap();
        }
        cells.push(s.geom.n_cells());
    }
    finish(&s, cells)
}

/// `p_bar V^gamma` of a tree state (`p_bar` the volume mean of `p`).
fn isentrope(s: &Simulation) -> f64 {
    let v = s.geom.mesh.cell_volumes();
    let vol: f64 = v.iter().sum();
    let pv: f64 = s.field("p").unwrap().iter().zip(v).map(|(p, v)| p * v).sum();
    pv / vol * vol.powf(GAMMA)
}

fn point_bits(m: &Mesh) -> Vec<u64> {
    m.points().iter().flat_map(|p| p.map(f64::to_bits)).collect()
}

fn patch_names(m: &Mesh) -> Vec<String> {
    m.patches().iter().map(|p| p.name.clone()).collect()
}

/// The bits of the final state of a compressible tree run, its final time and its step count.
fn state_bits(s: &Simulation, run: &Run) -> Vec<u64> {
    let mut b: Vec<u64> = prims(s)
        .iter()
        .flat_map(|p| [p.rho, p.u[0], p.u[1], p.u[2], p.p].map(f64::to_bits))
        .collect();
    b.push(run.time.to_bits());
    b.push(run.steps as u64);
    b
}

/// K2 of ADR 0257: `box_3d` with a section builds the extruded stack's mesh, bit for bit.
#[test]
fn k2_box_3d_sections_are_the_extruded_stack_meshes() {
    let prisms = ExtrudedStack::uniform(Section::rect_triangles([2, 2], [0.1, 0.1]).unwrap(), 40, 1.0, 0.4, 1.6)
        .unwrap()
        .mesh()
        .unwrap();
    let hexes = ExtrudedStack::uniform(Section::rect([2, 2], [0.2, 0.2]).unwrap(), 20, 1.0, 0.4, 1.6)
        .unwrap()
        .mesh()
        .unwrap();
    for (name, text, st, cells) in [("T-P", t_p(), prisms, 320), ("T-I", t_i(), hexes, 80)] {
        let m = rcfd_api::sim::build_mesh(&parse(&text)).unwrap();
        println!(
            "K2 {name}: content hash {:016x} / {:016x}, {} cells, patches {:?}",
            m.content_hash(),
            st.content_hash(),
            m.n_cells(),
            patch_names(&m)
        );
        assert_eq!(m.n_cells(), cells, "K2 cells {name}");
        assert_eq!(m.content_hash(), st.content_hash(), "K2 content hash {name}");
        assert_eq!(point_bits(&m), point_bits(&st), "K2 points {name}");
        assert_eq!(patch_names(&m), patch_names(&st), "K2 patches {name}");
        assert_eq!(
            patch_names(&m),
            ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"],
            "K2 {name}"
        );
    }
}

/// S1 of ADR 0257: a case without the tables writes neither; with a table it is written and the settings hash differs.
#[test]
fn s1_cases_without_the_tables_write_neither() {
    let smoothing =
        channel([20, 2, 2], PUSH, "", "").replace("deformation = \"layering\"", "deformation = \"smoothing\"");
    for (name, plain, with, pointer) in [
        ("P-X", p_x(""), p_x(MESH_SPEED), "/dynamic_mesh/compressible"),
        ("P-X", p_x(""), p_x(TRIANGLES), "/mesh/box_3d"),
        (
            "T-I smoothing",
            smoothing.clone(),
            format!("{smoothing}{RECTANGLES}"),
            "/mesh/box_3d",
        ),
    ] {
        let (a, b) = (parse(&plain), parse(&with));
        let (ta, tb) = (a.to_toml(), b.to_toml());
        let ja: serde_json::Value = serde_json::from_str(&a.to_json_string()).unwrap();
        let jb: serde_json::Value = serde_json::from_str(&b.to_json_string()).unwrap();
        let absent = !ta.contains("[mesh.box_3d]")
            && !ta.contains("[dynamic_mesh.compressible]")
            && ja.pointer("/mesh/box_3d").is_none()
            && ja.pointer("/dynamic_mesh/compressible").is_none();
        let table = format!("[{}]", pointer[1..].replace('/', "."));
        let present = tb.contains(&table) && jb.pointer(pointer).is_some();
        println!(
            "S1 {name} {pointer}: absent without {absent}, present with {present}, hashes {:016x} / {:016x}",
            a.tree.content_hash(),
            b.tree.content_hash()
        );
        assert!(absent, "S1 {name}: a table is written without being set\n{ta}");
        assert!(present, "S1 {name}: {table} is not written\n{tb}");
        assert_ne!(a.tree.content_hash(), b.tree.content_hash(), "S1 {name} {pointer} hash");
    }
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

/// U-eq, U-gcl and U-set of ADR 0257: T-U through the tree against its library run.
#[test]
fn u_moving_characteristic_faces_equal_the_library_run() {
    let text = t_u();
    let case = parse(&text);
    let mut gcl = 0.0f64;
    let (_, tree) = tree_run(&text, |s, _| {
        for p in prims(s) {
            gcl = gcl
                .max((p.rho - 1.0).abs())
                .max(vec3::norm(vec3::sub(p.u, U0)) / vec3::norm(U0))
                .max((p.p - 1.0).abs());
        }
    });
    // the library run: the conditions by the formulas of the case documentation, from the case's own numbers
    let num = |k: &str| case.tree.get(k).unwrap().as_f64().unwrap();
    let state = Prim {
        rho: 1.0,
        u: U0,
        p: 1.0,
    };
    let dir = vec3::scale(U0, 1.0 / vec3::norm(U0));
    let (r, t_inf, p_inf) = (
        num("compressible.gas_constant"),
        num("boundaries.ymin.temperature_value"),
        num("boundaries.ymin.pressure_value"),
    );
    let a = (GAMMA * r * t_inf).sqrt();
    let (m, ang) = (
        num("boundaries.ymin.mach_value"),
        num("boundaries.ymin.flow_angle").to_radians(),
    );
    let bcs = [
        (
            "xmin",
            CompBc::TotalInlet(TotalInlet {
                p0: num("boundaries.xmin.total_pressure_value"),
                t0: num("boundaries.xmin.temperature_value"),
                r_gas: r,
                dir,
            }),
        ),
        ("xmax", CompBc::PressureOutlet { p: 1.0 }),
        (
            "ymin",
            CompBc::FarField(Prim {
                rho: p_inf / (r * t_inf),
                u: [m * a * ang.cos(), m * a * ang.sin(), 0.0],
                p: p_inf,
            }),
        ),
        ("ymax", CompBc::NonReflecting { p: 1.0, sigma: 0.5 }),
    ];
    let pose = Pose {
        disp: [
            Some(compile(&case, "0.05 [m] * sin(2 * pi * t / 1 [s])")),
            Some(compile(&case, "0.02 [m] * sin(2 * pi * t / 0.5 [s])")),
            None,
        ],
        angle: Some(compile(&case, "0.1 * sin(2 * pi * t / 1 [s])")),
        origin: [0.6, 0.4, 0.0],
        axis: [0.0, 0.0, 1.0],
    };
    let mesh0 = box_2d(12, 8, 1.2, 0.8, false, false).unwrap();
    let geom0 = FvGeometry::new(Arc::new(mesh0.clone())).unwrap();
    let mut s = CompressibleSolver::new(geom0, gas(), CompSettings::default(), |_| state).unwrap();
    for (p, bc) in &bcs {
        s.set_bc(p, *bc).unwrap();
    }
    let mut speed = [0.0f64; 4];
    let mut cells = Vec::new();
    while !ended(s.time, 1.0) {
        let dt = step_of(&s, 1.0, false);
        let t_new = s.time + dt;
        let points: Vec<Vec3> = mesh0.points().iter().map(|&p| pose.map(p, t_new)).collect();
        let g = FvGeometry::new(Arc::new(moved_mesh(&mesh0, points).unwrap())).unwrap();
        s.step_ale(g, dt).unwrap();
        let wn = s.mesh_speeds().expect("mesh speeds after an ALE step");
        for (k, (name, _)) in bcs.iter().enumerate() {
            for f in s.geom.mesh.patch(name).unwrap().faces() {
                speed[k] = speed[k].max(wn[f].abs());
            }
        }
        cells.push(s.geom.n_cells());
    }
    let lib = finish(&s, cells);
    let e = equality(&tree, &lib);
    println!(
        "U-eq T-U: {} steps, rho {:.2e}, p {:.2e}, u {:.2e} (limit {EQ:e}); U-gcl {gcl:.2e} (limit {GCL:e})",
        tree.steps, e[0], e[1], e[2]
    );
    println!(
        "U-set: {} steps (>= {U_STEPS}); largest |w_n| on xmin, xmax, ymin, ymax {speed:.3?} (each >= {U_SPEED})",
        lib.steps
    );
    assert!(e.iter().all(|x| *x <= EQ), "U-eq {e:?}");
    assert!(gcl <= GCL, "U-gcl {gcl}");
    assert!(lib.steps >= U_STEPS, "U-set steps {}", lib.steps);
    assert!(speed.iter().all(|w| *w >= U_SPEED), "U-set {speed:?}");
}

/// S2, DT-eq, DT-n and DT-m of ADR 0257: the mesh-speed step is opt-in, and with it the tree equals the library run.
#[test]
fn dt_mesh_speed_step_is_opt_in_and_equals_the_library_run() {
    let (s0, plain) = tree_run(&p_x(""), |_, _| {});
    let (sa, absolute) = tree_run(&p_x(ABSOLUTE), |_, _| {});
    let s2 = state_bits(&s0, &plain) == state_bits(&sa, &absolute);
    let text = p_x(MESH_SPEED);
    let case = parse(&text);
    let mut drift = 0.0f64;
    let (_, tree) = tree_run(&text, |_, sum| drift = drift.max(sum.continuity));
    let dx = compile(&case, "-0.3 [m] * (1 - cos(pi * t / 4.0 [s]))");
    let lib = lib_layered_box(20, [1, 1], 4.0, &dx, true);
    let e = equality(&tree, &lib);
    let control = (plain.steps as f64 - tree.steps as f64).abs() / plain.steps as f64;
    println!(
        "S2 time_step = \"absolute\" against no table: equal bits {s2} ({} steps both: {})",
        plain.steps,
        plain.steps == absolute.steps
    );
    println!(
        "DT-eq T-D: {} steps, final cells {}, rho {:.2e}, p {:.2e}, u {:.2e} (limit {EQ:e})",
        tree.steps,
        tree.cells.last().unwrap(),
        e[0],
        e[1],
        e[2]
    );
    println!(
        "DT-n steps {} with the mesh-speed step, {} without: {:.3e} of the latter (>= {DT_CONTROL:e}); DT-m max mass drift {drift:.2e} (limit {MASS_DRIFT:e})",
        tree.steps, plain.steps, control
    );
    assert!(s2, "S2");
    assert!(e.iter().all(|x| *x <= EQ), "DT-eq {e:?}");
    assert!(control >= DT_CONTROL, "DT-n {control}");
    assert!(drift <= MASS_DRIFT, "DT-m {drift}");
}

/// P-eq, P-2, P-4 and P-6 of ADR 0257: prism layers through the tree against the library run.
#[test]
fn p_prism_layers_through_the_tree_equal_the_library_run() {
    let clock = std::time::Instant::now();
    let text = t_p();
    let case = parse(&text);
    let dx = compile(&case, "-0.3 [m] * (1 - cos(pi * t / 16.0 [s]))");
    let s0 = Simulation::new(&case).unwrap();
    let inv0 = isentrope(&s0);
    let mut drift = 0.0f64;
    let mut prev = s0.geom.n_cells() as i64;
    let (mut removed, mut added, mut other) = (0usize, 0usize, 0usize);
    let (s, tree) = tree_run(&text, |s, sum| {
        drift = drift.max(sum.continuity);
        let n = s.geom.n_cells() as i64;
        match n - prev {
            0 => {}
            -8 => removed += 1,
            8 => added += 1,
            _ => other += 1,
        }
        prev = n;
    });
    let p2 = isentrope(&s) / inv0 - 1.0;
    let stack =
        ExtrudedStack::uniform(Section::rect_triangles([2, 2], [0.1, 0.1]).unwrap(), 40, 1.0, 0.4, 1.6).unwrap();
    let lib = lib_layered_extruded(stack, 32.0, &dx);
    let e = equality(&tree, &lib);
    println!(
        "P-eq T-P: {} steps, final cells {}, rho {:.2e}, p {:.2e}, u {:.2e} (limit {EQ:e}); {:.1} s",
        tree.steps,
        tree.cells.last().unwrap(),
        e[0],
        e[1],
        e[2],
        clock.elapsed().as_secs_f64()
    );
    println!("P-2 p_bar V^gamma / (p0 V0^gamma) - 1 at t = 32: {p2:.3e} (limit {ISENTROPE:e}); P-4 max mass drift {drift:.2e} (limit {MASS_DRIFT:e})");
    println!("P-6 removals {removed}, additions {added} (each >= {MIN_EVENTS}); other cell-count changes {other}");
    assert!(e.iter().all(|x| *x <= EQ), "P-eq {e:?}");
    assert!(p2.abs() <= ISENTROPE, "P-2 {p2}");
    assert!(drift <= MASS_DRIFT, "P-4 {drift}");
    assert!(
        removed >= MIN_EVENTS && added >= MIN_EVENTS && other == 0,
        "P-6 {removed} {added} {other}"
    );
}

/// The step of a fixed-step transient run of the tree (`fixed_step` of `rcfd-api`'s `sim.rs`, ADR 0145 D1): the step
/// and whether it was stretched onto the end time.
fn tree_fixed_step(t: f64, dt: f64, end: f64) -> (f64, bool) {
    let d = dt.min((end - t).max(1e-12));
    let after = t + d;
    if after < end - 1e-12 && end - after < 1e-6 * d {
        (end - t, true)
    } else {
        (d, false)
    }
}

/// The final state of an incompressible run and what the tree run recorded on the way.
struct Flow {
    cells: Vec<usize>,
    u: Vec<Vec3>,
    p: Vec<f64>,
    phi: Vec<f64>,
    continuity: f64,
    volume: f64,
}

/// T-I or T-X through the tree; `sign` is -1 for the pushed piston and +1 for the pulled one.
fn channel_tree(text: &str, sign: f64) -> Flow {
    let c = parse(text);
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    let mut s = Simulation::new(&c).unwrap_or_else(|d| panic!("{d:?}"));
    let mut out = Flow {
        cells: Vec::new(),
        u: Vec::new(),
        p: Vec::new(),
        phi: Vec::new(),
        continuity: 0.0,
        volume: 0.0,
    };
    loop {
        let sum = s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        out.cells.push(s.geom.n_cells());
        out.continuity = out.continuity.max(sum.continuity);
        let t = s.solver_time();
        let vol: f64 = s.geom.mesh.cell_volumes().iter().sum();
        out.volume = out.volume.max((vol / (0.04 * (1.0 + sign * t * t)) - 1.0).abs());
        if sum.done {
            break;
        }
    }
    let f = |n: &str| s.field(n).unwrap();
    let (ux, uy, uz) = (f("ux"), f("uy"), f("uz"));
    out.u = (0..ux.len()).map(|i| [ux[i], uy[i], uz[i]]).collect();
    out.p = f("p");
    out.phi = s.flux().to_vec();
    out
}

/// The library run of T-I or T-X: ADR 0213's channel loop on a `PimpleSolver` made from the geometry, the initial
/// fields with their conditions and the settings of the tree's solver before its first step (ADR 0257).
fn channel_lib(text: &str, disp: &str) -> Flow {
    let case = parse(text);
    let sim0 = Simulation::new(&case).unwrap();
    let s0 = sim0.pimple_solver().expect("the transient solver");
    let mut stack = ExtrudedStack::uniform(Section::rect([2, 2], [0.2, 0.2]).unwrap(), 20, 1.0, 0.4, 1.6).unwrap();
    assert_eq!(
        point_bits(&stack.mesh().unwrap()),
        point_bits(&s0.geom.mesh),
        "set-up: the stack is the tree's mesh"
    );
    let mut s = PimpleSolver::new(s0.geom.clone(), s0.u.clone(), s0.p.clone(), s0.settings.clone());
    let dx = compile(&case, disp);
    let (dt0, end) = (0.005, 0.6);
    let follow = ["xmax".to_string()];
    let mut out = Flow {
        cells: Vec::new(),
        u: Vec::new(),
        p: Vec::new(),
        phi: Vec::new(),
        continuity: 0.0,
        volume: 0.0,
    };
    loop {
        let (dt, last) = tree_fixed_step(s.time, dt0, end);
        stack.set_piston(1.0 + at(&dx, s.time + dt)).unwrap();
        let st = move_mesh(&s.geom, stack.points(), dt).unwrap();
        let old = s.geom.mesh.clone();
        let walls = set_follow_conditions(&mut s.u, &st, &old, &follow, dt).unwrap();
        let info = s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
        if let Some(change) = stack.adjust() {
            let old = s.geom.clone();
            let mesh = stack.mesh().unwrap();
            let sources = stack.face_sources(&change, &old.mesh, &mesh).unwrap();
            let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
            let (vo, vn) = (old.mesh.cell_volumes(), geom.mesh.cell_volumes());
            let u = remap(&change, &s.u.values, vo, vn);
            let p1: Vec<[f64; 1]> = s.p.values.iter().map(|x| [*x]).collect();
            let p: Vec<f64> = remap(&change, &p1, vo, vn).into_iter().map(|x| x[0]).collect();
            let phi = remap_face_flux(&sources, &s.phi, &geom.mesh);
            s.replace_mesh(geom, u, p, phi).unwrap();
        }
        out.cells.push(s.geom.n_cells());
        if last || info.time >= end - 1e-12 {
            break;
        }
    }
    out.u = s.u.values.clone();
    out.p = s.p.values.clone();
    out.phi = s.phi.clone();
    out
}

/// Equality of ADR 0257 for an incompressible run: (u, p, phi) differences against `U = 1.2`, the largest library
/// pressure and the largest library flux.
fn flow_equality(tree: &Flow, lib: &Flow) -> [f64; 3] {
    assert_eq!(tree.cells, lib.cells, "steps and cell count after every step");
    assert_eq!(tree.phi.len(), lib.phi.len(), "faces");
    let big = |v: &[f64]| v.iter().fold(0.0f64, |a, b| a.max(b.abs()));
    let (p_max, phi_max) = (big(&lib.p), big(&lib.phi));
    let mut e = [0.0f64; 3];
    for (a, b) in tree.u.iter().zip(&lib.u) {
        for k in 0..3 {
            e[0] = e[0].max((a[k] - b[k]).abs() / 1.2);
        }
    }
    for (a, b) in tree.p.iter().zip(&lib.p) {
        e[1] = e[1].max((a - b).abs() / p_max);
    }
    for (a, b) in tree.phi.iter().zip(&lib.phi) {
        e[2] = e[2].max((a - b).abs() / phi_max);
    }
    e
}

/// Layer events of a run: (removals, additions, other changes) for `per_layer` cells per layer.
fn events(cells: &[usize], start: usize, per_layer: i64) -> (usize, usize, usize) {
    let (mut removed, mut added, mut other) = (0, 0, 0);
    let mut prev = start as i64;
    for &n in cells {
        let d = n as i64 - prev;
        if d == -per_layer {
            removed += 1;
        } else if d == per_layer {
            added += 1;
        } else if d != 0 {
            other += 1;
        }
        prev = n as i64;
    }
    (removed, added, other)
}

/// I-eq, I-1, I-3 and I-4 of ADR 0257: layering on the piso path through the tree against the library run.
#[test]
fn i_incompressible_layering_through_the_tree_equals_the_library_run() {
    let text = t_i();
    let tree = channel_tree(&text, -1.0);
    let lib = channel_lib(&text, PUSH);
    let e = flow_equality(&tree, &lib);
    let (removed, added, other) = events(&tree.cells, 80, 4);
    println!(
        "I-eq T-I: {} steps, final cells {}, u {:.2e}, p {:.2e}, phi {:.2e} (limit {EQ:e})",
        tree.cells.len(),
        tree.cells.last().unwrap(),
        e[0],
        e[1],
        e[2]
    );
    println!(
        "I-1 max continuity {:.2e} (limit {CONTINUITY:e}); I-3 max volume error {:.2e} (limit {VOLUME:e}); I-4 removals {removed} (>= {MIN_EVENTS}), additions {added}, other {other}",
        tree.continuity, tree.volume
    );
    assert!(e.iter().all(|x| *x <= EQ), "I-eq {e:?}");
    assert!(tree.continuity <= CONTINUITY, "I-1 {}", tree.continuity);
    assert!(tree.volume <= VOLUME, "I-3 {}", tree.volume);
    assert!(removed >= MIN_EVENTS && other == 0, "I-4 {removed} {other}");
}

/// X-eq and X-4 of ADR 0257: layer additions on the piso path through the tree against the library run.
#[test]
fn x_incompressible_layer_additions_through_the_tree_equal_the_library_run() {
    let text = t_x();
    let tree = channel_tree(&text, 1.0);
    let lib = channel_lib(&text, PULL);
    let e = flow_equality(&tree, &lib);
    let (removed, added, other) = events(&tree.cells, 80, 4);
    println!(
        "X-eq T-X: {} steps, final cells {}, u {:.2e}, p {:.2e}, phi {:.2e} (limit {EQ:e}); X-4 additions {added} (>= {MIN_EVENTS}), removals {removed}, other {other}; printed: max continuity {:.2e}, max volume error {:.2e}",
        tree.cells.len(),
        tree.cells.last().unwrap(),
        e[0],
        e[1],
        e[2],
        tree.continuity,
        tree.volume
    );
    assert!(e.iter().all(|x| *x <= EQ), "X-eq {e:?}");
    assert!(added >= MIN_EVENTS && other == 0, "X-4 {added} {other}");
}

/// ADR 0191's set-ups P-T, P-B and P-C, as its test L writes them (repeated here; that test is an ignored record).
mod adr0191 {
    use super::solver as solver_with_gas_constant;

    fn solver(end: f64, velocity: &str) -> String {
        solver_with_gas_constant(end, velocity).replace("gas_constant = 1.0\n", "")
    }

    pub fn p_t(dm: &str, tables: &str) -> String {
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
displacement_expression_x = "-0.3 [m] * (1 - cos(pi * t / 16.0 [s]))"
{dm}
{tables}
"#,
            solver(32.0, "[]")
        )
    }

    pub fn p_b(dm: &str, tables: &str) -> String {
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

    pub fn p_c(dm: &str, tables: &str) -> String {
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
}

/// L2 of ADR 0257: the new located errors N1 to N6; ADR 0191's E1 to E4 and E6 to E13 on its own inputs; its three E5
/// inputs and its E5 control now run; the set-ups of this file give no diagnostic.
#[test]
fn l2_located_errors() {
    use adr0191::{p_b, p_c, p_t};
    let has = |text: String, path: &str| {
        let p = paths(&text);
        assert!(p.iter().any(|x| x == path), "{path}: got {p:?}\n{text}");
        println!("L2 {path}: {p:?}");
    };
    let none = |name: &str, text: &str| {
        assert_eq!(paths(text), Vec::<String>::new(), "{name}\n{text}");
    };
    for (name, text) in [
        ("T-U", t_u()),
        ("T-D", p_x(MESH_SPEED)),
        ("T-Da", p_x(ABSOLUTE)),
        ("T-P", t_p()),
        ("T-I", t_i()),
        ("T-X", t_x()),
        ("P-T", p_t("", "")),
        ("P-B", p_b("", "")),
        ("P-C", p_c("", "")),
    ] {
        none(name, &text);
    }
    // N1: unknown values and an unknown key
    has(
        t_p().replace("section = \"triangles\"", "section = \"prisms\""),
        "mesh.box_3d.section",
    );
    has(
        p_x(MESH_SPEED).replace("time_step = \"mesh_speed\"", "time_step = \"relative\""),
        "dynamic_mesh.compressible.time_step",
    );
    for (text, table) in [
        (
            t_p().replace("section = \"triangles\"", "sections = \"triangles\""),
            "mesh.box_3d",
        ),
        (
            p_x(MESH_SPEED).replace("time_step = \"mesh_speed\"", "step = \"mesh_speed\""),
            "dynamic_mesh.compressible",
        ),
    ] {
        let p = paths(&text);
        assert!(p.iter().any(|x| x.starts_with(table)), "{table}: got {p:?}\n{text}");
        println!("L2 N1 unknown key in {table}: {p:?}");
    }
    // N2: the section with another generator, with and without a moving mesh, and with a mesh file
    has(p_b("", TRIANGLES), "mesh.box_3d.section");
    has(
        p_b("", RECTANGLES).replace("enabled = true", "enabled = false"),
        "mesh.box_3d.section",
    );
    has(
        t_p().replace("generator = \"box_3d\"", "source = \"file\"\nfile = \"none.msh\""),
        "mesh.box_3d.section",
    );
    // N3: fewer than 2 layers, with layering (static) and without a moving mesh (with the mesh)
    has(t_p().replace("nx = 40", "nx = 1"), "mesh.nx");
    has(
        t_p()
            .replace("nx = 40", "nx = 1")
            .replace("enabled = true", "enabled = false"),
        "mesh.nx",
    );
    // N4: layering with piso on the box stack, and with a steady algorithm
    has(channel([20, 2, 2], PUSH, "", ""), "dynamic_mesh.deformation");
    has(
        t_i().replace("algorithm = \"piso\"", "algorithm = \"simple\""),
        "dynamic_mesh.deformation",
    );
    // N5: the mesh-speed step with piso
    has(
        channel([20, 2, 2], PUSH, RECTANGLES, MESH_SPEED),
        "dynamic_mesh.compressible.time_step",
    );
    // N6: an object sized to the mesh with incompressible layering
    has(
        t_i().replace(
            "[boundaries.ymin]\nvelocity = \"zero_gradient\"",
            "[boundaries.ymin]\nvelocity = \"fixed\"\nvelocity_expression_x = \"0.1 [m/s]\"",
        ),
        "dynamic_mesh.deformation",
    );
    // ADR 0191 E1 to E4
    let pt = || p_t("", "");
    has(
        pt().replace("gamma = 1.4", "gamma = 1.4\nmode = \"steady\""),
        "dynamic_mesh.enabled",
    );
    has(p_b("motion = \"six_dof\"", ""), "dynamic_mesh.motion");
    has(p_b("follow_patches = [\"xmax\"]", ""), "dynamic_mesh.follow_patches");
    has(p_c("rest_patches = [\"ymin\"]", ""), "dynamic_mesh.rest_patches");
    // ADR 0191's three E5 inputs and its E5 control: no diagnostic, and five steps run (ADR 0257 D1)
    let e5 = [
        p_b("", "").replace("fixed_patches = [\"xmin\"]", "fixed_patches = []"),
        p_c("", "").replace(
            "[boundaries.xmax]\ncompressible_bc = \"fixed\"",
            "[boundaries.xmax]\ncompressible_bc = \"pressure_outlet\"",
        ),
        p_t(
            "",
            "[boundaries.ymin]\ncompressible_bc = \"nonreflecting\"\npressure_value = 1.0",
        ),
        p_t(
            "",
            "[boundaries.xmin]\ncompressible_bc = \"pressure_outlet\"\npressure_value = 1.0",
        ),
    ];
    assert_ne!(e5[0], p_b("", ""), "set-up: E5 input 1");
    assert_ne!(e5[1], p_c("", ""), "set-up: E5 input 2");
    for (k, text) in e5.iter().enumerate() {
        none("E5 input", text);
        let mut s = Simulation::new(&parse(text)).unwrap();
        for _ in 0..5 {
            s.step();
            assert!(s.failure().is_none(), "E5 input {k}: {:?}", s.failure());
        }
        println!(
            "L2 E5 input {}: no diagnostic, 5 steps to t = {:.4e}",
            k + 1,
            s.solver_time()
        );
    }
    // ADR 0191 E6 to E12
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
        has(p_t(k, ""), &format!("dynamic_mesh.{}", k.split(' ').next().unwrap()));
    }
    has(pt().replace("nx = 40", "nx = 1"), "mesh.nx");
    has(p_t("fixed_patches = [\"xmin\"]", ""), "dynamic_mesh.fixed_patches");
    has(
        p_t("", "[mesh.transform]\nrotate_z_deg = 10.0"),
        "mesh.transform.rotate_z_deg",
    );
    // ADR 0191 E13
    has(pt().replace("lz = 0.1", "lz = 0.1\ngrading = 2.0"), "mesh.grading");
    has(
        pt().replace("lz = 0.1", "lz = 0.1\nperiodic_x = true"),
        "mesh.periodic_x",
    );
    has(
        pt().replace("lz = 0.1", "lz = 0.1\nperiodic_y = true"),
        "mesh.periodic_y",
    );
    // E8 with incompressible layering
    has(
        t_i().replace("moving_patches = [\"xmax\"]", "moving_patches = [\"xmin\"]"),
        "dynamic_mesh.moving_patches",
    );
}

/// R2 of ADR 0257: an incompressible piston driven through the last static plane stops the run with a located failure.
#[test]
fn r2_incompressible_piston_through_the_last_static_plane_is_a_located_failure() {
    let text = channel([4, 1, 1], "-0.9 [m] * (1 - exp(-t / 1e-6 [s]))", RECTANGLES, "");
    let mut s = Simulation::new(&parse(&text)).unwrap_or_else(|d| panic!("{d:?}"));
    let n = s.run(100);
    let f = s.failure().expect("the run must fail");
    println!("R2 after {n} steps: {f}");
    assert!(f.starts_with("dynamic_mesh:") && f.contains("layering"), "{f}");
}
