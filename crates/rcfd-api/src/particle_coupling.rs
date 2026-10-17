#![allow(clippy::neg_cmp_op_on_partial_ord)]
//! Steady two-way particle coupling (F-128, ADR 0059 D4-D8).
//!
//! The steady algorithms have no time axis for the parcels to live on, so the coupling is a fixed-point iteration of the
//! particle-source-in-cell kind: every `particles.two_way_every` iterations from `particles.two_way_start` a FRESH tracker is
//! built, set to record the exchange ([`rcfd_particles::Coupling`]) and run from 0 to `particles.end_time` on the current
//! velocity (and temperature) field. The recorded momentum given to the carrier, divided by `rho V W`, is the new body
//! acceleration of every cell; `W = N/R` is the injection window (N parcels emitted by one injection at the rate R
//! parcels/s, the same for all injections: checked). The source used by the flow is under-relaxed,
//! `a <- a + beta (a_new - a)` starting from `a = 0`, and is held fixed between updates (lagged).
//!
//! The monitor `res_particles` is the relative change of the source, `sum |a_new - a| V / sum |a_new| V`; the run counts as
//! converged only if at least two updates were made and the change is below `particles.two_way_tolerance`. With heat
//! transfer the heat the parcels give to the carrier enters the temperature equation as `q = Q/(rho cp V W)` in K/s; the
//! mechanical energy exchange (drag dissipation) is not fed in. The denominators of the change have a floor of 1e-3 of the
//! momentum and the heat content the parcels carry out (a negligible exchange has a change of round-off, not of order one).

use crate::case::Case;
use crate::particles;
use crate::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_fv::simple::BodyForce;
use rcfd_particles::{State, Tracker};

fn num(case: &Case, key: &str, default: f64) -> f64 {
    case.tree.get(key).ok().and_then(|v| v.as_f64()).unwrap_or(default)
}

/// State of the steady two-way coupling.
pub struct SteadyCoupling {
    relax: f64,
    every: usize,
    start: usize,
    tolerance: f64,
    heat: bool,
    rho: f64,
    cp: f64,
    cp_p: f64,
    /// Relaxed (used) acceleration per cell [m/s^2].
    accel: Vec<Vec3>,
    /// Acceleration of the last tracking, not relaxed.
    raw: Vec<Vec3>,
    /// Relaxed heat source per cell [K/s] (heat transfer only).
    q: Option<Vec<f64>>,
    /// Heat source of the last tracking, not relaxed.
    q_raw: Option<Vec<f64>>,
    force: Option<BodyForce>,
    change: f64,
    updates: usize,
    window: f64,
    tracker: Option<Tracker>,
}

impl SteadyCoupling {
    /// Settings of the case (checked by `particles::check`).
    pub fn new(case: &Case, n_cells: usize) -> SteadyCoupling {
        let heat = particles::heat_transfer(case);
        SteadyCoupling {
            relax: num(case, "particles.two_way_relax", 0.5),
            every: num(case, "particles.two_way_every", 1.0).max(1.0) as usize,
            start: num(case, "particles.two_way_start", 0.0).max(0.0) as usize,
            tolerance: num(case, "particles.two_way_tolerance", 1e-6),
            heat,
            rho: num(case, "physics.rho", 1.0),
            cp: num(case, "physics.cp", 1.0),
            cp_p: num(case, "particles.particle_cp", 1.0),
            accel: vec![[0.0; 3]; n_cells],
            raw: vec![[0.0; 3]; n_cells],
            q: heat.then(|| vec![0.0; n_cells]),
            q_raw: heat.then(|| vec![0.0; n_cells]),
            force: None,
            change: 1.0,
            updates: 0,
            window: 0.0,
            tracker: None,
        }
    }

    /// Is an update due before the iteration after `completed` finished iterations?
    pub fn due(&self, completed: usize) -> bool {
        completed >= self.start && (completed - self.start) % self.every == 0
    }

    /// Track on the current field of `sim` and update the relaxed source. The error text is located.
    pub fn update(&mut self, sim: &Simulation) -> Result<(), String> {
        let case = &sim.case;
        let mut tr = particles::build_tracker(case, sim).map_err(|e| {
            e.iter()
                .map(|d| format!("{}: {}", d.path, d.message))
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        tr.enable_coupling();
        let mut dt = num(case, "particles.dt", particles::UNSET);
        if dt == particles::UNSET {
            dt = num(case, "solver.dt", particles::UNSET);
        }
        if !(dt > 0.0) {
            return Err("particles.dt: set the tracking step (solver.dt is not positive either)".into());
        }
        let end = num(case, "particles.end_time", 0.0);
        let fd = particles::flow_data(case, sim);
        let flow = fd.flow();
        while tr.time < end * (1.0 - 1e-12) {
            let h = dt.min(end - tr.time);
            tr.advance(&flow, h).map_err(|e| format!("particles: {e}"))?;
        }
        let active = tr.count(State::Active);
        if active > 0 {
            return Err(format!(
                "particles.end_time: {active} parcels are still in flight at the end of the tracking (t = {end}); the steady two-way coupling needs every parcel to leave or deposit (raise particles.end_time, or start the coupling later with particles.two_way_start when the initial field has no flow)"
            ));
        }
        self.updates += 1;
        let n_inj = particles::injection_names(case).len().max(1);
        let n_parcels = tr.injected() / n_inj;
        if n_parcels == 0 {
            // nothing injected: no exchange at all
            self.change = 0.0;
            self.tracker = Some(tr);
            return Ok(());
        }
        let rate = num(
            case,
            &format!("particles.injection.{}.rate", particles::injection_names(case)[0]),
            1.0,
        );
        let w = n_parcels as f64 / rate;
        self.window = w;
        let vol = sim.geom.mesh.cell_volumes();
        let cpl = tr.coupling().ok_or("particles: the coupling record is missing")?;
        let beta = self.relax;
        // Floors of the denominators of the change: 1e-3 of the momentum (heat) the parcels carry. An exchange below that is
        // negligible, and its change is round-off, not information (the momentum of parcels that move with the stream is not
        // exchanged at all; relative to itself its round-off would give a change of order one).
        let (mut m_v, mut m_t) = (0.0, 0.0);
        for (i, p) in tr.particles.iter().enumerate() {
            let m = p.parcel * p.rho * std::f64::consts::PI * p.d.powi(3) / 6.0;
            m_v += m * rcfd_core::vec3::norm(p.v);
            if self.heat {
                let t = tr.temperature(i);
                if t.is_finite() {
                    m_t += m * self.cp_p * t.abs();
                }
            }
        }
        let (mut num_c, mut den_c) = (0.0, 0.0);
        for c in 0..vol.len() {
            let f = 1.0 / (self.rho * vol[c] * w);
            let new = [cpl.impulse[c][0] * f, cpl.impulse[c][1] * f, cpl.impulse[c][2] * f];
            self.raw[c] = new;
            let old = self.accel[c];
            let diff = [new[0] - old[0], new[1] - old[1], new[2] - old[2]];
            num_c += (diff[0] * diff[0] + diff[1] * diff[1] + diff[2] * diff[2]).sqrt() * vol[c];
            den_c += (new[0] * new[0] + new[1] * new[1] + new[2] * new[2]).sqrt() * vol[c];
            for k in 0..3 {
                self.accel[c][k] += beta * diff[k];
            }
        }
        let mut change = num_c / den_c.max(1e-3 * m_v / (self.rho * w)).max(1e-300);
        if self.heat {
            let (q, q_raw) = (self.q.as_mut().expect("heat"), self.q_raw.as_mut().expect("heat"));
            let (mut nq, mut dq) = (0.0, 0.0);
            for c in 0..vol.len() {
                let new = cpl.energy_heat[c] / (self.rho * self.cp * vol[c] * w);
                q_raw[c] = new;
                nq += (new - q[c]).abs() * vol[c];
                dq += new.abs() * vol[c];
                q[c] += beta * (new - q[c]);
            }
            change = change.max(nq / dq.max(1e-3 * m_t / (self.rho * self.cp * w)).max(1e-300));
        }
        self.change = change;
        self.force = Some(BodyForce::from_cells(&sim.geom, self.accel.clone()));
        self.tracker = Some(tr);
        Ok(())
    }

    /// The relaxed acceleration as a face-consistent body force (none before the first update with parcels).
    pub fn force(&self) -> Option<&BodyForce> {
        self.force.as_ref()
    }

    /// The relaxed heat source [K/s] per cell (heat transfer only, none before the first update with parcels).
    pub fn heat_source(&self) -> Option<&[f64]> {
        if self.force.is_some() {
            self.q.as_deref()
        } else {
            None
        }
    }

    /// Relative change of the source at the last update (the monitor `res_particles`).
    pub fn change(&self) -> f64 {
        self.change
    }

    /// Number of trackings made.
    pub fn updates(&self) -> usize {
        self.updates
    }

    /// Converged: at least two updates and a change below the tolerance.
    pub fn converged(&self) -> bool {
        self.updates >= 2 && self.change < self.tolerance
    }

    /// Injection window `W = N/R` of the last update [s].
    pub fn window(&self) -> f64 {
        self.window
    }

    /// The relaxed acceleration per cell.
    pub fn accel(&self) -> &[Vec3] {
        &self.accel
    }

    /// The acceleration of the last tracking before relaxation.
    pub fn raw_accel(&self) -> &[Vec3] {
        &self.raw
    }

    /// The heat source of the last tracking before relaxation [K/s].
    pub fn raw_heat(&self) -> Option<&[f64]> {
        self.q_raw.as_deref()
    }

    /// The tracker of the last update (with its coupling record).
    pub fn last_tracker(&self) -> Option<&Tracker> {
        self.tracker.as_ref()
    }
}
