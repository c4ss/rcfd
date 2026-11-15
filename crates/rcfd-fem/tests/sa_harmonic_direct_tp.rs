//! F-528 (ADR 0543): the direct harmonic solve on the transformation path and with a pre-stress. Checks K1, FD1, RS1, RS2,
//! RY1, P1 to P3, Z1, T1, F1, the controls N1 to N3, the refusals RT1 to RT5 and the copy R1 to R11' of the refusals of
//! ADR 0448 (amendment 2) with R9 superseded.

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicOptions, HarmonicResponse, HarmonicSolver, ModalBasis, ModalDamping};
use rcfd_fem::harmonic_direct::{BaseMotion, Dashpot, DirectHarmonic, DirectHarmonicOptions, LossFactor, ModulusTable};
use rcfd_fem::mass::consistent_with;
use rcfd_fem::mpc::{add_supports, transformation};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{Constraint, ElasticSupport, ElementKind, FemError, LinearTerm, Material, SolidMesh, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen::generalized_eigen_dense;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const E_MOD: f64 = 2e11;
const RHO: f64 = 7850.0;
const B: f64 = 0.04;
const A: f64 = B * B;
const L: f64 = 1.0;
const ETA: f64 = 0.02;
/// The bar of ADR 0448.
const N: usize = 80;
/// The tip spring of each `xmax` node of the bar (four nodes: `EA/L` in all; ADR 0543 arithmetic).
const K_NODE: f64 = E_MOD * A / L / 4.0;
/// The uniform pre-stress of P1 to P3 (ADR 0543 arithmetic).
const SIGMA: f64 = 2e8;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} against <= {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} against >= {limit:e}"));
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

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. } | FemError::FieldLength { key, .. } => Some(key),
        _ => None,
    }
}

fn refused<T>(c: &mut Checks, name: &str, r: rcfd_fem::Result<T>, key: &str, contains: &str) {
    match r {
        Ok(_) => c.holds(name, false, "accepted"),
        Err(e) => {
            let m = e.to_string();
            c.holds(name, key_of(&e) == Some(key) && m.contains(contains), &m);
        }
    }
}

fn steel(e: f64, nu: f64) -> Material {
    Material::isotropic_elastic(e, nu, RHO, 0.0).unwrap()
}

fn w1() -> f64 {
    PI * (E_MOD / RHO).sqrt() / (2.0 * L)
}

// ------------------------------------------------------------------------------------------------------------ meshes

/// `mesh` cut at `x = cut`: the nodes there duplicated, the elements right of the cut on the copies. Returns the mesh (node
/// sets `all`, `xmin`, `xmax`, `junction` = the nodes at the cut and their copies; element set `all`) and the map of each
/// node to its node of `mesh`.
fn split(mesh: &SolidMesh, cut: f64) -> (SolidMesh, Vec<usize>) {
    let mut nodes = mesh.nodes().to_vec();
    let n0 = nodes.len();
    let at_cut: Vec<usize> = (0..n0).filter(|&a| (nodes[a][0] - cut).abs() < 1e-12).collect();
    let mut copy = vec![usize::MAX; n0];
    let mut map: Vec<usize> = (0..n0).collect();
    for &a in &at_cut {
        copy[a] = nodes.len();
        nodes.push(mesh.nodes()[a]);
        map.push(a);
    }
    let mut conn = Vec::new();
    for e in 0..mesh.n_elements() {
        let en = mesh.element_nodes(e);
        let xc: f64 = en.iter().map(|&a| mesh.nodes()[a][0]).sum::<f64>() / en.len() as f64;
        for &a in en {
            conn.push(if xc > cut && copy[a] != usize::MAX { copy[a] } else { a });
        }
    }
    let total = nodes.len();
    let mut m = SolidMesh::new(nodes).unwrap();
    m.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = m.n_elements();
    m.add_element_set("all", (0..ne).collect()).unwrap();
    m.add_node_set("all", (0..total).collect()).unwrap();
    m.add_node_set("xmin", mesh.node_set("xmin").unwrap().to_vec()).unwrap();
    m.add_node_set("xmax", mesh.node_set("xmax").unwrap().to_vec()).unwrap();
    let mut junction = at_cut.clone();
    junction.extend(at_cut.iter().map(|&a| copy[a]));
    m.add_node_set("junction", junction).unwrap();
    (m, map)
}

fn with_junction(mut mesh: SolidMesh, cut: f64) -> SolidMesh {
    let at: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&a| (mesh.nodes()[a][0] - cut).abs() < 1e-12)
        .collect();
    mesh.add_node_set("junction", at).unwrap();
    mesh
}

// --------------------------------------------------------------------------------------------------------------- bar

fn bar_mesh() -> SolidMesh {
    hex_box([0.0; 3], [L, B, B], [N, 1, 1]).unwrap()
}

fn hold_bar(m: &mut SolidModel) {
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
}

fn bar_on(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(E_MOD, 0.0)).unwrap();
    hold_bar(&mut m);
    m
}

fn bar_with(cells: usize) -> SolidModel {
    bar_on(hex_box([0.0; 3], [L, B, B], [cells, 1, 1]).unwrap())
}

fn sprung_bar() -> SolidModel {
    let mut m = bar_on(bar_mesh());
    m.add_support(ElasticSupport::nodal_spring("xmax", 0, K_NODE)).unwrap();
    m
}

fn bar_tip_load(model: &SolidModel) -> HarmonicLoad {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &a in model.mesh().node_set("xmax").unwrap() {
        f[dofs.equation(a, 0).unwrap()] = 0.25;
    }
    HarmonicLoad::real(f).unwrap()
}

/// The 1-D chain of `N` linear elements with consistent mass, complex modulus `es`, a real tip spring `ks` and a tip dashpot
/// `c`, tip force 1 N, node 0 held: every nodal displacement (index 0 the held node), by the Thomas algorithm.
fn chain(es: C64, w: f64, ks: f64, c: f64) -> Vec<C64> {
    let h = L / N as f64;
    let kk = es.scale(A / h);
    let mm = RHO * A * h / 6.0;
    let n = N;
    let diag: Vec<C64> = (1..=n)
        .map(|j| {
            if j < n {
                kk.scale(2.0) - C64::from_real(w * w * 4.0 * mm)
            } else {
                kk - C64::from_real(w * w * 2.0 * mm) + C64::from_real(ks) + C64::new(0.0, w * c)
            }
        })
        .collect();
    let off = -kk - C64::from_real(w * w * mm);
    let mut rp = vec![C64::ZERO; n];
    rp[n - 1] = C64::ONE;
    let mut dp = diag.clone();
    for j in 1..n {
        let m = off / dp[j - 1];
        dp[j] -= m * off;
        let prev = rp[j - 1];
        rp[j] -= m * prev;
    }
    let mut x = vec![C64::ZERO; n + 1];
    x[n] = rp[n - 1] / dp[n - 1];
    for j in (1..n).rev() {
        x[j] = (rp[j - 1] - off * x[j + 1]) / dp[j - 1];
    }
    x
}

/// `max_j |X_j - x(j)| / max_j |x(j)|` over the axial unknown of every node of the bar.
fn e_chain(model: &SolidModel, r: &HarmonicResponse, x: &[C64]) -> f64 {
    let dofs = model.dof_map();
    let h = L / N as f64;
    let scale = x.iter().map(|z| z.abs()).fold(0.0, f64::max);
    let mut worst = 0.0f64;
    for (a, p) in model.mesh().nodes().iter().enumerate() {
        let j = (p[0] / h).round() as usize;
        let eq = dofs.equation(a, 0).unwrap();
        let d = (C64::new(r.re[eq], r.im[eq]) - x[j]).abs();
        worst = worst.max(d);
    }
    worst / scale
}

// ------------------------------------------------------------------------------------------------------- cantilever

const CUT: f64 = 0.3;

fn c_mesh() -> SolidMesh {
    hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap()
}

fn cantilever_on(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(E_MOD, 0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn cantilever() -> SolidModel {
    cantilever_on(c_mesh())
}

/// The merged model of RS2: `C` with the rigid set on its junction and the spring at `xmax`.
fn merged() -> SolidModel {
    let mut m = cantilever_on(with_junction(c_mesh(), CUT));
    m.add_constraint(Constraint::rigid_set("junction", [CUT, 0.05, 0.05]))
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("xmax", 2, 1e6)).unwrap();
    m
}

/// The split model of RS2 and the map of its nodes to the merged model's.
fn split_cantilever() -> (SolidModel, Vec<usize>) {
    let (mesh, map) = split(&c_mesh(), CUT);
    let mut m = cantilever_on(mesh);
    m.add_constraint(Constraint::rigid_set("junction", [CUT, 0.05, 0.05]))
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("xmax", 2, 1e6)).unwrap();
    (m, map)
}

/// Junction node `J`: the original node at `x = CUT`, `y = z = 0`.
fn junction_node(model: &SolidModel) -> usize {
    model
        .mesh()
        .nodes()
        .iter()
        .position(|p| (p[0] - CUT).abs() < 1e-12 && p[1].abs() < 1e-12 && p[2].abs() < 1e-12)
        .unwrap()
}

fn c_load(model: &SolidModel, with_junction_load: bool) -> HarmonicLoad {
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let (mut re, mut im) = (vec![0.0; n], vec![0.0; n]);
    let tip = model.mesh().node_set("xmax").unwrap();
    let share = 1.0 / tip.len() as f64;
    for &a in tip {
        for (c, (fr, fi)) in [(0.3, 0.0), (1.0, 0.2), (-0.5, 0.0)].iter().enumerate() {
            let eq = dofs.equation(a, c).unwrap();
            re[eq] += share * fr;
            im[eq] += share * fi;
        }
    }
    if with_junction_load {
        let j = junction_node(model);
        re[dofs.equation(j, 2).unwrap()] += 0.4;
        im[dofs.equation(j, 0).unwrap()] += 0.1;
    }
    HarmonicLoad::new(re, im).unwrap()
}

// ------------------------------------------------------------------------------------------------------- references

/// `K + K_s (+ K_sigma)` on the stiffness pattern.
fn stiffness(model: &SolidModel, prestress: Option<&Prestress>) -> CsrMatrix {
    let mut k = stiffness_with(model, Some(1)).unwrap();
    add_supports(&mut k, model).unwrap();
    if let Some(p) = prestress {
        let kg = geometric_stiffness(model, p, Some(1)).unwrap();
        assert!(kg.row_ptr == k.row_ptr && kg.col == k.col);
        for (a, b) in k.val.iter_mut().zip(&kg.val) {
            *a += b;
        }
    }
    k
}

/// Every eigenpair of the dense pencil `T^T K T, T^T M T` (`K` from [`stiffness`]), each mode the mesh part of `T q`.
fn all_modes(model: &SolidModel, prestress: Option<&Prestress>) -> ModalBasis {
    let tr = transformation(model, &[]).unwrap();
    let k = stiffness(model, prestress);
    let m = consistent_with(model, Some(1)).unwrap();
    let zero = vec![0.0; tr.n_unknowns()];
    let (kr, _) = tr.reduce(&k, &zero).unwrap();
    let (mr, _) = tr.reduce(&m, &zero).unwrap();
    let nq = kr.n;
    let dense = |a: &CsrMatrix| {
        let mut d = vec![0.0; nq * nq];
        for r in 0..a.n {
            for p in a.row_ptr[r]..a.row_ptr[r + 1] {
                d[r * nq + a.col[p]] = a.val[p];
            }
        }
        d
    };
    let eig = generalized_eigen_dense(&dense(&kr), &dense(&mr), nq).unwrap();
    let n = tr.n_mesh();
    let modes = eig.vectors.iter().map(|q| tr.expand(q)[..n].to_vec()).collect();
    println!("dense basis: {nq} modes, lambda_1 {:e}", eig.values[0]);
    ModalBasis::new(eig.values, modes, model.dof_map()).unwrap()
}

fn modal(model: &SolidModel, basis: ModalBasis, damping: ModalDamping) -> HarmonicSolver {
    HarmonicSolver::new(model, basis, &HarmonicOptions::new(damping).static_correction(false)).unwrap()
}

/// `max_j |X_j - Y_j| / max_j |Y_j|` over every equation.
fn e_same(x: &HarmonicResponse, y: &HarmonicResponse) -> f64 {
    let mut num = 0.0f64;
    let mut den = 0.0f64;
    for j in 0..y.re.len() {
        let d = C64::new(x.re[j] - y.re[j], x.im[j] - y.im[j]).abs();
        num = num.max(d);
        den = den.max(C64::new(y.re[j], y.im[j]).abs());
    }
    num / den
}

/// `e` of a split model's response against the merged model's, over every node and component of the split model.
fn e_split(
    split: &SolidModel,
    map: &[usize],
    xs: &HarmonicResponse,
    merged: &SolidModel,
    xm: &HarmonicResponse,
) -> f64 {
    let (ds, dm) = (split.dof_map(), merged.dof_map());
    let mut num = 0.0f64;
    for (a, &b) in map.iter().enumerate() {
        for c in 0..3 {
            let (i, j) = (ds.equation(a, c).unwrap(), dm.equation(b, c).unwrap());
            num = num.max(C64::new(xs.re[i] - xm.re[j], xs.im[i] - xm.im[j]).abs());
        }
    }
    let den = (0..xm.re.len())
        .map(|j| C64::new(xm.re[j], xm.im[j]).abs())
        .fold(0.0, f64::max);
    num / den
}

fn bits(r: &HarmonicResponse) -> Vec<(u64, u64)> {
    r.re.iter()
        .zip(&r.im)
        .map(|(a, b)| (a.to_bits(), b.to_bits()))
        .collect()
}

fn rs2_options(model: &SolidModel, wc: f64) -> DirectHarmonicOptions {
    DirectHarmonicOptions::new()
        .loss_factor("all", LossFactor::Constant(ETA))
        .rayleigh(0.02 * wc, 0.02 / wc)
        .dashpot(Dashpot::ground(junction_node(model), 1, 1e3))
}

// ------------------------------------------------------------------------------------------------------------ checks

#[test]
fn k1_fd1_n2_spring_and_foundation_against_the_chain() {
    let mut c = Checks::new();
    let es = C64::new(E_MOD, E_MOD * ETA);
    let kt = 4.0 * K_NODE;
    let model = sprung_bar();
    let load = bar_tip_load(&model);
    let lossy = DirectHarmonic::new(
        &model,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA)),
    )
    .unwrap();
    for (tag, f) in [("a", 0.5), ("b", 1.0), ("c", 1.5)] {
        let w = f * w1();
        let r = lossy.response(&load, w).unwrap();
        c.at_most(
            &format!("K1{tag} loss 0.02 at {f} w1"),
            e_chain(&model, &r, &chain(es, w, kt, 0.0)),
            1e-8,
        );
    }
    let plain = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    for (tag, f) in [("d", 0.5), ("e", 1.5), ("f", 2.5)] {
        let w = f * w1();
        let r = plain.response(&load, w).unwrap();
        let x = chain(C64::from_real(E_MOD), w, kt, 0.0);
        c.at_most(&format!("K1{tag} undamped at {f} w1"), e_chain(&model, &r, &x), 1e-8);
    }
    let cdash = 0.1 * A * (E_MOD * RHO).sqrt();
    let mut o = DirectHarmonicOptions::new();
    for &a in model.mesh().node_set("xmax").unwrap() {
        o = o.dashpot(Dashpot::ground(a, 0, cdash / 4.0));
    }
    let dashed = DirectHarmonic::new(&model, &o).unwrap();
    let r = dashed.response(&load, w1()).unwrap();
    let x = chain(C64::from_real(E_MOD), w1(), kt, cdash);
    c.at_most("K1g spring and dashpot at 1.0 w1", e_chain(&model, &r, &x), 1e-8);

    let mut founded = bar_on(bar_mesh());
    founded
        .add_support(ElasticSupport::foundation("xmax", E_MOD / L))
        .unwrap();
    let fd = DirectHarmonic::new(
        &founded,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA)),
    )
    .unwrap();
    let r = fd.response(&bar_tip_load(&founded), w1()).unwrap();
    c.at_most(
        "FD1 foundation at 1.0 w1",
        e_chain(&founded, &r, &chain(es, w1(), kt, 0.0)),
        1e-8,
    );

    // N2: the spring matters
    let free_tip = bar_on(bar_mesh());
    let s0 = DirectHarmonic::new(
        &free_tip,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA)),
    )
    .unwrap();
    let r0 = s0.response(&bar_tip_load(&free_tip), 0.5 * w1()).unwrap();
    let r1 = lossy.response(&load, 0.5 * w1()).unwrap();
    c.at_least("N2 spring against no spring at 0.5 w1", e_same(&r0, &r1), 1e-2);
    c.finish();
}

#[test]
fn rs1_rigid_set_bar_against_the_merged_bar() {
    let mut c = Checks::new();
    let merged_bar = bar_on(bar_mesh());
    let (mesh, map) = split(&bar_mesh(), 0.5);
    let mut split_bar = bar_on(mesh);
    split_bar
        .add_constraint(Constraint::rigid_set("junction", [0.5, B / 2.0, B / 2.0]))
        .unwrap();
    let o = DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA));
    let sm = DirectHarmonic::new(&merged_bar, &o).unwrap();
    let ss = DirectHarmonic::new(&split_bar, &o).unwrap();
    for (tag, f) in [("a", 0.5), ("b", 1.0), ("c", 1.5)] {
        let w = f * w1();
        let xm = sm.response(&bar_tip_load(&merged_bar), w).unwrap();
        let xs = ss.response(&bar_tip_load(&split_bar), w).unwrap();
        c.at_most(
            &format!("RS1{tag} split bar with the rigid set at {f} w1"),
            e_split(&split_bar, &map, &xs, &merged_bar, &xm),
            1e-8,
        );
    }
    c.finish();
}

#[test]
fn rs2_n3_ry1_rigid_set_cantilever_and_rayleigh() {
    let mut c = Checks::new();
    let mm = merged();
    let basis = all_modes(&mm, None);
    let wc = basis.eigenvalues()[0].sqrt();
    println!("RS2 w_1 = {wc:e} rad/s");
    let (ms, map) = split_cantilever();
    let sm = DirectHarmonic::new(&mm, &rs2_options(&mm, wc)).unwrap();
    let ss = DirectHarmonic::new(&ms, &rs2_options(&ms, wc)).unwrap();
    let (lm, ls) = (c_load(&mm, true), c_load(&ms, true));
    for (tag, f) in [("a", 0.5), ("b", 1.7), ("c", 2.5)] {
        let w = f * wc;
        let xm = sm.response(&lm, w).unwrap();
        let xs = ss.response(&ls, w).unwrap();
        c.at_most(
            &format!("RS2{tag} split cantilever with the rigid set at {f} w_1"),
            e_split(&ms, &map, &xs, &mm, &xm),
            1e-8,
        );
    }
    // N3: the merged model without the rigid set
    let mut loose = cantilever_on(with_junction(c_mesh(), CUT));
    loose.add_support(ElasticSupport::nodal_spring("xmax", 2, 1e6)).unwrap();
    let sl = DirectHarmonic::new(&loose, &rs2_options(&loose, wc)).unwrap();
    let xl = sl.response(&c_load(&loose, true), 1.7 * wc).unwrap();
    let xs = ss.response(&ls, 1.7 * wc).unwrap();
    c.at_least(
        "N3 rigid set against none at 1.7 w_1",
        e_split(&ms, &map, &xs, &loose, &xl),
        1e-5,
    );

    // RY1: Rayleigh on the transformation path against F-399 with every mode
    let (alpha, beta) = (0.02 * wc, 0.02 / wc);
    let direct = DirectHarmonic::new(&mm, &DirectHarmonicOptions::new().rayleigh(alpha, beta)).unwrap();
    let ms_solver = modal(
        &mm,
        basis,
        ModalDamping::Rayleigh {
            mass: alpha,
            stiffness: beta,
        },
    );
    let load = c_load(&mm, false);
    for (tag, f) in [("a", 0.5), ("b", 1.0), ("c", 1.7), ("d", 2.5)] {
        let w = f * wc;
        let xd = direct.response(&load, w).unwrap();
        let xm = ms_solver.response(&load, w).unwrap();
        c.at_most(&format!("RY1{tag} Rayleigh at {f} w_1"), e_same(&xd, &xm), 1e-8);
    }
    c.finish();
}

#[test]
fn p1_p3_z1_n1_prestress() {
    let mut c = Checks::new();
    let model = cantilever();
    let p = Prestress::uniform(&model, [SIGMA, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let basis = all_modes(&model, Some(&p));
    let wp = basis.eigenvalues()[0].sqrt();
    println!("P1 w_1 = {wp:e} rad/s");
    let load = c_load(&model, false);
    let direct = DirectHarmonic::new(&model, &DirectHarmonicOptions::new().prestress(p.clone())).unwrap();
    let elastic = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let ms = modal(&model, basis.clone(), ModalDamping::Ratio(0.0));
    for (tag, f) in [("a", 0.5), ("b", 1.7), ("c", 2.5)] {
        let w = f * wp;
        let xd = direct.response(&load, w).unwrap();
        let xm = ms.response(&load, w).unwrap();
        c.at_most(
            &format!("P1{tag} pre-stressed undamped at {f} w_1"),
            e_same(&xd, &xm),
            1e-8,
        );
        let xe = elastic.response(&load, w).unwrap();
        c.at_least(
            &format!("N1{tag} pre-stress against none at {f} w_1"),
            e_same(&xe, &xd),
            1e-2,
        );
    }
    // P3: Rayleigh on K + K_sigma
    let (alpha, beta) = (0.02 * wp, 0.02 / wp);
    let dr = DirectHarmonic::new(
        &model,
        &DirectHarmonicOptions::new().prestress(p.clone()).rayleigh(alpha, beta),
    )
    .unwrap();
    let mr = modal(
        &model,
        basis,
        ModalDamping::Rayleigh {
            mass: alpha,
            stiffness: beta,
        },
    );
    for (tag, f) in [("a", 1.0), ("b", 2.5)] {
        let w = f * wp;
        c.at_most(
            &format!("P3{tag} pre-stressed Rayleigh at {f} w_1"),
            e_same(&dr.response(&load, w).unwrap(), &mr.response(&load, w).unwrap()),
            1e-8,
        );
    }
    // Z1: a zero pre-stress
    let z = Prestress::uniform(&model, [0.0; 6]).unwrap();
    let dz = DirectHarmonic::new(&model, &DirectHarmonicOptions::new().prestress(z)).unwrap();
    let w = 0.5 * wp;
    c.at_most(
        "Z1 zero pre-stress against none",
        e_same(&dz.response(&load, w).unwrap(), &elastic.response(&load, w).unwrap()),
        1e-14,
    );
    c.finish();
}

#[test]
fn p2_prestress_on_the_transformation_path() {
    let mut c = Checks::new();
    let model = merged();
    let p = Prestress::uniform(&model, [SIGMA, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let basis = all_modes(&model, Some(&p));
    let wp = basis.eigenvalues()[0].sqrt();
    println!("P2 w_1 = {wp:e} rad/s");
    let load = c_load(&model, false);
    let direct = DirectHarmonic::new(&model, &DirectHarmonicOptions::new().prestress(p)).unwrap();
    let ms = modal(&model, basis, ModalDamping::Ratio(0.0));
    for (tag, f) in [("a", 0.5), ("b", 1.7), ("c", 2.5)] {
        let w = f * wp;
        let xd = direct.response(&load, w).unwrap();
        let xm = ms.response(&load, w).unwrap();
        c.at_most(
            &format!("P2{tag} pre-stressed, rigid set and spring, at {f} w_1"),
            e_same(&xd, &xm),
            1e-8,
        );
    }
    c.finish();
}

#[test]
fn t1_threads_give_the_same_bits() {
    let mut c = Checks::new();
    // K1b
    let model = sprung_bar();
    let o = DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA));
    let r1 = DirectHarmonic::new(&model, &o.clone().threads(1))
        .unwrap()
        .response(&bar_tip_load(&model), w1())
        .unwrap();
    let r8 = DirectHarmonic::new(&model, &o.threads(8))
        .unwrap()
        .response(&bar_tip_load(&model), w1())
        .unwrap();
    let d = bits(&r1).iter().zip(&bits(&r8)).filter(|(a, b)| a != b).count();
    c.holds("T1 K1b threads 1 and 8", d == 0, &format!("{d} values differ"));
    // RS2b
    let (ms, _) = split_cantilever();
    let wc = all_modes(&merged(), None).eigenvalues()[0].sqrt();
    let l = c_load(&ms, true);
    let r1 = DirectHarmonic::new(&ms, &rs2_options(&ms, wc).threads(1))
        .unwrap()
        .response(&l, 1.7 * wc)
        .unwrap();
    let r8 = DirectHarmonic::new(&ms, &rs2_options(&ms, wc).threads(8))
        .unwrap()
        .response(&l, 1.7 * wc)
        .unwrap();
    let d = bits(&r1).iter().zip(&bits(&r8)).filter(|(a, b)| a != b).count();
    c.holds("T1 RS2b threads 1 and 8", d == 0, &format!("{d} values differ"));
    // P2b
    let mm = merged();
    let p = Prestress::uniform(&mm, [SIGMA, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let wp = all_modes(&mm, Some(&p)).eigenvalues()[0].sqrt();
    let l = c_load(&mm, false);
    let o = DirectHarmonicOptions::new().prestress(p);
    let r1 = DirectHarmonic::new(&mm, &o.clone().threads(1))
        .unwrap()
        .response(&l, 1.7 * wp)
        .unwrap();
    let r8 = DirectHarmonic::new(&mm, &o.threads(8))
        .unwrap()
        .response(&l, 1.7 * wp)
        .unwrap();
    let d = bits(&r1).iter().zip(&bits(&r8)).filter(|(a, b)| a != b).count();
    c.holds("T1 P2b threads 1 and 8", d == 0, &format!("{d} values differ"));
    c.finish();
}

#[test]
fn rt1_to_rt5_refusals_of_the_transformation_path() {
    let mut c = Checks::new();
    let mm = merged();
    let s = DirectHarmonic::new(&mm, &DirectHarmonicOptions::new()).unwrap();
    let mut base = BaseMotion::new();
    base.add(mm.mesh().node_set("xmin").unwrap()[0], 0, C64::ONE);
    refused(
        &mut c,
        "RT1 base motion on the transformation path",
        s.response_with_base(&c_load(&mm, false), &base, 1e3),
        "base motion",
        "transformation path",
    );
    refused(
        &mut c,
        "RT2 supernodal factor on the transformation path",
        DirectHarmonic::new(
            &mm,
            &DirectHarmonicOptions::new().factor(DirectFactor::SupernodalNestedDissection),
        ),
        "solver",
        "transformation path",
    );
    let mut cyc = cantilever();
    cyc.add_constraint(Constraint::cyclic("ymin", "ymax", [1.0, 0.0, 0.0], [0.0, 0.0, 0.0], 4))
        .unwrap();
    refused(
        &mut c,
        "RT3 cyclic constraint",
        DirectHarmonic::new(&cyc, &DirectHarmonicOptions::new()),
        "constraint",
        "Cyclic",
    );
    let mut lin = cantilever();
    let node = lin.mesh().node_set("xmax").unwrap()[0];
    lin.add_constraint(Constraint::linear(vec![LinearTerm::new(node, 1, 1.0)], 1e-3))
        .unwrap();
    refused(
        &mut c,
        "RT4 non-zero linear value",
        DirectHarmonic::new(&lin, &DirectHarmonicOptions::new()),
        "constraint",
        "takes zero values only",
    );
    let other = Prestress::uniform(&bar_with(10), [SIGMA, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let r = DirectHarmonic::new(&cantilever(), &DirectHarmonicOptions::new().prestress(other));
    c.holds(
        "RT5 a pre-stress of another model",
        r.is_err(),
        &match r {
            Ok(_) => "accepted".to_string(),
            Err(e) => e.to_string(),
        },
    );
    c.finish();
}

// ADR 0543 D7: the rows of `r1_to_r11_refusals_amended` (ADR 0448 amendment 2), R9 superseded by R9'.
#[test]
fn r1_to_r11_refusals_f528() {
    let mut c = Checks::new();
    let model = bar_with(10);
    let load = bar_tip_load(&model);
    refused(
        &mut c,
        "R1 negative loss factor",
        DirectHarmonic::new(
            &model,
            &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(-0.1)),
        ),
        "loss factor",
        "'all'",
    );
    refused(
        &mut c,
        "R2 loss factor on a set without material",
        DirectHarmonic::new(
            &model,
            &DirectHarmonicOptions::new().loss_factor("nosuch", LossFactor::Constant(0.1)),
        ),
        "loss factor",
        "nosuch",
    );
    refused(
        &mut c,
        "R3 table not ascending",
        ModulusTable::new(vec![10.0, 5.0], vec![E_MOD, E_MOD], vec![0.0, 0.0]),
        "modulus table",
        "5e0",
    );
    let short = ModulusTable::new(vec![0.0, 100.0], vec![E_MOD, E_MOD], vec![0.0, 0.0]).unwrap();
    let st = DirectHarmonic::new(
        &model,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Table(short)),
    )
    .unwrap();
    let w = w1();
    refused(
        &mut c,
        "R4 frequency outside the table",
        st.response(&load, w),
        "modulus table",
        &format!("{:e}", w / (2.0 * PI)),
    );
    refused(
        &mut c,
        "R5 dashpot on a missing node",
        DirectHarmonic::new(
            &model,
            &DirectHarmonicOptions::new().dashpot(Dashpot::ground(99_999, 0, 1.0)),
        ),
        "dashpot",
        "dashpot 0",
    );
    refused(
        &mut c,
        "R6 negative dashpot",
        DirectHarmonic::new(
            &model,
            &DirectHarmonicOptions::new().dashpot(Dashpot::ground(0, 0, -1.0)),
        ),
        "dashpot",
        "dashpot 0",
    );
    let plain = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let tip_node = model.mesh().node_set("xmax").unwrap()[0];
    let mut free_base = BaseMotion::new();
    free_base.add(tip_node, 0, C64::ONE);
    refused(
        &mut c,
        "R7 base motion on a free equation",
        plain.response_with_base(&load, &free_base, 0.5 * w1()),
        "base motion",
        &format!("node {tip_node} component 0"),
    );
    let mut moved = bar_with(10);
    moved.add_constraint(Constraint::displacement("xmax", 0, 1e-3)).unwrap();
    refused(
        &mut c,
        "R8b non-zero prescribed value (amendment 2)",
        DirectHarmonic::new(&moved, &DirectHarmonicOptions::new()),
        "constraint",
        "prescribed value",
    );
    let mut sprung = bar_with(10);
    sprung
        .add_support(ElasticSupport::nodal_spring("xmax", 0, 1e6))
        .unwrap();
    let ok = DirectHarmonic::new(&sprung, &DirectHarmonicOptions::new())
        .and_then(|s| s.response(&bar_tip_load(&sprung), 0.5 * w1()));
    match ok {
        Ok(r) => {
            let finite = r.re.iter().chain(&r.im).all(|v| v.is_finite());
            c.holds("R9' elastic support accepted (ADR 0543)", finite, "accepted, finite");
        }
        Err(e) => c.holds("R9' elastic support accepted (ADR 0543)", false, &e.to_string()),
    }
    refused(
        &mut c,
        "R10 negative frequency",
        plain.response(&load, -1.0),
        "frequency",
        "-1e0",
    );
    refused(
        &mut c,
        "R10 non-finite frequency",
        plain.response(&load, f64::NAN),
        "frequency",
        "NaN",
    );
    refused(
        &mut c,
        "R11 load of the wrong length",
        plain.response(&HarmonicLoad::real(vec![0.0; 3]).unwrap(), 1.0),
        "harmonic load",
        "length 3",
    );
    c.finish();
}

// ADR 0543 amendment 1: RY2, a load on the dependent junction node against F-399 with every mode.
#[test]
fn ry2_load_on_a_dependent_unknown() {
    let mut c = Checks::new();
    let mm = merged();
    let basis = all_modes(&mm, None);
    let wc = basis.eigenvalues()[0].sqrt();
    let (alpha, beta) = (0.02 * wc, 0.02 / wc);
    let direct = DirectHarmonic::new(&mm, &DirectHarmonicOptions::new().rayleigh(alpha, beta)).unwrap();
    let ms = modal(
        &mm,
        basis,
        ModalDamping::Rayleigh {
            mass: alpha,
            stiffness: beta,
        },
    );
    let load = c_load(&mm, true);
    for (tag, f) in [("a", 0.5), ("b", 1.7)] {
        let w = f * wc;
        let xd = direct.response(&load, w).unwrap();
        let xm = ms.response(&load, w).unwrap();
        c.at_most(
            &format!("RY2{tag} load on J, Rayleigh at {f} w_1"),
            e_same(&xd, &xm),
            1e-8,
        );
    }
    c.finish();
}
