//! Checkpoint / restart of steady SIMPLE runs (F-030).
//!
//! A checkpoint stores velocity, pressure and face flux bit-exactly in the
//! versioned, checksummed `rcfd-io` container together with the iteration
//! counter and a hash of the mesh. Because the solver is deterministic,
//! restarting from a checkpoint continues bit-identically.

use rcfd_core::{Error, Result};
use rcfd_fv::simple::{SimpleSolver, SimpleState};
use rcfd_io::container::Container;
use std::path::Path;

fn flatten(v: &[[f64; 3]]) -> Vec<f64> {
    v.iter().flatten().copied().collect()
}

/// Write a checkpoint of `solver` to `path`.
pub fn save_simple(path: impl AsRef<Path>, solver: &SimpleSolver) -> Result<()> {
    let st = solver.snapshot();
    let mut c = Container::new();
    let meta = serde_json::json!({
        "kind": "rcfd-simple-checkpoint",
        "iteration": st.iteration,
        "mesh_hash": format!("{:016x}", solver.geom.mesh.content_hash()),
        "n_cells": st.p.len(),
        "n_faces": st.phi.len(),
    });
    c.put("meta", meta.to_string().into_bytes());
    c.put_f64s("u", &flatten(&st.u));
    c.put_f64s("p", &st.p);
    c.put_f64s("phi", &st.phi);
    c.write_file(path)
}

/// Restore `solver` from a checkpoint written by [`save_simple`]; the mesh must be identical.
pub fn load_simple(path: impl AsRef<Path>, solver: &mut SimpleSolver) -> Result<()> {
    let c = Container::read_file(path)?;
    let meta: serde_json::Value = serde_json::from_slice(
        c.get("meta")
            .ok_or_else(|| Error::Io("checkpoint: no meta section".into()))?,
    )
    .map_err(|e| Error::Io(format!("checkpoint meta: {e}")))?;
    if meta["kind"] != "rcfd-simple-checkpoint" {
        return Err(Error::Io("not an rcfd SIMPLE checkpoint".into()));
    }
    let want = format!("{:016x}", solver.geom.mesh.content_hash());
    if meta["mesh_hash"].as_str() != Some(want.as_str()) {
        return Err(Error::Io("checkpoint was written for a different mesh".into()));
    }
    let u = c.get_f64s("u")?;
    let state = SimpleState {
        iteration: meta["iteration"].as_u64().unwrap_or(0) as usize,
        u: u.chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect(),
        p: c.get_f64s("p")?,
        phi: c.get_f64s("phi")?,
    };
    solver.restore(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_fv::simple::{Coupling, SimpleSettings};
    use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
    use rcfd_mesh::gen::cavity_2d;
    use std::sync::Arc;

    fn cavity(n: usize) -> SimpleSolver {
        let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(n, 1.0).unwrap())).unwrap());
        let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
        u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
            .unwrap();
        u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
            .unwrap();
        let p = ScalarField::uniform("p", &geom, 0.0);
        SimpleSolver::new(geom, u, p, SimpleSettings::new(0.01, Coupling::Simplec))
    }

    fn bits(s: &SimpleSolver) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
        let st = s.snapshot();
        (
            st.u.iter().flatten().map(|x| x.to_bits()).collect(),
            st.p.iter().map(|x| x.to_bits()).collect(),
            st.phi.iter().map(|x| x.to_bits()).collect(),
        )
    }

    #[test]
    fn restart_continues_bit_identically() {
        let dir = std::env::temp_dir().join(format!("rcfd_ckpt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("run.ckpt");

        let mut straight = cavity(12);
        for _ in 0..80 {
            straight.iterate();
        }

        let mut first = cavity(12);
        for _ in 0..40 {
            first.iterate();
        }
        save_simple(&file, &first).unwrap();

        let mut resumed = cavity(12);
        load_simple(&file, &mut resumed).unwrap();
        assert_eq!(resumed.snapshot().iteration, 40);
        for _ in 0..40 {
            resumed.iterate();
        }
        assert_eq!(bits(&straight), bits(&resumed), "restarted run must be bit-identical");
        assert_eq!(resumed.snapshot().iteration, 80);

        // residual history is exported as CSV
        let csv = resumed.history_csv();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "iteration,res_u,res_p,continuity,max_du,p_iters");
        assert_eq!(lines.len(), 41);
        assert!(lines[1].starts_with("41,"));
        let first_res: f64 = lines[1].split(',').nth(1).unwrap().parse().unwrap();
        let last_res: f64 = lines[40].split(',').nth(1).unwrap().parse().unwrap();
        assert!(
            last_res < first_res,
            "momentum residual should fall: {first_res} -> {last_res}"
        );

        // a checkpoint for another mesh is rejected, as is a corrupted file
        let mut other = cavity(8);
        assert!(load_simple(&file, &mut other).is_err());
        let mut bytes = std::fs::read(&file).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xff;
        std::fs::write(dir.join("bad.ckpt"), bytes).unwrap();
        assert!(load_simple(dir.join("bad.ckpt"), &mut cavity(12)).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
