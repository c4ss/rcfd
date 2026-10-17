//! Format registry with a conformance case per supported format (F-237, ADR 0051 D7).
//!
//! Every [`Status::Supported`] entry has a conformance case: a tiny mesh plus fields is written (or an embedded sample is read)
//! and validated against an in-registry parser or the crate's own reader. The cases check internal consistency of each
//! implementation against its documented layout; they do NOT show acceptance by third-party software.
//! `tamper = true` corrupts the written data before validation (used to show that each case can fail).

use crate::{ensight, gmsh, nastran, openfoam, plot3d, stl, tecplot, vtk, xdmf};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;

/// What the crate does with the format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Reader only.
    Read,
    /// Writer only.
    Write,
    /// Both.
    ReadWrite,
}

/// Support status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Implemented with a conformance case.
    Supported,
    /// Not implemented, with the reason.
    NotSupported(&'static str),
}

/// A registry entry.
#[derive(Debug, Clone, Copy)]
pub struct FormatInfo {
    /// Registry name.
    pub name: &'static str,
    /// File extensions or markers.
    pub extensions: &'static [&'static str],
    /// Direction.
    pub direction: Direction,
    /// Status.
    pub status: Status,
    /// Conformance case (`tamper` corrupts the written data).
    pub conformance: Option<fn(bool) -> Result<(), String>>,
}

/// All formats known to the crate.
pub fn all() -> Vec<FormatInfo> {
    use Direction::*;
    let s = Status::Supported;
    let f = |name, extensions, direction, case: fn(bool) -> Result<(), String>| FormatInfo {
        name,
        extensions,
        direction,
        status: s,
        conformance: Some(case),
    };
    vec![
        f("vtu", &[".vtu"], Write, case_vtu),
        f("vtp", &[".vtp"], Write, case_vtp),
        f("vtk_legacy", &[".vtk"], Write, case_vtk_legacy),
        f("openfoam", &["polyMesh"], ReadWrite, case_openfoam),
        f("ensight", &[".case"], Write, case_ensight),
        f("stl", &[".stl"], ReadWrite, case_stl),
        f("tecplot", &[".dat"], ReadWrite, case_tecplot),
        f("xdmf", &[".xmf"], Write, case_xdmf),
        f("plot3d", &[".x", ".xyz", ".p3d", ".grd"], ReadWrite, case_plot3d),
        f("gmsh", &[".msh"], Read, case_gmsh),
        f("fluent", &[".msh", ".cas"], Read, case_fluent),
        f("nastran_deck", &[".bdf"], ReadWrite, case_deck),
        FormatInfo {
            name: "cgns",
            extensions: &[".cgns"],
            direction: ReadWrite,
            status: Status::NotSupported("no HDF5/ADF layer and no independent reader locally (ADR 0048 D7)"),
            conformance: None,
        },
        FormatInfo {
            name: "hdf5",
            extensions: &[".h5", ".hdf5"],
            direction: ReadWrite,
            status: Status::NotSupported("no HDF5 library adopted (ADR 0048 D7)"),
            conformance: None,
        },
    ]
}

/// Run the conformance case of one format (errors carry the format name).
pub fn conformance_with(name: &str, tamper: bool) -> Result<(), String> {
    let list = all();
    let info = list
        .iter()
        .find(|i| i.name == name)
        .ok_or_else(|| format!("format '{name}' is not in the registry"))?;
    match (info.status, info.conformance) {
        (Status::Supported, Some(case)) => case(tamper).map_err(|e| format!("{name}: {e}")),
        (Status::NotSupported(why), _) => Err(format!("{name}: not supported ({why})")),
        _ => Err(format!("{name}: no conformance case registered")),
    }
}

/// Run the conformance case of one format.
pub fn conformance_for(name: &str) -> Result<(), String> {
    conformance_with(name, false)
}

/// Run every supported format: `(name, result)`.
pub fn run_all() -> Vec<(&'static str, Result<(), String>)> {
    all()
        .into_iter()
        .filter(|i| i.status == Status::Supported)
        .map(|i| (i.name, conformance_for(i.name)))
        .collect()
}

// ---- helpers ---------------------------------------------------------------------------------------------------------------

const MARK: f64 = 123.5;
const MARK_TXT: &str = "123.5";
const MARK_BAD: &str = "124.5";

fn tiny() -> Mesh {
    box_mesh([2, 1, 1], [2.0, 1.0, 1.0], [Grading::Uniform; 3]).expect("tiny mesh")
}

fn vals() -> Vec<f64> {
    vec![MARK, 2.25]
}

fn nums(s: &str) -> Vec<f64> {
    s.split(|c: char| c.is_whitespace())
        .filter_map(|t| t.parse::<f64>().ok())
        .collect()
}

fn xml_array(xml: &str, name: &str) -> Result<Vec<f64>, String> {
    let key = format!("Name=\"{name}\"");
    let i = xml.find(&key).ok_or_else(|| format!("array '{name}' missing"))?;
    let rest = &xml[i..];
    let s = rest.find('>').ok_or("unterminated tag")? + 1;
    let e = rest.find("</DataArray>").ok_or("unterminated array")?;
    Ok(nums(&rest[s..e]))
}

fn same(a: &[f64], b: &[f64], rel: f64, what: &str) -> Result<(), String> {
    if a.len() != b.len() {
        return Err(format!("{what}: {} values read, {} written", a.len(), b.len()));
    }
    let scale = b.iter().fold(1e-300f64, |m, v| m.max(v.abs()));
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        if (x - y).abs() > rel * scale {
            return Err(format!("{what}: value {i} is {x}, written {y}"));
        }
    }
    Ok(())
}

fn pts(m: &Mesh) -> Vec<f64> {
    m.points().iter().flat_map(|p| p.iter().copied()).collect()
}

fn corrupt(s: String, tamper: bool) -> String {
    if tamper {
        s.replacen(MARK_TXT, MARK_BAD, 1)
            .replacen("1.23500e+02", "1.24500e+02", 1)
    } else {
        s
    }
}

// ---- cases ---------------------------------------------------------------------------------------------------------------------

fn case_vtu(tamper: bool) -> Result<(), String> {
    let m = tiny();
    let v = vals();
    let xml = vtk::vtu_string(
        &m,
        &[vtk::VtkField {
            name: "T",
            values: vtk::VtkValues::Scalar(&v),
        }],
    )
    .map_err(|e| e.to_string())?;
    let xml = corrupt(xml, tamper);
    if !xml.contains(&format!(
        "NumberOfPoints=\"{}\" NumberOfCells=\"{}\"",
        m.n_points(),
        m.n_cells()
    )) {
        return Err("piece counts missing".into());
    }
    same(&xml_array(&xml, "Points")?, &pts(&m), 0.0, "points")?;
    same(&xml_array(&xml, "T")?, &v, 0.0, "cell field")
}

fn case_vtp(tamper: bool) -> Result<(), String> {
    let m = tiny();
    let n = m.patch("xmin").ok_or("patch xmin")?.size;
    let v = vec![MARK; n];
    let xml = vtk::vtp_string(
        &m,
        "xmin",
        &[vtk::VtkField {
            name: "T",
            values: vtk::VtkValues::Scalar(&v),
        }],
    )
    .map_err(|e| e.to_string())?;
    let xml = corrupt(xml, tamper);
    if !xml.contains(&format!("NumberOfPolys=\"{n}\"")) {
        return Err("polygon count missing".into());
    }
    let p = xml_array(&xml, "Points")?;
    if p.len() != 12 {
        return Err(format!("{} point coordinates for one quad, expected 12", p.len()));
    }
    if p.chunks(3).any(|q| q[0].abs() > 1e-15) {
        return Err("patch xmin points are not on x = 0".into());
    }
    same(&xml_array(&xml, "T")?, &v, 0.0, "patch field")
}

fn case_vtk_legacy(tamper: bool) -> Result<(), String> {
    let m = tiny();
    let v = vals();
    let txt = vtk::legacy_vtk_string(
        &m,
        &[vtk::VtkField {
            name: "T",
            values: vtk::VtkValues::Scalar(&v),
        }],
    )
    .map_err(|e| e.to_string())?;
    let txt = corrupt(txt, tamper);
    if !txt.starts_with("# vtk DataFile Version") || !txt.contains("DATASET UNSTRUCTURED_GRID") {
        return Err("header".into());
    }
    let after = |key: &str| -> Result<&str, String> {
        let i = txt.find(key).ok_or_else(|| format!("section '{key}' missing"))?;
        Ok(&txt[i + key.len()..])
    };
    let p = after(&format!("POINTS {} double\n", m.n_points()))?;
    same(&nums(p)[..3 * m.n_points()], &pts(&m), 0.0, "points")?;
    let t = after("LOOKUP_TABLE default\n")?;
    same(&nums(t)[..2], &v, 0.0, "cell field")
}

fn case_openfoam(tamper: bool) -> Result<(), String> {
    let m = tiny();
    let dir = std::env::temp_dir().join(format!("rcfd_reg_foam_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    openfoam::write_dir(&m, &dir).map_err(|e| e.to_string())?;
    if tamper {
        let _ = std::fs::remove_file(dir.join("neighbour"));
    }
    let r = openfoam::read_dir(&dir).map_err(|e| e.to_string());
    let _ = std::fs::remove_dir_all(&dir);
    let r = r?;
    if r.n_cells() != m.n_cells() || r.n_faces() != m.n_faces() || r.n_internal_faces() != m.n_internal_faces() {
        return Err("cell or face counts differ after the round trip".into());
    }
    same(&pts(&r), &pts(&m), 0.0, "points")?;
    for p in m.patches() {
        let q = r.patch(&p.name).ok_or_else(|| format!("patch {} lost", p.name))?;
        if q.size != p.size {
            return Err(format!(
                "patch {} has {} faces after the round trip, {} written",
                p.name, q.size, p.size
            ));
        }
    }
    if (r.total_volume() - m.total_volume()).abs() > 1e-12 {
        return Err("volume differs".into());
    }
    Ok(())
}

fn case_ensight(tamper: bool) -> Result<(), String> {
    let m = tiny();
    let v = vals();
    let step = ensight::Step {
        time: 0.0,
        variables: vec![ensight::Variable {
            name: "T",
            data: ensight::Data::CellScalar(&v),
        }],
    };
    let files = ensight::build(&m, "conf", ensight::Encoding::Ascii, &[step]).map_err(|e| e.to_string())?;
    let text = |name_part: &str| -> Result<String, String> {
        let f = files
            .files
            .iter()
            .find(|(n, _)| n.contains(name_part))
            .ok_or_else(|| format!("no file containing '{name_part}'"))?;
        String::from_utf8(f.1.clone()).map_err(|_| "not text".to_string())
    };
    let geo = text(".geo")?;
    let var = corrupt(text(".T")?, tamper);
    let case = text(".case")?;
    if !case.contains("FORMAT") || !case.contains("GEOMETRY") || !case.contains("VARIABLE") {
        return Err("case file sections".into());
    }
    if geo.matches("hexa8").count() != 1 {
        return Err("hexa8 block missing".into());
    }
    // coordinates: count line, then x block, y block, z block
    let ci = geo.find("coordinates").ok_or("coordinates missing")?;
    let tail = nums(&geo[ci + "coordinates".len()..]);
    let n = tail[0] as usize;
    if n != m.n_points() {
        return Err(format!("{n} coordinates, mesh has {}", m.n_points()));
    }
    let mut want = Vec::new();
    for d in 0..3 {
        for p in m.points() {
            want.push(p[d]);
        }
    }
    same(&tail[1..1 + 3 * n], &want, 1e-5, "coordinates")?;
    let hi = var.find("hexa8").ok_or("variable file has no hexa8 block")?;
    same(&nums(&var[hi + 5..]), &v, 1e-5, "cell variable")
}

fn case_stl(tamper: bool) -> Result<(), String> {
    let m = tiny();
    let names = stl::surface_patch_names(&m);
    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let solid = stl::solid_from_patches(&m, "reg", &refs).map_err(|e| e.to_string())?;
    let ascii = stl::write_ascii(&solid).map_err(|e| e.to_string())?;
    let ascii = if tamper {
        ascii.replacen("vertex", "vertx", 1)
    } else {
        ascii
    };
    let back = stl::read_bytes(ascii.as_bytes()).map_err(|e| e.to_string())?;
    let bin = stl::write_binary(&solid).map_err(|e| e.to_string())?;
    let back_bin = stl::read_bytes(&bin).map_err(|e| e.to_string())?;
    for (what, s) in [("ascii", &back[0]), ("binary", &back_bin[0])] {
        if s.triangles.len() != solid.triangles.len() {
            return Err(format!(
                "{what}: {} triangles, written {}",
                s.triangles.len(),
                solid.triangles.len()
            ));
        }
        if (s.area() - solid.area()).abs() > 1e-6 * solid.area() {
            return Err(format!("{what}: area {} vs {}", s.area(), solid.area()));
        }
    }
    // box 2 x 1 x 1: surface area 10
    if (solid.area() - 10.0).abs() > 1e-12 {
        return Err("surface area is not 10".into());
    }
    Ok(())
}

fn case_tecplot(tamper: bool) -> Result<(), String> {
    let m = tiny();
    let v = vals();
    let ds = tecplot::dataset_from_mesh(&m, "reg", &[("T", &v)]).map_err(|e| e.to_string())?;
    let txt = tecplot::write(&ds, tecplot::Packing::Block).map_err(|e| e.to_string())?;
    let txt = corrupt(txt, tamper);
    let back = tecplot::read(txt.as_bytes()).map_err(|e| e.to_string())?;
    let z = back.zones.first().ok_or("no zone")?;
    if z.n_nodes != m.n_points() || z.n_elems != m.n_cells() {
        return Err(format!("zone has {} nodes and {} elements", z.n_nodes, z.n_elems));
    }
    let ti = back.variables.iter().position(|n| n == "T").ok_or("variable T lost")?;
    same(&z.data[ti], &v, 1e-12, "cell field")?;
    let r = tecplot::mesh_from_zone(&back, 0).map_err(|e| e.to_string())?;
    if (r.total_volume() - m.total_volume()).abs() > 1e-12 {
        return Err("volume differs after the round trip".into());
    }
    Ok(())
}

fn case_xdmf(tamper: bool) -> Result<(), String> {
    let m = tiny();
    let v = vals();
    let f = xdmf::build(&m, "reg", &[("T", &v)]).map_err(|e| e.to_string())?;
    let xml = if tamper {
        f.xml.replacen("<Xdmf", "<Xdmx", 1)
    } else {
        f.xml.clone()
    };
    xdmf::check_wellformed(&xml)?;
    let blob = |part: &str| f.blobs.iter().find(|(n, _)| n.contains(part)).map(|b| b.1.clone());
    let pb = blob("points")
        .or_else(|| blob("geometry"))
        .ok_or("points blob missing")?;
    if pb.len() != 8 * 3 * m.n_points() {
        return Err(format!("points blob has {} bytes", pb.len()));
    }
    let got: Vec<f64> = pb
        .chunks(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        .collect();
    same(&got, &pts(&m), 0.0, "points")?;
    let tb = f.blobs.iter().find(|(n, _)| n.contains("field0")).ok_or_else(|| {
        format!(
            "field blob missing among {:?}",
            f.blobs.iter().map(|b| b.0.clone()).collect::<Vec<_>>()
        )
    })?;
    let tv: Vec<f64> =
        tb.1.chunks(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect();
    same(&tv, &v, 0.0, "cell field")
}

fn case_plot3d(tamper: bool) -> Result<(), String> {
    let (ni, nj, nk) = (3usize, 2usize, 2usize);
    let n = ni * nj * nk;
    let mut b = plot3d::Block {
        ni,
        nj,
        nk,
        x: vec![0.0; n],
        y: vec![0.0; n],
        z: vec![0.0; n],
        iblank: None,
    };
    for k in 0..nk {
        for j in 0..nj {
            for i in 0..ni {
                let id = b.index(i, j, k);
                b.x[id] = i as f64 * 0.5;
                b.y[id] = j as f64 * 0.25 + 0.1 * i as f64;
                b.z[id] = k as f64;
            }
        }
    }
    let g = plot3d::Grid {
        dim: 3,
        blocks: vec![b],
    };
    for (binary, double) in [(false, true), (true, true), (true, false)] {
        let fmt = plot3d::Format {
            binary,
            double,
            big_endian: false,
            iblank: false,
            multi_grid: true,
        };
        let mut bytes = plot3d::write_grid(&g, &fmt).map_err(|e| e.to_string())?;
        if tamper && !binary {
            bytes = String::from_utf8(bytes)
                .map_err(|e| e.to_string())?
                .replacen('2', "3", 1)
                .into_bytes();
        }
        let back = plot3d::read_grid(&bytes).map_err(|e| e.to_string())?;
        let rel = if binary && !double { 1e-6 } else { 0.0 };
        let bb = &back.blocks[0];
        if (bb.ni, bb.nj, bb.nk) != (ni, nj, nk) {
            return Err("block dimensions differ".into());
        }
        same(&bb.x, &g.blocks[0].x, rel, "x")?;
        same(&bb.y, &g.blocks[0].y, rel, "y")?;
        same(&bb.z, &g.blocks[0].z, rel, "z")?;
    }
    Ok(())
}

const GMSH_SAMPLE: &str =
    "$MeshFormat\n2.2 0 8\n$EndMeshFormat\n$PhysicalNames\n2\n2 1 \"walls\"\n3 2 \"fluid\"\n$EndPhysicalNames\n\
$Nodes\n4\n1 0 0 0\n2 1 0 0\n3 0 1 0\n4 0 0 1\n$EndNodes\n$Elements\n5\n\
1 2 2 1 1 1 3 2\n2 2 2 1 1 1 2 4\n3 2 2 1 1 1 4 3\n4 2 2 1 1 2 3 4\n5 4 2 2 1 1 2 3 4\n$EndElements\n";

fn case_gmsh(tamper: bool) -> Result<(), String> {
    let s = if tamper {
        GMSH_SAMPLE.replace("$Nodes\n4", "$Nodes\n5")
    } else {
        GMSH_SAMPLE.to_string()
    };
    let m = gmsh::read_bytes(s.as_bytes()).map_err(|e| e.to_string())?;
    if m.n_cells() != 1 || m.n_points() != 4 {
        return Err(format!("{} cells and {} points read", m.n_cells(), m.n_points()));
    }
    if (m.total_volume() - 1.0 / 6.0).abs() > 1e-14 {
        return Err(format!("tetrahedron volume {}", m.total_volume()));
    }
    Ok(())
}

fn fluent_sample() -> String {
    let id = |x: usize, y: usize, z: usize| format!("{:x}", 1 + x + 2 * y + 4 * z);
    let quad = |a: [usize; 3], b: [usize; 3], c: [usize; 3], d: [usize; 3]| {
        format!(
            "{} {} {} {} 1 0\n",
            id(a[0], a[1], a[2]),
            id(b[0], b[1], b[2]),
            id(c[0], c[1], c[2]),
            id(d[0], d[1], d[2])
        )
    };
    let mut s = String::from("(0 \"rcfd registry sample\")\n(2 3)\n(10 (0 1 8 0 3))\n(10 (1 1 8 1 3)(\n");
    for z in 0..2 {
        for y in 0..2 {
            for x in 0..2 {
                s += &format!("{:e} {:e} {:e}\n", x as f64, y as f64, z as f64);
            }
        }
    }
    s += "))\n(12 (0 1 1 0))\n(12 (7 1 1 1 4))\n(13 (0 1 6 0 0))\n(13 (3 1 6 3 4)(\n";
    s += &quad([0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]);
    s += &quad([1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]);
    s += &quad([0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]);
    s += &quad([0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]);
    s += &quad([0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]);
    s += &quad([0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]);
    s += "))\n";
    s
}

fn case_fluent(tamper: bool) -> Result<(), String> {
    let mut s = fluent_sample();
    if tamper {
        s = s.replacen("(12 (7 1 1 1 4))", "(12 (7 1 2 1 4))", 1);
    }
    let m = crate::fluent::read_str(&s).map_err(|e| e.to_string())?;
    if m.n_cells() != 1 || (m.total_volume() - 1.0).abs() > 1e-14 {
        return Err(format!("{} cells, volume {}", m.n_cells(), m.total_volume()));
    }
    Ok(())
}

fn case_deck(tamper: bool) -> Result<(), String> {
    let deck = nastran::Deck {
        grids: vec![
            (1, [0.0, 0.0, 0.0]),
            (2, [1.0, 0.0, 0.0]),
            (3, [1.0, 1.0, 0.0]),
            (4, [0.0, 1.0, 0.1 + 0.2]),
        ],
        elements: vec![nastran::Element {
            kind: nastran::ElemKind::Quad4,
            id: 10,
            pid: 1,
            nodes: vec![1, 2, 3, 4],
        }],
        pload4: vec![nastran::Pload4 {
            sid: 7,
            eid: 10,
            p: [MARK, f64::NAN, f64::NAN, f64::NAN],
        }],
        temps: vec![(8, 1, 300.0 / 7.0), (8, 2, 301.5), (8, 3, 302.0), (8, 4, 303.25)],
    };
    let txt = deck.to_string_checked().map_err(|e| e.to_string())?;
    let txt = corrupt(txt, tamper);
    let back = nastran::Deck::parse(&txt).map_err(|e| e.to_string())?;
    let eq = |a: f64, b: f64| a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan());
    let ok = back.grids == deck.grids
        && back.elements == deck.elements
        && back.pload4.len() == 1
        && back.pload4[0].p.iter().zip(&deck.pload4[0].p).all(|(a, b)| eq(*a, *b))
        && back.temps == deck.temps;
    if ok {
        Ok(())
    } else {
        Err("deck changed in the round trip".into())
    }
}
