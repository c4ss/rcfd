//! F-710 (ADR 0638): A1t, the uniform flow of ADR 0637's UTc through the case tree (`[mesh.sliding]`), whose step lands on a
//! node coincidence every whole rotor pitch (steps 8, 16, ..., 40). The limit is that registered in ADR 0638 before this
//! code ran; the test prints every measured number and then asserts the check.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};

/// UTc of ADR 0637 amendment 1 (every boundary fixed at `(1, 0.5, 0)`, `dt` the rotor pitch over 8).
const UTC: &str = r#"[mesh]
generator = "annulus_3d_cylindrical_interface"
nx = 144
ny = 8
nz = 1
nx_right = 120
ny_right = 8
nz_right = 1
r_in = 1.0
r_interface = 1.5
r_out = 2.0
lz = 0.1

[mesh.sliding]
angular_velocity = 1.0

[physics]
nu = 0.1

[initial]
velocity = [1.0, 0.5, 0.0]

[boundaries.inner]
velocity = "fixed"
velocity_value = [1.0, 0.5, 0.0]

[boundaries.outer]
velocity = "fixed"
velocity_value = [1.0, 0.5, 0.0]

[boundaries.zmin]
velocity = "fixed"
velocity_value = [1.0, 0.5, 0.0]

[boundaries.zmax]
velocity = "fixed"
velocity_value = [1.0, 0.5, 0.0]

[solver]
algorithm = "piso"
convection = "central"
non_orthogonal_correctors = 6
momentum_rel_tol = 1e-13
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-16
momentum_max_iter = 5000
pressure_max_iter = 5000
dt = 0.00545415391248228
end_time = 1.0
"#;

/// A1t of ADR 0638: UCt (UTc, 40 steps), the deviation of ADR 0637 U1 on every step.
#[test]
fn a1t_uniform_flow_through_the_tree_at_coinciding_nodes() {
    let case = Case::parse_toml(UTC).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let uu: Vec3 = [1.0, 0.5, 0.0];
    let (mut u_dev, mut p_dev) = (0.0f64, 0.0f64);
    let mut coinc = vec![];
    for k in 1..=40usize {
        sim.step();
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        let m = &sim.geom.mesh;
        let nfaces = sim.sliding_interface_faces().unwrap().len();
        let u = &sim.velocity().values;
        let du = u.iter().map(|x| vec3::norm(vec3::sub(*x, uu))).fold(0.0, f64::max) / vec3::norm(uu);
        let p = sim.field("p").unwrap();
        let v = m.cell_volumes();
        let pbar = p.iter().zip(v).map(|(a, b)| a * b).sum::<f64>() / v.iter().sum::<f64>();
        let dp = p.iter().map(|x| (x - pbar).abs()).fold(0.0, f64::max) / vec3::dot(uu, uu);
        println!("UCt step {k:2}: {nfaces} interface faces, velocity {du:.3e}, pressure {dp:.3e}");
        if nfaces != 264 {
            coinc.push(k);
        }
        u_dev = u_dev.max(du);
        p_dev = p_dev.max(dp);
    }
    let ok = u_dev <= 1e-10 && p_dev <= 1e-10;
    println!(
        "A1t: every step velocity {u_dev:.3e}, pressure {p_dev:.3e} (1e-10 each); coincidence steps {coinc:?} -> {}",
        if ok { "met" } else { "MISSED" }
    );
    assert!(ok);
}
