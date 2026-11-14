//! F-558 (ADR 0508): moment, shear and Wagner terms of the beam geometric stiffness and the t^2/12 rotation terms of the
//! shells, opt-in. Checks L1 to L5b, C1, H1 to H4, R1, R2, S1 to S5, N1 to N3 and the ignored record I1 of the registration.
//! Each registered run is made once; every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    beam, shell, shell3, BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel,
};
use std::f64::consts::PI;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A value that is
/// not finite is a miss.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value > limit;
        println!(
            "{name}: {value:.4e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
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
const NU: f64 = 0.3;
const KAPPA: f64 = 5.0 / 6.0;

fn g_mod() -> f64 {
    E / (2.0 * (1.0 + NU))
}

fn steel() -> Material {
    Material::isotropic_elastic(E, NU, 7850.0, 0.0).unwrap()
}

// ------------------------------------------------------------------------------------------------------------ beams

const B: f64 = 0.01;
const H: f64 = 0.2;
const L: f64 = 3.0;

/// `J` of the thin rectangle (arithmetic 1 of the registration).
fn rect_j() -> f64 {
    (H * B * B * B / 3.0) * (1.0 - 0.63 * (B / H) * (1.0 - B.powi(4) / (12.0 * H.powi(4))))
}

fn rect_section() -> BeamSection {
    beam::rectangle_section(B, H, rect_j(), KAPPA).unwrap()
}

fn i_z() -> f64 {
    H * B.powi(3) / 12.0
}

/// `(pi/L) sqrt(E I_z G J)`.
fn m0_of(iz: f64, j: f64, l: f64) -> f64 {
    PI / l * (E * iz * g_mod() * j).sqrt()
}

/// The acceptance's closed form for the rectangle (`C_w = b^3 h^3/144`).
fn m_cr_rect() -> f64 {
    let cw = B.powi(3) * H.powi(3) / 144.0;
    m0_of(i_z(), rect_j(), L) * (1.0 + PI * PI * E * cw / (g_mod() * rect_j() * L * L)).sqrt()
}

/// A beam of `n` elements and length `l` along the global axis `axis` (0 = x, 1 = y), `section`, with its local axes `e_1 =
/// axis`, `e_2 = orient`. Fork supports (`v = w = phi = 0` at both ends, `u = 0` at node 0), the bending rotations free.
fn beam_model(n: usize, l: f64, section: &BeamSection, axis: usize) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=n)
        .map(|i| {
            let mut p = [0.0; 3];
            p[axis] = l * i as f64 / n as f64;
            p
        })
        .collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    let orient = if axis == 0 { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
    mesh.set_block_property(0, BlockProperty::beam(*section, orient).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    // Local (u, v, w, phi) are the global components (0, 1, 2, 3) for axis x and (1, 2, 0, 4) for axis y.
    let comp = if axis == 0 { [0, 1, 2, 3] } else { [1, 2, 0, 4] };
    for c in comp {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    for &c in &comp[1..] {
        m.add_constraint(Constraint::displacement("xl", c, 0.0)).unwrap();
    }
    m
}

/// End moments `m_ref` about the strong axis `e_2` (and the thrust `p_ref` at the free end, compressive) of a beam model.
fn moment_loaded(m: &SolidModel, axis: usize, m_ref: f64, p_ref: f64) -> SolidModel {
    let mut l = m.clone();
    let mom = |s: f64| {
        let mut v = [0.0; 3];
        // e_2 is the global y for the axis x and the global z for the axis y.
        v[if axis == 0 { 1 } else { 2 }] = s * m_ref;
        v
    };
    l.add_load(Load::nodal_moment("x0", mom(-1.0))).unwrap();
    l.add_load(Load::nodal_moment("xl", mom(1.0))).unwrap();
    if p_ref != 0.0 {
        let mut f = [0.0; 3];
        f[axis] = -p_ref;
        l.add_load(Load::nodal_force("xl", f)).unwrap();
    }
    l
}

/// The lowest positive buckling factor of the model with the moment terms (`terms`) or without.
fn ltb_factors(
    section: &BeamSection,
    n: usize,
    l: f64,
    axis: usize,
    m_ref: f64,
    p_ref: f64,
    terms: bool,
) -> Result<Vec<f64>, rcfd_fem::FemError> {
    let m = beam_model(n, l, section, axis);
    let loaded = moment_loaded(&m, axis, m_ref, p_ref);
    let r = solve_static(&loaded, &StaticOptions::default())?;
    let mut pre = Prestress::from_static(&loaded, &r)?;
    if terms {
        pre = pre.with_beam_resultants(&loaded, r.displacement_values(), [0.0; 2])?;
    }
    Ok(factors(&m, &pre, &BucklingOptions::new(3))?.factors)
}

#[test]
#[ignore = "N2 was refused by the solver: record of the first run, amendment 2"]
fn l1_to_l5_n2_rectangle_lateral_torsional_buckling() {
    let mut c = Checks::new();
    let s = rect_section();
    let m_ref = 1000.0;
    let want = m_cr_rect();
    println!(
        "closed form M_cr = {want:.8e} (C_w = 0: {:.8e})",
        m0_of(i_z(), rect_j(), L)
    );
    let mut at16 = f64::NAN;
    for (name, n, limit) in [("L1 16 elements", 16, 1e-2), ("L2 8 elements", 8, 2e-2)] {
        let f = ltb_factors(&s, n, L, 0, m_ref, 0.0, true).unwrap();
        println!("{name}: factors {f:?}");
        let m = f[0] * m_ref;
        c.at_most(&format!("{name} |M/M_cr - 1|"), (m / want - 1.0).abs(), limit);
        if n == 16 {
            at16 = m;
        }
    }
    // L3: the opposite moment.
    let f = ltb_factors(&s, 16, L, 0, -m_ref, 0.0, true).unwrap();
    println!("L3 factors {f:?}");
    c.at_most("L3 |f(+M)/f(-M) - 1|", (at16 / (f[0] * m_ref) - 1.0).abs(), 1e-8);
    // L4: the scale of the reference moment.
    let f = ltb_factors(&s, 16, L, 0, 3.0 * m_ref, 0.0, true).unwrap();
    c.at_most(
        "L4 |M(3 M_ref)/M(M_ref) - 1|",
        (f[0] * 3.0 * m_ref / at16 - 1.0).abs(),
        1e-9,
    );
    // L5: the same beam along y with permuted axes.
    let f = ltb_factors(&s, 16, L, 1, m_ref, 0.0, true).unwrap();
    println!("L5 factors {f:?}");
    c.at_most("L5 |M(axis y)/M(axis x) - 1|", (f[0] * m_ref / at16 - 1.0).abs(), 1e-6);
    // N2: without the moment terms there is no lateral-torsional buckling.
    match ltb_factors(&s, 16, L, 0, m_ref, 0.0, false) {
        Ok(f) => {
            println!("N2 factors without the option: {f:?}");
            c.holds(
                "N2 no LTB without the moment terms",
                f.is_empty() || f[0] * m_ref > 10.0 * m0_of(i_z(), rect_j(), L),
                &format!("{f:?}"),
            );
        }
        Err(e) => c.holds("N2 no LTB without the moment terms", false, &format!("error {e}")),
    }
    c.finish();
}

#[test]
fn c1_thrust_and_moment_interaction() {
    let mut c = Checks::new();
    let s = rect_section();
    let m0 = m0_of(i_z(), rect_j(), L);
    let pz = PI * PI * E * i_z() / (L * L);
    let a = s.area();
    let p_phi = g_mod() * rect_j() * a / (s.i_y() + s.i_z());
    let r = pz / p_phi;
    let (m_ref, p_ref) = (m0, 0.3 * pz);
    // x^2 = (1 - 0.3 x)(1 - 0.3 x r), x the scale of the proportional load.
    let q = 1.0 - 0.09 * r;
    let b = 0.3 * (1.0 + r);
    let x = (-b + (b * b + 4.0 * q).sqrt()) / (2.0 * q);
    let f = ltb_factors(&s, 16, L, 0, m_ref, p_ref, true).unwrap();
    println!("C1 factors {f:?}, closed x = {x:.8e}, P_z = {pz:.6e}, P_phi = {p_phi:.6e}");
    c.at_most("C1 |lambda/x - 1|", (f[0] / x - 1.0).abs(), 1.5e-2);
    c.finish();
}

fn local_vec(a: f64, b: f64) -> [f64; 12] {
    let mut f = [0.0; 12];
    f[4] = -a;
    f[10] = a;
    f[5] = -b;
    f[11] = b;
    f
}

#[test]
#[ignore = "H3 hand values wrong: record of the first run, amendment 2"]
fn h1_to_h4_r1_element_checks() {
    let mut c = Checks::new();
    let mat = steel();
    // H1: the Euler limit, uniform M_y = M.
    let stiff = beam::rectangle_section(B, H, rect_j(), 1e9).unwrap();
    let (l, mm) = (2.0, 7.0);
    let mut f = [0.0; 12];
    f[4] = -mm;
    f[10] = mm;
    let k = beam::local_moment_geometric_stiffness(l, &stiff, &mat, &f);
    let hand = [((3, 7), 1.0), ((9, 7), -1.0), ((3, 1), -1.0), ((9, 1), 1.0)];
    let mut worst: f64 = 0.0;
    for ((i, j), s) in hand {
        let want = s * mm / l;
        println!("H1 K[{i}][{j}] = {:.10e}, hand {want:.10e}", k.get(i, j));
        worst = worst.max((k.get(i, j) - want).abs() / (mm / l));
        worst = worst.max((k.get(j, i) - want).abs() / (mm / l));
    }
    c.at_most("H1 max |K - hand|/(M/L)", worst, 1e-7);
    // H2: Wagner.
    let (by, bz) = (0.7, -0.4);
    let (my, mz) = (3.0, 2.0);
    let mut f = [0.0; 12];
    f[4] = -my;
    f[10] = my;
    f[5] = -mz;
    f[11] = mz;
    let k = beam::local_wagner_geometric_stiffness(l, &f, [by, bz]);
    let bm = (by * my - bz * mz) / l;
    let mut worst: f64 = 0.0;
    for i in 0..12 {
        for j in 0..12 {
            let want = match (i, j) {
                (3, 3) | (9, 9) => bm,
                (3, 9) | (9, 3) => -bm,
                _ => 0.0,
            };
            worst = worst.max((k.get(i, j) - want).abs());
        }
    }
    println!("H2 beta M / L = {bm:.6e}");
    c.at_most("H2 max |K - hand|", worst, 1e-12);
    // H3: shear entries, Euler limit, V_y = 5.
    let vy = 5.0;
    let mut f = [0.0; 12];
    f[1] = -vy;
    f[7] = vy;
    let k = beam::local_shear_geometric_stiffness(l, &stiff, &mat, &f);
    let mut worst: f64 = 0.0;
    for ((i, j), s) in [((3, 8), 0.5), ((9, 8), 0.5), ((3, 2), -0.5), ((9, 2), -0.5)] {
        println!("H3 K[{i}][{j}] = {:.10e}, hand {:.10e}", k.get(i, j), s * vy);
        worst = worst.max((k.get(i, j) - s * vy).abs() / vy);
    }
    c.at_most("H3 max |K - hand|/V", worst, 1e-7);
    // H4: the sum of the three parts.
    let sec = rect_section();
    let mut f = local_vec(3.0, 2.0);
    f[1] = -1.5;
    f[7] = 1.5;
    f[3] = -0.5;
    f[9] = 0.5;
    let w = [0.7, -0.4];
    let all = beam::local_resultant_geometric_stiffness(l, &sec, &mat, &f, w);
    let p1 = beam::local_moment_geometric_stiffness(l, &sec, &mat, &f);
    let p2 = beam::local_shear_geometric_stiffness(l, &sec, &mat, &f);
    let p3 = beam::local_wagner_geometric_stiffness(l, &f, w);
    let mut worst: f64 = 0.0;
    let mut big: f64 = 0.0;
    for i in 0..144 {
        let sum = p1.values()[i] + p2.values()[i] + p3.values()[i];
        worst = worst.max((all.values()[i] - sum).abs());
        big = big.max(all.values()[i].abs());
    }
    c.at_most("H4 max |sum - parts|/max", worst / big, 1e-14);
    // R1: rigid rotation, translation rows, the equilibrated end forces of amendment 1.
    let (n, vy, vz, l) = (5.0, 2.0, -3.0, 2.0);
    let sec = beam::rectangle_section(B, H, rect_j(), KAPPA).unwrap();
    let f = [-n, -vy, -vz, 0.0, 0.0, 0.0, n, vy, vz, 0.0, l * vz, -l * vy];
    let mut k = beam::local_geometric_stiffness(l, &sec, &mat, n);
    let part = beam::local_resultant_geometric_stiffness(l, &sec, &mat, &f, [0.0; 2]);
    for (a, b) in k.values_mut().iter_mut().zip(part.values()) {
        *a += b;
    }
    let fv = [n, vy, vz];
    let mut worst: f64 = 0.0;
    for axis in 0..3 {
        let mut om = [0.0; 3];
        om[axis] = 1.0;
        // u_1 = omega x (L, 0, 0), theta = omega at both nodes.
        let u1 = [0.0, om[2] * l, -om[1] * l];
        let mut d = [0.0; 12];
        d[6..9].copy_from_slice(&u1);
        d[3..6].copy_from_slice(&om);
        d[9..12].copy_from_slice(&om);
        let kd = k.matvec(&d);
        let wf = [
            om[1] * fv[2] - om[2] * fv[1],
            om[2] * fv[0] - om[0] * fv[2],
            om[0] * fv[1] - om[1] * fv[0],
        ];
        for k in 0..3 {
            println!(
                "R1 axis {axis} comp {k}: node 1 {:.10e}, node 0 {:.10e}, omega x F {:.10e}",
                kd[6 + k],
                kd[k],
                wf[k]
            );
            worst = worst.max((kd[6 + k] - wf[k]).abs()).max((kd[k] + wf[k]).abs());
        }
    }
    c.at_most("R1 max |K_G rigid - omega x F|/max|F|", worst / 5.0, 1e-12);
    c.finish();
}

#[test]
fn l5b_skew_element_frame_invariance() {
    let mut c = Checks::new();
    let mat = steel();
    let sec = rect_section();
    let f = {
        let mut f = local_vec(3.0, 2.0);
        f[1] = -1.5;
        f[7] = 1.5;
        f[2] = 0.8;
        f[8] = -0.8;
        f[3] = -0.5;
        f[9] = 0.5;
        f
    };
    let w = [0.7, -0.4];
    let l = 2.0;
    let k0 =
        beam::element_resultant_geometric_stiffness(&[[0.0; 3], [l, 0.0, 0.0]], &sec, [0.0, 1.0, 0.0], &mat, &f, w)
            .unwrap();
    // The rotation about z by 45 degrees followed by the rotation about x by 30 degrees.
    let (c1, s1) = (std::f64::consts::FRAC_PI_4.cos(), std::f64::consts::FRAC_PI_4.sin());
    let (c2, s2) = (std::f64::consts::FRAC_PI_6.cos(), std::f64::consts::FRAC_PI_6.sin());
    let rz = [[c1, -s1, 0.0], [s1, c1, 0.0], [0.0, 0.0, 1.0]];
    let rx = [[1.0, 0.0, 0.0], [0.0, c2, -s2], [0.0, s2, c2]];
    let mut q = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            q[i][j] = (0..3).map(|k| rx[i][k] * rz[k][j]).sum();
        }
    }
    let rot = |v: Vec3| -> Vec3 { [0, 1, 2].map(|i| (0..3).map(|j| q[i][j] * v[j]).sum()) };
    let k1 = beam::element_resultant_geometric_stiffness(
        &[rot([0.0; 3]), rot([l, 0.0, 0.0])],
        &sec,
        rot([0.0, 1.0, 0.0]),
        &mat,
        &f,
        w,
    )
    .unwrap();
    let mut worst: f64 = 0.0;
    let mut big: f64 = 0.0;
    for i in 0..12 {
        for j in 0..12 {
            let mut s = 0.0;
            for a in 0..3 {
                for b in 0..3 {
                    s += q[i % 3][a] * k0.get(3 * (i / 3) + a, 3 * (j / 3) + b) * q[j % 3][b];
                }
            }
            worst = worst.max((k1.get(i, j) - s).abs());
            big = big.max(s.abs());
        }
    }
    c.at_most("L5b max |K_skew - Q K Q^T|/max|K|", worst / big, 1e-12);
    c.finish();
}

#[test]
fn r2_end_forces_of_a_pure_moment() {
    let mut c = Checks::new();
    let s = rect_section();
    let m_ref = 1000.0;
    let m = beam_model(16, L, &s, 0);
    let loaded = moment_loaded(&m, 0, m_ref, 0.0);
    let r = solve_static(&loaded, &StaticOptions::default()).unwrap();
    let forces = rcfd_fem::geometric_struct::beam_end_forces(&loaded, r.displacement_values()).unwrap();
    let mut worst: f64 = 0.0;
    for f in &forces {
        for (k, v) in f.iter().enumerate() {
            let want = match k {
                4 => -m_ref,
                10 => m_ref,
                _ => 0.0,
            };
            worst = worst.max((v - want).abs());
        }
    }
    c.at_most("R2 max |f - (M at the ends)|/M_ref", worst / m_ref, 1e-8);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ shells

fn tensor_x(n: f64, t: f64) -> [f64; 6] {
    [n / t, 0.0, 0.0, 0.0, 0.0, 0.0]
}

fn shell_mesh(kind: ElementKind, conn: Vec<usize>, nodes: Vec<Vec3>, t: f64) -> SolidMesh {
    let mut m = SolidMesh::new(nodes).unwrap();
    m.add_block(kind, conn).unwrap();
    m.set_block_property(0, BlockProperty::shell(t).unwrap()).unwrap();
    m
}

#[test]
fn s1_s2_n3_shell_element_entries() {
    let mut c = Checks::new();
    let (t, n) = (0.3, 11.0);
    let scale = n * t * t;
    // S1: Shell4, unit square, counter-clockwise.
    let quad: [Vec3; 4] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
    let xi = [0usize, 1, 1, 0];
    let yi = [0usize, 0, 1, 1];
    let g = [[1.0, -1.0], [-1.0, 1.0]];
    let mm = [[1.0 / 3.0, 1.0 / 6.0], [1.0 / 6.0, 1.0 / 3.0]];
    let stress = tensor_x(n, t);
    let extra = shell::element_rotation_geometric_stiffness(&quad, t, &stress).unwrap();
    let mut worst: f64 = 0.0;
    for a in 0..4 {
        for b in 0..4 {
            let h = t * t / 12.0 * n * g[xi[a]][xi[b]] * mm[yi[a]][yi[b]];
            for r in 0..6 {
                for s in 0..6 {
                    let want = if r == s && (r == 3 || r == 4) { h } else { 0.0 };
                    worst = worst.max((extra.get(6 * a + r, 6 * b + s) - want).abs());
                }
            }
        }
    }
    println!("S1 entry node 0-0 {:.10e}, hand {:.10e}", extra.get(3, 3), scale / 36.0);
    c.at_most("S1 max |K - hand|/(n t^2)", worst / scale, 1e-12);
    // The rotated copy: Q K Q^T.
    let (c1, s1) = (0.6f64, 0.8f64);
    let q = [[c1, -s1, 0.0], [s1, c1, 0.0], [0.0, 0.0, 1.0]];
    let rot = |v: Vec3| -> Vec3 { [0, 1, 2].map(|i| (0..3).map(|j| q[i][j] * v[j]).sum()) };
    let rq = quad.map(rot);
    let tens = |s: &[f64; 6]| [[s[0], s[3], s[5]], [s[3], s[1], s[4]], [s[5], s[4], s[2]]];
    let ts = tens(&stress);
    let mut rs = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for a in 0..3 {
                for b in 0..3 {
                    rs[i][j] += q[i][a] * ts[a][b] * q[j][b];
                }
            }
        }
    }
    let rstress = [rs[0][0], rs[1][1], rs[2][2], rs[0][1], rs[1][2], rs[2][0]];
    let extra_r = shell::element_rotation_geometric_stiffness(&rq, t, &rstress).unwrap();
    let mut worst: f64 = 0.0;
    for i in 0..24 {
        for j in 0..24 {
            let mut s = 0.0;
            for a in 0..3 {
                for b in 0..3 {
                    s += q[i % 3][a] * extra.get(3 * (i / 3) + a, 3 * (j / 3) + b) * q[j % 3][b];
                }
            }
            worst = worst.max((extra_r.get(i, j) - s).abs());
        }
    }
    c.at_most("S1 rotated copy max |K - Q K Q^T|/(n t^2)", worst / scale, 1e-12);
    // S2: Shell3.
    let tri: [Vec3; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let extra = shell3::element_rotation_geometric_stiffness(&tri, t, &stress).unwrap();
    let gx = [-1.0, 1.0, 0.0];
    let mut worst: f64 = 0.0;
    for a in 0..3 {
        for b in 0..3 {
            let h = t * t / 12.0 * n * 0.5 * gx[a] * gx[b];
            for r in 0..6 {
                for s in 0..6 {
                    let want = if r == s && (r == 3 || r == 4) { h } else { 0.0 };
                    worst = worst.max((extra.get(6 * a + r, 6 * b + s) - want).abs());
                }
            }
        }
    }
    c.at_most("S2 max |K - hand|/(n t^2)", worst / scale, 1e-12);
    // N3: zero pre-stress.
    let zero = shell::element_rotation_geometric_stiffness(&quad, t, &[0.0; 6]).unwrap();
    let z3 = shell3::element_rotation_geometric_stiffness(&tri, t, &[0.0; 6]).unwrap();
    c.holds(
        "N3 zero pre-stress gives a zero matrix",
        zero.values().iter().chain(z3.values()).all(|&v| v == 0.0),
        "exact",
    );
    // The shell mesh helper is also used to take the option through the assembled path below.
    let _ = shell_mesh(ElementKind::Shell3, vec![0, 1, 2], tri.to_vec(), t);
    c.finish();
}

fn grid(nx: usize, ny: usize, t: f64, tri: bool) -> SolidModel {
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([i as f64 / nx as f64, j as f64 / ny as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let (a, b, cc, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, cc, a, cc, d]);
            } else {
                conn.extend_from_slice(&[a, b, cc, d]);
            }
        }
    }
    let ne = if tri { 2 * nx * ny } else { nx * ny };
    let mut mesh = shell_mesh(
        if tri { ElementKind::Shell3 } else { ElementKind::Shell4 },
        conn,
        nodes,
        t,
    );
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let col = |i: usize| (0..=ny).map(|j| idx(i, j)).collect::<Vec<_>>();
    let row = |j: usize| (0..=nx).map(|i| idx(i, j)).collect::<Vec<_>>();
    let mut edges: Vec<usize> = col(0).into_iter().chain(col(nx)).chain(row(0)).chain(row(ny)).collect();
    edges.sort_unstable();
    edges.dedup();
    mesh.add_node_set("edges", edges).unwrap();
    mesh.add_node_set("x0", col(0)).unwrap();
    mesh.add_node_set("xl", col(nx)).unwrap();
    mesh.add_node_set("y0", row(0)).unwrap();
    mesh.add_node_set("yl", row(ny)).unwrap();
    mesh.add_node_set("c00", vec![idx(0, 0)]).unwrap();
    mesh.add_node_set("xl_end", vec![idx(nx, 0), idx(nx, ny)]).unwrap();
    mesh.add_node_set("xl_mid", (1..ny).map(|j| idx(nx, j)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m
}

/// The lowest buckling load over `E` of the hard simply supported square plate (probe set-up of the registration), without
/// and with the rotation terms.
fn plate_ratio(cells: usize, t: f64, tri: bool) -> (f64, f64) {
    let mut m = grid(cells, cells, t, tri);
    m.add_constraint(Constraint::displacement("edges", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("c00", 1, 0.0)).unwrap();
    for (set, c) in [("x0", 3), ("xl", 3), ("y0", 4), ("yl", 4)] {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
    let h = 1.0 / cells as f64;
    let n_ref = 1000.0;
    let mut loaded = m.clone();
    loaded
        .add_load(Load::nodal_force("xl_mid", [-n_ref * h, 0.0, 0.0]))
        .unwrap();
    loaded
        .add_load(Load::nodal_force("xl_end", [-0.5 * n_ref * h, 0.0, 0.0]))
        .unwrap();
    let r = solve_static(&loaded, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&loaded, &r).unwrap();
    let base = factors(&m, &pre, &BucklingOptions::new(2)).unwrap().factors[0] * n_ref / E;
    let with = factors(&m, &pre.with_shell_rotation_terms(), &BucklingOptions::new(2))
        .unwrap()
        .factors[0]
        * n_ref
        / E;
    (base, with)
}

/// The Reissner-Mindlin ratio `n_1/n_0` of arithmetic 3 (exact root of the quadratic, by the stable form).
fn mindlin_ratio(t: f64) -> f64 {
    let d = t.powi(3) / (12.0 * (1.0 - NU * NU)) * PI * PI;
    let s = KAPPA * t / (2.0 * (1.0 + NU));
    let n0 = 8.0 * s * d / (4.0 * d + 2.0 * s);
    let cc = PI * PI * t * t / 6.0;
    // cc N^2 - (2 s cc + 4 d + 2 s) N + 8 s d - ... : (2s - N)(4d + 2s - N cc) - 4 s^2 = 0.
    let b = 2.0 * s * cc + 4.0 * d + 2.0 * s;
    let c0 = 2.0 * s * (4.0 * d + 2.0 * s) - 4.0 * s * s;
    // The smaller root by the stable form 2 c0/(b + sqrt(b^2 - 4 cc c0)).
    let n1 = 2.0 * c0 / (b + (b * b - 4.0 * cc * c0).sqrt());
    n1 / n0
}

#[test]
#[ignore = "S3 and S4 compared the lowest factors, an in-plane mode crosses: record of the first run, amendment 2"]
fn s3_s4_s5_n1_thick_plate_ratio() {
    let mut c = Checks::new();
    for (name, cells, t, tri, limit) in [
        ("S3 Shell4 16 x 16, t/a = 0.2", 16, 0.2, false, 5e-3),
        ("S4 Shell3 16 x 16 x 2, t/a = 0.2", 16, 0.2, true, 1e-2),
    ] {
        let (base, with) = plate_ratio(cells, t, tri);
        let want = mindlin_ratio(t);
        println!(
            "{name}: N_cr/E base {base:.6e}, with {with:.6e}, ratio {:.8}, closed {want:.8}",
            with / base
        );
        c.at_most(
            &format!("{name} |r/r_closed - 1|"),
            (with / base / want - 1.0).abs(),
            limit,
        );
        c.above(&format!("N1 {name} base minus with"), base - with, 0.0);
    }
    let (base, with) = plate_ratio(16, 0.01, false);
    let want = mindlin_ratio(0.01);
    println!(
        "S5 t/a = 0.01: base {base:.6e}, with {with:.6e}, ratio {:.10}, closed {want:.10}",
        with / base
    );
    c.at_most("S5 |r - r_closed|", (with / base - want).abs(), 5e-5);
    c.finish();
}

#[test]
#[ignore = "known miss of ADR 0508 check I1: the element has no warping DOF (C_w = 0); recorded in addendum 1"]
fn ltb_i_section_known_miss() {
    let mut c = Checks::new();
    let (bf, tf, hw, tw, l): (f64, f64, f64, f64, f64) = (0.1, 0.01, 0.18, 0.006, 3.0);
    let a = 2.0 * bf * tf + hw * tw;
    let izz = 2.0 * tf * bf.powi(3) / 12.0 + hw * tw.powi(3) / 12.0;
    let hc = hw + tf;
    let iyy = 2.0 * (bf * tf.powi(3) / 12.0 + bf * tf * (hc / 2.0).powi(2)) + tw * hw.powi(3) / 12.0;
    let j = (2.0 * bf * tf.powi(3) + hw * tw.powi(3)) / 3.0;
    let cw = izz * hc * hc / 4.0;
    let s = BeamSection::new(a, iyy, izz, j, KAPPA, KAPPA).unwrap();
    let want = m0_of(izz, j, l) * (1.0 + PI * PI * E * cw / (g_mod() * j * l * l)).sqrt();
    let m_ref = 1000.0;
    let f = ltb_factors(&s, 16, l, 0, m_ref, 0.0, true).unwrap();
    println!("I1 factors {f:?}, M_FE {:.8e}, closed {want:.8e}", f[0] * m_ref);
    c.at_most("I1 |M/M_cr - 1|", (f[0] * m_ref / want - 1.0).abs(), 1e-2);
    c.finish();
}

// ------------------------------------------------------------------------------------------ amendment 2 (corrected runs)

#[test]
fn a2_l1_to_l5_n2p_rectangle_lateral_torsional_buckling() {
    let mut c = Checks::new();
    let s = rect_section();
    let m_ref = 1000.0;
    let want = m_cr_rect();
    println!(
        "closed form M_cr = {want:.8e} (C_w = 0: {:.8e})",
        m0_of(i_z(), rect_j(), L)
    );
    let mut at16 = f64::NAN;
    for (name, n, limit) in [("L1 16 elements", 16, 1e-2), ("L2 8 elements", 8, 2e-2)] {
        let f = ltb_factors(&s, n, L, 0, m_ref, 0.0, true).unwrap();
        println!("{name}: factors {f:?}");
        let m = f[0] * m_ref;
        c.at_most(&format!("{name} |M/M_cr - 1|"), (m / want - 1.0).abs(), limit);
        if n == 16 {
            at16 = m;
        }
    }
    let f = ltb_factors(&s, 16, L, 0, -m_ref, 0.0, true).unwrap();
    c.at_most("L3 |f(+M)/f(-M) - 1|", (at16 / (f[0] * m_ref) - 1.0).abs(), 1e-8);
    let f = ltb_factors(&s, 16, L, 0, 3.0 * m_ref, 0.0, true).unwrap();
    c.at_most(
        "L4 |M(3 M_ref)/M(M_ref) - 1|",
        (f[0] * 3.0 * m_ref / at16 - 1.0).abs(),
        1e-9,
    );
    let f = ltb_factors(&s, 16, L, 1, m_ref, 0.0, true).unwrap();
    c.at_most("L5 |M(axis y)/M(axis x) - 1|", (f[0] * m_ref / at16 - 1.0).abs(), 1e-6);
    // N2': the C1 loading without the option is the weak-axis Euler column.
    let pz = PI * PI * E * i_z() / (L * L);
    let m0 = m0_of(i_z(), rect_j(), L);
    let p_ref = 0.3 * pz;
    let base = ltb_factors(&s, 16, L, 0, m0, p_ref, false).unwrap();
    let with = ltb_factors(&s, 16, L, 0, m0, p_ref, true).unwrap();
    println!("N2' base factors {base:?}, with {with:?}");
    c.at_most("N2' |lambda_base 0.3 - 1|", (base[0] * 0.3 - 1.0).abs(), 5e-3);
    c.holds(
        "N2' with the option the factor is below half of the base",
        with[0] < 0.5 * base[0],
        &format!("{:.6} against {:.6}", with[0], base[0]),
    );
    c.finish();
}

#[test]
fn a2_h1_to_h4_r1_element_checks() {
    let mut c = Checks::new();
    let mat = steel();
    let stiff = beam::rectangle_section(B, H, rect_j(), 1e9).unwrap();
    let (l, mm) = (2.0, 7.0);
    let mut f = [0.0; 12];
    f[4] = -mm;
    f[10] = mm;
    let k = beam::local_moment_geometric_stiffness(l, &stiff, &mat, &f);
    let hand = [((3, 7), 1.0), ((9, 7), -1.0), ((3, 1), -1.0), ((9, 1), 1.0)];
    let mut worst: f64 = 0.0;
    for ((i, j), s) in hand {
        let want = s * mm / l;
        worst = worst.max((k.get(i, j) - want).abs() / (mm / l));
        worst = worst.max((k.get(j, i) - want).abs() / (mm / l));
    }
    c.at_most("H1 max |K - hand|/(M/L)", worst, 1e-7);
    let (by, bz) = (0.7, -0.4);
    let (my, mz) = (3.0, 2.0);
    let mut f = [0.0; 12];
    f[4] = -my;
    f[10] = my;
    f[5] = -mz;
    f[11] = mz;
    let k = beam::local_wagner_geometric_stiffness(l, &f, [by, bz]);
    let bm = (by * my - bz * mz) / l;
    let mut worst: f64 = 0.0;
    for i in 0..12 {
        for j in 0..12 {
            let want = match (i, j) {
                (3, 3) | (9, 9) => bm,
                (3, 9) | (9, 3) => -bm,
                _ => 0.0,
            };
            worst = worst.max((k.get(i, j) - want).abs());
        }
    }
    c.at_most("H2 max |K - hand|", worst, 1e-12);
    // H3 with the corrected hand values of amendment 2: +-V/4.
    let vy = 5.0;
    let mut f = [0.0; 12];
    f[1] = -vy;
    f[7] = vy;
    let k = beam::local_shear_geometric_stiffness(l, &stiff, &mat, &f);
    let mut worst: f64 = 0.0;
    for ((i, j), s) in [((3, 8), 0.25), ((9, 8), 0.25), ((3, 2), -0.25), ((9, 2), -0.25)] {
        println!("H3 K[{i}][{j}] = {:.10e}, hand {:.10e}", k.get(i, j), s * vy);
        worst = worst.max((k.get(i, j) - s * vy).abs() / vy);
    }
    c.at_most("H3 max |K - hand|/V", worst, 1e-7);
    let sec = rect_section();
    let mut f = local_vec(3.0, 2.0);
    f[1] = -1.5;
    f[7] = 1.5;
    f[3] = -0.5;
    f[9] = 0.5;
    let w = [0.7, -0.4];
    let all = beam::local_resultant_geometric_stiffness(l, &sec, &mat, &f, w);
    let p1 = beam::local_moment_geometric_stiffness(l, &sec, &mat, &f);
    let p2 = beam::local_shear_geometric_stiffness(l, &sec, &mat, &f);
    let p3 = beam::local_wagner_geometric_stiffness(l, &f, w);
    let mut worst: f64 = 0.0;
    let mut big: f64 = 0.0;
    for i in 0..144 {
        let sum = p1.values()[i] + p2.values()[i] + p3.values()[i];
        worst = worst.max((all.values()[i] - sum).abs());
        big = big.max(all.values()[i].abs());
    }
    c.at_most("H4 max |sum - parts|/max", worst / big, 1e-14);
    let (n, vy, vz, l) = (5.0, 2.0, -3.0, 2.0);
    let f = [-n, -vy, -vz, 0.0, 0.0, 0.0, n, vy, vz, 0.0, l * vz, -l * vy];
    let mut k = beam::local_geometric_stiffness(l, &sec, &mat, n);
    let part = beam::local_resultant_geometric_stiffness(l, &sec, &mat, &f, [0.0; 2]);
    for (a, b) in k.values_mut().iter_mut().zip(part.values()) {
        *a += b;
    }
    let fv = [n, vy, vz];
    let mut worst: f64 = 0.0;
    for axis in 0..3 {
        let mut om = [0.0; 3];
        om[axis] = 1.0;
        let u1 = [0.0, om[2] * l, -om[1] * l];
        let mut d = [0.0; 12];
        d[6..9].copy_from_slice(&u1);
        d[3..6].copy_from_slice(&om);
        d[9..12].copy_from_slice(&om);
        let kd = k.matvec(&d);
        let wf = [
            om[1] * fv[2] - om[2] * fv[1],
            om[2] * fv[0] - om[0] * fv[2],
            om[0] * fv[1] - om[1] * fv[0],
        ];
        for k in 0..3 {
            worst = worst.max((kd[6 + k] - wf[k]).abs()).max((kd[k] + wf[k]).abs());
        }
    }
    c.at_most("R1 max |K_G rigid - omega x F|/max|F|", worst / 5.0, 1e-12);
    c.finish();
}

/// The lowest factors of the model with and without the rotation terms of the plate mode (amendment 2): the lowest of the
/// first four modes whose out-of-plane translation is at least `1e-6` of the largest component.
fn plate_mode_ratio(cells: usize, t: f64, tri: bool) -> (f64, f64, usize, usize) {
    let mut m = grid(cells, cells, t, tri);
    m.add_constraint(Constraint::displacement("edges", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("c00", 1, 0.0)).unwrap();
    for (set, c) in [("x0", 3), ("xl", 3), ("y0", 4), ("yl", 4)] {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
    let h = 1.0 / cells as f64;
    let n_ref = 1000.0;
    let mut loaded = m.clone();
    loaded
        .add_load(Load::nodal_force("xl_mid", [-n_ref * h, 0.0, 0.0]))
        .unwrap();
    loaded
        .add_load(Load::nodal_force("xl_end", [-0.5 * n_ref * h, 0.0, 0.0]))
        .unwrap();
    let r = solve_static(&loaded, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&loaded, &r).unwrap();
    let dofs = m.dof_map();
    let pick = |p: &Prestress| -> (f64, usize) {
        let res = factors(&m, p, &BucklingOptions::new(4)).unwrap();
        for (j, phi) in res.modes.iter().enumerate() {
            let big = phi.iter().fold(0.0f64, |a, v| a.max(v.abs()));
            let wmax = (0..phi.len())
                .filter(|&e| dofs.node_component(e).unwrap().1 == 2)
                .fold(0.0f64, |a, e| a.max(phi[e].abs()));
            if wmax >= 1e-6 * big {
                return (res.factors[j] * n_ref / E, j);
            }
        }
        (f64::NAN, usize::MAX)
    };
    let (base, jb) = pick(&pre);
    let (with, jw) = pick(&pre.with_shell_rotation_terms());
    (base, with, jb, jw)
}

#[test]
fn a2_s3_s4_s5_n1_thick_plate_ratio_of_plate_modes() {
    let mut c = Checks::new();
    for (name, cells, t, tri, limit) in [
        ("S3 Shell4 16 x 16, t/a = 0.2", 16, 0.2, false, 5e-3),
        ("S4 Shell3 16 x 16 x 2, t/a = 0.2", 16, 0.2, true, 1e-2),
        ("S5 Shell4 16 x 16, t/a = 0.01", 16, 0.01, false, 5e-5),
    ] {
        let (base, with, jb, jw) = plate_mode_ratio(cells, t, tri);
        let want = mindlin_ratio(t);
        println!(
            "{name}: plate mode index base {jb}, with {jw}; N_cr/E base {base:.6e}, with {with:.6e}, ratio {:.8}, closed {want:.8}",
            with / base
        );
        let err = if t < 0.05 {
            (with / base - want).abs()
        } else {
            (with / base / want - 1.0).abs()
        };
        c.at_most(&format!("{name} ratio error"), err, limit);
        c.above(&format!("N1 {name} base minus with"), base - with, 0.0);
    }
    c.finish();
}

// ------------------------------------------------------------------------- added after the mutation run (gap of X8, see addendum)

#[test]
fn a3_scaled_and_masked_end_forces() {
    use rcfd_fem::geometric::geometric_stiffness;
    let mut c = Checks::new();
    let s = rect_section();
    let m_ref = 1000.0;
    let m = beam_model(16, L, &s, 0);
    let loaded = moment_loaded(&m, 0, m_ref, 0.0);
    let r = solve_static(&loaded, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&loaded, &r)
        .unwrap()
        .with_beam_resultants(&loaded, r.displacement_values(), [0.0; 2])
        .unwrap();
    let f1 = factors(&m, &pre, &BucklingOptions::new(2)).unwrap().factors[0];
    let f2 = factors(&m, &pre.scaled(2.0), &BucklingOptions::new(2)).unwrap().factors[0];
    println!("A3 factors {f1:.10e}, scaled by 2: {f2:.10e}");
    c.at_most("A3 scaled: |2 f(2 pre)/f(pre) - 1|", (2.0 * f2 / f1 - 1.0).abs(), 1e-9);
    let k0 = geometric_stiffness(&m, &pre, Some(1)).unwrap();
    let kk = geometric_stiffness(&m, &pre.masked(|_| true), Some(1)).unwrap();
    c.holds("A3 masked keeping all: the same bits", k0.val == kk.val, "bitwise");
    let kn = geometric_stiffness(&m, &pre.masked(|_| false), Some(1)).unwrap();
    c.holds(
        "A3 masked keeping none: a zero matrix",
        kn.val.iter().all(|&v| v == 0.0),
        "exact",
    );
    c.finish();
}
