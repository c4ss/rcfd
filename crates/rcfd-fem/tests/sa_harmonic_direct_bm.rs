//! F-619 (ADR 0565): direct harmonic base motion and the reference response on the transformation path. Checks A1 to A3
//! (BM1a, BM3r, RF1), BM1 to BM4, Z1, Z2, T1, F1, the controls N1, N2 and the refusals RB1 to RB6. A4 (B0) is
//! `sa_harmonic_direct_bm_default_bits.rs`.

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::builders::hex_box;
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::harmonic::{HarmonicLoad, HarmonicResponse};
use rcfd_fem::harmonic_direct::{
    BaseMotion, Dashpot, DirectHarmonic, DirectHarmonicOptions, DirectHarmonicSolution, LossFactor,
};
use rcfd_fem::mass::consistent_with;
use rcfd_fem::mpc::{add_supports, transformation};
use rcfd_fem::{Constraint, ElasticSupport, FemError, LinearTerm, Material, SolidMesh, SolidModel};
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
/// The tip spring of each `xmax` node of the bar (four nodes: `EA/L` in all).
const K_NODE: f64 = E_MOD * A / L / 4.0;
/// The uniform pre-stress of BM2 (ADR 0543).
const SIGMA: f64 = 2e8;
/// The foundation of BM2 on `ymin`, N/m^3 (ADR 0565 arithmetic).
const K_F: f64 = 1e9;
const CUT: f64 = 0.3;

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

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap()
}

fn w1() -> f64 {
    PI * (E_MOD / RHO).sqrt() / (2.0 * L)
}

// --------------------------------------------------------------------------------------------------------------- bar

fn bar_mesh() -> SolidMesh {
    hex_box([0.0; 3], [L, B, B], [N, 1, 1]).unwrap()
}

fn bar() -> SolidModel {
    let mut m = SolidModel::new(bar_mesh());
    m.add_material("all", steel(0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

/// `S`: the bar with the tip spring.
fn sprung_bar() -> SolidModel {
    let mut m = bar();
    m.add_support(ElasticSupport::nodal_spring("xmax", 0, K_NODE)).unwrap();
    m
}

fn zero_load(model: &SolidModel) -> HarmonicLoad {
    HarmonicLoad::real(vec![0.0; model.dof_map().n_equations()]).unwrap()
}

fn bar_tip_load(model: &SolidModel) -> HarmonicLoad {
    let dofs = model.dof_map();
    let mut f = vec![0.0; dofs.n_equations()];
    for &a in model.mesh().node_set("xmax").unwrap() {
        f[dofs.equation(a, 0).unwrap()] = 0.25;
    }
    HarmonicLoad::real(f).unwrap()
}

fn xmin_base(model: &SolidModel, component: usize, g: C64) -> BaseMotion {
    let mut b = BaseMotion::new();
    b.add_set(model, "xmin", component, g).unwrap();
    b
}

/// The terms of the 1-D chain of ADR 0565 (arithmetic).
struct Chain {
    /// `E*`: the element stiffness is `E* A / h`.
    es: C64,
    /// The factor of the consistent mass `rho A h / 6 [2 1; 1 2]`: `-omega^2 + i omega alpha`.
    mfac: C64,
    /// The tip spring (complex: `K_t (1 + i omega beta)` with Rayleigh).
    ks: C64,
    /// The extra stiffness of the first element, `i omega c` of the base dashpots.
    c01: C64,
    /// The base value at node 0.
    g0: C64,
    /// The value of node `N` when it is driven too (BM4).
    g_end: Option<C64>,
    /// The tip force.
    f_tip: C64,
}

impl Chain {
    fn new(es: C64, w: f64, ks: f64, g0: C64) -> Chain {
        Chain {
            es,
            mfac: C64::from_real(-w * w),
            ks: C64::from_real(ks),
            c01: C64::ZERO,
            g0,
            g_end: None,
            f_tip: C64::ZERO,
        }
    }

    /// Every nodal value (index 0 the base), by the Thomas algorithm on the free nodes.
    fn solve(&self) -> Vec<C64> {
        let h = L / N as f64;
        let kk = self.es.scale(A / h);
        let mm = self.mfac.scale(RHO * A * h / 6.0);
        let mut diag = vec![C64::ZERO; N + 1];
        let mut off = vec![C64::ZERO; N];
        for e in 0..N {
            let extra = if e == 0 { self.c01 } else { C64::ZERO };
            diag[e] += kk + mm.scale(2.0) + extra;
            diag[e + 1] += kk + mm.scale(2.0) + extra;
            off[e] = -kk + mm - extra;
        }
        diag[N] += self.ks;
        let mut f = vec![C64::ZERO; N + 1];
        f[N] = self.f_tip;
        let mut x = vec![C64::ZERO; N + 1];
        x[0] = self.g0;
        let last = match self.g_end {
            Some(g) => {
                x[N] = g;
                N - 1
            }
            None => N,
        };
        // rows 1..=last; known values move to the right-hand side
        let mut rhs: Vec<C64> = (1..=last).map(|j| f[j]).collect();
        rhs[0] -= off[0] * x[0];
        if last < N {
            let k = rhs.len() - 1;
            rhs[k] -= off[N - 1] * x[N];
        }
        let mut d: Vec<C64> = (1..=last).map(|j| diag[j]).collect();
        let m = d.len();
        for k in 1..m {
            // rows k-1 and k are nodes k and k+1, coupled by off[k]
            let r = off[k] / d[k - 1];
            d[k] -= r * off[k];
            let prev = rhs[k - 1];
            rhs[k] -= r * prev;
        }
        let mut y = vec![C64::ZERO; m];
        y[m - 1] = rhs[m - 1] / d[m - 1];
        for k in (0..m - 1).rev() {
            y[k] = (rhs[k] - off[k + 1] * y[k + 1]) / d[k];
        }
        for (k, v) in y.into_iter().enumerate() {
            x[k + 1] = v;
        }
        x
    }
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

/// `max_j |X_j - Y_j| / max_j |Y_j|` over every equation.
fn e_same(x: &HarmonicResponse, y: &HarmonicResponse) -> f64 {
    e_vec(&x.re, &x.im, &y.re, &y.im)
}

fn e_vec(xr: &[f64], xi: &[f64], yr: &[f64], yi: &[f64]) -> f64 {
    let mut num = 0.0f64;
    let mut den = 0.0f64;
    for j in 0..yr.len() {
        num = num.max(C64::new(xr[j] - yr[j], xi[j] - yi[j]).abs());
        den = den.max(C64::new(yr[j], yi[j]).abs());
    }
    num / den
}

fn bits(r: &HarmonicResponse) -> Vec<(u64, u64)> {
    r.re.iter()
        .zip(&r.im)
        .map(|(a, b)| (a.to_bits(), b.to_bits()))
        .collect()
}

fn solution_bits(s: &DirectHarmonicSolution) -> Vec<(u64, u64)> {
    let mut v = bits(&s.response);
    for r in &s.references {
        v.extend(r.re.iter().zip(&r.im).map(|(a, b)| (a.to_bits(), b.to_bits())));
    }
    v
}

// ------------------------------------------------------------------------------------------------------- cantilever

fn c_mesh() -> SolidMesh {
    let mut mesh = hex_box([0.0; 3], [0.6, 0.1, 0.1], [6, 2, 2]).unwrap();
    let at: Vec<usize> = (0..mesh.nodes().len())
        .filter(|&a| (mesh.nodes()[a][0] - CUT).abs() < 1e-12)
        .collect();
    mesh.add_node_set("junction", at).unwrap();
    mesh
}

/// `C_R`: the merged cantilever of ADR 0543 RS2.
fn merged() -> SolidModel {
    let mut m = SolidModel::new(c_mesh());
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::rigid_set("junction", [CUT, 0.05, 0.05]))
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("xmax", 2, 1e6)).unwrap();
    m
}

/// The model of BM2: `C_R` with the foundation on `ymin`.
fn bm2_model() -> SolidModel {
    let mut m = merged();
    m.add_support(ElasticSupport::foundation("ymin", K_F)).unwrap();
    m
}

fn junction_node(model: &SolidModel) -> usize {
    model
        .mesh()
        .nodes()
        .iter()
        .position(|p| (p[0] - CUT).abs() < 1e-12 && p[1].abs() < 1e-12 && p[2].abs() < 1e-12)
        .unwrap()
}

/// The load of ADR 0543 RS2: the tip load and the load on `J`.
fn c_load(model: &SolidModel) -> HarmonicLoad {
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
    let j = junction_node(model);
    re[dofs.equation(j, 2).unwrap()] += 0.4;
    im[dofs.equation(j, 0).unwrap()] += 0.1;
    HarmonicLoad::new(re, im).unwrap()
}

/// `sqrt(lambda_1)` of the dense pencil `T^T (K + K_s (+ K_sigma)) T, T^T M T`.
fn first_omega(model: &SolidModel, prestress: Option<&Prestress>) -> f64 {
    let tr = transformation(model, &[]).unwrap();
    let mut k = stiffness_with(model, Some(1)).unwrap();
    add_supports(&mut k, model).unwrap();
    if let Some(p) = prestress {
        let kg = geometric_stiffness(model, p, Some(1)).unwrap();
        assert!(kg.row_ptr == k.row_ptr && kg.col == k.col);
        for (a, b) in k.val.iter_mut().zip(&kg.val) {
            *a += b;
        }
    }
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
    eig.values[0].sqrt()
}

fn bm2_prestress(model: &SolidModel) -> Prestress {
    Prestress::uniform(model, [SIGMA, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap()
}

/// The options of BM2 (`wc` its `w_1`).
fn bm2_options(model: &SolidModel, wc: f64) -> DirectHarmonicOptions {
    DirectHarmonicOptions::new()
        .loss_factor("all", LossFactor::Constant(ETA))
        .rayleigh(0.02 * wc, 0.02 / wc)
        .dashpot(Dashpot::ground(junction_node(model), 1, 1e3))
        .prestress(bm2_prestress(model))
}

const BM2_DASHPOT: f64 = 1e3;

fn bm2_g() -> C64 {
    C64::new(1e-7, 5e-8)
}

/// `F - A_ext G_full` of the relative-motion reference (ADR 0565 arithmetic), `G_full` the translation `g` along `y` of
/// every node, `A_ext` formed here from the element matrices.
fn relative_load(model: &SolidModel, load: &HarmonicLoad, wc: f64, w: f64, g: C64) -> HarmonicLoad {
    let k0 = stiffness_with(model, Some(1)).unwrap();
    let mut ks = k0.clone();
    for v in ks.val.iter_mut() {
        *v = 0.0;
    }
    add_supports(&mut ks, model).unwrap();
    let kg = geometric_stiffness(model, &bm2_prestress(model), Some(1)).unwrap();
    let m = consistent_with(model, Some(1)).unwrap();
    for x in [&ks, &kg, &m] {
        assert!(x.row_ptr == k0.row_ptr && x.col == k0.col, "one pattern");
    }
    let (alpha, beta) = (0.02 * wc, 0.02 / wc);
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let mut gf = vec![C64::ZERO; n];
    for a in 0..model.mesh().nodes().len() {
        gf[dofs.equation(a, 1).unwrap()] = g;
    }
    let jd = dofs.equation(junction_node(model), 1).unwrap();
    let (mut re, mut im) = (load.re().to_vec(), load.im().to_vec());
    for i in 0..n {
        let mut s = C64::ZERO;
        for p in k0.row_ptr[i]..k0.row_ptr[i + 1] {
            let j = k0.col[p];
            let kb = k0.val[p] + ks.val[p] + kg.val[p];
            let a = C64::new(1.0, ETA).scale(k0.val[p])
                + C64::from_real(ks.val[p] + kg.val[p] - w * w * m.val[p])
                + C64::new(0.0, w * (alpha * m.val[p] + beta * kb));
            s += a * gf[j];
        }
        if i == jd {
            s += C64::new(0.0, w * BM2_DASHPOT) * gf[jd];
        }
        re[i] -= s.re;
        im[i] -= s.im;
    }
    HarmonicLoad::new(re, im).unwrap()
}

// ------------------------------------------------------------------------------------------------------------ checks

#[test]
fn a1_bm1_n1_base_motion_of_the_sprung_bar_against_the_chain() {
    let mut c = Checks::new();
    let kt = 4.0 * K_NODE;
    let model = sprung_bar();
    let zero = zero_load(&model);
    let g1 = C64::ONE;
    let gc = C64::new(0.6, -0.8);
    let plain = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    for (tag, f) in [("a (A1)", 0.5), ("b", 1.5), ("c", 2.5)] {
        let w = f * w1();
        let r = plain
            .response_with_references(&zero, &xmin_base(&model, 0, g1), w)
            .unwrap();
        let x = Chain::new(C64::from_real(E_MOD), w, kt, g1).solve();
        c.at_most(
            &format!("BM1{tag} H6 with the tip spring, undamped, at {f} w1"),
            e_chain(&model, &r.response, &x),
            1e-8,
        );
    }
    let es = C64::new(E_MOD, E_MOD * ETA);
    let lossy = DirectHarmonic::new(
        &model,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA)),
    )
    .unwrap();
    let load = bar_tip_load(&model);
    let r = lossy
        .response_with_references(&load, &xmin_base(&model, 0, g1), w1())
        .unwrap();
    let mut ch = Chain::new(es, w1(), kt, g1);
    ch.f_tip = C64::ONE;
    c.at_most(
        "BM1d loss 0.02, base and tip load, at 1.0 w1",
        e_chain(&model, &r.response, &ch.solve()),
        1e-8,
    );
    // N1: the base matters
    let r0 = lossy.response(&load, w1()).unwrap();
    c.at_least("N1 BM1d against the tip load alone", e_same(&r0, &r.response), 1e-2);

    let r = lossy
        .response_with_references(&zero, &xmin_base(&model, 0, gc), w1())
        .unwrap();
    c.at_most(
        "BM1e loss 0.02, complex base, at 1.0 w1",
        e_chain(&model, &r.response, &Chain::new(es, w1(), kt, gc).solve()),
        1e-8,
    );

    let w = 1.5 * w1();
    let (alpha, beta) = (0.02 * w1(), 0.02 / w1());
    let ray = DirectHarmonic::new(&model, &DirectHarmonicOptions::new().rayleigh(alpha, beta)).unwrap();
    let r = ray
        .response_with_references(&zero, &xmin_base(&model, 0, gc), w)
        .unwrap();
    let mut ch = Chain::new(C64::new(E_MOD, E_MOD * w * beta), w, kt, gc);
    ch.mfac = C64::new(-w * w, w * alpha);
    ch.ks = C64::new(kt, kt * w * beta);
    c.at_most(
        "BM1f Rayleigh, complex base, at 1.5 w1",
        e_chain(&model, &r.response, &ch.solve()),
        1e-8,
    );

    let cdash = 0.1 * A * (E_MOD * RHO).sqrt();
    let h = L / N as f64;
    let nodes = model.mesh().nodes();
    let mut o = DirectHarmonicOptions::new();
    for &a in model.mesh().node_set("xmin").unwrap() {
        let p = nodes[a];
        let b = (0..nodes.len())
            .find(|&b| {
                (nodes[b][0] - h).abs() < 1e-12
                    && (nodes[b][1] - p[1]).abs() < 1e-12
                    && (nodes[b][2] - p[2]).abs() < 1e-12
            })
            .unwrap();
        o = o.dashpot(Dashpot::between(a, b, 0, cdash / 4.0));
    }
    let dashed = DirectHarmonic::new(&model, &o).unwrap();
    let r = dashed
        .response_with_references(&zero, &xmin_base(&model, 0, g1), w1())
        .unwrap();
    let mut ch = Chain::new(C64::from_real(E_MOD), w1(), kt, g1);
    ch.c01 = C64::new(0.0, w1() * cdash);
    c.at_most(
        "BM1g base dashpots, at 1.0 w1",
        e_chain(&model, &r.response, &ch.solve()),
        1e-8,
    );
    c.finish();
}

#[test]
fn a2_bm3_rigid_set_on_the_base_and_bm4_linear_constraints() {
    let mut c = Checks::new();
    let kt = 4.0 * K_NODE;
    let gc = C64::new(0.6, -0.8);
    let w = 0.5 * w1();
    let mut model = sprung_bar();
    model
        .add_constraint(Constraint::rigid_set("xmin", [0.0, B / 2.0, B / 2.0]))
        .unwrap();
    let rigid = model
        .constraints()
        .iter()
        .position(|k| matches!(k, Constraint::RigidSet { .. }))
        .unwrap();
    let s = DirectHarmonic::new(&model, &DirectHarmonicOptions::new()).unwrap();
    let r = s
        .response_with_references(&zero_load(&model), &xmin_base(&model, 0, gc), w)
        .unwrap();
    let x = Chain::new(C64::from_real(E_MOD), w, kt, gc).solve();
    c.at_most(
        "BM3 rigid set on the base against the chain at 0.5 w1",
        e_chain(&model, &r.response, &x),
        1e-8,
    );
    c.holds(
        "A2 one reference, of the rigid set",
        r.references.len() == 1 && r.references[0].constraint == rigid,
        &format!(
            "{} references, constraint {:?} (rigid set {rigid})",
            r.references.len(),
            r.references.first().map(|q| q.constraint)
        ),
    );
    let expected = [gc, C64::ZERO, C64::ZERO, C64::ZERO, C64::ZERO, C64::ZERO];
    let err = match r.references.first() {
        Some(q) => {
            (0..6)
                .map(|k| (C64::new(q.re[k], q.im[k]) - expected[k]).abs())
                .fold(0.0, f64::max)
                / gc.abs()
        }
        None => f64::INFINITY,
    };
    c.at_most("A2 = BM3r references against t = (G, 0, 0), w = 0", err, 1e-10);
    // RB5: a base that no rigid motion of the set meets
    let mut one = BaseMotion::new();
    one.add(model.mesh().node_set("xmin").unwrap()[0], 0, gc);
    refused(
        &mut c,
        "RB5 base on one node of the rigid set",
        s.response_with_references(&zero_load(&model), &one, w),
        "constraint",
        "contradicts",
    );

    // BM4: both ends driven through linear constraints
    let mut tied = bar();
    let nodes = tied.mesh().nodes().to_vec();
    let xmin = tied.mesh().node_set("xmin").unwrap().to_vec();
    for &t in tied.mesh().node_set("xmax").unwrap().to_vec().iter() {
        let b = *xmin
            .iter()
            .find(|&&b| (nodes[b][1] - nodes[t][1]).abs() < 1e-12 && (nodes[b][2] - nodes[t][2]).abs() < 1e-12)
            .unwrap();
        tied.add_constraint(Constraint::linear(
            vec![LinearTerm::new(t, 0, 1.0), LinearTerm::new(b, 0, -1.0)],
            0.0,
        ))
        .unwrap();
    }
    let s4 = DirectHarmonic::new(
        &tied,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA)),
    )
    .unwrap();
    let r4 = s4
        .response_with_references(&zero_load(&tied), &xmin_base(&tied, 0, gc), w)
        .unwrap();
    let mut ch = Chain::new(C64::new(E_MOD, E_MOD * ETA), w, 0.0, gc);
    ch.g_end = Some(gc);
    c.at_most(
        "BM4 both ends driven through linear constraints at 0.5 w1",
        e_chain(&tied, &r4.response, &ch.solve()),
        1e-8,
    );
    c.finish();
}

#[test]
fn a3_bm2_n2_t1_relative_motion_of_the_cantilever() {
    let mut c = Checks::new();
    let model = bm2_model();
    let p = bm2_prestress(&model);
    let wc = first_omega(&model, Some(&p));
    println!("BM2 w_1 = {wc:e} rad/s");
    let g = bm2_g();
    let s = DirectHarmonic::new(&model, &bm2_options(&model, wc)).unwrap();
    let load = c_load(&model);
    let base = xmin_base(&model, 1, g);
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    for (tag, f) in [("a", 0.5), ("b", 1.7), ("c", 2.5)] {
        let w = f * wc;
        let x = s.response_with_references(&load, &base, w).unwrap();
        let y = s
            .response_with_references(&relative_load(&model, &load, wc, w, g), &BaseMotion::new(), w)
            .unwrap();
        let (mut zr, mut zi) = (y.response.re.clone(), y.response.im.clone());
        for a in 0..model.mesh().nodes().len() {
            let e = dofs.equation(a, 1).unwrap();
            zr[e] += g.re;
            zi[e] += g.im;
        }
        c.at_most(
            &format!("BM2{tag} base on xmin against the relative motion at {f} w_1"),
            e_vec(&x.response.re, &x.response.im, &zr, &zi),
            1e-8,
        );
        let err = if x.references.len() == 1 && y.references.len() == 1 {
            let (mut rr, mut ri) = (y.references[0].re, y.references[0].im);
            rr[1] += g.re;
            ri[1] += g.im;
            e_vec(&x.references[0].re, &x.references[0].im, &rr, &ri)
        } else {
            f64::INFINITY
        };
        c.at_most(&format!("BM2r{tag} references at {f} w_1"), err, 1e-8);
        if tag == "b" {
            c.at_least(
                "N2 BM2b y against y + G_full",
                e_vec(&y.response.re, &y.response.im, &zr, &zi),
                1e-2,
            );
            // A3 = RF1: the kinematics of the rigid set from the returned reference
            let rf = match x.references.first() {
                Some(q) => {
                    let mut worst = 0.0f64;
                    for &a in model.mesh().node_set("junction").unwrap() {
                        let xa = model.mesh().nodes()[a];
                        let d = [xa[0] - CUT, xa[1] - 0.05, xa[2] - 0.05];
                        let residual = |t: &[f64; 6], u: &[f64], k: usize| {
                            let w = [t[3], t[4], t[5]];
                            let wxd = [
                                w[1] * d[2] - w[2] * d[1],
                                w[2] * d[0] - w[0] * d[2],
                                w[0] * d[1] - w[1] * d[0],
                            ];
                            u[dofs.equation(a, k).unwrap()] - t[k] - wxd[k]
                        };
                        for k in 0..3 {
                            let rr = residual(&q.re, &x.response.re, k);
                            let ri = residual(&q.im, &x.response.im, k);
                            worst = worst.max(rr.hypot(ri));
                        }
                    }
                    let scale = (0..n)
                        .map(|j| C64::new(x.response.re[j], x.response.im[j]).abs())
                        .fold(0.0, f64::max);
                    worst / scale
                }
                None => f64::INFINITY,
            };
            c.at_most("A3 = RF1 rigid-set kinematics of the references", rf, 1e-10);
            // T1: threads 1 and 8
            let s1 = DirectHarmonic::new(&model, &bm2_options(&model, wc).threads(1)).unwrap();
            let s8 = DirectHarmonic::new(&model, &bm2_options(&model, wc).threads(8)).unwrap();
            let b1 = solution_bits(&s1.response_with_references(&load, &base, w).unwrap());
            let b8 = solution_bits(&s8.response_with_references(&load, &base, w).unwrap());
            let differ = b1.len() != b8.len() || b1.iter().zip(&b8).any(|(a, b)| a != b);
            c.holds("T1 BM2b threads 1 and 8", !differ, &format!("differ: {differ}"));
        }
    }
    c.finish();
}

#[test]
fn z1_z2_rb_the_default_route_and_the_refusals() {
    let mut c = Checks::new();
    // Z1: the plain path
    let bar_m = bar();
    let s = DirectHarmonic::new(
        &bar_m,
        &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(ETA)),
    )
    .unwrap();
    let base = xmin_base(&bar_m, 0, C64::ONE);
    let w = 0.7 * w1();
    let a = s.response_with_base(&zero_load(&bar_m), &base, w).unwrap();
    let b = s.response_with_references(&zero_load(&bar_m), &base, w).unwrap();
    c.holds(
        "Z1 plain path: bits of response_with_base, no references",
        bits(&a) == bits(&b.response) && b.references.is_empty(),
        &format!("{} references", b.references.len()),
    );
    // Z2: the transformation path without a base
    let mm = merged();
    let wc = first_omega(&mm, None);
    println!("Z2 w_1 = {wc:e} rad/s");
    let o = DirectHarmonicOptions::new()
        .loss_factor("all", LossFactor::Constant(ETA))
        .rayleigh(0.02 * wc, 0.02 / wc)
        .dashpot(Dashpot::ground(junction_node(&mm), 1, 1e3));
    let s = DirectHarmonic::new(&mm, &o).unwrap();
    let a = s.response(&c_load(&mm), 1.7 * wc).unwrap();
    let b = s
        .response_with_references(&c_load(&mm), &BaseMotion::new(), 1.7 * wc)
        .unwrap();
    c.holds(
        "Z2 transformation path, empty base: bits of response, one reference",
        bits(&a) == bits(&b.response) && b.references.len() == 1,
        &format!("{} references", b.references.len()),
    );

    // refusals
    let s = DirectHarmonic::new(&mm, &DirectHarmonicOptions::new()).unwrap();
    let load = c_load(&mm);
    let mut free = BaseMotion::new();
    free.add(mm.mesh().node_set("xmax").unwrap()[0], 0, C64::ONE);
    refused(
        &mut c,
        "RB1 base on a free equation",
        s.response_with_references(&load, &free, 1e3),
        "base motion",
        "prescribed equation",
    );
    let mut dep = BaseMotion::new();
    dep.add(junction_node(&mm), 0, C64::ONE);
    refused(
        &mut c,
        "RB2 base on a rigid-set dependent",
        s.response_with_references(&load, &dep, 1e3),
        "base motion",
        "prescribed equation",
    );
    let x0 = mm.mesh().node_set("xmin").unwrap()[0];
    let mut twice = BaseMotion::new();
    twice.add(x0, 1, C64::ONE);
    twice.add(x0, 1, C64::ONE);
    refused(
        &mut c,
        "RB3 given twice",
        s.response_with_references(&load, &twice, 1e3),
        "base motion",
        "given twice",
    );
    let mut nan = BaseMotion::new();
    nan.add(x0, 1, C64::new(f64::NAN, 0.0));
    refused(
        &mut c,
        "RB4 non-finite amplitude",
        s.response_with_references(&load, &nan, 1e3),
        "base motion",
        "not finite",
    );
    let mut one = BaseMotion::new();
    one.add(x0, 0, C64::ONE);
    refused(
        &mut c,
        "RB6 response_with_base on the transformation path",
        s.response_with_base(&load, &one, 1e3),
        "base motion",
        "transformation path",
    );
    c.finish();
}
