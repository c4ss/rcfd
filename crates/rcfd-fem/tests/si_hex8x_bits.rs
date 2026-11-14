//! F-390 (ADR 0493): check B1, the default Hex8 paths next to the new formulations give the bits they gave before F-390: the
//! distorted patch of F-374, the one-layer cantilever with the default property and with `BlockProperty::solid(Full)` (whose
//! digests must equal the default's), the nearly incompressible cylinder, a cantilever with a temperature load and its four
//! lowest frequencies, an orthotropic Hex8 stiffness, the geometric stiffness of a static pre-stress, the energy-norm estimate
//! and the Tet4 and Tet10 patch stiffnesses. Uses only API that existed at aa0e4d5b; the expected digests were printed by this
//! file run on the unchanged base aa0e4d5b (`f390-scratch/b1-base.log`) and written into `EXPECTED` before any library code of
//! F-390.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::estimate::{energy_error, PatchOptions};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::material::MaterialAxes;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidFormulation, SolidMesh, SolidModel, SolidResult};
use rcfd_linalg::eigen::ResidualTest;
use std::collections::BTreeSet;
use std::f64::consts::PI;

/// Digests of aa0e4d5b in the order of the run, from the base run (`f390-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 26]> = Some([
    ("patch stiffness", 0xbf88_b64e_cf56_23f8),
    ("patch solution", 0xbcd8_ab86_5996_a0aa),
    ("patch stress", 0xd26c_9634_c918_3c00),
    ("cantilever stiffness", 0xe701_003d_bc6c_7e51),
    ("cantilever loads", 0x5574_3bed_a04b_0826),
    ("cantilever solution", 0x99da_42ab_460f_54dd),
    ("cantilever stress", 0x7dfa_0976_dd25_fb56),
    ("cantilever nodal stress", 0xc06f_0455_2647_40b0),
    ("cantilever explicit Full stiffness", 0xe701_003d_bc6c_7e51),
    ("cantilever explicit Full loads", 0x5574_3bed_a04b_0826),
    ("cantilever explicit Full solution", 0x99da_42ab_460f_54dd),
    ("cantilever explicit Full stress", 0x7dfa_0976_dd25_fb56),
    ("cantilever explicit Full nodal stress", 0xc06f_0455_2647_40b0),
    ("cantilever geometric stiffness", 0x452b_9097_897f_c368),
    ("box patch recovery", 0x8b65_90ac_c545_e178),
    ("box energy error", 0xb9f2_e9ee_b375_6955),
    ("cylinder solution", 0x0bc0_2205_4377_8910),
    ("thermal stiffness", 0x0c8b_c922_3645_ab6d),
    ("thermal loads", 0x7a29_c6a5_b505_2319),
    ("thermal solution", 0x8bba_9d8e_eca8_3762),
    ("thermal stress", 0x7bb7_6a53_c21a_400b),
    ("thermal nodal stress", 0xfc2a_cdac_4907_36a0),
    ("box frequencies", 0xba28_f9ee_ea02_a066),
    ("orthotropic solution", 0x0791_221b_acf7_9174),
    ("Tet4 patch stiffness", 0x64dc_6737_f53e_bb31),
    ("Tet10 patch stiffness", 0x8129_4bc5_b654_70fe),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for x in v {
        for b in x.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    h
}

fn flat6(v: &[[f64; 6]]) -> Vec<f64> {
    v.iter().flatten().copied().collect()
}

fn flat_opt6(v: &[Option<[f64; 6]>]) -> Vec<f64> {
    v.iter().flat_map(|o| o.unwrap_or([f64::NAN; 6])).collect()
}

const E_MOD: f64 = 2.0e11;

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, 7850.0, 1.2e-5).unwrap()
}

fn rebuild(mesh: &SolidMesh, nodes: Vec<Vec3>) -> SolidMesh {
    let mut out = SolidMesh::new(nodes).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).unwrap().to_vec())
            .unwrap();
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    out
}

/// SplitMix64 from a seed (as `sb_static.rs`).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

fn boundary(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

/// The patch box of `sb_static.rs`, interior nodes moved by `0.1 h`.
fn patch_mesh(tet: bool) -> SolidMesh {
    let (o, l, div) = ([0.1, -0.2, 0.3], [1.2, 1.0, 0.8], [4, 3, 3]);
    let base = if tet {
        tet_box(o, l, div).unwrap()
    } else {
        hex_box(o, l, div).unwrap()
    };
    let b = boundary(&base);
    let mut rng = Rng(0xF374_0275 + 11);
    let h = [l[0] / 4.0, l[1] / 3.0, l[2] / 3.0];
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if !b.contains(&i) {
            for k in 0..3 {
                p[k] += 0.1 * h[k] * (2.0 * rng.next() - 1.0);
            }
        }
    }
    rebuild(&base, nodes)
}

fn linear_field(x: Vec3) -> Vec3 {
    let a = [
        [1.0e-3, 0.3e-3, -0.2e-3],
        [0.5e-3, -0.7e-3, 0.4e-3],
        [0.1e-3, 0.6e-3, 0.9e-3],
    ];
    let mut u = [1e-3, -2e-3, 0.5e-3];
    for i in 0..3 {
        for j in 0..3 {
            u[i] += a[i][j] * x[j];
        }
    }
    u
}

fn with_material(mesh: SolidMesh, m: Material) -> SolidModel {
    let mut model = SolidModel::new(mesh);
    model.add_material("all", m).unwrap();
    model
}

/// The one-layer cantilever of ADR 0493 reading (a4), optionally with an explicit property on its block.
fn cantilever(explicit_full: bool, nu: f64) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [2.0, 0.1, 0.1], [20, 1, 1]).unwrap();
    if explicit_full {
        mesh.set_block_property(0, BlockProperty::solid(SolidFormulation::Full))
            .unwrap();
    }
    let corner = mesh
        .node_set("xmin")
        .unwrap()
        .iter()
        .copied()
        .find(|&n| mesh.nodes()[n] == [0.0, 0.0, 0.0])
        .unwrap();
    mesh.add_node_set("root_corner", vec![corner]).unwrap();
    let mut m = with_material(mesh, steel(nu));
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("root_corner", 1, 0.0))
        .unwrap();
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1000.0 / 0.01])).unwrap();
    m
}

fn polar(p: Vec3) -> Vec3 {
    let r = 1.0 + ((2.0 * p[0]).exp() - 1.0) / (2f64.exp() - 1.0);
    let th = 0.5 * PI * p[1];
    [r * th.cos(), r * th.sin(), p[2]]
}

fn cylinder() -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 0.1], [8, 16, 1]).unwrap();
    let nodes = base.nodes().iter().map(|&p| polar(p)).collect();
    let mut m = with_material(rebuild(&base, nodes), steel(0.4999));
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_load(Load::pressure("xmin", 1.0e6)).unwrap();
    m
}

fn clamped_box(material: Material) -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.25, 0.25], [8, 2, 2]).unwrap();
    let mut m = with_material(mesh, material);
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn static_digests(got: &mut Vec<(String, u64)>, tag: &str, m: &SolidModel) -> SolidResult {
    got.push((format!("{tag} stiffness"), digest(&stiffness(m).unwrap().val)));
    got.push((format!("{tag} loads"), digest(&nodal_load_vector(m, &[]).unwrap())));
    let r = solve_static(m, &StaticOptions::default()).unwrap();
    got.push((format!("{tag} solution"), digest(r.displacement_values())));
    let field = IntegrationPointField::from_result(m, &r).unwrap();
    got.push((format!("{tag} stress"), digest(&flat6(field.stress()))));
    let avg = nodal_average(m, &field, field.stress(), None).unwrap();
    got.push((format!("{tag} nodal stress"), digest(&flat_opt6(&avg))));
    r
}

#[test]
fn b1_default_hex8_paths_give_their_former_bits() {
    let mut got: Vec<(String, u64)> = Vec::new();

    // The distorted patch of F-374.
    let pm = with_material(patch_mesh(false), steel(0.3));
    let b = boundary(pm.mesh());
    let mut pres = Vec::new();
    for &n in &b {
        let u = linear_field(pm.mesh().nodes()[n]);
        for (k, &uk) in u.iter().enumerate() {
            pres.push(Prescribed::new(n, k, uk));
        }
    }
    got.push(("patch stiffness".into(), digest(&stiffness(&pm).unwrap().val)));
    let r = solve_static_with(&pm, &pres, &[], &StaticOptions::default()).unwrap();
    got.push(("patch solution".into(), digest(r.displacement_values())));
    let field = IntegrationPointField::from_result(&pm, &r).unwrap();
    got.push(("patch stress".into(), digest(&flat6(field.stress()))));

    // The cantilever, default and explicit Full.
    let c = cantilever(false, 0.3);
    let rc = static_digests(&mut got, "cantilever", &c);
    static_digests(&mut got, "cantilever explicit Full", &cantilever(true, 0.3));
    let pre = Prestress::from_static(&c, &rc).unwrap();
    got.push((
        "cantilever geometric stiffness".into(),
        digest(&geometric_stiffness(&c, &pre, None).unwrap().val),
    ));
    let mut bx = clamped_box(steel(0.3));
    bx.add_load(Load::traction("xmax", [1.0e5, 2.0e5, -3.0e5])).unwrap();
    let rb = solve_static(&bx, &StaticOptions::default()).unwrap();
    let (rec, ee) = energy_error(&bx, rb.displacement_values(), &PatchOptions::new()).unwrap();
    got.push(("box patch recovery".into(), digest(&flat_opt6(rec.nodal()))));
    got.push(("box energy error".into(), digest(ee.element_errors())));

    // The nearly incompressible cylinder.
    let cy = cylinder();
    let r = solve_static(&cy, &StaticOptions::default()).unwrap();
    got.push(("cylinder solution".into(), digest(r.displacement_values())));

    // A temperature load and the modes.
    let mut t = clamped_box(steel(0.3));
    let temps: Vec<f64> = t
        .mesh()
        .nodes()
        .iter()
        .map(|x| 300.0 + 40.0 * x[0] - 25.0 * x[2])
        .collect();
    t.add_load(Load::temperature(temps, 293.0)).unwrap();
    static_digests(&mut got, "thermal", &t);
    let opts = ModalOptions::new(4)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward);
    let modal = modal_analysis(&clamped_box(steel(0.3)), &opts).unwrap();
    got.push(("box frequencies".into(), digest(&modal.frequencies)));

    // The point-material path of the assembly.
    let c30 = (0.5f64).cos();
    let s30 = (0.5f64).sin();
    let axes = MaterialAxes::Fixed([[c30, s30, 0.0], [-s30, c30, 0.0], [0.0, 0.0, 1.0]]);
    let ortho = Material::orthotropic(
        [1.4e11, 1.0e10, 1.0e10],
        [0.3, 0.3, 0.4],
        [5.0e9, 5.0e9, 3.5e9],
        1600.0,
        [0.0; 3],
        axes,
    )
    .unwrap();
    let mut ob = clamped_box(ortho);
    ob.add_load(Load::traction("xmax", [1.0e5, 2.0e5, -3.0e5])).unwrap();
    let ro = solve_static(&ob, &StaticOptions::default()).unwrap();
    got.push(("orthotropic solution".into(), digest(ro.displacement_values())));

    // The other solid kinds.
    let t4 = with_material(patch_mesh(true), steel(0.3));
    got.push(("Tet4 patch stiffness".into(), digest(&stiffness(&t4).unwrap().val)));
    let t10 = with_material(upgrade_to_tet10(&patch_mesh(true)).unwrap().0, steel(0.3));
    got.push(("Tet10 patch stiffness".into(), digest(&stiffness(&t10).unwrap().val)));

    for (name, d) in &got {
        println!(
            "(\"{name}\", 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            d >> 48,
            (d >> 32) & 0xffff,
            (d >> 16) & 0xffff,
            d & 0xffff
        );
    }
    // The explicit Full gives the default's bits.
    for part in ["stiffness", "loads", "solution", "stress", "nodal stress"] {
        let a = got.iter().find(|(n, _)| n == &format!("cantilever {part}")).unwrap().1;
        let b = got
            .iter()
            .find(|(n, _)| n == &format!("cantilever explicit Full {part}"))
            .unwrap()
            .1;
        assert_eq!(a, b, "explicit Full {part}");
    }
    let expected = EXPECTED.expect("the digests of the base run");
    let got_ref: Vec<(&str, u64)> = got.iter().map(|(n, d)| (n.as_str(), *d)).collect();
    assert_eq!(got_ref, expected.to_vec());
}
