//! Rotor dynamics of a small fan rotor on two bearings, no structural FEM (fan breakdown E1, E2 and the E3 interface; ADR 0120).
//!
//! Model: a RIGID rotor (mass `m`, transverse inertia `It` about the centre of mass, polar inertia `Ip`) spinning at a constant
//! speed `spin` about `z`, carried by two ISOTROPIC bearings at axial positions `z_j` (measured from the centre of mass) with
//! radial stiffness `k_j`, viscous damping `c_j` and an optional hysteretic loss factor `eta_j`; the housing is rigid and grounded.
//! Four lateral degrees of freedom `x, y, theta_x, theta_y`; small motions. With isotropic bearings the problem is written in the
//! complex coordinates `w = x + i y` and `psi = theta_y - i theta_x`; a harmonic `exp(i lam t)` then gives a 2x2 complex system
//!
//! ```text
//! [ kt(lam) - m lam^2         kc(lam)                      ] [W  ]   [F]
//! [ kc(lam)                   kr(lam) - It lam^2 + Ip spin lam ] [Psi] = [M]
//! ```
//!
//! with `kt = sum d_j`, `kc = sum z_j d_j`, `kr = sum z_j^2 d_j`, `d_j(lam) = k_j (1 + i eta_j sgn(lam)) + i lam c_j`. `lam > 0` is
//! forward whirl, `lam < 0` backward whirl. A stationary-frame lateral harmonic at `omega` is split into its forward (`+omega`)
//! and backward (`-omega`) circular parts, each solved by Cramer's rule (the dependency-free "complex frequency-domain solver": the
//! workspace has real dense LU only, `rcfd_reactor::ode::lu_factor` and private copies, and no complex type; ADR 0120 D2).
//!
//! The hysteretic term carries `sgn(lam)`: it is an equivalent viscous damper `eta k / |lam|`, so it dissipates for both whirl
//! directions. Axial force and torque are not part of the lateral model: they pass to the mount unchanged (rigid axial and
//! torsional path at constant speed). NOT modelled: blade and shaft flexibility, housing flexibility, anisotropic or cross-coupled
//! bearings, bearing nonlinearity and clearance, rotor internal damping, thermal effects, speed fluctuation.

use rcfd_io::nastran::Deck;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// Complex number (the workspace has no complex type).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct C64 {
    /// Real part.
    pub re: f64,
    /// Imaginary part.
    pub im: f64,
}

impl C64 {
    /// `re + i im`.
    pub const fn new(re: f64, im: f64) -> Self {
        C64 { re, im }
    }
    /// The imaginary unit.
    pub const I: C64 = C64::new(0.0, 1.0);
    /// Complex conjugate.
    pub fn conj(self) -> Self {
        C64::new(self.re, -self.im)
    }
    /// Modulus.
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
    /// `r exp(i phi)`.
    pub fn polar(r: f64, phi: f64) -> Self {
        C64::new(r * phi.cos(), r * phi.sin())
    }
    /// Multiply by a real number.
    pub fn scale(self, s: f64) -> Self {
        C64::new(self.re * s, self.im * s)
    }
    fn is_finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }
}

impl Add for C64 {
    type Output = C64;
    fn add(self, o: C64) -> C64 {
        C64::new(self.re + o.re, self.im + o.im)
    }
}
impl Sub for C64 {
    type Output = C64;
    fn sub(self, o: C64) -> C64 {
        C64::new(self.re - o.re, self.im - o.im)
    }
}
impl Mul for C64 {
    type Output = C64;
    fn mul(self, o: C64) -> C64 {
        C64::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}
impl Div for C64 {
    type Output = C64;
    fn div(self, o: C64) -> C64 {
        let d = o.re * o.re + o.im * o.im;
        C64::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
}
impl Neg for C64 {
    type Output = C64;
    fn neg(self) -> C64 {
        C64::new(-self.re, -self.im)
    }
}

fn re(x: f64) -> C64 {
    C64::new(x, 0.0)
}

/// One isotropic bearing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bearing {
    /// Axial position from the centre of mass [m].
    pub z: f64,
    /// Radial stiffness [N/m], positive.
    pub k: f64,
    /// Viscous damping [N s/m], non-negative.
    pub c: f64,
    /// Hysteretic loss factor [-], non-negative (frequency domain only; ignored by [`natural_frequencies`]).
    pub eta: f64,
}

/// Rigid rotor on two bearings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rotor {
    /// Total mass [kg] (including any unbalance masses).
    pub mass: f64,
    /// Transverse moment of inertia about the centre of mass [kg m^2], non-negative.
    pub it: f64,
    /// Polar moment of inertia [kg m^2], non-negative.
    pub ip: f64,
    /// The two bearings.
    pub bearings: [Bearing; 2],
}

impl Rotor {
    /// Jeffcott (Laval) rotor: point mass `m` between two equal bearings at `z = -1, +1` m sharing stiffness `k` and damping `c`
    /// (each bearing `k/2`, `c/2`), no tilt inertia; its translation is `m w'' + c w' + k w = F`.
    pub fn jeffcott(m: f64, k: f64, c: f64) -> Rotor {
        let b = |z| Bearing {
            z,
            k: 0.5 * k,
            c: 0.5 * c,
            eta: 0.0,
        };
        Rotor {
            mass: m,
            it: 0.0,
            ip: 0.0,
            bearings: [b(-1.0), b(1.0)],
        }
    }

    /// Check the data; the error names the offending field.
    pub fn validate(&self) -> Result<(), String> {
        let pos = |v: f64, name: &str| -> Result<(), String> {
            if v > 0.0 && v.is_finite() {
                Ok(())
            } else {
                Err(format!("rotor: {name} = {v} must be positive and finite"))
            }
        };
        let nonneg = |v: f64, name: &str| -> Result<(), String> {
            if v >= 0.0 && v.is_finite() {
                Ok(())
            } else {
                Err(format!("rotor: {name} = {v} must be non-negative and finite"))
            }
        };
        pos(self.mass, "mass")?;
        nonneg(self.it, "it")?;
        nonneg(self.ip, "ip")?;
        for (j, b) in self.bearings.iter().enumerate() {
            if !b.z.is_finite() {
                return Err(format!("rotor: bearings[{j}].z = {} must be finite", b.z));
            }
            pos(b.k, &format!("bearings[{j}].k"))?;
            nonneg(b.c, &format!("bearings[{j}].c"))?;
            nonneg(b.eta, &format!("bearings[{j}].eta"))?;
        }
        if self.bearings[0].z == self.bearings[1].z {
            return Err(format!(
                "rotor: bearings[0].z and bearings[1].z are both {}: two bearings in one plane give no tilt stiffness",
                self.bearings[0].z
            ));
        }
        Ok(())
    }

    /// Dynamic stiffness `d_j(lam)` of bearing `j` (force on the housing per unit bearing displacement, complex coordinates).
    fn bearing_d(&self, j: usize, lam: f64) -> C64 {
        let b = self.bearings[j];
        let sgn = if lam > 0.0 {
            1.0
        } else if lam < 0.0 {
            -1.0
        } else {
            0.0
        };
        C64::new(b.k, b.k * b.eta * sgn + lam * b.c)
    }

    /// The 2x2 dynamic stiffness at signed whirl frequency `lam`.
    fn dyn_matrix(&self, spin: f64, lam: f64) -> [[C64; 2]; 2] {
        let (mut kt, mut kc, mut kr) = (C64::default(), C64::default(), C64::default());
        for j in 0..2 {
            let (d, z) = (self.bearing_d(j, lam), self.bearings[j].z);
            kt = kt + d;
            kc = kc + d.scale(z);
            kr = kr + d.scale(z * z);
        }
        [
            [kt - re(self.mass * lam * lam), kc],
            [kc, kr + re(-self.it * lam * lam + self.ip * spin * lam)],
        ]
    }

    /// Size of the terms that cancel in `det D(lam)` (bounds `|D11 D22|` and `|D12|^2`), for the singularity test.
    fn dyn_scale(&self, spin: f64, lam: f64) -> f64 {
        let (mut t, mut r) = (
            self.mass * lam * lam,
            self.it * lam * lam + self.ip * (spin * lam).abs(),
        );
        for j in 0..2 {
            let d = self.bearing_d(j, lam).abs();
            t += d;
            r += self.bearings[j].z * self.bearings[j].z * d;
        }
        t * r
    }
}

fn check_speed(v: f64, name: &str, positive: bool) -> Result<(), String> {
    if v.is_finite() && (v > 0.0 || (!positive && v == 0.0)) {
        Ok(())
    } else {
        let what = if positive { "positive" } else { "non-negative" };
        Err(format!("{name} = {v} must be {what} and finite"))
    }
}

/// Undamped synchronous (forward) critical speeds [rad/s], ascending: the positive roots `s = spin^2` of
/// `(kt - m s)(kr - (It - Ip) s) = kc^2`. A mode with `Ip >= It` (disc-like) has no synchronous crossing (its forward whirl stays
/// above the spin speed), so the list can hold one value or (`It = Ip` with `kc = 0` aside) two; it is never padded with NaN.
pub fn critical_speeds(rotor: &Rotor) -> Result<Vec<f64>, String> {
    rotor.validate()?;
    let (mut kt, mut kc, mut kr) = (0.0, 0.0, 0.0);
    for b in &rotor.bearings {
        kt += b.k;
        kc += b.z * b.k;
        kr += b.z * b.z * b.k;
    }
    let j = rotor.it - rotor.ip;
    let (a, b, c) = (rotor.mass * j, -(rotor.mass * kr + j * kt), kt * kr - kc * kc);
    let mut s = Vec::new();
    if a == 0.0 {
        s.push(-c / b);
    } else {
        // stable quadratic roots (no cancellation)
        let q = -0.5 * (b + b.signum() * (b * b - 4.0 * a * c).max(0.0).sqrt());
        s.push(q / a);
        s.push(c / q);
    }
    let mut w: Vec<f64> = s
        .into_iter()
        .filter(|v| *v > 0.0 && v.is_finite())
        .map(f64::sqrt)
        .collect();
    w.sort_by(|p, q| p.total_cmp(q));
    Ok(w)
}

fn pmul(a: &[C64], b: &[C64]) -> Vec<C64> {
    let mut o = vec![C64::default(); a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            o[i + j] = o[i + j] + *x * *y;
        }
    }
    o
}

fn peval(p: &[C64], x: C64) -> C64 {
    p.iter().rev().fold(C64::default(), |acc, c| acc * x + *c)
}

/// All roots of the polynomial `sum p[k] x^k` (Durand-Kerner, then Newton polish).
// shortcut: Durand-Kerner is accurate to ~sqrt(eps) only near a double root (coincident frequencies); use a companion-matrix QR
// eigen-solver if such rotors matter.
fn poly_roots(p: &[C64]) -> Result<Vec<C64>, String> {
    let mut p = p.to_vec();
    while p.len() > 1 && p.last().is_some_and(|c| c.abs() == 0.0) {
        p.pop();
    }
    let n = p.len() - 1;
    if n == 0 {
        return Ok(Vec::new());
    }
    let lead = p[n];
    let a: Vec<C64> = p.iter().map(|c| *c / lead).collect();
    let s = (0..n)
        .map(|k| a[k].abs().powf(1.0 / (n - k) as f64))
        .fold(0.0, f64::max);
    let seed = C64::new(0.4, 0.9);
    let mut z: Vec<C64> = Vec::with_capacity(n);
    let mut t = C64::new(1.0, 0.0);
    for _ in 0..n {
        t = t * seed;
        z.push(t.scale(s.max(1e-300)));
    }
    let mut converged = false;
    for _ in 0..2000 {
        let mut dmax: f64 = 0.0;
        for j in 0..n {
            let mut den = C64::new(1.0, 0.0);
            for l in 0..n {
                if l != j {
                    den = den * (z[j] - z[l]);
                }
            }
            let d = peval(&a, z[j]) / den;
            if d.is_finite() {
                z[j] = z[j] - d;
                dmax = dmax.max(d.abs());
            }
        }
        if dmax <= 1e-15 * s {
            converged = true;
            break;
        }
    }
    if !converged || z.iter().any(|v| !v.is_finite()) {
        return Err("natural_frequencies: the root iteration did not converge".into());
    }
    let dp: Vec<C64> = (1..=n).map(|k| a[k].scale(k as f64)).collect();
    for r in z.iter_mut() {
        for _ in 0..3 {
            let d = peval(&dp, *r);
            if d.abs() > 0.0 {
                let step = peval(&a, *r) / d;
                if step.is_finite() {
                    *r = *r - step;
                }
            }
        }
    }
    Ok(z)
}

/// Complex natural frequencies `lam` [rad/s] at spin speed `spin` (viscous damping included, hysteretic damping ignored): the roots
/// of `det D(lam) = 0`, motion `exp(i lam t)`. `re > 0` is forward whirl, `re < 0` backward whirl; `im > 0` is the decay rate
/// (damping ratio `im / |lam|`). Sorted by real part. Undamped: four real roots (fewer if `It = 0`).
pub fn natural_frequencies(rotor: &Rotor, spin: f64) -> Result<Vec<C64>, String> {
    rotor.validate()?;
    check_speed(spin, "natural_frequencies: spin", false)?;
    let (mut kt, mut kc, mut kr, mut ct, mut cc, mut cr) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for b in &rotor.bearings {
        kt += b.k;
        kc += b.z * b.k;
        kr += b.z * b.z * b.k;
        ct += b.c;
        cc += b.z * b.c;
        cr += b.z * b.z * b.c;
    }
    // ascending powers of lam
    let d11 = [re(kt), C64::new(0.0, ct), re(-rotor.mass)];
    let d12 = [re(kc), C64::new(0.0, cc)];
    let d22 = [re(kr), C64::new(rotor.ip * spin, cr), re(-rotor.it)];
    let a = pmul(&d11, &d22);
    let b = pmul(&d12, &d12);
    let det: Vec<C64> = (0..a.len())
        .map(|k| a[k] - b.get(k).copied().unwrap_or_default())
        .collect();
    let mut r = poly_roots(&det)?;
    r.sort_by(|p, q| p.re.total_cmp(&q.re));
    Ok(r)
}

/// Harmonic load on the rotor in stationary housing axes at circular frequency `omega` [rad/s]: `f(t) = Re(F exp(i omega t))`.
/// `force = [Fx, Fy, Fz]` [N], `moment = [Mx, My, Mz]` [N m] about the centre of mass. `Fz` and `Mz` are passed to the mount
/// unchanged. Loads taken from a rotating-frame (MRF) solution must first be turned to housing axes: a lateral rotor-frame
/// harmonic of order `k` appears at `(k + 1) spin` (forward) and `(k - 1) spin` (backward) in housing axes, so a steady
/// rotor-frame lateral force is a 1x rotating load like unbalance; stator and strut interaction loads are already in housing axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicLoad {
    /// Circular frequency [rad/s], positive.
    pub omega: f64,
    /// Force phasors.
    pub force: [C64; 3],
    /// Moment phasors about the centre of mass.
    pub moment: [C64; 3],
}

/// Steady response to one [`HarmonicLoad`]; every entry is a real-coordinate phasor `q(t) = Re(Q exp(i omega t))`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicResponse {
    /// Circular frequency [rad/s].
    pub omega: f64,
    /// Rotor motion `[x, y, theta_x, theta_y]` [m, rad] of the centre of mass.
    pub disp: [C64; 4],
    /// Lateral force ON THE HOUSING at each bearing, `[Fx, Fy]` [N].
    pub bearing_force: [[C64; 2]; 2],
    /// Resultant load on the (rigid, grounded) housing mount: the bearing forces plus the passed-through axial force [N].
    pub mount_force: [C64; 3],
    /// Resultant moment on the mount about the centre of mass, including the passed-through torque [N m].
    pub mount_moment: [C64; 3],
}

/// Cramer's rule for the 2x2 complex system; `scale` bounds the cancelling terms of the determinant.
fn solve2(a: [[C64; 2]; 2], b: [C64; 2], scale: f64, omega: f64) -> Result<[C64; 2], String> {
    if b[0].abs() == 0.0 && b[1].abs() == 0.0 {
        return Ok([C64::default(); 2]);
    }
    let det = a[0][0] * a[1][1] - a[0][1] * a[1][0];
    if !det.is_finite() || det.abs() <= 1e-12 * scale {
        return Err(format!(
            "response: the dynamic stiffness is singular at omega = {omega} rad/s (undamped resonance: add bearing damping or \
             move the frequency)"
        ));
    }
    Ok([
        (b[0] * a[1][1] - a[0][1] * b[1]) / det,
        (a[0][0] * b[1] - a[1][0] * b[0]) / det,
    ])
}

/// Steady response of `rotor` at spin speed `spin` to harmonic loads (E3 interface: unsteady blade-load harmonics at multiples of
/// the blade-passing frequency, or an unbalance from [`unbalance_load`]). One output per load, in the same order.
pub fn response(rotor: &Rotor, spin: f64, loads: &[HarmonicLoad]) -> Result<Vec<HarmonicResponse>, String> {
    rotor.validate()?;
    check_speed(spin, "response: spin", false)?;
    let half = 0.5;
    let mut out = Vec::with_capacity(loads.len());
    for (n, l) in loads.iter().enumerate() {
        check_speed(l.omega, &format!("response: loads[{n}].omega"), true)?;
        if l.force.iter().chain(l.moment.iter()).any(|v| !v.is_finite()) {
            return Err(format!("response: loads[{n}] has a non-finite force or moment"));
        }
        let [fx, fy, fz] = l.force;
        let [mx, my, mz] = l.moment;
        let i = C64::I;
        // forward (+omega) and backward (-omega) circular parts in w = x + i y, psi = theta_y - i theta_x
        let fwd = [(fx + i * fy).scale(half), (my - i * mx).scale(half)];
        let bwd = [
            (fx.conj() + i * fy.conj()).scale(half),
            (my.conj() - i * mx.conj()).scale(half),
        ];
        let [wf, pf] = solve2(
            rotor.dyn_matrix(spin, l.omega),
            fwd,
            rotor.dyn_scale(spin, l.omega),
            l.omega,
        )?;
        let [wb, pb] = solve2(
            rotor.dyn_matrix(spin, -l.omega),
            bwd,
            rotor.dyn_scale(spin, -l.omega),
            l.omega,
        )?;
        let to_xy = |a: C64, b: C64| (a + b.conj(), -i * (a - b.conj()));
        let (x, y) = to_xy(wf, wb);
        let (ty, tx_neg) = to_xy(pf, pb);
        let mut bearing_force = [[C64::default(); 2]; 2];
        let mut mount_force = [C64::default(), C64::default(), fz];
        let mut mount_moment = [C64::default(), C64::default(), mz];
        for j in 0..2 {
            let z = rotor.bearings[j].z;
            let qf = rotor.bearing_d(j, l.omega) * (wf + pf.scale(z));
            let qb = rotor.bearing_d(j, -l.omega) * (wb + pb.scale(z));
            let (qx, qy) = to_xy(qf, qb);
            bearing_force[j] = [qx, qy];
            mount_force[0] = mount_force[0] + qx;
            mount_force[1] = mount_force[1] + qy;
            mount_moment[0] = mount_moment[0] - qy.scale(z);
            mount_moment[1] = mount_moment[1] + qx.scale(z);
        }
        out.push(HarmonicResponse {
            omega: l.omega,
            disp: [x, y, -tx_neg, ty],
            bearing_force,
            mount_force,
            mount_moment,
        });
    }
    Ok(out)
}

/// Semi-major axis of the ellipse traced by the planar phasor pair `(X, Y)`: the peak amplitude of an orbit or a force.
pub fn ellipse_major(x: C64, y: C64) -> f64 {
    let i = C64::I;
    0.5 * ((x + i * y).abs() + (x.conj() + i * y.conj()).abs())
}

/// Point unbalance: `mass` [kg] at `radius` [m], angle `angle` [rad] from the rotor `x` axis at `t = 0`, axial position `z` [m]
/// from the centre of mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointUnbalance {
    /// Unbalance mass [kg].
    pub mass: f64,
    /// Radius [m].
    pub radius: f64,
    /// Angle [rad].
    pub angle: f64,
    /// Axial position from the centre of mass [m].
    pub z: f64,
}

/// Rotating load of a set of point unbalances at spin `spin` (E1): force `sum m r spin^2` rotating with the rotor, and the
/// moment of these forces about the centre of mass. A static unbalance is one mass (or masses) in the centre-of-mass plane; a
/// couple unbalance is two equal unbalances 180 degrees apart at different `z`. Amplitudes: `|force[0]|` = static force,
/// `max |moment|` = couple moment (circular, so `|Fx| = |Fy|`).
pub fn unbalance_load(u: &[PointUnbalance], spin: f64) -> Result<HarmonicLoad, String> {
    check_speed(spin, "unbalance_load: spin", true)?;
    let mut f = C64::default();
    let mut m = C64::default();
    for (n, p) in u.iter().enumerate() {
        if !(p.mass >= 0.0 && p.radius >= 0.0 && p.mass.is_finite() && p.radius.is_finite())
            || !p.angle.is_finite()
            || !p.z.is_finite()
        {
            return Err(format!(
                "unbalance_load: unbalance[{n}] needs a non-negative finite mass and radius and a finite angle and z"
            ));
        }
        let fi = C64::polar(p.mass * p.radius * spin * spin, p.angle);
        f = f + fi;
        m = m + fi.scale(p.z);
    }
    // f_x = Re(F e^{i spin t}), f_y = Re(-i F e^{i spin t}); M_x = -z f_y, M_y = z f_x
    let i = C64::I;
    Ok(HarmonicLoad {
        omega: spin,
        force: [f, -i * f, C64::default()],
        moment: [i * m, m, C64::default()],
    })
}

/// Balance-quality grades `G` [mm/s] of ISO 1940-1 (G 0.4 ... G 4000, consecutive grades a factor 2.5 apart, 6.3 = 2.5 x 2.5
/// rounded). UNVERIFIED: recalled, not read from the standard (which ISO 21940-11 is said to supersede, also UNVERIFIED). The
/// definition used below, `G = e spin`, is first principles and does not depend on this table.
pub const ISO1940_GRADES_UNVERIFIED: [f64; 11] = [0.4, 1.0, 2.5, 6.3, 16.0, 40.0, 100.0, 250.0, 630.0, 1600.0, 4000.0];

/// Balance grade `G = e spin` [mm/s] from the specific unbalance `e = U / m` [m] (the centre-of-mass offset) at spin [rad/s].
pub fn balance_grade(e: f64, spin: f64) -> Result<f64, String> {
    if !(e >= 0.0 && e.is_finite()) {
        return Err(format!(
            "balance_grade: specific unbalance e = {e} must be non-negative and finite"
        ));
    }
    check_speed(spin, "balance_grade: spin", true)?;
    Ok(e * spin * 1000.0)
}

/// Permissible residual unbalance `U_per = G m / spin` [kg m] (times 1e6 for g mm) for grade `grade` [mm/s], rotor mass [kg]
/// and maximum service speed `spin` [rad/s].
pub fn permissible_unbalance(grade: f64, mass: f64, spin: f64) -> Result<f64, String> {
    if !(grade > 0.0 && grade.is_finite()) {
        return Err(format!(
            "permissible_unbalance: grade = {grade} must be positive and finite"
        ));
    }
    if !(mass > 0.0 && mass.is_finite()) {
        return Err(format!(
            "permissible_unbalance: mass = {mass} must be positive and finite"
        ));
    }
    check_speed(spin, "permissible_unbalance: spin", true)?;
    Ok(grade / 1000.0 * mass / spin)
}

/// One-way structural load export (E4 hook, F-237 deck): the housing loads of `responses` as static `FORCE`/`MOMENT` load sets on
/// two grids at the bearing seats, `(0, 0, z_j)` with ids 1 and 2 in rotor axes (origin at the centre of mass). Harmonic `n` gives
/// set `sid0 + 2n` (real part) and `sid0 + 2n + 1` (imaginary part); the time load is `L(t) = L_re cos(omega t) - L_im sin(omega t)`.
/// Bearing `thrust_bearing` carries the passed-through axial force and torque. The deck resultant of each set equals
/// [`HarmonicResponse::mount_force`] / `mount_moment` (real or imaginary part).
pub fn load_deck(
    rotor: &Rotor,
    responses: &[HarmonicResponse],
    thrust_bearing: usize,
    sid0: i64,
) -> Result<Deck, String> {
    rotor.validate()?;
    if thrust_bearing > 1 {
        return Err(format!("load_deck: thrust_bearing = {thrust_bearing} must be 0 or 1"));
    }
    if sid0 <= 0 {
        return Err(format!("load_deck: sid0 = {sid0} must be positive"));
    }
    let mut deck = Deck::default();
    for (j, b) in rotor.bearings.iter().enumerate() {
        deck.grids.push((j as i64 + 1, [0.0, 0.0, b.z]));
    }
    for (n, r) in responses.iter().enumerate() {
        for (part, sid) in [(0, sid0 + 2 * n as i64), (1, sid0 + 2 * n as i64 + 1)] {
            let pick = |c: C64| if part == 0 { c.re } else { c.im };
            for j in 0..2 {
                let fz = if j == thrust_bearing {
                    pick(r.mount_force[2])
                } else {
                    0.0
                };
                deck.forces.push((
                    sid,
                    j as i64 + 1,
                    [pick(r.bearing_force[j][0]), pick(r.bearing_force[j][1]), fz],
                ));
            }
            deck.moments
                .push((sid, thrust_bearing as i64 + 1, [0.0, 0.0, pick(r.mount_moment[2])]));
        }
    }
    Ok(deck)
}
