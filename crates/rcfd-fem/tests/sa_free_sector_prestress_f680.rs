//! F-680 (ADR 0606): the centrifugal pre-stress of a free cyclic sector. `RotatingSector::new` with
//! `RotatingOptions::free_sector_prestress` on the free ring of ADR 0602 solves the index-0 centrifugal static problem with its
//! two rigid motions (the axial translation and the rotation about the axis) removed by two reaction-free gauge constraints,
//! against ADR 0602's constrained-copy route. Checks FP1, FP2, WH, AG, RX, OR, RM, NI, GN and the controls NC0b, NC2, NC3, DF
//! of the registration (NC0 is ADR 0602's own test; B1 to B3 are `baseline_hashes`, the existing tests and the diff). The ring
//! and the copy route are copied from `sa_ring_whirl_beams_f679.rs` (not edited).

use rcfd_fem::assemble::stiffness;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::rotating::{free_sector_unit_static, RotatingOptions, RotatingSector, WhirlModes};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElasticSupport, ElementKind, FemError, LinearTerm, Load, Material, SolidMesh,
    SolidModel,
};
use std::f64::consts::PI;

type Vec3 = [f64; 3];

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const R: f64 = 0.5;
const BW: f64 = 0.03;
const HH: f64 = 0.02;
const J_T: f64 = 4.7e-8;
const KSH: f64 = 5.0 / 6.0;
const N: usize = 24;
const ORIGIN: Vec3 = [0.3, -0.2, 0.5];
const MESHES: [usize; 2] = [2, 4];
const HARMONICS: [usize; 2] = [2, 3];
/// ADR 0602 arithmetic 1.
const OMEGA_1_TABLE: f64 = 312.434403687127;

/// ADR 0602 arithmetic 4: the polygon model's four lowest `|lambda|` on the coarse mesh, set F, `(j, k, values)`.
const AG_TABLE: [(usize, usize, [f64; 4]); 6] = [
    (1, 2, [277.606374132585, 360.935622735102, 424.386111234562, 424.654099882103]),
    (1, 3, [861.883303688418, 924.417315795607, 1242.513846364662, 1243.021062763019]),
    (2, 2, [253.762145009690, 420.426358337451, 460.879675777895, 461.415635628995]),
    (2, 3, [856.410018758138, 981.488151845963, 1271.221898263074, 1272.236345340532]),
    (3, 2, [239.861473143049, 489.872077918155, 516.050892824875, 516.854789006668]),
    (3, 3, [866.582637948845, 1054.225102212552, 1317.824847221716, 1319.346553541724]),
];

/// ADR 0602 arithmetic 4: the polygon's hoop force over `rho A R^2`, minus 1, per mesh.
const PR_TABLE: [f64; 2] = [-2.851712875399e-3, -7.136922537857e-4];

fn area() -> f64 {
    BW * HH
}
fn i_y() -> f64 {
    BW * HH * HH * HH / 12.0
}
fn i_z() -> f64 {
    HH * BW * BW * BW / 12.0
}

fn met(ok: bool) -> &'static str {
    if ok {
        "met"
    } else {
        "MISSED"
    }
}

fn speeds() -> [f64; 3] {
    [1.0, 2.0, 3.0].map(|j| j * OMEGA_1_TABLE / 6.0)
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

fn p(phi: f64) -> Vec3 {
    let (a, b) = (ea(), eb());
    let (s, c) = phi.sin_cos();
    [0, 1, 2].map(|k| ORIGIN[k] + R * (c * a[k] + s * b[k]))
}

fn section() -> BeamSection {
    BeamSection::new(area(), i_y(), i_z(), J_T, KSH, KSH).unwrap()
}

fn mat() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

/// The free ring sector of ADR 0602 D2 with `nc` elements (and a node set "one" = {1} for NC2).
fn sector(nc: usize) -> SolidModel {
    let al = 2.0 * PI / N as f64;
    let nodes: Vec<Vec3> = (0..=nc).map(|i| p(i as f64 * al / nc as f64)).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..nc).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(section(), axis()).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..nc).collect()).unwrap();
    mesh.add_node_set("low", vec![0]).unwrap();
    mesh.add_node_set("high", vec![nc]).unwrap();
    mesh.add_node_set("one", vec![1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", mat()).unwrap();
    m.add_constraint(Constraint::cyclic_with(
        "low",
        "high",
        axis(),
        ORIGIN,
        N,
        CyclicOptions::new().shells(true).beams(true),
    ))
    .unwrap();
    m
}

/// ADR 0602 D4: the pre-stress at 1 rad/s from a copy with the tangential and axial displacement of node `g` zero (ADR 0602
/// used `g = 1`).
fn prestress_at(nc: usize, g: usize) -> Prestress {
    let mut copy = sector(nc);
    let phi = g as f64 * 2.0 * PI / (N * nc) as f64;
    let t = [0, 1, 2].map(|k| -phi.sin() * ea()[k] + phi.cos() * eb()[k]);
    for d in [t, axis()] {
        copy.add_constraint(Constraint::linear(
            (0..3).map(|c| LinearTerm::new(g, c, d[c])).collect(),
            0.0,
        ))
        .unwrap();
    }
    copy.add_load(Load::centrifugal(axis(), ORIGIN, 1.0)).unwrap();
    let r = solve_static(&copy, &StaticOptions::default()).unwrap();
    Prestress::from_static(&copy, &r).unwrap()
}

/// `max |S_a - S_b|` over every element, point and component, over `max |S_b|`.
fn prestress_difference(nc: usize, a: &Prestress, b: &Prestress) -> f64 {
    let (mut d, mut s): (f64, f64) = (0.0, 0.0);
    for e in 0..nc {
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

/// FP2: the largest deviation of the hoop force of every element over `rho A R^2` minus 1 from the table.
fn hoop_deviation(m: &SolidModel, ps: &Prestress, table: f64) -> f64 {
    let n1 = RHO * area() * R * R;
    let mut pr: f64 = 0.0;
    for e in 0..m.mesh().n_elements() {
        let nodes = m.mesh().element_nodes(e);
        let (x0, x1) = (m.mesh().nodes()[nodes[0]], m.mesh().nodes()[nodes[1]]);
        let d = [0, 1, 2].map(|k| x1[k] - x0[k]);
        let l = dot(d, d).sqrt();
        let e1 = d.map(|v| v / l);
        let s = ps.of_element(e)[0];
        let t = [[s[0], s[3], s[5]], [s[3], s[1], s[4]], [s[5], s[4], s[2]]];
        let mut sn = 0.0;
        for i in 0..3 {
            for j in 0..3 {
                sn += e1[i] * t[i][j] * e1[j];
            }
        }
        pr = pr.max((area() * sn / n1 - 1.0 - table).abs());
    }
    pr
}

fn whirl(rs: &RotatingSector, k: usize, omega: f64) -> WhirlModes {
    rs.whirl_modes(k, omega)
        .unwrap()
        .modes()
        .expect("no loss of stiffness at k >= 2")
        .clone()
}

fn opt_in() -> RotatingOptions {
    RotatingOptions::new(3).coriolis(true).free_sector_prestress(true)
}

#[test]
fn fp1_fp2_wh_ag_gn_the_free_sector_prestress_against_the_constrained_copy() {
    let om = speeds();
    let mut ok = true;
    let (mut wh_max, mut ag_max): (f64, f64) = (0.0, 0.0);
    let mut wh_same = true;
    for (mi, &nc) in MESHES.iter().enumerate() {
        let m = sector(nc);
        let new = RotatingSector::new(&m, &opt_in()).unwrap();
        let ps_copy = prestress_at(nc, 1);
        let copy = RotatingSector::with_unit_prestress(&m, ps_copy.clone(), &RotatingOptions::new(3).coriolis(true)).unwrap();
        // GN.
        let fs = free_sector_unit_static(&m, &opt_in()).unwrap();
        let g = fs.gauge_node;
        let x = m.mesh().nodes()[g];
        let dx = [0, 1, 2].map(|k| x[k] - ORIGIN[k]);
        let s = dot(dx, axis());
        let rho = (dot(dx, dx) - s * s).sqrt();
        let gn = (1..nc).contains(&g) && (rho / R - 1.0).abs() <= 1e-12;
        println!(
            "GN n_c {nc}: gauge node {g}, rho_g / R - 1 = {:.2e} (node in 1..={}, limit 1e-12) {}",
            rho / R - 1.0,
            nc - 1,
            met(gn)
        );
        ok &= gn;
        // FP1.
        let fp1 = prestress_difference(nc, new.part().unit_prestress(), &ps_copy);
        println!(
            "FP1 n_c {nc}: opt-in pre-stress against the copy route: {fp1:.2e} (limit 1e-10) {}",
            met(fp1 <= 1e-10)
        );
        ok &= fp1 <= 1e-10;
        // FP2.
        let fp2 = hoop_deviation(&m, new.part().unit_prestress(), PR_TABLE[mi]);
        println!(
            "FP2 n_c {nc}: hoop force against ADR 0602 PR_TABLE: {fp2:.2e} (limit 1e-8) {}",
            met(fp2 <= 1e-8)
        );
        ok &= fp2 <= 1e-8;
        // WH, AG.
        for (j, &omega) in om.iter().enumerate() {
            for k in HARMONICS {
                let (a, b) = (whirl(&new, k, omega), whirl(&copy, k, omega));
                let same = a.modes.len() == b.modes.len()
                    && a.modes.iter().zip(&b.modes).all(|(x, y)| x.direction == y.direction);
                wh_same &= same;
                let mut d: f64 = 0.0;
                for (x, y) in a.modes.iter().zip(&b.modes) {
                    d = d.max((x.omega / y.omega - 1.0).abs());
                }
                wh_max = wh_max.max(d);
                println!(
                    "WH n_c {nc} Omega_{} k {k}: {} modes, max |omega_new / omega_copy - 1| = {d:.2e}; counts and directions equal {}",
                    j + 1,
                    a.modes.len(),
                    same
                );
                if mi == 0 {
                    let row = AG_TABLE.iter().find(|r| r.0 == j + 1 && r.1 == k).unwrap();
                    for q in 0..4 {
                        ag_max = ag_max.max((a.modes[q].omega / row.2[q] - 1.0).abs());
                    }
                }
            }
        }
    }
    println!(
        "WH every whirl mode: max {wh_max:.2e} (limit 1e-10) {}; counts and directions {}",
        met(wh_max <= 1e-10),
        met(wh_same)
    );
    ok &= wh_max <= 1e-10 && wh_same;
    println!(
        "AG coarse mesh against ADR 0602 AG_TABLE: max {ag_max:.2e} (limit 1e-8) {}",
        met(ag_max <= 1e-8)
    );
    ok &= ag_max <= 1e-8;
    assert!(ok, "a registered check missed (see the output)");
}

/// The six rigid motions of the sector (D1) on its equations: translations along `e_a`, `e_b`, `a`, rotations about `e_a`,
/// `e_b`, `a` through `O`. The last of each three is the cyclic one.
fn rigid_motions(m: &SolidModel) -> Vec<Vec<f64>> {
    let dofs = m.dof_map();
    let dirs = [ea(), eb(), axis()];
    let mut out = Vec::new();
    for rot in [false, true] {
        for d in dirs {
            let mut r = vec![0.0; dofs.n_equations()];
            for node in 0..dofs.n_nodes() {
                let x = m.mesh().nodes()[node];
                let dx = [0, 1, 2].map(|k| x[k] - ORIGIN[k]);
                let (u, th) = if rot { (cross(d, dx), d) } else { (d, [0.0; 3]) };
                for c in 0..3 {
                    r[dofs.equation(node, c).unwrap()] = u[c];
                    r[dofs.equation(node, 3 + c).unwrap()] = th[c];
                }
            }
            out.push(r);
        }
    }
    out
}

/// The rotation by `angle` about the unit axis `a` (Rodrigues).
fn rotate(a: Vec3, angle: f64, v: Vec3) -> Vec3 {
    let (s, c) = angle.sin_cos();
    let axv = cross(a, v);
    let adv = dot(a, v);
    [0, 1, 2].map(|k| v[k] * c + axv[k] * s + a[k] * adv * (1.0 - c))
}

fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

#[test]
fn rx_or_rm_the_rigid_motions_and_the_reaction_free_gauge() {
    let mut ok = true;
    // RX, both meshes: the library's free-sector solution.
    for &nc in &MESHES {
        let m = sector(nc);
        let fs = free_sector_unit_static(&m, &opt_in()).unwrap();
        let mut loaded = sector(nc);
        loaded.add_load(Load::centrifugal(axis(), ORIGIN, 1.0)).unwrap();
        let f = nodal_load_vector(&loaded, &[]).unwrap();
        let k = stiffness(&m).unwrap();
        let u = fs.result.displacement_values();
        let mut ku = vec![0.0; u.len()];
        k.matvec(u, &mut ku);
        let fmax = f.iter().fold(0.0f64, |s, x| s.max(x.abs()));
        let dofs = m.dof_map();
        let mut rx: f64 = 0.0;
        for node in 1..nc {
            for c in 0..dofs.components(node) {
                let eq = dofs.equation(node, c).unwrap();
                rx = rx.max((ku[eq] - f[eq]).abs());
            }
        }
        rx /= fmax;
        println!(
            "RX n_c {nc}: |K u - f| at the interior nodes (gauge {}) over max |f| = {rx:.2e} (limit 1e-10) {}",
            fs.gauge_node,
            met(rx <= 1e-10)
        );
        ok &= rx <= 1e-10;
    }
    // OR, RM: coarse mesh.
    let nc = MESHES[0];
    let m = sector(nc);
    let mut loaded = sector(nc);
    loaded.add_load(Load::centrifugal(axis(), ORIGIN, 1.0)).unwrap();
    let f = nodal_load_vector(&loaded, &[]).unwrap();
    let k = stiffness(&m).unwrap();
    let rs = rigid_motions(&m);
    let names = ["t e_a", "t e_b", "t a", "r e_a", "r e_b", "r a"];
    let kinf = (0..k.n)
        .map(|i| k.val[k.row_ptr[i]..k.row_ptr[i + 1]].iter().map(|v| v.abs()).sum::<f64>())
        .fold(0.0f64, f64::max);
    let dofs = m.dof_map();
    let rot = 2.0 * PI / N as f64;
    for (i, r) in rs.iter().enumerate() {
        let cyclic = i == 2 || i == 5;
        let rinf = r.iter().fold(0.0f64, |s, x| s.max(x.abs()));
        let mut kr = vec![0.0; r.len()];
        k.matvec(r, &mut kr);
        let krinf = kr.iter().fold(0.0f64, |s, x| s.max(x.abs())) / (kinf * rinf);
        let at = |node: usize, base: usize| -> Vec3 { [0, 1, 2].map(|c| r[dofs.equation(node, base + c).unwrap()]) };
        let mut mis: f64 = 0.0;
        for base in [0, 3] {
            let (lo, hi) = (at(0, base), at(nc, base));
            let rl = rotate(axis(), rot, lo);
            for c in 0..3 {
                mis = mis.max((hi[c] - rl[c]).abs());
            }
        }
        mis /= rinf;
        let rm_ok = krinf <= 1e-12 && if cyclic { mis <= 1e-14 } else { mis >= 0.1 };
        println!(
            "RM {}: |K r| / (|K| |r|) = {krinf:.2e} (limit 1e-12); cyclic mismatch {mis:.3e} (limit {}) {}",
            names[i],
            if cyclic { "<= 1e-14" } else { ">= 0.1" },
            met(rm_ok)
        );
        ok &= rm_ok;
        if cyclic {
            let or = dot_v(r, &f).abs() / (norm2(r) * norm2(&f));
            println!(
                "OR {}: |r^T f| / (|r| |f|) = {or:.2e} (limit 1e-13) {}",
                names[i],
                met(or <= 1e-13)
            );
            ok &= or <= 1e-13;
        }
    }
    // NI: the gauge at node 3 in place of node 1 (fine mesh).
    let ni = prestress_difference(MESHES[1], &prestress_at(MESHES[1], 3), &prestress_at(MESHES[1], 1));
    println!(
        "NI n_c {}: copy route at node 3 against node 1: {ni:.2e} (limit 1e-10) {}",
        MESHES[1],
        met(ni <= 1e-10)
    );
    ok &= ni <= 1e-10;
    assert!(ok, "a registered check missed (see the output)");
}

fn dot_v(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[test]
fn nc0b_without_the_option_the_free_sector_is_refused() {
    let m = sector(MESHES[0]);
    let r = RotatingSector::new(
        &m,
        &RotatingOptions::new(3).coriolis(true).free_sector_prestress(false),
    );
    let refused = matches!(r, Err(FemError::SingularStiffness { .. }));
    println!("NC0b free_sector_prestress(false): SingularStiffness {refused} {}", met(refused));
    assert!(refused, "expected SingularStiffness");
}

#[test]
fn nc2_the_opt_in_refuses_supports_and_other_constraints() {
    let mut ok = true;
    let mut a = sector(MESHES[0]);
    a.add_constraint(Constraint::displacement("one", 2, 0.0)).unwrap();
    let mut b = sector(MESHES[0]);
    b.add_support(ElasticSupport::nodal_spring("one", 2, 1.0e6)).unwrap();
    let mut c = sector(MESHES[0]);
    c.add_constraint(Constraint::linear(vec![LinearTerm::new(1, 0, 1.0)], 0.0))
        .unwrap();
    for (name, m) in [("displacement", &a), ("elastic support", &b), ("linear", &c)] {
        let r = RotatingSector::new(m, &opt_in());
        let refused = matches!(r, Err(FemError::InvalidValue { key: "free_sector_prestress", .. }));
        println!("NC2 ({name}): refused with key free_sector_prestress {refused} {}", met(refused));
        ok &= refused;
    }
    assert!(ok, "NC2 missed");
}

#[test]
fn nc3_df_the_option_is_not_read_without_stress_stiffening_and_is_off_by_default() {
    let df = !RotatingOptions::new(3).free_sector_prestress;
    println!("DF RotatingOptions::new(3).free_sector_prestress == false: {df} {}", met(df));
    let m = sector(MESHES[0]);
    let om = speeds()[2];
    let base = RotatingOptions::new(3).coriolis(true).stress_stiffening(false);
    let a = RotatingSector::new(&m, &base.clone().free_sector_prestress(true)).unwrap();
    let b = RotatingSector::new(&m, &base).unwrap();
    let (wa, wb) = (whirl(&a, 2, om), whirl(&b, 2, om));
    let same = wa.modes.len() == wb.modes.len()
        && wa
            .modes
            .iter()
            .zip(&wb.modes)
            .all(|(x, y)| x.omega.to_bits() == y.omega.to_bits())
        && wa.frequencies_all.iter().zip(&wb.frequencies_all).all(|(x, y)| x.to_bits() == y.to_bits());
    println!("NC3 stress_stiffening(false): whirl frequencies with and without the opt-in bit-identical {same} {}", met(same));
    assert!(df && same, "NC3 or DF missed");
}
