//! F-500 (ADR 0407): rotating modes of a cyclic sector at a harmonic index. Checks C1 to C3d, L1 to L3, P1 to P5, R1 and the
//! negative controls N1 to N3 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::{full_rotor, modal_analysis_harmonic, FullRotor, HarmonicModes};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{
    engine_order_excites, subspace_mac, HarmonicSpeedModes, RotatingOptions, RotatingPart, RotatingSector, SpeedModes,
};
use rcfd_fem::{Constraint, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const NU: f64 = 0.3;
const N: usize = 12;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
const MODES: usize = 4;

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn le(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.3e} (limit {limit:.0e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn ge(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what}: {}", if ok { "holds" } else { "FAILS" });
        if !ok {
            self.failed.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "failed: {:?}", self.failed);
    }
}

fn alpha() -> f64 {
    2.0 * PI / N as f64
}

/// The disc sector D of ADR 0395 (`si_cyclic_harmonic.rs`), without its rim sets.
fn d_mesh() -> SolidMesh {
    let b = hex_box([0.25, 0.0, -0.1], [0.75, alpha(), 0.2], [3, 2, 1]).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

fn model_on(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap())
        .unwrap();
    m
}

fn clamp(m: &mut SolidModel, set: &str) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn d_sector() -> SolidModel {
    let mut m = model_on(d_mesh());
    clamp(&mut m, "xmin");
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, [0.0; 3], N))
        .unwrap();
    m
}

/// The full rotor of D with `xmin` clamped (`full_of(sector, None)` of `si_cyclic_harmonic.rs`).
fn full_of(sector: &SolidModel) -> (FullRotor, SolidModel) {
    let rotor = full_rotor(sector).unwrap();
    let mut full = model_on(rotor.mesh().clone());
    clamp(&mut full, "xmin");
    (rotor, full)
}

fn equivalence_options(modes: usize) -> ModalOptions {
    let mut o = ModalOptions::new(modes).tolerance(1e-9);
    o.max_iterations = 400;
    o
}

fn options(modes: usize) -> RotatingOptions {
    RotatingOptions::new(modes).modal(equivalence_options(modes))
}

fn quad(a: &CsrMatrix, x: &[f64]) -> f64 {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    x.iter().zip(&y).map(|(p, q)| p * q).sum()
}

fn harmonic_modes(r: HarmonicSpeedModes, what: &str) -> HarmonicModes {
    match r {
        HarmonicSpeedModes::Modes(h) => *h,
        HarmonicSpeedModes::LossOfStiffness(l) => panic!("defective set-up: {what}: loss of stiffness {l:?}"),
    }
}

/// The rest modes of every index and `omega_ref`, the lowest sector frequency, rad/s.
fn rest(rs: &RotatingSector) -> (Vec<HarmonicModes>, f64) {
    let hs: Vec<HarmonicModes> = (0..=N / 2)
        .map(|k| harmonic_modes(rs.modes(k, 0.0).unwrap(), "rest"))
        .collect();
    let lmin = hs.iter().map(|h| h.values[0]).fold(f64::INFINITY, f64::min);
    (hs, lmin.sqrt())
}

/// The loss speed of the registration: `Omega_L^2 = 2 min lambda/d` over the rest modes with `d = phi^T (M_perp - s
/// K_sigma,1) phi > 0` (both parts), and the index attaining it; `s = 1`, or `s = 0` (the fallback) when no `d > 0`.
fn loss_speed(rs: &RotatingSector, hs: &[HarmonicModes]) -> (f64, usize, bool) {
    let part = rs.part();
    let mut best: Option<(f64, usize)> = None;
    let mut best_spin: Option<(f64, usize)> = None;
    for h in hs {
        for (i, phi) in h.modes.iter().enumerate() {
            let m = quad(part.mass(), &phi.cos) + quad(part.mass(), &phi.sin);
            let p = (quad(part.spin_softening(), &phi.cos) + quad(part.spin_softening(), &phi.sin)) / m;
            let g =
                (quad(part.unit_geometric_stiffness(), &phi.cos) + quad(part.unit_geometric_stiffness(), &phi.sin)) / m;
            let l = h.eigenvalues[i];
            let d = p - g;
            if d > 0.0 && best.map_or(true, |(v, _)| l / d < v) {
                best = Some((l / d, h.harmonic));
            }
            if p > 0.0 && best_spin.map_or(true, |(v, _)| l / p < v) {
                best_spin = Some((l / p, h.harmonic));
            }
        }
    }
    match best {
        Some((v, k)) => ((2.0 * v).sqrt(), k, true),
        None => {
            let (v, k) = best_spin.expect("a mode with in-plane motion");
            ((2.0 * v).sqrt(), k, false)
        }
    }
}

/// The sector eigenvalues of all indices below the cut-off of ADR 0395 H5, sorted, and the cut-off.
fn below_cutoff(hs: &[HarmonicModes]) -> (Vec<f64>, f64) {
    let lambda_c = 0.999
        * hs.iter()
            .map(|h| h.values.iter().fold(0.0f64, |m, &v| m.max(v)))
            .fold(f64::INFINITY, f64::min);
    let mut below: Vec<f64> = hs
        .iter()
        .flat_map(|h| h.eigenvalues.iter().copied())
        .filter(|&l| l < lambda_c)
        .collect();
    below.sort_by(f64::total_cmp);
    (below, lambda_c)
}

fn worst_relative(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(s, f)| ((s / f).sqrt() - 1.0).abs())
        .fold(0.0f64, f64::max)
}

// ------------------------------------------------------------------------------------------------------------ C1, C2, L3

#[test]
fn c1_c2_sector_equals_full_rotor_by_family() {
    let mut ch = Checks::new();
    let sector = d_sector();
    let rs = RotatingSector::new(&sector, &options(MODES)).unwrap();
    let (rest_modes, w_ref) = rest(&rs);
    println!("omega_ref = {w_ref:.9e} rad/s ({:.6e} Hz)", w_ref / (2.0 * PI));
    let (rotor, full) = full_of(&sector);
    let sdofs = sector.dof_map();
    let fdofs = full.dof_map();
    // The full count: the sector count at rest plus 8 (ADR 0395 H5), the same at every speed.
    let n_full = below_cutoff(&rest_modes).0.len() + 8;
    let fp = RotatingPart::new(&full, AXIS, [0.0; 3], &options(n_full)).unwrap();
    let mut at_09: Option<(Vec<f64>, f64)> = None;
    for factor in [0.0, 0.5, 0.9] {
        let w = factor * w_ref;
        let hs: Vec<HarmonicModes> = (0..=N / 2)
            .map(|k| harmonic_modes(rs.modes(k, w).unwrap(), "comparison speed (L3)"))
            .collect();
        for h in &hs {
            println!(
                "{factor} omega_ref, k = {}: multiplicity {}, f = {:?} Hz, splits {:?}, shift {:e}, iterations {}",
                h.harmonic, h.multiplicity, h.frequencies, h.pair_splits, h.shift, h.iterations
            );
        }
        let (below, lambda_c) = below_cutoff(&hs);
        let fr = match fp.modes(w).unwrap() {
            SpeedModes::Modes(r) => r,
            SpeedModes::LossOfStiffness(l) => panic!("defective set-up: the full rotor loses stiffness at {w}: {l:?}"),
        };
        println!(
            "{factor} omega_ref full: {} unknowns, {} modes, shift {:e}, iterations {}, largest {:e}, cut-off {lambda_c:e}, n_c {}",
            fr.unknowns,
            fr.eigenvalues.len(),
            fr.shift,
            fr.iterations,
            fr.eigenvalues.last().unwrap(),
            below.len()
        );
        assert!(
            *fr.eigenvalues.last().unwrap() > lambda_c,
            "defective set-up: the full model's computed modes do not pass the cut-off"
        );
        let near = below
            .iter()
            .chain(fr.eigenvalues.iter())
            .map(|l| (l / lambda_c - 1.0).abs())
            .fold(f64::INFINITY, f64::min);
        assert!(
            near >= 1e-7,
            "defective set-up: an eigenvalue within 1e-7 of the cut-off"
        );
        let full_below: Vec<f64> = fr.eigenvalues.iter().copied().filter(|&l| l < lambda_c).collect();
        ch.holds(
            &format!("C1 {factor} omega_ref count"),
            full_below.len() == below.len(),
            &format!("full count {} = sector count {}", full_below.len(), below.len()),
        );
        ch.le(
            &format!("C1 {factor} omega_ref frequencies"),
            worst_relative(&below, &full_below),
            1e-8,
        );
        let split = hs
            .iter()
            .filter(|h| h.multiplicity == 2)
            .flat_map(|h| h.pair_splits.iter().copied())
            .fold(0.0f64, f64::max);
        ch.le(&format!("C1b {factor} omega_ref pair split"), split, 1e-8);
        // C2: the Rayleigh quotient of each expanded sector mode on the full rotor's own K_eff and M.
        let keff = fp.operator(w);
        let mut worst: f64 = 0.0;
        for h in &hs {
            let mut wk: f64 = 0.0;
            for (i, mode) in h.modes.iter().enumerate() {
                let x = rotor.expand(&sdofs, &fdofs, h.harmonic, mode);
                let rq = quad(&keff, &x) / quad(fp.mass(), &x);
                wk = wk.max((rq / h.eigenvalues[i] - 1.0).abs());
            }
            println!("C2 {factor} omega_ref k = {}: {wk:.3e}", h.harmonic);
            worst = worst.max(wk);
        }
        ch.le(&format!("C2 {factor} omega_ref family quotients"), worst, 1e-8);
        if factor == 0.9 {
            ch.holds(
                "L3",
                true,
                "no index and not the full rotor loses stiffness at 0.9 omega_ref",
            );
            at_09 = Some((full_below, lambda_c));
        }
    }
    // N1: the sector without the pre-stress against the full rotor with it, at 0.9 omega_ref.
    let (full_below, _) = at_09.unwrap();
    let w = 0.9 * w_ref;
    let rs_off = RotatingSector::new(&sector, &options(MODES).stress_stiffening(false)).unwrap();
    let hs_off: Vec<HarmonicModes> = (0..=N / 2)
        .map(|k| harmonic_modes(rs_off.modes(k, w).unwrap(), "N1"))
        .collect();
    let mut off: Vec<f64> = hs_off.iter().flat_map(|h| h.eigenvalues.iter().copied()).collect();
    off.sort_by(f64::total_cmp);
    let n1 = worst_relative(&off, &full_below);
    println!("N1: {n1:.3e} (expected > 1e-3)");
    ch.holds(
        "N1",
        n1 > 1e-3,
        "the sector without K_sigma differs from the full rotor by more than 1e-3",
    );
    // N2: the sector at rest against the full rotor at 0.9 omega_ref.
    let mut r0: Vec<f64> = rest_modes.iter().flat_map(|h| h.eigenvalues.iter().copied()).collect();
    r0.sort_by(f64::total_cmp);
    let n2 = worst_relative(&r0, &full_below);
    println!("N2: {n2:.3e} (expected > 1e-3)");
    ch.holds(
        "N2",
        n2 > 1e-3,
        "the rest spectrum differs from the full rotor at speed by more than 1e-3",
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ C3

fn same_bits(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn c3_zero_speed_bits_and_continuity() {
    let mut ch = Checks::new();
    let sector = d_sector();
    let rs = RotatingSector::new(&sector, &options(MODES)).unwrap();
    let (rest_modes, w_ref) = rest(&rs);
    for (k, h) in rest_modes.iter().enumerate() {
        let r = modal_analysis_harmonic(&sector, &[], k, &equivalence_options(MODES)).unwrap();
        let ok = same_bits(&h.eigenvalues, &r.eigenvalues)
            && same_bits(&h.values, &r.values)
            && same_bits(&h.frequencies, &r.frequencies)
            && same_bits(&h.pair_splits, &r.pair_splits)
            && same_bits(&h.residuals, &r.residuals)
            && h.modes.len() == r.modes.len()
            && h.modes
                .iter()
                .zip(&r.modes)
                .all(|(a, b)| same_bits(&a.cos, &b.cos) && same_bits(&a.sin, &b.sin))
            && h.iterations == r.iterations
            && h.shift.to_bits() == r.shift.to_bits()
            && h.unknowns == r.unknowns
            && h.multiplicity == r.multiplicity
            && *h == r;
        ch.holds(
            &format!("C3 k = {k}"),
            ok,
            "modes(k, 0) has the bits of modal_analysis_harmonic",
        );
    }
    // C3c: a tiny speed against rest.
    let w = 1e-6 * w_ref;
    let mut worst: f64 = 0.0;
    for (k, h0) in rest_modes.iter().enumerate() {
        let h = harmonic_modes(rs.modes(k, w).unwrap(), "C3c");
        for (a, b) in h.values.iter().zip(&h0.values) {
            worst = worst.max((a / b - 1.0).abs());
        }
    }
    ch.le("C3c 1e-6 omega_ref against rest", worst, 1e-8);
    // C3d: index 0 at speed against RotatingPart (ADR 0403) on the same sector.
    let w = 0.5 * w_ref;
    let h = harmonic_modes(rs.modes(0, w).unwrap(), "C3d");
    let part = RotatingPart::new(&sector, AXIS, [0.0; 3], &options(MODES)).unwrap();
    let r = match part.modes(w).unwrap() {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("C3d: {l:?}"),
    };
    println!("C3d: {:?} against {:?}", h.eigenvalues, r.eigenvalues);
    ch.holds(
        "C3d",
        same_bits(&h.eigenvalues, &r.eigenvalues),
        "index 0 at 0.5 omega_ref has the bits of RotatingPart::modes",
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ L1, L2

#[test]
fn l1_l2_loss_of_stiffness_per_index() {
    let mut ch = Checks::new();
    let sector = d_sector();
    let rs0 = RotatingSector::new(&sector, &options(MODES)).unwrap();
    let (rest_modes, w_ref) = rest(&rs0);
    let (w_l, k_star, stiffening) = loss_speed(&rs0, &rest_modes);
    println!(
        "Omega_L = {w_l:.9e} rad/s = {:.6} omega_ref, k* = {k_star}, stress stiffening {stiffening}",
        w_l / w_ref
    );
    assert!(w_l > 0.9 * w_ref, "defective set-up: Omega_L <= 0.9 omega_ref");
    let opts = options(MODES).stress_stiffening(stiffening);
    let rs = RotatingSector::new(&sector, &opts).unwrap();
    let mut sum = 0usize;
    let mut even = true;
    let mut star_ok = false;
    for k in 0..=N / 2 {
        match rs.modes(k, w_l).unwrap() {
            HarmonicSpeedModes::Modes(h) => {
                println!("Omega_L k = {k}: modes, f = {:?} Hz", h.frequencies);
                if k == k_star {
                    println!("L1: index k* has modes");
                }
            }
            HarmonicSpeedModes::LossOfStiffness(l) => {
                println!("Omega_L k = {k}: loss {l:?}");
                let neg = l.loss.negative.expect("a negative count (no zero pivot)");
                sum += neg;
                if l.multiplicity == 2 && neg % 2 != 0 {
                    even = false;
                }
                if k == k_star {
                    star_ok = l.harmonic == k_star
                        && l.loss.speed.to_bits() == w_l.to_bits()
                        && neg >= if l.multiplicity == 2 { 2 } else { 1 };
                }
            }
        }
    }
    ch.holds(
        "L1",
        star_ok,
        "index k* reports a loss of stiffness with its index, the speed's bits and a count",
    );
    let (_, full) = full_of(&sector);
    let fp = RotatingPart::new(&full, AXIS, [0.0; 3], &opts).unwrap();
    let full_neg = match fp.modes(w_l).unwrap() {
        SpeedModes::LossOfStiffness(l) => l.negative,
        SpeedModes::Modes(_) => Some(0),
    };
    println!("L2: full rotor negative count {full_neg:?}, sum over the indices {sum}");
    ch.holds(
        "L2 identity",
        full_neg == Some(sum),
        "full negative count = sum of the index counts",
    );
    ch.holds("L2 even", even, "every pair count is even");
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ P1 to P5

#[test]
#[ignore = "record of ADR 0407: P2 missed in the registered run (0.8952 < 0.9, in the single family k = N/2 = 6, whose eigenvalues come in exactly degenerate pairs on the axisymmetric disc, so the vector assurance compares arbitrary bases); every other check of this test met; corrected tracking by amendment 1, p1_p2b_p3_p5_campbell_by_family_amended below"]
fn p1_p2_p3_p5_campbell_by_family_known_miss() {
    let mut ch = Checks::new();
    let sector = d_sector();
    let rs0 = RotatingSector::new(&sector, &options(MODES)).unwrap();
    let (rest_modes, w_ref) = rest(&rs0);
    let (w_l, k_star, stiffening) = loss_speed(&rs0, &rest_modes);
    println!("omega_ref = {w_ref:e}, Omega_L = {w_l:e}, k* = {k_star}, stiffening {stiffening}");
    assert!(w_l > 0.9 * w_ref, "defective set-up: Omega_L <= 0.9 omega_ref");
    let rs = RotatingSector::new(&sector, &options(MODES).stress_stiffening(stiffening)).unwrap();
    let mut speeds: Vec<f64> = (0..10).map(|j| j as f64 * 0.1 * w_ref).collect();
    speeds.push(w_l);
    let harmonics: Vec<usize> = (0..=N / 2).collect();
    let table = rs.campbell(&harmonics, &speeds).unwrap();
    // P1.
    let mut shape = table.families.len() == N / 2 + 1 && table.sectors == N;
    for (i, f) in table.families.iter().enumerate() {
        let mult = if i == 0 || 2 * i == N { 1 } else { 2 };
        shape &= f.harmonic == i && f.multiplicity == mult && f.table.tracks.len() == MODES;
        let finite = f.table.tracks.iter().all(|t| {
            t.eigenvalues.iter().flatten().all(|v| v.is_finite())
                && t.frequencies.iter().flatten().all(|v| v.is_finite())
                && t.mac.iter().flatten().all(|v| v.is_finite())
        });
        shape &= finite;
        println!(
            "family k = {}: multiplicity {}, {} tracks, losses {:?}",
            f.harmonic,
            f.multiplicity,
            f.table.tracks.len(),
            f.table.losses
        );
    }
    let star = &table.families[k_star].table;
    let star_loss = star.losses.iter().any(|l| l.speed.to_bits() == w_l.to_bits())
        && star.tracks.iter().all(|t| t.frequencies[10].is_none());
    ch.holds(
        "P1 shape",
        shape,
        "7 families in index order, multiplicities, 4 tracks each, every number finite",
    );
    ch.holds(
        "P1 loss",
        star_loss,
        "the loss of k* at Omega_L listed in its family, frequencies None there",
    );
    // C3b: the speed-0 column.
    let mut c3b = true;
    for (f, h) in table.families.iter().zip(&rest_modes) {
        for (t, tr) in f.table.tracks.iter().enumerate() {
            c3b &= tr.eigenvalues[0].map(f64::to_bits) == Some(h.values[t].to_bits());
        }
    }
    ch.holds("C3b", c3b, "the speed-0 column has the bits of modal_analysis_harmonic");
    // P2 and N3.
    let mut min_mac = f64::INFINITY;
    for f in &table.families {
        for t in &f.table.tracks {
            for j in 1..10 {
                if let Some(v) = t.mac[j] {
                    min_mac = min_mac.min(v);
                }
            }
        }
    }
    ch.ge("P2 smallest track assurance at 0.1 .. 0.9 omega_ref", min_mac, 0.9);
    let mut n3 = f64::INFINITY;
    for f in table.families.iter().filter(|f| f.multiplicity == 2) {
        for t in &f.table.tracks {
            for j in 1..10 {
                if let (Some(a), Some(b)) = (&t.modes[j - 1], &t.modes[j]) {
                    n3 = n3.min(subspace_mac(
                        rs.part().mass(),
                        std::slice::from_ref(a),
                        std::slice::from_ref(b),
                    ));
                }
            }
        }
    }
    println!("N3: smallest vector MAC of the pairs' first vectors over the table: {n3:.6e} (reported)");
    // P3.
    let mut per_family = vec![0usize; N / 2 + 1];
    let mut worst: f64 = 0.0;
    let mut count = 0;
    for order in 1..=24u32 {
        for c in table.crossings(order) {
            if c.crossing.speed > 0.9 * w_ref {
                println!(
                    "crossing above 0.9 omega_ref (reported): k = {} track {} order {} at {:.6e} rad/s",
                    c.harmonic, c.crossing.track, order, c.crossing.speed
                );
                continue;
            }
            per_family[c.harmonic] += 1;
            count += 1;
            let h = harmonic_modes(rs.modes(c.harmonic, c.crossing.speed).unwrap(), "P3 direct solve");
            let target = (f64::from(order) * c.crossing.speed).powi(2);
            let near = h
                .values
                .iter()
                .copied()
                .min_by(|a, b| (a - target).abs().total_cmp(&(b - target).abs()))
                .unwrap();
            let rel = (near.sqrt() / (f64::from(order) * c.crossing.speed) - 1.0).abs();
            println!(
                "crossing k = {} track {} order {order} at {:.6e} rad/s, {:.6e} Hz, excited {}: direct {rel:.3e}",
                c.harmonic, c.crossing.track, c.crossing.speed, c.crossing.frequency, c.excited
            );
            worst = worst.max(rel);
        }
    }
    println!("P3: {count} crossings below 0.9 omega_ref, per family {per_family:?}");
    ch.holds(
        "P3 every family",
        per_family.iter().all(|&c| c >= 1),
        "at least one crossing per family",
    );
    ch.le("P3 crossings against direct solves", worst, 1e-2);
    // P5.
    let csv = table.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    let mut csv_ok = lines.len() == 1 + (N / 2 + 1) * speeds.len() && lines[0].starts_with("harmonic,speed_rad_s");
    for (fi, f) in table.families.iter().enumerate() {
        for j in 0..speeds.len() {
            let fields: Vec<&str> = lines[1 + fi * speeds.len() + j].split(',').collect();
            csv_ok &= fields[0] == f.harmonic.to_string();
            csv_ok &= fields[1].parse::<f64>().map(f64::to_bits) == Ok(speeds[j].to_bits());
            for (t, tr) in f.table.tracks.iter().enumerate() {
                csv_ok &= match tr.frequencies[j] {
                    Some(v) => fields[3 + t].parse::<f64>().map(f64::to_bits) == Ok(v.to_bits()),
                    None => fields[3 + t].is_empty(),
                };
            }
        }
    }
    let orders: Vec<u32> = (1..=24).collect();
    let json = table.to_json(&orders);
    let json_ok = json.matches("\"harmonic\": ").count() > N / 2
        && !json.contains("NaN")
        && !json.contains("inf")
        && json.contains("null")
        && json.contains("\"excited\": true");
    println!("{}", &csv[..csv.len().min(600)]);
    ch.holds(
        "P5 CSV",
        csv_ok,
        "rows, harmonic column, speeds and frequencies parse back by to_bits",
    );
    ch.holds(
        "P5 JSON",
        json_ok,
        "families, no NaN or inf, null at the loss, flagged crossings",
    );
    ch.finish();
}

/// ADR 0407 amendment 1, P2b: the record above repeated with the cluster tracking.
#[test]
fn p1_p2b_p3_p5_campbell_by_family_amended() {
    let mut ch = Checks::new();
    let sector = d_sector();
    let rs0 = RotatingSector::new(&sector, &options(MODES)).unwrap();
    let (rest_modes, w_ref) = rest(&rs0);
    let (w_l, k_star, stiffening) = loss_speed(&rs0, &rest_modes);
    println!("omega_ref = {w_ref:e}, Omega_L = {w_l:e}, k* = {k_star}, stiffening {stiffening}");
    assert!(w_l > 0.9 * w_ref, "defective set-up: Omega_L <= 0.9 omega_ref");
    let rs = RotatingSector::new(&sector, &options(MODES).stress_stiffening(stiffening)).unwrap();
    let mut speeds: Vec<f64> = (0..10).map(|j| j as f64 * 0.1 * w_ref).collect();
    speeds.push(w_l);
    let harmonics: Vec<usize> = (0..=N / 2).collect();
    let table = rs.campbell(&harmonics, &speeds).unwrap();
    // P1.
    let mut shape = table.families.len() == N / 2 + 1 && table.sectors == N;
    for (i, f) in table.families.iter().enumerate() {
        let mult = if i == 0 || 2 * i == N { 1 } else { 2 };
        shape &= f.harmonic == i && f.multiplicity == mult && f.table.tracks.len() == MODES;
        let finite = f.table.tracks.iter().all(|t| {
            t.eigenvalues.iter().flatten().all(|v| v.is_finite())
                && t.frequencies.iter().flatten().all(|v| v.is_finite())
                && t.mac.iter().flatten().all(|v| v.is_finite())
        });
        shape &= finite;
        println!(
            "family k = {}: multiplicity {}, {} tracks, losses {:?}",
            f.harmonic,
            f.multiplicity,
            f.table.tracks.len(),
            f.table.losses
        );
    }
    let star = &table.families[k_star].table;
    let star_loss = star.losses.iter().any(|l| l.speed.to_bits() == w_l.to_bits())
        && star.tracks.iter().all(|t| t.frequencies[10].is_none());
    ch.holds(
        "P1 shape",
        shape,
        "7 families in index order, multiplicities, 4 tracks each, every number finite",
    );
    ch.holds(
        "P1 loss",
        star_loss,
        "the loss of k* at Omega_L listed in its family, frequencies None there",
    );
    // C3b: the speed-0 column.
    let mut c3b = true;
    for (f, h) in table.families.iter().zip(&rest_modes) {
        for (t, tr) in f.table.tracks.iter().enumerate() {
            c3b &= tr.eigenvalues[0].map(f64::to_bits) == Some(h.values[t].to_bits());
        }
    }
    ch.holds("C3b", c3b, "the speed-0 column has the bits of modal_analysis_harmonic");
    // P2 and N3.
    let mut min_mac = f64::INFINITY;
    for f in &table.families {
        for t in &f.table.tracks {
            for j in 1..10 {
                if let Some(v) = t.mac[j] {
                    min_mac = min_mac.min(v);
                }
            }
        }
    }
    ch.ge("P2b smallest track assurance at 0.1 .. 0.9 omega_ref", min_mac, 0.9);
    let mut n3 = f64::INFINITY;
    for f in table.families.iter().filter(|f| f.multiplicity == 2) {
        for t in &f.table.tracks {
            for j in 1..10 {
                if let (Some(a), Some(b)) = (&t.modes[j - 1], &t.modes[j]) {
                    n3 = n3.min(subspace_mac(
                        rs.part().mass(),
                        std::slice::from_ref(a),
                        std::slice::from_ref(b),
                    ));
                }
            }
        }
    }
    println!("N3: smallest vector MAC of the pairs' first vectors over the table: {n3:.6e} (reported)");
    // P3.
    let mut per_family = vec![0usize; N / 2 + 1];
    let mut worst: f64 = 0.0;
    let mut count = 0;
    for order in 1..=24u32 {
        for c in table.crossings(order) {
            if c.crossing.speed > 0.9 * w_ref {
                println!(
                    "crossing above 0.9 omega_ref (reported): k = {} track {} order {} at {:.6e} rad/s",
                    c.harmonic, c.crossing.track, order, c.crossing.speed
                );
                continue;
            }
            per_family[c.harmonic] += 1;
            count += 1;
            let h = harmonic_modes(rs.modes(c.harmonic, c.crossing.speed).unwrap(), "P3 direct solve");
            let target = (f64::from(order) * c.crossing.speed).powi(2);
            let near = h
                .values
                .iter()
                .copied()
                .min_by(|a, b| (a - target).abs().total_cmp(&(b - target).abs()))
                .unwrap();
            let rel = (near.sqrt() / (f64::from(order) * c.crossing.speed) - 1.0).abs();
            println!(
                "crossing k = {} track {} order {order} at {:.6e} rad/s, {:.6e} Hz, excited {}: direct {rel:.3e}",
                c.harmonic, c.crossing.track, c.crossing.speed, c.crossing.frequency, c.excited
            );
            worst = worst.max(rel);
        }
    }
    println!("P3: {count} crossings below 0.9 omega_ref, per family {per_family:?}");
    ch.holds(
        "P3 every family",
        per_family.iter().all(|&c| c >= 1),
        "at least one crossing per family",
    );
    ch.le("P3 crossings against direct solves", worst, 1e-2);
    // P5.
    let csv = table.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    let mut csv_ok = lines.len() == 1 + (N / 2 + 1) * speeds.len() && lines[0].starts_with("harmonic,speed_rad_s");
    for (fi, f) in table.families.iter().enumerate() {
        for j in 0..speeds.len() {
            let fields: Vec<&str> = lines[1 + fi * speeds.len() + j].split(',').collect();
            csv_ok &= fields[0] == f.harmonic.to_string();
            csv_ok &= fields[1].parse::<f64>().map(f64::to_bits) == Ok(speeds[j].to_bits());
            for (t, tr) in f.table.tracks.iter().enumerate() {
                csv_ok &= match tr.frequencies[j] {
                    Some(v) => fields[3 + t].parse::<f64>().map(f64::to_bits) == Ok(v.to_bits()),
                    None => fields[3 + t].is_empty(),
                };
            }
        }
    }
    let orders: Vec<u32> = (1..=24).collect();
    let json = table.to_json(&orders);
    let json_ok = json.matches("\"harmonic\": ").count() > N / 2
        && !json.contains("NaN")
        && !json.contains("inf")
        && json.contains("null")
        && json.contains("\"excited\": true");
    println!("{}", &csv[..csv.len().min(600)]);
    ch.holds(
        "P5 CSV",
        csv_ok,
        "rows, harmonic column, speeds and frequencies parse back by to_bits",
    );
    ch.holds(
        "P5 JSON",
        json_ok,
        "families, no NaN or inf, null at the loss, flagged crossings",
    );
    ch.finish();
}

/// ADR 0407 amendment 1, P2c: the Campbell table with both effects on over 0 .. 0.9 omega_ref.
#[test]
fn p2c_campbell_with_both_effects() {
    let mut ch = Checks::new();
    let sector = d_sector();
    let rs = RotatingSector::new(&sector, &options(MODES)).unwrap();
    let (_, w_ref) = rest(&rs);
    let speeds: Vec<f64> = (0..10).map(|j| j as f64 * 0.1 * w_ref).collect();
    let harmonics: Vec<usize> = (0..=N / 2).collect();
    let table = rs.campbell(&harmonics, &speeds).unwrap();
    let mut shape = table.families.len() == N / 2 + 1;
    let mut min_mac = f64::INFINITY;
    for f in &table.families {
        shape &= f.table.tracks.len() == MODES && f.table.losses.is_empty();
        let mut fam = f64::INFINITY;
        for t in &f.table.tracks {
            shape &= t.eigenvalues.iter().all(|v| v.is_some_and(f64::is_finite));
            shape &= t.mac.iter().flatten().all(|v| v.is_finite());
            for v in t.mac.iter().flatten() {
                fam = fam.min(*v);
            }
        }
        println!("P2c family k = {}: smallest assurance {fam:.6e}", f.harmonic);
        min_mac = min_mac.min(fam);
    }
    ch.holds(
        "P2c shape",
        shape,
        "7 families, 4 tracks each, no loss, every number finite",
    );
    ch.ge("P2c smallest track assurance", min_mac, 0.9);
    let mut per_family = vec![0usize; N / 2 + 1];
    let mut worst: f64 = 0.0;
    for order in 1..=24u32 {
        for c in table.crossings(order) {
            per_family[c.harmonic] += 1;
            let h = harmonic_modes(rs.modes(c.harmonic, c.crossing.speed).unwrap(), "P2c direct solve");
            let target = (f64::from(order) * c.crossing.speed).powi(2);
            let near = h
                .values
                .iter()
                .copied()
                .min_by(|a, b| (a - target).abs().total_cmp(&(b - target).abs()))
                .unwrap();
            worst = worst.max((near.sqrt() / (f64::from(order) * c.crossing.speed) - 1.0).abs());
        }
    }
    println!("P2c: crossings per family {per_family:?}");
    ch.holds(
        "P2c every family",
        per_family.iter().all(|&c| c >= 1),
        "at least one crossing per family",
    );
    ch.le("P2c crossings against direct solves", worst, 1e-2);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ P4

#[test]
fn p4_engine_order_excitation() {
    let mut ch = Checks::new();
    let a = alpha();
    let mut all = true;
    for e in 0..=36u32 {
        for k in 0..=N / 2 {
            let proj = |d: f64| {
                let (mut re, mut im) = (0.0, 0.0);
                for j in 0..N {
                    re += (d * j as f64 * a).cos();
                    im += (d * j as f64 * a).sin();
                }
                (re * re + im * im).sqrt()
            };
            let s = proj(f64::from(e) - k as f64) + proj(f64::from(e) + k as f64);
            let expected = s > 6.0;
            if engine_order_excites(e, k, N) != expected {
                println!(
                    "P4: order {e}, index {k}: projection {s:e}, excites {}",
                    engine_order_excites(e, k, N)
                );
                all = false;
            }
        }
    }
    ch.holds(
        "P4",
        all,
        "engine_order_excites equals the projection rule for E = 0 .. 36, k = 0 .. 6",
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------ R1

fn key_of(e: Option<FemError>, what: &str) -> &'static str {
    match e {
        Some(FemError::InvalidValue { key, reason }) => {
            println!("{what}: refused, key {key}: {reason}");
            key
        }
        other => panic!("{what}: expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn r1_refusals() {
    let mut ch = Checks::new();
    let mut plain = model_on(d_mesh());
    clamp(&mut plain, "xmin");
    let k = key_of(
        RotatingSector::new(&plain, &options(MODES)).err(),
        "no cyclic constraint",
    );
    ch.holds(
        "R1 no cyclic",
        k == "cyclic",
        "a model without a cyclic constraint is refused",
    );
    let sector = d_sector();
    let rs = RotatingSector::new(&sector, &options(MODES)).unwrap();
    ch.holds(
        "R1 harmonic",
        key_of(rs.modes(7, 100.0).err(), "harmonic 7") == "harmonic",
        "harmonic above N/2",
    );
    ch.holds(
        "R1 speed",
        key_of(rs.modes(1, -1.0).err(), "speed -1") == "speed",
        "negative speed",
    );
    ch.holds(
        "R1 speed NaN",
        key_of(rs.modes(1, f64::NAN).err(), "speed NaN") == "speed",
        "NaN speed",
    );
    ch.holds(
        "R1 speeds",
        key_of(rs.campbell(&[0, 1], &[2.0, 1.0]).err(), "decreasing speeds") == "speeds",
        "decreasing speeds",
    );
    ch.holds(
        "R1 harmonics empty",
        key_of(rs.campbell(&[], &[1.0]).err(), "no harmonics") == "harmonics",
        "empty index list",
    );
    ch.holds(
        "R1 harmonics decreasing",
        key_of(rs.campbell(&[2, 1], &[1.0]).err(), "decreasing harmonics") == "harmonics",
        "decreasing index list",
    );
    ch.holds(
        "R1 harmonics above",
        key_of(rs.campbell(&[7], &[1.0]).err(), "harmonic 7 in the list") == "harmonics",
        "index above N/2",
    );
    let rs0 = RotatingSector::new(&sector, &options(0)).unwrap();
    ch.holds(
        "R1 modes",
        key_of(rs0.modes(1, 100.0).err(), "0 modes") == "modes",
        "0 modes",
    );
    ch.finish();
}
