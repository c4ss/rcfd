//! F-510 (ADR 0424): odd-index instability of a rotating part on the model path. The cube on springs of ADR 0416 with its
//! axial motion tied and an axial support: negative (`RotatingPart::with_nodal_springs`), an odd count of negative
//! eigenvalues of `K_eff` and the closed-form divergence `s = +/- omega_a`; positive, an even count and stable. A cyclic
//! sector whose single index 0 loses stiffness first (stress stiffening off) against its full rotor. Checks O1 to R1 and the
//! controls N1, N2 of the registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::full_rotor;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{
    HarmonicStateWhirl, HarmonicWhirlSpeedModes, RotatingOptions, RotatingPart, RotatingSector, Stability, StateWhirl,
    STABILITY_TOLERANCE,
};
use rcfd_fem::{Constraint, ElasticSupport, FemError, LinearTerm, Material, SolidMesh, SolidModel};
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
const W0: f64 = 1000.0;
const WY: f64 = 1.2 * W0;
const WA: f64 = 0.6 * W0;
const H: f64 = 0.1;

fn material() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

fn mass() -> f64 {
    RHO * H * H * H
}

/// `k_a = m omega_a^2 / 8`, the axial support per node.
fn ka() -> f64 {
    mass() * WA * WA / 8.0
}

/// ADR 0416 D5: `(lambda_+, lambda_-)`, `lambda_- = -(S + sqrt D)/2`, `lambda_+ = P / lambda_-`.
fn closed(wx: f64, wy: f64, w: f64) -> (f64, f64) {
    let s = wx * wx + wy * wy + 2.0 * w * w;
    let p = (wx * wx - w * w) * (wy * wy - w * w);
    let d = (wx * wx - wy * wy).powi(2) + 8.0 * w * w * (wx * wx + wy * wy);
    let lm = -(s + d.sqrt()) / 2.0;
    (p / lm, lm)
}

// ------------------------------------------------------------------------------------------------------ CT (D3)

/// The tied cube: ADR 0416's cube with `u_z` tied over the nodes instead of fixed, and the axial support `+k_a` per node.
fn tied_cube(wx: f64, wy: f64) -> SolidModel {
    let mesh = hex_box([-H / 2.0; 3], [H; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    let nodes = m.mesh().nodes().len();
    assert_eq!(nodes, 8, "defective set-up");
    for i in 1..nodes {
        m.add_constraint(Constraint::linear(
            vec![LinearTerm::new(i, 2, 1.0), LinearTerm::new(0, 2, -1.0)],
            0.0,
        ))
        .unwrap();
    }
    m.add_support(ElasticSupport::nodal_spring("all", 0, mass() * wx * wx / 8.0))
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("all", 1, mass() * wy * wy / 8.0))
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("all", 2, ka())).unwrap();
    m
}

fn options(coriolis: bool) -> RotatingOptions {
    RotatingOptions::new(4).coriolis(coriolis)
}

/// CT+ (the model's axial support) or CT- (with `-2 k_a` added: net `-k_a`).
fn part<'a>(model: &'a SolidModel, negative: bool, coriolis: bool) -> RotatingPart<'a> {
    let p = RotatingPart::new(model, AXIS, ORIGIN, &options(coriolis)).unwrap();
    if negative {
        p.with_nodal_springs("all", 2, -2.0 * ka()).unwrap()
    } else {
        p
    }
}

fn count(r: &StateWhirl) -> Option<Option<usize>> {
    r.loss.map(|l| l.negative)
}

fn growth_of(s: Stability) -> Option<f64> {
    match s {
        Stability::Stable => None,
        Stability::Unstable { growth } => Some(growth),
    }
}

fn cabs(s: [f64; 2]) -> f64 {
    s[0].hypot(s[1])
}

fn smax(ev: &[[f64; 2]]) -> f64 {
    ev.iter().map(|s| cabs(*s)).fold(0.0f64, f64::max)
}

fn nearest(ev: &[[f64; 2]], t: [f64; 2], skip: &[usize]) -> usize {
    let mut best = (f64::INFINITY, 0);
    for (j, s) in ev.iter().enumerate() {
        if skip.contains(&j) {
            continue;
        }
        let d = (s[0] - t[0]).hypot(s[1] - t[1]);
        if d < best.0 {
            best = (d, j);
        }
    }
    best.1
}

/// The closed-form frequencies `targets` (rad/s, > 0) among the eigenvalues: each matched to the nearest unused eigenvalue
/// in order; the worst `|s - i omega| / omega`.
fn present(ev: &[[f64; 2]], targets: &[f64]) -> f64 {
    let mut used = Vec::new();
    let mut worst: f64 = 0.0;
    for &w in targets {
        let j = nearest(ev, [0.0, w], &used);
        used.push(j);
        worst = worst.max((ev[j][0]).hypot(ev[j][1] - w) / w);
    }
    worst
}

/// The closed-form translation pair at the speed `w` (ADR 0416 D5 isotropic).
fn pair(w: f64) -> [f64; 2] {
    if w < W0 {
        [W0 - w, W0 + w]
    } else {
        [w - W0, w + W0]
    }
}

fn residuals(r: &StateWhirl) -> f64 {
    r.modes.iter().fold(0.0f64, |a, m| a.max(m.residual))
}

fn print_state(tag: &str, r: &StateWhirl) {
    println!(
        "{tag}: unknowns {}, loss {:?}, flag {:?}, real defect {:.3e}, modes (growth, omega) {:?}",
        r.unknowns,
        r.loss,
        r.stability,
        r.real_defect,
        r.modes.iter().map(|m| (m.growth, m.omega)).collect::<Vec<_>>()
    );
}

#[test]
fn o1_o2_o3_q1_n1_negative_axial_support_odd() {
    let mut ch = Checks::new();
    let cb = tied_cube(W0, W0);
    let p = part(&cb, true, true);
    let (mut o1g, mut o2, mut o2m, mut o3, mut q1) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut flags = true;
    let mut counts = true;
    for (f, c) in [(0.0, 1), (0.5, 1), (1.2, 3), (1.5, 3), (2.0, 3)] {
        let w = f * W0;
        let r = p.whirl_state(w).unwrap();
        print_state(&format!("CTI- {f} omega_0"), &r);
        let ok = count(&r) == Some(Some(c));
        println!("  count {:?} (expected Some({c}))", count(&r));
        counts &= ok;
        match r.stability {
            Stability::Unstable { growth } => o1g = o1g.max((growth / WA - 1.0).abs()),
            Stability::Stable => flags = false,
        }
        let ev = &r.eigenvalues;
        let ip = nearest(ev, [WA, 0.0], &[]);
        let im = nearest(ev, [-WA, 0.0], &[ip]);
        let sm = smax(ev);
        let rest = ev
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != ip && *j != im)
            .map(|(_, s)| s[0].abs() / sm)
            .fold(0.0f64, f64::max);
        o2 = o2.max(rest);
        o2m = o2m.max((ev[im][0] + WA).hypot(ev[im][1]) / WA);
        let pr = pair(w);
        o3 = o3.max(present(ev, &pr));
        q1 = q1.max(residuals(&r));
    }
    ch.holds("O1 counts", counts, "Some(1), Some(1), Some(3), Some(3), Some(3)");
    ch.holds("O1 flags", flags, "Unstable at every speed");
    ch.le("O1 growth against omega_a", o1g, 1e-8);
    ch.le("O2 real parts of the rest / max|s|", o2, 1e-8);
    ch.le("O2 the root -omega_a", o2m, 1e-8);
    ch.le("O3 the closed-form pair", o3, 1e-8);
    ch.le("Q1 residuals CTI-", q1, 1e-9);
    // N1: without Coriolis the in-plane pair diverges too.
    let pn = part(&cb, true, false);
    let w = 1.5 * W0;
    let r = pn.whirl_state(w).unwrap();
    print_state("N1 CTI- 1.5 omega_0 without Coriolis", &r);
    let expected = (w * w - W0 * W0).sqrt();
    match growth_of(r.stability) {
        Some(g) => ch.le("N1 growth against sqrt(Omega^2 - omega_0^2)", (g / expected - 1.0).abs(), 1e-8),
        None => ch.holds("N1 flag", false, "Unstable"),
    }
    ch.finish();
}

#[test]
fn v1_q1_positive_axial_support_even() {
    let mut ch = Checks::new();
    let cb = tied_cube(W0, W0);
    let p = part(&cb, false, true);
    let (mut v1, mut q1) = (0.0f64, 0.0f64);
    let mut flags = true;
    let mut counts = true;
    for (f, c) in [(0.5, None), (1.2, Some(Some(2))), (1.5, Some(Some(2))), (2.0, Some(Some(2)))] {
        let w = f * W0;
        let r = p.whirl_state(w).unwrap();
        print_state(&format!("CTI+ {f} omega_0"), &r);
        counts &= count(&r) == c;
        flags &= r.stability == Stability::Stable;
        let pr = pair(w);
        v1 = v1.max(present(&r.eigenvalues, &[WA, pr[0], pr[1]]));
        q1 = q1.max(residuals(&r));
    }
    ch.holds("V1 counts", counts, "None, Some(2), Some(2), Some(2)");
    ch.holds("V1 flags", flags, "Stable at every speed");
    ch.le("V1 omega_a and the closed-form pair", v1, 1e-8);
    ch.le("Q1 residuals CTI+", q1, 1e-9);
    ch.finish();
}

#[test]
fn v2_v3_q1_anisotropic_tied_cube_in_the_band() {
    let mut ch = Checks::new();
    let cb = tied_cube(W0, WY);
    let w = 1.1 * W0;
    let sigma = closed(W0, WY, w).0.sqrt();
    println!("sigma(1.1 omega_0) = {sigma:.10}");
    let mut q1: f64 = 0.0;
    // V2: negative axial support, even count, unstable.
    let r = part(&cb, true, true).whirl_state(w).unwrap();
    print_state("CTA- 1.1 omega_0", &r);
    ch.holds("V2 count", count(&r) == Some(Some(2)), &format!("{:?}", count(&r)));
    match growth_of(r.stability) {
        Some(g) => ch.le("V2 growth against omega_a", (g / WA - 1.0).abs(), 1e-8),
        None => ch.holds("V2 flag", false, "Unstable"),
    }
    let mut re: Vec<f64> = r.eigenvalues.iter().map(|s| s[0]).collect();
    re.sort_by(|a, b| b.total_cmp(a));
    ch.le("V2 second largest real part against sigma", (re[1] / sigma - 1.0).abs(), 1e-8);
    q1 = q1.max(residuals(&r));
    // V3: positive axial support, odd count.
    let r = part(&cb, false, true).whirl_state(w).unwrap();
    print_state("CTA+ 1.1 omega_0", &r);
    ch.holds("V3 count", count(&r) == Some(Some(1)), &format!("{:?}", count(&r)));
    match growth_of(r.stability) {
        Some(g) => ch.le("V3 growth against sigma", (g / sigma - 1.0).abs(), 1e-8),
        None => ch.holds("V3 flag", false, "Unstable"),
    }
    q1 = q1.max(residuals(&r));
    ch.le("Q1 residuals CTA", q1, 1e-9);
    ch.finish();
}

fn key_of(e: FemError) -> String {
    match e {
        FemError::InvalidValue { key, .. } => key.to_string(),
        FemError::UnknownSet { .. } => "UnknownSet".to_string(),
        FemError::ComponentOutOfRange { .. } => "ComponentOutOfRange".to_string(),
        other => format!("{other:?}"),
    }
}

#[test]
fn r1_refusals_and_a_zero_spring() {
    let mut ch = Checks::new();
    let cb = tied_cube(W0, W0);
    let p = || RotatingPart::new(&cb, AXIS, ORIGIN, &options(true)).unwrap();
    let cases: [(&str, usize, f64, &str); 4] = [
        ("nowhere", 2, 1.0, "UnknownSet"),
        ("all", 3, 1.0, "ComponentOutOfRange"),
        ("all", 2, f64::NAN, "stiffness"),
        ("all", 2, f64::INFINITY, "stiffness"),
    ];
    for (set, c, k, want) in cases {
        let got = match p().with_nodal_springs(set, c, k) {
            Ok(_) => "accepted".to_string(),
            Err(e) => key_of(e),
        };
        ch.holds(&format!("R1 ({set}, {c}, {k})"), got == want, &format!("{got} (expected {want})"));
    }
    let a = p().operator(1.5 * W0);
    let b = p().with_nodal_springs("all", 2, 0.0).unwrap().operator(1.5 * W0);
    let same = a.row_ptr == b.row_ptr
        && a.col == b.col
        && a.val.iter().zip(&b.val).all(|(x, y)| x.to_bits() == y.to_bits());
    ch.holds("R1 zero spring", same, "operator bit-identical");
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ the sector (D4)

const N: usize = 6;

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

fn ds_options() -> RotatingOptions {
    let mut o = ModalOptions::new(4).tolerance(1e-9);
    o.max_iterations = 400;
    RotatingOptions::new(4)
        .modal(o)
        .coriolis(true)
        .stress_stiffening(false)
}

fn count_of(r: &StateWhirl) -> Option<usize> {
    match r.loss {
        None => Some(0),
        Some(l) => l.negative,
    }
}

/// `c_k(Omega)` of D4 step 2 (a zero pivot counts as not lost).
fn c_of(rs: &RotatingSector, k: usize, w: f64) -> usize {
    count_of(&rs.whirl_state(k, w).unwrap().state).unwrap_or(0)
}

/// D4 steps 3, 4: the first of 2, 4, 8, 16 omega_ref at or above `from` with `c_k >= j`, then 60 halvings.
fn crossing(rs: &RotatingSector, k: usize, j: usize, from: f64, w_ref: f64) -> f64 {
    let hi = [2.0, 4.0, 8.0, 16.0]
        .iter()
        .map(|f| f * w_ref)
        .filter(|&w| w >= from)
        .find(|&w| c_of(rs, k, w) >= j);
    let Some(hi) = hi else {
        return f64::INFINITY;
    };
    let (mut lo, mut up) = (from, hi);
    for _ in 0..60 {
        let mid = 0.5 * (lo + up);
        if c_of(rs, k, mid) >= j {
            up = mid;
        } else {
            lo = mid;
        }
    }
    up
}

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

fn flag_of(ev: &[[f64; 2]]) -> Stability {
    let sm = smax(ev);
    let remax = ev.iter().map(|s| s[0].abs()).fold(0.0f64, f64::max);
    if sm == 0.0 || remax <= STABILITY_TOLERANCE * sm {
        Stability::Stable
    } else {
        Stability::Unstable {
            growth: ev.iter().map(|s| s[0]).fold(f64::NEG_INFINITY, f64::max),
        }
    }
}

#[test]
fn s0_s1_s2_s3_s4_sector_single_index_odd() {
    let mut ch = Checks::new();
    let sector = ds_sector();
    let rs = RotatingSector::new(&sector, &ds_options()).unwrap();
    let w_ref = (0..=N / 2)
        .map(|k| match rs.whirl_modes(k, 0.0).unwrap() {
            HarmonicWhirlSpeedModes::Modes { modes, .. } => modes.modes[0].omega,
            HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("defective set-up: loss at rest {l:?}"),
        })
        .fold(f64::INFINITY, f64::min);
    println!("S0: omega_ref = {w_ref:.9e} rad/s (stress stiffening off, Coriolis on)");
    let w_l: Vec<f64> = (0..=N / 2)
        .map(|k| {
            assert_eq!(c_of(&rs, k, 0.8 * w_ref), 0, "defective set-up: index {k} lost at 0.8 omega_ref");
            let w = crossing(&rs, k, 1, 0.8 * w_ref, w_ref);
            println!("S0: Omega_L,{k} = {w:.12e} rad/s = {:.9} omega_ref", w / w_ref);
            w
        })
        .collect();
    let k_o = if w_l[0] <= w_l[N / 2] { 0 } else { N / 2 };
    assert!(w_l[k_o].is_finite(), "defective set-up: no single index lost by 16 omega_ref");
    let w_2 = crossing(&rs, k_o, 2, w_l[k_o], w_ref);
    let earlier: Vec<usize> = (0..=N / 2).filter(|&k| k != k_o && w_l[k] <= w_l[k_o]).collect();
    let w_next = (0..=N / 2)
        .filter(|&k| k != k_o && w_l[k] > w_l[k_o])
        .map(|k| w_l[k])
        .fold(w_2.min(16.0 * w_ref), f64::min);
    let w_t = (0.5 * (w_l[k_o] * w_l[k_o] + w_next * w_next)).sqrt();
    println!(
        "S0: k_o = {k_o}, Omega_2 = {w_2:.12e}, indices lost before k_o {earlier:?}, Omega_next = {w_next:.12e} \
         ({:.9} omega_ref), Omega_T = {w_t:.12e} rad/s = {:.9} omega_ref",
        w_next / w_ref,
        w_t / w_ref
    );
    ch.holds("S0 Omega_L,k_o > 0.8 omega_ref", w_l[k_o] > 0.8 * w_ref, "bound");

    let hs: Vec<HarmonicStateWhirl> = (0..=N / 2).map(|k| rs.whirl_state(k, w_t).unwrap()).collect();
    for h in &hs {
        print_state(&format!("k = {}", h.harmonic), &h.state);
    }
    let ho = &hs[k_o].state;
    ch.holds("S1 count of k_o", count_of(ho) == Some(1), &format!("{:?}", count_of(ho)));
    ch.holds("S1 flag of k_o", !ho.stability.is_stable(), &format!("{:?}", ho.stability));

    let rotor = full_rotor(&sector).unwrap();
    let mut full = model_on(rotor.mesh().clone());
    clamp(&mut full, "xmin");
    let fp = RotatingPart::new(&full, AXIS, ORIGIN, &ds_options()).unwrap();
    let fr = fp.whirl_state(w_t).unwrap();
    print_state("full rotor", &fr);
    match (growth_of(ho.stability), growth_of(fr.stability)) {
        (Some(a), Some(b)) => ch.le("S2 growth of k_o against the full rotor's", (a / b - 1.0).abs(), 1e-8),
        _ => ch.holds("S2 flags", false, "both Unstable"),
    }
    let counts: Vec<Option<usize>> = hs.iter().map(|h| count_of(&h.state)).collect();
    let pair_even = hs
        .iter()
        .zip(&counts)
        .all(|(h, c)| h.multiplicity == 1 || c.is_some_and(|c| c % 2 == 0));
    let sum: Option<usize> = counts.iter().copied().sum();
    ch.holds(
        "S3 Sylvester count, odd",
        sum.is_some() && sum == count_of(&fr) && pair_even && sum.is_some_and(|c| c % 2 == 1),
        &format!("indices {counts:?}, sum {sum:?}, full {:?}", count_of(&fr)),
    );
    let mut all = Vec::new();
    let mut idx = Vec::new();
    for h in &hs {
        for s in &h.state.eigenvalues {
            all.push(*s);
            idx.push(h.harmonic);
        }
    }
    ch.holds(
        "S4 count",
        all.len() == fr.eigenvalues.len(),
        &format!("{} = {}", all.len(), fr.eigenvalues.len()),
    );
    let mut s4: f64 = 0.0;
    let mut matched: Vec<Vec<[f64; 2]>> = vec![Vec::new(); N / 2 + 1];
    for (i, j) in matching(&all, &fr.eigenvalues) {
        let t = fr.eigenvalues[j];
        s4 = s4.max((all[i][0] - t[0]).hypot(all[i][1] - t[1]) / cabs(t));
        matched[idx[i]].push(t);
    }
    ch.le("S4 union against the full rotor", s4, 1e-8);
    for (k, h) in hs.iter().enumerate() {
        let fk = flag_of(&matched[k]);
        ch.holds(
            &format!("S4 k = {k} flag"),
            h.state.stability.is_stable() == fk.is_stable(),
            &format!("sector {:?}, full matched {fk:?}", h.state.stability),
        );
    }
    ch.finish();
}
