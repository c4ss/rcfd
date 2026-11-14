//! F-521 (ADR 0437): shell results in the solid output. Checks S1 to S6, X1, R1 and the negative controls N1 to N8 of the
//! registration (B1 is `si_beam_output_bytes.rs`, B1b `si_shell_output_bytes.rs`, B2 the existing tests). The parser is the one
//! of `si_beam_output.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::shell::stresses;
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{
    beam, BeamSection, BlockProperty, Constraint, DofMap, ElementKind, Load, Material, SolidMesh, SolidModel,
    SolidResult,
};
use rcfd_post::solid_output::{shell_fields, solid_vtu_string, write_solid_vtu};

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

// ------------------------------------------------------------------------------------------------- fixtures S and X

const T: f64 = 0.01;

fn section1() -> BeamSection {
    BeamSection::new(0.012, 3.0e-6, 5.0e-6, 4.0e-6, 0.85, 0.75).unwrap()
}

fn strip_point(i: usize, j: usize) -> Vec3 {
    let ea = [2.0 / 3.0, 1.0 / 3.0, 2.0 / 3.0];
    let eb = [-2.0 / 3.0, 2.0 / 3.0, 1.0 / 3.0];
    let (a, b) = (0.25 * i as f64, 0.1 * j as f64);
    [a * ea[0] + b * eb[0], a * ea[1] + b * eb[1], a * ea[2] + b * eb[2]]
}

/// Fixture S (`mixed = false`) or X (`mixed = true`) of the registration. `property = false` leaves the Shell4 block without
/// its thickness (N8).
fn strip_model(mixed: bool, property: bool) -> SolidModel {
    let mut nodes: Vec<Vec3> = Vec::new();
    for j in 0..2 {
        for i in 0..5 {
            nodes.push(strip_point(i, j));
        }
    }
    nodes.push([1.0, 1.0, 1.0]);
    if mixed {
        let p4 = nodes[4];
        nodes.push([p4[0] + 0.3, p4[1] + 0.2, p4[2] - 0.1]);
        nodes.push([2.0, 2.0, 2.0]);
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let mut conn = Vec::new();
    for i in 0..4 {
        conn.extend_from_slice(&[i, i + 1, i + 6, i + 5]);
    }
    let shell_block = if mixed {
        mesh.add_block(ElementKind::Beam2, vec![4, 11]).unwrap();
        mesh.set_block_property(0, BlockProperty::beam(section1(), [1.0, 2.0, 3.0]).unwrap())
            .unwrap();
        mesh.add_block(ElementKind::Shell4, conn).unwrap()
    } else {
        mesh.add_block(ElementKind::Shell4, conn).unwrap()
    };
    if property {
        mesh.set_block_property(shell_block, BlockProperty::shell(T).unwrap())
            .unwrap();
    }
    let n_el = if mixed { 5 } else { 4 };
    mesh.add_element_set("all", (0..n_el).collect()).unwrap();
    mesh.add_node_set("clamp", vec![0, 5]).unwrap();
    mesh.add_node_set("lone", vec![10]).unwrap();
    if mixed {
        mesh.add_node_set("lone2", vec![12]).unwrap();
    }
    mesh.add_node_set("force", vec![4]).unwrap();
    mesh.add_node_set("moment", vec![9]).unwrap();
    model_with(mesh, if mixed { &["lone", "lone2"] } else { &["lone"] })
}

fn model_with(mesh: SolidMesh, lone: &[&str]) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.1e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("clamp", c, 0.0)).unwrap();
    }
    for set in lone {
        for c in 0..3 {
            m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
        }
    }
    m.add_load(Load::nodal_force("force", [30.0, -40.0, 50.0])).unwrap();
    m.add_load(Load::nodal_moment("moment", [3.0, -2.0, 1.0])).unwrap();
    m
}

fn solved(model: &SolidModel) -> (SolidResult, SolidResult) {
    let res = solve_static(model, &StaticOptions::default()).unwrap();
    let plain = SolidResult::new(res.dofs().clone(), res.displacement_values().to_vec()).unwrap();
    (res, plain)
}

/// Expected (displacement, rotation, (reaction, reaction moment) when present) from the result's own accessors.
type Expected = (Vec<f64>, Vec<f64>, Option<(Vec<f64>, Vec<f64>)>);

fn expected_points(r: &SolidResult, n: usize) -> Expected {
    let mut d = Vec::new();
    let mut w = Vec::new();
    for node in 0..n {
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
        for node in 0..n {
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

const SHELL_CELL_NAMES: [(&str, usize); 6] = [
    ("membrane_force", 3),
    ("moment", 3),
    ("shear_force", 2),
    ("stress_top", 3),
    ("stress_bottom", 3),
    ("shell_axes", 9),
];

/// The six shell arrays expected for `n_el` elements from `shell::stresses` (NaN on the other elements).
fn expected_shell(model: &SolidModel, r: &SolidResult, n_el: usize) -> Vec<Vec<f64>> {
    let widths = [3usize, 3, 2, 3, 3, 9];
    let mut out: Vec<Vec<f64>> = widths.iter().map(|w| vec![f64::NAN; w * n_el]).collect();
    for (e, s) in stresses(model, r.displacement_values()).unwrap() {
        let rows: [Vec<f64>; 6] = [
            s.membrane.to_vec(),
            s.bending.to_vec(),
            s.shear.to_vec(),
            s.top.to_vec(),
            s.bottom.to_vec(),
            s.axes.iter().flatten().copied().collect(),
        ];
        for (k, row) in rows.iter().enumerate() {
            out[k][widths[k] * e..widths[k] * (e + 1)].copy_from_slice(row);
        }
    }
    out
}

#[test]
fn s1_to_s6_shell_strip_written_and_read_back() {
    let model = strip_model(false, true);
    let (res, plain) = solved(&model);
    let mesh = model.mesh();
    let want = expected_shell(&model, &res, 4);
    assert!(want.iter().all(|a| a.iter().all(|x| x.is_finite())));
    let mut c = Checks(Vec::new());
    for (tag, r) in [("S", &res), ("Sp", &plain)] {
        let xml = solid_vtu_string(&model, r).unwrap();
        let rd = parse(&xml);
        let (d, w, reac) = expected_points(r, 11);
        // S1
        let (e, nan) = read_error(&rd.point("displacement"), &d);
        c.at_most(&format!("S1 {tag} displacement read back"), e, 1e-15);
        c.holds(&format!("S1 {tag} displacement finite"), nan, "no NaN");
        let (e, nan) = read_error(&rd.point("rotation"), &w);
        c.at_most(&format!("S1 {tag} rotation read back"), e, 1e-15);
        c.holds(&format!("S1 {tag} rotation NaN positions"), nan, "NaN positions");
        c.holds(
            &format!("S1 {tag} rotation NaN at node 10 and finite at 0..9"),
            w[30..33].iter().all(|x| x.is_nan()) && w[..30].iter().all(|x| x.is_finite()),
            "expected pattern",
        );
        // S2
        match (&reac, tag) {
            (Some((f, m)), "S") => {
                let (e, nan) = read_error(&rd.point("reaction"), f);
                c.at_most("S2 S reaction read back", e, 1e-15);
                c.holds("S2 S reaction finite", nan, "no NaN");
                let (e, nan) = read_error(&rd.point("reaction_moment"), m);
                c.at_most("S2 S reaction_moment read back", e, 1e-15);
                c.holds("S2 S reaction_moment NaN positions", nan, "NaN at node 10");
                c.holds(
                    "S2 S reaction moment NaN at node 10 and finite at 0..9",
                    m[30..33].iter().all(|x| x.is_nan()) && m[..30].iter().all(|x| x.is_finite()),
                    "expected pattern",
                );
                c.holds(
                    "S2 S reaction moment of the clamp is not zero",
                    m[..3].iter().any(|x| x.abs() > 1e-6),
                    "nonzero",
                );
            }
            (None, "Sp") => {
                let absent = !rd.point_names.iter().any(|(n, _)| n.starts_with("reaction"));
                c.holds("S2 Sp no reaction arrays", absent, "absent");
            }
            _ => panic!("reactions present exactly for S"),
        }
        // S3
        if tag == "S" {
            c.holds("S3 points", rd.n_points == 11, "11 points");
            let bits: Vec<f64> = mesh.nodes().iter().flatten().copied().collect();
            c.holds("S3 point bits", bits_equal(&rd.points, &bits), "bit for bit");
            c.holds("S3 cell count", rd.n_cells == 4, "4 cells");
            c.holds("S3 types", rd.types == vec![9; 4], "types 9");
            c.holds("S3 offsets", rd.offsets == vec![4, 8, 12, 16], "offsets");
            let conn: Vec<usize> = (0..4).flat_map(|k| mesh.element_nodes(k).to_vec()).collect();
            c.holds("S3 connectivity", rd.connectivity == conn, "element nodes in order");
        }
        // S4
        for (k, (name, comps)) in SHELL_CELL_NAMES.iter().enumerate() {
            assert_eq!(want[k].len(), comps * 4);
            let (e, nan) = read_error(&rd.cell(name), &want[k]);
            c.at_most(&format!("S4 {tag} {name} read back"), e, 1e-15);
            c.holds(&format!("S4 {tag} {name} finite"), nan, "no NaN");
        }
        // S5
        let mut pn = vec![("displacement", 3), ("rotation", 3)];
        if tag == "S" {
            pn.extend([("reaction", 3), ("reaction_moment", 3)]);
        }
        c.holds(
            &format!("S5 {tag} point array names"),
            rd.point_names == owned(&pn),
            &format!("{:?}", rd.point_names),
        );
        c.holds(
            &format!("S5 {tag} cell array names"),
            rd.cell_names == owned(&SHELL_CELL_NAMES),
            &format!("{:?}", rd.cell_names),
        );
        // S6
        let f = shell_fields(&model, r).unwrap();
        c.holds("S6 displacement bits", bits_equal(&f.displacement, &d), "bits");
        c.holds("S6 rotation bits", bits_equal(&f.rotation, &w), "bits");
        match (&f.reaction, &f.reaction_moment, &reac) {
            (Some(a), Some(b), Some((x, y))) => {
                c.holds("S6 reaction bits", bits_equal(a, x) && bits_equal(b, y), "bits")
            }
            (None, None, None) => {}
            _ => c.holds("S6 reactions presence", false, "mismatch"),
        }
        let got = [
            &f.membrane_force,
            &f.moment,
            &f.shear_force,
            &f.stress_top,
            &f.stress_bottom,
            &f.shell_axes,
        ];
        for (k, (name, _)) in SHELL_CELL_NAMES.iter().enumerate() {
            c.holds(&format!("S6 {tag} {name} bits"), bits_equal(got[k], &want[k]), "bits");
        }
        c.holds("S6 no section forces", f.section_force_start.is_none(), "None");
        // The file of write_solid_vtu equals the string.
        let path = std::env::temp_dir().join(format!("rcfd_f521_s6_{}.vtu", tag.len()));
        write_solid_vtu(&path, &model, r).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        c.holds(
            &format!("S6 {tag} file equals string"),
            bytes == xml.as_bytes(),
            "equal",
        );
        // S4b
        if tag == "S" {
            let top = rd.cell("stress_top");
            let bot = rd.cell("stress_bottom");
            let mom = rd.cell("moment");
            let scale = top.iter().fold(0.0f64, |m, x| m.max(x.abs()));
            let mut err = 0.0f64;
            for k in 0..12 {
                err = err.max(((top[k] - bot[k]) - 12.0 * mom[k] / (T * T)).abs());
            }
            c.at_most("S4b top - bottom against 12 M / t^2", err / scale, 1e-12);
        }
    }
    c.finish();
}

#[test]
fn x1_beam_first_mixed_model() {
    let model = strip_model(true, true);
    let (res, _) = solved(&model);
    let mesh = model.mesh();
    let xml = solid_vtu_string(&model, &res).unwrap();
    let rd = parse(&xml);
    let mut c = Checks(Vec::new());
    c.holds("X1 cells", rd.n_cells == 5 && rd.n_points == 13, "5 cells, 13 points");
    c.holds("X1 types", rd.types == vec![3, 9, 9, 9, 9], "3 9 9 9 9");
    c.holds("X1 offsets", rd.offsets == vec![2, 6, 10, 14, 18], "offsets");
    let conn: Vec<usize> = (0..5).flat_map(|k| mesh.element_nodes(k).to_vec()).collect();
    c.holds("X1 connectivity", rd.connectivity == conn, "element nodes");
    let mut names = owned(&SHELL_CELL_NAMES);
    names.push(("section_force_start".into(), 6));
    names.push(("section_force_end".into(), 6));
    c.holds("X1 cell names", rd.cell_names == names, &format!("{:?}", rd.cell_names));
    let want = expected_shell(&model, &res, 5);
    for (k, (name, comps)) in SHELL_CELL_NAMES.iter().enumerate() {
        let read = rd.cell(name);
        c.holds(
            &format!("X1 {name} NaN on the beam cell"),
            read[..*comps].iter().all(|x| x.is_nan()),
            "NaN",
        );
        let (e, nan) = read_error(&read, &want[k]);
        c.at_most(&format!("X1 {name} read back"), e, 1e-15);
        c.holds(&format!("X1 {name} NaN positions"), nan, "cell 0 only");
        c.holds(
            &format!("X1 {name} finite on shells"),
            read[*comps..].iter().all(|x| x.is_finite()),
            "finite",
        );
    }
    let sf = beam::section_forces(&model, res.displacement_values()).unwrap();
    assert_eq!(sf.len(), 1);
    assert_eq!(sf[0].0, 0);
    for (name, row) in [
        ("section_force_start", sf[0].1.start),
        ("section_force_end", sf[0].1.end),
    ] {
        let read = rd.cell(name);
        let (e, nan) = read_error(&read[..6], &row);
        c.at_most(&format!("X1 {name} read back"), e, 1e-15);
        c.holds(&format!("X1 {name} cell 0 finite"), nan, "finite");
        c.holds(
            &format!("X1 {name} NaN on shells"),
            read[6..].iter().all(|x| x.is_nan()) && read.len() == 30,
            "NaN",
        );
    }
    let (d, w, reac) = expected_points(&res, 13);
    let (e, _) = read_error(&rd.point("displacement"), &d);
    c.at_most("X1 displacement read back", e, 1e-15);
    let (e, nan) = read_error(&rd.point("rotation"), &w);
    c.at_most("X1 rotation read back", e, 1e-15);
    c.holds(
        "X1 rotation NaN at nodes 10 and 12 only",
        nan && [10usize, 12]
            .iter()
            .all(|&k| w[3 * k..3 * k + 3].iter().all(|x| x.is_nan()))
            && (0..13)
                .filter(|k| ![10, 12].contains(k))
                .all(|k| w[3 * k..3 * k + 3].iter().all(|x| x.is_finite())),
        "pattern",
    );
    let (f, m) = reac.unwrap();
    let (e, _) = read_error(&rd.point("reaction"), &f);
    c.at_most("X1 reaction read back", e, 1e-15);
    let (e, nan) = read_error(&rd.point("reaction_moment"), &m);
    c.at_most("X1 reaction_moment read back", e, 1e-15);
    c.holds("X1 reaction moment NaN positions", nan, "nodes 10, 12");
    c.holds(
        "X1 point names",
        rd.point_names
            == owned(&[
                ("displacement", 3),
                ("rotation", 3),
                ("reaction", 3),
                ("reaction_moment", 3),
            ]),
        &format!("{:?}", rd.point_names),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------ negative controls

fn result_with(counts: &[usize]) -> SolidResult {
    let dofs = DofMap::from_counts(counts);
    let n = dofs.n_equations();
    SolidResult::new(dofs, vec![0.0; n]).unwrap()
}

/// The refusal text of both entry points, required equal.
fn refusal(model: &SolidModel, r: &SolidResult) -> String {
    let a = solid_vtu_string(model, r).unwrap_err().to_string();
    let b = shell_fields(model, r).unwrap_err().to_string();
    assert_eq!(a, b);
    a
}

#[test]
fn n1_solid_and_shell_blocks_refused() {
    let nodes: Vec<Vec3> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [3.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [4.0, 1.0, 0.0],
        [3.0, 1.0, 0.0],
    ];
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Tet4, vec![0, 1, 2, 3]).unwrap();
    mesh.add_block(ElementKind::Shell4, vec![4, 5, 6, 7]).unwrap();
    mesh.set_block_property(1, BlockProperty::shell(T).unwrap()).unwrap();
    let model = SolidModel::new(mesh);
    let r = SolidResult::new(model.dof_map(), vec![0.0; model.dof_map().n_equations()]).unwrap();
    let e = refusal(&model, &r);
    println!("N1: {e}");
    assert!(
        e.contains("Tet4") && e.contains("Shell4") && e.contains("ADR 0437"),
        "{e}"
    );
}

#[test]
fn n2_models_without_shells_refused_by_shell_fields() {
    let hex = rcfd_fem::builders::hex_box([0.0; 3], [1.0; 3], [1, 1, 1]).unwrap();
    let m = SolidModel::new(hex);
    let r = SolidResult::new(m.dof_map(), vec![0.0; m.dof_map().n_equations()]).unwrap();
    let e = shell_fields(&m, &r).unwrap_err().to_string();
    println!("N2 hex: {e}");
    assert!(e.contains("no Shell4 block"), "{e}");
    let mut mesh = SolidMesh::new(vec![[0.0; 3], [1.0, 0.0, 0.0]]).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1]).unwrap();
    let m = SolidModel::new(mesh);
    let r = SolidResult::new(m.dof_map(), vec![0.0; m.dof_map().n_equations()]).unwrap();
    let e = shell_fields(&m, &r).unwrap_err().to_string();
    println!("N2 beam: {e}");
    assert!(e.contains("no Shell4 block"), "{e}");
}

#[test]
#[ignore = "record of the N6 set-up of ADR 0437 as registered (9 nodes with 27 unknowns is refused by the length test, not the node-count test); superseded by n3_to_n7_layout_refusals_amended (amendment 1)"]
fn n3_to_n7_layout_refusals() {
    let m = strip_model(false, true);
    let e = refusal(&m, &SolidResult::new(DofMap::uniform(11, 3), vec![0.0; 33]).unwrap());
    println!("N3: {e}");
    assert!(e.contains("length 33") && e.contains("expected 63"), "{e}");
    assert!(!e.contains("unknowns in the result and"), "{e}");
    let e = refusal(&m, &result_with(&[6, 6, 6, 6, 3, 6, 6, 6, 6, 6, 6]));
    println!("N4: {e}");
    assert!(
        e.contains("node 4 has 3 unknowns in the result and 6 in the model") && e.contains("displacement"),
        "{e}"
    );
    let e = refusal(&m, &result_with(&[3, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6]));
    println!("N5: {e}");
    assert!(
        e.contains("node 0 has 3 unknowns in the result and 6 in the model"),
        "{e}"
    );
    let e = refusal(&m, &SolidResult::new(DofMap::uniform(9, 3), vec![0.0; 27]).unwrap());
    println!("N6: {e}");
    assert!(e.contains("the result numbers 9 nodes and the model 11"), "{e}");
    let own = result_with(&[6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 3]);
    assert!(solid_vtu_string(&m, &own).is_ok());
    assert!(shell_fields(&m, &own).is_ok());
}

#[test]
fn n3_to_n7_layout_refusals_amended() {
    let m = strip_model(false, true);
    let e = refusal(&m, &SolidResult::new(DofMap::uniform(11, 3), vec![0.0; 33]).unwrap());
    println!("N3: {e}");
    assert!(e.contains("length 33") && e.contains("expected 63"), "{e}");
    assert!(!e.contains("unknowns in the result and"), "{e}");
    let e = refusal(&m, &result_with(&[6, 6, 6, 6, 3, 6, 6, 6, 6, 6, 6]));
    println!("N4: {e}");
    assert!(
        e.contains("node 4 has 3 unknowns in the result and 6 in the model") && e.contains("displacement"),
        "{e}"
    );
    let e = refusal(&m, &result_with(&[3, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6]));
    println!("N5: {e}");
    assert!(
        e.contains("node 0 has 3 unknowns in the result and 6 in the model"),
        "{e}"
    );
    // N6b (amendment 1): 12 nodes and 63 unknowns, so that the length test passes and the node count decides.
    let e = refusal(&m, &result_with(&[6, 6, 6, 6, 6, 6, 6, 6, 6, 3, 3, 3]));
    println!("N6b: {e}");
    assert!(e.contains("the result numbers 12 nodes and the model 11"), "{e}");
    let own = result_with(&[6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 3]);
    assert!(solid_vtu_string(&m, &own).is_ok());
    assert!(shell_fields(&m, &own).is_ok());
}

#[test]
fn n8_shell_block_without_thickness_refused() {
    let m = strip_model(false, false);
    let r = SolidResult::new(m.dof_map(), vec![0.0; m.dof_map().n_equations()]).unwrap();
    let e = refusal(&m, &r);
    println!("N8: {e}");
    assert!(e.contains("Shell property"), "{e}");
}

// ----------------------------------------------------------------------------------------------------- R1 (ADR 0437 D4)

/// The fixture of `si_shell_output.rs` (ADR 0432 check N2), copied.
fn old_strip() -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..2 {
        for i in 0..5 {
            nodes.push([0.25 * i as f64, 0.1 * j as f64, 0.0]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let mut conn = Vec::new();
    for i in 0..4 {
        conn.extend_from_slice(&[i, i + 1, i + 6, i + 5]);
    }
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..4).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    m
}

/// R1: the running copy of the retired `n2_shell_result_refused_by_the_solid_output`: the same model and result are now written.
#[test]
fn n2_shell_result_refused_by_the_solid_output_after_landed() {
    let m = old_strip();
    let mut pres = Vec::new();
    for k in [0, 5] {
        for c in 0..6 {
            pres.push(Prescribed::new(k, c, 0.0));
        }
    }
    let r = solve_static_with(
        &m,
        &pres,
        &[NodalForceAt::new(4, [0.0, 0.0, 1.0])],
        &StaticOptions::default(),
    )
    .unwrap();
    let xml = solid_vtu_string(&m, &r).expect("a shell result is written since ADR 0437");
    let rd = parse(&xml);
    assert_eq!(rd.types, vec![9; 4]);
    let (d, w, _) = expected_points(&r, 10);
    assert!(read_error(&rd.point("displacement"), &d).0 <= 1e-15);
    assert!(read_error(&rd.point("rotation"), &w).0 <= 1e-15);
    let want = expected_shell(&m, &r, 4);
    for (k, (name, _)) in SHELL_CELL_NAMES.iter().enumerate() {
        assert!(read_error(&rd.cell(name), &want[k]).0 <= 1e-15, "{name}");
    }
    assert!(w[27..30].iter().any(|x| x.abs() > 0.0), "the tip rotates");
}
