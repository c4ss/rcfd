//! F-850 (ADR 0778), check (1): a running assertion of the 16-element lateral-torsional buckling moments of the monosymmetric
//! I of ADR 0553 (A1' and A2', asserted there only in ignored records) against the closed form with beta_x, within 2e-2.
//! Rows A1'', A2'' and A3'' of ADR 0778. The set-up is that of `sa_beam_warping_f627.rs` (ADR 0553 amendment 2), copied.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{buckling_factors, Warping, WarpingSection};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const KAPPA: f64 = 5.0 / 6.0;
const M_REF: f64 = 1000.0;
const L: f64 = 3.0;
const N: usize = 16;

/// The closed form as ADR 0553 arithmetic 2 prints it: `|M|` with flange 1 (the larger) in compression and in tension.
const M_COMPRESSED_ADR0553: f64 = 1.78454564e5;
const M_TENSION_ADR0553: f64 = 5.39587954e4;

/// `z` of the mid-plane of flange 1 from the centroid (ADR 0553 D5).
const Z_1: f64 = 7.1868131868e-2;

/// The limit of the acceptance of F-850, clause (1).
const LIMIT: f64 = 2e-2;

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

/// The monosymmetric I of ADR 0553: flange 1 (`0.15 x 0.012`, the larger) on `+e_3`.
fn mono() -> (BeamSection, WarpingSection) {
    WarpingSection::monosymmetric_i(0.15, 0.012, 0.08, 0.008, 0.2, 0.006, KAPPA).unwrap()
}

/// The fork beam along `x`, orientation `y`: `u = 0` at node 0, `v = w = phi = 0` at both ends, warping free.
fn fork(section: &BeamSection) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=N).map(|i| [L * i as f64 / N as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..N).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(*section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..N).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![N]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap())
        .unwrap();
    for k in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", k, 0.0)).unwrap();
    }
    for k in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", k, 0.0)).unwrap();
    }
    m
}

/// One load case: the lowest factor and the fibre strain of flange 1 from the static solution (`z_1 dtheta_y/dx`).
fn ltb(section: &BeamSection, ws: WarpingSection, s: f64) -> (f64, f64) {
    let m = fork(section);
    let mut loaded = m.clone();
    loaded
        .add_load(Load::nodal_moment("x0", [0.0, -s * M_REF, 0.0]))
        .unwrap();
    loaded
        .add_load(Load::nodal_moment("xl", [0.0, s * M_REF, 0.0]))
        .unwrap();
    let r = rcfd_fem::solve::solve_static(&loaded, &StaticOptions::default()).unwrap();
    let u = r.displacement_values().to_vec();
    let pre = Prestress::from_static(&loaded, &r)
        .unwrap()
        .with_beam_resultants(&loaded, &u, [0.0; 2])
        .unwrap();
    // ADR 0553 amendment 2: three factors, a subspace of 40.
    let mut opts = BucklingOptions::new(3);
    opts.subspace_size = Some(40);
    let b = buckling_factors(&m, &pre, &Warping::new().block(0, ws), &opts).unwrap();
    let dofs = m.dof_map();
    let theta_y = |node: usize| u[dofs.equation(node, 4).unwrap()];
    let strain1 = Z_1 * (theta_y(N) - theta_y(0)) / L;
    println!("sign {s:+}: factors {:?}, flange-1 strain {strain1:.4e}", b.factors);
    (b.factors[0], strain1)
}

/// The closed form of ADR 0553 D3 from the section constants: `|M|` with the larger flange in compression, in tension.
fn closed(section: &BeamSection, ws: &WarpingSection) -> (f64, f64) {
    let k = PI / L;
    let py = E * section.i_z() * k * k;
    let g = E / (2.0 * (1.0 + NU));
    let gjw = g * section.torsion() + E * ws.c_w() * k * k;
    let bx = ws.wagner_about_shear_centre()[0];
    let root = ((bx * py / 2.0).powi(2) + py * gjw).sqrt();
    (bx.abs() * py / 2.0 + root, -bx.abs() * py / 2.0 + root)
}

#[test]
fn a1_a2_sixteen_element_moments_against_the_closed_form() {
    let (s, ws) = mono();
    let (big, small) = closed(&s, &ws);
    println!("closed form from the section: compressed {big:.8e}, tension {small:.8e}");
    let (f_p, e_p) = ltb(&s, ws, 1.0);
    let (f_m, e_m) = ltb(&s, ws, -1.0);
    // A3'': one load case each way, the larger moment the compressed one.
    assert!(
        e_p.is_finite() && e_m.is_finite() && (e_p < 0.0) != (e_m < 0.0),
        "A3'' MISSED: flange-1 strains {e_p:e} and {e_m:e} do not put flange 1 one in compression and one in tension"
    );
    let (f_c, f_t) = if e_p < 0.0 { (f_p, f_m) } else { (f_m, f_p) };
    let (m_c, m_t) = (f_c * M_REF, f_t * M_REF);
    println!("factors: compressed {f_c:.15e}, tension {f_t:.15e}");
    assert!(
        m_c > m_t,
        "A3'' MISSED: compressed {m_c:e} is not above tension {m_t:e}"
    );
    println!("A3'': met ({m_c:.6e} > {m_t:.6e})");
    let mut missed = Vec::new();
    for (name, value, reference) in [
        ("A1'' compressed, closed form of the test", m_c, big),
        ("A1'' compressed, ADR 0553 digits", m_c, M_COMPRESSED_ADR0553),
        ("A2'' tension, closed form of the test", m_t, small),
        ("A2'' tension, ADR 0553 digits", m_t, M_TENSION_ADR0553),
    ] {
        let err = rel(value, reference);
        let ok = err.is_finite() && err <= LIMIT;
        println!(
            "{name}: {err:.4e} ({value:.8e} against {reference:.8e}; limit <= {LIMIT:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            missed.push(format!("{name} = {err:e} > {LIMIT:e}"));
        }
    }
    assert!(missed.is_empty(), "missed: {missed:#?}");
}
