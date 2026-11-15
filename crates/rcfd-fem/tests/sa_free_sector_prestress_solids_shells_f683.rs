//! F-683 (ADR 0611): the free-sector pre-stress of ADR 0606 on solid (Hex20, Hex8) and shell (Shell4, Shell3) sectors, and
//! its scope without stress stiffening. Checks FS, RX, RXc, OR, GN, WS and the controls NC0s, NS1, NS2, NS3, DF of the
//! registration (B2, B3 are the existing tests and the diff). The solid sector follows `si_rotating_cyclic.rs` (ADR 0407) in
//! the oblique frame of ADR 0602; the shell sector is the frustum of `sa_rotating_cyclic_shells.rs` (ADR 0561 D3) without
//! its clamp (helpers copied, not edited).

use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::rotating::{free_sector_unit_static, RotatingOptions, RotatingSector, WhirlModes};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    BlockProperty, Constraint, ElasticSupport, ElementKind, FemError, LinearTerm, Load, Material, SolidMesh,
    SolidModel, SolidResult,
};
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
/// The solid sectors (ADR 0611 D2).
const N_SOLID: usize = 12;
/// The shell frustum of ADR 0561 D3.
const N_SHELL: usize = 6;
const NC: usize = 2;
const NA: usize = 3;
const T: f64 = 0.05;
const K: usize = 2;

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

fn ea() -> Vec3 {
    let l = 5.0f64.sqrt();
    [2.0 / l, -1.0 / l, 0.0]
}

fn eb() -> Vec3 {
    cross(axis(), ea())
}

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

/// `(r, phi, z) -> O + r (cos phi e_a + sin phi e_b) + z a`.
fn frame_map(p: Vec3) -> Vec3 {
    let (s, c) = p[1].sin_cos();
    let (a, b, n) = (ea(), eb(), axis());
    [0, 1, 2].map(|k| ORIGIN[k] + p[0] * (c * a[k] + s * b[k]) + p[2] * n[k])
}

/// A model of a sector: the mesh, the names of its cyclic node sets and its largest radius.
struct Case {
    name: &'static str,
    model: SolidModel,
    low: &'static str,
    high: &'static str,
    r_max: f64,
}

fn solid_model(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    m.add_constraint(Constraint::cyclic("ymin", "ymax", axis(), ORIGIN, N_SOLID))
        .unwrap();
    m
}

/// S20 (D2): the Hex20 disc sector, free.
fn s20() -> Case {
    let al = 2.0 * PI / N_SOLID as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.5, al, 0.3], [3, 2, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&b, frame_map).unwrap().0;
    Case {
        name: "S20",
        model: solid_model(mesh),
        low: "ymin",
        high: "ymax",
        r_max: 0.75,
    }
}

/// S8 (D2): the Hex8 disc sector, free.
fn s8() -> Case {
    let al = 2.0 * PI / N_SOLID as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.5, al, 0.3], [2, 3, 2]).unwrap();
    let nodes: Vec<Vec3> = b.nodes().iter().map(|&p| frame_map(p)).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, b.blocks()[0].connectivity().to_vec())
        .unwrap();
    for name in ["ymin", "ymax", "xmin"] {
        mesh.add_node_set(name, b.node_set(name).unwrap().to_vec()).unwrap();
    }
    mesh.add_element_set("all", (0..b.n_elements()).collect()).unwrap();
    Case {
        name: "S8",
        model: solid_model(mesh),
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

/// SH4, SH3 (D2): the frustum sector of ADR 0561 D3 without its clamp (a node set `clamp` on row 0 for the twin).
fn shell(kind: ElementKind) -> Case {
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
    mesh.set_block_property(0, BlockProperty::shell(T).unwrap()).unwrap();
    let ne = mesh.blocks()[0].len();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("clamp", (0..columns).map(|i| id(i, 0)).collect())
        .unwrap();
    mesh.add_node_set("low", (0..=NA).map(|a| id(0, a)).collect()).unwrap();
    mesh.add_node_set("high", (0..=NA).map(|a| id(NC, a)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N_SHELL,
        CyclicOptions::new().shells(true),
    ))
    .unwrap();
    Case {
        name: if kind == ElementKind::Shell4 { "SH4" } else { "SH3" },
        model: m,
        low: "low",
        high: "high",
        r_max: 1.0,
    }
}

fn free_cases() -> Vec<Case> {
    vec![s20(), s8(), shell(ElementKind::Shell4), shell(ElementKind::Shell3)]
}

/// ADR 0611 amendment 1: S8 with the surface sets of its `hex_box` mesh (the cyclic constraint of a solid reads them).
fn s8_a1() -> Case {
    let al = 2.0 * PI / N_SOLID as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.5, al, 0.3], [2, 3, 2]).unwrap();
    let nodes: Vec<Vec3> = b.nodes().iter().map(|&p| frame_map(p)).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Hex8, b.blocks()[0].connectivity().to_vec())
        .unwrap();
    for name in ["ymin", "ymax", "xmin"] {
        mesh.add_node_set(name, b.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in b.surface_set_names() {
        mesh.add_surface_set(name, b.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    mesh.add_element_set("all", (0..b.n_elements()).collect()).unwrap();
    Case {
        name: "S8",
        model: solid_model(mesh),
        low: "ymin",
        high: "ymax",
        r_max: 0.75,
    }
}

fn free_cases_a1() -> Vec<Case> {
    vec![s20(), s8_a1(), shell(ElementKind::Shell4), shell(ElementKind::Shell3)]
}

/// The clamped twins of NS3: S20 with `xmin` held in 0..3, SH4 with `clamp` held in 0..6.
fn clamped_cases() -> Vec<Case> {
    let mut a = s20();
    for c in 0..3 {
        a.model
            .add_constraint(Constraint::displacement("xmin", c, 0.0))
            .unwrap();
    }
    a.name = "S20 clamped";
    let mut b = shell(ElementKind::Shell4);
    for c in 0..6 {
        b.model
            .add_constraint(Constraint::displacement("clamp", c, 0.0))
            .unwrap();
    }
    b.name = "SH4 clamped";
    vec![a, b]
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
    let d = [0, 1, 2].map(|k| x[k] - ORIGIN[k]);
    let s = dot(d, axis());
    [0, 1, 2].map(|k| d[k] - s * axis()[k])
}

/// D2: the constrained-copy route, `a . u_A = 0` at the first interior node and `t_B . u_B = 0` at the last.
fn copy_route(case: &Case) -> (SolidModel, SolidResult, Prestress, usize, usize) {
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
    (copy, r, ps, na, nb)
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

fn prestress_bits_equal(m: &SolidModel, a: &Prestress, b: &Prestress) -> bool {
    (0..m.mesh().n_elements()).all(|e| {
        let (pa, pb) = (a.of_element(e), b.of_element(e));
        pa.len() == pb.len()
            && pa
                .iter()
                .zip(pb)
                .all(|(x, y)| (0..6).all(|c| x[c].to_bits() == y[c].to_bits()))
    })
}

fn load_vector(case: &Case) -> Vec<f64> {
    let mut loaded = case.model.clone();
    loaded.add_load(Load::centrifugal(axis(), ORIGIN, 1.0)).unwrap();
    nodal_load_vector(&loaded, &[]).unwrap()
}

/// `max |K u - f|` over every equation of the interior nodes, over `max |f|`.
fn interior_residual(case: &Case, u: &[f64], f: &[f64]) -> f64 {
    let m = &case.model;
    let k = stiffness(m).unwrap();
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
        let dx = [0, 1, 2].map(|k| x[k] - ORIGIN[k]);
        let w = cross(axis(), dx);
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

/// Arithmetic 5: `omega_1 / 2` of index `K` at rest.
fn half_speed(rs: &RotatingSector) -> f64 {
    let h = rs.modes(K, 0.0).unwrap();
    0.5 * h.modes().expect("rest modes").values[0].sqrt()
}

fn base() -> RotatingOptions {
    RotatingOptions::new(3).coriolis(true)
}

/// Run 1 (ADR 0611): stopped at the set-up (S8 without its surface sets, amendment 1); kept as the record.
#[test]
#[ignore = "ADR 0611 amendment 1: defective set-up (S8 without its surface sets); the record of run 1"]
fn fs_rx_rxc_or_gn_ws_the_free_solid_and_shell_sectors_against_the_constrained_copy() {
    fs_run(free_cases());
}

/// ADR 0611 amendment 1: FS to WS on the corrected S8.
#[test]
fn fs_rx_rxc_or_gn_ws_the_free_solid_and_shell_sectors_against_the_constrained_copy_amended() {
    fs_run(free_cases_a1());
}

fn fs_run(cases: Vec<Case>) {
    let mut ok = true;
    for case in cases {
        let m = &case.model;
        let name = case.name;
        let opt = base().free_sector_prestress(true);
        let new = RotatingSector::new(m, &opt).unwrap();
        let (_, r_copy, ps_copy, na, nb) = copy_route(&case);
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
        let gn = interior(&case).contains(&g) && (rho / case.r_max - 1.0).abs() <= 1e-12;
        println!(
            "GN {name}: gauge node {g} interior {}, rho_g / r_max - 1 = {:.2e} (limit 1e-12) {}",
            interior(&case).contains(&g),
            rho / case.r_max - 1.0,
            met(gn)
        );
        ok &= gn;
        // RX, RXc.
        let f = load_vector(&case);
        let rx = interior_residual(&case, lib.result.displacement_values(), &f);
        println!(
            "RX {name}: |K u - f| at the interior nodes over max |f| = {rx:.2e} (limit 1e-10) {}",
            met(rx <= 1e-10)
        );
        ok &= rx <= 1e-10;
        let rxc = interior_residual(&case, r_copy.displacement_values(), &f);
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
        // WS (S20, SH4).
        if name == "S20" || name == "SH4" {
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
    }
    assert!(ok, "a registered check missed (see the output)");
}

#[test]
fn nc0s_without_the_opt_in_the_free_solid_and_shell_sectors_are_refused() {
    let mut ok = true;
    for case in [s20(), shell(ElementKind::Shell4)] {
        let r = RotatingSector::new(&case.model, &base());
        let refused = matches!(r, Err(FemError::SingularStiffness { .. }));
        println!("NC0s {}: SingularStiffness {refused} {}", case.name, met(refused));
        ok &= refused;
    }
    assert!(ok, "NC0s missed");
}

/// Run 1 (ADR 0611): stopped at the set-up (S8 without its surface sets, amendment 1) after DF; kept as the record.
#[test]
#[ignore = "ADR 0611 amendment 1: defective set-up (S8 without its surface sets); the record of run 1"]
fn ns1_ns2_ns3_df_the_opt_in_without_stress_stiffening_changes_no_bit() {
    ns_run(free_cases);
}

/// ADR 0611 amendment 1: NS1 to NS3 and DF on the corrected S8.
#[test]
fn ns1_ns2_ns3_df_the_opt_in_without_stress_stiffening_changes_no_bit_amended() {
    ns_run(free_cases_a1);
}

fn ns_run(free: fn() -> Vec<Case>) {
    let mut ok = true;
    let df = !RotatingOptions::new(3).free_sector_prestress;
    println!(
        "DF RotatingOptions::new(3).free_sector_prestress == false: {df} {}",
        met(df)
    );
    ok &= df;
    let off = base().stress_stiffening(false);
    let off_in = off.clone().free_sector_prestress(true);
    let mut cases = free();
    cases.extend(clamped_cases());
    for case in &cases {
        let m = &case.model;
        let name = case.name;
        let (b, a) = (RotatingSector::new(m, &off), RotatingSector::new(m, &off_in));
        let (b, a) = match (b, a) {
            (Ok(b), Ok(a)) => (b, a),
            (b, a) => {
                println!(
                    "NS3 (a) {name}: accepted both ways: without {:?}, with {:?} MISSED",
                    b.err(),
                    a.err()
                );
                ok = false;
                continue;
            }
        };
        // NS1.
        let ns1 = prestress_bits_equal(m, a.part().unit_prestress(), b.part().unit_prestress());
        println!("NS1 {name}: unit_prestress bit-identical {ns1} {}", met(ns1));
        ok &= ns1;
        // NS2.
        let omega = half_speed(&b);
        let (wa, wb) = (whirl(&a, K, omega), whirl(&b, K, omega));
        let ns2 = wa.modes.len() == wb.modes.len()
            && wa.frequencies_all.len() == wb.frequencies_all.len()
            && wa
                .modes
                .iter()
                .zip(&wb.modes)
                .all(|(x, y)| x.omega.to_bits() == y.omega.to_bits() && x.direction == y.direction)
            && wa
                .frequencies_all
                .iter()
                .zip(&wb.frequencies_all)
                .all(|(x, y)| x.to_bits() == y.to_bits());
        println!(
            "NS2 {name} k {K} Omega {omega:.6e}: {} modes, whirl frequencies bit-identical {ns2} {}",
            wa.modes.len(),
            met(ns2)
        );
        ok &= ns2;
        if name == "S20" {
            // NS3 (c).
            let bad = N_SOLID / 2 + 1;
            let (eb_, ea_) = (b.whirl_modes(bad, omega), a.whirl_modes(bad, omega));
            let c = eb_.is_err() && ea_.is_err() && format!("{:?}", eb_.err()) == format!("{:?}", ea_.err());
            println!(
                "NS3 (c) {name}: whirl_modes({bad}) refused both ways, equal {c} {}",
                met(c)
            );
            ok &= c;
        }
        println!("NS3 (a) {name}: accepted both ways met");
    }
    // NS3 (a), the sprung S20.
    let mut sprung = s20();
    sprung
        .model
        .add_support(ElasticSupport::nodal_spring("xmin", 0, 1.0e6))
        .unwrap();
    sprung.name = "S20 sprung";
    let (b, a) = (
        RotatingSector::new(&sprung.model, &off),
        RotatingSector::new(&sprung.model, &off_in),
    );
    let both = match (&b, &a) {
        (Ok(b), Ok(a)) => prestress_bits_equal(&sprung.model, a.part().unit_prestress(), b.part().unit_prestress()),
        _ => false,
    };
    println!(
        "NS3 (a) S20 sprung: accepted both ways, unit_prestress bit-identical {both} {}",
        met(both)
    );
    ok &= both;
    // NS3 (b): a non-zero prescribed value.
    let mut moved = s20();
    moved
        .model
        .add_constraint(Constraint::displacement("xmin", 0, 1.0e-3))
        .unwrap();
    let (b, a) = (
        RotatingSector::new(&moved.model, &off),
        RotatingSector::new(&moved.model, &off_in),
    );
    let nb = b.is_err() && a.is_err() && format!("{:?}", b.err()) == format!("{:?}", a.err());
    println!(
        "NS3 (b) S20 with a non-zero displacement: refused both ways, equal {nb} {}",
        met(nb)
    );
    ok &= nb;
    // NS3 (d): with stress stiffening on, the opt-in refuses the clamped and the sprung sector.
    let on_in = base().free_sector_prestress(true);
    for (name, m) in [
        ("S20 clamped", &clamped_cases()[0].model),
        ("S20 sprung", &sprung.model),
    ] {
        let r = RotatingSector::new(m, &on_in);
        let d = matches!(
            r,
            Err(FemError::InvalidValue {
                key: "free_sector_prestress",
                ..
            })
        );
        println!(
            "NS3 (d) {name}: stress stiffening on, refused free_sector_prestress {d} {}",
            met(d)
        );
        ok &= d;
    }
    assert!(ok, "a registered check missed (see the output)");
}
