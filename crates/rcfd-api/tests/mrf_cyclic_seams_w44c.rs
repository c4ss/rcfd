//! Wave 44C (ADR 0113): MRF zones with cyclic seams through the case tree (A1-A3) and the empty fan passage at library level (K6).
//! Limits registered in ADR 0113 before this code ran.

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_core::vec3;
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::sources::MrfZone;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use std::sync::Arc;

/// The passage of `fan_sector.rs` without the blade (default blade parameters, so the C-grid is sheared), with an MRF zone on
/// every cell (hub and shroud moving axially with the flow, the K4 set-up).
fn fan_case(extra: &str, kind: &str) -> String {
    format!(
        r#"[mesh]
generator = "axial_fan_sector"
[mesh.fan]
n_up = 8
n_chord = 12
n_down = 8
n_side = 10
n_mid = 2
n_span = 16
n_gap = 7
n_bl = 2
growth = 1.2
first_cell_height = 1e-3
include_blade = false
[physics]
nu = 0.02
rho = 1.0
[initial]
velocity = [0.0, 0.0, 1.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, 1.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
velocity = "fixed"
velocity_value = [0.0, 0.0, 1.0]
[boundaries.shroud]
velocity = "fixed"
velocity_value = [0.0, 0.0, 1.0]
[sources.frame]
type = "{kind}"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 20.0
{extra}
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 300
"#
    )
}

fn refusal(text: &str) -> Vec<rcfd_api::case::Diagnostic> {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    match Simulation::new(&case) {
        Ok(_) => panic!("the set-up must be refused"),
        Err(d) => d,
    }
}

/// A1: on the ADR 0008 M2 annulus (no cyclic faces) the key absent, false and true give bit-identical fields after 20 iterations.
#[test]
fn a1_default_and_key_bit_identical_on_the_annulus() {
    let text = |extra: &str| {
        format!(
            r#"
[mesh]
generator = "annulus_3d"
nx = 48
ny = 16
nz = 2
r_in = 0.5
r_out = 1.0
lz = 0.4
[physics]
nu = 0.1
rho = 1.0
[boundaries.inner]
velocity = "fixed"
velocity_expression_x = "-1.0 [m/s] * y / 1 [m]"
velocity_expression_y = "1.0 [m/s] * x / 1 [m]"
[boundaries.outer]
velocity = "fixed"
[boundaries.zmin]
velocity = "symmetry"
[boundaries.zmax]
velocity = "symmetry"
[sources.rotor]
type = "mrf"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 1.0
zone = "all"
{extra}
[solver]
algorithm = "simplec"
convection = "linear_upwind"
non_orthogonal_correctors = 1
max_iterations = 20
"#
        )
    };
    let fields = |extra: &str| {
        let case = Case::parse_toml(&text(extra)).unwrap_or_else(|e| panic!("{e:?}"));
        let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
        sim.run(20);
        ["ux", "uy", "uz", "p"].map(|f| sim.field(f).unwrap())
    };
    let base = fields("");
    for extra in ["cyclic_seams = false", "cyclic_seams = true"] {
        let other = fields(extra);
        for (a, b) in base.iter().zip(&other) {
            assert!(
                a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()),
                "{extra}: not bit-identical"
            );
        }
    }
}

/// A2: the fan passage is refused at the seams without the key and at the hub with it.
#[test]
fn a2_fan_passage_refusal_moves_from_the_seams_to_the_hub() {
    for (extra, want) in [("", "seam_"), ("cyclic_seams = true", "'hub'")] {
        let d = refusal(&fan_case(extra, "mrf"));
        println!("{extra:?}: {d:?}");
        assert!(
            d.iter().any(|x| x.path == "sources.frame.zone"
                && x.message.contains("surface of revolution")
                && x.message.contains(want)),
            "{extra:?}: {d:?}"
        );
    }
}

/// A3: `cyclic_seams` on a rotating_frame source is a located error.
#[test]
fn a3_cyclic_seams_on_a_rotating_frame_is_refused() {
    // the settings check runs when the case is parsed
    let d = match Case::parse_toml(&fan_case("cyclic_seams = true", "rotating_frame")) {
        Err(d) => d,
        Ok(case) => rcfd_api::sources::check(&case),
    };
    assert!(d.iter().any(|x| x.path == "sources.frame.cyclic_seams"), "{d:?}");
}

/// K6 (library level): the K4 set-up on the fan passage, 300 iterations from (0, 0, U); returns (max |u - U| / U, seam pair
/// mismatch, whether the worst cell touches hub/shroud, whether it touches a seam).
fn k6() -> (f64, f64, bool, bool) {
    let case = Case::parse_toml(&fan_case("", "mrf")).unwrap_or_else(|e| panic!("{e:?}"));
    let geom = Arc::new(FvGeometry::new(Arc::new(build_mesh(&case).unwrap())).unwrap());
    let u0 = [0.0, 0.0, 1.0];
    let mut u = VectorField::uniform("U", &geom, u0);
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    for w in ["hub", "shroud", "inlet"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector(u0))).unwrap();
    }
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut s = SimpleSettings::new(0.02, Coupling::Simplec);
    s.alpha_u = 0.8;
    s.alpha_p = 1.0;
    s.convection = rcfd_fv::convection::ConvScheme::Central;
    let mut solver = SimpleSolver::new(geom.clone(), u, p, s);
    let m = MrfZone {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        omega: 20.0,
        cyclic_seams: true,
    };
    let zone = vec![true; geom.n_cells()];
    let mut off = vec![0.0; geom.mesh.n_faces()];
    m.add_frame_flux(&geom, &zone, &mut off);
    solver.conv_flux_offset = Some(off);
    solver.component_diag_correction = true;
    solver.coupled_offdiag_in_h1 = true;
    for _ in 0..300 {
        let ub = solver.u.boundary_values(&geom);
        let mut f = BodyForce::uniform(&geom, [0.0; 3]);
        m.add_force(&geom, &solver.u.values, &ub, &zone, &mut f);
        solver.body_force = Some(f);
        solver.iterate();
    }
    let (mut worst, mut at) = (0.0, 0);
    for (c, v) in solver.u.values.iter().enumerate() {
        let d = v[0].abs().max(v[1].abs()).max((v[2] - 1.0).abs());
        if d > worst || !d.is_finite() {
            (worst, at) = (d, c);
        }
    }
    let mesh = &geom.mesh;
    let touches = |names: [&str; 2]| {
        mesh.cell_faces(at)
            .iter()
            .any(|&f| mesh.patch_of_face(f).is_some_and(|p| names.contains(&p.name.as_str())))
    };
    let (fc, sf) = (mesh.face_centres(), mesh.face_area_vectors());
    let flux = |f: usize| vec3::dot(m.frame_velocity(fc[f]), sf[f]);
    let pi = mesh.patches().iter().position(|p| p.name == "seam_lo").unwrap();
    let map = geom.periodic_partner[pi].as_ref().unwrap();
    let pair = mesh
        .patch("seam_lo")
        .unwrap()
        .faces()
        .zip(map)
        .map(|(f, &g)| (flux(f) + flux(g)).abs() / (20.0 * fc[f][0].hypot(fc[f][1]) * mesh.face_area(f)))
        .fold(0.0, f64::max);
    (worst, pair, touches(["hub", "shroud"]), touches(["seam_lo", "seam_hi"]))
}

/// K6: registered limit max |u - (0, 0, U)| <= 1e-8 U; predicted miss owed to the hub/shroud facets (ADR 0113).
#[test]
#[ignore = "known miss (ADR 0113 K6): the fan hub/shroud faces are not surfaces of revolution; their frame flux goes nowhere"]
fn k6_fan_empty_passage_exact_state_known_miss() {
    let (worst, pair, hub, seam) = k6();
    println!("K6: max |u - U| {worst:.3e} U (limit 1e-8); seam pair mismatch {pair:.3e}; worst cell touches hub/shroud {hub}, a seam {seam}");
    assert!(worst <= 1e-8);
}
