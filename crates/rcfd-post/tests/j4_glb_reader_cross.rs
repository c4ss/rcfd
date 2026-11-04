//! Job J4 (ADR 0184) registered check R11: a GLB written by the independent F-117 exporter (`rcfd_post::gltf`) is read
//! by the F-274 surface reader (`rcfd_io::gltf`).

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::surface::{diagnose, DiagOptions};
use rcfd_post::gltf::{glb_patches, GltfOptions, GltfScalar};

const ALL: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

#[test]
fn r11_glb_from_the_exporter_reads_back_as_the_closed_box() {
    let mesh = box_mesh([3, 2, 2], [1.5, 1.0, 0.5], [Grading::Uniform; 3]).unwrap();
    let area: f64 = ALL
        .iter()
        .map(|p| {
            mesh.patch(p)
                .unwrap()
                .faces()
                .map(|f| rcfd_core::vec3::norm(mesh.face_area_vectors()[f]))
                .sum::<f64>()
        })
        .sum();
    let volume = mesh.total_volume();
    let x: Vec<f64> = mesh.cell_centres().iter().map(|c| c[0]).collect();
    for scalar in [None, Some(GltfScalar::Cell(&x))] {
        let coloured = scalar.is_some();
        let (bytes, stats) = glb_patches(&mesh, &ALL, scalar, &GltfOptions::default()).unwrap();
        let s = rcfd_io::gltf::read_bytes(&bytes, "m", rcfd_io::gltf::no_external).unwrap();
        eprintln!(
            "J4 R11 coloured {coloured}: {} triangles written, {} read, {} vertices after welding, notes {:?}",
            stats.triangles,
            s.surface.n_tris(),
            s.surface.points.len(),
            s.notes
        );
        assert_eq!(s.surface.n_tris(), stats.triangles);
        assert_eq!(s.patches.len(), 1);
        let d = diagnose(&s.surface, &DiagOptions::default());
        assert!(d.is_closed_manifold(), "{d:?}");
        assert_eq!(d.euler, 2);
        let (a, v) = (s.surface.area(), s.surface.enclosed_volume());
        eprintln!(
            "J4 R11 area {a:e} (mesh {area:e}), volume {v:e} (mesh {volume:e}), relative errors {:e} {:e}",
            (a - area).abs() / area,
            (v - volume).abs() / volume
        );
        assert!((a - area).abs() <= 1e-6 * area, "{a} {area}");
        assert!((v - volume).abs() <= 1e-6 * volume, "{v} {volume}");
    }
}
