//! F-663 (ADR 0588): distributed loads at a height on warping beams. Checks A1 to A8, E1, L0, L1, L2, N1 and R of the
//! registration (B1 is `sa_beam_warping_f663_bits.rs`). Each registered run is made once; every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    buckling_factors, buckling_factors_with, matrices, solve_static, Warping, WarpingPrestress, WarpingSection,
};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use std::collections::HashMap;

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
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value < limit;
        println!(
            "{name}: {value:.4e} (limit < {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} >= {limit:e}"));
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
const Q_REF: f64 = 1000.0;
const L: f64 = 3.0;

/// The flange mid-planes of the doubly symmetric I (`h_c/2`) and flange 1 of the monosymmetric I, from the centroid.
const DS_FLANGE: f64 = 0.095;
const MS_TOP: f64 = 0.0718681318681;

/// The Rayleigh-Ritz references of D4 (`N = 81`), `lambda` for `Q_REF`.
const REF_DS_TOP: f64 = 45.1856414209;
const REF_DS_SC: f64 = 59.0640165887;
const REF_DS_BOT: f64 = 77.1475881351;
const REF_MS_SC_DOWN: f64 = 135.747863965;
const REF_MS_SC_UP: f64 = 69.3139026678;
const REF_MS_C_DOWN: f64 = 163.396623703;
const REF_MS_TOP_DOWN: f64 = 126.453805543;
/// The monosymmetric I at the shear centre, downward, with `beta_x = 0` (N1).
const REF_MS_BX0: f64 = 98.619;

fn steel() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn dsym() -> (BeamSection, WarpingSection) {
    WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, KAPPA).unwrap()
}

fn mono() -> (BeamSection, WarpingSection) {
    WarpingSection::monosymmetric_i(0.15, 0.012, 0.08, 0.008, 0.2, 0.006, KAPPA).unwrap()
}

fn orient(axis: usize) -> Vec3 {
    if axis == 0 {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// The section axis `e_3` of the beam along `axis` (`e_1 x v`): `z` for axis x, `x` for axis y.
fn e3(axis: usize) -> Vec3 {
    if axis == 0 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    }
}

/// The global components of the local `(u, v, w, phi)` for the beam along `axis`.
fn comps(axis: usize) -> [usize; 4] {
    if axis == 0 {
        [0, 1, 2, 3]
    } else {
        [1, 2, 0, 4]
    }
}

/// A beam of `n` elements along `axis`, element set `all`, node sets `x0`, `xl`, `interior` (nodes 1 to n - 1) and `ends`;
/// fork supports (`u = 0` at node 0, `v = w = phi = 0` at both ends), warping free.
fn fork(n: usize, section: &BeamSection, axis: usize) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=n)
        .map(|i| {
            let mut p = [0.0; 3];
            p[axis] = L * i as f64 / n as f64;
            p
        })
        .collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(*section, orient(axis)).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    mesh.add_node_set("interior", (1..n).collect()).unwrap();
    mesh.add_node_set("ends", vec![0, n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    let c = comps(axis);
    for k in c {
        m.add_constraint(Constraint::displacement("x0", k, 0.0)).unwrap();
    }
    for &k in &c[1..] {
        m.add_constraint(Constraint::displacement("xl", k, 0.0)).unwrap();
    }
    m
}

fn six_prestress(loaded: &SolidModel) -> Prestress {
    let r = rcfd_fem::solve::solve_static(loaded, &StaticOptions::default()).unwrap();
    Prestress::from_static(loaded, &r)
        .unwrap()
        .with_beam_resultants(loaded, r.displacement_values(), [0.0; 2])
        .unwrap()
}

/// The uniform load `s Q_REF e_3` per length (`s = -1` downward).
fn udl(axis: usize, s: f64) -> Vec3 {
    let e = e3(axis);
    [s * Q_REF * e[0], s * Q_REF * e[1], s * Q_REF * e[2]]
}

fn line_loaded(m: &SolidModel, q: Vec3) -> SolidModel {
    let mut l = m.clone();
    l.add_load(Load::line_load("all", q)).unwrap();
    l
}

/// The lowest factor of the fork beam under the UDL `s Q_REF e_3`, with the distributed load point `point` (none: no entry).
fn uniform(section: &BeamSection, ws: WarpingSection, n: usize, axis: usize, s: f64, point: Option<[f64; 2]>) -> f64 {
    let m = fork(n, section, axis);
    let q = udl(axis, s);
    let pre = six_prestress(&line_loaded(&m, q));
    let mut w = Warping::new().block(0, ws);
    if let Some(p) = point {
        w = w.line_load_point("all", q, p);
    }
    let r = buckling_factors(&m, &pre, &w, &BucklingOptions::new(3)).unwrap();
    println!(
        "  factors {:?} (n {n}, axis {axis}, sign {s:+}, point {point:?}), Sturm {:?}",
        r.factors, r.sturm
    );
    r.factors[0]
}

/// The same with the height lumped into ADR 0582 point entries (`q l` at the interior nodes, `q l/2` at the ends).
fn uniform_lumped(section: &BeamSection, ws: WarpingSection, n: usize, s: f64, point: [f64; 2]) -> f64 {
    let m = fork(n, section, 0);
    let q = udl(0, s);
    let pre = six_prestress(&line_loaded(&m, q));
    let l = L / n as f64;
    let f_in = [q[0] * l, q[1] * l, q[2] * l];
    let f_end = [0.5 * f_in[0], 0.5 * f_in[1], 0.5 * f_in[2]];
    let w = Warping::new()
        .block(0, ws)
        .load_point("interior", f_in, point)
        .load_point("ends", f_end, point);
    let r = buckling_factors(&m, &pre, &w, &BucklingOptions::new(3)).unwrap();
    println!("  lumped factors {:?} (n {n}, sign {s:+}, point {point:?})", r.factors);
    r.factors[0]
}

#[test]
fn a_l0_n1_uniform_load_at_a_height() {
    let mut c = Checks::new();
    let (ds, dws) = dsym();
    let (ms, mws) = mono();
    let z_0 = mws.shear_centre()[1];
    println!("z_0 {z_0:.12e}, beta_x {:.12e}", mws.wagner_about_shear_centre()[0]);
    let top = DS_FLANGE;
    // A1 to A3 at 16 elements.
    let ds_top = uniform(&ds, dws, 16, 0, -1.0, Some([0.0, top]));
    let ds_sc = uniform(&ds, dws, 16, 0, -1.0, None);
    let ds_bot = uniform(&ds, dws, 16, 0, -1.0, Some([0.0, -top]));
    for (name, f, want) in [
        ("A1 DS on the top flange", ds_top, REF_DS_TOP),
        ("A2 DS at the shear centre", ds_sc, REF_DS_SC),
        ("A3 DS on the bottom flange", ds_bot, REF_DS_BOT),
    ] {
        println!(
            "{name}: lambda {f:.10e}, reference {want:.10e}, signed {:.4e}",
            f / want - 1.0
        );
        c.at_most(&format!("{name} (16 elements)"), rel(f, want), 2e-2);
    }
    // A4: the offset path.
    let ms_down = uniform(&ms, mws, 16, 0, -1.0, None);
    let ms_up = uniform(&ms, mws, 16, 0, 1.0, None);
    let ms_c_down = uniform(&ms, mws, 16, 0, -1.0, Some([0.0, 0.0]));
    let ms_top_down = uniform(&ms, mws, 16, 0, -1.0, Some([0.0, MS_TOP]));
    for (name, f, want) in [
        ("A4 MS at the shear centre, downward", ms_down, REF_MS_SC_DOWN),
        ("A4 MS at the shear centre, upward", ms_up, REF_MS_SC_UP),
        ("A4 MS at the centroid, downward", ms_c_down, REF_MS_C_DOWN),
        ("A4 MS on flange 1, downward", ms_top_down, REF_MS_TOP_DOWN),
    ] {
        println!(
            "{name}: lambda {f:.10e}, reference {want:.10e}, signed {:.4e}",
            f / want - 1.0
        );
        c.at_most(name, rel(f, want), 2e-2);
    }
    // A5: 8 elements.
    for (name, p, want) in [
        ("A5 DS on the top flange", Some([0.0, top]), REF_DS_TOP),
        ("A5 DS at the shear centre", None, REF_DS_SC),
        ("A5 DS on the bottom flange", Some([0.0, -top]), REF_DS_BOT),
    ] {
        let f = uniform(&ds, dws, 8, 0, -1.0, p);
        println!("{name} (8 elements): lambda {f:.10e}, signed {:.4e}", f / want - 1.0);
        c.at_most(&format!("{name} (8 elements)"), rel(f, want), 2e-2);
    }
    // A6: along global y.
    let ds_top_y = uniform(&ds, dws, 16, 1, -1.0, Some([0.0, top]));
    c.at_most("A6 axis y, DS on the top flange", rel(ds_top_y, ds_top), 1e-6);
    // A7: the pre-stress of the warping static solve.
    for (name, sec, ws, p, f_ref) in [
        ("DS on the top flange", &ds, dws, [0.0, top], ds_top),
        ("MS at the centroid, downward", &ms, mws, [0.0, 0.0], ms_c_down),
    ] {
        let m = fork(16, sec, 0);
        let q = udl(0, -1.0);
        let loaded = line_loaded(&m, q);
        let w = Warping::new().block(0, ws).line_load_point("all", q, p);
        let r = solve_static(&loaded, &w, None).unwrap();
        let pre = WarpingPrestress::from_static(&loaded, &w, &r, [0.0; 2]).unwrap();
        let got = buckling_factors_with(&m, &pre, &w, &BucklingOptions::new(3))
            .unwrap()
            .factors[0];
        println!("A7 {name}: {got:.12e} against {f_ref:.12e}");
        c.at_most(
            &format!("A7 from_static against six-unknown ({name})"),
            rel(got, f_ref),
            1e-9,
        );
    }
    // A8: lumped against consistent.
    for (name, p, f_ref) in [
        ("DS on the top flange", [0.0, top], ds_top),
        ("DS on the bottom flange", [0.0, -top], ds_bot),
    ] {
        let f = uniform_lumped(&ds, dws, 16, -1.0, p);
        let d = rel(f, f_ref);
        println!("A8 {name}: lumped {f:.12e}, consistent {f_ref:.12e}, d {d:.4e}");
        c.above(&format!("A8 lumped differs ({name})"), d, 1e-8);
        c.at_most(&format!("A8 lumped within ({name})"), d, 1e-4);
    }
    // L0: an entry at the shear centre adds nothing.
    let ds_sc_entry = uniform(&ds, dws, 16, 0, -1.0, Some([0.0, 0.0]));
    let ms_down_entry = uniform(&ms, mws, 16, 0, -1.0, Some([0.0, z_0]));
    c.holds(
        "L0 DS entry at (0, 0) against no entry",
        ds_sc_entry.to_bits() == ds_sc.to_bits(),
        &format!("{ds_sc_entry:?} against {ds_sc:?}"),
    );
    c.holds(
        "L0 MS entry at (0, z_0) against no entry",
        ms_down_entry.to_bits() == ms_down.to_bits(),
        &format!("{ms_down_entry:?} against {ms_down:?}"),
    );
    // N1: negative controls.
    c.below("N1 DS top/SC", ds_top / ds_sc, 0.85);
    c.above("N1 DS bottom/SC", ds_bot / ds_sc, 1.2);
    c.above("N1 MS down/up", ms_down / ms_up, 1.7);
    c.above("N1 MS against beta_x = 0", rel(ms_down, REF_MS_BX0), 0.2);
    c.finish();
}

#[test]
fn e1_end_forces_under_the_line_load() {
    let mut c = Checks::new();
    let (ds, _) = dsym();
    let n = 16;
    let m = fork(n, &ds, 0);
    let q_z = -Q_REF;
    let pre = six_prestress(&line_loaded(&m, udl(0, -1.0)));
    let l = L / n as f64;
    let mm = |x: f64| 0.5 * q_z * x * (L - x);
    let vv = |x: f64| 0.5 * q_z * (L - 2.0 * x);
    let scale = Q_REF * L * L / 8.0;
    let mut worst: f64 = 0.0;
    for e in 0..n {
        let (f, _) = pre.beam_terms(e).expect("end forces");
        let (x0, x1) = (e as f64 * l, (e + 1) as f64 * l);
        let xm = 0.5 * (x0 + x1);
        let shift = q_z * l * l / 12.0;
        let errs = [
            -f[2] - vv(xm),
            f[8] - vv(xm),
            -f[4] - (mm(x0) + shift),
            f[10] - (mm(x1) + shift),
        ];
        if e == 0 || e == 8 {
            println!(
                "E1 element {e}: r0 V {:.9e} M {:.9e}; r1 V {:.9e} M {:.9e}; expected V {:.9e}, M {:.9e} {:.9e}",
                -f[2],
                -f[4],
                f[8],
                f[10],
                vv(xm),
                mm(x0) + shift,
                mm(x1) + shift
            );
        }
        for v in errs {
            worst = worst.max(if v.is_finite() { v.abs() } else { f64::INFINITY });
        }
    }
    c.at_most("E1 end shears and moments against D3", worst / scale, 1e-9);
    c.finish();
}

/// The closed form `(l/420) [...]` of the Hermite block (D1).
fn hermite_block(l: f64) -> [[f64; 4]; 4] {
    let k = l / 420.0;
    [
        [156.0 * k, 22.0 * l * k, 54.0 * k, -13.0 * l * k],
        [22.0 * l * k, 4.0 * l * l * k, 13.0 * l * k, -3.0 * l * l * k],
        [54.0 * k, 13.0 * l * k, 156.0 * k, -22.0 * l * k],
        [-13.0 * l * k, -3.0 * l * l * k, -22.0 * l * k, 4.0 * l * l * k],
    ]
}

#[test]
fn l1_distributed_term_in_the_geometric_stiffness() {
    let mut c = Checks::new();
    let (ds, dws) = dsym();
    let n = 16;
    let m = fork(n, &ds, 0);
    let q = udl(0, -1.0);
    let pre = six_prestress(&line_loaded(&m, q));
    let w0 = Warping::new().block(0, dws);
    let w1 = w0.clone().line_load_point("all", q, [0.0, DS_FLANGE]);
    let a = matrices(&m, &pre, &w0, None).unwrap();
    let b = matrices(&m, &pre, &w1, None).unwrap();
    c.holds(
        "L1 stiffness bit-equal",
        a.stiffness == b.stiffness,
        "K with and without the entry",
    );
    let (ga, gb) = (&a.geometric, &b.geometric);
    c.holds(
        "L1 geometric pattern equal",
        ga.row_ptr == gb.row_ptr && ga.col == gb.col,
        "row_ptr and col",
    );
    let cc = -Q_REF * DS_FLANGE;
    let l = L / n as f64;
    let hb = hermite_block(l);
    let mut want: HashMap<(usize, usize), f64> = HashMap::new();
    for e in 0..n {
        let eqs = [
            m.dof_map().equation(e, 3).unwrap(),
            a.dofs.equation(e).unwrap(),
            m.dof_map().equation(e + 1, 3).unwrap(),
            a.dofs.equation(e + 1).unwrap(),
        ];
        for i in 0..4 {
            for j in 0..4 {
                *want.entry((eqs[i], eqs[j])).or_insert(0.0) += cc * hb[i][j];
            }
        }
    }
    let scale = want.values().fold(0.0f64, |s, v| s.max(v.abs()));
    let mut others_equal = true;
    let mut worst: f64 = 0.0;
    let mut seen = 0usize;
    for r in 0..ga.n {
        for p in ga.row_ptr[r]..ga.row_ptr[r + 1] {
            let col = ga.col[p];
            let d = gb.val[p] - ga.val[p];
            match want.get(&(r, col)) {
                Some(&w) => {
                    seen += 1;
                    worst = worst.max(if d.is_finite() { (d - w).abs() } else { f64::INFINITY });
                }
                None => {
                    if ga.val[p] != gb.val[p] {
                        others_equal = false;
                    }
                }
            }
        }
    }
    c.holds(
        "L1 geometric equal off the twist equations",
        others_equal,
        "every other entry",
    );
    c.holds(
        "L1 every twist entry in the pattern",
        seen == want.len(),
        &format!("{seen} of {}", want.len()),
    );
    println!("L1 scale {scale:.6e}, worst {worst:.4e}");
    c.at_most("L1 difference against c M_H", worst / scale, 1e-12);
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

/// Two disjoint fork beams along `x` (block 0 at `y = 0`, block 1 at `y = 1`, 16 elements each), element sets `all`, `b0`,
/// `b1`.
fn two_beams(section: &BeamSection) -> SolidModel {
    let n = 16;
    let mut nodes: Vec<Vec3> = (0..=n).map(|i| [L * i as f64 / n as f64, 0.0, 0.0]).collect();
    nodes.extend((0..=n).map(|i| [L * i as f64 / n as f64, 1.0, 0.0]));
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let o = n + 1;
    mesh.add_block(ElementKind::Beam2, (0..n).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    mesh.add_block(ElementKind::Beam2, (0..n).flat_map(|i| [o + i, o + i + 1]).collect())
        .unwrap();
    for b in 0..2 {
        mesh.set_block_property(b, BlockProperty::beam(*section, orient(0)).unwrap())
            .unwrap();
    }
    mesh.add_element_set("all", (0..2 * n).collect()).unwrap();
    mesh.add_element_set("b0", (0..n).collect()).unwrap();
    mesh.add_element_set("b1", (n..2 * n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0, o]).unwrap();
    mesh.add_node_set("xl", vec![n, o + n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for k in 0..4 {
        m.add_constraint(Constraint::displacement("x0", k, 0.0)).unwrap();
    }
    for k in 1..4 {
        m.add_constraint(Constraint::displacement("xl", k, 0.0)).unwrap();
    }
    m
}

/// One beam of two warping blocks: elements 0 to 7 with `s1`, 8 to 15 with `s2`; element set `all`.
fn two_sections(s1: &BeamSection, s2: &BeamSection) -> SolidModel {
    let n = 16;
    let nodes: Vec<Vec3> = (0..=n).map(|i| [L * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Beam2, (0..8).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    mesh.add_block(ElementKind::Beam2, (8..n).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    mesh.set_block_property(0, BlockProperty::beam(*s1, orient(0)).unwrap())
        .unwrap();
    mesh.set_block_property(1, BlockProperty::beam(*s2, orient(0)).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for k in 0..4 {
        m.add_constraint(Constraint::displacement("x0", k, 0.0)).unwrap();
    }
    for k in 1..4 {
        m.add_constraint(Constraint::displacement("xl", k, 0.0)).unwrap();
    }
    m
}

#[test]
fn l2_per_element_shear_centre() {
    let mut c = Checks::new();
    let (ds, dws) = dsym();
    let (ms, mws) = mono();
    let m = two_sections(&ms, &ds);
    let q = udl(0, -1.0);
    let pre = six_prestress(&line_loaded(&m, q));
    let w0 = Warping::new().block(0, mws).block(1, dws);
    let w1 = w0.clone().line_load_point("all", q, [0.0, 0.05]);
    let a = matrices(&m, &pre, &w0, None).unwrap();
    let b = matrices(&m, &pre, &w1, None).unwrap();
    let n_eq = a.dofs.n_equations();
    let mut u = vec![0.0; n_eq];
    for node in 0..=16 {
        u[m.dof_map().equation(node, 3).unwrap()] = 1.0;
    }
    let energy = |g: &rcfd_linalg::CsrMatrix| {
        let mut gu = vec![0.0; n_eq];
        g.matvec(&u, &mut gu);
        u.iter().zip(&gu).map(|(a, b)| a * b).sum::<f64>()
    };
    let got = energy(&b.geometric) - energy(&a.geometric);
    let l = L / 16.0;
    let z_0 = mws.shear_centre()[1];
    let want = 8.0 * l * (-Q_REF * (0.05 - z_0)) + 8.0 * l * (-Q_REF * 0.05);
    println!("L2 rigid-twist energy {got:.15e} against {want:.15e}");
    c.at_most("L2 per-element shear centre", rel(got, want), 1e-12);
    c.finish();
}

#[test]
fn r_refusals() {
    let mut c = Checks::new();
    let (ds, dws) = dsym();
    let (ms, mws) = mono();
    let m = fork(16, &ds, 0);
    let down = udl(0, -1.0);
    let pre = six_prestress(&line_loaded(&m, down));
    let run = |model: &SolidModel, pre: &Prestress, w: &Warping| -> Option<String> {
        buckling_factors(model, pre, w, &BucklingOptions::new(3))
            .err()
            .map(|e| key_of(&e))
    };
    let mut check = |name: &str, got: Option<String>, want: Option<&str>| {
        c.holds(name, got.as_deref() == want, &format!("{got:?} (expected {want:?})"));
    };
    let base = Warping::new().block(0, dws);
    check(
        "R unknown set",
        run(&m, &pre, &base.clone().line_load_point("nowhere", down, [0.0, 0.1])),
        Some("UnknownSet"),
    );
    check(
        "R non-finite force",
        run(
            &m,
            &pre,
            &base.clone().line_load_point("all", [0.0, 0.0, f64::NAN], [0.0, 0.1]),
        ),
        Some("load_point"),
    );
    check(
        "R non-finite point",
        run(
            &m,
            &pre,
            &base.clone().line_load_point("all", down, [0.0, f64::INFINITY]),
        ),
        Some("load_point"),
    );
    let lateral = [0.0, Q_REF, 0.0];
    let lat_pre = six_prestress(&line_loaded(&m, lateral));
    check(
        "R lateral load at (0, 0.095)",
        run(
            &m,
            &lat_pre,
            &base.clone().line_load_point("all", lateral, [0.0, DS_FLANGE]),
        ),
        Some("load_point"),
    );
    check(
        "R lateral load at (0, 0) accepted",
        run(&m, &lat_pre, &base.clone().line_load_point("all", lateral, [0.0, 0.0])),
        None,
    );
    check(
        "R vertical load at (0, 0.095) accepted",
        run(&m, &pre, &base.clone().line_load_point("all", down, [0.0, DS_FLANGE])),
        None,
    );
    // An element without a warping unknown: the plain beam of block 1.
    let tb = two_beams(&ds);
    let tb_pre = six_prestress(&line_loaded(&tb, down));
    check(
        "R element without a warping unknown",
        run(
            &tb,
            &tb_pre,
            &base.clone().line_load_point("b1", down, [0.0, DS_FLANGE]),
        ),
        Some("warping"),
    );
    check(
        "R the warping block of the two beams accepted",
        run(
            &tb,
            &tb_pre,
            &base.clone().line_load_point("b0", down, [0.0, DS_FLANGE]),
        ),
        None,
    );
    // Two warping blocks of different shear centres in one set: accepted.
    let ts = two_sections(&ms, &ds);
    let ts_pre = six_prestress(&line_loaded(&ts, down));
    let w2 = Warping::new().block(0, mws).block(1, dws);
    check(
        "R two sections in one set accepted",
        run(&ts, &ts_pre, &w2.line_load_point("all", down, [0.0, 0.05])),
        None,
    );
    c.finish();
}
