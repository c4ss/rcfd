//! F-374 (ADR 0275): assembly, displacement constraints, nodal forces and the linear static solve. Checks S1, D1, P1, P2,
//! U1, U2, R1, R2, F1 to F5, the extensions E1 to E8 and the negative controls N1, N2 of the registration (seed
//! 0xF374_0275).

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::{assemble_with, stiffness_with};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::{prescribed_equations, rigid_gram_ratios, Prescribed};
use rcfd_fem::element::element_for;
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, FemError, Load, Material, SolidModel, SolidResult};
use rcfd_linalg::CsrMatrix;
use std::collections::{BTreeMap, BTreeSet};

const SEED: u64 = 0xF374_0275;
const KINDS: [ElementKind; 3] = [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10];
const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;

/// SplitMix64, a deterministic source only (ADR 0273 D6, ADR 0275 D8).
struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(SEED.wrapping_add(tag))
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    fn sym(&mut self, r: f64) -> f64 {
        r * (2.0 * self.uniform() - 1.0)
    }
}

fn kind_index(kind: ElementKind) -> u64 {
    match kind {
        ElementKind::Tet4 => 0,
        ElementKind::Hex8 => 1,
        _ => 2,
    }
}

fn material() -> Material {
    Material::isotropic_elastic(E_MOD, NU, 7850.0, 0.0).unwrap()
}

/// The same blocks and sets on a new node list (which may be longer than the old one).
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

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

/// A box of the kind; for Tet10 the Tet4 box upgraded (straight edges). With `perturb`, the interior grid nodes move by
/// `0.1 h_k (2U - 1)` per coordinate (ADR 0275 arithmetic 6) before the upgrade.
fn box_of(kind: ElementKind, origin: Vec3, lengths: Vec3, div: [usize; 3], perturb: Option<u64>) -> SolidMesh {
    let base = match kind {
        ElementKind::Hex8 => hex_box(origin, lengths, div).unwrap(),
        _ => tet_box(origin, lengths, div).unwrap(),
    };
    let base = match perturb {
        None => base,
        Some(tag) => {
            let mut rng = Rng::new(tag);
            let boundary = boundary_nodes(&base);
            let h = [
                lengths[0] / div[0] as f64,
                lengths[1] / div[1] as f64,
                lengths[2] / div[2] as f64,
            ];
            let mut nodes = base.nodes().to_vec();
            for (i, p) in nodes.iter_mut().enumerate() {
                if !boundary.contains(&i) {
                    for k in 0..3 {
                        p[k] += 0.1 * h[k] * (2.0 * rng.uniform() - 1.0);
                    }
                }
            }
            rebuild(&base, nodes)
        }
    };
    match kind {
        ElementKind::Tet10 => upgrade_to_tet10(&base).unwrap().0,
        _ => base,
    }
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m
}

fn symmetry(k: &CsrMatrix) -> f64 {
    let mut amax: f64 = 0.0;
    let mut dmax: f64 = 0.0;
    for i in 0..k.n {
        for p in k.row_ptr[i]..k.row_ptr[i + 1] {
            let j = k.col[p];
            let (s, e) = (k.row_ptr[j], k.row_ptr[j + 1]);
            let q = s + k.col[s..e].binary_search(&i).expect("pattern symmetric");
            amax = amax.max(k.val[p].abs());
            dmax = dmax.max((k.val[p] - k.val[q]).abs());
        }
    }
    dmax / amax
}

fn node_near(mesh: &SolidMesh, x: Vec3) -> usize {
    let mut best = (f64::INFINITY, 0);
    for (i, p) in mesh.nodes().iter().enumerate() {
        let d = (0..3).map(|k| (p[k] - x[k]).powi(2)).sum::<f64>();
        if d < best.0 {
            best = (d, i);
        }
    }
    assert!(best.0 < 1e-24, "no node at {x:?}");
    best.1
}

/// Consistent nodal loads of a uniform traction `t` on the flat faces of a surface set (ADR 0275 arithmetic 10).
fn traction_loads(mesh: &SolidMesh, set: &str, t: Vec3) -> Vec<NodalForceAt> {
    let mut acc: BTreeMap<usize, Vec3> = BTreeMap::new();
    for &fr in mesh.surface_set(set).unwrap() {
        let nodes = mesh.face_nodes(fr);
        let a = mesh.face_area(fr);
        let share: Vec<f64> = match nodes.len() {
            4 => vec![0.25; 4],
            3 => vec![1.0 / 3.0; 3],
            6 => vec![0.0, 0.0, 0.0, 1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
            n => panic!("face of {n} nodes"),
        };
        for (&n, s) in nodes.iter().zip(share) {
            let e = acc.entry(n).or_insert([0.0; 3]);
            for k in 0..3 {
                e[k] += s * a * t[k];
            }
        }
    }
    acc.into_iter().map(|(n, f)| NodalForceAt::new(n, f)).collect()
}

/// Force and moment imbalance of loads plus reactions: (|sum (f + r)| / sum |f + r|_node... ) as registered: the sums are
/// divided by `scale = sum_nodes |w_node|_2` of the given weights and, for the moment, by `diag` as well.
fn balance(
    res: &SolidResult,
    mesh: &SolidMesh,
    f: &[f64],
    centre: Vec3,
    diag: f64,
    scale_by_loads: bool,
) -> (f64, f64) {
    let r = res.reactions().unwrap();
    let mut sf = [0.0; 3];
    let mut sm = [0.0; 3];
    let mut scale = 0.0;
    for (n, x) in mesh.nodes().iter().enumerate() {
        let mut t = [0.0; 3];
        let mut w = 0.0;
        for k in 0..3 {
            t[k] = f[3 * n + k] + r[3 * n + k];
            let wk = if scale_by_loads { f[3 * n + k] } else { r[3 * n + k] };
            w += wk * wk;
        }
        scale += w.sqrt();
        let y = [x[0] - centre[0], x[1] - centre[1], x[2] - centre[2]];
        let m = [
            y[1] * t[2] - y[2] * t[1],
            y[2] * t[0] - y[0] * t[2],
            y[0] * t[1] - y[1] * t[0],
        ];
        for k in 0..3 {
            sf[k] += t[k];
            sm[k] += m[k];
        }
    }
    let nf = (sf[0] * sf[0] + sf[1] * sf[1] + sf[2] * sf[2]).sqrt();
    let nm = (sm[0] * sm[0] + sm[1] * sm[1] + sm[2] * sm[2]).sqrt();
    (nf / scale, nm / (diag * scale))
}

// ---------------------------------------------------------------------------------------------------------------- patch

const P_ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const P_LENGTHS: Vec3 = [1.2, 1.0, 0.8];
const P_DIV: [usize; 3] = [4, 3, 3];
const FIELD_C: Vec3 = [1e-3, -2e-3, 0.5e-3];
const FIELD_A: [[f64; 3]; 3] = [
    [1.0e-3, 0.3e-3, -0.2e-3],
    [0.5e-3, -0.7e-3, 0.4e-3],
    [0.1e-3, 0.6e-3, 0.9e-3],
];

fn linear_field(x: Vec3) -> Vec3 {
    let mut u = FIELD_C;
    for i in 0..3 {
        for j in 0..3 {
            u[i] += FIELD_A[i][j] * x[j];
        }
    }
    u
}

fn quadratic_field(x: Vec3) -> Vec3 {
    let [a, b, c] = x;
    [1e-3 * (a * a + b * c), 1e-3 * (b * b - a * c), 1e-3 * (c * c + a * b)]
}

fn patch_mesh(kind: ElementKind) -> SolidMesh {
    box_of(kind, P_ORIGIN, P_LENGTHS, P_DIV, Some(10 + kind_index(kind)))
}

/// Solves the patch with the boundary values of `field`; returns (result, interior error relative to max |field|).
fn patch_solve(kind: ElementKind, field: fn(Vec3) -> Vec3) -> (SolidModel, SolidResult, Vec<Prescribed>, f64) {
    let mesh = patch_mesh(kind);
    let boundary = boundary_nodes(&mesh);
    let mut pres = Vec::new();
    for &n in &boundary {
        let u = field(mesh.nodes()[n]);
        for (k, &uk) in u.iter().enumerate() {
            pres.push(Prescribed::new(n, k, uk));
        }
    }
    let model = model_of(mesh);
    let res = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap();
    let mesh = model.mesh();
    let mut umax: f64 = 0.0;
    let mut err: f64 = 0.0;
    for (n, x) in mesh.nodes().iter().enumerate() {
        let u = field(*x);
        let uh = res.displacement(n);
        for k in 0..3 {
            umax = umax.max(u[k].abs());
            if !boundary.contains(&n) {
                err = err.max((uh[k] - u[k]).abs());
            }
        }
    }
    (model, res, pres, err / umax)
}

#[test]
fn p1_p2_r2_patch_test_and_n1() {
    for kind in KINDS {
        let (model, res, pres, err) = patch_solve(kind, linear_field);
        let mesh = model.mesh();
        let n_interior = mesh.nodes().len() - boundary_nodes(mesh).len();
        // P2: prescribed values bit for bit.
        let mut p2 = true;
        for p in &pres {
            p2 &= res.displacement(p.node)[p.component].to_bits() == p.value.to_bits();
        }
        // R2: reactions balance (no load).
        let f = vec![0.0; 3 * mesh.nodes().len()];
        let centre = [
            P_ORIGIN[0] + 0.5 * P_LENGTHS[0],
            P_ORIGIN[1] + 0.5 * P_LENGTHS[1],
            P_ORIGIN[2] + 0.5 * P_LENGTHS[2],
        ];
        let diag = (P_LENGTHS.iter().map(|l| l * l).sum::<f64>()).sqrt();
        let (bf, bm) = balance(&res, mesh, &f, centre, diag, false);
        // N1: quadratic boundary values.
        let (_, _, _, n1) = patch_solve(kind, quadratic_field);
        let fixed = prescribed_equations(&model, &pres).unwrap();
        let gram = rigid_gram_ratios(&model, &fixed).unwrap();
        let stats = res.stats().unwrap();
        println!(
            "P1 {:?}: nodes {}, interior {}, unknowns solved {}, interior error {:.3e} (limit 1e-10); P2 bit-equal {}; \
             R2 force {:.3e} moment {:.3e} (limit 1e-10); N1 quadratic {:.3e} (> 1e-6); E8 gram ratio {:.3e}; \
             residual {:.3e}",
            kind,
            mesh.nodes().len(),
            n_interior,
            stats.unknowns(),
            err,
            p2,
            bf,
            bm,
            n1,
            gram[0].1,
            stats.relative_residual()
        );
        assert!(err <= 1e-10, "P1 {kind:?}: {err:e}");
        assert!(p2, "P2 {kind:?}");
        assert!(bf <= 1e-10 && bm <= 1e-10, "R2 {kind:?}: {bf:e} {bm:e}");
        assert!(n1 > 1e-6, "N1 {kind:?}: {n1:e}");
        assert!(gram[0].1 > 1e-10, "E8 {kind:?}");
    }
}

// ------------------------------------------------------------------------------------------------------------- uniaxial

const U_LENGTHS: Vec3 = [2.0, 0.5, 0.4];
const U_DIV: [usize; 3] = [6, 2, 2];
const U_FORCE: f64 = 1.0e6;

fn uniaxial_mesh(kind: ElementKind) -> SolidMesh {
    box_of(kind, [0.0; 3], U_LENGTHS, U_DIV, None)
}

fn three_two_one(mesh: &SolidMesh, with_p3: bool) -> Vec<Prescribed> {
    let p1 = node_near(mesh, [0.0, 0.0, 0.0]);
    let p2 = node_near(mesh, [0.0, 0.0, U_LENGTHS[2]]);
    let p3 = node_near(mesh, [0.0, U_LENGTHS[1], 0.0]);
    let mut v = vec![
        Prescribed::new(p1, 0, 0.0),
        Prescribed::new(p1, 1, 0.0),
        Prescribed::new(p1, 2, 0.0),
        Prescribed::new(p2, 0, 0.0),
        Prescribed::new(p2, 1, 0.0),
    ];
    if with_p3 {
        v.push(Prescribed::new(p3, 0, 0.0));
    }
    v
}

fn end_loads(mesh: &SolidMesh) -> Vec<NodalForceAt> {
    let area = U_LENGTHS[1] * U_LENGTHS[2];
    let t = U_FORCE / area;
    let mut v = traction_loads(mesh, "xmax", [t, 0.0, 0.0]);
    v.extend(traction_loads(mesh, "xmin", [-t, 0.0, 0.0]));
    v
}

/// (U1 elongation error, U2 lateral y error, U2 lateral z error).
fn uniaxial_errors(mesh: &SolidMesh, res: &SolidResult) -> (f64, f64, f64) {
    let area = U_LENGTHS[1] * U_LENGTHS[2];
    let eps = U_FORCE / (E_MOD * area);
    let delta = U_FORCE * U_LENGTHS[0] / (E_MOD * area);
    let mut e1: f64 = 0.0;
    for &n in mesh.node_set("xmax").unwrap() {
        e1 = e1.max((res.displacement(n)[0] - delta).abs() / delta);
    }
    let (mut ey, mut ez): (f64, f64) = (0.0, 0.0);
    for (n, x) in mesh.nodes().iter().enumerate() {
        let u = res.displacement(n);
        ey = ey.max((u[1] + NU * eps * x[1]).abs() / (NU * eps * U_LENGTHS[1]));
        ez = ez.max((u[2] + NU * eps * x[2]).abs() / (NU * eps * U_LENGTHS[2]));
    }
    (e1, ey, ez)
}

#[test]
fn u1_u2_uniaxial_tension_and_e6() {
    for kind in KINDS {
        let mesh = uniaxial_mesh(kind);
        let pres = three_two_one(&mesh, true);
        let loads = end_loads(&mesh);
        let model = model_of(mesh);
        let res = solve_static_with(&model, &pres, &loads, &StaticOptions::default()).unwrap();
        let (e1, ey, ez) = uniaxial_errors(model.mesh(), &res);
        let fixed = prescribed_equations(&model, &pres).unwrap();
        let gram = rigid_gram_ratios(&model, &fixed).unwrap();
        println!(
            "U {:?}: unknowns {}, U1 elongation {:.3e}, U2 lateral y {:.3e} z {:.3e} (limit 1e-10); E8 gram ratio {:.3e}; \
             residual {:.3e}",
            kind,
            res.stats().unwrap().unknowns(),
            e1,
            ey,
            ez,
            gram[0].1,
            res.stats().unwrap().relative_residual()
        );
        assert!(e1 <= 1e-10 && ey <= 1e-10 && ez <= 1e-10, "U {kind:?}");
        assert!(gram[0].1 > 1e-10, "E8 {kind:?}");
        if kind == ElementKind::Hex8 {
            let n = res.displacement_values().len();
            let opts = StaticOptions::with_solver(StaticSolver::ConjugateGradient {
                tolerance: 1e-13,
                max_iterations: 10 * n,
            });
            let cg = solve_static_with(&model, &pres, &loads, &opts).unwrap();
            let umax = res.displacement_values().iter().fold(0.0f64, |m, v| m.max(v.abs()));
            let d = res
                .displacement_values()
                .iter()
                .zip(cg.displacement_values())
                .fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
            println!(
                "E6 Hex8: CG-Jacobi {} iterations, difference to direct {:.3e} (limit 1e-7)",
                cg.stats().unwrap().iterations(),
                d / umax
            );
            assert!(d / umax <= 1e-7, "E6");
        }
    }
    // U3: Hex8 [4, 1, 1] through the model only (node sets, Constraint::displacement, Load::nodal_force).
    let mut mesh = box_of(ElementKind::Hex8, [0.0; 3], U_LENGTHS, [4, 1, 1], None);
    let p1 = node_near(&mesh, [0.0, 0.0, 0.0]);
    let p2 = node_near(&mesh, [0.0, 0.0, U_LENGTHS[2]]);
    let p3 = node_near(&mesh, [0.0, U_LENGTHS[1], 0.0]);
    mesh.add_node_set("p1", vec![p1]).unwrap();
    mesh.add_node_set("p2", vec![p2]).unwrap();
    mesh.add_node_set("p3", vec![p3]).unwrap();
    let mut model = model_of(mesh);
    for (set, comp) in [("p1", 0), ("p1", 1), ("p1", 2), ("p2", 0), ("p2", 1), ("p3", 0)] {
        model.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
    }
    model
        .add_load(Load::nodal_force("xmax", [U_FORCE / 4.0, 0.0, 0.0]))
        .unwrap();
    model
        .add_load(Load::nodal_force("xmin", [-U_FORCE / 4.0, 0.0, 0.0]))
        .unwrap();
    let res = solve_static(&model, &StaticOptions::default()).unwrap();
    let (e1, ey, ez) = uniaxial_errors(model.mesh(), &res);
    println!("U3 Hex8 [4, 1, 1] model path: U1 {e1:.3e}, U2 y {ey:.3e} z {ez:.3e} (limit 1e-10)");
    assert!(e1 <= 1e-10 && ey <= 1e-10 && ez <= 1e-10, "U3");
}

// ------------------------------------------------------------------------------------------------------------ reactions

const R_LENGTHS: Vec3 = [1.0, 0.6, 0.5];
const R_DIV: [usize; 3] = [4, 3, 2];

fn reaction_case(kind: ElementKind) -> (SolidModel, Vec<NodalForceAt>) {
    let mesh = box_of(kind, [0.0; 3], R_LENGTHS, R_DIV, None);
    let mut rng = Rng::new(20 + kind_index(kind));
    let mut loaded: BTreeSet<usize> = BTreeSet::new();
    loaded.extend(mesh.node_set("xmax").unwrap().iter().copied());
    loaded.extend(mesh.node_set("ymax").unwrap().iter().copied());
    let forces: Vec<NodalForceAt> = loaded
        .into_iter()
        .map(|n| NodalForceAt::new(n, [rng.sym(1e3), rng.sym(1e3), rng.sym(1e3)]))
        .collect();
    let mut model = model_of(mesh);
    for c in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    model
        .add_load(Load::nodal_force("zmax", [100.0, -200.0, 300.0]))
        .unwrap();
    (model, forces)
}

#[test]
fn r1_reaction_balance_n2_and_e7() {
    let centre = [0.5 * R_LENGTHS[0], 0.5 * R_LENGTHS[1], 0.5 * R_LENGTHS[2]];
    let diag = (R_LENGTHS.iter().map(|l| l * l).sum::<f64>()).sqrt();
    for kind in KINDS {
        let (model, forces) = reaction_case(kind);
        let res = solve_static_with(&model, &[], &forces, &StaticOptions::default()).unwrap();
        let f = nodal_load_vector(&model, &forces).unwrap();
        let (bf, bm) = balance(&res, model.mesh(), &f, centre, diag, true);
        // N2: the applied forces alone.
        let mut sf = [0.0; 3];
        let mut scale = 0.0;
        for n in 0..model.mesh().nodes().len() {
            let mut w = 0.0;
            for k in 0..3 {
                sf[k] += f[3 * n + k];
                w += f[3 * n + k] * f[3 * n + k];
            }
            scale += f64::sqrt(w);
        }
        let n2 = (sf[0] * sf[0] + sf[1] * sf[1] + sf[2] * sf[2]).sqrt() / scale;
        println!(
            "R1 {:?}: unknowns {}, force {:.3e} moment {:.3e} (limit 1e-10); N2 loads alone {:.3e} (> 1e-3); residual {:.3e}",
            kind,
            res.stats().unwrap().unknowns(),
            bf,
            bm,
            n2,
            res.stats().unwrap().relative_residual()
        );
        assert!(bf <= 1e-10 && bm <= 1e-10, "R1 {kind:?}: {bf:e} {bm:e}");
        assert!(n2 > 1e-3, "N2 {kind:?}");
        if kind == ElementKind::Tet10 {
            let r1 = solve_static_with(&model, &[], &forces, &StaticOptions::default().threads(1)).unwrap();
            let r8 = solve_static_with(&model, &[], &forces, &StaticOptions::default().threads(8)).unwrap();
            let same_u = r1
                .displacement_values()
                .iter()
                .zip(r8.displacement_values())
                .all(|(a, b)| a.to_bits() == b.to_bits());
            let same_r = r1
                .reactions()
                .unwrap()
                .iter()
                .zip(r8.reactions().unwrap())
                .all(|(a, b)| a.to_bits() == b.to_bits());
            println!("E7 Tet10: solve with 1 and 8 threads bit-equal: displacement {same_u}, reactions {same_r}");
            assert!(same_u && same_r, "E7");
        }
    }
}

// --------------------------------------------------------------------------------------------- symmetry and threads

#[test]
fn s1_d1_symmetry_and_thread_identity() {
    let mut worst: f64 = 0.0;
    for kind in KINDS {
        let pm = model_of(patch_mesh(kind));
        let k1 = stiffness_with(&pm, Some(1)).unwrap();
        let k8 = stiffness_with(&pm, Some(8)).unwrap();
        let same = k1.row_ptr == k8.row_ptr
            && k1.col == k8.col
            && k1.val.iter().zip(&k8.val).all(|(a, b)| a.to_bits() == b.to_bits());
        let s_p = symmetry(&k1);
        let s_u = symmetry(&stiffness_with(&model_of(uniaxial_mesh(kind)), None).unwrap());
        let s_r = symmetry(&stiffness_with(&reaction_case(kind).0, None).unwrap());
        println!(
            "S1/D1 {:?}: patch K {} x {} ({} entries), 1 and 8 threads bit-equal {}; symmetry patch {:.3e}, uniaxial {:.3e}, \
             reactions {:.3e} (limit 1e-13)",
            kind,
            k1.n,
            k1.n,
            k1.nnz(),
            same,
            s_p,
            s_u,
            s_r
        );
        assert!(same, "D1 {kind:?}");
        worst = worst.max(s_p).max(s_u).max(s_r);
    }
    assert!(worst <= 1e-13, "S1: {worst:e}");
}

// ------------------------------------------------------------------------------------------------------------- refusals

fn expect_rigid(res: Result<SolidResult, FemError>, count: usize, part: Option<usize>, label: &str) {
    match res {
        Err(e @ FemError::RigidBodyMotionFree { .. }) => {
            let msg = e.to_string();
            println!("{label}: {msg}");
            assert!(msg.contains("rigid-body motion"), "{label}: message");
            if let FemError::RigidBodyMotionFree {
                count: c, part_node, ..
            } = e
            {
                assert_eq!(c, count, "{label}: count");
                if let Some(p) = part {
                    assert_eq!(part_node, p, "{label}: part");
                }
            }
        }
        other => panic!("{label}: expected a rigid-body refusal, got {other:?}"),
    }
}

#[test]
fn f1_to_f4_free_rigid_body_motions_are_refused() {
    let mesh = uniaxial_mesh(ElementKind::Hex8);
    let loads = end_loads(&mesh);
    // F1: u_x = 0 on xmin only.
    let mut m1 = model_of(mesh.clone());
    m1.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    expect_rigid(
        solve_static_with(&m1, &[], &loads, &StaticOptions::default()),
        3,
        None,
        "F1",
    );
    // F2: the 3-2-1 set without P3.
    let m2 = model_of(mesh.clone());
    let p = three_two_one(&mesh, false);
    expect_rigid(
        solve_static_with(&m2, &p, &loads, &StaticOptions::default()),
        1,
        None,
        "F2",
    );
    // F3: three collinear nodes fully held.
    let mut p3 = Vec::new();
    for x in [0.0, 1.0, 2.0] {
        let n = node_near(&mesh, [x, 0.0, 0.0]);
        for c in 0..3 {
            p3.push(Prescribed::new(n, c, 0.0));
        }
    }
    expect_rigid(
        solve_static_with(&m2, &p3, &loads, &StaticOptions::default()),
        1,
        None,
        "F3",
    );
    // F4: two disjoint boxes, the first clamped.
    let a = hex_box([0.0; 3], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let b = hex_box([3.0, 0.0, 0.0], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let n1 = a.nodes().len();
    let mut nodes = a.nodes().to_vec();
    nodes.extend_from_slice(b.nodes());
    let mut two = SolidMesh::new(nodes).unwrap();
    two.add_block(ElementKind::Hex8, a.blocks()[0].connectivity().to_vec())
        .unwrap();
    two.add_block(
        ElementKind::Hex8,
        b.blocks()[0].connectivity().iter().map(|&v| v + n1).collect(),
    )
    .unwrap();
    two.add_element_set("all", (0..two.n_elements()).collect()).unwrap();
    two.add_node_set("clamp", a.node_set("xmin").unwrap().to_vec()).unwrap();
    let mut m4 = model_of(two);
    for c in 0..3 {
        m4.add_constraint(Constraint::displacement("clamp", c, 0.0)).unwrap();
    }
    expect_rigid(solve_static(&m4, &StaticOptions::default()), 6, Some(n1), "F4");
}

#[test]
fn f5_missing_node_and_component() {
    let mesh = uniaxial_mesh(ElementKind::Hex8);
    let n = mesh.nodes().len();
    let mut pres = three_two_one(&mesh, true);
    let model = model_of(mesh);
    let missing = n + 7;
    pres.push(Prescribed::new(missing, 0, 0.0));
    let e = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap_err();
    let msg = e.to_string();
    println!("F5 node: {msg}");
    assert!(matches!(e, FemError::MissingNode { node, .. } if node == missing));
    assert!(msg.contains("constraint") && msg.contains(&missing.to_string()));
    pres.pop();
    pres.push(Prescribed::new(0, 3, 0.0));
    let e = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap_err();
    println!("F5 component: {e}");
    assert!(matches!(
        e,
        FemError::MissingComponent {
            node: 0,
            component: 3,
            ..
        }
    ));
    pres.pop();
    let e = solve_static_with(
        &model,
        &pres,
        &[NodalForceAt::new(missing, [1.0, 0.0, 0.0])],
        &StaticOptions::default(),
    )
    .unwrap_err();
    println!("F5 force: {e}");
    assert!(e.to_string().contains(&missing.to_string()));
    assert!(matches!(e, FemError::MissingNode { .. }));
}

// ----------------------------------------------------------------------------------------------------------- extensions

#[test]
fn e1_to_e4_free_node_conflicts_loads_element_errors() {
    // E1: a node of no element.
    let base = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let n = base.nodes().len();
    let mut nodes = base.nodes().to_vec();
    nodes.push([5.0, 5.0, 5.0]);
    let mut m = model_of(rebuild(&base, nodes));
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let e = solve_static(&m, &StaticOptions::default()).unwrap_err();
    println!("E1 free: {e}");
    assert!(matches!(e, FemError::FreeNode { node, component: 0 } if node == n));
    let held: Vec<Prescribed> = (0..3).map(|c| Prescribed::new(n, c, 1e-3)).collect();
    let r = solve_static_with(&m, &held, &[], &StaticOptions::default()).unwrap();
    println!("E1 held: solved, displacement of node {n} {:?}", r.displacement(n));
    assert_eq!(r.displacement(n), &[1e-3, 1e-3, 1e-3]);
    // E2: the same equation twice.
    let mut m2 = model_of(base.clone());
    for c in 0..3 {
        m2.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    let node = m2.mesh().node_set("xmin").unwrap()[0];
    let same = [Prescribed::new(node, 0, 0.0)];
    assert!(solve_static_with(&m2, &same, &[], &StaticOptions::default()).is_ok());
    let diff = [Prescribed::new(node, 0, 1e-3)];
    let e = solve_static_with(&m2, &diff, &[], &StaticOptions::default()).unwrap_err();
    println!("E2: {e}");
    assert!(matches!(e, FemError::ConflictingConstraint { node: nn, component: 0, .. } if nn == node));
    // E3: an unsupported load. Gravity until ADR 0276 (F-375) built it; since then the thermal load, which ADR 0281
    // builds (edited by the lead on 2026-11-12, ADR 0276 remaining question 1, check X4).
    let mut m3 = m2.clone();
    m3.add_load(Load::temperature(vec![350.0; m3.mesh().nodes().len()], 293.15))
        .unwrap();
    let e = solve_static(&m3, &StaticOptions::default()).unwrap_err();
    println!("E3: {e}");
    assert!(matches!(
        e,
        FemError::UnsupportedLoad {
            load: "Temperature",
            ..
        }
    ));
    // E4: an element error located by the assembly.
    let m4 = model_of(hex_box([0.0; 3], [1.0, 1.0, 1.0], [3, 2, 2]).unwrap());
    let e = assemble_with(&m4, Some(2), |ctx| {
        if ctx.element == 5 {
            Err(FemError::InvalidValue {
                key: "test",
                reason: "forced failure".into(),
            })
        } else {
            element_for(ctx.kind).stiffness(ctx.coords, &ctx.material.elasticity_matrix()?)
        }
    })
    .unwrap_err();
    println!("E4: {e}");
    assert!(e.to_string().starts_with("element 5: "));
    assert!(matches!(e, FemError::Element { element: 5, .. }));
}

#[test]
fn e5_hinge_mechanism_is_refused_by_the_pivot_test() {
    // Cube A [0, 1]^3 and cube B [1, 2] x [0, 1] x [1, 2] sharing the edge x = 1, z = 1.
    let a = hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    let mut nodes = a.nodes().to_vec();
    let mut conn_b = Vec::new();
    let b = hex_box([1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
    for &v in b.blocks()[0].connectivity() {
        let x = b.nodes()[v];
        let found = nodes.iter().position(|p| *p == x);
        let id = match found {
            Some(i) => i,
            None => {
                nodes.push(x);
                nodes.len() - 1
            }
        };
        conn_b.push(id);
    }
    assert_eq!(nodes.len(), 14, "two nodes shared");
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, a.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Hex8, conn_b).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_node_set("clamp", a.node_set("xmin").unwrap().to_vec())
        .unwrap();
    let mut m = model_of(mesh);
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("clamp", c, 0.0)).unwrap();
    }
    let r = solve_static_with(
        &m,
        &[],
        &[NodalForceAt::new(13, [0.0, 0.0, 1.0])],
        &StaticOptions::default(),
    );
    match r {
        Err(e @ FemError::SingularStiffness { .. }) => {
            println!("E5: {e}");
        }
        other => panic!("E5: expected SingularStiffness, got {other:?}"),
    }
}

// ------------------------------------------------------------------------------------------- amendment 1: check R3

#[test]
fn r3_balance_against_independent_loads() {
    let centre = [0.5 * R_LENGTHS[0], 0.5 * R_LENGTHS[1], 0.5 * R_LENGTHS[2]];
    let diag = (R_LENGTHS.iter().map(|l| l * l).sum::<f64>()).sqrt();
    for kind in KINDS {
        let (model, forces) = reaction_case(kind);
        let n = model.mesh().nodes().len();
        let mut f_test = vec![0.0; 3 * n];
        for &node in model.mesh().node_set("zmax").unwrap() {
            for (k, v) in [100.0, -200.0, 300.0].into_iter().enumerate() {
                f_test[3 * node + k] += v;
            }
        }
        for p in &forces {
            for k in 0..3 {
                f_test[3 * p.node + k] += p.force[k];
            }
        }
        let f_lib = nodal_load_vector(&model, &forces).unwrap();
        let fmax = f_test.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let dev = f_lib.iter().zip(&f_test).fold(0.0f64, |m, (a, b)| m.max((a - b).abs())) / fmax;
        let res = solve_static_with(&model, &[], &forces, &StaticOptions::default()).unwrap();
        let (bf, bm) = balance(&res, model.mesh(), &f_test, centre, diag, true);
        println!(
            "R3 {kind:?}: load vector against the test's {dev:.3e} (limit 1e-15); balance with the test's loads: force {bf:.3e} \
             moment {bm:.3e} (limit 1e-10)"
        );
        assert!(dev <= 1e-15, "R3a {kind:?}: {dev:e}");
        assert!(bf <= 1e-10 && bm <= 1e-10, "R3b {kind:?}: {bf:e} {bm:e}");
    }
}
