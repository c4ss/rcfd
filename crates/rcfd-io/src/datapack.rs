//! Public validation-data pack manifest (F-125).
//!
//! `validation-data/manifest.json` lists every external benchmark dataset with its upstream URL,
//! SHA-256, licence record and citation, plus in-repo reference tables with their checksums.
//! This module parses and checks the manifest and verifies local copies. It never touches the
//! network; downloading is done by `validation-data/fetch.py`.

use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

/// Licence record of a dataset.
#[derive(Debug, Clone, Deserialize)]
pub struct Licence {
    /// SPDX identifier or `LicenseRef-...`.
    pub spdx: String,
    /// Licence name or statement.
    pub name: String,
    /// Where the licence is stated.
    pub url: String,
    /// Attribution line.
    #[serde(default)]
    pub attribution: String,
    /// Caveats.
    #[serde(default)]
    pub notes: String,
}

/// One file of a dataset: either fetched (`path` + `url`) or an in-repo table (`local`).
#[derive(Debug, Clone, Deserialize)]
pub struct FileEntry {
    /// Download target relative to `data/<id>/`.
    #[serde(default)]
    pub path: Option<String>,
    /// Upstream URL.
    #[serde(default)]
    pub url: Option<String>,
    /// Repo-relative path of an in-repo table.
    #[serde(default)]
    pub local: Option<String>,
    /// Lower-case hex SHA-256.
    #[serde(default)]
    pub sha256: String,
    /// Size in bytes.
    #[serde(default)]
    pub bytes: u64,
}

impl FileEntry {
    /// Name shown in reports.
    pub fn name(&self) -> &str {
        self.local.as_deref().or(self.path.as_deref()).unwrap_or("?")
    }
}

/// A dataset.
#[derive(Debug, Clone, Deserialize)]
pub struct Dataset {
    /// Unique id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Case id in `cases/` it validates, if any.
    #[serde(default)]
    pub case: Option<String>,
    /// Literature citation.
    pub citation: String,
    /// `available` or `unresolved`.
    pub status: String,
    /// Why it is unresolved.
    #[serde(default)]
    pub unresolved_reason: String,
    /// Licence record.
    pub licence: Licence,
    /// Files.
    #[serde(default)]
    pub files: Vec<FileEntry>,
}

/// The manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    /// Must be `rcfd-data-manifest`.
    pub format: String,
    /// Must be 1.
    pub version: u32,
    /// Datasets.
    pub datasets: Vec<Dataset>,
}

/// Verification result of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    /// Present and checksum matches.
    Ok,
    /// Not present locally (not an error for fetched files).
    Missing,
    /// Present with a different checksum.
    Mismatch(String),
}

impl Manifest {
    /// Parse a manifest file.
    pub fn load(path: &Path) -> Result<Manifest, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Structural checks; returns every problem found (empty = valid). `repo_root` is used to
    /// check `local` files (existence and checksum) and case ids.
    pub fn validate(&self, repo_root: &Path) -> Vec<String> {
        let mut p = Vec::new();
        if self.format != "rcfd-data-manifest" || self.version != 1 {
            p.push("not an rcfd-data-manifest version 1 document".to_string());
        }
        let mut ids = std::collections::HashSet::new();
        for d in &self.datasets {
            let at = |m: &str| format!("{}: {m}", d.id);
            if !ids.insert(d.id.as_str()) {
                p.push(at("duplicate dataset id"));
            }
            if d.title.is_empty() || d.citation.is_empty() {
                p.push(at("title and citation are required"));
            }
            if d.licence.spdx.is_empty() || d.licence.name.is_empty() {
                p.push(at("licence record needs spdx and name"));
            }
            if let Some(c) = &d.case {
                if !repo_root.join("cases").join(c).join("case.json").exists() {
                    p.push(at(&format!("case '{c}' does not exist in cases/")));
                }
            }
            match d.status.as_str() {
                "unresolved" => {
                    if d.unresolved_reason.is_empty() {
                        p.push(at("unresolved dataset needs unresolved_reason"));
                    }
                    if !d.files.is_empty() {
                        p.push(at("unresolved dataset must not list files"));
                    }
                    continue;
                }
                "available" => {}
                s => {
                    p.push(at(&format!("unknown status '{s}'")));
                    continue;
                }
            }
            if d.licence.url.is_empty() {
                p.push(at("licence record needs a url"));
            }
            if d.files.is_empty() {
                p.push(at("available dataset has no files"));
            }
            let mut names = std::collections::HashSet::new();
            for f in &d.files {
                let name = f.name().to_string();
                let fat = |m: &str| format!("{}/{name}: {m}", d.id);
                if !names.insert(name.clone()) {
                    p.push(fat("duplicate file"));
                }
                if f.sha256.len() != 64 || !f.sha256.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
                    p.push(fat("sha256 must be 64 lower-case hex digits"));
                }
                if f.bytes == 0 {
                    p.push(fat("bytes must be recorded"));
                }
                let low = format!(
                    "{} {} {}",
                    f.url.as_deref().unwrap_or(""),
                    f.path.as_deref().unwrap_or(""),
                    f.local.as_deref().unwrap_or("")
                )
                .to_lowercase();
                for bad in ["ansys", "unpacked", "program files", "\\", "file://"] {
                    if low.contains(bad) {
                        p.push(fat(&format!(
                            "source mentions '{bad}': only public upstreams are allowed"
                        )));
                    }
                }
                match (&f.local, &f.url, &f.path) {
                    (Some(l), None, None) => {
                        let lp = Path::new(l);
                        if lp.is_absolute() || lp.components().any(|c| matches!(c, Component::ParentDir)) {
                            p.push(fat("local path must be repo-relative"));
                            continue;
                        }
                        match file_state(&repo_root.join(lp), &f.sha256) {
                            FileState::Ok => {}
                            FileState::Missing => p.push(fat("in-repo file is missing")),
                            FileState::Mismatch(g) => {
                                p.push(fat(&format!("checksum differs from the manifest (actual {g})")))
                            }
                        }
                    }
                    (None, Some(u), Some(rel)) => {
                        if !(u.starts_with("https://") || u.starts_with("http://")) {
                            p.push(fat("url must be http(s)"));
                        }
                        let rp = Path::new(rel);
                        if rp.is_absolute() || rp.components().any(|c| matches!(c, Component::ParentDir)) {
                            p.push(fat("path must be relative without '..'"));
                        }
                    }
                    _ => p.push(fat("a file needs either `local`, or both `url` and `path`")),
                }
            }
        }
        p
    }

    /// Check the local copies of one dataset: in-repo files under `repo_root`, fetched files under
    /// `data_dir/<id>/`.
    pub fn verify(&self, repo_root: &Path, data_dir: &Path) -> Vec<(String, String, FileState)> {
        let mut out = Vec::new();
        for d in self.datasets.iter().filter(|d| d.status == "available") {
            for f in &d.files {
                let path: PathBuf = match (&f.local, &f.path) {
                    (Some(l), _) => repo_root.join(l),
                    (None, Some(rel)) => data_dir.join(&d.id).join(rel),
                    _ => continue,
                };
                out.push((d.id.clone(), f.name().to_string(), file_state(&path, &f.sha256)));
            }
        }
        out
    }

    /// Markdown provenance table (dataset, validates, licence, files with checksums).
    pub fn provenance_markdown(&self) -> String {
        let mut s = String::from("# Validation data provenance\n\n");
        for d in &self.datasets {
            s.push_str(&format!("## {} (`{}`)\n\n", d.title, d.id));
            s.push_str(&format!("- status: {}\n", d.status));
            if let Some(c) = &d.case {
                s.push_str(&format!("- validates case: `{c}`\n"));
            }
            s.push_str(&format!("- citation: {}\n", d.citation));
            s.push_str(&format!(
                "- licence: {} ({}), {}\n",
                d.licence.spdx, d.licence.name, d.licence.url
            ));
            if !d.licence.attribution.is_empty() {
                s.push_str(&format!("- attribution: {}\n", d.licence.attribution));
            }
            if !d.licence.notes.is_empty() {
                s.push_str(&format!("- licence notes: {}\n", d.licence.notes));
            }
            if !d.unresolved_reason.is_empty() {
                s.push_str(&format!("- unresolved: {}\n", d.unresolved_reason));
            }
            for f in &d.files {
                let src = f.url.clone().unwrap_or_else(|| "in this repository".into());
                s.push_str(&format!(
                    "  - `{}` sha256 `{}` ({} bytes), {}\n",
                    f.name(),
                    f.sha256,
                    f.bytes,
                    src
                ));
            }
            s.push('\n');
        }
        s
    }
}

fn file_state(path: &Path, expected: &str) -> FileState {
    match std::fs::read(path) {
        Err(_) => FileState::Missing,
        Ok(b) => {
            let got = sha256_hex(&b);
            if got == expected {
                FileState::Ok
            } else {
                FileState::Mismatch(got)
            }
        }
    }
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
    0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
    0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
    0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
    0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
    0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
    0xc67178f2,
];

/// SHA-256 of `data` as lower-case hex (FIPS 180-4).
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, c) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_fips_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let million_a = vec![b'a'; 1_000_000];
        assert_eq!(
            sha256_hex(&million_a),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
}
