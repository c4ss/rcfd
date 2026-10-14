//! Interchange formats (F-119): checks registered in docs/adr/0011-formats-and-registers.md before this file was run.

use rcfd_core::vec3::{dot, norm, sub, Vec3};
use rcfd_io::common::{build_mesh, cell_face_template, Shape};
use rcfd_io::guard::fuzz_no_panic;
use rcfd_io::{plot3d, stl, tecplot, xdmf};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use std::path::Path;

fn unit_box(n: usize) -> Mesh {
    box_mesh([n; 3], [1.0; 3], [Grading::Uniform; 3]).unwrap()
}

fn all_patches(m: &Mesh) -> Vec<String> {
    stl::surface_patch_names(m)
}

fn data(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(name)
}

/// hex + pyramid on its top + tet on a pyramid side + prism on a tet side (all four shapes, conforming)
fn mixed_mesh() -> Mesh {
    let mut pts: Vec<Vec3> = vec![
        [0., 0., 0.],
        [1., 0., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [1., 0., 1.],
        [1., 1., 1.],
        [0., 1., 1.],
        [0.5, 0.5, 1.5],
        [0.5, -0.5, 1.2],
    ];
    let v = [0.0, -0.4, -1.0];
    for k in [4usize, 5, 9] {
        pts.push([pts[k][0] + v[0], pts[k][1] + v[1], pts[k][2] + v[2]]);
    }
    let cell = |shape: Shape, nodes: &[usize]| -> Vec<Vec<usize>> {
        cell_face_template(nodes.len(), shape)
            .unwrap()
            .iter()
            .map(|f| f.iter().map(|&i| nodes[i]).collect())
            .collect()
    };
    let cells = vec![
        cell(Shape::Hex, &[0, 1, 2, 3, 4, 5, 6, 7]),
        cell(Shape::Pyramid, &[4, 5, 6, 7, 8]),
        cell(Shape::Tet, &[4, 5, 8, 9]),
        cell(Shape::Prism, &[4, 5, 9, 10, 11, 12]),
    ];
    build_mesh(pts, cells, vec![], vec![]).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

// ------------------------------------------------------------------------------------------------ STL

#[test]
fn stl_ascii_round_trip_of_box_boundary() {
    let m = unit_box(20);
    let names = all_patches(&m);
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let s = stl::solid_from_patches(&m, "box", &refs).unwrap();
    assert_eq!(s.triangles.len(), 4800);
    let text = stl::write_ascii(&s).unwrap();
    let back = stl::read_bytes(text.as_bytes()).unwrap();
    assert_eq!(back.len(), 1);
    let b = &back[0];
    // S1 bit exact
    let mut maxd: f64 = 0.0;
    for (t, u) in s.triangles.iter().zip(&b.triangles) {
        for (p, q) in t.iter().zip(u) {
            for k in 0..3 {
                maxd = maxd.max((p[k] - q[k]).abs());
            }
        }
    }
    assert_eq!(maxd, 0.0, "S1 ASCII vertices must be bit exact");
    assert_eq!(s.stored_normals, b.stored_normals);
    // S3 area
    let patch_area: f64 = names
        .iter()
        .map(|n| m.patch(n).unwrap().faces().map(|f| m.face_area(f)).sum::<f64>())
        .sum();
    assert!(rel(b.area(), patch_area) <= 1e-12, "area {} vs {patch_area}", b.area());
    // S4 orientation and normal
    let mut wrong = 0;
    let mut worst: f64 = 0.0;
    for i in 0..b.triangles.len() {
        let c = b.computed_normal(i);
        if dot(c, b.stored_normals[i]) <= 0.0 {
            wrong += 1;
        }
        worst = worst.max(norm(sub(c, b.stored_normals[i])));
    }
    assert_eq!(wrong, 0, "S4 wrongly wound triangles");
    assert!(worst <= 1e-12, "S4 normal deviation {worst}");
    // S5 topology
    let w = stl::weld(b);
    let e = w.edge_stats();
    assert_eq!(w.points.len(), 2402);
    assert_eq!(w.triangles.len(), 4800);
    assert_eq!((e.boundary, e.non_manifold, e.misoriented), (0, 0, 0));
    assert_eq!(e.edges, 7200);
    assert_eq!(w.euler(), 2);
    assert!(
        (w.enclosed_volume() - 1.0).abs() <= 1e-12,
        "volume {}",
        w.enclosed_volume()
    );
}

#[test]
fn stl_binary_round_trip_of_box_boundary() {
    let m = unit_box(20);
    let names = all_patches(&m);
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut s = stl::solid_from_patches(&m, "box", &refs).unwrap();
    // shift so coordinates are not all float32-representable
    for t in &mut s.triangles {
        for v in t.iter_mut() {
            for c in v.iter_mut() {
                *c = *c * 1.0000001 + 0.1;
            }
        }
    }
    let bytes = stl::write_binary(&s).unwrap();
    assert_eq!(bytes.len(), 84 + 50 * 4800);
    assert!(!bytes.starts_with(b"solid"), "S6 header");
    let back = stl::read_bytes(&bytes).unwrap();
    let b = &back[0];
    let mut worst: f64 = 0.0;
    for (t, u) in s.triangles.iter().zip(&b.triangles) {
        for (p, q) in t.iter().zip(u) {
            for k in 0..3 {
                let r = if p[k] == 0.0 {
                    (q[k] - p[k]).abs()
                } else {
                    (q[k] - p[k]).abs() / p[k].abs()
                };
                worst = worst.max(r);
            }
        }
    }
    assert!(worst <= 6.0e-8, "S2 relative error {worst}");
    assert!(rel(b.area(), s.area()) <= 1e-6);
    let w = stl::weld(b);
    let e = w.edge_stats();
    assert_eq!((e.boundary, e.non_manifold), (0, 0));
    let v0 = stl::weld(&s).enclosed_volume();
    assert!(
        (w.enclosed_volume() - v0).abs() <= 1e-6,
        "volume {} vs {v0}",
        w.enclosed_volume()
    );
}

#[test]
fn stl_binary_header_starting_with_solid_is_still_binary() {
    let m = unit_box(2);
    let s = stl::solid_from_patches(&m, "b", &["xmin"]).unwrap();
    let mut bytes = stl::write_binary(&s).unwrap();
    bytes[..5].copy_from_slice(b"solid");
    let back = stl::read_bytes(&bytes).unwrap();
    assert_eq!(back[0].triangles.len(), s.triangles.len());
}

#[test]
fn stl_malformed_input_has_located_errors() {
    let m = unit_box(2);
    let s = stl::solid_from_patches(&m, "b", &["xmin"]).unwrap();
    let text = stl::write_ascii(&s).unwrap();
    // garble a token on line 7: two blank lines after 'solid' make line 7 the second vertex of facet 1
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    lines.insert(1, String::new());
    lines.insert(1, String::new());
    assert!(lines[6].trim_start().starts_with("vertex"), "{}", lines[6]);
    lines[6] = "      vertex 0.0 abc 1.0".into();
    let e = stl::read_bytes(
        lines
            .join(
                "
",
            )
            .as_bytes(),
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("line 7") && e.contains("abc"), "{e}");
    // missing vertex
    let mut l2: Vec<String> = text.lines().map(String::from).collect();
    l2.remove(6);
    let e = stl::read_bytes(l2.join("\n").as_bytes()).unwrap_err().to_string();
    assert!(e.contains("line "), "{e}");
    // truncated ASCII (no endsolid)
    let cut: String = text.lines().take(10).collect::<Vec<_>>().join("\n");
    assert!(stl::read_bytes(cut.as_bytes())
        .unwrap_err()
        .to_string()
        .contains("line "));
    // non-finite vertex
    lines[6] = "      vertex 0.0 NaN 1.0".into();
    assert!(stl::read_bytes(lines.join("\n").as_bytes()).is_err());
    // binary count inconsistent with the length
    let mut b = stl::write_binary(&s).unwrap();
    b.truncate(b.len() - 10);
    let e = stl::read_bytes(&b).unwrap_err().to_string();
    assert!(e.contains("byte offset"), "{e}");
    // two solids are both read, trailing garbage after endsolid is an error
    let two = format!("{text}{text}");
    assert_eq!(stl::read_bytes(two.as_bytes()).unwrap().len(), 2);
    assert!(stl::read_bytes(format!("{text}garbage").as_bytes()).is_err());
}

#[test]
fn stl_hostile_input_never_panics_and_huge_counts_fail_fast() {
    let m = unit_box(2);
    let s = stl::solid_from_patches(&m, "b", &["xmin", "ymax"]).unwrap();
    let ascii = stl::write_ascii(&s).unwrap().into_bytes();
    let bin = stl::write_binary(&s).unwrap();
    fuzz_no_panic(&ascii, 3000, |b| {
        let _ = stl::read_bytes(b);
    });
    fuzz_no_panic(&bin, 3000, |b| {
        let _ = stl::read_bytes(b);
    });
    let mut hostile = vec![0u8; 100];
    hostile[80..84].copy_from_slice(&u32::MAX.to_le_bytes());
    let t = std::time::Instant::now();
    assert!(stl::read_bytes(&hostile).is_err());
    assert!(t.elapsed().as_secs_f64() < 1.0);
}

// ------------------------------------------------------------------------------------------------ Tecplot

fn cell_vols(m: &Mesh) -> Vec<f64> {
    m.cell_volumes().to_vec()
}

fn tecplot_round_trip(m: &Mesh) {
    let f1: Vec<f64> = (0..m.n_cells()).map(|c| c as f64 / 7.0 + 1.0 / 3.0).collect();
    let f2: Vec<f64> = (0..m.n_cells()).map(|c| -1.0e-9 * (c as f64 + 0.1).sqrt()).collect();
    let ds = tecplot::dataset_from_mesh(m, "t", &[("p", &f1), ("T", &f2)]).unwrap();
    let text = tecplot::write(&ds, tecplot::Packing::Block).unwrap();
    let back = tecplot::read(text.as_bytes()).unwrap();
    // T1: bit exact
    assert_eq!(back.variables, ds.variables);
    let (a, b) = (&ds.zones[0], &back.zones[0]);
    assert_eq!((a.n_nodes, a.n_elems, a.kind), (b.n_nodes, b.n_elems, b.kind));
    assert_eq!(a.locations, b.locations);
    for (x, y) in a.data.iter().zip(&b.data) {
        assert_eq!(x, y, "T1 data must be bit exact");
    }
    assert_eq!(a.connectivity, b.connectivity);
    // T2 volumes
    let m2 = tecplot::mesh_from_zone(&back, 0).unwrap();
    assert_eq!(m2.n_cells(), m.n_cells());
    let (v1, v2) = (cell_vols(m), cell_vols(&m2));
    let sum_abs: f64 = v1.iter().zip(&v2).map(|(a, b)| (a - b).abs()).sum();
    assert!(sum_abs / m.total_volume() <= 1e-13, "cell volume mismatch {sum_abs}");
    assert!(rel(m2.total_volume(), m.total_volume()) <= 1e-13);
}

#[test]
fn tecplot_round_trip_all_shapes() {
    tecplot_round_trip(&unit_box(6));
    tecplot_round_trip(&mixed_mesh());
    for f in ["tet_v4.1_ascii.msh", "hex_v4.1_ascii.msh", "prism_v4.1_ascii.msh"] {
        let m = rcfd_io::gmsh::read_file(data(f)).unwrap();
        tecplot_round_trip(&m);
    }
}

#[test]
fn tecplot_point_and_block_packing_agree() {
    let m = unit_box(3);
    let mut ds = tecplot::dataset_from_mesh(&m, "t", &[]).unwrap();
    // nodal variable instead of the cell ones
    ds.variables.push("w".into());
    let n = ds.zones[0].n_nodes;
    ds.zones[0].locations.push(tecplot::Location::Nodal);
    ds.zones[0].data.push((0..n).map(|i| (i as f64).sin()).collect());
    let blk = tecplot::read(tecplot::write(&ds, tecplot::Packing::Block).unwrap().as_bytes()).unwrap();
    let pnt = tecplot::read(tecplot::write(&ds, tecplot::Packing::Point).unwrap().as_bytes()).unwrap();
    assert_eq!(blk, pnt);
    assert_eq!(blk.zones[0].data, ds.zones[0].data);
    // cell-centred with POINT is refused
    let f = vec![0.0; m.n_cells()];
    let ds2 = tecplot::dataset_from_mesh(&m, "t", &[("p", &f)]).unwrap();
    assert!(tecplot::write(&ds2, tecplot::Packing::Point).is_err());
}

#[test]
fn tecplot_reads_ordered_zone_and_varlocation_ranges() {
    let t = "TITLE = \"x\"\nVARIABLES = \"X\" \"Y\" \"Z\" \"a\" \"b\"\nZONE T=\"z\", I=2, J=2, K=2, DATAPACKING=BLOCK, VARLOCATION=([4-5]=CELLCENTERED)\n\
0 1 0 1 0 1 0 1\n0 0 1 1 0 0 1 1\n0 0 0 0 1 1 1 1\n7\n8\n";
    let d = tecplot::read(t.as_bytes()).unwrap();
    let z = &d.zones[0];
    assert_eq!((z.n_nodes, z.n_elems), (8, 1));
    assert_eq!(z.data[3], vec![7.0]);
    assert_eq!(z.data[4], vec![8.0]);
}

#[test]
fn tecplot_refuses_polyhedra() {
    // pentagonal prism: 7 faces
    let n = 5;
    let mut pts: Vec<Vec3> = Vec::new();
    for z in [0.0, 1.0] {
        for k in 0..n {
            let a = 2.0 * std::f64::consts::PI * k as f64 / n as f64;
            pts.push([a.cos(), a.sin(), z]);
        }
    }
    let mut faces: Vec<Vec<usize>> = vec![(0..n).collect(), (n..2 * n).collect()];
    for k in 0..n {
        faces.push(vec![k, (k + 1) % n, n + (k + 1) % n, n + k]);
    }
    let m = build_mesh(pts, vec![faces], vec![], vec![]).unwrap();
    let e = tecplot::dataset_from_mesh(&m, "t", &[]).unwrap_err().to_string();
    assert!(e.contains("cell 0"), "{e}");
    assert!(xdmf::build(&m, "x", &[]).is_err());
}

#[test]
fn tecplot_reader_errors_are_located() {
    let m = unit_box(2);
    let f = vec![1.0; 8];
    let ds = tecplot::dataset_from_mesh(&m, "t", &[("p", &f)]).unwrap();
    let text = tecplot::write(&ds, tecplot::Packing::Block).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    // garble the first data line
    let mut l = lines.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    l[3] = "0 0 oops 0 0".into();
    let e = tecplot::read(l.join("\n").as_bytes()).unwrap_err().to_string();
    assert!(e.contains("line 4") && e.contains("oops"), "{e}");
    // short data
    let cut = lines[..lines.len() - 3].join("\n");
    let e = tecplot::read(cut.as_bytes()).unwrap_err().to_string();
    assert!(e.contains("short") || e.contains("line"), "{e}");
    // connectivity out of range
    let mut l = lines.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let last = l.len() - 1;
    l[last] = "1 2 3 4 5 6 7 999".into();
    let e = tecplot::read(l.join("\n").as_bytes()).unwrap_err().to_string();
    assert!(e.contains("999"), "{e}");
    // unknown zone type
    let bad = text.replace("FEBRICK", "FEPOLYHEDRON");
    assert!(tecplot::read(bad.as_bytes())
        .unwrap_err()
        .to_string()
        .contains("ZONETYPE"));
}

#[test]
fn tecplot_hostile_input() {
    let m = unit_box(2);
    let f = vec![1.0; 8];
    let ds = tecplot::dataset_from_mesh(&m, "t", &[("p", &f)]).unwrap();
    let text = tecplot::write(&ds, tecplot::Packing::Block).unwrap().into_bytes();
    fuzz_no_panic(&text, 3000, |b| {
        let _ = tecplot::read(b);
    });
    let huge = "VARIABLES = \"X\" \"Y\" \"Z\"\nZONE NODES=4294967295, ELEMENTS=4294967295, ZONETYPE=FEBRICK, DATAPACKING=BLOCK\n1 2 3\n";
    let t = std::time::Instant::now();
    assert!(tecplot::read(huge.as_bytes()).is_err());
    assert!(t.elapsed().as_secs_f64() < 1.0);
    let huge_i = "VARIABLES = \"X\"\nZONE I=4294967295, J=4294967295, K=4294967295\n1\n";
    assert!(tecplot::read(huge_i.as_bytes()).is_err());
}

// ------------------------------------------------------------------------------------------------ Plot3D

fn sample_grid(iblank: bool) -> plot3d::Grid {
    let mut blocks = Vec::new();
    for (bi, (ni, nj, nk)) in [(5usize, 4usize, 3usize), (2, 2, 2), (7, 3, 4)].into_iter().enumerate() {
        let n = ni * nj * nk;
        let (mut x, mut y, mut z) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        for k in 0..nk {
            for j in 0..nj {
                for i in 0..ni {
                    let id = i + ni * (j + nj * k);
                    x[id] = i as f64 * 0.37 + bi as f64 + 1.0 / 3.0 * j as f64;
                    y[id] = j as f64 * 0.11 + 1e-7 * (i * k) as f64;
                    z[id] = k as f64 * 0.7 - 0.2 * (bi as f64).sqrt();
                }
            }
        }
        let ib = iblank.then(|| (0..n).map(|q| (q % 3) as i32 - 1).collect());
        blocks.push(plot3d::Block {
            ni,
            nj,
            nk,
            x,
            y,
            z,
            iblank: ib,
        });
    }
    plot3d::Grid { dim: 3, blocks }
}

#[test]
fn plot3d_round_trips() {
    for ib in [false, true] {
        let g = sample_grid(ib);
        // binary double, both byte orders; ASCII
        for big in [false, true] {
            let f = plot3d::Format {
                binary: true,
                double: true,
                big_endian: big,
                iblank: ib,
                multi_grid: true,
            };
            let b = plot3d::write_grid(&g, &f).unwrap();
            assert_eq!(
                plot3d::read_grid(&b).unwrap(),
                g,
                "binary double big_endian={big} iblank={ib}"
            );
        }
        let f = plot3d::Format {
            binary: false,
            iblank: ib,
            ..Default::default()
        };
        let b = plot3d::write_grid(&g, &f).unwrap();
        assert_eq!(plot3d::read_grid(&b).unwrap(), g, "ascii iblank={ib}");
        // single precision
        let f = plot3d::Format {
            double: false,
            iblank: ib,
            ..Default::default()
        };
        let back = plot3d::read_grid(&plot3d::write_grid(&g, &f).unwrap()).unwrap();
        let mut worst: f64 = 0.0;
        for (a, c) in g.blocks.iter().zip(&back.blocks) {
            assert_eq!(a.iblank, c.iblank);
            for (p, q) in
                a.x.iter()
                    .chain(&a.y)
                    .chain(&a.z)
                    .zip(c.x.iter().chain(&c.y).chain(&c.z))
            {
                worst = worst.max(if *p == 0.0 { q.abs() } else { (p - q).abs() / p.abs() });
            }
        }
        assert!(worst <= 6.0e-8, "single precision error {worst}");
    }
}

#[test]
fn plot3d_single_grid_2d_and_function_file() {
    let b = plot3d::Block {
        ni: 3,
        nj: 2,
        nk: 1,
        x: vec![0., 1., 2., 0., 1., 2.],
        y: vec![0., 0., 0., 1., 1., 1.],
        z: vec![0.; 6],
        iblank: None,
    };
    let g = plot3d::Grid {
        dim: 2,
        blocks: vec![b],
    };
    for f in [
        plot3d::Format {
            multi_grid: false,
            ..Default::default()
        },
        plot3d::Format::default(),
        plot3d::Format {
            binary: false,
            multi_grid: false,
            ..Default::default()
        },
    ] {
        let bytes = plot3d::write_grid(&g, &f).unwrap();
        assert_eq!(plot3d::read_grid(&bytes).unwrap(), g, "{f:?}");
    }
    // P4
    let ff = plot3d::FunctionFile {
        blocks: vec![(
            2,
            2,
            2,
            vec![
                (0..8).map(|i| i as f64 / 3.0).collect(),
                (0..8).map(|i| -(i as f64)).collect(),
            ],
        )],
    };
    for big in [false, true] {
        let by = plot3d::write_function_binary(
            &ff,
            &plot3d::Format {
                big_endian: big,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(plot3d::read_function_binary(&by).unwrap(), ff);
    }
}

#[test]
fn plot3d_block_to_mesh_graded_box() {
    let (ni, nj, nk) = (9usize, 5usize, 4usize);
    let (mut x, mut y, mut z) = (vec![], vec![], vec![]);
    for k in 0..nk {
        for j in 0..nj {
            for i in 0..ni {
                x.push(2.0 * (i as f64 / 8.0).powf(1.5));
                y.push(0.5 * (j as f64 / 4.0));
                z.push(1.5 * (k as f64 / 3.0).powi(2));
            }
        }
    }
    let g = plot3d::Grid {
        dim: 3,
        blocks: vec![plot3d::Block {
            ni,
            nj,
            nk,
            x,
            y,
            z,
            iblank: None,
        }],
    };
    let m = plot3d::block_to_mesh(&g, 0).unwrap();
    m.validate().unwrap();
    assert_eq!(m.n_cells(), 8 * 4 * 3);
    assert!(rel(m.total_volume(), 2.0 * 0.5 * 1.5) <= 1e-12, "{}", m.total_volume());
    assert_eq!(m.patches().len(), 6);
    let area: f64 = m
        .patches()
        .iter()
        .map(|p| p.faces().map(|f| m.face_area(f)).sum::<f64>())
        .sum();
    let want = 2.0 * (2.0 * 0.5 + 2.0 * 1.5 + 0.5 * 1.5);
    assert!(rel(area, want) <= 1e-12, "{area} vs {want}");
}

#[test]
fn plot3d_malformed_and_hostile() {
    let g = sample_grid(true);
    let bin = plot3d::write_grid(
        &g,
        &plot3d::Format {
            iblank: true,
            ..Default::default()
        },
    )
    .unwrap();
    let asc = plot3d::write_grid(
        &g,
        &plot3d::Format {
            binary: false,
            iblank: true,
            ..Default::default()
        },
    )
    .unwrap();
    // record length that fits no layout: drop 8 bytes from the second block's record (rewrite markers is not needed:
    // the marker check fails first and is located)
    let mut cut = bin.clone();
    cut.truncate(cut.len() - 8);
    let e = plot3d::read_grid(&cut).unwrap_err().to_string();
    assert!(e.contains("record") || e.contains("byte offset"), "{e}");
    // non-integer dimension in ASCII is located
    let t = String::from_utf8(asc.clone()).unwrap().replacen("5 4 3", "5 x 3", 1);
    let e = plot3d::read_grid(t.as_bytes()).unwrap_err().to_string();
    assert!(e.contains("line 2"), "{e}");
    fuzz_no_panic(&bin, 3000, |b| {
        let _ = plot3d::read_grid(b);
    });
    fuzz_no_panic(&asc, 3000, |b| {
        let _ = plot3d::read_grid(b);
    });
    // header claiming 2^31-1 cubed points in a tiny file
    let mut h = Vec::new();
    for payload in [vec![1i32], vec![i32::MAX, i32::MAX, i32::MAX]] {
        let p: Vec<u8> = payload.iter().flat_map(|v| v.to_le_bytes()).collect();
        h.extend_from_slice(&(p.len() as u32).to_le_bytes());
        h.extend_from_slice(&p);
        h.extend_from_slice(&(p.len() as u32).to_le_bytes());
    }
    let t = std::time::Instant::now();
    assert!(plot3d::read_grid(&h).is_err());
    assert!(t.elapsed().as_secs_f64() < 1.0);
    assert!(plot3d::read_grid_ascii("1\n2147483647 2147483647 2147483647\n1 2 3", true, 3).is_err());
}

// ------------------------------------------------------------------------------------------------ XDMF

fn read_f64s(p: &Path) -> Vec<f64> {
    std::fs::read(p)
        .unwrap()
        .chunks_exact(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

#[test]
fn xdmf_writer_files_are_consistent() {
    let m = mixed_mesh();
    let f1: Vec<f64> = (0..m.n_cells()).map(|c| c as f64 / 3.0).collect();
    let dir = std::env::temp_dir().join(format!("rcfd_xdmf_{}", std::process::id()));
    let files = xdmf::write(&dir, "mixed", &m, &[("p", &f1)]).unwrap();
    let xml = std::fs::read_to_string(&files[0]).unwrap();
    xdmf::check_wellformed(&xml).unwrap();
    // X2: every referenced file exists next to the xmf, relative, no ".."
    let mut refs = 0;
    for part in xml.split("</DataItem>") {
        if let Some(i) = part.rfind('>') {
            let name = part[i + 1..].trim();
            if name.ends_with(".bin") {
                refs += 1;
                assert!(!name.contains("..") && !name.contains('/') && !name.contains('\\'));
                assert!(dir.join(name).exists(), "{name}");
            }
        }
    }
    assert_eq!(refs, 3);
    // X1: dims * precision == file size; values bit exact
    let pts = read_f64s(&dir.join("mixed.points.bin"));
    assert_eq!(pts.len(), 3 * m.n_points());
    for (k, p) in m.points().iter().enumerate() {
        assert_eq!(&pts[3 * k..3 * k + 3], p);
    }
    assert_eq!(read_f64s(&dir.join("mixed.field0.bin")), f1);
    let conn_bytes = std::fs::read(dir.join("mixed.cells.bin")).unwrap();
    assert!(xml.contains(&format!("Dimensions=\"{}\"", conn_bytes.len() / 8)));
    // X3: mixed topology walk gives 4 cells with types 9, 7, 6, 8
    let conn: Vec<i64> = conn_bytes
        .chunks_exact(8)
        .map(|c| i64::from_le_bytes(c.try_into().unwrap()))
        .collect();
    let (mut i, mut types) = (0, vec![]);
    while i < conn.len() {
        let t = conn[i];
        types.push(t);
        i += 1 + match t {
            9 => 8,
            8 => 6,
            7 => 5,
            6 => 4,
            _ => panic!("type {t}"),
        };
    }
    assert_eq!(i, conn.len());
    assert_eq!(types, vec![9, 7, 6, 8]);
    assert!(conn.iter().all(|&v| v < m.n_points() as i64));
    // stem validation
    assert!(xdmf::build(&m, "../evil", &[]).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Writes the mixed-shape mesh for the manual third-party reader check (scripts/check_interchange.py) when the
/// environment variable RCFD_EXTERNAL_DIR is set; otherwise does nothing.
#[test]
fn write_mixed_mesh_for_external_readers() {
    let Ok(dir) = std::env::var("RCFD_EXTERNAL_DIR") else {
        return;
    };
    let m = mixed_mesh();
    let f: Vec<f64> = (0..m.n_cells()).map(|c| 10.0 + c as f64).collect();
    std::fs::create_dir_all(&dir).unwrap();
    let dir = Path::new(&dir);
    xdmf::write(dir, "mixed", &m, &[("p", &f)]).unwrap();
    let ds = tecplot::dataset_from_mesh(&m, "mixed", &[("p", &f)]).unwrap();
    std::fs::write(
        dir.join("mixed.dat"),
        tecplot::write(&ds, tecplot::Packing::Block).unwrap(),
    )
    .unwrap();
    let sol = stl::solid_from_patches(&m, "mixed", &[&m.patches()[0].name]).unwrap();
    std::fs::write(dir.join("mixed_patch.stl"), stl::write_binary(&sol).unwrap()).unwrap();
    std::fs::write(
        dir.join("mixed.volume"),
        format!(
            "{:?}
{}
{}
",
            m.total_volume(),
            m.n_cells(),
            sol.triangles.len()
        ),
    )
    .unwrap();
    // Plot3D: the 3-block sample grid, binary double and ASCII, for vtkMultiBlockPLOT3DReader
    let g = sample_grid(false);
    std::fs::write(
        dir.join("sample_bin.x"),
        plot3d::write_grid(&g, &plot3d::Format::default()).unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.join("sample_asc.x"),
        plot3d::write_grid(
            &g,
            &plot3d::Format {
                binary: false,
                ..Default::default()
            },
        )
        .unwrap(),
    )
    .unwrap();
}
