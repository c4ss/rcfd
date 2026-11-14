//! F-512 (ADR 0423): beam results in the solid output. Checks A1 to A6 and the negative controls N1 to N3 of the
//! registration (B1 is in `si_beam_output_bytes.rs`, B2 the existing tests). The parser is the one of `sb_solid_output.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam;
use rcfd_fem::builders::hex_box;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, DofMap, ElementKind, Load, Material, SolidMesh, SolidModel, SolidResult,
};
use rcfd_post::solid_output::{beam_fields, solid_vtu_string, write_solid_vtu};

// ------------------------------------------------------------------------------------------------------------ the parser

fn attr(tag_text: &str, key: &str) -> String {
    let k = format!("{key}=\"");
    let s = tag_text.find(&k).unwrap_or_else(|| panic!("attribute {key} missing")) + k.len();
    let e = tag_text[s..].find('"').unwrap() + s;
    tag_text[s..e].to_string()
}

fn section<'a>(xml: &'a str, tag: &str) -> &'a str {
    let open = format!("<{tag}>");
    let s = xml.find(&open).unwrap_or_else(|| panic!("{tag} missing")) + open.len();
    let e = xml[s..].find(&format!("</{tag}>")).unwrap() + s;
    &xml[s..e]
}

fn array(sec: &str, name: &str) -> (usize, Vec<String>) {
    let key = format!("Name=\"{name}\"");
    let at = sec.find(&key).unwrap_or_else(|| panic!("array {name} missing"));
    let start = sec[..at].rfind("<DataArray").unwrap();
    let head_end = sec[start..].find('>').unwrap() + start;
    let end = sec[head_end..].find("</DataArray>").unwrap() + head_end;
    let tokens = sec[head_end + 1..end].split_whitespace().map(str::to_string).collect();
    (
        attr(&sec[start..head_end], "NumberOfComponents").parse().unwrap(),
        tokens,
    )
}

fn names(sec: &str) -> Vec<(String, usize)> {
    sec.match_indices("<DataArray")
        .map(|(i, _)| {
            let e = sec[i..].find('>').unwrap() + i;
            (
                attr(&sec[i..e], "Name"),
                attr(&sec[i..e], "NumberOfComponents").parse().unwrap(),
            )
        })
        .collect()
}

fn f64s(tokens: &[String]) -> Vec<f64> {
    tokens.iter().map(|t| t.parse().unwrap()).collect()
}

fn usizes(tokens: &[String]) -> Vec<usize> {
    tokens.iter().map(|t| t.parse().unwrap()).collect()
}

struct Read {
    n_points: usize,
    n_cells: usize,
    points: Vec<f64>,
    offsets: Vec<usize>,
    connectivity: Vec<usize>,
    types: Vec<usize>,
    point_names: Vec<(String, usize)>,
    cell_names: Vec<(String, usize)>,
    xml: String,
}

impl Read {
    fn point(&self, name: &str) -> Vec<f64> {
        let (c, t) = array(section(&self.xml, "PointData"), name);
        assert_eq!(t.len(), c * self.n_points, "{name} length");
        f64s(&t)
    }
    fn cell(&self, name: &str) -> Vec<f64> {
        let (c, t) = array(section(&self.xml, "CellData"), name);
        assert_eq!(t.len(), c * self.n_cells, "{name} length");
        f64s(&t)
    }
}

fn parse(xml: &str) -> Read {
    let ps = xml.find("<Piece").unwrap();
    let pe = xml[ps..].find('>').unwrap() + ps;
    let cells = section(xml, "Cells");
    Read {
        n_points: attr(&xml[ps..pe], "NumberOfPoints").parse().unwrap(),
        n_cells: attr(&xml[ps..pe], "NumberOfCells").parse().unwrap(),
        points: f64s(&array(section(xml, "Points"), "Points").1),
        offsets: usizes(&array(cells, "offsets").1),
        connectivity: usizes(&array(cells, "connectivity").1),
        types: usizes(&array(cells, "types").1),
        point_names: names(section(xml, "PointData")),
        cell_names: names(section(xml, "CellData")),
        xml: xml.to_string(),
    }
}

// ----------------------------------------------------------------------------------------------------------- the checks

struct Checks(Vec<String>);

impl Checks {
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

/// `max |read - expected| / max |expected|` over the finite expected entries, and whether the NaN entries match.
fn read_error(read: &[f64], expected: &[f64]) -> (f64, bool) {
    assert_eq!(read.len(), expected.len(), "lengths");
    let scale = expected
        .iter()
        .filter(|x| x.is_finite())
        .fold(0.0f64, |m, x| m.max(x.abs()));
    let mut err = 0.0f64;
    let mut nan_ok = true;
    for (r, e) in read.iter().zip(expected) {
        if e.is_nan() {
            nan_ok &= r.is_nan();
        } else {
            nan_ok &= !r.is_nan();
            err = err.max((r - e).abs());
        }
    }
    (if scale > 0.0 { err / scale } else { err }, nan_ok)
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

// ---------------------------------------------------------------------------------------------------------- fixture F

const N_NODES: usize = 5;

fn section1() -> BeamSection {
    BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap()
}

fn frame_model() -> SolidModel {
    let nodes: Vec<Vec3> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [1.0, 1.0, 1.0],
        [2.0, 2.0, 2.0],
    ];
    let v = [1.0, 2.0, 3.0];
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2]).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![2, 3]).unwrap();
    for b in 0..2 {
        mesh.set_block_property(b, BlockProperty::beam(section1(), v).unwrap())
            .unwrap();
    }
    mesh.add_element_set("all", vec![0, 1, 2]).unwrap();
    mesh.add_node_set("root", vec![0]).unwrap();
    mesh.add_node_set("tip", vec![3]).unwrap();
    mesh.add_node_set("lone", vec![4]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("root", c, 0.0)).unwrap();
    }
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("lone", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("tip", [1200.0, -2500.0, 3700.0])).unwrap();
    m.add_load(Load::nodal_moment("tip", [450.0, -300.0, 200.0])).unwrap();
    m
}

/// The expected point arrays from the result's own accessors: (displacement, rotation, reaction, reaction moment).
/// (displacement, rotation, (reaction, reaction moment) when present).
type Expected = (Vec<f64>, Vec<f64>, Option<(Vec<f64>, Vec<f64>)>);

fn expected_points(r: &SolidResult) -> Expected {
    let mut d = Vec::new();
    let mut w = Vec::new();
    for node in 0..N_NODES {
        let u = r.displacement(node);
        d.extend_from_slice(&u[0..3]);
        if u.len() == 6 {
            w.extend_from_slice(&u[3..6]);
        } else {
            w.extend_from_slice(&[f64::NAN; 3]);
        }
    }
    let reac = r.reactions().map(|rv| {
        let mut f = Vec::new();
        let mut m = Vec::new();
        for node in 0..N_NODES {
            let s = r.dofs().equation(node, 0).unwrap();
            f.extend_from_slice(&rv[s..s + 3]);
            if r.dofs().components(node) == 6 {
                m.extend_from_slice(&rv[s + 3..s + 6]);
            } else {
                m.extend_from_slice(&[f64::NAN; 3]);
            }
        }
        (f, m)
    });
    (d, w, reac)
}

fn owned(v: &[(&str, usize)]) -> Vec<(String, usize)> {
    v.iter().map(|(n, c)| (n.to_string(), *c)).collect()
}

#[test]
fn a1_to_a6_beam_frame_written_and_read_back() {
    let model = frame_model();
    let res = solve_static(&model, &StaticOptions::default()).unwrap();
    let plain = SolidResult::new(res.dofs().clone(), res.displacement_values().to_vec()).unwrap();
    let mesh = model.mesh();
    let sf = beam::section_forces(&model, res.displacement_values()).unwrap();
    let sf_start: Vec<f64> = sf.iter().flat_map(|(_, s)| s.start).collect();
    let sf_end: Vec<f64> = sf.iter().flat_map(|(_, s)| s.end).collect();
    assert_eq!(sf.iter().map(|(e, _)| *e).collect::<Vec<_>>(), vec![0, 1, 2]);
    let mut c = Checks(Vec::new());
    for (tag, r) in [("F", &res), ("F'", &plain)] {
        let xml = solid_vtu_string(&model, r).unwrap();
        let rd = parse(&xml);
        let (d, w, reac) = expected_points(r);
        // A1
        let (e, nan) = read_error(&rd.point("displacement"), &d);
        c.at_most(&format!("A1 {tag} displacement read back"), e, 1e-15);
        c.holds(&format!("A1 {tag} displacement finite"), nan, "no NaN");
        let (e, nan) = read_error(&rd.point("rotation"), &w);
        c.at_most(&format!("A1 {tag} rotation read back"), e, 1e-15);
        c.holds(&format!("A1 {tag} rotation NaN at node 4 only"), nan, "NaN positions");
        c.holds(
            &format!("A1 {tag} rotation is NaN at node 4 and finite at 0..3"),
            w[12..15].iter().all(|x| x.is_nan()) && w[..12].iter().all(|x| x.is_finite()),
            "expected pattern",
        );
        // A2
        match &reac {
            Some((f, m)) => {
                let (e, nan) = read_error(&rd.point("reaction"), f);
                c.at_most(&format!("A2 {tag} reaction read back"), e, 1e-15);
                c.holds(&format!("A2 {tag} reaction finite"), nan, "no NaN");
                let (e, nan) = read_error(&rd.point("reaction_moment"), m);
                c.at_most(&format!("A2 {tag} reaction_moment read back"), e, 1e-15);
                c.holds(
                    &format!("A2 {tag} reaction_moment NaN at node 4 only"),
                    nan,
                    "NaN positions",
                );
                c.holds(
                    &format!("A2 {tag} root reaction moment nonzero"),
                    m[0..3].iter().any(|x| x.abs() > 0.0),
                    "the fixture has moments",
                );
            }
            None => c.holds(
                &format!("A2 {tag} reaction arrays absent"),
                !rd.xml.contains("Name=\"reaction\"") && !rd.xml.contains("Name=\"reaction_moment\""),
                "absent",
            ),
        }
        // A3
        let conn_ok = (0..3).all(|k| rd.connectivity[2 * k..2 * k + 2] == *mesh.element_nodes(k));
        c.holds(
            &format!("A3 {tag} line cells"),
            rd.n_cells == 3
                && rd.types == vec![3, 3, 3]
                && rd.offsets == vec![2, 4, 6]
                && rd.connectivity.len() == 6
                && conn_ok,
            &format!(
                "cells {}, types {:?}, offsets {:?}, connectivity {:?}",
                rd.n_cells, rd.types, rd.offsets, rd.connectivity
            ),
        );
        let pts: Vec<f64> = mesh.nodes().iter().flatten().copied().collect();
        c.holds(
            &format!("A3 {tag} points"),
            rd.n_points == N_NODES && bits_equal(&rd.points, &pts),
            "5 points, bits of the mesh nodes",
        );
        // A4
        let (e, _) = read_error(&rd.cell("section_force_start"), &sf_start);
        c.at_most(&format!("A4 {tag} section_force_start read back"), e, 1e-15);
        let (e, _) = read_error(&rd.cell("section_force_end"), &sf_end);
        c.at_most(&format!("A4 {tag} section_force_end read back"), e, 1e-15);
        // A5
        let want_points = if reac.is_some() {
            owned(&[
                ("displacement", 3),
                ("rotation", 3),
                ("reaction", 3),
                ("reaction_moment", 3),
            ])
        } else {
            owned(&[("displacement", 3), ("rotation", 3)])
        };
        let want_cells = owned(&[("section_force_start", 6), ("section_force_end", 6)]);
        c.holds(
            &format!("A5 {tag} array names"),
            rd.point_names == want_points && rd.cell_names == want_cells,
            &format!("point {:?}, cell {:?}", rd.point_names, rd.cell_names),
        );
        // A6
        let bf = beam_fields(&model, r).unwrap();
        let reac_bits = match (&bf.reaction, &bf.reaction_moment, &reac) {
            (Some(a), Some(b), Some((f, m))) => bits_equal(a, f) && bits_equal(b, m),
            (None, None, None) => true,
            _ => false,
        };
        c.holds(
            &format!("A6 {tag} beam_fields bits"),
            bits_equal(&bf.displacement, &d)
                && bits_equal(&bf.rotation, &w)
                && reac_bits
                && bits_equal(&bf.section_force_start, &sf_start)
                && bits_equal(&bf.section_force_end, &sf_end),
            "displacement, rotation, reactions, section forces",
        );
        let dir = std::env::temp_dir().join(format!("rcfd_si_beam_output_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("frame_{}.vtu", if reac.is_some() { "r" } else { "p" }));
        write_solid_vtu(&path, &model, r).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        c.holds(&format!("A6 {tag} file equals string"), text == xml, "write_solid_vtu");
    }
    c.finish();
}

#[test]
fn n1_n2_n3_refusals() {
    let mut c = Checks(Vec::new());
    // N1: a Tet4 block and a separate Beam2 block.
    let nodes: Vec<Vec3> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [3.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
    ];
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Tet4, vec![0, 1, 2, 3]).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![4, 5]).unwrap();
    mesh.set_block_property(1, BlockProperty::beam(section1(), [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    let mixed = SolidModel::new(mesh);
    let dofs = mixed.dof_map();
    let zeros = vec![0.0; dofs.n_equations()];
    let r = SolidResult::new(dofs, zeros).unwrap();
    let e1 = solid_vtu_string(&mixed, &r).unwrap_err().to_string();
    let e2 = beam_fields(&mixed, &r).unwrap_err().to_string();
    println!("N1: {e1}");
    c.holds(
        "N1 mixed model refused naming Beam2 and ADR 0423",
        e1.contains("Beam2") && e1.contains("ADR 0423") && e2.contains("Beam2") && e2.contains("ADR 0423"),
        &e1,
    );
    // N2: the frame with a result of another numbering.
    let model = frame_model();
    let other = SolidResult::new(DofMap::uniform(N_NODES, 3), vec![0.0; 3 * N_NODES]).unwrap();
    let e = solid_vtu_string(&model, &other).unwrap_err().to_string();
    println!("N2: {e}");
    c.holds(
        "N2 other DofMap refused naming displacement",
        e.contains("displacement") && beam_fields(&model, &other).is_err(),
        &e,
    );
    // N3 (amendment 1): a solid model with a result whose node 0 has six unknowns (and node 1 none, so the count is 3 n).
    let hex = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap());
    let n = hex.mesh().nodes().len();
    let mut counts = vec![3; n];
    counts[0] = 6;
    counts[1] = 0;
    let six = SolidResult::new(DofMap::from_counts(&counts), vec![0.0; 3 * n]).unwrap();
    let e = solid_vtu_string(&hex, &six).unwrap_err().to_string();
    println!("N3: {e}");
    c.holds(
        "N3 six components on a solid model refused as before",
        e.contains("node 0 has 6 components; the solid output writes 3"),
        &e,
    );
    c.finish();
}
