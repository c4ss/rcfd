//! F-593 (ADR 0505): the library blade mesher `rcfd_fem::builders::{fan_blade_tet4, fan_blade_tet10,
//! computer_fan_blade_tet10}` against the construction of the F-387 test (`sb_fan_blade.rs`, copied verbatim below as the
//! reference, not imported and not edited): bit-equal meshes and sets (BM1 to BM3, BC1, BC2), the four surface sets (BS1 to
//! BS5), the F-387 identities and results through the builder (BR1 to BR3), refusals (BF1, BF2) and the negative controls
//! NC1 to NC3. The material is nominal (PBT-like round numbers), not data; nothing here is validated against a measured
//! blade.
#![allow(dead_code)]

use rcfd_core::vec3::{cross, dot, norm, sub, Vec3};
use rcfd_fem::builders::{computer_fan_blade_tet10, fan_blade_tet10, fan_blade_tet4, upgrade_to_tet10};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, ElementKind, Load, Material, SolidMesh, SolidModel, SolidResult};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_mesh::fan::{AxialFan, Side};
use rcfd_mesh::fan_design::{BladeDeviation, ComputerFan, Deviation};
use std::f64::consts::PI;

// Nominal PBT-like material (ADR 0500 "Material"): not data.
const E_MOD: f64 = 2.5e9;
const NU: f64 = 0.38;
const RHO: f64 = 1300.0;
// Registered speed and pressure.
const RPM: f64 = 2000.0;
const PRESSURE: f64 = 50.0;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
/// `beta_1 L`, the first root of `1 + cos x cosh x = 0` (as in sb_modal.rs).
const M1: [usize; 3] = [12, 2, 8];
const M2: [usize; 3] = [24, 2, 16];

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
}

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed. A
/// non-finite value is a miss.
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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.4e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}
fn fan() -> AxialFan {
    ComputerFan::default().axial_fan(0, 0.0, 0.0)
}

fn signed_volume(p: [Vec3; 4]) -> f64 {
    dot(sub(p[1], p[0]), cross(sub(p[2], p[0]), sub(p[3], p[0]))) / 6.0
}

/// Facts of a structured blade mesh used by the set-up checks.
struct BladeMeshInfo {
    positive: usize,
    negative: usize,
    // ADR 0500 amendment 1: the sign of the volume times the parity sign of the Kuhn order.
    parity_plus: usize,
    parity_minus: usize,
    min_over_mean: f64,
    hub_faces: usize,
    tip_faces: usize,
    pressure_faces: usize,
}

/// Route S (ADR 0500 D1): the structured Tet4 mesh of the blade with `[n_c, n_t, n_s]` cells, with the sets `all`, `hub`,
/// `tip` (node and surface sets) and `pressure` (surface set).
fn blade_tet4(f: &AxialFan, div: [usize; 3]) -> (SolidMesh, BladeMeshInfo) {
    let [nc, nt, ns] = div;
    // idx[j][i][k]; at i = 0 and i = nc the thickness nodes merge into the k = 0 node.
    let mut nodes: Vec<Vec3> = Vec::new();
    let mut node_j: Vec<usize> = Vec::new();
    let mut node_press: Vec<bool> = Vec::new();
    let mut idx = vec![vec![vec![0usize; nt + 1]; nc + 1]; ns + 1];
    for (j, plane) in idx.iter_mut().enumerate() {
        let r = f.r_hub + (f.r_tip - f.r_hub) * j as f64 / ns as f64;
        let st = f.station_at(r);
        for (i, column) in plane.iter_mut().enumerate() {
            let x = 0.5 * (1.0 - (PI * i as f64 / nc as f64).cos());
            let (zu, su) = f.surface_zs(&st, x, Side::Suction);
            let (zl, sl) = f.surface_zs(&st, x, Side::Pressure);
            let edge = i == 0 || i == nc;
            for k in 0..=nt {
                if edge && k > 0 {
                    column[k] = column[0];
                    continue;
                }
                let w = k as f64 / nt as f64;
                let (z, s) = (zu + w * (zl - zu), su + w * (sl - su));
                nodes.push([r * (s / r).cos(), r * (s / r).sin(), z]);
                node_j.push(j);
                node_press.push(edge || k == nt);
                column[k] = nodes.len() - 1;
            }
        }
    }
    const PERMS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    const ODD: [bool; 6] = [false, true, true, false, false, true];
    let mut conn = Vec::new();
    let (mut positive, mut negative) = (0, 0);
    let (mut parity_plus, mut parity_minus) = (0, 0);
    let mut vols = Vec::new();
    for j in 0..ns {
        for i in 0..nc {
            for k in 0..nt {
                for (p, odd) in PERMS.into_iter().zip(ODD) {
                    let mut c = [i, k, j];
                    let mut v = [idx[j][i][k]; 4];
                    for s in 0..3 {
                        c[p[s]] += 1;
                        v[s + 1] = idx[c[2]][c[0]][c[1]];
                    }
                    if (0..4).any(|a| (a + 1..4).any(|b| v[a] == v[b])) {
                        continue;
                    }
                    let vol = signed_volume(v.map(|n| nodes[n]));
                    if (vol > 0.0) != odd {
                        parity_plus += 1;
                    } else {
                        parity_minus += 1;
                    }
                    if vol > 0.0 {
                        positive += 1;
                    } else {
                        negative += 1;
                        v.swap(1, 2);
                    }
                    vols.push(vol.abs());
                    conn.extend_from_slice(&v);
                }
            }
        }
    }
    let mean = vols.iter().sum::<f64>() / vols.len() as f64;
    let min = vols.iter().copied().fold(f64::INFINITY, f64::min);
    let nn = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Tet4, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..nn).collect()).unwrap();
    mesh.add_node_set("hub", (0..nn).filter(|&n| node_j[n] == 0).collect())
        .unwrap();
    mesh.add_node_set("tip", (0..nn).filter(|&n| node_j[n] == ns).collect())
        .unwrap();
    let (mut hub, mut tip, mut press) = (Vec::new(), Vec::new(), Vec::new());
    for (key, m) in mesh.face_incidence() {
        if m.len() != 1 {
            continue;
        }
        if key.iter().all(|&n| node_j[n] == 0) {
            hub.push(m[0]);
        } else if key.iter().all(|&n| node_j[n] == ns) {
            tip.push(m[0]);
        } else if key.iter().all(|&n| node_press[n]) {
            press.push(m[0]);
        }
    }
    let info = BladeMeshInfo {
        positive,
        negative,
        parity_plus,
        parity_minus,
        min_over_mean: min / mean,
        hub_faces: hub.len(),
        tip_faces: tip.len(),
        pressure_faces: press.len(),
    };
    mesh.add_surface_set("hub", hub).unwrap();
    mesh.add_surface_set("tip", tip).unwrap();
    mesh.add_surface_set("pressure", press).unwrap();
    (mesh, info)
}

fn tet10(mesh: &SolidMesh) -> SolidMesh {
    upgrade_to_tet10(mesh).unwrap().0
}

/// The clamped model: the material on `all`, every component of the set `clamp` held.
fn clamped(mesh: SolidMesh, clamp: &str) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, NU, RHO, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement(clamp, c, 0.0)).unwrap();
    }
    m
}

fn with_load(base: &SolidModel, load: Load) -> SolidModel {
    let mut m = base.clone();
    m.add_load(load).unwrap();
    m
}

fn static_options() -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::Direct).refinement(1)
}

fn modal_options(modes: usize) -> ModalOptions {
    ModalOptions::new(modes)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward)
}

fn corners(mesh: &SolidMesh, e: usize) -> [Vec3; 4] {
    let n = mesh.element_nodes(e);
    [0, 1, 2, 3].map(|a| mesh.nodes()[n[a]])
}

/// `(V, S_perp)`: the meshed volume and its first moment about the z axis, from the corners.
fn first_moment(mesh: &SolidMesh) -> (f64, Vec3) {
    let (mut v, mut s) = (0.0, [0.0; 3]);
    for e in 0..mesh.n_elements() {
        let p = corners(mesh, e);
        let ve = signed_volume(p);
        v += ve;
        for c in 0..2 {
            s[c] += ve * (p[0][c] + p[1][c] + p[2][c] + p[3][c]) / 4.0;
        }
    }
    (v, s)
}

/// Sum of the outward area vectors of a surface set, from the corners (oriented away from the element's other corner).
fn area_vector(mesh: &SolidMesh, set: &str) -> Vec3 {
    let mut a = [0.0; 3];
    for f in mesh.surface_set(set).unwrap() {
        let fc: Vec<usize> = mesh.face_nodes(*f)[..3].to_vec();
        let en = &mesh.element_nodes(f.element)[..4];
        let opp = *en.iter().find(|n| !fc.contains(n)).unwrap();
        let x = |n: usize| mesh.nodes()[n];
        let mut c = cross(sub(x(fc[1]), x(fc[0])), sub(x(fc[2]), x(fc[0])));
        if dot(c, sub(x(opp), x(fc[0]))) > 0.0 {
            c = c.map(|v| -v);
        }
        for k in 0..3 {
            a[k] += 0.5 * c[k];
        }
    }
    a
}

/// Sum of the reactions per component.
fn reaction_sum(model: &SolidModel, r: &SolidResult) -> Vec3 {
    let dofs = model.dof_map();
    let re = r.reactions().unwrap();
    let mut s = [0.0; 3];
    for n in 0..dofs.n_nodes() {
        for (c, sc) in s.iter_mut().enumerate() {
            *sc += re[dofs.equation(n, c).unwrap()];
        }
    }
    s
}

fn rel_vec(a: Vec3, b: Vec3) -> f64 {
    norm(sub(a, b)) / norm(b)
}

/// `|U - W/2| / (W/2)` of a displacement field (scaled by `scale`) of a loaded model.
fn energy_defect(model: &SolidModel, u: &[f64], scale: f64) -> (f64, f64, f64) {
    let us: Vec<f64> = u.iter().map(|v| v * scale).collect();
    let uu = IntegrationPointField::compute(model, &us).unwrap().strain_energy();
    let f = nodal_load_vector(model, &[]).unwrap();
    let w: f64 = f.iter().zip(&us).map(|(a, b)| a * b).sum();
    ((uu - 0.5 * w).abs() / (0.5 * w), uu, w)
}

/// `|u_bar|`, the area mean of the displacement over the Tet10 faces of the set `tip` (corner weights 0, mid-edge `A/3`).
fn tip_displacement(model: &SolidModel, r: &SolidResult) -> f64 {
    let mesh = model.mesh();
    let (mut a, mut ui) = (0.0, [0.0; 3]);
    for f in mesh.surface_set("tip").unwrap() {
        let fnodes = mesh.face_nodes(*f);
        assert_eq!(fnodes.len(), 6, "a six-node face");
        let x = |n: usize| mesh.nodes()[n];
        let af = 0.5 * norm(cross(sub(x(fnodes[1]), x(fnodes[0])), sub(x(fnodes[2]), x(fnodes[0]))));
        a += af;
        for &m in &fnodes[3..] {
            let d = r.displacement(m);
            for k in 0..3 {
                ui[k] += af / 3.0 * d[k];
            }
        }
    }
    norm(ui) / a
}

// ---------------------------------------------------------------------------------------------------------------------
// ADR 0505 checks.
// ---------------------------------------------------------------------------------------------------------------------

/// Number of differences between two meshes: node count, every coordinate by bits, the element count, the kind and every
/// connectivity entry.
fn mesh_diffs(a: &SolidMesh, b: &SolidMesh) -> usize {
    if a.nodes().len() != b.nodes().len() || a.n_elements() != b.n_elements() {
        return usize::MAX;
    }
    let mut d = 0;
    for (p, q) in a.nodes().iter().zip(b.nodes()) {
        d += (0..3).filter(|&c| p[c].to_bits() != q[c].to_bits()).count();
    }
    for e in 0..a.n_elements() {
        if a.element_kind(e) != b.element_kind(e) {
            d += 1;
        }
        let (x, y) = (a.element_nodes(e), b.element_nodes(e));
        d += if x.len() != y.len() {
            1
        } else {
            x.iter().zip(y).filter(|(m, n)| m != n).count()
        };
    }
    d
}

/// Number of the named sets (element, node, surface) that differ from the reference or are missing in `m`.
fn set_diffs(m: &SolidMesh, reference: &SolidMesh, element: &[&str], node: &[&str], surface: &[&str]) -> usize {
    let mut d = 0;
    for n in element {
        d += usize::from(m.element_set(n).is_none() || m.element_set(n) != reference.element_set(n));
    }
    for n in node {
        d += usize::from(m.node_set(n).is_none() || m.node_set(n) != reference.node_set(n));
    }
    for n in surface {
        d += usize::from(m.surface_set(n).is_none() || m.surface_set(n) != reference.surface_set(n));
    }
    d
}

fn area_sum(mesh: &SolidMesh, set: &str) -> f64 {
    mesh.surface_set(set)
        .unwrap()
        .iter()
        .map(|f| {
            let fc = mesh.face_nodes(*f);
            let x = |n: usize| mesh.nodes()[n];
            0.5 * norm(cross(sub(x(fc[1]), x(fc[0])), sub(x(fc[2]), x(fc[0]))))
        })
        .sum()
}

fn pairs(mesh: &SolidMesh, set: &str) -> Vec<(usize, usize)> {
    mesh.surface_set(set)
        .unwrap()
        .iter()
        .map(|f| (f.element, f.face))
        .collect()
}

/// The set checks BS1 to BS5 on one mesh.
fn check_sets(ch: &mut Checks, name: &str, mesh: &SolidMesh, div: [usize; 3]) {
    let [nc, nt, ns] = div;
    let (hubs, tips) = (2 * nt * (nc - 1), 2 * nt * (nc - 1));
    let (press, suct) = (2 * nc * ns, 2 * nc * ns);
    let names = ["hub", "tip", "pressure", "suction"];
    let sizes: Vec<usize> = names
        .iter()
        .map(|s| mesh.surface_set(s).map_or(0, |v| v.len()))
        .collect();
    ch.holds(
        &format!("BS1 {name} sizes hub/tip/pressure/suction"),
        sizes == [hubs, tips, press, suct],
        &format!("{sizes:?} against {:?}", [hubs, tips, press, suct]),
    );
    let mut common = 0;
    for a in 0..4 {
        for b in a + 1..4 {
            let sb: std::collections::BTreeSet<_> = pairs(mesh, names[b]).into_iter().collect();
            common += pairs(mesh, names[a]).iter().filter(|p| sb.contains(p)).count();
        }
    }
    ch.holds(
        &format!("BS2 {name} disjoint"),
        common == 0,
        &format!("{common} common members"),
    );
    let boundary: std::collections::BTreeSet<(usize, usize)> = mesh
        .face_incidence()
        .values()
        .filter(|m| m.len() == 1)
        .map(|m| (m[0].element, m[0].face))
        .collect();
    let union: std::collections::BTreeSet<(usize, usize)> = names.iter().flat_map(|s| pairs(mesh, s)).collect();
    let total = 4 * nc * ns + 4 * nt * (nc - 1);
    ch.holds(
        &format!("BS3 {name} union is the boundary"),
        union == boundary && boundary.len() == total && sizes.iter().sum::<usize>() == total,
        &format!(
            "union {}, boundary {}, formula {total}, missing {}, extra {}",
            union.len(),
            boundary.len(),
            boundary.difference(&union).count(),
            union.difference(&boundary).count()
        ),
    );
    let mut sum = [0.0; 3];
    let mut area = 0.0;
    for s in names {
        let a = area_vector(mesh, s);
        for k in 0..3 {
            sum[k] += a[k];
        }
        area += area_sum(mesh, s);
    }
    ch.at_most(
        &format!("BS4 {name} closed boundary |sum A| / sum |A|"),
        norm(sum) / area,
        1e-12,
    );
    let d = dot(area_vector(mesh, "suction"), area_vector(mesh, "pressure"));
    ch.holds(
        &format!("BS5 {name} suction faces away from pressure"),
        d < 0.0,
        &format!("dot = {d:e}"),
    );
}

#[test]
fn builder_meshes_and_sets() {
    let mut ch = Checks::new();
    let f = fan();
    let node_sets = ["all", "hub", "tip"];
    let surf3 = ["hub", "tip", "pressure"];
    for (name, div, count) in [("M1", M1, 1056), ("M2", M2, 4416)] {
        let (r4, _) = blade_tet4(&f, div);
        let b4 = fan_blade_tet4(&f, div).unwrap();
        let d4 = mesh_diffs(&b4, &r4);
        ch.holds(
            &format!("BM1 {name} Tet4 bit-equal"),
            d4 == 0 && b4.n_elements() == count,
            &format!("{d4} differences, {} elements", b4.n_elements()),
        );
        let r10 = tet10(&r4);
        let b10 = fan_blade_tet10(&f, div).unwrap();
        let d10 = mesh_diffs(&b10, &r10);
        ch.holds(
            &format!("BM2 {name} Tet10 bit-equal"),
            d10 == 0,
            &format!("{d10} differences"),
        );
        let sd =
            set_diffs(&b10, &r10, &["all"], &node_sets, &surf3) + set_diffs(&b4, &r4, &["all"], &node_sets, &surf3);
        ch.holds(
            &format!("BM3 {name} sets equal (Tet4 and Tet10)"),
            sd == 0,
            &format!("{sd} sets differ"),
        );
        check_sets(&mut ch, &format!("{name} Tet10"), &b10, div);
        check_sets(&mut ch, &format!("{name} Tet4"), &b4, div);
        if name == "M1" {
            // NC1: a different division is detected by the comparison.
            let other = fan_blade_tet4(&f, [12, 2, 9]).unwrap();
            let nc1 = mesh_diffs(&other, &r4);
            ch.holds("NC1 [12, 2, 9] differs from M1", nc1 > 0, &format!("{nc1} differences"));
            // NC2: pressure and suction are not the same set.
            ch.holds(
                "NC2 pressure and suction non-empty, different",
                !pairs(&b10, "pressure").is_empty()
                    && !pairs(&b10, "suction").is_empty()
                    && pairs(&b10, "pressure") != pairs(&b10, "suction"),
                "checked",
            );
            // NC3: without the suction set the boundary is not covered, by 2 n_c n_s faces.
            let covered: std::collections::BTreeSet<_> = surf3.iter().flat_map(|s| pairs(&b10, s)).collect();
            let boundary = 4 * M1[0] * M1[2] + 4 * M1[1] * (M1[0] - 1);
            ch.holds(
                "NC3 boundary uncovered without suction",
                boundary - covered.len() == 2 * M1[0] * M1[2],
                &format!("{} uncovered", boundary - covered.len()),
            );
        }
    }
    // BC1: ComputerFan defaults, blade 0.
    let all_surf = ["hub", "tip", "pressure", "suction"];
    let cf = ComputerFan::default();
    let c0 = computer_fan_blade_tet10(&cf, 0, M1).unwrap();
    let b0 = fan_blade_tet10(&cf.axial_fan(0, 0.0, 0.0), M1).unwrap();
    let d = mesh_diffs(&c0, &b0) + set_diffs(&c0, &b0, &["all"], &node_sets, &all_surf);
    ch.holds(
        "BC1 ComputerFan blade 0 equals the AxialFan builder",
        d == 0,
        &format!("{d} differences"),
    );
    // BC2: the blade index selects the blade.
    let mut cf2 = ComputerFan::default();
    let mut blades = vec![BladeDeviation::default(); cf2.n_blades];
    blades[1].chord_rel = 0.05;
    cf2.deviation = Some(Deviation {
        blades,
        ..Deviation::default()
    });
    let c1 = computer_fan_blade_tet10(&cf2, 1, M1).unwrap();
    let a1 = fan_blade_tet10(&cf2.axial_fan(1, 0.0, 0.0), M1).unwrap();
    let c0b = computer_fan_blade_tet10(&cf2, 0, M1).unwrap();
    let eq = mesh_diffs(&c1, &a1) + set_diffs(&c1, &a1, &["all"], &node_sets, &all_surf);
    let dif = mesh_diffs(&c1, &c0b);
    ch.holds(
        "BC2 blade 1 equals axial_fan(1) and differs from blade 0",
        eq == 0 && dif > 0,
        &format!("{eq} differences to axial_fan(1), {dif} to blade 0"),
    );
    // BF1: refusals.
    let refused = [
        fan_blade_tet4(&f, [0, 2, 8]).is_err(),
        fan_blade_tet4(&f, [12, 0, 8]).is_err(),
        fan_blade_tet4(&f, [12, 2, 0]).is_err(),
        fan_blade_tet4(&f, [1, 2, 8]).is_err(),
        computer_fan_blade_tet10(&cf, cf.n_blades, M1).is_err(),
        computer_fan_blade_tet10(&cf, usize::MAX, M1).is_err(),
        {
            let mut bad = f.clone();
            bad.r_tip = bad.r_hub;
            fan_blade_tet4(&bad, M1).is_err()
        },
    ];
    ch.holds(
        "BF1 seven refusals",
        refused.iter().all(|&r| r),
        &format!("{refused:?}"),
    );
    // BF2: the smallest legal mesh.
    let small = fan_blade_tet4(&f, [2, 1, 1]).unwrap();
    let small10 = fan_blade_tet10(&f, [2, 1, 1]).unwrap();
    let nonempty = all_surf.iter().all(|s| !small10.surface_set(s).unwrap().is_empty());
    ch.holds(
        "BF2 [2, 1, 1] builds, 6 Tet4, sets non-empty",
        small.n_elements() == 6 && small10.n_elements() == 6 && nonempty,
        &format!("{} elements", small.n_elements()),
    );
    ch.finish();
}

/// Results of the F-387 runs on one Tet10 mesh, as raw numbers.
struct Run {
    sum_c: Vec3,
    sum_p: Vec3,
    e_r: f64,
    e_p: f64,
    d_tip_c: f64,
    d_tip_p: f64,
    f1: f64,
}

fn run(mesh: SolidMesh) -> Run {
    let (_, s) = first_moment(&mesh);
    let a_p = area_vector(&mesh, "pressure");
    let base = clamped(mesh, "hub");
    let w2 = omega() * omega();
    let mc = with_load(&base, Load::centrifugal(AXIS, [0.0; 3], omega()));
    let rc = solve_static(&mc, &static_options()).unwrap();
    let sum_c = reaction_sum(&mc, &rc);
    let e_r = rel_vec(sum_c, s.map(|x| -RHO * w2 * x));
    let mp = with_load(&base, Load::pressure("pressure", PRESSURE));
    let rp = solve_static(&mp, &static_options()).unwrap();
    let sum_p = reaction_sum(&mp, &rp);
    let e_p = rel_vec(sum_p, a_p.map(|x| PRESSURE * x));
    let r = modal_analysis(&base, &modal_options(4)).unwrap();
    Run {
        sum_c,
        sum_p,
        e_r,
        e_p,
        d_tip_c: tip_displacement(&mc, &rc),
        d_tip_p: tip_displacement(&mp, &rp),
        f1: r.frequencies[0],
    }
}

fn bits(r: &Run) -> Vec<u64> {
    let mut v: Vec<u64> = r.sum_c.iter().chain(&r.sum_p).map(|x| x.to_bits()).collect();
    v.extend([r.d_tip_c, r.d_tip_p, r.f1].map(f64::to_bits));
    v
}

#[test]
fn builder_f387_results() {
    let mut ch = Checks::new();
    let f = fan();
    for (name, div) in [("M1", M1), ("M2", M2)] {
        let reference = run(tet10(&blade_tet4(&f, div).0));
        let built = run(fan_blade_tet10(&f, div).unwrap());
        ch.at_most(&format!("BR1 {name} e_R centrifugal"), built.e_r, 1e-8);
        ch.at_most(&format!("BR1 {name} e_P pressure"), built.e_p, 1e-8);
        let (bb, br) = (bits(&built), bits(&reference));
        let differing = bb.iter().zip(&br).filter(|(a, b)| a != b).count();
        ch.holds(
            &format!("BR2 {name} 9 results bit-equal to the reference"),
            differing == 0,
            &format!(
                "{differing} of {} differ; d_tip p {:.6e}, c {:.6e}, f1 {:.6}",
                bb.len(),
                built.d_tip_p,
                built.d_tip_c,
                built.f1
            ),
        );
        let finite =
            bb.iter().all(|b| f64::from_bits(*b).is_finite()) && built.e_r.is_finite() && built.e_p.is_finite();
        ch.holds(&format!("BR3 {name} all finite"), finite, "checked");
    }
    ch.finish();
}
