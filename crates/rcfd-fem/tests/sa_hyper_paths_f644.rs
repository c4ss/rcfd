//! F-644 (ADR 0581): the checks ADR 0555 left on the hyperelastic linear paths. F0 to F1r: the six lowest frequencies of the
//! near-incompressible BBar cantilever (`K/G = 1e4`) with the hyperelastic material against its isotropic twin, at the
//! registered modal tolerance `1e-8`. Q0: the order-free measure. D0 to D1r and H0 to H1n: the damped-mode roots (matched
//! without regard to order) and the direct harmonic response of a model with two hyperelastic materials and two loss factors
//! against the twin. Set-ups of ADR 0555 D7 and amendment 2, copied (that file is not edited). Every measured value is printed;
//! a non-finite value or a refused call is a miss.

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedOptions};
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::mat_hyper;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::modal::{modal_analysis, ModalOptions, ModalResult};
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen::ResidualTest;

/// Measured values against their limits; the test fails at the end if any missed.
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
            self.0.push(format!("{name} = {value:e} not <= {limit:e}"));
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} not >= {limit:e}"));
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

// ----------------------------------------------------------------------------------------------- energies (ADR 0555 D7)

const RHO: f64 = 1000.0;

fn nhc() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6, 1e7).unwrap()
}
fn mrc() -> HyperelasticEnergy {
    HyperelasticEnergy::mooney_rivlin(3.5e5, 1.5e5, 1e7).unwrap()
}
fn nhi() -> HyperelasticEnergy {
    HyperelasticEnergy::neo_hookean(1e6, 1e10).unwrap()
}

fn hyper(e: HyperelasticEnergy) -> Material {
    Material::hyperelastic(e, RHO).unwrap()
}

/// The isotropic twin of an energy: `E`, `nu` of its small-strain `(G, K)`.
fn twin(e: &HyperelasticEnergy) -> Material {
    let (g, k) = mat_hyper::small_strain_moduli(e);
    let young = 9.0 * k * g / (3.0 * k + g);
    let nu = (3.0 * k - 2.0 * g) / (2.0 * (3.0 * k + g));
    Material::isotropic_elastic(young, nu, RHO, 0.0).unwrap()
}

// ----------------------------------------------------------------------------------------------- set-ups (ADR 0581 D2)

/// Cantilever L of ADR 0555 D7.
fn cantilever(material: Material, formulation: SolidFormulation) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [2.0, 0.5, 0.5], [8, 2, 2]).unwrap();
    if formulation != SolidFormulation::Full {
        mesh.set_block_property(0, BlockProperty::solid(formulation)).unwrap();
    }
    let mut model = SolidModel::new(mesh);
    model.add_material("all", material).unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::nodal_force("xmax", [0.0, 0.0, 1.0])).unwrap();
    model
}

/// Two-material L of ADR 0555 amendment 2: the element sets `root` (centre `x < 1`) and `tip`.
fn two_materials(root: Material, tip: Material) -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [2.0, 0.5, 0.5], [8, 2, 2]).unwrap();
    let centre_x = |e: usize, mesh: &SolidMesh| -> f64 {
        let nodes = mesh.element_nodes(e);
        nodes.iter().map(|&n| mesh.nodes()[n][0]).sum::<f64>() / nodes.len() as f64
    };
    let ne = mesh.n_elements();
    let root_set: Vec<usize> = (0..ne).filter(|&e| centre_x(e, &mesh) < 1.0).collect();
    let tip_set: Vec<usize> = (0..ne).filter(|&e| centre_x(e, &mesh) >= 1.0).collect();
    assert_eq!((root_set.len(), tip_set.len()), (16, 16));
    mesh.add_element_set("root", root_set).unwrap();
    mesh.add_element_set("tip", tip_set).unwrap();
    let mut model = SolidModel::new(mesh);
    model.add_material("root", root).unwrap();
    model.add_material("tip", tip).unwrap();
    for k in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", k, 0.0)).unwrap();
    }
    model.add_load(Load::nodal_force("xmax", [0.0, 0.0, 1.0])).unwrap();
    model
}

fn hyper_pair() -> SolidModel {
    two_materials(hyper(nhc()), hyper(mrc()))
}
fn twin_pair() -> SolidModel {
    two_materials(twin(&nhc()), twin(&mrc()))
}

// ----------------------------------------------------------------------------------------------- measures

/// Worst relative difference of two frequency lists, position by position (both ascending); infinite if the lengths differ.
fn worst_frequency(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return f64::INFINITY;
    }
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs() / y.abs())
        .fold(0.0f64, |m, v| if v.is_finite() { m.max(v) } else { f64::INFINITY })
}

fn dist(x: &C64, y: &C64) -> f64 {
    (x.re - y.re).hypot(x.im - y.im) / y.re.hypot(y.im)
}

/// Kuhn's augmenting path from row `k` in the graph `{(k, l): d[k][l] <= t}`.
fn augment(k: usize, d: &[Vec<f64>], t: f64, seen: &mut [bool], owner: &mut [Option<usize>]) -> bool {
    for l in 0..d.len() {
        if d[k][l] <= t && !seen[l] {
            seen[l] = true;
            if owner[l].map_or(true, |j| augment(j, d, t, seen, owner)) {
                owner[l] = Some(k);
                return true;
            }
        }
    }
    false
}

/// ADR 0581 D3: the bottleneck assignment `min over bijections pi of max_k |a_k - b_pi(k)| / |b_pi(k)|`; infinite for
/// lists of different lengths, an empty list or a non-finite distance.
fn order_free(a: &[C64], b: &[C64]) -> f64 {
    let n = a.len();
    if n != b.len() || n == 0 {
        return f64::INFINITY;
    }
    let d: Vec<Vec<f64>> = a.iter().map(|x| b.iter().map(|y| dist(x, y)).collect()).collect();
    if d.iter().flatten().any(|v| !v.is_finite()) {
        return f64::INFINITY;
    }
    let mut candidates: Vec<f64> = d.iter().flatten().copied().collect();
    candidates.sort_by(f64::total_cmp);
    for &t in &candidates {
        let mut owner = vec![None; n];
        if (0..n).all(|k| augment(k, &d, t, &mut vec![false; n], &mut owner)) {
            return t;
        }
    }
    f64::INFINITY
}

/// The positional measure of ADR 0555 L8m (a record here).
fn positional(a: &[C64], b: &[C64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return f64::INFINITY;
    }
    a.iter().zip(b).map(|(x, y)| dist(x, y)).fold(0.0f64, f64::max)
}

/// Relative difference of two complex responses, `max_j |x_j - y_j| / max_j |y_j|`.
fn response_rel(xre: &[f64], xim: &[f64], yre: &[f64], yim: &[f64]) -> f64 {
    if xre.len() != yre.len() || xre.is_empty() {
        return f64::INFINITY;
    }
    let mut d = 0.0f64;
    let mut m = 0.0f64;
    for j in 0..xre.len() {
        d = d.max((xre[j] - yre[j]).hypot(xim[j] - yim[j]));
        m = m.max(yre[j].hypot(yim[j]));
    }
    d / m
}

// ----------------------------------------------------------------------------------------------- F: the modes at K/G = 1e4

fn modes(material: Material, options: &ModalOptions) -> rcfd_fem::Result<ModalResult> {
    modal_analysis(&cantilever(material, SolidFormulation::BBar), options)
}

fn describe(r: &rcfd_fem::Result<ModalResult>) -> String {
    match r {
        Ok(r) => format!(
            "Ok, {} iterations, frequencies {:?}, residuals {:?}",
            r.iterations, r.frequencies, r.residuals
        ),
        Err(e) => format!("Err: {e}"),
    }
}

#[test]
fn f0_f1_f1n_f1r_near_incompressible_bbar_modes_against_the_twin() {
    let mut c = Checks::new();
    let e = nhi();
    let options = ModalOptions::new(6).tolerance(1e-8);
    let a = modes(hyper(e), &options);
    let b = modes(twin(&e), &options);
    println!("F0 hyperelastic: {}", describe(&a));
    println!("F0 twin: {}", describe(&b));
    let fine = |r: &rcfd_fem::Result<ModalResult>| -> bool {
        r.as_ref()
            .is_ok_and(|r| r.frequencies.len() == 6 && r.residuals.iter().take(6).all(|&x| x.is_finite() && x <= 1e-8))
    };
    c.holds("F0 hyperelastic: Ok, six frequencies, residuals <= 1e-8", fine(&a), "");
    c.holds("F0 twin: Ok, six frequencies, residuals <= 1e-8", fine(&b), "");
    let fa = a.as_ref().map(|r| r.frequencies.clone()).unwrap_or_default();
    let fb = b.as_ref().map(|r| r.frequencies.clone()).unwrap_or_default();
    c.at_most("F1 six frequencies, worst relative", worst_frequency(&fa, &fb), 1e-8);
    // F1n: the twin with G (1 + 1e-6).
    let en = HyperelasticEnergy::neo_hookean(1e6 * (1.0 + 1e-6), 1e10).unwrap();
    let n = modes(twin(&en), &options);
    println!("F1n perturbed twin: {}", describe(&n));
    let fn_ = n.as_ref().map(|r| r.frequencies.clone()).unwrap_or_default();
    c.at_least(
        "F1n against the twin with G (1 + 1e-6)",
        worst_frequency(&fa, &fn_),
        1e-7,
    );
    // F1r (record): the backward-error test at the default tolerance.
    let back = ModalOptions::new(6).residual_test(ResidualTest::Backward);
    let ra = modes(hyper(e), &back);
    let rb = modes(twin(&e), &back);
    println!("F1r (record) backward, hyperelastic: {}", describe(&ra));
    println!("F1r (record) backward, twin: {}", describe(&rb));
    if let (Ok(ra), Ok(rb)) = (&ra, &rb) {
        println!(
            "F1r (record) backward, worst relative frequency difference: {:e}; against the F run (hyperelastic): {:e}",
            worst_frequency(&ra.frequencies, &rb.frequencies),
            worst_frequency(&ra.frequencies, &fa)
        );
    }
    c.finish();
}

// ----------------------------------------------------------------------------------------------- Q0: the measure

#[test]
#[ignore = "known miss of ADR 0581 Q0b (registered run, log run1.log): |W - 1e-6| = 1.000113e-12 against 1e-12, the expected value of the set-up wrong (the exact W is 1e-6 / (1 + 1e-6)); Q0a 0, Q0c 1.9999, Q0d inf met; amendment 1, continued in q0_the_order_free_measure_corrected"]
fn q0_the_order_free_measure_known_miss() {
    let mut c = Checks::new();
    let roots: Vec<C64> = (1..=4)
        .flat_map(|k| {
            let w = 7.0 * (k * k) as f64;
            [C64::new(-0.01 * w, w), C64::new(-0.01 * w, -w)]
        })
        .collect();
    let reversed: Vec<C64> = roots.iter().rev().copied().collect();
    let w = order_free(&roots, &reversed);
    c.holds("Q0a against the reverse: exactly 0", w == 0.0, &format!("{w:e}"));
    let mut scaled = roots.clone();
    scaled[3] = C64::new(scaled[3].re * (1.0 + 1e-6), scaled[3].im * (1.0 + 1e-6));
    let w = order_free(&roots, &scaled);
    c.at_most("Q0b one root scaled by 1 + 1e-6: |W - 1e-6|", (w - 1e-6).abs(), 1e-12);
    let mut duplicated = roots.clone();
    duplicated[0] = duplicated[1];
    let w = order_free(&duplicated, &roots);
    let gap = dist(&roots[0], &roots[1]) / 2.0;
    c.at_least("Q0c root 0 replaced by root 1 (a bijection enforced)", w, gap);
    let w = order_free(&roots, &roots[..7]);
    c.holds("Q0d lists of 8 and 7: a miss", w.is_infinite(), &format!("{w:e}"));
    c.finish();
}

/// ADR 0581 amendment 1: Q0 with the expected value of Q0b corrected to `1e-6 / (1 + 1e-6)`, the limit unchanged.
#[test]
fn q0_the_order_free_measure_corrected() {
    let mut c = Checks::new();
    let roots: Vec<C64> = (1..=4)
        .flat_map(|k| {
            let w = 7.0 * (k * k) as f64;
            [C64::new(-0.01 * w, w), C64::new(-0.01 * w, -w)]
        })
        .collect();
    let reversed: Vec<C64> = roots.iter().rev().copied().collect();
    let w = order_free(&roots, &reversed);
    c.holds("Q0a against the reverse: exactly 0", w == 0.0, &format!("{w:e}"));
    let mut scaled = roots.clone();
    scaled[3] = C64::new(scaled[3].re * (1.0 + 1e-6), scaled[3].im * (1.0 + 1e-6));
    let w = order_free(&roots, &scaled);
    c.at_most(
        "Q0b' one root scaled by 1 + 1e-6: |W - 1e-6 / (1 + 1e-6)|",
        (w - 1e-6 / (1.0 + 1e-6)).abs(),
        1e-12,
    );
    let mut duplicated = roots.clone();
    duplicated[0] = duplicated[1];
    let w = order_free(&duplicated, &roots);
    let gap = dist(&roots[0], &roots[1]) / 2.0;
    c.at_least("Q0c root 0 replaced by root 1 (a bijection enforced)", w, gap);
    let w = order_free(&roots, &roots[..7]);
    c.holds("Q0d lists of 8 and 7: a miss", w.is_infinite(), &format!("{w:e}"));
    c.finish();
}

// ----------------------------------------------------------------------------------------------- D: damped modes

fn damped_options(tip: f64) -> DampedOptions {
    let mut d = DampedOptions::new(4);
    d.loss_factors.push(("root".to_string(), LossFactor::Constant(0.02)));
    d.loss_factors.push(("tip".to_string(), LossFactor::Constant(tip)));
    d
}

#[test]
fn d0_d1_d1n_d1r_two_material_damped_roots_order_free() {
    let mut c = Checks::new();
    let a = damped_modes(&hyper_pair(), &damped_options(0.05));
    let b = damped_modes(&twin_pair(), &damped_options(0.05));
    let ra = a.as_ref().map(|r| r.roots.clone()).unwrap_or_default();
    let rb = b.as_ref().map(|r| r.roots.clone()).unwrap_or_default();
    c.holds(
        "D0 both Ok, equal non-zero root counts",
        a.is_ok() && b.is_ok() && !ra.is_empty() && ra.len() == rb.len(),
        &format!(
            "{} / {} ({:?})",
            ra.len(),
            rb.len(),
            a.as_ref().err().or(b.as_ref().err()).map(|e| e.to_string())
        ),
    );
    for (k, (x, y)) in ra.iter().zip(&rb).enumerate() {
        println!(
            "D (record) root {k}: hyperelastic {:e} {:+e}i, twin {:e} {:+e}i",
            x.re, x.im, y.re, y.im
        );
    }
    c.at_most(
        "D1 damped-mode roots, order-free worst relative",
        order_free(&ra, &rb),
        1e-10,
    );
    let n = damped_modes(&twin_pair(), &damped_options(0.06));
    let rn = n.as_ref().map(|r| r.roots.clone()).unwrap_or_default();
    c.at_least("D1n against the twin with tip at 0.06", order_free(&ra, &rn), 1e-6);
    println!("D1r (record) positional measure: {:e}", positional(&ra, &rb));
    c.finish();
}

// ----------------------------------------------------------------------------------------------- H: direct harmonic

#[test]
fn h0_h1_h1n_two_material_direct_harmonic() {
    let mut c = Checks::new();
    let options = |tip: f64| {
        DirectHarmonicOptions::new()
            .loss_factor("root", LossFactor::Constant(0.02))
            .loss_factor("tip", LossFactor::Constant(tip))
            .threads(1)
    };
    let (ma, mb) = (hyper_pair(), twin_pair());
    let ha = DirectHarmonic::new(&ma, &options(0.05));
    let hb = DirectHarmonic::new(&mb, &options(0.05));
    let hn = DirectHarmonic::new(&mb, &options(0.06));
    c.holds(
        "H0 DirectHarmonic::new Ok (hyperelastic, twin, perturbed twin)",
        ha.is_ok() && hb.is_ok() && hn.is_ok(),
        &format!(
            "{:?}",
            ha.as_ref()
                .err()
                .or(hb.as_ref().err())
                .or(hn.as_ref().err())
                .map(|e| e.to_string())
        ),
    );
    let load = HarmonicLoad::from_model(&ma).unwrap();
    for omega in [50.0, 500.0] {
        let x = ha.as_ref().ok().map(|h| h.response(&load, omega));
        let y = hb.as_ref().ok().map(|h| h.response(&load, omega));
        let z = hn.as_ref().ok().map(|h| h.response(&load, omega));
        match (x, y, z) {
            (Some(Ok(x)), Some(Ok(y)), Some(Ok(z))) => {
                c.holds(&format!("H0 responses Ok at omega {omega}"), true, "");
                c.at_most(
                    &format!("H1 direct harmonic at omega {omega}"),
                    response_rel(&x.re, &x.im, &y.re, &y.im),
                    1e-10,
                );
                c.at_least(
                    &format!("H1n against the twin with tip at 0.06, omega {omega}"),
                    response_rel(&x.re, &x.im, &z.re, &z.im),
                    1e-6,
                );
            }
            _ => c.holds(
                &format!("H0 responses Ok at omega {omega}"),
                false,
                "a call refused or failed",
            ),
        }
    }
    c.finish();
}
