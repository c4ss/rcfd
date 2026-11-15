//! F-643 (ADR 0582): load height and non-uniform moment for monosymmetric warping beams. Checks U1, A1 to A9, L0, L1, N1 and R
//! of the registration. Each registered run is made once; every value is printed.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    buckling_factors, buckling_factors_with, matrices, solve_static, Warping, WarpingPrestress, WarpingSection,
};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};

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
const P_REF: f64 = 1000.0;
const M_REF: f64 = 1000.0;
const L: f64 = 3.0;

/// The flange mid-planes of the doubly symmetric I (`h_c/2`) and of flange 1 of the monosymmetric I, from the centroid.
const DS_FLANGE: f64 = 0.095;
const MS_TOP: f64 = 0.0718681318681;

/// The Rayleigh-Ritz references of D4 (`N = 81`), `lambda` for `P_REF`.
const REF_DS_SC: f64 = 106.64131188;
const REF_DS_TOP: f64 = 76.653190682;
const REF_DS_BOT: f64 = 147.52392793;
const REF_MS_SC_DOWN: f64 = 229.64455760;
const REF_MS_SC_UP: f64 = 133.91738647;
const REF_MS_C_DOWN: f64 = 285.61557187;
const REF_MS_TOP_DOWN: f64 = 210.84970872;
const REF_MS_C_UP: f64 = 106.14875777;
/// The monosymmetric I at the shear centre, downward, with `beta_x = 0` (N1).
const REF_MS_BX0: f64 = 177.97;

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

/// A beam of `n` elements along `axis`, node sets `x0`, `xl`, `mid`; fork supports (`u = 0` at node 0, `v = w = phi = 0` at
/// both ends), warping free.
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
    mesh.add_node_set("mid", vec![n / 2]).unwrap();
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

/// The central force `s P_REF e_3` (`s = -1` downward).
fn force(axis: usize, s: f64) -> Vec3 {
    let e = e3(axis);
    [s * P_REF * e[0], s * P_REF * e[1], s * P_REF * e[2]]
}

/// The lowest factor of the fork beam under the central force `s P_REF e_3`, with the load point `point` (none: no entry).
fn central(section: &BeamSection, ws: WarpingSection, n: usize, axis: usize, s: f64, point: Option<[f64; 2]>) -> f64 {
    let m = fork(n, section, axis);
    let mut loaded = m.clone();
    let f = force(axis, s);
    loaded.add_load(Load::nodal_force("mid", f)).unwrap();
    let pre = six_prestress(&loaded);
    let mut w = Warping::new().block(0, ws);
    if let Some(p) = point {
        w = w.load_point("mid", f, p);
    }
    let r = buckling_factors(&m, &pre, &w, &BucklingOptions::new(3)).unwrap();
    println!(
        "  factors {:?} (n {n}, axis {axis}, sign {s:+}, point {point:?}), Sturm {:?}",
        r.factors, r.sturm
    );
    r.factors[0]
}

fn moment_loaded(m: &SolidModel, s: f64) -> SolidModel {
    let mut l = m.clone();
    l.add_load(Load::nodal_moment("x0", [0.0, -s * M_REF, 0.0])).unwrap();
    l.add_load(Load::nodal_moment("xl", [0.0, s * M_REF, 0.0])).unwrap();
    l
}

#[test]
fn u1_uniform_moment_unchanged() {
    let mut c = Checks::new();
    let (s, ws) = mono();
    let m = fork(16, &s, 0);
    let w = Warping::new().block(0, ws);
    let mut o40 = BucklingOptions::new(3);
    o40.subspace_size = Some(40);
    for (sign, opts, want, name) in [
        (-1.0, BucklingOptions::new(3), 178.68421280057305, "-M_ref default"),
        (-1.0, o40.clone(), 178.68421280057328, "-M_ref subspace 40"),
        (1.0, o40, 53.97979218576993, "+M_ref subspace 40"),
    ] {
        let pre = six_prestress(&moment_loaded(&m, sign));
        let f = buckling_factors(&m, &pre, &w, &opts).unwrap().factors[0];
        c.holds(
            &format!("U1 {name}"),
            f.to_bits() == f64::to_bits(want),
            &format!("{f:?} against {want:?}"),
        );
    }
    c.finish();
}

#[test]
fn a_l0_n1_central_point_load() {
    let mut c = Checks::new();
    let (ds, dws) = dsym();
    let (ms, mws) = mono();
    let z_0 = mws.shear_centre()[1];
    println!("z_0 {z_0:.12e}, beta_x {:.12e}", mws.wagner_about_shear_centre()[0]);
    let up = DS_FLANGE;
    // A1 to A5 at 16 elements.
    let ds_sc = central(&ds, dws, 16, 0, -1.0, None);
    let ds_top = central(&ds, dws, 16, 0, -1.0, Some([0.0, up]));
    let ds_bot = central(&ds, dws, 16, 0, -1.0, Some([0.0, -up]));
    let ms_down = central(&ms, mws, 16, 0, -1.0, Some([0.0, z_0]));
    let ms_up = central(&ms, mws, 16, 0, 1.0, Some([0.0, z_0]));
    let a = [
        ("A1 DS at the shear centre", ds_sc, REF_DS_SC),
        ("A2 DS above (top flange)", ds_top, REF_DS_TOP),
        ("A3 DS below (bottom flange)", ds_bot, REF_DS_BOT),
        ("A4 MS at the shear centre, downward", ms_down, REF_MS_SC_DOWN),
        ("A5 MS at the shear centre, upward", ms_up, REF_MS_SC_UP),
    ];
    for (name, f, want) in a {
        println!(
            "{name}: lambda {f:.10e}, reference {want:.10e}, signed {:.4e}",
            f / want - 1.0
        );
        c.at_most(&format!("{name} (16 elements)"), rel(f, want), 2e-2);
    }
    // A6: the offset load-height path.
    for (name, s, p, want) in [
        ("A6 MS at the centroid, downward", -1.0, [0.0, 0.0], REF_MS_C_DOWN),
        ("A6 MS on flange 1, downward", -1.0, [0.0, MS_TOP], REF_MS_TOP_DOWN),
        ("A6 MS at the centroid, upward", 1.0, [0.0, 0.0], REF_MS_C_UP),
    ] {
        let f = central(&ms, mws, 16, 0, s, Some(p));
        println!(
            "{name}: lambda {f:.10e}, reference {want:.10e}, signed {:.4e}",
            f / want - 1.0
        );
        c.at_most(name, rel(f, want), 2e-2);
    }
    // A7: 8 elements.
    for (name, sec, ws, s, p, want) in [
        ("A7 DS at the shear centre", &ds, dws, -1.0, None, REF_DS_SC),
        ("A7 DS above", &ds, dws, -1.0, Some([0.0, up]), REF_DS_TOP),
        ("A7 DS below", &ds, dws, -1.0, Some([0.0, -up]), REF_DS_BOT),
        ("A7 MS downward", &ms, mws, -1.0, Some([0.0, z_0]), REF_MS_SC_DOWN),
        ("A7 MS upward", &ms, mws, 1.0, Some([0.0, z_0]), REF_MS_SC_UP),
    ] {
        let f = central(sec, ws, 8, 0, s, p);
        println!("{name} (8 elements): lambda {f:.10e}, signed {:.4e}", f / want - 1.0);
        c.at_most(&format!("{name} (8 elements)"), rel(f, want), 2e-2);
    }
    // A8: along global y.
    let ds_top_y = central(&ds, dws, 16, 1, -1.0, Some([0.0, up]));
    let ms_down_y = central(&ms, mws, 16, 1, -1.0, Some([0.0, z_0]));
    c.at_most("A8 axis y, DS above", rel(ds_top_y, ds_top), 1e-6);
    c.at_most("A8 axis y, MS downward", rel(ms_down_y, ms_down), 1e-6);
    // A9: the pre-stress of the warping static solve.
    for (name, sec, ws, p, f_ref) in [
        ("DS above", &ds, dws, [0.0, up], ds_top),
        ("MS downward", &ms, mws, [0.0, z_0], ms_down),
    ] {
        let m = fork(16, sec, 0);
        let mut loaded = m.clone();
        let f = force(0, -1.0);
        loaded.add_load(Load::nodal_force("mid", f)).unwrap();
        let w = Warping::new().block(0, ws).load_point("mid", f, p);
        let r = solve_static(&loaded, &w, None).unwrap();
        let pre = WarpingPrestress::from_static(&loaded, &w, &r, [0.0; 2]).unwrap();
        let got = buckling_factors_with(&m, &pre, &w, &BucklingOptions::new(3))
            .unwrap()
            .factors[0];
        println!("A9 {name}: {got:.12e} against {f_ref:.12e}");
        c.at_most(
            &format!("A9 from_static against six-unknown ({name})"),
            rel(got, f_ref),
            1e-9,
        );
    }
    // L0: a load point at the shear centre adds nothing.
    let ds_sc_entry = central(&ds, dws, 16, 0, -1.0, Some([0.0, 0.0]));
    let ms_down_none = central(&ms, mws, 16, 0, -1.0, None);
    c.holds(
        "L0 DS entry at (0, 0) against no entry",
        ds_sc_entry.to_bits() == ds_sc.to_bits(),
        &format!("{ds_sc_entry:?} against {ds_sc:?}"),
    );
    c.holds(
        "L0 MS entry at (0, z_0) against no entry",
        ms_down.to_bits() == ms_down_none.to_bits(),
        &format!("{ms_down:?} against {ms_down_none:?}"),
    );
    // N1: negative controls.
    c.below("N1 DS top/SC", ds_top / ds_sc, 0.8);
    c.above("N1 DS bottom/SC", ds_bot / ds_sc, 1.25);
    c.above("N1 MS down/up", ms_down / ms_up, 1.5);
    c.above("N1 MS against beta_x = 0", rel(ms_down, REF_MS_BX0), 0.15);
    c.finish();
}

#[test]
fn l1_load_height_term_in_the_geometric_stiffness() {
    let mut c = Checks::new();
    let (ds, dws) = dsym();
    let m = fork(16, &ds, 0);
    let mut loaded = m.clone();
    let f = force(0, -1.0);
    loaded.add_load(Load::nodal_force("mid", f)).unwrap();
    let pre = six_prestress(&loaded);
    let w0 = Warping::new().block(0, dws);
    let w1 = w0.clone().load_point("mid", f, [0.0, DS_FLANGE]);
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
    let rx = m.dof_map().equation(8, 3).unwrap();
    let mut others_equal = true;
    let mut diff = f64::NAN;
    for r in 0..ga.n {
        for p in ga.row_ptr[r]..ga.row_ptr[r + 1] {
            if r == rx && ga.col[p] == rx {
                diff = gb.val[p] - ga.val[p];
            } else if ga.val[p].to_bits() != gb.val[p].to_bits() && !(ga.val[p] == 0.0 && gb.val[p] == 0.0) {
                others_equal = false;
            }
        }
    }
    c.holds(
        "L1 geometric bit-equal off the twist of node 8",
        others_equal,
        "every other entry",
    );
    println!(
        "L1 difference at the twist of node 8: {diff:.15e} (hand {:.15e})",
        -P_REF * DS_FLANGE
    );
    c.at_most("L1 difference against -P a", rel(diff, -P_REF * DS_FLANGE), 1e-12);
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

/// Two disjoint fork beams along `x` (block 0 at `y = 0`, block 1 at `y = 1`, 16 elements each), sets `mid1` (node 8 of block
/// 1) and `mid` (node 8 of block 0).
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
    mesh.add_node_set("x0", vec![0, o]).unwrap();
    mesh.add_node_set("xl", vec![n, o + n]).unwrap();
    mesh.add_node_set("mid", vec![n / 2]).unwrap();
    mesh.add_node_set("mid1", vec![o + n / 2]).unwrap();
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

/// One beam of two warping blocks: elements 0 to 7 with `(s1, w1)`, 8 to 15 with `(s2, w2)`; set `mid` (node 8).
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
    mesh.add_node_set("mid", vec![8]).unwrap();
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
fn r_refusals() {
    let mut c = Checks::new();
    let (ds, dws) = dsym();
    let (ms, mws) = mono();
    let m = fork(16, &ds, 0);
    let down = force(0, -1.0);
    let mut loaded = m.clone();
    loaded.add_load(Load::nodal_force("mid", down)).unwrap();
    let pre = six_prestress(&loaded);
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
        run(&m, &pre, &base.clone().load_point("nowhere", down, [0.0, 0.1])),
        Some("UnknownSet"),
    );
    check(
        "R non-finite force",
        run(
            &m,
            &pre,
            &base.clone().load_point("mid", [0.0, 0.0, f64::NAN], [0.0, 0.1]),
        ),
        Some("load_point"),
    );
    check(
        "R non-finite point",
        run(&m, &pre, &base.clone().load_point("mid", down, [0.0, f64::INFINITY])),
        Some("load_point"),
    );
    // A lateral force at the top flange has a first-order torque about the centroid.
    let lateral = [0.0, P_REF, 0.0];
    let mut lat_loaded = m.clone();
    lat_loaded.add_load(Load::nodal_force("mid", lateral)).unwrap();
    let lat_pre = six_prestress(&lat_loaded);
    check(
        "R lateral force at (0, 0.095)",
        run(&m, &lat_pre, &base.clone().load_point("mid", lateral, [0.0, DS_FLANGE])),
        Some("load_point"),
    );
    check(
        "R lateral force at (0, 0) accepted",
        run(&m, &lat_pre, &base.clone().load_point("mid", lateral, [0.0, 0.0])),
        None,
    );
    check(
        "R vertical force at (0, 0.095) accepted",
        run(&m, &pre, &base.clone().load_point("mid", down, [0.0, DS_FLANGE])),
        None,
    );
    // A node without a warping unknown: the mid node of the plain beam.
    let tb = two_beams(&ds);
    let mut tb_loaded = tb.clone();
    tb_loaded.add_load(Load::nodal_force("mid", down)).unwrap();
    tb_loaded.add_load(Load::nodal_force("mid1", down)).unwrap();
    let tb_pre = six_prestress(&tb_loaded);
    check(
        "R node without a warping unknown",
        run(&tb, &tb_pre, &base.clone().load_point("mid1", down, [0.0, DS_FLANGE])),
        Some("warping"),
    );
    // Two warping blocks of different shear centres at the load node.
    let ts = two_sections(&ms, &ds);
    let mut ts_loaded = ts.clone();
    ts_loaded.add_load(Load::nodal_force("mid", down)).unwrap();
    let ts_pre = six_prestress(&ts_loaded);
    let w2 = Warping::new().block(0, mws).block(1, dws);
    check(
        "R different shear centres at the node",
        run(&ts, &ts_pre, &w2.load_point("mid", down, [0.0, 0.0])),
        Some("load_point"),
    );
    c.finish();
}
