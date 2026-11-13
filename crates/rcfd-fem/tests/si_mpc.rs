//! F-393 (ADR 0296): linear multi-point constraints by the transformation, rigid sets, elastic supports. Checks A1, L1,
//! Y1, W1, S1, Q1, Q2, R1 to R3, K1, F1, C1 to C4, T1, T2, E1 and the negative controls N1 to N6 of the registration (A2 is
//! `si_scaffold_bits` and `baseline_hashes`).

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::element::element_for;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::mpc::{self, solve_static_mpc, MpcSolution, ReferenceLoad, RowOrigin};
use rcfd_fem::solve::{solve_static, solve_static_with, CgPreconditioner, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, ElasticSupport, FaceRef, FemError, LinearTerm, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::skyline::SkylineLdlt;
use rcfd_linalg::CsrMatrix;

const E: f64 = 2.0e11;

fn material(nu: f64) -> Material {
    Material::isotropic_elastic(E, nu, 7850.0, 0.0).unwrap()
}

fn model_of(mesh: SolidMesh, nu: f64) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(nu)).unwrap();
    m
}

fn tet10_box(origin: Vec3, lengths: Vec3, div: [usize; 3]) -> SolidMesh {
    upgrade_to_tet10(&tet_box(origin, lengths, div).unwrap()).unwrap().0
}

/// The registered generator: 64-bit LCG, values in [-1, 1).
struct Lcg(u64);

impl Lcg {
    fn new() -> Lcg {
        Lcg(296)
    }
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        2.0 * ((self.0 >> 11) as f64 / (1u64 << 53) as f64) - 1.0
    }
}

/// The same mesh with node `i` moved to `f(i, x_i)`: blocks, node, element and surface sets kept.
fn remap<F: FnMut(usize, Vec3) -> Vec3>(mesh: &SolidMesh, mut f: F) -> SolidMesh {
    let nodes: Vec<Vec3> = mesh.nodes().iter().enumerate().map(|(i, &x)| f(i, x)).collect();
    let mut out = SolidMesh::new(nodes).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    let names: Vec<String> = mesh.node_set_names().map(String::from).collect();
    for n in names {
        out.add_node_set(&n, mesh.node_set(&n).unwrap().to_vec()).unwrap();
    }
    let names: Vec<String> = mesh.element_set_names().map(String::from).collect();
    for n in names {
        out.add_element_set(&n, mesh.element_set(&n).unwrap().to_vec()).unwrap();
    }
    let names: Vec<String> = mesh.surface_set_names().map(String::from).collect();
    for n in names {
        out.add_surface_set(&n, mesh.surface_set(&n).unwrap().to_vec()).unwrap();
    }
    out
}

fn inf(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn rel_diff(a: &[f64], b: &[f64]) -> f64 {
    let d: Vec<f64> = a.iter().zip(b).map(|(x, y)| x - y).collect();
    inf(&d) / inf(b)
}

fn mpc_solve(model: &SolidModel, loads: &[ReferenceLoad]) -> MpcSolution {
    solve_static_mpc(model, &[], &[], loads, &StaticOptions::default()).unwrap()
}

/// `max |K(i,j) - K(j,i)| / max |K|`; panics if the pattern is not symmetric.
fn asymmetry(k: &CsrMatrix) -> f64 {
    let get = |i: usize, j: usize| -> Option<f64> {
        let row = &k.col[k.row_ptr[i]..k.row_ptr[i + 1]];
        row.binary_search(&j).ok().map(|p| k.val[k.row_ptr[i] + p])
    };
    let mut d: f64 = 0.0;
    for i in 0..k.n {
        for p in k.row_ptr[i]..k.row_ptr[i + 1] {
            let j = k.col[p];
            let t = get(j, i).expect("symmetric pattern");
            d = d.max((k.val[p] - t).abs());
        }
    }
    d / inf(&k.val)
}

/// Five random admissible motions `T dq`: the largest `|rho . T dq| / (|rho| |T dq|)`.
fn work_ratio(s: &MpcSolution) -> f64 {
    let tr = &s.transformation;
    let mut rng = Lcg::new();
    let mut worst: f64 = 0.0;
    for _ in 0..5 {
        let dq: Vec<f64> = (0..tr.independent().len()).map(|_| rng.next()).collect();
        let mut dx = tr.expand(&dq);
        for (v, g) in dx.iter_mut().zip(tr.g()) {
            *v -= g;
        }
        let w: f64 = s.constraint_forces.iter().zip(&dx).map(|(a, b)| a * b).sum();
        worst = worst.max(w.abs() / (norm2(&s.constraint_forces) * norm2(&dx)));
    }
    worst
}

// ---------------------------------------------------------------------------------------------------------------- P

fn patch_field(x: Vec3) -> Vec3 {
    [
        1e-3 * (x[0] + 2.0 * x[1] - x[2]),
        1e-3 * (0.5 * x[0] - x[1] + 3.0 * x[2]),
        1e-3 * (-x[0] + x[1] + 0.7 * x[2]),
    ]
}

const P_ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const P_LENGTHS: Vec3 = [1.2, 1.0, 0.8];

fn patch_meshes() -> Vec<(&'static str, SolidMesh)> {
    vec![
        ("P-Hex8", hex_box(P_ORIGIN, P_LENGTHS, [3, 3, 3]).unwrap()),
        ("P-Tet10", tet10_box(P_ORIGIN, P_LENGTHS, [2, 2, 2])),
    ]
}

fn patch_prescribed(mesh: &SolidMesh) -> Vec<Prescribed> {
    let mut boundary: Vec<usize> = BOX_FACES
        .iter()
        .flat_map(|f| mesh.node_set(f).unwrap().to_vec())
        .collect();
    boundary.sort_unstable();
    boundary.dedup();
    let mut out = Vec::new();
    for a in boundary {
        let u = patch_field(mesh.nodes()[a]);
        for (c, &v) in u.iter().enumerate() {
            out.push(Prescribed::new(a, c, v));
        }
    }
    out
}

#[test]
fn a1_prescribed_only_keeps_the_basic_tier() {
    for (name, mesh) in patch_meshes() {
        let pres = patch_prescribed(&mesh);
        for gravity in [false, true] {
            let mut m = model_of(mesh.clone(), 0.3);
            if gravity {
                m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
            }
            assert!(!mpc::uses_transformation(&m), "{name}");
            let r = solve_static_with(&m, &pres, &[], &StaticOptions::default().threads(1)).unwrap();
            let fixed = prescribed_equations(&m, &pres).unwrap();
            let k = stiffness(&m).unwrap();
            let f = rcfd_fem::loads::nodal_load_vector(&m, &[]).unwrap();
            let sys = reduce(&k, &f, &fixed).unwrap();
            let uf = SkylineLdlt::factor(&sys.matrix).unwrap().solve(&sys.rhs);
            let u = sys.expand(&uf);
            let equal = r
                .displacement_values()
                .iter()
                .zip(&u)
                .all(|(a, b)| a.to_bits() == b.to_bits());
            println!("A1 {name} gravity {gravity}: bit-equal to the elimination: {equal}");
            assert!(equal, "A1 {name} gravity {gravity}");
        }
    }
}

#[test]
fn l1_prescribed_as_linear_constraints() {
    for (name, mesh) in patch_meshes() {
        let pres = patch_prescribed(&mesh);
        let base = model_of(mesh.clone(), 0.3);
        let up = solve_static_with(&base, &pres, &[], &StaticOptions::default())
            .unwrap()
            .displacement_values()
            .to_vec();
        for a in [1.0, 2.5] {
            let mut m = model_of(mesh.clone(), 0.3);
            for p in &pres {
                m.add_constraint(Constraint::linear(
                    vec![LinearTerm::new(p.node, p.component, a)],
                    a * p.value,
                ))
                .unwrap();
            }
            assert!(mpc::uses_transformation(&m));
            let ul = solve_static(&m, &StaticOptions::default()).unwrap();
            let d = rel_diff(ul.displacement_values(), &up);
            println!("L1 {name} coefficient {a}: |u_L - u_P|/|u_P| = {d:e}");
            assert!(d <= 1e-12, "L1 {name} {a}: {d:e}");
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------- S

const SIGMA: f64 = 1e6;

fn skew_model(angle_deg_constraint: f64) -> SolidModel {
    let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let base = hex_box([0.0; 3], [1.0, 0.5, 0.4], [4, 2, 2]).unwrap();
    let mesh = remap(&base, |_, x| [c * x[0] - s * x[1], s * x[0] + c * x[1], x[2]]);
    let (cc, ss) = (
        angle_deg_constraint.to_radians().cos(),
        angle_deg_constraint.to_radians().sin(),
    );
    let mut m = model_of(mesh, 0.3);
    let xmin = m.mesh().node_set("xmin").unwrap().to_vec();
    let ymin = m.mesh().node_set("ymin").unwrap().to_vec();
    for a in xmin {
        m.add_constraint(Constraint::linear(
            vec![LinearTerm::new(a, 0, cc), LinearTerm::new(a, 1, ss)],
            0.0,
        ))
        .unwrap();
    }
    for a in ymin {
        m.add_constraint(Constraint::linear(
            vec![LinearTerm::new(a, 0, -ss), LinearTerm::new(a, 1, cc)],
            0.0,
        ))
        .unwrap();
    }
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_load(Load::traction("xmax", [SIGMA * c, SIGMA * s, 0.0])).unwrap();
    m
}

fn skew_exact(m: &SolidModel) -> Vec<f64> {
    let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let nu = 0.3;
    let mut out = Vec::new();
    for x in m.mesh().nodes() {
        let x1 = c * x[0] + s * x[1];
        let x2 = -s * x[0] + c * x[1];
        let k = SIGMA / E;
        let u = [k * (x1 * c + nu * x2 * s), k * (x1 * s - nu * x2 * c), -k * nu * x[2]];
        out.extend_from_slice(&u);
    }
    out
}

#[test]
fn s1_skew_roller() {
    let m = skew_model(30.0);
    let s = mpc_solve(&m, &[]);
    let exact = skew_exact(&m);
    let d = rel_diff(s.result.displacement_values(), &exact);
    println!(
        "S1 skew roller: |u - u_exact|/|u_exact| = {d:e}; dropped {}",
        s.dropped.len()
    );
    // Y1 and W1 on S.
    let y = asymmetry(&s.reduced_matrix);
    let w = work_ratio(&s);
    println!("Y1 S: asymmetry {y:e}; W1 S: work ratio {w:e}");
    assert!(d <= 1e-10, "S1 {d:e}");
    assert!(y <= 1e-13, "Y1 S {y:e}");
    assert!(w <= 1e-12, "W1 S {w:e}");
    // The default solve dispatches to the same path.
    let r = solve_static(&m, &StaticOptions::default()).unwrap();
    assert_eq!(r.displacement_values(), s.result.displacement_values());
}

#[test]
fn n6_skew_roller_at_31_degrees_misses() {
    let m = skew_model(31.0);
    let s = mpc_solve(&m, &[]);
    let d = rel_diff(s.result.displacement_values(), &skew_exact(&m));
    println!("N6 normals at 31 degrees: measure {d:e}");
    assert!(d > 1e-6, "N6 {d:e}");
}

#[test]
fn t2_cg_agrees_with_direct() {
    let m = skew_model(30.0);
    let d = mpc_solve(&m, &[]);
    let cg = StaticOptions::with_solver(StaticSolver::ConjugateGradient {
        tolerance: 1e-12,
        max_iterations: 10_000,
    });
    let c = solve_static_mpc(&m, &[], &[], &[], &cg).unwrap();
    let r = rel_diff(c.result.displacement_values(), d.result.displacement_values());
    println!("T2 CG {} iterations: |u_cg - u_d|/|u_d| = {r:e}", c.report.iterations);
    assert!(r <= 1e-7, "T2 {r:e}");
}

#[test]
fn n3_preconditioned_cg_refused() {
    let m = skew_model(30.0);
    let o = StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::BlockJacobi,
        tolerance: 1e-10,
        max_iterations: 1000,
    });
    let e = solve_static(&m, &o).expect_err("N3");
    println!("N3: {e}");
    assert!(matches!(e, FemError::InvalidValue { key: "solver", .. }), "N3 {e:?}");
}

// ---------------------------------------------------------------------------------------------------------------- Q

const Q_L: Vec3 = [1.0, 0.8, 0.6];

fn q_eps() -> [[f64; 3]; 3] {
    let e = [[1.0, 0.2, -0.1], [0.2, -0.5, 0.3], [-0.1, 0.3, 0.8]];
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = 1e-3 * e[i][j];
        }
    }
    out
}

fn periodic_model() -> SolidModel {
    let base = hex_box([0.0; 3], Q_L, [3, 3, 3]).unwrap();
    let mut boundary = vec![false; base.nodes().len()];
    for f in BOX_FACES {
        for &a in base.node_set(f).unwrap() {
            boundary[a] = true;
        }
    }
    let mut rng = Lcg::new();
    let h = 0.2;
    let mesh = remap(&base, |i, x| {
        if boundary[i] {
            x
        } else {
            [
                x[0] + 0.15 * h * rng.next(),
                x[1] + 0.15 * h * rng.next(),
                x[2] + 0.15 * h * rng.next(),
            ]
        }
    });
    let mut mesh = mesh;
    mesh.add_node_set("corner", vec![0]).unwrap();
    let eps = q_eps();
    let mut m = model_of(mesh, 0.3);
    assert_eq!(m.mesh().nodes()[0], [0.0; 3]);
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("corner", c, 0.0)).unwrap();
    }
    for k in 0..3 {
        let hi = m.mesh().node_set(BOX_FACES[2 * k + 1]).unwrap().to_vec();
        let lo = m.mesh().node_set(BOX_FACES[2 * k]).unwrap().to_vec();
        for &a in &hi {
            let xa = m.mesh().nodes()[a];
            let b = *lo
                .iter()
                .find(|&&b| {
                    let xb = m.mesh().nodes()[b];
                    (0..3).filter(|&j| j != k).all(|j| xb[j].to_bits() == xa[j].to_bits())
                })
                .expect("partner on the min face");
            for i in 0..3 {
                m.add_constraint(Constraint::linear(
                    vec![LinearTerm::new(a, i, 1.0), LinearTerm::new(b, i, -1.0)],
                    eps[i][k] * Q_L[k],
                ))
                .unwrap();
            }
        }
    }
    m
}

#[test]
fn q1_q2_periodic_box() {
    let m = periodic_model();
    let s = mpc_solve(&m, &[]);
    let eps = q_eps();
    let emax = 1e-3;
    let lmax = 1.0;
    let mut du: f64 = 0.0;
    for (a, x) in m.mesh().nodes().iter().enumerate() {
        let u = s.result.displacement(a);
        for i in 0..3 {
            let ex: f64 = (0..3).map(|j| eps[i][j] * x[j]).sum();
            du = du.max((u[i] - ex).abs());
        }
    }
    let du = du / (emax * lmax);
    let want = [
        eps[0][0],
        eps[1][1],
        eps[2][2],
        2.0 * eps[0][1],
        2.0 * eps[1][2],
        2.0 * eps[2][0],
    ];
    let mut de: f64 = 0.0;
    let mesh = m.mesh();
    for e in 0..mesh.n_elements() {
        let el = element_for(mesh.element_kind(e));
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let ue: Vec<f64> = nodes.iter().flat_map(|&a| s.result.displacement(a).to_vec()).collect();
        for &xi in el.stiffness_rule().points() {
            let st = el.strain_at(&coords, &ue, xi).unwrap();
            for r in 0..6 {
                de = de.max((st[r] - want[r]).abs());
            }
        }
    }
    let de = de / emax;
    let dropped = s.dropped.len();
    let all_linear = s.dropped.iter().all(|d| matches!(d.origin, RowOrigin::Linear { .. }));
    let worst_residual = s
        .dropped
        .iter()
        .fold(0.0f64, |m, d| m.max(d.residual / d.scale.max(f64::MIN_POSITIVE)));
    println!(
        "Q1 strain {de:e}, displacement {du:e}; Q2 dropped {dropped} (all Linear: {all_linear}, worst residue {worst_residual:e})"
    );
    let y = asymmetry(&s.reduced_matrix);
    let w = work_ratio(&s);
    println!("Y1 Q: asymmetry {y:e}; W1 Q: work ratio {w:e}");
    assert!(de <= 1e-10, "Q1 strain {de:e}");
    assert!(du <= 1e-10, "Q1 displacement {du:e}");
    assert_eq!(dropped, 33, "Q2");
    assert!(all_linear, "Q2 origins");
    assert!(y <= 1e-13, "Y1 Q {y:e}");
    assert!(w <= 1e-12, "W1 Q {w:e}");
}

// ---------------------------------------------------------------------------------------------------------------- R

const R_L: f64 = 2.0;
const R_B: f64 = 0.1;
const R_REF: Vec3 = [2.0, 0.05, 0.05];

fn rigid_mesh() -> SolidMesh {
    tet10_box([0.0; 3], [R_L, R_B, R_B], [20, 2, 2])
}

fn clamp(m: &mut SolidModel, set: &str) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn rigid_model() -> SolidModel {
    let mut m = model_of(rigid_mesh(), 0.0);
    clamp(&mut m, "xmin");
    m.add_constraint(Constraint::rigid_set("xmax", R_REF)).unwrap();
    m
}

/// Resultant and moment (about the reference) of the constraint forces at the rigid set's nodes, against the applied ones.
fn resultant_errors(m: &SolidModel, s: &MpcSolution, f: Vec3, mo: Vec3) -> (f64, f64) {
    let set = m.mesh().node_set("xmax").unwrap();
    let dofs = m.dof_map();
    let (mut r, mut mm) = ([0.0; 3], [0.0; 3]);
    let mut rho_max: f64 = 0.0;
    for &a in set {
        let x = m.mesh().nodes()[a];
        let d = [x[0] - R_REF[0], x[1] - R_REF[1], x[2] - R_REF[2]];
        rho_max = rho_max.max((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt());
        let p: Vec<f64> = (0..3)
            .map(|c| s.constraint_forces[dofs.equation(a, c).unwrap()])
            .collect();
        for i in 0..3 {
            r[i] += p[i];
        }
        mm[0] += d[1] * p[2] - d[2] * p[1];
        mm[1] += d[2] * p[0] - d[0] * p[2];
        mm[2] += d[0] * p[1] - d[1] * p[0];
    }
    let n = |v: Vec3| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let er = n([r[0] - f[0], r[1] - f[1], r[2] - f[2]]) / n(f).max(n(mo) / rho_max);
    let em = n([mm[0] - mo[0], mm[1] - mo[1], mm[2] - mo[2]]) / n(mo).max(n(f) * rho_max);
    (er, em)
}

#[test]
fn r1_to_r3_rigid_end_of_a_bar() {
    let m = rigid_model();
    let area = R_B * R_B;
    let inertia = R_B.powi(4) / 12.0;
    // R-F.
    let f = [1e4, 0.0, 0.0];
    let sf = mpc_solve(&m, &[ReferenceLoad::new(3, f, [0.0; 3])]);
    let want = f[0] * R_L / (E * area);
    let tx = sf.references[0].translation[0];
    let r1 = (tx - want).abs() / want;
    let mut r1n: f64 = 0.0;
    for &a in m.mesh().node_set("xmax").unwrap() {
        r1n = r1n.max((sf.result.displacement(a)[0] - want).abs() / want);
    }
    let (rf_r, rf_m) = resultant_errors(&m, &sf, f, [0.0; 3]);
    // R-M.
    let mo = [0.0, 1e3, 0.0];
    let sm = mpc_solve(&m, &[ReferenceLoad::new(3, [0.0; 3], mo)]);
    let want_w = mo[1] * R_L / (E * inertia);
    let wy = sm.references[0].rotation[1];
    let r2 = (wy - want_w).abs() / want_w;
    let (rm_r, rm_m) = resultant_errors(&m, &sm, [0.0; 3], mo);
    println!(
        "R1 t_x {tx:e} (exact {want:e}): {r1:e}, nodes {r1n:e}; R2 w_y {wy:e} (exact {want_w:e}): {r2:e}; R3 F: \
         resultant {rf_r:e} moment {rf_m:e}; M: resultant {rm_r:e} moment {rm_m:e}"
    );
    let y = asymmetry(&sm.reduced_matrix);
    let (wf, wm) = (work_ratio(&sf), work_ratio(&sm));
    println!(
        "Y1 R: asymmetry {y:e}; W1 R-F {wf:e}, R-M {wm:e}; unknowns {}",
        sm.reduced_matrix.n
    );
    assert!(r1 <= 1e-10 && r1n <= 1e-10, "R1 {r1:e} {r1n:e}");
    assert!(r2 <= 1e-2, "R2 {r2:e}");
    for (what, v) in [
        ("F resultant", rf_r),
        ("F moment", rf_m),
        ("M resultant", rm_r),
        ("M moment", rm_m),
    ] {
        assert!(v <= 1e-10, "R3 {what} {v:e}");
    }
    assert!(y <= 1e-13, "Y1 R {y:e}");
    assert!(wf <= 1e-12 && wm <= 1e-12, "W1 R {wf:e} {wm:e}");
}

#[test]
fn t1_threads_do_not_change_the_bits() {
    let m = rigid_model();
    let loads = [ReferenceLoad::new(3, [0.0; 3], [0.0, 1e3, 0.0])];
    let a = solve_static_mpc(&m, &[], &[], &loads, &StaticOptions::default().threads(1)).unwrap();
    let b = solve_static_mpc(&m, &[], &[], &loads, &StaticOptions::default().threads(8)).unwrap();
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<u64>>();
    let same = bits(a.result.displacement_values()) == bits(b.result.displacement_values())
        && bits(&a.constraint_forces) == bits(&b.constraint_forces)
        && a.references == b.references;
    println!("T1 threads 1 and 8 bit-equal: {same}");
    assert!(same, "T1");
}

#[test]
fn e1_fill_of_the_rigid_set() {
    let m = rigid_model();
    let s = mpc_solve(&m, &[ReferenceLoad::new(3, [1e4, 0.0, 0.0], [0.0; 3])]);
    let mut c = model_of(rigid_mesh(), 0.0);
    clamp(&mut c, "xmin");
    let fixed = prescribed_equations(&c, &[]).unwrap();
    let k = stiffness(&c).unwrap();
    let f = vec![0.0; k.n];
    let kff = reduce(&k, &f, &fixed).unwrap().matrix;
    let env_r = SkylineLdlt::factor(&s.reduced_matrix).unwrap().envelope_size();
    let env_f = SkylineLdlt::factor(&kff).unwrap().envelope_size();
    println!(
        "E1 K_r: {} unknowns, {} nonzeros, envelope {env_r}; K_FF clamped only: {} unknowns, {} nonzeros, envelope {env_f}",
        s.reduced_matrix.n,
        s.reduced_matrix.val.len(),
        kff.n,
        kff.val.len()
    );
}

#[test]
fn n1_collinear_rigid_set_is_singular() {
    let mut mesh = rigid_mesh();
    let edge: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&a| {
            let x = mesh.nodes()[a];
            x[0] == R_L && x[1] == 0.0
        })
        .collect();
    println!("N1 edge nodes {edge:?}");
    mesh.add_node_set("edge", edge).unwrap();
    let mut m = model_of(mesh, 0.0);
    clamp(&mut m, "xmin");
    m.add_constraint(Constraint::rigid_set("edge", R_REF)).unwrap();
    let e = solve_static_mpc(
        &m,
        &[],
        &[],
        &[ReferenceLoad::new(3, [1e4, 0.0, 0.0], [0.0; 3])],
        &StaticOptions::default(),
    )
    .expect_err("N1");
    println!("N1: {e}");
    let msg = e.to_string();
    assert!(msg.contains("reference") && msg.contains("constraint 3"), "N1 {msg}");
}

// ---------------------------------------------------------------------------------------------------------------- K, F

#[test]
fn k1_bar_on_an_end_spring() {
    let (k, f, l, area) = (5e8, 1e4, 1.0, 0.01);
    let mut m = model_of(hex_box([0.0; 3], [l, 0.1, 0.1], [10, 1, 1]).unwrap(), 0.0);
    m.add_support(ElasticSupport::nodal_spring("xmin", 0, k / 4.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_load(Load::nodal_force("xmax", [f / 4.0, 0.0, 0.0])).unwrap();
    let s = mpc_solve(&m, &[]);
    let want = f * (1.0 / k + l / (E * area));
    let mut d: f64 = 0.0;
    for &a in m.mesh().node_set("xmax").unwrap() {
        d = d.max((s.result.displacement(a)[0] - want).abs() / want);
    }
    let spring: f64 = s.support_forces.iter().sum();
    let y = asymmetry(&s.reduced_matrix);
    println!("K1 tip {want:e}: {d:e}; support force sum {spring:e}; Y1 K asymmetry {y:e}");
    assert!(d <= 1e-10, "K1 {d:e}");
    assert!(y <= 1e-13, "Y1 K {y:e}");
}

fn foundation_model(mesh: SolidMesh, rollers: bool) -> SolidModel {
    let mut m = model_of(mesh, 0.3);
    m.add_support(ElasticSupport::foundation("zmin", 1e10)).unwrap();
    m.add_load(Load::pressure("zmax", 1e6)).unwrap();
    if rollers {
        m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    }
    m
}

#[test]
fn f1_block_on_a_foundation() {
    let (p, kf, h) = (1e6, 1e10, 0.5);
    let lengths = [1.0, 0.8, h];
    for (name, mesh) in [
        ("F-Hex8", hex_box([0.0; 3], lengths, [3, 3, 2]).unwrap()),
        ("F-Tet10", tet10_box([0.0; 3], lengths, [2, 2, 2])),
    ] {
        let m = foundation_model(mesh, true);
        let s = mpc_solve(&m, &[]);
        let (wb, wt) = (p / kf, p / kf + p * h / E);
        let (mut db, mut dt): (f64, f64) = (0.0, 0.0);
        for &a in m.mesh().node_set("zmin").unwrap() {
            db = db.max((s.result.displacement(a)[2] + wb).abs() / wb);
        }
        for &a in m.mesh().node_set("zmax").unwrap() {
            dt = dt.max((s.result.displacement(a)[2] + wt).abs() / wt);
        }
        let y = asymmetry(&s.reduced_matrix);
        println!("F1 {name}: bottom {db:e}, top {dt:e}; Y1 asymmetry {y:e}");
        assert!(db <= 1e-10 && dt <= 1e-10, "F1 {name} {db:e} {dt:e}");
        assert!(y <= 1e-13, "Y1 {name} {y:e}");
    }
}

#[test]
fn n2_foundation_without_rollers_is_singular() {
    let m = foundation_model(hex_box([0.0; 3], [1.0, 0.8, 0.5], [3, 3, 2]).unwrap(), false);
    let e = solve_static(&m, &StaticOptions::default()).expect_err("N2");
    println!("N2: {e}");
    assert!(
        matches!(e, FemError::SingularStiffness { .. }) || e.to_string().contains("singular"),
        "N2 {e:?}"
    );
}

// ---------------------------------------------------------------------------------------------------------------- C

fn cantilever() -> SolidModel {
    let mut m = model_of(hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 2, 2]).unwrap(), 0.3);
    clamp(&mut m, "xmin");
    m.add_load(Load::nodal_force("xmax", [0.0, 0.0, -100.0])).unwrap();
    m
}

/// Three nodes of `xmax`: the first, the middle and the last of the set.
fn tip_nodes(m: &SolidModel) -> (usize, usize, usize) {
    let s = m.mesh().node_set("xmax").unwrap();
    (s[0], s[s.len() / 2], s[s.len() - 1])
}

fn tie(a: usize, b: usize, ca: f64, cb: f64, value: f64) -> Constraint {
    Constraint::linear(vec![LinearTerm::new(a, 2, ca), LinearTerm::new(b, 2, cb)], value)
}

fn invalid_reason(r: Result<impl std::fmt::Debug, FemError>, what: &str) -> String {
    match r.expect_err(what) {
        FemError::InvalidValue {
            key: "constraint",
            reason,
        } => {
            println!("{what}: {reason}");
            reason
        }
        other => panic!("{what}: {other:?}"),
    }
}

#[test]
fn c1_redundant_constraint_dropped_with_a_report() {
    let base = cantilever();
    let (a, b, _) = tip_nodes(&base);
    let mut once = base.clone();
    once.add_constraint(tie(a, b, 1.0, -1.0, 0.0)).unwrap();
    let mut twice = once.clone();
    twice.add_constraint(tie(a, b, 1.0, -1.0, 0.0)).unwrap();
    let s1 = mpc_solve(&once, &[]);
    let s2 = mpc_solve(&twice, &[]);
    let d = rel_diff(s2.result.displacement_values(), s1.result.displacement_values());
    println!("C1 dropped {:?}; difference {d:e}", s2.dropped);
    assert!(s1.dropped.is_empty());
    assert_eq!(s2.dropped.len(), 1, "C1");
    assert_eq!(
        s2.dropped[0].origin.constraint(),
        Some(4),
        "C1 names the second constraint"
    );
    assert!(d <= 1e-12, "C1 {d:e}");
}

#[test]
fn c4_redundant_row_with_non_unit_coefficients() {
    let base = cantilever();
    let (a, b, _) = tip_nodes(&base);
    let (c, s) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
    let mut once = base.clone();
    once.add_constraint(tie(a, b, c, s, 0.0)).unwrap();
    let u1 = mpc_solve(&once, &[]).result.displacement_values().to_vec();
    for f in [3.7, 0.3, 1.9, 7.1, 1.0 / 3.0] {
        let mut m = once.clone();
        m.add_constraint(tie(a, b, f * c, f * s, 0.0)).unwrap();
        let r = mpc_solve(&m, &[]);
        let d = rel_diff(r.result.displacement_values(), &u1);
        println!("C4 factor {f}: dropped {:?}; difference {d:e}", r.dropped);
        assert_eq!(r.dropped.len(), 1, "C4 {f}");
        assert_eq!(r.dropped[0].origin.constraint(), Some(4), "C4 {f}");
        assert!(d <= 1e-12, "C4 {f} {d:e}");
    }
}

#[test]
fn c2_contradictory_constraints_refused() {
    let base = cantilever();
    let (a, b, _) = tip_nodes(&base);
    let mut m = base.clone();
    m.add_constraint(tie(a, b, 1.0, -1.0, 0.0)).unwrap();
    m.add_constraint(tie(a, b, 1.0, -1.0, 1e-3)).unwrap();
    let r = invalid_reason(solve_static(&m, &StaticOptions::default()), "C2 ties");
    assert!(
        r.contains("contradicts") && r.contains("constraint 4") && r.contains("constraint 3 (Linear)"),
        "C2 {r}"
    );
    let mut m = base.clone();
    let n = m.mesh().node_set("xmin").unwrap()[1];
    m.add_constraint(Constraint::linear(vec![LinearTerm::new(n, 0, 1.0)], 1e-3))
        .unwrap();
    let r = invalid_reason(solve_static(&m, &StaticOptions::default()), "C2 prescribed");
    let want = format!("the prescribed displacement of node {n} component 0");
    assert!(
        r.contains("contradicts") && r.contains("constraint 3") && r.contains(&want),
        "C2 {r}"
    );
}

#[test]
fn c3_closed_chains_refused() {
    let base = cantilever();
    let (a, b, c) = tip_nodes(&base);
    let mut m = base.clone();
    m.add_constraint(tie(a, b, 1.0, -1.0, 0.0)).unwrap();
    m.add_constraint(tie(b, a, 1.0, -1.0, 0.0)).unwrap();
    let r = invalid_reason(solve_static(&m, &StaticOptions::default()), "C3 two");
    assert!(
        r.contains("closes a chain") && r.contains("constraint 4") && r.contains("constraint 3"),
        "C3 {r}"
    );
    let mut m = base.clone();
    m.add_constraint(tie(a, b, 1.0, -1.0, 0.0)).unwrap();
    m.add_constraint(tie(b, c, 1.0, -1.0, 0.0)).unwrap();
    m.add_constraint(tie(c, a, 1.0, -1.0, 0.0)).unwrap();
    let r = invalid_reason(solve_static(&m, &StaticOptions::default()), "C3 three");
    for i in [3, 4, 5] {
        assert!(
            r.contains("closes a chain") && r.contains(&format!("constraint {i}")),
            "C3 three {r}"
        );
    }
}

#[test]
#[ignore = "record of ADR 0296: asserts the modal refusal of linear constraints, rigid sets and supports, retired by F-486 (ADR 0384); the modal analysis on the transformation is checked in si_modal_mpc"]
fn n4_modal_refuses_the_new_data() {
    let base = cantilever();
    let (a, b, _) = tip_nodes(&base);
    let mut m = base.clone();
    m.add_constraint(tie(a, b, 1.0, -1.0, 0.0)).unwrap();
    let e = modal_analysis(&m, &ModalOptions::new(2)).expect_err("N4 linear");
    println!("N4: {e}");
    assert!(
        matches!(e, FemError::NotYetSupported { variant: "Linear", .. }),
        "N4 {e:?}"
    );
    let mut m = base.clone();
    m.add_support(ElasticSupport::nodal_spring("xmax", 2, 1e6)).unwrap();
    let e = modal_analysis(&m, &ModalOptions::new(2)).expect_err("N4 spring");
    println!("N4: {e}");
    assert!(
        matches!(
            e,
            FemError::NotYetSupported {
                variant: "NodalSpring",
                ..
            }
        ),
        "N4 {e:?}"
    );
}

#[test]
#[ignore = "record of ADR 0296: asserts the refusal of a Cyclic constraint (ADR 0297), retired by F-394 (ADR 0386); its model is refused by the boundary match now, checked in si_cyclic R1d"]
fn n5_cyclic_still_refused() {
    let mut mesh = hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 2, 2]).unwrap();
    let faces = |name: &str, mesh: &SolidMesh| -> Vec<FaceRef> { mesh.surface_set(name).unwrap().to_vec() };
    let (lo, hi) = (faces("ymin", &mesh), faces("ymax", &mesh));
    mesh.add_surface_set("low", lo).unwrap();
    mesh.add_surface_set("high", hi).unwrap();
    let mut m = model_of(mesh, 0.3);
    clamp(&mut m, "xmin");
    let (a, b, _) = tip_nodes(&m);
    m.add_constraint(tie(a, b, 1.0, -1.0, 0.0)).unwrap();
    m.add_constraint(Constraint::cyclic("low", "high", [1.0, 0.0, 0.0], [0.0; 3], 12))
        .unwrap();
    let e = solve_static(&m, &StaticOptions::default()).expect_err("N5");
    println!("N5: {e}");
    assert!(
        matches!(
            e,
            FemError::NotYetSupported {
                variant: "Cyclic",
                package: "ADR 0297",
                ..
            }
        ),
        "N5 {e:?}"
    );
}
