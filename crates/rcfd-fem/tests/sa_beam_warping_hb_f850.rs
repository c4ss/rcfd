//! F-850 (ADR 0778), check (2): the check HB- of ADR 0585 (no bending in the harmonic response of the fork beam under an
//! axial pre-stress and a torque), in a file of its own so that the committed mutation of the warping geometric stiffness
//! (`sa_beam_warping_hb_f850.mutation.patch`: the axial force placed in the slot of the moment `M_y`) is caught by it and
//! by nothing else. The set-up is that of H1- and HB- of `sa_beam_warping_f661.rs`, copied.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    load_vector, solve_static, Warping, WarpingHarmonic, WarpingHarmonicOptions, WarpingPrestress, WarpingSection,
};
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const L: f64 = 3.0;
const T: f64 = 1000.0;
const N: usize = 16;

/// The limit of HB- in ADR 0585.
const LIMIT: f64 = 1e-10;

/// The fork beam along `x`: `u = 0` at node 0, `v = w = phi = 0` at both ends, warping free. No load.
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
    mesh.add_node_set("mid", vec![N / 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap())
        .unwrap();
    for c in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    for c in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", c, 0.0)).unwrap();
    }
    m
}

#[test]
fn hb_no_bending_in_the_response_under_axial_prestress() {
    let (s, ws): (BeamSection, WarpingSection) =
        WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, 5.0 / 6.0).unwrap();
    let w = Warping::new().block(0, ws);
    let k = PI / L;
    let g = E / (2.0 * (1.0 + NU));
    let ip = s.i_y() + s.i_z();
    // |N_T| = (G J + E C_w k^2) A/I_p; the pre-stress of HB- is N = -0.5 |N_T| (compression).
    let n_t = (g * s.torsion() + E * ws.c_w() * k * k) * s.area() / ip;
    let n_ax = -0.5 * n_t;
    // C4 of ADR 0585: omega_1^2 = (G J k^2 + N I_p/A k^2 + E C_w k^4)/(rho I_p).
    let omega_1 =
        ((g * s.torsion() * k * k + E * ws.c_w() * k.powi(4) + n_ax * ip / s.area() * k * k) / (RHO * ip)).sqrt();
    println!("N_T {n_t:.6e}, N {n_ax:.6e}, omega_1(N) {omega_1:.6e}");

    // The pre-stress: the warping static solve of the fork beam under the axial force alone.
    let mut axial = fork(&s);
    axial.add_load(Load::nodal_force("xl", [n_ax, 0.0, 0.0])).unwrap();
    let r0 = solve_static(&axial, &w, None).unwrap();
    let p = WarpingPrestress::from_static(&axial, &w, &r0, [0.0; 2]).unwrap();

    // The harmonic model: the midspan torque, no damping, the response at 0.5 omega_1(N) (H1- of ADR 0585).
    let mut m = fork(&s);
    m.add_load(Load::nodal_moment("mid", [T, 0.0, 0.0])).unwrap();
    let load = HarmonicLoad::real(load_vector(&m, &w).unwrap()).unwrap();
    let o = WarpingHarmonicOptions::new().loss_factor(0.0).prestress(p);
    let h = WarpingHarmonic::new(&m, &w, &o).unwrap();
    let r = h.response(&load, 0.5 * omega_1).unwrap();

    let dofs = m.dof_map();
    let mut bend = 0.0f64;
    for node in 0..=N {
        for comp in [0, 1, 2, 4, 5] {
            if let Some(e) = dofs.equation(node, comp) {
                assert!(
                    r.re[e].is_finite() && r.im[e].is_finite(),
                    "HB- MISSED: a bending component is not finite"
                );
                bend = bend.max(r.re[e].abs()).max(r.im[e].abs());
            }
        }
    }
    let tmax = (0..=N)
        .map(|node| r.re[dofs.equation(node, 3).unwrap()].abs())
        .fold(0.0, f64::max);
    assert!(
        tmax.is_finite() && tmax > 0.0,
        "the largest twist {tmax:e} is not finite and positive"
    );
    let ratio = bend / tmax;
    let ok = ratio.is_finite() && ratio <= LIMIT;
    println!(
        "HB- bending over twist: {ratio:.6e} (bending {bend:.6e}, twist {tmax:.6e}; limit <= {LIMIT:e}) {}",
        if ok { "met" } else { "MISSED" }
    );
    assert!(ok, "HB- MISSED: bending over twist {ratio:e} > {LIMIT:e}");
}
