//! Registers through the settings tree (ADR 0011 section 3, criteria R6, R7, R10, R11) on a 20 x 20 box_2d mesh
//! (the settings layer has no 3D box generator; the library tests in rcfd-post use the 20^3 cube). G1 and G2 (ADR 0095) cover
//! `type = "field"` registers.

use rcfd_api::case::Case;
use rcfd_api::registers;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_api::sources::Sources;
use rcfd_fv::{FvGeometry, VectorField};
use std::sync::Arc;

fn case_text(extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 20
lx = 1.0
ly = 1.0
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
{extra}
"#
    )
}

const REG_BOX: &str = r#"
[registers.core]
type = "box"
box_min = [0.25, 0.25, -1.0]
box_max = [0.75, 0.75, 1.0]
"#;

fn parse(extra: &str) -> Result<Case, String> {
    Case::parse_toml(&case_text(extra)).map_err(|d| d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))
}

fn err_of(extra: &str) -> String {
    match parse(extra) {
        Err(e) => e,
        Ok(c) => match Simulation::new(&c) {
            Err(d) => d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
            Ok(_) => "no error".into(),
        },
    }
}

#[test]
fn registers_are_in_the_schema_and_validate() {
    let c = parse(REG_BOX).expect("valid registers");
    assert_eq!(registers::names(&c), vec!["core".to_string()]);
    let schema = rcfd_api::schema::case_schema();
    let text = serde_json::to_string(&schema.to_json_schema()).unwrap();
    assert!(text.contains("\"registers\"") && text.contains("boundary_adjacent"));
}

#[test]
fn r7_porous_sink_is_nonzero_exactly_on_the_register() {
    let c = parse(&format!(
        "{REG_BOX}\n[sources.bed]\ntype = \"porous\"\nzone = \"register\"\nregister = \"core\"\nmodel = \"darcy_forchheimer\"
permeability = 0.01\n"
    ))
    .unwrap();
    let mesh = Arc::new(build_mesh(&c).unwrap());
    let geom = FvGeometry::new(mesh.clone()).unwrap();
    let src = Sources::from_case(&c, &geom, None).unwrap().expect("sources");
    let u = VectorField::uniform("U", &geom, [0.1, 0.0, 0.0]);
    let (_, sink) = src.evaluate(&geom, &u);
    let sink = sink.expect("sink");
    let reg = registers::build(&c, "core", &mesh, None).unwrap();
    assert_eq!(reg.count(), 100);
    let nonzero = sink.iter().filter(|&&s| s != 0.0).count();
    println!("R7 register cells {} non-zero sink cells {}", reg.count(), nonzero);
    assert_eq!(nonzero, reg.count());
    for (c, (&s, &m)) in sink.iter().zip(reg.mask()).enumerate() {
        if m {
            assert!(s != 0.0, "cell {c}");
        } else {
            assert_eq!(s.to_bits(), 0.0f64.to_bits(), "cell {c}");
        }
    }
}

#[test]
fn r6_initial_patches_change_only_the_register_cells() {
    let base = Simulation::new(&parse(REG_BOX).unwrap()).unwrap().field("p").unwrap();
    let c = parse(&format!(
        "{REG_BOX}\n[initial.patches.hot]\nregister = \"core\"\nvariable = \"pressure\"\nvalue = [7.5]\n[initial.patches.fast]\nregister = \"core\"\nvariable = \"velocity\"\nvalue = [1.0, 2.0, 3.0]\n"
    ))
    .unwrap();
    let sim = Simulation::new(&c).unwrap();
    let p = sim.field("p").unwrap();
    let mesh = build_mesh(&c).unwrap();
    let reg = registers::build(&c, "core", &mesh, None).unwrap();
    let changed = p.iter().zip(&base).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    println!("R6 changed cells {changed} of {}", p.len());
    assert_eq!(changed, 100);
    for (i, ((a, b), &m)) in p.iter().zip(&base).zip(reg.mask()).enumerate() {
        if m {
            assert_eq!(*a, 7.5, "cell {i}");
        } else {
            assert_eq!(a.to_bits(), b.to_bits(), "cell {i}");
        }
    }
    let ux = sim.field("ux").unwrap();
    assert_eq!(ux.iter().filter(|&&v| v == 1.0).count(), 100);
    assert_eq!(ux.iter().filter(|&&v| v == 0.0).count(), 300);
}

#[test]
fn expression_boundary_combine_and_grow_registers_build() {
    let c = parse(
        r#"
[registers.disc]
type = "expression"
expression = "(x - 0.5 [m])*(x - 0.5 [m]) + (y - 0.5 [m])*(y - 0.5 [m])"
below = 0.09
[registers.left]
type = "boundary_adjacent"
patches = ["xmin"]
layers = 2
[registers.both]
type = "combine"
operation = "union"
operands = ["disc", "left"]
[registers.only_disc]
type = "combine"
operation = "difference"
operands = ["disc", "left"]
[registers.ring]
type = "combine"
operation = "difference"
operands = ["both", "only_disc"]
grow = 1
"#,
    )
    .unwrap();
    let mesh = build_mesh(&c).unwrap();
    let all = registers::build_all(&c, &mesh, None).unwrap();
    let direct =
        rcfd_post::registers::Register::from_centre_fn(&mesh, |p| (p[0] - 0.5).powi(2) + (p[1] - 0.5).powi(2) <= 0.09);
    assert_eq!(all["disc"], direct);
    assert_eq!(all["left"].count(), 40);
    assert_eq!(all["both"], all["disc"].union(&all["left"]).unwrap());
    assert_eq!(all["only_disc"], all["disc"].difference(&all["left"]).unwrap());
    let ring = all["both"].difference(&all["only_disc"]).unwrap().grow(&mesh, 1);
    assert_eq!(all["ring"], ring);
}

#[test]
fn r10_errors_are_located_by_setting_path() {
    let src = |reg: &str| {
        format!("{REG_BOX}\n[sources.bed]\ntype = \"porous\"\nzone = \"register\"\nregister = \"{reg}\"\nmodel = \"darcy_forchheimer\"
permeability = 0.01\n")
    };
    let e = err_of(&src("nope"));
    assert!(
        e.contains("sources.bed.register") && e.contains("nope") && e.contains("core"),
        "{e}"
    );
    let e = err_of(&format!(
        "{REG_BOX}\n[initial.patches.a]\nregister = \"nope\"\nvariable = \"pressure\"\nvalue = [1.0]\n"
    ));
    assert!(e.contains("initial.patches.a.register") && e.contains("nope"), "{e}");
    let e = err_of(&format!(
        "{REG_BOX}\n[initial.patches.a]\nregister = \"core\"\nvariable = \"velocity\"\nvalue = [1.0]\n"
    ));
    assert!(e.contains("initial.patches.a.value"), "{e}");
    let e = err_of("[registers.c]\ntype = \"combine\"\noperation = \"union\"\noperands = [\"zzz\"]\n");
    assert!(e.contains("registers.c.operands") && e.contains("zzz"), "{e}");
    let e = err_of("[registers.a]\ntype = \"combine\"\noperands = [\"b\"]\n[registers.b]\ntype = \"combine\"\noperands = [\"a\"]\n");
    assert!(e.contains("cycle"), "{e}");
    let e = err_of("[registers.b]\ntype = \"box\"\nbox_min = [0.5, 0.0, 0.0]\nbox_max = [0.5, 1.0, 1.0]\n");
    assert!(e.contains("registers.b.box_max"), "{e}");
    let e = err_of("[registers.x]\ntype = \"expression\"\nexpression = \"x +\"\n");
    assert!(e.contains("registers.x.expression"), "{e}");
    let e = err_of("[registers.x]\ntype = \"expression\"\nexpression = \"1.0/(x - 0.025 [m])\"\n");
    assert!(e.contains("registers.x.expression") && e.contains("not finite"), "{e}");
    let e = err_of("[registers.x]\ntype = \"boundary_adjacent\"\npatches = [\"xmin\"]\nlayers = 0\n");
    assert!(e.contains("registers.x.layers"), "{e}");
    let e = err_of("[registers.x]\ntype = \"boundary_adjacent\"\npatches = [\"nope\"]\n[sources.s]\ntype = \"porous\"\nzone = \"register\"\nregister = \"x\"\nmodel = \"darcy_forchheimer\"
permeability = 0.01\n");
    assert!(e.contains("registers.x.patches") && e.contains("nope"), "{e}");
    // a register that is empty where a source needs cells
    let e = err_of("[registers.out]\ntype = \"box\"\nbox_min = [5.0, 5.0, -1.0]\nbox_max = [6.0, 6.0, 1.0]\n[sources.s]\ntype = \"porous\"\nzone = \"register\"\nregister = \"out\"\nmodel = \"darcy_forchheimer\"
permeability = 0.01\n");
    assert!(e.contains("sources.s.zone") && e.contains("no cells"), "{e}");
}

// ---- ADR 0095 (wave 39D): `type = "field"` registers, G1 and G2 ----

const FIELD_INIT: &str = r#"
[initial]
velocity_expression_x = "4 [m/s] * (y / 1 [m]) * (1 - y / 1 [m])"
"#;

const REG_FIELD: &str = r#"
[registers.fast]
type = "field"
field = "velocity_magnitude"
above = 0.75
"#;

const REG_BAND_BOX: &str = r#"
[registers.fast]
type = "box"
box_min = [-1.0, 0.25, -1.0]
box_max = [2.0, 0.75, 1.0]
"#;

const PATCH_HOT: &str = "[initial.patches.hot]\nregister = \"fast\"\nvariable = \"pressure\"\nvalue = [7.5]\n";
const BED_FAST: &str = "[sources.bed]\ntype = \"porous\"\nzone = \"register\"\nregister = \"fast\"\nmodel = \"darcy_forchheimer\"\npermeability = 0.01\n";

fn run_fields(extra: &str, n: usize) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut sim = Simulation::new(&parse(extra).unwrap()).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..n {
        sim.step();
    }
    (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("p").unwrap(),
    )
}

#[test]
fn g1_field_threshold_register_is_patched_and_restricts_a_source() {
    let c = parse(&format!("{FIELD_INIT}{REG_FIELD}{PATCH_HOT}{BED_FAST}")).unwrap();
    let sim = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    let mesh = Arc::new(build_mesh(&c).unwrap());
    let n = mesh.n_cells();
    // initial fields: velocity is not patched, so the simulation's velocity is the unpatched initial velocity
    let (ux, uy) = (sim.field("ux").unwrap(), sim.field("uy").unwrap());
    let uz = sim.field("uz").unwrap_or_else(|_| vec![0.0; n]);
    let base = Simulation::new(&parse(&format!("{FIELD_INIT}{REG_FIELD}")).unwrap()).unwrap();
    let p0 = base.field("p").unwrap();
    let fields = registers::InitialFields {
        u: (0..n).map(|i| [ux[i], uy[i], uz[i]]).collect(),
        p: p0.clone(),
        t: vec![293.15; n],
    };
    let reg = registers::build(&c, "fast", &mesh, Some(&fields)).unwrap();
    // G1a: analytic count
    println!("G1a field register cells {} (limit exactly 200)", reg.count());
    assert_eq!(reg.count(), 200);
    // G1b: equal to the library threshold of |u|
    let mag: Vec<f64> = fields.u.iter().map(|v| rcfd_core::vec3::norm(*v)).collect();
    let direct = rcfd_post::registers::Register::threshold(&mesh, &mag, 0.75, f64::INFINITY).unwrap();
    assert_eq!(reg, direct, "G1b");
    // G1c: the patch changes exactly the register cells
    let p = sim.field("p").unwrap();
    let changed = p.iter().zip(&p0).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    println!("G1c patched cells {changed} (limit exactly 200)");
    for (i, ((a, b), &m)) in p.iter().zip(&p0).zip(reg.mask()).enumerate() {
        if m {
            assert_eq!(*a, 7.5, "cell {i}");
        } else {
            assert_eq!(a.to_bits(), b.to_bits(), "cell {i}");
        }
    }
    assert_eq!(changed, 200);
    // G1d: the porous sink is non-zero on exactly the register cells
    let geom = FvGeometry::new(mesh.clone()).unwrap();
    let src = Sources::from_case(&c, &geom, Some(&fields)).unwrap().expect("sources");
    let u = VectorField::uniform("U", &geom, [0.1, 0.0, 0.0]);
    let sink = src.evaluate(&geom, &u).1.expect("sink");
    let nonzero = sink.iter().filter(|&&s| s != 0.0).count();
    println!("G1d non-zero sink cells {nonzero} (limit exactly 200)");
    assert_eq!(nonzero, 200);
    for (i, (&s, &m)) in sink.iter().zip(reg.mask()).enumerate() {
        if m {
            assert!(s != 0.0, "cell {i}");
        } else {
            assert_eq!(s.to_bits(), 0.0f64.to_bits(), "cell {i}");
        }
    }
    // G1e: end to end, the field register drives the run exactly like a box register on the same cells
    let a = run_fields(&format!("{FIELD_INIT}{REG_FIELD}{PATCH_HOT}{BED_FAST}"), 5);
    let b = run_fields(&format!("{FIELD_INIT}{REG_BAND_BOX}{PATCH_HOT}{BED_FAST}"), 5);
    let no_src = run_fields(&format!("{FIELD_INIT}{REG_FIELD}{PATCH_HOT}"), 5);
    let bits = |x: &[f64], y: &[f64]| x.iter().zip(y).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    let diff =
        a.0.iter()
            .zip(&no_src.0)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0_f64, f64::max);
    println!(
        "G1e differing cells field vs box register: ux {} uy {} p {} (limit 0); max |ux| change by the source {diff:.3e} (limit > 1e-6)",
        bits(&a.0, &b.0),
        bits(&a.1, &b.1),
        bits(&a.2, &b.2)
    );
    assert_eq!((bits(&a.0, &b.0), bits(&a.1, &b.1), bits(&a.2, &b.2)), (0, 0, 0));
    assert!(diff > 1e-6, "{diff}");
}

#[test]
fn g2_field_register_errors_are_located() {
    // a consumer without the initial fields
    let e = err_of(&format!(
        "{FIELD_INIT}{REG_FIELD}[energy]\nenabled = true\n[zone_sources.q]\ntarget = \"temperature\"\nvalue = 1.0\nzone = \"register\"\nregister = \"fast\"\n"
    ));
    assert!(e.contains("registers.fast.type") && e.contains("initial fields"), "{e}");
    // an initialisation method that runs after the registers
    let e = err_of(&format!("{FIELD_INIT}{REG_FIELD}").replace("[initial]\n", "[initial]\nmethod = \"potential\"\n"));
    assert!(e.contains("registers.fast.type") && e.contains("initial.method"), "{e}");
    let e = err_of(&format!("{FIELD_INIT}{REG_FIELD}below = 0.5\n"));
    assert!(e.contains("registers.fast.above"), "{e}");
}
