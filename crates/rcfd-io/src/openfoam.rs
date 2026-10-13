//! OpenFOAM `constant/polyMesh` reader and writer (ASCII).
//!
//! The on-disk layout (points, faces, owner, neighbour, boundary, optional
//! cellZones/faceZones) is the publicly documented owner/neighbour format,
//! which is also rcfd's native in-memory addressing, so no topology
//! reconstruction is needed.

use crate::common::guess_kind;
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::{Mesh, PatchKind};
use std::fmt::Write as _;
use std::path::Path;

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Io(format!("polyMesh: {}", msg.into())))
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Semi,
}

fn tokenize(src: &str) -> Vec<Tok> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '/' && b.get(i + 1) == Some(&'/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && b.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if c == '"' {
            let s = i + 1;
            i += 1;
            while i < b.len() && b[i] != '"' {
                i += 1;
            }
            out.push(Tok::Word(b[s..i.min(b.len())].iter().collect()));
            i += 1;
        } else {
            match c {
                '(' => out.push(Tok::LParen),
                ')' => out.push(Tok::RParen),
                '{' => out.push(Tok::LBrace),
                '}' => out.push(Tok::RBrace),
                ';' => out.push(Tok::Semi),
                _ => {
                    let s = i;
                    while i < b.len() && !b[i].is_whitespace() && !"(){};\"".contains(b[i]) {
                        i += 1;
                    }
                    out.push(Tok::Word(b[s..i].iter().collect()));
                    continue;
                }
            }
            i += 1;
        }
    }
    out
}

struct Cur {
    t: Vec<Tok>,
    i: usize,
}

impl Cur {
    fn next(&mut self) -> Option<Tok> {
        let t = self.t.get(self.i).cloned();
        self.i += 1;
        t
    }
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn word(&mut self) -> Result<String> {
        match self.next() {
            Some(Tok::Word(w)) => Ok(w),
            other => bad(format!("expected word, found {other:?}")),
        }
    }
    fn usize(&mut self) -> Result<usize> {
        let w = self.word()?;
        w.parse().or_else(|_| bad(format!("expected integer, found '{w}'")))
    }
    fn f64(&mut self) -> Result<f64> {
        let w = self.word()?;
        w.parse().or_else(|_| bad(format!("expected number, found '{w}'")))
    }
    fn expect(&mut self, t: Tok) -> Result<()> {
        match self.next() {
            Some(ref x) if *x == t => Ok(()),
            other => bad(format!("expected {t:?}, found {other:?}")),
        }
    }
}

/// Parse a file: skips the FoamFile header and rejects binary data.
fn open(path: &Path) -> Result<Cur> {
    let src = std::fs::read_to_string(path).map_err(|e| Error::Io(format!("{}: {e}", path.display())))?;
    let mut c = Cur {
        t: tokenize(&src),
        i: 0,
    };
    if matches!(c.peek(), Some(Tok::Word(w)) if w == "FoamFile") {
        c.next();
        c.expect(Tok::LBrace)?;
        let mut depth = 1;
        let mut binary = false;
        let mut prev = String::new();
        while depth > 0 {
            match c.next() {
                Some(Tok::LBrace) => depth += 1,
                Some(Tok::RBrace) => depth -= 1,
                Some(Tok::Word(w)) => {
                    if prev == "format" && w == "binary" {
                        binary = true;
                    }
                    prev = w;
                }
                Some(_) => {}
                None => return bad("unterminated FoamFile header"),
            }
        }
        if binary {
            return bad(format!(
                "{} is binary; only ASCII polyMesh is supported",
                path.display()
            ));
        }
    }
    Ok(c)
}

fn read_points(path: &Path) -> Result<Vec<Vec3>> {
    let mut c = open(path)?;
    let n = c.usize()?;
    c.expect(Tok::LParen)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        c.expect(Tok::LParen)?;
        v.push([c.f64()?, c.f64()?, c.f64()?]);
        c.expect(Tok::RParen)?;
    }
    c.expect(Tok::RParen)?;
    Ok(v)
}

fn read_faces(path: &Path) -> Result<Vec<Vec<usize>>> {
    let mut c = open(path)?;
    let n = c.usize()?;
    c.expect(Tok::LParen)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let k = c.usize()?;
        c.expect(Tok::LParen)?;
        let mut f = Vec::with_capacity(k);
        for _ in 0..k {
            f.push(c.usize()?);
        }
        c.expect(Tok::RParen)?;
        v.push(f);
    }
    c.expect(Tok::RParen)?;
    Ok(v)
}

fn read_labels(path: &Path) -> Result<Vec<usize>> {
    let mut c = open(path)?;
    let n = c.usize()?;
    c.expect(Tok::LParen)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(c.usize()?);
    }
    c.expect(Tok::RParen)?;
    Ok(v)
}

/// Parse `N ( name { key value...; ... } ... )`, returning name and key/value-token lists.
type Dict = Vec<(String, Vec<String>)>;
fn read_dict_list(path: &Path) -> Result<Vec<(String, Dict)>> {
    let mut c = open(path)?;
    let n = c.usize()?;
    c.expect(Tok::LParen)?;
    let mut out = Vec::new();
    for _ in 0..n {
        let name = c.word()?;
        c.expect(Tok::LBrace)?;
        let mut d = Vec::new();
        loop {
            match c.next() {
                Some(Tok::RBrace) => break,
                Some(Tok::Word(key)) => {
                    let mut val = Vec::new();
                    loop {
                        match c.next() {
                            Some(Tok::Semi) => break,
                            Some(Tok::Word(w)) => val.push(w),
                            Some(Tok::LParen) => val.push("(".into()),
                            Some(Tok::RParen) => val.push(")".into()),
                            other => return bad(format!("bad entry '{key}': {other:?}")),
                        }
                    }
                    d.push((key, val));
                }
                other => return bad(format!("unexpected token in dictionary '{name}': {other:?}")),
            }
        }
        out.push((name, d));
    }
    c.expect(Tok::RParen)?;
    Ok(out)
}

fn get<'a>(d: &'a Dict, key: &str) -> Option<&'a Vec<String>> {
    d.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn labels_in(v: &[String]) -> Vec<usize> {
    let open = v.iter().position(|w| w == "(");
    let close = v.iter().rposition(|w| w == ")");
    match (open, close) {
        (Some(o), Some(c)) if c > o => v[o + 1..c].iter().filter_map(|w| w.parse().ok()).collect(),
        _ => vec![],
    }
}

/// Read a `polyMesh` directory.
pub fn read_dir(dir: impl AsRef<Path>) -> Result<Mesh> {
    let dir = dir.as_ref();
    let points = read_points(&dir.join("points"))?;
    let faces = read_faces(&dir.join("faces"))?;
    let owner = read_labels(&dir.join("owner"))?;
    let neighbour = read_labels(&dir.join("neighbour"))?;
    if owner.len() != faces.len() {
        return bad(format!(
            "owner has {} entries but there are {} faces",
            owner.len(),
            faces.len()
        ));
    }
    let bnd = read_dict_list(&dir.join("boundary"))?;
    let mut patches = Vec::new();
    let mut partners = Vec::new();
    let mut next = neighbour.len();
    for (name, d) in &bnd {
        let word = |k: &str| get(d, k).and_then(|v| v.first()).cloned();
        let size: usize = word("nFaces")
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| Error::Io(format!("polyMesh: patch '{name}' lacks nFaces")))?;
        let start: usize = word("startFace")
            .and_then(|w| w.parse().ok())
            .ok_or_else(|| Error::Io(format!("polyMesh: patch '{name}' lacks startFace")))?;
        if start != next {
            return bad(format!(
                "patch '{name}' startFace {start} is not contiguous (expected {next})"
            ));
        }
        next += size;
        let ty = word("type").unwrap_or_default();
        let kind = match ty.as_str() {
            "wall" => PatchKind::Wall,
            "symmetryPlane" | "symmetry" => PatchKind::Symmetry,
            "empty" => PatchKind::Empty,
            "cyclic" | "cyclicAMI" => PatchKind::Cyclic,
            "patch" => match get(d, "inGroups") {
                Some(g) if g.iter().any(|w| w == "inlet") => PatchKind::Inlet,
                Some(g) if g.iter().any(|w| w == "outlet") => PatchKind::Outlet,
                _ => guess_kind(name),
            },
            _ => PatchKind::Generic,
        };
        if kind == PatchKind::Cyclic {
            if let Some(p) = word("neighbourPatch") {
                partners.push((name.clone(), p));
            }
        }
        patches.push((name.clone(), kind, size));
    }
    let mut mesh = Mesh::from_face_lists(points, faces, owner, neighbour, patches)?;
    for (a, b) in partners {
        mesh.set_periodic(&a, &b)?;
    }
    if let Ok(z) = read_dict_list(&dir.join("cellZones")) {
        for (name, d) in z {
            let ids = get(&d, "cellLabels").map(|v| labels_in(v)).unwrap_or_default();
            mesh.add_cell_zone(&name, ids)?;
        }
    }
    if let Ok(z) = read_dict_list(&dir.join("faceZones")) {
        for (name, d) in z {
            let ids = get(&d, "faceLabels").map(|v| labels_in(v)).unwrap_or_default();
            mesh.add_face_zone(&name, ids)?;
        }
    }
    Ok(mesh)
}

fn header(class: &str, object: &str, note: &str) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "FoamFile\n{{\n    version     2.0;\n    format      ascii;\n    class       {class};\n"
    );
    if !note.is_empty() {
        let _ = writeln!(s, "    note        \"{note}\";");
    }
    let _ = write!(
        s,
        "    location    \"constant/polyMesh\";\n    object      {object};\n}}\n\n"
    );
    s
}

/// Write `mesh` as an ASCII `polyMesh` directory (created if needed).
pub fn write_dir(mesh: &Mesh, dir: impl AsRef<Path>) -> Result<()> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir)?;
    let mut s = header("vectorField", "points", "");
    let _ = writeln!(s, "{}\n(", mesh.n_points());
    for p in mesh.points() {
        let _ = writeln!(s, "({} {} {})", p[0], p[1], p[2]);
    }
    s.push_str(")\n");
    std::fs::write(dir.join("points"), s)?;

    let mut s = header("faceList", "faces", "");
    let _ = writeln!(s, "{}\n(", mesh.n_faces());
    for f in 0..mesh.n_faces() {
        let ids = mesh.face_points(f);
        let _ = write!(s, "{}(", ids.len());
        for (i, p) in ids.iter().enumerate() {
            let _ = write!(s, "{}{}", if i > 0 { " " } else { "" }, p);
        }
        s.push_str(")\n");
    }
    s.push_str(")\n");
    std::fs::write(dir.join("faces"), s)?;

    let note = format!(
        "nPoints:{}  nCells:{}  nFaces:{}  nInternalFaces:{}",
        mesh.n_points(),
        mesh.n_cells(),
        mesh.n_faces(),
        mesh.n_internal_faces()
    );
    for (name, data) in [("owner", mesh.owner()), ("neighbour", mesh.neighbour())] {
        let mut s = header("labelList", name, &note);
        let _ = writeln!(s, "{}\n(", data.len());
        for v in data {
            let _ = writeln!(s, "{v}");
        }
        s.push_str(")\n");
        std::fs::write(dir.join(name), s)?;
    }

    let mut s = header("polyBoundaryMesh", "boundary", "");
    let _ = writeln!(s, "{}\n(", mesh.patches().len());
    for p in mesh.patches() {
        let ty = match p.kind {
            PatchKind::Wall => "wall",
            PatchKind::Symmetry => "symmetryPlane",
            PatchKind::Empty => "empty",
            PatchKind::Cyclic => "cyclic",
            _ => "patch",
        };
        let _ = writeln!(s, "    {}\n    {{\n        type {ty};", p.name);
        if let Some(n) = &p.periodic_with {
            let _ = writeln!(s, "        neighbourPatch {n};");
        }
        match p.kind {
            PatchKind::Inlet => s.push_str(
                "        inGroups List<word> 1(inlet);
",
            ),
            PatchKind::Outlet => s.push_str(
                "        inGroups List<word> 1(outlet);
",
            ),
            _ => {}
        }
        let _ = writeln!(s, "        nFaces {};\n        startFace {};\n    }}", p.size, p.start);
    }
    s.push_str(")\n");
    std::fs::write(dir.join("boundary"), s)?;

    for (file, class, zty, key, zones) in [
        ("cellZones", "regIOobject", "cellZone", "cellLabels", mesh.cell_zones()),
        ("faceZones", "regIOobject", "faceZone", "faceLabels", mesh.face_zones()),
    ] {
        if zones.is_empty() {
            let _ = std::fs::remove_file(dir.join(file));
            continue;
        }
        let mut s = header(class, file, "");
        let _ = writeln!(s, "{}\n(", zones.len());
        for z in zones {
            let _ = writeln!(
                s,
                "{}\n{{\n    type {zty};\n    {key} List<label> {}(",
                z.name,
                z.ids.len()
            );
            for id in &z.ids {
                let _ = writeln!(s, "{id}");
            }
            s.push_str(");\n}\n");
        }
        s.push_str(")\n");
        std::fs::write(dir.join(file), s)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::gen::{backward_step, box_mesh, cylinder_ogrid, Grading, StepSpec};
    use rcfd_mesh::quality::check_mesh;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("rcfd-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn round_trip_is_exact() {
        for (label, mut m) in [
            (
                "box",
                box_mesh(
                    [3, 2, 2],
                    [1.0, 0.7, 0.3],
                    [Grading::Expansion(3.0), Grading::Uniform, Grading::Uniform],
                )
                .unwrap(),
            ),
            (
                "step",
                backward_step(StepSpec {
                    l_in: 1.0,
                    l_out: 3.0,
                    h_in: 1.0,
                    step: 0.5,
                    depth: 0.1,
                    n: [3, 6, 4, 2],
                })
                .unwrap(),
            ),
            (
                "ogrid",
                cylinder_ogrid(16, 6, 0.5, 4.0, Grading::Expansion(5.0), 0.2).unwrap(),
            ),
        ] {
            m.add_cell_zone("zoneA", vec![0, 1]).unwrap();
            m.add_face_zone("fz", vec![0]).unwrap();
            let dir = tmp(label);
            write_dir(&m, &dir).unwrap();
            let back = read_dir(&dir).unwrap();
            assert_eq!(back.content_hash(), m.content_hash(), "{label}");
            assert_eq!(back.topology(), m.topology(), "{label}");
            assert!(check_mesh(&back).is_ok());
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn cyclic_patches_round_trip() {
        let mut m = box_mesh([2, 2, 2], [1.0; 3], [Grading::Uniform; 3]).unwrap();
        m.set_periodic("xmin", "xmax").unwrap();
        m.set_periodic("xmax", "xmin").unwrap();
        let dir = tmp("cyc");
        write_dir(&m, &dir).unwrap();
        let back = read_dir(&dir).unwrap();
        assert_eq!(back.patch("xmin").unwrap().periodic_with.as_deref(), Some("xmax"));
        assert_eq!(back.patch("xmin").unwrap().kind, PatchKind::Cyclic);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_hand_written_sample_with_comments() {
        // two hexes along x (owner/neighbour layout as produced by blockMesh-style tools)
        let dir = tmp("hand");
        std::fs::create_dir_all(&dir).unwrap();
        let hdr = |c: &str, o: &str| {
            format!("/*--- banner ---*/\nFoamFile\n{{\n version 2.0; format ascii; class {c}; object {o};\n}}\n// comment\n")
        };
        let pts: Vec<[f64; 3]> = (0..2)
            .flat_map(|z| (0..2).flat_map(move |y| (0..3).map(move |x| [x as f64, y as f64, z as f64])))
            .collect();
        let mut s = hdr("vectorField", "points") + "12\n(\n";
        for p in &pts {
            s += &format!("({} {} {})\n", p[0], p[1], p[2]);
        }
        std::fs::write(dir.join("points"), s + ")\n").unwrap();
        // point id = x + 3y + 6z
        let faces = [
            "4(1 4 10 7)",  // internal x=1
            "4(0 6 9 3)",   // x=0
            "4(0 1 7 6)",   // y=0 cell0
            "4(3 9 10 4)",  // y=1 cell0
            "4(0 3 4 1)",   // z=0 cell0
            "4(6 7 10 9)",  // z=1 cell0
            "4(2 5 11 8)",  // x=2  (cell1)
            "4(1 2 8 7)",   // y=0 cell1
            "4(4 10 11 5)", // y=1 cell1
            "4(1 4 5 2)",   // z=0 cell1
            "4(7 8 11 10)", // z=1 cell1
        ];
        let mut s = hdr("faceList", "faces") + "11\n(\n";
        for f in faces {
            s += f;
            s += "\n";
        }
        std::fs::write(dir.join("faces"), s + ")\n").unwrap();
        std::fs::write(
            dir.join("owner"),
            hdr("labelList", "owner") + "11(0 0 0 0 0 0 1 1 1 1 1)\n",
        )
        .unwrap();
        std::fs::write(dir.join("neighbour"), hdr("labelList", "neighbour") + "1(1)\n").unwrap();
        std::fs::write(
            dir.join("boundary"),
            hdr("polyBoundaryMesh", "boundary")
                + "3\n(\n inlet\n {\n type patch;\n nFaces 1;\n startFace 1;\n }\n walls\n {\n type wall;\n inGroups List<word> 1(wall);\n nFaces 4;\n startFace 2;\n }\n others\n {\n type empty;\n nFaces 5;\n startFace 6;\n }\n)\n",
        )
        .unwrap();
        let m = read_dir(&dir).unwrap();
        assert_eq!(m.n_cells(), 2);
        assert_eq!(m.n_internal_faces(), 1);
        assert!((m.total_volume() - 2.0).abs() < 1e-12);
        assert_eq!(m.patch("inlet").unwrap().kind, PatchKind::Inlet);
        assert_eq!(m.patch("walls").unwrap().kind, PatchKind::Wall);
        assert_eq!(m.patch("others").unwrap().kind, PatchKind::Empty);
        // the x=1 face normal points from cell 0 to cell 1 (+x)
        assert!(m.face_area_vectors()[0][0] > 0.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_bad_input() {
        let dir = tmp("bad");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(read_dir(&dir).is_err()); // missing files
        std::fs::write(dir.join("points"), "FoamFile { format binary; }\n1(0)\n").unwrap();
        assert!(read_points(&dir.join("points")).is_err());
        let m = box_mesh([1, 1, 1], [1.0; 3], [Grading::Uniform; 3]).unwrap();
        write_dir(&m, &dir).unwrap();
        // corrupt: non-contiguous patch
        let b = std::fs::read_to_string(dir.join("boundary"))
            .unwrap()
            .replace("startFace 1;", "startFace 4;");
        std::fs::write(dir.join("boundary"), b).unwrap();
        assert!(read_dir(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
