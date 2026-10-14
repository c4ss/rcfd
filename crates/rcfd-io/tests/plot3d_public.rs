//! Plot3D reader on a public sample (F-119, ADR 0011 P5): the blunt-fin grid `bluntfinxyz.bin` of the VTK example
//! data (single block 40 x 32 x 32, single precision, big endian, no record markers), fetched with
//! `python validation-data/fetch.py fetch plot3d-bluntfin`, never vendored; the test skips when it is absent.
//!
//! Oracle: VTK 9.6.2 `vtkMultiBlockPLOT3DReader` (run by scripts/check_interchange.py) reports the block dimensions
//! (40, 32, 32) and the bounds x [-7.815747261047363, 14.362203598022461], y [0, 8.327558517456055],
//! z [0, 5.7242512702941895]. Registered criteria: same dimensions, bounds equal exactly (both read the same float32
//! values).

use rcfd_io::datapack::{FileState, Manifest};
use rcfd_io::plot3d;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn reads_the_public_blunt_fin_grid() {
    let data = root().join("validation-data").join("data");
    let path = data.join("plot3d-bluntfin").join("bluntfinxyz.bin");
    if !path.exists() {
        eprintln!(
            "skipped: {} not fetched (python validation-data/fetch.py fetch plot3d-bluntfin)",
            path.display()
        );
        return;
    }
    let manifest = Manifest::load(&root().join("validation-data").join("manifest.json")).unwrap();
    for (id, name, st) in manifest.verify(&root(), &data) {
        if id == "plot3d-bluntfin" {
            assert!(matches!(st, FileState::Ok), "{name}: {st:?}");
        }
    }
    let g = plot3d::read_file(&path).unwrap();
    assert_eq!((g.dim, g.blocks.len()), (3, 1));
    let b = &g.blocks[0];
    assert_eq!((b.ni, b.nj, b.nk), (40, 32, 32));
    let bounds = |v: &[f64]| {
        (
            v.iter().cloned().fold(f64::INFINITY, f64::min),
            v.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    assert_eq!(bounds(&b.x), (-7.815747261047363, 14.362203598022461));
    assert_eq!(bounds(&b.y), (0.0, 8.327558517456055));
    assert_eq!(bounds(&b.z), (0.0, 5.7242512702941895));
    // The grid has a collapsed (singular) edge at the fin leading edge, so cells with a zero-area face exist: the
    // conversion to a polyhedral mesh refuses it with a located message instead of building a broken mesh.
    // (Not a registered criterion: recorded as a limitation of block_to_mesh.)
    let e = plot3d::block_to_mesh(&g, 0).unwrap_err().to_string();
    assert!(e.contains("cell") && e.contains("degenerate"), "{e}");
}
