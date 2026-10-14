//! `rcfd convert` interchange formats (F-119, ADR 0011): STL, Tecplot ASCII, XDMF output; STL re-encode; Plot3D input;
//! malformed input exits 1 with a located message.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_formats_cli_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn hex_msh() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../rcfd-io/tests/data/hex_v4.1_ascii.msh")
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn convert_mesh_to_stl_tecplot_xdmf() {
    let d = tmp("out");
    let msh = hex_msh();
    // STL binary of all patches, then ASCII of one patch
    let b = d.join("all.stl");
    let o = rcfd(&["convert", s(&msh), s(&b)]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let solids = rcfd_io::stl::read_file(&b).unwrap();
    let w = rcfd_io::stl::weld(&solids[0]);
    assert_eq!(w.edge_stats().boundary, 0, "closed surface");
    // the box [0,2]x[0,1]x[0,1] has volume 2
    assert!((w.enclosed_volume() - 2.0).abs() < 1e-5, "{}", w.enclosed_volume());
    let a = d.join("one.stl");
    let first = rcfd_io::gmsh::read_file(&msh).unwrap().patches()[0].name.clone();
    let o = rcfd(&["convert", s(&msh), s(&a), "--patch", &first, "--ascii"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(std::fs::read_to_string(&a).unwrap().starts_with("solid"));
    // re-encode ASCII to binary
    let r = d.join("re.stl");
    let o = rcfd(&["convert", s(&a), s(&r)]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let (x, y) = (
        rcfd_io::stl::read_file(&a).unwrap(),
        rcfd_io::stl::read_file(&r).unwrap(),
    );
    assert_eq!(x[0].triangles.len(), y[0].triangles.len());
    // STL cannot become a mesh format
    let o = rcfd(&["convert", s(&a), s(&d.join("x.vtu"))]);
    assert_eq!(o.status.code(), Some(1));
    // Tecplot and XDMF
    let t = d.join("m.dat");
    assert!(rcfd(&["convert", s(&msh), s(&t)]).status.success());
    let ds = rcfd_io::tecplot::read_file(&t).unwrap();
    assert_eq!(ds.zones[0].n_elems, 64);
    let x = d.join("m.xmf");
    assert!(rcfd(&["convert", s(&msh), s(&x)]).status.success());
    rcfd_io::xdmf::check_wellformed(&std::fs::read_to_string(&x).unwrap()).unwrap();
    assert!(d.join("m.points.bin").exists() && d.join("m.cells.bin").exists());
}

#[test]
fn convert_plot3d_input_and_located_errors() {
    let d = tmp("p3d");
    let (ni, nj, nk) = (4usize, 3usize, 3usize);
    let (mut x, mut y, mut z) = (vec![], vec![], vec![]);
    for k in 0..nk {
        for j in 0..nj {
            for i in 0..ni {
                x.push(i as f64);
                y.push(j as f64 * 0.5);
                z.push(k as f64 * 0.25);
            }
        }
    }
    let g = rcfd_io::plot3d::Grid {
        dim: 3,
        blocks: vec![rcfd_io::plot3d::Block {
            ni,
            nj,
            nk,
            x,
            y,
            z,
            iblank: None,
        }],
    };
    let f = d.join("g.x");
    std::fs::write(&f, rcfd_io::plot3d::write_grid(&g, &Default::default()).unwrap()).unwrap();
    let o = rcfd(&["check-mesh", s(&f)]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let out = d.join("g.dat");
    assert!(rcfd(&["convert", s(&f), s(&out)]).status.success());
    assert_eq!(rcfd_io::tecplot::read_file(&out).unwrap().zones[0].n_elems, 3 * 2 * 2);
    // garbled STL: exit 1 and a located message naming the file
    let bad = d.join("bad.stl");
    std::fs::write(
        &bad,
        "solid x\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 q 0\n",
    )
    .unwrap();
    let o = rcfd(&["convert", s(&bad), s(&d.join("o.stl"))]);
    assert_eq!(o.status.code(), Some(1));
    let e = String::from_utf8_lossy(&o.stderr);
    assert!(e.contains("bad.stl") && e.contains("line 5"), "{e}");
    // garbled Plot3D
    let badp = d.join("bad.x");
    std::fs::write(&badp, "1\n3 3\n").unwrap();
    let o = rcfd(&["check-mesh", s(&badp)]);
    assert_eq!(o.status.code(), Some(1));
}
