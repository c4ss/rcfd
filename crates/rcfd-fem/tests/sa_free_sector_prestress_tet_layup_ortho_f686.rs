//! F-686 (ADR 0633): the free-sector pre-stress of ADR 0606 on a curved Tet10 sector (T10), a Shell4 layup frustum sector (L2)
//! and an orthotropic Hex20 sector with cylindrical axes (O20). Checks FS, RX, RXc, OR, GN, WS, the controls NC0, RL3, DF and
//! the diagnostic OL of the registration (B2, B3 are the existing tests and the diff). The models follow ADR 0611 (helpers
//! copied from `sa_free_sector_prestress_solids_shells_f683.rs`, not edited) in its oblique frame.

use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::material::MaterialAxes;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::rotating::{free_sector_unit_static, RotatingOptions, RotatingSector, WhirlModes};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    BlockProperty, Constraint, ElementKind, FemError, LinearTerm, Load, Material, Ply, SolidMesh, SolidModel,
    SolidResult,
};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
const N_SOLID: usize = 12;
const N_SHELL: usize = 6;
const NC: usize = 2;
const NA: usize = 3;
const K: usize = 2;
/// The ply thickness of L2 (two plies, total 0.05 as the frustum of ADR 0561).
const T_PLY: f64 = 0.025;

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

fn axis() -> Vec3 {
    [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]
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

fn ea() -> Vec3 {
    let l = 5.0f64.sqrt();
    [2.0 / l, -1.0 / l, 0.0]
}

fn eb() -> Vec3 {
    cross(axis(), ea())
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

/// The orthotropic material of ADR 0465 with cylindrical axes about the cyclic axis (D2, O20).
fn ortho_cyl() -> Material {
    Material::orthotropic(
        [1.4e11, 1.0e10, 1.0e10],
        [0.3, 0.3, 0.45],
        [5.0e9, 5.0e9, 3.5e9],
        1600.0,
        [0.0; 3],
        MaterialAxes::cylindrical(ORIGIN, axis()).unwrap(),
    )
    .unwrap()
}

/// The ply material of ADR 0496 with density `rho`.
fn ply_material(rho: f64) -> Material {
    let axes = MaterialAxes::fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
    Material::orthotropic(
        [10.0e9, 4.0e9, 4.0e9],
        [0.35, 0.35, 0.45],
        [1.6e9, 1.6e9, 1.4e9],
        rho,
        [0.0; 3],
        axes,
    )
    .unwrap()
}

/// The L2 layup (D2): bottom 0 degrees `rho = 1400`, top 90 degrees `rho = 2800`, each `T_PLY`.
fn plies() -> Vec<Ply> {
    vec![
        Ply::new(ply_material(1400.0), T_PLY, 0.0).unwrap(),
        Ply::new(ply_material(2800.0), T_PLY, 0.5 * PI).unwrap(),
    ]
}

/// `m1 = sum rho_k t_k zbar_k` of the plies, bottom first, mid-plane at the half-thickness.
fn m1_of(p: &[Ply]) -> f64 {
    let t: f64 = p.iter().map(|q| q.thickness).sum();
    let mut z = -0.5 * t;
    let mut m1 = 0.0;
    for q in p {
        m1 += q.material.density() * q.thickness * (z + 0.5 * q.thickness);
        z += q.thickness;
    }
    m1
}

/// `(r, phi, z) -> O + r (cos phi e_a + sin phi e_b) + z a`.
fn frame_map(p: Vec3) -> Vec3 {
    let (s, c) = p[1].sin_cos();
    let (a, b, n) = (ea(), eb(), axis());
    [0, 1, 2].map(|k| ORIGIN[k] + p[0] * (c * a[k] + s * b[k]) + p[2] * n[k])
}

struct Case {
    name: &'static str,
    model: SolidModel,
    low: &'static str,
    high: &'static str,
    r_max: f64,
}

fn solid_model(mesh: SolidMesh, material: Material) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material).unwrap();
    m.add_constraint(Constraint::cyclic("ymin", "ymax", axis(), ORIGIN, N_SOLID))
        .unwrap();
    m
}

/// T10 (D2): the Tet10 box sector, every node mapped (curved Tet10), its sets kept.
fn t10() -> Case {
    let al = 2.0 * PI / N_SOLID as f64;
    let b = upgrade_to_tet10(&tet_box([0.25, 0.0, -0.1], [0.5, al, 0.3], [2, 2, 1]).unwrap())
        .unwrap()
        .0;
    let nodes: Vec<Vec3> = b.nodes().iter().map(|&p| frame_map(p)).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Tet10, b.blocks()[0].connectivity().to_vec())
        .unwrap();
    for name in b.node_set_names() {
        mesh.add_node_set(name, b.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in b.element_set_names() {
        mesh.add_element_set(name, b.element_set(name).unwrap().to_vec())
            .unwrap();
    }
    for name in b.surface_set_names() {
        mesh.add_surface_set(name, b.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    Case {
        name: "T10",
        model: solid_model(mesh, steel()),
        low: "ymin",
        high: "ymax",
        r_max: 0.75,
    }
}

/// O20 (D2): the Hex20 sector S20 of ADR 0611 with the cylindrical orthotropic material.
fn o20() -> Case {
    let al = 2.0 * PI / N_SOLID as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.5, al, 0.3], [3, 2, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&b, frame_map).unwrap().0;
    Case {
        name: "O20",
        model: solid_model(mesh, ortho_cyl()),
        low: "ymin",
        high: "ymax",
        r_max: 0.75,
    }
}

/// The point of the ADR 0556 D3 frustum at column `i` (of `N NC` around) and generator parameter `s`.
fn point(i: usize, s: f64) -> Vec3 {
    let n = axis();
    let (a, b) = (ea(), eb());
    let phi = 2.0 * PI * i as f64 / (N_SHELL * NC) as f64;
    let r = 1.0 - 0.4 * s;
    let z = 0.8 * s;
    let (sp, cp) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + r * (cp * a[k] + sp * b[k]) + z * n[k])
}

fn push_element(conn: &mut Vec<usize>, kind: ElementKind, q: [usize; 4]) {
    match kind {
        ElementKind::Shell4 => conn.extend(q),
        _ => conn.extend([q[0], q[1], q[2], q[0], q[2], q[3]]),
    }
}

/// The frustum sector of ADR 0611 (free) of `kind` with the L2 layup.
fn layup_shell(kind: ElementKind) -> SolidModel {
    let columns = NC + 1;
    let id = |i: usize, a: usize| a * columns + i;
    let mut nodes = Vec::new();
    for a in 0..=NA {
        for i in 0..columns {
            nodes.push(point(i, a as f64 / NA as f64));
        }
    }
    let mut conn = Vec::new();
    for a in 0..NA {
        for i in 0..NC {
            push_element(
                &mut conn,
                kind,
                [id(i, a), id(i + 1, a), id(i + 1, a + 1), id(i, a + 1)],
            );
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::layup(plies()).unwrap())
        .unwrap();
    let ne = mesh.blocks()[0].len();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("low", (0..=NA).map(|a| id(0, a)).collect()).unwrap();
    mesh.add_node_set("high", (0..=NA).map(|a| id(NC, a)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N_SHELL,
        CyclicOptions::new().shells(true),
    ))
    .unwrap();
    m
}

/// L2 (D2).
fn l2() -> Case {
    Case {
        name: "L2",
        model: layup_shell(ElementKind::Shell4),
        low: "low",
        high: "high",
        r_max: 1.0,
    }
}

fn cases() -> Vec<Case> {
    vec![t10(), l2(), o20()]
}

fn interior(case: &Case) -> Vec<usize> {
    let mesh = case.model.mesh();
    let mut on = vec![false; mesh.nodes().len()];
    for name in [case.low, case.high] {
        for &n in mesh.node_set(name).unwrap() {
            on[n] = true;
        }
    }
    (0..on.len()).filter(|&n| !on[n]).collect()
}

fn radial(x: Vec3) -> Vec3 {
    let d = sub(x, ORIGIN);
    let s = dot(d, axis());
    [0, 1, 2].map(|k| d[k] - s * axis()[k])
}

/// D2: the constrained-copy route, `a . u_A = 0` at the first interior node and `t_B . u_B = 0` at the last.
fn copy_route(case: &Case) -> (SolidResult, Prestress, usize, usize) {
    let inner = interior(case);
    let (na, nb) = (inner[0], *inner.last().unwrap());
    let mut copy = case.model.clone();
    let d = radial(copy.mesh().nodes()[nb]);
    let rho = dot(d, d).sqrt();
    let t = cross(axis(), d).map(|v| v / rho);
    copy.add_constraint(Constraint::linear(
        (0..3).map(|c| LinearTerm::new(na, c, axis()[c])).collect(),
        0.0,
    ))
    .unwrap();
    copy.add_constraint(Constraint::linear(
        (0..3).map(|c| LinearTerm::new(nb, c, t[c])).collect(),
        0.0,
    ))
    .unwrap();
    copy.add_load(Load::centrifugal(axis(), ORIGIN, 1.0)).unwrap();
    let r = solve_static(&copy, &StaticOptions::default()).unwrap();
    let ps = Prestress::from_static(&copy, &r).unwrap();
    (r, ps, na, nb)
}

fn prestress_difference(m: &SolidModel, a: &Prestress, b: &Prestress) -> f64 {
    let (mut d, mut s): (f64, f64) = (0.0, 0.0);
    for e in 0..m.mesh().n_elements() {
        let (pa, pb) = (a.of_element(e), b.of_element(e));
        assert_eq!(pa.len(), pb.len());
        for (x, y) in pa.iter().zip(pb) {
            for c in 0..6 {
                d = d.max((x[c] - y[c]).abs());
                s = s.max(y[c].abs());
            }
        }
    }
    d / s
}

fn load_vector(m: &SolidModel) -> Vec<f64> {
    let mut loaded = m.clone();
    loaded.add_load(Load::centrifugal(axis(), ORIGIN, 1.0)).unwrap();
    nodal_load_vector(&loaded, &[]).unwrap()
}

/// D2: the stiffness of the residual; O20 by `operator(0)` of the sector without stress stiffening.
fn residual_stiffness(case: &Case) -> CsrMatrix {
    if case.name == "O20" {
        let rs = RotatingSector::new(&case.model, &RotatingOptions::new(3).stress_stiffening(false)).unwrap();
        rs.part().operator(0.0)
    } else {
        stiffness(&case.model).unwrap()
    }
}

/// `max |K u - f|` over every equation of the interior nodes, over `max |f|`.
fn interior_residual(case: &Case, k: &CsrMatrix, u: &[f64], f: &[f64]) -> f64 {
    let m = &case.model;
    let mut ku = vec![0.0; u.len()];
    k.matvec(u, &mut ku);
    let fmax = f.iter().fold(0.0f64, |s, x| s.max(x.abs()));
    let dofs = m.dof_map();
    let mut rx: f64 = 0.0;
    for node in interior(case) {
        for c in 0..dofs.components(node) {
            let eq = dofs.equation(node, c).unwrap();
            rx = rx.max((ku[eq] - f[eq]).abs());
        }
    }
    rx / fmax
}

/// D1: `r_1 = (a, 0)`, `r_2 = (a x (x - O), a)` on the model's equations.
fn cyclic_rigid_motions(m: &SolidModel) -> [Vec<f64>; 2] {
    let dofs = m.dof_map();
    let mut r1 = vec![0.0; dofs.n_equations()];
    let mut r2 = vec![0.0; dofs.n_equations()];
    for node in 0..m.mesh().nodes().len() {
        let x = m.mesh().nodes()[node];
        let w = cross(axis(), sub(x, ORIGIN));
        for c in 0..dofs.components(node) {
            let eq = dofs.equation(node, c).unwrap();
            if c < 3 {
                r1[eq] = axis()[c];
                r2[eq] = w[c];
            } else {
                r2[eq] = axis()[c - 3];
            }
        }
    }
    [r1, r2]
}

fn dot_v(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn whirl(rs: &RotatingSector, k: usize, omega: f64) -> WhirlModes {
    rs.whirl_modes(k, omega)
        .unwrap()
        .modes()
        .expect("arithmetic 5: no loss of stiffness at omega_1 / 2")
        .clone()
}

fn half_speed(rs: &RotatingSector) -> f64 {
    let h = rs.modes(K, 0.0).unwrap();
    0.5 * h.modes().expect("rest modes").values[0].sqrt()
}

fn base() -> RotatingOptions {
    RotatingOptions::new(3).coriolis(true)
}

#[test]
fn fs_rx_rxc_or_gn_ws_the_free_tet10_layup_and_orthotropic_sectors_against_the_constrained_copy() {
    let mut ok = true;
    for case in cases() {
        let m = &case.model;
        let name = case.name;
        let opt = base().free_sector_prestress(true);
        let new = RotatingSector::new(m, &opt).unwrap();
        let (r_copy, ps_copy, na, nb) = copy_route(&case);
        // FS.
        let fs = prestress_difference(m, new.part().unit_prestress(), &ps_copy);
        println!(
            "FS {name}: opt-in pre-stress against the copy route (A {na}, B {nb}): {fs:.2e} (limit 1e-10) {}",
            met(fs <= 1e-10)
        );
        ok &= fs <= 1e-10;
        // GN.
        let lib = free_sector_unit_static(m, &opt).unwrap();
        let g = lib.gauge_node;
        let d = radial(m.mesh().nodes()[g]);
        let rho = dot(d, d).sqrt();
        let inside = interior(&case).contains(&g);
        let gn = inside && (rho / case.r_max - 1.0).abs() <= 1e-12;
        println!(
            "GN {name}: gauge node {g} interior {inside}, rho_g / r_max - 1 = {:.2e} (limit 1e-12) {}",
            rho / case.r_max - 1.0,
            met(gn)
        );
        ok &= gn;
        // RX, RXc.
        let f = load_vector(m);
        let k = residual_stiffness(&case);
        let rx = interior_residual(&case, &k, lib.result.displacement_values(), &f);
        println!(
            "RX {name}: |K u - f| at the interior nodes over max |f| = {rx:.2e} (limit 1e-10) {}",
            met(rx <= 1e-10)
        );
        ok &= rx <= 1e-10;
        let rxc = interior_residual(&case, &k, r_copy.displacement_values(), &f);
        println!(
            "RXc {name}: the copy route's residual = {rxc:.2e} (limit 1e-10) {}",
            met(rxc <= 1e-10)
        );
        ok &= rxc <= 1e-10;
        // OR.
        let fn2 = dot_v(&f, &f).sqrt();
        for (i, r) in cyclic_rigid_motions(m).iter().enumerate() {
            let or = dot_v(r, &f).abs() / (dot_v(r, r).sqrt() * fn2);
            println!(
                "OR {name} r_{}: |r^T f| / (|r| |f|) = {or:.2e} (limit 1e-13) {}",
                i + 1,
                met(or <= 1e-13)
            );
            ok &= or <= 1e-13;
        }
        // WS.
        let copy = RotatingSector::with_unit_prestress(m, ps_copy.clone(), &base()).unwrap();
        let omega = half_speed(&copy);
        let (a, b) = (whirl(&new, K, omega), whirl(&copy, K, omega));
        let same =
            a.modes.len() == b.modes.len() && a.modes.iter().zip(&b.modes).all(|(x, y)| x.direction == y.direction);
        let mut dmax: f64 = 0.0;
        for (x, y) in a.modes.iter().zip(&b.modes) {
            dmax = dmax.max((x.omega / y.omega - 1.0).abs());
        }
        let ws = dmax <= 1e-8 && same;
        println!(
            "WS {name} k {K} Omega {omega:.6e}: {} modes, max |omega_new / omega_copy - 1| = {dmax:.2e} (limit 1e-8); counts and directions equal {same} {}",
            a.modes.len(),
            met(ws)
        );
        ok &= ws;
    }
    assert!(ok, "a registered check missed (see the output)");
}

#[test]
fn nc0_rl3_df_refusals_and_the_default() {
    let mut ok = true;
    let df = !RotatingOptions::new(3).free_sector_prestress;
    println!(
        "DF RotatingOptions::new(3).free_sector_prestress == false: {df} {}",
        met(df)
    );
    ok &= df;
    for case in cases() {
        let r = RotatingSector::new(&case.model, &base());
        let refused = matches!(r, Err(FemError::SingularStiffness { .. }));
        println!(
            "NC0 {}: SingularStiffness {refused} {}{}",
            case.name,
            met(refused),
            if refused {
                String::new()
            } else {
                format!(" ({:?})", r.err())
            }
        );
        ok &= refused;
    }
    let s3 = layup_shell(ElementKind::Shell3);
    for (label, opt) in [("without", base()), ("with", base().free_sector_prestress(true))] {
        let r = RotatingSector::new(&s3, &opt);
        let refused = matches!(r, Err(FemError::NotYetSupported { variant: "Layup", .. }));
        println!(
            "RL3 Shell3 layup frustum, {label} the opt-in: refused NotYetSupported Layup {refused} {} ({:?})",
            met(refused),
            r.err()
        );
        ok &= refused;
    }
    assert!(ok, "a registered check missed (see the output)");
}

/// OL (ADR 0633 D1 finding, arithmetic 6): the torque of the layup centrifugal load on a facet in a plane through the axis.
#[test]
fn ol_the_layup_centrifugal_load_on_a_meridian_facet_has_the_predicted_torque() {
    let (a, e) = (axis(), ea());
    let corner = |r: f64, z: f64| -> Vec3 { [0, 1, 2].map(|k| ORIGIN[k] + r * e[k] + z * a[k]) };
    let c = vec![corner(0.5, 0.0), corner(0.7, 0.0), corner(0.7, 0.2), corner(0.5, 0.25)];
    let mut mesh = SolidMesh::new(c.clone()).unwrap();
    mesh.add_block(ElementKind::Shell4, vec![0, 1, 2, 3]).unwrap();
    mesh.set_block_property(0, BlockProperty::layup(plies()).unwrap())
        .unwrap();
    mesh.add_element_set("all", vec![0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    let f = load_vector(&m);
    let r2 = &cyclic_rigid_motions(&m)[1];
    let work = dot_v(r2, &f);
    // The closed form: P = m1 A e_3 . (r_perp(x_c) x a), e_3 = (x_2 - x_0) x (x_3 - x_1) normalised, x_c the area centroid.
    let n = cross(sub(c[2], c[0]), sub(c[3], c[1]));
    let nn = dot(n, n).sqrt();
    let e3 = n.map(|v| v / nn);
    let (mut area, mut xc) = (0.0, [0.0; 3]);
    for (p, q, s) in [(0, 1, 2), (0, 2, 3)] {
        let at = 0.5 * dot(cross(sub(c[q], c[p]), sub(c[s], c[p])), e3);
        area += at;
        for k in 0..3 {
            xc[k] += at * (c[p][k] + c[q][k] + c[s][k]) / 3.0;
        }
    }
    let xc = xc.map(|v| v / area);
    let rc = radial(xc);
    let rho_c = dot(rc, rc).sqrt();
    let m1 = m1_of(&plies());
    let p = m1 * area * dot(e3, cross(rc, a));
    let scale = m1 * area * rho_c;
    let d1 = (work - p).abs() / scale;
    let d2 = (work.abs() / scale - 1.0).abs();
    println!(
        "OL: m1 {m1:.6e}, A {area:.6e}, rho_c {rho_c:.6e}; r_2^T f = {work:.6e}, P = {p:.6e}; |r_2^T f - P| / (m1 A rho_c) = {d1:.2e} (limit 1e-10) {}; ||r_2^T f| / (m1 A rho_c) - 1| = {d2:.2e} (limit 1e-12) {}",
        met(d1 <= 1e-10),
        met(d2 <= 1e-12)
    );
    assert!(d1 <= 1e-10 && d2 <= 1e-12, "OL missed (see the output)");
}
