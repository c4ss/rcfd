//! ADR 0164 (job 49A): the opt-in force components `total_effective` / `viscous_effective` take the viscous stress with
//! the momentum equation's face viscosity (molecular + eddy viscosity, on wall faces the wall-function value). Limits of
//! ADR 0164 C1-C3 (registered before this file was run); C4-C5 test the far-field role `turbulence = "freestream"` of
//! amendment 1 (registered before they were run). ADR 0166 D4: with a turbulence model the default `total` / `viscous`
//! are now the effective values, so C2/C3 compare against the molecular values by their new names `total_molecular` /
//! `viscous_molecular` (the pre-ADR 0166 defaults); the limits are unchanged.

use rcfd_api::case::Case;
use rcfd_api::sim::{ReportDef, Simulation};

/// Flat plate, Re = 5e6 per m, wall cell for y+ about 40 (a wall-function cell), coarse.
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
[reports.d_total]
type = "force_coefficient"
patch = "ymin"
direction = [1.0, 0.0, 0.0]
a_ref = 0.02
[reports.d_visc]
type = "force_coefficient"
patch = "ymin"
direction = [1.0, 0.0, 0.0]
a_ref = 0.02
component = "viscous"
[reports.d_total_eff]
type = "force_coefficient"
patch = "ymin"
direction = [1.0, 0.0, 0.0]
a_ref = 0.02
component = "total_effective"
[reports.d_visc_eff]
type = "force_coefficient"
patch = "ymin"
direction = [1.0, 0.0, 0.0]
a_ref = 0.02
component = "viscous_effective"
[reports.d_total_mol]
type = "force_coefficient"
patch = "ymin"
direction = [1.0, 0.0, 0.0]
a_ref = 0.02
component = "total_molecular"
[reports.d_visc_mol]
type = "force_coefficient"
patch = "ymin"
direction = [1.0, 0.0, 0.0]
a_ref = 0.02
component = "viscous_molecular"
"#;

fn values(c: &Case, n: usize) -> Vec<(String, f64)> {
    let mut s = Simulation::new(c).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..n {
        s.step();
    }
    ReportDef::from_case(c)
        .iter()
        .map(|r| (r.name.clone(), s.eval_report(r).unwrap()))
        .collect()
}

fn get(v: &[(String, f64)], name: &str) -> f64 {
    v.iter().find(|(n, _)| n == name).unwrap().1
}

/// C1: laminar constant viscosity: the effective components are the default ones, bit for bit.
#[test]
fn c1_laminar_effective_equals_default() {
    let c = Case::parse_toml(PLATE).unwrap();
    let v = values(&c, 40);
    assert_eq!(get(&v, "d_visc").to_bits(), get(&v, "d_visc_eff").to_bits());
    assert_eq!(get(&v, "d_total").to_bits(), get(&v, "d_total_eff").to_bits());
    assert!(get(&v, "d_visc") > 0.0);
}

/// C2 / C3: Spalart-Allmaras with its wall function at y+ about 40: the effective viscous part is well above the
/// molecular one (the molecular one is about u+/y+ of the wall shear), and the pressure part is the same in both. The
/// molecular values are the `*_molecular` components (ADR 0166 D4; before ADR 0166 the default ones).
#[test]
fn c2_c3_sa_wall_function_effective_shear() {
    let mut c = Case::parse_toml(PLATE).unwrap();
    c.set("turbulence.model", serde_json::json!("spalart_allmaras"))
        .unwrap();
    let v = values(&c, 400);
    let (mol, eff) = (get(&v, "d_visc_mol"), get(&v, "d_visc_eff"));
    assert!(mol > 0.0 && eff.is_finite(), "{v:?}");
    assert!(eff / mol > 1.5, "effective {eff} molecular {mol}");
    let p_mol = get(&v, "d_total_mol") - mol;
    let p_eff = get(&v, "d_total_eff") - eff;
    assert!(
        (p_mol - p_eff).abs() <= 1e-12 * (get(&v, "d_total_mol").abs() + 1e-30),
        "{v:?}"
    );
}

/// C4 (ADR 0164 amendment 1): `turbulence = "freestream"` on a `velocity = "freestream"` patch fixes nuTilda at
/// `nutilde_ratio * nu` on the inflow faces and leaves it zero-gradient (the owner value) on the outflow faces.
#[test]
fn c4_freestream_turbulence_role_fixes_inflow_only() {
    let text = PLATE
        .replace(
            "[boundaries.xmin]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]",
            "[boundaries.xmin]\nvelocity = \"freestream\"\nvelocity_value = [1.0, 0.0, 0.0]\npressure = \"freestream\"\nturbulence = \"freestream\"",
        )
        .replace(
            "[boundaries.xmax]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"",
            "[boundaries.xmax]\nvelocity = \"freestream\"\nvelocity_value = [1.0, 0.0, 0.0]\npressure = \"freestream\"\nturbulence = \"freestream\"",
        )
        + "[turbulence]\nmodel = \"spalart_allmaras\"\n";
    assert_eq!(text.matches("turbulence = \"freestream\"").count(), 2);
    let c = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..30 {
        s.step();
    }
    let g = &s.geom;
    let nb = s.boundary_field("nuTilda").unwrap();
    let cells = s.field("nuTilda").unwrap();
    let own = g.mesh.owner();
    let nu = 2e-7;
    for f in g.mesh.patch("xmin").unwrap().faces() {
        assert_eq!(nb[f - g.n_internal], 3.0 * nu, "inflow face {f}");
    }
    let mut differs = false;
    for f in g.mesh.patch("xmax").unwrap().faces() {
        assert_eq!(nb[f - g.n_internal], cells[own[f]], "outflow face {f}");
        differs |= cells[own[f]] != 3.0 * nu;
    }
    assert!(
        differs,
        "the outflow faces should carry the cell values, not the fixed value"
    );
}

/// C5: the role needs a free-stream velocity on the patch.
#[test]
fn c5_freestream_turbulence_role_needs_a_freestream_velocity() {
    let mut c = Case::parse_toml(PLATE).unwrap();
    c.set("turbulence.model", serde_json::json!("spalart_allmaras"))
        .unwrap();
    c.set("boundaries.xmin.turbulence", serde_json::json!("freestream"))
        .unwrap();
    let errs = c.semantic_errors();
    assert!(errs.iter().any(|d| d.path == "boundaries.xmin.turbulence"), "{errs:?}");
}
