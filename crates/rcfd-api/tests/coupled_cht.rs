//! Solved flow with conjugate solid zones through the settings tree (F-032 and F-051, ADR 0029 checks C3, D1 to D4 and
//! E1 to E7). The limits are those registered in docs/adr/0029-wave19a-coupled.md before this file was written; none was
//! changed afterwards.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

fn build(text: &str) -> Simulation {
    Simulation::new(&case(text)).unwrap_or_else(|e| panic!("{e:?}"))
}

fn errors(text: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => match Simulation::new(&c) {
            Err(e) => e,
            Ok(_) => vec![],
        },
    }
}

fn has(errs: &[Diagnostic], path: &str) -> bool {
    errs.iter().any(|d| d.path == path)
}

fn patch_area(s: &Simulation, patch: &str) -> f64 {
    let sf = s.geom.mesh.face_area_vectors();
    s.geom
        .mesh
        .patch(patch)
        .unwrap()
        .faces()
        .map(|f| vec3::norm(sf[f]))
        .sum()
}

fn run_to_end(s: &mut Simulation) {
    while !s.is_done() {
        s.step();
    }
    assert!(s.failure().is_none(), "{:?}", s.failure());
}

// ------------------------------------------------------------------------------------------------ C3 cavity

fn cavity_text(nx: usize, lx: f64, solids: bool, p_ref: usize) -> String {
    let mut t = format!(
        r#"
title = "cavity with solid walls"
[mesh]
generator = "box_2d"
nx = {nx}
ny = 50
lx = {lx}
ly = 1.0
[physics]
nu = 0.71
rho = 1.0
cp = 1.0
conductivity = 1.0
thermal_expansion = 1.0
gravity = [0.0, -710.0, 0.0]
reference_temperature = 0.5
[energy]
enabled = true
buoyancy = true
relax = 0.9
[initial]
temperature = 0.5
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "coupled"
convection = "central"
alpha_u = 0.9
tolerance_u = 1e-10
tolerance_p = 1e-10
pressure_rel_tol = 1e-10
max_iterations = 12000
p_ref_cell = {p_ref}
[reports.q_in]
type = "heat_flow"
patch = "xmin"
[reports.q_out]
type = "heat_flow"
patch = "xmax"
"#
    );
    if solids {
        t += r#"
[solids.left]
zone = "box"
box_min = [-10.0, -10.0, -10.0]
box_max = [0.1, 10.0, 10.0]
conductivity = 1e4
density = 1.0
specific_heat = 1.0
[solids.right]
zone = "box"
box_min = [1.1, -10.0, -10.0]
box_max = [10.0, 10.0, 10.0]
conductivity = 1e4
density = 1.0
specific_heat = 1.0
"#;
    }
    t
}

fn nearest_cell(nx: usize, ny: usize, lx: f64, x: f64, y: f64) -> usize {
    let mesh = rcfd_mesh::gen::box_2d(nx, ny, lx, 1.0, false, false).unwrap();
    let cc = mesh.cell_centres();
    (0..cc.len())
        .min_by(|&a, &b| {
            let da = (cc[a][0] - x).powi(2) + (cc[a][1] - y).powi(2);
            let db = (cc[b][0] - x).powi(2) + (cc[b][1] - y).powi(2);
            da.partial_cmp(&db).unwrap()
        })
        .unwrap()
}

#[test]
fn c3_cavity_with_high_conductivity_solid_walls() {
    let pref = nearest_cell(60, 50, 1.2, 0.6, 0.5);
    let mut s = build(&cavity_text(60, 1.2, true, pref));
    run_to_end(&mut s);
    let (qin, qout) = (s.monitors.last("q_in").unwrap(), s.monitors.last("q_out").unwrap());
    let a = patch_area(&s, "xmin");
    let nu_solid = -qin / a;
    let mut n = build(&cavity_text(50, 1.0, false, 0));
    run_to_end(&mut n);
    let nu_plain = -n.monitors.last("q_in").unwrap() / patch_area(&n, "xmin");
    // interface heat flow at x = 0.1 (uniform mesh, face conductivity: harmonic mean of the two cells)
    let t = s.field("T").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let (own, nei) = (s.geom.mesh.owner(), s.geom.mesh.neighbour());
    let sf = s.geom.mesh.face_area_vectors();
    let mut q_if = 0.0;
    for f in 0..s.geom.n_internal {
        let (o, nb) = (own[f], nei[f]);
        let (xo, xn) = (cc[o][0], cc[nb][0]);
        if (xo - 0.09).abs() < 1e-9 && (xn - 0.11).abs() < 1e-9 || (xn - 0.09).abs() < 1e-9 && (xo - 0.11).abs() < 1e-9
        {
            // heat flow in the -x direction (towards xmin) is out of the left solid; the left solid cell is the lower x
            let (left, right) = if xo < xn { (o, nb) } else { (nb, o) };
            let k = 2.0 * 1e4 * 1.0 / (1e4 + 1.0);
            let d = cc[right][0] - cc[left][0];
            q_if += k * (t[right] - t[left]) / d * vec3::norm(sf[f]);
        }
    }
    // heat flowing in +x through the interface (q_if is computed towards -x, positive when the right cell is hotter) equals
    // the heat entering through xmin (the report is the heat flow out of the domain, negative at the heated wall)
    let rel_if = (q_if - qin).abs() / qin.abs();
    let bal = (qin + qout).abs() / qin.abs();
    let (ux, uy) = (s.field("ux").unwrap(), s.field("uy").unwrap());
    let solid_cells: Vec<usize> = (0..cc.len()).filter(|&c| cc[c][0] < 0.1 || cc[c][0] > 1.1).collect();
    let solid_zero = solid_cells.iter().all(|&c| ux[c] == 0.0 && uy[c] == 0.0);
    let flux = s.flux();
    let mut leak: f64 = 0.0;
    for f in 0..s.geom.mesh.n_faces() {
        let touches = |c: usize| cc[c][0] < 0.1 || cc[c][0] > 1.1;
        if touches(own[f]) || (f < s.geom.n_internal && touches(nei[f])) {
            leak = leak.max(flux[f].abs());
        }
    }
    println!(
        "C3: coupled {} its (converged {}); Nu {nu_solid:.4} vs 1.118 ({:+.2}% , limit 2%); no-solid Nu {nu_plain:.4}, difference {:.2e} (limit 1e-2); |Q_xmin+Q_xmax|/|Q_xmin| {bal:.2e} (1e-9); interface vs Q_xmin {rel_if:.2e} (1e-9); solid u==0 {solid_zero}; max |phi| on solid faces {leak:.1e} (== 0)",
        s.iteration(),
        s.is_converged(),
        100.0 * (nu_solid / 1.118 - 1.0),
        (nu_solid - nu_plain).abs() / nu_plain
    );
    assert!((nu_solid / 1.118 - 1.0).abs() <= 2e-2);
    assert!((nu_solid - nu_plain).abs() / nu_plain <= 1e-2);
    assert!(bal <= 1e-9);
    assert!(rel_if <= 1e-9);
    assert!(solid_zero && leak == 0.0);
}

// ------------------------------------------------------------------------------------------------ D duct

const EXACT_T: &str = "300 [K] + 1 [K/m] * x + if(y < 1 [m], 1.5 [K] * ((y / 1 [m])^2 / 2 - (y / 1 [m])^4 / 12), 0.625 [K] + 0.1 [K/m] * (y - 1 [m]))";

fn duct_text(nf: usize) -> String {
    let ny = nf + nf / 2;
    format!(
        r#"
title = "solved-flow fluid-solid duct"
[mesh]
generator = "box_2d"
nx = 8
ny = {ny}
lx = 2.0
ly = 1.5
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = 1.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 301.0
[boundaries.xmin]
velocity = "fixed"
velocity_expression_x = "if(y < 1 [m], 1.5 [m/s] * (1 - (y / 1 [m])^2), 0 [m/s])"
temperature = "fixed"
temperature_expression = "{EXACT_T}"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
temperature = "fixed"
temperature_expression = "{EXACT_T}"
[boundaries.ymin]
velocity = "symmetry"
[boundaries.ymax]
velocity = "fixed"
temperature = "heat_flux"
heat_flux_value = 1.0
[solver]
algorithm = "coupled"
convection = "central"
alpha_u = 0.9
tolerance_u = 1e-10
tolerance_p = 1e-10
pressure_rel_tol = 1e-10
max_iterations = 3000
[solids.wall]
zone = "box"
box_min = [-10.0, 1.0, -10.0]
box_max = [10.0, 10.0, 10.0]
conductivity = 10.0
density = 4.0
specific_heat = 1.0
"#
    )
}

struct Duct {
    s: Simulation,
    nf: usize,
}

fn duct(nf: usize) -> Duct {
    let mut s = build(&duct_text(nf));
    run_to_end(&mut s);
    Duct { s, nf }
}

impl Duct {
    fn dy(&self) -> f64 {
        1.0 / self.nf as f64
    }
    /// Cells of the column nearest x = 1 (mid-length).
    fn mid_column(&self) -> Vec<usize> {
        let cc = self.s.geom.mesh.cell_centres();
        let xc = cc.iter().map(|c| c[0]).fold(
            f64::INFINITY,
            |a, b| if (b - 1.0).abs() < (a - 1.0).abs() { b } else { a },
        );
        (0..cc.len()).filter(|&i| (cc[i][0] - xc).abs() < 1e-9).collect()
    }
}

/// (outer surface minus bulk, wall-to-bulk difference from the fluid side) at mid-length.
fn duct_temperatures(d: &Duct) -> (f64, f64) {
    let t = d.s.field("T").unwrap();
    let ux = d.s.field("ux").unwrap();
    let cc = d.s.geom.mesh.cell_centres();
    let (mut num, mut den) = (0.0, 0.0);
    let (mut top, mut top_y) = (f64::NAN, f64::NEG_INFINITY);
    let (mut tp, mut tp_y) = (f64::NAN, f64::NEG_INFINITY);
    for i in d.mid_column() {
        let y = cc[i][1];
        if y < 1.0 {
            num += ux[i] * d.dy() * t[i];
            den += ux[i] * d.dy();
            if y > tp_y {
                tp_y = y;
                tp = t[i];
            }
        }
        if y > top_y {
            top_y = y;
            top = t[i] + 1.0 / 10.0 * 0.5 * d.dy();
        }
    }
    let tb = num / den;
    (top - tb, tp + 1.0 * d.dy() / 2.0 - tb)
}

#[test]
fn d1_delta_t_total_at_two_resolutions() {
    let exact = 17.0 / 35.0 + 0.5 / 10.0;
    for nf in [16, 32] {
        let d = duct(nf);
        let (dt, _) = duct_temperatures(&d);
        let rel = (dt - exact).abs() / exact;
        println!(
            "D1 n_f={nf}: {} its (converged {}), dT_tot {dt:.6} vs {exact:.6}, relative error {rel:.3e} (limit 1e-2)",
            d.s.iteration(),
            d.s.is_converged()
        );
        assert!(rel <= 1e-2, "D1 n_f={nf}: {rel}");
    }
}

#[test]
fn d2_nusselt_number() {
    let d = duct(32);
    let (_, dtw) = duct_temperatures(&d);
    let nu = 4.0 * 1.0 / dtw;
    let want = 140.0 / 17.0;
    println!(
        "D2 Nu {nu:.4} vs {want:.4}, relative error {:.3e} (limit 2e-2)",
        (nu - want).abs() / want
    );
    assert!((nu - want).abs() / want <= 2e-2);
}

#[test]
fn d3_velocity_and_mass_flow() {
    let d = duct(32);
    let (ux, uy) = (d.s.field("ux").unwrap(), d.s.field("uy").unwrap());
    let cc = d.s.geom.mesh.cell_centres();
    let umax = 1.5;
    let (mut eu, mut ev) = (0.0f64, 0.0f64);
    for i in d.mid_column() {
        if cc[i][1] < 1.0 {
            eu = eu.max((ux[i] - 1.5 * (1.0 - cc[i][1] * cc[i][1])).abs());
            ev = ev.max(uy[i].abs());
        }
    }
    let solid_zero = (0..cc.len())
        .filter(|&i| cc[i][1] > 1.0)
        .all(|i| ux[i] == 0.0 && uy[i] == 0.0);
    let flux = d.s.flux();
    let mesh = &d.s.geom.mesh;
    let sum = |p: &str| mesh.patch(p).unwrap().faces().map(|f| flux[f]).sum::<f64>();
    let (qin, qout) = (-sum("xmin"), sum("xmax"));
    let scale = qin; // U H per unit depth times the depth
    println!(
        "D3: u vs parabola {:.2e} U_max (limit 1e-2), |v| {:.2e} U_max (limit 1e-3), solid exactly zero {solid_zero}, inlet {qin:.12e} outlet {qout:.12e}, difference {:.2e} U H (limit 1e-10)",
        eu / umax,
        ev / umax,
        (qin - qout).abs() / scale
    );
    assert!(eu <= 1e-2 * umax && ev <= 1e-3 * umax);
    assert!(solid_zero);
    assert!((qin - qout).abs() / scale <= 1e-10);
}

fn d4_balances() -> (f64, f64, f64, f64) {
    let d = duct(32);
    let s = &d.s;
    let mesh = &s.geom.mesh;
    let (cc, fcs, own, sf) = (
        mesh.cell_centres(),
        mesh.face_centres(),
        mesh.owner(),
        mesh.face_area_vectors(),
    );
    let t = s.field("T").unwrap();
    let tb = s.boundary_field("T").unwrap();
    let flux = s.flux();
    let n_int = s.geom.n_internal;
    let q = 1.0;
    let lx_depth = patch_area(s, "ymax"); // L_x times the depth: the scale q L_x
    let kcell = |c: usize| if cc[c][1] > 1.0 { 10.0 } else { 1.0 };
    // per patch, split into solid and fluid rows: (enthalpy out, conduction out)
    let patch = |p: &str, solid: bool| {
        let (mut h, mut c) = (0.0, 0.0);
        for f in mesh.patch(p).unwrap().faces() {
            let o = own[f];
            if (cc[o][1] > 1.0) != solid {
                continue;
            }
            let a = vec3::norm(sf[f]);
            let dn = (vec3::dot(vec3::sub(fcs[f], cc[o]), sf[f]) / a).abs();
            h += flux[f] * tb[f - n_int];
            c += kcell(o) * (t[o] - tb[f - n_int]) / dn * a;
        }
        (h, c)
    };
    let (xi_f, xo_f) = (patch("xmin", false), patch("xmax", false));
    let (xi_s, xo_s) = (patch("xmin", true), patch("xmax", true));
    // interface: heat into the fluid through the faces between the solid cell above and the fluid cell below
    let nei = mesh.neighbour();
    let mut q_if = 0.0;
    for f in 0..n_int {
        let (a, b) = (own[f], nei[f]);
        if (cc[a][1] > 1.0) != (cc[b][1] > 1.0) {
            let (sol, flu) = if cc[a][1] > 1.0 { (a, b) } else { (b, a) };
            let k = 2.0 * 10.0 * 1.0 / 11.0;
            q_if += k * (t[sol] - t[flu]) / (cc[sol][1] - cc[flu][1]) * vec3::norm(sf[f]);
        }
    }
    let scale = q * lx_depth;
    let whole = xi_f.0 + xo_f.0 + xi_s.0 + xo_s.0 + xi_f.1 + xo_f.1 + xi_s.1 + xo_s.1 - q * lx_depth;
    let solid_literal = q * lx_depth - q_if;
    let solid_full = q * lx_depth - q_if - xi_s.1 - xo_s.1;
    let fluid = q_if - (xi_f.0 + xo_f.0) - (xi_f.1 + xo_f.1);
    println!(
        "D4: whole {:.2e}, solid (registered form, no end conduction) {:.2e}, solid (with end conduction {:.2e}) {:.2e}, fluid {:.2e} of q L_x (limit 1e-9)",
        whole.abs() / scale,
        solid_literal.abs() / scale,
        (xi_s.1 + xo_s.1) / scale,
        solid_full.abs() / scale,
        fluid.abs() / scale
    );
    (
        whole.abs() / scale,
        solid_literal.abs() / scale,
        solid_full.abs() / scale,
        fluid.abs() / scale,
    )
}

/// Balances (a), (c) as registered and (b) with the conduction through the solid rows of the inlet and outlet patches
/// included (a corrected statement written after the registered form missed, see the next test and ADR 0029 addendum).
#[test]
fn d4_energy_balances() {
    let (whole, _, solid_full, fluid) = d4_balances();
    assert!(whole <= 1e-9);
    assert!(solid_full <= 1e-9);
    assert!(fluid <= 1e-9);
}

/// Registered D4(b) as written: heat in through ymax equals the heat passing the interface, `<= 1e-9 q L_x`. Missed: the
/// registered statement left out the conduction through the solid rows of the fixed-temperature inlet and outlet, which is
/// not zero in the discrete solution (the net is about 2.6e-5 q L_x); with it the balance closes to round-off.
#[test]
#[ignore = "known miss: registered form omits the solid end conduction (ADR 0029 addendum)"]
fn d4_solid_registered_form_known_miss() {
    let (_, solid_literal, _, _) = d4_balances();
    assert!(solid_literal <= 1e-9, "{solid_literal}");
}

// ------------------------------------------------------------------------------------------------ E errors

#[test]
fn e1_solids_need_the_coupled_algorithm_or_a_frozen_flow() {
    let simple = duct_text(8).replace("algorithm = \"coupled\"", "algorithm = \"simple\"");
    let e = errors(&simple);
    assert!(has(&e, "solver.frozen_flow"), "{e:?}");
    assert!(e.iter().any(|d| d.message.contains("coupled")), "{e:?}");
    let frozen = duct_text(8).replace("max_iterations = 3000", "max_iterations = 3000\nfrozen_flow = true");
    let e = errors(&frozen);
    assert!(has(&e, "solver.frozen_flow"), "{e:?}");
}

#[test]
fn e2_turbulence_and_property_laws_are_rejected() {
    let e = errors(&(duct_text(8) + "[turbulence]\nmodel = \"k_epsilon\"\n"));
    assert!(has(&e, "turbulence.model"), "{e:?}");
    let e = errors(&(duct_text(8) + "[properties.conductivity]\nlaw = \"polynomial\"\ncoefficients = [1.0, 0.0]\n"));
    assert!(e.iter().any(|d| d.path.starts_with("properties")), "{e:?}");
}

#[test]
fn e3_reference_cell_in_a_solid() {
    // closed domain, default p_ref_cell = 0 lies in the left solid
    let e = errors(&cavity_text(60, 1.2, true, 0));
    assert!(has(&e, "solver.p_ref_cell"), "{e:?}");
}

#[test]
fn e4_fluid_split_by_a_solid_slab() {
    let mut t = cavity_text(30, 1.0, false, 0);
    t.push_str("[solids.slab]\nzone = \"box\"\nbox_min = [0.4, -10.0, -10.0]\nbox_max = [0.6, 10.0, 10.0]\nconductivity = 1.0\ndensity = 1.0\nspecific_heat = 1.0\n");
    let e = errors(&t);
    assert!(has(&e, "solids.slab.zone"), "{e:?}");
}

#[test]
fn e5_inflow_through_a_solid_cell() {
    let t = duct_text(8).replace(
        "velocity_expression_x = \"if(y < 1 [m], 1.5 [m/s] * (1 - (y / 1 [m])^2), 0 [m/s])\"",
        "velocity_value = [1.0, 0.0, 0.0]",
    );
    let e = errors(&t);
    assert!(has(&e, "boundaries.xmin.velocity"), "{e:?}");
}

#[test]
fn e6_open_face_without_fixed_pressure() {
    let t = duct_text(8).replace(
        "pressure = \"fixed\"\npressure_value = 0.0\n",
        "pressure = \"zero_gradient\"\n",
    );
    let e = errors(&t);
    assert!(has(&e, "boundaries.xmax.pressure"), "{e:?}");
}

#[test]
fn e7_periodic_and_zone_sources_are_still_diagnosed() {
    let periodic = format!(
        "{}\n",
        cavity_text(8, 1.0, false, 0).replace("ny = 50", "ny = 8\nperiodic_x = true")
    );
    let e = errors(&periodic);
    assert!(!e.is_empty(), "periodic coupled must be diagnosed");
    println!("E7 periodic: {:?}", e.first().map(|d| (&d.path, &d.message)));
    let zs = duct_text(8) + "[zone_sources.q]\ntarget = \"temperature\"\nvalue = 1.0\nzone = \"all\"\n";
    let e = errors(&zs);
    assert!(e.iter().any(|d| d.path.starts_with("zone_sources.q")), "{e:?}");
}
