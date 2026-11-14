//! F-548, ADR 0467: the share of each damped mode carried by the residual-vector coordinates and the pseudo-mode flag
//! (checks P1, P2, T0, T1, R1, R2, M1, M2, D1, D2, H1).

use rcfd_fem::builders::hex_box;
use rcfd_fem::damped::{damped_modes, DampedMode, DampedModes, DampedOptions, PSEUDO_SHARE};
use rcfd_fem::harmonic_direct::{Dashpot, LossFactor};
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen::ResidualTest;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const E_MOD: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const AREA: f64 = 1e-4;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {} ({detail})", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:e} (limit {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:e} (lower limit {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(name.to_string());
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(E_MOD, nu, RHO, 0.0).unwrap()
}

fn modal(m: usize) -> ModalOptions {
    ModalOptions::new(m)
        .tolerance(1e-12)
        .residual_test(ResidualTest::Backward)
}

fn cantilever() -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0, 0.1, 0.1], [10, 2, 2]).unwrap());
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn bar() -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0, 0.01, 0.01], [100, 1, 1]).unwrap());
    m.add_material("all", steel(0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m
}

fn impedance() -> f64 {
    AREA * (E_MOD * RHO).sqrt()
}

fn c0() -> f64 {
    (E_MOD / RHO).sqrt()
}

fn tip(c: f64, residual: bool, threads: usize) -> DampedModes {
    let model = bar();
    let mut o = DampedOptions::with_modal(modal(20))
        .residual_vectors(residual)
        .threads(threads);
    for &a in model.mesh().node_set("xmax").unwrap() {
        o = o.dashpot(Dashpot::ground(a, 0, c / 4.0));
    }
    damped_modes(&model, &o).unwrap()
}

/// The reported oscillating mode nearest `exact`.
fn nearest(d: &DampedModes, exact: C64) -> &DampedMode {
    d.modes
        .iter()
        .filter(|m| m.oscillating)
        .min_by(|p, q| (p.eigenvalue - exact).abs().total_cmp(&(q.eigenvalue - exact).abs()))
        .unwrap()
}

/// The non-oscillating mode of smallest modulus.
fn smallest_real(d: &DampedModes) -> &DampedMode {
    d.modes
        .iter()
        .filter(|m| !m.oscillating)
        .min_by(|p, q| p.eigenvalue.abs().total_cmp(&q.eigenvalue.abs()))
        .unwrap()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn apply(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

/// The share from the public data: the coefficients of the shape on the residual vectors over `x^H M x`.
fn independent_share(mass: &CsrMatrix, d: &DampedModes, m: &DampedMode) -> f64 {
    let re: Vec<f64> = m.shape.iter().map(|v| v.re).collect();
    let im: Vec<f64> = m.shape.iter().map(|v| v.im).collect();
    let (mre, mim) = (apply(mass, &re), apply(mass, &im));
    let whole = dot(&re, &mre) + dot(&im, &mim);
    let part: f64 = d
        .residual_vectors
        .iter()
        .map(|r| dot(r, &mre).powi(2) + dot(r, &mim).powi(2))
        .sum();
    part / whole
}

fn show(tag: &str, d: &DampedModes) {
    println!(
        "{tag}: {} reported (osc {}, real {}), pseudo {}, notes {:?}",
        d.modes.len(),
        d.oscillating,
        d.non_oscillating,
        d.pseudo_modes,
        d.notes
    );
    for m in &d.modes {
        if m.pseudo || !m.oscillating {
            println!(
                "  lambda {} osc {} share {:.6e} pseudo {}",
                m.eigenvalue, m.oscillating, m.residual_share, m.pseudo
            );
        }
    }
}

/// The three oscillating targets of the bar at `c = 0.9 Z` (`f < 1`) and `c = 2 Z`.
fn targets(f: f64) -> Vec<C64> {
    let s = c0();
    if f < 1.0 {
        (1..=3)
            .map(|j| C64::new(-(0.9f64).atanh(), (2 * j - 1) as f64 * PI / 2.0).scale(s))
            .collect()
    } else {
        (1..=3)
            .map(|j| C64::new(-(0.5f64).atanh(), j as f64 * PI).scale(s))
            .collect()
    }
}

#[test]
fn flags_on_the_bar_near_the_impedance() {
    let mut c = Checks::new();
    let z = impedance();
    let model = bar();
    let mass = consistent(&model).unwrap();
    for (f, tag) in [(0.9, "c = 0.9 Z"), (2.0, "c = 2 Z")] {
        let d = tip(f * z, true, 4);
        show(tag, &d);
        // P1
        let mut worst: f64 = 0.0;
        let mut range = true;
        for m in &d.modes {
            range &= m.residual_share >= 0.0 && m.residual_share <= 1.0 + 1e-10;
            worst = worst.max((m.residual_share - independent_share(&mass, &d, m)).abs());
        }
        c.holds(&format!("P1 {tag}: shares in [0, 1]"), range, "");
        c.at_most(&format!("P1 {tag}: share against the independent one"), worst, 1e-10);
        // P2
        let exact = d.modes.iter().all(|m| m.pseudo == (m.residual_share > PSEUDO_SHARE));
        let count = d.modes.iter().filter(|m| m.pseudo).count();
        let note = d.notes.iter().any(|n| n.contains(&format!("{count} roots flagged")));
        c.holds(
            &format!("P2 {tag}: flag = share > 0.1, count, note"),
            exact && PSEUDO_SHARE == 0.1 && d.pseudo_modes == count && (note == (count > 0)),
            &format!("{count} flagged"),
        );
        // T0, T1
        let mut tmax: f64 = 0.0;
        let mut unflagged = true;
        for t in targets(f) {
            let m = nearest(&d, t);
            tmax = tmax.max(m.residual_share);
            unflagged &= !m.pseudo;
        }
        let mut skip = None;
        if f > 1.0 {
            let m = smallest_real(&d);
            tmax = tmax.max(m.residual_share);
            unflagged &= !m.pseudo;
            skip = Some(m.eigenvalue.re);
        }
        c.holds(&format!("T {tag}: the target roots are not flagged"), unflagged, "");
        c.at_most(&format!("T {tag}: largest share of a target root"), tmax, 1e-2);
        // R1, R2, M1
        let others: Vec<&DampedMode> = d
            .modes
            .iter()
            .filter(|m| !m.oscillating && Some(m.eigenvalue.re) != skip)
            .collect();
        let min_share = others.iter().map(|m| m.residual_share).fold(f64::INFINITY, f64::min);
        c.holds(
            &format!("R {tag}: every other real root is flagged"),
            others.iter().all(|m| m.pseudo) && !others.is_empty(),
            &format!("{} roots", others.len()),
        );
        c.at_least(&format!("M1 {tag}: smallest share of those roots"), min_share, 0.3);
        // M2
        let osc_unflagged = d
            .modes
            .iter()
            .filter(|m| m.oscillating && !m.pseudo)
            .map(|m| m.residual_share)
            .fold(0.0, f64::max);
        let osc_flagged = d.modes.iter().filter(|m| m.oscillating && m.pseudo).count();
        println!("  oscillating roots flagged: {osc_flagged}");
        c.at_most(
            &format!("M2 {tag}: largest unflagged oscillating share"),
            osc_unflagged,
            5e-2,
        );
        // H1
        let d1 = tip(f * z, true, 1);
        let same = d1.modes.len() == d.modes.len()
            && d1
                .modes
                .iter()
                .zip(&d.modes)
                .all(|(a, b)| a.residual_share.to_bits() == b.residual_share.to_bits() && a.pseudo == b.pseudo);
        c.holds(&format!("H1 {tag}: threads 1 and 4 bit-identical"), same, "");
    }
    c.finish();
}

#[test]
fn defaults_carry_no_share_and_no_flag() {
    let mut c = Checks::new();
    let z = impedance();
    // D1: without the option
    for (tag, d) in [
        ("bar Z/2", tip(0.5 * z, false, 4)),
        ("bar 2 Z", tip(2.0 * z, false, 4)),
        ("bar 0.9 Z", tip(0.9 * z, false, 4)),
    ] {
        let clean = d
            .modes
            .iter()
            .all(|m| m.residual_share.to_bits() == 0.0f64.to_bits() && !m.pseudo);
        c.holds(
            &format!("D1 {tag}: share 0, no flag, no count, no note"),
            clean && d.pseudo_modes == 0 && !d.notes.iter().any(|n| n.contains("pseudo")),
            "",
        );
    }
    // D2: option on, no dashpot
    let model = cantilever();
    let u = modal_analysis(&model, &modal(12)).unwrap();
    let w: Vec<f64> = u.eigenvalues.iter().map(|l| l.sqrt()).collect();
    let ray = DampedOptions::with_modal(modal(12))
        .rayleigh(0.2 * w[0], 0.5 / w[11])
        .threads(4);
    let lf = DampedOptions::with_modal(modal(12)).loss_factor("all", LossFactor::Constant(0.04));
    for (what, o) in [("Rayleigh", ray), ("loss factor", lf)] {
        let off = damped_modes(&model, &o).unwrap();
        let on = damped_modes(&model, &o.clone().residual_vectors(true)).unwrap();
        let same = off == on
            && on
                .modes
                .iter()
                .all(|m| m.residual_share.to_bits() == 0.0f64.to_bits() && !m.pseudo);
        c.holds(
            &format!("D2 {what}: option on without dashpots equals off, no flag"),
            same && on.pseudo_modes == 0,
            "",
        );
    }
    c.finish();
}
