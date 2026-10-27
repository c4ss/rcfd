//! ADR 0128 finding 8: an upload larger than the device's buffer limit is a located error, not a wgpu panic. Runs on
//! every DX12 / Vulkan adapter; with no adapter it prints a located SKIP line and passes.

use rcfd_render::{adapters, vertex_bytes_check, Field, Renderer, SurfaceMesh, VERTEX_BYTES};

/// Unit square as `n` vertical strips (2 triangles each), points shared between strips.
fn strips(n: usize) -> SurfaceMesh {
    let mut pts = Vec::with_capacity(2 * n + 2);
    for i in 0..=n {
        let x = i as f64 / n as f64;
        pts.push([x, 0.0, 0.0]);
        pts.push([x, 1.0, 0.0]);
    }
    let faces: Vec<Vec<usize>> = (0..n).map(|i| vec![2 * i, 2 * i + 2, 2 * i + 3, 2 * i + 1]).collect();
    SurfaceMesh::new(pts, &faces, vec![0; n], vec!["square".into()]).unwrap()
}

#[test]
fn vertex_bytes_check_bounds() {
    let t = 3 * VERTEX_BYTES;
    assert_eq!(vertex_bytes_check(10, 10 * t).unwrap(), 10 * t);
    assert_eq!(vertex_bytes_check(10, 10 * t + 1).unwrap(), 10 * t);
    assert!(vertex_bytes_check(10, 10 * t - 1).is_err());
    assert!(vertex_bytes_check(u64::MAX / 2, u64::MAX).is_err(), "overflow");
    let e = vertex_bytes_check(11, 10 * t).unwrap_err().to_string();
    assert!(
        e.contains("max_buffer_size") && e.contains(&(10 * t).to_string()),
        "{e}"
    );
}

#[test]
fn u1_upload_over_the_buffer_limit_is_an_error() {
    let rs: Vec<Renderer> = adapters()
        .iter()
        .filter_map(|a| Renderer::from_adapter(a).ok())
        .collect();
    if rs.is_empty() {
        eprintln!(
            "SKIP {}:{}: no wgpu adapter (DX12 or Vulkan) on this machine",
            file!(),
            line!()
        );
        return;
    }
    for r in &rs {
        let limit = r.max_buffer_size();
        let tris = limit / (3 * VERTEX_BYTES) + 1; // one triangle over the limit
        let s = strips(tris.div_ceil(2) as usize);
        let got = r.upload(&s, Field::None);
        eprintln!(
            "U1 {}: max_buffer_size {limit}, {} triangles: {:?}",
            r.info().name,
            s.triangles().count(),
            got.as_ref().err()
        );
        assert!(matches!(got, Err(rcfd_core::Error::Mesh(_))), "{}", r.info().name);
        // control: a small surface uploads
        assert!(r.upload(&strips(4), Field::None).is_ok());
    }
}
