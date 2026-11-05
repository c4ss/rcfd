//! The built-in case templates (moved from `rcfd-mcp`, ADR 0186 G2, text unchanged): starting points of the MCP session
//! tool `create_case` and of the workspace command `new_case`.

/// Built-in starting points for the session tool `create_case` and the project tool `new_case` of `rcfd-mcp`.
pub const TEMPLATES: &[(&str, &str)] = &[("lid_driven_cavity", LID_DRIVEN_CAVITY)];

/// Lid-driven cavity at Re = 100 (unit lid speed, unit side, nu = 0.01).
pub const LID_DRIVEN_CAVITY: &str = r#"title = "lid-driven cavity Re=100"
[mesh]
generator = "cavity_2d"
nx = 16
ny = 16
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 600
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
"#;
