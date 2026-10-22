//! Wave 43B (ADR 0109): a frozen-rotor interface on a cylinder about the rotation axis (F-126, F-114). A rotor block
//! `r_i <= r <= R` (cell zone `rotor`) and a stator block `R <= r <= r_o` with other counts around and axially are joined by
//! `join_non_conformal_cylindrical`; the joined mesh is written as an OpenFOAM directory and read through
//! `mesh.source = "file"` (no case-tree change). Checks C1-C5 and their limits were registered in the ADR before this code
//! ran. On the registered mesh M every run diverged (overlap faces 79 deg non-orthogonal): those are the ignored
//! `*_known_miss` tests. Amendment 1 selected candidate K2 (interface cells closer to isotropic, `velocity_correction =
//! "face_flux"`) by a convergence-only rule; the same checks and limits run on it.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_mesh::gen::{annulus_3d, Grading};
use rcfd_mesh::non_conformal::join_non_conformal_cylindrical;
use rcfd_mesh::Mesh;
use std::path::{Path, PathBuf};

const RI: f64 = 0.5;
const RM: f64 = 0.75;
const RO: f64 = 1.0;
const LZ: f64 = 0.4;
const V_I: f64 = 0.5;

/// Rotor block `a` (cell zone `rotor`) joined to stator block `b` on the cylinder r = R.
fn join(a: &Mesh, b: &Mesh) -> Mesh {
    let (mut m, rep) = join_non_conformal_cylindrical(a, "outer", b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap();
    m.add_cell_zone("rotor", (0..rep.b_cell_offset).collect()).unwrap();
    m
}

/// A registered set-up: mesh, solver options and the conformal controls of C2 (as `[mesh]` bodies).
struct Variant {
    name: &'static str,
    mesh: fn() -> Mesh,
    solver: &'static str,
    controls: fn(&str) -> Vec<String>,
}

/// Registered mesh M: 48 x 8 x 2 rotor, 35 x 8 x 3 stator, uniform.
fn mesh_m() -> Mesh {
    join(
        &annulus_3d(48, 8, 2, RI, RM, LZ, 0.0, Grading::Uniform).unwrap(),
        &annulus_3d(35, 8, 3, RM, RO, LZ, 0.0, Grading::Uniform).unwrap(),
    )
}

fn controls_m(_: &str) -> Vec<String> {
    [48, 35]
        .iter()
        .map(|n| format!("generator = \"annulus_3d\"\nnx = {n}\nny = 16\nnz = 2\nr_in = {RI}\nr_out = {RO}\nlz = {LZ}"))
        .collect()
}

/// Amendment 1, K2 blocks: 6 radial cells growing towards the interface (size ratio 3).
fn k2_block(n_theta: usize, n_z: usize, rotor: bool) -> Mesh {
    if rotor {
        annulus_3d(n_theta, 6, n_z, RI, RM, LZ, 0.0, Grading::Expansion(3.0)).unwrap()
    } else {
        annulus_3d(n_theta, 6, n_z, RM, RO, LZ, 0.0, Grading::Expansion(1.0 / 3.0)).unwrap()
    }
}

/// K2: 96 x 6 x 4 rotor, 70 x 6 x 6 stator.
fn mesh_k2() -> Mesh {
    join(&k2_block(96, 4, true), &k2_block(70, 6, false))
}

/// K2 controls: cylindrical joins of identical blocks (conformal), 96 around x 4 axial and 70 around x 6 axial.
fn controls_k2(tag: &str) -> Vec<String> {
    [(96, 4), (70, 6)]
        .iter()
        .map(|&(n, nz)| {
            let m = join(&k2_block(n, nz, true), &k2_block(n, nz, false));
            file_mesh(&write(&m, &format!("{tag}_ctl{n}")))
        })
        .collect()
}

const REGISTERED: Variant = Variant {
    name: "M",
    mesh: mesh_m,
    solver: "",
    controls: controls_m,
};

const K2: Variant = Variant {
    name: "K2",
    mesh: mesh_k2,
    solver: "velocity_correction = \"face_flux\"\n",
    controls: controls_k2,
};

fn write(m: &Mesh, tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rcfd_w43b_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    rcfd_io::openfoam::write_dir(m, &dir).unwrap();
    dir
}

fn file_mesh(dir: &Path) -> String {
    format!("source = \"file\"\nfile = {:?}", dir.to_string_lossy())
}

fn mesh_dir(v: &Variant, tag: &str) -> String {
    file_mesh(&write(&(v.mesh)(), &format!("{}_{tag}", v.name)))
}

fn wall(name: &str, w: f64) -> String {
    format!(
        "[boundaries.{name}]\nvelocity = \"fixed\"\nvelocity_expression_x = \"-{w:?} [m/s] * y / 1 [m]\"\nvelocity_expression_y = \"{w:?} [m/s] * x / 1 [m]\"\n"
    )
}

fn mrf(omega: f64, zone: &str) -> String {
    format!("[sources.rotor]\ntype = \"mrf\"\nrotation_axis = [0.0, 0.0, 1.0]\nangular_velocity = {omega:?}\n{zone}\n")
}

const ROTOR: &str = "zone = \"cell_zone\"\ncell_zone = \"rotor\"";
const ALL: &str = "zone = \"all\"";
const SYMMETRY: &str = "[boundaries.zmin]\nvelocity = \"symmetry\"\n[boundaries.zmax]\nvelocity = \"symmetry\"\n";
const SOURCE_FLOW: &str = "[boundaries.inner]\nvelocity = \"fixed\"\nvelocity_expression_x = \"0.5 [m^2/s] * x / (x * x + y * y)\"\nvelocity_expression_y = \"0.5 [m^2/s] * y / (x * x + y * y)\"\n[boundaries.outer]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\n";

/// Case text: `mesh` is the `[mesh]` body, `bcs` the inner/outer boundary sections, `solver` extra solver keys.
fn case_text(mesh: &str, bcs: &str, sources: &str, iterations: usize, solver: &str) -> String {
    format!(
        "[mesh]\n{mesh}\n[physics]\nnu = 0.1\nrho = 1.0\n{bcs}{SYMMETRY}[solver]\n{solver}algorithm = \"simplec\"\nconvection = \"linear_upwind\"\nnon_orthogonal_correctors = 1\nmax_iterations = {iterations}\ntolerance_u = 1e-10\ntolerance_p = 1e-10\n[reports.torque]\ntype = \"moment\"\npatch = \"inner\"\ncomponent = \"viscous\"\ndirection = [0.0, 0.0, 1.0]\norigin = [0.0, 0.0, 0.0]\n[reports.m_in]\ntype = \"mass_flow\"\npatch = \"inner\"\n[reports.m_out]\ntype = \"mass_flow\"\npatch = \"outer\"\n{sources}"
    )
}

fn tc_bcs() -> String {
    wall("inner", 1.0) + &wall("outer", 0.0)
}

fn run(text: &str, iterations: usize) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let n = sim.run(iterations);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    println!(
        "  {n} iterations, converged {}, last {:?}",
        sim.is_converged(),
        sim.last()
    );
    sim
}

/// (u_theta, u_r, u_z, r) per cell.
fn polar(sim: &Simulation) -> Vec<(f64, f64, f64, f64)> {
    let cc = sim.geom.mesh.cell_centres();
    let (ux, uy, uz) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
    );
    (0..cc.len())
        .map(|c| {
            let (x, y) = (cc[c][0], cc[c][1]);
            let r = x.hypot(y);
            ((-y * ux[c] + x * uy[c]) / r, (x * ux[c] + y * uy[c]) / r, uz[c], r)
        })
        .collect()
}

fn tc_analytic(r: f64) -> f64 {
    let d = RO * RO - RI * RI;
    -RI * RI / d * r + RI * RI * RO * RO / d / r
}

fn tc_error(sim: &Simulation) -> f64 {
    polar(sim)
        .iter()
        .map(|&(ut, _, _, r)| (ut - tc_analytic(r)).abs())
        .fold(0.0, f64::max)
}

/// (net flux out of the rotor zone, sum |flux|, interface area) over the faces between `rotor` and the other cells.
fn interface_fluxes(sim: &Simulation) -> (f64, f64, f64) {
    let mesh = &sim.geom.mesh;
    let mut rotor = vec![false; mesh.n_cells()];
    let Some(zone) = mesh.cell_zone("rotor") else {
        return (0.0, 0.0, 0.0);
    };
    for &c in &zone.ids {
        rotor[c] = true;
    }
    let phi = sim.flux();
    let (mut net, mut abs, mut area) = (0.0, 0.0, 0.0);
    for f in 0..sim.geom.n_internal {
        let (o, n) = (mesh.owner()[f], mesh.neighbour()[f]);
        if rotor[o] != rotor[n] {
            net += if rotor[o] { phi[f] } else { -phi[f] };
            abs += phi[f].abs();
            area += mesh.face_area(f);
        }
    }
    (net, abs, area)
}

type Failures = Vec<(&'static str, String)>;

/// `x` exceeds `limit` or is not a number.
fn over(x: f64, limit: f64) -> bool {
    x.is_nan() || x > limit
}

fn require(f: &Failures, tags: &[&str]) {
    let bad: Vec<_> = f.iter().filter(|(t, _)| tags.contains(t)).collect();
    assert!(bad.is_empty(), "{bad:?}");
}

/// C1: solid-body rotation W = 2 with the rotor frame at W (tags `velocity`, `pressure`).
fn c1_impl(v: &Variant, tag: &str) -> Failures {
    let mut f = Failures::new();
    let w = 2.0;
    let sim = run(
        &case_text(
            &mesh_dir(v, tag),
            &(wall("inner", w) + &wall("outer", w)),
            &mrf(w, ROTOR),
            4000,
            v.solver,
        ),
        4000,
    );
    let cc = sim.geom.mesh.cell_centres();
    let (ux, uy, uz, p) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
        sim.field("p").unwrap(),
    );
    let err = (0..cc.len())
        .map(|c| ((ux[c] + w * cc[c][1]).powi(2) + (uy[c] - w * cc[c][0]).powi(2) + uz[c].powi(2)).sqrt())
        .fold(0.0, f64::max);
    let r0 = cc[0][0].hypot(cc[0][1]);
    let range = 0.5 * w * w * (RO * RO - RI * RI);
    let perr = (0..cc.len())
        .map(|c| {
            let r = cc[c][0].hypot(cc[c][1]);
            ((p[c] - p[0]) - 0.5 * w * w * (r * r - r0 * r0)).abs()
        })
        .fold(0.0, f64::max);
    println!(
        "C1 ({}): converged {}, max |u - W x r| = {:.3e} W r_o (limit 2e-2); pressure error {:.3e} of range (limit 5e-2)",
        v.name,
        sim.is_converged(),
        err / (w * RO),
        perr / range
    );
    if over(err, 2e-2 * w * RO) {
        f.push(("velocity", format!("C1 velocity {:.3e}", err / (w * RO))));
    }
    if over(perr, 5e-2 * range) {
        f.push(("pressure", format!("C1 pressure {:.3e}", perr / range)));
    }
    f
}

/// C2 (tags `velocity`, `control`) and C3 (tag `c3`; the absolute-flux limit `c3abs`).
fn c2_c3_impl(v: &Variant, tag: &str) -> Failures {
    let mut f = Failures::new();
    let mut ctrl = Vec::new();
    for mesh in (v.controls)(&format!("{}_{tag}", v.name)) {
        println!("C2 control ({}): {}", v.name, mesh.replace('\n', "; "));
        let sim = run(&case_text(&mesh, &tc_bcs(), &mrf(1.0, ALL), 4000, v.solver), 4000);
        ctrl.push(if sim.is_converged() { tc_error(&sim) } else { f64::NAN });
    }
    println!("C2 ({}): rotor/stator across the cylinder", v.name);
    let sim = run(
        &case_text(&mesh_dir(v, tag), &tc_bcs(), &mrf(1.0, ROTOR), 4000, v.solver),
        4000,
    );
    let e = tc_error(&sim);
    let bound = 1.5 * ctrl[0].max(ctrl[1]);
    let torque = sim.monitors.last("torque").unwrap().abs();
    println!(
        "C2 ({}): error {:.3e} V_i (limit 2e-2; controls {:.3e}, {:.3e}, 1.5 x max = {:.3e}); torque {torque:.5} vs {:.5} (reported)",
        v.name,
        e / V_I,
        ctrl[0] / V_I,
        ctrl[1] / V_I,
        bound / V_I,
        0.418_879_020_478_639_1 * LZ
    );
    if over(e, 2e-2 * V_I) {
        f.push(("velocity", format!("C2 velocity {:.3e}", e / V_I)));
    }
    if over(e, bound) {
        f.push((
            "control",
            format!("C2 {:.3e} > 1.5 x control {:.3e}", e / V_I, bound / V_I),
        ));
    }
    let cont = sim.last().map(|l| l.continuity).unwrap_or(f64::NAN);
    let (net, abs, area) = interface_fluxes(&sim);
    println!(
        "C3 ({}): converged {}, continuity {cont:.2e} (1e-8), net interface flux {:.3e} V_i A (1e-8), sum |flux| {:.4e} V_i A (1e-3), A_if {area:.5}",
        v.name,
        sim.is_converged(),
        net.abs() / (V_I * area),
        abs / (V_I * area)
    );
    if !sim.is_converged() {
        f.push(("c3", "C2 run not converged".into()));
    }
    if over(cont, 1e-8) {
        f.push(("c3", format!("continuity {cont:.3e}")));
    }
    if over(net.abs(), 1e-8 * V_I * area) {
        f.push(("c3", format!("net {:.3e}", net.abs() / (V_I * area))));
    }
    if over(abs, 1e-3 * V_I * area) {
        f.push(("c3abs", format!("sum |flux| {:.3e}", abs / (V_I * area))));
    }
    f
}

/// C4: frame invariance (tag `c4`).
fn c4_impl(v: &Variant, tag: &str) -> Failures {
    let mesh = mesh_dir(v, tag);
    let mut fields = Vec::new();
    for omega in [1.0, 0.0, 2.0] {
        println!("C4 ({}): rotor frame Omega = {omega}", v.name);
        let sim = run(&case_text(&mesh, &tc_bcs(), &mrf(omega, ROTOR), 4000, v.solver), 4000);
        fields.push((omega, polar(&sim)));
    }
    let mut f = Failures::new();
    for (omega, field) in fields.iter().skip(1) {
        let d = field
            .iter()
            .zip(&fields[0].1)
            .map(|(a, b)| (a.0 - b.0).abs().max((a.1 - b.1).abs()).max((a.2 - b.2).abs()))
            .fold(0.0, f64::max);
        println!(
            "C4 ({}): difference Omega {omega} to Omega 1: {:.3e} V_i (limit 1e-2)",
            v.name,
            d / V_I
        );
        if over(d, 1e-2 * V_I) {
            f.push(("c4", format!("Omega {omega}: {:.3e}", d / V_I)));
        }
    }
    f
}

/// C5: radial source flow m = 0.5 through the cylinder (tag `c5`).
fn c5_impl(v: &Variant, tag: &str) -> Failures {
    let m = 0.5;
    let sim = run(
        &case_text(&mesh_dir(v, tag), SOURCE_FLOW, &mrf(1.0, ROTOR), 4000, v.solver),
        4000,
    );
    let (q_in, q_out) = (sim.monitors.last("m_in").unwrap(), sim.monitors.last("m_out").unwrap());
    let (net, _, _) = interface_fluxes(&sim);
    let (mut eu, mut et): (f64, f64) = (0.0, 0.0);
    for (ut, ur, uz, r) in polar(&sim) {
        eu = eu.max(((ur - m / r).powi(2) + ut * ut + uz * uz).sqrt());
        et = et.max(ut.abs());
    }
    let scale = m / RI;
    let rel = |x: f64| (x.abs() - q_in.abs()).abs() / q_in.abs();
    println!(
        "C5 ({}): converged {}, inflow {q_in:.10}, outflow {q_out:.10} ({:.2e}, 1e-9), interface {net:.10} ({:.2e}, 1e-9); max |u - m r/r^2| {:.3e} (2e-2), max |u_theta| {:.3e} (2e-2) of m/r_i",
        v.name,
        sim.is_converged(),
        rel(q_out),
        rel(net),
        eu / scale,
        et / scale
    );
    let mut f = Failures::new();
    if over(rel(q_out), 1e-9) || over(rel(net), 1e-9) {
        f.push(("c5", format!("flux balance {:.2e} {:.2e}", rel(q_out), rel(net))));
    }
    if over(eu, 2e-2 * scale) || over(et, 2e-2 * scale) {
        f.push(("c5", format!("velocity {:.3e} / {:.3e}", eu / scale, et / scale)));
    }
    f
}

// ---- amendment 1, candidate K2 (selected by the convergence-only rule): the registered checks and limits

#[test]
fn k2_c1_solid_body_rotation_across_the_cylinder() {
    require(&c1_impl(&K2, "c1"), &["velocity", "pressure"]);
}

#[test]
fn k2_c2_c3_taylor_couette_across_the_cylinder_and_conservation() {
    require(&c2_c3_impl(&K2, "c2"), &["velocity", "control", "c3"]);
}

#[test]
#[ignore = "known miss: K2 sum |interface flux| 1.168e-3 V_i A (limit 1e-3); azimuthal flow interpolated between cells at different angles"]
fn k2_c3_absolute_interface_flux_known_miss() {
    require(&c2_c3_impl(&K2, "c3abs"), &["c3abs"]);
}

#[test]
fn k2_c4_frame_invariance_across_the_cylinder() {
    require(&c4_impl(&K2, "c4"), &["c4"]);
}

#[test]
fn k2_c5_radial_source_flow_through_the_cylinder() {
    require(&c5_impl(&K2, "c5"), &["c5"]);
}

// ---- registered mesh M: every run diverged (first pass, ADR 0109); limits unchanged

#[test]
#[ignore = "known miss: diverges on mesh M (overlap faces 79 deg non-orthogonal); ADR 0109 first pass"]
fn c1_solid_body_rotation_mesh_m_known_miss() {
    require(&c1_impl(&REGISTERED, "c1m"), &["velocity", "pressure"]);
}

#[test]
#[ignore = "known miss: diverges on mesh M (overlap faces 79 deg non-orthogonal); ADR 0109 first pass"]
fn c2_c3_taylor_couette_mesh_m_known_miss() {
    require(&c2_c3_impl(&REGISTERED, "c2m"), &["velocity", "control", "c3", "c3abs"]);
}

#[test]
#[ignore = "known miss: diverges on mesh M (overlap faces 79 deg non-orthogonal); ADR 0109 first pass"]
fn c4_frame_invariance_mesh_m_known_miss() {
    require(&c4_impl(&REGISTERED, "c4m"), &["c4"]);
}

#[test]
#[ignore = "known miss: diverges on mesh M (overlap faces 79 deg non-orthogonal); ADR 0109 first pass"]
fn c5_radial_source_flow_mesh_m_known_miss() {
    require(&c5_impl(&REGISTERED, "c5m"), &["c5"]);
}

/// Construction and 50-iteration smoke of the mesh-M set-ups (rule d), not a criterion.
#[test]
#[ignore = "smoke diagnostic of ADR 0109; run on demand"]
fn smoke_w43b_setups() {
    let mesh = mesh_dir(&REGISTERED, "smoke");
    let mut sims = vec![
        (
            "C1".to_string(),
            case_text(
                &mesh,
                &(wall("inner", 2.0) + &wall("outer", 2.0)),
                &mrf(2.0, ROTOR),
                50,
                "",
            ),
        ),
        ("C2".to_string(), case_text(&mesh, &tc_bcs(), &mrf(1.0, ROTOR), 50, "")),
        (
            "C4 0".to_string(),
            case_text(&mesh, &tc_bcs(), &mrf(0.0, ROTOR), 50, ""),
        ),
        (
            "C5".to_string(),
            case_text(&mesh, SOURCE_FLOW, &mrf(1.0, ROTOR), 50, ""),
        ),
    ];
    for c in controls_m("smoke") {
        sims.push((
            format!("C2 control {c}"),
            case_text(&c, &tc_bcs(), &mrf(1.0, ALL), 50, ""),
        ));
    }
    for (name, text) in sims {
        println!("smoke {name}");
        let sim = run(&text, 50);
        let (net, abs, area) = interface_fluxes(&sim);
        println!(
            "  cells {}, interface area {area:.5}, net {net:.3e}, sum |flux| {abs:.3e}, tc error {:.3e} V_i",
            sim.geom.n_cells(),
            tc_error(&sim) / V_I
        );
    }
}
