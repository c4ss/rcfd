//! Shared case texts of the ADR 0205 (F-295) case-tree tests: CH (the channel of ADR 0199), NP (CH without its periodic seam),
//! LAM (CH with a laminar region on one side of the seam), the S4 sector of ADR 0199, and B2 / B2S (the S6 text of ADR 0199 and
//! its SIMPLEC form), with the FNV-1a field record of `f286_default_bits.rs`.

#![allow(dead_code)]

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::f64::consts::PI;

/// The cell fields a record hashes (as `f286_default_bits.rs`).
pub const FIELDS: [&str; 9] = ["ux", "uy", "uz", "p", "k", "omega", "nuTilda", "epsilon", "nut"];

/// The kinematic viscosity of CH, NP and LAM (`1/395`).
pub const NU_CH: f64 = 0.0025316455696202532;

/// FNV-1a (64 bit) over the little-endian bytes of `f64::to_bits`, as `baseline_hashes.rs`.
pub fn fnv1a(values: &[f64], acc: &mut u64) {
    for x in values {
        for b in x.to_bits().to_le_bytes() {
            *acc ^= u64::from(b);
            *acc = acc.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// (steps taken, fields hashed, hash).
pub type Record = (usize, usize, u64);

/// Build a simulation, with the diagnostics in the panic message.
pub fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let errs = case.semantic_errors();
    assert!(errs.is_empty(), "{errs:?}\n{text}");
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}\n{text}"))
}

/// `n` steps or iterations of the text, then the record of its fields and face flux.
pub fn record(text: &str, n: usize) -> Record {
    let mut sim = sim_of(text);
    let steps = sim.run(n);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let mut acc = 0xcbf2_9ce4_8422_2325_u64;
    let mut fields = 0;
    for f in FIELDS {
        if let Ok(v) = sim.field(f) {
            fnv1a(&v, &mut acc);
            fields += 1;
        }
    }
    fnv1a(sim.flux(), &mut acc);
    (steps, fields, acc)
}

/// The turbulence lines of CH (replaced to change the model).
pub const CH_SST: &str = "model = \"k_omega_sst\"\nk_initial = 1.0\nomega_initial = 40.0\n";

/// CH: the channel of ADR 0199 (box_2d 4 x 20, periodic in x, Re_tau = 395), `turbulence` extra lines (after the SST lines),
/// `extra` text before `[solver]`, and the `[solver]` lines.
pub fn ch(turbulence: &str, extra: &str, solver: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 20
lx = 1.0
ly = 2.0
periodic_x = true
[physics]
nu = 0.0025316455696202532
rho = 1.0
[initial]
velocity = [15.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[sources.drive]
type = "acceleration"
acceleration = [1.0, 0.0, 0.0]
zone = "all"
[turbulence]
{CH_SST}{turbulence}{extra}[solver]
convection = "upwind"
{solver}"#
    )
}

/// `[turbulence.periodic_seam]` with the given diffusivity (ADR 0205 D3), as `extra` text of [`ch`].
pub fn seam(diffusivity: &str) -> String {
    format!("[turbulence.periodic_seam]\ndiffusivity = \"{diffusivity}\"\n")
}

/// SIMPLEC solver lines.
pub const SIMPLEC: &str = "algorithm = \"simplec\"\nmax_iterations = 1000\n";

/// The `pimple` solver lines of the V0b channel record.
pub const PIMPLE_CH: &str = "algorithm = \"pimple\"\ntime_scheme = \"bdf2\"\nn_outer = 3\ndt = 0.1\nend_time = 100.0\n";

/// LAM: CH on SIMPLEC with a laminar region over the first column of cells (the owners of the `xmin` seam faces); `extra` text.
pub fn lam(extra: &str) -> String {
    ch(
        "",
        &format!(
            "{extra}[regions.lam]\nzone = \"box\"\nbox_min = [-1.0, -1.0, -1.0]\nbox_max = [0.25, 3.0, 1.0]\nturbulence = \"laminar\"\n"
        ),
        SIMPLEC,
    )
}

/// NP: CH on SIMPLEC without its periodic seam and drive: a fixed inlet `(15, 0, 0)` with k = 1 and omega = 40 at `xmin`, a
/// fixed-pressure outlet at `xmax`; `extra` text.
pub fn np(extra: &str) -> String {
    let t = ch("", extra, SIMPLEC);
    let drive = "[sources.drive]\ntype = \"acceleration\"\nacceleration = [1.0, 0.0, 0.0]\nzone = \"all\"\n";
    let ymin = "[boundaries.ymin]\n";
    assert!(
        t.contains(drive) && t.contains("periodic_x = true\n") && t.contains(ymin),
        "the CH text changed"
    );
    let ends = "[boundaries.xmin]\nvelocity = \"fixed\"\nvelocity_value = [15.0, 0.0, 0.0]\nturbulence_spec = \"values\"\nk_value = 1.0\nomega_value = 40.0\n[boundaries.xmax]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\n";
    t.replace(drive, "")
        .replace("periodic_x = true\n", "periodic_x = false\n")
        .replace(ymin, &format!("{ends}{ymin}"))
}

/// The S4 sector of ADR 0199 (ADR 0183 PC-3 with k-omega SST, `pimple`); `extra` text before `[solver]`.
pub fn s4(extra: &str) -> String {
    format!(
        r#"
[mesh]
generator = "annulus_3d_sector"
nx = 8
ny = 6
nz = 8
r_in = 0.5
r_out = 1.0
lz = 0.5
sector_angle = 40.0
[physics]
nu = 0.01
rho = 1.0
[initial]
velocity = [0.0, 0.0, 1.0]
[boundaries.zmin]
velocity = "fixed"
velocity_expression_x = "2 [1/s] * y + 0.2 [1/s] * x"
velocity_expression_y = "-2 [1/s] * x + 0.2 [1/s] * y"
velocity_expression_z = "1 [m/s] + 1 [1/s] * y"
turbulence_spec = "values"
k_value = 0.01
omega_value = 10.0
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
velocity = "moving_wall"
wall_rotation_rate = -2.0
wall_velocity_projection = "tangential"
turbulence = "wall"
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 2.0
centrifugal = true
[turbulence]
model = "k_omega_sst"
{extra}[solver]
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
algorithm = "pimple"
time_scheme = "bdf2"
n_outer = 3
n_correctors = 2
dt = 0.05
end_time = 100.0
"#
    )
}

// ---------------------------------------------------------------------------------------------------------------
// B2: the ADR 0086 bladed passage with the solver of ADR 0183 D5 (the S6 text of ADR 0199)
// ---------------------------------------------------------------------------------------------------------------

const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RHO: f64 = 1.2;
const Q_DESIGN: f64 = 1.4032;
/// ADR 0183 D5: dt = T_c / 100.
const H_DT: f64 = 4.605e-5;

fn omega() -> f64 {
    1486.0 * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

/// The 16 registered FAN-01 sections (ADR 0075 amendment 1), as `fan01_gci_w37c.rs` reads them.
fn sections() -> Vec<Vec<f64>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation-data/reference/w34d_fan01_cad_sections.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rows: Vec<Vec<f64>> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("r_mm"))
        .map(|l| {
            l.split(',')
                .take(8)
                .map(|v| v.trim().parse::<f64>().unwrap())
                .collect::<Vec<f64>>()
        })
        .filter(|r| r[0] != 247.0)
        .collect();
    assert_eq!(rows.len(), 16);
    rows
}

fn list(rows: &[Vec<f64>], f: impl Fn(&[f64]) -> f64) -> String {
    let v: Vec<String> = rows.iter().map(|s| format!("{:?}", f(s))).collect();
    format!("[{}]", v.join(", "))
}

/// The B2 counts: the ADR 0086 x0.75 sqrt(2) scaling of FAN-01-C (`fan01_gci_w37c.rs`, `scaled(0.75 * 2f64.sqrt())`).
fn b2_counts() -> (usize, usize, usize, usize, usize, usize, usize) {
    let s = 0.75 * 2f64.sqrt();
    let r = |n: usize| (n as f64 * s).round() as usize;
    (r(8), r(16), r(8), r(10), r(4), r(14), r(7))
}

/// The pimple lines of B2, which B2S replaces by `algorithm = "simplec"`.
const B2_PIMPLE: &str = "algorithm = \"pimple\"\nn_outer = 5\ntime_scheme = \"bdf2\"\n";

/// B2: the S6 text of ADR 0199 (`pimple`), with `extra` text after the `[turbulence]` model line.
pub fn b2(extra: &str) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = b2_counts();
    let mesh = format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"camber_seam_h\"\nn_up = {n_up}\nn_chord = {n_chord}\nn_down = {n_down}\nn_side = {n_side}\nn_nose = {n_nose}\nn_core = 1\nfront_gap = 0.3\nn_span = {n_span}\nn_gap = {n_gap}\nn_bl = 2\ngrowth = 1.2\n\
first_cell_height = 0.0\ny_plus = 30.0\nref_speed = {ref_speed:?}\nnu = 1.5e-5\n",
        list(&rows, |s| s[0] * 1e-3),
        list(&rows, |s| s[1] * 1e-3),
        list(&rows, |s| s[2]),
        list(&rows, |s| s[3]),
        list(&rows, |s| s[4]),
        list(&rows, |s| s[5]),
        list(&rows, |s| (s[6] - x0) * 1e-3),
        list(&rows, |s| s[0] * 1e-3 * (s[7] - t0).to_radians()),
    );
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let k = 1.5 * (0.01 * u_in) * (0.01 * u_in);
    let om = k.sqrt() / (0.09f64.powf(0.25) * 0.0176);
    let casing = format!(
        "velocity = \"moving_wall\"\nwall_rotation_rate = {:?}\nwall_velocity_projection = \"tangential\"\nturbulence = \"wall\"\n",
        -w
    );
    let rel = format!("velocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n");
    format!(
        r#"{mesh}
[physics]
nu = 1.5e-5
rho = {RHO:?}
[turbulence]
model = "k_omega_sst"
{extra}[initial]
velocity = [0.0, 0.0, {u_in:?}]
{rel}[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u_in:?}]
{rel}turbulence = "inlet"
turbulence_spec = "values"
k_value = {k:?}
omega_value = {om:?}
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
velocity = "fixed"
[boundaries.shroud]
{casing}[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
centrifugal = true
[boundaries.blade]
velocity = "fixed"
[solver]
{B2_PIMPLE}dt = {H_DT:?}
end_time = 0.2532
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
"#
    )
}

/// B2S: B2 with `algorithm = "simplec"` in place of its pimple lines.
pub fn b2s(extra: &str) -> String {
    let t = b2(extra);
    assert!(t.contains(B2_PIMPLE), "the B2 text changed");
    t.replace(B2_PIMPLE, "algorithm = \"simplec\"\n")
}
