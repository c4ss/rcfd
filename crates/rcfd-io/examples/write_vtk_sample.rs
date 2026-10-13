//! Writes a small cavity mesh with a known analytic field to the directory given as argument
//! (used to check the VTK writers against a reference reader).

use rcfd_io::vtk::{write_vtp, write_vtu, VtkField, VtkValues};
use rcfd_mesh::gen::cavity_2d;

fn main() {
    let dir = std::env::args().nth(1).expect("usage: write_vtk_sample <dir>");
    let mesh = cavity_2d(4, 1.0).expect("mesh");
    let p: Vec<f64> = mesh.cell_centres().iter().map(|c| 2.0 * c[0] + 3.0 * c[1]).collect();
    let u: Vec<[f64; 3]> = mesh.cell_centres().iter().map(|c| [c[1], -c[0], 0.0]).collect();
    let fields = [
        VtkField {
            name: "p",
            values: VtkValues::Scalar(&p),
        },
        VtkField {
            name: "U",
            values: VtkValues::Vector(&u),
        },
    ];
    write_vtu(format!("{dir}/cavity.vtu"), &mesh, &fields).expect("vtu");
    write_vtp(format!("{dir}/lid.vtp"), &mesh, "lid", &[]).expect("vtp");
    std::fs::write(
        format!("{dir}/cavity.vtk"),
        rcfd_io::vtk::legacy_vtk_string(&mesh, &fields).expect("legacy"),
    )
    .expect("write");
}
