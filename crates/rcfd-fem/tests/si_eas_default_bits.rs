//! F-596 (ADR 0608): the bit record B0 (D7). The BBar (F-bar, ADR 0506) and Full Hex8 paths of the nonlinear driver, the
//! element entry points and the linear Incompatible path (ADR 0493) give the bits they gave before F-596. Uses only API that
//! existed at c64046db; the expected digests were printed by this file run on the unchanged base c64046db
//! (`f596-scratch/b0-base.log`) and written into `EXPECTED` before any library code of F-596.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::ElementState;
use rcfd_fem::material::PointContext;
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions};
use rcfd_fem::nonlinear::{
    hex8_force_and_tangent_at, hex8_second_piola_kirchhoff, hex8_strains, initial_states, internal_force, solve,
    solve_in_time, tangent, NewtonOptions, NonlinearResult, TimeHistory, TimeSteppingOptions,
};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidFormulation, SolidMesh, SolidModel};

/// Digests of c64046db in the order of the run, from the base run (`f596-scratch/b0-base.log`); written after it.
const EXPECTED: Option<[(&str, u64); 11]> = Some([
    ("TL distorted box Full solve", 0xf560_b4aa_07ee_951a),
    ("TL distorted box BBar solve", 0x8581_b64b_c5d1_cabd),
    ("J2 bar Full solve", 0x1891_5029_20d9_0733),
    ("J2 bar BBar solve", 0x53b6_8c1f_046b_3e77),
    ("creep bar BBar solve_in_time", 0x909a_e332_14e9_c4b5),
    ("nl_transient bar Full", 0xb49a_2197_c23a_98b2),
    ("nl_transient bar BBar", 0x98d1_ec30_3e39_967d),
    ("J2 internal_force and tangent Full", 0x580e_9787_2607_3ff2),
    ("J2 internal_force and tangent BBar", 0x57ac_8cc8_43c4_1a40),
    ("hex8 entry points Full and BBar", 0x5421_9658_d402_ba65),
    (
        "Incompatible cantilever solve_static and stiffness",
        0xb6f2_8488_11b1_30ed,
    ),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    digest_bytes(v.iter().flat_map(|x| x.to_bits().to_le_bytes()))
}

fn digest_bytes<I: IntoIterator<Item = u8>>(bytes: I) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// SplitMix64.
struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(0xF596_0608u64.wrapping_add(tag))
    }
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    fn sym(&mut self, r: f64) -> f64 {
        r * (2.0 * self.uniform() - 1.0)
    }
}

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(2.0e11, nu, 7850.0, 0.0).unwrap()
}

fn j2() -> Material {
    Material::j2(steel(0.3), 2.0e8, 2.0e9, 1.0e9).unwrap()
}

fn creep() -> Material {
    let base = Material::isotropic_elastic(2.0e9, 0.3, 1200.0, 0.0).unwrap();
    Material::creep(base, 1.0 / (2.0e9 * 2.0e5f64.powi(3)), 4.0, 0.0).unwrap()
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

fn flat(r: &NonlinearResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap());
    for s in &r.steps {
        v.push(s.load_factor);
        v.push(s.step as f64);
        v.push(s.halvings as f64);
        v.extend_from_slice(&s.residuals);
        v.extend_from_slice(&s.energies);
    }
    for s in &r.states {
        v.extend_from_slice(s.values());
    }
    v
}

fn flat_history(h: &TimeHistory) -> Vec<f64> {
    let mut v = Vec::new();
    for s in &h.steps {
        v.push(s.time);
        v.extend_from_slice(&s.u);
        v.extend_from_slice(&s.internal_force);
        v.push(s.halvings as f64);
        v.push(s.substeps as f64);
        v.push(s.max_creep_increment);
        v.push(s.iterations as f64);
        for st in &s.states {
            v.extend_from_slice(st.values());
        }
    }
    v
}

fn stretch(m: &SolidModel, value: f64) -> Vec<Prescribed> {
    m.mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 0, value))
        .collect()
}

fn distorted_hex8(rng: &mut Rng) -> Vec<Vec3> {
    let unit: [Vec3; 8] = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    unit.iter()
        .map(|p| [p[0] + rng.sym(0.1), p[1] + rng.sym(0.1), p[2] + rng.sym(0.1)])
        .collect()
}

fn with(mut mesh: SolidMesh, f: SolidFormulation) -> SolidMesh {
    if f != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    }
    mesh
}

fn distorted_box(f: SolidFormulation, tag: u64) -> SolidMesh {
    let base = hex_box([0.0; 3], [2.0, 1.0, 1.0], [3, 2, 2]).unwrap();
    let mut rng = Rng::new(tag);
    let nodes = base
        .nodes()
        .iter()
        .map(|p| [p[0] + rng.sym(0.04), p[1] + rng.sym(0.04), p[2] + rng.sym(0.04)])
        .collect();
    with(rebuild(&base, nodes), f)
}

fn digests() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();
    let full = SolidFormulation::Full;
    let bbar = SolidFormulation::BBar;
    // ADR 0404 and ADR 0506: a distorted Hex8 box clamped on xmin, its xmax face moved along and across (SVK).
    for (name, f) in [
        ("TL distorted box Full solve", full),
        ("TL distorted box BBar solve", bbar),
    ] {
        let mut m = SolidModel::new(distorted_box(f, 1));
        m.add_material("all", steel(0.3)).unwrap();
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
        let mut pres = Vec::new();
        for &n in m.mesh().node_set("xmax").unwrap() {
            pres.push(Prescribed::new(n, 0, 0.2));
            pres.push(Prescribed::new(n, 1, 0.3));
        }
        let r = solve(&m, &pres, &[], &NewtonOptions::new(4).threads(4)).unwrap();
        out.push((name, digest(&flat(&r))));
    }
    // ADR 0475 and ADR 0506: a J2 Hex8 bar stretched past yield, Full and BBar.
    for (name, f) in [("J2 bar Full solve", full), ("J2 bar BBar solve", bbar)] {
        let mut m = SolidModel::new(with(hex_box([0.0; 3], [4.0, 1.0, 1.0], [4, 1, 1]).unwrap(), f));
        m.add_material("all", j2()).unwrap();
        m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
        let r = solve(&m, &stretch(&m, 1.6e-2), &[], &NewtonOptions::new(5).threads(4)).unwrap();
        out.push((name, digest(&flat(&r))));
    }
    // ADR 0489 and ADR 0506: a creep Hex8 bar under a held traction, BBar.
    {
        let mut m = SolidModel::new(with(hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap(), bbar));
        m.add_material("all", creep()).unwrap();
        m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
        m.add_load(Load::traction("xmax", [2.0e5, 0.0, 0.0])).unwrap();
        let opts = TimeSteppingOptions::new(5e-4).newton(NewtonOptions::new(1).threads(4));
        let h = solve_in_time(&m, &[], &[0.1, 0.3, 1.0], &opts).unwrap();
        out.push(("creep bar BBar solve_in_time", digest(&flat_history(&h))));
    }
    // nl_transient: an elastic Hex8 bar under a tip load history with its strain energy, Full and BBar.
    for (name, f) in [("nl_transient bar Full", full), ("nl_transient bar BBar", bbar)] {
        let mut m = SolidModel::new(with(hex_box([0.0; 3], [4.0, 1.0, 1.0], [4, 1, 1]).unwrap(), f));
        m.add_material("all", steel(0.3)).unwrap();
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
        let n = m.dof_map().n_equations();
        let mut pattern = vec![0.0; n];
        for &nd in m.mesh().node_set("xmax").unwrap() {
            pattern[m.dof_map().equation(nd, 1).unwrap()] = 1.0e8;
        }
        let loads = [LoadHistory::new(pattern, LoadTable::constant(1.0, 0.0, 1.0).unwrap())];
        let mut it = Integrator::new(&m, &loads, &NlTransientOptions::new(1e-4).threads(4)).unwrap();
        it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
        it.run(3).unwrap();
        let v = vec![it.strain_energy().unwrap(), it.kinetic_energy().unwrap()];
        let mut d = digest(&v);
        d ^= digest_bytes(it.state().unwrap().to_bytes());
        out.push((name, d));
    }
    // The assembled internal force and tangent of a distorted J2 Hex8 model, Full and BBar.
    for (name, f) in [
        ("J2 internal_force and tangent Full", full),
        ("J2 internal_force and tangent BBar", bbar),
    ] {
        let mut m = SolidModel::new(distorted_box(f, 7));
        m.add_material("all", j2()).unwrap();
        let n = m.dof_map().n_equations();
        let mut rng = Rng::new(8);
        let u: Vec<f64> = (0..n).map(|_| rng.sym(4e-3)).collect();
        let st = initial_states(&m).unwrap();
        let mut v = internal_force(&m, &u, &st).unwrap();
        v.extend_from_slice(&tangent(&m, &u, &st, Some(3)).unwrap().val);
        out.push((name, digest(&v)));
    }
    // The Hex8 entry points of ADR 0506 D5, Full and BBar, J2.
    {
        let mut rng = Rng::new(11);
        let x = distorted_hex8(&mut rng);
        let u: Vec<f64> = (0..24).map(|_| rng.sym(3e-3)).collect();
        let zero = ElementState::new(vec![0.0; 8 * rcfd_fem::mat_j2::STATE_SIZE]);
        let ctx = PointContext::default();
        let mut v = Vec::new();
        for f in [full, bbar] {
            let (fo, k) = hex8_force_and_tangent_at(f, &x, &j2(), &u, &zero, &ctx).unwrap();
            v.extend_from_slice(&fo);
            v.extend_from_slice(k.values());
            for s in hex8_second_piola_kirchhoff(f, &x, &j2(), &u, &zero).unwrap() {
                v.extend_from_slice(&s);
            }
            for s in hex8_strains(f, &x, &u).unwrap() {
                v.extend_from_slice(&s);
            }
        }
        out.push(("hex8 entry points Full and BBar", digest(&v)));
    }
    // ADR 0493: the linear Incompatible cantilever, solve_static and the assembled stiffness.
    {
        let inc = SolidFormulation::Incompatible;
        let mut m = SolidModel::new(with(hex_box([0.0; 3], [2.0, 0.1, 0.1], [20, 1, 1]).unwrap(), inc));
        m.add_material("all", steel(0.3)).unwrap();
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
        m.add_load(Load::traction("xmax", [0.0, 0.0, -1.0e5])).unwrap();
        let r = solve_static(&m, &StaticOptions::default()).unwrap();
        let mut v = r.displacement_values().to_vec();
        v.extend_from_slice(&stiffness(&m).unwrap().val);
        out.push(("Incompatible cantilever solve_static and stiffness", digest(&v)));
    }
    out
}

#[test]
fn b0_default_paths_keep_their_bits() {
    let got = digests();
    for (name, d) in &got {
        println!(
            "    (\"{name}\", 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            d >> 48,
            (d >> 32) & 0xffff,
            (d >> 16) & 0xffff,
            d & 0xffff
        );
    }
    let expected = EXPECTED.expect("the digests of the base run are written into EXPECTED");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, ed, "{name}: digest changed");
    }
}
