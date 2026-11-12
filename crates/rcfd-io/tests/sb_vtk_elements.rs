//! F-377 (ADR 0278): the finite-element `.vtu` writer of `rcfd_io::vtk` from plain arrays. Checks W1 to W4 and the negative
//! control N1 of the registration. The parser below is the test's own, independent of the writer.

use rcfd_core::vec3::Vec3;
use rcfd_core::Error;
use rcfd_io::vtk::{fe_vtu_string, write_fe_vtu, FeArray, FeCellBlock, VTK_HEXAHEDRON, VTK_QUADRATIC_TETRA, VTK_TETRA};

// ------------------------------------------------------------------------------------------------------------ the parser

fn attr(tag_text: &str, key: &str) -> String {
    let k = format!("{key}=\"");
    let s = tag_text.find(&k).unwrap_or_else(|| panic!("attribute {key} missing")) + k.len();
    let e = tag_text[s..].find('"').unwrap() + s;
    tag_text[s..e].to_string()
}

/// The text between `<tag>` and `</tag>` (None when the section is absent).
fn section<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let s = xml.find(&open)? + open.len();
    let e = xml[s..].find(&format!("</{tag}>")).unwrap() + s;
    Some(&xml[s..e])
}

/// The `DataArray` of a name in a section: (type, components, whitespace-separated tokens).
fn array(sec: &str, name: &str) -> Option<(String, usize, Vec<String>)> {
    let key = format!("Name=\"{name}\"");
    let at = sec.find(&key)?;
    let start = sec[..at].rfind("<DataArray").unwrap();
    let head_end = sec[start..].find('>').unwrap() + start;
    let head = &sec[start..head_end];
    let end = sec[head_end..].find("</DataArray>").unwrap() + head_end;
    let tokens = sec[head_end + 1..end].split_whitespace().map(str::to_string).collect();
    Some((
        attr(head, "type"),
        attr(head, "NumberOfComponents").parse().unwrap(),
        tokens,
    ))
}

fn names(sec: &str) -> Vec<String> {
    sec.match_indices("<DataArray")
        .map(|(i, _)| {
            let e = sec[i..].find('>').unwrap() + i;
            attr(&sec[i..e], "Name")
        })
        .collect()
}

fn f64s(tokens: &[String]) -> Vec<f64> {
    tokens.iter().map(|t| t.parse().unwrap()).collect()
}

fn usizes(tokens: &[String]) -> Vec<usize> {
    tokens.iter().map(|t| t.parse().unwrap()).collect()
}

struct Parsed {
    n_points: usize,
    n_cells: usize,
    points: Vec<f64>,
    connectivity: Vec<usize>,
    offsets: Vec<usize>,
    types: Vec<usize>,
}

fn node_count(ty: usize) -> Option<usize> {
    match ty {
        10 => Some(4),
        12 => Some(8),
        24 => Some(10),
        _ => None,
    }
}

/// Parses the grid and checks counts, offsets and types for consistency (W1); an inconsistency is returned as text.
fn parse_consistent(xml: &str) -> Result<Parsed, String> {
    let ps = xml.find("<Piece").ok_or("no Piece")?;
    let pe = xml[ps..].find('>').unwrap() + ps;
    let n_points: usize = attr(&xml[ps..pe], "NumberOfPoints").parse().unwrap();
    let n_cells: usize = attr(&xml[ps..pe], "NumberOfCells").parse().unwrap();
    let (_, pc, pt) = array(section(xml, "Points").ok_or("no Points")?, "Points").ok_or("no Points array")?;
    let cells = section(xml, "Cells").ok_or("no Cells")?;
    let (ct, _, conn) = array(cells, "connectivity").ok_or("no connectivity")?;
    let (ot, _, offs) = array(cells, "offsets").ok_or("no offsets")?;
    let (tt, _, types) = array(cells, "types").ok_or("no types")?;
    let p = Parsed {
        n_points,
        n_cells,
        points: f64s(&pt),
        connectivity: usizes(&conn),
        offsets: usizes(&offs),
        types: usizes(&types),
    };
    if pc != 3 || p.points.len() != 3 * n_points {
        return Err(format!("{} point values for {n_points} points", p.points.len()));
    }
    if (ct.as_str(), ot.as_str(), tt.as_str()) != ("Int64", "Int64", "UInt8") {
        return Err("array types".into());
    }
    if p.offsets.len() != n_cells || p.types.len() != n_cells {
        return Err(format!(
            "{} offsets, {} types for {n_cells} cells",
            p.offsets.len(),
            p.types.len()
        ));
    }
    let mut prev = 0;
    for (c, (&o, &t)) in p.offsets.iter().zip(&p.types).enumerate() {
        let k = node_count(t).ok_or(format!("cell {c}: type {t}"))?;
        if o != prev + k {
            return Err(format!("cell {c}: offset {o}, expected {}", prev + k));
        }
        prev = o;
    }
    if prev != p.connectivity.len() {
        return Err(format!("last offset {prev}, connectivity {}", p.connectivity.len()));
    }
    if let Some(&bad) = p.connectivity.iter().find(|&&i| i >= n_points) {
        return Err(format!("index {bad} >= {n_points}"));
    }
    for (tag, n) in [("PointData", n_points), ("CellData", n_cells)] {
        if let Some(sec) = section(xml, tag) {
            for name in names(sec) {
                let (ty, comps, tok) = array(sec, &name).unwrap();
                if ty != "Float64" || tok.len() != comps * n {
                    return Err(format!("{tag} {name}: {} values, {comps} x {n}", tok.len()));
                }
            }
        }
    }
    Ok(p)
}

// ------------------------------------------------------------------------------------------------------------- the grid

const SPECIAL: [f64; 9] = [
    -0.0,
    1.0 / 3.0,
    0.1,
    1e300,
    -2.5e-310,
    f64::MAX,
    f64::MIN_POSITIVE,
    -7.25,
    std::f64::consts::PI,
];

fn value(i: usize) -> f64 {
    if i % 3 == 0 {
        SPECIAL[(i / 3) % SPECIAL.len()]
    } else {
        (i as f64).sqrt() * if i % 2 == 0 { -1.0 } else { 1.0 } / 7.0
    }
}

struct Grid {
    points: Vec<Vec3>,
    conn: [Vec<usize>; 4],
    temperature: Vec<f64>,
    velocity: Vec<f64>,
    id: Vec<f64>,
    stress: Vec<f64>,
}

fn grid() -> Grid {
    let points: Vec<Vec3> = (0..30)
        .map(|p| [value(3 * p), value(3 * p + 1), value(3 * p + 2)])
        .collect();
    // Each cell its own points, in a shuffled order (the writer must not reorder them).
    let conn = [
        vec![3, 0, 2, 1],
        vec![4, 5, 6, 7, 11, 10, 9, 8],
        vec![21, 12, 13, 14, 15, 16, 17, 18, 19, 20],
        vec![22, 29, 23, 28, 24, 27, 25, 26],
    ];
    Grid {
        points,
        conn,
        temperature: (0..30).map(|i| value(i + 100)).collect(),
        velocity: (0..90).map(|i| value(i + 200)).collect(),
        id: (0..4).map(|i| value(i + 300)).collect(),
        stress: (0..24).map(|i| value(i + 400)).collect(),
    }
}

fn blocks(g: &Grid) -> [FeCellBlock<'_>; 4] {
    [
        FeCellBlock {
            cell_type: VTK_TETRA,
            connectivity: &g.conn[0],
        },
        FeCellBlock {
            cell_type: VTK_HEXAHEDRON,
            connectivity: &g.conn[1],
        },
        FeCellBlock {
            cell_type: VTK_QUADRATIC_TETRA,
            connectivity: &g.conn[2],
        },
        FeCellBlock {
            cell_type: VTK_HEXAHEDRON,
            connectivity: &g.conn[3],
        },
    ]
}

fn arr<'a>(name: &'a str, components: usize, values: &'a [f64]) -> FeArray<'a> {
    FeArray {
        name,
        components,
        values,
    }
}

fn written(g: &Grid) -> String {
    fe_vtu_string(
        &g.points,
        &blocks(g),
        &[arr("temperature", 1, &g.temperature), arr("velocity", 3, &g.velocity)],
        &[arr("id", 1, &g.id), arr("stress", 6, &g.stress)],
    )
    .unwrap()
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn w1_counts_offsets_types_and_n1() {
    let g = grid();
    let xml = written(&g);
    let p = parse_consistent(&xml).expect("W1 consistent");
    let conn: Vec<usize> = g.conn.iter().flatten().copied().collect();
    let w1 = p.n_points == 30
        && p.n_cells == 4
        && p.connectivity == conn
        && p.offsets == vec![4, 12, 22, 30]
        && p.types == vec![10, 12, 24, 12];
    // N1: one offset increased by 1 must be reported.
    let cells = section(&xml, "Cells").unwrap();
    let bad_cells = cells.replacen("4 12 22 30", "4 13 22 30", 1);
    assert_ne!(bad_cells, cells, "N1 set-up");
    let bad = xml.replacen(cells, &bad_cells, 1);
    let n1 = parse_consistent(&bad);
    println!(
        "W1: points {}, cells {}, offsets {:?}, types {:?}, connectivity equal {}; N1 reported: {:?}",
        p.n_points,
        p.n_cells,
        p.offsets,
        p.types,
        p.connectivity == conn,
        n1.as_ref().err()
    );
    assert!(w1, "W1");
    assert!(n1.is_err(), "N1");
}

#[test]
fn w2_every_value_has_the_bits_written() {
    let g = grid();
    let xml = written(&g);
    let p = parse_consistent(&xml).unwrap();
    let pts: Vec<f64> = g.points.iter().flatten().copied().collect();
    let pd = section(&xml, "PointData").unwrap();
    let cd = section(&xml, "CellData").unwrap();
    let get = |sec: &str, name: &str, comps: usize| {
        let (_, c, t) = array(sec, name).unwrap();
        assert_eq!(c, comps, "{name} components");
        f64s(&t)
    };
    let checks = [
        ("Points", bits_equal(&p.points, &pts)),
        ("temperature", bits_equal(&get(pd, "temperature", 1), &g.temperature)),
        ("velocity", bits_equal(&get(pd, "velocity", 3), &g.velocity)),
        ("id", bits_equal(&get(cd, "id", 1), &g.id)),
        ("stress", bits_equal(&get(cd, "stress", 6), &g.stress)),
    ];
    let specials = SPECIAL.iter().all(|s| pts.iter().any(|x| x.to_bits() == s.to_bits()));
    println!("W2: {checks:?}; every special value present in the points {specials}");
    assert!(specials, "W2 set-up");
    assert!(checks.iter().all(|c| c.1), "W2 {checks:?}");
    // The file is written as the string.
    let dir = std::env::temp_dir().join(format!("rcfd_sb_vtk_elements_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("grid.vtu");
    write_fe_vtu(
        &path,
        &g.points,
        &blocks(&g),
        &[arr("temperature", 1, &g.temperature), arr("velocity", 3, &g.velocity)],
        &[arr("id", 1, &g.id), arr("stress", 6, &g.stress)],
    )
    .unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), xml);
    let _ = std::fs::remove_dir_all(&dir);
}

fn refused_path(r: rcfd_core::Result<String>) -> (String, String) {
    match r {
        Err(Error::InvalidValue { path, reason }) => (path, reason),
        other => panic!("expected a refusal, got {:?}", other.map(|s| s.len())),
    }
}

#[test]
fn w3_a_field_of_the_wrong_length_is_refused_by_name() {
    let g = grid();
    let b = blocks(&g);
    let short = &g.temperature[..29];
    let mut long = g.stress.clone();
    long.push(1.0);
    let odd: Vec<f64> = (0..6 * 4 + 3).map(|i| i as f64).collect();
    let cases = [
        (
            "temperature",
            fe_vtu_string(&g.points, &b, &[arr("temperature", 1, short)], &[]),
        ),
        ("stress", fe_vtu_string(&g.points, &b, &[], &[arr("stress", 6, &long)])),
        ("strain", fe_vtu_string(&g.points, &b, &[], &[arr("strain", 6, &odd)])),
    ];
    for (name, r) in cases {
        let (path, reason) = refused_path(r);
        println!("W3 {name}: path '{path}', reason: {reason}");
        assert_eq!(path, name, "W3 path");
        assert!(reason.contains(name), "W3 message names the array");
    }
}

#[test]
fn w4_other_refusals_name_the_key_or_array() {
    let g = grid();
    let tet = [0usize, 1, 2, 3];
    let nine: Vec<usize> = (0..9).collect();
    let out_of_range = [0usize, 1, 2, 30];
    let one = [1.0; 30];
    let blk = |cell_type, connectivity| FeCellBlock {
        cell_type,
        connectivity,
    };
    let good = [blk(VTK_TETRA, &tet[..])];
    let cases: Vec<(&str, rcfd_core::Result<String>)> = vec![
        ("cell_type", fe_vtu_string(&g.points, &[blk(42, &tet[..])], &[], &[])),
        ("cell_type", fe_vtu_string(&g.points, &[blk(13, &tet[..])], &[], &[])),
        (
            "connectivity",
            fe_vtu_string(&g.points, &[blk(VTK_HEXAHEDRON, &nine[..])], &[], &[]),
        ),
        (
            "connectivity",
            fe_vtu_string(&g.points, &[blk(VTK_TETRA, &out_of_range[..])], &[], &[]),
        ),
        ("zero", fe_vtu_string(&g.points, &good, &[arr("zero", 0, &[])], &[])),
        ("name", fe_vtu_string(&g.points, &good, &[arr("", 1, &one)], &[])),
        ("a\"b", fe_vtu_string(&g.points, &good, &[arr("a\"b", 1, &one)], &[])),
        (
            "t",
            fe_vtu_string(&g.points, &good, &[arr("t", 1, &one), arr("t", 1, &one)], &[]),
        ),
    ];
    for (key, r) in cases {
        let (path, reason) = refused_path(r);
        println!("W4 {key}: path '{path}', reason: {reason}");
        assert_eq!(path, key, "W4");
    }
}
