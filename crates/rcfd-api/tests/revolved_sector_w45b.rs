//! Wave 45B (ADR 0116): the `annulus_3d_sector` generator (an empty sector whose walls are exact surfaces of revolution) through
//! the case tree: cyclic seams (G2), the MRF zone opening with `cyclic_seams = true` (G3), the exact empty passage through
//! `Sources` and the `sim.rs` F1 line (G4), solid-body rotation sector against the full annulus (G5), the fan meshes unchanged
//! (G6), located errors (G7) and the negative control N1. Limits registered in ADR 0116 before these tests ran. This sector
//! has no blade: an MRF zone on the bladed FAN-01 passage stays refused at the blade wall (ADR 0116 context).

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_core::vec3;
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::sources::MrfZone;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use std::sync::Arc;

const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const Q_DESIGN: f64 = 1.4032;

/// Counts (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) of the ADR 0086 family member F3.
const F3: (usize, usize, usize, usize, usize, usize, usize) = (6, 12, 6, 8, 3, 10, 5);

fn fan_omega() -> f64 {
    1486.0 * 2.0 * std::f64::consts::PI / 60.0
}

/// The 16 registered FAN-01 sections (ADR 0075 amendment 1), as `fan01_stall_w43c.rs` reads them.
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

/// The ADR 0086 F3 mesh section (copied from `fan01_stall_w43c.rs::mesh_toml`), bladed or empty (E3).
fn fan01_mesh(blade: bool) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let area = std::f64::consts::PI * (R_DUCT * R_DUCT - R_HUB * R_HUB);
    let ref_speed = (Q_DESIGN / area).hypot(fan_omega() * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = F3;
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"camber_seam_h\"\nn_up = {n_up}\nn_chord = {n_chord}\nn_down = {n_down}\nn_side = {n_side}\nn_nose = {n_nose}\nn_core = 1\nfront_gap = 0.3\nn_span = {n_span}\nn_gap = {n_gap}\nn_bl = 2\ngrowth = 1.2\n\
first_cell_height = 0.0\ny_plus = 30.0\nref_speed = {ref_speed:?}\nnu = 1.5e-5\n{}",
        list(&rows, |s| s[0] * 1e-3),
        list(&rows, |s| s[1] * 1e-3),
        list(&rows, |s| s[2]),
        list(&rows, |s| s[3]),
        list(&rows, |s| s[4]),
        list(&rows, |s| s[5]),
        list(&rows, |s| (s[6] - x0) * 1e-3),
        list(&rows, |s| s[0] * 1e-3 * (s[7] - t0).to_radians()),
        if blade { "" } else { "include_blade = false\n" },
    )
}

/// `Mesh::content_hash` (points, faces, owner / neighbour, patches, zones) of `build_mesh`.
fn mesh_hash(text: &str) -> u64 {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    build_mesh(&case).unwrap_or_else(|e| panic!("{e:?}")).content_hash()
}

/// G6: the default `axial_fan_sector` mesh, the ADR 0086 F3 bladed mesh (B3) and its empty passage (E3) are bit-identical to
/// main 511bed4 (hashes recorded there before any wave-45B change).
#[test]
fn g6_fan_meshes_unchanged() {
    let default = format!("[mesh]\ngenerator = \"axial_fan_sector\"\n{}", "");
    let got = [
        mesh_hash(&default),
        mesh_hash(&fan01_mesh(true)),
        mesh_hash(&fan01_mesh(false)),
    ];
    println!(
        "G6 hashes: default {:016x}, B3 {:016x}, E3 {:016x}",
        got[0], got[1], got[2]
    );
    // Recorded on Windows. The fan mesh calls the platform's libm, so the hashes differ per platform: the x86_64 Linux values were
    // produced at 511bed4 (e65b4ea in the current history) and at HEAD in one Linux container (ADR 0137); other platforms print and skip.
    if cfg!(windows) {
        assert_eq!(
            got,
            [0x8471_bf9a_1aef_1dd4, 0xbe64_f588_a252_d0b4, 0x04fb_abed_bad4_1768]
        );
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        assert_eq!(
            got,
            [0x95aa_2af7_f4ed_8b6c, 0xb256_3dff_4908_6f53, 0xec27_db08_c015_4f65]
        );
    } else {
        eprintln!("G6: no recorded hashes for this platform (printed above)");
    }
}

// ------------------------------------------------------------------------------------------------------------ common set-up

const R_OUT: f64 = R_DUCT; // r_tip 0.2475 + clearance 0.0025
const LZ: f64 = 0.4;
const SECTOR_DEG: f64 = 40.0; // 9 blades
/// Sector-against-annulus limit as a fraction of the velocity scale (ADR 0008 M3, as in ADR 0113).
const L_DIFF: f64 = 4.3e-4;

/// Inlet speed Q / A of FAN-01 (ADR 0086), 9.48 m/s.
fn u_in() -> f64 {
    Q_DESIGN / (std::f64::consts::PI * (R_OUT * R_OUT - R_HUB * R_HUB))
}

/// The mesh section: `annulus_3d_sector` 8 x 10 x 24 (theta x r x z) of the FAN-01 empty passage, or `annulus_3d` with 72 cells
/// around (the same cells over 360 degrees).
fn mesh_text(sector: bool) -> String {
    let (generator, nx, angle) = if sector {
        ("annulus_3d_sector", 8, format!("sector_angle = {SECTOR_DEG:?}"))
    } else {
        ("annulus_3d", 72, String::new())
    };
    format!(
        "[mesh]\ngenerator = \"{generator}\"\nnx = {nx}\nny = 10\nnz = 24\nr_in = {R_HUB:?}\nr_out = {R_OUT:?}\nlz = {LZ:?}\n{angle}\n"
    )
}

/// G4 / N1 case: the exact empty passage (walls and inlet at (0, 0, U), outlet zero-gradient at p = 0, MRF zone all at the
/// FAN-01 speed, initial (0, 0, U)); `extra` goes into the source (the `cyclic_seams` key).
fn exact_case(extra: &str) -> String {
    let (u, w) = (u_in(), fan_omega());
    format!(
        r#"{}
[physics]
nu = 1.5e-5
rho = 1.0
[initial]
velocity = [0.0, 0.0, {u:?}]
[boundaries.zmin]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u:?}]
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.inner]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u:?}]
[boundaries.outer]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u:?}]
[sources.rotor]
type = "mrf"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
zone = "all"
{extra}
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 300
"#,
        mesh_text(true)
    )
}

/// G5 case: solid-body rotation, both walls at W = Omega x r, zmin / zmax symmetry planes, MRF zone all at Omega, fluid at
/// rest initially; the sector with `cyclic_seams = true`, the full annulus without.
fn rotation_case(sector: bool, max_iterations: usize) -> String {
    let w = fan_omega();
    let seams = if sector { "cyclic_seams = true" } else { "" };
    format!(
        r#"{}
[physics]
nu = 0.49
rho = 1.0
[boundaries.inner]
velocity = "fixed"
velocity_expression_x = "-{w:?} [1/s] * y"
velocity_expression_y = "{w:?} [1/s] * x"
[boundaries.outer]
velocity = "fixed"
velocity_expression_x = "-{w:?} [1/s] * y"
velocity_expression_y = "{w:?} [1/s] * x"
[boundaries.zmin]
velocity = "symmetry"
[boundaries.zmax]
velocity = "symmetry"
[sources.rotor]
type = "mrf"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
zone = "all"
{seams}
[solver]
algorithm = "simplec"
convection = "central"
alpha_u = 0.8
alpha_p = 1.0
tolerance_u = 1e-10
tolerance_p = 1e-9
max_iterations = {max_iterations}
"#,
        mesh_text(sector)
    )
}

fn parse(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

fn sim(text: &str) -> Simulation {
    Simulation::new(&parse(text)).unwrap_or_else(|e| panic!("{e:?}"))
}

fn geometry(text: &str) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(build_mesh(&parse(text)).unwrap_or_else(|e| panic!("{e:?}")))).unwrap())
}

fn mrf(seams: bool) -> MrfZone {
    MrfZone {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        omega: fan_omega(),
        cyclic_seams: seams,
    }
}

fn check(fail: &mut Vec<String>, name: &str, value: f64, limit: f64) {
    let ok = value <= limit;
    println!(
        "  {name}: {value:.3e} (limit {limit:.3e}) {}",
        if ok { "pass" } else { "MISS" }
    );
    if !ok {
        fail.push(format!("{name} {value:.3e} > {limit:.3e}"));
    }
}

fn require(fail: Vec<String>) {
    assert!(fail.is_empty(), "{fail:?}");
}

fn r_perp(x: vec3::Vec3) -> f64 {
    x[0].hypot(x[1])
}

/// max over cells of max(|u_x|, |u_y|, |u_z - U|) / U, and max |p| / U^2.
fn exact_errors(u: &[vec3::Vec3], p: &[f64]) -> (f64, f64) {
    let uu = u_in();
    let du = u
        .iter()
        .map(|v| v[0].abs().max(v[1].abs()).max((v[2] - uu).abs()))
        .fold(0.0, f64::max);
    (du / uu, p.iter().map(|x| x.abs()).fold(0.0, f64::max) / (uu * uu))
}

// ---------------------------------------------------------------------------------------------------------------- G2

/// Per-cell closure of the frame-flux offset, worst over the cells (zone all), relative to sum |Omega| r_perp |S| (ADR 0113 G).
fn closure(geom: &FvGeometry, m: &MrfZone) -> f64 {
    let mesh = &geom.mesh;
    let zone = vec![true; geom.n_cells()];
    let mut off = vec![0.0; mesh.n_faces()];
    m.add_frame_flux(geom, &zone, &mut off);
    let (fc, sf) = (mesh.face_centres(), mesh.face_area_vectors());
    let mut worst: f64 = 0.0;
    for c in 0..mesh.n_cells() {
        let (mut s, mut d) = (0.0f64, 0.0f64);
        for &f in mesh.cell_faces(c) {
            s += if mesh.owner()[f] == c { off[f] } else { -off[f] };
            d += m.omega.abs() * r_perp(fc[f]) * vec3::norm(sf[f]);
        }
        worst = worst.max(s.abs() / d);
    }
    worst
}

/// (worst |R c_f - c_g| / r_out, worst |F_f + F_g| / (|Omega| r |S|)) over the theta_min faces f and their partners g.
fn seam_pairs(geom: &FvGeometry, m: &MrfZone) -> (f64, f64) {
    let mesh = &geom.mesh;
    let pi = mesh.patches().iter().position(|p| p.name == "theta_min").unwrap();
    let map = geom.periodic_partner[pi].as_ref().expect("theta_min has partners");
    let (fc, sf) = (mesh.face_centres(), mesh.face_area_vectors());
    let (c, s) = (SECTOR_DEG.to_radians().cos(), SECTOR_DEG.to_radians().sin());
    let flux = |f: usize| vec3::dot(m.frame_velocity(fc[f]), sf[f]);
    let (mut pos, mut pair): (f64, f64) = (0.0, 0.0);
    for (f, &g) in mesh.patch("theta_min").unwrap().faces().zip(map) {
        assert_eq!(mesh.patch_of_face(g).unwrap().name, "theta_max");
        let x = fc[f];
        let rx = [c * x[0] - s * x[1], s * x[0] + c * x[1], x[2]];
        pos = pos.max(vec3::norm(vec3::sub(rx, fc[g])) / R_OUT);
        pair = pair.max((flux(f) + flux(g)).abs() / (m.omega.abs() * r_perp(x) * mesh.face_area(f)));
    }
    (pos, pair)
}

#[test]
fn g2_cyclic_seams_of_the_tree_sector() {
    let geom = geometry(&exact_case(""));
    let m = mrf(true);
    let (pos, pair) = seam_pairs(&geom, &m);
    let mut fail = Vec::new();
    println!("G2 (FvGeometry accepted the pair; {} cells):", geom.n_cells());
    check(&mut fail, "partner face centre |R c_lo - c_hi| / r_out", pos, 1e-9);
    check(&mut fail, "pair mismatch |F_f + F_g| / (Omega r |S|)", pair, 1e-12);
    check(&mut fail, "per-cell frame-flux closure", closure(&geom, &m), 1e-12);
    require(fail);
}

// ---------------------------------------------------------------------------------------------------------------- G3

#[test]
fn g3_mrf_zone_opens_with_cyclic_seams_and_is_refused_without() {
    let text = exact_case("cyclic_seams = true");
    let _ = sim(&text);
    let geom = geometry(&text);
    let (worst, at) = mrf(true).worst_non_axisymmetry(&geom, &vec![true; geom.n_cells()]);
    let place = at.and_then(|f| geom.mesh.patch_of_face(f)).map(|p| p.name.clone());
    let mut fail = Vec::new();
    println!("G3: Simulation::new accepted the zone; worst face on {place:?}");
    check(&mut fail, "worst_non_axisymmetry with seams", worst, 1e-6);
    let d = match Simulation::new(&parse(&exact_case(""))) {
        Ok(_) => panic!("without cyclic_seams the zone must be refused"),
        Err(d) => d,
    };
    println!("G3 refusal: {d:?}");
    assert!(
        d.iter().any(|x| x.path == "sources.rotor.zone"
            && x.message.contains("surface of revolution")
            && (x.message.contains("'theta_min'") || x.message.contains("'theta_max'"))),
        "{d:?}"
    );
    require(fail);
}

// ---------------------------------------------------------------------------------------------------------------- G4, N1

#[test]
fn g4_exact_empty_passage_through_the_case_tree() {
    let mut s = sim(&exact_case("cyclic_seams = true"));
    let mut f1 = true;
    for _ in 0..300 {
        s.step(); // step() runs even after the residual targets are met: no early stop
        f1 &= s.simple_solver().is_some_and(|x| x.coupled_offdiag_in_h1);
    }
    let (du, p) = exact_errors(&s.velocity().values, &s.field("p").unwrap());
    let mut fail = Vec::new();
    println!("G4 (300 steps; F1 flag set in every step: {f1}):");
    check(&mut fail, "max |u - (0, 0, U)| / U", du, 1e-8);
    check(&mut fail, "max |p| / U^2", p, 1e-8);
    assert!(f1, "coupled_offdiag_in_h1 was not set inside step()");
    require(fail);
}

/// The G4 set-up at library level on the tree mesh, the MRF zone wired as `sim.rs` wires it (ADR 0113 K6 harness, default
/// settings); `seams = false` is N1, called past the refusal.
fn exact_library(seams: bool, iters: usize) -> (f64, f64) {
    let geom = geometry(&exact_case(""));
    let u0 = [0.0, 0.0, u_in()];
    let mut u = VectorField::uniform("U", &geom, u0);
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    for w in ["inner", "outer", "zmin"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector(u0))).unwrap();
    }
    u.set_bc(&geom, "zmax", Box::new(ZeroGradient)).unwrap();
    p.set_bc(&geom, "zmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut solver = SimpleSolver::new(geom.clone(), u, p, SimpleSettings::new(1.5e-5, Coupling::Simplec));
    let m = mrf(seams);
    let zone = vec![true; geom.n_cells()];
    let mut off = vec![0.0; geom.mesh.n_faces()];
    m.add_frame_flux(&geom, &zone, &mut off);
    solver.conv_flux_offset = Some(off);
    solver.component_diag_correction = true;
    solver.coupled_offdiag_in_h1 = seams;
    for _ in 0..iters {
        let ub = solver.u.boundary_values(&geom);
        let mut f = BodyForce::uniform(&geom, [0.0; 3]);
        m.add_force(&geom, &solver.u.values, &ub, &zone, &mut f);
        solver.body_force = Some(f);
        solver.iterate();
    }
    exact_errors(&solver.u.values, &solver.p.values)
}

/// N1 (negative control, run first): G4 without the seam frame flux must miss 1e-8 U by at least 10x.
#[test]
fn n1_exact_empty_passage_without_seam_frame_flux_is_wrong() {
    let (du, _) = exact_library(false, 300);
    println!("N1: max |u - (0, 0, U)| {du:.3e} U (must be >= 1e-7)");
    assert!(du >= 1e-7 || !du.is_finite());
}

// ---------------------------------------------------------------------------------------------------------------- G5

/// For each cell of `a`, the cell of `b` at the same centre (within 1e-9).
fn matching(a: &Simulation, b: &Simulation) -> Vec<usize> {
    let (ca, cb) = (a.geom.mesh.cell_centres(), b.geom.mesh.cell_centres());
    ca.iter()
        .map(|x| {
            let d = |k: usize| vec3::norm(vec3::sub(cb[k], *x));
            let j = (0..cb.len()).min_by(|&i, &j| d(i).total_cmp(&d(j))).unwrap();
            assert!(d(j) <= 1e-9, "no matching cell");
            j
        })
        .collect()
}

#[test]
fn g5_solid_body_rotation_sector_against_the_full_annulus() {
    let w = fan_omega();
    let wr = w * R_OUT;
    let run = |sector: bool| {
        let mut s = sim(&rotation_case(sector, 4000));
        let n = s.run(4000);
        let name = if sector { "sector" } else { "annulus" };
        println!(
            "  {name}: {n} iterations, converged {}, stop: {:?}",
            s.is_converged(),
            s.stop_reason()
        );
        s
    };
    println!("G5:");
    let (a, b) = (run(true), run(false));
    let m = matching(&a, &b);
    let (ua, ub) = (&a.velocity().values, &b.velocity().values);
    let (pa, pb) = (a.field("p").unwrap(), b.field("p").unwrap());
    let ca = a.geom.mesh.cell_centres();
    let (mut du, mut dp, mut ea): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for (i, &j) in m.iter().enumerate() {
        for k in 0..3 {
            du = du.max((ua[i][k] - ub[j][k]).abs());
        }
        dp = dp.max(((pa[i] - pa[0]) - (pb[j] - pb[m[0]])).abs());
        let exact = [-w * ca[i][1], w * ca[i][0], 0.0];
        ea = ea.max(vec3::norm(vec3::sub(ua[i], exact)));
    }
    let p_range = 0.5 * w * w * (R_OUT * R_OUT - R_HUB * R_HUB);
    println!(
        "  reported: sector against analytic {:.3e} W r_out; sector - annulus pressure {:.3e} of W^2 (r_o^2 - r_i^2) / 2",
        ea / wr,
        dp / p_range
    );
    let mut fail = Vec::new();
    check(&mut fail, "sector - annulus velocity / (W r_out)", du / wr, L_DIFF);
    assert!(a.is_converged() && b.is_converged(), "both runs must converge");
    require(fail);
}

// ---------------------------------------------------------------------------------------------------------------- G7

/// Diagnostics of parsing and building the mesh of `text` (empty when both succeed).
fn mesh_errors(text: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(text) {
        Err(d) => d,
        Ok(case) => build_mesh(&case).err().unwrap_or_default(),
    }
}

#[test]
fn g7_invalid_sector_settings_are_located() {
    let base = mesh_text(true);
    let set = |from: &str, to: &str| {
        assert!(base.contains(from), "{from}");
        base.replace(from, to)
    };
    let angle = format!("sector_angle = {SECTOR_DEG:?}");
    let cases = [
        (set(&angle, "sector_angle = 0.0"), "mesh.sector_angle"),
        (set(&angle, ""), "mesh.sector_angle"),
        (set(&angle, "sector_angle = 360.0"), "mesh.sector_angle"),
        (set(&angle, "sector_angle = 400.0"), "mesh.sector_angle"),
        (set("r_out = 0.25", "r_out = 0.124"), "mesh.r_out"),
        (set("r_out = 0.25", "r_out = 0.1"), "mesh.r_out"),
        (set("nx = 8", "nx = 0"), "mesh.nx"),
        (set("ny = 10", "ny = 0"), "mesh.ny"),
        (set("nz = 24", "nz = 0"), "mesh.nz"),
        (
            format!("{}sector_angle = 40.0\n", mesh_text(false)),
            "mesh.sector_angle",
        ),
    ];
    for (text, path) in cases {
        let d = mesh_errors(&text);
        println!("G7 {path}: {d:?}");
        assert!(d.iter().any(|x| x.path == path), "{text}\n{d:?}");
    }
    assert!(mesh_errors(&base).is_empty());
}

// ---------------------------------------------------------------------------------------------------------------- smoke

/// Smoke (rule d, before registration): construction, `FvGeometry` acceptance, the face-level revolution value of every patch,
/// and 30 iterations of G4 (tree), N1 (library) and G5 (both meshes), finiteness only.
#[test]
#[ignore = "smoke of ADR 0116; run on demand"]
fn smoke_w45b() {
    let geom = geometry(&exact_case(""));
    let m = mrf(true);
    let (fc, sf) = (geom.mesh.face_centres(), geom.mesh.face_area_vectors());
    for p in geom.mesh.patches() {
        let worst = p
            .faces()
            .map(|f| vec3::dot(m.frame_velocity(fc[f]), sf[f]).abs() / (m.omega * r_perp(fc[f]) * vec3::norm(sf[f])))
            .fold(0.0, f64::max);
        println!(
            "smoke: patch {} ({} faces) worst |F_f| / (Omega r |S|) {worst:.3e}",
            p.name, p.size
        );
    }
    let mut s = sim(&exact_case("cyclic_seams = true"));
    for _ in 0..30 {
        s.step();
    }
    let (du, p) = exact_errors(&s.velocity().values, &s.field("p").unwrap());
    let f1 = s.simple_solver().map(|x| x.coupled_offdiag_in_h1);
    println!(
        "smoke G4 30 steps: finite {} (F1 flag {f1:?})",
        du.is_finite() && p.is_finite()
    );
    let (du, _) = exact_library(false, 30);
    println!("smoke N1 30 iterations: finite {}", du.is_finite());
    for sector in [true, false] {
        let mut s = sim(&rotation_case(sector, 30));
        let n = s.run(30);
        let fin = s.velocity().values.iter().all(|v| v.iter().all(|x| x.is_finite()));
        let name = if sector { "sector" } else { "annulus" };
        println!("smoke G5 {name}: {n} iterations, finite {fin}, last {:?}", s.last());
    }
}
