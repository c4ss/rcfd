//! Fluent `.msh` reader (ASCII), from the publicly documented section layout.
//!
//! Handled sections: dimension (2), nodes (10), cells (12), faces (13) and
//! zone names (45 / 39). Cell shapes are irrelevant: cells are rebuilt from
//! their faces, so polyhedral meshes work. Binary sections (20xx/30xx),
//! 2D meshes and multi-region files are rejected with an explicit error.

use crate::common::{build_mesh, SurfaceGroup};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::{Mesh, PatchKind};
use std::collections::HashMap;
use std::path::Path;

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Io(format!("fluent: {}", msg.into())))
}

#[derive(Debug)]
enum Node<'a> {
    Atom(&'a str),
    List(Vec<Node<'a>>),
}

fn parse_sexpr(src: &str) -> Result<Vec<Node<'_>>> {
    let b = src.as_bytes();
    let mut stack: Vec<Vec<Node>> = vec![vec![]];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'(' => {
                stack.push(vec![]);
                i += 1;
            }
            b')' => {
                if stack.len() < 2 {
                    return bad("unbalanced ')'");
                }
                let l = stack.pop().expect("non-empty");
                stack.last_mut().expect("parent").push(Node::List(l));
                i += 1;
            }
            b'"' => {
                let s = i + 1;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += 1;
                }
                stack.last_mut().expect("top").push(Node::Atom(&src[s..i.min(b.len())]));
                i += 1;
            }
            c if c.is_ascii_whitespace() => i += 1,
            _ => {
                let s = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'(' && b[i] != b')' && b[i] != b'"' {
                    i += 1;
                }
                stack.last_mut().expect("top").push(Node::Atom(&src[s..i]));
            }
        }
    }
    if stack.len() != 1 {
        return bad("unbalanced '('");
    }
    Ok(stack.pop().expect("root"))
}

fn atom<'a>(n: &Node<'a>) -> Option<&'a str> {
    match n {
        Node::Atom(a) => Some(a),
        Node::List(_) => None,
    }
}

fn hex(s: &str) -> Result<usize> {
    usize::from_str_radix(s, 16).or_else(|_| bad(format!("bad hex number '{s}'")))
}

struct FaceRec {
    nodes: Vec<usize>,
    c0: usize,
    c1: usize,
}

struct FaceZone {
    id: usize,
    bc_type: usize,
    faces: Vec<FaceRec>,
}

/// Read a Fluent ASCII `.msh` file.
pub fn read_file(path: impl AsRef<Path>) -> Result<Mesh> {
    let src = std::fs::read(path.as_ref())?;
    read_str(&String::from_utf8_lossy(&src))
}

/// Parse Fluent ASCII mesh text.
pub fn read_str(src: &str) -> Result<Mesh> {
    let top = parse_sexpr(src)?;
    let mut points: Vec<Vec3> = Vec::new();
    let mut zone_names: HashMap<usize, String> = HashMap::new();
    let mut cell_zones: Vec<(usize, usize, usize)> = Vec::new();
    let mut face_zones: Vec<FaceZone> = Vec::new();
    for sec in &top {
        let Node::List(items) = sec else { continue };
        let Some(first) = items.first().and_then(atom) else {
            continue;
        };
        let Ok(index) = first.parse::<usize>() else { continue };
        match index {
            0 | 1 => {}
            2 => {
                if items.get(1).and_then(atom) != Some("3") {
                    return bad("only 3D meshes are supported");
                }
            }
            10 => {
                let Some(Node::List(h)) = items.get(1) else {
                    return bad("malformed node section");
                };
                let zone = hex(atom(&h[0]).unwrap_or("0"))?;
                if zone == 0 {
                    continue;
                }
                let (f, l) = (hex(atom(&h[1]).unwrap_or("0"))?, hex(atom(&h[2]).unwrap_or("0"))?);
                let nd = h
                    .get(4)
                    .and_then(atom)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(3);
                if nd != 3 {
                    return bad("only 3D node sections are supported");
                }
                let Some(Node::List(data)) = items.get(2) else {
                    return bad("node section without data");
                };
                if data.len() != 3 * (l + 1 - f) {
                    return bad(format!(
                        "node section {zone}: expected {} coordinates, found {}",
                        3 * (l + 1 - f),
                        data.len()
                    ));
                }
                if points.len() < l {
                    points.resize(l, [0.0; 3]);
                }
                for (k, c) in data.chunks(3).enumerate() {
                    let mut p = [0.0; 3];
                    for (d, n) in c.iter().enumerate() {
                        let s = atom(n).unwrap_or("");
                        p[d] = s.parse().or_else(|_| bad(format!("bad coordinate '{s}'")))?;
                    }
                    points[f - 1 + k] = p;
                }
            }
            12 => {
                let Some(Node::List(h)) = items.get(1) else {
                    return bad("malformed cell section");
                };
                let zone = hex(atom(&h[0]).unwrap_or("0"))?;
                if zone == 0 {
                    continue;
                }
                let (f, l) = (hex(atom(&h[1]).unwrap_or("0"))?, hex(atom(&h[2]).unwrap_or("0"))?);
                let ty = h.get(3).and_then(atom).map(hex).transpose()?.unwrap_or(1);
                if ty != 0 {
                    cell_zones.push((zone, f, l));
                }
            }
            13 => {
                let Some(Node::List(h)) = items.get(1) else {
                    return bad("malformed face section");
                };
                let zone = hex(atom(&h[0]).unwrap_or("0"))?;
                if zone == 0 {
                    continue;
                }
                let bc_type = hex(atom(&h[3]).unwrap_or("0"))?;
                let ftype = hex(atom(&h[4]).unwrap_or("0"))?;
                let Some(Node::List(data)) = items.get(2) else {
                    return bad("face section without data");
                };
                let toks: Vec<&str> = data.iter().filter_map(atom).collect();
                let mut faces = Vec::new();
                let mut i = 0;
                while i < toks.len() {
                    let n = if ftype == 0 || ftype == 5 {
                        let n = hex(toks[i])?;
                        i += 1;
                        n
                    } else {
                        ftype
                    };
                    if i + n + 2 > toks.len() {
                        return bad("truncated face record");
                    }
                    let nodes: Result<Vec<usize>> = toks[i..i + n]
                        .iter()
                        .map(|t| {
                            hex(t)
                                .and_then(|v| v.checked_sub(1).ok_or_else(|| Error::Io("fluent: node index 0".into())))
                        })
                        .collect();
                    faces.push(FaceRec {
                        nodes: nodes?,
                        c0: hex(toks[i + n])?,
                        c1: hex(toks[i + n + 1])?,
                    });
                    i += n + 2;
                }
                face_zones.push(FaceZone {
                    id: zone,
                    bc_type,
                    faces,
                });
            }
            45 | 39 => {
                if let Some(Node::List(h)) = items.get(1) {
                    let id = h.first().and_then(atom).and_then(|s| s.parse::<usize>().ok());
                    let name = h.get(2).and_then(atom);
                    if let (Some(id), Some(name)) = (id, name) {
                        zone_names.insert(id, name.to_string());
                    }
                }
            }
            i if i >= 2000 => return bad("binary Fluent mesh sections are not supported"),
            _ => {}
        }
    }
    if points.is_empty() || face_zones.is_empty() {
        return bad("no nodes or faces found");
    }
    let n_cells = face_zones
        .iter()
        .flat_map(|z| z.faces.iter())
        .map(|f| f.c0.max(f.c1))
        .max()
        .unwrap_or(0);
    let mut cells: Vec<Vec<Vec<usize>>> = vec![vec![]; n_cells];
    for z in &face_zones {
        for f in &z.faces {
            if f.nodes.iter().any(|&p| p >= points.len()) {
                return bad("face references a node out of range");
            }
            for c in [f.c0, f.c1] {
                if c > 0 {
                    cells[c - 1].push(f.nodes.clone());
                }
            }
        }
    }
    if cells.iter().any(|c| c.is_empty()) {
        return bad("some cells have no faces");
    }
    let mut surfaces = Vec::new();
    for z in &face_zones {
        if z.bc_type == 2 {
            continue; // interior
        }
        let name = zone_names
            .get(&z.id)
            .cloned()
            .unwrap_or_else(|| format!("zone_{}", z.id));
        let kind = match z.bc_type {
            3 => Some(PatchKind::Wall),
            4 | 10 | 20 => Some(PatchKind::Inlet),
            5 | 36 | 31 => Some(PatchKind::Outlet),
            7 => Some(PatchKind::Symmetry),
            12 => Some(PatchKind::Cyclic),
            _ => None,
        };
        surfaces.push(SurfaceGroup {
            name,
            faces: z.faces.iter().map(|f| f.nodes.clone()).collect(),
            kind,
        });
    }
    let groups: Vec<(String, Vec<usize>)> = cell_zones
        .iter()
        .map(|&(id, f, l)| {
            (
                zone_names.get(&id).cloned().unwrap_or_else(|| format!("zone_{id}")),
                (f - 1..l.min(n_cells)).collect(),
            )
        })
        .collect();
    build_mesh(points, cells, surfaces, groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two unit hexes along x, point id (1-based) = 1 + x + 3y + 6z.
    fn sample(mixed: bool) -> String {
        let id = |x: usize, y: usize, z: usize| 1 + x + 3 * y + 6 * z;
        let quad = |a: [usize; 3], b: [usize; 3], c: [usize; 3], d: [usize; 3], c0: usize, c1: usize| {
            let n = |p: [usize; 3]| format!("{:x}", id(p[0], p[1], p[2]));
            format!(
                "{}{} {} {} {} {} {}\n",
                if mixed { "4 " } else { "" },
                n(a),
                n(b),
                n(c),
                n(d),
                c0,
                c1
            )
        };
        let mut s = String::from(
            "(0 \"Sample (with parens) comment\")\n(1 \"rcfd test\")\n(2 3)\n(10 (0 1 c 0 3))\n(10 (1 1 c 1 3)(\n",
        );
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..3 {
                    s += &format!("{:e} {:e} {:e}\n", x as f64, y as f64, z as f64);
                }
            }
        }
        s += "))\n(12 (0 1 2 0))\n(12 (7 1 2 1 4))\n";
        s += &format!("(13 (0 1 b 0 0))\n(13 (2 1 1 2 {})(\n", if mixed { 0 } else { 4 });
        s += &quad([1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1], 1, 2);
        s += ")\n)\n";
        // inlet x=0 (zone 4, pressure-inlet) cell 1
        s += &format!("(13 (4 2 2 4 {})(\n", if mixed { 0 } else { 4 });
        s += &quad([0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0], 1, 0);
        s += "))\n";
        // outlet x=2 (zone 5) cell 2
        s += &format!("(13 (5 3 3 5 {})(\n", if mixed { 0 } else { 4 });
        s += &quad([2, 0, 0], [2, 1, 0], [2, 1, 1], [2, 0, 1], 2, 0);
        s += "))\n";
        // walls (zone 3): remaining 8 faces
        s += &format!("(13 (3 4 b 3 {})(\n", if mixed { 0 } else { 4 });
        for (x0, c) in [(0, 1), (1, 2)] {
            s += &quad([x0, 0, 0], [x0 + 1, 0, 0], [x0 + 1, 0, 1], [x0, 0, 1], c, 0); // y=0
            s += &quad([x0, 1, 0], [x0, 1, 1], [x0 + 1, 1, 1], [x0 + 1, 1, 0], c, 0); // y=1
            s += &quad([x0, 0, 0], [x0, 1, 0], [x0 + 1, 1, 0], [x0 + 1, 0, 0], c, 0); // z=0
            s += &quad([x0, 0, 1], [x0 + 1, 0, 1], [x0 + 1, 1, 1], [x0, 1, 1], c, 0);
            // z=1
        }
        s += "))\n(45 (7 fluid fluid)())\n(45 (3 wall wall-3)())\n(45 (4 pressure-inlet in)())\n(45 (5 pressure-outlet out)())\n(45 (2 interior interior-7)())\n";
        s
    }

    #[test]
    fn reads_quad_and_mixed_face_sections() {
        let a = read_str(&sample(false)).unwrap();
        let b = read_str(&sample(true)).unwrap();
        assert_eq!(a.content_hash(), b.content_hash());
        for m in [&a, &b] {
            assert_eq!(m.n_cells(), 2);
            assert_eq!(m.n_points(), 12);
            assert_eq!(m.n_internal_faces(), 1);
            assert_eq!(m.n_faces(), 11);
            assert!((m.total_volume() - 2.0).abs() < 1e-12);
            let names: Vec<&str> = m.patches().iter().map(|p| p.name.as_str()).collect();
            assert_eq!(names, vec!["in", "out", "wall-3"]);
            assert_eq!(m.patch("in").unwrap().kind, PatchKind::Inlet);
            assert_eq!(m.patch("out").unwrap().kind, PatchKind::Outlet);
            assert_eq!(m.patch("wall-3").unwrap().kind, PatchKind::Wall);
            assert_eq!(m.patch("wall-3").unwrap().size, 8);
            assert_eq!(m.cell_zone("fluid").unwrap().ids, vec![0, 1]);
        }
    }

    #[test]
    fn rejects_unsupported_and_corrupt_input() {
        assert!(read_str("(2 2)\n").is_err());
        assert!(read_str("(2010 (1 1 2 3 4)(1 2 3))\n").is_err());
        assert!(read_str("(10 (1 1 2 1 3)(0 0 0))\n").is_err());
        assert!(read_str("((").is_err());
        assert!(read_str(&sample(false).replace("(13 (3 4 b 3 4)(", "(13 (3 4 b 3 4)(1")).is_err());
    }
}
