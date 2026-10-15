//! Restart files and solution transfer for [`Simulation`] (F-115, F-121, F-241; ADR 0031 D3).
//!
//! A restart file is the `rcfd-sim-checkpoint` container (kind and version 1, so older files stay
//! readable) with additive content: the meta key `engine` (`steady`, the default when absent, or
//! `transient`) and a `mesh` section holding the run mesh, so that a restart onto another mesh
//! needs no source case. The older `save_checkpoint` / `load_checkpoint` keep their steady-only
//! contract; the run-control driver uses the methods of this file.

use super::{Engine, Simulation};
use crate::case::Case;
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_fv::ddt::History;
use rcfd_fv::pimple::{PimpleState, StepInfo};
use rcfd_fv::pressure::divergence;
use rcfd_io::container::{mesh_from_bytes, mesh_to_bytes, Container};
use rcfd_mesh::Mesh;
use rcfd_post::remap::{RemapMethod, Remapper};
use serde_json::{json, Value};
use std::path::Path;

fn io(msg: impl Into<String>) -> Error {
    Error::Io(msg.into())
}

fn vec3s(flat: &[f64]) -> Vec<Vec3> {
    flat.chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect()
}

fn put_hist(c: &mut Container, prefix: &str, h: &History) {
    c.put_f64s(&format!("{prefix}.old"), &h.old);
    if let Some(o) = &h.older {
        c.put_f64s(&format!("{prefix}.older"), o);
    }
    if let Some(o) = &h.oldest {
        c.put_f64s(&format!("{prefix}.oldest"), o);
    }
    c.put_f64s(
        &format!("{prefix}.dt"),
        &[h.dt_old.unwrap_or(f64::NAN), h.dt_older.unwrap_or(f64::NAN)],
    );
}

fn get_hist(c: &Container, prefix: &str) -> Result<History> {
    let opt = |suffix: &str| -> Result<Option<Vec<f64>>> {
        let name = format!("{prefix}.{suffix}");
        if c.get(&name).is_some() {
            Ok(Some(c.get_f64s(&name)?))
        } else {
            Ok(None)
        }
    };
    let dt = c.get_f64s(&format!("{prefix}.dt"))?;
    if dt.len() != 2 {
        return Err(io("checkpoint: bad history step sizes"));
    }
    let some = |x: f64| if x.is_nan() { None } else { Some(x) };
    Ok(History {
        old: c.get_f64s(&format!("{prefix}.old"))?,
        older: opt("older")?,
        dt_old: some(dt[0]),
        oldest: opt("oldest")?,
        dt_older: some(dt[1]),
    })
}

fn meta_of(c: &Container) -> Result<Value> {
    let meta: Value = serde_json::from_slice(c.get("meta").ok_or_else(|| io("checkpoint: no meta section"))?)
        .map_err(|e| io(format!("checkpoint meta: {e}")))?;
    if meta["kind"] != "rcfd-sim-checkpoint" {
        return Err(io("not an rcfd simulation checkpoint"));
    }
    Ok(meta)
}

impl Simulation {
    fn mesh_hash_hex(&self) -> String {
        format!("{:016x}", self.geom.mesh.content_hash())
    }

    /// Transient laminar constant-density PISO/PIMPLE without any additional transported state.
    fn transient_restartable(&self) -> bool {
        matches!(self.engine, Engine::Transient(_))
            && self.turb.is_none()
            && self.energy.is_none()
            && self.scalars.is_none()
            && self.chem.is_none()
            && self.cht.is_none()
            && self.stats.is_none()
            && self.rheology.is_none()
            && self.catalogue.is_none()
            && self.udfs.is_empty()
    }

    /// Whether [`save_restart`](Self::save_restart) works for this run: steady SIMPLE/SIMPLEC without
    /// a turbulence model (as [`supports_checkpoint`](Self::supports_checkpoint)), or a transient
    /// laminar constant-density PISO/PIMPLE run without energy, scalars, statistics, rheology,
    /// catalogue boundary conditions, user functions or conjugate heat transfer.
    pub fn supports_restart(&self) -> bool {
        self.supports_checkpoint() || self.transient_restartable()
    }

    /// Write a restart file (see the module documentation). Errors when the run is not supported.
    pub fn save_restart(&self, path: &Path) -> Result<()> {
        let mut c = Container::new();
        let hash = self.mesh_hash_hex();
        match &self.engine {
            Engine::Steady(s) if self.turb.is_none() => {
                let st = s.snapshot();
                c.put(
                    "meta",
                    json!({
                        "kind": "rcfd-sim-checkpoint", "version": 1, "engine": "steady",
                        "iteration": st.iteration, "mesh_hash": hash,
                        "n_cells": st.p.len(), "n_faces": st.phi.len(),
                        "energy": self.energy.is_some(),
                    })
                    .to_string()
                    .into_bytes(),
                );
                c.put_f64s("u", &st.u.iter().flatten().copied().collect::<Vec<_>>());
                c.put_f64s("p", &st.p);
                c.put_f64s("phi", &st.phi);
                if let Some(en) = &self.energy {
                    c.put_f64s("T", &en.solver.t.values);
                }
            }
            Engine::Transient(s) if self.transient_restartable() => {
                let st = s.snapshot();
                c.put(
                    "meta",
                    json!({
                        "kind": "rcfd-sim-checkpoint", "version": 1, "engine": "transient",
                        "iteration": st.step_no, "step_no": st.step_no, "mesh_hash": hash,
                        "n_cells": st.p.len(), "n_faces": st.phi.len(), "energy": false,
                        "has_last": st.last.is_some(),
                        "last_step": st.last.map(|l| l.step), "last_p_iters": st.last.map(|l| l.p_iters),
                    })
                    .to_string()
                    .into_bytes(),
                );
                c.put_f64s("u", &st.u.iter().flatten().copied().collect::<Vec<_>>());
                c.put_f64s("p", &st.p);
                c.put_f64s("phi", &st.phi);
                c.put_f64s("time", &[st.time]);
                for k in 0..3 {
                    put_hist(&mut c, &format!("hu{k}"), &st.hist_u[k]);
                }
                put_hist(&mut c, "hphi", &st.phi_hist);
                if let Some(l) = st.last {
                    c.put_f64s("last", &[l.time, l.dt, l.courant, l.continuity, l.res_u, l.res_p]);
                }
            }
            _ => {
                return Err(Error::Other(
                    "restart files are available for steady SIMPLE/SIMPLEC runs without a turbulence model and for \
                     transient laminar constant-density PISO/PIMPLE runs without energy, scalars, statistics, \
                     rheology, catalogue boundary conditions or user functions"
                        .into(),
                ))
            }
        }
        c.put("mesh", mesh_to_bytes(&self.geom.mesh));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        c.write_file(path)
    }

    /// Restore a restart file written by [`save_restart`](Self::save_restart) (or a v1 checkpoint)
    /// of the same engine into this freshly built simulation on the same mesh; the run continues
    /// exactly where the file left off.
    pub fn load_restart(&mut self, path: &Path) -> Result<()> {
        let c = Container::read_file(path)?;
        let meta = meta_of(&c)?;
        match meta["engine"].as_str().unwrap_or("steady") {
            "steady" => self.load_checkpoint(path),
            "transient" => {
                if meta["mesh_hash"].as_str() != Some(self.mesh_hash_hex().as_str()) {
                    return Err(io("checkpoint was written for a different mesh"));
                }
                if !self.transient_restartable() {
                    return Err(Error::Other(
                        "this simulation cannot be restored from a transient restart file".into(),
                    ));
                }
                let st = Self::transient_state(&c, &meta)?;
                let Engine::Transient(s) = &mut self.engine else {
                    unreachable!("checked by transient_restartable")
                };
                s.restore(&st)?;
                self.reset_run_state();
                Ok(())
            }
            other => Err(io(format!("unknown checkpoint engine '{other}'"))),
        }
    }

    fn reset_run_state(&mut self) {
        self.done = false;
        self.converged = false;
        self.last = None;
        self.stop_reason = None;
    }

    fn transient_state(c: &Container, meta: &Value) -> Result<PimpleState> {
        let last = if meta["has_last"].as_bool().unwrap_or(false) {
            let l = c.get_f64s("last")?;
            if l.len() != 6 {
                return Err(io("checkpoint: bad last-step record"));
            }
            Some(StepInfo {
                step: meta["last_step"].as_u64().unwrap_or(0) as usize,
                time: l[0],
                dt: l[1],
                courant: l[2],
                continuity: l[3],
                res_u: l[4],
                res_p: l[5],
                p_iters: meta["last_p_iters"].as_u64().unwrap_or(0) as usize,
            })
        } else {
            None
        };
        let time = c.get_f64s("time")?;
        Ok(PimpleState {
            u: vec3s(&c.get_f64s("u")?),
            p: c.get_f64s("p")?,
            phi: c.get_f64s("phi")?,
            time: *time.first().ok_or_else(|| io("checkpoint: no time"))?,
            step_no: meta["step_no"].as_u64().unwrap_or(0) as usize,
            hist_u: [get_hist(c, "hu0")?, get_hist(c, "hu1")?, get_hist(c, "hu2")?],
            phi_hist: get_hist(c, "hphi")?,
            last,
        })
    }

    /// Start this freshly built simulation from the solution in a restart file (ADR 0031 D3).
    ///
    /// * Same mesh: the state is restored exactly (same engine), or, for a steady file into a
    ///   transient run, the velocity, pressure and flux initialise the run at time 0 with a fresh
    ///   history.
    /// * Different mesh: velocity, pressure (and temperature when both runs solve the energy
    ///   equation) are transferred with `method`; the face flux is rebuilt from the velocity; a
    ///   transient file keeps its time, history is fresh. The source mesh is the one embedded in
    ///   the file, else the mesh of `source_case`. `conservative_overlap` on a mesh pair that is not
    ///   made of box cells falls back to `conservative` (reported).
    ///
    /// Returns a report (`restart`: `exact`, `initialised` or `transferred`, and details).
    pub fn restart_from(&mut self, path: &Path, method: RemapMethod, source_case: Option<&Case>) -> Result<Value> {
        let c = Container::read_file(path)?;
        let meta = meta_of(&c)?;
        let file_engine = meta["engine"].as_str().unwrap_or("steady").to_string();
        if self.turb.is_some() {
            return Err(Error::Other(
                "the turbulence-model state is not part of a restart file; restarting a turbulence run is not supported yet"
                    .into(),
            ));
        }
        let same_mesh = meta["mesh_hash"].as_str() == Some(self.mesh_hash_hex().as_str());
        let target_transient = matches!(self.engine, Engine::Transient(_));
        let target_steady = matches!(self.engine, Engine::Steady(_));
        if !target_transient && !target_steady {
            return Err(Error::Other(
                "restart is available for SIMPLE/SIMPLEC and PISO/PIMPLE runs only".into(),
            ));
        }
        if file_engine == "transient" && target_steady {
            return Err(Error::Other(
                "a transient restart file cannot start a steady run (use a steady file)".into(),
            ));
        }
        if same_mesh && file_engine == target_kind(target_transient) {
            self.load_restart(path)?;
            return Ok(json!({"restart": "exact", "engine": file_engine, "cells": self.geom.n_cells()}));
        }
        // fields of the file
        let u_src = vec3s(&c.get_f64s("u")?);
        let p_src = c.get_f64s("p")?;
        let t_src = (meta["energy"].as_bool().unwrap_or(false) && self.energy.is_some())
            .then(|| c.get_f64s("T"))
            .transpose()?;
        let time_in = if file_engine == "transient" {
            c.get_f64s("time")?.first().copied().unwrap_or(0.0)
        } else {
            0.0
        };
        let (u, p, t, phi, report) = if same_mesh {
            (
                u_src,
                p_src,
                t_src,
                Some(c.get_f64s("phi")?),
                json!({"restart": "initialised", "from": file_engine, "cells": self.geom.n_cells()}),
            )
        } else {
            let src_mesh: Mesh = match c.get("mesh") {
                Some(b) => mesh_from_bytes(b)?,
                None => {
                    let sc = source_case.ok_or_else(|| {
                        io("the restart file was written for a different mesh and contains no mesh; give the source case (restart.source_case / --source-case)")
                    })?;
                    crate::sim::build_mesh(sc).map_err(|d| {
                        Error::Mesh(format!(
                            "source case mesh: {}",
                            d.iter().map(|x| x.message.clone()).collect::<Vec<_>>().join("; ")
                        ))
                    })?
                }
            };
            if meta["mesh_hash"].as_str() != Some(format!("{:016x}", src_mesh.content_hash()).as_str()) {
                return Err(io(
                    "the restart file was not written for the mesh of the source case (or its embedded mesh is damaged)",
                ));
            }
            if u_src.len() != src_mesh.n_cells() || p_src.len() != src_mesh.n_cells() {
                return Err(io("restart file fields do not match its mesh"));
            }
            let mut used = method;
            let mut note = Value::Null;
            let remap = match Remapper::new(&src_mesh, &self.geom.mesh, used) {
                Ok(r) => r,
                Err(e) if used == RemapMethod::ConservativeOverlap => {
                    note = json!(format!("conservative_overlap unavailable ({e}); used conservative"));
                    used = RemapMethod::Conservative;
                    Remapper::new(&src_mesh, &self.geom.mesh, used)?
                }
                Err(e) => return Err(e),
            };
            let u = remap.vector(&u_src)?;
            let p = remap.scalar(&p_src)?;
            let t = t_src.as_ref().map(|t| remap.scalar(t)).transpose()?;
            (
                u,
                p,
                t,
                None,
                json!({
                    "restart": "transferred", "from": file_engine, "method": used.name(),
                    "fallback": note, "source_cells": src_mesh.n_cells(),
                    "target_cells": self.geom.n_cells(), "min_coverage": remap.min_coverage(),
                }),
            )
        };
        let mut report = report;
        match &mut self.engine {
            Engine::Steady(s) => {
                match phi {
                    Some(phi) => {
                        let st = rcfd_fv::simple::SimpleState {
                            iteration: 0,
                            u,
                            p,
                            phi,
                        };
                        s.restore(&st)?;
                    }
                    None => s.set_fields(u, p)?,
                }
                report["max_flux_imbalance"] = json!(flux_imbalance(&self.geom, &s.phi));
            }
            Engine::Transient(s) => {
                s.reinitialise(u, p, phi, time_in)?;
                report["max_flux_imbalance"] = json!(flux_imbalance(&self.geom, &s.phi));
                report["time"] = json!(time_in);
            }
            _ => unreachable!("checked above"),
        }
        if let (Some(en), Some(t)) = (&mut self.energy, t) {
            en.solver.t.values = t;
        }
        self.reset_run_state();
        Ok(report)
    }
}

fn target_kind(transient: bool) -> &'static str {
    if transient {
        "transient"
    } else {
        "steady"
    }
}

/// Largest cell flux imbalance divided by the largest face area.
fn flux_imbalance(geom: &rcfd_fv::FvGeometry, phi: &[f64]) -> f64 {
    let div = divergence(geom, phi);
    let amax = geom
        .mesh
        .face_area_vectors()
        .iter()
        .map(|s| rcfd_core::vec3::norm(*s))
        .fold(0.0, f64::max)
        .max(1e-300);
    div.iter().fold(0.0f64, |a, &b| a.max(b.abs())) / amax
}
