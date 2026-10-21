//! Wave 39A (ADR 0092, F-118): a 3D Fluent mesh of 3 840 curved, twisted and sheared hexahedra written by this test in the
//! ASCII layout and in the binary layouts (3010/3013 double, 2010/2013 single precision) that the reader implements, read back
//! and put through the rcfd mesh quality check.
//!
//! The writer encodes the reader's own reading of the public section description (binary framing as in the reader's unit
//! tests), so this is a round trip, not a check against a Fluent-written file; no public 3D or binary Fluent file has been
//! found. The file is generated here, so default CI needs no external fixture.
//!
//! Limits registered in ADR 0092 before the first run: counts and patch sizes equal; patch names and kinds as written
//! (velocity-inlet -> Inlet, pressure-outlet -> Outlet, wall -> Wall, symmetry -> Symmetry); quality check passes; total and
//! minimum volume, maximum non-orthogonality, skewness and aspect ratio equal those of the source mesh to 1e-12 relative for
//! ASCII and 3010/3013 (and the two content hashes equal), to 1e-5 relative (non-orthogonality 1e-3 degrees) for 2010/2013.

use rcfd_io::fluent::{read_bytes, Options};
use rcfd_mesh::gen::{annulus_3d, warp, Grading};
use rcfd_mesh::quality::{check_mesh, QualityReport};
use rcfd_mesh::{Mesh, PatchKind};

/// Annulus r 0.5..1, 48 x 8 x 10 hexahedra, height 1, radially graded; twisted by 0.4 rad over the height and sheared in x.
fn source() -> Mesh {
    let m = annulus_3d(48, 8, 10, 0.5, 1.0, 1.0, 0.0, Grading::Expansion(3.0)).unwrap();
    warp(&m, |p| {
        let a = 0.4 * p[2];
        let (s, c) = a.sin_cos();
        [c * p[0] - s * p[1] + 0.2 * p[2], s * p[0] + c * p[1], p[2]]
    })
    .unwrap()
}

/// (rcfd patch, Fluent bc type id, Fluent type name, expected kind after import)
const ZONES: [(&str, usize, &str, PatchKind); 4] = [
    ("inner", 3, "wall", PatchKind::Wall),
    ("outer", 7, "symmetry", PatchKind::Symmetry),
    ("zmin", 10, "velocity-inlet", PatchKind::Inlet),
    ("zmax", 5, "pressure-outlet", PatchKind::Outlet),
];

enum Enc {
    Ascii,
    Double,
    Single,
}

/// Fluent file of `m`: nodes, one cell zone (7, hexahedra), the interior face zone (2) and one face zone per patch (3..),
/// quadrilateral faces `n0 n1 n2 n3 c0 c1` with c0 the owner (1-based ids).
fn write(m: &Mesh, enc: Enc) -> Vec<u8> {
    let (np, nc) = (m.n_points(), m.n_cells());
    let mut b: Vec<u8> = Vec::new();
    let put = |b: &mut Vec<u8>, s: String| b.extend_from_slice(s.as_bytes());
    put(
        &mut b,
        format!("(0 \"rcfd w39a test mesh\")\n(2 3)\n(10 (0 1 {np:x} 0))\n"),
    );
    let (nsec, fsec) = match enc {
        Enc::Ascii => (10, 13),
        Enc::Double => (3010, 3013),
        Enc::Single => (2010, 2013),
    };
    put(&mut b, format!("({nsec} (1 1 {np:x} 1 3)("));
    for p in m.points() {
        match enc {
            Enc::Ascii => put(&mut b, format!("\n{:e} {:e} {:e}", p[0], p[1], p[2])),
            Enc::Double => p.iter().for_each(|v| b.extend_from_slice(&v.to_le_bytes())),
            Enc::Single => p.iter().for_each(|v| b.extend_from_slice(&(*v as f32).to_le_bytes())),
        }
    }
    let close = |b: &mut Vec<u8>, sec: usize, ascii: bool| {
        if ascii {
            b.extend_from_slice(b"\n))\n");
        } else {
            b.extend_from_slice(format!("End of Binary Section   {sec})\n").as_bytes());
        }
    };
    let ascii = matches!(enc, Enc::Ascii);
    close(&mut b, nsec, ascii);
    put(&mut b, format!("(12 (0 1 {nc:x} 0))\n(12 (7 1 {nc:x} 1 4))\n"));
    let nf = m.n_faces();
    put(&mut b, format!("(13 (0 1 {nf:x} 0))\n"));
    let mut zones: Vec<(usize, usize, std::ops::Range<usize>)> = vec![(2, 2, 0..m.n_internal_faces())];
    for (k, (name, bc, _, _)) in ZONES.iter().enumerate() {
        zones.push((3 + k, *bc, m.patch(name).unwrap().faces()));
    }
    for (zone, bc, r) in zones {
        put(
            &mut b,
            format!("({fsec} ({zone:x} {:x} {:x} {bc:x} 4)(", r.start + 1, r.end),
        );
        for f in r {
            let n = m.face_points(f);
            assert_eq!(n.len(), 4);
            let c1 = if f < m.n_internal_faces() {
                m.neighbour()[f] + 1
            } else {
                0
            };
            let ids = [n[0] + 1, n[1] + 1, n[2] + 1, n[3] + 1, m.owner()[f] + 1, c1];
            if ascii {
                put(
                    &mut b,
                    format!(
                        "\n{:x} {:x} {:x} {:x} {:x} {:x}",
                        ids[0], ids[1], ids[2], ids[3], ids[4], ids[5]
                    ),
                );
            } else {
                ids.iter().for_each(|v| b.extend_from_slice(&(*v as i32).to_le_bytes()));
            }
        }
        close(&mut b, fsec, ascii);
    }
    put(
        &mut b,
        "(45 (7 fluid fluid)())\n(45 (2 interior interior-2)())\n".into(),
    );
    for (k, (name, _, ty, _)) in ZONES.iter().enumerate() {
        put(&mut b, format!("(45 ({:x} {ty} {name})())\n", 3 + k));
    }
    b
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

/// Registered comparison of the imported mesh `m` (quality `q`) against the source (`s`, `qs`).
fn compare(label: &str, m: &Mesh, q: &QualityReport, s: &Mesh, qs: &QualityReport, tol: f64, tol_deg: f64) {
    println!(
        "{label}: cells {}, points {}, faces {}; volume rel {:.2e}, min volume rel {:.2e}, non-orth {:.6} vs {:.6} deg, skew rel {:.2e}, aspect rel {:.2e}; ok {} errors {:?} warnings {:?}",
        m.n_cells(),
        m.n_points(),
        m.n_faces(),
        rel(q.total_volume, qs.total_volume),
        rel(q.min_volume, qs.min_volume),
        q.max_non_orthogonality,
        qs.max_non_orthogonality,
        rel(q.max_skewness, qs.max_skewness),
        rel(q.max_aspect_ratio, qs.max_aspect_ratio),
        q.is_ok(),
        q.errors,
        q.warnings
    );
    assert_eq!(
        (m.n_cells(), m.n_points(), m.n_faces(), m.n_internal_faces()),
        (s.n_cells(), s.n_points(), s.n_faces(), s.n_internal_faces()),
        "{label}"
    );
    for (name, _, _, kind) in &ZONES {
        let p = m.patch(name).unwrap_or_else(|| panic!("{label}: no patch {name}"));
        assert_eq!(
            (&p.kind, p.size),
            (kind, s.patch(name).unwrap().size),
            "{label}: {name}"
        );
    }
    assert!(q.is_ok(), "{label}: {:?}", q.errors);
    assert!(rel(q.total_volume, qs.total_volume) <= tol, "{label}: total volume");
    assert!(rel(q.min_volume, qs.min_volume) <= tol, "{label}: min volume");
    assert!(
        (q.max_non_orthogonality - qs.max_non_orthogonality).abs() <= tol_deg,
        "{label}: non-orthogonality"
    );
    assert!(rel(q.max_skewness, qs.max_skewness) <= tol, "{label}: skewness");
    assert!(
        rel(q.max_aspect_ratio, qs.max_aspect_ratio) <= tol,
        "{label}: aspect ratio"
    );
}

#[test]
fn f1_f3_three_d_fluent_mesh_ascii_and_binary_pass_the_mesh_check() {
    let s = source();
    let qs = check_mesh(&s);
    assert_eq!(s.n_cells(), 3840);
    assert!(qs.is_ok(), "source mesh: {:?}", qs.errors);
    let read = |enc: Enc| {
        let (m, rep) = read_bytes(&write(&s, enc), &Options::default()).unwrap();
        assert_eq!(rep.dimension, 3);
        assert!(rep.ignored_sections.is_empty(), "{:?}", rep.ignored_sections);
        let q = check_mesh(&m);
        (m, q, rep.binary)
    };
    let (a, qa, bin_a) = read(Enc::Ascii);
    assert!(!bin_a);
    compare("F1 ascii", &a, &qa, &s, &qs, 1e-12, 1e-10);
    let (d, qd, bin_d) = read(Enc::Double);
    assert!(bin_d);
    compare("F2 binary 3010/3013", &d, &qd, &s, &qs, 1e-12, 1e-10);
    assert_eq!(
        d.content_hash(),
        a.content_hash(),
        "F2: binary and ASCII imports differ"
    );
    let (f, qf, bin_f) = read(Enc::Single);
    assert!(bin_f);
    compare("F3 binary 2010/2013", &f, &qf, &s, &qs, 1e-5, 1e-3);
}
