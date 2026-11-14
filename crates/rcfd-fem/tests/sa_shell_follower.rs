//! F-573 (ADR 0487): the follower form of the shell pressure on the current mid-surface and its load stiffness. Checks S1s,
//! S3s, S4s, W1, K1 (the elements), C2a to C2d (the cylinder at a prescribed stretch), C1r and the ignored record C1 (the
//! cylinder by the Newton driver, the predicted miss of D0), S2s and B1s to B4s (ring buckling with shells). Each registered
//! run is made once; every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::follower::{buckling_factors, FollowerPressure};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::nonlinear::{solve_with, Contribution, DriverExtensions, NewtonOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::cell::RefCell;
use std::f64::consts::PI;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn record(&mut self, name: &str, value: f64) {
        println!("{name}: {value:.6e} (recorded)");
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const T: f64 = 0.01;

fn material() -> Material {
    Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap()
}

/// SplitMix64, a deterministic source only.
fn uniform(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

fn dense(n: usize, triplets: &[(usize, usize, f64)]) -> Vec<f64> {
    let mut k = vec![0.0; n * n];
    for &(r, c, v) in triplets {
        k[r * n + c] += v;
    }
    k
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

fn asymmetry(k: &[f64], n: usize) -> f64 {
    let mut a: f64 = 0.0;
    for i in 0..n {
        for j in 0..n {
            a = a.max((k[i * n + j] - k[j * n + i]).abs());
        }
    }
    a / max_abs(k)
}

/// A shell model of one block, every element in the set "all".
fn shell_mesh_model(kind: ElementKind, nodes: Vec<Vec3>, conn: Vec<usize>) -> SolidModel {
    let ne = conn.len() / if kind == ElementKind::Shell4 { 4 } else { 3 };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m
}

fn single(kind: ElementKind, nodes: Vec<Vec3>) -> SolidModel {
    let n = nodes.len();
    shell_mesh_model(kind, nodes, (0..n).collect())
}

/// Quads `(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)` of a grid of `(ni + 1) x (nj + 1)` nodes, node `j (ni + 1) + i`;
/// cut along the diagonal from the first to the third corner when `tri`.
fn grid_conn(ni: usize, nj: usize, tri: bool, wrap: bool) -> Vec<usize> {
    let cols = if wrap { ni } else { ni + 1 };
    let idx = |i: usize, j: usize| j * cols + (i % cols);
    let mut conn = Vec::new();
    for j in 0..nj {
        for i in 0..ni {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
            }
        }
    }
    conn
}

fn kind_of(tri: bool) -> ElementKind {
    if tri {
        ElementKind::Shell3
    } else {
        ElementKind::Shell4
    }
}

/// The 2 x 2 patch on the cylinder `r = 2`, `theta` in `[0, 0.5]`, `z` in `[0, 1]` (D2).
fn patch(tri: bool) -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..=2 {
        for i in 0..=2 {
            let th = 0.25 * i as f64;
            nodes.push([2.0 * th.cos(), 2.0 * th.sin(), 0.5 * j as f64]);
        }
    }
    shell_mesh_model(kind_of(tri), nodes, grid_conn(2, 2, tri, false))
}

fn warped4() -> SolidModel {
    single(
        ElementKind::Shell4,
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.06], [1.1, 0.9, 0.0], [-0.1, 1.0, 0.08]],
    )
}

fn tri3() -> SolidModel {
    single(
        ElementKind::Shell3,
        vec![[0.0, 0.0, 0.0], [1.0, 0.1, 0.05], [0.2, 0.9, -0.03]],
    )
}

/// The rotation of the Rodrigues vector `w` applied to `x`.
fn rotate(w: Vec3, x: Vec3) -> Vec3 {
    let th = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    let k = [w[0] / th, w[1] / th, w[2] / th];
    let kx = [
        k[1] * x[2] - k[2] * x[1],
        k[2] * x[0] - k[0] * x[2],
        k[0] * x[1] - k[1] * x[0],
    ];
    let kd = k[0] * x[0] + k[1] * x[1] + k[2] * x[2];
    let (c, s) = (th.cos(), th.sin());
    [
        x[0] * c + kx[0] * s + k[0] * kd * (1.0 - c),
        x[1] * c + kx[1] * s + k[1] * kd * (1.0 - c),
        x[2] * c + kx[2] * s + k[2] * kd * (1.0 - c),
    ]
}

const P_S: f64 = 3.0e7;

// ------------------------------------------------------------------------------------------------------- S1s, S3s, K1

#[test]
#[ignore = "run 1 record of ADR 0487: K1 tangent missed by the sign of zero of the test's reference (amendment 1); corrected in s1s_s3s_k1_corrected"]
fn s1s_s3s_k1_load_stiffness_against_central_differences_known_miss() {
    let mut ck = Checks::new();
    let mut seed = 0xF573_0487_u64;
    for (name, model, single_element) in [
        ("Shell4 element", warped4(), true),
        ("Shell3 element", tri3(), true),
        ("Shell4 patch", patch(false), false),
        ("Shell3 patch", patch(true), false),
    ] {
        let n = model.dof_map().n_equations();
        let fp = FollowerPressure::on_shells(&model, "all", P_S).unwrap();
        let u: Vec<f64> = (0..n).map(|_| 0.05 * (2.0 * uniform(&mut seed) - 1.0)).collect();
        let k = dense(n, &fp.load_stiffness(&u, 1.0).unwrap());
        let h = 1e-6;
        let mut kcd = vec![0.0; n * n];
        for j in 0..n {
            let mut up = u.clone();
            let mut um = u.clone();
            up[j] += h;
            um[j] -= h;
            let fpl = fp.force(&up, 1.0).unwrap();
            let fmi = fp.force(&um, 1.0).unwrap();
            for i in 0..n {
                kcd[i * n + j] = (fpl[i] - fmi[i]) / (2.0 * h);
            }
        }
        let diff: Vec<f64> = k.iter().zip(&kcd).map(|(a, b)| a - b).collect();
        ck.at_most(
            &format!("S1s {name} max |K - K_cd| / max |K|"),
            max_abs(&diff) / max_abs(&k),
            1e-6,
        );
        if single_element {
            ck.at_least(&format!("S3s {name} max |K - K^T| / max |K|"), asymmetry(&k, n), 1e-3);
            // K1: the contribution adds -f and -(K + K^T)/2.
            let f = fp.force(&u, 1.0).unwrap();
            let mut force = vec![0.0; n];
            let mut tangent = Vec::new();
            fp.add(&u, 1.0, &mut force, &mut tangent).unwrap();
            let same_f = force.iter().zip(&f).all(|(a, b)| a.to_bits() == (0.0 - b).to_bits());
            let kt = dense(n, &tangent);
            let mut same_k = true;
            for i in 0..n {
                for j in 0..n {
                    let want = -0.5 * (k[i * n + j] + k[j * n + i]);
                    same_k &= kt[i * n + j].to_bits() == want.to_bits();
                }
            }
            ck.holds(&format!("K1 {name} contribution force -f"), same_f, "bits");
            ck.holds(&format!("K1 {name} contribution tangent -(K + K^T)/2"), same_k, "bits");
        }
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- S4s, W1

#[test]
fn s4s_w1_against_the_dead_shell_pressure() {
    let mut ck = Checks::new();
    let w = [0.3, -0.5, 0.7];
    let flat4: Vec<Vec3> = [[0.0, 0.0, 0.0], [1.2, 0.1, 0.0], [1.0, 0.9, 0.0], [-0.1, 1.1, 0.0]]
        .iter()
        .map(|x| rotate(w, *x))
        .collect();
    let flat3: Vec<Vec3> = [[0.0, 0.0, 0.0], [1.0, 0.1, 0.0], [0.2, 0.9, 0.0]]
        .iter()
        .map(|x| rotate(w, *x))
        .collect();
    for (name, model) in [
        ("Shell4 flat", single(ElementKind::Shell4, flat4)),
        ("Shell3 flat", single(ElementKind::Shell3, flat3)),
    ] {
        let n = model.dof_map().n_equations();
        let fp = FollowerPressure::on_shells(&model, "all", P_S).unwrap();
        let ff = fp.force(&vec![0.0; n], 1.0).unwrap();
        let mut dead = model.clone();
        dead.add_load(Load::shell_pressure("all", P_S)).unwrap();
        let fd = nodal_load_vector(&dead, &[]).unwrap();
        let diff: Vec<f64> = ff.iter().zip(&fd).map(|(a, b)| a - b).collect();
        ck.at_most(
            &format!("S4s {name} max |f - f_dead| / max |f_dead|"),
            max_abs(&diff) / max_abs(&fd),
            1e-10,
        );
    }
    // W1: the warped Shell4 at u = 0.
    let model = warped4();
    let n = model.dof_map().n_equations();
    let dofs = model.dof_map();
    let fp = FollowerPressure::on_shells(&model, "all", P_S).unwrap();
    let ff = fp.force(&vec![0.0; n], 1.0).unwrap();
    let mut dead = model.clone();
    dead.add_load(Load::shell_pressure("all", P_S)).unwrap();
    let fd = nodal_load_vector(&dead, &[]).unwrap();
    let (mut rf, mut rd) = ([0.0; 3], [0.0; 3]);
    let mut nodal: f64 = 0.0;
    let mut fmax: f64 = 0.0;
    for node in 0..4 {
        for c in 0..3 {
            let eq = dofs.equation(node, c).unwrap();
            rf[c] += ff[eq];
            rd[c] += fd[eq];
            nodal = nodal.max((ff[eq] - fd[eq]).abs());
            fmax = fmax.max(fd[eq].abs());
        }
    }
    let d = ((rf[0] - rd[0]).powi(2) + (rf[1] - rd[1]).powi(2) + (rf[2] - rd[2]).powi(2)).sqrt();
    let m = (rd[0] * rd[0] + rd[1] * rd[1] + rd[2] * rd[2]).sqrt();
    println!("W1 resultant follower {rf:?}, dead {rd:?}");
    ck.at_most("W1 warped Shell4 |F - F_dead| / |F_dead|", d / m, 1e-10);
    ck.record(
        "W1 warped Shell4 nodal max |f_a - f_a,dead| / max |f_dead|",
        nodal / fmax,
    );
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- C2

const NC: usize = 64;
const NZ: usize = 4;
const LC: f64 = 0.4;

fn cylinder(tri: bool) -> SolidModel {
    let mut nodes = Vec::new();
    for k in 0..=NZ {
        for i in 0..NC {
            let th = 2.0 * PI * i as f64 / NC as f64;
            nodes.push([th.cos(), th.sin(), LC * k as f64 / NZ as f64]);
        }
    }
    shell_mesh_model(kind_of(tri), nodes, grid_conn(NC, NZ, tri, true))
}

#[test]
#[ignore = "run 1 record of ADR 0487: C2b Shell3 missed, the end rings of the triangulation carry a tangential force (amendment 1); corrected in c2_cylinder_corrected"]
fn c2_cylinder_at_a_prescribed_stretch_known_miss() {
    let mut ck = Checks::new();
    let (p, lambda) = (1.0e6, 1.5);
    for tri in [false, true] {
        let name = if tri { "Shell3" } else { "Shell4" };
        let model = cylinder(tri);
        let dofs = model.dof_map();
        let n = dofs.n_equations();
        let mut u = vec![0.0; n];
        for (node, x) in model.mesh().nodes().iter().enumerate() {
            u[dofs.equation(node, 0).unwrap()] = (lambda - 1.0) * x[0];
            u[dofs.equation(node, 1).unwrap()] = (lambda - 1.0) * x[1];
        }
        let fp = FollowerPressure::on_shells(&model, "all", p).unwrap();
        let f = fp.force(&u, 1.0).unwrap();
        let mut dead = model.clone();
        dead.add_load(Load::shell_pressure("all", p)).unwrap();
        let fd = nodal_load_vector(&dead, &[]).unwrap();
        let (mut sr, mut sd) = (0.0, 0.0);
        let (mut off, mut fmax): (f64, f64) = (0.0, 0.0);
        for (node, x) in model.mesh().nodes().iter().enumerate() {
            let th = x[1].atan2(x[0]);
            let (c, s) = (th.cos(), th.sin());
            let g = |v: &[f64], comp: usize| v[dofs.equation(node, comp).unwrap()];
            sr += g(&f, 0) * c + g(&f, 1) * s;
            sd += g(&fd, 0) * c + g(&fd, 1) * s;
            off = off.max((-g(&f, 0) * s + g(&f, 1) * c).abs()).max(g(&f, 2).abs());
            for comp in 0..3 {
                fmax = fmax.max(g(&f, comp).abs());
            }
        }
        let want = p * NC as f64 * lambda * LC * (2.0 * PI / NC as f64).sin();
        println!("C2 {name}: radial sum {sr:.12e}, closed form {want:.12e}, dead {sd:.12e}");
        ck.at_most(
            &format!("C2a {name} |sum f.e_r / (p N lambda R L sin(2 pi/N)) - 1|"),
            (sr / want - 1.0).abs(),
            1e-11,
        );
        ck.at_most(
            &format!("C2b {name} max |f.e_theta|, |f.e_z| / max |f|"),
            off / fmax,
            1e-11,
        );
        ck.at_most(
            &format!("C2c {name} |lambda sum f_dead.e_r / sum f.e_r - 1|"),
            (lambda * sd / sr - 1.0).abs(),
            1e-11,
        );
        ck.at_least(
            &format!("C2d {name} sum f.e_r / sum f_dead.e_r - 1"),
            sr / sd - 1.0,
            0.49,
        );
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- C1, C1r

const R: f64 = 1.0;
const BW: f64 = 0.05;
const NQ: usize = 24;

/// The quarter-ring strip of D3: 24 elements over the quarter, 1 across the width, the plane and the symmetry held.
fn strip(tri: bool) -> SolidModel {
    let mut nodes = Vec::new();
    for k in 0..=1 {
        for i in 0..=NQ {
            let th = 0.5 * PI * i as f64 / NQ as f64;
            nodes.push([R * th.cos(), R * th.sin(), BW * k as f64]);
        }
    }
    let mut m = shell_mesh_model(kind_of(tri), nodes, grid_conn(NQ, 1, tri, false));
    let mut mesh = m.mesh().clone();
    mesh.add_node_set("nodes", (0..2 * (NQ + 1)).collect()).unwrap();
    mesh.add_node_set("th0", vec![0, NQ + 1]).unwrap();
    mesh.add_node_set("th90", vec![NQ, 2 * NQ + 1]).unwrap();
    m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    for c in [2, 3, 4] {
        m.add_constraint(Constraint::displacement("nodes", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("th0", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("th0", 5, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("th90", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("th90", 5, 0.0)).unwrap();
    m
}

fn p_thin(lambda: f64) -> f64 {
    E * T * (lambda * lambda - 1.0) / (2.0 * R)
}

/// Records the displacement of every call with its load factor, then delegates.
struct Recorder<'a> {
    inner: &'a FollowerPressure,
    calls: RefCell<Vec<(f64, Vec<f64>)>>,
}

impl Contribution for Recorder<'_> {
    fn name(&self) -> &str {
        "recorder"
    }
    fn add(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        self.calls.borrow_mut().push((load_factor, u.to_vec()));
        self.inner.add(u, load_factor, force, tangent)
    }
}

fn mean_stretch(model: &SolidModel, u: &[f64]) -> f64 {
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes();
    let mut s = 0.0;
    for (node, x) in nodes.iter().enumerate() {
        let a = x[0] + u[dofs.equation(node, 0).unwrap()];
        let b = x[1] + u[dofs.equation(node, 1).unwrap()];
        s += (a * a + b * b).sqrt() / R;
    }
    s / nodes.len() as f64
}

#[test]
fn c1r_the_newton_driver_refuses_shells() {
    let mut ck = Checks::new();
    for (tri, want) in [(false, "Shell4"), (true, "Shell3")] {
        let m = strip(tri);
        let fp = FollowerPressure::on_shells(&m, "all", p_thin(1.51)).unwrap();
        let ext = DriverExtensions {
            contributions: vec![&fp],
            step_control: None,
        };
        let r = solve_with(&m, &[], &[], &NewtonOptions::new(20), ext);
        let detail = match &r {
            Ok(_) => "Ok".to_string(),
            Err(e) => format!("{e:?}"),
        };
        let ok = matches!(&r, Err(FemError::NotYetSupported { item: "element kind", variant, .. }) if *variant == want);
        ck.holds(&format!("C1r {want} strip refused by the Newton driver"), ok, &detail);
    }
    ck.finish();
}

#[test]
#[ignore = "predicted miss of ADR 0487 check C1 (D0: the Newton driver takes no shells), recorded in addendum 1"]
fn c1_cylinder_follower_internal_pressure_known_miss() {
    let mut ck = Checks::new();
    let m = strip(false);
    let p_ref = p_thin(1.51);
    let fp = FollowerPressure::on_shells(&m, "all", p_ref).unwrap();
    let rec = Recorder {
        inner: &fp,
        calls: RefCell::new(Vec::new()),
    };
    let ext = DriverExtensions {
        contributions: vec![&rec],
        step_control: None,
    };
    match solve_with(&m, &[], &[], &NewtonOptions::new(20), ext) {
        Err(e) => ck.holds("C1 the Newton driver solves the shell strip", false, &format!("{e:?}")),
        Ok(r) => {
            let calls = rec.calls.borrow();
            let mut last = 0.0;
            for s in &r.steps {
                let u = &calls
                    .iter()
                    .rev()
                    .find(|(l, _)| l.to_bits() == s.load_factor.to_bits())
                    .unwrap()
                    .1;
                let lam = mean_stretch(&m, u);
                let p = s.load_factor * p_ref;
                ck.at_most(
                    &format!("C1 step {} lambda {lam:.5}: |p/p_thin - 1|", s.step),
                    (p / p_thin(lam) - 1.0).abs(),
                    1e-2,
                );
                last = lam;
            }
            ck.at_least("C1 last stretch", last, 1.5);
        }
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- B1s to B4s, S2s

const P_B: f64 = 1.0e3;

/// Correlation of the radial displacement of every node with `cos 2 theta`.
fn cos2_correlation(model: &SolidModel, mode: &[f64]) -> f64 {
    let dofs = model.dof_map();
    let (mut uf, mut uu, mut ff) = (0.0, 0.0, 0.0);
    for (node, x) in model.mesh().nodes().iter().enumerate() {
        let th = x[1].atan2(x[0]);
        let ur = mode[dofs.equation(node, 0).unwrap()] * th.cos() + mode[dofs.equation(node, 1).unwrap()] * th.sin();
        let f = (2.0 * th).cos();
        uf += ur * f;
        uu += ur * ur;
        ff += f * f;
    }
    uf.abs() / (uu * ff).sqrt()
}

/// `(p_cr with, p_cr without, mode correlations)` of the strip under the external pressure `P_B`.
fn ring_buckling(ck: &mut Checks, tri: bool) -> (f64, f64, f64, f64) {
    let mut m = strip(tri);
    let base = m.clone();
    m.add_load(Load::shell_pressure("all", -P_B)).unwrap();
    let s = solve_static(&m, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&m, &s).unwrap();
    let fp = FollowerPressure::on_shells(&base, "all", -P_B).unwrap();
    if !tri {
        let n = base.dof_map().n_equations();
        let kp = CsrMatrix::from_triplets(n, fp.load_stiffness(&vec![0.0; n], 1.0).unwrap());
        let fixed = prescribed_equations(&base, &[]).unwrap();
        let red = reduce(&kp, &vec![0.0; n], &fixed).unwrap().matrix;
        let nf = red.n;
        let mut d = vec![0.0; nf * nf];
        for r in 0..nf {
            for k in red.row_ptr[r]..red.row_ptr[r + 1] {
                d[r * nf + red.col[k]] += red.val[k];
            }
        }
        ck.at_most("S2s reduced max |K_p - K_p^T| / max |K_p|", asymmetry(&d, nf), 1e-11);
    }
    let with = buckling_factors(&base, &pre, &[&fp], &BucklingOptions::new(2)).unwrap();
    let without = factors(&base, &pre, &BucklingOptions::new(2)).unwrap();
    println!(
        "B {}: factors with {:?}, without {:?}",
        if tri { "Shell3" } else { "Shell4" },
        with.factors,
        without.factors
    );
    (
        with.factors[0] * P_B,
        without.factors[0] * P_B,
        cos2_correlation(&base, &with.modes[0]),
        cos2_correlation(&base, &without.modes[0]),
    )
}

#[test]
fn b1s_b2s_b3s_b4s_s2s_ring_buckling_with_shells() {
    let mut ck = Checks::new();
    let ei_r3 = E * T * T * T / 12.0 / (R * R * R);
    let (p1, p2, c1, c2) = ring_buckling(&mut ck, false);
    println!(
        "B Shell4: p_cr/(E I/R^3) = {:.6} with, {:.6} without",
        p1 / ei_r3,
        p2 / ei_r3
    );
    ck.at_most(
        "B1s |p_cr/(3 E I/R^3) - 1| with the load stiffness",
        (p1 / (3.0 * ei_r3) - 1.0).abs(),
        2e-2,
    );
    ck.at_most(
        "B2s |p_cr/(4 E I/R^3) - 1| without",
        (p2 / (4.0 * ei_r3) - 1.0).abs(),
        3e-2,
    );
    ck.at_least("B3s cos 2 theta correlation, with", c1, 0.99);
    ck.at_least("B3s cos 2 theta correlation, without", c2, 0.99);
    let (q1, q2, d1, d2) = ring_buckling(&mut ck, true);
    ck.record("B4s Shell3 p_cr/(3 E I/R^3) - 1 with", q1 / (3.0 * ei_r3) - 1.0);
    ck.record("B4s Shell3 p_cr/(4 E I/R^3) - 1 without", q2 / (4.0 * ei_r3) - 1.0);
    ck.record("B4s Shell3 cos 2 theta correlation, with", d1);
    ck.record("B4s Shell3 cos 2 theta correlation, without", d2);
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- amendment 1

#[test]
fn s1s_s3s_k1_corrected() {
    let mut ck = Checks::new();
    let mut seed = 0xF573_0487_u64;
    for (name, model, single_element) in [
        ("Shell4 element", warped4(), true),
        ("Shell3 element", tri3(), true),
        ("Shell4 patch", patch(false), false),
        ("Shell3 patch", patch(true), false),
    ] {
        let n = model.dof_map().n_equations();
        let fp = FollowerPressure::on_shells(&model, "all", P_S).unwrap();
        let u: Vec<f64> = (0..n).map(|_| 0.05 * (2.0 * uniform(&mut seed) - 1.0)).collect();
        let k = dense(n, &fp.load_stiffness(&u, 1.0).unwrap());
        let h = 1e-6;
        let mut kcd = vec![0.0; n * n];
        for j in 0..n {
            let mut up = u.clone();
            let mut um = u.clone();
            up[j] += h;
            um[j] -= h;
            let fpl = fp.force(&up, 1.0).unwrap();
            let fmi = fp.force(&um, 1.0).unwrap();
            for i in 0..n {
                kcd[i * n + j] = (fpl[i] - fmi[i]) / (2.0 * h);
            }
        }
        let diff: Vec<f64> = k.iter().zip(&kcd).map(|(a, b)| a - b).collect();
        ck.at_most(
            &format!("S1s {name} max |K - K_cd| / max |K|"),
            max_abs(&diff) / max_abs(&k),
            1e-6,
        );
        if single_element {
            ck.at_least(&format!("S3s {name} max |K - K^T| / max |K|"), asymmetry(&k, n), 1e-3);
            let f = fp.force(&u, 1.0).unwrap();
            let mut force = vec![0.0; n];
            let mut tangent = Vec::new();
            fp.add(&u, 1.0, &mut force, &mut tangent).unwrap();
            let same_f = force.iter().zip(&f).all(|(a, b)| a.to_bits() == (0.0 - b).to_bits());
            let kt = dense(n, &tangent);
            let mut same_k = true;
            for i in 0..n {
                for j in 0..n {
                    let want = -0.5 * (k[i * n + j] + k[j * n + i]) + 0.0;
                    same_k &= (kt[i * n + j] + 0.0).to_bits() == want.to_bits();
                }
            }
            ck.holds(&format!("K1 {name} contribution force -f"), same_f, "bits");
            ck.holds(
                &format!("K1' {name} contribution tangent -(K + K^T)/2 (x + 0.0)"),
                same_k,
                "bits",
            );
        }
    }
    ck.finish();
}

#[test]
fn c2_cylinder_corrected() {
    let mut ck = Checks::new();
    let (p, lambda) = (1.0e6, 1.5);
    for tri in [false, true] {
        let name = if tri { "Shell3" } else { "Shell4" };
        let model = cylinder(tri);
        let dofs = model.dof_map();
        let n = dofs.n_equations();
        let mut u = vec![0.0; n];
        for (node, x) in model.mesh().nodes().iter().enumerate() {
            u[dofs.equation(node, 0).unwrap()] = (lambda - 1.0) * x[0];
            u[dofs.equation(node, 1).unwrap()] = (lambda - 1.0) * x[1];
        }
        let fp = FollowerPressure::on_shells(&model, "all", p).unwrap();
        let f = fp.force(&u, 1.0).unwrap();
        let mut dead = model.clone();
        dead.add_load(Load::shell_pressure("all", p)).unwrap();
        let fd = nodal_load_vector(&dead, &[]).unwrap();
        let (mut sr, mut sd, mut st) = (0.0, 0.0, 0.0);
        let (mut off, mut off_inner, mut fmax): (f64, f64, f64) = (0.0, 0.0, 0.0);
        for (node, x) in model.mesh().nodes().iter().enumerate() {
            let th = x[1].atan2(x[0]);
            let (c, s) = (th.cos(), th.sin());
            let g = |v: &[f64], comp: usize| v[dofs.equation(node, comp).unwrap()];
            sr += g(&f, 0) * c + g(&f, 1) * s;
            sd += g(&fd, 0) * c + g(&fd, 1) * s;
            let ft = -g(&f, 0) * s + g(&f, 1) * c;
            st += ft;
            let o = ft.abs().max(g(&f, 2).abs());
            off = off.max(o);
            let ring = node / NC;
            if ring > 0 && ring < NZ {
                off_inner = off_inner.max(o);
            }
            for comp in 0..3 {
                fmax = fmax.max(g(&f, comp).abs());
            }
        }
        let want = p * NC as f64 * lambda * LC * (2.0 * PI / NC as f64).sin();
        println!("C2 {name}: radial sum {sr:.12e}, closed form {want:.12e}, dead {sd:.12e}");
        ck.at_most(
            &format!("C2a {name} |sum f.e_r / (p N lambda R L sin(2 pi/N)) - 1|"),
            (sr / want - 1.0).abs(),
            1e-11,
        );
        if tri {
            ck.record(
                "C2b Shell3 all nodes max |f.e_theta|, |f.e_z| / max |f| (the end rings)",
                off / fmax,
            );
            ck.at_most(
                "C2b' Shell3 interior rings max |f.e_theta|, |f.e_z| / max |f|",
                off_inner / fmax,
                1e-11,
            );
            ck.at_most("C2b' Shell3 |sum f.e_theta| / max |f|", st.abs() / fmax, 1e-11);
        } else {
            ck.at_most(
                &format!("C2b {name} max |f.e_theta|, |f.e_z| / max |f|"),
                off / fmax,
                1e-11,
            );
        }
        ck.at_most(
            &format!("C2c {name} |lambda sum f_dead.e_r / sum f.e_r - 1|"),
            (lambda * sd / sr - 1.0).abs(),
            1e-11,
        );
        ck.at_least(
            &format!("C2d {name} sum f.e_r / sum f_dead.e_r - 1"),
            sr / sd - 1.0,
            0.49,
        );
    }
    ck.finish();
}
