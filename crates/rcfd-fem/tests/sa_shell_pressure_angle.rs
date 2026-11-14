//! F-540 (ADR 0461): the angle-weighted shell pressure (`Load::shell_pressure_angle_weighted`) on curved facet meshes. The
//! checks of the registration, in its order; every limit is the registered one and every prototype number is that of
//! `f540-scratch/diag*.log`.

use rcfd_core::vec3::{cross, dot, norm, Vec3};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::shell3;
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{BlockProperty, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

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
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.6e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

const E: f64 = 2.0e11;

fn material() -> Material {
    Material::isotropic_elastic(E, 0.3, 7850.0, 0.0).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

/// How a cell of the cylinder is meshed: `None` one Shell4, `Some(0)` two Shell3 cut along `q0-q2`, `Some(1)` along `q1-q3`.
type Kind<'a> = &'a dyn Fn(usize, usize) -> Option<usize>;

/// A model of Shell3 triangles and Shell4 quadrilaterals in two blocks, element set "all".
fn shell_model(nodes: Vec<Vec3>, tris: Vec<usize>, quads: Vec<usize>, t: f64) -> SolidModel {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let (mut count, mut block) = (0, 0);
    if !tris.is_empty() {
        count += tris.len() / 3;
        mesh.add_block(ElementKind::Shell3, tris).unwrap();
        mesh.set_block_property(block, BlockProperty::shell(t).unwrap()).unwrap();
        block += 1;
    }
    if !quads.is_empty() {
        count += quads.len() / 4;
        mesh.add_block(ElementKind::Shell4, quads).unwrap();
        mesh.set_block_property(block, BlockProperty::shell(t).unwrap()).unwrap();
    }
    mesh.add_element_set("all", (0..count).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m
}

/// The cylinder quarter of ADR 0454 arithmetic 13 (R = 1, H = 0.5, t = 0.01, nu = 0.3, p = 1e5, the symmetry conditions of its
/// test, the end z = H free) on `nq x nz` cells meshed by `kind`, the interior rows moved axially when `jitter` (ADR 0461
/// design 5), loaded by `load`. Returns `u_r / (p R^2 / (E t))` at every node and the load vector.
fn cylinder(nq: usize, nz: usize, kind: Kind, jitter: bool, load: fn(&str, f64) -> Load) -> (Vec<f64>, Vec<f64>) {
    let (r0, lh, t, p) = (1.0, 0.5, 0.01, 1e5);
    let idx = |i: usize, j: usize| (nz + 1) * i + j;
    let dz = lh / nz as f64;
    let mut nodes: Vec<Vec3> = Vec::new();
    for i in 0..=nq {
        let ph = PI / 2.0 * i as f64 / nq as f64;
        for j in 0..=nz {
            let mut z = lh * j as f64 / nz as f64;
            if jitter && j > 0 && j < nz {
                z += 0.3 * dz * (3.0 * PI * i as f64 / nq as f64).sin() * (PI * j as f64 / nz as f64).sin();
            }
            nodes.push([r0 * ph.cos(), r0 * ph.sin(), z]);
        }
    }
    let (mut tris, mut quads) = (Vec::new(), Vec::new());
    for i in 0..nq {
        for j in 0..nz {
            let q = [idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)];
            match kind(i, j) {
                None => quads.extend_from_slice(&q),
                Some(0) => tris.extend_from_slice(&[q[0], q[1], q[2], q[0], q[2], q[3]]),
                Some(_) => tris.extend_from_slice(&[q[0], q[1], q[3], q[1], q[2], q[3]]),
            }
        }
    }
    let mut m = shell_model(nodes.clone(), tris, quads, t);
    m.add_load(load("all", p)).unwrap();
    let mut pres = Vec::new();
    for j in 0..=nz {
        for comp in [1, 3, 5] {
            pres.push(Prescribed::new(idx(0, j), comp, 0.0));
        }
        for comp in [0, 4, 5] {
            pres.push(Prescribed::new(idx(nq, j), comp, 0.0));
        }
    }
    for i in 0..=nq {
        for comp in [2, 3, 4] {
            pres.push(Prescribed::new(idx(i, 0), comp, 0.0));
        }
    }
    let f = nodal_load_vector(&m, &[]).unwrap();
    let r = solve_static_with(&m, &pres, &[], &StaticOptions::default()).unwrap();
    let reference = p * r0 * r0 / (E * t);
    let ur = nodes
        .iter()
        .enumerate()
        .map(|(k, x)| {
            let u = r.displacement(k);
            (u[0] * x[0] + u[1] * x[1]) / r0 / reference
        })
        .collect();
    (ur, f)
}

fn range(v: &[f64]) -> (f64, f64) {
    v.iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| (lo.min(x), hi.max(x)))
}

/// `max |v / target - 1|`.
fn dev(v: &[f64], target: f64) -> f64 {
    v.iter().map(|&x| rel(x, target)).fold(0.0, f64::max)
}

const ONE: fn(usize, usize) -> Option<usize> = |_, _| Some(0);

#[test]
fn cy_angle_weighted_cylinder() {
    let mut c = Checks::new();
    let aw = Load::shell_pressure_angle_weighted as fn(&str, f64) -> Load;
    let def = Load::shell_pressure as fn(&str, f64) -> Load;
    let c128 = (PI / 128.0).cos();
    let c64 = (PI / 64.0).cos();

    let (ur, _) = cylinder(32, 16, &ONE, false, aw);
    let (lo, hi) = range(&ur);
    println!("A1 one-diagonal 32 x 16, angle-weighted: u_r / ref in [{lo:.10}, {hi:.10}]");
    c.at_most("A1 O1 mesh: max |u_r / ref - 1|", dev(&ur, 1.0), 1e-2);
    c.at_most("DE1 O1 mesh: max |u_r / ref / cos(pi/128) - 1|", dev(&ur, c128), 1e-7);
    c.at_most("P1 A1 min against the prototype", rel(lo, 0.9996988181), 1e-5);
    c.at_most("P1 A1 max against the prototype", rel(hi, 0.9996988195), 1e-5);

    let (ur, _) = cylinder(16, 16, &ONE, false, aw);
    c.at_most("DE2 16 x 16: max |u_r / ref / cos(pi/64) - 1|", dev(&ur, c64), 1e-7);

    let patterns: [(&str, Kind); 3] = [
        ("other diagonal", &|_, _| Some(1)),
        ("checkerboard", &|i, j| Some((i + j) % 2)),
        ("herringbone", &|i, _| Some(i % 2)),
    ];
    for (name, kind) in patterns {
        let (ur, _) = cylinder(32, 16, kind, false, aw);
        c.at_most(&format!("CY2 {name}: max |u_r / ref / cos(pi/128) - 1|"), dev(&ur, c128), 1e-7);
    }

    let (ur, _) = cylinder(16, 8, &|i, _| if i % 4 == 0 { None } else { Some(0) }, false, aw);
    c.at_most("CY3 mixed Shell3 + Shell4: max |u_r / ref / cos(pi/64) - 1|", dev(&ur, c64), 1e-7);

    let (ur, _) = cylinder(32, 16, &ONE, true, aw);
    let (lo, hi) = range(&ur);
    println!("CY4 jittered 32 x 16, angle-weighted: u_r / ref in [{lo:.10}, {hi:.10}]");
    c.at_most("CY4 jittered: max |u_r / ref - 1|", dev(&ur, 1.0), 1e-2);
    c.at_most("P1 CY4 min against the prototype", rel(lo, 0.9932465311), 1e-5);
    c.at_most("P1 CY4 max against the prototype", rel(hi, 1.0042112359), 1e-5);

    let (ur, _) = cylinder(32, 16, &ONE, false, def);
    let (lo, hi) = range(&ur);
    println!("NC1 one-diagonal 32 x 16, default: u_r / ref in [{lo:.10}, {hi:.10}]");
    c.at_most("NC1 default min against F-419's O1", rel(lo, 0.9027895620), 1e-8);
    c.at_most("NC1 default max against F-419's O1", rel(hi, 1.0950504406), 1e-8);

    let (ur, _) = cylinder(32, 16, &ONE, true, def);
    c.above("NC2 jittered, default: max |u_r / ref - 1|", dev(&ur, 1.0), 5e-2);
    c.finish();
}

#[test]
fn ld_n1_angle_weighted_loads() {
    let mut c = Checks::new();
    let p = 1e5;
    // LD1: the element of ADR 0454 arithmetic 15 in space.
    let q = rotation([1.0, -2.0, 2.0], 0.7);
    let origin = [0.3, -0.2, 0.5];
    let x: Vec<Vec3> = [[0.0, 0.0], [0.9, 0.15], [0.3, 0.7]]
        .iter()
        .map(|a| {
            let r = apply(&q, [a[0], a[1], 0.0]);
            [origin[0] + r[0], origin[1] + r[1], origin[2] + r[2]]
        })
        .collect();
    let fe = shell3::pressure_load_vector_angle_weighted(&x, p).unwrap();
    let n = cross(sub(x[1], x[0]), sub(x[2], x[0]));
    let area = 0.5 * norm(n);
    let mut err: f64 = 0.0;
    let mut moments: f64 = 0.0;
    for k in 0..3 {
        let s: f64 = (0..3).map(|a| fe[6 * a + k]).sum();
        err = err.max((s - p * area * n[k] / norm(n)).abs() / (p * area));
        for a in 0..3 {
            moments = moments.max(fe[6 * a + 3 + k].abs());
        }
    }
    c.at_most("LD1 sum of the nodal forces against p A e_3, relative", err, 1e-12);
    c.holds("LD1 nodal moments zero", moments == 0.0, &format!("max |m| = {moments:e}"));

    // LD2: the right isosceles triangle and an equilateral triangle.
    let x = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let fe = shell3::pressure_load_vector_angle_weighted(&x, p).unwrap();
    let want = [0.25, 0.125, 0.125];
    let mut err: f64 = 0.0;
    for a in 0..3 {
        err = err.max((fe[6 * a + 2] - p * want[a]).abs() / (p * 0.5 / 3.0));
        err = err.max(fe[6 * a].abs().max(fe[6 * a + 1].abs()) / (p * 0.5 / 3.0));
    }
    c.at_most("LD2 right isosceles: F_z = p (1/4, 1/8, 1/8)", err, 1e-12);
    let h = 3f64.sqrt() / 2.0;
    let x: Vec<Vec3> = [[0.0, 0.0], [1.0, 0.0], [0.5, h]]
        .iter()
        .map(|a| {
            let r = apply(&q, [a[0], a[1], 0.0]);
            [origin[0] + r[0], origin[1] + r[1], origin[2] + r[2]]
        })
        .collect();
    let fa = shell3::pressure_load_vector_angle_weighted(&x, p).unwrap();
    let fd = shell3::pressure_load_vector(&x, p).unwrap();
    let scale = p * 0.5 * h / 3.0;
    let err = (0..18).map(|i| (fa[i] - fd[i]).abs() / scale).fold(0.0, f64::max);
    c.at_most("LD2 equilateral against pressure_load_vector", err, 1e-12);

    // LD4: a Shell4 cylinder takes Shell4's own load in the new mode.
    let none = |_: usize, _: usize| None;
    let (_, fa) = cylinder(16, 8, &none, false, Load::shell_pressure_angle_weighted);
    let (_, fd) = cylinder(16, 8, &none, false, Load::shell_pressure);
    let same = fa.len() == fd.len() && fa.iter().zip(&fd).all(|(a, b)| a.to_bits() == b.to_bits());
    c.holds("LD4 Shell4 load vector equal bit for bit", same, &format!("{} entries", fa.len()));

    // N1: refusals.
    let mut m = SolidModel::new(hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap());
    let r = m.add_load(Load::shell_pressure_angle_weighted("all", 1.0));
    c.holds(
        "N1 on Hex8 elements refused",
        matches!(&r, Err(FemError::InvalidValue { key: "shell pressure", .. })),
        &format!("{r:?}"),
    );
    let tri = shell_model(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![0, 1, 2],
        vec![],
        0.01,
    );
    let mut m = tri.clone();
    let r = m.add_load(Load::shell_pressure_angle_weighted("all", f64::NAN));
    c.holds(
        "N1 non-finite pressure refused",
        matches!(&r, Err(FemError::InvalidValue { key: "pressure", .. })),
        &format!("{r:?}"),
    );
    let mut m = tri.clone();
    let r = m.add_load(Load::shell_pressure_angle_weighted("nowhere", 1.0));
    c.holds(
        "N1 unknown set refused",
        matches!(&r, Err(FemError::UnknownSet { .. })),
        &format!("{r:?}"),
    );
    let mut m = tri;
    let r = m.add_load(Load::shell_pressure_angle_weighted("all", 1.0));
    c.holds("N1 on Shell3 elements taken", r.is_ok(), &format!("{r:?}"));
    c.finish();
}

#[test]
fn ld3_simply_supported_plate_unchanged() {
    let mut c = Checks::new();
    let n = 32;
    let t = 0.01;
    let idx = |i: usize, j: usize| j * (n + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            nodes.push([i as f64 / n as f64, j as f64 / n as f64, 0.0]);
        }
    }
    let mut tris = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let q = [idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)];
            tris.extend_from_slice(&[q[0], q[1], q[2], q[0], q[2], q[3]]);
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
    let centre = idx(n / 2, n / 2);
    let mut w = Vec::new();
    for load in [Load::shell_pressure("all", 1000.0), Load::shell_pressure_angle_weighted("all", 1000.0)] {
        let mut m = shell_model(nodes.clone(), tris.clone(), vec![], t);
        m.add_load(load).unwrap();
        let r = solve_static_with(&m, &pres, &[], &StaticOptions::default()).unwrap();
        w.push(r.displacement(centre)[2]);
    }
    println!("LD3 centre deflection: default {:.12e}, angle-weighted {:.12e}", w[0], w[1]);
    c.at_most("LD3 |w_c(new) / w_c(default) - 1|", rel(w[1], w[0]), 1e-10);
    c.finish();
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn rotation(axis: Vec3, angle: f64) -> [[f64; 3]; 3] {
    let n = norm(axis);
    let a = [axis[0] / n, axis[1] / n, axis[2] / n];
    let (s, c) = angle.sin_cos();
    let k = [[0.0, -a[2], a[1]], [a[2], 0.0, -a[0]], [-a[1], a[0], 0.0]];
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let kk: f64 = (0..3).map(|m| k[i][m] * k[m][j]).sum();
            r[i][j] = f64::from(u8::from(i == j)) + s * k[i][j] + (1.0 - c) * kk;
        }
    }
    r
}

fn apply(q: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [dot(q[0], v), dot(q[1], v), dot(q[2], v)]
}
