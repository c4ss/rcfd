//! Registers through the settings tree (ADR 0011 section 3, criteria R6, R7, R10, R11) on a 20 x 20 box_2d mesh
//! (the settings layer has no 3D box generator; the library tests in rcfd-post use the 20^3 cube).

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
    let src = Sources::from_case(&c, &geom).unwrap().expect("sources");
    let u = VectorField::uniform("U", &geom, [0.1, 0.0, 0.0]);
    let (_, sink) = src.evaluate(&geom, &u);
    let sink = sink.expect("sink");
    let reg = registers::build(&c, "core", &mesh).unwrap();
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
    let reg = registers::build(&c, "core", &mesh).unwrap();
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
    let all = registers::build_all(&c, &mesh).unwrap();
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
