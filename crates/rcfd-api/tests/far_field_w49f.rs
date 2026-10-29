//! ADR 0169 (job 49F): a `velocity = "freestream"` far field takes the `freestream` turbulence role by default with
//! k-omega SST and k-epsilon too (D1); without an inlet specification its k and omega / epsilon come from
//! `turbulence.initial_intensity` and `initial_viscosity_ratio` (D2). Limits F1-F6 of ADR 0169, registered before this
//! file was written. F7 is the `turb` lib test `freestream_far_field_default_is_sa_sst_and_k_epsilon`.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_api::turb;
use std::path::PathBuf;

/// ADR 0166's plate (Re = 5e6 per m, wall cell for y+ about 40).
const PLATE: &str = r#"
title = "plate, y+ 40"
[mesh]
generator = "box_2d"
nx = 30
ny = 24
lx = 2.0
ly = 1.0
wall_layers = 10
layer_walls = "lower"
layer_growth = 1.2
target_y_plus = 40.0
layer_reference_velocity = 1.0
layer_reference_length = 1.0
[physics]
nu = 2e-7
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "symmetry"
[solver]
algorithm = "simplec"
max_iterations = 100000
"#;

const NU: f64 = 2e-7;

/// The plate with free-stream inflow (`xmin`) and outflow (`xmax`) patches and the given extra lines on both (ADR 0166
/// T5's set-up), as text.
fn freestream_text(model: &str, lines: &str) -> String {
    let text = PLATE
        .replace(
            "[boundaries.xmin]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]",
            &format!("[boundaries.xmin]\nvelocity = \"freestream\"\nvelocity_value = [1.0, 0.0, 0.0]\npressure = \"freestream\"\n{lines}"),
        )
        .replace(
            "[boundaries.xmax]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"",
            &format!("[boundaries.xmax]\nvelocity = \"freestream\"\nvelocity_value = [1.0, 0.0, 0.0]\npressure = \"freestream\"\n{lines}"),
        )
        + &format!("[turbulence]\nmodel = \"{model}\"\n");
    assert_eq!(text.matches("velocity = \"freestream\"").count(), 2);
    text
}

/// [`freestream_text`] parsed; it must validate.
fn freestream_plate(model: &str, lines: &str) -> Case {
    let c = Case::parse_toml(&freestream_text(model, lines)).unwrap_or_else(|e| panic!("{e:?}"));
    let errs = c.semantic_errors();
    assert!(errs.is_empty(), "{model} {lines:?}: {errs:?}");
    c
}

fn run(c: &Case, n: usize) -> Simulation {
    let mut s = Simulation::new(c).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..n {
        s.step();
    }
    s
}

/// Bits of the named cell fields and of the face flux of a run.
fn bits(s: &Simulation, fields: &[&str]) -> Vec<u64> {
    let mut out = Vec::new();
    for f in fields {
        out.extend(
            s.field(f)
                .unwrap_or_else(|e| panic!("{f}: {e}"))
                .iter()
                .map(|x| x.to_bits()),
        );
    }
    out.extend(s.flux().iter().map(|x| x.to_bits()));
    out
}

type FacePairs = Vec<(f64, f64)>;

/// Far-field faces of `name`: (inflow faces `xmin`, outflow faces `xmax`) as (face value, owner cell value) pairs.
fn far_field_faces(s: &Simulation, name: &str) -> (FacePairs, FacePairs) {
    let g = &s.geom;
    let nb = s.boundary_field(name).unwrap_or_else(|e| panic!("{name}: {e}"));
    let cells = s.field(name).unwrap();
    let own = g.mesh.owner();
    let pairs = |p: &str| -> FacePairs {
        g.mesh
            .patch(p)
            .unwrap()
            .faces()
            .map(|f| (nb[f - g.n_internal], cells[own[f]]))
            .collect()
    };
    (pairs("xmin"), pairs("xmax"))
}

/// The second scalar of a two-equation model.
fn second(model: &str) -> &'static str {
    if model == "k_omega_sst" {
        "omega"
    } else {
        "epsilon"
    }
}

/// ADR 0169 D2's values for the plate: I = 0.05, nut/nu = 10, U = 1, nu = 2e-7, Cmu = 0.09 (beta* for SST, the
/// standard k-epsilon c_mu), the clamps of the inlet helpers.
fn expected(model: &str) -> (f64, f64) {
    let k = turb::k_from_intensity(0.05, 1.0).max(1e-10);
    let e = turb::epsilon_from_viscosity_ratio(0.09, k, NU, 10.0).max(1e-10);
    if model == "k_omega_sst" {
        (k, (e / (0.09 * k)).max(1e-10))
    } else {
        (k, e)
    }
}

/// Inflow faces hold `want` bit for bit; outflow faces equal their owner cells; all finite.
fn assert_freestream_faces(s: &Simulation, model: &str, want: (f64, f64)) {
    for (name, v) in [("k", want.0), (second(model), want.1)] {
        let (inflow, outflow) = far_field_faces(s, name);
        assert!(!inflow.is_empty() && !outflow.is_empty());
        for (face, _) in &inflow {
            assert_eq!(
                face.to_bits(),
                v.to_bits(),
                "{model} {name}: inflow face {face}, expected {v}"
            );
        }
        for (face, cell) in &outflow {
            assert!(face.is_finite(), "{model} {name}: {face}");
            assert_eq!(
                face.to_bits(),
                cell.to_bits(),
                "{model} {name}: outflow face {face} owner {cell}"
            );
        }
    }
}

/// F1 / F2: SST and k-epsilon: the far field without a `turbulence` line validates and is the explicit `freestream`
/// role bit for bit; control: the explicit `zero_gradient` (the old default) differs. The inflow faces hold the D2
/// values, the outflow faces the cell values.
#[test]
fn f1_f2_sst_and_k_epsilon_far_field_default_is_freestream() {
    for model in ["k_omega_sst", "k_epsilon"] {
        let fields = ["ux", "uy", "p", "k", second(model)];
        let auto = run(&freestream_plate(model, ""), 30);
        let explicit = run(&freestream_plate(model, "turbulence = \"freestream\""), 30);
        let old = run(&freestream_plate(model, "turbulence = \"zero_gradient\""), 30);
        let a = bits(&auto, &fields);
        assert_eq!(a, bits(&explicit, &fields), "{model}: auto against explicit freestream");
        assert_ne!(a, bits(&old, &fields), "{model}: control, explicit zero_gradient");
        let want = expected(model);
        assert_freestream_faces(&auto, model, want);
        println!(
            "F1/F2 {model}: auto = freestream bit for bit, zero_gradient differs; inflow k {} {} {}",
            want.0,
            second(model),
            want.1
        );
    }
}

/// F3: an explicit specification on the far field is used as given (here a turbulence length).
#[test]
fn f3_an_explicit_far_field_specification_is_honoured() {
    let c = freestream_plate("k_omega_sst", "turbulence_length = 0.1");
    assert!(turb::freestream_role(&c, "xmin") && !turb::freestream_fallback(&c, "xmin"));
    let s = run(&c, 30);
    let (k, w) = turb::inlet_values_sst(&c, "boundaries.xmin", NU, 1.0);
    let (kf, wf) = expected("k_omega_sst");
    assert_eq!(k.to_bits(), kf.to_bits(), "the same intensity gives the same k");
    assert_ne!(w.to_bits(), wf.to_bits(), "the length gives another omega");
    let (inflow, _) = far_field_faces(&s, "omega");
    assert!(inflow.iter().all(|(f, _)| f.to_bits() == w.to_bits()), "{inflow:?} {w}");
    println!("F3: omega from the length {w}, from the fallback {wf}");
}

fn has_error(c: &Case, path: &str) -> bool {
    c.semantic_errors().iter().any(|d| d.path == path)
}

/// F4: validation. A fixed-velocity inlet and an explicit `inlet` role keep the `turbulence_length` error; an explicit
/// `freestream` without a specification is valid; RSM keeps its inlet error.
#[test]
fn f4_validation_of_the_inlet_specification() {
    for model in ["k_omega_sst", "k_epsilon"] {
        let mut c = Case::parse_toml(PLATE).unwrap();
        c.set("turbulence.model", serde_json::json!(model)).unwrap();
        assert!(
            has_error(&c, "boundaries.xmin.turbulence_length"),
            "{model}: fixed inlet"
        );
        // parsed with Spalart-Allmaras (valid), then switched: `parse_toml` refuses an invalid case as a whole
        let mut c = Case::parse_toml(&freestream_text("spalart_allmaras", "turbulence = \"inlet\"")).unwrap();
        c.set("turbulence.model", serde_json::json!(model)).unwrap();
        assert!(
            has_error(&c, "boundaries.xmin.turbulence_length"),
            "{model}: explicit inlet role"
        );
        // explicit freestream without a specification: no error (checked in freestream_plate)
        freestream_plate(model, "turbulence = \"freestream\"");
    }
    let mut c = Case::parse_toml(&freestream_text("spalart_allmaras", "turbulence = \"freestream\"")).unwrap();
    c.set("turbulence.model", serde_json::json!("rsm")).unwrap();
    assert!(
        has_error(&c, "boundaries.xmin.turbulence"),
        "rsm: {:?}",
        c.semantic_errors()
    );
    // under auto RSM keeps the zero-gradient far field
    let mut c = Case::parse_toml(&freestream_text("spalart_allmaras", "")).unwrap();
    c.set("turbulence.model", serde_json::json!("rsm")).unwrap();
    assert_eq!(turb::role(&c, "xmin"), turb::Role::ZeroGradient);
    assert!(
        !has_error(&c, "boundaries.xmin.turbulence"),
        "rsm auto: {:?}",
        c.semantic_errors()
    );
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// F5: the 8 NACA 0012 example cases validate; their far field is a free-stream inlet with the D2 values.
#[test]
fn f5_naca_example_cases_validate_with_a_freestream_far_field() {
    let mut n = 0;
    for tree in ["example-projects", "example-projects-linux"] {
        for v in [
            "naca0012-aoa4",
            "naca0012-aoa2",
            "naca0012-aoa4-rotated",
            "naca0012-aoa2-rotated",
        ] {
            let p = repo()
                .join(tree)
                .join("wind-tunnel")
                .join("cases")
                .join(format!("{v}.toml"));
            let c = Case::load(&p).unwrap_or_else(|e| panic!("{}: {e:?}", p.display()));
            let errs = c.semantic_errors();
            assert!(errs.is_empty(), "{tree}/{v}: {errs:?}");
            assert_eq!(turb::role(&c, "farfield"), turb::Role::Inlet, "{tree}/{v}");
            assert!(turb::freestream_fallback(&c, "farfield"), "{tree}/{v}");
            n += 1;
        }
    }
    assert_eq!(n, 8);
}

/// F6 (replaces ADR 0166 T8 (a)): a live switch from Spalart-Allmaras to SST gives the far field SST's free-stream
/// condition with the D2 values.
#[test]
fn f6_live_switch_to_sst_takes_the_freestream_far_field() {
    let sa = freestream_plate("spalart_allmaras", "");
    let sst = freestream_plate("k_omega_sst", "");
    let mut s = run(&sa, 20);
    s.reconfigure_turbulence(&sst).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..10 {
        s.step();
    }
    assert_freestream_faces(&s, "k_omega_sst", expected("k_omega_sst"));
}
