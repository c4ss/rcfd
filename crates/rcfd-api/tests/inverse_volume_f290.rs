//! F-290 (ADR 0203): the value `inverse_volume` of `dynamic_mesh.smoothing.method` runs ADR 0090 D4's `InverseVolume(k)`
//! displacement-diffusion stiffness, with `k` from `dynamic_mesh.smoothing.exponent`.

use rcfd_api::case::Case;

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
        ("P-I inverse_distance, exponent 2", p_i("method = \"inverse_distance\"\nexponent = 2.0\n")),
        ("P-V laplacian (no smoothing table)", p_v(true, "")),
        ("ADR 0191 P-B", p_v(false, "")),
    ];
    for (what, text) in &texts {
        println!("D0 {what}: {:016x}", case(text).tree.content_hash());
    }
}
