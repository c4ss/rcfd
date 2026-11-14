//! Aerodynamic damping and added mass of a mode by prescribed wall motion (the energy method). Declared by ADR 0440 (F-408,
//! the scaffold of ADR 0311); filled by ADR 0446 (the registration of F-421; design stub ADR 0324).
//!
//! The mode coordinate is prescribed as `q(t) = q0 sin(w t)` from `t = 0`; [`drive_mode`] moves the wall of a
//! `cosim::Fluid` in a mode shape through an `InterfaceMap` and records the generalised force `Q` (the fluid's force on the
//! structure projected on the mode by the transposed map, the power-conjugate of `q`). [`Harmonic::fit`] fits `Q ~ Q_m + Q_p
//! sin(w t) + Q_q cos(w t)` by least squares; [`periodic_response`] fits every cycle and refuses a history whose first
//! harmonic still changes from cycle to cycle. From the in-phase part `Q_p` and the quadrature part `Q_q`: the added mass
//! `Q_p/(w^2 q0)`, the damping `-Q_q/(w q0)`, the work per cycle `pi q0 Q_q` and the damping ratio `-W/(2 pi w^2 q0^2 m)` of
//! a mode of modal mass `m` (ADR 0446 D1).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::cosim::{Fluid, InterfaceMap};
use std::f64::consts::PI;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The fit `Q(t) ~ mean + in_phase sin(w t) + quadrature cos(w t)` of a force history against the motion `q0 sin(w t)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Harmonic {
    /// The mean `Q_m`.
    pub mean: f64,
    /// The part in phase with the displacement, `Q_p`.
    pub in_phase: f64,
    /// The part in phase with the velocity, `Q_q`.
    pub quadrature: f64,
}

fn finite_positive(what: &str, x: f64) -> Result<(), String> {
    if x.is_finite() && x > 0.0 {
        Ok(())
    } else {
        Err(format!(
            "aerodynamic damping: {what} must be positive and finite, got {x}"
        ))
    }
}

impl Harmonic {
    /// Least-squares fit of `values` at `times` (at least three samples) by a mean, `sin(omega t)` and `cos(omega t)`.
    /// On uniform samples that cover one period the result is the discrete Fourier coefficients.
    pub fn fit(times: &[f64], values: &[f64], omega: f64) -> Result<Harmonic, String> {
        finite_positive("the frequency", omega)?;
        if times.len() != values.len() {
            return Err(format!(
                "aerodynamic damping: {} times for {} values",
                times.len(),
                values.len()
            ));
        }
        if times.len() < 3 {
            return Err(format!(
                "aerodynamic damping: a fit needs at least 3 samples, got {}",
                times.len()
            ));
        }
        if !times.iter().chain(values).all(|x| x.is_finite()) {
            return Err("aerodynamic damping: a non-finite sample".into());
        }
        let mut a = [[0.0f64; 4]; 3];
        for (&t, &y) in times.iter().zip(values) {
            let b = [1.0, (omega * t).sin(), (omega * t).cos()];
            for i in 0..3 {
                for j in 0..3 {
                    a[i][j] += b[i] * b[j];
                }
                a[i][3] += b[i] * y;
            }
        }
        let x = solve3(a).ok_or("aerodynamic damping: the samples do not determine the harmonic (singular fit)")?;
        Ok(Harmonic {
            mean: x[0],
            in_phase: x[1],
            quadrature: x[2],
        })
    }

    /// Euclidean norm of the first harmonic `(in_phase, quadrature)`.
    pub fn amplitude(&self) -> f64 {
        self.in_phase.hypot(self.quadrature)
    }
}

/// Gaussian elimination with partial pivoting of a 3 x 3 system given as an augmented matrix.
fn solve3(mut a: [[f64; 4]; 3]) -> Option<[f64; 3]> {
    let scale = a.iter().flat_map(|r| r[..3].iter()).fold(0.0f64, |m, x| m.max(x.abs()));
    for k in 0..3 {
        let p = (k..3).max_by(|&i, &j| a[i][k].abs().total_cmp(&a[j][k].abs()))?;
        if a[p][k].abs() <= 1e-14 * scale {
            return None;
        }
        a.swap(k, p);
        for i in k + 1..3 {
            let f = a[i][k] / a[k][k];
            for j in k..4 {
                a[i][j] -= f * a[k][j];
            }
        }
    }
    let mut x = [0.0; 3];
    for k in (0..3).rev() {
        let s: f64 = (k + 1..3).map(|j| a[k][j] * x[j]).sum();
        x[k] = (a[k][3] - s) / a[k][k];
    }
    Some(x)
}

/// The work of the force `values` along the motion `q0 sin(omega t)` by the periodic trapezoid rule, `sum_j Q_j q0 omega
/// cos(omega t_j) dt` (uniform samples covering whole periods; exact for trigonometric polynomials of degree below the
/// number of samples per period less one).
pub fn trapezoid_work(times: &[f64], values: &[f64], q0: f64, omega: f64, dt: f64) -> f64 {
    times
        .iter()
        .zip(values)
        .map(|(&t, &y)| y * q0 * omega * (omega * t).cos() * dt)
        .sum()
}

/// The periodic response of a mode: the fit of the last cycle and the derived quantities (ADR 0446 D1).
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodicResponse {
    /// Amplitude `q0` of the mode coordinate.
    pub amplitude: f64,
    /// Angular frequency `w` [rad/s].
    pub omega: f64,
    /// The fit of the last cycle.
    pub harmonic: Harmonic,
    /// The fit of every cycle.
    pub cycles: Vec<Harmonic>,
    /// `|H_C - H_{C-1}|/|H_C|` of the first harmonics of the last two cycles.
    pub cycle_change: f64,
}

impl PeriodicResponse {
    /// The in-phase part read as a mass, `Q_p/(w^2 q0)`.
    pub fn added_mass(&self) -> f64 {
        self.harmonic.in_phase / (self.omega * self.omega * self.amplitude)
    }

    /// The in-phase part read as a stiffness, `-Q_p/q0`.
    pub fn added_stiffness(&self) -> f64 {
        -self.harmonic.in_phase / self.amplitude
    }

    /// The damping `c = -Q_q/(w q0)` (positive when the fluid takes energy from the mode).
    pub fn damping(&self) -> f64 {
        -self.harmonic.quadrature / (self.omega * self.amplitude)
    }

    /// The impedance (force resisting the motion per velocity, `e^{i w t}` convention) `(c, w m_a)`: real and imaginary part.
    pub fn impedance(&self) -> (f64, f64) {
        (self.damping(), self.omega * self.added_mass())
    }

    /// The work of the fluid on the mode per cycle, `pi q0 Q_q` (negative when the fluid damps the mode).
    pub fn work_per_cycle(&self) -> f64 {
        PI * self.amplitude * self.harmonic.quadrature
    }

    /// The damping ratio of a mode of modal mass `modal_mass` (all the mass that moves with the mode), `-W/(2 pi w^2 q0^2
    /// m)`.
    pub fn damping_ratio(&self, modal_mass: f64) -> f64 {
        -self.work_per_cycle() / (2.0 * PI * self.omega * self.omega * self.amplitude * self.amplitude * modal_mass)
    }
}

/// The response of a history `values` sampled at `t_n = n dt`, `n = 1 ..= cycles steps_per_cycle`, `dt = 2 pi/(omega
/// steps_per_cycle)`, against the motion `amplitude sin(omega t)`: every cycle fitted; refused unless the first harmonic of
/// the last two cycles changes by at most `tolerance` relative, the error naming the change.
pub fn periodic_response(
    values: &[f64],
    steps_per_cycle: usize,
    amplitude: f64,
    omega: f64,
    tolerance: f64,
) -> Result<PeriodicResponse, String> {
    finite_positive("the amplitude", amplitude)?;
    finite_positive("the frequency", omega)?;
    finite_positive("the periodicity tolerance", tolerance)?;
    if steps_per_cycle < 8 {
        return Err(format!(
            "aerodynamic damping: at least 8 steps per cycle are needed, got {steps_per_cycle}"
        ));
    }
    let n_cycles = values.len() / steps_per_cycle;
    if n_cycles < 2 || values.len() != n_cycles * steps_per_cycle {
        return Err(format!(
            "aerodynamic damping: the history of {} samples is not at least 2 whole cycles of {steps_per_cycle} steps",
            values.len()
        ));
    }
    let dt = 2.0 * PI / (omega * steps_per_cycle as f64);
    let mut cycles = Vec::with_capacity(n_cycles);
    for c in 0..n_cycles {
        let range = c * steps_per_cycle..(c + 1) * steps_per_cycle;
        let times: Vec<f64> = range.clone().map(|n| (n + 1) as f64 * dt).collect();
        cycles.push(Harmonic::fit(&times, &values[range], omega)?);
    }
    let (last, prev) = (cycles[n_cycles - 1], cycles[n_cycles - 2]);
    let change = (last.in_phase - prev.in_phase).hypot(last.quadrature - prev.quadrature) / last.amplitude();
    if !(change <= tolerance) {
        return Err(format!(
            "aerodynamic damping: the force history is not periodic: the cycle-to-cycle change of the first harmonic between cycles {} and {} is {change:e} (tolerance {tolerance:e})",
            n_cycles - 1,
            n_cycles
        ));
    }
    Ok(PeriodicResponse {
        amplitude,
        omega,
        harmonic: last,
        cycles,
        cycle_change: change,
    })
}

/// A prescribed motion of the wall in a mode: `shape` (one vector per interface node of the map) times `amplitude
/// sin(omega t)`, over `cycles` cycles of `steps_per_cycle` steps.
#[derive(Debug, Clone, PartialEq)]
pub struct ModeMotion {
    /// The mode shape on the interface nodes.
    pub shape: Vec<Vec3>,
    /// Amplitude `q0` of the mode coordinate.
    pub amplitude: f64,
    /// Angular frequency `w` [rad/s].
    pub omega: f64,
    /// Steps per cycle `N`.
    pub steps_per_cycle: usize,
    /// Number of cycles `C`.
    pub cycles: usize,
}

impl ModeMotion {
    /// The time step `2 pi/(w N)`.
    pub fn dt(&self) -> f64 {
        2.0 * PI / (self.omega * self.steps_per_cycle as f64)
    }

    /// The mode coordinate at `t`.
    pub fn q(&self, t: f64) -> f64 {
        self.amplitude * (self.omega * t).sin()
    }
}

/// The history of a prescribed mode motion.
#[derive(Debug, Clone, PartialEq)]
pub struct ModeHistory {
    /// The motion.
    pub motion: ModeMotion,
    /// The generalised force `Q_n` at `t_n = n dt`, `n = 1 ..= C N`.
    pub generalised: Vec<f64>,
    /// The face forces at the end of the last step.
    pub last_face_forces: Vec<Vec3>,
    /// Cumulative work of the face forces along the face displacements (trapezoidal in each step) at `t_0 ..= t_{C N}`.
    pub face_work: Vec<f64>,
}

impl ModeHistory {
    /// The periodic response of the history (refused if not periodic within `tolerance`).
    pub fn response(&self, tolerance: f64) -> Result<PeriodicResponse, String> {
        periodic_response(
            &self.generalised,
            self.motion.steps_per_cycle,
            self.motion.amplitude,
            self.motion.omega,
            tolerance,
        )
    }
}

/// Moves the wall of `fluid` in the mode of `motion` through `map` (ADR 0446 D2). The fluid must start with its wall at
/// the reference position (`q(0) = 0`). Refuses a shape of the wrong length or non-finite, fewer than 8 steps per cycle or
/// 2 cycles, a non-positive or non-finite amplitude or frequency; an error of the fluid or the map stops the run naming the
/// step.
pub fn drive_mode(fluid: &mut dyn Fluid, map: &InterfaceMap, motion: &ModeMotion) -> Result<ModeHistory, String> {
    finite_positive("the amplitude", motion.amplitude)?;
    finite_positive("the frequency", motion.omega)?;
    if motion.shape.len() != map.n_nodes() {
        return Err(format!(
            "aerodynamic damping: the mode shape has {} vectors for {} interface nodes",
            motion.shape.len(),
            map.n_nodes()
        ));
    }
    if !motion.shape.iter().flatten().all(|x| x.is_finite()) {
        return Err("aerodynamic damping: a non-finite mode shape".into());
    }
    if motion.steps_per_cycle < 8 {
        return Err(format!(
            "aerodynamic damping: at least 8 steps per cycle are needed, got {}",
            motion.steps_per_cycle
        ));
    }
    if motion.cycles < 2 {
        return Err(format!(
            "aerodynamic damping: at least 2 cycles are needed, got {}",
            motion.cycles
        ));
    }
    let dt = motion.dt();
    let steps = motion.steps_per_cycle * motion.cycles;
    let at = |n: usize| -> Vec<Vec3> {
        let q = motion.q(n as f64 * dt);
        motion.shape.iter().map(|s| vec3::scale(*s, q)).collect()
    };
    let faces_of =
        |nodes: &[Vec3]| -> Result<Vec<Vec3>, String> { map.face_displacements(&map.displacement_to_points(nodes)?) };
    let mut generalised = Vec::with_capacity(steps);
    let mut face_work = Vec::with_capacity(steps + 1);
    face_work.push(0.0);
    let mut f_prev = fluid.forces()?;
    let mut d_prev = faces_of(&at(0))?;
    for n in 1..=steps {
        let nodes = at(n);
        let points = map
            .displacement_to_points(&nodes)
            .map_err(|e| format!("aerodynamic damping: step {n}: {e}"))?;
        let forces = fluid
            .advance(dt, &points)
            .map_err(|e| format!("aerodynamic damping: step {n}: {e}"))?;
        let nodal = map
            .forces_to_nodes(&forces)
            .map_err(|e| format!("aerodynamic damping: step {n}: {e}"))?;
        generalised.push(motion.shape.iter().zip(&nodal).map(|(s, f)| vec3::dot(*s, *f)).sum());
        let d = map
            .face_displacements(&points)
            .map_err(|e| format!("aerodynamic damping: step {n}: {e}"))?;
        let w: f64 = (0..d.len())
            .map(|i| {
                vec3::dot(
                    vec3::sub(d[i], d_prev[i]),
                    vec3::scale(vec3::add(f_prev[i], forces[i]), 0.5),
                )
            })
            .sum();
        face_work.push(face_work[n - 1] + w);
        d_prev = d;
        f_prev = forces;
    }
    Ok(ModeHistory {
        motion: motion.clone(),
        generalised,
        last_face_forces: f_prev,
        face_work,
    })
}
