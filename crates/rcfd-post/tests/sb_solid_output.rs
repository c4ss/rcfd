//! F-377 (ADR 0278): a solid result written as a finite-element `.vtu` by `rcfd_post::solid_output`. Checks P1, P2, B1,
//! K1 to K3, R1, E1 and the negative controls N2, N3 of the registration. The parser is the test's own.

use rcfd_core::vec3::Vec3;
use rcfd_core::Error;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::recover::{nodal_average, principal, tensor_norm, von_mises, IntegrationPointField};
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{Material, SolidModel, SolidResult};
use rcfd_post::solid_output::{solid_fields, solid_vtu_string, write_solid_vtu};
use std::collections::BTreeSet;

const KINDS: [ElementKind; 3] = [ElementKind::Hex8, ElementKind::Tet4, ElementKind::Tet10];
const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const LENGTHS: Vec3 = [1.2, 1.0, 0.8];
const DIV: [usize; 3] = [2, 2, 2];
/// The uniform strain (tensor, symmetric): `u_i = sum_j EPS[i][j] x_j`.
const EPS: [[f64; 3]; 3] = [
    [1.0e-3, 0.3e-3, 0.4e-3],
    [0.3e-3, -0.5e-3, -0.2e-3],
    [0.4e-3, -0.2e-3, 0.8e-3],
];

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

/// (components, values) of the named array of a section.
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
    points: Vec<Vec3>,
    cells: Vec<Vec<usize>>,
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
    let n_points: usize = attr(&xml[ps..pe], "NumberOfPoints").parse().unwrap();
    let n_cells: usize = attr(&xml[ps..pe], "NumberOfCells").parse().unwrap();
    let pts = f64s(&array(section(xml, "Points"), "Points").1);
    assert_eq!(pts.len(), 3 * n_points, "points");
    let cells_sec = section(xml, "Cells");
    let conn = usizes(&array(cells_sec, "connectivity").1);
    let offs = usizes(&array(cells_sec, "offsets").1);
    let types = usizes(&array(cells_sec, "types").1);
    assert_eq!(offs.len(), n_cells, "offsets count");
    assert_eq!(types.len(), n_cells, "types count");
    assert_eq!(*offs.last().unwrap(), conn.len(), "last offset");
    let mut cells = Vec::with_capacity(n_cells);
    let mut prev = 0;
    for &o in &offs {
        assert!(o > prev, "offsets increase");
        cells.push(conn[prev..o].to_vec());
        prev = o;
    }
    assert!(conn.iter().all(|&i| i < n_points), "indices in range");
    Read {
        n_points,
        n_cells,
        points: pts.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
        cells,
        types,
        point_names: names(section(xml, "PointData")),
        cell_names: names(section(xml, "CellData")),
        xml: xml.to_string(),
    }
}

// -------------------------------------------------------------------------------------------------------------- the boxes

fn mesh_of(kind: ElementKind) -> SolidMesh {
    match kind {
        ElementKind::Hex8 => hex_box(ORIGIN, LENGTHS, DIV).unwrap(),
        ElementKind::Tet4 => tet_box(ORIGIN, LENGTHS, DIV).unwrap(),
        _ => upgrade_to_tet10(&tet_box(ORIGIN, LENGTHS, DIV).unwrap()).unwrap().0,
    }
}

fn strain_field(eps: &[[f64; 3]; 3], x: Vec3) -> Vec3 {
    let mut u = [0.0; 3];
    for i in 0..3 {
        for j in 0..3 {
            u[i] += eps[i][j] * x[j];
        }
    }
    u
}

fn boundary(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

fn case(kind: ElementKind) -> (SolidModel, SolidResult, usize) {
    let mesh = mesh_of(kind);
    let b = boundary(&mesh);
    let mut pres = Vec::new();
    for &n in &b {
        let u = strain_field(&EPS, mesh.nodes()[n]);
        for (k, &uk) in u.iter().enumerate() {
            pres.push(Prescribed::new(n, k, uk));
        }
    }
    let interior = mesh.nodes().len() - b.len();
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_MOD, NU, 7850.0, 0.0).unwrap())
        .unwrap();
    let res = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap();
    (model, res, interior)
}

fn bits_equal(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn vtk_type(kind: ElementKind) -> usize {
    match kind {
        ElementKind::Tet4 => 10,
        ElementKind::Hex8 => 12,
        _ => 24,
    }
}

// ----------------------------------------------------------------------------------------------------------- P1, P2, B1

#[test]
fn p1_p2_b1_arrays_bits_counts_and_uniform_strain() {
    let expected_points: [(&str, usize); 8] = [
        ("displacement", 3),
        ("stress", 6),
        ("von_mises", 1),
        ("principal_stress", 3),
        ("principal_direction_1", 3),
        ("principal_direction_2", 3),
        ("principal_direction_3", 3),
        ("reaction", 3),
    ];
    for kind in KINDS {
        let (model, res, interior) = case(kind);
        let mesh = model.mesh();
        let xml = solid_vtu_string(&model, &res).unwrap();
        let r = parse(&xml);
        let f = solid_fields(&model, &res).unwrap();
        // P1: names, components, bits of the file against solid_fields.
        let names_ok = expected_points
            .iter()
            .all(|(n, c)| r.point_names.iter().any(|(m, d)| m == n && d == c))
            && r.point_names.len() == expected_points.len()
            && r.cell_names == vec![("stress".to_string(), 6)];
        let mut dirs = [Vec::new(), Vec::new(), Vec::new()];
        for node in 0..r.n_points {
            for (k, d) in dirs.iter_mut().enumerate() {
                d.extend_from_slice(&f.principal_directions[9 * node + 3 * k..9 * node + 3 * k + 3]);
            }
        }
        let reaction = f.reaction.clone().expect("the static solve returns reactions");
        let file_bits = [
            ("displacement", bits_equal(&r.point("displacement"), &f.displacement)),
            ("stress", bits_equal(&r.point("stress"), &f.stress)),
            ("von_mises", bits_equal(&r.point("von_mises"), &f.von_mises)),
            (
                "principal_stress",
                bits_equal(&r.point("principal_stress"), &f.principal_stress),
            ),
            (
                "principal_direction_1",
                bits_equal(&r.point("principal_direction_1"), &dirs[0]),
            ),
            (
                "principal_direction_2",
                bits_equal(&r.point("principal_direction_2"), &dirs[1]),
            ),
            (
                "principal_direction_3",
                bits_equal(&r.point("principal_direction_3"), &dirs[2]),
            ),
            ("reaction", bits_equal(&r.point("reaction"), &reaction)),
            ("cell stress", bits_equal(&r.cell("stress"), &f.element_stress)),
        ];
        // P1: solid_fields against an assembly from the library calls.
        let n = mesh.nodes().len();
        let disp: Vec<f64> = (0..n).flat_map(|v| res.displacement(v).to_vec()).collect();
        let reac: Vec<f64> = (0..n)
            .flat_map(|v| {
                let s = res.dofs().equation(v, 0).unwrap();
                res.reactions().unwrap()[s..s + 3].to_vec()
            })
            .collect();
        let field = IntegrationPointField::from_result(&model, &res).unwrap();
        let nodal: Vec<[f64; 6]> = nodal_average(&model, &field, field.stress(), None)
            .unwrap()
            .into_iter()
            .map(|s| s.unwrap())
            .collect();
        let st: Vec<f64> = nodal.iter().flatten().copied().collect();
        let vm: Vec<f64> = nodal.iter().map(von_mises).collect();
        let ps: Vec<f64> = nodal.iter().flat_map(|s| principal(s).values).collect();
        let pd: Vec<f64> = nodal
            .iter()
            .flat_map(|s| principal(s).directions.into_iter().flatten())
            .collect();
        let es: Vec<f64> = field
            .element_means(field.stress())
            .unwrap()
            .into_iter()
            .flatten()
            .collect();
        let lib_bits = bits_equal(&f.displacement, &disp)
            && bits_equal(&reaction, &reac)
            && bits_equal(&f.stress, &st)
            && bits_equal(&f.von_mises, &vm)
            && bits_equal(&f.principal_stress, &ps)
            && bits_equal(&f.principal_directions, &pd)
            && bits_equal(&f.element_stress, &es);
        // P2: counts, types, connectivity.
        let conn: Vec<Vec<usize>> = (0..mesh.n_elements()).map(|e| mesh.element_nodes(e).to_vec()).collect();
        let pts: Vec<f64> = mesh.nodes().iter().flatten().copied().collect();
        let read_pts: Vec<f64> = r.points.iter().flatten().copied().collect();
        let p2 = r.n_points == n
            && r.n_cells == mesh.n_elements()
            && r.types.iter().all(|&t| t == vtk_type(kind))
            && r.cells == conn
            && bits_equal(&read_pts, &pts);
        // B1: displacement eps x from the read-back coordinates; N3: a non-symmetric eps.
        let u = r.point("displacement");
        let mut eps_n3 = EPS;
        eps_n3[0][1] += 1e-4;
        let (mut b1, mut n3, mut umax) = (0.0f64, 0.0f64, 0.0f64);
        for (v, x) in r.points.iter().enumerate() {
            let ue = strain_field(&EPS, *x);
            let un = strain_field(&eps_n3, *x);
            for k in 0..3 {
                b1 = b1.max((u[3 * v + k] - ue[k]).abs());
                n3 = n3.max((u[3 * v + k] - un[k]).abs());
                umax = umax.max(ue[k].abs());
            }
        }
        println!(
            "P1/P2/B1 {kind:?}: nodes {n} (not on the faces {interior}), cells {}; names {names_ok}; file bits {file_bits:?}; \
             library bits {lib_bits}; P2 {p2}; B1 max |u - eps x| {b1:.3e} (limit 1e-15), relative {:.3e}, max |u| {umax:.3e}; \
             N3 {n3:.3e} (> 1e-15)",
            r.n_cells,
            b1 / umax
        );
        assert!(names_ok, "P1 names {kind:?}: {:?} {:?}", r.point_names, r.cell_names);
        assert!(file_bits.iter().all(|c| c.1), "P1 file bits {kind:?}");
        assert!(lib_bits, "P1 library bits {kind:?}");
        assert!(p2, "P2 {kind:?}");
        assert!(b1 <= 1e-15, "B1 {kind:?}: {b1:e}");
        assert!(n3 > 1e-15, "N3 {kind:?}: {n3:e}");
    }
}

// ------------------------------------------------------------------------------------------------------------- K1 to K3

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn len(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

/// The mid-edge pairs of VTK_QUADRATIC_TETRA, from the vtkQuadraticTetra class documentation (ADR 0278, read 3).
const VTK_TET10_PAIRS: [[usize; 2]; 6] = [[0, 1], [1, 2], [2, 0], [0, 3], [1, 3], [2, 3]];
/// The edges of a VTK hexahedron, from the vtkQuadraticHexahedron class documentation (ADR 0278, read 3).
const VTK_HEX_EDGES: [[usize; 2]; 12] = [
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    [4, 5],
    [5, 6],
    [6, 7],
    [7, 4],
    [0, 4],
    [1, 5],
    [2, 6],
    [3, 7],
];

fn tet_volume6(x: &[Vec3]) -> f64 {
    dot(cross(sub(x[1], x[0]), sub(x[2], x[0])), sub(x[3], x[0]))
}

fn midpoint_error(x: &[Vec3]) -> f64 {
    VTK_TET10_PAIRS
        .iter()
        .enumerate()
        .map(|(k, [a, b])| {
            let m = [
                0.5 * (x[*a][0] + x[*b][0]),
                0.5 * (x[*a][1] + x[*b][1]),
                0.5 * (x[*a][2] + x[*b][2]),
            ];
            len(sub(x[4 + k], m))
        })
        .fold(0.0, f64::max)
}

#[test]
fn k1_k2_k3_node_orders_from_the_vtk_documentation_and_n2() {
    let lmax = LENGTHS.iter().copied().fold(0.0, f64::max);
    let h = [LENGTHS[0] / 2.0, LENGTHS[1] / 2.0, LENGTHS[2] / 2.0];
    for kind in KINDS {
        let (model, res, _) = case(kind);
        let r = parse(&solid_vtu_string(&model, &res).unwrap());
        let coords = |c: &[usize]| -> Vec<Vec3> { c.iter().map(|&i| r.points[i]).collect() };
        match kind {
            ElementKind::Hex8 => {
                let (mut edge_err, mut vert_err, mut min_dot) = (0.0f64, 0.0f64, f64::INFINITY);
                for c in &r.cells {
                    let x = coords(c);
                    for [a, b] in VTK_HEX_EDGES {
                        let l = len(sub(x[b], x[a]));
                        let e = h.iter().map(|hk| (l - hk).abs()).fold(f64::INFINITY, f64::min);
                        edge_err = edge_err.max(e / lmax);
                    }
                    let v0 = sub(x[4], x[0]);
                    for k in 1..4 {
                        vert_err = vert_err.max(len(sub(sub(x[4 + k], x[k]), v0)) / lmax);
                    }
                    let nb = [
                        cross(sub(x[1], x[0]), sub(x[2], x[0]))[0] + cross(sub(x[2], x[0]), sub(x[3], x[0]))[0],
                        cross(sub(x[1], x[0]), sub(x[2], x[0]))[1] + cross(sub(x[2], x[0]), sub(x[3], x[0]))[1],
                        cross(sub(x[1], x[0]), sub(x[2], x[0]))[2] + cross(sub(x[2], x[0]), sub(x[3], x[0]))[2],
                    ];
                    let cb: Vec3 = (0..3)
                        .map(|i| (x[0][i] + x[1][i] + x[2][i] + x[3][i]) / 4.0)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                    let ct: Vec3 = (0..3)
                        .map(|i| (x[4][i] + x[5][i] + x[6][i] + x[7][i]) / 4.0)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                    min_dot = min_dot.min(dot(nb, sub(ct, cb)));
                }
                println!(
                    "K3 Hex8: {} cells; edge length error {edge_err:.3e}, vertical edges {vert_err:.3e} (limit 1e-14 L); \
                     min base normal . (top - base) {min_dot:.3e} (> 0)",
                    r.n_cells
                );
                assert!(edge_err <= 1e-14 && vert_err <= 1e-14 && min_dot > 0.0, "K3");
            }
            _ => {
                let min_vol = r
                    .cells
                    .iter()
                    .map(|c| tet_volume6(&coords(&c[..4])))
                    .fold(f64::INFINITY, f64::min);
                println!(
                    "K1 {kind:?}: {} cells, min 6 V of the corners {min_vol:.3e} (> 0)",
                    r.n_cells
                );
                assert!(min_vol > 0.0, "K1 {kind:?}");
                if kind == ElementKind::Tet10 {
                    let k2 = r.cells.iter().map(|c| midpoint_error(&coords(c))).fold(0.0, f64::max) / lmax;
                    // N2: positions 5 and 6 swapped in every cell must fail.
                    let n2 = r
                        .cells
                        .iter()
                        .map(|c| {
                            let mut s = c.clone();
                            s.swap(5, 6);
                            midpoint_error(&coords(&s)) / lmax
                        })
                        .fold(f64::INFINITY, f64::min);
                    println!("K2 Tet10: midpoint error {k2:.3e} (limit 1e-15); N2 swapped 5, 6: min error {n2:.3e} (> 1e-15)");
                    assert!(k2 <= 1e-15, "K2: {k2:e}");
                    assert!(n2 > 1e-15, "N2: {n2:e}");
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------ R1, E1

#[test]
fn r1_a_result_of_another_mesh_is_refused_by_name() {
    let (_, hex_res, _) = case(ElementKind::Hex8);
    let (tet10_model, _, _) = case(ElementKind::Tet10);
    match solid_vtu_string(&tet10_model, &hex_res) {
        Err(Error::InvalidValue { path, reason }) => {
            println!("R1: path '{path}', reason: {reason}");
            assert_eq!(path, "displacement", "R1");
        }
        other => panic!("R1: expected a refusal, got {:?}", other.map(|s| s.len())),
    }
}

#[test]
fn e1_stresses_read_back_equal_d_eps_and_the_file_is_written() {
    let lambda = E_MOD * NU / ((1.0 + NU) * (1.0 - 2.0 * NU));
    let mu = E_MOD / (2.0 * (1.0 + NU));
    let tr = EPS[0][0] + EPS[1][1] + EPS[2][2];
    let d_eps = [
        lambda * tr + 2.0 * mu * EPS[0][0],
        lambda * tr + 2.0 * mu * EPS[1][1],
        lambda * tr + 2.0 * mu * EPS[2][2],
        2.0 * mu * EPS[0][1],
        2.0 * mu * EPS[1][2],
        2.0 * mu * EPS[2][0],
    ];
    let scale = tensor_norm(&d_eps);
    let vm_exact = von_mises(&d_eps);
    let dir = std::env::temp_dir().join(format!("rcfd_sb_solid_output_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for kind in KINDS {
        let (model, res, _) = case(kind);
        let path = dir.join(format!("{kind:?}.vtu"));
        write_solid_vtu(&path, &model, &res).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, solid_vtu_string(&model, &res).unwrap(), "file equals the string");
        let r = parse(&text);
        let vm = r.point("von_mises");
        let es = r.cell("stress");
        let e_vm = vm.iter().map(|v| (v - vm_exact).abs()).fold(0.0, f64::max) / scale;
        let e_es = es
            .chunks(6)
            .flat_map(|s| s.iter().zip(&d_eps).map(|(a, b)| (a - b).abs()))
            .fold(0.0, f64::max)
            / scale;
        println!("E1 {kind:?}: von Mises {e_vm:.3e}, element stress {e_es:.3e} (limit 1e-10, relative to |D eps| {scale:.4e})");
        assert!(e_vm <= 1e-10 && e_es <= 1e-10, "E1 {kind:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
