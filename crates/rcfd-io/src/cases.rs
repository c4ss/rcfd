//! Benchmark and validation case registry (F-006).
//!
//! Each case lives in `cases/<id>/case.json` next to its `reference/` data.
//! The registry lists cases, resolves their reference files and can build the
//! case mesh through the block generators in `rcfd-mesh`.

use rcfd_core::{Error, Result};
use rcfd_mesh::gen::{self, Grading, StepSpec};
use rcfd_mesh::Mesh;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Mesh recipe of a case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshRecipe {
    /// Generator name (`cavity_2d`, `channel`, `backward_step`, `box`, `cylinder_ogrid`) or null.
    pub generator: Option<String>,
    /// Generator parameters.
    #[serde(default)]
    pub params: Value,
    /// Free-form note.
    #[serde(default)]
    pub note: Option<String>,
}

/// Literature reference of a case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    /// Full citation.
    pub citation: String,
    /// Reference data files relative to the case directory.
    #[serde(default)]
    pub files: Vec<String>,
    /// Inline scalar reference data.
    #[serde(default)]
    pub data: Value,
    /// Pass/fail tolerance description.
    #[serde(default)]
    pub tolerance: String,
}

/// A benchmark case definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseDef {
    /// Short unique id (directory name).
    pub id: String,
    /// Title.
    pub title: String,
    /// Description.
    pub description: String,
    /// Physics summary.
    #[serde(default)]
    pub physics: Value,
    /// Mesh recipe.
    pub mesh: MeshRecipe,
    /// Boundary conditions (informal).
    #[serde(default)]
    pub boundary_conditions: Value,
    /// Reference data.
    pub reference: Reference,
    /// Roadmap items that use this case.
    #[serde(default)]
    pub roadmap: Vec<String>,
    /// Directory the case was loaded from.
    #[serde(skip)]
    pub dir: PathBuf,
}

impl CaseDef {
    /// Absolute paths of the reference files.
    pub fn reference_paths(&self) -> Vec<PathBuf> {
        self.reference.files.iter().map(|f| self.dir.join(f)).collect()
    }

    /// Read a numeric CSV reference file (header row required) into columns.
    pub fn read_csv(&self, file: &str) -> Result<(Vec<String>, Vec<Vec<f64>>)> {
        let text = std::fs::read_to_string(self.dir.join(file))?;
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let header: Vec<String> = lines
            .next()
            .ok_or_else(|| Error::Io(format!("{file}: empty")))?
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let mut cols = vec![Vec::new(); header.len()];
        for (n, l) in lines.enumerate() {
            let cells: Vec<&str> = l.split(',').collect();
            if cells.len() != header.len() {
                return Err(Error::Io(format!(
                    "{file}: row {} has {} fields, expected {}",
                    n + 2,
                    cells.len(),
                    header.len()
                )));
            }
            for (c, v) in cols.iter_mut().zip(cells) {
                c.push(
                    v.trim()
                        .parse::<f64>()
                        .map_err(|_| Error::Io(format!("{file}: row {}: '{v}' is not a number", n + 2)))?,
                );
            }
        }
        Ok((header, cols))
    }

    /// Build the case mesh. For generators that take a list of resolutions
    /// (`cavity_2d`) `level` selects the entry; otherwise it is ignored.
    pub fn generate_mesh(&self, level: usize) -> Result<Mesh> {
        let name = self.mesh.generator.as_deref().ok_or_else(|| {
            Error::Mesh(format!(
                "case '{}' has no mesh generator: {}",
                self.id,
                self.mesh.note.clone().unwrap_or_default()
            ))
        })?;
        let p = &self.mesh.params;
        let usizes = |key: &str| -> Result<Vec<usize>> {
            p[key]
                .as_array()
                .ok_or_else(|| Error::Mesh(format!("param '{key}' missing")))?
                .iter()
                .map(|v| {
                    v.as_u64()
                        .map(|x| x as usize)
                        .ok_or_else(|| Error::Mesh(format!("param '{key}' must hold integers")))
                })
                .collect()
        };
        let f64s = |key: &str| -> Result<Vec<f64>> {
            p[key]
                .as_array()
                .ok_or_else(|| Error::Mesh(format!("param '{key}' missing")))?
                .iter()
                .map(|v| {
                    v.as_f64()
                        .ok_or_else(|| Error::Mesh(format!("param '{key}' must hold numbers")))
                })
                .collect()
        };
        let grading = |key: &str| -> Grading {
            if let Some(r) = p[key]["symmetric"].as_f64() {
                Grading::Symmetric(r)
            } else if let Some(r) = p[key]["expansion"].as_f64() {
                Grading::Expansion(r)
            } else {
                Grading::Uniform
            }
        };
        match name {
            "cavity_2d" => {
                let n = usizes("n")?;
                let n = *n
                    .get(level.min(n.len().saturating_sub(1)))
                    .ok_or_else(|| Error::Mesh("empty resolution list".into()))?;
                gen::cavity_2d(n, p["size"].as_f64().unwrap_or(1.0))
            }
            "channel" => {
                let (n, l) = (usizes("n")?, f64s("l")?);
                if n.len() != 3 || l.len() != 3 {
                    return Err(Error::Mesh("channel needs n and l of length 3".into()));
                }
                gen::channel(
                    [n[0], n[1], n[2]],
                    [l[0], l[1], l[2]],
                    [grading("x_grading"), grading("y_grading"), grading("z_grading")],
                    p["two_d"].as_bool().unwrap_or(false),
                )
            }
            "box" => {
                let (n, l) = (usizes("n")?, f64s("l")?);
                gen::box_mesh(
                    [n[0], n[1], n[2]],
                    [l[0], l[1], l[2]],
                    [grading("x_grading"), grading("y_grading"), grading("z_grading")],
                )
            }
            "backward_step" => {
                let n = usizes("n")?;
                let g = |k: &str| p[k].as_f64().ok_or_else(|| Error::Mesh(format!("param '{k}' missing")));
                let spec = StepSpec {
                    l_in: g("l_in")?,
                    l_out: g("l_out")?,
                    h_in: g("h_in")?,
                    step: g("step")?,
                    depth: g("depth")?,
                    n: [n[0], n[1], n[2], n[3]],
                };
                // optional clustering toward the step and the walls (wave 6): x_ratio / y_ratio
                let grading = match (p["x_ratio"].as_f64(), p["y_ratio"].as_f64()) {
                    (None, None) => gen::StepGrading::uniform(),
                    (rx, ry) => gen::StepGrading::clustered(rx.unwrap_or(1.0), ry.unwrap_or(1.0)),
                };
                gen::backward_step_graded(spec, grading)
            }
            other => Err(Error::Mesh(format!("unknown mesh generator '{other}'"))),
        }
    }
}

/// All cases found under a directory.
#[derive(Debug, Clone)]
pub struct Registry {
    cases: Vec<CaseDef>,
}

impl Registry {
    /// Load every `<dir>/*/case.json`.
    pub fn load(dir: impl AsRef<Path>) -> Result<Registry> {
        let dir = dir.as_ref();
        let mut cases = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(|e| Error::Io(format!("{}: {e}", dir.display())))? {
            let path = entry?.path();
            let file = path.join("case.json");
            if file.is_file() {
                let text = std::fs::read_to_string(&file)?;
                let mut c: CaseDef =
                    serde_json::from_str(&text).map_err(|e| Error::Io(format!("{}: {e}", file.display())))?;
                if path.file_name().and_then(|n| n.to_str()) != Some(c.id.as_str()) {
                    return Err(Error::Io(format!(
                        "{}: id '{}' does not match directory name",
                        file.display(),
                        c.id
                    )));
                }
                c.dir = path;
                cases.push(c);
            }
        }
        cases.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Registry { cases })
    }
    /// All cases sorted by id.
    pub fn list(&self) -> &[CaseDef] {
        &self.cases
    }
    /// Case by id.
    pub fn get(&self, id: &str) -> Option<&CaseDef> {
        self.cases.iter().find(|c| c.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::quality::check_mesh;

    fn registry() -> Registry {
        Registry::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases")).unwrap()
    }

    #[test]
    fn lists_required_cases() {
        let r = registry();
        let ids: Vec<&str> = r.list().iter().map(|c| c.id.as_str()).collect();
        for want in [
            "cavity",
            "backward_step",
            "channel",
            "flat_plate",
            "naca0012",
            "heated_cavity",
        ] {
            assert!(ids.contains(&want), "missing case {want}: {ids:?}");
        }
        for c in r.list() {
            assert!(!c.reference.citation.is_empty() && !c.title.is_empty(), "{}", c.id);
            assert!(!c.roadmap.is_empty(), "{}", c.id);
            for p in c.reference_paths() {
                assert!(p.is_file(), "{}: missing reference file {}", c.id, p.display());
            }
        }
    }

    #[test]
    fn reference_data_is_well_formed_and_sane() {
        let r = registry();
        let cav = r.get("cavity").unwrap();
        let (h, cols) = cav.read_csv("reference/ghia1982_u_centerline.csv").unwrap();
        assert_eq!(h, vec!["y", "u_Re100", "u_Re400", "u_Re1000"]);
        assert_eq!(cols[0].len(), 17);
        for c in &cols[1..] {
            assert_eq!(c[0], 1.0); // lid velocity at y = 1
            assert_eq!(*c.last().unwrap(), 0.0); // no slip at y = 0
        }
        // y decreasing from 1 to 0
        assert!(cols[0].windows(2).all(|w| w[0] > w[1]));
        // known extrema: the Re=1000 profile reaches about -0.383 near y = 0.17
        let min = cols[3].iter().cloned().fold(f64::INFINITY, f64::min);
        assert!((min + 0.38289).abs() < 1e-9);
        let hc = r.get("heated_cavity").unwrap();
        let (_, c) = hc.read_csv("reference/devahldavis1983.csv").unwrap();
        assert!(c[1].windows(2).all(|w| w[1] > w[0]), "Nusselt number grows with Ra");
        assert!(c[2].iter().zip(&c[1]).all(|(max, avg)| max > avg));
        assert!(hc.read_csv("nope.csv").is_err());
    }

    #[test]
    fn case_meshes_build_and_pass_quality_checks() {
        let r = registry();
        let mut built = 0;
        for c in r.list() {
            if c.mesh.generator.is_some() {
                let m = c.generate_mesh(0).unwrap_or_else(|e| panic!("{}: {e}", c.id));
                let q = check_mesh(&m);
                assert!(q.is_ok(), "{}: {:?}", c.id, q.errors);
                built += 1;
            } else {
                assert!(c.generate_mesh(0).is_err());
            }
        }
        assert_eq!(built, 5);
        let cav = r.get("cavity").unwrap();
        assert_eq!(cav.generate_mesh(0).unwrap().n_cells(), 32 * 32);
        assert_eq!(cav.generate_mesh(2).unwrap().n_cells(), 128 * 128);
        assert_eq!(cav.generate_mesh(99).unwrap().n_cells(), 128 * 128);
    }

    #[test]
    fn rejects_mismatched_ids() {
        let d = std::env::temp_dir().join(format!("rcfd-test-{}-cases", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a")).unwrap();
        std::fs::write(
            d.join("a/case.json"),
            r#"{"id":"b","title":"t","description":"d","mesh":{"generator":null},"reference":{"citation":"c"}}"#,
        )
        .unwrap();
        assert!(Registry::load(&d).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
