//! F-708 (ADR 0636): the linear-stress coefficient of the stabilised incompatible-modes Hex8 (rule L of ADR 0630,
//! `LinearStress`) under a load, against the F-bar Hex8 of ADR 0506 on the same and on a refined mesh: the confined punch block
//! at `nu = 0.49` (PB1, PB2) and the displacement-controlled cantilever (PC1, PC2), the Full Hex8 locking control N1, the
//! stability of the loaded state SL, and the heavy record G5 of the ADR 0628 sweep for rule L. Every aggregated value is
//! required finite before it is compared; a failed solve is a non-finite measure. The helpers are copies of those of
//! `si_eas_hg3.rs` (existing tests are not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, BOX_FACES};
use rcfd_fem::element::element_for;
use rcfd_fem::material::{HyperelasticEnergy, PointContext};
use rcfd_fem::nl_incompatible::IncompatibleStabilisation;
use rcfd_fem::nonlinear::{initial_states, solve, tangent, NewtonOptions, NonlinearResult};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{generalized_eigen_dense, symmetric_eigen};
use std::collections::BTreeSet;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A value that is
/// not finite misses.
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
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

/// The smallest of finite values; NaN if any is not finite.
fn least<I: IntoIterator<Item = f64>>(values: I) -> f64 {
    let mut w = f64::INFINITY;
    for v in values {
        if !v.is_finite() {
            return f64::NAN;
        }
        w = w.min(v);
    }
    w
}

/// SplitMix64 from a seed.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed)
    }
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

fn rebuild(mesh: &SolidMesh, nodes: Vec<Vec3>) -> SolidMesh {
    let mut out = SolidMesh::new(nodes).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).unwrap().to_vec())
            .unwrap();
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    out
}

fn with_formulation(mut mesh: SolidMesh, f: SolidFormulation) -> SolidMesh {
    if f != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(f)).unwrap();
    }
    mesh
}

fn element_coords(mesh: &SolidMesh, e: usize) -> Vec<Vec3> {
    mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect()
}

fn boundary_nodes(mesh: &SolidMesh) -> BTreeSet<usize> {
    let mut s = BTreeSet::new();
    for f in BOX_FACES {
        s.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    s
}

// ============================================================================ the material and the elements

const MU: f64 = 1.0e6;
const NU: f64 = 0.49;
const HG_SEED: u64 = 0xF685_0614;
const INC: SolidFormulation = SolidFormulation::Incompatible;
const FULL: SolidFormulation = SolidFormulation::Full;
const FBAR: SolidFormulation = SolidFormulation::BBar;
const OFF: IncompatibleStabilisation = IncompatibleStabilisation::Off;
const LIN: IncompatibleStabilisation = IncompatibleStabilisation::LinearStress;
const REST: IncompatibleStabilisation = IncompatibleStabilisation::RestStress;

fn kappa_of(nu: f64) -> f64 {
    2.0 * MU * (1.0 + nu) / (3.0 * (1.0 - 2.0 * nu))
}

fn material(nu: f64) -> Material {
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, kappa_of(nu)).unwrap(), 1000.0).unwrap()
}

/// An element of the comparison (ADR 0636 D2).
#[derive(Clone, Copy)]
struct Kind {
    name: &'static str,
    formulation: SolidFormulation,
    rule: IncompatibleStabilisation,
}

const K_FBAR: Kind = Kind {
    name: "Fbar",
    formulation: FBAR,
    rule: OFF,
};
const K_L: Kind = Kind {
    name: "L",
    formulation: INC,
    rule: LIN,
};
const K_FULL: Kind = Kind {
    name: "Full",
    formulation: FULL,
    rule: OFF,
};
const K_R: Kind = Kind {
    name: "R",
    formulation: INC,
    rule: REST,
};
const K_INC: Kind = Kind {
    name: "Inc",
    formulation: INC,
    rule: OFF,
};

fn model_of(mesh: SolidMesh, kind: Kind) -> SolidModel {
    let mut m = SolidModel::new(with_formulation(mesh, kind.formulation));
    m.set_incompatible_stabilisation(kind.rule);
    m.add_material("all", material(NU)).unwrap();
    m
}

fn newton() -> NewtonOptions {
    NewtonOptions::new(10).threads(4)
}

/// The nodes of a set whose coordinates pass a filter.
fn nodes_where(mesh: &SolidMesh, set: &str, keep: impl Fn(&Vec3) -> bool) -> Vec<usize> {
    mesh.node_set(set)
        .unwrap()
        .iter()
        .copied()
        .filter(|&n| keep(&mesh.nodes()[n]))
        .collect()
}

/// The sum of the driver's reactions over a node set's component.
fn reaction_sum(r: &NonlinearResult, nodes: &[usize], comp: usize) -> f64 {
    let re = r.result.reactions().unwrap();
    let dofs = r.result.dofs();
    nodes.iter().map(|&n| re[dofs.equation(n, comp).unwrap()]).sum()
}

/// SL: the lowest eigenvalue of the free block of the tangent at the converged state over its largest diagonal entry.
fn lowest_free(m: &SolidModel, r: &NonlinearResult, prescribed: &BTreeSet<usize>) -> f64 {
    let u = r.result.displacement_values();
    let k = tangent(m, u, &r.states, Some(4)).unwrap();
    let free: Vec<usize> = (0..k.n).filter(|e| !prescribed.contains(e)).collect();
    let nf = free.len();
    let mut pos = vec![usize::MAX; k.n];
    for (i, &e) in free.iter().enumerate() {
        pos[e] = i;
    }
    let mut a = vec![0.0; nf * nf];
    for (row, &e) in free.iter().enumerate() {
        for p in k.row_ptr[e]..k.row_ptr[e + 1] {
            let c = pos[k.col[p]];
            if c != usize::MAX {
                a[row * nf + c] = k.val[p];
            }
        }
    }
    let dmax = (0..nf).map(|i| a[i * nf + i].abs()).fold(0.0, f64::max);
    let ev = symmetric_eigen(&a, nf).unwrap();
    least(ev.values.iter().copied()) / dmax
}

// ============================================================================ PB: the confined punch block (reading c1)

const DELTA: f64 = 0.1;

/// The block of ADR 0636 reading c1 on `n^3` (unit cube, rollers on every side and the bottom, the punch `x, y <= 1/2` on
/// the top moved by `-DELTA`); returns the model and the prescribed equations.
fn block(n: usize, kind: Kind) -> (SolidModel, BTreeSet<usize>) {
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    let tol = 1e-9;
    let punch = nodes_where(&mesh, "zmax", |p| p[0] <= 0.5 + tol && p[1] <= 0.5 + tol);
    let corner = nodes_where(&mesh, "zmax", |p| (p[0] - 1.0).abs() < tol && (p[1] - 1.0).abs() < tol);
    mesh.add_node_set("punch", punch).unwrap();
    mesh.add_node_set("corner", corner).unwrap();
    let mut m = model_of(mesh, kind);
    let bcs: [(&str, usize, f64); 6] = [
        ("xmin", 0, 0.0),
        ("xmax", 0, 0.0),
        ("ymin", 1, 0.0),
        ("ymax", 1, 0.0),
        ("zmin", 2, 0.0),
        ("punch", 2, -DELTA),
    ];
    let mut prescribed = BTreeSet::new();
    for (set, comp, value) in bcs {
        m.add_constraint(Constraint::displacement(set, comp, value)).unwrap();
        for &nd in m.mesh().node_set(set).unwrap() {
            prescribed.insert(m.dof_map().equation(nd, comp).unwrap());
        }
    }
    (m, prescribed)
}

/// `(R, w_c, increments, SL)` of a block run; NaN when the solve fails. SL only when asked.
fn run_block(n: usize, kind: Kind, with_sl: bool) -> (f64, f64, usize, f64) {
    let (m, prescribed) = block(n, kind);
    match solve(&m, &[], &[], &newton()) {
        Ok(r) => {
            let punch = m.mesh().node_set("punch").unwrap().to_vec();
            let corner = m.mesh().node_set("corner").unwrap()[0];
            let rz = -reaction_sum(&r, &punch, 2) / MU;
            let wc = r.result.displacement(corner)[2];
            let sl = if with_sl {
                lowest_free(&m, &r, &prescribed)
            } else {
                f64::NAN
            };
            println!(
                "PB {n}^3 {}: R/mu {rz:.8}, w_c {wc:+.8}, increments {}, SL {sl:.4e}",
                kind.name,
                r.steps.len()
            );
            (rz, wc, r.steps.len(), sl)
        }
        Err(e) => {
            println!("PB {n}^3 {}: solve failed: {e}", kind.name);
            (f64::NAN, f64::NAN, 0, f64::NAN)
        }
    }
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

#[test]
fn pb_punch_block_against_fbar() {
    let mut ch = Checks::new();
    let mut fine = (0.0, 0.0);
    let mut coarse_l = 0.0;
    for (n, tol, nlim, id) in [(4, 0.15, 0.15, "PB1"), (8, 0.10, 0.10, "PB2")] {
        let (rf, wf, _, _) = run_block(n, K_FBAR, false);
        let (rl, wl, _, sl) = run_block(n, K_L, n == 4);
        let (rfu, wfu, _, _) = run_block(n, K_FULL, false);
        ch.at_most(&format!("{id} block {n}^3: e_R = |R_L / R_Fbar - 1|"), rel(rl, rf), tol);
        ch.at_most(&format!("{id} block {n}^3: e_w = |w_L / w_Fbar - 1|"), rel(wl, wf), tol);
        ch.at_least(&format!("N1 block {n}^3: |R_Full / R_Fbar - 1|"), rel(rfu, rf), nlim);
        println!("N2 block {n}^3: |w_Full / w_Fbar - 1| = {:.4e} (printed)", rel(wfu, wf));
        if n == 4 {
            ch.above(
                "SL block 4^3: lowest free tangent eigenvalue / max diagonal, rule L",
                sl,
                0.0,
            );
            coarse_l = rl;
            for kind in [K_R, K_INC] {
                let (rx, wx, _, _) = run_block(n, kind, false);
                println!(
                    "report block 4^3 {}: e_R {:.4e}, e_w {:.4e}",
                    kind.name,
                    rel(rx, rf),
                    rel(wx, wf)
                );
            }
        } else {
            fine = (rf, wf);
            let (rx, wx, _, _) = run_block(n, K_INC, false);
            println!("report block 8^3 Inc: e_R {:.4e}, e_w {:.4e}", rel(rx, rf), rel(wx, wf));
        }
    }
    println!(
        "X1 block: R_L(4^3) / R_Fbar(8^3) - 1 = {:+.4e} (report)",
        coarse_l / fine.0 - 1.0
    );
    ch.finish();
}

// ============================================================================ PC: the cantilever (reading c2)

const CANT_L: f64 = 1.0;
const CANT_H: f64 = 0.2;
const TIP_V: f64 = 0.3;

fn cantilever(nx: usize, ny: usize, kind: Kind) -> (SolidModel, BTreeSet<usize>) {
    let mesh = hex_box([0.0; 3], [CANT_L, CANT_H, CANT_H], [nx, ny, ny]).unwrap();
    let mut m = model_of(mesh, kind);
    let mut bcs: Vec<(&str, usize, f64)> = (0..3).map(|c| ("xmin", c, 0.0)).collect();
    bcs.push(("xmax", 1, TIP_V));
    let mut prescribed = BTreeSet::new();
    for (set, comp, value) in bcs {
        m.add_constraint(Constraint::displacement(set, comp, value)).unwrap();
        for &nd in m.mesh().node_set(set).unwrap() {
            prescribed.insert(m.dof_map().equation(nd, comp).unwrap());
        }
    }
    (m, prescribed)
}

/// `(P, d, increments, SL)` of a cantilever run; NaN when the solve fails.
fn run_cantilever(nx: usize, ny: usize, kind: Kind, with_sl: bool) -> (f64, f64, usize, f64) {
    let (m, prescribed) = cantilever(nx, ny, kind);
    match solve(&m, &[], &[], &newton()) {
        Ok(r) => {
            let tip = m.mesh().node_set("xmax").unwrap().to_vec();
            let p = reaction_sum(&r, &tip, 1) / MU;
            let d = -tip.iter().map(|&n| r.result.displacement(n)[0]).sum::<f64>() / tip.len() as f64;
            let sl = if with_sl {
                lowest_free(&m, &r, &prescribed)
            } else {
                f64::NAN
            };
            println!(
                "PC {nx}x{ny}x{ny} {}: P/mu {p:.8e}, d {d:.8e}, increments {}, SL {sl:.4e}",
                kind.name,
                r.steps.len()
            );
            (p, d, r.steps.len(), sl)
        }
        Err(e) => {
            println!("PC {nx}x{ny}x{ny} {}: solve failed: {e}", kind.name);
            (f64::NAN, f64::NAN, 0, f64::NAN)
        }
    }
}

#[test]
fn pc_cantilever_against_fbar() {
    let mut ch = Checks::new();
    let mut fine = 0.0;
    let mut coarse_l = 0.0;
    for (nx, ny, tol, id) in [(10, 2, 0.15, "PC1"), (20, 4, 0.10, "PC2")] {
        let (pf, df, _, _) = run_cantilever(nx, ny, K_FBAR, false);
        let (pl, dl, _, sl) = run_cantilever(nx, ny, K_L, nx == 10);
        let (pfu, _, _, _) = run_cantilever(nx, ny, K_FULL, false);
        let tag = format!("{id} cantilever {nx}x{ny}x{ny}");
        ch.at_most(&format!("{tag}: e_P = |P_L / P_Fbar - 1|"), rel(pl, pf), tol);
        ch.at_most(&format!("{tag}: e_d = |d_L / d_Fbar - 1|"), rel(dl, df), tol);
        ch.at_least(
            &format!("N1 cantilever {nx}x{ny}x{ny}: |P_Full / P_Fbar - 1|"),
            rel(pfu, pf),
            tol,
        );
        if nx == 10 {
            ch.above(
                "SL cantilever 10x2x2: lowest free tangent eigenvalue / max diagonal, rule L",
                sl,
                0.0,
            );
            coarse_l = pl;
            for kind in [K_R, K_INC] {
                let (px, dx, _, _) = run_cantilever(nx, ny, kind, false);
                println!(
                    "report cantilever 10x2x2 {}: e_P {:.4e}, e_d {:.4e}",
                    kind.name,
                    rel(px, pf),
                    rel(dx, df)
                );
            }
        } else {
            fine = pf;
        }
    }
    println!(
        "X1 cantilever: P_L(10x2x2) / P_Fbar(20x4x4) - 1 = {:+.4e} (report)",
        coarse_l / fine - 1.0
    );
    ch.finish();
}

// ============================================================================ G5: the ADR 0628 sweep of rule L (reading c5)

/// The ADR 0614 c1 patch: the unit cube on `n^3` Hex8, interior nodes moved by `0.1 h (2U - 1)` (seed `0xF685_0614`).
fn patch(n: usize, nu: f64, f: SolidFormulation, stabilised: IncompatibleStabilisation) -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    let boundary = boundary_nodes(&base);
    let mut rng = Rng::new(HG_SEED);
    let h = 1.0 / n as f64;
    let mut nodes = base.nodes().to_vec();
    for (i, p) in nodes.iter_mut().enumerate() {
        if !boundary.contains(&i) {
            for v in p.iter_mut() {
                *v += 0.1 * h * (2.0 * rng.uniform() - 1.0);
            }
        }
    }
    let mut m = SolidModel::new(with_formulation(rebuild(&base, nodes), f));
    m.set_incompatible_stabilisation(stabilised);
    m.add_material("all", material(nu)).unwrap();
    m
}

fn diag_strain(t: f64, lambda: f64) -> [f64; 6] {
    [
        0.5 * (t * t - 1.0),
        0.5 * (t * t - 1.0),
        0.5 * (lambda * lambda - 1.0),
        0.0,
        0.0,
        0.0,
    ]
}

/// The lateral stretch of uniaxial stress: 200 bisections of `S_11` on `[1, 2]`.
fn lateral(nu: f64, lambda: f64) -> f64 {
    let (mut lo, mut hi) = (1.0, 2.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        let s = material(nu)
            .stress(&diag_strain(mid, lambda), &PointContext::default(), &[])
            .unwrap()
            .0;
        if s[0] < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The homogeneous field `u = (F - I) X` at every node, `F = diag(t, t, lambda)`.
fn homogeneous(m: &SolidModel, t: f64, lambda: f64) -> Vec<f64> {
    let st = [t - 1.0, t - 1.0, lambda - 1.0];
    let dofs = m.dof_map();
    let mut u = vec![0.0; dofs.n_equations()];
    for (n, x) in m.mesh().nodes().iter().enumerate() {
        for c in 0..3 {
            u[dofs.equation(n, c).unwrap()] = st[c] * x[c];
        }
    }
    u
}

/// The equations of the interior nodes (node order, three each).
fn interior(m: &SolidModel) -> Vec<usize> {
    let boundary = boundary_nodes(m.mesh());
    let dofs = m.dof_map();
    let mut eqs = Vec::new();
    for node in 0..m.mesh().nodes().len() {
        if !boundary.contains(&node) {
            for c in 0..3 {
                eqs.push(dofs.equation(node, c).unwrap());
            }
        }
    }
    eqs
}

/// The free block (dense, row-major) of a CSR matrix.
fn dense_free(k: &rcfd_linalg::CsrMatrix, free: &[usize]) -> Vec<f64> {
    let nf = free.len();
    let mut pos = vec![usize::MAX; k.n];
    for (i, &e) in free.iter().enumerate() {
        pos[e] = i;
    }
    let mut out = vec![0.0; nf * nf];
    for (r, &e) in free.iter().enumerate() {
        for p in k.row_ptr[e]..k.row_ptr[e + 1] {
            let c = pos[k.col[p]];
            if c != usize::MAX {
                out[r * nf + c] = k.val[p];
            }
        }
    }
    out
}

/// The vector Laplacian of the Full Hex8 (2 x 2 x 2 rule) on the free block.
fn laplacian(m: &SolidModel, free: &[usize]) -> Vec<f64> {
    let nf = free.len();
    let mut pos = vec![usize::MAX; m.dof_map().n_equations()];
    for (i, &e) in free.iter().enumerate() {
        pos[e] = i;
    }
    let el = element_for(ElementKind::Hex8);
    let dofs = m.dof_map();
    let mut out = vec![0.0; nf * nf];
    for e in 0..m.mesh().n_elements() {
        let nodes = m.mesh().element_nodes(e);
        let coords = element_coords(m.mesh(), e);
        for ip in el.integration_points(&coords, el.stiffness_rule()).unwrap() {
            let g = &ip.geometry.grad;
            for (a, &na) in nodes.iter().enumerate() {
                for (b, &nb) in nodes.iter().enumerate() {
                    let v = ip.weight * (g[a][0] * g[b][0] + g[a][1] * g[b][1] + g[a][2] * g[b][2]);
                    for c in 0..3 {
                        let (r, s) = (pos[dofs.equation(na, c).unwrap()], pos[dofs.equation(nb, c).unwrap()]);
                        if r != usize::MAX && s != usize::MAX {
                            out[r * nf + s] += v;
                        }
                    }
                }
            }
        }
    }
    out
}

/// A registered case of ADR 0630 c2.
#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    nu: f64,
    confined: bool,
}

const CASES: [Case; 6] = [
    Case {
        name: "U30",
        nu: 0.3,
        confined: false,
    },
    Case {
        name: "C30",
        nu: 0.3,
        confined: true,
    },
    Case {
        name: "C45",
        nu: 0.45,
        confined: true,
    },
    Case {
        name: "C49",
        nu: 0.49,
        confined: true,
    },
    Case {
        name: "U45",
        nu: 0.45,
        confined: false,
    },
    Case {
        name: "U49",
        nu: 0.49,
        confined: false,
    },
];

#[test]
#[ignore = "heavy record of ADR 0636 (G5: six cases x 51 stretches of dense 375 x 375 pencils, rule L); run once with --ignored; the fast companion is hf3_fast_companion_on_a_4_cube_patch of si_eas_hg3.rs"]
fn g5_sweep_linear_stress_heavy() {
    let mut ch = Checks::new();
    let n = 6;
    for case in CASES {
        let name = case.name;
        let lin = patch(n, case.nu, INC, LIN);
        let free = interior(&lin);
        let lap = laplacian(&lin, &free);
        let mut ml = Vec::new();
        for j in 0..=50 {
            let lambda = 1.0 - 0.01 * j as f64;
            let t = if case.confined { 1.0 } else { lateral(case.nu, lambda) };
            let u = homogeneous(&lin, t, lambda);
            let k = tangent(&lin, &u, &initial_states(&lin).unwrap(), Some(4)).unwrap();
            let e = generalized_eigen_dense(&dense_free(&k, &free), &lap, free.len()).unwrap();
            let m = e.values[0] / MU;
            println!("G5 {name} {lambda:.2} {t:.8} m_L {m:.6}");
            ml.push(m);
        }
        let (lo, at) = ml.iter().enumerate().fold(
            (f64::INFINITY, 0),
            |(m, a), (j, &v)| if v < m { (v, j) } else { (m, a) },
        );
        println!("G5 {name} smallest m_L {lo:.6} at lambda {:.2}", 1.0 - 0.01 * at as f64);
        ch.at_least(&format!("G5 {name} m_L over [0.5, 1]"), least(ml), 0.5);
    }
    ch.finish();
}
