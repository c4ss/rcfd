//! F-598 (ADR 0546): the warping unknown of Beam2 (the 14-unknown element) and the I-section lateral-torsional buckling.
//! Checks A1 to A6, N1 to N4, E1 to E8, P1, M1 and R1 to R9 of the registration. Each registered run is made once; every value
//! is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    self, buckling_factors, index_of, matrices, warping_dofs, Warping, WarpingSection, TWIST,
};
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::element::ElementMatrix;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::model::ElasticSupport;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    beam, BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel,
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
const M_REF: f64 = 1000.0;

fn g_mod() -> f64 {
    E / (2.0 * (1.0 + NU))
}

fn steel() -> Material {
    Material::isotropic_elastic(E, NU, 7850.0, 0.0).unwrap()
}

/// The I section of ADR 0508 I1 and its warping section (D1).
fn i_parts() -> (BeamSection, WarpingSection) {
    WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, KAPPA).unwrap()
}

/// The acceptance's closed form.
fn m_cr(s: &BeamSection, cw: f64, l: f64) -> f64 {
    let gj = g_mod() * s.torsion();
    PI / l * (E * s.i_z() * gj * (1.0 + PI * PI * E * cw / (gj * l * l))).sqrt()
}

const RB: f64 = 0.01;
const RH: f64 = 0.2;

fn rect_section() -> BeamSection {
    let j = (RH * RB.powi(3) / 3.0) * (1.0 - 0.63 * (RB / RH) * (1.0 - RB.powi(4) / (12.0 * RH.powi(4))));
    beam::rectangle_section(RB, RH, j, KAPPA).unwrap()
}

/// A beam of `n` elements and length `l` along the global axis `axis` (0 = x, 1 = y), fork supports (`v = w = phi = 0` at both
/// ends, `u = 0` at node 0), the set-up of ADR 0508.
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
    mesh.add_node_set("mid", vec![n / 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    let comp = if axis == 0 { [0, 1, 2, 3] } else { [1, 2, 0, 4] };
    for c in comp {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    for &c in &comp[1..] {
        m.add_constraint(Constraint::displacement("xl", c, 0.0)).unwrap();
    }
    m
}

/// End moments `m_ref` about the strong axis `e_2`.
fn moment_loaded(m: &SolidModel, axis: usize, m_ref: f64) -> SolidModel {
    let mut l = m.clone();
    let mom = |s: f64| {
        let mut v = [0.0; 3];
        v[if axis == 0 { 1 } else { 2 }] = s * m_ref;
        v
    };
    l.add_load(Load::nodal_moment("x0", mom(-1.0))).unwrap();
    l.add_load(Load::nodal_moment("xl", mom(1.0))).unwrap();
    l
}

/// The pre-stress of the static solve of `loaded` with the resultant terms of ADR 0508 (pair `(0, 0)`).
fn prestress_of(loaded: &SolidModel) -> Prestress {
    let r = solve_static(loaded, &StaticOptions::default()).unwrap();
    Prestress::from_static(loaded, &r)
        .unwrap()
        .with_beam_resultants(loaded, r.displacement_values(), [0.0; 2])
        .unwrap()
}

fn warp_factors(
    section: &BeamSection,
    ws: WarpingSection,
    n: usize,
    l: f64,
    axis: usize,
    m_ref: f64,
    restrain: bool,
) -> Vec<f64> {
    let m = beam_model(n, l, section, axis);
    let pre = prestress_of(&moment_loaded(&m, axis, m_ref));
    let mut w = Warping::new().block(0, ws);
    if restrain {
        w = w.restrain("x0").restrain("xl");
    }
    let r = buckling_factors(&m, &pre, &w, &BucklingOptions::new(3)).unwrap();
    println!(
        "  warping factors {:?} (n {n}, L {l}, axis {axis}, M_ref {m_ref})",
        r.factors
    );
    r.factors
}

fn f558_factors(section: &BeamSection, n: usize, l: f64, m_ref: f64) -> Vec<f64> {
    let m = beam_model(n, l, section, 0);
    let pre = prestress_of(&moment_loaded(&m, 0, m_ref));
    let f = factors(&m, &pre, &BucklingOptions::new(3)).unwrap().factors;
    println!("  F-558 factors {f:?} (n {n}, L {l})");
    f
}

#[test]
fn a_n_lateral_torsional_buckling_of_the_i_beam() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let want = m_cr(&s, ws.c_w(), 3.0);
    println!(
        "C_w {:.6e}, closed M_cr(3) {want:.8e}, M_0 {:.8e}",
        ws.c_w(),
        m_cr(&s, 0.0, 3.0)
    );
    let f16 = warp_factors(&s, ws, 16, 3.0, 0, M_REF, false);
    let m16 = f16[0] * M_REF;
    c.at_most("A1 |M/M_cr - 1| (16 elements, L 3)", (m16 / want - 1.0).abs(), 1e-2);
    println!("A1 signed {:.6e}", m16 / want - 1.0);
    let f8 = warp_factors(&s, ws, 8, 3.0, 0, M_REF, false);
    c.at_most("A2 |M/M_cr - 1| (8 elements)", (f8[0] * M_REF / want - 1.0).abs(), 2e-2);
    for l in [1.5, 6.0] {
        let f = warp_factors(&s, ws, 16, l, 0, M_REF, false);
        let w = m_cr(&s, ws.c_w(), l);
        println!(
            "A3 L {l}: M {:.8e}, closed {w:.8e}, signed {:.6e}",
            f[0] * M_REF,
            f[0] * M_REF / w - 1.0
        );
        c.at_most(
            &format!("A3 |M/M_cr - 1| (L {l})"),
            (f[0] * M_REF / w - 1.0).abs(),
            1e-2,
        );
    }
    let fm = warp_factors(&s, ws, 16, 3.0, 0, -M_REF, false);
    c.at_most("A4 |f(+M)/f(-M) - 1|", (f16[0] / fm[0] - 1.0).abs(), 1e-8);
    let fy = warp_factors(&s, ws, 16, 3.0, 1, M_REF, false);
    c.at_most("A5 |M(axis y)/M(axis x) - 1|", (fy[0] / f16[0] - 1.0).abs(), 1e-6);
    let f3 = warp_factors(&s, ws, 16, 3.0, 0, 3.0 * M_REF, false);
    c.at_most(
        "A6 |M(3 M_ref)/M(M_ref) - 1|",
        (f3[0] * 3.0 * M_REF / m16 - 1.0).abs(),
        1e-9,
    );
    // N1: Saint-Venant only.
    let fsv = warp_factors(&s, WarpingSection::saint_venant(), 16, 3.0, 0, M_REF, false);
    let f558 = f558_factors(&s, 16, 3.0, M_REF);
    let msv = fsv[0] * M_REF;
    let m558 = f558[0] * M_REF;
    println!(
        "N1 M_SV {msv:.8e}, M_0 {:.8e}, F-558 {m558:.8e}; signed {:.6e}, {:.6e}",
        m_cr(&s, 0.0, 3.0),
        msv / m_cr(&s, 0.0, 3.0) - 1.0,
        msv / m558 - 1.0
    );
    c.at_most("N1 |M_SV/M_0 - 1|", (msv / m_cr(&s, 0.0, 3.0) - 1.0).abs(), 1e-2);
    c.at_most("N1 |M_SV/M_F558 - 1|", (msv / m558 - 1.0).abs(), 1e-2);
    // N4: the warping term acts.
    let n4 = m16 / m558 - 1.0;
    c.holds(
        "N4 M_warping/M_F558 - 1 > 0.15",
        n4.is_finite() && n4 > 0.15,
        &format!("{n4:.6e}"),
    );
    // N3: warping restrained at both ends.
    let fr = warp_factors(&s, ws, 16, 3.0, 0, M_REF, true);
    let mr = fr[0] * M_REF;
    let k = 2.0 * PI / 3.0;
    let mff = (E * s.i_z() * k * k * (g_mod() * s.torsion() + E * ws.c_w() * k * k)).sqrt();
    println!(
        "N3 M_restrained {mr:.8e}, M_free {m16:.8e} (ratio {:.6}), M_ff {mff:.8e}",
        mr / m16
    );
    c.holds(
        "N3 1.2 M_free < M_restrained < 1.01 M_ff",
        mr.is_finite() && mr > 1.2 * m16 && mr < 1.01 * mff,
        &format!("{mr:.6e} in ({:.6e}, {:.6e})", 1.2 * m16, 1.01 * mff),
    );
    c.finish();
}

#[test]
fn n2_rectangle_on_the_warping_path() {
    let mut c = Checks::new();
    let s = rect_section();
    let cw = RB.powi(3) * RH.powi(3) / 144.0;
    let want = m_cr(&s, cw, 3.0);
    let f = warp_factors(&s, WarpingSection::new(cw, 0.0, 0.0).unwrap(), 16, 3.0, 0, M_REF, false);
    let f558 = f558_factors(&s, 16, 3.0, M_REF);
    println!(
        "N2 M {:.8e}, closed {want:.8e}, F-558 {:.8e}",
        f[0] * M_REF,
        f558[0] * M_REF
    );
    c.at_most("N2 |M/M_cr - 1|", (f[0] * M_REF / want - 1.0).abs(), 1e-2);
    c.at_most("N2 |M/M_F558 - 1|", (f[0] / f558[0] - 1.0).abs(), 5e-3);
    c.finish();
}

/// The two hand matrices of ADR 0546 D2 (`int H' H'` and `int H'' H''`).
fn hand_g(l: f64) -> [[f64; 4]; 4] {
    let c = 1.0 / (30.0 * l);
    [
        [36.0 * c, 3.0 * l * c, -36.0 * c, 3.0 * l * c],
        [3.0 * l * c, 4.0 * l * l * c, -3.0 * l * c, -l * l * c],
        [-36.0 * c, -3.0 * l * c, 36.0 * c, -3.0 * l * c],
        [3.0 * l * c, -l * l * c, -3.0 * l * c, 4.0 * l * l * c],
    ]
}

fn hand_w(l: f64) -> [[f64; 4]; 4] {
    let c = 1.0 / (l * l * l);
    [
        [12.0 * c, 6.0 * l * c, -12.0 * c, 6.0 * l * c],
        [6.0 * l * c, 4.0 * l * l * c, -6.0 * l * c, 2.0 * l * l * c],
        [-12.0 * c, -6.0 * l * c, 12.0 * c, -6.0 * l * c],
        [6.0 * l * c, 2.0 * l * l * c, -6.0 * l * c, 4.0 * l * l * c],
    ]
}

fn twist_diff(k: &ElementMatrix, hand: &[[f64; 4]; 4]) -> f64 {
    let mut d: f64 = 0.0;
    let mut h: f64 = 0.0;
    for a in 0..4 {
        for b in 0..4 {
            d = d.max((k.get(TWIST[a], TWIST[b]) - hand[a][b]).abs());
            h = h.max(hand[a][b].abs());
        }
    }
    d / h
}

/// Whether the entries of `k14` off the twist rows and columns equal `k12` by the index map, and the twist rows are zero off
/// the twist columns.
fn embedded_bits(k14: &ElementMatrix, k12: &ElementMatrix) -> bool {
    let mut ok = true;
    for i in 0..12 {
        for j in 0..12 {
            if [3, 9].contains(&i) || [3, 9].contains(&j) {
                continue;
            }
            ok &= k14.get(index_of(i), index_of(j)).to_bits() == k12.get(i, j).to_bits();
        }
    }
    for &a in &TWIST {
        for j in 0..14 {
            if !TWIST.contains(&j) {
                ok &= k14.get(a, j) == 0.0 && k14.get(j, a) == 0.0;
            }
        }
    }
    ok
}

#[test]
fn e1_to_e4_e7_e8_element_checks() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let mat = steel();
    let l = 0.7;
    let (gj, ecw) = (g_mod() * s.torsion(), E * ws.c_w());
    // E1.
    let k = beam_warping::local_stiffness(l, &s, &mat, ws.c_w());
    let (hg, hw) = (hand_g(l), hand_w(l));
    let mut hand = [[0.0; 4]; 4];
    for a in 0..4 {
        for b in 0..4 {
            hand[a][b] = gj * hg[a][b] + ecw * hw[a][b];
        }
    }
    c.at_most(
        "E1 torsion block against the hand matrices",
        twist_diff(&k, &hand),
        1e-12,
    );
    // E2.
    c.holds(
        "E2 the other entries bit-equal to beam::local_stiffness",
        embedded_bits(&k, &beam::local_stiffness(l, &s, &mat)),
        "",
    );
    // E3.
    let kmax = k.values().iter().fold(0.0f64, |a, v| a.max(v.abs()));
    let mut x = vec![0.0; 14];
    x[3] = 1.0;
    x[10] = 1.0;
    let r = k.matvec(&x);
    let rmax = r.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    c.at_most("E3 rigid twist |K x|/max|K|", rmax / kmax, 1e-12);
    let mut x = vec![0.0; 14];
    x[10] = l;
    x[6] = 1.0;
    x[13] = 1.0;
    let kx = k.matvec(&x);
    let energy: f64 = x.iter().zip(&kx).map(|(a, b)| a * b).sum();
    c.at_most(
        "E3 constant rate |x K x/(G J L) - 1|",
        (energy / (gj * l) - 1.0).abs(),
        1e-12,
    );
    // E4.
    let n = -5.0e3;
    let kg = beam_warping::local_geometric_stiffness(l, &s, &mat, n, None, [0.0; 2]);
    let nip = n * (s.i_y() + s.i_z()) / s.area();
    let mut hand = [[0.0; 4]; 4];
    for a in 0..4 {
        for b in 0..4 {
            hand[a][b] = nip * hg[a][b];
        }
    }
    c.at_most("E4 geometric torsion block", twist_diff(&kg, &hand), 1e-12);
    c.holds(
        "E4 the other entries bit-equal to beam::local_geometric_stiffness",
        embedded_bits(&kg, &beam::local_geometric_stiffness(l, &s, &mat, n)),
        "",
    );
    // E7: uniform M_y = 3, M_z = 2 (section resultants -f at x = 0, +f at x = L).
    let f = [0.0, 0.0, 0.0, 0.0, -3.0, -2.0, 0.0, 0.0, 0.0, 0.0, 3.0, 2.0];
    let beta = [0.7, -0.4];
    let a = beam_warping::local_geometric_stiffness(l, &s, &mat, 0.0, Some(&f), beta);
    let b = beam_warping::local_geometric_stiffness(l, &s, &mat, 0.0, Some(&f), [0.0; 2]);
    let bm = beta[0] * 3.0 - beta[1] * 2.0;
    let mut d: f64 = 0.0;
    let mut h: f64 = 0.0;
    for i in 0..14 {
        for j in 0..14 {
            let hand = match (TWIST.iter().position(|&t| t == i), TWIST.iter().position(|&t| t == j)) {
                (Some(p), Some(q)) => bm * hg[p][q],
                _ => 0.0,
            };
            d = d.max((a.get(i, j) - b.get(i, j) - hand).abs());
            h = h.max(hand.abs());
        }
    }
    c.at_most("E7 per-section Wagner terms", d / h, 1e-12);
    // E8.
    let (bf, tf, hw, tw): (f64, f64, f64, f64) = (0.1, 0.01, 0.18, 0.006);
    let area = 2.0 * bf * tf + hw * tw;
    let izz = 2.0 * tf * bf.powi(3) / 12.0 + hw * tw.powi(3) / 12.0;
    let hc = hw + tf;
    let iyy = 2.0 * (bf * tf.powi(3) / 12.0 + bf * tf * (hc / 2.0).powi(2)) + tw * hw.powi(3) / 12.0;
    let j = (2.0 * bf * tf.powi(3) + hw * tw.powi(3)) / 3.0;
    let ok = s.area().to_bits() == area.to_bits()
        && s.i_y().to_bits() == iyy.to_bits()
        && s.i_z().to_bits() == izz.to_bits()
        && s.torsion().to_bits() == j.to_bits()
        && ws.c_w().to_bits() == (tf * bf.powi(3) * hc * hc / 24.0).to_bits()
        && ws.wagner() == [0.0, 0.0];
    c.holds(
        "E8 doubly_symmetric_i",
        ok,
        &format!("C_w {:e}, wagner {:?}", ws.c_w(), ws.wagner()),
    );
    c.finish();
}

#[test]
fn e5_frame_invariance() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let mat = steel();
    let l = 0.7;
    let r = 0.5f64.sqrt();
    let q = [[r, -r, 0.0], [r, r, 0.0], [0.0, 0.0, 1.0]];
    let x_coords = [[0.0, 0.0, 0.0], [l, 0.0, 0.0]];
    let s_coords = [[0.0, 0.0, 0.0], [l * r, l * r, 0.0]];
    let (x_or, s_or) = ([0.0, 1.0, 0.0], [-r, r, 0.0]);
    let f = [
        50.0,
        -2.0,
        3.0,
        0.0,
        1000.0,
        -400.0,
        -50.0,
        2.0,
        -3.0,
        0.0,
        -1000.0 - 3.0 * l,
        400.0 - 2.0 * l,
    ];
    let mut qq = [[0.0; 14]; 14];
    for node in 0..2 {
        let o = 7 * node;
        for blk in 0..2 {
            for i in 0..3 {
                for j in 0..3 {
                    qq[o + 3 * blk + i][o + 3 * blk + j] = q[i][j];
                }
            }
        }
        qq[o + 6][o + 6] = 1.0;
    }
    let rotate = |k: &ElementMatrix| {
        let mut out = [[0.0; 14]; 14];
        for i in 0..14 {
            for j in 0..14 {
                let mut sum = 0.0;
                for a in 0..14 {
                    for b in 0..14 {
                        sum += qq[i][a] * k.get(a, b) * qq[j][b];
                    }
                }
                out[i][j] = sum;
            }
        }
        out
    };
    let pairs = [
        (
            "stiffness",
            beam_warping::element_stiffness(&x_coords, &s, x_or, &mat, ws.c_w()).unwrap(),
            beam_warping::element_stiffness(&s_coords, &s, s_or, &mat, ws.c_w()).unwrap(),
        ),
        (
            "geometric stiffness",
            beam_warping::element_geometric_stiffness(&x_coords, &s, x_or, &mat, -5e3, Some(&f), [0.7, -0.4]).unwrap(),
            beam_warping::element_geometric_stiffness(&s_coords, &s, s_or, &mat, -5e3, Some(&f), [0.7, -0.4]).unwrap(),
        ),
    ];
    for (name, kx, ks) in pairs {
        let want = rotate(&kx);
        let mut d: f64 = 0.0;
        let mut m: f64 = 0.0;
        for i in 0..14 {
            for j in 0..14 {
                d = d.max((ks.get(i, j) - want[i][j]).abs());
                m = m.max(want[i][j].abs());
            }
        }
        c.at_most(&format!("E5 {name} skew against Q K Q^T"), d / m, 1e-12);
    }
    c.finish();
}

/// Dense Gaussian elimination with partial pivoting.
fn solve_dense(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    for col in 0..n {
        let p = (col..n)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .unwrap();
        a.swap(col, p);
        b.swap(col, p);
        for r in col + 1..n {
            let f = a[r][col] / a[col][col];
            for k in col..n {
                a[r][k] -= f * a[col][k];
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let s: f64 = (r + 1..n).map(|k| a[r][k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r][r];
    }
    x
}

#[test]
fn e6_cantilever_under_end_torque() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let mat = steel();
    let (l, n, t) = (3.0, 16usize, 1000.0);
    let h = l / n as f64;
    let ke = beam_warping::local_stiffness(h, &s, &mat, ws.c_w());
    let gj = g_mod() * s.torsion();
    let kk = (gj / (E * ws.c_w())).sqrt();
    for (name, held, want, limit) in [
        ("root warping held", true, t / gj * (l - (kk * l).tanh() / kk), 1e-3),
        ("root warping free", false, t * l / gj, 1e-10),
    ] {
        let nd = 2 * (n + 1);
        let mut a = vec![vec![0.0; nd]; nd];
        for e in 0..n {
            let idx = [2 * e, 2 * e + 1, 2 * e + 2, 2 * e + 3];
            for p in 0..4 {
                for q in 0..4 {
                    a[idx[p]][idx[q]] += ke.get(TWIST[p], TWIST[q]);
                }
            }
        }
        let fixed: Vec<usize> = if held { vec![0, 1] } else { vec![0] };
        let free: Vec<usize> = (0..nd).filter(|i| !fixed.contains(i)).collect();
        let af: Vec<Vec<f64>> = free.iter().map(|&i| free.iter().map(|&j| a[i][j]).collect()).collect();
        let mut bf = vec![0.0; free.len()];
        bf[free.iter().position(|&i| i == 2 * n).unwrap()] = t;
        let x = solve_dense(af, bf);
        let phi = x[free.iter().position(|&i| i == 2 * n).unwrap()];
        println!("E6 {name}: phi(L) {phi:.10e}, closed {want:.10e}");
        c.at_most(&format!("E6 {name}"), (phi / want - 1.0).abs(), limit);
    }
    c.finish();
}

#[test]
fn p1_per_section_wagner_in_the_assembly() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let n = 8;
    let l = 3.0;
    let nodes: Vec<Vec3> = (0..=n).map(|i| [l * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, (0..4).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    mesh.add_block(ElementKind::Beam2, (4..8).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    for b in 0..2 {
        mesh.set_block_property(b, BlockProperty::beam(s, [0.0, 1.0, 0.0]).unwrap())
            .unwrap();
    }
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for comp in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", comp, 0.0)).unwrap();
    }
    for comp in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", comp, 0.0)).unwrap();
    }
    let pre = prestress_of(&moment_loaded(&m, 0, M_REF));
    let beta = [0.7, -0.4];
    let wa = Warping::new()
        .block(0, WarpingSection::new(ws.c_w(), beta[0], beta[1]).unwrap())
        .block(1, ws);
    let wb = Warping::new().block(0, ws).block(1, ws);
    let ga = matrices(&m, &pre, &wa, Some(1)).unwrap();
    let gb = matrices(&m, &pre, &wb, Some(1)).unwrap();
    let nn = ga.dofs.n_equations();
    let mut hand = vec![vec![0.0; nn]; nn];
    let dofs = m.dof_map();
    let h = l / n as f64;
    let hg = hand_g(h);
    for e in 0..4 {
        let (f, _) = pre.beam_terms(e).unwrap();
        let (r0, r1) = ([-f[4], -f[5]], [f[10], f[11]]);
        let bm = 0.5 * ((beta[0] * r0[0] - beta[1] * r0[1]) + (beta[0] * r1[0] - beta[1] * r1[1]));
        let eqs = [
            dofs.equation(e, 3).unwrap(),
            ga.dofs.equation(e).unwrap(),
            dofs.equation(e + 1, 3).unwrap(),
            ga.dofs.equation(e + 1).unwrap(),
        ];
        for p in 0..4 {
            for q in 0..4 {
                hand[eqs[p]][eqs[q]] += bm * hg[p][q];
            }
        }
    }
    let same_pattern = ga.geometric.row_ptr == gb.geometric.row_ptr && ga.geometric.col == gb.geometric.col;
    c.holds("P1 the same pattern", same_pattern, "");
    let mut d: f64 = 0.0;
    let mut hmax: f64 = 0.0;
    let mut covered = 0.0;
    for r in 0..nn {
        for p in ga.geometric.row_ptr[r]..ga.geometric.row_ptr[r + 1] {
            let col = ga.geometric.col[p];
            let diff = ga.geometric.val[p] - gb.geometric.val[p];
            d = d.max((diff - hand[r][col]).abs());
            covered += hand[r][col].abs();
        }
        for v in &hand[r] {
            hmax = hmax.max(v.abs());
        }
    }
    let total: f64 = hand.iter().flatten().map(|v| v.abs()).sum();
    c.holds(
        "P1 every hand entry on the pattern",
        (covered - total).abs() <= 1e-12 * total,
        "",
    );
    c.at_most(
        "P1 K_sigma(beta) - K_sigma(0) against the hand Wagner matrices",
        d / hmax,
        1e-8,
    );
    c.finish();
}

/// The I beam (warping, block 0, nodes 0..=16) and the rectangle (no warping, block 1, nodes 17..=33, at y = 1), fork supports,
/// end moments `M_REF` and `M_REF r`.
fn mixed_model(r: f64) -> (SolidModel, SolidModel) {
    let (s, _) = i_parts();
    let n = 16;
    let mut nodes: Vec<Vec3> = (0..=n).map(|i| [3.0 * i as f64 / n as f64, 0.0, 0.0]).collect();
    nodes.extend((0..=n).map(|i| [3.0 * i as f64 / n as f64, 1.0, 0.0]));
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, (0..n).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    mesh.add_block(
        ElementKind::Beam2,
        (0..n).flat_map(|i| [n + 1 + i, n + 2 + i]).collect(),
    )
    .unwrap();
    mesh.set_block_property(0, BlockProperty::beam(s, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.set_block_property(1, BlockProperty::beam(rect_section(), [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..2 * n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0, n + 1]).unwrap();
    mesh.add_node_set("xl", vec![n, 2 * n + 1]).unwrap();
    mesh.add_node_set("i0", vec![0]).unwrap();
    mesh.add_node_set("il", vec![n]).unwrap();
    mesh.add_node_set("r0", vec![n + 1]).unwrap();
    mesh.add_node_set("rl", vec![2 * n + 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for comp in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", comp, 0.0)).unwrap();
    }
    for comp in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", comp, 0.0)).unwrap();
    }
    let mut loaded = m.clone();
    loaded.add_load(Load::nodal_moment("i0", [0.0, -M_REF, 0.0])).unwrap();
    loaded.add_load(Load::nodal_moment("il", [0.0, M_REF, 0.0])).unwrap();
    loaded
        .add_load(Load::nodal_moment("r0", [0.0, -M_REF * r, 0.0]))
        .unwrap();
    loaded
        .add_load(Load::nodal_moment("rl", [0.0, M_REF * r, 0.0]))
        .unwrap();
    (m, loaded)
}

#[test]
#[ignore = "known miss of ADR 0546 check M1 (run 1): I-beam factor 1.5085e-9 against 1e-9, the solver tolerance 1e-10 (amendment 1)"]
fn m1_mixed_model_run1_known_miss() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let r = 4266.115 / (30.0 * M_REF);
    let (m, loaded) = mixed_model(r);
    let pre = prestress_of(&loaded);
    let res = buckling_factors(&m, &pre, &Warping::new().block(0, ws), &BucklingOptions::new(3)).unwrap();
    println!("M1 mixed factors {:?}", res.factors);
    let rect = f558_factors(&rect_section(), 16, 3.0, M_REF * r);
    let ibeam = warp_factors(&s, ws, 16, 3.0, 0, M_REF, false);
    c.at_most("M1 rectangle factor", (res.factors[0] / rect[0] - 1.0).abs(), 1e-9);
    c.at_most("M1 I-beam factor", (res.factors[1] / ibeam[0] - 1.0).abs(), 1e-9);
    c.finish();
}

/// M1' of ADR 0546 amendment 1: M1 with the eigensolver tolerance `1e-13` on the three solves, limits unchanged.
#[test]
fn a1_m1p_mixed_model() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let opts = BucklingOptions::new(3).tolerance(1e-13);
    let r = 4266.115 / (30.0 * M_REF);
    let (m, loaded) = mixed_model(r);
    let res = buckling_factors(&m, &prestress_of(&loaded), &Warping::new().block(0, ws), &opts).unwrap();
    println!("M1' mixed factors {:?}, residuals {:?}", res.factors, res.residuals);
    let mr = beam_model(16, 3.0, &rect_section(), 0);
    let rect = factors(&mr, &prestress_of(&moment_loaded(&mr, 0, M_REF * r)), &opts).unwrap();
    let mi = beam_model(16, 3.0, &s, 0);
    let ibeam = buckling_factors(
        &mi,
        &prestress_of(&moment_loaded(&mi, 0, M_REF)),
        &Warping::new().block(0, ws),
        &opts,
    )
    .unwrap();
    println!(
        "M1' rectangle alone {:?}, I beam alone {:?}",
        rect.factors, ibeam.factors
    );
    c.at_most(
        "M1' rectangle factor",
        (res.factors[0] / rect.factors[0] - 1.0).abs(),
        1e-9,
    );
    c.at_most(
        "M1' I-beam factor",
        (res.factors[1] / ibeam.factors[0] - 1.0).abs(),
        1e-9,
    );
    c.finish();
}

fn key_of(e: &FemError) -> String {
    match e {
        FemError::InvalidValue { key, .. } => key.to_string(),
        FemError::Element { source, .. } => key_of(source),
        FemError::UnknownSet { .. } => "UnknownSet".to_string(),
        other => format!("{other:?}"),
    }
}

#[test]
fn r1_to_r9_refusals() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let opts = BucklingOptions::new(3);
    let base = beam_model(8, 3.0, &s, 0);
    let pre = prestress_of(&moment_loaded(&base, 0, M_REF));
    let mut check = |name: &str, r: Result<(), FemError>, want: &str| {
        let got = r.err().map(|e| key_of(&e));
        c.holds(name, got.as_deref() == Some(want), &format!("{got:?} (want {want})"));
    };
    let run =
        |m: &SolidModel, p: &Prestress, w: &Warping, o: &BucklingOptions| buckling_factors(m, p, w, o).map(|_| ());
    // RF1.
    check("R1 no block", run(&base, &pre, &Warping::new(), &opts), "warping");
    check(
        "R1 missing block",
        run(&base, &pre, &Warping::new().block(5, ws), &opts),
        "warping",
    );
    check(
        "R1 block twice",
        run(&base, &pre, &Warping::new().block(0, ws).block(0, ws), &opts),
        "warping",
    );
    {
        let mut mesh = SolidMesh::new(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]).unwrap();
        mesh.add_block(ElementKind::Shell3, vec![0, 1, 2]).unwrap();
        mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
        let m = SolidModel::new(mesh);
        check(
            "R1 a Shell3 block",
            warping_dofs(&m, &Warping::new().block(0, ws)).map(|_| ()),
            "warping",
        );
    }
    // RF2: the two-block beam, block 0 alone.
    let (mixed, mixed_loaded) = {
        let n = 8;
        let nodes: Vec<Vec3> = (0..=n).map(|i| [3.0 * i as f64 / n as f64, 0.0, 0.0]).collect();
        let mut mesh = SolidMesh::new(nodes).unwrap();
        mesh.add_block(ElementKind::Beam2, (0..4).flat_map(|i| [i, i + 1]).collect())
            .unwrap();
        mesh.add_block(ElementKind::Beam2, (4..8).flat_map(|i| [i, i + 1]).collect())
            .unwrap();
        for b in 0..2 {
            mesh.set_block_property(b, BlockProperty::beam(s, [0.0, 1.0, 0.0]).unwrap())
                .unwrap();
        }
        mesh.add_element_set("all", (0..n).collect()).unwrap();
        mesh.add_node_set("x0", vec![0]).unwrap();
        mesh.add_node_set("xl", vec![n]).unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("all", steel()).unwrap();
        for comp in [0, 1, 2, 3] {
            m.add_constraint(Constraint::displacement("x0", comp, 0.0)).unwrap();
        }
        for comp in [1, 2, 3] {
            m.add_constraint(Constraint::displacement("xl", comp, 0.0)).unwrap();
        }
        let l = moment_loaded(&m, 0, M_REF);
        (m, l)
    };
    let mpre = prestress_of(&mixed_loaded);
    check(
        "R2 shared node with a non-warping element",
        run(&mixed, &mpre, &Warping::new().block(0, ws), &opts),
        "warping",
    );
    // RF3: a kink.
    {
        let mut mesh = SolidMesh::new(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]]).unwrap();
        mesh.add_block(ElementKind::Beam2, vec![0, 1, 1, 2]).unwrap();
        mesh.set_block_property(0, BlockProperty::beam(s, [0.0, 0.0, 1.0]).unwrap())
            .unwrap();
        let m = SolidModel::new(mesh);
        check(
            "R3 a kink",
            warping_dofs(&m, &Warping::new().block(0, ws)).map(|_| ()),
            "warping",
        );
    }
    // RF4: an elastic support.
    {
        let mut m = base.clone();
        m.add_support(ElasticSupport::nodal_spring("xl", 0, 1e6)).unwrap();
        check(
            "R4 the transformation path",
            run(&m, &pre, &Warping::new().block(0, ws), &opts),
            "warping",
        );
    }
    // RF5.
    {
        let mut m = beam_model(8, 3.0, &s, 0);
        m.add_constraint(Constraint::displacement("mid", 1, 1e-3)).unwrap();
        check(
            "R5 nonzero prescribed value",
            run(&m, &pre, &Warping::new().block(0, ws), &opts),
            "constraint",
        );
    }
    // RF6.
    check(
        "R6 restrained set missing",
        run(&base, &pre, &Warping::new().block(0, ws).restrain("nope"), &opts),
        "UnknownSet",
    );
    {
        let r = 4266.115 / (30.0 * M_REF);
        let (m, loaded) = mixed_model(r);
        let p = prestress_of(&loaded);
        check(
            "R6 restrained node without a warping unknown",
            run(&m, &p, &Warping::new().block(0, ws).restrain("r0"), &opts),
            "warping",
        );
    }
    // RF7.
    check(
        "R7 negative c_w",
        WarpingSection::new(-1.0, 0.0, 0.0).map(|_| ()),
        "c_w",
    );
    check(
        "R7 non-finite wagner",
        WarpingSection::new(1.0, f64::NAN, 0.0).map(|_| ()),
        "wagner",
    );
    check(
        "R7 zero flange width",
        WarpingSection::doubly_symmetric_i(0.0, 0.01, 0.18, 0.006, KAPPA).map(|_| ()),
        "b_f",
    );
    // RF8: a torque at mid-span.
    {
        let mut loaded = base.clone();
        loaded.add_load(Load::nodal_moment("mid", [100.0, 0.0, 0.0])).unwrap();
        let p = prestress_of(&loaded);
        check(
            "R8 torque in the pre-stress",
            run(&base, &p, &Warping::new().block(0, ws), &opts),
            "warping",
        );
    }
    // RF9.
    check(
        "R9 coordinate order",
        run(
            &base,
            &pre,
            &Warping::new().block(0, ws),
            &BucklingOptions::new(3).coordinate_order(true),
        ),
        "factorisation",
    );
    c.finish();
}
