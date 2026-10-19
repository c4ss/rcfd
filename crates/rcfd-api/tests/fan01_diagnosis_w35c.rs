//! Wave 35C (ADR 0078): diagnosis of the FAN-01 deficit of ADR 0075 attempt 3 by controlled one-factor arms at the design point, an
//! energy / angular-momentum balance of every run, and a blade-element estimate from the CAD sections. Everything here was registered in
//! ADR 0078 before it was written. No arm is an F-126 attempt and no fan performance claim is made.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_mesh::fan::{camber_line, AxialFan, SectionFamily, Station};
use rcfd_mesh::fan_mesh::graded_cells;
use std::f64::consts::PI;
use std::time::Instant;

/// FAN-01 sections (as `fan01_validation.rs`, from `w34d_fan01_cad_sections.csv`): r [mm], chord [mm], stagger from the axis [deg],
/// max camber / c, its position x/c, max thickness / c, leading edge x [mm] and theta [deg] (CAD frame).
const SECTIONS: [[f64; 8]; 16] = [
    [124.0, 98.92, 43.63, 0.0364, 0.53, 0.1086, -23.70, -15.93],
    [127.0, 96.38, 44.81, 0.0365, 0.53, 0.1088, -22.40, -16.34],
    [130.0, 93.84, 45.99, 0.0366, 0.53, 0.1090, -21.10, -16.75],
    [140.0, 85.99, 49.94, 0.0369, 0.52, 0.1094, -17.05, -18.05],
    [150.0, 79.23, 53.26, 0.0372, 0.52, 0.1097, -13.80, -19.26],
    [160.0, 73.68, 55.89, 0.0374, 0.52, 0.1100, -11.35, -20.33],
    [170.0, 69.41, 58.20, 0.0375, 0.52, 0.1102, -9.43, -21.21],
    [180.0, 65.89, 60.21, 0.0375, 0.52, 0.1104, -7.87, -21.95],
    [190.0, 62.88, 61.97, 0.0376, 0.52, 0.1106, -6.59, -22.60],
    [200.0, 60.51, 63.57, 0.0376, 0.52, 0.1107, -5.53, -23.14],
    [210.0, 58.57, 65.04, 0.0376, 0.52, 0.1109, -4.62, -23.60],
    [220.0, 57.11, 66.39, 0.0376, 0.52, 0.1110, -3.87, -23.98],
    [230.0, 56.19, 67.67, 0.0376, 0.52, 0.1111, -3.23, -24.29],
    [240.0, 55.49, 68.83, 0.0376, 0.515, 0.1111, -2.68, -24.55],
    [245.0, 55.08, 69.35, 0.0376, 0.515, 0.1111, -2.43, -24.69],
    [247.5, 54.87, 69.60, 0.0376, 0.515, 0.1112, -2.29, -24.76],
];

const N_BLADES: f64 = 9.0;
const R_HUB: f64 = 0.124;
const R_TIP: f64 = 0.2475;
const R_DUCT: f64 = 0.250;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075), UNVERIFIED
const NU: f64 = 1.5e-5;
const RPM: f64 = 1486.0;
const Q_DESIGN: f64 = 1.4032;
const DP_MEAS: f64 = 126.46;
const POWER_MEAS: f64 = 333.30;
const H1: f64 = 2.0e-4; // ADR 0075 attempt 2 first cell
const TRUNC_TE: f64 = 0.01; // generator default
const WALL_CLOCK_MIN: f64 = 60.0;

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

fn list(f: impl Fn(&[f64; 8]) -> f64) -> String {
    let v: Vec<String> = SECTIONS.iter().map(|s| format!("{:?}", f(s))).collect();
    format!("[{}]", v.join(", "))
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Outlet {
    Fixed,
    NoReverse,
    Radial,
}

/// One registered arm (ADR 0078).
#[derive(Clone, Debug)]
struct Arm {
    label: &'static str,
    projected_casing: bool,
    axial_extent: f64,
    z_le_root: f64,
    n_up: usize,
    n_down: usize,
    n_gap: usize,
    n_chord: usize,
    n_side: usize,
    convection: &'static str,
    outlet: Outlet,
    /// amendment 2 (X2): false = empty passage (no blade), hub and shroud at rest in the absolute frame
    include_blade: bool,
    /// amendment 2 (X2): false = inertial frame (no `rotating_frame`, velocities absolute)
    rotating: bool,
}

fn e0() -> Arm {
    Arm {
        label: "E0",
        projected_casing: false,
        axial_extent: 0.4,
        z_le_root: 0.1,
        n_up: 8,
        n_down: 8,
        n_gap: 7,
        n_chord: 12,
        n_side: 10,
        convection: "upwind",
        outlet: Outlet::Fixed,
        include_blade: true,
        rotating: true,
    }
}

/// Hub section of the generator (station 0) with the hub leading edge at `z_le_root`.
fn hub_station() -> (AxialFan, Station) {
    let s = SECTIONS[0];
    let st = Station {
        radius: R_HUB,
        chord: s[1] * 1e-3,
        stagger: s[2].to_radians(),
        camber: s[3],
        camber_pos: s[4],
        thickness: s[5],
        sweep: 0.0,
        lean: 0.0,
    };
    let fan = AxialFan {
        n_blades: 9,
        r_hub: R_HUB,
        r_tip: R_TIP,
        tip_clearance: R_DUCT - R_TIP,
        axial_extent: 0.4,
        z_le_root: 0.0,
        family: SectionFamily::Naca4,
        stations: vec![st],
    };
    (fan, st)
}

/// Largest axial cell of `n` cells over `len` with the boundary-layer cells at one end (the generator's `graded_cells`).
fn largest_cell(len: f64, n: usize, wall_at_low_end: bool) -> f64 {
    let w = Some((H1, 1.2, 2));
    let x = if wall_at_low_end {
        graded_cells(len, n, w, None)
    } else {
        graded_cells(len, n, None, w)
    };
    match x {
        Ok(x) => x.windows(2).map(|p| p[1] - p[0]).fold(0.0, f64::max),
        Err(_) => f64::INFINITY,
    }
}

/// ADR 0078 counts rule: the smallest `n` whose largest axial cell over `len` is at most the E0 value over `len0` with 8 cells.
fn count_rule(len0: f64, len: f64, wall_at_low_end: bool) -> usize {
    let target = largest_cell(len0, 8, wall_at_low_end);
    (3..200)
        .find(|&n| largest_cell(len, n, wall_at_low_end) <= target)
        .unwrap()
}

/// Largest ratio of neighbouring axial cells of `n` cells over `len` (the generator's geometric growth).
fn growth_ratio(len: f64, n: usize, wall_at_low_end: bool) -> f64 {
    let w = Some((H1, 1.2, 2));
    let Ok(x) = (if wall_at_low_end {
        graded_cells(len, n, w, None)
    } else {
        graded_cells(len, n, None, w)
    }) else {
        return f64::INFINITY;
    };
    let h: Vec<f64> = x.windows(2).map(|p| p[1] - p[0]).collect();
    h.windows(2).map(|p| (p[1] / p[0]).max(p[0] / p[1])).fold(0.0, f64::max)
}

/// ADR 0078 amendment 1 rule: the smallest `n` over `len` whose growth ratio is at most `target`.
fn ratio_rule(len: f64, target: f64, wall_at_low_end: bool) -> usize {
    (3..200)
        .find(|&n| growth_ratio(len, n, wall_at_low_end) <= target)
        .unwrap()
}

/// Axial positions (relative to `z_le_root`) of the hub trailing-edge cut plane.
fn hub_te_offset() -> f64 {
    let (fan, st) = hub_station();
    fan.camber_zs(&st, 1.0 - TRUNC_TE).0
}

fn arm_c1() -> Arm {
    let zb = 0.1 + hub_te_offset();
    Arm {
        label: "C1",
        axial_extent: 0.8,
        n_down: count_rule(0.4 - zb, 0.8 - zb, true),
        ..e0()
    }
}

fn arm_d1() -> Arm {
    let (fan, st) = hub_station();
    let za = fan.camber_zs(&st, 0.02).0; // generator default trunc_le
    Arm {
        label: "D1",
        z_le_root: 0.3,
        axial_extent: 0.6,
        n_up: count_rule(0.1 + za, 0.3 + za, false),
        ..e0()
    }
}

/// Amendment 1: E0 length with the downstream growth ratio of C1 (C1 - C0 isolates the outlet location, C0 - E0 the downstream grading).
fn arm_c0() -> Arm {
    let zb = 0.1 + hub_te_offset();
    let c1 = arm_c1();
    Arm {
        label: "C0",
        n_down: ratio_rule(0.4 - zb, growth_ratio(0.8 - zb, c1.n_down, true), true),
        ..e0()
    }
}

/// Amendment 1: E0 length with the upstream growth ratio of D1.
fn arm_d0() -> Arm {
    let (fan, st) = hub_station();
    let za = fan.camber_zs(&st, 0.02).0;
    let d1 = arm_d1();
    Arm {
        label: "D0",
        n_up: ratio_rule(0.1 + za, growth_ratio(0.3 + za, d1.n_up, false), false),
        ..e0()
    }
}

fn arm_k() -> Arm {
    let (c1, d1) = (arm_c1(), arm_d1());
    Arm {
        label: "K",
        projected_casing: true,
        z_le_root: 0.3,
        axial_extent: 1.0,
        n_up: d1.n_up,
        n_down: c1.n_down,
        n_gap: 11,
        ..e0()
    }
}

fn arms() -> Vec<Arm> {
    vec![
        e0(),
        Arm {
            label: "E1",
            projected_casing: true,
            ..e0()
        },
        arm_c1(),
        Arm {
            label: "C2",
            outlet: Outlet::NoReverse,
            ..e0()
        },
        Arm {
            label: "C3",
            outlet: Outlet::Radial,
            ..e0()
        },
        arm_d1(),
        Arm {
            label: "D2",
            n_gap: 11,
            ..e0()
        },
        // amendment 1: N1 as registered is NOT RUNNABLE (both smokes failed the stability rule); N1b is minmod, registered after its smoke
        Arm {
            label: "N1b",
            convection: "tvd_minmod",
            ..e0()
        },
        // amendment 1: in-plane refinement only (n_up, n_down stay 8; the axial grading is measured by C0 / D0)
        Arm {
            label: "N2",
            n_chord: 20,
            n_side: 16,
            ..e0()
        },
        arm_k(),
        arm_c0(),
        arm_d0(),
        // amendment 2: X2 empty-passage check (exact absolute swirl zero)
        Arm {
            label: "X2a",
            include_blade: false,
            ..e0()
        },
        Arm {
            label: "X2b",
            include_blade: false,
            rotating: false,
            ..e0()
        },
        Arm {
            label: "X2c",
            include_blade: false,
            n_chord: 20,
            n_side: 16,
            ..e0()
        },
        Arm {
            label: "X2d",
            include_blade: false,
            convection: "tvd_minmod",
            ..e0()
        },
        Arm {
            label: "X2e",
            include_blade: false,
            convection: "linear_upwind",
            ..e0()
        },
    ]
}

fn arm(label: &str) -> Arm {
    arms().into_iter().find(|a| a.label == label).unwrap()
}

fn mesh_toml(a: &Arm) -> String {
    let le_x0 = SECTIONS[0][6];
    let le_t0 = SECTIONS[0][7];
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = {R_HUB}\nr_tip = {R_TIP}\ntip_clearance = 0.0025\naxial_extent = {:?}\nz_le_root = {:?}\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
n_up = {}\nn_chord = {}\nn_down = {}\nn_side = {}\nn_mid = 1\nn_span = 16\nn_gap = {}\nn_bl = 2\ngrowth = 1.2\nfirst_cell_height = {H1:?}\n{}",
        a.axial_extent,
        a.z_le_root,
        list(|s| s[0] * 1e-3),
        list(|s| s[1] * 1e-3),
        list(|s| s[2]),
        list(|s| s[3]),
        list(|s| s[4]),
        list(|s| s[5]),
        list(|s| (s[6] - le_x0) * 1e-3),
        list(|s| s[0] * 1e-3 * (s[7] - le_t0).to_radians()),
        a.n_up,
        a.n_chord,
        a.n_down,
        a.n_side,
        a.n_gap,
        if a.include_blade { "" } else { "include_blade = false\n" },
    )
}

/// The ADR 0075 attempt-3 design-point case with the arm's changes (and the report-only shroud moment).
fn case_toml(a: &Arm) -> String {
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let k = 1.5 * (0.01 * u_in) * (0.01 * u_in);
    let om = k.sqrt() / (0.09f64.powf(0.25) * 0.0176);
    let projected = format!("velocity = \"moving_wall\"\nwall_rotation_rate = {:?}\nwall_velocity_projection = \"tangential\"\nturbulence = \"wall\"\n", -w);
    let shroud = if !a.rotating {
        "velocity = \"fixed\"\nturbulence = \"wall\"\n".to_string()
    } else if a.projected_casing || !a.include_blade {
        projected.clone()
    } else {
        format!("velocity = \"fixed\"\nvelocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\nturbulence = \"wall\"\n")
    };
    // X2 (amendment 2): the empty passage has the hub at rest in the absolute frame too
    let hub = if a.rotating && !a.include_blade {
        projected
    } else {
        "velocity = \"fixed\"\n".to_string()
    };
    let rel = if a.rotating {
        format!("velocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n")
    } else {
        String::new()
    };
    let blade = if a.include_blade {
        "[boundaries.blade]\nvelocity = \"fixed\"\n[reports.m_blade]\ntype = \"moment\"\npatch = \"blade\"\ndirection = [0.0, 0.0, 1.0]\n"
    } else {
        ""
    };
    let frame = if a.rotating {
        format!("[sources.frame]\ntype = \"rotating_frame\"\nrotation_axis = [0.0, 0.0, 1.0]\nangular_velocity = {w:?}\ncentrifugal = true\n")
    } else {
        String::new()
    };
    let outlet = match a.outlet {
        Outlet::Fixed => "velocity = \"zero_gradient\"\npressure = \"fixed\"\n".to_string(),
        Outlet::NoReverse => "bc_kind = \"pressure_outlet\"\nbc_pressure = 0.0\nprevent_reverse_flow = true\n".to_string(),
        Outlet::Radial => format!(
            "bc_kind = \"radial_equilibrium_outlet\"\nradial_reference = \"area_average\"\nbc_pressure = 0.0\nradial_frame_rate = {w:?}\n"
        ),
    };
    format!(
        r#"{}
[physics]
nu = {NU:?}
rho = {RHO:?}
[turbulence]
model = "k_omega_sst"
[initial]
velocity = [0.0, 0.0, {u_in:?}]
{rel}[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u_in:?}]
{rel}turbulence = "inlet"
turbulence_spec = "values"
k_value = {k:?}
omega_value = {om:?}
[boundaries.outlet]
{outlet}[boundaries.hub]
{hub}[boundaries.shroud]
{shroud}{frame}{blade}[solver]
algorithm = "simplec"
convection = "{}"
max_iterations = 8000
tolerance_u = 1e-5
tolerance_p = 1e-5
[reports.m_in]
type = "mass_flow"
patch = "inlet"
[reports.m_out]
type = "mass_flow"
patch = "outlet"
[reports.p_in]
type = "area_average"
patch = "inlet"
field = "p"
[reports.p_out]
type = "area_average"
patch = "outlet"
field = "p"
[reports.m_hub]
type = "moment"
patch = "hub"
direction = [0.0, 0.0, 1.0]
[reports.m_shroud]
type = "moment"
patch = "shroud"
direction = [0.0, 0.0, 1.0]
"#,
        mesh_toml(a),
        a.convection
    )
}

// ---------------------------------------------------------------- surface statistics (ADR 0078 "quantities")

/// One face sample: centre, absolute velocity, kinematic static pressure, flux oriented in +z, area projected on z.
struct Sample {
    x: [f64; 3],
    c: [f64; 3],
    p: f64,
    phi: f64,
    a: f64,
}

fn absolute(w: [f64; 3], x: [f64; 3], om: f64) -> [f64; 3] {
    [w[0] - om * x[1], w[1] + om * x[0], w[2]]
}

fn polar(c: [f64; 3], x: [f64; 3]) -> (f64, f64, f64) {
    let r = x[0].hypot(x[1]);
    ((x[0] * c[0] + x[1] * c[1]) / r, (-x[1] * c[0] + x[0] * c[1]) / r, r)
}

fn patch_samples(sim: &Simulation, name: &str, om: f64) -> Vec<Sample> {
    let g = &sim.geom;
    let ni = g.n_internal;
    let ub = sim.velocity().boundary_values(g);
    let pb = sim.boundary_field("p").unwrap();
    let phi = sim.flux();
    let (cf, sf) = (g.mesh.face_centres(), g.mesh.face_area_vectors());
    g.mesh
        .patch(name)
        .unwrap()
        .faces()
        .map(|f| {
            let s = if sf[f][2] >= 0.0 { 1.0 } else { -1.0 };
            Sample {
                x: cf[f],
                c: absolute(ub[f - ni], cf[f], om),
                p: pb[f - ni],
                phi: s * phi[f],
                a: sf[f][2].abs(),
            }
        })
        .collect()
}

/// Internal faces whose owner and neighbour centres lie on different sides of `z0`, oriented in +z; face values are cell means.
fn cut_samples(sim: &Simulation, z0: f64, om: f64) -> Vec<Sample> {
    let g = &sim.geom;
    let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
    let (cf, sf, cc) = (g.mesh.face_centres(), g.mesh.face_area_vectors(), g.mesh.cell_centres());
    let (ux, uy, uz, p) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
        sim.field("p").unwrap(),
    );
    let phi = sim.flux();
    let mut out = Vec::new();
    for f in 0..g.n_internal {
        let (o, n) = (own[f], nei[f]);
        let s = if cc[o][2] < z0 && cc[n][2] >= z0 {
            1.0
        } else if cc[o][2] >= z0 && cc[n][2] < z0 {
            -1.0
        } else {
            continue;
        };
        let w = [0.5 * (ux[o] + ux[n]), 0.5 * (uy[o] + uy[n]), 0.5 * (uz[o] + uz[n])];
        out.push(Sample {
            x: cf[f],
            c: absolute(w, cf[f], om),
            p: 0.5 * (p[o] + p[n]),
            phi: s * phi[f],
            a: (s * sf[f][2]).abs(),
        });
    }
    out
}

/// Surface statistics (full annulus, SI units: Pa, J/kg, kg/s, kg m^2/s^2).
#[derive(Debug, Clone, Copy, Default)]
struct Stats {
    q: f64,
    area: f64,
    p_area: f64,
    p_mass: f64,
    pt_mass: f64,
    kz: f64,
    kth: f64,
    kr: f64,
    nonuni: f64,
    l_flux: f64,
    back_area: f64,
    back_mass: f64,
}

fn stats(s: &[Sample]) -> Stats {
    let q: f64 = s.iter().map(|v| v.phi).sum();
    let area: f64 = s.iter().map(|v| v.a).sum();
    let m = |f: &dyn Fn(&Sample) -> f64| s.iter().map(|v| f(v) * v.phi).sum::<f64>() / q;
    let kz = m(&|v| 0.5 * v.c[2] * v.c[2]);
    let kth = m(&|v| {
        let (_, t, _) = polar(v.c, v.x);
        0.5 * t * t
    });
    let kr = m(&|v| {
        let (r, _, _) = polar(v.c, v.x);
        0.5 * r * r
    });
    let cbar = q / area;
    let neg_a: f64 = s.iter().filter(|v| v.phi < 0.0).map(|v| v.a).sum();
    let neg_m: f64 = s.iter().filter(|v| v.phi < 0.0).map(|v| -v.phi).sum();
    let p_mass = m(&|v| v.p);
    Stats {
        q: N_BLADES * q,
        area: N_BLADES * area,
        p_area: RHO * s.iter().map(|v| v.p * v.a).sum::<f64>() / area,
        p_mass: RHO * p_mass,
        pt_mass: RHO * (p_mass + kz + kth + kr),
        kz: RHO * kz,
        kth: RHO * kth,
        kr: RHO * kr,
        nonuni: RHO * (kz - 0.5 * cbar * cbar),
        l_flux: N_BLADES
            * RHO
            * s.iter()
                .map(|v| {
                    let (_, t, r) = polar(v.c, v.x);
                    r * t * v.phi
                })
                .sum::<f64>(),
        back_area: neg_a / area,
        back_mass: neg_m / q,
    }
}

/// Mass-averaged (r c_theta, c_z, c_theta) in 8 equal radial bins from the hub to the shroud.
fn bins(s: &[Sample]) -> Vec<(f64, f64, f64, f64)> {
    let nb = 8;
    let dr = (R_DUCT - R_HUB) / nb as f64;
    (0..nb)
        .map(|b| {
            let (lo, hi) = (R_HUB + b as f64 * dr, R_HUB + (b + 1) as f64 * dr);
            let sel: Vec<&Sample> = s
                .iter()
                .filter(|v| {
                    let r = v.x[0].hypot(v.x[1]);
                    r >= lo && (r < hi || (b == nb - 1 && r <= hi + 1e-9))
                })
                .collect();
            let q: f64 = sel.iter().map(|v| v.phi).sum();
            let avg = |f: &dyn Fn(&Sample) -> f64| sel.iter().map(|v| f(v) * v.phi).sum::<f64>() / q;
            let rct = avg(&|v| {
                let (_, t, r) = polar(v.c, v.x);
                r * t
            });
            let cth = avg(&|v| polar(v.c, v.x).1);
            (0.5 * (lo + hi), rct, avg(&|v| v.c[2]), cth)
        })
        .collect()
}

/// Section values interpolated linearly in r: (stagger [rad], camber m, camber position p, chord [m]).
fn section_at(r: f64) -> (f64, f64, f64, f64) {
    let rm = (r * 1e3).clamp(SECTIONS[0][0], SECTIONS[15][0]);
    let i = (0..15).find(|&i| rm <= SECTIONS[i + 1][0]).unwrap();
    let (a, b) = (SECTIONS[i], SECTIONS[i + 1]);
    let t = (rm - a[0]) / (b[0] - a[0]);
    let l = |k: usize| a[k] + t * (b[k] - a[k]);
    (l(2).to_radians(), l(3), l(4), l(1) * 1e-3)
}

// ---------------------------------------------------------------- one run with the registered stop rules and diagnostics

#[derive(Debug, Clone, Default)]
struct Outcome {
    converged: bool,
    how: String,
    iterations: usize,
    minutes: f64,
    dp_ts_34d: f64,
    dp_tt_34d: f64,
    torque: f64,
    torque_shroud: f64,
    nu_torque: f64,
    nu_dpts: f64,
    w_m: f64,
    w_l: f64,
    dp_tt: f64,
    dp_ts: f64,
    loss: f64,
}

fn series_change(sim: &Simulation, f: &dyn Fn(&[f64]) -> f64, keys: &[&str], window: usize) -> f64 {
    let s: Vec<_> = keys.iter().map(|k| sim.monitors.get(k).cloned()).collect();
    if s.iter().any(|x| x.is_none()) {
        return f64::INFINITY;
    }
    let s: Vec<_> = s.into_iter().map(|x| x.unwrap()).collect();
    let n = s[0].y.len();
    if n <= window {
        return f64::INFINITY;
    }
    let at = |i: usize| f(&s.iter().map(|x| x.y[i]).collect::<Vec<_>>());
    (at(n - 1) - at(n - 1 - window)).abs()
}

fn run(a: &Arm, max_iter: usize) -> Outcome {
    let text = case_toml(a);
    let case = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{}: {e:?}", a.label));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{}: {e:?}", a.label));
    let n_cells = sim.geom.mesh.n_cells();
    let t0 = Instant::now();
    let mut n = 0;
    let mut stop = None;
    while n < max_iter && !sim.is_done() {
        let st = sim.step();
        n += 1;
        if n <= 30 && (n % 10 == 0 || n == 1) {
            println!(
                "W35C {} smoke it {n}: continuity {:.3e} res_u {:.3e} res_p {:.3e}",
                a.label, st.continuity, st.res_u, st.res_p
            );
        }
        if !(st.continuity.is_finite() && st.res_u.is_finite() && st.res_p.is_finite()) || st.continuity > 1e6 {
            stop = Some(format!("diverged: continuity {:.3e} at iteration {n}", st.continuity));
            break;
        }
        if t0.elapsed().as_secs_f64() > 60.0 * WALL_CLOCK_MIN {
            stop = Some(format!("NOT CONVERGED (time): wall-clock cap at iteration {n}"));
            break;
        }
    }
    let minutes = t0.elapsed().as_secs_f64() / 60.0;
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let mon = |k: &str| sim.monitors.last(k).unwrap_or(f64::NAN);
    let om_f = if a.rotating { w } else { 0.0 };
    let m_blade = if a.include_blade { mon("m_blade") } else { 0.0 };
    let torque = -N_BLADES * (m_blade + mon("m_hub"));
    let torque_shroud = -N_BLADES * mon("m_shroud");
    let dp_ts_34d = RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in));
    // ADR 0075 harness Delta p_tt (owner-cell outlet values)
    let g = &sim.geom;
    let own = g.mesh.owner();
    let cc = g.mesh.cell_centres();
    let (ux, uy, uz, p) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
        sim.field("p").unwrap(),
    );
    let phi = sim.flux();
    let (mut num, mut den) = (0.0, 0.0);
    for f in g.mesh.patch("outlet").unwrap().faces() {
        let c = own[f];
        let ua = absolute([ux[c], uy[c], uz[c]], cc[c], om_f);
        num += (p[c] + 0.5 * (ua[0] * ua[0] + ua[1] * ua[1] + ua[2] * ua[2])) * phi[f];
        den += phi[f];
    }
    let dp_tt_34d = RHO * (num / den - (mon("p_in") + 0.5 * u_in * u_in));
    let tkeys: &[&str] = if a.include_blade {
        &["m_blade", "m_hub"]
    } else {
        &["m_hub"]
    };
    let nu_torque = series_change(&sim, &|v| -N_BLADES * v.iter().sum::<f64>(), tkeys, 1000);
    // amendment 1: blade and hub separately (the extension arms add rotating hub length), and the pressure part from the face pressures
    let pb = sim.boundary_field("p").unwrap();
    let (cf, sf) = (g.mesh.face_centres(), g.mesh.face_area_vectors());
    let pressure_part = |name: &str| -> f64 {
        let mut m = 0.0;
        for f in g.mesh.patch(name).unwrap().faces() {
            let pf = RHO * pb[f - g.n_internal];
            m += cf[f][0] * pf * sf[f][1] - cf[f][1] * pf * sf[f][0];
        }
        -N_BLADES * m
    };
    for (name, key) in [("blade", "m_blade"), ("hub", "m_hub"), ("shroud", "m_shroud")] {
        if name == "blade" && !a.include_blade {
            continue;
        }
        println!(
            "W35C {} TORQUE {name}: {:.5} N m (change over 1000 its {:.3e}), pressure part {:.5} N m",
            a.label,
            -N_BLADES * mon(key),
            series_change(&sim, &|v| -N_BLADES * v[0], &[key], 1000),
            pressure_part(name)
        );
    }
    let nu_dpts = series_change(
        &sim,
        &|v| RHO * (v[1] - (v[0] + 0.5 * u_in * u_in)),
        &["p_in", "p_out"],
        1000,
    );
    let (m_in, m_out) = (mon("m_in"), mon("m_out"));
    let mb = (m_in + m_out).abs() / m_in.abs();
    let (converged, how) = if let Some(s) = stop {
        (false, s)
    } else if sim.failure().is_none() && sim.is_converged() {
        (true, format!("residual tolerance reached in {n} iterations"))
    } else if sim.failure().is_none() {
        let ct = nu_torque / torque.abs();
        let cp = nu_dpts / dp_ts_34d.abs().max(1e-300);
        let ok = n >= 8000 && ct < 5e-3 && cp < 5e-3 && mb <= 1e-3;
        (
            ok,
            format!(
                "at {n} iterations: torque change {ct:.3e}, Delta p_ts change {cp:.3e} over 1000 iterations, mass balance {mb:.3e}; last {:?}",
                sim.last()
            ),
        )
    } else {
        (false, format!("failure after {n} iterations: {:?}", sim.failure()))
    };
    println!(
        "W35C {} RESULT: {n_cells} cells, {n} iterations, {minutes:.1} min, converged {converged} ({how})",
        a.label
    );
    println!(
        "W35C {} ADR0075-defs: Delta p_ts {dp_ts_34d:.3} Pa, Delta p_tt {dp_tt_34d:.3} Pa, torque {torque:.5} N m (shroud {torque_shroud:.5}); nu_torque {nu_torque:.3e} N m, nu_dpts {nu_dpts:.3e} Pa",
        a.label
    );
    // ---- energy and angular-momentum balance (ADR 0078 quantities)
    let s_in = patch_samples(&sim, "inlet", om_f);
    let s_out = patch_samples(&sim, "outlet", om_f);
    let s_u = cut_samples(&sim, a.z_le_root - 0.04, om_f);
    let s_d = cut_samples(&sim, a.z_le_root + 0.09, om_f);
    let (i_, o_, u_, d_) = (stats(&s_in), stats(&s_out), stats(&s_u), stats(&s_d));
    for (name, s) in [("inlet", i_), ("cut U", u_), ("cut D", d_), ("outlet", o_)] {
        println!(
            "W35C {} surface {name}: Q {:.6} (cut check {:.2e}), area {:.5}, p_area {:.3}, p_mass {:.3}, pt_mass {:.3}, rho<cz2/2> {:.3}, rho<cth2/2> {:.3}, rho<cr2/2> {:.3}, nonuniform {:.3} Pa, L {:.5} N m, backflow area {:.4} mass {:.4}",
            a.label,
            s.q,
            (s.q - Q_DESIGN).abs() / Q_DESIGN,
            s.area,
            s.p_area,
            s.p_mass,
            s.pt_mass,
            s.kz,
            s.kth,
            s.kr,
            s.nonuni,
            s.l_flux,
            s.back_area,
            s.back_mass
        );
    }
    let q = Q_DESIGN;
    let w_m = torque * w / (RHO * q);
    let w_l = w * (o_.l_flux - i_.l_flux) / (RHO * q);
    let dp_tt = o_.pt_mass - i_.pt_mass;
    let dp_ts = o_.p_area - i_.pt_mass;
    let loss = RHO * w_l - dp_tt;
    let ident = (dp_tt - dp_ts) - ((o_.p_mass - o_.p_area) + o_.kz + o_.kth + o_.kr);
    println!(
        "W35C {} BALANCE: W_M {w_m:.3} J/kg, W_L {w_l:.3} J/kg, psi_M {:.4}, psi_L {:.4}; torque balance dL {:.5} vs M + M_s {:.5} (diff {:.5} N m); exact Delta p_tt {dp_tt:.3}, Delta p_ts {dp_ts:.3}, loss rho W_L - Delta p_tt {loss:.3} Pa; identity residual {ident:.2e}",
        a.label,
        w_m / (w * R_TIP).powi(2),
        w_l / (w * R_TIP).powi(2),
        o_.l_flux - i_.l_flux,
        torque + torque_shroud,
        (o_.l_flux - i_.l_flux) - (torque + torque_shroud)
    );
    println!(
        "W35C {} DECOMP: Delta p_tt - Delta p_ts {:.3} = (p_mass - p_area) {:.3} + rho<cz2/2> {:.3} + rho<cth2/2> {:.3} + rho<cr2/2> {:.3}; of rho<cz2/2>: uniform {:.3}, non-uniformity {:.3}",
        a.label,
        dp_tt - dp_ts,
        o_.p_mass - o_.p_area,
        o_.kz,
        o_.kth,
        o_.kr,
        o_.kz - o_.nonuni,
        o_.nonuni
    );
    // amendment 2: the cuts are checked against the flow through the mesh (the inlet patch), not against Q_DESIGN
    let q_mesh = i_.q;
    let cuts_ok = [u_, d_].iter().all(|s| (s.q - q_mesh).abs() <= 1e-3 * q_mesh);
    if cuts_ok {
        for (name, a0, b0) in [("inlet->U", i_, u_), ("U->D (rotor)", u_, d_), ("D->outlet", d_, o_)] {
            let work = w * (b0.l_flux - a0.l_flux) / q_mesh;
            let dpt = b0.pt_mass - a0.pt_mass;
            println!(
                "W35C {} SEGMENT {name}: work {work:.3} Pa, Delta p_t {dpt:.3} Pa, loss {:.3} Pa",
                a.label,
                work - dpt
            );
        }
    } else {
        println!("W35C {} SEGMENT: a cut fails the mass check, not used", a.label);
    }
    for (r, rct, cz, cth) in bins(&s_d) {
        println!(
            "W35C {} SPAN D r {r:.4}: r c_theta {rct:.5}, c_theta {cth:.3}, c_z {cz:.3}, work Omega r c_theta {:.2} J/kg",
            a.label,
            w * rct
        );
    }
    for (r, _rct, cz, cth) in bins(&s_u) {
        let (g_, m, pp, _) = section_at(r);
        let beta = ((w * r - cth) / cz).atan();
        let metal = g_ + (2.0 * m / pp).atan();
        println!(
            "W35C {} SPAN U r {r:.4}: c_z {cz:.3}, c_theta {cth:.3}, relative inflow angle {:.2} deg, LE metal {:.2} deg, incidence {:.2} deg",
            a.label,
            beta.to_degrees(),
            metal.to_degrees(),
            (beta - metal).to_degrees()
        );
    }
    Outcome {
        converged,
        how,
        iterations: n,
        minutes,
        dp_ts_34d,
        dp_tt_34d,
        torque,
        torque_shroud,
        nu_torque,
        nu_dpts,
        w_m,
        w_l,
        dp_tt,
        dp_ts,
        loss,
    }
}

// ---------------------------------------------------------------- S2: moving_wall accepted with k-omega SST

/// The SST channel example on a first-cell y+ 30 mesh with both walls moving at -10 m/s along x, the walls given as `kind`.
fn s2_channel(kind: &str, model: &str) -> String {
    include_str!("../../../cases/channel/sst_channel.toml")
        .replace("ny = 60", "ny = 12")
        .replace("wall_layers = 16", "wall_layers = 3")
        .replace("target_y_plus = 1.0", "target_y_plus = 30.0")
        .replace("velocity = [17.0, 0.0, 0.0]", "velocity = [7.0, 0.0, 0.0]")
        .replace(
            "[boundaries.ymin]\nvelocity = \"fixed\"",
            &format!(
                "[boundaries.ymin]\nvelocity = \"{kind}\"\nvelocity_value = [-10.0, 0.0, 0.0]\nturbulence = \"wall\""
            ),
        )
        .replace(
            "[boundaries.ymax]\nvelocity = \"fixed\"",
            &format!(
                "[boundaries.ymax]\nvelocity = \"{kind}\"\nvelocity_value = [-10.0, 0.0, 0.0]\nturbulence = \"wall\""
            ),
        )
        .replace("model = \"k_omega_sst\"", &format!("model = \"{model}\""))
}

#[test]
fn s2a_moving_wall_equals_fixed_moving_wall_with_sst() {
    let fields = |kind: &str| {
        let mut sim = Simulation::new(&Case::parse_toml(&s2_channel(kind, "k_omega_sst")).unwrap()).unwrap();
        sim.run(200);
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        ["ux", "uy", "p", "k", "omega"].map(|n| sim.field(n).unwrap())
    };
    let (a, b) = (fields("fixed"), fields("moving_wall"));
    let worst = a
        .iter()
        .zip(&b)
        .flat_map(|(x, y)| x.iter().zip(y).map(|(p, q)| (p - q).abs()))
        .fold(0.0, f64::max);
    println!("S2a: largest difference of u, p, k, omega after 200 iterations {worst:e}");
    assert_eq!(worst, 0.0);
}

#[test]
fn s2b_moving_wall_still_refused_with_k_epsilon() {
    let text = s2_channel("moving_wall", "k_epsilon").replace("omega_initial = 39.5", "epsilon_initial = 3.6");
    let e = match Case::parse_toml(&text) {
        Err(e) => e,
        Ok(c) => Simulation::new(&c).map(|_| ()).unwrap_err(),
    };
    println!("S2b: {e:?}");
    assert!(e.iter().any(|d| d.path.starts_with("boundaries.ymin.")), "{e:?}");
}

// ---------------------------------------------------------------- construction and smoke (ADR 0078 code policy)

#[test]
fn arms_construct_and_count_rules() {
    for a in arms() {
        let case = Case::parse_toml(&case_toml(&a)).unwrap_or_else(|e| panic!("{}: {e:?}", a.label));
        let m = build_mesh(&case).unwrap_or_else(|e| panic!("{}: {e:?}", a.label));
        let q = rcfd_mesh::quality::check_mesh(&m);
        println!(
            "W35C construct {}: n_up {} n_down {} n_gap {} n_chord {} n_side {}: {} cells, max non-orthogonality {:.1} deg, skewness {:.2}, aspect {:.0}",
            a.label,
            a.n_up,
            a.n_down,
            a.n_gap,
            a.n_chord,
            a.n_side,
            m.n_cells(),
            q.max_non_orthogonality,
            q.max_skewness,
            q.max_aspect_ratio
        );
        assert!(q.is_ok(), "{}: {:?}", a.label, q.errors);
    }
    // the count rules keep the largest axial cell at or below the E0 value
    let zb = 0.1 + hub_te_offset();
    let c1 = arm_c1();
    assert!(largest_cell(0.8 - zb, c1.n_down, true) <= largest_cell(0.4 - zb, 8, true));
    assert!(largest_cell(0.8 - zb, c1.n_down - 1, true) > largest_cell(0.4 - zb, 8, true));
    let (fan, st) = hub_station();
    let za = fan.camber_zs(&st, 0.02).0;
    println!(
        "W35C grading downstream (hub layer): E0 ratio {:.3} largest {:.4} m; C0 ({}) ratio {:.3} largest {:.4}; C1 ({}) ratio {:.3} largest {:.4}",
        growth_ratio(0.4 - zb, 8, true),
        largest_cell(0.4 - zb, 8, true),
        arm_c0().n_down,
        growth_ratio(0.4 - zb, arm_c0().n_down, true),
        largest_cell(0.4 - zb, arm_c0().n_down, true),
        c1.n_down,
        growth_ratio(0.8 - zb, c1.n_down, true),
        largest_cell(0.8 - zb, c1.n_down, true)
    );
    println!(
        "W35C grading upstream (hub layer): E0 ratio {:.3} largest {:.4} m; D0 ({}) ratio {:.3} largest {:.4}; D1 ({}) ratio {:.3} largest {:.4}",
        growth_ratio(0.1 + za, 8, false),
        largest_cell(0.1 + za, 8, false),
        arm_d0().n_up,
        growth_ratio(0.1 + za, arm_d0().n_up, false),
        largest_cell(0.1 + za, arm_d0().n_up, false),
        arm_d1().n_up,
        growth_ratio(0.3 + za, arm_d1().n_up, false),
        largest_cell(0.3 + za, arm_d1().n_up, false)
    );
}

/// 30-iteration smoke of every arm (continuity finite); N1 300 iterations with the ADR 0075 stability criterion is a separate test.
#[test]
#[ignore = "smoke of the ADR 0078 arms (about 1 min each)"]
fn smoke_arms() {
    for a in arms() {
        let o = run(&a, 30);
        println!("W35C smoke {}: {}", a.label, o.how);
    }
}

/// N1 scheme selection rule (ADR 0078): 300 iterations, stable = continuity finite and its maximum over 200-300 not above its value at 10.
#[test]
#[ignore = "N1 scheme stability smoke (300 iterations per scheme)"]
fn smoke_n1_scheme_stability() {
    for scheme in ["tvd_van_leer", "tvd_minmod"] {
        let a = Arm {
            convection: scheme,
            ..e0()
        };
        let case = Case::parse_toml(&case_toml(&a)).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        let mut c10 = f64::NAN;
        let mut cmax = 0.0f64;
        let mut finite = true;
        for it in 1..=300 {
            let st = sim.step();
            if !st.continuity.is_finite() {
                finite = false;
                break;
            }
            if it == 10 {
                c10 = st.continuity;
            }
            if it >= 200 {
                cmax = cmax.max(st.continuity);
            }
        }
        let stable = finite && sim.failure().is_none() && cmax <= c10;
        println!("W35C N1 smoke {scheme}: continuity at 10 {c10:.3e}, max over 200-300 {cmax:.3e}, stable {stable}");
        if stable {
            break;
        }
    }
}

// ---------------------------------------------------------------- the registered runs (one per arm; about 16-45 min each)

fn registered(label: &str) {
    let a = arm(label);
    let o = run(&a, 8000);
    let m_meas = POWER_MEAS / omega();
    println!(
        "W35C {label} SUMMARY: converged {}, Delta p_ts {:.3} (exact {:.3}) Pa, Delta p_tt {:.3} (exact {:.3}) Pa, torque {:.5} N m ({:+.1} % of measured), W_M {:.2}, W_L {:.2} J/kg, loss {:.3} Pa, nu_torque {:.3e}, nu_dpts {:.3e}, {} its, {:.1} min",
        o.converged,
        o.dp_ts_34d,
        o.dp_ts,
        o.dp_tt_34d,
        o.dp_tt,
        o.torque,
        100.0 * (o.torque / m_meas - 1.0),
        o.w_m,
        o.w_l,
        o.loss,
        o.nu_torque,
        o.nu_dpts,
        o.iterations,
        o.minutes
    );
    println!(
        "W35C {label} GAPS: Delta p_ts gap {:.3} Pa, torque gap {:.5} N m, shroud torque {:.5} N m",
        DP_MEAS - o.dp_ts_34d,
        m_meas - o.torque,
        o.torque_shroud
    );
    if label == "E0" {
        // reproduction rule: attempt 3 Delta p_ts -3.8 Pa (printed to 0.1), torque 1.320 N m
        println!(
            "W35C E0 reproduction: Delta p_ts {:.3} vs -3.8 (0.2 Pa), torque {:.5} vs 1.320 (0.1 %)",
            o.dp_ts_34d, o.torque
        );
    }
}

#[test]
#[ignore = "ADR 0078 E0 (about 16 min)"]
fn w35c_e0_base() {
    registered("E0");
}
#[test]
#[ignore = "ADR 0078 E1 (about 16 min)"]
fn w35c_e1_projected_casing() {
    registered("E1");
}
#[test]
#[ignore = "ADR 0078 C1 (about 17 min)"]
fn w35c_c1_outlet_extended() {
    registered("C1");
}
#[test]
#[ignore = "ADR 0078 C2 (about 16 min)"]
fn w35c_c2_outlet_no_reverse() {
    registered("C2");
}
#[test]
#[ignore = "ADR 0078 C3 (about 16 min)"]
fn w35c_c3_outlet_radial_equilibrium() {
    registered("C3");
}
#[test]
#[ignore = "ADR 0078 D1 (about 17 min)"]
fn w35c_d1_inlet_extended() {
    registered("D1");
}
#[test]
#[ignore = "ADR 0078 D2 (about 20 min)"]
fn w35c_d2_tip_gap() {
    registered("D2");
}
#[test]
#[ignore = "ADR 0078 amendment 1 N1b (about 16 min)"]
fn w35c_n1b_tvd_minmod() {
    registered("N1b");
}
#[test]
#[ignore = "ADR 0078 N2 (about 45 min)"]
fn w35c_n2_refined() {
    registered("N2");
}
#[test]
#[ignore = "ADR 0078 amendment 1 C0 (about 20 min)"]
fn w35c_c0_downstream_grading() {
    registered("C0");
}
#[test]
#[ignore = "ADR 0078 amendment 1 D0 (about 20 min)"]
fn w35c_d0_upstream_grading() {
    registered("D0");
}
#[test]
#[ignore = "ADR 0078 amendment 2 X2a (about 16 min)"]
fn w35c_x2a_empty_passage_rotating_frame() {
    registered("X2a");
}
#[test]
#[ignore = "ADR 0078 amendment 2 X2b (about 16 min)"]
fn w35c_x2b_empty_passage_inertial() {
    registered("X2b");
}
#[test]
#[ignore = "ADR 0078 amendment 2 X2c (about 30 min)"]
fn w35c_x2c_empty_passage_rotating_refined() {
    registered("X2c");
}
#[test]
#[ignore = "ADR 0078 amendment 2 X2d (about 16 min)"]
fn w35c_x2d_empty_passage_minmod() {
    registered("X2d");
}
#[test]
#[ignore = "ADR 0078 amendment 2 X2e (about 16 min)"]
fn w35c_x2e_empty_passage_linear_upwind() {
    registered("X2e");
}
#[test]
#[ignore = "ADR 0078 amendment 2: X2e scheme stability smoke (300 iterations)"]
fn smoke_x2e_stability() {
    let a = arm("X2e");
    let case = Case::parse_toml(&case_toml(&a)).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    let (mut c10, mut cmax, mut finite) = (f64::NAN, 0.0f64, true);
    for it in 1..=300 {
        let st = sim.step();
        if !st.continuity.is_finite() {
            finite = false;
            break;
        }
        if it == 10 {
            c10 = st.continuity;
        }
        if it >= 200 {
            cmax = cmax.max(st.continuity);
        }
    }
    println!(
        "W35C X2e smoke: continuity at 10 {c10:.3e}, max over 200-300 {cmax:.3e}, stable {}",
        finite && sim.failure().is_none() && cmax <= c10
    );
}
#[test]
#[ignore = "ADR 0078 K (about 25 min)"]
fn w35c_k_combined() {
    registered("K");
}

// ---------------------------------------------------------------- arm E: blade-element estimate (no CFD)

/// Thin-airfoil zero-lift angle of a NACA 4-digit camber line, `-(1/pi) int_0^pi dyc/dx (cos t - 1) dt`, `x = (1 - cos t)/2`.
fn alpha_l0(m: f64, p: f64) -> f64 {
    let n = 20000;
    let h = PI / n as f64;
    let s: f64 = (0..n)
        .map(|i| {
            let t = (i as f64 + 0.5) * h;
            let (_, d) = camber_line(SectionFamily::Naca4, m, p, 0.5 * (1.0 - t.cos()));
            d * (t.cos() - 1.0) * h
        })
        .sum();
    -s / PI
}

/// Per-station blade-element result: (r, Delta c_theta, alpha deg, C_L).
type ElementStation = (f64, f64, f64, f64);

/// Blade-element estimate at volume flow `q`: (mass-averaged work J/kg, torque N m, per-station results).
fn blade_element(q: f64, cl_cap: f64, cd: f64) -> (f64, f64, Vec<ElementStation>) {
    let w = omega();
    let cx = q / annulus_area();
    let mut st = Vec::new();
    for s in SECTIONS {
        let r = s[0] * 1e-3;
        let (c, g, m, p) = (s[1] * 1e-3, s[2].to_radians(), s[3], s[4]);
        let (u, pitch, a0) = (w * r, 2.0 * PI * r / N_BLADES, alpha_l0(m, p));
        let mut dc = 0.0;
        let (mut alpha, mut cl) = (0.0, 0.0);
        for _ in 0..10000 {
            let wt = u - 0.5 * dc;
            let wi = cx.hypot(wt);
            let b = wt.atan2(cx);
            alpha = b - g;
            cl = (2.0 * PI * (alpha - a0)).min(cl_cap);
            // F_theta = L cos b + D sin b = rho s c_x Delta c_theta
            let ft = 0.5 * wi * wi * c * (cl * b.cos() + cd * b.sin());
            let new = ft / (pitch * cx);
            let done = (new - dc).abs() < 1e-12;
            dc = 0.5 * (dc + new);
            if done {
                break;
            }
        }
        st.push((r, dc, alpha.to_degrees(), cl));
    }
    // trapezoid over the blade span; no work in the tip gap
    let (mut num, mut mom) = (0.0, 0.0);
    for p in st.windows(2) {
        let (a, b) = (p[0], p[1]);
        let dr = b.0 - a.0;
        num += 0.5 * (w * a.0 * a.1 * a.0 + w * b.0 * b.1 * b.0) * dr * 2.0 * PI * cx;
        mom += 0.5 * (a.0 * a.1 * a.0 + b.0 * b.1 * b.0) * dr * 2.0 * PI * cx * RHO;
    }
    (num / q, mom, st)
}

#[test]
fn arm_e_blade_element_estimate() {
    // self-check (ADR 0078 E step 4): parabolic camber (p = 0.5) has alpha_L0 = -2 m exactly
    let e = (alpha_l0(0.0376, 0.5) + 2.0 * 0.0376).abs();
    println!("W35C E self-check: |alpha_L0 + 2 m| = {e:.2e}");
    assert!(e <= 1e-8, "{e:e}");
    let ut2 = (omega() * R_TIP).powi(2);
    for (q, power) in [(Q_DESIGN, POWER_MEAS), (1.8039, 292.9)] {
        for (name, cap, cd) in [
            ("EA inviscid linear", f64::INFINITY, 0.0),
            ("EB C_L <= 1.1, C_D = 0.02", 1.1, 0.02),
        ] {
            let (y, m, st) = blade_element(q, cap, cd);
            println!(
                "W35C E {name} Q {q}: work {y:.1} J/kg (psi {:.4}), ideal rho Y {:.1} Pa, torque {m:.3} N m; measured shaft {:.1} J/kg, {:.3} N m",
                y / ut2,
                RHO * y,
                power / (RHO * q),
                power / omega()
            );
            for s in st.iter().step_by(3) {
                println!(
                    "W35C E {name} Q {q} r {:.4}: Delta c_theta {:.3} m/s, r Delta c_theta {:.5}, alpha {:.2} deg, C_L {:.3}",
                    s.0,
                    s.1,
                    s.0 * s.1,
                    s.2,
                    s.3
                );
            }
            assert!(y.is_finite() && y > 0.0);
        }
    }
}
