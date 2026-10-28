//! Opt-in time-series field store of the run path (ADR 0124): with `output.store_interval` (steps) or
//! `output.store_time_interval` (simulated seconds) above 0 and an output directory, [`crate::runctl::drive`] writes
//! the cell fields `output.store_fields` (default ux, uy, uz, p) to `<out_dir>/fields/` (`rcfd_tsstore`), plus the
//! final state when the run ends. Off by default; when off nothing here runs. The hook only reads the solver, so a
//! run with the store on takes the same steps as one with it off.

use crate::sim::Simulation;
use rcfd_tsstore::{Location, Precision, WriteOptions, Writer, STORE_FILE};
use std::path::Path;

/// Default stored fields.
pub const DEFAULT_FIELDS: [&str; 4] = ["ux", "uy", "uz", "p"];

/// Unit strings the store records; only names whose unit is fixed by the solver (kinematic vs static pressure
/// depends on the engine, so `p` and the rest are "unknown").
fn unit_of(name: &str) -> &'static str {
    match name {
        "ux" | "uy" | "uz" | "speed" => "m/s",
        "T" => "K",
        _ => "unknown",
    }
}

pub(crate) struct Hook {
    w: Writer,
    every: usize,
    dt: f64,
    bucket: f64,
    names: Vec<String>,
    last: Option<u64>,
}

fn bucket(time: f64, dt: f64) -> f64 {
    // relative slack so a time that is a multiple up to round-off counts as reached
    (time / dt + 1e-9).floor()
}

impl Hook {
    /// `None` when the store is off (both keys 0) or there is no directory for it; `Err` is a warning message. The
    /// store goes to `store_dir` when given (`rcfd run --store-dir`, ADR 0130 D3), else to `<out_dir>/fields`.
    pub(crate) fn open(
        sim: &Simulation,
        out_dir: Option<&Path>,
        store_dir: Option<&Path>,
    ) -> Option<Result<Hook, String>> {
        let every = sim
            .case
            .get("output.store_interval")
            .ok()
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let dt = sim
            .case
            .get("output.store_time_interval")
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        if every == 0 && dt <= 0.0 {
            return None;
        }
        let dir = match store_dir {
            Some(d) => d.to_path_buf(),
            None => out_dir?.join("fields"),
        };
        Some(Self::open_in(sim, &dir, every, dt).map_err(|e| format!("field store disabled: {e}")))
    }

    fn open_in(sim: &Simulation, dir: &Path, every: usize, dt: f64) -> Result<Hook, String> {
        if dt > 0.0 && sim.is_steady() {
            return Err("output.store_time_interval needs a transient run; use output.store_interval".into());
        }
        let mut names: Vec<String> = sim
            .case
            .get("output.store_fields")
            .ok()
            .and_then(|v| {
                v.as_array()
                    .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            })
            .unwrap_or_default();
        if names.is_empty() {
            names = DEFAULT_FIELDS.iter().map(|s| s.to_string()).collect();
        }
        for n in &names {
            sim.field(n).map_err(|e| format!("output.store_fields: {e}"))?;
        }
        let mesh = &sim.geom.mesh;
        let exists = dir.join(STORE_FILE).exists();
        let w = if exists && sim.iteration() > 0 {
            // a continued run adds to the store of the run it continues
            let w = Writer::open_append(dir, WriteOptions::default()).map_err(|e| e.to_string())?;
            if w.mesh_hash() != mesh.content_hash() {
                return Err(format!("{} holds a different mesh", dir.display()));
            }
            if w.snapshots().last().is_some_and(|s| s.step >= sim.iteration() as u64) {
                return Err(format!(
                    "{} already holds steps beyond the continued run's start",
                    dir.display()
                ));
            }
            w
        } else {
            if exists {
                // a fresh run replaces the store of an earlier run in the same output directory, as solution.vtu is replaced
                std::fs::remove_file(dir.join(STORE_FILE)).map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(dir.join(rcfd_tsstore::INDEX_FILE));
            }
            Writer::create(dir, mesh, WriteOptions::default()).map_err(|e| e.to_string())?
        };
        Ok(Hook {
            w,
            every,
            dt,
            bucket: if dt > 0.0 { bucket(sim.time(), dt) } else { 0.0 },
            names,
            last: None,
        })
    }

    fn write(&mut self, sim: &Simulation, step: u64, time: f64) -> Result<(), String> {
        let mut s = self.w.snapshot(step, time).map_err(|e| e.to_string())?;
        for n in &self.names {
            let v = sim.field(n).map_err(|e| e.to_string())?;
            s.field(n, unit_of(n), Location::Cell, &v, Precision::F64)
                .map_err(|e| e.to_string())?;
        }
        s.commit().map_err(|e| e.to_string())?;
        self.last = Some(step);
        Ok(())
    }

    /// Store the state after step `index` when it is due.
    pub(crate) fn after_step(&mut self, sim: &Simulation, index: usize, time: f64) -> Result<(), String> {
        let by_step = self.every > 0 && index % self.every == 0;
        let by_time = self.dt > 0.0 && bucket(time, self.dt) > self.bucket;
        if by_time {
            self.bucket = bucket(time, self.dt);
        }
        if by_step || by_time {
            self.write(sim, index as u64, time)?;
        }
        Ok(())
    }

    /// Store the final state (unless the last step was stored already) and write the footer.
    pub(crate) fn finish(mut self, sim: &Simulation) -> Result<(), String> {
        let step = sim.iteration() as u64;
        if self.last != Some(step) {
            self.write(sim, step, sim.time())?;
        }
        self.w.finish().map_err(|e| e.to_string())
    }
}
