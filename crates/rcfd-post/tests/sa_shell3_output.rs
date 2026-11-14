//! F-419 (ADR 0454): check PO1, Shell3 in the shell output of ADR 0437: the cell arrays of a mixed Shell3 + Shell4 result
//! equal `rcfd_fem::shell::stresses` bit for bit, and the `.vtu` writes Shell3 as `VTK_TRIANGLE` (5) and Shell4 as `VTK_QUAD`
//! (9); a Shell3-only model is written the same way.

use rcfd_fem::constraint::Prescribed;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::shell::stresses;
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{BlockProperty, ElementKind, Material, SolidMesh, SolidModel};
use rcfd_post::solid_output::{shell_fields, solid_vtu_string};

fn types(xml: &str) -> Vec<usize> {
    let key = "Name=\"types\"";
    let at = xml.find(key).expect("types array");
    let head_end = xml[at..].find('>').unwrap() + at;
    let end = xml[head_end..].find("</DataArray>").unwrap() + head_end;
    xml[head_end + 1..end]
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect()
}

fn bits(a: &[f64]) -> Vec<u64> {
    a.iter().map(|x| x.to_bits()).collect()
}

/// A strip of two cells on `[0, 2] x [0, 1]`: cell 0 a Shell4 (when `mixed`) or cut, cell 1 cut into two Shell3; clamped at
/// `x = 0`, loaded at `x = 2`.
fn strip(mixed: bool) -> (SolidModel, Vec<Prescribed>, Vec<NodalForceAt>) {
    let nodes = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [2.0, 1.0, 0.0],
    ];
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let mut tris = vec![1, 2, 5, 1, 5, 4];
    let mut b = 0;
    if mixed {
        mesh.add_block(ElementKind::Shell4, vec![0, 1, 4, 3]).unwrap();
        mesh.set_block_property(b, BlockProperty::shell(0.01).unwrap()).unwrap();
        b += 1;
    } else {
        tris.extend_from_slice(&[0, 1, 4, 0, 4, 3]);
    }
    let ne = if mixed { 3 } else { 4 };
    mesh.add_block(ElementKind::Shell3, tris).unwrap();
    mesh.set_block_property(b, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    let mut pres = Vec::new();
    for k in [0, 3] {
        for comp in 0..6 {
            pres.push(Prescribed::new(k, comp, 0.0));
        }
    }
    let forces = vec![
        NodalForceAt::new(2, [10.0, 0.0, 1.0]),
        NodalForceAt::new(5, [10.0, 0.0, 1.0]),
    ];
    (m, pres, forces)
}

#[test]
#[ignore = "record of the registered run of ADR 0454 (commit 39af6a49): the cell arrays met on the mixed model (bit for bit); solid_vtu_string refused cell type 5 (rcfd_io::vtk had no triangle); corrected by amendment 1 as po1b_shell3_in_the_shell_output"]
fn po1_shell3_in_the_shell_output_known_miss() {
    let mut missed = Vec::new();
    for mixed in [true, false] {
        let (m, pres, forces) = strip(mixed);
        let r = solve_static_with(&m, &pres, &forces, &StaticOptions::default()).unwrap();
        let f = shell_fields(&m, &r).unwrap();
        let st = stresses(&m, r.displacement_values()).unwrap();
        let mut ok = st.len() == m.mesh().n_elements();
        for (e, s) in &st {
            ok &= bits(&f.membrane_force[3 * e..3 * e + 3]) == bits(&s.membrane)
                && bits(&f.moment[3 * e..3 * e + 3]) == bits(&s.bending)
                && bits(&f.shear_force[2 * e..2 * e + 2]) == bits(&s.shear)
                && bits(&f.stress_top[3 * e..3 * e + 3]) == bits(&s.top)
                && bits(&f.stress_bottom[3 * e..3 * e + 3]) == bits(&s.bottom);
            for k in 0..3 {
                ok &= bits(&f.shell_axes[9 * e + 3 * k..9 * e + 3 * k + 3]) == bits(&s.axes[k]);
            }
        }
        println!("PO1 mixed {mixed}: cell arrays equal shell::stresses bit for bit: {ok}");
        if !ok {
            missed.push(format!("PO1 mixed {mixed}: cell arrays"));
        }
        let xml = solid_vtu_string(&m, &r).unwrap();
        let ty = types(&xml);
        let want = if mixed { vec![9, 5, 5] } else { vec![5, 5, 5, 5] };
        println!("PO1 mixed {mixed}: cell types {ty:?} (expected {want:?})");
        if ty != want {
            missed.push(format!("PO1 mixed {mixed}: cell types {ty:?}"));
        }
    }
    assert!(missed.is_empty(), "missed: {missed:#?}");
}

/// PO1 corrected (ADR 0454 amendment 1): the same test and limits after `rcfd_io::vtk` gained the triangle.
#[test]
fn po1b_shell3_in_the_shell_output() {
    let mut missed = Vec::new();
    for mixed in [true, false] {
        let (m, pres, forces) = strip(mixed);
        let r = solve_static_with(&m, &pres, &forces, &StaticOptions::default()).unwrap();
        let f = shell_fields(&m, &r).unwrap();
        let st = stresses(&m, r.displacement_values()).unwrap();
        let mut ok = st.len() == m.mesh().n_elements();
        for (e, s) in &st {
            ok &= bits(&f.membrane_force[3 * e..3 * e + 3]) == bits(&s.membrane)
                && bits(&f.moment[3 * e..3 * e + 3]) == bits(&s.bending)
                && bits(&f.shear_force[2 * e..2 * e + 2]) == bits(&s.shear)
                && bits(&f.stress_top[3 * e..3 * e + 3]) == bits(&s.top)
                && bits(&f.stress_bottom[3 * e..3 * e + 3]) == bits(&s.bottom);
            for k in 0..3 {
                ok &= bits(&f.shell_axes[9 * e + 3 * k..9 * e + 3 * k + 3]) == bits(&s.axes[k]);
            }
        }
        println!("PO1 mixed {mixed}: cell arrays equal shell::stresses bit for bit: {ok}");
        if !ok {
            missed.push(format!("PO1 mixed {mixed}: cell arrays"));
        }
        let xml = solid_vtu_string(&m, &r).unwrap();
        let ty = types(&xml);
        let want = if mixed { vec![9, 5, 5] } else { vec![5, 5, 5, 5] };
        println!("PO1 mixed {mixed}: cell types {ty:?} (expected {want:?})");
        if ty != want {
            missed.push(format!("PO1 mixed {mixed}: cell types {ty:?}"));
        }
    }
    assert!(missed.is_empty(), "missed: {missed:#?}");
}
