//! F-587 (ADR 0506): the bit record B0 (D8). The Full Hex8 paths of the nonlinear driver of ADR 0404 (SVK), ADR 0475 (J2),
//! ADR 0485 and ADR 0499 (viscoelastic, `solve_from_at`, `solve_in_time`, `nl_transient` with its strain energy) and ADR 0489
//! (creep, `solve_in_time`), the assembled force and tangent and the element entry points give the bits they gave before
//! F-587. Uses only API that existed at 3aec9cdb; the expected digests were printed by this file run on the unchanged base
//! 3aec9cdb (`f587-scratch/b0-base.log`) and written into `EXPECTED` before any library code of F-587.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::{element_for, ElementState};
use rcfd_fem::material::{PointContext, PronyTerm};
use rcfd_fem::nl_transient::{Integrator, NlTransientOptions};
use rcfd_fem::nonlinear::{
    element_force_and_tangent_at, element_second_piola_kirchhoff, element_trial_state, initial_states, internal_force,
    solve, solve_from_at, solve_in_time, tangent, NewtonOptions, NonlinearResult, TimeHistory, TimeSteppingOptions,
};
use rcfd_fem::transient::{LoadHistory, LoadTable};
use rcfd_fem::{Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};

/// Digests of 3aec9cdb in the order of the run, from the base run (`f587-scratch/b0-base.log`).
const EXPECTED: Option<[(&str, u64); 11]> = Some([
    ("TL Hex8 box solve", 0x0a4f_3942_e855_0771),
    ("J2 Hex8 bar solve", 0x1891_5029_20d9_0733),
    ("J2 cylinder 12x6 Full solve", 0xfc03_cb81_ac4a_0fa0),
    ("creep cylinder 12x6 Full solve_in_time", 0x2180_f7d7_7a95_ebaa),
    ("visco cube solve_in_time", 0x7988_dec7_d7e6_dbee),
    ("visco cube solve_from_at", 0x4ea2_9c46_31d1_fd8b),
    ("nl_transient elastic bar", 0xb49a_2197_c23a_98b2),
    ("nl_transient visco bar", 0x36e1_acac_d6b3_8bde),
    ("J2 Hex8 internal_force and tangent", 0x5fe8_572f_b38d_9f6b),
    ("J2 Hex8 element entry points", 0x394b_2ca0_3537_26b8),
    ("creep Hex8 element force and tangent", 0xdae9_00c3_6bb4_4526),
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
        Rng(0xF587_0506u64.wrapping_add(tag))
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

fn visco() -> Material {
    let terms = [(0.3, 0.1, 1e-3), (0.2, 0.0, 1e-1), (0.1, 0.05, 10.0)]
        .iter()
        .map(|&(g, k, t)| PronyTerm::new(g, k, t).unwrap())
        .collect();
    Material::viscoelastic(
        Material::isotropic_elastic(1.0e9, 0.3, 1200.0, 0.0).unwrap(),
        terms,
        None,
    )
    .unwrap()
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

/// The graded plane-strain quarter cylinder of ADR 0506 D6 with `nr x nt` Hex8 (Full).
fn cylinder(nr: usize, nt: usize, material: Material, p: f64) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 0.02], [nr, nt, 1]).unwrap();
    let nodes = base
        .nodes()
        .iter()
        .map(|q| {
            let r = 1.0 + ((2.0 * q[0]).exp() - 1.0) / (2f64.exp() - 1.0);
            let th = 0.5 * std::f64::consts::PI * q[1];
            [r * th.cos(), r * th.sin(), q[2]]
        })
        .collect();
    let mut mesh = rebuild(&base, nodes);
    let nn = mesh.nodes().len();
    mesh.add_node_set("every", (0..nn).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    m.add_load(Load::pressure("xmin", p)).unwrap();
    m
}

/// A box with symmetry planes on `xmin`, `ymin`, `zmin`.
fn sym_box(lengths: Vec3, div: [usize; 3], material: Material) -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], lengths, div).unwrap());
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
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

fn digests() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();
    let newton = NewtonOptions::new(4).threads(4);

    // ADR 0404: a Hex8 box clamped on xmin, its xmax face moved along and across (SVK, large deformation).
    {
        let mut m = SolidModel::new(hex_box([0.0; 3], [2.0, 1.0, 1.0], [3, 2, 2]).unwrap());
        m.add_material("all", steel(0.3)).unwrap();
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
        let mut pres = Vec::new();
        for &n in m.mesh().node_set("xmax").unwrap() {
            pres.push(Prescribed::new(n, 0, 0.2));
            pres.push(Prescribed::new(n, 1, 0.3));
        }
        let r = solve(&m, &pres, &[], &newton).unwrap();
        out.push(("TL Hex8 box solve", digest(&flat(&r))));
    }
    // ADR 0475: a J2 Hex8 bar stretched past yield.
    {
        let m = sym_box([4.0, 1.0, 1.0], [4, 1, 1], j2());
        let r = solve(&m, &stretch(&m, 1.6e-2), &[], &NewtonOptions::new(5).threads(4)).unwrap();
        out.push(("J2 Hex8 bar solve", digest(&flat(&r))));
    }
    // ADR 0475 on the cylinder of ADR 0506 D6 (12 x 6, Full).
    {
        let k = 2.0e7 / 3f64.sqrt();
        let p = k * (2.0 * 1.25f64.ln() + 1.0 - 1.25 * 1.25 / 4.0);
        let mat = Material::j2(steel(0.499), 2.0e7, 0.0, 0.0).unwrap();
        let m = cylinder(12, 6, mat, p);
        let r = solve(&m, &[], &[], &NewtonOptions::new(5).threads(4)).unwrap();
        out.push(("J2 cylinder 12x6 Full solve", digest(&flat(&r))));
    }
    // ADR 0489 on the cylinder (12 x 6, Full).
    {
        let m = cylinder(12, 6, creep(), 2.0e5);
        let opts = TimeSteppingOptions::new(5e-4).newton(NewtonOptions::new(1).threads(4));
        let h = solve_in_time(&m, &[], &[0.1, 0.3, 1.0], &opts).unwrap();
        out.push(("creep cylinder 12x6 Full solve_in_time", digest(&flat_history(&h))));
    }
    // ADR 0485, 0499: a viscoelastic Hex8 cube under a held stretch, solve_in_time and one solve_from_at.
    {
        let m = sym_box([1.0, 1.0, 1.0], [2, 2, 2], visco());
        let pres = stretch(&m, 1e-3);
        let opts = TimeSteppingOptions::new(f64::INFINITY).newton(NewtonOptions::new(1).threads(4));
        let h = solve_in_time(&m, &pres, &[0.01, 0.1, 1.0], &opts).unwrap();
        out.push(("visco cube solve_in_time", digest(&flat_history(&h))));
        let n = m.dof_map().n_equations();
        let mut start = vec![0.0; n];
        let fixed: Vec<(usize, f64)> = pres
            .iter()
            .map(|p| (m.dof_map().equation(p.node, p.component).unwrap(), p.value))
            .collect();
        for &(eq, v) in &fixed {
            start[eq] = v;
        }
        let ctx = PointContext::new().with_time(0.05, 0.05).unwrap();
        let eq = solve_from_at(
            &m,
            &pres,
            &vec![0.0; n],
            &start,
            &initial_states(&m).unwrap(),
            &NewtonOptions::new(1).threads(4),
            &[],
            &ctx,
        )
        .unwrap();
        let mut v = eq.u.clone();
        v.extend_from_slice(&eq.internal_force);
        v.extend_from_slice(&eq.residuals);
        v.extend_from_slice(&eq.energies);
        for s in &eq.states {
            v.extend_from_slice(s.values());
        }
        out.push(("visco cube solve_from_at", digest(&v)));
    }
    // nl_transient: an elastic Hex8 bar under a tip load history, with its strain energy; a viscoelastic bar.
    for (name, material) in [
        ("nl_transient elastic bar", steel(0.3)),
        ("nl_transient visco bar", visco()),
    ] {
        let elastic = matches!(material, Material::IsotropicElastic { .. });
        let mut m = SolidModel::new(hex_box([0.0; 3], [4.0, 1.0, 1.0], [4, 1, 1]).unwrap());
        m.add_material("all", material).unwrap();
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
        let n = m.dof_map().n_equations();
        let mut pattern = vec![0.0; n];
        for &nd in m.mesh().node_set("xmax").unwrap() {
            pattern[m.dof_map().equation(nd, 1).unwrap()] = if elastic { 1.0e8 } else { 1.0e5 };
        }
        let loads = [LoadHistory::new(pattern, LoadTable::constant(1.0, 0.0, 1.0).unwrap())];
        let mut it = Integrator::new(&m, &loads, &NlTransientOptions::new(1e-4).threads(4)).unwrap();
        it.start(0.0, &vec![0.0; n], &vec![0.0; n]).unwrap();
        it.run(3).unwrap();
        let mut v: Vec<f64> = Vec::new();
        if elastic {
            v.push(it.strain_energy().unwrap());
            v.push(it.kinetic_energy().unwrap());
        }
        let bytes = it.state().unwrap().to_bytes();
        let mut d = digest(&v);
        d ^= digest_bytes(bytes);
        out.push((name, d));
    }
    // The assembled internal force and tangent of a distorted J2 Hex8 model.
    {
        let base = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 2, 2]).unwrap();
        let mut rng = Rng::new(7);
        let nodes = base
            .nodes()
            .iter()
            .map(|p| [p[0] + rng.sym(0.05), p[1] + rng.sym(0.05), p[2] + rng.sym(0.05)])
            .collect();
        let mut m = SolidModel::new(rebuild(&base, nodes));
        m.add_material("all", j2()).unwrap();
        let n = m.dof_map().n_equations();
        let u: Vec<f64> = (0..n).map(|_| rng.sym(4e-3)).collect();
        let st = initial_states(&m).unwrap();
        let mut v = internal_force(&m, &u, &st).unwrap();
        v.extend_from_slice(&tangent(&m, &u, &st, Some(3)).unwrap().val);
        out.push(("J2 Hex8 internal_force and tangent", digest(&v)));
    }
    // The element entry points of a distorted Hex8 with J2 and with creep.
    {
        let mut rng = Rng::new(11);
        let el = element_for(ElementKind::Hex8);
        let x = distorted_hex8(&mut rng);
        let u: Vec<f64> = (0..24).map(|_| rng.sym(3e-3)).collect();
        let zero = ElementState::new(vec![0.0; 8 * rcfd_fem::mat_j2::STATE_SIZE]);
        let ctx = PointContext::default();
        let (f, k) = element_force_and_tangent_at(el, &x, &j2(), &u, &zero, &ctx).unwrap();
        let mut v = f;
        v.extend_from_slice(k.values());
        for s in element_second_piola_kirchhoff(el, &x, &j2(), &u, &zero).unwrap() {
            v.extend_from_slice(&s);
        }
        v.extend_from_slice(element_trial_state(el, &x, &j2(), &u, &zero).unwrap().values());
        out.push(("J2 Hex8 element entry points", digest(&v)));
        let mut cs = Vec::new();
        for _ in 0..8 {
            for _ in 0..6 {
                cs.push(rng.sym(2e-4));
            }
            cs.push(3e-4);
        }
        let cst = ElementState::new(cs);
        let mat = Material::creep(steel(0.3), 1e-25, 4.0, -0.5).unwrap();
        let ctx = PointContext::new().with_time(2.0, 0.5).unwrap();
        let (f, k) = element_force_and_tangent_at(el, &x, &mat, &u, &cst, &ctx).unwrap();
        let mut v = f;
        v.extend_from_slice(k.values());
        out.push(("creep Hex8 element force and tangent", digest(&v)));
    }
    out
}

#[test]
fn b0_full_nonlinear_paths_keep_their_bits() {
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
