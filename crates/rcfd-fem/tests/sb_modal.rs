//! F-383 (ADR 0284): consistent mass matrix and linear modal analysis. Checks C1 to C4, F1 to F3, B1 to B5, T1, T1b, T2, S1,
//! D1, X1 and the negative controls N1, N2 of the registration (seed 0xF383_0284). T1 is the heavy record (`#[ignore]`); T2
//! covers the same code path on a coarser mesh.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::assemble_with;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::element::element_for;
use rcfd_fem::mass::{consistent, consistent_with, element_mass_with_rule, mass_properties, rigid_motions};
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::quadrature;
use rcfd_fem::{Constraint, FemError, Material, SolidModel};
use rcfd_linalg::eigen::Shift;
use rcfd_linalg::CsrMatrix;
use std::collections::BTreeSet;
use std::f64::consts::PI;

const SEED: u64 = 0xF383_0284;
const KINDS: [ElementKind; 3] = [ElementKind::Tet4, ElementKind::Hex8, ElementKind::Tet10];
const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;

// Mass box MB and free box FB (ADR 0284 set-ups).
const MB_ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const MB_LENGTHS: Vec3 = [1.2, 1.0, 0.8];
const MB_DIV: [usize; 3] = [4, 3, 3];
const FB_ORIGIN: Vec3 = [0.2, 0.1, -0.1];
const FB_LENGTHS: Vec3 = [1.0, 0.6, 0.4];
const FB_DIV: [usize; 3] = [3, 2, 2];

// Bar and cantilever.
const BAR_L: f64 = 1.0;
const BAR_B: f64 = 0.1;
const CANT_L: f64 = 1.0;
const CANT_B: f64 = 0.05;
const BETA_L: [f64; 2] = [1.8751040687, 4.6940911330];
/// Exact Timoshenko corrections, kappa = 5/6 (ADR 0284 D6, arithmetic 3).
const TIMO_C: [f64; 2] = [1.988e-3, 1.3611e-2];

/// SplitMix64, a deterministic source only (ADR 0273 D6).
struct Rng(u64);

impl Rng {
    fn new(tag: u64) -> Rng {
        Rng(SEED.wrapping_add(tag))
    }
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
}

/// A list of measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
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

fn kind_index(kind: ElementKind) -> u64 {
    match kind {
        ElementKind::Tet4 => 0,
        ElementKind::Hex8 => 1,
        _ => 2,
    }
}

/// The same blocks and sets on a new node list.
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

/// A box of the kind (Tet10 from the Tet4 box, straight edges); with `perturb`, the interior grid nodes move by `0.1 h_k
/// (2U - 1)` per coordinate before the upgrade.
fn box_of(kind: ElementKind, origin: Vec3, lengths: Vec3, div: [usize; 3], perturb: Option<u64>) -> SolidMesh {
    let base = match kind {
        ElementKind::Hex8 => hex_box(origin, lengths, div).unwrap(),
        _ => tet_box(origin, lengths, div).unwrap(),
    };
    let base = match perturb {
        None => base,
        Some(tag) => {
            let mut rng = Rng::new(tag);
            let mut boundary = BTreeSet::new();
            for f in BOX_FACES {
                boundary.extend(base.node_set(f).unwrap().iter().copied());
            }
            let h = [
                lengths[0] / div[0] as f64,
                lengths[1] / div[1] as f64,
                lengths[2] / div[2] as f64,
            ];
            let mut nodes = base.nodes().to_vec();
            for (i, p) in nodes.iter_mut().enumerate() {
                if !boundary.contains(&i) {
                    for k in 0..3 {
                        p[k] += 0.1 * h[k] * (2.0 * rng.uniform() - 1.0);
                    }
                }
            }
            rebuild(&base, nodes)
        }
    };
    match kind {
        ElementKind::Tet10 => upgrade_to_tet10(&base).unwrap().0,
        _ => base,
    }
}

fn model_of(mesh: SolidMesh, nu: f64, density: f64) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, nu, density, 0.0).unwrap())
        .unwrap();
    m
}

fn mv(m: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; m.n];
    m.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// `max_ij |phi_i^T M phi_j - delta_ij|` with the full mass matrix.
fn m_orthonormality(m: &CsrMatrix, modes: &[Vec<f64>]) -> f64 {
    let mphi: Vec<Vec<f64>> = modes.iter().map(|p| mv(m, p)).collect();
    let mut worst: f64 = 0.0;
    for (i, p) in modes.iter().enumerate() {
        for (j, mp) in mphi.iter().enumerate() {
            let target = if i == j { 1.0 } else { 0.0 };
            worst = worst.max((dot(p, mp) - target).abs());
        }
    }
    worst
}

fn max_abs(v: impl IntoIterator<Item = f64>) -> f64 {
    v.into_iter().fold(0.0, |a: f64, b| a.max(b.abs()))
}

fn print_run(name: &str, r: &ModalResult) {
    println!(
        "{name}: unknowns {}, iterations {}, shift {:e}, rigid free {}, Sturm {} = {} at {:e}, max residual {:.2e}",
        r.unknowns,
        r.iterations,
        r.shift,
        r.rigid_free,
        r.sturm.counted,
        r.sturm.expected,
        r.sturm.shift,
        max_abs(r.residuals.iter().copied())
    );
    println!("{name}: frequencies {:?}", r.frequencies);
}

// ---------------------------------------------------------------------------------------------------------------------
// C1 to C4, N1: mass properties of a box.

/// Closed-form Gram matrix of the six rigid motions (translations, rotations about the origin) of a box (ADR 0284 D1).
fn box_gram(origin: Vec3, l: Vec3, rho: f64) -> ([[f64; 6]; 6], f64, Vec3, [[f64; 3]; 3]) {
    let m = rho * l[0] * l[1] * l[2];
    let c = [origin[0] + l[0] / 2.0, origin[1] + l[1] / 2.0, origin[2] + l[2] / 2.0];
    let sq: Vec<f64> = (0..3)
        .map(|k| ((origin[k] + l[k]).powi(3) - origin[k].powi(3)) / (3.0 * l[k]))
        .collect();
    let mut io = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            io[i][j] = if i == j {
                m * (0..3).filter(|&k| k != i).map(|k| sq[k]).sum::<f64>()
            } else {
                -m * c[i] * c[j]
            };
        }
    }
    let mut g = [[0.0; 6]; 6];
    for i in 0..3 {
        g[i][i] = m;
    }
    // e_i . (e_k x c)
    for i in 0..3 {
        for k in 0..3 {
            let mut ek = [0.0; 3];
            ek[k] = 1.0;
            let cr = [ek[1] * c[2] - ek[2] * c[1], ek[2] * c[0] - ek[0] * c[2], ek[0] * c[1] - ek[1] * c[0]];
            g[i][3 + k] = m * cr[i];
            g[3 + k][i] = m * cr[i];
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            g[3 + i][3 + j] = io[i][j];
        }
    }
    (g, m, c, io)
}

fn gram(m: &CsrMatrix, r: &[Vec<f64>; 6]) -> [[f64; 6]; 6] {
    let mut g = [[0.0; 6]; 6];
    let mr: Vec<Vec<f64>> = r.iter().map(|x| mv(m, x)).collect();
    for i in 0..6 {
        for j in 0..6 {
            g[i][j] = dot(&r[i], &mr[j]);
        }
    }
    g
}

/// Errors (C1, C2, C3 about the origin) of a mass matrix's Gram matrix against the box closed form.
fn gram_errors(g: &[[f64; 6]; 6], gc: &[[f64; 6]; 6], m: f64, c: Vec3) -> (f64, f64, f64) {
    let mut c1: f64 = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            c1 = c1.max((g[i][j] - gc[i][j]).abs() / m);
        }
    }
    let cn = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
    let centre = [g[1][5] / m, g[2][3] / m, g[0][4] / m];
    let mut c2: f64 = 0.0;
    for k in 0..3 {
        c2 = c2.max((centre[k] - c[k]).abs() / cn);
    }
    for i in 0..3 {
        for k in 3..6 {
            c2 = c2.max((g[i][k] - gc[i][k]).abs() / (m * cn));
            c2 = c2.max((g[k][i] - gc[k][i]).abs() / (m * cn));
        }
    }
    let imax = max_abs((3..6).flat_map(|i| (3..6).map(move |j| (i, j))).map(|(i, j)| gc[i][j]));
    let c3 = max_abs((3..6).flat_map(|i| (3..6).map(move |j| (i, j))).map(|(i, j)| g[i][j] - gc[i][j])) / imax;
    (c1, c2, c3)
}

#[test]
fn c1_to_c4_box_mass_properties_and_n1() {
    let mut ch = Checks::new();
    let (gc, m_exact, c_exact, io) = box_gram(MB_ORIGIN, MB_LENGTHS, RHO);
    // Inertia about the centre, closed form.
    let cc = c_exact[0] * c_exact[0] + c_exact[1] * c_exact[1] + c_exact[2] * c_exact[2];
    let mut ic = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            ic[i][j] = io[i][j] - m_exact * (if i == j { cc } else { 0.0 } - c_exact[i] * c_exact[j]);
        }
    }
    let icmax = max_abs(ic.iter().flatten().copied());
    for kind in KINDS {
        let k = kind.name();
        let model = model_of(
            box_of(kind, MB_ORIGIN, MB_LENGTHS, MB_DIV, Some(10 + kind_index(kind))),
            0.3,
            RHO,
        );
        let m = consistent(&model).unwrap();
        let r = rigid_motions(&model, [0.0; 3]);
        let g = gram(&m, &r);
        let (c1, c2, c3) = gram_errors(&g, &gc, m_exact, c_exact);
        ch.at_most(&format!("C1 {k} translation block / m"), c1, 1e-12);
        ch.at_most(&format!("C2 {k} centre and translation-rotation block"), c2, 1e-12);
        ch.at_most(&format!("C3 {k} inertia about the origin"), c3, 1e-12);
        let p = mass_properties(&model, &m).unwrap();
        ch.at_most(&format!("C3 {k} mass_properties mass"), (p.mass - m_exact).abs() / m_exact, 1e-12);
        let cn = cc.sqrt();
        ch.at_most(
            &format!("C3 {k} mass_properties centre"),
            max_abs((0..3).map(|i| p.centre[i] - c_exact[i])) / cn,
            1e-12,
        );
        ch.at_most(
            &format!("C3 {k} mass_properties inertia about the centre"),
            max_abs((0..3).flat_map(|i| (0..3).map(move |j| (i, j))).map(|(i, j)| p.inertia[i][j] - ic[i][j])) / icmax,
            1e-12,
        );
        // C4: exact symmetry, and bit-identical for 1 and 8 threads.
        let mut asym: f64 = 0.0;
        for i in 0..m.n {
            for q in m.row_ptr[i]..m.row_ptr[i + 1] {
                let j = m.col[q];
                let pos = m.row_ptr[j] + m.col[m.row_ptr[j]..m.row_ptr[j + 1]].binary_search(&i).unwrap();
                asym = asym.max((m.val[q] - m.val[pos]).abs());
            }
        }
        ch.at_most(&format!("C4 {k} max |M - M^T|"), asym, 0.0);
        let m1 = consistent_with(&model, Some(1)).unwrap();
        let m8 = consistent_with(&model, Some(8)).unwrap();
        let same = m1.val.iter().zip(&m8.val).all(|(a, b)| a.to_bits() == b.to_bits())
            && m1.col == m8.col
            && m1.row_ptr == m8.row_ptr;
        ch.holds(&format!("C4 {k} 1 and 8 threads"), same, "bit for bit");
    }
    // N1: Tet4 with the one-point rule.
    let model = model_of(
        box_of(ElementKind::Tet4, MB_ORIGIN, MB_LENGTHS, MB_DIV, Some(10)),
        0.3,
        RHO,
    );
    let m = assemble_with(&model, None, |ctx| {
        element_mass_with_rule(element_for(ctx.kind), ctx.coords, RHO, quadrature::tet_1())
    })
    .unwrap();
    let g = gram(&m, &rigid_motions(&model, [0.0; 3]));
    let (_, _, c3) = gram_errors(&g, &gc, m_exact, c_exact);
    let ok = c3 > 1e-6;
    println!(
        "N1 Tet4 tet_1 inertia error: {c3:.3e} (control, must be > 1e-6) {}",
        if ok { "holds" } else { "FAILED" }
    );
    ch.holds("N1", ok, "control");
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// F1 to F3, N2: free box.

/// Solves the dense 6 x 6 system `g c = b` by Gaussian elimination with partial pivoting.
fn solve6(mut g: [[f64; 6]; 6], mut b: [f64; 6]) -> [f64; 6] {
    for col in 0..6 {
        let p = (col..6)
            .max_by(|&a, &c| g[a][col].abs().partial_cmp(&g[c][col].abs()).unwrap())
            .unwrap();
        g.swap(col, p);
        b.swap(col, p);
        for r in col + 1..6 {
            let f = g[r][col] / g[col][col];
            for c in col..6 {
                g[r][c] -= f * g[col][c];
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = [0.0; 6];
    for r in (0..6).rev() {
        let mut s = b[r];
        for c in r + 1..6 {
            s -= g[r][c] * x[c];
        }
        x[r] = s / g[r][r];
    }
    x
}

#[test]
fn f1_to_f3_free_box_and_n2() {
    let mut ch = Checks::new();
    for kind in KINDS {
        let k = kind.name();
        let model = model_of(
            box_of(kind, FB_ORIGIN, FB_LENGTHS, FB_DIV, Some(20 + kind_index(kind))),
            0.3,
            RHO,
        );
        let r = modal_analysis(&model, &ModalOptions::new(7)).unwrap();
        print_run(&format!("FB {k}"), &r);
        let f7 = r.frequencies[6];
        let ratio = max_abs(r.frequencies[..6].iter().copied()) / f7;
        ch.at_most(&format!("F1 {k} max |f_1..6| / f_7"), ratio, 1e-6);
        let m = consistent(&model).unwrap();
        ch.at_most(&format!("F1 {k} M-orthonormality"), m_orthonormality(&m, &r.modes), 1e-9);
        // F2: the six lowest modes span the rigid motions.
        let rig = rigid_motions(&model, mass_properties(&model, &m).unwrap().centre);
        let g = gram(&m, &rig);
        let mut worst: f64 = 0.0;
        for phi in &r.modes[..6] {
            let mphi = mv(&m, phi);
            let mut b = [0.0; 6];
            for i in 0..6 {
                b[i] = dot(&rig[i], &mphi);
            }
            let c = solve6(g, b);
            let d: Vec<f64> = (0..phi.len())
                .map(|e| phi[e] - (0..6).map(|i| c[i] * rig[i][e]).sum::<f64>())
                .collect();
            worst = worst.max(dot(&d, &mv(&m, &d)).max(0.0).sqrt());
        }
        ch.at_most(&format!("F2 {k} M-norm of the non-rigid part of modes 1..6"), worst, 1e-8);
        if kind == ElementKind::Hex8 {
            let lambda7 = r.eigenvalues[6];
            println!("F3 Hex8 sigma / lambda_7 = {:.4e}", r.shift / lambda7);
            ch.holds(
                "F3 Hex8 0 < sigma < 0.1 lambda_7",
                r.shift > 0.0 && r.shift < 0.1 * lambda7,
                &format!("sigma {:e}, lambda_7 {:e}", r.shift, lambda7),
            );
            let n2 = modal_analysis(&model, &ModalOptions::new(7).shift(Shift::Value(0.0)));
            println!("N2 Hex8 sigma = 0: {n2:?}", n2 = n2.as_ref().map(|r| r.frequencies.clone()));
            ch.holds(
                "N2 Hex8 sigma = 0 refused",
                matches!(n2, Err(FemError::Solver { .. })),
                "an error of the eigensolver",
            );
        }
    }
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// B1 to B5, D1: fixed-free bar with nu = 0.

fn bar(kind: ElementKind, n: usize) -> SolidModel {
    let mesh = box_of(kind, [0.0; 3], [BAR_L, BAR_B, BAR_B], [n, 1, 1], None);
    let mut m = model_of(mesh, 0.0, RHO);
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m
}

fn bar_exact(n: usize) -> f64 {
    (2 * n - 1) as f64 * (E_MOD / RHO).sqrt() / (4.0 * BAR_L)
}

/// The fixed-free 1-D linear chain with consistent mass (ADR 0284 D5).
fn bar_discrete(n: usize, elements: usize) -> f64 {
    let h = BAR_L / elements as f64;
    let k = (2 * n - 1) as f64 * PI / (2.0 * BAR_L);
    let lam = 6.0 * E_MOD / RHO / (h * h) * (1.0 - (k * h).cos()) / (2.0 + (k * h).cos());
    lam.sqrt() / (2.0 * PI)
}

/// Kinetic fraction of the x components of each mode.
fn axial_fractions(model: &SolidModel, m: &CsrMatrix, r: &ModalResult) -> Vec<f64> {
    let dofs = model.dof_map();
    r.modes
        .iter()
        .map(|phi| {
            let mut px = phi.clone();
            for (e, v) in px.iter_mut().enumerate() {
                if dofs.node_component(e).unwrap().1 != 0 {
                    *v = 0.0;
                }
            }
            dot(&px, &mv(m, &px))
        })
        .collect()
}

/// Runs the bar; returns the relative errors of the first three axial frequencies.
fn run_bar(ch: &mut Checks, kind: ElementKind, n: usize) -> [f64; 3] {
    let name = format!("BAR {} {n}", kind.name());
    let model = bar(kind, n);
    let r = modal_analysis(&model, &ModalOptions::new(4)).unwrap();
    print_run(&name, &r);
    let m = consistent(&model).unwrap();
    ch.at_most(&format!("{name} M-orthonormality"), m_orthonormality(&m, &r.modes), 1e-9);
    let ax = axial_fractions(&model, &m, &r);
    println!("{name} axial fractions {ax:?}");
    let axial: Vec<usize> = (0..ax.len()).filter(|&i| ax[i] >= 0.99).collect();
    ch.holds(
        &format!("B5 {name} the first three modes are axial"),
        axial.len() >= 3 && axial[..3] == [0, 1, 2],
        &format!("axial modes {axial:?}"),
    );
    let mut e = [f64::NAN; 3];
    for (j, &i) in axial.iter().take(3).enumerate() {
        e[j] = r.frequencies[i] / bar_exact(j + 1) - 1.0;
        println!("{name} f_{} = {:.6e} Hz, error {:.5e}", j + 1, r.frequencies[i], e[j]);
        if kind == ElementKind::Hex8 {
            ch.at_most(
                &format!("B3 {name} f_{} against the discrete closed form", j + 1),
                (r.frequencies[i] / bar_discrete(j + 1, n) - 1.0).abs(),
                1e-9,
            );
        } else {
            ch.at_least(
                &format!("B4 {name} f_{} / f_exact - 1 (lower bound)", j + 1),
                e[j],
                -1e-12,
            );
        }
    }
    e
}

#[test]
fn b1_to_b5_fixed_free_bar() {
    let mut ch = Checks::new();
    let e8 = run_bar(&mut ch, ElementKind::Hex8, 8);
    let e16 = run_bar(&mut ch, ElementKind::Hex8, 16);
    let e32 = run_bar(&mut ch, ElementKind::Hex8, 32);
    for n in 0..3 {
        ch.at_most(&format!("B1 Hex8 32 |f_{} / f_exact - 1|", n + 1), e32[n].abs(), 5e-3);
    }
    ch.at_least("B2 Hex8 order of f_1, 16 -> 32", (e16[0] / e32[0]).log2(), 1.8);
    ch.at_least("B2 Hex8 order of f_1, 8 -> 16", (e8[0] / e16[0]).log2(), 1.8);
    let t4 = run_bar(&mut ch, ElementKind::Tet4, 32);
    let t10 = run_bar(&mut ch, ElementKind::Tet10, 8);
    for n in 0..3 {
        ch.at_most(&format!("B4 Tet4 32 |f_{} / f_exact - 1|", n + 1), t4[n].abs(), 5e-3);
        ch.at_most(
            &format!("B4 Tet4 32 e_{} - e_{}(Hex8 32)", n + 1, n + 1),
            t4[n] - e32[n],
            1e-9,
        );
        ch.at_most(&format!("B4 Tet10 8 |f_{} / f_exact - 1|", n + 1), t10[n].abs(), 5e-3);
    }
    ch.finish();
}

#[test]
fn d1_modal_bit_identical_for_1_and_8_threads() {
    let model = bar(ElementKind::Hex8, 16);
    let a = modal_analysis(&model, &ModalOptions::new(4).threads(1)).unwrap();
    let b = modal_analysis(&model, &ModalOptions::new(4).threads(8)).unwrap();
    let bits = |r: &ModalResult| -> Vec<u64> {
        r.eigenvalues
            .iter()
            .chain(r.modes.iter().flatten())
            .map(|v| v.to_bits())
            .collect()
    };
    println!("D1 iterations {} and {}", a.iterations, b.iterations);
    assert_eq!(a.iterations, b.iterations);
    assert!(bits(&a) == bits(&b), "D1: results differ between 1 and 8 threads");
    println!("D1: bit for bit met");
}

// ---------------------------------------------------------------------------------------------------------------------
// T1, T1b, T2, S1: cantilever of slenderness 20.

fn cantilever(kind: ElementKind, nx: usize, ny: usize) -> SolidModel {
    let mesh = box_of(kind, [0.0; 3], [CANT_L, CANT_B, CANT_B], [nx, ny, ny], None);
    let mut m = model_of(mesh, 0.3, RHO);
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn f_eb(n: usize) -> f64 {
    let a = CANT_B * CANT_B;
    let i = CANT_B.powi(4) / 12.0;
    BETA_L[n] * BETA_L[n] * (E_MOD * i / (RHO * a * CANT_L.powi(4))).sqrt() / (2.0 * PI)
}

fn f_ref(n: usize) -> f64 {
    f_eb(n) * (1.0 - TIMO_C[n])
}

/// Runs a cantilever; returns the frequencies of the first two bending pairs `[[f_1a, f_1b], [f_2a, f_2b]]`.
fn run_cantilever(ch: &mut Checks, kind: ElementKind, nx: usize, ny: usize) -> [[f64; 2]; 2] {
    let name = format!("CANT {} {nx}x{ny}x{ny}", kind.name());
    let model = cantilever(kind, nx, ny);
    let t0 = std::time::Instant::now();
    let r = modal_analysis(&model, &ModalOptions::new(6)).unwrap();
    println!("{name}: modal analysis {:.2} s", t0.elapsed().as_secs_f64());
    print_run(&name, &r);
    let m = consistent(&model).unwrap();
    ch.at_most(&format!("{name} M-orthonormality"), m_orthonormality(&m, &r.modes), 1e-9);
    let rig = rigid_motions(&model, [0.0; 3]);
    let mass = dot(&rig[0], &mv(&m, &rig[0]));
    let mut bending = Vec::new();
    for (i, phi) in r.modes.iter().enumerate() {
        let mphi = mv(&m, phi);
        let e: Vec<f64> = (0..3).map(|k| dot(&rig[k], &mphi).powi(2) / mass).collect();
        println!(
            "{name} mode {i}: f = {:.6e} Hz, e_x {:.3e}, e_y {:.3e}, e_z {:.3e}",
            r.frequencies[i], e[0], e[1], e[2]
        );
        if e[1] + e[2] >= 0.02 {
            bending.push(i);
        }
    }
    println!("{name} bending modes {bending:?}");
    assert!(bending.len() >= 4, "{name}: fewer than four bending modes among six: {bending:?}");
    let f = |j: usize| r.frequencies[bending[j]];
    let out = [[f(0), f(1)], [f(2), f(3)]];
    for (p, pair) in out.iter().enumerate() {
        println!(
            "{name} bending pair {}: {:.6e}, {:.6e} Hz; split {:.3e}; against f_ref {:.6e}: {:.4e}, {:.4e}; against f_EB {:.6e}: {:.4e}, {:.4e}",
            p + 1,
            pair[0],
            pair[1],
            (pair[1] - pair[0]) / pair[0],
            f_ref(p),
            pair[0] / f_ref(p) - 1.0,
            pair[1] / f_ref(p) - 1.0,
            f_eb(p),
            pair[0] / f_eb(p) - 1.0,
            pair[1] / f_eb(p) - 1.0
        );
    }
    out
}

#[test]
fn s1_symmetric_hex8_cantilever_bending_planes() {
    let mut ch = Checks::new();
    let p = run_cantilever(&mut ch, ElementKind::Hex8, 40, 2);
    ch.at_most("S1 Hex8 40x2x2 first pair |f_a - f_b| / f_a", ((p[0][1] - p[0][0]) / p[0][0]).abs(), 1e-6);
    ch.at_most("S1 Hex8 40x2x2 second pair |f_a - f_b| / f_a", ((p[1][1] - p[1][0]) / p[1][0]).abs(), 1e-6);
    ch.finish();
}

#[test]
fn t2_tet10_cantilever_covering() {
    let mut ch = Checks::new();
    let p = run_cantilever(&mut ch, ElementKind::Tet10, 40, 2);
    for (n, limit) in [(0, 3e-2), (1, 6e-2)] {
        for (s, f) in p[n].iter().enumerate() {
            ch.at_most(
                &format!("T2 Tet10 40x2x2 bending {} member {s} against f_ref", n + 1),
                (f / f_ref(n) - 1.0).abs(),
                limit,
            );
        }
    }
    ch.finish();
}

#[test]
#[ignore = "heavy record (ADR 0284 T1): Tet10 cantilever 80 x 4 x 4, about 39 000 unknowns"]
fn t1_tet10_cantilever_heavy() {
    let mut ch = Checks::new();
    let coarse = run_cantilever(&mut ch, ElementKind::Tet10, 20, 1);
    println!("T1 record 20x1x1: {coarse:?}");
    let p = run_cantilever(&mut ch, ElementKind::Tet10, 80, 4);
    for (n, limit) in [(0, 1e-2), (1, 2.5e-2)] {
        for (s, f) in p[n].iter().enumerate() {
            ch.at_most(
                &format!("T1 Tet10 80x4x4 bending {} member {s} against f_ref", n + 1),
                (f / f_ref(n) - 1.0).abs(),
                limit,
            );
            ch.at_most(
                &format!("T1b Tet10 80x4x4 bending {} member {s} against f_EB", n + 1),
                (f / f_eb(n) - 1.0).abs(),
                limit,
            );
        }
    }
    ch.finish();
}

// ---------------------------------------------------------------------------------------------------------------------
// X1: refusals.

fn invalid_key(r: Result<ModalResult, FemError>) -> Option<(&'static str, String)> {
    match r {
        Err(FemError::InvalidValue { key, reason }) => Some((key, reason)),
        _ => None,
    }
}

#[test]
fn x1_refusals() {
    let mut ch = Checks::new();
    let model = bar(ElementKind::Hex8, 2);
    let a = invalid_key(modal_analysis(&model, &ModalOptions::new(0)));
    println!("X1 modes 0: {a:?}");
    ch.holds("X1 modes = 0", matches!(&a, Some(("modes", _))), "InvalidValue modes");
    let b = invalid_key(modal_analysis(&model, &ModalOptions::new(10_000)));
    println!("X1 modes 10000: {b:?}");
    ch.holds("X1 modes >= unknowns", matches!(&b, Some(("modes", _))), "InvalidValue modes");
    let moved = {
        let mut m = SolidModel::new(model.mesh().clone());
        m.add_material("all", Material::isotropic_elastic(E_MOD, 0.0, RHO, 0.0).unwrap())
            .unwrap();
        m.add_constraint(Constraint::displacement("xmin", 0, 1e-3)).unwrap();
        m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
        m
    };
    let c = invalid_key(modal_analysis(&moved, &ModalOptions::new(2)));
    println!("X1 non-zero constraint: {c:?}");
    ch.holds(
        "X1 non-zero prescribed value",
        matches!(&c, Some(("constraint", s)) if s.contains("node")),
        "InvalidValue constraint naming the node",
    );
    let mut massless = model_of(moved.mesh().clone(), 0.0, 0.0);
    massless.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    let d = invalid_key(modal_analysis(&massless, &ModalOptions::new(2)));
    println!("X1 density 0: {d:?}");
    ch.holds(
        "X1 massless unknowns",
        matches!(&d, Some(("density", s)) if s.contains("node")),
        "InvalidValue density naming the node",
    );
    ch.finish();
}
