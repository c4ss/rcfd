//! F-117: GLB export of boundary surfaces with scalar colouring.
//! Checks fixed in advance: container validity (structural self-check), outward winding,
//! colours equal the viridis map of the normalised value, and size linear in the face count.

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_post::gltf::{check_glb, glb_patches, wrong_winding, GltfOptions, GltfScalar};
use rcfd_post::render::colormap;

const ALL: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

fn boxm(n: usize) -> rcfd_mesh::Mesh {
    box_mesh([n, n, n], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap()
}

#[test]
fn contour_glb_is_valid_with_correct_colours() {
    let mesh = boxm(4);
    let x: Vec<f64> = mesh.cell_centres().iter().map(|c| c[0]).collect();
    let opt = GltfOptions {
        range: None,
        scalar_name: "x".into(),
    };
    let (bytes, st) = glb_patches(&mesh, &ALL, Some(GltfScalar::Cell(&x)), &opt).unwrap();
    let doc = check_glb(&bytes).expect("valid GLB");
    assert_eq!(st.faces, 6 * 16);
    assert_eq!(st.triangles, 6 * 16 * 2);
    assert_eq!(st.vertices, 6 * 16 * 4);
    assert_eq!(
        wrong_winding(&bytes),
        0,
        "triangles must be counter-clockwise seen from outside"
    );
    assert_eq!(doc["meshes"][0]["primitives"].as_array().unwrap().len(), 6);
    assert_eq!(doc["asset"]["extras"]["scalar"], "x");

    // every vertex colour equals the colour map of its stored value
    let jl = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let bin = &bytes[28 + jl..];
    let (lo, hi) = st.range;
    assert!(
        (lo - 0.125).abs() < 1e-12 && (hi - 0.875).abs() < 1e-12,
        "range {lo} {hi}"
    );
    let mut seen_lo = false;
    let mut seen_hi = false;
    for p in doc["meshes"][0]["primitives"].as_array().unwrap() {
        let ca = &doc["accessors"][p["attributes"]["COLOR_0"].as_u64().unwrap() as usize];
        let sa = &doc["accessors"][p["attributes"]["_SCALAR"].as_u64().unwrap() as usize];
        assert_eq!(ca["componentType"], 5121);
        assert_eq!(ca["normalized"], true);
        let co = doc["bufferViews"][ca["bufferView"].as_u64().unwrap() as usize]["byteOffset"]
            .as_u64()
            .unwrap() as usize;
        let so = doc["bufferViews"][sa["bufferView"].as_u64().unwrap() as usize]["byteOffset"]
            .as_u64()
            .unwrap() as usize;
        for k in 0..ca["count"].as_u64().unwrap() as usize {
            let v = f32::from_le_bytes(bin[so + 4 * k..so + 4 * k + 4].try_into().unwrap()) as f64;
            let rgb = colormap((v - lo) / (hi - lo));
            assert_eq!(&bin[co + 4 * k..co + 4 * k + 4], &[rgb[0], rgb[1], rgb[2], 255]);
            seen_lo |= rgb == colormap(0.0);
            seen_hi |= rgb == colormap(1.0);
        }
    }
    assert!(seen_lo && seen_hi, "extremes of the range must appear");
}

#[test]
fn size_is_linear_in_face_count() {
    let opt = GltfOptions::default();
    let mut rows = Vec::new();
    for n in [4usize, 8, 16, 32] {
        let mesh = boxm(n);
        let v: Vec<f64> = mesh.cell_centres().iter().map(|c| c[1]).collect();
        let (b, st) = glb_patches(&mesh, &ALL, Some(GltfScalar::Cell(&v)), &opt).unwrap();
        check_glb(&b).unwrap();
        rows.push((st.faces as f64, b.len() as f64));
    }
    // incremental bytes per face between consecutive refinements agree within 1 %
    let slopes: Vec<f64> = rows.windows(2).map(|w| (w[1].1 - w[0].1) / (w[1].0 - w[0].0)).collect();
    for s in &slopes {
        assert!((s / slopes[0] - 1.0).abs() < 0.01, "slopes {slopes:?}");
    }
    // quad = 4 vertices x 32 bytes (pos 12 + normal 12 + colour 4 + scalar 4) + 6 indices x 4 bytes = 152
    assert!((slopes[0] - 152.0).abs() < 2.0, "{slopes:?}");
}

#[test]
fn explicit_range_face_values_and_errors() {
    let mesh = boxm(3);
    let nf = mesh.n_faces();
    let fv: Vec<f64> = (0..nf).map(|f| f as f64).collect();
    let opt = GltfOptions {
        range: Some((0.0, nf as f64)),
        scalar_name: "id".into(),
    };
    let (b, st) = glb_patches(&mesh, &["xmin"], Some(GltfScalar::Face(&fv)), &opt).unwrap();
    check_glb(&b).unwrap();
    assert_eq!(st.range, (0.0, nf as f64));
    assert!(glb_patches(&mesh, &["nope"], None, &opt).is_err());
    assert!(glb_patches(&mesh, &[], None, &opt).is_err());
    let bad = GltfOptions {
        range: Some((1.0, 1.0)),
        scalar_name: String::new(),
    };
    assert!(glb_patches(&mesh, &["xmin"], Some(GltfScalar::Face(&fv)), &bad).is_err());
    assert!(glb_patches(&mesh, &["xmin"], Some(GltfScalar::Cell(&[0.0])), &opt).is_err());
    // geometry only: no colour attributes
    let (g, _) = glb_patches(&mesh, &["xmin"], None, &GltfOptions::default()).unwrap();
    let d = check_glb(&g).unwrap();
    assert!(d["meshes"][0]["primitives"][0]["attributes"]["COLOR_0"].is_null());
}

#[test]
fn corrupted_files_are_rejected_by_the_check() {
    let mesh = boxm(2);
    let (b, _) = glb_patches(&mesh, &["zmax"], None, &GltfOptions::default()).unwrap();
    assert!(check_glb(&b[..b.len() - 4]).is_err());
    let mut m = b.clone();
    m[0] = b'X';
    assert!(check_glb(&m).is_err());
    let mut l = b.clone();
    l[8] ^= 1;
    assert!(check_glb(&l).is_err());
}
