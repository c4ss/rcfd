//! F-709 (ADR 0635): the opt-in force of the first mass moment in the centrifugal load of a Shell4 layup
//! (`SolidModel::set_layup_first_moment_force`). Checks TQ, FR, OR1, NF, NT on single facets (FM in a meridian plane, FO
//! oblique), FS, RX, RXc, OR, GN, NT-HB, NC1, NC0 on the free fan-like sector HB (a hub ring and a blade in the meridian plane),
//! and NB, DF of the defaults (B1, B2 are the existing tests). Frame, layup and copy route as ADR 0633 (helpers copied from
//! `sa_free_sector_prestress_tet_layup_ortho_f686.rs`, not edited).

use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::material::MaterialAxes;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::rotating::{free_sector_unit_static, RotatingOptions, RotatingSector};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    BlockProperty, Constraint, ElementKind, FemError, LinearTerm, Load, Material, Ply, SolidMesh, SolidModel,
    SolidResult,
};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
/// The ply thickness of L2 (ADR 0633 D2).
const T_PLY: f64 = 0.025;
/// The speed of the facet checks (D3).
const OMEGA: f64 = 50.0;
/// HB (D3): sectors, hub radius, blade tip radius, axial extent, divisions.
const N_HB: usize = 6;
const R_HUB: f64 = 0.5;
const R_TIP: f64 = 1.0;
const H_AX: f64 = 0.3;
const ND: usize = 3;
/// Arithmetic 5: `m1 int rho dA` of the blade over `m1`.
const BLADE_INT_RHO: f64 = 0.3 * (1.0 - 0.25) / 2.0;

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

fn unit(v: Vec3) -> Vec3 {
    let l = dot(v, v).sqrt();
    v.map(|x| x / l)
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

/// The L2 layup (ADR 0633 D2): bottom 0 degrees `rho = 1400`, top 90 degrees `rho = 2800`, each `T_PLY`.
fn plies() -> Vec<Ply> {
    vec![
        Ply::new(ply_material(1400.0), T_PLY, 0.0).unwrap(),
        Ply::new(ply_material(2800.0), T_PLY, 0.5 * PI).unwrap(),
    ]
}

/// `(rho_k t_k, zbar_k)` of the plies, bottom first, the mid-plane at the half-thickness.
fn ply_moments(p: &[Ply]) -> Vec<(f64, f64)> {
    let t: f64 = p.iter().map(|q| q.thickness).sum();
    let mut z = -0.5 * t;
    let mut out = Vec::new();
    for q in p {
        out.push((q.material.density() * q.thickness, z + 0.5 * q.thickness));
        z += q.thickness;
    }
    out
}

fn m1_of(p: &[Ply]) -> f64 {
    ply_moments(p).iter().map(|(m, z)| m * z).sum()
}

fn radial(x: Vec3) -> Vec3 {
    let d = sub(x, ORIGIN);
    let s = dot(d, axis());
    [0, 1, 2].map(|k| d[k] - s * axis()[k])
}

fn dot_v(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
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

fn load_vector(m: &SolidModel, omega: f64) -> Vec<f64> {
    let mut loaded = m.clone();
    loaded.add_load(Load::centrifugal(axis(), ORIGIN, omega)).unwrap();
    nodal_load_vector(&loaded, &[]).unwrap()
}

/// The sum of the node forces (the translations) of a load vector.
fn force_resultant(m: &SolidModel, f: &[f64]) -> Vec3 {
    let dofs = m.dof_map();
    let mut s = [0.0; 3];
    for node in 0..m.mesh().nodes().len() {
        for (c, sc) in s.iter_mut().enumerate() {
            if let Some(eq) = dofs.equation(node, c) {
                *sc += f[eq];
            }
        }
    }
    s
}

/// `e_3` (the normalised cross product of the diagonals, the library's convention, ADR 0633 OL), the area and the area
/// centroid of a planar quad.
fn quad_geometry(c: &[Vec3]) -> (Vec3, f64, Vec3) {
    let e3 = unit(cross(sub(c[2], c[0]), sub(c[3], c[1])));
    let (mut area, mut xc) = (0.0, [0.0; 3]);
    for (p, q, s) in [(0, 1, 2), (0, 2, 3)] {
        let at = 0.5 * dot(cross(sub(c[q], c[p]), sub(c[s], c[p])), e3);
        area += at;
        for k in 0..3 {
            xc[k] += at * (c[p][k] + c[q][k] + c[s][k]) / 3.0;
        }
    }
    (e3, area, xc.map(|v| v / area))
}

/// FM (D3): the meridian facet of ADR 0633 OL.
fn fm_corners() -> Vec<Vec3> {
    let (a, e) = (axis(), ea());
    let corner = |r: f64, z: f64| -> Vec3 { [0, 1, 2].map(|k| ORIGIN[k] + r * e[k] + z * a[k]) };
    vec![corner(0.5, 0.0), corner(0.7, 0.0), corner(0.7, 0.2), corner(0.5, 0.25)]
}

/// FO (D3): the oblique planar facet.
fn fo_corners() -> Vec<Vec3> {
    let (a, e, b) = (axis(), ea(), eb());
    let n0 = unit([0, 1, 2].map(|k| 0.6 * e[k] + 0.5 * b[k] + 0.6 * a[k]));
    let u1 = unit(cross(a, n0));
    let u2 = cross(n0, u1);
    let c0: Vec3 = [0, 1, 2].map(|k| ORIGIN[k] + 0.6 * e[k] + 0.1 * a[k]);
    [(0.0, 0.0), (0.2, 0.02), (0.17, 0.21), (-0.03, 0.15)]
        .iter()
        .map(|&(s, t)| [0, 1, 2].map(|k| c0[k] + s * u1[k] + t * u2[k]))
        .collect()
}

/// One Shell4 facet with `property`, steel block material.
fn facet_model(c: &[Vec3], property: BlockProperty, on: bool) -> SolidModel {
    let mut mesh = SolidMesh::new(c.to_vec()).unwrap();
    mesh.add_block(ElementKind::Shell4, vec![0, 1, 2, 3]).unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", vec![0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.set_layup_first_moment_force(on);
    m
}

fn layup_facet(c: &[Vec3], on: bool) -> SolidModel {
    facet_model(c, BlockProperty::layup(plies()).unwrap(), on)
}

/// D1: the through-thickness integral of the exact body force, ply by ply, `Omega^2 A sum_k rho_k t_k P(x_c + zbar_k e_3 - O)`.
fn exact_resultant(c: &[Vec3], omega: f64) -> Vec3 {
    let (e3, area, xc) = quad_geometry(c);
    let mut s = [0.0; 3];
    for (m, z) in ply_moments(&plies()) {
        let r = radial([0, 1, 2].map(|k| xc[k] + z * e3[k]));
        for k in 0..3 {
            s[k] += omega * omega * area * m * r[k];
        }
    }
    s
}

/// The closed-form torque of the default load (ADR 0633 D1) and the scale `m1 A rho_c Omega^2`.
fn default_torque(c: &[Vec3], omega: f64) -> (f64, f64) {
    let (e3, area, xc) = quad_geometry(c);
    let rc = radial(xc);
    let m1 = m1_of(&plies());
    let w2 = omega * omega;
    (
        m1 * area * w2 * dot(e3, cross(rc, axis())),
        m1 * area * dot(rc, rc).sqrt() * w2,
    )
}

fn rel(a: Vec3, b: Vec3) -> f64 {
    let d = sub(a, b);
    (dot(d, d) / dot(b, b)).sqrt()
}

#[test]
fn tq_fr_or1_nf_nt_the_layup_centrifugal_load_on_a_meridian_and_an_oblique_facet() {
    let mut ok = true;
    for (name, c) in [("FM", fm_corners()), ("FO", fo_corners())] {
        let (tc, scale) = default_torque(&c, OMEGA);
        let reference = exact_resultant(&c, OMEGA);
        // On.
        let m = layup_facet(&c, true);
        let f = load_vector(&m, OMEGA);
        let [r1, r2] = cyclic_rigid_motions(&m);
        let tq = dot_v(&r2, &f).abs() / scale;
        println!(
            "TQ-{name}: on, |r_2^T f| / (m1 A rho_c Omega^2) = {tq:.2e} (limit 1e-12) {}",
            met(tq <= 1e-12)
        );
        ok &= tq <= 1e-12;
        let fr_vec = force_resultant(&m, &f);
        let fr = rel(fr_vec, reference);
        println!(
            "FR-{name}: on, resultant {fr_vec:?} against the exact {reference:?}: {fr:.2e} (limit 1e-12) {}",
            met(fr <= 1e-12)
        );
        ok &= fr <= 1e-12;
        let or1 = dot_v(&r1, &f).abs() / (dot_v(&r1, &r1).sqrt() * dot_v(&f, &f).sqrt());
        println!(
            "OR1-{name}: on, |r_1^T f| / (|r_1| |f|) = {or1:.2e} (limit 1e-13) {}",
            met(or1 <= 1e-13)
        );
        ok &= or1 <= 1e-13;
        // Off.
        let m = layup_facet(&c, false);
        let f = load_vector(&m, OMEGA);
        let nf = rel(force_resultant(&m, &f), reference);
        println!(
            "NF-{name}: off, the resultant's miss = {nf:.2e} (at least 1e-3) {}",
            met(nf >= 1e-3)
        );
        ok &= nf >= 1e-3;
        let r2 = &cyclic_rigid_motions(&m)[1];
        let work = dot_v(r2, &f);
        let nt = (work - tc).abs() / scale;
        let size = tc.abs() / scale;
        println!(
            "NT-{name}: off, r_2^T f = {work:.6e}, closed form {tc:.6e}: {nt:.2e} (limit 1e-10) {}; |T_c| / scale = {size:.6e} (at least 0.1) {}",
            met(nt <= 1e-10),
            met(size >= 0.1)
        );
        ok &= nt <= 1e-10 && size >= 0.1;
    }
    assert!(ok, "a registered check missed (see the output)");
}

/// HB (D3): the hub ring and the blade in the meridian plane at mid-sector, the L2 layup on every element, free.
fn hb(on: bool) -> SolidModel {
    let (a, e, b) = (axis(), ea(), eb());
    let at = |r: f64, phi: f64, z: f64| -> Vec3 {
        let (s, c) = phi.sin_cos();
        [0, 1, 2].map(|k| ORIGIN[k] + r * (c * e[k] + s * b[k]) + z * a[k])
    };
    let alpha = 2.0 * PI / N_HB as f64;
    let mut nodes = Vec::new();
    // Hub: node (i, j) = j * 3 + i, i = 0, 1, 2 the columns, j = 0..=ND the axial stations.
    for j in 0..=ND {
        for i in 0..3 {
            nodes.push(at(R_HUB, 0.5 * alpha * i as f64, H_AX * j as f64 / ND as f64));
        }
    }
    let hub = |i: usize, j: usize| j * 3 + i;
    // Blade: radial stations q = 1..=ND (q = 0 is hub column 1), axial j.
    let base = nodes.len();
    for j in 0..=ND {
        for q in 1..=ND {
            let r = R_HUB + (R_TIP - R_HUB) * q as f64 / ND as f64;
            nodes.push(at(r, 0.5 * alpha, H_AX * j as f64 / ND as f64));
        }
    }
    let blade = |q: usize, j: usize| if q == 0 { hub(1, j) } else { base + j * ND + (q - 1) };
    let mut conn = Vec::new();
    for j in 0..ND {
        for i in 0..2 {
            conn.extend([hub(i, j), hub(i + 1, j), hub(i + 1, j + 1), hub(i, j + 1)]);
        }
    }
    for j in 0..ND {
        for q in 0..ND {
            conn.extend([blade(q, j), blade(q + 1, j), blade(q + 1, j + 1), blade(q, j + 1)]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::layup(plies()).unwrap())
        .unwrap();
    let ne = mesh.blocks()[0].len();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("low", (0..=ND).map(|j| hub(0, j)).collect()).unwrap();
    mesh.add_node_set("high", (0..=ND).map(|j| hub(2, j)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N_HB,
        CyclicOptions::new().shells(true),
    ))
    .unwrap();
    m.set_layup_first_moment_force(on);
    m
}

fn interior(m: &SolidModel) -> Vec<usize> {
    let mesh = m.mesh();
    let mut on = vec![false; mesh.nodes().len()];
    for name in ["low", "high"] {
        for &n in mesh.node_set(name).unwrap() {
            on[n] = true;
        }
    }
    (0..on.len()).filter(|&n| !on[n]).collect()
}

/// D3: the constrained-copy route, `a . u_A = 0` at hub column 1, `z = 0` (node 1), `t_B . u_B = 0` at hub column 1,
/// `z = 0.3` (node `3 ND + 1`).
fn copy_route(m: &SolidModel) -> (SolidResult, Prestress) {
    let (na, nb) = (1, 3 * ND + 1);
    let mut copy = m.clone();
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
    (r, ps)
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

/// `max |K u - f|` over every equation of the interior nodes, over `max |f|`.
fn interior_residual(m: &SolidModel, k: &CsrMatrix, u: &[f64], f: &[f64]) -> f64 {
    let mut ku = vec![0.0; u.len()];
    k.matvec(u, &mut ku);
    let fmax = f.iter().fold(0.0f64, |s, x| s.max(x.abs()));
    let dofs = m.dof_map();
    let mut rx: f64 = 0.0;
    for node in interior(m) {
        for c in 0..dofs.components(node) {
            let eq = dofs.equation(node, c).unwrap();
            rx = rx.max((ku[eq] - f[eq]).abs());
        }
    }
    rx / fmax
}

fn base() -> RotatingOptions {
    RotatingOptions::new(3).coriolis(true)
}

#[test]
fn fs_rx_rxc_or_gn_nt_nc1_nc0_the_free_hub_and_meridian_blade_layup_sector() {
    let mut ok = true;
    let opt = base().free_sector_prestress(true);
    // On.
    let m = hb(true);
    let new = RotatingSector::new(&m, &opt).unwrap();
    let (r_copy, ps_copy) = copy_route(&m);
    let fs = prestress_difference(&m, new.part().unit_prestress(), &ps_copy);
    println!(
        "FS HB on: opt-in pre-stress against the copy route = {fs:.2e} (limit 1e-10) {}",
        met(fs <= 1e-10)
    );
    ok &= fs <= 1e-10;
    let lib = free_sector_unit_static(&m, &opt).unwrap();
    let g = lib.gauge_node;
    let d = radial(m.mesh().nodes()[g]);
    let rho = dot(d, d).sqrt();
    let inside = interior(&m).contains(&g);
    let gn = inside && (rho / R_TIP - 1.0).abs() <= 1e-12;
    println!(
        "GN HB: gauge node {g} interior {inside}, rho_g / r_max - 1 = {:.2e} (limit 1e-12) {}",
        rho / R_TIP - 1.0,
        met(gn)
    );
    ok &= gn;
    let f = load_vector(&m, 1.0);
    let k = stiffness(&m).unwrap();
    let rx = interior_residual(&m, &k, lib.result.displacement_values(), &f);
    println!(
        "RX HB on: |K u - f| at the interior nodes over max |f| = {rx:.2e} (limit 1e-10) {}",
        met(rx <= 1e-10)
    );
    ok &= rx <= 1e-10;
    let rxc = interior_residual(&m, &k, r_copy.displacement_values(), &f);
    println!(
        "RXc HB on: the copy route's residual = {rxc:.2e} (limit 1e-10) {}",
        met(rxc <= 1e-10)
    );
    ok &= rxc <= 1e-10;
    let fnorm = dot_v(&f, &f).sqrt();
    for (i, r) in cyclic_rigid_motions(&m).iter().enumerate() {
        let or = dot_v(r, &f).abs() / (dot_v(r, r).sqrt() * fnorm);
        println!(
            "OR HB on r_{}: |r^T f| / (|r| |f|) = {or:.2e} (limit 1e-13) {}",
            i + 1,
            met(or <= 1e-13)
        );
        ok &= or <= 1e-13;
    }
    // Off.
    let m0 = hb(false);
    let f0 = load_vector(&m0, 1.0);
    let work = dot_v(&cyclic_rigid_motions(&m0)[1], &f0);
    let expected = m1_of(&plies()) * BLADE_INT_RHO;
    let nt = (work.abs() / expected - 1.0).abs();
    println!(
        "NT-HB off: r_2^T f = {work:.6e}, m1 int rho dA = {expected:.6e}: {nt:.2e} (limit 1e-10) {}",
        met(nt <= 1e-10)
    );
    ok &= nt <= 1e-10;
    let new0 = RotatingSector::new(&m0, &opt).unwrap();
    let (_, ps0) = copy_route(&m0);
    let nc1 = prestress_difference(&m0, new0.part().unit_prestress(), &ps0);
    println!(
        "NC1 HB off: free-sector pre-stress against the copy route = {nc1:.2e} (at least 1e-5) {}",
        met(nc1 >= 1e-5)
    );
    ok &= nc1 >= 1e-5;
    // NC0.
    for (label, model) in [("on", &m), ("off", &m0)] {
        let r = RotatingSector::new(model, &base());
        let refused = matches!(r, Err(FemError::SingularStiffness { .. }));
        println!(
            "NC0 HB {label}: without free_sector_prestress refused SingularStiffness {refused} {}",
            met(refused)
        );
        ok &= refused;
    }
    assert!(ok, "a registered check missed (see the output)");
}

fn bits(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn nb_df_the_opt_in_leaves_other_loads_and_the_default() {
    let mut ok = true;
    let c = fo_corners();
    let mesh = SolidMesh::new(c.clone()).unwrap();
    let df = !SolidModel::new(mesh).layup_first_moment_force();
    println!(
        "DF SolidModel::new(..).layup_first_moment_force() == false: {df} {}",
        met(df)
    );
    ok &= df;
    let pairs: Vec<(&str, SolidModel, SolidModel)> = vec![
        (
            "Shell 0.05 Shell4",
            facet_model(&c, BlockProperty::shell(0.05).unwrap(), false),
            facet_model(&c, BlockProperty::shell(0.05).unwrap(), true),
        ),
        (
            "one isotropic ply Shell4",
            facet_model(
                &c,
                BlockProperty::layup(vec![Ply::new(steel(), 0.05, 0.0).unwrap()]).unwrap(),
                false,
            ),
            facet_model(
                &c,
                BlockProperty::layup(vec![Ply::new(steel(), 0.05, 0.0).unwrap()]).unwrap(),
                true,
            ),
        ),
        ("Shell 0.05 Shell3", shell3_model(&c, false), shell3_model(&c, true)),
        ("Hex8 solid", hex_model(false), hex_model(true)),
    ];
    for (name, off, on) in &pairs {
        let same = bits(&load_vector(off, OMEGA), &load_vector(on, OMEGA));
        println!("NB {name}: centrifugal load bits equal on and off {same} {}", met(same));
        ok &= same;
    }
    let gravity = |m: &SolidModel| {
        let mut l = m.clone();
        l.add_load(Load::gravity([1.0, -2.0, -9.81])).unwrap();
        nodal_load_vector(&l, &[]).unwrap()
    };
    let same = bits(&gravity(&layup_facet(&c, false)), &gravity(&layup_facet(&c, true)));
    println!(
        "NB gravity on the L2 layup FO: bits equal on and off {same} {}",
        met(same)
    );
    ok &= same;
    assert!(ok, "a registered check missed (see the output)");
}

/// NB: the FO facet split in two Shell3 triangles, `Shell { 0.05 }` steel.
fn shell3_model(c: &[Vec3], on: bool) -> SolidModel {
    let mut mesh = SolidMesh::new(c.to_vec()).unwrap();
    mesh.add_block(ElementKind::Shell3, vec![0, 1, 2, 0, 2, 3]).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.05).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.set_layup_first_moment_force(on);
    m
}

/// NB: a Hex8 steel box.
fn hex_model(on: bool) -> SolidModel {
    let mesh = hex_box([0.25, 0.0, -0.1], [0.5, 0.3, 0.3], [2, 2, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.set_layup_first_moment_force(on);
    m
}
