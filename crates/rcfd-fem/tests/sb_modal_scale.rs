//! F-457 registered checks (ADR 0354) on the cantilever of ADR 0284: C1, C2 (Tet10 40 x 2 x 2 with the scale-aware test at
//! 1e-12 against the amendment-1 run), D0 (the default test reproduces that run bit for bit), H1 (Hex8 40 x 2 x 2) and the
//! diagnostic record G1. Helpers copied from `sb_modal.rs` (not edited).

use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::mass::{consistent, rigid_motions};
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;

const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const CANT_L: f64 = 1.0;
const CANT_B: f64 = 0.05;
/// The scale-aware tolerance of the acceptance.
const TAU: f64 = 1e-12;
/// The default-test tolerance of the amendment-1 runs (ADR 0284 amendment 1).
const AMEND_TOLERANCE: f64 = 1e-7;
/// Frequencies of the amendment-1 run `t2_amended_tet10_cantilever_covering`, Tet10 40 x 2 x 2 (ADR 0354 reading 1).
const AMEND_TET10: [f64; 6] = [
    40.83463169470747,
    40.843159320566095,
    253.00307073453942,
    253.07827467142752,
    696.06213246288,
    696.3847805894118,
];
/// Frequencies of the amendment-1 run `s1_amended_symmetric_hex8_cantilever_bending_planes`, Hex8 40 x 2 x 2.
const AMEND_HEX8: [f64; 6] = [
    43.53945747018705,
    43.539457471918574,
    270.17409714143236,
    270.1740971417086,
    745.4462105967888,
    745.4462105968945,
];

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

fn cantilever(kind: ElementKind, nx: usize, ny: usize) -> SolidModel {
    let div = [nx, ny, ny];
    let lengths = [CANT_L, CANT_B, CANT_B];
    let mesh: SolidMesh = match kind {
        ElementKind::Hex8 => hex_box([0.0; 3], lengths, div).unwrap(),
        ElementKind::Tet10 => upgrade_to_tet10(&tet_box([0.0; 3], lengths, div).unwrap()).unwrap().0,
        _ => tet_box([0.0; 3], lengths, div).unwrap(),
    };
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, 0.3, RHO, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
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

/// Indices of the transverse bending modes (`e_y + e_z >= 0.02`, ADR 0284 D8), ascending.
fn bending_modes(model: &SolidModel, m: &CsrMatrix, r: &ModalResult) -> Vec<usize> {
    let rig = rigid_motions(model, [0.0; 3]);
    let mass = dot(&rig[0], &mv(m, &rig[0]));
    let mut out = Vec::new();
    for (i, phi) in r.modes.iter().enumerate() {
        let mphi = mv(m, phi);
        let e: Vec<f64> = (0..3).map(|k| dot(&rig[k], &mphi).powi(2) / mass).collect();
        if e[1] + e[2] >= 0.02 {
            out.push(i);
        }
    }
    out
}

fn print_run(name: &str, r: &ModalResult) {
    println!(
        "{name}: unknowns {}, iterations {}, shift {:e}, Sturm {} = {} at {:e}, residuals {:?}",
        r.unknowns, r.iterations, r.shift, r.sturm.counted, r.sturm.expected, r.sturm.shift, r.residuals
    );
    println!("{name}: frequencies {:?}", r.frequencies);
}

/// The four bending frequencies (first and second pair) of a run, relative to the amendment-1 values at the same
/// bending positions.
fn bending_against(
    ch: &mut Checks,
    name: &str,
    model: &SolidModel,
    m: &CsrMatrix,
    r: &ModalResult,
    reference: &[f64; 6],
) -> [f64; 4] {
    let b = bending_modes(model, m, r);
    println!("{name}: bending modes {b:?}");
    ch.holds(&format!("{name} four bending modes"), b.len() >= 4, &format!("{b:?}"));
    let mut d = [f64::NAN; 4];
    if b.len() >= 4 {
        // The amendment-1 runs have modes 0 to 5 all bending (ADR 0284 addendum), so position j of the bending list is
        // reference index j.
        for j in 0..4 {
            d[j] = (r.frequencies[b[j]] / reference[j] - 1.0).abs();
        }
    }
    println!("{name}: |f / f_amend - 1| of the first and second bending pairs {d:?}");
    d
}

/// C1, C2, D0: Tet10 40 x 2 x 2.
#[test]
fn c1_c2_d0_tet10_cantilever_scale_aware() {
    let mut ch = Checks::new();
    let model = cantilever(ElementKind::Tet10, 40, 2);
    let m = consistent(&model).unwrap();

    // D0: the default test as in amendment 1, on this tree.
    let d = modal_analysis(&model, &ModalOptions::new(6).tolerance(AMEND_TOLERANCE)).unwrap();
    print_run("D0 Tet10 40x2x2 default 1e-7", &d);
    let same = d.frequencies.len() == 6
        && d.frequencies
            .iter()
            .zip(&AMEND_TET10)
            .all(|(a, b)| a.to_bits() == b.to_bits());
    ch.holds(
        "D0 frequencies equal the amendment-1 run bit for bit",
        same,
        &format!("{:?}", d.frequencies),
    );
    ch.holds("D0 iterations 7", d.iterations == 7, &format!("{}", d.iterations));

    // C1: the scale-aware test at 1e-12.
    let opts = ModalOptions::new(6)
        .tolerance(TAU)
        .residual_test(ResidualTest::Backward);
    let t0 = std::time::Instant::now();
    let r = modal_analysis(&model, &opts);
    println!("C1: modal analysis {:.2} s", t0.elapsed().as_secs_f64());
    let r = match r {
        Ok(r) => r,
        Err(e) => panic!("C1 Tet10 40x2x2 backward 1e-12 did not converge: {e}"),
    };
    print_run("C1 Tet10 40x2x2 backward 1e-12", &r);
    ch.holds("C1 iterations <= 20", r.iterations <= 20, &format!("{}", r.iterations));
    ch.at_most("C1 M-orthonormality", m_orthonormality(&m, &r.modes), 1e-9);
    ch.holds(
        "C1 Sturm counted = expected",
        r.sturm.counted == r.sturm.expected,
        &format!("{} = {}", r.sturm.counted, r.sturm.expected),
    );

    // C2: the four bending frequencies against the amendment-1 values.
    let dev = bending_against(&mut ch, "C2 Tet10", &model, &m, &r, &AMEND_TET10);
    for (j, v) in dev.iter().enumerate() {
        ch.at_most(
            &format!("C2 Tet10 bending pair {} member {}", j / 2 + 1, j % 2),
            *v,
            1e-10,
        );
    }
    ch.finish();
}

/// H1 (extension): Hex8 40 x 2 x 2.
#[test]
fn h1_hex8_cantilever_scale_aware() {
    let mut ch = Checks::new();
    let model = cantilever(ElementKind::Hex8, 40, 2);
    let m = consistent(&model).unwrap();
    let opts = ModalOptions::new(6)
        .tolerance(TAU)
        .residual_test(ResidualTest::Backward);
    let r = match modal_analysis(&model, &opts) {
        Ok(r) => r,
        Err(e) => panic!("H1 Hex8 40x2x2 backward 1e-12 did not converge: {e}"),
    };
    print_run("H1 Hex8 40x2x2 backward 1e-12", &r);
    ch.holds("H1 iterations <= 20", r.iterations <= 20, &format!("{}", r.iterations));
    let dev = bending_against(&mut ch, "H1 Hex8", &model, &m, &r, &AMEND_HEX8);
    for (j, v) in dev.iter().enumerate() {
        ch.at_most(
            &format!("H1 Hex8 bending pair {} member {}", j / 2 + 1, j % 2),
            *v,
            1e-10,
        );
    }
    let b = bending_modes(&model, &m, &r);
    if b.len() >= 4 {
        for p in 0..2 {
            let (fa, fb) = (r.frequencies[b[2 * p]], r.frequencies[b[2 * p + 1]]);
            ch.at_most(&format!("H1 Hex8 pair {} split", p + 1), ((fb - fa) / fa).abs(), 1e-6);
        }
    }
    ch.finish();
}

/// G1 (record): the same four frequencies from runs that stop at other iterations.
#[test]
fn g1_noise_record() {
    let model = cantilever(ElementKind::Tet10, 40, 2);
    let m = consistent(&model).unwrap();
    let mut runs: Vec<(String, ModalOptions)> = [1e-3, 1e-4, 1e-5, 1e-6]
        .iter()
        .map(|&t| (format!("default {t:e}"), ModalOptions::new(6).tolerance(t)))
        .collect();
    for t in [1e-13, 1e-14] {
        runs.push((
            format!("backward {t:e}"),
            ModalOptions::new(6).tolerance(t).residual_test(ResidualTest::Backward),
        ));
    }
    for (name, opts) in runs {
        match modal_analysis(&model, &opts) {
            Ok(r) => {
                let mut ch = Checks::new();
                let dev = bending_against(&mut ch, &format!("G1 {name}"), &model, &m, &r, &AMEND_TET10);
                println!(
                    "G1 {name}: iterations {}, residuals {:?}, deviations {dev:?}",
                    r.iterations, r.residuals
                );
            }
            Err(e) => println!("G1 {name}: not converged: {e}"),
        }
    }
}
