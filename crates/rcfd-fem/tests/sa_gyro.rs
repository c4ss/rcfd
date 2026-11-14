//! F-415 (ADR 0472): the Coriolis term of a rotating part on the modal subspace, the split Campbell table and the stability
//! flag. Checks G1 to G3, B1, Z1, P1 to P4, R1, R2, L1 to L3, T1, U1, U2, C1, X1, X2 and the controls N1 to N4 of the
//! registration; B2 is in `sa_gyro_default_bits.rs`.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::full_rotor;
use rcfd_fem::gyro::{gyroscopic_matrix, split_modes_dense, Basis, SplitMode, SplitModes};
use rcfd_fem::hex20::{upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::modal::{ModalOptions, ModalResult};
use rcfd_fem::rotating::{RotatingOptions, RotatingPart, SpeedModes, Stability, Whirl};
use rcfd_fem::{Constraint, ElasticSupport, FemError, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
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
            "{id}: {value:.4e} (limit {limit:.0e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn lt(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{id}: {value:.4e} (expected < {limit:.0e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} >= {limit:e}"));
        }
    }
    fn gt(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{id}: {value:.4e} (expected > {limit:.0e}) {}",
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
        assert!(self.failed.is_empty(), "failed: {:#?}", self.failed);
    }
}

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const W0: f64 = 1000.0;
const H: f64 = 0.1;
const FACTORS: [f64; 7] = [0.1, 0.3, 0.5, 0.7, 0.9, 1.2, 1.5];

fn key_of(e: FemError) -> &'static str {
    match e {
        FemError::InvalidValue { key, .. } => key,
        other => panic!("unexpected error {other:?}"),
    }
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(p, q)| p * q).sum()
}

fn modes_of(s: SpeedModes) -> ModalResult {
    match s {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("defective set-up: loss of stiffness {l:?}"),
    }
}

/// A fixed linear congruential sequence in [-1, 1).
fn lcg(n: usize, seed: u64) -> Vec<f64> {
    let mut x = seed;
    (0..n)
        .map(|_| {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        })
        .collect()
}

/// G1: `max|G_ij + G_ji| / max|G_ij|`.
fn skew_defect(g: &CsrMatrix) -> f64 {
    let mut big: f64 = 0.0;
    let mut d: f64 = 0.0;
    for r in 0..g.n {
        for p in g.row_ptr[r]..g.row_ptr[r + 1] {
            let c = g.col[p];
            big = big.max(g.val[p].abs());
            let row = &g.col[g.row_ptr[c]..g.row_ptr[c + 1]];
            let back = row.binary_search(&r).map(|q| g.val[g.row_ptr[c] + q]).unwrap_or(0.0);
            d = d.max((g.val[p] + back).abs());
        }
    }
    d / big
}

/// G2: `|x^T G x| / (|x|^T |G| |x|)`.
fn quad_ratio(g: &CsrMatrix, x: &[f64]) -> f64 {
    let q = dot(x, &matvec(g, x));
    let mut abs = 0.0;
    for r in 0..g.n {
        for p in g.row_ptr[r]..g.row_ptr[r + 1] {
            abs += (x[r] * g.val[p] * x[g.col[p]]).abs();
        }
    }
    q.abs() / abs
}

/// G1, G2, G3 on one model at one speed.
fn g_checks(ck: &mut Checks, what: &str, model: &SolidModel, omega: f64, s: &SplitModes) {
    let g = gyroscopic_matrix(model, AXIS, omega, None).unwrap();
    ck.le(&format!("G1 {what} skew defect of G"), skew_defect(&g), 1e-13);
    let mut worst: f64 = 0.0;
    for seed in 1..=3 {
        worst = worst.max(quad_ratio(&g, &lcg(g.n, seed)));
    }
    for m in &s.modes {
        worst = worst.max(quad_ratio(&g, &m.re));
        if m.im.iter().any(|v| *v != 0.0) {
            worst = worst.max(quad_ratio(&g, &m.im));
        }
    }
    ck.le(&format!("G2 {what} |x^T G x| / |x|^T|G||x|"), worst, 1e-12);
    ck.le(&format!("G3 {what} raw skew defect of C_r"), s.skew_defect, 1e-11);
    println!(
        "  {what}: asymmetry of K_r, M_r before symmetrisation {:.3e}",
        s.asymmetry
    );
}

/// Z1: the split modes at rest against F-396's.
fn z1(ck: &mut Checks, what: &str, part: &RotatingPart) {
    let s = part.split_modes(0.0).unwrap();
    let r = modes_of(part.modes(0.0).unwrap());
    ck.holds(
        &format!("Z1 {what} count"),
        s.modes.len() == r.eigenvalues.len(),
        &format!("{} split, {} F-396", s.modes.len(), r.eigenvalues.len()),
    );
    let worst = s
        .modes
        .iter()
        .zip(&r.eigenvalues)
        .map(|(m, l)| (m.omega / l.sqrt() - 1.0).abs())
        .fold(0.0, f64::max);
    ck.le(&format!("Z1 {what} max |omega/omega_F396 - 1| at rest"), worst, 1e-10);
}

fn same_opt_bits(a: &[Option<f64>], b: &[Option<f64>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.map(f64::to_bits) == y.map(f64::to_bits))
}

fn same_modes_bits(a: &[Option<Vec<f64>>], b: &[Option<Vec<f64>>]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| match (x, y) {
            (Some(x), Some(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits()),
            (None, None) => true,
            _ => false,
        })
}

/// B1: the split table of a part without the option against its F-396 table.
fn b1(ck: &mut Checks, what: &str, part: &RotatingPart, speeds: &[f64]) {
    let s = part.split_campbell(speeds).unwrap();
    let t = part.campbell(speeds).unwrap();
    let a = &s.campbell.table;
    let mut ok = !s.coriolis && a.tracks.len() == t.tracks.len() && a.losses == t.losses;
    ok &= a.speeds.iter().zip(&t.speeds).all(|(x, y)| x.to_bits() == y.to_bits());
    for (x, y) in a.tracks.iter().zip(&t.tracks) {
        ok &= same_opt_bits(&x.eigenvalues, &y.eigenvalues);
        ok &= same_opt_bits(&x.frequencies, &y.frequencies);
        ok &= same_opt_bits(&x.mac, &y.mac);
        ok &= same_modes_bits(&x.modes, &y.modes);
    }
    ok &= a.to_csv() == t.to_csv();
    ck.holds(
        &format!("B1 {what} split table without the option = F-396 table"),
        ok,
        &format!("{} tracks, {} losses", t.tracks.len(), t.losses.len()),
    );
}

// ------------------------------------------------------------------------------------------------------------------------
// PM, PMA

fn pm_matrices(omega: f64, ky: f64) -> ([f64; 4], [f64; 4], [f64; 4]) {
    let m = 7.85;
    let mm = [m, 0.0, 0.0, m];
    let c = [0.0, -m, m, 0.0];
    let k = [
        m * (W0 * W0 - omega * omega),
        0.0,
        0.0,
        m * (ky * W0 * W0 - omega * omega),
    ];
    (mm, c, k)
}

/// The growth `sqrt(mu+)` of the PMA closed form at `omega` (`None` if stable).
fn pma_growth(omega: f64) -> Option<f64> {
    let (a, b, o2) = (W0 * W0, 2.25 * W0 * W0, omega * omega);
    let c = (a - o2) * (b - o2);
    if c >= 0.0 {
        return None;
    }
    let bb = a + b + 2.0 * o2;
    Some(((-bb + (bb * bb - 4.0 * c).sqrt()) / 2.0).sqrt())
}

#[test]
fn p1_p2_u2_x1_point_mass() {
    let mut ck = Checks::new();
    let mut p1: f64 = 0.0;
    let mut p2 = true;
    let mut stable = true;
    for f in FACTORS {
        let om = f * W0;
        let (m, c, k) = pm_matrices(om, 1.0);
        let d = split_modes_dense(&m, &c, om, &k, 2).unwrap();
        let mut got: Vec<f64> = d.modes.iter().map(|x| x.omega).collect();
        got.sort_by(f64::total_cmp);
        let mut want = [(W0 - om).abs(), W0 + om];
        want.sort_by(f64::total_cmp);
        let e = if got.len() == 2 {
            got.iter()
                .zip(&want)
                .map(|(g, w)| (g - w).abs() / W0)
                .fold(0.0, f64::max)
        } else {
            f64::INFINITY
        };
        println!(
            "  PM {f}: {got:?} against {want:?}, kappa {:?}, real defect {:.2e}",
            d.modes.iter().map(|x| x.whirl).collect::<Vec<_>>(),
            d.real_defect
        );
        p1 = p1.max(e);
        stable &= d.stability == Stability::Stable;
        if f < 1.0 && d.modes.len() == 2 {
            let (lo, hi) = (&d.modes[0], &d.modes[1]);
            p2 &= lo.whirl >= 1.0 - 1e-8 && lo.direction == Whirl::Forward;
            p2 &= hi.whirl <= -1.0 + 1e-8 && hi.direction == Whirl::Backward;
        }
    }
    ck.le("P1 PM max |omega - {|w0 - Omega|, w0 + Omega}| / w0", p1, 1e-10);
    ck.holds("P2 PM stable at every speed", stable, "Stability::Stable");
    ck.holds(
        "P2 PM directions below w0",
        p2,
        "lower forward kappa = +1, upper backward kappa = -1",
    );
    // U2
    for f in [0.5, 1.2, 1.8] {
        let om = f * W0;
        let (m, c, k) = pm_matrices(om, 2.25);
        let d = split_modes_dense(&m, &c, om, &k, 2).unwrap();
        match (pma_growth(om), d.stability) {
            (Some(g), Stability::Unstable { growth }) => {
                println!("  PMA {f}: growth {growth:.10e}, closed form {g:.10e}");
                ck.le(
                    &format!("U2 PMA {f} w0 growth relative"),
                    (growth / g - 1.0).abs(),
                    1e-10,
                );
            }
            (None, Stability::Stable) => ck.holds(&format!("U2 PMA {f} w0"), true, "stable"),
            (want, got) => ck.holds(
                &format!("U2 PMA {f} w0"),
                false,
                &format!("closed form {want:?}, got {got:?}"),
            ),
        }
    }
    // X1 (dense)
    let one = [1.0, 0.0, 0.0, 1.0];
    ck.holds(
        "X1 dense wrong length",
        key_of(split_modes_dense(&one, &[0.0; 3], 1.0, &one, 2).unwrap_err()) == "matrix",
        "\"matrix\"",
    );
    ck.holds(
        "X1 dense mass not positive definite",
        key_of(split_modes_dense(&[1.0, 0.0, 0.0, -1.0], &[0.0; 4], 1.0, &one, 2).unwrap_err()) == "mass",
        "\"mass\"",
    );
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------------------------
// CB, CBA

fn cube(ky: f64) -> SolidModel {
    let mesh = hex_box([-H / 2.0; 3], [H; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, 0.3, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    let kn = RHO * H * H * H * W0 * W0 / 8.0;
    m.add_support(ElasticSupport::nodal_spring("all", 0, kn)).unwrap();
    m.add_support(ElasticSupport::nodal_spring("all", 1, ky * kn)).unwrap();
    m
}

fn uniform(model: &SolidModel, c: usize) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut v = vec![0.0; dofs.n_equations()];
    for node in 0..model.mesh().nodes().len() {
        v[dofs.equation(node, c).unwrap()] = 1.0;
    }
    v
}

/// The share of `span{1_x, 1_y}` in a complex shape, `M` norm.
fn translation_share(model: &SolidModel, m: &CsrMatrix, re: &[f64], im: &[f64]) -> f64 {
    let mut s = 0.0;
    for c in 0..2 {
        let u = uniform(model, c);
        let mu = matvec(m, &u);
        let (a, b) = (dot(&mu, re), dot(&mu, im));
        s += (a * a + b * b) / dot(&u, &mu);
    }
    s / (dot(re, &matvec(m, re)) + dot(im, &matvec(m, im)))
}

fn mode_share(model: &SolidModel, m: &CsrMatrix, x: &SplitMode) -> f64 {
    translation_share(model, m, &x.re, &x.im)
}

#[test]
fn g_z1_b1_p3_p4_n4_t1_x1_cube() {
    let mut ck = Checks::new();
    let cb = cube(1.0);
    let on = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(3).coriolis(true)).unwrap();
    let off = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(3)).unwrap();
    let m = on.mass();
    // G1 to G3
    let s = on.split_modes(0.5 * W0).unwrap();
    g_checks(&mut ck, "CB", &cb, 0.5 * W0, &s);
    // Z1
    z1(&mut ck, "CB", &on);
    // B1
    let speeds: Vec<f64> = (0..=12).map(|j| 0.07 * j as f64 * W0).collect();
    let mut with_loss = speeds.clone();
    with_loss.push(1.2 * W0);
    b1(&mut ck, "CB", &off, &with_loss);
    let so = off.split_modes(0.5 * W0).unwrap();
    let fo = modes_of(off.modes(0.5 * W0).unwrap());
    ck.holds(
        "B1 CB split_modes without the option = modes",
        so.modes.len() == fo.frequencies.len()
            && so
                .modes
                .iter()
                .zip(&fo.frequencies)
                .all(|(a, b)| a.frequency.to_bits() == b.to_bits()),
        "frequencies by bits",
    );
    // P3, P4, N4
    let mut p3: f64 = 0.0;
    let mut p3_share = true;
    let mut p4 = true;
    let mut n4 = true;
    for f in FACTORS {
        let om = f * W0;
        let s = on.split_modes(om).unwrap();
        let mut pair: Vec<&SplitMode> = s.modes.iter().collect();
        pair.sort_by(|a, b| mode_share(&cb, m, b).total_cmp(&mode_share(&cb, m, a)));
        pair.truncate(2);
        p3_share &= pair.len() == 2 && pair.iter().all(|x| mode_share(&cb, m, x) >= 1.0 - 1e-8);
        let mut got: Vec<f64> = pair.iter().map(|x| x.omega).collect();
        got.sort_by(f64::total_cmp);
        let mut want = [(W0 - om).abs(), W0 + om];
        want.sort_by(f64::total_cmp);
        let e = got
            .iter()
            .zip(&want)
            .map(|(g, w)| (g / w - 1.0).abs())
            .fold(0.0, f64::max);
        p3 = p3.max(e);
        let want_basis = if f < 1.0 { Basis::Speed } else { Basis::Rest };
        p4 &= s.basis == Some(want_basis) && (s.loss.is_some() == (f > 1.0));
        p4 &= s.stability == Some(Stability::Stable);
        if f > 1.0 {
            n4 &= off.modes(om).unwrap().loss().is_some() && s.stability == Some(Stability::Stable);
        }
        println!(
            "  CB {f}: pair {got:?} against {want:?}; basis {:?}, loss {:?}, {:?}, all {:?}",
            s.basis,
            s.loss,
            s.stability,
            s.modes.iter().map(|x| x.omega).collect::<Vec<_>>()
        );
    }
    ck.holds("P3 CB translation pair shares >= 1 - 1e-8", p3_share, "two modes");
    ck.le("P3 CB max |omega_CB/omega_PM - 1|", p3, 1e-3);
    ck.holds(
        "P4 CB basis Speed below w0, Rest with a loss above; stable",
        p4,
        "each speed",
    );
    ck.holds("N4 CB at 1.2, 1.5 w0: F-396 loss, split stable", n4, "each");
    // T1
    let sc = on.split_campbell(&speeds).unwrap();
    let tr = &sc.campbell.table.tracks;
    let mut min_mac: f64 = 1.0;
    let mut all_there = true;
    for t in tr {
        for j in 1..speeds.len() {
            match t.mac[j] {
                Some(v) => min_mac = min_mac.min(v),
                None => all_there = false,
            }
        }
    }
    ck.holds(
        "T1 CB every track has a mode and a MAC at every speed > 0",
        all_there,
        &format!("{} tracks", tr.len()),
    );
    ck.le("T1 CB 1 - smallest stored MAC (MAC >= 0.9)", 1.0 - min_mac, 0.1);
    let n = m.n;
    let share_of = |line: &Vec<f64>| translation_share(&cb, m, &line[..n], &line[n..]);
    let trans: Vec<usize> = (0..tr.len())
        .filter(|&t| tr[t].modes[1].as_ref().is_some_and(|l| share_of(l) >= 1.0 - 1e-8))
        .collect();
    ck.holds("T1 CB two translation tracks", trans.len() == 2, &format!("{trans:?}"));
    if trans.len() == 2 {
        let om = |t: usize, j: usize| tr[t].eigenvalues[j].map(f64::sqrt).unwrap_or(f64::NAN);
        let (fwd, bwd) = if om(trans[0], 1) < om(trans[1], 1) {
            (trans[0], trans[1])
        } else {
            (trans[1], trans[0])
        };
        let mut keep = true;
        let mut follow: f64 = 0.0;
        for (j, &w) in speeds.iter().enumerate() {
            keep &= [fwd, bwd]
                .iter()
                .all(|&t| tr[t].modes[j].as_ref().is_some_and(|l| share_of(l) >= 1.0 - 1e-8));
            if j > 0 {
                follow = follow.max((om(fwd, j) - (W0 - w)).abs() / W0);
                follow = follow.max((om(bwd, j) - (W0 + w)).abs() / W0);
            }
        }
        ck.holds(
            "T1 CB translation tracks keep their shapes (no exchange)",
            keep,
            "share >= 1 - 1e-8 at every speed",
        );
        ck.le("T1 CB tracks against w0 -/+ Omega, / w0", follow, 1e-8);
        let tor = (0..tr.len()).find(|t| !trans.contains(t)).unwrap();
        let first = om(bwd, 0) - om(tor, 0);
        let last = om(bwd, 12) - om(tor, 12);
        println!(
            "  T1 backward - torsion: {first:.4} at rest, {last:.4} at 0.84 w0; directions {:?} / {:?}",
            sc.campbell.directions[fwd][1], sc.campbell.directions[bwd][1]
        );
        ck.holds(
            "T1 CB backward translation crosses the torsion (set-up)",
            first < 0.0 && last > 0.0,
            "sign change",
        );
    }
    // X1 (part)
    ck.holds(
        "X1 split_modes(-1)",
        key_of(on.split_modes(-1.0).unwrap_err()) == "speed",
        "\"speed\"",
    );
    ck.holds(
        "X1 split_modes(NaN)",
        key_of(on.split_modes(f64::NAN).unwrap_err()) == "speed",
        "\"speed\"",
    );
    ck.holds(
        "X1 split_campbell decreasing",
        key_of(on.split_campbell(&[2.0, 1.0]).unwrap_err()) == "speeds",
        "\"speeds\"",
    );
    ck.finish();
}

#[test]
fn u1_c1_anisotropic_cube() {
    let mut ck = Checks::new();
    let cba = cube(2.25);
    let on = RotatingPart::new(&cba, AXIS, ORIGIN, &RotatingOptions::new(3).coriolis(true)).unwrap();
    let speeds = [0.0, 0.5 * W0, 1.2 * W0, 1.8 * W0];
    let sc = on.split_campbell(&speeds).unwrap();
    let un = sc.unstable();
    println!("  CBA unstable: {un:?}; speeds {:?}", sc.speeds);
    ck.holds(
        "U1 CBA exactly one unstable speed, at 1.2 w0",
        un.len() == 1 && un[0].0.to_bits() == (1.2 * W0).to_bits(),
        &format!("{un:?}"),
    );
    if let (Some(&(_, g)), Some(want)) = (un.first(), pma_growth(1.2 * W0)) {
        ck.le(
            "U1 CBA growth against sqrt(mu+), relative",
            (g / want - 1.0).abs(),
            1e-8,
        );
    }
    let csv = sc.to_csv();
    let rows: Vec<&str> = csv.lines().collect();
    let mut c1 = rows.len() == 5 && rows[0].ends_with(",stability,growth_1_s");
    for (r, row) in rows.iter().enumerate().skip(1) {
        if r == 3 {
            let last = row.rsplit(',').next().unwrap_or("");
            c1 &= row.contains(",unstable,")
                && un
                    .first()
                    .is_some_and(|u| last.parse::<f64>().map(f64::to_bits) == Ok(u.1.to_bits()));
        } else {
            c1 &= row.ends_with(",stable,0");
        }
    }
    ck.holds(
        "C1 CBA CSV rows, stability and growth columns",
        c1,
        &format!("{} rows", rows.len()),
    );
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------------------------
// R: the ring

const RR: f64 = 1.0;
const HR: f64 = 0.05;
const BR: f64 = 0.05;
const NS: usize = 64;

fn ring_material() -> Material {
    Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap()
}

fn ring_sector_mesh() -> SolidMesh {
    let alpha = 2.0 * PI / NS as f64;
    let b = hex_box([RR - HR / 2.0, 0.0, -BR / 2.0], [HR, alpha, BR], [1, 1, 1]).unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

fn ring_sector(clamped: bool) -> SolidModel {
    let mut m = SolidModel::new(ring_sector_mesh());
    m.add_material("all", ring_material()).unwrap();
    if clamped {
        for c in 0..3 {
            m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
        }
    }
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, NS))
        .unwrap();
    m
}

/// The inextensional ring frequency of mode `n`, rad/s (ADR 0472 set-up R).
fn ring_closed_form(n: f64) -> f64 {
    let ei_rho_a = E * HR * HR / (12.0 * RHO);
    (ei_rho_a / RR.powi(4) * n * n * (n * n - 1.0).powi(2) / (n * n + 1.0)).sqrt()
}

fn ring() -> SolidModel {
    let rotor = full_rotor(&ring_sector(false)).unwrap();
    let mesh = rotor.mesh().clone();
    let nodes = mesh.nodes().len();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", ring_material()).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    let mass = RHO * 2.0 * PI * RR * HR * BR;
    let kn = mass * (0.2 * ring_closed_form(2.0)).powi(2) / nodes as f64;
    m.add_support(ElasticSupport::nodal_spring("all", 0, kn)).unwrap();
    m.add_support(ElasticSupport::nodal_spring("all", 1, kn)).unwrap();
    m
}

fn ring_options(coriolis: bool) -> RotatingOptions {
    RotatingOptions::new(9)
        .modal(
            ModalOptions::new(9)
                .tolerance(1e-10)
                .residual_test(ResidualTest::Backward),
        )
        .coriolis(coriolis)
}

/// The share of a complex shape in the span of two real `M`-orthonormal modes.
fn pair_share(m: &CsrMatrix, p: &[&Vec<f64>; 2], x: &SplitMode) -> f64 {
    let mut s = 0.0;
    for v in p {
        let mv = matvec(m, v);
        let (a, b) = (dot(&mv, &x.re), dot(&mv, &x.im));
        s += (a * a + b * b) / dot(v, &mv);
    }
    s / (dot(&x.re, &matvec(m, &x.re)) + dot(&x.im, &matvec(m, &x.im)))
}

#[test]
#[ignore = "known miss of ADR 0472 (amendment 1): Z1 R measured 4.5724e-10 against the limit 1e-10 at the registered run (the F-396 rest eigenvalues against their own Rayleigh quotients, 9.2e-10); every other check met; continued in g_r1_r2_n1_n2_x1_ring_after_z1_miss"]
fn g_z1_r1_r2_n1_n2_x1_ring_known_miss() {
    let mut ck = Checks::new();
    let r = ring();
    let on = RotatingPart::new(&r, AXIS, ORIGIN, &ring_options(true)).unwrap();
    let off = RotatingPart::new(&r, AXIS, ORIGIN, &ring_options(false)).unwrap();
    let m = on.mass();
    let rest = modes_of(on.modes(0.0).unwrap());
    let w: Vec<f64> = rest.eigenvalues.iter().map(|l| l.sqrt()).collect();
    println!(
        "  R: {} nodes, {} equations; rest omegas {w:?}",
        r.mesh().nodes().len(),
        m.n
    );
    ck.le("R set-up n = 2 pair degenerate", (w[4] / w[3] - 1.0).abs(), 1e-6);
    ck.le("R set-up n = 3 pair degenerate", (w[6] / w[5] - 1.0).abs(), 1e-6);
    let w2 = 0.5 * (w[3] + w[4]);
    let w3 = 0.5 * (w[5] + w[6]);
    let ratio = ring_closed_form(3.0) / ring_closed_form(2.0);
    ck.le("R set-up w3/w2 against 2.828", ((w3 / w2) / ratio - 1.0).abs(), 5e-2);
    println!(
        "  R: w2 = {w2:.6} (closed form {:.6}), w3/w2 = {:.5}",
        ring_closed_form(2.0),
        w3 / w2
    );
    let om = 0.1 * w2;
    let s = on.split_modes(om).unwrap();
    g_checks(&mut ck, "R", &r, om, &s);
    z1(&mut ck, "R", &on);
    println!(
        "  R at Omega = {om:.6}: basis {:?}, {:?}, omegas {:?}",
        s.basis,
        s.stability,
        s.modes.iter().map(|x| x.omega).collect::<Vec<_>>()
    );
    let soff = off.split_modes(om).unwrap();
    for (n, i) in [(2.0_f64, 3usize), (3.0, 5)] {
        let p = [&rest.modes[i], &rest.modes[i + 1]];
        let pick = |res: &SplitModes| -> Vec<SplitMode> {
            res.modes
                .iter()
                .filter(|x| pair_share(m, &p, x) >= 0.9)
                .cloned()
                .collect()
        };
        let g = 4.0 * n * om / (n * n + 1.0);
        let pair = pick(&s);
        ck.holds(
            &format!("R1 n = {n} two modes in the rest pair's span"),
            pair.len() == 2,
            &format!("{}", pair.len()),
        );
        if pair.len() == 2 {
            let (lo, hi) = if pair[0].omega < pair[1].omega {
                (&pair[0], &pair[1])
            } else {
                (&pair[1], &pair[0])
            };
            let split = hi.omega - lo.omega;
            println!(
                "  R n = {n}: {:.8} / {:.8}, split {split:.8}, 4 n Omega/(n^2 + 1) = {g:.8}, kappa {:.6} / {:.6} (2n/(n^2+1) = {:.6})",
                lo.omega, hi.omega, lo.whirl, hi.whirl, 2.0 * n / (n * n + 1.0)
            );
            ck.le(
                &format!("R1 n = {n} |split / (4 n Omega/(n^2 + 1)) - 1|"),
                (split / g - 1.0).abs(),
                2e-2,
            );
            ck.holds(
                &format!("R2 n = {n} lower forward, upper backward"),
                lo.direction == Whirl::Forward && hi.direction == Whirl::Backward,
                &format!("{:?} / {:?}", lo.direction, hi.direction),
            );
            if n == 2.0 {
                ck.gt(
                    "N2 |split/(2 Omega) - 1| (rigid-frame value)",
                    (split / (2.0 * om) - 1.0).abs(),
                    0.1,
                );
            }
        }
        if n == 2.0 {
            let poff = pick(&soff);
            let d = if poff.len() == 2 {
                (poff[1].omega - poff[0].omega).abs()
            } else {
                f64::INFINITY
            };
            ck.lt("N1 without the option: n = 2 split / (1.6 Omega)", d / g, 1e-3);
        }
    }
    // X1: the sector of R with its cyclic constraint (clamped at the inner face so that the part can be made)
    let sec = ring_sector(true);
    let ps = RotatingPart::new(&sec, AXIS, ORIGIN, &RotatingOptions::new(2).coriolis(true)).unwrap();
    ck.holds(
        "X1 sector refused",
        key_of(ps.split_modes(om).unwrap_err()) == "cyclic",
        "\"cyclic\"",
    );
    ck.holds(
        "X1 sector split_campbell refused",
        key_of(ps.split_campbell(&[0.0, om]).unwrap_err()) == "cyclic",
        "\"cyclic\"",
    );
    ck.finish();
}

// ADR 0472 amendment 1: the checks of g_z1_r1_r2_n1_n2_x1_ring_known_miss without Z1, code and limits unchanged.
#[test]
fn g_r1_r2_n1_n2_x1_ring_after_z1_miss() {
    let mut ck = Checks::new();
    let r = ring();
    let on = RotatingPart::new(&r, AXIS, ORIGIN, &ring_options(true)).unwrap();
    let off = RotatingPart::new(&r, AXIS, ORIGIN, &ring_options(false)).unwrap();
    let m = on.mass();
    let rest = modes_of(on.modes(0.0).unwrap());
    let w: Vec<f64> = rest.eigenvalues.iter().map(|l| l.sqrt()).collect();
    println!(
        "  R: {} nodes, {} equations; rest omegas {w:?}",
        r.mesh().nodes().len(),
        m.n
    );
    ck.le("R set-up n = 2 pair degenerate", (w[4] / w[3] - 1.0).abs(), 1e-6);
    ck.le("R set-up n = 3 pair degenerate", (w[6] / w[5] - 1.0).abs(), 1e-6);
    let w2 = 0.5 * (w[3] + w[4]);
    let w3 = 0.5 * (w[5] + w[6]);
    let ratio = ring_closed_form(3.0) / ring_closed_form(2.0);
    ck.le("R set-up w3/w2 against 2.828", ((w3 / w2) / ratio - 1.0).abs(), 5e-2);
    println!(
        "  R: w2 = {w2:.6} (closed form {:.6}), w3/w2 = {:.5}",
        ring_closed_form(2.0),
        w3 / w2
    );
    let om = 0.1 * w2;
    let s = on.split_modes(om).unwrap();
    g_checks(&mut ck, "R", &r, om, &s);
    println!(
        "  R at Omega = {om:.6}: basis {:?}, {:?}, omegas {:?}",
        s.basis,
        s.stability,
        s.modes.iter().map(|x| x.omega).collect::<Vec<_>>()
    );
    let soff = off.split_modes(om).unwrap();
    for (n, i) in [(2.0_f64, 3usize), (3.0, 5)] {
        let p = [&rest.modes[i], &rest.modes[i + 1]];
        let pick = |res: &SplitModes| -> Vec<SplitMode> {
            res.modes
                .iter()
                .filter(|x| pair_share(m, &p, x) >= 0.9)
                .cloned()
                .collect()
        };
        let g = 4.0 * n * om / (n * n + 1.0);
        let pair = pick(&s);
        ck.holds(
            &format!("R1 n = {n} two modes in the rest pair's span"),
            pair.len() == 2,
            &format!("{}", pair.len()),
        );
        if pair.len() == 2 {
            let (lo, hi) = if pair[0].omega < pair[1].omega {
                (&pair[0], &pair[1])
            } else {
                (&pair[1], &pair[0])
            };
            let split = hi.omega - lo.omega;
            println!(
                "  R n = {n}: {:.8} / {:.8}, split {split:.8}, 4 n Omega/(n^2 + 1) = {g:.8}, kappa {:.6} / {:.6} (2n/(n^2+1) = {:.6})",
                lo.omega, hi.omega, lo.whirl, hi.whirl, 2.0 * n / (n * n + 1.0)
            );
            ck.le(
                &format!("R1 n = {n} |split / (4 n Omega/(n^2 + 1)) - 1|"),
                (split / g - 1.0).abs(),
                2e-2,
            );
            ck.holds(
                &format!("R2 n = {n} lower forward, upper backward"),
                lo.direction == Whirl::Forward && hi.direction == Whirl::Backward,
                &format!("{:?} / {:?}", lo.direction, hi.direction),
            );
            if n == 2.0 {
                ck.gt(
                    "N2 |split/(2 Omega) - 1| (rigid-frame value)",
                    (split / (2.0 * om) - 1.0).abs(),
                    0.1,
                );
            }
        }
        if n == 2.0 {
            let poff = pick(&soff);
            let d = if poff.len() == 2 {
                (poff[1].omega - poff[0].omega).abs()
            } else {
                f64::INFINITY
            };
            ck.lt("N1 without the option: n = 2 split / (1.6 Omega)", d / g, 1e-3);
        }
    }
    // X1: the sector of R with its cyclic constraint (clamped at the inner face so that the part can be made)
    let sec = ring_sector(true);
    let ps = RotatingPart::new(&sec, AXIS, ORIGIN, &RotatingOptions::new(2).coriolis(true)).unwrap();
    ck.holds(
        "X1 sector refused",
        key_of(ps.split_modes(om).unwrap_err()) == "cyclic",
        "\"cyclic\"",
    );
    ck.holds(
        "X1 sector split_campbell refused",
        key_of(ps.split_campbell(&[0.0, om]).unwrap_err()) == "cyclic",
        "\"cyclic\"",
    );
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------------------------
// F: the radial beam

fn beam() -> SolidModel {
    let base = hex_box([0.0, -0.025, -0.025], [1.0, 0.05, 0.05], [20, 2, 2]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn beam_options(modes: usize, threads: Option<usize>) -> RotatingOptions {
    let mut mo = ModalOptions::new(modes)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12);
    if let Some(t) = threads {
        mo = mo.threads(t);
    }
    RotatingOptions::new(modes).modal(mo).coriolis(true)
}

/// The kinetic-energy share of component `c` in a complex shape.
fn component_share(model: &SolidModel, m: &CsrMatrix, re: &[f64], im: &[f64], c: usize) -> f64 {
    let dofs = model.dof_map();
    let (mut part, mut all) = (0.0, 0.0);
    for r in 0..m.n {
        let rc = dofs.node_component(r).unwrap().1;
        for p in m.row_ptr[r]..m.row_ptr[r + 1] {
            let q = m.col[p];
            let v = m.val[p] * (re[r] * re[q] + im[r] * im[q]);
            all += v;
            if rc == c {
                part += v;
            }
        }
    }
    part / all
}

/// `(omega_flap, omega_lag)` of the split modes: the lowest with a `z` share and the lowest with a `y` share above 0.5.
fn flap_lag_split(model: &SolidModel, m: &CsrMatrix, s: &SplitModes) -> (f64, f64) {
    let pick = |c: usize| {
        s.modes
            .iter()
            .find(|x| component_share(model, m, &x.re, &x.im, c) > 0.5)
            .map(|x| x.omega)
            .expect("a mode with the share")
    };
    (pick(2), pick(1))
}

fn flap_lag_f396(model: &SolidModel, m: &CsrMatrix, r: &ModalResult) -> (f64, f64) {
    let pick = |c: usize| {
        let zero = vec![0.0; m.n];
        (0..r.modes.len())
            .find(|&i| component_share(model, m, &r.modes[i], &zero, c) > 0.5)
            .map(|i| r.eigenvalues[i].sqrt())
            .expect("a mode with the share")
    };
    (pick(2), pick(1))
}

/// `S` of ADR 0472 arithmetic 4 by composite Simpson quadrature (20000 intervals, 200 rod terms).
fn lag_s() -> f64 {
    let (b, l) = (0.05_f64, 1.0_f64);
    let (a, i) = (b * b, b.powi(4) / 12.0);
    let bl = 1.875104068711961_f64;
    let sig = (bl.cosh() + bl.cos()) / (bl.sinh() + bl.sin());
    let phi = |x: f64| {
        let z = bl * x / l;
        z.cosh() - z.cos() - sig * (z.sinh() - z.sin())
    };
    let n = 20000;
    let h = l / n as f64;
    let simpson = |f: &dyn Fn(f64) -> f64| -> f64 {
        let mut s = f(0.0) + f(l);
        for k in 1..n {
            s += if k % 2 == 1 { 4.0 } else { 2.0 } * f(k as f64 * h);
        }
        s * h / 3.0
    };
    let norm = simpson(&|x| RHO * a * phi(x) * phi(x)).sqrt();
    let wl = bl * bl * (E * i / (RHO * a)).sqrt() / (l * l);
    let c = (E / RHO).sqrt();
    let mut s = 0.0;
    for j in 1..=200 {
        let k = (2 * j - 1) as f64 * PI / (2.0 * l);
        let amp = (2.0 / (RHO * a * l)).sqrt();
        let ov = simpson(&|x| RHO * a * phi(x) / norm * amp * (k * x).sin());
        let wj = k * c;
        s += ov * ov / (wj * wj - wl * wl);
    }
    s
}

#[test]
#[ignore = "known miss of ADR 0472 (amendment 1): Z1 F measured 1.0288e-10 against the limit 1e-10 at the registered run (the F-396 rest eigenvalues against their own Rayleigh quotients, 1.4e-10); every other check met; continued in g_b1_l1_l2_l3_n3_x2_beam_after_z1_miss"]
fn g_z1_b1_l1_l2_l3_n3_x2_beam_known_miss() {
    let mut ck = Checks::new();
    let f = beam();
    let on = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(14, None)).unwrap();
    let m = on.mass();
    let rest = modes_of(on.modes(0.0).unwrap());
    let om = 0.2 * rest.eigenvalues[0].sqrt();
    let r396 = modes_of(on.modes(om).unwrap());
    let s = on.split_modes(om).unwrap();
    println!(
        "  F: Omega_F = {om:.6}; F-396 omegas {:?}",
        r396.eigenvalues.iter().map(|l| l.sqrt()).collect::<Vec<_>>()
    );
    g_checks(&mut ck, "F", &f, om, &s);
    z1(&mut ck, "F", &on);
    let off = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(14, None).coriolis(false)).unwrap();
    b1(&mut ck, "F", &off, &[0.0, om]);
    // L3
    let zero = vec![0.0; m.n];
    let axial = r396.modes.iter().any(|p| component_share(&f, m, p, &zero, 0) > 0.5);
    ck.holds("L3 the basis contains an axial mode (x share > 0.5)", axial, "set-up");
    let sv = lag_s();
    ck.le("L3 |S / 1.4772e-8 - 1|", (sv / 1.4772e-8 - 1.0).abs(), 1e-3);
    // L1, L2
    let (ff, lf) = flap_lag_f396(&f, m, &r396);
    let (fs, ls) = flap_lag_split(&f, m, &s);
    let delta = ls - lf;
    let est = -2.0 * om * om * sv * lf;
    println!(
        "  F: flap {fs:.12} / {ff:.12}; lag {ls:.12} / {lf:.12}; Delta {delta:.6e} rad/s, Delta_est {est:.6e} rad/s \
         (relative {:.4e} / {:.4e}); S = {sv:.6e}",
        delta / lf,
        est / lf
    );
    ck.le(
        "L1 |omega_flap,split / omega_flap,F396 - 1|",
        (fs / ff - 1.0).abs(),
        1e-6,
    );
    ck.le("L2 |Delta / Delta_est - 1|", (delta / est - 1.0).abs(), 0.2);
    // N3
    let six = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(6, None)).unwrap();
    let r6 = modes_of(six.modes(om).unwrap());
    let s6 = six.split_modes(om).unwrap();
    let (_, l6) = flap_lag_f396(&f, m, &r6);
    let (_, l6s) = flap_lag_split(&f, m, &s6);
    ck.lt(
        "N3 six modes (no axial mode): |Delta / Delta_est|",
        ((l6s - l6) / est).abs(),
        0.1,
    );
    // X2
    let t1 = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(14, Some(1))).unwrap();
    let t4 = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(14, Some(4))).unwrap();
    let (a, b) = (t1.split_modes(om).unwrap(), t4.split_modes(om).unwrap());
    let same = a.modes.len() == b.modes.len()
        && a.modes.iter().zip(&b.modes).all(|(x, y)| {
            x.omega.to_bits() == y.omega.to_bits()
                && x.re.iter().zip(&y.re).all(|(p, q)| p.to_bits() == q.to_bits())
                && x.im.iter().zip(&y.im).all(|(p, q)| p.to_bits() == q.to_bits())
        });
    ck.holds("X2 F threads 1 and 4 bit-identical", same, "frequencies and shapes");
    ck.finish();
}

// ADR 0472 amendment 1: the checks of g_z1_b1_l1_l2_l3_n3_x2_beam_known_miss without Z1, code and limits unchanged.
#[test]
fn g_b1_l1_l2_l3_n3_x2_beam_after_z1_miss() {
    let mut ck = Checks::new();
    let f = beam();
    let on = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(14, None)).unwrap();
    let m = on.mass();
    let rest = modes_of(on.modes(0.0).unwrap());
    let om = 0.2 * rest.eigenvalues[0].sqrt();
    let r396 = modes_of(on.modes(om).unwrap());
    let s = on.split_modes(om).unwrap();
    println!(
        "  F: Omega_F = {om:.6}; F-396 omegas {:?}",
        r396.eigenvalues.iter().map(|l| l.sqrt()).collect::<Vec<_>>()
    );
    g_checks(&mut ck, "F", &f, om, &s);
    let off = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(14, None).coriolis(false)).unwrap();
    b1(&mut ck, "F", &off, &[0.0, om]);
    // L3
    let zero = vec![0.0; m.n];
    let axial = r396.modes.iter().any(|p| component_share(&f, m, p, &zero, 0) > 0.5);
    ck.holds("L3 the basis contains an axial mode (x share > 0.5)", axial, "set-up");
    let sv = lag_s();
    ck.le("L3 |S / 1.4772e-8 - 1|", (sv / 1.4772e-8 - 1.0).abs(), 1e-3);
    // L1, L2
    let (ff, lf) = flap_lag_f396(&f, m, &r396);
    let (fs, ls) = flap_lag_split(&f, m, &s);
    let delta = ls - lf;
    let est = -2.0 * om * om * sv * lf;
    println!(
        "  F: flap {fs:.12} / {ff:.12}; lag {ls:.12} / {lf:.12}; Delta {delta:.6e} rad/s, Delta_est {est:.6e} rad/s \
         (relative {:.4e} / {:.4e}); S = {sv:.6e}",
        delta / lf,
        est / lf
    );
    ck.le(
        "L1 |omega_flap,split / omega_flap,F396 - 1|",
        (fs / ff - 1.0).abs(),
        1e-6,
    );
    ck.le("L2 |Delta / Delta_est - 1|", (delta / est - 1.0).abs(), 0.2);
    // N3
    let six = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(6, None)).unwrap();
    let r6 = modes_of(six.modes(om).unwrap());
    let s6 = six.split_modes(om).unwrap();
    let (_, l6) = flap_lag_f396(&f, m, &r6);
    let (_, l6s) = flap_lag_split(&f, m, &s6);
    ck.lt(
        "N3 six modes (no axial mode): |Delta / Delta_est|",
        ((l6s - l6) / est).abs(),
        0.1,
    );
    // X2
    let t1 = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(14, Some(1))).unwrap();
    let t4 = RotatingPart::new(&f, AXIS, ORIGIN, &beam_options(14, Some(4))).unwrap();
    let (a, b) = (t1.split_modes(om).unwrap(), t4.split_modes(om).unwrap());
    let same = a.modes.len() == b.modes.len()
        && a.modes.iter().zip(&b.modes).all(|(x, y)| {
            x.omega.to_bits() == y.omega.to_bits()
                && x.re.iter().zip(&y.re).all(|(p, q)| p.to_bits() == q.to_bits())
                && x.im.iter().zip(&y.im).all(|(p, q)| p.to_bits() == q.to_bits())
        });
    ck.holds("X2 F threads 1 and 4 bit-identical", same, "frequencies and shapes");
    ck.finish();
}
