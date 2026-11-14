//! F-408 (ADR 0440): the bit record of the registered models of the earlier tiers. Checks B1 to B8 and the negative control
//! N1 of the registration.
//!
//! This file uses only the API of main d3d05478: it was run once in the worktree at that commit (plus the registration and
//! this file, no library line changed) before any library code of F-408, and the digests it printed are the constants of
//! `MAIN`. After the change it runs unchanged and compares.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::{element_dofs, stiffness};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::element::element_for;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::material::PointContext;
use rcfd_fem::modal::{modal_analysis, modal_analysis_with, ModalOptions};
use rcfd_fem::nonlinear::{element_second_piola_kirchhoff, solve, NewtonOptions, NonlinearResult};
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
use rcfd_fem::shell;
use rcfd_fem::solve::{solve_static_with, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::transient::{Integrator, Method, Rayleigh, TransientOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};

/// FNV-1a 64 over the count and the `to_bits()` of each value, little-endian bytes (the digest of ADR 0291).
fn digest<I: IntoIterator<Item = f64>>(values: I) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let mut n = 0u64;
    let mut buf = Vec::new();
    for v in values {
        buf.push(v.to_bits());
        n += 1;
    }
    eat(n);
    for b in buf {
        eat(b);
    }
    h
}

fn digest_usize(values: &[usize]) -> u64 {
    digest(values.iter().map(|&v| f64::from_bits(v as u64)))
}

const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;

fn steel() -> Material {
    Material::isotropic_elastic(E_MOD, 0.3, RHO, 0.0).unwrap()
}

fn model_of(mesh: SolidMesh, mat: Material) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat).unwrap();
    m
}

fn tet10_box(origin: Vec3, lengths: Vec3, div: [usize; 3]) -> SolidMesh {
    upgrade_to_tet10(&tet_box(origin, lengths, div).unwrap()).unwrap().0
}

fn flat6(v: &[[f64; 6]]) -> Vec<f64> {
    v.iter().flat_map(|s| s.iter().copied()).collect()
}

// ------------------------------------------------------------------------------------------------ P, C, B (ADR 0291)

fn patch_field(x: Vec3) -> Vec3 {
    [
        1e-3 * (x[0] + 2.0 * x[1] - x[2]),
        1e-3 * (0.5 * x[0] - x[1] + 3.0 * x[2]),
        1e-3 * (-x[0] + x[1] + 0.7 * x[2]),
    ]
}

const P_ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const P_LENGTHS: Vec3 = [1.2, 1.0, 0.8];

fn patch_models() -> Vec<(&'static str, SolidModel, Vec<Prescribed>)> {
    let meshes = [
        ("P-Hex8", hex_box(P_ORIGIN, P_LENGTHS, [3, 3, 3]).unwrap()),
        ("P-Tet10", tet10_box(P_ORIGIN, P_LENGTHS, [2, 2, 2])),
    ];
    meshes
        .into_iter()
        .map(|(name, mesh)| {
            let mut boundary: Vec<usize> = BOX_FACES
                .iter()
                .flat_map(|f| mesh.node_set(f).unwrap().to_vec())
                .collect();
            boundary.sort_unstable();
            boundary.dedup();
            let mut pres = Vec::new();
            for &n in &boundary {
                let u = patch_field(mesh.nodes()[n]);
                for (c, &v) in u.iter().enumerate() {
                    pres.push(Prescribed::new(n, c, v));
                }
            }
            (name, model_of(mesh, steel()), pres)
        })
        .collect()
}

fn cantilever() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]).unwrap();
    let mut m = model_of(mesh, steel());
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [0.0, 0.0, -100.0])).unwrap();
    m
}

fn bars() -> Vec<(&'static str, SolidModel)> {
    let meshes = [
        ("B-Hex8", hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 1, 1]).unwrap()),
        ("B-Tet10", tet10_box([0.0; 3], [1.0, 0.1, 0.1], [6, 1, 1])),
    ];
    meshes
        .into_iter()
        .map(|(name, mesh)| {
            let mut m = model_of(mesh, steel());
            for c in 0..3 {
                m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
            }
            (name, m)
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------ E, H (F-397)

const CANT_L: f64 = 1.0;
const CANT_H: f64 = 0.02;

/// The elastica cantilever of `si_nonlinear.rs` at `alpha`.
fn elastica_model(alpha: f64) -> SolidModel {
    let mesh = upgrade_to_hex20(&hex_box([0.0; 3], [CANT_L, CANT_H, CANT_H], [50, 1, 1]).unwrap())
        .unwrap()
        .0;
    let mut m = model_of(mesh, Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap());
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", comp, 0.0)).unwrap();
    }
    let i = CANT_H * CANT_H.powi(3) / 12.0;
    let p = alpha * E_MOD * i / (CANT_L * CANT_L);
    m.add_load(Load::traction("xmax", [0.0, p / (CANT_H * CANT_H), 0.0]))
        .unwrap();
    m
}

/// The load-control bar of `si_nonlinear.rs` (Hex8, three symmetry planes, the traction of the stretch 1.2).
fn stretch_bar() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.5, 0.5], [2, 1, 1]).unwrap();
    let mut m = model_of(mesh, Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap());
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_load(Load::traction(
        "xmax",
        [E_MOD * 1.2 * (1.2 * 1.2 - 1.0) / 2.0, 0.0, 0.0],
    ))
    .unwrap();
    m
}

fn push_nonlinear(push: &mut dyn FnMut(String, u64), name: &str, r: &NonlinearResult) {
    push(
        format!("{name} displacement"),
        digest(r.result.displacement_values().iter().copied()),
    );
    push(
        format!("{name} reactions"),
        digest(r.result.reactions().unwrap().iter().copied()),
    );
    push(
        format!("{name} load factors"),
        digest(r.steps.iter().map(|s| s.load_factor)),
    );
    push(
        format!("{name} residuals"),
        digest(r.steps.iter().flat_map(|s| s.residuals.iter().copied())),
    );
    push(
        format!("{name} energies"),
        digest(r.steps.iter().flat_map(|s| s.energies.iter().copied())),
    );
    push(
        format!("{name} counts"),
        digest(
            r.steps
                .iter()
                .flat_map(|s| [s.step as f64, s.halvings as f64, (s.residuals.len() - 1) as f64]),
        ),
    );
}

fn elastica_stress(m: &SolidModel, r: &NonlinearResult) -> Vec<f64> {
    let mesh = m.mesh();
    let dofs = m.dof_map();
    let u = r.result.displacement_values();
    let mut out = Vec::new();
    for e in 0..mesh.n_elements() {
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
        let ue: Vec<f64> = element_dofs(&dofs, nodes).iter().map(|&q| u[q]).collect();
        let s = element_second_piola_kirchhoff(
            element_for(mesh.element_kind(e)),
            &coords,
            m.material_of(e).unwrap(),
            &ue,
            &r.states[e],
        )
        .unwrap();
        out.extend(flat6(&s));
    }
    out
}

// ------------------------------------------------------------------------------------------------ F (F-398)

fn transient_bar() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 1, 1]).unwrap();
    let mut m = model_of(mesh, Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap());
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn free_history(model: &SolidModel, phi: &[f64], options: &TransientOptions, steps: usize) -> [Vec<f64>; 3] {
    let mut it = Integrator::new(model, &[], options).unwrap();
    it.start(0.0, phi, &vec![0.0; phi.len()]).unwrap();
    let mut h = [Vec::new(), Vec::new(), Vec::new()];
    for k in 0..=steps {
        if k > 0 {
            it.step().unwrap();
        }
        h[0].extend(it.displacement().unwrap());
        h[1].extend(it.velocity().unwrap());
        h[2].extend(it.acceleration().unwrap());
    }
    h
}

// ------------------------------------------------------------------------------------------------ S (F-392)

fn shell_plate() -> (SolidModel, Vec<Prescribed>) {
    let n = 16;
    let idx = |i: usize, j: usize| j * (n + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            nodes.push([i as f64 / n as f64, j as f64 / n as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..n {
        for i in 0..n {
            conn.extend([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut pres = Vec::new();
    for (k, x) in nodes.iter().enumerate() {
        for comp in [0, 1, 5] {
            pres.push(Prescribed::new(k, comp, 0.0));
        }
        let onx = x[0] == 0.0 || x[0] == 1.0;
        let ony = x[1] == 0.0 || x[1] == 1.0;
        if onx || ony {
            pres.push(Prescribed::new(k, 2, 0.0));
        }
        if onx {
            pres.push(Prescribed::new(k, 3, 0.0));
        }
        if ony {
            pres.push(Prescribed::new(k, 4, 0.0));
        }
    }
    let nn = nodes.len();
    let ne = n * n;
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..nn).collect()).unwrap();
    let mut m = model_of(mesh, steel());
    m.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    (m, pres)
}

// ------------------------------------------------------------------------------------------------ Q (ADR 0273)

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 * (1.0 / 9007199254740992.0)
    }
}

fn q_strains() -> Vec<[f64; 6]> {
    let mut g = Lcg(0xF408_0440);
    (0..64)
        .map(|_| {
            let mut s = [0.0; 6];
            for v in &mut s {
                *v = (g.next() - 0.5) * 1e-3;
            }
            s
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------ all digests

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut push = |name: String, d: u64| {
        println!("    (\"{name}\", 0x{d:016x}),");
        out.push((name, d));
    };
    // B1: P and its stiffness.
    for (name, model, pres) in patch_models() {
        let r = solve_static_with(&model, &pres, &[], &StaticOptions::default().threads(1)).unwrap();
        push(
            format!("{name} displacement"),
            digest(r.displacement_values().iter().copied()),
        );
        push(
            format!("{name} reactions"),
            digest(r.reactions().unwrap().iter().copied()),
        );
        let field = IntegrationPointField::from_result(&model, &r).unwrap();
        push(format!("{name} stress"), digest(flat6(field.stress())));
        let k = stiffness(&model).unwrap();
        push(
            format!("K {name} pattern"),
            digest_usize(&k.row_ptr) ^ digest_usize(&k.col).rotate_left(1),
        );
        push(format!("K {name} values"), digest(k.val.iter().copied()));
    }
    // B2: C direct and PCG.
    let c = cantilever();
    let solvers = [
        ("C-direct", StaticOptions::default().threads(1)),
        (
            "C-pcg",
            StaticOptions::with_solver(StaticSolver::PreconditionedCg {
                preconditioner: CgPreconditioner::BlockJacobi,
                tolerance: 1e-8,
                max_iterations: 20000,
            }),
        ),
    ];
    for (name, opts) in solvers {
        let r = solve_static_with(&c, &[], &[], &opts).unwrap();
        push(
            format!("{name} displacement"),
            digest(r.displacement_values().iter().copied()),
        );
        push(
            format!("{name} reactions"),
            digest(r.reactions().unwrap().iter().copied()),
        );
        let field = IntegrationPointField::from_result(&c, &r).unwrap();
        push(format!("{name} stress"), digest(flat6(field.stress())));
        let nodal = nodal_average(&c, &field, field.stress(), None).unwrap();
        push(
            format!("{name} nodal stress"),
            digest(nodal.iter().flat_map(|s| s.unwrap_or([f64::NAN; 6]))),
        );
    }
    let k = stiffness(&c).unwrap();
    push(
        "K C pattern".into(),
        digest_usize(&k.row_ptr) ^ digest_usize(&k.col).rotate_left(1),
    );
    push("K C values".into(), digest(k.val.iter().copied()));
    // B3: bar modes.
    for (name, model) in bars() {
        let r = modal_analysis(&model, &ModalOptions::new(4)).unwrap();
        push(format!("{name} eigenvalues"), digest(r.eigenvalues.iter().copied()));
        push(format!("{name} frequencies"), digest(r.frequencies.iter().copied()));
        push(format!("{name} modes"), digest(r.modes.iter().flatten().copied()));
    }
    // B4: the elastica at alpha = 1, five load steps.
    let em = elastica_model(1.0);
    let er = solve(&em, &[], &[], &NewtonOptions::new(5)).unwrap();
    push_nonlinear(&mut push, "E", &er);
    push("E stress".into(), digest(elastica_stress(&em, &er)));
    // B5: the bar with one halving.
    let hm = stretch_bar();
    let hr = solve(&hm, &[], &[], &NewtonOptions::new(1).max_iterations(4)).unwrap();
    push_nonlinear(&mut push, "H", &hr);
    // B6: free vibration, Newmark and generalised-alpha with Rayleigh damping.
    let fm = transient_bar();
    let modal = modal_analysis(&fm, &ModalOptions::new(2)).unwrap();
    let phi = modal.modes[0].clone();
    let omega = modal.eigenvalues[0].sqrt();
    let dt = 0.1 / omega;
    let f1 = free_history(
        &fm,
        &phi,
        &TransientOptions::new(dt).method(Method::average_acceleration()),
        200,
    );
    let f2 = free_history(
        &fm,
        &phi,
        &TransientOptions::new(dt)
            .method(Method::GeneralisedAlpha { rho_inf: 0.8 })
            .damping(Rayleigh::from_ratio(0.02, omega, 5.0 * omega).unwrap()),
        100,
    );
    for (name, h) in [("F1", &f1), ("F2", &f2)] {
        for (q, what) in ["displacement", "velocity", "acceleration"].iter().enumerate() {
            push(format!("{name} {what} history"), digest(h[q].iter().copied()));
        }
    }
    // B7: the Shell4 plate.
    let (sm, spres) = shell_plate();
    let sr = solve_static_with(&sm, &spres, &[], &StaticOptions::default()).unwrap();
    push(
        "S displacement".into(),
        digest(sr.displacement_values().iter().copied()),
    );
    push("S reactions".into(), digest(sr.reactions().unwrap().iter().copied()));
    let st = shell::stresses(&sm, sr.displacement_values()).unwrap();
    push("S membrane".into(), digest(st.iter().flat_map(|(_, s)| s.membrane)));
    push("S bending".into(), digest(st.iter().flat_map(|(_, s)| s.bending)));
    push("S shear".into(), digest(st.iter().flat_map(|(_, s)| s.shear)));
    push("S top".into(), digest(st.iter().flat_map(|(_, s)| s.top)));
    push("S bottom".into(), digest(st.iter().flat_map(|(_, s)| s.bottom)));
    let sk = stiffness(&sm).unwrap();
    push("K S values".into(), digest(sk.val.iter().copied()));
    let smod = modal_analysis_with(&sm, &spres, &ModalOptions::new(4)).unwrap();
    push("S frequencies".into(), digest(smod.frequencies.iter().copied()));
    push("S modes".into(), digest(smod.modes.iter().flatten().copied()));
    // B8: the isotropic constitutive call with the default point context.
    let mat = Material::isotropic_elastic(E_MOD, 0.3, RHO, 0.0).unwrap();
    let mut stress = Vec::new();
    let mut tangent = Vec::new();
    for eps in q_strains() {
        let (s, d) = mat.stress(&eps, &PointContext::default(), &[]).unwrap();
        stress.extend(s);
        tangent.extend(d.iter().flatten().copied());
    }
    push("Q stress".into(), digest(stress));
    push("Q tangent".into(), digest(tangent));
    out
}

/// The digests printed by this file on main d3d05478 (the record run of the registration).
const MAIN: &[(&str, u64)] = &[
    ("P-Hex8 displacement", 0xb63760e800d5386b),
    ("P-Hex8 reactions", 0xb1f9e97743715c44),
    ("P-Hex8 stress", 0x977d10acac84df90),
    ("K P-Hex8 pattern", 0xd4ad9c039a8591a9),
    ("K P-Hex8 values", 0x0feb5c655461b48b),
    ("P-Tet10 displacement", 0x9db5fd3873108200),
    ("P-Tet10 reactions", 0xbda9b12b25ef84e3),
    ("P-Tet10 stress", 0x5d6c91c973442d5c),
    ("K P-Tet10 pattern", 0xfc495cf74e9afccf),
    ("K P-Tet10 values", 0x558f616371f10b4f),
    ("C-direct displacement", 0x36ba978186575b91),
    ("C-direct reactions", 0x013a7fa67b8d609e),
    ("C-direct stress", 0xf8195fc6f210799e),
    ("C-direct nodal stress", 0xff16adba1f5ef2e5),
    ("C-pcg displacement", 0x92a7cebf5760028a),
    ("C-pcg reactions", 0x69d30f77b48fec73),
    ("C-pcg stress", 0x9e606f6eaa329775),
    ("C-pcg nodal stress", 0xf20167aa66172022),
    ("K C pattern", 0x87caf720b3a8d1b7),
    ("K C values", 0x08aeab234e3188f7),
    ("B-Hex8 eigenvalues", 0x52252f590ed9ba7d),
    ("B-Hex8 frequencies", 0xfa31dd28316c8b14),
    ("B-Hex8 modes", 0x3890924ef2010520),
    ("B-Tet10 eigenvalues", 0xa41ea8b12eed3386),
    ("B-Tet10 frequencies", 0x33db964b9d3e6d7c),
    ("B-Tet10 modes", 0x109c955ae7319ba4),
    ("E displacement", 0x25a283aaf7df0d58),
    ("E reactions", 0xd44ad6b024f88751),
    ("E load factors", 0xb4105b3a7ba55ea0),
    ("E residuals", 0xfaf7c82ac137d4e2),
    ("E energies", 0x050c85266541a40d),
    ("E counts", 0x0daa33038712cabb),
    ("E stress", 0x7246fe1c60695da9),
    ("H displacement", 0x9d247a026c3d219f),
    ("H reactions", 0x861d891e4195b589),
    ("H load factors", 0x9aee17b1f7881337),
    ("H residuals", 0x897b7cef29ac54cd),
    ("H energies", 0x5f03e1dde7541701),
    ("H counts", 0x425d393ce08f5703),
    ("F1 displacement history", 0xa02bc2e0194e505f),
    ("F1 velocity history", 0xe58d1219d181717a),
    ("F1 acceleration history", 0x48edd23fc6cae5f9),
    ("F2 displacement history", 0x47cc4beade679e7d),
    ("F2 velocity history", 0x89b65e172e5484a7),
    ("F2 acceleration history", 0x952404e2083f59dd),
    ("S displacement", 0x373759ab50c477ab),
    ("S reactions", 0xbf53e086bc49204b),
    ("S membrane", 0x1d0bd31239f24134),
    ("S bending", 0x6d155168ffdd3bea),
    ("S shear", 0xfffb3162d1547926),
    ("S top", 0x3a89c9e0e27c5d9b),
    ("S bottom", 0x928c833a81ed769b),
    ("K S values", 0x92dd677ee0c4661c),
    ("S frequencies", 0x5a2ec10046ca9435),
    ("S modes", 0xaff40a85046e8bdc),
    ("Q stress", 0xa32c979fc7882878),
    ("Q tangent", 0xd491aaa4e8944f12),
];

/// B1 to B8: every digest equals main's.
#[test]
fn b1_to_b8_registered_models_have_the_bits_of_main() {
    println!("digests of this tree:");
    let now = all_digests();
    assert!(!MAIN.is_empty(), "record run: no digests of main recorded yet");
    assert_eq!(now.len(), MAIN.len(), "number of digests");
    let mut missed = Vec::new();
    for ((name, d), (mname, md)) in now.iter().zip(MAIN) {
        assert_eq!(name, mname, "digest order");
        let ok = d == md;
        println!(
            "{name}: {} (main 0x{md:016x}, now 0x{d:016x})",
            if ok { "equal" } else { "DIFFERS" }
        );
        if !ok {
            missed.push(name.clone());
        }
    }
    assert!(missed.is_empty(), "digests differ from main: {missed:?}");
}

/// N1: one ulp in one displacement entry changes the digest.
#[test]
fn n1_digest_sees_one_ulp() {
    let (_, model, pres) = patch_models().remove(0);
    let r = solve_static_with(&model, &pres, &[], &StaticOptions::default().threads(1)).unwrap();
    let mut u = r.displacement_values().to_vec();
    let a = digest(u.iter().copied());
    let k = u.len() / 2;
    u[k] = f64::from_bits(u[k].to_bits() + 1);
    let b = digest(u.iter().copied());
    println!("N1: 0x{a:016x} -> 0x{b:016x}");
    assert_ne!(a, b);
}
