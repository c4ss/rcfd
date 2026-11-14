//! F-387 (ADR 0500): demonstration on a fan blade. One blade of the parametric 120 mm computer fan
//! (`ComputerFan::default()`, blade 0) in quadratic tetrahedra, clamped at the hub: the reactions of the centrifugal load at
//! 2000 rpm and of a uniform pressure against their closed sums on the mesh, the strain energy against half the work, the
//! change of the tip displacement and of the first frequency between two meshes, and the flat-plate bounds of the first bending
//! frequency. Checks S0 to S2, R1, P1, E1, C1, C1b, C2, MR, PL0, PL, the route-D rows, and the negative controls NC1 to NC5 of
//! the registration. The material is nominal (PBT-like round numbers), not data; nothing here is validated against a measured
//! blade.

use rcfd_core::vec3::{cross, dot, norm, sub, Vec3};
use rcfd_fem::builders::{tet_box, upgrade_to_tet10};
use rcfd_fem::convert::from_tet_mesh;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mass::{consistent, rigid_motions};
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::{solve_static, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, ElementKind, FaceRef, Load, Material, SolidMesh, SolidModel, SolidResult};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use rcfd_mesh::fan::{gauss_legendre, AxialFan, Side};
use rcfd_mesh::fan_design::ComputerFan;
use rcfd_mesh::surface::TriSurface;
use rcfd_mesh::tetmesh::{tet_quality, tetrahedralize, TetOptions};
use std::collections::BTreeMap;
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
const BETA_1: f64 = 1.8751040687;
const M1: [usize; 3] = [12, 2, 8];
const M2: [usize; 3] = [24, 2, 16];
const M3: [usize; 3] = [24, 4, 16];

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
        println!("{name}: {value:.4e} (limit <= {limit:e}) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn below(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value < limit;
        println!("{name}: {value:.4e} (limit < {limit:e}) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name} = {value:e} >= {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!("{name}: {value:.4e} (limit >= {limit:e}) {}", if ok { "met" } else { "MISSED" });
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

/// Volume `V_b` of the parametric blade and its mean chord `c_m`: 8-point Gauss-Legendre on each station interval.
fn blade_volume_and_mean_chord(f: &AxialFan) -> (f64, f64) {
    let (xg, wg) = gauss_legendre(8);
    let (mut v, mut c) = (0.0, 0.0);
    for w in f.stations.windows(2) {
        let (a, b) = (w[0].radius, w[1].radius);
        for (x, wt) in xg.iter().zip(&wg) {
            let r = 0.5 * (a + b) + 0.5 * (b - a) * x;
            let h = 0.5 * (b - a) * wt;
            v += h * f.section_area(r);
            c += h * f.station_at(r).chord;
        }
    }
    (v, c / (f.r_tip - f.r_hub))
}

/// Facts of a structured blade mesh used by the set-up checks.
struct BladeMeshInfo {
    positive: usize,
    negative: usize,
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
    let mut conn = Vec::new();
    let (mut positive, mut negative) = (0, 0);
    let mut vols = Vec::new();
    for j in 0..ns {
        for i in 0..nc {
            for k in 0..nt {
                for p in PERMS {
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

/// Route D (ADR 0500 D2): the tetrahedral mesher on `blade_surface(n_chord, n_span)` without refinement, in Tet4, and the
/// mesher's quality record.
fn blade_tet4_mesher(f: &AxialFan, n_chord: usize, n_span: usize) -> (SolidMesh, String) {
    let bs = f.blade_surface(n_chord, n_span).unwrap();
    let m = 2 * n_chord;
    let n_row = m * (n_span + 1);
    let radius = |p: Vec3| (p[0] * p[0] + p[1] * p[1]).sqrt();
    let mut tags = vec![0u32; bs.triangles.len()];
    for (t, tag) in bs.triangles.iter().zip(tags.iter_mut()) {
        let at = |r0: f64| t.iter().all(|&v| (radius(bs.points[v]) - r0).abs() <= 1e-12 * r0);
        if at(f.r_hub) {
            *tag = 1;
        } else if at(f.r_tip) {
            *tag = 2;
        } else if t.iter().all(|&v| v < n_row && (v % m == 0 || v % m >= n_chord)) {
            *tag = 3;
        }
    }
    let n_press = tags.iter().filter(|&&t| t == 3).count();
    assert_eq!(n_press, 2 * n_chord * n_span, "route D: pressure triangles");
    let mut s = TriSurface::new(bs.points.clone(), bs.triangles.clone());
    s.tags = tags;
    let opt = TetOptions {
        refine: false,
        ..TetOptions::default()
    };
    let tm = tetrahedralize(&s, &opt).unwrap();
    let q = tet_quality(&tm, 2.0);
    let rec = format!(
        "{} Tet4, {} points, report {:?}, min dihedral {:.3} deg, max radius-edge {:.1}, fraction radius-edge <= 2 {:.3}",
        tm.cells.len(),
        tm.points.len(),
        tm.report,
        q.min_dihedral_deg,
        q.max_radius_edge,
        q.frac_radius_edge_ok
    );
    let names: BTreeMap<u32, String> = [(1, "hub"), (2, "tip"), (3, "pressure")]
        .iter()
        .map(|&(k, v)| (k, v.to_string()))
        .collect();
    (from_tet_mesh(&tm, &names).unwrap(), rec)
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

/// Corner points of element `e`.
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

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

/// Effective mass fractions of each mode for the six rigid motions about the origin.
fn effective_fractions(model: &SolidModel, r: &ModalResult) -> Vec<[f64; 6]> {
    let m = consistent(model).unwrap();
    let rig = rigid_motions(model, [0.0; 3]);
    let mr: Vec<Vec<f64>> = rig.iter().map(|v| mv(&m, v)).collect();
    let dotv = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
    r.modes
        .iter()
        .map(|phi| std::array::from_fn(|k| dotv(phi, &mr[k]).powi(2) / dotv(&rig[k], &mr[k])))
        .collect()
}

fn print_modes(name: &str, model: &SolidModel, r: &ModalResult) {
    let fr = effective_fractions(model, r);
    for (i, f) in r.frequencies.iter().enumerate() {
        println!(
            "{name} mode {}: {f:.4} Hz, residual {:.2e}, effective mass fractions tx {:.3} ty {:.3} tz {:.3} rx {:.3} ry {:.3} rz {:.3}",
            i + 1,
            r.residuals[i],
            fr[i][0],
            fr[i][1],
            fr[i][2],
            fr[i][3],
            fr[i][4],
            fr[i][5]
        );
    }
}

/// The measured quantities of one blade mesh.
struct BladeRun {
    elements: usize,
    e_r: f64,
    e_p: f64,
    e_e_centrifugal: f64,
    e_e_pressure: f64,
    d_tip_pressure: f64,
    d_tip_centrifugal: f64,
    f1: f64,
    max_residual: f64,
}

fn run_blade(name: &str, mesh: SolidMesh) -> BladeRun {
    let elements = mesh.n_elements();
    let (v, s) = first_moment(&mesh);
    let a_p = area_vector(&mesh, "pressure");
    let base = clamped(mesh, "hub");
    let dofs = base.dof_map();
    println!(
        "{name}: {elements} Tet10, {} nodes, {} equations, V = {v:.6e} m^3, r_c = {:.6e} m",
        dofs.n_nodes(),
        dofs.n_equations(),
        norm(s) / v
    );
    let w2 = omega() * omega();
    // R1, centrifugal.
    let mc = with_load(&base, Load::centrifugal(AXIS, [0.0; 3], omega()));
    let rc = solve_static(&mc, &static_options()).unwrap();
    let want_c = s.map(|x| -RHO * w2 * x);
    let sum_c = reaction_sum(&mc, &rc);
    let e_r = rel_vec(sum_c, want_c);
    println!("{name} R1: reactions {sum_c:?} N, -rho omega^2 S_perp {want_c:?} N");
    let (e_e_centrifugal, uc, wc) = energy_defect(&mc, rc.displacement_values(), 1.0);
    println!("{name} E1 centrifugal: U = {uc:.6e} J, W/2 = {:.6e} J", 0.5 * wc);
    let d_tip_centrifugal = tip_displacement(&mc, &rc);
    // P1, pressure.
    let mp = with_load(&base, Load::pressure("pressure", PRESSURE));
    let rp = solve_static(&mp, &static_options()).unwrap();
    let want_p = a_p.map(|x| PRESSURE * x);
    let sum_p = reaction_sum(&mp, &rp);
    let e_p = rel_vec(sum_p, want_p);
    println!("{name} P1: reactions {sum_p:?} N, p sum A_out {want_p:?} N");
    let (e_e_pressure, up, wp) = energy_defect(&mp, rp.displacement_values(), 1.0);
    println!("{name} E1 pressure: U = {up:.6e} J, W/2 = {:.6e} J", 0.5 * wp);
    let d_tip_pressure = tip_displacement(&mp, &rp);
    println!("{name}: d_tip pressure {d_tip_pressure:.6e} m, centrifugal {d_tip_centrifugal:.6e} m");
    // Modes at rest.
    let r = modal_analysis(&base, &modal_options(4)).unwrap();
    print_modes(name, &base, &r);
    let max_residual = r.residuals.iter().copied().fold(0.0, |a: f64, b| if b.is_finite() { a.max(b) } else { f64::INFINITY });
    BladeRun {
        elements,
        e_r,
        e_p,
        e_e_centrifugal,
        e_e_pressure,
        d_tip_pressure,
        d_tip_centrifugal,
        f1: r.frequencies[0],
        max_residual,
    }
}

fn check_identities(ch: &mut Checks, name: &str, prefix: &str, b: &BladeRun) {
    ch.at_most(&format!("{prefix}R1 {name} e_R centrifugal reactions"), b.e_r, 1e-8);
    ch.at_most(&format!("{prefix}P1 {name} e_P pressure reactions"), b.e_p, 1e-8);
    ch.at_most(&format!("{prefix}E1 {name} e_E centrifugal"), b.e_e_centrifugal, 1e-8);
    ch.at_most(&format!("{prefix}E1 {name} e_E pressure"), b.e_e_pressure, 1e-8);
}

fn check_setup(ch: &mut Checks, name: &str, div: [usize; 3], info: &BladeMeshInfo, mesh: &SolidMesh, v_b: f64, s1: f64) {
    let [nc, nt, ns] = div;
    ch.holds(
        &format!("S0 {name} one orientation sign"),
        info.positive == 0 || info.negative == 0,
        &format!("{} positive, {} negative", info.positive, info.negative),
    );
    ch.holds(
        &format!("S0 {name} no degenerate tetrahedron"),
        info.min_over_mean > 0.0 && info.min_over_mean.is_finite(),
        &format!("min V_e / mean V_e = {:.3e}", info.min_over_mean),
    );
    let (v, _) = first_moment(mesh);
    ch.at_most(&format!("S1 {name} |V_M - V_b| / V_b"), (v - v_b).abs() / v_b, s1);
    let want_el = 6 * (nc - 2) * nt * ns + 6 * nt * ns;
    ch.holds(
        &format!("S2 {name} counts"),
        info.pressure_faces == 2 * nc * ns
            && info.hub_faces == 2 * nt * (nc - 1)
            && info.tip_faces == 2 * nt * (nc - 1)
            && mesh.n_elements() == want_el,
        &format!(
            "pressure {} (want {}), hub {}, tip {} (want {}), elements {} (want {want_el})",
            info.pressure_faces,
            2 * nc * ns,
            info.hub_faces,
            info.tip_faces,
            2 * nt * (nc - 1),
            mesh.n_elements()
        ),
    );
}

#[test]
fn s_routes_and_m1_m2() {
    let mut ch = Checks::new();
    let f = fan();
    let (v_b, c_m) = blade_volume_and_mean_chord(&f);
    println!("blade: span {:.6e} m, mean chord {c_m:.6e} m, V_b {v_b:.6e} m^3", f.r_tip - f.r_hub);
    let (m1_4, i1) = blade_tet4(&f, M1);
    let (m2_4, i2) = blade_tet4(&f, M2);
    check_setup(&mut ch, "M1", M1, &i1, &m1_4, v_b, 2e-2);
    check_setup(&mut ch, "M2", M2, &i2, &m2_4, v_b, 5e-3);
    let m1 = tet10(&m1_4);
    let m2 = tet10(&m2_4);
    ch.holds("S0 Tet10 accepted by add_block", true, "M1 and M2 lifted");
    ch.at_least(
        "S2 elements M2 / M1",
        m2.n_elements() as f64 / m1.n_elements() as f64,
        2.0,
    );
    let b1 = run_blade("M1", m1.clone());
    let b2 = run_blade("M2", m2);
    check_identities(&mut ch, "M1", "", &b1);
    check_identities(&mut ch, "M2", "", &b2);
    ch.at_most("MR M1 modal residual (backward)", b1.max_residual, 1e-12);
    ch.at_most("MR M2 modal residual (backward)", b2.max_residual, 1e-12);
    println!(
        "C1 d_tip pressure: M1 {:.6e} m ({} elements), M2 {:.6e} m ({} elements)",
        b1.d_tip_pressure, b1.elements, b2.d_tip_pressure, b2.elements
    );
    ch.below("C1 |d_tip(M2)/d_tip(M1) - 1| pressure", (b2.d_tip_pressure / b1.d_tip_pressure - 1.0).abs(), 0.05);
    println!(
        "C1b d_tip centrifugal: M1 {:.6e} m, M2 {:.6e} m",
        b1.d_tip_centrifugal, b2.d_tip_centrifugal
    );
    ch.below(
        "C1b |d_tip(M2)/d_tip(M1) - 1| centrifugal",
        (b2.d_tip_centrifugal / b1.d_tip_centrifugal - 1.0).abs(),
        0.05,
    );
    println!("C2 f_1: M1 {:.6} Hz, M2 {:.6} Hz", b1.f1, b2.f1);
    ch.below("C2 |f_1(M2)/f_1(M1) - 1|", (b2.f1 / b1.f1 - 1.0).abs(), 0.05);

    // NC1: the axis through (1 mm, 0, 0) against the moment about the origin.
    let (v1, s1) = first_moment(&m1);
    let base1 = clamped(m1.clone(), "hub");
    let mc = with_load(&base1, Load::centrifugal(AXIS, [1e-3, 0.0, 0.0], omega()));
    let rc = solve_static(&mc, &static_options()).unwrap();
    let want = s1.map(|x| -RHO * omega() * omega() * x);
    let nc1 = rel_vec(reaction_sum(&mc, &rc), want);
    println!("NC1 (V = {v1:.6e}): shifted axis, 1 mm / r_c = {:.4e}", 1e-3 * v1 / norm(s1));
    ch.at_least("NC1 e_R with the axis shifted by 1 mm", nc1, 1e-2);
    // NC2, NC3: the pressure case against the suction faces; the energy of a scaled field.
    let mp = with_load(&base1, Load::pressure("pressure", PRESSURE));
    let rp = solve_static(&mp, &static_options()).unwrap();
    let a_p = area_vector(&m1, "pressure");
    let mut suction = Vec::new();
    {
        let press: std::collections::BTreeSet<(usize, usize)> = m1
            .surface_set("pressure")
            .unwrap()
            .iter()
            .map(|f| (f.element, f.face))
            .collect();
        let hub_tip: std::collections::BTreeSet<(usize, usize)> = ["hub", "tip"]
            .iter()
            .flat_map(|s| m1.surface_set(s).unwrap().iter().map(|f| (f.element, f.face)))
            .collect();
        for (key, fr) in m1.face_incidence() {
            let _ = key;
            if fr.len() == 1 {
                let k = (fr[0].element, fr[0].face);
                if !press.contains(&k) && !hub_tip.contains(&k) {
                    suction.push(FaceRef {
                        element: k.0,
                        face: k.1,
                    });
                }
            }
        }
    }
    let mut m1s = m1.clone();
    m1s.add_surface_set("suction", suction).unwrap();
    let a_s = area_vector(&m1s, "suction");
    let sum_p = reaction_sum(&mp, &rp);
    let nc2 = norm(sub(sum_p, a_s.map(|x| PRESSURE * x))) / norm(a_p.map(|x| PRESSURE * x));
    ch.at_least("NC2 pressure reactions against the suction-side force", nc2, 1.0);
    let (nc3, _, _) = energy_defect(&mp, rp.displacement_values(), 1.0 + 1e-3);
    ch.at_least("NC3 e_E of the field scaled by 1 + 1e-3", nc3, 5e-4);
    // NC4: M1 in Tet4 (no lift).
    let r4 = modal_analysis(&clamped(m1_4, "hub"), &modal_options(4)).unwrap();
    println!("NC4 Tet4 M1 f_1 = {:.6} Hz", r4.frequencies[0]);
    ch.at_least("NC4 |f_1(Tet4 M1)/f_1(M2) - 1|", (r4.frequencies[0] / b2.f1 - 1.0).abs(), 0.10);
    ch.finish();
}

#[test]
fn d_mesher_route() {
    let mut ch = Checks::new();
    let f = fan();
    let (d4, rec) = blade_tet4_mesher(&f, 16, 20);
    println!("route D: {rec}");
    let d = tet10(&d4);
    let bd = run_blade("D", d);
    check_identities(&mut ch, "D", "D", &bd);
    let (m2_4, _) = blade_tet4(&f, M2);
    let m2 = tet10(&m2_4);
    // DREC: against M2 (no limit).
    let base = clamped(m2, "hub");
    let mp = with_load(&base, Load::pressure("pressure", PRESSURE));
    let rp = solve_static(&mp, &static_options()).unwrap();
    let d2 = tip_displacement(&mp, &rp);
    let f2 = modal_analysis(&base, &modal_options(1)).unwrap().frequencies[0];
    println!(
        "DREC route D against M2: d_tip {:.6e} / {d2:.6e} - 1 = {:.4e}; f_1 {:.6} / {f2:.6} - 1 = {:.4e}",
        bd.d_tip_pressure,
        bd.d_tip_pressure / d2 - 1.0,
        bd.f1,
        bd.f1 / f2 - 1.0
    );
    ch.finish();
}

/// The plate of the blade's span, mean chord and thickness; `(f_b, f_EB, frequencies, z fractions)`.
fn run_plate(div: [usize; 3]) -> (Option<usize>, f64, Vec<f64>) {
    let f = fan();
    let (v_b, c_m) = blade_volume_and_mean_chord(&f);
    let l = f.r_tip - f.r_hub;
    let t = v_b / (l * c_m);
    let mesh = tet10(&tet_box([0.0; 3], [l, c_m, t], div).unwrap());
    let model = clamped(mesh, "xmin");
    let r = modal_analysis(&model, &modal_options(4)).unwrap();
    print_modes(&format!("plate {div:?}"), &model, &r);
    let fr = effective_fractions(&model, &r);
    let first_bending = (0..r.frequencies.len()).find(|&i| fr[i][2] >= 0.3);
    let f_eb = BETA_1 * BETA_1 / (2.0 * PI * l * l) * (E_MOD * t * t / (12.0 * RHO)).sqrt();
    println!("plate {div:?}: L {l:.6e} m, c_m {c_m:.6e} m, t_m {t:.6e} m, f_EB {f_eb:.6} Hz");
    (first_bending, f_eb, r.frequencies.clone())
}

#[test]
fn pl_flat_plate() {
    let mut ch = Checks::new();
    let (fb, f_eb, fr) = run_plate([16, 12, 2]);
    ch.holds("PL0 the first bending mode is mode 1", fb == Some(0), &format!("{fb:?}"));
    let f_b = fb.map_or(f64::NAN, |i| fr[i]);
    let lo = 0.99 * f_eb;
    let hi = 1.01 * f_eb / (1.0 - NU * NU).sqrt();
    println!(
        "PL f_b = {f_b:.6} Hz, f_b / f_EB = {:.6}, window [{lo:.6}, {hi:.6}] Hz, upper ratio {:.6}",
        f_b / f_eb,
        hi / f_eb
    );
    ch.at_least("PL f_b - 0.99 f_EB (Hz)", f_b - lo, 0.0);
    ch.at_least("PL 1.01 f_EB / sqrt(1 - nu^2) - f_b (Hz)", hi - f_b, 0.0);
    ch.at_least("NC5 f_2 - upper bound (Hz)", fr[1] - hi, 0.0);
    ch.finish();
}

/// Heavy record (ADR 0500 D3): M3 = (24, 4, 16) against M2, and the plate on [24, 18, 3]; no limit.
#[test]
#[ignore = "heavy record of ADR 0500, run once"]
fn heavy_m3_and_plate_refinement() {
    let f = fan();
    let (m2_4, _) = blade_tet4(&f, M2);
    let (m3_4, _) = blade_tet4(&f, M3);
    let b2 = run_blade("M2", tet10(&m2_4));
    let b3 = run_blade("M3", tet10(&m3_4));
    println!(
        "HEAVY M3/M2 - 1: d_tip pressure {:.4e}, centrifugal {:.4e}, f_1 {:.4e}; identities M3 e_R {:.2e} e_P {:.2e} e_E {:.2e} {:.2e}",
        b3.d_tip_pressure / b2.d_tip_pressure - 1.0,
        b3.d_tip_centrifugal / b2.d_tip_centrifugal - 1.0,
        b3.f1 / b2.f1 - 1.0,
        b3.e_r,
        b3.e_p,
        b3.e_e_centrifugal,
        b3.e_e_pressure
    );
    let (fb, f_eb, fr) = run_plate([24, 18, 3]);
    let f_b = fb.map_or(f64::NAN, |i| fr[i]);
    println!("HEAVY plate [24, 18, 3]: f_b {f_b:.6} Hz, f_b / f_EB {:.6}", f_b / f_eb);
}
