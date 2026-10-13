//! Native rcfd case/data container (F-015).
//!
//! A simple versioned, checksummed, sectioned binary file (HDF5-like in
//! spirit: named datasets in one file, but dependency-free):
//!
//! ```text
//! "RCFDCASE" | u32 version | u32 n_sections |
//!   per section: u32 name_len | name | u64 payload_len | u64 fnv64(payload) | payload
//! ```
//!
//! All integers are little-endian; floating point data is stored as raw
//! IEEE-754 bits, so write/read is bit-exact (required for restarts).
//! Unknown sections are preserved on read, and files written by a newer
//! major format version are rejected.

use rcfd_core::hash::fnv64;
use rcfd_core::settings::{Schema, SettingsTree};
use rcfd_core::{Error, Result};
use rcfd_mesh::mesh::MeshTopology;
use rcfd_mesh::{Mesh, Patch, Zone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// File magic.
pub const MAGIC: &[u8; 8] = b"RCFDCASE";
/// Format version written by this build.
pub const VERSION: u32 = 1;

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Io(format!("container: {}", msg.into())))
}

/// Named binary sections.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Container {
    sections: BTreeMap<String, Vec<u8>>,
}

impl Container {
    /// Empty container.
    pub fn new() -> Self {
        Self::default()
    }
    /// Insert or replace a section.
    pub fn put(&mut self, name: &str, bytes: Vec<u8>) {
        self.sections.insert(name.to_string(), bytes);
    }
    /// Section payload.
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.sections.get(name).map(|v| v.as_slice())
    }
    /// Section names (sorted).
    pub fn names(&self) -> Vec<&str> {
        self.sections.keys().map(|s| s.as_str()).collect()
    }
    /// Store an `f64` array (bit exact).
    pub fn put_f64s(&mut self, name: &str, data: &[f64]) {
        let mut b = Vec::with_capacity(data.len() * 8);
        for x in data {
            b.extend_from_slice(&x.to_bits().to_le_bytes());
        }
        self.put(name, b);
    }
    /// Load an `f64` array.
    pub fn get_f64s(&self, name: &str) -> Result<Vec<f64>> {
        let b = self
            .get(name)
            .ok_or_else(|| Error::Io(format!("container: no section '{name}'")))?;
        if b.len() % 8 != 0 {
            return bad(format!("section '{name}' is not an f64 array"));
        }
        Ok(b.chunks_exact(8)
            .map(|c| f64::from_bits(u64::from_le_bytes(c.try_into().expect("8 bytes"))))
            .collect())
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(self.sections.len() as u32).to_le_bytes());
        for (name, data) in &self.sections {
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&(data.len() as u64).to_le_bytes());
            out.extend_from_slice(&fnv64(data).to_le_bytes());
            out.extend_from_slice(data);
        }
        out
    }

    /// Parse bytes, verifying magic, version and checksums.
    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        struct R<'a>(&'a [u8], usize);
        impl<'a> R<'a> {
            fn take(&mut self, n: usize) -> Result<&'a [u8]> {
                if self.1 + n > self.0.len() {
                    return bad("truncated file");
                }
                let s = &self.0[self.1..self.1 + n];
                self.1 += n;
                Ok(s)
            }
            fn u32(&mut self) -> Result<u32> {
                Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4 bytes")))
            }
            fn u64(&mut self) -> Result<u64> {
                Ok(u64::from_le_bytes(self.take(8)?.try_into().expect("8 bytes")))
            }
        }
        let mut r = R(b, 0);
        if r.take(8)? != MAGIC {
            return bad("not an rcfd container (bad magic)");
        }
        let version = r.u32()?;
        if version == 0 || version > VERSION {
            return bad(format!(
                "unsupported container version {version} (this build reads up to {VERSION})"
            ));
        }
        let n = r.u32()?;
        let mut sections = BTreeMap::new();
        for _ in 0..n {
            let nl = r.u32()? as usize;
            let name = String::from_utf8(r.take(nl)?.to_vec()).or_else(|_| bad("section name is not UTF-8"))?;
            let len = r.u64()? as usize;
            let sum = r.u64()?;
            let data = r.take(len)?;
            if fnv64(data) != sum {
                return bad(format!("checksum mismatch in section '{name}'"));
            }
            sections.insert(name, data.to_vec());
        }
        if r.1 != b.len() {
            return bad("trailing bytes after last section");
        }
        Ok(Container { sections })
    }

    /// Write atomically (temp file then rename).
    pub fn write_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let tmp = path.with_extension("tmp-write");
        std::fs::write(&tmp, self.to_bytes())?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
    /// Read a container file.
    pub fn read_file(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_bytes(&std::fs::read(path)?)
    }
}

#[derive(Serialize, Deserialize)]
struct MeshMeta {
    patches: Vec<Patch>,
    cell_zones: Vec<Zone>,
    face_zones: Vec<Zone>,
    n_points: usize,
    n_faces: usize,
    n_neighbour: usize,
    n_face_points: usize,
}

fn put_u64s(out: &mut Vec<u8>, v: &[usize]) {
    for x in v {
        out.extend_from_slice(&(*x as u64).to_le_bytes());
    }
}

/// Encode a mesh (topology exact; geometry is recomputed on load).
pub fn mesh_to_bytes(m: &Mesh) -> Vec<u8> {
    let t = m.topology();
    let meta = MeshMeta {
        patches: t.patches.clone(),
        cell_zones: t.cell_zones.clone(),
        face_zones: t.face_zones.clone(),
        n_points: t.points.len(),
        n_faces: t.owner.len(),
        n_neighbour: t.neighbour.len(),
        n_face_points: t.face_points.len(),
    };
    let json = serde_json::to_vec(&meta).expect("meta serializable");
    let mut out = Vec::new();
    out.extend_from_slice(&(json.len() as u64).to_le_bytes());
    out.extend_from_slice(&json);
    for p in &t.points {
        for x in p {
            out.extend_from_slice(&x.to_bits().to_le_bytes());
        }
    }
    put_u64s(&mut out, &t.face_offsets);
    put_u64s(&mut out, &t.face_points);
    put_u64s(&mut out, &t.owner);
    put_u64s(&mut out, &t.neighbour);
    out
}

struct Cursor<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.pos + n > self.b.len() {
            return bad("truncated mesh section");
        }
        let s = &self.b[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u64s(&mut self, n: usize) -> Result<Vec<usize>> {
        Ok(self
            .take(8 * n)?
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().expect("8")) as usize)
            .collect())
    }
}

/// Decode and re-validate a mesh.
pub fn mesh_from_bytes(b: &[u8]) -> Result<Mesh> {
    let mut c = Cursor { b, pos: 0 };
    let jl = u64::from_le_bytes(c.take(8)?.try_into().expect("8")) as usize;
    let meta: MeshMeta = serde_json::from_slice(c.take(jl)?).or_else(|e| bad(format!("mesh metadata: {e}")))?;
    let mut points = Vec::with_capacity(meta.n_points);
    for chunk in c.take(24 * meta.n_points)?.chunks_exact(24) {
        let f = |i: usize| f64::from_bits(u64::from_le_bytes(chunk[8 * i..8 * i + 8].try_into().expect("8")));
        points.push([f(0), f(1), f(2)]);
    }
    let face_offsets = c.u64s(meta.n_faces + 1)?;
    let face_points = c.u64s(meta.n_face_points)?;
    let owner = c.u64s(meta.n_faces)?;
    let neighbour = c.u64s(meta.n_neighbour)?;
    Mesh::from_topology(MeshTopology {
        points,
        face_offsets,
        face_points,
        owner,
        neighbour,
        patches: meta.patches,
        cell_zones: meta.cell_zones,
        face_zones: meta.face_zones,
    })
}

#[derive(Serialize, Deserialize)]
struct Meta {
    format: String,
    version: u32,
    mesh_hash: u64,
    settings_hash: u64,
}

/// Everything stored in a case file.
pub struct LoadedCase {
    /// The mesh.
    pub mesh: Mesh,
    /// The settings tree (validated against the supplied schema).
    pub settings: SettingsTree,
    /// Raw field arrays by name (sections prefixed `field/`).
    pub fields: BTreeMap<String, Vec<f64>>,
}

/// Write mesh + settings (+ optional named field arrays) to a case file.
pub fn save_case(
    path: impl AsRef<Path>,
    mesh: &Mesh,
    settings: &SettingsTree,
    fields: &[(&str, &[f64])],
) -> Result<()> {
    let mut c = Container::new();
    c.put("mesh", mesh_to_bytes(mesh));
    c.put("settings", settings.to_json_string().into_bytes());
    let meta = Meta {
        format: "rcfd-case".into(),
        version: VERSION,
        mesh_hash: mesh.content_hash(),
        settings_hash: settings.content_hash(),
    };
    c.put("meta", serde_json::to_vec(&meta).expect("meta serializable"));
    for (name, data) in fields {
        c.put_f64s(&format!("field/{name}"), data);
    }
    c.write_file(path)
}

/// Load a case file; verifies the stored mesh and settings hashes.
pub fn load_case(path: impl AsRef<Path>, schema: Schema) -> Result<LoadedCase> {
    let c = Container::read_file(path)?;
    let need = |n: &str| {
        c.get(n)
            .ok_or_else(|| Error::Io(format!("container: missing section '{n}'")))
    };
    let meta: Meta = serde_json::from_slice(need("meta")?).or_else(|e| bad(format!("meta: {e}")))?;
    let mesh = mesh_from_bytes(need("mesh")?)?;
    let json: serde_json::Value =
        serde_json::from_slice(need("settings")?).or_else(|e| bad(format!("settings: {e}")))?;
    let settings = SettingsTree::from_json(schema, &json)?;
    if mesh.content_hash() != meta.mesh_hash {
        return bad("mesh hash mismatch");
    }
    if settings.content_hash() != meta.settings_hash {
        return bad("settings hash mismatch");
    }
    let mut fields = BTreeMap::new();
    for n in c.names() {
        if let Some(f) = n.strip_prefix("field/") {
            fields.insert(f.to_string(), c.get_f64s(n)?);
        }
    }
    Ok(LoadedCase { mesh, settings, fields })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_core::settings::Schema;
    use rcfd_core::units::Dimension;
    use rcfd_mesh::gen::{box_mesh, cylinder_ogrid, Grading};
    use serde_json::json;

    fn schema() -> Schema {
        Schema::object()
            .field(
                "solver",
                Schema::object()
                    .field("relax", Schema::float(0.7))
                    .field("n", Schema::int(10)),
            )
            .field(
                "inlet",
                Schema::object().field("u", Schema::float(1.0).dim(Dimension::VELOCITY)),
            )
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("rcfd-test-{}-{name}", std::process::id()))
    }

    #[test]
    fn mesh_and_settings_round_trip_with_identical_hashes() {
        let mut mesh = cylinder_ogrid(12, 5, 0.5, 3.0, Grading::Expansion(4.0), 0.1).unwrap();
        mesh.add_cell_zone("inner", vec![0, 1, 2]).unwrap();
        mesh.set_periodic("cylinder", "farfield").unwrap();
        let mut s = SettingsTree::new(schema());
        s.set("solver.relax", json!(0.123456789012345)).unwrap();
        s.set("inlet.u", json!("36 km/h")).unwrap();
        let fld: Vec<f64> = (0..mesh.n_cells())
            .map(|i| (i as f64).sin() * 1e-17 + 1.0 / 3.0)
            .collect();
        let path = tmp("case.rcfd");
        save_case(&path, &mesh, &s, &[("p", &fld)]).unwrap();
        let back = load_case(&path, schema()).unwrap();
        assert_eq!(back.mesh.content_hash(), mesh.content_hash());
        assert_eq!(back.mesh.topology(), mesh.topology());
        assert_eq!(back.settings.content_hash(), s.content_hash());
        assert_eq!(back.settings.to_json(), s.to_json());
        // bit-exact field data
        assert!(back.fields["p"]
            .iter()
            .zip(&fld)
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corruption_and_version_checks() {
        let mesh = box_mesh([2, 2, 1], [1.0; 3], [Grading::Uniform; 3]).unwrap();
        let s = SettingsTree::new(schema());
        let path = tmp("corrupt.rcfd");
        save_case(&path, &mesh, &s, &[]).unwrap();
        let good = std::fs::read(&path).unwrap();
        // flip a payload byte -> checksum mismatch
        let mut bad_bytes = good.clone();
        let n = bad_bytes.len();
        bad_bytes[n - 5] ^= 0xff;
        assert!(Container::from_bytes(&bad_bytes)
            .unwrap_err()
            .to_string()
            .contains("checksum"));
        // wrong magic
        let mut m = good.clone();
        m[0] = b'X';
        assert!(Container::from_bytes(&m).is_err());
        // future version
        let mut v = good.clone();
        v[8..12].copy_from_slice(&99u32.to_le_bytes());
        assert!(Container::from_bytes(&v).unwrap_err().to_string().contains("version"));
        // truncation
        assert!(Container::from_bytes(&good[..good.len() - 3]).is_err());
        // settings from a different schema are rejected
        let other = Schema::object().field("zzz", Schema::int(1));
        assert!(load_case(&path, other).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unknown_sections_survive_and_arrays_are_bit_exact() {
        let mut c = Container::new();
        c.put("future/thing", vec![1, 2, 3]);
        c.put_f64s("x", &[f64::NAN, -0.0, 1e-300, f64::INFINITY]);
        let back = Container::from_bytes(&c.to_bytes()).unwrap();
        assert_eq!(back.get("future/thing").unwrap(), &[1, 2, 3]);
        let x = back.get_f64s("x").unwrap();
        assert!(x[0].is_nan() && x[1].to_bits() == (-0.0f64).to_bits() && x[3].is_infinite());
        assert!(back.get_f64s("nope").is_err());
        assert_eq!(back, c);
    }
}
