//! ADR 0183 amendment P-AB, case-tree checks of the EXP-ROT prerequisites: P-A (`solver.linear_upwind_limiter` accepted for
//! `piso` / `pimple`: PA-0 bit-identity, PA-1 scope) and P-B (`dynamic_mesh.rest_patches`, walls at rest on a rigidly moving
//! mesh, and a rigid prescribed motion with no following patch: PB-1 kinematics with its negative control PB-1N, PB-2
//! located errors, PB-3 settings hash). The limits are those registered in the amendment before this code ran.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;
use rcfd_mesh::motion::swept_volumes;

const OMEGA: f64 = 155.614;
const R_OUT: f64 = 0.25;

/// Paths of the diagnostics of a case: parse errors, then static errors, then the errors of `Simulation::new`.
fn paths(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.iter().map(|x| x.path.clone()).collect(),
        Ok(c) => {
            let mut d: Vec<String> = c.semantic_errors().iter().map(|x| x.path.clone()).collect();
            if d.is_empty() {
                if let Err(e) = Simulation::new(&c) {
                    d = e.iter().map(|x| x.path.clone()).collect();
                }
            }
            d
        }
    }
}

fn has(text: &str, path: &str) {
    let p = paths(text);
    assert!(p.iter().any(|x| x == path), "{path}: got {p:?}\n{text}");
}

fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}\n{text}"))
}

// ---------------------------------------------------------------------------------------------------------------
// P-A
// ---------------------------------------------------------------------------------------------------------------

/// `cavity_2d` 20 x 20, `nu = 0.01`, `linear_upwind`, the given algorithm and extra solver lines.
fn cavity(alg: &str, extra: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 20\nny = 20\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\n{extra}algorithm = \"{alg}\"\nconvection = \"linear_upwind\"\ndt = 0.01\nend_time = 1.0\n"
    )
}

/// Bits of `ux`, `uy`, `p` and the flux after 20 steps.
fn bits_after_20(text: &str) -> Vec<Vec<u64>> {
    let mut sim = sim_of(text);
    for _ in 0..20 {
        sim.step();
    }
    let b = |v: Vec<f64>| v.into_iter().map(f64::to_bits).collect::<Vec<_>>();
    vec![
        b(sim.field("ux").unwrap()),
        b(sim.field("uy").unwrap()),
        b(sim.field("p").unwrap()),
        b(sim.flux().to_vec()),
    ]
}

/// PA-0: for `piso` and `pimple` the key absent and `"none"` are bit-identical.
#[test]
fn pa0_key_absent_and_none_are_bit_identical_for_piso_and_pimple() {
    for alg in ["piso", "pimple"] {
        let absent = bits_after_20(&cavity(alg, ""));
        let none = bits_after_20(&cavity(alg, "linear_upwind_limiter = \"none\"\n"));
        assert!(absent == none, "PA-0 {alg}: fields differ");
        println!("PA-0 {alg}: bit-identical");
    }
}

/// PA-1: the key is accepted for `piso` and `pimple` with `linear_upwind` (and with `dynamic_mesh`), and is a located
/// error with another scheme, with the fractional step and with the coupled solver.
#[test]
fn pa1_scope_of_the_key() {
    let bj = "linear_upwind_limiter = \"barth_jespersen\"\n";
    for alg in ["piso", "pimple"] {
        let lim = bits_after_20(&cavity(alg, bj));
        let unl = bits_after_20(&cavity(alg, ""));
        // the key reaches the transient solver: the limited run is not the unlimited one
        assert!(lim != unl, "PA-1 {alg}: the limiter changed nothing");
        println!("PA-1 {alg}: accepted, and the limited run differs from the unlimited one");
    }
    sim_of(&pb1_case(true));
    println!("PA-1 piso with dynamic_mesh: accepted");
    let at = "solver.linear_upwind_limiter";
    has(
        &cavity("piso", bj).replace("convection = \"linear_upwind\"", "convection = \"upwind\""),
        at,
    );
    has(
        &cavity("piso", &format!("{bj}time_advance = \"fractional_step\"\n")),
        at,
    );
    has(
        &cavity("simple", bj).replace("algorithm = \"simple\"", "algorithm = \"coupled\""),
        at,
    );
}

// ---------------------------------------------------------------------------------------------------------------
// P-B
// ---------------------------------------------------------------------------------------------------------------

/// The PB-1 case: `annulus_3d` 24 x 4 x 2 (r 0.124 to 0.25, lz 0.05), inlet `zmin` (0, 0, 9.478), outlet `zmax` at pressure 0,
/// rigid prescribed rotation at Omega about z through (0.01, 0, 0), no moving patch; `inner` and `outer` at rest when `rest`.
fn pb1_case(rest: bool) -> String {
    let dt = std::f64::consts::PI / 180.0 / OMEGA;
    let rest_line = if rest {
        "rest_patches = [\"inner\", \"outer\"]\n"
    } else {
        ""
    };
    format!(
        r#"[mesh]
generator = "annulus_3d"
nx = 24
ny = 4
nz = 2
r_in = 0.124
r_out = 0.25
lz = 0.05
[physics]
nu = 1.5e-5
rho = 1.2
[initial]
velocity = [0.0, 0.0, 9.478]
[boundaries.zmin]
velocity = "fixed"
velocity_value = [0.0, 0.0, 9.478]
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
velocity = "fixed"
[solver]
algorithm = "piso"
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
dt = {dt:?}
end_time = 1.0
momentum_rel_tol = 1e-13
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-16
[dynamic_mesh]
enabled = true
{rest_line}rotation_angle_expression = "155.614 [1/s] * t"
rotation_origin = [0.01, 0.0, 0.0]
"#
    )
}

/// Three steps of the PB-1 case: per step, over the faces of `inner` and `outer`, (max |phi_f - m_f| / (Omega r_out |S_f|),
/// max |tangential part of u_b| / (Omega r_out), max |m_f| / (Omega r_out |S_f|)), with `m_f` from the swept volumes of the
/// meshes before and after the step.
fn pb1_run(rest: bool) -> Vec<(f64, f64, f64)> {
    let dt = std::f64::consts::PI / 180.0 / OMEGA;
    let mut sim = sim_of(&pb1_case(rest));
    let mut out = Vec::new();
    for _ in 0..3 {
        let old = sim.geom.mesh.clone();
        sim.step();
        let g = sim.geom.clone();
        let new = &g.mesh;
        let m: Vec<f64> = swept_volumes(&old, new.points()).iter().map(|w| w / dt).collect();
        let ub = sim.velocity().boundary_values(&g);
        let phi = sim.flux();
        let (mut rel, mut tan, mut sweep): (f64, f64, f64) = (0.0, 0.0, 0.0);
        for name in ["inner", "outer"] {
            for f in new.patch(name).unwrap().faces() {
                let s = new.face_area_vectors()[f];
                let a = vec3::norm(s);
                let n = vec3::scale(s, 1.0 / a);
                let u = ub[f - g.n_internal];
                let t = vec3::sub(u, vec3::scale(n, vec3::dot(u, n)));
                rel = rel.max((phi[f] - m[f]).abs() / (OMEGA * R_OUT * a));
                tan = tan.max(vec3::norm(t) / (OMEGA * R_OUT));
                sweep = sweep.max(m[f].abs() / (OMEGA * R_OUT * a));
            }
        }
        out.push((rel, tan, sweep));
    }
    out
}

/// PB-1: walls at rest carry exactly the swept volume and have no tangential velocity, on faces that sweep volume.
#[test]
fn pb1_walls_at_rest_carry_the_swept_volume_with_no_tangential_part() {
    let r = pb1_run(true);
    for (k, (rel, tan, sweep)) in r.iter().enumerate() {
        println!(
            "PB-1 step {}: max |phi - m| {rel:.3e} Omega r_out |S| (1e-12), max |u_t| {tan:.3e} Omega r_out (1e-12), max |m| {sweep:.3e} Omega r_out |S| (>= 1e-3)",
            k + 1
        );
    }
    for (rel, tan, sweep) in r {
        assert!(sweep >= 1e-3, "PB-1 non-vacuity: {sweep:.3e}");
        assert!(rel <= 1e-12, "PB-1 relative flux {rel:.3e}");
        assert!(tan <= 1e-12, "PB-1 tangential part {tan:.3e}");
    }
}

/// PB-1N: the same case with plain `fixed` 0 walls leaves the relative flux `-m_f`, so PB-1's limit is missed.
#[test]
fn pb1n_plain_walls_leave_the_relative_flux() {
    let r = pb1_run(false);
    let worst = r.iter().map(|x| x.0).fold(0.0, f64::max);
    println!("PB-1N: max |phi - m| {worst:.3e} Omega r_out |S| (>= 1e-3)");
    assert!(worst >= 1e-3, "PB-1N: {worst:.3e}");
}

/// PB-2: the located errors of `rest_patches` and of an empty `moving_patches`.
#[test]
fn pb2_located_errors() {
    let base = pb1_case(true);
    let rest_at = "dynamic_mesh.rest_patches";
    has(&base.replace("[\"inner\", \"outer\"]", "[\"nope\"]"), rest_at);
    has(
        &base.replace("enabled = true\n", "enabled = true\nmoving_patches = [\"inner\"]\n"),
        rest_at,
    );
    has(
        &base.replace("enabled = true\n", "enabled = true\nfollow_patches = [\"outer\"]\n"),
        rest_at,
    );
    has(
        &base.replace(
            "[boundaries.inner]\nvelocity = \"fixed\"",
            "[boundaries.inner]\nvelocity = \"zero_gradient\"",
        ),
        "boundaries.inner.velocity",
    );
    has(
        &base.replace(
            "[boundaries.inner]\nvelocity = \"fixed\"",
            "[boundaries.inner]\nvelocity = \"fixed\"\nvelocity_value = [0.0, 1.0, 0.0]",
        ),
        "boundaries.inner.velocity_value",
    );
    has(
        &base.replace(
            "[boundaries.inner]\nvelocity = \"fixed\"",
            "[boundaries.inner]\nvelocity = \"fixed\"\nvelocity_expression_x = \"1 [m/s]\"",
        ),
        "boundaries.inner.velocity_expression_x",
    );
    has(
        &base.replace("enabled = true\n", "enabled = true\ndeformation = \"smoothing\"\n"),
        rest_at,
    );
    let no_rest = pb1_case(false);
    has(
        &no_rest.replace("enabled = true\n", "enabled = true\ndeformation = \"smoothing\"\n"),
        "dynamic_mesh.moving_patches",
    );
    has(
        &no_rest.replace("enabled = true\n", "enabled = true\nmotion = \"six_dof\"\n"),
        "dynamic_mesh.moving_patches",
    );
    // accepted: a rigid prescribed motion with no moving patch, with and without walls at rest
    assert!(paths(&base).is_empty(), "{:?}", paths(&base));
    assert!(paths(&no_rest).is_empty(), "{:?}", paths(&no_rest));
}

/// PB-3: the key at its default leaves the settings value and the content hash unchanged; a non-default value changes the
/// hash, is written and round-trips.
#[test]
fn pb3_rest_patches_hash_and_text() {
    let absent = Case::parse_toml(&pb1_case(false)).unwrap();
    let empty =
        Case::parse_toml(&pb1_case(false).replace("enabled = true\n", "enabled = true\nrest_patches = []\n")).unwrap();
    let set = Case::parse_toml(&pb1_case(true)).unwrap();
    assert_eq!(absent.tree.to_json(), empty.tree.to_json(), "PB-3 settings value");
    assert_eq!(
        absent.tree.content_hash(),
        empty.tree.content_hash(),
        "PB-3 hash at the default"
    );
    assert_ne!(
        absent.tree.content_hash(),
        set.tree.content_hash(),
        "PB-3 hash of a non-default value"
    );
    let text = set.to_toml();
    assert!(
        text.contains("rest_patches = [") && text.contains("\"inner\""),
        "PB-3 written text:\n{text}"
    );
    let back = Case::parse_toml(&text).unwrap();
    assert_eq!(back.tree.to_json(), set.tree.to_json(), "PB-3 round trip value");
    assert_eq!(
        back.tree.content_hash(),
        set.tree.content_hash(),
        "PB-3 round trip hash"
    );
    println!(
        "PB-3: absent / [] hash {:016x}, [\"inner\", \"outer\"] {:016x}, round trip equal",
        absent.tree.content_hash(),
        set.tree.content_hash()
    );
}
