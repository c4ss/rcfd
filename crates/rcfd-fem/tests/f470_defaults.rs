//! ADR 0370 D1: the default elasticity multigrid route (ADR 0305) on fast cases: count, level orders and an FNV-1a hash
//! of the displacement bits, printed; run on this branch and on main 7a533a53 and compared line by line. Uses only API
//! that exists on main.

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mesh::SolidMesh;
use rcfd_fem::solve::{smoothed_aggregation, solve_static, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_linalg::amg_elastic::ElasticAmgSettings;

fn model(mesh: SolidMesh, symmetric: bool) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 0.0, 0.0).unwrap())
        .unwrap();
    if symmetric {
        m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
        m.add_load(Load::traction("xmax", [1.0e6, 0.0, 0.0])).unwrap();
    } else {
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
        m.add_load(Load::traction("xmax", [0.0, 0.0, -1.0e6])).unwrap();
    }
    m
}

fn fnv(bits: impl Iterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bits {
        for byte in b.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

#[test]
fn d1_default_route_hashes() {
    let (tet, _) = upgrade_to_tet10(&tet_box([0.0; 3], [8.0, 1.5, 1.5], [32, 6, 6]).unwrap()).unwrap();
    let cases = [
        (
            "cube 11",
            model(hex_box([0.0; 3], [1.0; 3], [11; 3]).unwrap(), false),
            1e-10,
        ),
        (
            "cube 23",
            model(hex_box([0.0; 3], [1.0; 3], [23; 3]).unwrap(), false),
            1e-10,
        ),
        (
            "fast Hex8 32x6x6",
            model(hex_box([0.0; 3], [8.0, 1.5, 1.5], [32, 6, 6]).unwrap(), false),
            1e-10,
        ),
        (
            "P1 symmetry cube 11",
            model(hex_box([0.0; 3], [1.0; 3], [11; 3]).unwrap(), true),
            1e-10,
        ),
        ("fast Tet10 32x6x6", model(tet, false), 1e-8),
    ];
    for (tag, m, tol) in cases {
        let dofs = m.dof_map();
        let fixed = prescribed_equations(&m, &[]).unwrap();
        let k = stiffness_with(&m, None).unwrap();
        let f = nodal_load_vector(&m, &[]).unwrap();
        let sys = reduce(&k, &f, &fixed).unwrap();
        let amg = smoothed_aggregation(
            &sys.matrix,
            sys.free(),
            &dofs,
            m.mesh().nodes(),
            &ElasticAmgSettings::default(),
        )
        .unwrap();
        let orders: Vec<usize> = (0..amg.n_levels()).map(|l| amg.matrix(l).n).collect();
        let o = StaticOptions::with_solver(StaticSolver::PreconditionedCg {
            preconditioner: CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default()),
            tolerance: tol,
            max_iterations: 60_000,
        });
        let r = solve_static(&m, &o).unwrap();
        let h = fnv(r.displacement_values().iter().map(|v| v.to_bits()));
        println!(
            "D1 {tag}: levels {orders:?}, {} iterations, residual {:e}, hash {h:016x}",
            r.stats().unwrap().iterations(),
            r.stats().unwrap().relative_residual()
        );
    }
}
