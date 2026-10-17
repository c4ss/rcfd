#![allow(clippy::neg_cmp_op_on_partial_ord)]
//! Particle tracking driven by the run loop (F-056, ADR 0037 D1).
//!
//! `Simulation::new` builds a [`ParticleRun`] for transient algorithms when `particles.enabled`; the end of
//! `Simulation::step` calls [`ParticleRun::after_step`], which does exactly what the library hook
//! [`crate::particles::track_transient`] does after its own `sim.step()` (same arithmetic, same call order), so the two agree
//! bit for bit. Steady algorithms are tracked once on the final field by [`steady_run`] when the results are written, as
//! [`crate::particles::track_frozen`] does. The flow is only read.

use crate::case::{Case, Diagnostic};
use crate::particles::{self, UNSET};
use crate::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FvGeometry, VectorField};
use rcfd_particles::vtp::write_vtp;
use rcfd_particles::{Flow, Particle, State, Tracker};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// One row of the track log.
struct Row {
    id: u64,
    t: f64,
    x: [f64; 3],
    state: State,
}

/// Optional log of the particle positions (`particles.tracks`).
#[derive(Default)]
struct TrackLog {
    rows: Vec<Row>,
    /// Time of the last row of each parcel (NaN = none yet).
    last_t: Vec<f64>,
}

fn state_name(s: State) -> &'static str {
    match s {
        State::Active => "active",
        State::Deposited => "deposited",
        State::Escaped => "escaped",
        State::Stuck => "stuck",
    }
}

/// Transient two-way coupling state (ADR 0059 D2-D3): the particles are advanced first, over the step interval, on the
/// field of the old time level; the exchange of that interval is delivered to the carrier as a body acceleration that is
/// constant over the step.
pub struct TransientCoupling {
    heat: bool,
    rho: f64,
    cp: f64,
    /// The acceleration delivered to the current step (none when nothing was exchanged).
    force: Option<BodyForce>,
    /// The heat source of the current step [K/s] (heat transfer only).
    heat_src: Option<Vec<f64>>,
    /// Momentum delivered to the carrier so far, `sum rho V a h` [N s].
    pub delivered: Vec3,
    /// Momentum of the exchange record so far (the sum of the tracker records) [N s].
    pub exchanged: Vec3,
    /// Heat delivered to the carrier so far, `sum rho cp V q h` [J].
    pub delivered_heat: f64,
}

/// Particle state of a running simulation.
pub struct ParticleRun {
    /// The tracker (all parcels).
    pub tracker: Tracker,
    /// Tracking steps performed.
    pub steps: usize,
    t_prev: f64,
    end: f64,
    every: usize,
    track_every: usize,
    out_dir: Option<PathBuf>,
    tracks: Option<TrackLog>,
    two_way: Option<TransientCoupling>,
}

fn io_err(e: std::io::Error, what: &str) -> String {
    format!("cannot write {what}: {e}")
}

impl ParticleRun {
    /// Wrap a built tracker. `t0` is the simulation time at construction.
    pub fn new(case: &Case, tracker: Tracker, t0: f64) -> ParticleRun {
        let get = |p: &str| case.tree.get(p).ok();
        let int = |p: &str, d: i64| get(p).and_then(|v| v.as_i64()).unwrap_or(d);
        let mut tracker = tracker;
        let two_way = particles::two_way(case).then(|| {
            tracker.enable_coupling();
            TransientCoupling {
                heat: particles::heat_transfer(case),
                rho: get("physics.rho").and_then(|v| v.as_f64()).unwrap_or(1.0),
                cp: get("physics.cp").and_then(|v| v.as_f64()).unwrap_or(1.0),
                force: None,
                heat_src: None,
                delivered: [0.0; 3],
                exchanged: [0.0; 3],
                delivered_heat: 0.0,
            }
        });
        ParticleRun {
            tracker,
            steps: 0,
            t_prev: t0,
            end: get("particles.end_time").and_then(|v| v.as_f64()).unwrap_or(0.0),
            every: int("particles.output_every", 0).max(0) as usize,
            track_every: int("particles.track_every", 1).max(1) as usize,
            out_dir: None,
            tracks: get("particles.tracks")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                .then(TrackLog::default),
            two_way,
        }
    }

    /// Is the two-way coupling active?
    pub fn is_two_way(&self) -> bool {
        self.two_way.is_some()
    }

    /// The two-way state (delivered and exchanged totals), if active.
    pub fn coupling(&self) -> Option<&TransientCoupling> {
        self.two_way.as_ref()
    }

    /// The acceleration to deliver to the flow step in progress (two-way only).
    pub fn two_way_force(&self) -> Option<&BodyForce> {
        self.two_way.as_ref().and_then(|t| t.force.as_ref())
    }

    /// The heat source [K/s] of the flow step in progress (two-way with heat transfer only).
    pub fn two_way_heat(&self) -> Option<&[f64]> {
        self.two_way.as_ref().and_then(|t| t.heat_src.as_deref())
    }

    /// Two-way pre-step (before the flow step of length `dt`): advance the tracker by `min(dt, end - t)` on the old-level
    /// velocity `u` (and temperature `temp`), then convert the exchange of that interval into the body acceleration and the
    /// heat source of the step. Without two-way coupling this does nothing.
    pub fn pre_step(
        &mut self,
        case: &Case,
        geom: &FvGeometry,
        u: &VectorField,
        temp: Option<&[f64]>,
        dt: f64,
    ) -> Result<(), String> {
        let Some(tw) = &mut self.two_way else { return Ok(()) };
        tw.force = None;
        tw.heat_src = None;
        if !(self.tracker.time < self.end * (1.0 - 1e-12)) || !(dt > 0.0) {
            return Ok(());
        }
        let h = dt.min(self.end - self.tracker.time);
        if tw.heat {
            if let (Some(t), Some(th)) = (temp, self.tracker.thermal_mut()) {
                th.t_fluid.clear();
                th.t_fluid.extend_from_slice(t);
            }
        }
        if let Some(c) = self.tracker.coupling_mut() {
            c.reset();
        }
        let fd = particles::flow_data_from(case, geom, u);
        self.advance_step(&fd.flow(), h)?;
        let tw = self.two_way.as_mut().expect("two-way");
        let cpl = self.tracker.coupling().ok_or("the coupling record is missing")?;
        let vol = geom.mesh.cell_volumes();
        let mut cells = Vec::with_capacity(vol.len());
        let mut any = false;
        for c in 0..vol.len() {
            let f = 1.0 / (tw.rho * vol[c] * h);
            let a = [cpl.impulse[c][0] * f, cpl.impulse[c][1] * f, cpl.impulse[c][2] * f];
            any |= a != [0.0; 3];
            for k in 0..3 {
                tw.exchanged[k] += cpl.impulse[c][k];
                tw.delivered[k] += tw.rho * vol[c] * a[k] * h;
            }
            cells.push(a);
        }
        if any {
            tw.force = Some(BodyForce::from_cells(geom, cells));
        }
        if tw.heat {
            let q: Vec<f64> = (0..vol.len())
                .map(|c| cpl.energy_heat[c] / (tw.rho * tw.cp * vol[c] * h))
                .collect();
            for c in 0..vol.len() {
                tw.delivered_heat += tw.rho * tw.cp * vol[c] * q[c] * h;
            }
            tw.heat_src = Some(q);
        }
        Ok(())
    }

    /// Directory of the numbered `particles_<n>.vtp` files written during the run (none = not written).
    pub fn set_output_dir(&mut self, dir: Option<PathBuf>) {
        self.out_dir = dir;
    }

    /// One tracking step of length `h` on `flow` (advance, count, log, numbered output).
    fn advance_step(&mut self, flow: &Flow, h: f64) -> Result<(), String> {
        let n_old = self.tracker.particles.len();
        let before: Vec<State> = self.tracker.particles[..n_old].iter().map(|p| p.state).collect();
        self.tracker.advance(flow, h).map_err(|e| e.to_string())?;
        self.steps += 1;
        if let Some(log) = &mut self.tracks {
            let t = self.tracker.time;
            let on = self.steps % self.track_every == 0;
            log.last_t.resize(self.tracker.particles.len(), f64::NAN);
            for (i, p) in self.tracker.particles.iter().enumerate() {
                let new = i >= n_old;
                let was_active = !new && before[i] == State::Active;
                if new || (was_active && (on || p.state != State::Active)) {
                    log.rows.push(Row {
                        id: p.id,
                        t,
                        x: p.x,
                        state: p.state,
                    });
                    log.last_t[i] = t;
                }
            }
        }
        if self.every > 0 && self.steps % self.every == 0 {
            if let Some(dir) = &self.out_dir {
                let path = dir.join(format!("particles_{}.vtp", self.steps));
                write_vtp(&path, &self.tracker.particles).map_err(|e| io_err(e, &path.display().to_string()))?;
            }
        }
        Ok(())
    }

    /// The hook at the end of a transient flow step of `sim`.
    pub fn after_step(&mut self, sim: &Simulation) -> Result<(), String> {
        let t = sim.time();
        let dt = t - self.t_prev;
        self.t_prev = t;
        if self.two_way.is_some() {
            // the tracking of this step was done before the flow step (pre_step)
            return Ok(());
        }
        if !(self.tracker.time < self.end * (1.0 - 1e-12)) || dt <= 0.0 {
            return Ok(());
        }
        let h = dt.min(self.end - self.tracker.time);
        let fd = particles::flow_data(&sim.case, sim);
        self.advance_step(&fd.flow(), h)
    }

    /// Rows of the track log including a closing row for every parcel still active whose last row is older.
    fn track_rows(&self) -> Vec<(u64, f64, [f64; 3], State)> {
        let Some(log) = &self.tracks else { return Vec::new() };
        let mut rows: Vec<(u64, f64, [f64; 3], State)> = log.rows.iter().map(|r| (r.id, r.t, r.x, r.state)).collect();
        for (i, p) in self.tracker.particles.iter().enumerate() {
            let last = log.last_t.get(i).copied().unwrap_or(f64::NAN);
            if p.state == State::Active && last != self.tracker.time {
                rows.push((p.id, self.tracker.time, p.x, p.state));
            }
        }
        rows
    }

    /// The track log as CSV text (`id,time,x,y,z,state`), if recorded.
    pub fn tracks_csv(&self) -> Option<String> {
        self.tracks.as_ref()?;
        let mut s = String::from("id,time,x,y,z,state\n");
        for (id, t, x, st) in self.track_rows() {
            s.push_str(&format!("{id},{t},{},{},{},{}\n", x[0], x[1], x[2], state_name(st)));
        }
        Some(s)
    }

    /// The report: counts, mean diameter and mass by state.
    pub fn report(&self, with_tracks: bool) -> Value {
        let ps = &self.tracker.particles;
        let mut mass = [0.0f64; 4];
        let mut dsum = 0.0;
        for p in ps {
            mass[p.state as usize] += p.parcel * p.rho * std::f64::consts::PI * p.d.powi(3) / 6.0;
            dsum += p.d;
        }
        let t = &self.tracker;
        let mut v = json!({
            "time": t.time,
            "injected": t.injected(),
            "counts": {
                "active": t.count(State::Active),
                "deposited": t.count(State::Deposited),
                "escaped": t.count(State::Escaped),
                "stuck": t.count(State::Stuck),
            },
            "mean_diameter": if ps.is_empty() { 0.0 } else { dsum / ps.len() as f64 },
            "mass_by_state": {
                "active": mass[0], "deposited": mass[1], "escaped": mass[2], "stuck": mass[3],
            },
            "steps": self.steps,
        });
        if with_tracks && self.tracks.is_some() {
            let rows: Vec<Value> = self
                .track_rows()
                .into_iter()
                .map(|(id, t, x, st)| json!([id, t, x[0], x[1], x[2], state_name(st)]))
                .collect();
            v["tracks"] = Value::Array(rows);
        }
        v
    }

    /// Write `particles.vtp`, `particle_report.json` and, with `particles.tracks`, `particle_tracks.csv` into `dir`.
    pub fn write_outputs(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| io_err(e, &dir.display().to_string()))?;
        let put = |name: &str, bytes: &[u8]| {
            std::fs::write(dir.join(name), bytes).map_err(|e| io_err(e, &dir.join(name).display().to_string()))
        };
        write_vtp(dir.join("particles.vtp"), &self.tracker.particles)
            .map_err(|e| io_err(e, &dir.join("particles.vtp").display().to_string()))?;
        put("particle_report.json", self.report(false).to_string().as_bytes())?;
        if let Some(csv) = self.tracks_csv() {
            put("particle_tracks.csv", csv.as_bytes())?;
        }
        Ok(())
    }

    /// Particles (for tests and the service).
    pub fn particles(&self) -> &[Particle] {
        &self.tracker.particles
    }
}

/// Track the particles of `sim` once on its current (final) field from 0 to `particles.end_time`, with the arithmetic of
/// [`crate::particles::track_frozen`]; the numbered files go to `dir` when given.
pub fn steady_run(sim: &Simulation, dir: Option<&Path>) -> Result<ParticleRun, Vec<Diagnostic>> {
    let case = &sim.case;
    let tr = particles::build_tracker(case, sim)?;
    let mut dt = case
        .tree
        .get("particles.dt")
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(UNSET);
    if dt == UNSET {
        dt = case
            .tree
            .get("solver.dt")
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(UNSET);
    }
    if !(dt > 0.0) {
        return Err(vec![Diagnostic {
            path: "particles.dt".into(),
            line: None,
            message: "set the tracking step (solver.dt is not positive either)".into(),
        }]);
    }
    let mut run = ParticleRun::new(case, tr, 0.0);
    run.out_dir = dir.map(Path::to_path_buf);
    let fd = particles::flow_data(case, sim);
    let flow = fd.flow();
    while run.tracker.time < run.end * (1.0 - 1e-12) {
        let h = dt.min(run.end - run.tracker.time);
        run.advance_step(&flow, h).map_err(|m| {
            vec![Diagnostic {
                path: "particles".into(),
                line: None,
                message: m,
            }]
        })?;
    }
    Ok(run)
}

/// Write the particle outputs of a finished run into `dir` (nothing unless `particles.enabled`): the transient run's own
/// tracker, or, for a steady algorithm, a tracking on the final field.
pub fn write_run_outputs(sim: &Simulation, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !particles::enabled(&sim.case) {
        return Ok(());
    }
    if let Some(pr) = sim.particle_run() {
        pr.write_outputs(dir)?;
    } else if sim.is_steady() {
        let run = steady_run(sim, Some(dir)).map_err(|e| {
            e.iter()
                .map(|d| format!("{}: {}", d.path, d.message))
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        run.write_outputs(dir)?;
    }
    Ok(())
}
