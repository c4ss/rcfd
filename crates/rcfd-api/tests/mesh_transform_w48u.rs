//! ADR 0155 (job 48U): `[mesh.transform]` through the case: spellings and refusals (K1-K3), derived geometry, topology,
//! exact angles and invariants of the rotated meshes (U3-U6), periodic patches (P1), the MT101 warnings (W1) and the
//! geometric lift-direction check (E3). Only the case-level API is used (`Case`, `build_mesh`, the mesh, `FvGeometry`,
//! the quality report, the wall distance), so the same file ran on the base code, where every test that sets the table
//! failed with "unknown setting 'transform'" (R0, the control that the key did not exist).

use rcfd_api::case::Case;
use rcfd_api::sim::build_mesh;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::FvGeometry;
use rcfd_mesh::Mesh;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// The committed aerofoil mesh (ADR 0142 D3), as a `[mesh]` table naming it by its absolute path.
fn aerofoil_mesh() -> String {
    let p = repo()
        .join("example-projects")
        .join("wind-tunnel")
        .join("meshes")
        .join("naca0012-ogrid")
        .canonicalize()
        .unwrap();
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.strip_prefix("//?/").unwrap_or(&s).to_string();
    format!("[mesh]\nsource = \"file\"\nfile = \"{s}\"\n")
}

/// "The meshes" of ADR 0155's checks: (name, `[mesh]` table, an origin allowed for it, generator mesh).
fn meshes() -> Vec<(&'static str, String, Vec3, bool)> {
    vec![
        (
            "cavity_2d",
            "[mesh]\ngenerator = \"cavity_2d\"\nnx = 8\nny = 8\n".into(),
            [0.25, -0.5, 0.0],
            true,
        ),
        (
            "cylinder_ogrid",
            "[mesh]\ngenerator = \"cylinder_ogrid\"\nnx = 32\nny = 12\nr_in = 0.5\nr_out = 5.0\ngrading = 10.0\n".into(),
            [0.25, -0.5, 0.0],
            true,
        ),
        (
            "annulus_3d_sector",
            "[mesh]\ngenerator = \"annulus_3d_sector\"\nnx = 8\nny = 4\nnz = 3\nr_in = 0.5\nr_out = 1.0\nlz = 0.3\nsector_angle = 40.0\n"
                .into(),
            [0.0, 0.0, 0.7],
            true,
        ),
        (
            "box_2d periodic_x",
            "[mesh]\ngenerator = \"box_2d\"\nnx = 6\nny = 4\nlx = 2.0\nly = 1.0\nperiodic_x = true\n".into(),
            [0.25, -0.5, 0.0],
            true,
        ),
        ("aerofoil", aerofoil_mesh(), [0.25, -0.5, 0.0], false),
    ]
}

const ANGLES: [f64; 7] = [-4.0, 2.0, 30.0, 90.0, -90.0, 180.0, 123.456];

fn table(deg: f64, o: Vec3) -> String {
    format!(
        "\n[mesh.transform]\nrotate_z_deg = {deg:?}\nrotate_origin = [{:?}, {:?}, {:?}]\n",
        o[0], o[1], o[2]
    )
}

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|d| panic!("{d:?}\n{text}"))
}

fn mesh(text: &str) -> Mesh {
    build_mesh(&case(text)).unwrap_or_else(|d| panic!("{d:?}\n{text}"))
}

/// The rotation of ADR 0155 D1 written out here (independently of the implementation): about z through `o`.
fn rot(deg: f64, o: Vec3, p: Vec3) -> Vec3 {
    let (s, c) = deg.to_radians().sin_cos();
    let (dx, dy) = (p[0] - o[0], p[1] - o[1]);
    [o[0] + c * dx - s * dy, o[1] + s * dx + c * dy, p[2]]
}

fn rot_vec(deg: f64, v: Vec3) -> Vec3 {
    rot(deg, [0.0; 3], v)
}

fn scale(p: Vec3) -> f64 {
    vec3::norm(p).max(1.0)
}

// ------------------------------------------------------------------------------------------------ K1-K3

/// K1: the table, dotted keys, an inline table, JSON and `Case::set` give equal settings data and equal meshes.
#[test]
fn k1_spellings_give_equal_settings_and_meshes() {
    let base = aerofoil_mesh();
    let spellings = [
        (
            "table",
            format!("{base}\n[mesh.transform]\nrotate_z_deg = -4.0\nrotate_origin = [0.25, 0.0, 0.0]\n"),
        ),
        (
            "dotted keys",
            format!("{base}transform.rotate_z_deg = -4.0\ntransform.rotate_origin = [0.25, 0.0, 0.0]\n"),
        ),
        (
            "inline table",
            format!("{base}transform = {{ rotate_z_deg = -4.0, rotate_origin = [0.25, 0.0, 0.0] }}\n"),
        ),
    ];
    let first = case(&spellings[0].1);
    let want = first.tree.to_json();
    let hash = build_mesh(&first).unwrap().content_hash();
    for (what, text) in &spellings {
        let c = case(text);
        assert_eq!(c.tree.to_json(), want, "{what}");
        assert_eq!(build_mesh(&c).unwrap().content_hash(), hash, "{what}");
    }
    // JSON: the same settings as a JSON document
    let mut doc: Value = case(&base).tree.to_json();
    doc["mesh"]["transform"] = json!({"rotate_z_deg": -4.0, "rotate_origin": [0.25, 0.0, 0.0]});
    let j = Case::parse_json(&doc.to_string()).unwrap_or_else(|d| panic!("{d:?}"));
    assert_eq!(j.tree.to_json(), want, "json");
    assert_eq!(build_mesh(&j).unwrap().content_hash(), hash, "json");
    // the `--set` path
    let mut s = case(&base);
    s.set("mesh.transform.rotate_z_deg", json!(-4.0)).unwrap();
    s.set("mesh.transform.rotate_origin", json!([0.25, 0.0, 0.0])).unwrap();
    assert_eq!(s.tree.to_json(), want, "Case::set");
    assert_eq!(build_mesh(&s).unwrap().content_hash(), hash, "Case::set");
    // the JSON and TOML writers round trip the table
    for text in [first.to_toml(), first.to_json_string()] {
        let back = if text.starts_with('{') {
            Case::parse_json(&text)
        } else {
            Case::parse_toml(&text)
        }
        .unwrap();
        assert_eq!(back.tree.to_json(), want);
    }
    println!(
        "K1: 6 spellings equal, settings {:016x}, mesh {hash:016x}",
        first.tree.content_hash()
    );
}

/// K2: refusals with a diagnostic at their path; a misspelled table is an unknown setting as before.
#[test]
fn k2_bad_values_are_refused_at_their_path() {
    let base = "[mesh]\ngenerator = \"cavity_2d\"\nnx = 4\nny = 4\n";
    let cases = [
        (
            "[mesh.transform]\nrotate_x_deg = 3.0\n",
            "mesh.transform.rotate_x_deg",
            "unknown setting",
        ),
        (
            "[mesh.transfrom]\nrotate_z_deg = 3.0\n",
            "mesh.transfrom",
            "unknown setting",
        ),
        (
            "[mesh.transform]\nrotate_z_deg = \"-4 deg\"\n",
            "mesh.transform.rotate_z_deg",
            "degrees",
        ),
        (
            "[mesh.transform]\nrotate_origin = [0.25, 0.0]\n",
            "mesh.transform.rotate_origin",
            "3 components",
        ),
        (
            "[mesh.transform]\nrotate_z_deg = 400.0\n",
            "mesh.transform.rotate_z_deg",
            "outside",
        ),
    ];
    for (extra, path, words) in cases {
        let text = format!("{base}{extra}");
        let errs = Case::parse_toml(&text).expect_err(extra);
        println!("K2 {extra:?}: {errs:?}");
        assert!(
            errs.iter().any(|e| e.path == path && e.message.contains(words)),
            "{extra}: {errs:?}"
        );
    }
    // the same string refused through Case::set (the --set path, the service, the MCP tools)
    let mut c = case(base);
    let e = c.set("mesh.transform.rotate_z_deg", json!("-4 deg")).unwrap_err();
    assert!(e.to_string().contains("degrees"), "{e}");
    // a unit string for the origin is a length (D3): 25 cm is 0.25 m
    let o = case(&format!(
        "{base}[mesh.transform]\nrotate_z_deg = 1.0\nrotate_origin = [\"25 cm\", 0.0, 0.0]\n"
    ));
    assert_eq!(o.get("mesh.transform.rotate_origin").unwrap(), json!([0.25, 0.0, 0.0]));
}

/// K3: the JSON schema lists the optional table with its units; the defaults validate against it and have no table.
#[test]
fn k3_the_json_schema_lists_the_optional_table() {
    let js = rcfd_api::schema::case_schema().to_json_schema();
    let m = &js["properties"]["mesh"];
    let t = &m["properties"]["transform"];
    println!("K3 transform: {t}");
    assert_eq!(t["x-rcfd-optional"], json!(true));
    assert!(!m["required"].as_array().unwrap().contains(&json!("transform")));
    assert_eq!(t["properties"]["rotate_z_deg"]["x-rcfd-unit"], json!("deg"));
    assert_eq!(t["properties"]["rotate_z_deg"]["minimum"], json!(-360.0));
    assert_eq!(t["properties"]["rotate_z_deg"]["maximum"], json!(360.0));
    assert_eq!(t["properties"]["rotate_origin"]["x-rcfd-unit"], json!("m"));
    let d = Case::new();
    assert!(d.tree.to_json()["mesh"].get("transform").is_none());
    rcfd_core::settings::validate_json_schema(&js, &d.tree.to_json()).unwrap();
    // a case that sets the table validates too
    let c = case("[mesh.transform]\nrotate_z_deg = -4.0\n");
    rcfd_core::settings::validate_json_schema(&js, &c.tree.to_json()).unwrap();
    // an absent table reads as its defaults
    assert_eq!(d.get("mesh.transform.rotate_z_deg").unwrap(), json!(0.0));
    assert_eq!(d.get("mesh.transform.rotate_origin").unwrap(), json!([]));
}

// ------------------------------------------------------------------------------------------------ U3-U6

/// U3 and U4 on "the meshes", every angle: derived geometry within its limits, topology equal exactly.
#[test]
fn u3_u4_derived_geometry_and_topology_of_the_rotated_meshes() {
    let mut worst = [0.0f64; 5];
    for (name, text, o, generator) in meshes() {
        let m0 = mesh(&text);
        let tol = if generator { 1e-12 } else { 1e-9 };
        for a in ANGLES {
            let m = mesh(&format!("{text}{}", table(a, o)));
            // U4: topology
            let (t0, t) = (m0.topology(), m.topology());
            assert_eq!(t.face_offsets, t0.face_offsets, "{name} {a}");
            assert_eq!(t.face_points, t0.face_points, "{name} {a}");
            assert_eq!(t.owner, t0.owner, "{name} {a}");
            assert_eq!(t.neighbour, t0.neighbour, "{name} {a}");
            assert_eq!(t.patches, t0.patches, "{name} {a}");
            assert_eq!(t.cell_zones, t0.cell_zones, "{name} {a}");
            assert_eq!(t.face_zones, t0.face_zones, "{name} {a}");
            assert!(
                t.points
                    .iter()
                    .zip(&t0.points)
                    .all(|(p, q)| p[2].to_bits() == q[2].to_bits()),
                "{name} {a}: z changed"
            );
            // U3: volumes
            let (v0, v) = (m0.total_volume(), m.total_volume());
            let dv = (v - v0).abs() / v0;
            assert!(dv <= 1e-12, "{name} {a}: total volume {dv:e}");
            worst[0] = worst[0].max(dv);
            for (c, (x, y)) in m.cell_volumes().iter().zip(m0.cell_volumes()).enumerate() {
                let r = (x - y).abs() / y;
                worst[1] = worst[1].max(r);
                assert!(r <= tol, "{name} {a}: cell {c} volume {r:e}");
            }
            // faces: area and normal
            for f in 0..m.n_faces() {
                let (s0, s) = (m0.face_area_vectors()[f], m.face_area_vectors()[f]);
                let (a0, a1) = (vec3::norm(s0), vec3::norm(s));
                let ra = (a1 - a0).abs() / a0;
                let dn = vec3::norm(vec3::sub(vec3::unit(s), rot_vec(a, vec3::unit(s0))));
                worst[2] = worst[2].max(ra);
                worst[3] = worst[3].max(dn);
                assert!(
                    ra <= tol && dn <= tol,
                    "{name} {a}: face {f}: area {ra:e}, normal {dn:e}"
                );
            }
            // cell centres
            for (c, (x, y)) in m.cell_centres().iter().zip(m0.cell_centres()).enumerate() {
                let d = vec3::norm(vec3::sub(*x, rot(a, o, *y))) / scale(*y);
                worst[4] = worst[4].max(d);
                assert!(d <= 1e-12, "{name} {a}: centre {c} {d:e}");
            }
        }
        // control: the unrotated mesh against the rotation fails the centre check
        let off = m0
            .cell_centres()
            .iter()
            .map(|y| vec3::norm(vec3::sub(*y, rot(30.0, o, *y))) / scale(*y))
            .fold(0.0, f64::max);
        assert!(off > 1e-3, "{name}: control {off}");
        println!("U3 {name}: {} cells, control centre error {off:.3e}", m0.n_cells());
    }
    println!(
        "U3 worst: total volume {:.2e}, cell volume {:.2e}, face area {:.2e}, normal {:.2e}, centre {:.2e}",
        worst[0], worst[1], worst[2], worst[3], worst[4]
    );
}

/// U5: an angle that reduces to 0 is no transform (bit for bit); 90 degrees is exact; a tiny angle is not 0.
#[test]
fn u5_exact_angles() {
    for (name, text, o, _) in meshes() {
        let m0 = mesh(&text);
        for a in [0.0, -0.0, 360.0, -360.0] {
            let m = mesh(&format!("{text}{}", table(a, o)));
            assert_eq!(m.content_hash(), m0.content_hash(), "{name} {a}");
            assert!(
                m.points()
                    .iter()
                    .zip(m0.points())
                    .all(|(p, q)| (0..3).all(|k| p[k].to_bits() == q[k].to_bits())),
                "{name} {a}"
            );
            assert!(
                m.cell_volumes()
                    .iter()
                    .zip(m0.cell_volumes())
                    .all(|(x, y)| x.to_bits() == y.to_bits()),
                "{name} {a}"
            );
        }
        // about the origin (on the sector's periodic axis too): exact values (a signed zero may flip: 0 - 0 is +0)
        let m = mesh(&format!("{text}{}", table(90.0, [0.0; 3])));
        assert!(
            m.points()
                .iter()
                .zip(m0.points())
                .all(|(p, q)| p[0] == -q[1] && p[1] == q[0] && p[2].to_bits() == q[2].to_bits()),
            "{name}: 90 degrees not exact"
        );
        let tiny = mesh(&format!("{text}{}", table(1e-9, o)));
        assert_ne!(tiny.content_hash(), m0.content_hash(), "{name}: 1e-9 degrees");
        println!("U5 {name}: 0, -0, 360, -360 bit-identical; 90 exact; 1e-9 changes the hash");
    }
}

/// U6: wall distance and the quality report of the aerofoil mesh rotated by -4 about the quarter chord.
#[test]
fn u6_invariants_on_the_aerofoil_mesh() {
    let text = aerofoil_mesh();
    let m0 = mesh(&text);
    let m = mesh(&format!("{text}{}", table(-4.0, [0.25, 0.0, 0.0])));
    assert_eq!(m.n_cells(), 6144);
    let w0 = rcfd_turb::wall::wall_distance(&m0, &["airfoil"]).unwrap();
    let w = rcfd_turb::wall::wall_distance(&m, &["airfoil"]).unwrap();
    let dw = w.iter().zip(&w0).map(|(a, b)| (a - b).abs() / b).fold(0.0, f64::max);
    let (q0, q) = (rcfd_mesh::quality::check_mesh(&m0), rcfd_mesh::quality::check_mesh(&m));
    let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1e-300);
    let (dno, dsk, dar) = (
        rel(q.max_non_orthogonality, q0.max_non_orthogonality),
        rel(q.max_skewness, q0.max_skewness),
        rel(q.max_aspect_ratio, q0.max_aspect_ratio),
    );
    println!(
        "U6: wall distance {dw:.2e}; non-orthogonality {} vs {} ({dno:.2e}), skewness {dsk:.2e}, aspect {dar:.2e}",
        q.max_non_orthogonality, q0.max_non_orthogonality
    );
    assert!(dw <= 1e-12, "wall distance {dw:e}");
    assert!(dno <= 1e-9 && dsk <= 1e-9 && dar <= 1e-9);
    let names = |m: &Mesh| m.patches().iter().map(|p| (p.name.clone(), p.size)).collect::<Vec<_>>();
    assert_eq!(names(&m), names(&m0));
}

// ------------------------------------------------------------------------------------------------ P1

/// The partner face of every face of every periodic patch.
fn partners(g: &FvGeometry) -> Vec<Option<Vec<usize>>> {
    g.periodic_partner.clone()
}

/// P1: translational and on-axis rotational pairs stay matched; an off-axis rotation of a rotational sector is refused,
/// and without the refusal the pair would not match (control).
#[test]
fn p1_periodic_patches() {
    let all = meshes();
    let pick = |n: &str| all.iter().find(|m| m.0 == n).unwrap().1.clone();
    for (name, text, a, o) in [
        ("box_2d periodic_x", pick("box_2d periodic_x"), 30.0, [0.3, 0.2, 0.0]),
        ("annulus_3d_sector", pick("annulus_3d_sector"), 17.0, [0.0, 0.0, 0.7]),
    ] {
        let g0 = FvGeometry::new(Arc::new(mesh(&text))).unwrap();
        let g = FvGeometry::new(Arc::new(mesh(&format!("{text}{}", table(a, o))))).unwrap();
        assert_eq!(partners(&g), partners(&g0), "{name}");
        let n = partners(&g).iter().flatten().map(Vec::len).sum::<usize>();
        println!("P1 {name}: rotated {a} about {o:?}: {n} periodic faces matched as before");
        assert!(n > 0);
    }
    let sector = pick("annulus_3d_sector");
    let off = format!("{sector}{}", table(17.0, [1.0, 0.0, 0.0]));
    let d = build_mesh(&case(&off)).expect_err("refused");
    println!("P1 refusal: {d:?}");
    assert!(d
        .iter()
        .any(|x| x.path == "mesh.transform.rotate_origin" && x.message.contains("periodic")));
    // control: the same points rotation applied to the mesh alone (the stored periodic rotation left as it is)
    let m0 = mesh(&sector);
    let moved = rcfd_mesh::gen::warp(&m0, |p| rot(17.0, [1.0, 0.0, 0.0], p)).unwrap();
    let e = FvGeometry::new(Arc::new(moved)).expect_err("control");
    println!("P1 control: {e}");
    assert!(e.to_string().contains("not matched by the periodic transform"));
}

// ------------------------------------------------------------------------------------------------ W1

const W1_CASE: &str = r#"
[mesh]
generator = "cavity_2d"
nx = 8
ny = 8

[physics]
nu = 0.01

[initial]
velocity_expression_x = "x / 1 [s]"

[expressions.shape]
definition = "2 * y"

[boundaries.lid]
velocity = "moving_wall"
velocity_value = [1.0, 0.0, 0.0]
wall_rotation_rate = 0.5

[boundaries.walls]
velocity = "fixed"

[solver]
algorithm = "simplec"

[reports.probe1]
type = "probe"
field = "p"
point = [0.5, 0.5, 0.05]

[reports.turn]
type = "moment"
patch = "walls"

[sources.bed]
type = "porous"
zone = "box"
box_min = [0.2, 0.2, -1.0]
box_max = [0.4, 0.4, 1.0]
model = "darcy_forchheimer"
permeability = 0.01

[sources.spin]
type = "rotating_frame"
angular_velocity = 0.1
"#;

fn mt101(c: &Case) -> Vec<String> {
    let mut v: Vec<String> = rcfd_api::rules::check_case(c, None)
        .into_iter()
        .filter(|d| d.id == "MT101")
        .map(|d| d.path)
        .collect();
    v.sort();
    v
}

/// W1: one MT101 at each mesh-relative setting of a rotated case and none without a rotation; none for the rotated
/// example cases.
#[test]
fn w1_world_coordinate_settings_are_warned() {
    let rotated = case(&format!("{W1_CASE}{}", table(-4.0, [0.0; 3])));
    let got = mt101(&rotated);
    for d in rcfd_api::rules::check_case(&rotated, None)
        .iter()
        .filter(|d| d.id == "MT101")
    {
        println!("W1 {d}");
    }
    let mut want = vec![
        "boundaries.lid.wall_rotation_origin",
        "expressions.shape.definition",
        "initial.velocity_expression_x",
        "reports.probe1.point",
        "reports.turn.origin",
        "sources.bed.box_min",
        "sources.spin.rotation_origin",
    ];
    want.sort_unstable();
    assert_eq!(got, want);
    let rd = rcfd_api::rules::check_case(&rotated, None);
    assert!(rd
        .iter()
        .filter(|d| d.id == "MT101")
        .all(|d| d.severity == rcfd_api::rules::Severity::Warning && d.category == "mesh"));
    // controls: no table, and a zero angle
    assert!(mt101(&case(W1_CASE)).is_empty());
    assert!(mt101(&case(&format!("{W1_CASE}{}", table(0.0, [0.0; 3])))).is_empty());
    // the rotated example cases use no such setting
    for v in ["naca0012-aoa4-rotated", "naca0012-aoa2-rotated"] {
        let p = repo()
            .join("example-projects")
            .join("wind-tunnel")
            .join("cases")
            .join(format!("{v}.toml"));
        if let Ok(c) = Case::load(&p) {
            assert!(mt101(&c).is_empty(), "{v}: {:?}", mt101(&c));
            println!("W1 {v}: no MT101");
        }
    }
}

// ------------------------------------------------------------------------------------------------ E3 (geometric)

/// E3, geometric: rotated by -4 degrees about the quarter chord, the leading edge (the airfoil point at x = 0) moves up
/// and the trailing edge (x = 1) down: nose up into a flow along +x, a positive angle of attack. +4 flips both (control).
#[test]
fn e3_minus_alpha_turns_the_nose_up() {
    let text = aerofoil_mesh();
    let m0 = mesh(&text);
    let wall: Vec<usize> = {
        let p = m0.patch("airfoil").unwrap();
        let mut ids: Vec<usize> = p.faces().flat_map(|f| m0.face_points(f).to_vec()).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };
    let le = *wall
        .iter()
        .min_by(|a, b| m0.points()[**a][0].total_cmp(&m0.points()[**b][0]))
        .unwrap();
    let te = *wall
        .iter()
        .max_by(|a, b| m0.points()[**a][0].total_cmp(&m0.points()[**b][0]))
        .unwrap();
    for (deg, up) in [(-4.0, true), (4.0, false)] {
        let m = mesh(&format!("{text}{}", table(deg, [0.25, 0.0, 0.0])));
        let (yl, yt) = (m.points()[le][1], m.points()[te][1]);
        println!(
            "E3 {deg}: leading edge {:?} -> y {yl:.5}, trailing edge {:?} -> y {yt:.5}",
            m0.points()[le],
            m0.points()[te]
        );
        assert_eq!(yl > 0.0 && yt < 0.0, up, "{deg}");
        assert_eq!(yl < 0.0 && yt > 0.0, !up, "{deg}");
    }
}
