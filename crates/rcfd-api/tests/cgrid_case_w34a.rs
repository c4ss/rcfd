//! Wave 34A (ADR 0072, R1-E): the C-grid example case built with the smoothed construction (`mesh.fan.c_grid_construction =
//! "smoothed"`) through the same mesh-JSON producer the service uses; `rules::check_case` must raise no error-level finding.

use rcfd_api::case::Case;
use rcfd_api::rules::{check_case, has_errors, mesh_context, Severity};
use rcfd_api::sim::build_mesh;
use rcfd_mesh::quality::check_mesh;

#[test]
fn r1e_rules_raise_no_error_on_the_smoothed_example_case() {
    let base = include_str!("../../../cases/axial_fan_cgrid/axial_fan_cgrid.toml");
    let text = base.replace(
        "\ntopology = \"c_grid\"",
        "\ntopology = \"c_grid\"\nc_grid_construction = \"smoothed\"",
    );
    assert!(text.contains("c_grid_construction"));
    let case = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let mesh = build_mesh(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let q = check_mesh(&mesh);
    let rules = check_case(&case, Some(&mesh_context(&q)));
    println!(
        "R1-E {} cells; mesh check ok {}; non-orthogonality max {:.2} deg; skewness max {:.3}; {} rule findings",
        q.n_cells,
        q.is_ok(),
        q.max_non_orthogonality,
        q.max_skewness,
        rules.len()
    );
    for r in &rules {
        println!("R1-E finding {} {:?} {}: {}", r.id, r.severity, r.path, r.message);
    }
    assert!(!has_errors(&rules), "error-level finding");
    assert!(rules.iter().all(|r| r.severity != Severity::Error));
    assert!(q.is_ok(), "check_mesh rejects the mesh");
}

#[test]
fn unknown_construction_is_rejected() {
    let base = include_str!("../../../cases/axial_fan_cgrid/axial_fan_cgrid.toml");
    let text = base.replace(
        "\ntopology = \"c_grid\"",
        "\ntopology = \"c_grid\"\nc_grid_construction = \"elliptic\"",
    );
    assert!(Case::parse_toml(&text).is_err() || build_mesh(&Case::parse_toml(&text).unwrap()).is_err());
}
