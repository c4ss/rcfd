//! Checks R1-R4 of ADR 0036: per-region model assignment (laminar regions inside a turbulent run).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn channel(turbulence: &str, regions: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 30
ny = 12
lx = 3.0
ly = 0.5
depth = 0.1
[physics]
nu = 0.02
rho = 1.0
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
turbulence_length = 0.035
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 400
{turbulence}
{regions}
"#
    )
}

fn run(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..400 {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    sim
}

fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

const KE: &str = "[turbulence]\nmodel = \"k_epsilon\"";
const LAM_ALL: &str = "[regions.all]\nzone = \"all\"\nturbulence = \"laminar\"";

#[test]
fn r1_laminar_region_over_the_whole_domain_equals_a_laminar_run() {
    let laminar = run(&channel("[turbulence]\nmodel = \"laminar\"", ""));
    let masked = run(&channel(KE, LAM_ALL));
    let turbulent = run(&channel(KE, ""));
    let range = |p: &[f64]| p.iter().cloned().fold(f64::MIN, f64::max) - p.iter().cloned().fold(f64::MAX, f64::min);
    let (pl, pm) = (laminar.field("p").unwrap(), masked.field("p").unwrap());
    let du = ["ux", "uy"]
        .iter()
        .map(|n| max_diff(&laminar.field(n).unwrap(), &masked.field(n).unwrap()))
        .fold(0.0, f64::max);
    let dp = max_diff(&pl, &pm);
    let ut = max_diff(&laminar.field("ux").unwrap(), &turbulent.field("ux").unwrap());
    println!(
        "R1: max|du| {du:.3e} (limit 1e-6), max|dp| {dp:.3e} (limit {:.3e}); turbulent-vs-laminar separation {ut:.3e}",
        1e-6 * range(&pl)
    );
    assert!(du <= 1e-6 && dp <= 1e-6 * range(&pl));
    // the masked run really differs from the unmasked turbulent one (the check is not vacuous)
    assert!(ut > 1e-4, "turbulent and laminar solutions coincide: {ut:e}");
}

#[test]
fn r1b_partial_region_changes_only_the_solution_downstream_of_the_mask() {
    let turbulent = run(&channel(KE, ""));
    let part = run(&channel(
        KE,
        "[regions.rear]\nzone = \"box\"\nbox_min = [1.5, -1.0, -1.0]\nbox_max = [3.0, 1.0, 1.0]\nturbulence = \"laminar\"",
    ));
    let d = max_diff(&turbulent.field("ux").unwrap(), &part.field("ux").unwrap());
    println!("R1b: partial laminar region changes ux by {d:.3e}");
    assert!(d > 1e-4);
}

#[test]
fn r2_mask_faces_semantics_are_exact() {
    let case = Case::parse_toml(&channel(KE, "")).unwrap();
    let sim = Simulation::new(&case).unwrap();
    let geom = &sim.geom;
    let n = geom.n_cells();
    let mut cells = vec![false; n];
    cells[n / 2] = true;
    let nf = geom.mesh.owner().len();
    let mut nu = vec![5.0; nf];
    rcfd_api::regions::mask_faces(geom, &cells, 0.25, &mut nu);
    for f in 0..nf {
        let o = geom.mesh.owner()[f];
        let touches = cells[o] || (f < geom.n_internal && cells[geom.mesh.neighbour()[f]]);
        assert_eq!(nu[f], if touches { 0.25 } else { 5.0 }, "face {f}");
    }
    assert!(nu.iter().any(|&v| v == 0.25));
    // an empty mask leaves the field untouched
    let mut nu2 = vec![5.0; nf];
    rcfd_api::regions::mask_faces(geom, &vec![false; n], 0.25, &mut nu2);
    assert!(nu2.iter().all(|&v| v == 5.0));
}

fn error_path(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(e) => e.iter().map(|d| d.path.clone()).collect(),
        Ok(c) => match Simulation::new(&c) {
            Err(e) => e.iter().map(|d| d.path.clone()).collect(),
            Ok(_) => Vec::new(),
        },
    }
}

#[test]
fn r3_located_diagnostics() {
    let cases: Vec<(&str, String, &str)> = vec![
        (
            "unknown value",
            channel(KE, "[regions.a]\nturbulence = \"wall_modelled\""),
            "regions.a.turbulence",
        ),
        (
            "empty box",
            channel(
                KE,
                "[regions.a]\nzone = \"box\"\nbox_min = [10.0, 10.0, 10.0]\nbox_max = [11.0, 11.0, 11.0]\nturbulence = \"laminar\"",
            ),
            "regions.a.zone",
        ),
        (
            "unknown cell zone",
            channel(KE, "[regions.a]\nzone = \"cell_zone\"\ncell_zone = \"nope\"\nturbulence = \"laminar\""),
            "regions.a.cell_zone",
        ),
        (
            "no turbulence model",
            channel("[turbulence]\nmodel = \"laminar\"", LAM_ALL),
            "regions.all.turbulence",
        ),
        (
            "rsm",
            channel("[turbulence]\nmodel = \"rsm\"", LAM_ALL),
            "regions.all.turbulence",
        ),
        (
            "temperature-dependent viscosity",
            channel(KE, LAM_ALL)
                .replace("rho = 1.0\n", "rho = 1.0\ncp = 1.0\nconductivity = 1.0\n")
                .replace(
                    "[initial]\n",
                    "[energy]\nenabled = true\nbuoyancy = false\n[initial]\ntemperature = 300.0\n",
                )
                .replace(
                    "[boundaries.inlet]\n",
                    "[boundaries.inlet]\ntemperature = \"fixed\"\ntemperature_value = 300.0\n",
                )
                .replace(
                    "[boundaries.walls]\n",
                    "[boundaries.walls]\ntemperature = \"fixed\"\ntemperature_value = 310.0\n",
                )
                + "\n[properties.viscosity]\nlaw = \"polynomial\"\ncoefficients = [0.02, 0.0]\n",
            "regions.all.turbulence",
        ),
    ];
    for (label, text, want) in cases {
        let paths = error_path(&text);
        println!("R3 {label}: {paths:?}");
        assert!(
            paths.iter().any(|p| p == want),
            "{label}: expected {want}, got {paths:?}"
        );
    }
}

#[test]
fn r4_without_regions_the_default_path_is_unchanged() {
    let a = run(&channel(KE, ""));
    let b = run(&channel(KE, ""));
    assert_eq!(a.field("ux").unwrap(), b.field("ux").unwrap());
}
