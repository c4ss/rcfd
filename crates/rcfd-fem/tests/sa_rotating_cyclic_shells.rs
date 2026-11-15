//! F-647 (ADR 0561): shells on the rotating cyclic paths and the full rotor. Checks F1, CO1a to CO1d, MA, MB, MC, W, CA, OR,
//! X2, XF, XL, SM, B3 and the negative controls NC1 to NC4 of the registration (B1 and B2 are the existing tests and
//! `baseline_hashes`; B4 is `sa_rotating_cyclic_shells_default_bits.rs`).

use rcfd_fem::cyclic::{full_rotor, modal_analysis_harmonic, rotation_matrix, solve_static_harmonic, FullRotor};
use rcfd_fem::cyclic::{HarmonicLoad, Mat3};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::rotating::{coriolis_matrix, coriolis_matrix_with_shells, RotatingOptions, RotatingPart, RotatingSector};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElementKind, FaceRef, FemError, Load, Material, Ply, SolidMesh, SolidModel,
};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const N: usize = 6;
const NC: usize = 2;
const NA: usize = 3;
const T: f64 = 0.05;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
}

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
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

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn unit(a: Vec3) -> Vec3 {
    let l = dot(a, a).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

/// The point of the ADR 0556 D3 frustum at column `i` (of `N NC` around) and generator parameter `s`, radial offset `dr`.
fn point(i: usize, s: f64, dr: f64) -> Vec3 {
    let n = axis();
    let l = 5.0f64.sqrt();
    let ea = [2.0 / l, -1.0 / l, 0.0];
    let eb = cross(n, ea);
    let phi = 2.0 * PI * i as f64 / (N * NC) as f64;
    let r = 1.0 - 0.4 * s + dr;
    let z = 0.8 * s;
    let (sp, cp) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (cp * ea[k] + sp * eb[k]) + z * n[k])
}

fn modal_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes);
    o.tolerance = 1e-12;
    o.residual_test = ResidualTest::Backward;
    o
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

struct Record {
    fails: Vec<String>,
}

impl Record {
    fn new() -> Record {
        Record { fails: Vec::new() }
    }
    fn hold(&mut self, name: &str, ok: bool, detail: String) {
        println!("{name}: {detail} {}", met(ok));
        if !ok {
            self.fails.push(format!("{name}: {detail}"));
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        self.hold(name, value <= limit, format!("{value:.4e} (limit <= {limit:e})"));
    }
    fn done(self) {
        assert!(self.fails.is_empty(), "missed: {:#?}", self.fails);
    }
}

// ---------------------------------------------------------------------------------------------- the shell sector and ring

/// Nodes of a quad, cut into two triangles along the diagonal from its first node for Shell3.
fn push_element(conn: &mut Vec<usize>, kind: ElementKind, q: [usize; 4]) {
    match kind {
        ElementKind::Shell4 => conn.extend(q),
        _ => conn.extend([q[0], q[1], q[2], q[0], q[2], q[3]]),
    }
}

/// The shell mesh of ADR 0556: `columns` columns (`periodic`: closing on column 0), node `(i, a)` at `a columns + i`.
fn shell_mesh(kind: ElementKind, columns: usize, periodic: bool) -> SolidMesh {
    let id = |i: usize, a: usize| a * columns + i % columns;
    let mut nodes = Vec::new();
    for a in 0..=NA {
        for i in 0..columns {
            nodes.push(point(i, a as f64 / NA as f64, 0.0));
        }
    }
    let quads = if periodic { columns } else { columns - 1 };
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..quads {
            push_element(
                &mut conn,
                kind,
                [id(i, a), id(i + 1, a), id(i + 1, a + 1), id(i, a + 1)],
            );
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    let ne = mesh.blocks()[0].len();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("clamp", (0..columns).map(|i| id(i, 0)).collect())
        .unwrap();
    mesh
}

fn sector_node(i: usize, a: usize) -> usize {
    a * (NC + 1) + i
}

fn clamp(m: &mut SolidModel) {
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("clamp", comp, 0.0)).unwrap();
    }
}

fn shell_options() -> CyclicOptions {
    CyclicOptions::new().shells(true)
}

/// The D3 sector with node sets `low` and `high`, the clamp and the cyclic constraint with the shells option.
fn sector(kind: ElementKind) -> SolidModel {
    let mut mesh = shell_mesh(kind, NC + 1, false);
    mesh.add_node_set("low", (0..=NA).map(|a| sector_node(0, a)).collect())
        .unwrap();
    mesh.add_node_set("high", (0..=NA).map(|a| sector_node(NC, a)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        shell_options(),
    ))
    .unwrap();
    m
}

/// The reference ring: `full_rotor` of the sector, a node set `r<p>` on every node, the material and the clamp (D3).
fn ring_of(sector: &SolidModel) -> (FullRotor, SolidModel) {
    let rotor = full_rotor(sector).unwrap();
    let mut mesh = rotor.mesh().clone();
    for p in 0..mesh.nodes().len() {
        mesh.add_node_set(&format!("r{p}"), vec![p]).unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    clamp(&mut m);
    (rotor, m)
}

fn rot(j: usize) -> Mat3 {
    rotation_matrix(axis(), 2.0 * PI * j as f64 / N as f64)
}

fn apply(r: &Mat3, v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| r[i][0] * v[0] + r[i][1] * v[1] + r[i][2] * v[2])
}

fn phase(k: usize, j: usize) -> (f64, f64) {
    let t = 2.0 * PI * ((j * k) % N) as f64 / N as f64;
    (t.cos(), t.sin())
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn inf(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

// ---------------------------------------------------------------------------------------------- F1

fn f1(kind: ElementKind, rec: &mut Record) {
    let m = sector(kind);
    let (rotor, _) = ring_of(&m);
    let full = rotor.mesh();
    let formula = shell_mesh(kind, N * NC, true);
    let per_quad = if kind == ElementKind::Shell4 { 1 } else { 2 };
    let name = format!("F1 {kind:?}");
    rec.hold(
        &format!("{name} counts"),
        full.nodes().len() == 48 && full.n_elements() == 36 * per_quad && full.blocks().len() == 1,
        format!(
            "{} nodes, {} elements, {} blocks",
            full.nodes().len(),
            full.n_elements(),
            full.blocks().len()
        ),
    );
    let block = &full.blocks()[0];
    rec.hold(
        &format!("{name} kind and property"),
        block.kind() == kind && block.property() == Some(&BlockProperty::shell(T).unwrap()),
        format!("{:?}, {:?}", block.kind(), block.property()),
    );
    let clamp = full.node_set("clamp").map(|s| s.len());
    rec.hold(&format!("{name} clamp"), clamp == Some(12), format!("{clamp:?} nodes"));
    let fe = formula.n_elements();
    let mut used = vec![false; fe];
    let mut worst = 0.0f64;
    for e in 0..full.n_elements() {
        let xs: Vec<Vec3> = full.element_nodes(e).iter().map(|&p| full.nodes()[p]).collect();
        let (best, d) = (0..fe)
            .map(|g| {
                let d = formula
                    .element_nodes(g)
                    .iter()
                    .zip(&xs)
                    .map(|(&p, &x)| dist(formula.nodes()[p], x))
                    .fold(0.0f64, f64::max);
                (g, d)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        used[best] = true;
        worst = worst.max(d);
    }
    rec.hold(
        &format!("{name} every formula element matched once"),
        used.iter().all(|&u| u),
        format!("{} of {fe}", used.iter().filter(|&&u| u).count()),
    );
    rec.at_most(&format!("{name} node positions / 1 m"), worst, 1e-12);
}

#[test]
fn f1_full_rotor_copies_shell_blocks() {
    let mut rec = Record::new();
    f1(ElementKind::Shell4, &mut rec);
    f1(ElementKind::Shell3, &mut rec);
    rec.done();
}

// ---------------------------------------------------------------------------------------------- CO1

/// The plane of CO1: origin and two orthonormal in-plane directions (tilted against every axis).
fn plane() -> (Vec3, Vec3, Vec3) {
    let q0 = [0.1, 0.2, -0.3];
    let u1 = unit([1.0, -2.0, 0.5]);
    let w = [0.3, 0.4, 1.0];
    let u2 = unit(sub(w, u1.map(|c| c * dot(w, u1))));
    (q0, u1, u2)
}

fn in_plane(s: f64, t: f64) -> Vec3 {
    let (q0, u1, u2) = plane();
    [0, 1, 2].map(|k| q0[k] + s * u1[k] + t * u2[k])
}

/// One element of `kind` on the CO1 plane: the trapezoid `(0, 0), (0.6, 0), (0.5, 0.4), (0.1, 0.4)` (area 0.2) or the
/// triangle `(0, 0), (0.6, 0), (0.2, 0.5)` (area 0.15); `property` on its block. Returns the model, the area and `e_3`.
fn one_element(kind: ElementKind, property: BlockProperty) -> (SolidModel, f64, Vec3) {
    let (pts, area): (Vec<Vec3>, f64) = if kind == ElementKind::Shell4 {
        (
            vec![
                in_plane(0.0, 0.0),
                in_plane(0.6, 0.0),
                in_plane(0.5, 0.4),
                in_plane(0.1, 0.4),
            ],
            0.2,
        )
    } else {
        (vec![in_plane(0.0, 0.0), in_plane(0.6, 0.0), in_plane(0.2, 0.5)], 0.15)
    };
    let e3 = if kind == ElementKind::Shell4 {
        unit(cross(sub(pts[2], pts[0]), sub(pts[3], pts[1])))
    } else {
        unit(cross(sub(pts[1], pts[0]), sub(pts[2], pts[0])))
    };
    let n = pts.len();
    let mut mesh = SolidMesh::new(pts).unwrap();
    mesh.add_block(kind, (0..n).collect()).unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", vec![0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    (m, area, e3)
}

/// Uniform fields over the model's equations: translation `v` (components 0..2) and rotation `theta` (3..5).
fn uniform(m: &SolidModel, v: Vec3, theta: Vec3) -> Vec<f64> {
    let d = m.dof_map();
    let mut x = vec![0.0; d.n_equations()];
    for p in 0..m.mesh().nodes().len() {
        for c in 0..3 {
            x[d.equation(p, c).unwrap()] = v[c];
            x[d.equation(p, 3 + c).unwrap()] = theta[c];
        }
    }
    x
}

fn form(c: &CsrMatrix, x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(matvec(c, y)).map(|(a, b)| a * b).sum()
}

fn skew_exact(c: &CsrMatrix) -> bool {
    for i in 0..c.n {
        for p in c.row_ptr[i]..c.row_ptr[i + 1] {
            let j = c.col[p];
            let v = c.val[p];
            if i == j {
                if v != 0.0 {
                    return false;
                }
                continue;
            }
            let q = (c.row_ptr[j]..c.row_ptr[j + 1]).find(|&q| c.col[q] == i);
            match q {
                Some(q) if c.val[q] == -v => {}
                _ => return false,
            }
        }
    }
    true
}

fn co1(name: &str, kind: ElementKind, property: BlockProperty, moments: [f64; 3], rec: &mut Record) {
    let (m, area, e3) = one_element(kind, property);
    let a = axis();
    let c = coriolis_matrix_with_shells(&m, a, None).unwrap();
    let (v1, v2) = ([0.3, -1.1, 0.7], [-0.4, 0.2, 1.3]);
    let (t1, t2) = ([0.9, 0.5, -0.6], [-0.2, 1.4, 0.8]);
    let zero = [0.0; 3];
    let [m0, m1, m2] = moments;
    let ft = form(&c, &uniform(&m, v1, zero), &uniform(&m, v2, zero));
    let ft_ref = m0 * area * dot(v1, cross(a, v2));
    let fr = form(&c, &uniform(&m, zero, t1), &uniform(&m, zero, t2));
    let fr_ref = m2 * area * dot(cross(t1, e3), cross(a, cross(t2, e3)));
    let fx = form(&c, &uniform(&m, v1, zero), &uniform(&m, zero, t2));
    let fx_ref = m1 * area * dot(v1, cross(a, cross(t2, e3)));
    println!("{name}: v1 C v2 {ft:.12e} (ref {ft_ref:.12e}), t1 C t2 {fr:.12e} (ref {fr_ref:.12e}), v1 C t2 {fx:.12e} (ref {fx_ref:.12e})");
    rec.at_most(&format!("{name} translations"), (ft / ft_ref - 1.0).abs(), 1e-13);
    rec.at_most(&format!("{name} rotations"), (fr / fr_ref - 1.0).abs(), 1e-13);
    if m1 == 0.0 {
        let scale = (m0 * m2).sqrt() * area * dot(v1, v1).sqrt() * dot(t2, t2).sqrt();
        rec.at_most(&format!("{name} mixed (zero)"), fx.abs() / scale, 1e-13);
    } else {
        rec.at_most(&format!("{name} mixed"), (fx / fx_ref - 1.0).abs(), 1e-13);
    }
    rec.hold(&format!("CO1d {name} skew bit for bit"), skew_exact(&c), String::new());
}

#[test]
#[ignore = "known miss of ADR 0561 run 1 (commit 9ec638ba): every uniform-field form of CO1a, CO1b, CO1c was 0, relative error 1.0 against the limit 1e-13 (v1 C v2 refs 96.03, 72.02, 53.09; t1 C t2 refs -1.354e-3, -1.016e-3, -8.456e-4; layup mixed ref 9.678e-2); shell to_global symmetrised the skew matrix to zero; amendment 2; continues as co1_shell_coriolis_uniform_fields_after_amendment_2"]
fn co1_shell_coriolis_uniform_fields_known_miss() {
    co1_all();
}

#[test]
fn co1_shell_coriolis_uniform_fields_after_amendment_2() {
    co1_all();
}

fn co1_all() {
    let mut rec = Record::new();
    let homogeneous = [RHO * T, 0.0, RHO * T * T * T / 12.0];
    co1(
        "CO1a Shell4",
        ElementKind::Shell4,
        BlockProperty::shell(T).unwrap(),
        homogeneous,
        &mut rec,
    );
    co1(
        "CO1b Shell3",
        ElementKind::Shell3,
        BlockProperty::shell(T).unwrap(),
        homogeneous,
        &mut rec,
    );
    // CO1c: two plies; the mass moments by the closed form (zbar from -t/2 along e_3).
    let soft = Material::isotropic_elastic(7e10, 0.33, 2000.0, 0.0).unwrap();
    let plies = vec![Ply::new(mat(), 0.02, 0.0).unwrap(), Ply::new(soft, 0.03, 0.0).unwrap()];
    let layers = [(RHO, 0.02), (2000.0, 0.03)];
    let total: f64 = layers.iter().map(|l| l.1).sum();
    let mut z = -0.5 * total;
    let mut moments = [0.0; 3];
    for (rho, t) in layers {
        let zbar = z + 0.5 * t;
        moments[0] += rho * t;
        moments[1] += rho * t * zbar;
        moments[2] += rho * t * (zbar * zbar + t * t / 12.0);
        z += t;
    }
    println!("CO1c mass moments {moments:?}");
    co1(
        "CO1c Shell4 layup",
        ElementKind::Shell4,
        BlockProperty::layup(plies).unwrap(),
        moments,
        &mut rec,
    );
    // CO1d on the D3 sectors (the matrix of the sector's own whirl paths).
    for kind in [ElementKind::Shell4, ElementKind::Shell3] {
        let m = sector(kind);
        let s = RotatingSector::new(&m, &RotatingOptions::new(4).modal(modal_options(4))).unwrap();
        let c = s.part().coriolis().unwrap();
        rec.hold(
            &format!("CO1d {kind:?} sector skew bit for bit"),
            skew_exact(&c),
            String::new(),
        );
    }
    rec.done();
}

// ---------------------------------------------------------------------------------------------- the rotating checks

/// D4: the sector lists per harmonic (positive values, any order) against the ring list; per-harmonic maxima of `|s/r - 1|`.
fn compare(name: &str, sector: &[Vec<f64>], ring: &[f64], rec: &mut Record) -> f64 {
    let c = (1.0 - 1e-6)
        * sector
            .iter()
            .map(|l| l.iter().copied().fold(0.0f64, f64::max))
            .fold(f64::INFINITY, f64::min);
    let mut s: Vec<(f64, usize)> = sector
        .iter()
        .enumerate()
        .flat_map(|(k, l)| l.iter().copied().filter(|&v| v < c).map(move |v| (v, k)))
        .collect();
    s.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut r: Vec<f64> = ring.iter().copied().filter(|&v| v < c).collect();
    r.sort_by(f64::total_cmp);
    let largest = ring.iter().copied().fold(0.0f64, f64::max);
    rec.hold(
        &format!("{name} set-up: ring list complete"),
        largest >= c,
        format!("largest ring {largest:.6e}, cut-off {c:.6e}"),
    );
    rec.hold(
        &format!("{name} counts"),
        s.len() == r.len(),
        format!("{} sector, {} ring", s.len(), r.len()),
    );
    let mut per = vec![0.0f64; sector.len()];
    for ((v, k), w) in s.iter().zip(&r) {
        per[*k] = per[*k].max((v / w - 1.0).abs());
    }
    let worst = per.iter().copied().fold(0.0f64, f64::max);
    rec.hold(
        &format!("{name} per harmonic"),
        worst <= 1e-8,
        format!(
            "[{}] (limit <= 1e-8)",
            per.iter().map(|x| format!("{x:.3e}")).collect::<Vec<_>>().join(", ")
        ),
    );
    worst
}

#[derive(Clone, Copy, Debug)]
enum Effects {
    A,
    B,
    C,
}

fn options(modes: usize, e: Effects) -> RotatingOptions {
    let o = RotatingOptions::new(modes).modal(modal_options(modes));
    match e {
        Effects::A => o,
        Effects::B => o.spin_softening(false),
        Effects::C => o.stress_stiffening(false),
    }
}

/// The equations of the clamped ring nodes.
fn clamped(ring: &SolidModel) -> Vec<bool> {
    let d = ring.dof_map();
    let mut out = vec![false; d.n_equations()];
    for &p in ring.mesh().node_set("clamp").unwrap() {
        for c in 0..d.components(p) {
            out[d.equation(p, c).unwrap()] = true;
        }
    }
    out
}

fn rotating(kind: ElementKind, tag: char) {
    let mut rec = Record::new();
    let m = sector(kind);
    let (rotor, ring) = ring_of(&m);
    let (sd, rd) = (m.dof_map(), ring.dof_map());
    let rest: Vec<_> = (0..=N / 2)
        .map(|k| modal_analysis_harmonic(&m, &[], k, &modal_options(4)).unwrap())
        .collect();
    let w1 = rest
        .iter()
        .map(|h| h.eigenvalues[0])
        .fold(f64::INFINITY, f64::min)
        .sqrt();
    let speeds = [0.0, 0.2 * w1, 0.4 * w1];
    println!("{kind:?}: omega_1 {w1:.6e} rad/s, speeds {speeds:?}");
    let fixed = clamped(&ring);
    for e in [Effects::A, Effects::B, Effects::C] {
        let set = match e {
            Effects::A => "MA",
            Effects::B => "MB",
            Effects::C => "MC",
        };
        let sec = RotatingSector::new(&m, &options(4, e)).unwrap();
        let part = RotatingPart::new(&ring, axis(), ORIGIN, &options(24, e).shells(true)).unwrap();
        for (si, &w) in speeds.iter().enumerate() {
            let name = format!("{set}{tag} speed {si} ({w:.4e} rad/s)");
            let hs: Vec<_> = (0..=N / 2).map(|k| sec.modes(k, w).unwrap()).collect();
            let rr = part.modes(w).unwrap();
            let (Some(rr), true) = (rr.modes(), hs.iter().all(|h| h.modes().is_some())) else {
                rec.hold(&format!("{name} set-up: no loss of stiffness"), false, "loss".into());
                continue;
            };
            let lists: Vec<Vec<f64>> = hs
                .iter()
                .map(|h| h.modes().unwrap().eigenvalues.iter().map(|l| l.sqrt()).collect())
                .collect();
            let ring_list: Vec<f64> = rr.eigenvalues.iter().map(|l| l.sqrt()).collect();
            compare(&name, &lists, &ring_list, &mut rec);
            if let Effects::A = e {
                // OR and X2: the expanded modes in the ring's mass and operator at this speed.
                let keff = part.operator(w);
                let mass = part.mass();
                let mut phis = Vec::new();
                let mut x2 = 0.0f64;
                for h in &hs {
                    let h = h.modes().unwrap();
                    for (mode, &l) in h.modes.iter().zip(&h.eigenvalues) {
                        let phi = rotor.expand(&sd, &rd, h.harmonic, mode);
                        let kp = matvec(&keff, &phi);
                        let mp = matvec(mass, &phi);
                        let r: Vec<f64> = (0..phi.len())
                            .filter(|&i| !fixed[i])
                            .map(|i| kp[i] - l * mp[i])
                            .collect();
                        x2 = x2.max(inf(&r) / (inf(&kp) + l.abs() * inf(&mp)));
                        phis.push(phi);
                    }
                }
                let mut or = 0.0f64;
                for (i, p) in phis.iter().enumerate() {
                    let mp = matvec(mass, p);
                    for (j, q) in phis.iter().enumerate() {
                        let g: f64 = q.iter().zip(&mp).map(|(a, b)| a * b).sum();
                        let target = if i == j { 1.0 } else { 0.0 };
                        or = or.max((g - target).abs());
                    }
                }
                rec.at_most(&format!("OR{tag} speed {si} ({} modes)", phis.len()), or, 1e-8);
                rec.at_most(&format!("X2{tag} speed {si}"), x2, 1e-8);
            }
        }
        if let Effects::A = e {
            // CA: the Campbell table's eigenvalues are the values of the modes at each speed.
            let ca = sec.campbell(&[0, 1, 2, 3], &speeds).unwrap();
            let mut same = true;
            for (k, fam) in ca.families.iter().enumerate() {
                for (si, &w) in speeds.iter().enumerate() {
                    let mut t: Vec<u64> = fam
                        .table
                        .tracks
                        .iter()
                        .filter_map(|tr| tr.eigenvalues[si])
                        .map(f64::to_bits)
                        .collect();
                    let h = sec.modes(k, w).unwrap();
                    let mut v: Vec<u64> = h.modes().unwrap().values.iter().map(|x| x.to_bits()).collect();
                    t.sort_unstable();
                    v.sort_unstable();
                    same &= t == v;
                }
            }
            rec.hold(
                &format!("CA{tag} Campbell eigenvalues equal the modes' values"),
                same,
                String::new(),
            );
        }
    }
    // W: the whirl modes with the Coriolis matrix.
    let sec = RotatingSector::new(&m, &options(4, Effects::A).coriolis(true)).unwrap();
    let part = RotatingPart::new(
        &ring,
        axis(),
        ORIGIN,
        &options(24, Effects::A).coriolis(true).shells(true),
    )
    .unwrap();
    for (si, &w) in speeds.iter().enumerate() {
        let name = format!("W{tag} speed {si} ({w:.4e} rad/s)");
        let hs: Vec<_> = (0..=N / 2).map(|k| sec.whirl_modes(k, w).unwrap()).collect();
        let rr = part.whirl_modes(w).unwrap();
        let (Some(rr), true) = (rr.modes(), hs.iter().all(|h| h.modes().is_some())) else {
            rec.hold(&format!("{name} set-up: no loss of stiffness"), false, "loss".into());
            continue;
        };
        let lists: Vec<Vec<f64>> = hs
            .iter()
            .map(|h| h.modes().unwrap().modes.iter().map(|x| x.omega).collect())
            .collect();
        let ring_list: Vec<f64> = rr.modes.iter().map(|x| x.omega).collect();
        compare(&name, &lists, &ring_list, &mut rec);
    }
    // W0 (amendment 2): the Coriolis term is felt: k = 1 at the top speed, whirl against set A.
    let w2 = speeds[2];
    let plain = RotatingSector::new(&m, &options(4, Effects::A)).unwrap();
    let mut a: Vec<f64> = plain
        .modes(1, w2)
        .unwrap()
        .modes()
        .unwrap()
        .eigenvalues
        .iter()
        .map(|l| l.sqrt())
        .collect();
    let mut b: Vec<f64> = sec
        .whirl_modes(1, w2)
        .unwrap()
        .modes()
        .unwrap()
        .modes
        .iter()
        .map(|x| x.omega)
        .collect();
    a.sort_by(f64::total_cmp);
    b.sort_by(f64::total_cmp);
    let cut = (1.0 - 1e-6) * a.last().unwrap().min(*b.last().unwrap());
    let w0 = a
        .iter()
        .zip(&b)
        .filter(|(x, y)| **x < cut && **y < cut)
        .map(|(x, y)| (y / x - 1.0).abs())
        .fold(0.0f64, f64::max);
    rec.hold(
        &format!("W0{tag} Coriolis effect, k = 1, top speed"),
        w0 >= 1e-4,
        format!("{w0:.4e} (limit >= 1e-4)"),
    );
    // SM: smoke of the other paths on the same Coriolis matrix.
    let mut ok = true;
    let mut detail = String::new();
    for k in 0..=N / 2 {
        match sec.split_modes(k, w2) {
            Ok(r) => {
                ok &= !r.modes.modes.is_empty() && r.modes.modes.iter().all(|x| x.omega.is_finite() && x.omega > 0.0)
            }
            Err(e) => {
                ok = false;
                detail.push_str(&format!("split_modes({k}): {e:?}; "));
            }
        }
        match sec.whirl_state(k, w2) {
            Ok(r) => ok &= !r.state.modes.is_empty() && r.state.modes.iter().all(|x| x.omega.is_finite()),
            Err(e) => {
                ok = false;
                detail.push_str(&format!("whirl_state({k}): {e:?}; "));
            }
        }
    }
    match sec.whirl_campbell(&[0, 1, 2, 3], &speeds) {
        Ok(c) => ok &= c.families.len() == 4,
        Err(e) => {
            ok = false;
            detail.push_str(&format!("whirl_campbell: {e:?}"));
        }
    }
    rec.hold(&format!("SM{tag} split_modes, whirl_state, whirl_campbell"), ok, detail);
    rec.done();
}

#[test]
fn rotating_shell4_sector_against_the_full_ring() {
    rotating(ElementKind::Shell4, '4');
}

#[test]
fn rotating_shell3_sector_against_the_full_ring() {
    rotating(ElementKind::Shell3, '3');
}

// ---------------------------------------------------------------------------------------------- XF, XL

/// The force and moment of sector node `q` (ADR 0556 D4).
fn nodal_load(q: f64) -> (Vec3, Vec3) {
    (
        [
            1e3 * (1.3 * q + 0.2).sin(),
            1e3 * (0.7 * q + 0.5).cos(),
            1e3 * (2.1 * q + 1.1).sin(),
        ],
        [
            1e2 * (1.9 * q + 0.3).cos(),
            1e2 * (0.4 * q + 0.9).sin(),
            1e2 * (1.1 * q + 1.7).cos(),
        ],
    )
}

fn expansion(kind: ElementKind, tag: char) {
    let mut rec = Record::new();
    let m = sector(kind);
    let (rotor, ring0) = ring_of(&m);
    let (sd, rd) = (m.dof_map(), ring0.dof_map());
    let n = sd.n_equations();
    let (mut worst_f, mut worst_l) = (0.0f64, 0.0f64);
    for k in 1..=N / 2 {
        let doubled = 2 * k < N;
        let (mut fa, mut fb) = (vec![0.0; n], vec![0.0; n]);
        for a in 1..=NA {
            for i in 0..=NC {
                let q = sector_node(i, a);
                let (f, mo) = nodal_load(q as f64);
                let (g, go) = nodal_load(q as f64 + 7.0);
                for c in 0..3 {
                    fa[sd.equation(q, c).unwrap()] = f[c];
                    fa[sd.equation(q, 3 + c).unwrap()] = mo[c];
                    if doubled {
                        fb[sd.equation(q, c).unwrap()] = g[c];
                        fb[sd.equation(q, 3 + c).unwrap()] = go[c];
                    }
                }
            }
        }
        let load = rotor.expand_load(&sd, &rd, k, &fa, &fb);
        // XL: the hand sum over the sectors and every sector node.
        let mut hand = vec![0.0; rd.n_equations()];
        for j in 0..N {
            let r = rot(j);
            let (c, s) = phase(k, j);
            for p in 0..m.mesh().nodes().len() {
                let full = rotor.node(j, p);
                for base in [0, 3] {
                    let v: Vec3 = [0, 1, 2].map(|t| {
                        let e = sd.equation(p, base + t).unwrap();
                        c * fa[e] - s * fb[e]
                    });
                    let w = apply(&r, v);
                    for t in 0..3 {
                        hand[rd.equation(full, base + t).unwrap()] += w[t];
                    }
                }
            }
        }
        let dl: Vec<f64> = load.iter().zip(&hand).map(|(a, b)| a - b).collect();
        let xl = inf(&dl) / inf(&hand);
        println!("XL{tag} k = {k}: {xl:.4e}");
        worst_l = worst_l.max(xl);
        // XF: the sector solution expanded against the ring under the expanded load.
        let hs = solve_static_harmonic(
            &m,
            &[],
            &HarmonicLoad::new(k, fa.clone(), fb.clone()),
            &StaticOptions::default(),
        )
        .unwrap();
        let field = rotor.expand_field(&sd, &rd, k, &hs.cos, &hs.sin);
        let mut ring = ring0.clone();
        for p in 0..ring.mesh().nodes().len() {
            let f: Vec3 = [0, 1, 2].map(|t| load[rd.equation(p, t).unwrap()]);
            let mo: Vec3 = [0, 1, 2].map(|t| load[rd.equation(p, 3 + t).unwrap()]);
            ring.add_load(Load::nodal_force(&format!("r{p}"), f)).unwrap();
            ring.add_load(Load::nodal_moment(&format!("r{p}"), mo)).unwrap();
        }
        let ur = solve_static(&ring, &StaticOptions::default()).unwrap();
        let u = ur.displacement_values();
        let (mut du, mut dt, mut mu, mut mt) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for p in 0..ring.mesh().nodes().len() {
            for c in 0..6 {
                let e = rd.equation(p, c).unwrap();
                let d = (field[e] - u[e]).abs();
                if c < 3 {
                    du = du.max(d);
                    mu = mu.max(u[e].abs());
                } else {
                    dt = dt.max(d);
                    mt = mt.max(u[e].abs());
                }
            }
        }
        let xf = (du / mu).max(dt / mt);
        println!("XF{tag} k = {k}: e_u {:.4e}, e_theta {:.4e}", du / mu, dt / mt);
        worst_f = worst_f.max(xf);
    }
    rec.at_most(&format!("XF{tag} max over k"), worst_f, 1e-10);
    rec.at_most(&format!("XL{tag} max over k"), worst_l, 1e-14);
    rec.done();
}

#[test]
fn xf_xl_shell4_expand_field_and_load() {
    expansion(ElementKind::Shell4, '4');
}

#[test]
fn xf_xl_shell3_expand_field_and_load() {
    expansion(ElementKind::Shell3, '3');
}

// ---------------------------------------------------------------------------------------------- B3: the solid twin

const SPEEDS: [f64; 3] = [0.0, 200.0, 400.0];
const LOSS: f64 = -1.0;

/// The Hex8 twin sector of ADR 0556 B3: surface sets `low` / `high`, node sets `low_nodes` / `high_nodes`, `clamp`, `all`.
fn solid_sector() -> SolidMesh {
    let id = |i: usize, a: usize, w: usize| w * (NA + 1) * (NC + 1) + a * (NC + 1) + i;
    let mut nodes = Vec::new();
    for w in 0..2 {
        for a in 0..=NA {
            for i in 0..=NC {
                nodes.push(point(i, a as f64 / NA as f64, (w as f64 - 0.5) * 2.0 * T));
            }
        }
    }
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..NC {
            for w in 0..2 {
                conn.extend([id(i, a, w), id(i + 1, a, w), id(i + 1, a + 1, w), id(i, a + 1, w)]);
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = NA * NC;
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let col = |i: usize| -> Vec<usize> {
        let mut v: Vec<usize> = (0..2).flat_map(|w| (0..=NA).map(move |a| id(i, a, w))).collect();
        v.sort_unstable();
        v
    };
    for (name, i) in [("low", 0), ("high", NC)] {
        let nodes = col(i);
        let mut faces = Vec::new();
        for e in 0..ne {
            for f in 0..6 {
                let fr = FaceRef { element: e, face: f };
                if mesh.face_nodes(fr).iter().all(|p| nodes.binary_search(p).is_ok()) {
                    faces.push(fr);
                }
            }
        }
        mesh.add_surface_set(name, faces).unwrap();
        mesh.add_node_set(&format!("{name}_nodes"), nodes).unwrap();
    }
    let mut clamp: Vec<usize> = (0..2).flat_map(|w| (0..=NC).map(move |i| id(i, 0, w))).collect();
    clamp.sort_unstable();
    mesh.add_node_set("clamp", clamp).unwrap();
    mesh
}

fn solid_clamped(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    for comp in 0..3 {
        m.add_constraint(Constraint::displacement("clamp", comp, 0.0)).unwrap();
    }
    m
}

/// The values of B3 (and, with `shells = false`, of B4): the default options, or node sets with both shells options.
fn solid_values(shells: bool) -> Vec<f64> {
    let mut m = solid_clamped(solid_sector());
    if shells {
        m.add_constraint(Constraint::cyclic_with(
            "low_nodes",
            "high_nodes",
            axis(),
            ORIGIN,
            N,
            shell_options(),
        ))
        .unwrap();
    } else {
        m.add_constraint(Constraint::cyclic("low", "high", axis(), ORIGIN, N))
            .unwrap();
    }
    let mut out = Vec::new();
    let opts = RotatingOptions::new(3).modal(modal_options(3)).shells(shells);
    let s = RotatingSector::new(&m, &opts).unwrap();
    for k in 0..=N / 2 {
        for w in SPEEDS {
            match s.modes(k, w).unwrap().modes() {
                Some(h) => out.extend(&h.eigenvalues),
                None => out.push(LOSS),
            }
        }
    }
    let sw = RotatingSector::new(&m, &opts.clone().coriolis(true)).unwrap();
    for k in 0..=N / 2 {
        for w in SPEEDS {
            match sw.whirl_modes(k, w).unwrap().modes() {
                Some(r) => out.extend(r.modes.iter().map(|x| x.omega)),
                None => out.push(LOSS),
            }
        }
    }
    let rotor = full_rotor(&m).unwrap();
    out.extend(rotor.mesh().nodes().iter().flatten());
    let ring = solid_clamped(rotor.mesh().clone());
    let (sd, rd) = (m.dof_map(), ring.dof_map());
    let h = modal_analysis_harmonic(&m, &[], 1, &modal_options(3)).unwrap();
    out.extend(rotor.expand(&sd, &rd, 1, &h.modes[0]));
    let n = sd.n_equations();
    let fa: Vec<f64> = (0..n).map(|i| (0.37 * i as f64 + 0.1).sin()).collect();
    let fb: Vec<f64> = (0..n).map(|i| (0.53 * i as f64 + 0.2).cos()).collect();
    out.extend(rotor.expand_field(&sd, &rd, 1, &fa, &fb));
    out.extend(rotor.expand_load(&sd, &rd, 1, &fa, &fb));
    let part = RotatingPart::new(
        &ring,
        axis(),
        ORIGIN,
        &RotatingOptions::new(6)
            .modal(modal_options(6))
            .coriolis(true)
            .shells(shells),
    )
    .unwrap();
    match part.whirl_modes(400.0).unwrap().modes() {
        Some(r) => out.extend(r.modes.iter().map(|x| x.omega)),
        None => out.push(LOSS),
    }
    out
}

#[test]
fn b3_solid_twin_default_against_the_shells_options() {
    let a = solid_values(false);
    let b = solid_values(true);
    let same = a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits());
    println!("B3: {} values, equal bits: {same}", a.len());
    assert!(same, "B3");
}

// ---------------------------------------------------------------------------------------------- NC1 to NC4

#[test]
fn nc_refusals_without_the_options() {
    let mut rec = Record::new();
    // NC1: full_rotor of a Beam2 model with the shells option.
    let mut mesh = SolidMesh::new(vec![point(0, 0.5, 0.0), point(1, 0.5, 0.0), point(2, 0.5, 0.0)]).unwrap();
    mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2]).unwrap();
    let section = BeamSection::new(0.012, 3e-6, 5e-6, 4e-6, 0.85, 0.75).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section, axis()).unwrap())
        .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_node_set("low", vec![0]).unwrap();
    mesh.add_node_set("high", vec![2]).unwrap();
    let mut mb = SolidModel::new(mesh);
    mb.add_material("all", mat()).unwrap();
    mb.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        shell_options(),
    ))
    .unwrap();
    let r = full_rotor(&mb).map(|_| ());
    rec.hold(
        "NC1 full_rotor of Beam2 with the shells option",
        matches!(
            r,
            Err(FemError::UnsupportedKind {
                kind: "Beam2",
                operation: "full_rotor",
                ..
            })
        ),
        format!("{r:?}"),
    );
    // NC2 (amendment 1): the Hex8 twin sector with a disjoint Shell4 element, the default options.
    let twin = solid_sector();
    let nh = twin.nodes().len();
    let mut nodes = twin.nodes().to_vec();
    nodes.extend_from_slice(&[[3.0, 0.0, 0.0], [4.0, 0.0, 0.0], [4.0, 1.0, 0.0], [3.0, 1.0, 0.0]]);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, twin.blocks()[0].connectivity().to_vec())
        .unwrap();
    mesh.add_block(ElementKind::Shell4, vec![nh, nh + 1, nh + 2, nh + 3])
        .unwrap();
    mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
    let ne = twin.n_elements();
    mesh.add_element_set("all", (0..=ne).collect()).unwrap();
    for name in ["low", "high"] {
        mesh.add_surface_set(name, twin.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    let mut m2 = SolidModel::new(mesh);
    m2.add_material("all", mat()).unwrap();
    m2.add_constraint(Constraint::cyclic("low", "high", axis(), ORIGIN, N))
        .unwrap();
    let r = full_rotor(&m2).map(|_| ());
    rec.hold(
        "NC2 full_rotor of a Hex8 sector with a Shell4 element, default options",
        matches!(
            r,
            Err(FemError::UnsupportedKind {
                kind: "Shell4",
                operation: "full_rotor",
                ..
            })
        ),
        format!("{r:?}"),
    );
    // NC3: the ring as a part with the default rotating options: the whirl refuses the shell as before.
    let m = sector(ElementKind::Shell4);
    let (_, ring) = ring_of(&m);
    let part = RotatingPart::new(&ring, axis(), ORIGIN, &RotatingOptions::new(4).modal(modal_options(4))).unwrap();
    let r = part.whirl_modes(100.0).map(|_| ());
    let shell4 = |r: &Result<(), FemError>| match r {
        Err(FemError::Element { source, .. }) => {
            matches!(**source, FemError::NotYetSupported { variant: "Shell4", .. })
        }
        _ => false,
    };
    rec.hold("NC3 ring whirl with the default options", shell4(&r), format!("{r:?}"));
    // NC4: the public coriolis_matrix of the shell sector.
    let r = coriolis_matrix(&m, axis(), None).map(|_| ());
    rec.hold("NC4 coriolis_matrix of the shell sector", shell4(&r), format!("{r:?}"));
    rec.done();
}
