//! F-511 (ADR 0420): whirl stability of a cyclic sector above the loss of stiffness: each harmonic index by the state matrix
//! of ADR 0416 on its real cos/sin problem, the flag and the growth rate per index, the union of the indices against the full
//! rotor of the disc sector of ADR 0411 at a speed above its first loss of stiffness. Checks D to R1 and the controls N1, N2
//! of the registration (U2 is the unit test of `rotating::whirl::state::sector`).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::cyclic::{full_rotor, FullRotor, SectorMode};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{
    HarmonicStateWhirl, HarmonicWhirlSpeedModes, RotatingOptions, RotatingPart, RotatingSector, Stability, StateWhirl,
    STABILITY_TOLERANCE,
};
use rcfd_fem::{Constraint, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

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
    fn gt(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{id}: {value:.3e} (expected > {limit:.0e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} <= {limit:e}"));
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

const E: f64 = 2.0e11;
const NU: f64 = 0.3;
const RHO: f64 = 7850.0;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const N: usize = 6;

fn material() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

// ------------------------------------------------------------------------------------------------------ DS (ADR 0411)

fn alpha() -> f64 {
    2.0 * PI / N as f64
}

fn ds_mesh() -> SolidMesh {
    let b = hex_box([0.25, 0.0, -0.1], [0.75, alpha(), 0.2], [2, 1, 1]).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

fn model_on(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m
}

fn clamp(m: &mut SolidModel, set: &str) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn ds_sector() -> SolidModel {
    let mut m = model_on(ds_mesh());
    clamp(&mut m, "xmin");
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, N))
        .unwrap();
    m
}

fn full_of(sector: &SolidModel) -> (FullRotor, SolidModel) {
    let rotor = full_rotor(sector).unwrap();
    let mut full = model_on(rotor.mesh().clone());
    clamp(&mut full, "xmin");
    (rotor, full)
}

fn ds_modal() -> ModalOptions {
    let mut o = ModalOptions::new(4).tolerance(1e-9);
    o.max_iterations = 400;
    o
}

fn ds_options(coriolis: bool, stress: bool) -> RotatingOptions {
    RotatingOptions::new(4)
        .modal(ds_modal())
        .coriolis(coriolis)
        .stress_stiffening(stress)
}

fn omega_ref(rs: &RotatingSector) -> f64 {
    (0..=N / 2)
        .map(|k| match rs.whirl_modes(k, 0.0).unwrap() {
            HarmonicWhirlSpeedModes::Modes { modes, .. } => modes.modes[0].omega,
            HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: loss at rest {l:?}"),
        })
        .fold(f64::INFINITY, f64::min)
}

fn lost(rs: &RotatingSector, k: usize, w: f64) -> bool {
    rs.whirl_modes(k, w).unwrap().loss().is_some()
}

fn count_of(r: &StateWhirl) -> Option<usize> {
    match r.loss {
        None => Some(0),
        Some(l) => l.negative,
    }
}

fn cabs(s: [f64; 2]) -> f64 {
    s[0].hypot(s[1])
}

/// The global greedy matching of the registration: `(i, j)` pairs of `a[i]` and `b[j]` by increasing distance.
fn matching(a: &[[f64; 2]], b: &[[f64; 2]]) -> Vec<(usize, usize)> {
    let mut d: Vec<(f64, usize, usize)> = Vec::with_capacity(a.len() * b.len());
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            d.push(((x[0] - y[0]).hypot(x[1] - y[1]), i, j));
        }
    }
    d.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)).then(p.2.cmp(&q.2)));
    let (mut ta, mut tb) = (vec![false; a.len()], vec![false; b.len()]);
    let mut out = Vec::new();
    for (_, i, j) in d {
        if !ta[i] && !tb[j] {
            ta[i] = true;
            tb[j] = true;
            out.push((i, j));
        }
    }
    out
}

/// The union of the indices' eigenvalues with the index of each.
fn union_of(hs: &[HarmonicStateWhirl]) -> (Vec<[f64; 2]>, Vec<usize>) {
    let mut all = Vec::new();
    let mut idx = Vec::new();
    for h in hs {
        for s in &h.state.eigenvalues {
            all.push(*s);
            idx.push(h.harmonic);
        }
    }
    (all, idx)
}

fn worst_matched(sector: &[[f64; 2]], full: &[[f64; 2]]) -> f64 {
    matching(sector, full)
        .iter()
        .map(|&(i, j)| (sector[i][0] - full[j][0]).hypot(sector[i][1] - full[j][1]) / cabs(full[j]))
        .fold(0.0f64, f64::max)
}

/// The D3 flag of ADR 0416 on a set of eigenvalues.
fn flag_of(ev: &[[f64; 2]]) -> Stability {
    let smax = ev.iter().map(|s| cabs(*s)).fold(0.0f64, f64::max);
    let remax = ev.iter().map(|s| s[0].abs()).fold(0.0f64, f64::max);
    if smax == 0.0 || remax <= STABILITY_TOLERANCE * smax {
        Stability::Stable
    } else {
        Stability::Unstable {
            growth: ev.iter().map(|s| s[0]).fold(f64::NEG_INFINITY, f64::max),
        }
    }
}

fn growth_of(s: Stability) -> Option<f64> {
    match s {
        Stability::Stable => None,
        Stability::Unstable { growth } => Some(growth),
    }
}

fn same_flag(a: Stability, b: Stability) -> bool {
    a.is_stable() == b.is_stable()
}

// ------------------------------------------------------------------------------------------------------ X1, W2, Q1 at 0.8

#[test]
fn x1_w2_q1_positive_definite_speed() {
    let mut ch = Checks::new();
    let sector = ds_sector();
    let rs = RotatingSector::new(&sector, &ds_options(true, true)).unwrap();
    let w_ref = omega_ref(&rs);
    let w = 0.8 * w_ref;
    println!("omega_ref = {w_ref:.9e} rad/s, speed {w:.9e}");
    let (mut x1, mut w2o, mut w2t, mut q1) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for k in 0..=N / 2 {
        let h = rs.whirl_state(k, w).unwrap();
        let wm = match rs.whirl_modes(k, w).unwrap() {
            HarmonicWhirlSpeedModes::Modes { modes, .. } => modes,
            HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: loss at 0.8 omega_ref {l:?}"),
        };
        ch.holds(&format!("X1 k = {k} loss"), h.state.loss.is_none(), "None");
        ch.holds(&format!("X1 k = {k} flag"), h.state.stability.is_stable(), "Stable");
        let mut om: Vec<f64> = h
            .state
            .eigenvalues
            .iter()
            .filter(|s| s[1] > 0.0)
            .map(|s| s[1])
            .collect();
        om.sort_by(f64::total_cmp);
        ch.holds(
            &format!("X1 k = {k} count"),
            om.len() == wm.frequencies_all.len() && om.len() == h.state.unknowns,
            &format!("{} = {} = {}", om.len(), wm.frequencies_all.len(), h.state.unknowns),
        );
        for (a, b) in om.iter().zip(&wm.frequencies_all) {
            x1 = x1.max((a / b - 1.0).abs());
        }
        for x in &h.state.modes {
            q1 = q1.max(x.residual);
        }
        if k == 1 || k == 2 {
            for (i, x) in h.state.modes.iter().enumerate() {
                let y = &wm.modes[i];
                w2o = w2o.max((x.omega / y.omega - 1.0).abs());
                w2t = w2t.max((h.waves[i].unwrap() - y.wave.unwrap()).abs());
            }
            println!(
                "k = {k}: omega {:?}, wave {:?}",
                h.state.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
                h.waves
            );
        }
    }
    ch.le("X1 frequencies against whirl_modes", x1, 1e-9);
    ch.le("W2 omega of the modes against whirl_modes", w2o, 1e-9);
    ch.le("W2 wave sense against whirl_modes", w2t, 1e-8);
    ch.le("Q1 residuals at 0.8 omega_ref", q1, 1e-9);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ above the first loss

#[test]
fn d_b1_l1_u1_f1_p1_q1_e1_w1_t1_n1_n2_above_the_first_loss() {
    let mut ch = Checks::new();
    let sector = ds_sector();
    let rs_both = RotatingSector::new(&sector, &ds_options(true, true)).unwrap();
    let w_ref = omega_ref(&rs_both);
    println!("D: omega_ref = {w_ref:.9e} rad/s");
    // D3 step 2: the bracket, both effects first, then the fallback.
    let bracket = |rs: &RotatingSector| -> Option<f64> {
        for f in [2.0, 4.0, 8.0, 16.0] {
            let w = f * w_ref;
            let ks: Vec<usize> = (0..=N / 2).filter(|&k| lost(rs, k, w)).collect();
            println!("D: at {f} omega_ref the indices {ks:?} have lost stiffness");
            if !ks.is_empty() {
                return Some(w);
            }
        }
        None
    };
    let (stress, hi) = match bracket(&rs_both) {
        Some(hi) => {
            println!("D: both effects on: a loss by {:.0} omega_ref", hi / w_ref);
            (true, hi)
        }
        None => {
            println!(
                "D: both effects on: no index loses definiteness up to 16 omega_ref; fallback stress_stiffening(false)"
            );
            let rs_off = RotatingSector::new(&sector, &ds_options(true, false)).unwrap();
            let hi = bracket(&rs_off).expect("defective set-up: no loss up to 16 omega_ref with the fallback");
            (false, hi)
        }
    };
    let rs = RotatingSector::new(&sector, &ds_options(true, stress)).unwrap();
    // D3 step 3: bisection per lost index.
    let mut w_l = f64::INFINITY;
    let mut k_star = 0;
    for k in 0..=N / 2 {
        if !lost(&rs, k, hi) {
            println!("D: index {k} has not lost at {hi:.6e}");
            continue;
        }
        let (mut lo, mut up) = (0.8 * w_ref, hi);
        assert!(!lost(&rs, k, lo), "defective set-up: index {k} lost at 0.8 omega_ref");
        for _ in 0..60 {
            let mid = 0.5 * (lo + up);
            if lost(&rs, k, mid) {
                up = mid;
            } else {
                lo = mid;
            }
        }
        println!("D: Omega_L,{k} = {up:.12e} rad/s = {:.9} omega_ref", up / w_ref);
        if up < w_l {
            w_l = up;
            k_star = k;
        }
    }
    let w_t = 1.25 * w_l;
    println!(
        "D: stress stiffening {stress}; Omega_L = {w_l:.12e} rad/s = {:.9} omega_ref, k* = {k_star}; Omega_T = {w_t:.12e}",
        w_l / w_ref
    );
    ch.holds("D Omega_L > 0.8 omega_ref", w_l > 0.8 * w_ref, "bound (i)");

    // The sector at Omega_T.
    let hs: Vec<HarmonicStateWhirl> = (0..=N / 2).map(|k| rs.whirl_state(k, w_t).unwrap()).collect();
    for h in &hs {
        println!(
            "k = {}: unknowns {}, loss {:?}, flag {:?}, real defect {:.3e}; modes (growth, omega): {:?}; whirl {:?}; wave {:?}",
            h.harmonic,
            h.state.unknowns,
            h.state.loss,
            h.state.stability,
            h.state.real_defect,
            h.state.modes.iter().map(|x| (x.growth, x.omega)).collect::<Vec<_>>(),
            h.state.modes.iter().map(|x| x.whirl).collect::<Vec<_>>(),
            h.waves
        );
    }
    let hk = &hs[k_star];
    let pair_star = k_star != 0 && 2 * k_star != N;
    let b1_count = match hk.state.loss {
        Some(l) => l.negative.is_some_and(|c| c >= 1 && (!pair_star || c % 2 == 0)),
        None => false,
    };
    ch.holds("B1 loss of k* at Omega_T", b1_count, &format!("{:?}", hk.state.loss));
    ch.holds(
        "B1 eigenvalues of k*",
        hk.state.eigenvalues.len() == 2 * hk.state.unknowns
            && hk
                .state
                .eigenvalues
                .iter()
                .all(|s| s[0].is_finite() && s[1].is_finite()),
        &format!(
            "{} finite of {} unknowns",
            hk.state.eigenvalues.len(),
            hk.state.unknowns
        ),
    );
    ch.holds("B1 whirl_modes still a loss", lost(&rs, k_star, w_t), "LossOfStiffness");

    // The full rotor.
    let (rotor, full) = full_of(&sector);
    let fp = RotatingPart::new(&full, AXIS, ORIGIN, &ds_options(true, stress)).unwrap();
    let fr = fp.whirl_state(w_t).unwrap();
    println!(
        "full rotor: unknowns {}, loss {:?}, flag {:?}, real defect {:.3e}, {} eigenvalues",
        fr.unknowns,
        fr.loss,
        fr.stability,
        fr.real_defect,
        fr.eigenvalues.len()
    );
    // L1
    let counts: Vec<Option<usize>> = hs.iter().map(|h| count_of(&h.state)).collect();
    let pair_even = hs
        .iter()
        .zip(&counts)
        .all(|(h, c)| h.multiplicity == 1 || c.is_some_and(|c| c % 2 == 0));
    let sum: Option<usize> = counts.iter().copied().sum();
    ch.holds(
        "L1 Sylvester count",
        sum.is_some() && sum == count_of(&fr) && pair_even,
        &format!("indices {counts:?}, sum {sum:?}, full {:?}", count_of(&fr)),
    );
    // U1
    let (all, idx) = union_of(&hs);
    ch.holds(
        "U1 count",
        all.len() == fr.eigenvalues.len(),
        &format!("{} = {}", all.len(), fr.eigenvalues.len()),
    );
    let pairs = matching(&all, &fr.eigenvalues);
    let mut u1: f64 = 0.0;
    let mut matched: Vec<Vec<[f64; 2]>> = vec![Vec::new(); N / 2 + 1];
    for &(i, j) in &pairs {
        let d = (all[i][0] - fr.eigenvalues[j][0]).hypot(all[i][1] - fr.eigenvalues[j][1]) / cabs(fr.eigenvalues[j]);
        u1 = u1.max(d);
        matched[idx[i]].push(fr.eigenvalues[j]);
    }
    let smin = fr.eigenvalues.iter().map(|s| cabs(*s)).fold(f64::INFINITY, f64::min);
    let smax = fr.eigenvalues.iter().map(|s| cabs(*s)).fold(0.0f64, f64::max);
    println!(
        "U1: {} matched pairs; full |s| from {smin:.6e} to {smax:.6e}",
        pairs.len()
    );
    ch.le("U1 union against the full rotor", u1, 1e-8);
    // F1
    let flags: Vec<Stability> = hs.iter().map(|h| h.state.stability).collect();
    let comb = Stability::combined(&flags);
    ch.holds(
        "F1 (i) full flag against the indices'",
        same_flag(fr.stability, comb),
        &format!("full {:?}, combined {comb:?}", fr.stability),
    );
    if let (Some(a), Some(b)) = (growth_of(fr.stability), growth_of(comb)) {
        ch.le("F1 (i) growth", (a / b - 1.0).abs(), 1e-8);
    }
    for (k, h) in hs.iter().enumerate() {
        let fk = flag_of(&matched[k]);
        ch.holds(
            &format!("F1 (ii) k = {k} flag"),
            same_flag(h.state.stability, fk),
            &format!("sector {:?}, full matched {fk:?}", h.state.stability),
        );
        if let (Some(a), Some(b)) = (growth_of(h.state.stability), growth_of(fk)) {
            ch.le(&format!("F1 (ii) k = {k} growth"), (a / b - 1.0).abs(), 1e-8);
        }
    }
    // P1
    let mut p1_applied = false;
    for h in &hs {
        if h.multiplicity == 1 {
            if let Some(c) = count_of(&h.state) {
                if c % 2 == 1 {
                    p1_applied = true;
                    ch.holds(
                        &format!("P1 k = {} odd count {c}", h.harmonic),
                        !h.state.stability.is_stable(),
                        "Unstable",
                    );
                }
            }
        }
    }
    if let Some(c) = count_of(&fr) {
        if c % 2 == 1 {
            p1_applied = true;
            ch.holds(
                &format!("P1 full rotor odd count {c}"),
                !fr.stability.is_stable(),
                "Unstable",
            );
        }
    }
    if !p1_applied {
        println!("P1: vacuous, no odd count");
    }
    // Q1, E1, W1
    let sdofs = sector.dof_map();
    let fdofs = full.dof_map();
    let fixed: Vec<bool> = {
        let mut f = vec![false; fdofs.n_equations()];
        for (eq, _) in prescribed_equations(&full, &[]).unwrap() {
            f[eq] = true;
        }
        f
    };
    let keff = fp.operator(w_t);
    let m = fp.mass();
    let c_full = fp.coriolis().unwrap();
    let n = sdofs.n_equations();
    let (mut q1, mut e1, mut w1) = (0.0f64, 0.0f64, 0.0f64);
    let mut separated = 0;
    for (k, h) in hs.iter().enumerate() {
        let pair = h.multiplicity == 2;
        for (i, x) in h.state.modes.iter().enumerate() {
            q1 = q1.max(x.residual);
            let split = |v: &[f64]| SectorMode {
                cos: v[..n].to_vec(),
                sin: if pair { v[n..].to_vec() } else { vec![0.0; n] },
            };
            let re = rotor.expand(&sdofs, &fdofs, k, &split(&x.re));
            let im = rotor.expand(&sdofs, &fdofs, k, &split(&x.im));
            let (kr, ki) = (matvec(&keff, &re), matvec(&keff, &im));
            let (mr, mi) = (matvec(m, &re), matvec(m, &im));
            let g2 = 2.0 * w_t;
            let gr: Vec<f64> = matvec(&c_full, &re).iter().map(|v| g2 * v).collect();
            let gi: Vec<f64> = matvec(&c_full, &im).iter().map(|v| g2 * v).collect();
            let (g, om) = (x.growth, x.omega);
            let (s2r, s2i) = (g * g - om * om, 2.0 * g * om);
            let sa = g.hypot(om);
            let (mut num, mut nk, mut nm, mut ng) = (0.0, 0.0, 0.0, 0.0);
            for e in 0..re.len() {
                if fixed[e] {
                    continue;
                }
                let a = kr[e] + s2r * mr[e] - s2i * mi[e] + g * gr[e] - om * gi[e];
                let b = ki[e] + s2r * mi[e] + s2i * mr[e] + g * gi[e] + om * gr[e];
                num += a * a + b * b;
                nk += kr[e] * kr[e] + ki[e] * ki[e];
                nm += mr[e] * mr[e] + mi[e] * mi[e];
                ng += gr[e] * gr[e] + gi[e] * gi[e];
            }
            e1 = e1.max(num.sqrt() / (nk.sqrt() + sa * sa * nm.sqrt() + sa * ng.sqrt()));
            if pair {
                let s = [g, om];
                let mut skipped = false;
                let mut gap = f64::INFINITY;
                for t in &h.state.eigenvalues {
                    if !skipped && t[0].to_bits() == s[0].to_bits() && t[1].to_bits() == s[1].to_bits() {
                        skipped = true;
                        continue;
                    }
                    gap = gap.min((t[0] - s[0]).hypot(t[1] - s[1]) / sa);
                }
                if gap > 1e-6 {
                    separated += 1;
                    w1 = w1.max((h.waves[i].unwrap().abs() - 1.0).abs());
                }
            }
        }
    }
    println!("W1: {separated} separated pair modes");
    ch.le("Q1 residuals at Omega_T", q1, 1e-9);
    ch.le("E1 full residual of the expanded sector modes", e1, 1e-8);
    ch.le("W1 | |tau| - 1 | of separated pair modes", w1, 1e-6);
    // T1
    let speeds = [0.8 * w_ref, w_t];
    let table = rs.whirl_stability(&[0, 1, 2, 3], &speeds).unwrap();
    let mut t1 = table.harmonics == vec![0, 1, 2, 3] && table.speeds == speeds.to_vec();
    for (hi_, row) in table.results.iter().enumerate() {
        for (j, x) in row.iter().enumerate() {
            t1 &= *x == rs.whirl_state(hi_, speeds[j]).unwrap();
        }
    }
    for j in 0..speeds.len() {
        let col: Vec<Stability> = table.results.iter().map(|r| r[j].state.stability).collect();
        t1 &= table.rotor(j) == Stability::combined(&col);
    }
    ch.holds(
        "T1 table entries and rotor flags",
        t1,
        "equal to whirl_state and combined",
    );
    let csv = table.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    let width = table
        .results
        .iter()
        .flat_map(|r| r.iter().map(|x| x.state.modes.len()))
        .max()
        .unwrap();
    let mut t1c = lines.len() == 1 + 8 && lines[0].split(',').count() == 5 + width;
    let mut row = 1;
    for (h, rr) in table.harmonics.iter().zip(&table.results) {
        for (w, x) in table.speeds.iter().zip(rr) {
            let f: Vec<&str> = lines[row].split(',').collect();
            row += 1;
            t1c &= f.len() == 5 + width;
            t1c &= f[0] == h.to_string();
            t1c &= f[1].parse::<f64>().unwrap().to_bits() == w.to_bits();
            let neg = match x.state.loss {
                None => "0".to_string(),
                Some(l) => l.negative.map_or_else(String::new, |n| n.to_string()),
            };
            t1c &= f[2] == neg;
            t1c &= f[3] == x.state.stability.is_stable().to_string();
            let g = growth_of(x.state.stability).unwrap_or(0.0);
            t1c &= f[4].parse::<f64>().unwrap().to_bits() == g.to_bits();
            for (jm, md) in x.state.modes.iter().enumerate() {
                t1c &= f[5 + jm].parse::<f64>().unwrap().to_bits() == md.frequency.to_bits();
            }
        }
    }
    println!("T1 CSV header: {}", lines[0]);
    ch.holds("T1 CSV", t1c, "1 + 8 lines, columns and round trip");
    // N1
    let rs_n1 = RotatingSector::new(&sector, &ds_options(false, stress)).unwrap();
    let hs_n1: Vec<HarmonicStateWhirl> = (0..=N / 2).map(|k| rs_n1.whirl_state(k, w_t).unwrap()).collect();
    let (all_n1, _) = union_of(&hs_n1);
    ch.gt(
        "N1 sector without Coriolis against the full rotor with it",
        worst_matched(&all_n1, &fr.eigenvalues),
        1e-6,
    );
    // N2
    let fp_n2 = RotatingPart::new(&full, AXIS, ORIGIN, &ds_options(false, stress)).unwrap();
    let fr_n2 = fp_n2.whirl_state(w_t).unwrap();
    println!("N2: full rotor without Coriolis: {:?}", fr_n2.stability);
    ch.gt(
        "N2 growth of the full rotor without Coriolis",
        growth_of(fr_n2.stability).unwrap_or(0.0),
        1.0,
    );
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ R1

fn key_of(e: FemError) -> &'static str {
    match e {
        FemError::InvalidValue { key, .. } => key,
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn r1_refusals() {
    let mut ch = Checks::new();
    let sector = ds_sector();
    let rs = RotatingSector::new(&sector, &ds_options(true, true)).unwrap();
    ch.holds(
        "R1 harmonic 4",
        key_of(rs.whirl_state(4, 1.0).unwrap_err()) == "harmonic",
        "refused with \"harmonic\"",
    );
    ch.holds(
        "R1 speed -1",
        key_of(rs.whirl_state(1, -1.0).unwrap_err()) == "speed",
        "refused with \"speed\"",
    );
    ch.holds(
        "R1 speed NaN",
        key_of(rs.whirl_state(1, f64::NAN).unwrap_err()) == "speed",
        "refused with \"speed\"",
    );
    ch.holds(
        "R1 harmonics empty",
        key_of(rs.whirl_stability(&[], &[1.0]).unwrap_err()) == "harmonics",
        "refused with \"harmonics\"",
    );
    ch.holds(
        "R1 speeds decreasing",
        key_of(rs.whirl_stability(&[1], &[2.0, 1.0]).unwrap_err()) == "speeds",
        "refused with \"speeds\"",
    );
    ch.finish();
}
