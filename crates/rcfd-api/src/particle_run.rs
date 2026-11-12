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
        State::Film => "film",
        State::Evaporated => "evaporated",
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
    /// The droplet terms (evaporation or breakup tables, ADR 0348 D6); `None` on the path of ADR 0059.
    pub droplets: Option<DropletLedger>,
    /// The vapour source of the species of the current step [1/s] (evaporation only, ADR 0348 D4).
    vapour_src: Option<Vec<f64>>,
}

/// What the carrier received from the droplet terms of the record, summed from the start (ADR 0348 D6). In droplet mode the
/// carrier receives over a flow step `dt` the sources `droplet_sources(vol, dt)` of the step's record: the mass to the vapour
/// species, the momentum as the body acceleration, and of the energy the sensible part
/// `Q_T = energy_heat + vapour_enthalpy - vapour_mass (c_l T^n + L_v)` to the temperature equation; the mechanical part and
/// the enthalpy of the arrived vapour at the carrier temperature are kept here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DropletLedger {
    /// The record's vapour mass [kg].
    pub vapour_record: f64,
    /// Vapour mass given to the species, `sum rho V S_Y dt` [kg].
    pub vapour_delivered: f64,
    /// The record's total energy `sum (energy_mech + energy_heat + vapour_kinetic + vapour_enthalpy)` [J].
    pub energy_record: f64,
    /// The sensible heat `sum Q_T` [J] (what the temperature equation receives, before the normalisation).
    pub heat_record: f64,
    /// The record's `sum energy_heat` [J] (the heat drawn from the gas, without the vapour terms).
    pub heat_raw: f64,
    /// The enthalpy of the arrived vapour at the carrier temperature `sum vapour_mass (c_l T^n + L_v)` [J] (`H_v`).
    pub vapour_held: f64,
    /// The mechanical energy `sum (energy_mech + vapour_kinetic)` [J] (`K`; not fed into the temperature, as ADR 0059).
    pub mechanical: f64,
    /// Gravity impulse on the parcels (with buoyancy) [N s].
    pub gravity_impulse: Vec3,
    /// Work of gravity on the parcels [J].
    pub gravity_work: f64,
    /// Momentum given to the parcels by walls [N s].
    pub wall_impulse: Vec3,
    /// Kinetic energy given to the parcels by walls [J].
    pub wall_energy: f64,
    /// Momentum given to the parcels by breakup without a carrier counterpart [N s].
    pub breakup_impulse: Vec3,
    /// Kinetic energy given to the parcels by breakup without a carrier counterpart [J].
    pub breakup_energy: f64,
    /// The last step: the record's vapour mass [kg].
    pub last_vapour: f64,
    /// The last step: the sum over the cells of the absolute vapour mass [kg].
    pub last_vapour_abs: f64,
    /// The last step: `sum Q_T` [J].
    pub last_heat: f64,
    /// The last step: the sum over the cells of `abs(Q_T)` [J].
    pub last_heat_abs: f64,
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
    /// The erosion constants when they use a preset (ADR 0197 P6); `None` without erosion or without a preset.
    erosion_preset: Option<particles::ErosionConstants>,
    /// The species that receives the vapour (evaporation table, ADR 0348); `None` without evaporation.
    vapour_species: Option<String>,
}

fn io_err(e: std::io::Error, what: &str) -> String {
    format!("cannot write {what}: {e}")
}

/// `"preset"` or `"case"`: where an erosion constant comes from (ADR 0197 P6).
fn origin(from_preset: bool) -> &'static str {
    if from_preset {
        "preset"
    } else {
        "case"
    }
}

/// The record of the preset in the report (ADR 0197 P6), with the values the tracker uses.
fn erosion_material_json(c: &particles::ErosionConstants, er: &rcfd_particles::Erosion) -> Value {
    let p = c.preset.expect("a preset");
    json!({
        "preset": p.name,
        "row": p.row,
        "source": p.source,
        "k": er.k(),
        "n": er.n(),
        "density": er.density(),
        "from": {
            "k": origin(c.from_preset[0]),
            "n": origin(c.from_preset[1]),
            "density": origin(c.from_preset[2]),
        },
        "provenance": rcfd_particles::PRESET_PROVENANCE,
    })
}

/// The three comment lines `particle_erosion.csv` starts with when a preset is used (ADR 0197 P6).
fn erosion_csv_preamble(c: &particles::ErosionConstants, er: &rcfd_particles::Erosion) -> String {
    let p = c.preset.expect("a preset");
    format!(
        "# erosion preset {}: {}, {}\n# values used: k = {:?} ({}), n = {:?} ({}), density = {:?} ({})\n# {}\n",
        p.name,
        p.row,
        p.source,
        er.k(),
        origin(c.from_preset[0]),
        er.n(),
        origin(c.from_preset[1]),
        er.density(),
        origin(c.from_preset[2]),
        rcfd_particles::PRESET_PROVENANCE
    )
}

impl ParticleRun {
    /// Wrap a built tracker. `t0` is the simulation time at construction.
    pub fn new(case: &Case, tracker: Tracker, t0: f64) -> ParticleRun {
        let get = |p: &str| case.tree.get(p).ok();
        let int = |p: &str, d: i64| get(p).and_then(|v| v.as_i64()).unwrap_or(d);
        let mut tracker = tracker;
        let droplets = particles::droplet_exchange(case);
        let two_way = particles::two_way(case).then(|| {
            if droplets {
                // ADR 0348 D3: the record with the droplet terms
                tracker.enable_droplet_exchange();
            } else {
                tracker.enable_coupling();
            }
            TransientCoupling {
                heat: particles::heat_transfer(case),
                rho: get("physics.rho").and_then(|v| v.as_f64()).unwrap_or(1.0),
                cp: get("physics.cp").and_then(|v| v.as_f64()).unwrap_or(1.0),
                force: None,
                heat_src: None,
                delivered: [0.0; 3],
                exchanged: [0.0; 3],
                delivered_heat: 0.0,
                droplets: droplets.then(DropletLedger::default),
                vapour_src: None,
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
            erosion_preset: particles::erosion(case)
                .then(|| particles::erosion_constants(case))
                .filter(|c| c.preset.is_some()),
            vapour_species: particles::evaporation(case).then(|| particles::vapour_species(case)),
        }
    }

    /// The species that receives the vapour (evaporation only, ADR 0348).
    pub fn vapour_species(&self) -> Option<&str> {
        self.vapour_species.as_deref()
    }

    /// The vapour source [1/s] of the species for the flow step in progress (evaporation only, ADR 0348 D4).
    pub fn vapour_source(&self) -> Option<(&str, &[f64])> {
        let name = self.vapour_species.as_deref()?;
        let src = self.two_way.as_ref()?.vapour_src.as_deref()?;
        Some((name, src))
    }

    /// Give the tracker the carrier's vapour mass fraction of the step (the old time level), clamped below at 0 (ADR 0348
    /// D3, T5); a value at or above 1 is an error. Nothing without evaporation.
    pub fn set_carrier_vapour(&mut self, y: &[f64]) -> Result<(), String> {
        if self.vapour_species.is_none() {
            return Ok(());
        }
        if let Some((c, v)) = y.iter().enumerate().find(|(_, v)| !(**v < 1.0)) {
            return Err(format!(
                "particles.evaporation.species: the vapour mass fraction of cell {c} is {v}, not below 1 (ADR 0348)"
            ));
        }
        let seen: Vec<f64> = y.iter().map(|v| v.max(0.0)).collect();
        self.tracker.set_evaporation_vapour(&seen).map_err(|e| e.to_string())
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
        if self.vapour_species.is_some() {
            // ADR 0348 D4: the species source of every step is set (zero when nothing is exchanged)
            tw.vapour_src = Some(vec![0.0; geom.n_cells()]);
        }
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
        if self.two_way.as_ref().is_some_and(|t| t.droplets.is_some()) {
            return self.deliver_droplets(geom, dt);
        }
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

    /// Droplet mode (ADR 0348 D4): convert the record of the step into the sources of the flow step of length `dt`.
    fn deliver_droplets(&mut self, geom: &FvGeometry, dt: f64) -> Result<(), String> {
        let latent = self.tracker.evaporation().map_or(0.0, |e| e.spec().latent_heat);
        let (c_l, t_n): (f64, Option<&[f64]>) = match self.tracker.thermal() {
            Some(th) => (th.cp_p, Some(th.t_fluid.as_slice())),
            None => (0.0, None),
        };
        let cpl = self.tracker.coupling().ok_or("the coupling record is missing")?;
        let tw = self.two_way.as_mut().expect("two-way");
        let vol = geom.mesh.cell_volumes();
        let n = vol.len();
        let src = cpl.droplet_sources(vol, dt);
        let led = tw.droplets.as_mut().expect("droplet mode");
        let mut cells = Vec::with_capacity(n);
        let mut any = false;
        for c in 0..n {
            let a = [
                src.momentum[c][0] / tw.rho,
                src.momentum[c][1] / tw.rho,
                src.momentum[c][2] / tw.rho,
            ];
            any |= a != [0.0; 3];
            for k in 0..3 {
                tw.exchanged[k] += cpl.impulse[c][k] + cpl.vapour_impulse[c][k];
                tw.delivered[k] += tw.rho * vol[c] * a[k] * dt;
            }
            cells.push(a);
        }
        if any {
            tw.force = Some(BodyForce::from_cells(geom, cells));
        }
        let (mut lv, mut lv_abs) = (0.0, 0.0);
        if let Some(ys) = tw.vapour_src.as_mut() {
            for c in 0..n {
                ys[c] = src.mass[c] / tw.rho;
                led.vapour_delivered += tw.rho * vol[c] * ys[c] * dt;
                lv += cpl.vapour_mass[c];
                lv_abs += cpl.vapour_mass[c].abs();
            }
        }
        led.vapour_record += lv;
        led.last_vapour = lv;
        led.last_vapour_abs = lv_abs;
        // the enthalpy of the arrived vapour at the carrier temperature of the step, h_v(T^n) = c_l T^n + L_v (ADR 0230 D4)
        let held: Vec<f64> = (0..n)
            .map(|c| t_n.map_or(0.0, |t| cpl.vapour_mass[c] * (c_l * t[c] + latent)))
            .collect();
        for c in 0..n {
            led.energy_record +=
                cpl.energy_mech[c] + cpl.energy_heat[c] + cpl.vapour_kinetic[c] + cpl.vapour_enthalpy[c];
            led.mechanical += cpl.energy_mech[c] + cpl.vapour_kinetic[c];
            led.heat_raw += cpl.energy_heat[c];
            led.vapour_held += held[c];
        }
        let (mut lq, mut lq_abs) = (0.0, 0.0);
        if tw.heat {
            let mut q = vec![0.0; n];
            for c in 0..n {
                let qt = cpl.energy_heat[c] + cpl.vapour_enthalpy[c] - held[c];
                lq += qt;
                lq_abs += qt.abs();
                q[c] = qt / (tw.rho * tw.cp * vol[c] * dt);
                tw.delivered_heat += tw.rho * tw.cp * vol[c] * q[c] * dt;
            }
            tw.heat_src = Some(q);
        }
        led.heat_record += lq;
        led.last_heat = lq;
        led.last_heat_abs = lq_abs;
        for k in 0..3 {
            led.gravity_impulse[k] += cpl.gravity_impulse[k];
            led.wall_impulse[k] += cpl.wall_impulse[k];
            led.breakup_impulse[k] += cpl.breakup_impulse[k];
        }
        led.gravity_work += cpl.gravity_work;
        led.wall_energy += cpl.wall_energy;
        led.breakup_energy += cpl.breakup_energy;
        Ok(())
    }

    /// Give the random walk the turbulence model's `k` and `epsilon` of `turb` (ADR 0208 D2); nothing unless
    /// `particles.dispersion_from_model.enabled`.
    pub(crate) fn refresh_dispersion(
        &mut self,
        case: &Case,
        turb: Option<&crate::turb::TurbState>,
    ) -> Result<(), String> {
        if !particles::dispersion_from_model(case) {
            return Ok(());
        }
        let (k, eps) = particles::model_k_epsilon(turb)?;
        self.tracker.set_dispersion_fields(&k, &eps).map_err(|e| e.to_string())
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
        // the solver's own time (ADR 0160 D2; inside `step`, before any end-time snap, the two are equal)
        let t = sim.solver_time();
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
        // ADR 0208 D2: the turbulence fields at the end of the step (nothing without dispersion_from_model)
        self.refresh_dispersion(&sim.case, sim.turb_state())?;
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
        // six states (ADR 0348 D7: a parcel can be `Evaporated` with the evaporation table)
        let mut mass = [0.0f64; 6];
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
        if let Some(ev) = t.evaporation() {
            // ADR 0348 D7: only with the evaporation table
            v["counts"]["evaporated"] = json!(t.count(State::Evaporated));
            v["mass_by_state"]["evaporated"] = json!(mass[5]);
            v["droplets"] = json!({
                "evaporated": ev.evaporated(),
                "completed": ev.completed(),
            });
        }
        if let Some(bu) = t.breakup() {
            // ADR 0348 D7: only with the breakup table
            if !v["droplets"].is_object() {
                v["droplets"] = json!({});
            }
            v["droplets"]["breakup_events"] = json!(bu.events());
        }
        if let Some(f) = t.film() {
            // ADR 0192 S3: only when the film is on, so a run without it writes the report it always wrote
            v["counts"]["film"] = json!(t.count(State::Film));
            v["mass_by_state"]["film"] = json!(mass[4]);
            v["film"] = json!({
                "faces": f.len(),
                "held": f.held(),
                "drained": f.drained(),
                "absorbed": f.received(),
                "max_thickness": f.max_thickness(),
                "max_reynolds": f.max_reynolds(),
            });
            let g = f.gas();
            if g != rcfd_particles::FilmGas::default() {
                // ADR 0206 S3: only with a gas drive
                v["film"]["gas"] = json!({
                    "buoyancy": g.buoyancy,
                    "shear": g.shear,
                    "gas_density": f.gas_density(),
                    "max_shear": f.max_shear(),
                    "opposed": f.opposed(),
                });
            }
        }
        if let Some(er) = t.erosion() {
            // ADR 0194 S3: only when erosion is on; rates over the tracked time
            v["erosion"] = json!({
                "faces": er.len(),
                "hits": er.total_hits(),
                "impact_mass": er.total_impact_mass(),
                "eroded": er.total_eroded(),
                "max_rate": er.max_rate(t.time),
                "max_depth_rate": er.max_depth_rate(t.time),
            });
            if let Some(c) = &self.erosion_preset {
                // ADR 0197 P6: only with a preset
                v["erosion"]["material"] = erosion_material_json(c, er);
            }
        }
        if let Some(rw) = t.rough_wall() {
            let c = rw.counts();
            v["rough_wall"] = json!({
                "collisions": c.collisions,
                "redraws": c.redraws,
                "fallbacks": c.fallbacks,
                "normal_incidence": c.normal_incidence,
                "grazing": c.grazing,
            });
        }
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

    /// Write `particles.vtp`, `particle_report.json`, with `particles.tracks` `particle_tracks.csv`, with the wall film
    /// `particle_film.csv` (ADR 0192) and with erosion `particle_erosion.csv` (ADR 0194) into `dir`.
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
        if let Some(f) = self.tracker.film() {
            put("particle_film.csv", f.csv().as_bytes())?;
        }
        if let Some(er) = self.tracker.erosion() {
            // ADR 0197 P6: with a preset, three comment lines before the header; without one, the file of ADR 0194
            let mut csv = match &self.erosion_preset {
                Some(c) => erosion_csv_preamble(c, er),
                None => String::new(),
            };
            csv.push_str(&er.csv(self.tracker.time));
            put("particle_erosion.csv", csv.as_bytes())?;
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
