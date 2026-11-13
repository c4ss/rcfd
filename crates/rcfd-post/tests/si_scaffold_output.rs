//! F-388 (ADR 0291): the output mapping of the intermediate tier's kinds (check O1) and the existence of the two
//! rcfd-post modules (check X1, rcfd-post part).

use rcfd_core::vec3::Vec3;
use rcfd_fem::ElementKind;
use rcfd_io::vtk::{
    fe_vtu_string, vtk_cell_nodes, FeCellBlock, VTK_LINE, VTK_QUAD, VTK_QUADRATIC_HEXAHEDRON, VTK_TETRA,
};
use rcfd_post::solid_output::vtk_from_fem;

#[test]
fn x1_post_modules_exist() {
    #[allow(unused_imports)]
    use rcfd_post::{solid_dynamic, solid_fsi};
    println!("X1 (rcfd-post): solid_dynamic and solid_fsi exist");
}

/// The whitespace-separated tokens of the `DataArray` named `name`.
fn tokens(xml: &str, name: &str) -> Vec<String> {
    let at = xml.find(&format!("Name=\"{name}\"")).expect("array present");
    let head_end = xml[at..].find('>').unwrap() + at;
    let end = xml[head_end..].find("</DataArray>").unwrap() + head_end;
    xml[head_end + 1..end].split_whitespace().map(str::to_string).collect()
}

#[test]
fn o1_vtk_mapping_of_the_new_kinds() {
    let identity: Vec<usize> = (0..20).collect();
    assert_eq!(vtk_from_fem(ElementKind::Hex20).unwrap(), (25, &identity[..]));
    assert_eq!(vtk_from_fem(ElementKind::Beam2).unwrap(), (3, &[0usize, 1][..]));
    assert_eq!(vtk_from_fem(ElementKind::Shell4).unwrap(), (9, &[0usize, 1, 2, 3][..]));
    assert_eq!(
        (VTK_LINE, VTK_QUAD, VTK_QUADRATIC_HEXAHEDRON),
        (3, 9, 25),
        "the VTK type numbers"
    );
    assert_eq!(
        [vtk_cell_nodes(3), vtk_cell_nodes(9), vtk_cell_nodes(25)],
        [Some(2), Some(4), Some(20)]
    );
    let points: Vec<Vec3> = (0..20).map(|a| [a as f64, 0.5 * a as f64, 1.0]).collect();
    let line = [0usize, 1];
    let quad = [0usize, 1, 2, 3];
    let hex20: Vec<usize> = (0..20).collect();
    let blocks = [
        FeCellBlock {
            cell_type: VTK_LINE,
            connectivity: &line,
        },
        FeCellBlock {
            cell_type: VTK_QUAD,
            connectivity: &quad,
        },
        FeCellBlock {
            cell_type: VTK_QUADRATIC_HEXAHEDRON,
            connectivity: &hex20,
        },
    ];
    let xml = fe_vtu_string(&points, &blocks, &[], &[]).unwrap();
    assert_eq!(tokens(&xml, "types"), ["3", "9", "25"]);
    assert_eq!(tokens(&xml, "offsets"), ["2", "6", "26"]);
    let bad = [FeCellBlock {
        cell_type: 42,
        connectivity: &quad,
    }];
    let e = fe_vtu_string(&points, &bad, &[], &[]).unwrap_err();
    println!("O1: type 42 refused: {e}");
    let tet = [FeCellBlock {
        cell_type: VTK_TETRA,
        connectivity: &quad,
    }];
    assert!(fe_vtu_string(&points, &tet, &[], &[]).is_ok());
    println!("O1: types 3, 9, 25 mapped and written");
}
