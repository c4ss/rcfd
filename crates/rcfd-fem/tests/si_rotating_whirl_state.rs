//! F-505 (ADR 0416): whirl modes above the loss of stiffness: the state matrix of the gyroscopic problem by Hessenberg
//! reduction and the Francis QR algorithm, a stability flag with the growth rate, gyroscopic stabilisation of the point mass
//! and of the cube on springs, the unstable band of an anisotropic spring pair. Checks A1 to R1 and the controls N1, N2 of
//! the registration (A2 is the unit test of `rotating::whirl::state`).

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::rotating::{
    gyroscopic_eigen_dense, gyroscopic_state_eigen_dense, LossOfStiffness, RotatingOptions, RotatingPart, Stability,
    StateWhirl, StateWhirlMode, Whirl,
};
use rcfd_fem::{Constraint, ElasticSupport, FemError, Material, SolidModel};
use rcfd_linalg::CsrMatrix;

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
            "{id}: {value:.6e} (expected > {limit:.0e}) {}",
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
const W0: f64 = 1000.0;
const WY: f64 = 1.2 * W0;
const H: f64 = 0.1;
const MASS: f64 = 7.85;

fn material() -> Material {
    Material::isotropic_elastic(E, NU, RHO, 0.0).unwrap()
}

/// D5: `(lambda_+, lambda_-)`, `lambda_- = -(S + sqrt D)/2`, `lambda_+ = P / lambda_-`.
fn closed(wx: f64, wy: f64, w: f64) -> (f64, f64) {
    let s = wx * wx + wy * wy + 2.0 * w * w;
    let p = (wx * wx - w * w) * (wy * wy - w * w);
    let d = (wx * wx - wy * wy).powi(2) + 8.0 * w * w * (wx * wx + wy * wy);
    let lm = -(s + d.sqrt()) / 2.0;
    (p / lm, lm)
}

/// The point mass: `(K, M, G)` row-major 2 x 2.
fn point_mass(wx: f64, wy: f64, w: f64, coriolis: bool) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let k = vec![MASS * (wx * wx - w * w), 0.0, 0.0, MASS * (wy * wy - w * w)];
    let m = vec![MASS, 0.0, 0.0, MASS];
    let g = if coriolis {
        vec![0.0, -2.0 * w * MASS, 2.0 * w * MASS, 0.0]
    } else {
        vec![0.0; 4]
    };
    (k, m, g)
}

// ------------------------------------------------------------------------------------------------------ A1

fn lcg(state: &mut u64) -> f64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
}

fn spd(n: usize, state: &mut u64) -> Vec<f64> {
    let b: Vec<f64> = (0..n * n).map(|_| lcg(state)).collect();
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = (0..n).map(|k| b[i * n + k] * b[j * n + k]).sum::<f64>();
        }
        a[i * n + i] += n as f64;
    }
    a
}

#[test]
fn a1_state_solver_against_the_symmetric_solver() {
    let mut ch = Checks::new();
    let mut st = 7u64;
    let mut worst: f64 = 0.0;
    let mut stable = true;
    let mut q1: f64 = 0.0;
    for n in [1usize, 2, 5, 12] {
        let k = spd(n, &mut st);
        let m = spd(n, &mut st);
        let mut g = vec![0.0; n * n];
        for i in 0..n {
            for j in i + 1..n {
                let v = 3.0 * lcg(&mut st);
                g[i * n + j] = v;
                g[j * n + i] = -v;
            }
        }
        let reference = gyroscopic_eigen_dense(&k, &m, &g, n).unwrap();
        let r = gyroscopic_state_eigen_dense(&k, &m, &g, n).unwrap();
        let mut sref: Vec<[f64; 2]> = reference.omegas.iter().map(|w| [0.0, -w]).collect();
        sref.reverse();
        sref.extend(reference.omegas.iter().map(|w| [0.0, *w]));
        assert_eq!(r.eigenvalues.len(), 2 * n);
        let smax = r.eigenvalues.iter().map(|s| s[0].hypot(s[1])).fold(0.0f64, f64::max);
        let d = r
            .eigenvalues
            .iter()
            .zip(&sref)
            .map(|(s, t)| (s[0] - t[0]).hypot(s[1] - t[1]) / smax)
            .fold(0.0f64, f64::max);
        println!("A1 n = {n}: deviation {d:.3e}, real defect {:.3e}", r.real_defect);
        worst = worst.max(d);
        stable &= r.stability == Stability::Stable;
        q1 = r.residuals.iter().fold(q1, |a, &b| a.max(b));
    }
    ch.le("A1 eigenvalues against +/- i omega of the symmetric solver", worst, 1e-10);
    ch.holds("A1 flag", stable, "every random gyroscopic problem flagged Stable");
    println!("A1 (information) largest residual {q1:.3e}");
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ PMI: P1, P2, N2

/// `kappa` of a point-mass mode, `C = m [[0, -1], [1, 0]]`.
fn pm_kappa(re: &[f64], im: &[f64]) -> f64 {
    let c = [0.0, -MASS, MASS, 0.0];
    let cs = [c[0] * im[0] + c[1] * im[1], c[2] * im[0] + c[3] * im[1]];
    2.0 * (re[0] * cs[0] + re[1] * cs[1]) / (MASS * (re[0] * re[0] + re[1] * re[1] + im[0] * im[0] + im[1] * im[1]))
}

#[test]
fn p1_p2_n2_isotropic_point_mass_above_omega_0() {
    let mut ch = Checks::new();
    let (mut p1, mut q1): (f64, f64) = (0.0, 0.0);
    let mut flags = true;
    let mut backward = true;
    for f in [1.1, 1.5, 2.0, 3.0, 5.0] {
        let w = f * W0;
        let (k, m, g) = point_mass(W0, W0, w, true);
        let r = gyroscopic_state_eigen_dense(&k, &m, &g, 2).unwrap();
        assert_eq!(r.omegas.len(), 2, "defective set-up: {r:?}");
        let e0 = (r.omegas[0] / (w - W0) - 1.0).abs();
        let e1 = (r.omegas[1] / (w + W0) - 1.0).abs();
        let kap = [pm_kappa(&r.re[0], &r.im[0]), pm_kappa(&r.re[1], &r.im[1])];
        println!(
            "PMI {f} omega_0: omegas {:?}, growth {:?}, deviations {e0:.2e} {e1:.2e}, flag {:?}, real defect {:.2e}, \
             kappa {kap:?}, residuals {:?}",
            r.omegas, r.growth, r.stability, r.real_defect, r.residuals
        );
        p1 = p1.max(e0).max(e1);
        q1 = r.residuals.iter().fold(q1, |a, &b| a.max(b));
        flags &= r.stability == Stability::Stable;
        backward &= kap.iter().all(|&x| x <= -1.0 + 1e-8 && Whirl::of(x) == Whirl::Backward);
    }
    ch.le("P1 Omega -/+ omega_0 relative to each value", p1, 1e-8);
    ch.holds("P1 flag", flags, "flagged Stable at 1.1 .. 5 omega_0");
    ch.holds("P2", backward, "both modes kappa <= -1 + 1e-8 (backward)");
    ch.le("Q1 point mass isotropic residual", q1, 1e-9);
    // N2: no gyroscopic term.
    let w = 1.5 * W0;
    let (k, m, g) = point_mass(W0, W0, w, false);
    let r = gyroscopic_state_eigen_dense(&k, &m, &g, 2).unwrap();
    println!("N2: {:?} eigenvalues {:?}", r.stability, r.eigenvalues);
    match r.stability {
        Stability::Unstable { growth } => {
            ch.gt("N2 growth without G", growth, 1e3);
            ch.le(
                "N2 growth against sqrt(Omega^2 - omega_0^2)",
                (growth - (w * w - W0 * W0).sqrt()).abs() / W0,
                1e-8,
            );
        }
        Stability::Stable => ch.holds("N2", false, "flagged Unstable without G"),
    }
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ PMA: AN1, AN2

fn in_band(f: f64) -> bool {
    f > 1.0 && f < 1.2
}

#[test]
fn an1_an2_anisotropic_point_mass_band() {
    let mut ch = Checks::new();
    let (mut an1, mut q1): (f64, f64) = (0.0, 0.0);
    let mut flags = true;
    for f in [0.5, 0.9, 1.02, 1.05, 1.1, 1.15, 1.18, 1.3, 1.5, 2.0, 3.0] {
        let w = f * W0;
        let (k, m, g) = point_mass(W0, WY, w, true);
        let r = gyroscopic_state_eigen_dense(&k, &m, &g, 2).unwrap();
        let (lp, lm) = closed(W0, WY, w);
        assert_eq!(r.omegas.len(), 2, "defective set-up: {r:?}");
        q1 = r.residuals.iter().fold(q1, |a, &b| a.max(b));
        let dev = match (in_band(f), r.stability) {
            (true, Stability::Unstable { growth }) => {
                let eg = (growth - lp.sqrt()).abs() / W0;
                let ew = (r.omegas[1] - (-lm).sqrt()).abs() / W0;
                eg.max(ew)
            }
            (false, Stability::Stable) => {
                let e0 = (r.omegas[0] - (-lp).sqrt()).abs() / W0;
                let e1 = (r.omegas[1] - (-lm).sqrt()).abs() / W0;
                e0.max(e1)
            }
            _ => {
                flags = false;
                0.0
            }
        };
        println!(
            "PMA {f} omega_0: {:?}, omegas {:?}, growth {:?}, closed lambda+ {lp:.6e} lambda- {lm:.6e}, deviation {dev:.2e}",
            r.stability, r.omegas, r.growth
        );
        an1 = an1.max(dev);
    }
    ch.holds(
        "AN1 flags",
        flags,
        "Unstable exactly at 1.02 .. 1.18 omega_0, Stable at the other six",
    );
    ch.le("AN1 growth and frequencies against the closed form (/ omega_0)", an1, 1e-8);
    ch.le("Q1 point mass anisotropic residual", q1, 1e-9);
    // AN2: the band edges by bisection of the flag.
    let stable = |w: f64| {
        let (k, m, g) = point_mass(W0, WY, w, true);
        gyroscopic_state_eigen_dense(&k, &m, &g, 2).unwrap().stability.is_stable()
    };
    assert!(
        stable(0.95 * W0) && !stable(1.1 * W0) && stable(1.3 * W0),
        "defective set-up: brackets"
    );
    let (mut lo, mut hi) = (0.95 * W0, 1.1 * W0);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if stable(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let lower = 0.5 * (lo + hi);
    let (mut lo, mut hi) = (1.1 * W0, 1.3 * W0);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if stable(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let upper = 0.5 * (lo + hi);
    println!("AN2 edges {lower:.15e} {upper:.15e}");
    ch.le("AN2 lower edge against omega_x", (lower / W0 - 1.0).abs(), 1e-8);
    ch.le("AN2 upper edge against omega_y", (upper / WY - 1.0).abs(), 1e-8);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------ the cubes

fn cube(wx: f64, wy: f64) -> SolidModel {
    let mesh = hex_box([-H / 2.0; 3], [H; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    let mass = RHO * H * H * H;
    m.add_support(ElasticSupport::nodal_spring("all", 0, mass * wx * wx / 8.0))
        .unwrap();
    m.add_support(ElasticSupport::nodal_spring("all", 1, mass * wy * wy / 8.0))
        .unwrap();
    m
}

fn options(coriolis: bool) -> RotatingOptions {
    RotatingOptions::new(4).coriolis(coriolis)
}

fn uniform(model: &SolidModel, c: usize) -> Vec<f64> {
    let dofs = model.dof_map();
    let mut v = vec![0.0; dofs.n_equations()];
    for node in 0..model.mesh().nodes().len() {
        v[dofs.equation(node, c).unwrap()] = 1.0;
    }
    v
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(p, q)| p * q).sum()
}

/// The complex share of a mode in `span{1_x, 1_y}` (ADR 0411's `translation_share`).
fn translation_share(model: &SolidModel, m: &CsrMatrix, w: &StateWhirlMode) -> f64 {
    let mut s = 0.0;
    for c in 0..2 {
        let u = uniform(model, c);
        let mu = matvec(m, &u);
        let (a, b) = (dot(&mu, &w.re), dot(&mu, &w.im));
        s += (a * a + b * b) / dot(&u, &mu);
    }
    s / (dot(&w.re, &matvec(m, &w.re)) + dot(&w.im, &matvec(m, &w.im)))
}

fn pair_of<'m>(model: &SolidModel, m: &CsrMatrix, r: &'m StateWhirl) -> Vec<&'m StateWhirlMode> {
    r.modes
        .iter()
        .filter(|w| translation_share(model, m, w) >= 1.0 - 1e-8)
        .collect()
}

fn negative(r: &StateWhirl) -> Option<Option<usize>> {
    r.loss.map(|l: LossOfStiffness| l.negative)
}

#[test]
fn b1_x1_f1_f2_f3_p3_n1_isotropic_cube() {
    let mut ch = Checks::new();
    let cb = cube(W0, W0);
    let part = RotatingPart::new(&cb, AXIS, ORIGIN, &options(true)).unwrap();
    let plain = RotatingPart::new(&cb, AXIS, ORIGIN, &options(false)).unwrap();
    let m = part.mass();
    let mut q1: f64 = 0.0;
    // X1: below the loss, against ADR 0411's solver.
    let w = 0.5 * W0;
    let x = part.whirl_state(w).unwrap();
    let reference = match part.whirl_modes(w).unwrap() {
        rcfd_fem::rotating::WhirlSpeedModes::Modes(r) => r,
        other => panic!("defective set-up: {other:?}"),
    };
    let freqs: Vec<f64> = x.eigenvalues.iter().filter(|s| s[1] > 0.0).map(|s| s[1]).collect();
    let x1 = freqs
        .iter()
        .zip(&reference.frequencies_all)
        .map(|(a, b)| (a / b - 1.0).abs())
        .fold(0.0f64, f64::max);
    ch.holds(
        "X1 count",
        freqs.len() == reference.frequencies_all.len() && x.unknowns == reference.unknowns,
        "as many positive frequencies as ADR 0411's frequencies_all",
    );
    ch.le("X1 frequencies against whirl_modes at 0.5 omega_0", x1, 1e-9);
    ch.holds(
        "X1 flag",
        x.loss.is_none() && x.stability == Stability::Stable,
        "loss None, flagged Stable",
    );
    q1 = x.modes.iter().fold(q1, |a, w| a.max(w.residual));
    // B1 and P3.
    let mut p3: f64 = 0.0;
    let mut p3_ok = true;
    for f in [1.2, 1.5] {
        let w = f * W0;
        let r = part.whirl_state(w).unwrap();
        let pair = pair_of(&cb, m, &r);
        println!(
            "CBI {f} omega_0: loss {:?}, {:?}, real defect {:.2e}, omegas {:?}, growth {:?}, kappa {:?}, residuals {:?}",
            r.loss,
            r.stability,
            r.real_defect,
            r.modes.iter().map(|x| x.omega).collect::<Vec<_>>(),
            r.modes.iter().map(|x| x.growth).collect::<Vec<_>>(),
            r.modes.iter().map(|x| x.whirl).collect::<Vec<_>>(),
            r.modes.iter().map(|x| x.residual).collect::<Vec<_>>()
        );
        if f == 1.2 {
            ch.holds(
                "B1",
                negative(&r) == Some(Some(2))
                    && r.eigenvalues.len() == 2 * r.unknowns
                    && r.unknowns == 16
                    && r.eigenvalues.iter().all(|s| s[0].is_finite() && s[1].is_finite())
                    && r.eigenvalues.iter().filter(|s| s[1] > 0.0 || (s[1] == 0.0 && s[0] >= 0.0)).count() == 16,
                "loss with negative Some(2), 32 finite eigenvalues, 16 reported",
            );
        }
        q1 = r.modes.iter().fold(q1, |a, w| a.max(w.residual));
        if pair.len() != 2 {
            p3_ok = false;
            println!("P3: the translation pair is not among the modes at {f} omega_0");
            continue;
        }
        p3 = p3
            .max((pair[0].omega / (w - W0) - 1.0).abs())
            .max((pair[1].omega / (w + W0) - 1.0).abs());
        p3_ok &= negative(&r) == Some(Some(2))
            && r.stability == Stability::Stable
            && pair
                .iter()
                .all(|x| x.whirl <= -1.0 + 1e-8 && x.direction == Whirl::Backward);
    }
    ch.le("P3 cube translation pair against Omega -/+ omega_0", p3, 1e-8);
    ch.holds(
        "P3",
        p3_ok,
        "negative Some(2), flagged Stable, both translation modes backward with kappa <= -1 + 1e-8",
    );
    // F1 / N1: no Coriolis.
    let w = 1.2 * W0;
    let r = plain.whirl_state(w).unwrap();
    println!("F1: {:?}", r.stability);
    match r.stability {
        Stability::Unstable { growth } => {
            ch.le(
                "F1 growth against sqrt(Omega^2 - omega_0^2)",
                (growth - (w * w - W0 * W0).sqrt()).abs() / W0,
                1e-8,
            );
            ch.gt("N1 growth without Coriolis", growth, 1e2);
        }
        Stability::Stable => ch.holds("F1", false, "flagged Unstable without Coriolis"),
    }
    // F2: an odd index at 2 omega_0.
    let r2 = part.whirl_state(2.0 * W0).unwrap();
    println!("F2: loss {:?}, {:?}", r2.loss, r2.stability);
    ch.holds(
        "F2",
        negative(&r2) == Some(Some(3)) && matches!(r2.stability, Stability::Unstable { growth } if growth > 0.0),
        "negative Some(3) (odd) and flagged Unstable with growth > 0",
    );
    // F3: the table.
    let speeds = [0.5 * W0, 1.2 * W0, 1.5 * W0, 2.0 * W0];
    let table = part.whirl_stability(&speeds).unwrap();
    let mut same = table.results.len() == 4;
    for (w, r) in speeds.iter().zip(&table.results) {
        same &= *r == part.whirl_state(*w).unwrap();
    }
    let csv = table.to_csv();
    print!("F3 CSV:\n{csv}");
    let lines: Vec<&str> = csv.lines().collect();
    let mut csv_ok = lines.len() == 5;
    let stable_col = ["true", "true", "true", "false"];
    let negative_col = ["0", "2", "2", "3"];
    for (j, line) in lines.iter().skip(1).enumerate() {
        let fields: Vec<&str> = line.split(',').collect();
        let r = &table.results[j];
        csv_ok &= fields.len() == 4 + r.modes.len();
        csv_ok &= fields.get(1) == Some(&negative_col[j]) && fields.get(2) == Some(&stable_col[j]);
        let growth = match r.stability {
            Stability::Stable => 0.0,
            Stability::Unstable { growth } => growth,
        };
        csv_ok &= fields.get(3).and_then(|s| s.parse::<f64>().ok()).map(f64::to_bits) == Some(growth.to_bits());
        for (i, mode) in r.modes.iter().enumerate() {
            csv_ok &= fields.get(4 + i).and_then(|s| s.parse::<f64>().ok()).map(f64::to_bits)
                == Some(mode.frequency.to_bits());
        }
    }
    ch.holds("F3 results", same, "whirl_stability equals whirl_state at each speed");
    ch.holds(
        "F3 CSV",
        csv_ok,
        "5 lines, stable true true true false, negative 0 2 2 3, numbers round-trip",
    );
    ch.le("Q1 cube isotropic residual", q1, 1e-9);
    ch.finish();
}

#[test]
fn an3_anisotropic_cube_band() {
    let mut ch = Checks::new();
    let cb = cube(W0, WY);
    let part = RotatingPart::new(&cb, AXIS, ORIGIN, &options(true)).unwrap();
    let m = part.mass();
    let (mut an3, mut q1): (f64, f64) = (0.0, 0.0);
    let mut flags = true;
    let mut losses = true;
    let expected_loss = [None, Some(Some(1)), Some(Some(1)), Some(Some(1)), Some(Some(2))];
    for (j, f) in [0.9, 1.05, 1.1, 1.18, 1.3].into_iter().enumerate() {
        let w = f * W0;
        let r = part.whirl_state(w).unwrap();
        let (lp, lm) = closed(W0, WY, w);
        q1 = r.modes.iter().fold(q1, |a, x| a.max(x.residual));
        losses &= negative(&r) == expected_loss[j];
        let mut freqs: Vec<f64> = pair_of(&cb, m, &r)
            .iter()
            .filter(|x| x.omega > 0.0)
            .map(|x| x.omega)
            .collect();
        freqs.sort_by(f64::total_cmp);
        let expected: Vec<f64> = if in_band(f) {
            vec![(-lm).sqrt()]
        } else {
            vec![(-lp).sqrt(), (-lm).sqrt()]
        };
        let mut dev = if freqs.len() == expected.len() {
            freqs
                .iter()
                .zip(&expected)
                .map(|(a, b)| (a - b).abs() / W0)
                .fold(0.0f64, f64::max)
        } else {
            flags = false;
            f64::INFINITY
        };
        match (in_band(f), r.stability) {
            (true, Stability::Unstable { growth }) => dev = dev.max((growth - lp.sqrt()).abs() / W0),
            (false, Stability::Stable) => {}
            _ => flags = false,
        }
        println!(
            "CBA {f} omega_0: loss {:?}, {:?}, translation frequencies {freqs:?} (closed {expected:?}), deviation {dev:.2e}",
            r.loss, r.stability
        );
        an3 = an3.max(dev);
    }
    ch.holds(
        "AN3 flags",
        flags,
        "Unstable exactly at 1.05, 1.1, 1.18 omega_0; the translation modes found",
    );
    ch.le("AN3 growth and translation frequencies (/ omega_0)", an3, 1e-8);
    ch.holds("AN3 losses", losses, "None, Some(1) x 3, Some(2)");
    ch.le("Q1 cube anisotropic residual", q1, 1e-9);
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
    let cb = cube(W0, W0);
    let part = RotatingPart::new(&cb, AXIS, ORIGIN, &options(true)).unwrap();
    ch.holds(
        "R1 speed -1",
        key_of(part.whirl_state(-1.0).unwrap_err()) == "speed",
        "refused with \"speed\"",
    );
    ch.holds(
        "R1 speed NaN",
        key_of(part.whirl_state(f64::NAN).unwrap_err()) == "speed",
        "refused with \"speed\"",
    );
    ch.holds(
        "R1 speeds decreasing",
        key_of(part.whirl_stability(&[0.0, 2.0, 1.0]).unwrap_err()) == "speeds",
        "refused with \"speeds\"",
    );
    ch.holds(
        "R1 dense wrong length",
        key_of(gyroscopic_state_eigen_dense(&[1.0], &[1.0, 0.0, 0.0, 1.0], &[0.0; 4], 2).unwrap_err()) == "matrix",
        "refused with \"matrix\"",
    );
    ch.holds(
        "R1 dense mass not positive definite",
        key_of(gyroscopic_state_eigen_dense(&[1.0, 0.0, 0.0, 1.0], &[-1.0, 0.0, 0.0, 1.0], &[0.0; 4], 2).unwrap_err())
            == "mass",
        "refused with \"mass\"",
    );
    ch.holds(
        "R1 dense too large",
        key_of(gyroscopic_state_eigen_dense(&[], &[], &[], 1501).unwrap_err()) == "unknowns",
        "refused with \"unknowns\"",
    );
    ch.finish();
}
