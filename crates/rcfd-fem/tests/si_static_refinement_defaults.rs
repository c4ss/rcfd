//! F-455 (ADR 0397) check A7 (a): the digest of the default static solve of the twelve cantilevers of D4 (Tet4, Hex8,
//! Tet10; L/h = 5, 10, 20, 40). Uses only the API that existed before F-455, so the same file gives the reference digest on
//! main e9f76a1e (amendment 1) and the check on the branch.

use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::mesh::ElementKind;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};

const KINDS: [ElementKind; 3] = [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10];
const SLENDERNESS: [usize; 4] = [5, 10, 20, 40];
const H: f64 = 0.2;

/// The cantilever of D4: section 0.2 x 0.2 m, length s h, divisions [s, 2, 2], clamped on xmin, tip traction on xmax.
fn cantilever(kind: ElementKind, s: usize) -> SolidModel {
    let lengths = [s as f64 * H, H, H];
    let div = [s, 2, 2];
    let mesh = match kind {
        ElementKind::Hex8 => hex_box([0.0; 3], lengths, div).unwrap(),
        ElementKind::Tet4 => tet_box([0.0; 3], lengths, div).unwrap(),
        _ => upgrade_to_tet10(&tet_box([0.0; 3], lengths, div).unwrap()).unwrap().0,
    };
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::traction("xmax", [0.0, 0.0, -1e6])).unwrap();
    model
}

fn fnv(h: &mut u64, bits: u64) {
    for b in bits.to_le_bytes() {
        *h ^= u64::from(b);
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// The digest of the bits of the displacement, the reactions and the relative residual of the default solve of all twelve
/// cases, in the order Tet4, Hex8, Tet10 and s = 5, 10, 20, 40.
fn default_digest() -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for kind in KINDS {
        for s in SLENDERNESS {
            let res = solve_static(&cantilever(kind, s), &StaticOptions::default()).unwrap();
            for v in res.displacement_values() {
                fnv(&mut h, v.to_bits());
            }
            for v in res.reactions().unwrap() {
                fnv(&mut h, v.to_bits());
            }
            let st = res.stats().unwrap();
            fnv(&mut h, st.relative_residual().to_bits());
            println!(
                "{} s = {s}: solver {}, relative residual {:.3e}",
                kind.name(),
                st.solver(),
                st.relative_residual()
            );
        }
    }
    h
}

/// The digest of main e9f76a1e (ADR 0397 amendment 1), recorded before any library line of F-455.
const MAIN_DIGEST: Option<u64> = Some(0xb9d9_056b_0c5a_7aff);

#[test]
fn a7a_default_digest() {
    let d = default_digest();
    println!("A7 (a): default digest {d:016x}");
    if let Some(m) = MAIN_DIGEST {
        assert_eq!(d, m, "A7 (a): the default bits changed");
    }
}
