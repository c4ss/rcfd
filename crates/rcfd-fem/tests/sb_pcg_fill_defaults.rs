//! ADR 0357 (F-460), check D1: hashes of the default solve and of the existing iterative solvers of the fast cantilever of
//! ADR 0282. Uses only the API of main 0a226bda, so the same file is compiled there for the comparison.

use rcfd_fem::builders::hex_box;
use rcfd_fem::solve::{solve_static, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_linalg::precond::IcOptions;

fn cantilever() -> SolidModel {
    let mesh = hex_box([0.0; 3], [8.0, 1.5, 1.5], [32, 6, 6]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 0.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1.0e6])).unwrap();
    m
}

fn fnv1a(values: &[f64], acc: &mut u64) {
    for v in values {
        for b in v.to_bits().to_le_bytes() {
            *acc ^= b as u64;
            *acc = acc.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

#[test]
fn d1_default_and_existing_solvers_hash() {
    let model = cantilever();
    let pcg = |p: CgPreconditioner| {
        StaticOptions::with_solver(StaticSolver::PreconditionedCg {
            preconditioner: p,
            tolerance: 1e-10,
            max_iterations: 20_000,
        })
    };
    for (tag, o) in [
        ("direct", StaticOptions::default()),
        (
            "cg-jacobi",
            StaticOptions::with_solver(StaticSolver::ConjugateGradient {
                tolerance: 1e-10,
                max_iterations: 20_000,
            }),
        ),
        ("cg-block-jacobi", pcg(CgPreconditioner::BlockJacobi)),
        (
            "cg-ic0",
            pcg(CgPreconditioner::IncompleteCholesky(IcOptions::default())),
        ),
    ] {
        match solve_static(&model, &o) {
            Ok(r) => {
                let mut h = 0xcbf2_9ce4_8422_2325u64;
                fnv1a(r.displacement_values(), &mut h);
                fnv1a(r.reactions().unwrap(), &mut h);
                let s = r.stats().unwrap();
                fnv1a(
                    &[s.relative_residual(), s.iterations() as f64, s.unknowns() as f64],
                    &mut h,
                );
                println!(
                    "D1 {tag}: solver '{}', iterations {}, hash {h:016x}",
                    s.solver(),
                    s.iterations()
                );
            }
            Err(e) => println!("D1 {tag}: error '{e}'"),
        }
    }
}
