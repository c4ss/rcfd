//! Fluent reader on a real Fluent-written mesh from a public source (F-118).
//!
//! The file (`elbow.msh`, OpenFOAM-11 tutorial, GPL) is not vendored: it is fetched with
//! `python validation-data/fetch.py fetch fluent-elbow-openfoam`, checked against the SHA-256 in
//! the manifest, and the test is skipped (with a message) when it is absent, so default CI never
//! needs it or the network.
//!
//! Checks fixed up front: 537 nodes and 918 triangles as declared in the header (hex 219, 396);
//! the five boundary zones with their names and mapped kinds plus `frontAndBack`; the mesh passes
//! the rcfd quality check; the extruded volume equals thickness times the area from the shoelace
//! formula over the boundary edges of the file (an independent computation) to 1e-9 relative.

use rcfd_io::datapack::{FileState, Manifest};
use rcfd_io::fluent::{read_file_with, Options};
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::PatchKind;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Shoelace area from the boundary edges of the ASCII file: an edge `n0 n1 c_right c_left` with
/// a zero right cell has the domain on its left only when `c_left` is non-zero; orient every
/// boundary edge so the domain is on its left and sum x0*y1 - x1*y0.
fn area_from_file(text: &str) -> f64 {
    let mut xy: Vec<(f64, f64)> = Vec::new();
    let mut sum = 0.0;
    let mut mode = 0; // 1 nodes, 2 interior faces, 3 boundary faces
    let mut node_first = 0usize;
    for line in text.lines() {
        let t = line.trim();
        if let Some(h) = t.strip_prefix("(10 (") {
            let f: Vec<&str> = h.split([' ', ')']).filter(|s| !s.is_empty()).collect();
            if f[0] != "0" {
                node_first = usize::from_str_radix(f[1], 16).unwrap();
                let last = usize::from_str_radix(f[2], 16).unwrap();
                if xy.len() < last {
                    xy.resize(last, (0.0, 0.0));
                }
                mode = 1;
                continue;
            }
        } else if let Some(h) = t.strip_prefix("(13 (") {
            let f: Vec<&str> = h.split([' ', ')']).filter(|s| !s.is_empty()).collect();
            mode = if f[0] == "0" {
                0
            } else if f[3] == "2" {
                2
            } else {
                3
            };
            continue;
        } else if t.starts_with('(') {
            mode = 0;
        }
        let v: Vec<&str> = t.split_whitespace().collect();
        match mode {
            1 if v.len() == 2 && !t.contains('(') => {
                if let (Ok(x), Ok(y)) = (v[0].parse::<f64>(), v[1].parse::<f64>()) {
                    xy[node_first - 1] = (x, y);
                    node_first += 1;
                }
            }
            3 if v.len() == 4 => {
                let id = |s: &str| usize::from_str_radix(s, 16).unwrap();
                let (a, b) = (xy[id(v[0]) - 1], xy[id(v[1]) - 1]);
                let (cr, cl) = (id(v[2]), id(v[3]));
                let (p, q) = if cl != 0 && cr == 0 { (a, b) } else { (b, a) };
                assert!(cl == 0 || cr == 0, "boundary edge with two cells");
                sum += p.0 * q.1 - q.0 * p.1;
            }
            _ => {}
        }
    }
    // The sign only depends on the right/left cell convention assumed above (the first run showed
    // the opposite one); the magnitude is what the check compares.
    (0.5 * sum).abs()
}

#[test]
fn reads_the_public_elbow_mesh() {
    let manifest = Manifest::load(&root().join("validation-data").join("manifest.json")).unwrap();
    let data = root().join("validation-data").join("data");
    let path = data.join("fluent-elbow-openfoam").join("elbow.msh");
    if !path.exists() {
        eprintln!(
            "skipped: {} not fetched (python validation-data/fetch.py fetch fluent-elbow-openfoam)",
            path.display()
        );
        return;
    }
    for (id, name, st) in manifest.verify(&root(), &data) {
        if id == "fluent-elbow-openfoam" {
            assert!(matches!(st, FileState::Ok), "{name}: {st:?}");
        }
    }
    let text = std::fs::read_to_string(&path).unwrap();
    let thickness = 0.25;
    let (mesh, rep) = read_file_with(&path, &Options { thickness }).unwrap();
    assert_eq!(rep.dimension, 2);
    assert!(!rep.binary);
    assert_eq!(mesh.n_cells(), 0x396);
    assert_eq!(mesh.n_points(), 2 * 0x219);
    let names: Vec<&str> = mesh.patches().iter().map(|p| p.name.as_str()).collect();
    for want in [
        "wall-4",
        "velocity-inlet-5",
        "velocity-inlet-6",
        "pressure-outlet-7",
        "wall-8",
        "frontAndBack",
    ] {
        assert!(names.contains(&want), "{want} missing in {names:?}");
    }
    assert_eq!(mesh.patch("wall-4").unwrap().kind, PatchKind::Wall);
    assert_eq!(mesh.patch("velocity-inlet-5").unwrap().kind, PatchKind::Inlet);
    assert_eq!(mesh.patch("pressure-outlet-7").unwrap().kind, PatchKind::Outlet);
    assert_eq!(mesh.patch("frontAndBack").unwrap().kind, PatchKind::Empty);
    assert_eq!(mesh.patch("frontAndBack").unwrap().size, 2 * 0x396);
    let q = check_mesh(&mesh);
    assert!(q.is_ok(), "{:?} {:?}", q.errors, q.warnings);
    let area = area_from_file(&text);
    let rel = (mesh.total_volume() - area * thickness).abs() / (area * thickness);
    assert!(area > 0.0);
    println!(
        "elbow: {} cells, area from the file {area:.6}, mesh volume {:.6} (thickness {thickness}), relative difference {rel:.2e}; ignored sections {:?}",
        mesh.n_cells(),
        mesh.total_volume(),
        rep.ignored_sections
    );
    assert!(rel < 1e-9, "{rel}");
}
