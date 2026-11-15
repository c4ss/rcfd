//! F-626 (ADR 0548): the warping unknown on the static solve, the modes and the pre-stressed modes, and torsion in the buckling
//! pre-stress. Checks S1 to S7, T1 to T7, F1 to F7, P1 to P3, PT1 to PT3 and R of the registration. Each registered run is made
//! once; every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    self, buckling_factors, buckling_factors_with, modal_analysis, prestressed_modes, solve_static, Warping,
    WarpingPrestress, WarpingSection, TWIST,
};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::model::ElasticSupport;
use rcfd_fem::solve::StaticOptions;
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
const RHO: f64 = 7850.0;
const KAPPA: f64 = 5.0 / 6.0;
const L: f64 = 3.0;
const T: f64 = 1000.0;
const M_REF: f64 = 1000.0;
const T_REF: f64 = 4.0e4;

fn g_mod() -> f64 {
    E / (2.0 * (1.0 + NU))
}

fn steel() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn i_parts() -> (BeamSection, WarpingSection) {
    WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, KAPPA).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

/// A beam of `n` elements and length `L` along the unit `dir`, the section's `e_2` along `orient`; node sets `x0`, `xl`,
/// `ta` (node `n/4`) and `nodes` (all).
fn mesh_along(n: usize, section: &BeamSection, dir: Vec3, orient: Vec3, material: Material) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=n)
        .map(|i| {
            let s = L * i as f64 / n as f64;
            [s * dir[0], s * dir[1], s * dir[2]]
        })
        .collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(*section, orient).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    mesh.add_node_set("ta", vec![n / 4]).unwrap();
    mesh.add_node_set("nodes", (0..=n).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m
}

/// The cantilever: the root with all six components held, along `dir`.
fn cantilever(n: usize, section: &BeamSection, dir: Vec3, orient: Vec3) -> SolidModel {
    let mut m = mesh_along(n, section, dir, orient, steel());
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    m
}

/// The fork beam along `x`: `u = 0` at node 0, `v = w = phi = 0` at both ends.
fn fork(n: usize, section: &BeamSection) -> SolidModel {
    let mut m = mesh_along(n, section, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], steel());
    for c in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    for c in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", c, 0.0)).unwrap();
    }
    m
}

/// The torsion-only model: the fork beam with the components 0, 1, 2, 4, 5 held at every node.
fn torsion_only(n: usize, section: &BeamSection) -> SolidModel {
    let mut m = fork(n, section);
    for c in [0, 1, 2, 4, 5] {
        m.add_constraint(Constraint::displacement("nodes", c, 0.0)).unwrap();
    }
    m
}

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

/// C2 of ADR 0548: the internal torques left and right of a torque `t` at `a`, `phi(0) = phi'(0) = 0`, `phi(L) = phi''(L) = 0`.
fn exact_split(s: &BeamSection, cw: f64, a: f64, t: f64) -> (f64, f64) {
    let gj = g_mod() * s.torsion();
    let ecw = E * cw;
    let k = (gj / ecw).sqrt();
    let rows = |x: f64| {
        let ph = [1.0, x, (k * x).cosh(), (k * x).sinh()];
        let d1 = [0.0, 1.0, k * (k * x).sinh(), k * (k * x).cosh()];
        let d2 = [0.0, 0.0, k * k * (k * x).cosh(), k * k * (k * x).sinh()];
        let d3 = [0.0, 0.0, k.powi(3) * (k * x).sinh(), k.powi(3) * (k * x).cosh()];
        let tq: Vec<f64> = (0..4).map(|i| gj * d1[i] - ecw * d3[i]).collect();
        (ph, d1, d2, tq)
    };
    let mut m = vec![vec![0.0; 8]; 8];
    let mut b = vec![0.0; 8];
    let (p0, d0, _, _) = rows(0.0);
    let (pl, _, ddl, _) = rows(L);
    let (pa, da, dda, ta) = rows(a);
    for i in 0..4 {
        m[0][i] = p0[i];
        m[1][i] = d0[i];
        m[2][4 + i] = pl[i];
        m[3][4 + i] = ddl[i];
        m[4][i] = pa[i];
        m[4][4 + i] = -pa[i];
        m[5][i] = da[i];
        m[5][4 + i] = -da[i];
        m[6][i] = dda[i];
        m[6][4 + i] = -dda[i];
        m[7][i] = ta[i];
        m[7][4 + i] = -ta[i];
    }
    b[7] = t;
    let c = solve_dense(m, b);
    let tl: f64 = (0..4).map(|i| ta[i] * c[i]).sum();
    let tr: f64 = (0..4).map(|i| ta[i] * c[4 + i]).sum();
    (tl, tr)
}

#[test]
fn s1_to_s7_static_solve() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let gj = g_mod() * s.torsion();
    let k = (gj / (E * ws.c_w())).sqrt();
    let want = T / gj * (L - (k * L).tanh() / k);
    let x = [1.0, 0.0, 0.0];
    let twist_at_tip = |n: usize, held: bool, dir: Vec3, orient: Vec3| {
        let mut m = cantilever(n, &s, dir, orient);
        m.add_load(Load::nodal_moment("xl", [T * dir[0], T * dir[1], T * dir[2]]))
            .unwrap();
        let mut w = Warping::new().block(0, ws);
        if held {
            w = w.restrain("x0");
        }
        let r = solve_static(&m, &w, None).unwrap();
        let dofs = m.dof_map();
        let tip: f64 = (0..3)
            .map(|c| dir[c] * r.displacement[dofs.equation(n, 3 + c).unwrap()])
            .sum();
        (tip, r, m)
    };
    let (p16, r16, m16) = twist_at_tip(16, true, x, [0.0, 1.0, 0.0]);
    println!(
        "S1 phi(L) {p16:.10e}, closed {want:.10e}, signed {:.4e}",
        p16 / want - 1.0
    );
    c.at_most("S1 |phi(L)/C1 - 1| (acceptance, 16 elements)", rel(p16, want), 1e-3);
    let (pf, _, _) = twist_at_tip(16, false, x, [0.0, 1.0, 0.0]);
    println!("S2 phi(L) {pf:.10e}, T L/(G J) {:.10e}", T * L / gj);
    c.at_most("S2 root warping free", rel(pf, T * L / gj), 1e-10);
    let b = r16.reactions[r16.dofs.equation(0).unwrap()];
    let bw = T * (k * L).tanh() / k;
    println!("S3 root bimoment {b:.10e}, closed |B| {bw:.10e}");
    c.at_most("S3 root bimoment", rel(b.abs(), bw), 1e-4);
    let (p8, _, _) = twist_at_tip(8, true, x, [0.0, 1.0, 0.0]);
    c.at_most("S4 8 elements", rel(p8, want), 1e-3);
    let q = 1.0 / 2f64.sqrt();
    let (ps, _, _) = twist_at_tip(16, true, [q, q, 0.0], [-q, q, 0.0]);
    println!("S5 skew phi(L) {ps:.15e}, axis x {p16:.15e}");
    c.at_most("S5 frame invariance", rel(ps, p16), 1e-9);
    // S6: a torsion-free load.
    let mut m6 = cantilever(16, &s, x, [0.0, 1.0, 0.0]);
    m6.add_load(Load::nodal_force("xl", [1e4, 200.0, -300.0])).unwrap();
    m6.add_load(Load::nodal_moment("xl", [0.0, 50.0, -20.0])).unwrap();
    let w = Warping::new().block(0, ws).restrain("x0");
    let r6 = solve_static(&m6, &w, None).unwrap();
    let plain = rcfd_fem::solve::solve_static(&m6, &StaticOptions::default()).unwrap();
    let up = plain.displacement_values();
    let umax = up.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    let d = r6
        .six_unknown()
        .iter()
        .zip(up)
        .fold(0.0f64, |a, (x, y)| a.max((x - y).abs()));
    c.at_most("S6 six unknowns against the plain static solve", d / umax, 1e-10);
    let chi = (0..=16).fold(0.0f64, |a, nd| a.max(r6.warping_at(nd).unwrap().abs()));
    c.holds(
        "S6 warping unknowns of a torsion-free load",
        chi <= 1e-12 * umax,
        &format!("max |chi| {chi:e}, 1e-12 max|u| {:e}", 1e-12 * umax),
    );
    // S7.
    let r3 = r16.reactions[m16.dof_map().equation(0, 3).unwrap()];
    println!("S7 twist reaction {r3:.15e}");
    c.at_most("S7 twist reaction against -T", rel(r3, -T), 1e-10);
    c.finish();
}

/// The T2 set-ups: the fork beam with the warping held at `x = 0`, loaded by end moments `-/+m_ref` about `e_2` and the
/// torque `t_ref` about `x` at node 4.
fn t2_loaded(s: &BeamSection, m_ref: f64, t_ref: f64) -> SolidModel {
    let mut l = fork(16, s);
    if m_ref != 0.0 {
        l.add_load(Load::nodal_moment("x0", [0.0, -m_ref, 0.0])).unwrap();
        l.add_load(Load::nodal_moment("xl", [0.0, m_ref, 0.0])).unwrap();
    }
    if t_ref != 0.0 {
        l.add_load(Load::nodal_moment("ta", [t_ref, 0.0, 0.0])).unwrap();
    }
    l
}

fn t2_warping(ws: WarpingSection) -> Warping {
    Warping::new().block(0, ws).restrain("x0")
}

fn from_static(loaded: &SolidModel, w: &Warping) -> WarpingPrestress {
    let r = solve_static(loaded, w, None).unwrap();
    WarpingPrestress::from_static(loaded, w, &r, [0.0; 2]).unwrap()
}

/// The Saint-Venant pre-stress: the plain static solve with the resultant terms.
fn sv_prestress(loaded: &SolidModel) -> Prestress {
    let r = rcfd_fem::solve::solve_static(loaded, &StaticOptions::default()).unwrap();
    Prestress::from_static(loaded, &r)
        .unwrap()
        .with_beam_resultants(loaded, r.displacement_values(), [0.0; 2])
        .unwrap()
}

/// The hand pre-stress of T2: the plain static displacement with its twist replaced by the twist whose element torques are
/// those of C2 for `t_ref` at node 4.
fn hand_prestress(loaded: &SolidModel, s: &BeamSection, cw: f64, t_ref: f64) -> Prestress {
    let n = 16;
    let r = rcfd_fem::solve::solve_static(loaded, &StaticOptions::default()).unwrap();
    let (tl, tr) = exact_split(s, cw, L / 4.0, t_ref);
    let gj = g_mod() * s.torsion();
    let h = L / n as f64;
    let dofs = loaded.dof_map();
    let mut u = r.displacement_values().to_vec();
    let mut phi = 0.0;
    for i in 0..=n {
        u[dofs.equation(i, 3).unwrap()] = phi;
        if i < n {
            phi += if i < n / 4 { tl } else { tr } * h / gj;
        }
    }
    Prestress::from_static(loaded, &r)
        .unwrap()
        .with_beam_resultants(loaded, &u, [0.0; 2])
        .unwrap()
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
fn t1_to_t7_torsion_in_the_buckling_pre_stress() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = t2_warping(ws);
    let opts = BucklingOptions::new(3);
    // T1.
    let l1 = t2_loaded(&s, 0.0, T);
    let p1 = from_static(&l1, &w);
    let (tl, tr) = exact_split(&s, ws.c_w(), L / 4.0, T);
    println!("T1 closed split {tl:.10e} {tr:.10e}");
    let mut worst = 0.0f64;
    for e in 0..16 {
        let (f, _) = p1.prestress().beam_terms(e).unwrap();
        let want = if e < 4 { tl } else { tr };
        worst = worst.max(rel(f[9], want));
        if e == 0 || e == 3 || e == 4 || e == 15 {
            println!("  element {e}: torque {:.10e} (and -f[3] {:.10e})", f[9], -f[3]);
        }
    }
    c.at_most("T1 torque split of the warping static solve", worst, 1e-4);
    // T2, T3.
    let base = fork(16, &s);
    let loaded = t2_loaded(&s, M_REF, T_REF);
    let lam = |p: &WarpingPrestress| {
        let r = buckling_factors_with(&base, p, &w, &opts).unwrap();
        println!("  factors {:?}", r.factors);
        r.factors[0]
    };
    let l_static = lam(&from_static(&loaded, &w));
    let l_hand = lam(&WarpingPrestress::with_torsion(hand_prestress(
        &loaded,
        &s,
        ws.c_w(),
        T_REF,
    )));
    let l_sv = lam(&WarpingPrestress::with_torsion(sv_prestress(&loaded)));
    println!("T2 lambda static {l_static:.12e}, hand {l_hand:.12e}, Saint-Venant {l_sv:.12e}");
    c.at_most(
        "T2 lambda(warping static)/lambda(closed split) - 1 (acceptance)",
        rel(l_static, l_hand),
        1e-5,
    );
    let d3 = rel(l_sv, l_hand);
    c.holds(
        "T3 the Saint-Venant split differs",
        d3 > 1e-2,
        &format!("{d3:.4e} > 1e-2"),
    );
    // T4.
    let l_m = lam(&from_static(&t2_loaded(&s, M_REF, 0.0), &w));
    let d4 = l_static / l_m - 1.0;
    c.holds(
        "T4 the torque acts",
        d4 < -0.1,
        &format!("{d4:.4e} < -0.1 (M only {l_m:.8e})"),
    );
    // T5.
    let l_neg = lam(&from_static(&t2_loaded(&s, M_REF, -T_REF), &w));
    c.at_most("T5 |lambda(+T)/lambda(-T) - 1|", rel(l_static, l_neg), 1e-8);
    // T6.
    let sv = sv_prestress(&loaded);
    let k1 = buckling_factors(&base, &sv, &w, &opts).err().map(|e| key_of(&e));
    c.holds(
        "T6 ADR 0546 buckling_factors refuses the torque",
        k1.as_deref() == Some("warping"),
        &format!("{k1:?}"),
    );
    let k2 = buckling_factors_with(&base, &WarpingPrestress::torsion_free(sv), &w, &opts)
        .err()
        .map(|e| key_of(&e));
    c.holds(
        "T6 torsion_free refuses the torque",
        k2.as_deref() == Some("warping"),
        &format!("{k2:?}"),
    );
    // T7: the ADR 0546 A1 beam.
    let wf = Warping::new().block(0, ws);
    let a1 = t2_loaded(&s, M_REF, 0.0);
    let f0 = buckling_factors(&base, &sv_prestress(&a1), &wf, &opts).unwrap().factors[0];
    let f1 = buckling_factors_with(&base, &from_static(&a1, &wf), &wf, &opts)
        .unwrap()
        .factors[0];
    println!("T7 ADR 0546 {f0:.15e}, warping static {f1:.15e}");
    c.at_most("T7 torsion-free against ADR 0546", rel(f1, f0), 1e-9);
    c.finish();
}

fn w2_closed(s: &BeamSection, cw: f64, m: usize, n_ax: f64) -> f64 {
    let k = m as f64 * PI / L;
    let ip = s.i_y() + s.i_z();
    ((g_mod() * s.torsion() + n_ax * ip / s.area()) * k * k + E * cw * k.powi(4)) / (RHO * ip)
}

#[test]
fn f1_to_f5_modes() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let r = modal_analysis(&torsion_only(16, &s), &w, &ModalOptions::new(3)).unwrap();
    println!("F1 eigenvalues {:?}", r.eigenvalues);
    for (m, limit) in [(1, 1e-4), (2, 1e-3), (3, 5e-3)] {
        let want = w2_closed(&s, ws.c_w(), m, 0.0);
        println!("F1 m {m}: {:.10e}, closed {want:.10e}", r.eigenvalues[m - 1]);
        c.at_most(&format!("F1 m {m}"), rel(r.eigenvalues[m - 1], want), limit);
    }
    let r8 = modal_analysis(&torsion_only(8, &s), &w, &ModalOptions::new(1)).unwrap();
    c.at_most(
        "F2 8 elements m 1",
        rel(r8.eigenvalues[0], w2_closed(&s, ws.c_w(), 1, 0.0)),
        1e-3,
    );
    let sv = WarpingSection::saint_venant();
    let r0 = modal_analysis(
        &torsion_only(16, &s),
        &Warping::new().block(0, sv),
        &ModalOptions::new(1),
    )
    .unwrap();
    let w0 = w2_closed(&s, 0.0, 1, 0.0);
    c.at_most("F3 C_w = 0 against G J k^2/(rho I_p)", rel(r0.eigenvalues[0], w0), 1e-4);
    let plain = rcfd_fem::modal::modal_analysis(&torsion_only(16, &s), &ModalOptions::new(1)).unwrap();
    println!("F3 plain (linear twist) {:.10e}", plain.eigenvalues[0]);
    c.at_most(
        "F3 against the plain modes",
        rel(r0.eigenvalues[0], plain.eigenvalues[0]),
        1e-2,
    );
    let full = modal_analysis(&fork(16, &s), &w, &ModalOptions::new(4)).unwrap();
    let w1 = w2_closed(&s, ws.c_w(), 1, 0.0);
    println!("F4 full eigenvalues {:?}", full.eigenvalues);
    c.at_most("F4 mode 2 of the full beam", rel(full.eigenvalues[1], w1), 1e-4);
    c.holds(
        "F4 modes 1 and 3 apart",
        full.eigenvalues[0] < 0.9 * w1 && full.eigenvalues[2] > 1.1 * w1,
        &format!("{:.4e}, {:.4e}", full.eigenvalues[0] / w1, full.eigenvalues[2] / w1),
    );
    let rr = modal_analysis(
        &torsion_only(16, &s),
        &Warping::new().block(0, ws).restrain("x0").restrain("xl"),
        &ModalOptions::new(1),
    )
    .unwrap();
    let wr = rr.eigenvalues[0];
    c.holds(
        "F5 warping held at both ends",
        1.2 * r.eigenvalues[0] < wr && wr < 158870.0 * (1.0 + 1e-2),
        &format!(
            "{wr:.6e} (free {:.6e}, ratio {:.4})",
            r.eigenvalues[0],
            wr / r.eigenvalues[0]
        ),
    );
    c.finish();
}

#[test]
fn f6_f7_element_mass() {
    let mut c = Checks::new();
    let (s, _) = i_parts();
    let mat = steel();
    let l = 0.7;
    let m14 = beam_warping::local_mass(l, &s, &mat);
    let ip = s.i_y() + s.i_z();
    let h = [
        [156.0, 22.0 * l, 54.0, -13.0 * l],
        [22.0 * l, 4.0 * l * l, 13.0 * l, -3.0 * l * l],
        [54.0, 13.0 * l, 156.0, -22.0 * l],
        [-13.0 * l, -3.0 * l * l, -22.0 * l, 4.0 * l * l],
    ];
    let f = RHO * ip * l / 420.0;
    let mut dmax = 0.0f64;
    let mut hmax = 0.0f64;
    for a in 0..4 {
        for b in 0..4 {
            dmax = dmax.max((m14.get(TWIST[a], TWIST[b]) - f * h[a][b]).abs());
            hmax = hmax.max((f * h[a][b]).abs());
        }
    }
    c.at_most("F6 twist block of the mass against the hand matrix", dmax / hmax, 1e-12);
    let m12 = beam::local_mass(l, &s, &mat);
    let idx = |i: usize| if i < 6 { i } else { i + 1 };
    let mut emb = vec![0.0; 196];
    for i in 0..12 {
        for j in 0..12 {
            emb[idx(i) * 14 + idx(j)] = m12.values()[i * 12 + j];
        }
    }
    let mut bits = true;
    for i in 0..14 {
        for j in 0..14 {
            if TWIST.contains(&i) && TWIST.contains(&j) {
                continue;
            }
            bits &= m14.values()[i * 14 + j].to_bits() == emb[i * 14 + j].to_bits();
        }
    }
    c.holds("F6 other entries bit-equal to beam::local_mass", bits, "");
    let mut x = [0.0; 14];
    x[3] = 1.0;
    x[10] = 1.0;
    let mut xmx = 0.0;
    for i in 0..14 {
        for j in 0..14 {
            xmx += x[i] * m14.values()[i * 14 + j] * x[j];
        }
    }
    c.at_most("F6 rigid twist kinetic", rel(xmx, RHO * ip * l), 1e-12);
    // F7.
    let q = 1.0 / 2f64.sqrt();
    let mx = beam_warping::element_mass(&[[0.0; 3], [l, 0.0, 0.0]], &s, [0.0, 1.0, 0.0], &mat).unwrap();
    let ms = beam_warping::element_mass(&[[0.0; 3], [l * q, l * q, 0.0]], &s, [-q, q, 0.0], &mat).unwrap();
    let r = [[q, q, 0.0], [-q, q, 0.0], [0.0, 0.0, 1.0]];
    let mut tm = [[0.0; 14]; 14];
    for node in 0..2 {
        let o = 7 * node;
        for blk in 0..2 {
            for a in 0..3 {
                for b in 0..3 {
                    tm[o + 3 * blk + a][o + 3 * blk + b] = r[a][b];
                }
            }
        }
        tm[o + 6][o + 6] = 1.0;
    }
    let mut d = 0.0f64;
    let mut big = 0.0f64;
    for i in 0..14 {
        for j in 0..14 {
            let mut v = 0.0;
            for a in 0..14 {
                for b in 0..14 {
                    v += tm[a][i] * mx.values()[a * 14 + b] * tm[b][j];
                }
            }
            d = d.max((ms.values()[i * 14 + j] - v).abs());
            big = big.max(ms.values()[i * 14 + j].abs());
        }
    }
    c.at_most("F7 frame invariance of the mass", d / big, 1e-12);
    c.finish();
}

fn axial_prestress(s: &BeamSection, w: &Warping, n_ax: f64) -> WarpingPrestress {
    let mut l = fork(16, s);
    l.add_load(Load::nodal_force("xl", [n_ax, 0.0, 0.0])).unwrap();
    from_static(&l, w)
}

#[test]
fn p1_to_p3_prestressed_modes() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let ip = s.i_y() + s.i_z();
    let k1 = PI / L;
    let n_t = (g_mod() * s.torsion() + E * ws.c_w() * k1 * k1) * s.area() / ip;
    println!("N_T {n_t:.8e}");
    for frac in [0.5, -0.5] {
        let n_ax = frac * n_t;
        let p = axial_prestress(&s, &w, n_ax);
        let r = prestressed_modes(&torsion_only(16, &s), &p, &w, &ModalOptions::new(1)).unwrap();
        let want = w2_closed(&s, ws.c_w(), 1, n_ax);
        println!("P1 N {n_ax:.6e}: {:.10e}, closed {want:.10e}", r.eigenvalues[0]);
        c.at_most(&format!("P1 N = {frac} |N_T|"), rel(r.eigenvalues[0], want), 1e-4);
    }
    let p = axial_prestress(&s, &w, 0.5 * n_t);
    let full = prestressed_modes(&fork(16, &s), &p, &w, &ModalOptions::new(4)).unwrap();
    println!("P2 full eigenvalues {:?}", full.eigenvalues);
    c.at_most(
        "P2 mode 2 of the full beam in tension",
        rel(full.eigenvalues[1], w2_closed(&s, ws.c_w(), 1, 0.5 * n_t)),
        1e-4,
    );
    let tm = torsion_only(16, &s);
    let zero = WarpingPrestress::torsion_free(Prestress::uniform(&tm, [0.0; 6]).unwrap());
    let a = prestressed_modes(&tm, &zero, &w, &ModalOptions::new(3)).unwrap();
    let b = modal_analysis(&tm, &w, &ModalOptions::new(3)).unwrap();
    let d = a
        .eigenvalues
        .iter()
        .zip(&b.eigenvalues)
        .fold(0.0f64, |m, (x, y)| m.max(rel(*x, *y)));
    c.at_most("P3 zero pre-stress against the modes", d, 1e-12);
    c.finish();
}

#[test]
fn pt1_to_pt3_prestressed_modes_with_torque() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = t2_warping(ws);
    let base = fork(16, &s);
    let loaded = t2_loaded(&s, M_REF, T_REF);
    let opts = ModalOptions::new(4);
    let ev = |p: &WarpingPrestress| {
        let r = prestressed_modes(&base, &p.scaled(30.0), &w, &opts).unwrap();
        println!("  eigenvalues {:?}", r.eigenvalues);
        r.eigenvalues
    };
    let e_static = ev(&from_static(&loaded, &w));
    let e_hand = ev(&WarpingPrestress::with_torsion(hand_prestress(
        &loaded,
        &s,
        ws.c_w(),
        T_REF,
    )));
    let e_sv = ev(&WarpingPrestress::with_torsion(sv_prestress(&loaded)));
    for i in 0..3 {
        c.at_most(&format!("PT1 mode {}", i + 1), rel(e_static[i], e_hand[i]), 1e-5);
    }
    let d = rel(e_sv[0], e_hand[0]);
    c.holds(
        "PT2 the Saint-Venant split differs",
        d > 5e-3,
        &format!("{d:.4e} > 5e-3"),
    );
    let k = prestressed_modes(&base, &WarpingPrestress::torsion_free(sv_prestress(&loaded)), &w, &opts)
        .err()
        .map(|e| key_of(&e));
    c.holds(
        "PT3 torsion_free refuses the torque",
        k.as_deref() == Some("warping"),
        &format!("{k:?}"),
    );
    c.finish();
}

#[test]
fn r_refusals() {
    let mut c = Checks::new();
    let (s, ws) = i_parts();
    let w = Warping::new().block(0, ws);
    let mut check = |name: &str, got: Option<String>, want: &str| {
        c.holds(name, got.as_deref() == Some(want), &format!("{got:?} (want {want})"));
    };
    let err = |r: Result<(), FemError>| r.err().map(|e| key_of(&e));
    // RF1: a Shell3 block.
    {
        let mut mesh = SolidMesh::new(vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
            [6.0, 0.0, 0.0],
            [5.0, 1.0, 0.0],
        ])
        .unwrap();
        mesh.add_block(ElementKind::Beam2, vec![0, 1]).unwrap();
        mesh.set_block_property(0, BlockProperty::beam(s, [0.0, 1.0, 0.0]).unwrap())
            .unwrap();
        mesh.add_block(ElementKind::Shell3, vec![2, 3, 4]).unwrap();
        mesh.set_block_property(1, BlockProperty::shell(0.01).unwrap()).unwrap();
        mesh.add_element_set("all", vec![0, 1]).unwrap();
        let mut m = SolidModel::new(mesh);
        m.add_material("all", steel()).unwrap();
        check(
            "R RF1 a Shell3 block",
            err(solve_static(&m, &Warping::new().block(1, ws), None).map(|_| ())),
            "warping",
        );
    }
    // RF2: two blocks, warping on block 0 only.
    {
        let n = 8;
        let nodes: Vec<Vec3> = (0..=n).map(|i| [L * i as f64 / n as f64, 0.0, 0.0]).collect();
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
        let mut m = SolidModel::new(mesh);
        m.add_material("all", steel()).unwrap();
        for comp in 0..6 {
            m.add_constraint(Constraint::displacement("x0", comp, 0.0)).unwrap();
        }
        check(
            "R RF2 shared node",
            err(solve_static(&m, &w, None).map(|_| ())),
            "warping",
        );
    }
    // RF4.
    {
        let mut m = cantilever(8, &s, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        m.add_support(ElasticSupport::nodal_spring("xl", 0, 1e6)).unwrap();
        check(
            "R RF4 elastic support",
            err(solve_static(&m, &w, None).map(|_| ())),
            "warping",
        );
    }
    // RF6.
    let cant = cantilever(8, &s, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    check(
        "R RF6 unknown restrained set",
        err(solve_static(&cant, &Warping::new().block(0, ws).restrain("nope"), None).map(|_| ())),
        "UnknownSet",
    );
    // from_static with another numbering.
    {
        let r8 = solve_static(&cant, &w, None).unwrap();
        let m16 = cantilever(16, &s, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        check(
            "R from_static of another numbering",
            err(WarpingPrestress::from_static(&m16, &w, &r8, [0.0; 2]).map(|_| ())),
            "warping",
        );
    }
    // RF10.
    {
        let base = fork(16, &s);
        let mut loaded = base.clone();
        loaded.add_load(Load::nodal_moment("x0", [0.0, -M_REF, 0.0])).unwrap();
        loaded.add_load(Load::nodal_moment("xl", [0.0, M_REF, 0.0])).unwrap();
        let p = from_static(&loaded, &w);
        let other = Warping::new().block(0, WarpingSection::saint_venant());
        check(
            "R RF10 other sections",
            err(buckling_factors_with(&base, &p, &other, &BucklingOptions::new(3)).map(|_| ())),
            "warping",
        );
    }
    let tm = torsion_only(8, &s);
    check(
        "R modes coordinate_order",
        err(modal_analysis(&tm, &w, &ModalOptions::new(1).coordinate_order(true)).map(|_| ())),
        "factorisation",
    );
    {
        let mut m = tm.clone();
        m.add_constraint(Constraint::displacement("ta", 3, 1e-3)).unwrap();
        check(
            "R modes nonzero prescribed value",
            err(modal_analysis(&m, &w, &ModalOptions::new(1)).map(|_| ())),
            "constraint",
        );
    }
    {
        let mut m = mesh_along(
            8,
            &s,
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            Material::isotropic_elastic(E, NU, 0.0, 0.0).unwrap(),
        );
        for comp in 0..6 {
            m.add_constraint(Constraint::displacement("x0", comp, 0.0)).unwrap();
        }
        check(
            "R modes massless",
            err(modal_analysis(&m, &w, &ModalOptions::new(1)).map(|_| ())),
            "density",
        );
    }
    c.finish();
}
