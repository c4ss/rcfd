//! F-342 (ADR 0262): the case key `[mesh.adapt]` runs the adaptive steady loop of `rcfd_fv::adaptive` (ADR 0242, ADR 0247).
//! Limits registered in ADR 0262 before any of these tests was run.
//!
//! Every tree run is compared with a library run written here with `rcfd_fv::adaptive` calls only, on a lattice built here
//! from `axis_coords` as ADR 0262 D2 states and with ADR 0242's `setup` (copied unchanged from
//! `crates/rcfd-fv/tests/adapt_cavity_f102.rs`). R1 (the 24 x 24 cavity of the case file against ADR 0242's own runs) is
//! ignored as heavy: run once in release with `--include-ignored`.

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_core::vec3::Vec3;
use rcfd_core::Result;
use rcfd_fv::adaptive::{
    fixed_fraction_marks, register_marks, restrict_marks, run_adaptive_simple, run_adaptive_simple_marked,
    run_adaptive_simple_target, run_adaptive_simple_with, velocity_edge_length_indicator, velocity_gradient_indicator,
    AdaptiveRun, AdaptiveSettings, Register,
};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::gradient::least_squares;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::adapt::{AmrMesh, RefineTransfer};
use rcfd_mesh::gen::{axis_coords, Grading};
use rcfd_mesh::PatchKind;
use std::path::Path;
use std::sync::Arc;

const AXES: [bool; 3] = [true, true, false];
const MAX_ITER: usize = 20000;
const TOL: f64 = 1e-7;

// ------------------------------------------------------------------------------------------------ set-ups (ADR 0262)

/// T-S: the 8 x 8 cavity with ADR 0242's solver settings and an empty table (ADR 0242's loop).
const T_S: &str = r#"title = "F-342 T-S: adapted Re 100 cavity on an 8 x 8 base"

[mesh]
generator = "cavity_2d"
nx = 8
ny = 8
lx = 1.0
ly = 1.0

[mesh.adapt]

[physics]
nu = 0.01

[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[boundaries.walls]
velocity = "fixed"

[solver]
algorithm = "simplec"
convection = "linear_upwind"
non_orthogonal_correctors = 1
simplec_non_orthogonal_compensation = true
"#;

/// T-C: a box_2d channel with the simple algorithm, one adaptation step.
const T_C: &str = r#"title = "F-342 T-C: adapted box_2d channel"

[mesh]
generator = "box_2d"
nx = 16
ny = 4
lx = 2.0
ly = 0.5

[mesh.adapt]
cycles = 1
max_iterations = 2000

[physics]
nu = 0.01

[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0

[boundaries.ymin]
velocity = "fixed"

[boundaries.ymax]
velocity = "fixed"

[solver]
algorithm = "simple"
alpha_u = 0.7
alpha_p = 0.3
"#;

/// T-3: a lid-driven box_3d cube, one adaptation step of 20 iterations per solve.
const T_3: &str = r#"title = "F-342 T-3: adapted box_3d cube"

[mesh]
generator = "box_3d"
nx = 4
ny = 4
nz = 4
lx = 1.0
ly = 1.0
lz = 1.0

[mesh.adapt]
cycles = 1
max_level = 1
refine_fraction = 0.25
max_iterations = 20

[physics]
nu = 0.01

[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[boundaries.xmin]
velocity = "fixed"

[boundaries.xmax]
velocity = "fixed"

[boundaries.ymin]
velocity = "fixed"

[boundaries.zmin]
velocity = "fixed"

[boundaries.zmax]
velocity = "fixed"

[solver]
algorithm = "simplec"
convection = "linear_upwind"
non_orthogonal_correctors = 1
simplec_non_orthogonal_compensation = true
"#;

const REGISTER: &str = "\n[registers.top]\ntype = \"box\"\nbox_min = [0.0, 0.75, -1.0]\nbox_max = [1.0, 1.0, 1.0]\n";

/// A set-up with lines added at the top of its `[mesh.adapt]` table.
fn adapt_with(base: &str, lines: &str) -> String {
    assert!(base.contains("[mesh.adapt]\n"));
    base.replacen("[mesh.adapt]\n", &format!("[mesh.adapt]\n{lines}"), 1)
}

fn t_t() -> String {
    adapt_with(T_S, "marking = \"target_count\"\ntarget_cells = 300\n")
}

fn t_e() -> String {
    adapt_with(T_S, "indicator = \"edge_length\"\nedge_length_exponent = 2.0\n")
}

fn t_u() -> String {
    adapt_with(
        T_S,
        "indicator = \"velocity_gradient_unscaled\"\ntransfer = \"injection\"\nbalance = false\n",
    )
}

fn t_rc() -> String {
    format!("{}{REGISTER}", adapt_with(T_S, "register = \"top\"\n"))
}

fn t_rr() -> String {
    format!(
        "{}{REGISTER}",
        adapt_with(T_S, "register = \"top\"\nregister_use = \"refine\"\ncycles = 1\n")
    )
}

fn t_z() -> String {
    adapt_with(T_S, "cycles = 0\n")
}

fn repo() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

// ------------------------------------------------------------------------------------------------ the tree and the library

struct Tree {
    sim: Simulation,
}

impl Tree {
    fn run(text: &str) -> Tree {
        let case = Case::parse_toml(text).unwrap_or_else(|d| panic!("{d:?}"));
        let mut sim = Simulation::new(&case).unwrap_or_else(|d| panic!("{d:?}"));
        let steps = sim.run(10);
        assert_eq!(steps, 1, "the adaptive run is one step");
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        Tree { sim }
    }

    fn solver(&self) -> &SimpleSolver {
        self.sim.simple_solver().unwrap()
    }
}

/// ADR 0242's lattice of the cavity, with the slab depth given (the generator's `lx / nx`, or ADR 0242's 0.1).
fn cavity_lattice(n: usize, max_level: u8, depth: f64) -> AmrMesh {
    let w = |s: &str| (s.to_string(), PatchKind::Wall);
    AmrMesh::new(
        [
            axis_coords(n, 1.0, Grading::Uniform).unwrap(),
            axis_coords(n, 1.0, Grading::Uniform).unwrap(),
            axis_coords(1, depth, Grading::Uniform).unwrap(),
        ],
        AXES,
        max_level,
        [
            w("walls"),
            w("walls"),
            w("walls"),
            w("lid"),
            ("frontAndBack".into(), PatchKind::Empty),
            ("frontAndBack".into(), PatchKind::Empty),
        ],
    )
    .unwrap()
}

/// The lattice of T-S (D2: the generator's slab `lx / nx`).
fn ts_lattice(n: usize, max_level: u8) -> AmrMesh {
    cavity_lattice(n, max_level, 1.0 / n as f64)
}

// ADR 0242's set-up, copied unchanged from crates/rcfd-fv/tests/adapt_cavity_f102.rs
fn cavity_settings() -> SimpleSettings {
    let mut s = SimpleSettings::new(0.01, Coupling::Simplec);
    s.convection = ConvScheme::LinearUpwind;
    s.non_orth_correctors = 1;
    s.simplec_non_orth_compensation = true;
    s
}

fn cavity_setup(geom: &Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)> {
    let mut u = VectorField::uniform("U", geom, [0.0; 3]);
    u.set_bc(geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))?;
    u.set_bc(geom, "walls", Box::new(FixedValue::vector([0.0; 3])))?;
    let p = ScalarField::uniform("p", geom, 0.0);
    Ok((u, p, cavity_settings()))
}

fn adaptive_settings(cycles: usize, max_iter: usize) -> AdaptiveSettings {
    AdaptiveSettings {
        cycles,
        refine_fraction: 0.4,
        balance: true,
        transfer: RefineTransfer::Linear,
        scale_by_length: true,
        max_iter,
        tol: TOL,
    }
}
// end of ADR 0242's set-up

fn top_register(s: &SimpleSolver) -> Register {
    let r = rcfd_post::registers::Register::in_box(&s.geom.mesh, [0.0, 0.75, -1.0], [1.0, 1.0, 1.0]).unwrap();
    Register::from_mask(r.mask().to_vec())
}

/// Bit-for-bit equality of a tree run and a library run (ADR 0262 "Equality"); prints the sizes.
fn assert_same(id: &str, tree: &Tree, lib: &AdaptiveRun) {
    let t = tree.solver();
    let cycles = tree.sim.adaptive_cycles().unwrap();
    let lattice = tree.sim.adaptive_lattice().unwrap();
    println!(
        "{id}: tree {} cells, cycles {:?}; library {} cells",
        t.geom.n_cells(),
        cycles
            .iter()
            .map(|c| (c.cells, c.iterations, c.converged))
            .collect::<Vec<_>>(),
        lib.solver.geom.n_cells()
    );
    assert_eq!(cycles, lib.cycles.as_slice(), "{id}: cycle records");
    assert_eq!(lattice.levels(), lib.amr.levels(), "{id}: levels");
    assert_eq!(
        t.geom.mesh.content_hash(),
        lib.solver.geom.mesh.content_hash(),
        "{id}: mesh"
    );
    assert!(t.u.values == lib.solver.u.values, "{id}: u differs");
    assert!(t.p.values == lib.solver.p.values, "{id}: p differs");
    assert!(t.phi == lib.solver.phi, "{id}: phi differs");
    assert_eq!(
        tree.sim.geom.n_cells(),
        lib.solver.geom.n_cells(),
        "{id}: Simulation.geom"
    );
}

/// Whether two tree runs differ (the final cell count, the levels or a bit of `u`).
fn differ(a: &Tree, b: &Tree) -> bool {
    a.solver().geom.n_cells() != b.solver().geom.n_cells()
        || a.sim.adaptive_lattice().unwrap().levels() != b.sim.adaptive_lattice().unwrap().levels()
        || a.solver().u.values != b.solver().u.values
}

// ------------------------------------------------------------------------------------------------ S1

#[test]
#[ignore = "record (ADR 0262 amendment 1): defective set-up, the substring `adapt` is in T-S's title; corrected by s1b"]
fn s1_a_case_without_the_table_keeps_its_text_hash_and_mesh() {
    let without = T_S.replace("[mesh.adapt]\n", "");
    assert_ne!(without, T_S);
    let c0 = Case::parse_toml(&without).unwrap();
    let c1 = Case::parse_toml(T_S).unwrap();
    assert!(!c0.to_toml().contains("adapt") && !c0.to_json_string().contains("adapt"));
    assert_eq!(c0.get("mesh.adapt.cycles").unwrap(), serde_json::json!(2));
    let sim = Simulation::new(&c0).unwrap();
    assert_eq!(sim.geom.mesh.content_hash(), build_mesh(&c0).unwrap().content_hash());
    assert!(sim.adaptive_cycles().is_none());
    assert!(c1.to_toml().contains("adapt") && c1.to_json_string().contains("adapt"));
    println!(
        "S1: settings hash without {:016x}, with {:016x}",
        c0.tree.content_hash(),
        c1.tree.content_hash()
    );
    assert_ne!(c0.tree.content_hash(), c1.tree.content_hash());
}

/// S1b (ADR 0262 amendment 1): S1 with the table looked for as a table, not as a substring.
#[test]
fn s1b_a_case_without_the_table_keeps_its_text_hash_and_mesh() {
    let without = T_S.replace("[mesh.adapt]\n", "");
    assert_ne!(without, T_S);
    let c0 = Case::parse_toml(&without).unwrap();
    let c1 = Case::parse_toml(T_S).unwrap();
    let has_line = |t: &str| t.lines().any(|l| l.trim() == "[mesh.adapt]");
    let json_has = |t: &str| {
        let v: serde_json::Value = serde_json::from_str(t).unwrap();
        v["mesh"].get("adapt").is_some()
    };
    assert!(c0.tree.to_json().pointer("/mesh/adapt").is_none(), "S1b data");
    assert!(!has_line(&c0.to_toml()), "S1b toml");
    assert!(!json_has(&c0.to_json_string()), "S1b json");
    assert_eq!(c0.get("mesh.adapt.cycles").unwrap(), serde_json::json!(2));
    let sim = Simulation::new(&c0).unwrap();
    assert_eq!(sim.geom.mesh.content_hash(), build_mesh(&c0).unwrap().content_hash());
    assert!(sim.adaptive_cycles().is_none());
    assert!(c1.tree.to_json().pointer("/mesh/adapt").is_some(), "S1b data with");
    assert!(has_line(&c1.to_toml()), "S1b toml with");
    println!(
        "S1b: settings hash without {:016x}, with {:016x}",
        c0.tree.content_hash(),
        c1.tree.content_hash()
    );
    assert_ne!(c0.tree.content_hash(), c1.tree.content_hash());
}

// ------------------------------------------------------------------------------------------------ E1, E1n, Z

#[test]
fn e1_the_tree_runs_adr_0242_s_loop_bit_for_bit() {
    let tree = Tree::run(T_S);
    let lib = run_adaptive_simple(ts_lattice(8, 2), cavity_setup, &adaptive_settings(2, MAX_ITER)).unwrap();
    assert_same("E1", &tree, &lib);
    // E1n: the loop adapted
    let cycles = tree.sim.adaptive_cycles().unwrap();
    assert_eq!(cycles.len(), 3);
    assert!(
        cycles.windows(2).all(|w| w[1].cells > w[0].cells),
        "E1n cells {cycles:?}"
    );
    assert!(cycles.iter().all(|c| c.converged), "E1n converged {cycles:?}");
    assert!(
        tree.sim.adaptive_lattice().unwrap().levels().contains(&2),
        "E1n level 2"
    );
    assert!(tree.sim.is_converged() && tree.sim.is_done());
    println!("E1 stop reason: {:?}", tree.sim.stop_reason());
}

#[test]
fn z_zero_cycles_is_the_plain_solve_on_the_base_lattice() {
    let tree = Tree::run(&t_z());
    let geom = Arc::new(FvGeometry::new(Arc::new(ts_lattice(8, 2).to_mesh().unwrap())).unwrap());
    let (u, p, s) = cavity_setup(&geom).unwrap();
    let mut solver = SimpleSolver::new(geom, u, p, s);
    let sum = solver.run(MAX_ITER, TOL);
    let t = tree.solver();
    println!("Z: {} cells, {} iterations", t.geom.n_cells(), sum.iterations);
    assert_eq!(tree.sim.adaptive_cycles().unwrap().len(), 1);
    assert_eq!(tree.sim.adaptive_cycles().unwrap()[0].iterations, sum.iterations);
    assert!(t.u.values == solver.u.values && t.p.values == solver.p.values && t.phi == solver.phi);
}

// ------------------------------------------------------------------------------------------------ E2 (ADR 0247 R3), E2n, E2t, E2r

#[test]
fn e2_the_criteria_of_adr_0247_through_the_tree_bit_for_bit() {
    let ts = Tree::run(T_S);
    let set = adaptive_settings(2, MAX_ITER);

    let tt = Tree::run(&t_t());
    let lib = run_adaptive_simple_target(ts_lattice(8, 2), cavity_setup, &set, 300, |s: &SimpleSolver| {
        velocity_gradient_indicator(s, AXES, true)
    })
    .unwrap();
    assert_same("E2 T-T", &tt, &lib);
    let n = tt.solver().geom.n_cells();
    println!("E2t: T-T ends with {n} cells (target 300)");
    assert!((270..=330).contains(&n), "E2t {n}");

    let te = Tree::run(&t_e());
    let lib = run_adaptive_simple_with(ts_lattice(8, 2), cavity_setup, &set, |s: &SimpleSolver| {
        velocity_edge_length_indicator(s, AXES, 2.0)
    })
    .unwrap();
    assert_same("E2 T-E", &te, &lib);

    let tu = Tree::run(&t_u());
    let mut su = set;
    su.scale_by_length = false;
    su.transfer = RefineTransfer::Injection;
    su.balance = false;
    let lib = run_adaptive_simple(ts_lattice(8, 2), cavity_setup, &su).unwrap();
    assert_same("E2 T-U", &tu, &lib);

    let trc = Tree::run(&t_rc());
    let lib = run_adaptive_simple_marked(
        ts_lattice(8, 2),
        cavity_setup,
        &set,
        |s: &SimpleSolver, amr: &AmrMesh, _k: usize| {
            let ind = velocity_gradient_indicator(s, AXES, true)?;
            let marks = fixed_fraction_marks(&ind, &amr.levels(), amr.max_level(), 0.4)?;
            restrict_marks(&marks, &top_register(s))
        },
    )
    .unwrap();
    assert_same("E2 T-RC", &trc, &lib);

    let trr = Tree::run(&t_rr());
    let lib = run_adaptive_simple_marked(
        ts_lattice(8, 2),
        cavity_setup,
        &adaptive_settings(1, MAX_ITER),
        |s: &SimpleSolver, amr: &AmrMesh, _k: usize| register_marks(&top_register(s), &amr.levels(), amr.max_level()),
    )
    .unwrap();
    assert_same("E2 T-RR", &trr, &lib);
    // E2r: the refined cells of T-RR lie in the box widened by the balance layer
    let levels = trr.sim.adaptive_lattice().unwrap().levels();
    let cc = trr.solver().geom.mesh.cell_centres();
    let low = (0..levels.len())
        .filter(|&c| levels[c] == 1)
        .map(|c| cc[c][1])
        .fold(f64::INFINITY, f64::min);
    println!("E2r: lowest centre of a level-1 cell of T-RR y = {low}");
    assert!(low >= 0.75 - 0.125, "E2r {low}");

    // E2n: each option changes the result
    for (id, t) in [("T-T", &tt), ("T-E", &te), ("T-U", &tu), ("T-RC", &trc), ("T-RR", &trr)] {
        println!(
            "E2n {id}: {} cells against T-S {}",
            t.solver().geom.n_cells(),
            ts.solver().geom.n_cells()
        );
        assert!(differ(t, &ts), "E2n {id} equals T-S");
    }
}

#[test]
fn e2_a_box_2d_channel_and_a_box_3d_cube_bit_for_bit() {
    let tree = Tree::run(T_C);
    let w = |s: &str| (s.to_string(), PatchKind::Wall);
    let lattice = AmrMesh::new(
        [
            axis_coords(16, 2.0, Grading::Uniform).unwrap(),
            axis_coords(4, 0.5, Grading::Uniform).unwrap(),
            axis_coords(1, 2.0 / 16.0, Grading::Uniform).unwrap(),
        ],
        AXES,
        2,
        [
            w("xmin"),
            w("xmax"),
            w("ymin"),
            w("ymax"),
            ("frontAndBack".into(), PatchKind::Empty),
            ("frontAndBack".into(), PatchKind::Empty),
        ],
    )
    .unwrap();
    let setup = |geom: &Arc<FvGeometry>| -> Result<(VectorField, ScalarField, SimpleSettings)> {
        let mut u = VectorField::uniform("U", geom, [0.0; 3]);
        u.set_bc(geom, "xmin", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))?;
        u.set_bc(geom, "xmax", Box::new(ZeroGradient))?;
        u.set_bc(geom, "ymin", Box::new(FixedValue::vector([0.0; 3])))?;
        u.set_bc(geom, "ymax", Box::new(FixedValue::vector([0.0; 3])))?;
        let mut p = ScalarField::uniform("p", geom, 0.0);
        p.set_bc(geom, "xmax", Box::new(FixedValue::scalar(0.0)))?;
        let mut s = SimpleSettings::new(0.01, Coupling::Simple);
        s.convection = ConvScheme::LinearUpwind;
        Ok((u, p, s))
    };
    let lib = run_adaptive_simple(lattice, setup, &adaptive_settings(1, 2000)).unwrap();
    assert_same("E2 T-C", &tree, &lib);

    let tree = Tree::run(T_3);
    let lattice = AmrMesh::new(
        [
            axis_coords(4, 1.0, Grading::Uniform).unwrap(),
            axis_coords(4, 1.0, Grading::Uniform).unwrap(),
            axis_coords(4, 1.0, Grading::Uniform).unwrap(),
        ],
        [true; 3],
        1,
        [w("xmin"), w("xmax"), w("ymin"), w("ymax"), w("zmin"), w("zmax")],
    )
    .unwrap();
    let setup = |geom: &Arc<FvGeometry>| -> Result<(VectorField, ScalarField, SimpleSettings)> {
        let mut u = VectorField::uniform("U", geom, [0.0; 3]);
        for n in ["xmin", "xmax", "ymin", "zmin", "zmax"] {
            u.set_bc(geom, n, Box::new(FixedValue::vector([0.0; 3])))?;
        }
        u.set_bc(geom, "ymax", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))?;
        Ok((u, ScalarField::uniform("p", geom, 0.0), cavity_settings()))
    };
    let mut set = adaptive_settings(1, 20);
    set.refine_fraction = 0.25;
    let lib = run_adaptive_simple(lattice, setup, &set).unwrap();
    assert_same("E2 T-3", &tree, &lib);
}

// ------------------------------------------------------------------------------------------------ L (located errors, D5)

/// The paths of the diagnostics of a case text: the parse and the static checks, else the build.
fn paths(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.into_iter().map(|x| x.path).collect(),
        Ok(c) => match Simulation::new(&c) {
            Err(d) => d.into_iter().map(|x| x.path).collect(),
            Ok(_) => Vec::new(),
        },
    }
}

#[test]
fn l_located_errors() {
    let solver = |base: &str, line: &str| format!("{base}{line}\n");
    let mesh_key = |base: &str, after: &str, line: &str| {
        assert!(base.contains(after));
        base.replacen(after, &format!("{after}{line}\n"), 1)
    };
    let field_register = "\n[registers.fast]\ntype = \"field\"\nfield = \"velocity_magnitude\"\nabove = 0.5\n";
    let cases: Vec<(&str, String, &str)> = vec![
        (
            "V1",
            adapt_with(T_S, "indicator = \"hessian\"\n"),
            "mesh.adapt.indicator",
        ),
        ("V1", adapt_with(T_S, "cycle = 2\n"), "mesh.adapt.cycle"),
        (
            "V2",
            adapt_with(T_S, "refine_fraction = 1.5\n"),
            "mesh.adapt.refine_fraction",
        ),
        ("V2", adapt_with(T_S, "max_level = 11\n"), "mesh.adapt.max_level"),
        ("V2", adapt_with(T_S, "cycles = -1\n"), "mesh.adapt.cycles"),
        ("V2", adapt_with(T_S, "tolerance = 0.0\n"), "mesh.adapt.tolerance"),
        (
            "V3",
            T_S.replace("generator = \"cavity_2d\"", "generator = \"channel_2d\""),
            "mesh.generator",
        ),
        (
            "V3",
            mesh_key(T_S, "ly = 1.0\n", "source = \"file\"\nfile = \"missing.msh\""),
            "mesh.source",
        ),
        ("V4", mesh_key(T_C, "ly = 0.5\n", "grading = 2.0"), "mesh.grading"),
        (
            "V4",
            mesh_key(T_C, "ly = 0.5\n", "periodic_x = true"),
            "mesh.periodic_x",
        ),
        ("V4", mesh_key(T_C, "ly = 0.5\n", "wall_layers = 3"), "mesh.wall_layers"),
        (
            "V4",
            format!("{T_3}\n[mesh.box_3d]\nsection = \"triangles\"\n"),
            "mesh.box_3d.section",
        ),
        (
            "V4",
            format!("{T_S}\n[mesh.transform]\nrotate_z_deg = 5.0\n"),
            "mesh.transform.rotate_z_deg",
        ),
        (
            "V5",
            T_S.replace("algorithm = \"simplec\"", "algorithm = \"piso\""),
            "solver.algorithm",
        ),
        (
            "V6",
            adapt_with(T_S, "marking = \"target_count\"\n"),
            "mesh.adapt.target_cells",
        ),
        ("V7", adapt_with(T_S, "register = \"nowhere\"\n"), "mesh.adapt.register"),
        (
            "V7",
            adapt_with(T_S, "register_use = \"refine\"\n"),
            "mesh.adapt.register_use",
        ),
        (
            "V7",
            format!("{}{field_register}", adapt_with(T_S, "register = \"fast\"\n")),
            "mesh.adapt.register",
        ),
        (
            "V8",
            format!("{T_S}\n[turbulence]\nmodel = \"k_epsilon\"\n"),
            "turbulence.model",
        ),
        ("V8", format!("{T_S}\n[energy]\nenabled = true\n"), "energy.enabled"),
        (
            "V8",
            mesh_key(T_S, "nu = 0.01\n", "body_force = [0.0, -1.0, 0.0]"),
            "physics.body_force",
        ),
        (
            "V8",
            solver(T_S, "linear_upwind_limiter = \"barth_jespersen\""),
            "solver.linear_upwind_limiter",
        ),
        (
            "V8",
            format!("{T_S}\n[convergence.c]\nmonitor = \"res_u\"\ntype = \"below\"\ntolerance = 1e-6\n"),
            "convergence.c",
        ),
        (
            "V8",
            mesh_key(
                T_S,
                "velocity_value = [1.0, 0.0, 0.0]\n",
                "velocity_expression_x = \"1 [m/s]\"",
            ),
            "boundaries.lid.velocity_expression_x",
        ),
        (
            "V8",
            format!("{T_S}\n[initial]\nmethod = \"potential\"\n"),
            "initial.method",
        ),
    ];
    let mut bad = Vec::new();
    for (id, text, want) in &cases {
        let got = paths(text);
        println!("L {id} {want}: {got:?}");
        if !got.iter().any(|p| p == want) {
            bad.push(format!("{id} {want}: {got:?}"));
        }
    }
    let file = std::fs::read_to_string(repo().join("cases/cavity/adapt_re100.toml")).unwrap();
    for (id, text) in [
        ("T-S", T_S.to_string()),
        ("T-T", t_t()),
        ("T-E", t_e()),
        ("T-U", t_u()),
        ("T-RC", t_rc()),
        ("T-RR", t_rr()),
        ("T-Z", t_z()),
        ("T-C", T_C.to_string()),
        ("T-3", T_3.to_string()),
        ("R1 file", file),
    ] {
        let got = paths(&text);
        println!("L control {id}: {got:?}");
        if !got.is_empty() {
            bad.push(format!("control {id}: {got:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// ------------------------------------------------------------------------------------------------ R1 (heavy record)

/// ADR 0242's point sampler (copied from crates/rcfd-fv/tests/adapt_cavity_f102.rs): mean over the cells whose closed box
/// contains the point of `U_c + G_c . (x - x_c)`.
struct Sampler {
    lo: Vec<Vec3>,
    hi: Vec<Vec3>,
    cc: Vec<Vec3>,
    u: Vec<Vec3>,
    g: [Vec<Vec3>; 3],
}

impl Sampler {
    fn new(s: &SimpleSolver) -> Sampler {
        let geom = &s.geom;
        let m = &geom.mesh;
        let pts = m.points();
        let mut lo = Vec::with_capacity(m.n_cells());
        let mut hi = Vec::with_capacity(m.n_cells());
        for c in 0..m.n_cells() {
            let mut l = [f64::INFINITY; 3];
            let mut h = [f64::NEG_INFINITY; 3];
            for &f in m.cell_faces(c) {
                for &p in m.face_points(f) {
                    for a in 0..3 {
                        l[a] = l[a].min(pts[p][a]);
                        h[a] = h[a].max(pts[p][a]);
                    }
                }
            }
            lo.push(l);
            hi.push(h);
        }
        let bu = s.u.boundary_values(geom);
        let g = [0, 1, 2].map(|k| {
            let cells: Vec<f64> = s.u.values.iter().map(|v| v[k]).collect();
            let bv: Vec<f64> = bu.iter().map(|v| v[k]).collect();
            least_squares(geom, &cells, &bv)
        });
        Sampler {
            lo,
            hi,
            cc: m.cell_centres().to_vec(),
            u: s.u.values.clone(),
            g,
        }
    }

    fn at(&self, x: Vec3) -> Vec3 {
        let mut sum = [0.0; 3];
        let mut n = 0.0;
        for c in 0..self.cc.len() {
            if (0..2).all(|a| x[a] >= self.lo[c][a] && x[a] <= self.hi[c][a]) {
                for k in 0..3 {
                    let d = [x[0] - self.cc[c][0], x[1] - self.cc[c][1], 0.0];
                    sum[k] += self.u[c][k] + self.g[k][c][0] * d[0] + self.g[k][c][1] * d[1];
                }
                n += 1.0;
            }
        }
        assert!(n > 0.0, "no cell contains {x:?}");
        [sum[0] / n, sum[1] / n, sum[2] / n]
    }

    /// The dense set: u on x = 0.5 at y = j/100, then v on y = 0.5 at x = j/100 (ADR 0242's sampler uses z = 0.05; here
    /// z is the slab's middle, which the box test does not look at).
    fn dense(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(198);
        for j in 1..100 {
            out.push(self.at([0.5, j as f64 / 100.0, 0.0])[0]);
        }
        for j in 1..100 {
            out.push(self.at([j as f64 / 100.0, 0.5, 0.0])[1]);
        }
        out
    }

    fn ghia(&self) -> f64 {
        let read = |name: &str| -> Vec<Vec<f64>> {
            let path = repo().join("cases/cavity/reference").join(name);
            std::fs::read_to_string(&path)
                .unwrap()
                .lines()
                .skip(1)
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.split(',').map(|v| v.trim().parse().unwrap()).collect())
                .collect()
        };
        let mut worst = 0.0f64;
        let mut rows = 0;
        for r in read("ghia1982_u_centerline.csv")
            .iter()
            .filter(|r| r[0] > 0.0 && r[0] < 1.0)
        {
            worst = worst.max((self.at([0.5, r[0], 0.0])[0] - r[1]).abs());
            rows += 1;
        }
        for r in read("ghia1982_v_centerline_Re100.csv")
            .iter()
            .filter(|r| r[0] > 0.0 && r[0] < 1.0)
        {
            worst = worst.max((self.at([r[0], 0.5, 0.0])[1] - r[1]).abs());
            rows += 1;
        }
        assert_eq!(rows, 30);
        worst
    }
}

fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

#[test]
#[ignore = "heavy (ADR 0262 R1: the 24 x 24 cavity of the case file, ADR 0242's A and the 96 x 96 F): run in release with --include-ignored"]
fn r1_the_case_file_reproduces_adr_0242_s_cavity() {
    let t0 = std::time::Instant::now();
    let case = Case::load(repo().join("cases/cavity/adapt_re100.toml")).unwrap_or_else(|d| panic!("{d:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|d| panic!("{d:?}"));
    sim.run(10);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let tree = Tree { sim };
    println!(
        "R1 tree run {:.1} s: {:?}",
        t0.elapsed().as_secs_f64(),
        tree.sim.stop_reason()
    );
    // R1a: the library on the tree's lattice
    let lib = run_adaptive_simple(ts_lattice(24, 2), cavity_setup, &adaptive_settings(2, MAX_ITER)).unwrap();
    assert_same("R1a", &tree, &lib);
    // R1b: ADR 0242's own runs (slab 0.1)
    let a = run_adaptive_simple(
        cavity_lattice(24, 2, 0.1),
        cavity_setup,
        &adaptive_settings(2, MAX_ITER),
    )
    .unwrap();
    let t1 = std::time::Instant::now();
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_lattice(96, 0, 0.1).to_mesh().unwrap())).unwrap());
    let (u, p, s) = cavity_setup(&geom).unwrap();
    let mut f = SimpleSolver::new(geom, u, p, s);
    let fsum = f.run(MAX_ITER, TOL);
    println!(
        "R1 F: {} cells, {} iterations, converged {}, {:.1} s",
        f.geom.n_cells(),
        fsum.iterations,
        fsum.converged,
        t1.elapsed().as_secs_f64()
    );
    let (st, sa, sf) = (Sampler::new(tree.solver()), Sampler::new(&a.solver), Sampler::new(&f));
    let (dt, da, df) = (st.dense(), sa.dense(), sf.dense());
    let d_tree = max_diff(&dt, &df);
    let d_a = max_diff(&da, &df);
    let samples = max_diff(&dt, &da);
    let g_tree = st.ghia();
    let cycles = tree.sim.adaptive_cycles().unwrap();
    println!(
        "R1b: tree cycles {:?}; ADR 0242 A cycles {:?}; N_A tree {} A {}; dense tree - A {samples:.3e}; D tree {d_tree:.6} \
         A {d_a:.6}; G tree {g_tree:.6} A {:.6} F {:.6}; total {:.1} s",
        cycles.iter().map(|c| (c.cells, c.iterations, c.converged)).collect::<Vec<_>>(),
        a.cycles.iter().map(|c| (c.cells, c.iterations, c.converged)).collect::<Vec<_>>(),
        tree.solver().geom.n_cells(),
        a.solver.geom.n_cells(),
        sa.ghia(),
        sf.ghia(),
        t0.elapsed().as_secs_f64()
    );
    let cells: Vec<usize> = cycles.iter().map(|c| c.cells).collect();
    assert_eq!(cells, vec![576, 1269, 2799], "R1b cycle cells");
    assert_eq!(tree.solver().geom.n_cells(), 2799, "R1b N_A");
    assert!(cycles.iter().all(|c| c.converged), "R1b converged");
    assert!(fsum.converged, "R1b F converged");
    assert!(samples <= 1e-5, "R1b samples {samples}");
    assert!(
        (0.00230..=0.00232).contains(&d_tree) && d_tree <= 0.01,
        "R1b D {d_tree}"
    );
    assert!(
        (0.00915..=0.00917).contains(&g_tree) && g_tree <= 0.02,
        "R1b G {g_tree}"
    );
}
