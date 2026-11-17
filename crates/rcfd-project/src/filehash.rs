//! The hash of a mesh file or an OpenFOAM mesh folder (ADR 0734 P4): what `hash_project_file` answers and what the
//! runner writes to `run.json` `mesh_hash` at submit.
//!
//! A file: `fnv64:` of its bytes, read in pieces (the same value as [`crate::records::hash_bytes`] of the whole). A
//! folder: `fnv64:` over the sorted lines `<relative path>\0<file hash>\n` of every file below it, `/` between path
//! components, so the hash does not depend on the order the system lists the files in. Symbolic links are refused, in a
//! folder as at its top. The work is capped at [`MAX_BYTES`] and [`MAX_FILES`] (ADR 0734 Q5 (b)); the error names the
//! limit.

use crate::records::fnv64;
use rcfd_core::hash::Fnv64;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The most bytes hashed, over all files (4 GiB).
pub const MAX_BYTES: u64 = 4 << 30;
/// The most files hashed below a folder.
pub const MAX_FILES: u64 = 100_000;

/// What was hashed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathHash {
    /// `"file"` or `"folder"`.
    pub kind: &'static str,
    /// `fnv64:<16 hex>`.
    pub hash: String,
    /// Bytes read, over all files.
    pub bytes: u64,
    /// Files read (1 for a file).
    pub files: u64,
}

/// Why a path was not hashed.
#[derive(Debug)]
pub enum PathHashError {
    /// The path is not there.
    Missing(PathBuf),
    /// A symbolic link (refused, ADR 0734 P4).
    Symlink(PathBuf),
    /// Over [`MAX_BYTES`] or [`MAX_FILES`]: the text names the limit.
    OverLimit(String),
    /// Reading failed.
    Io(PathBuf, std::io::Error),
}

impl std::fmt::Display for PathHashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathHashError::Missing(p) => write!(f, "{}: no such file or folder", p.display()),
            PathHashError::Symlink(p) => write!(f, "{}: a symbolic link is not hashed", p.display()),
            PathHashError::OverLimit(m) => f.write_str(m),
            PathHashError::Io(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

impl std::error::Error for PathHashError {}

struct Budget {
    bytes: u64,
    files: u64,
    max_bytes: u64,
    max_files: u64,
}

impl Budget {
    fn file(&mut self, p: &Path) -> Result<(), PathHashError> {
        self.files += 1;
        if self.files > self.max_files {
            return Err(PathHashError::OverLimit(format!(
                "{}: more than {} files, the limit of a mesh hash (max_files)",
                p.display(),
                self.max_files
            )));
        }
        Ok(())
    }
}

fn hash_file(p: &Path, budget: &mut Budget) -> Result<String, PathHashError> {
    let io = |e| PathHashError::Io(p.to_path_buf(), e);
    let mut f = std::fs::File::open(p).map_err(io)?;
    let mut h = Fnv64::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(io)?;
        if n == 0 {
            break;
        }
        budget.bytes += n as u64;
        if budget.bytes > budget.max_bytes {
            return Err(PathHashError::OverLimit(format!(
                "{}: more than {} bytes, the limit of a mesh hash (max_bytes, 4 GiB)",
                p.display(),
                budget.max_bytes
            )));
        }
        h.write(&buf[..n]);
    }
    Ok(format!("fnv64:{:016x}", h.finish()))
}

/// The hash of the file or folder at `path` with the default limits.
pub fn hash_path(path: &Path) -> Result<PathHash, PathHashError> {
    hash_path_limited(path, MAX_BYTES, MAX_FILES)
}

/// [`hash_path`] with other limits (tests).
pub fn hash_path_limited(path: &Path, max_bytes: u64, max_files: u64) -> Result<PathHash, PathHashError> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => PathHashError::Missing(path.to_path_buf()),
        _ => PathHashError::Io(path.to_path_buf(), e),
    })?;
    let mut budget = Budget {
        bytes: 0,
        files: 0,
        max_bytes,
        max_files,
    };
    if meta.file_type().is_symlink() {
        return Err(PathHashError::Symlink(path.to_path_buf()));
    }
    if meta.is_file() {
        budget.file(path)?;
        let hash = hash_file(path, &mut budget)?;
        return Ok(PathHash {
            kind: "file",
            hash,
            bytes: budget.bytes,
            files: 1,
        });
    }
    let mut lines: Vec<(String, String)> = Vec::new();
    let mut stack = vec![(path.to_path_buf(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        let rd = std::fs::read_dir(&dir).map_err(|e| PathHashError::Io(dir.clone(), e))?;
        for e in rd {
            let e = e.map_err(|e| PathHashError::Io(dir.clone(), e))?;
            let p = e.path();
            let ft = e.file_type().map_err(|e| PathHashError::Io(p.clone(), e))?;
            let name = e.file_name().to_string_lossy().into_owned();
            let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            if ft.is_symlink() {
                return Err(PathHashError::Symlink(p));
            } else if ft.is_dir() {
                stack.push((p, r));
            } else {
                budget.file(&p)?;
                let h = hash_file(&p, &mut budget)?;
                lines.push((r, h));
            }
        }
    }
    lines.sort();
    let mut text = Vec::new();
    for (r, h) in &lines {
        text.extend_from_slice(r.as_bytes());
        text.push(0);
        text.extend_from_slice(h.as_bytes());
        text.push(b'\n');
    }
    Ok(PathHash {
        kind: "folder",
        hash: format!("fnv64:{:016x}", fnv64(&text)),
        bytes: budget.bytes,
        files: lines.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rcfd_filehash_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_file_hashes_as_its_bytes_and_a_folder_by_its_sorted_lines() {
        let d = dir("a");
        let f = d.join("m.msh");
        let bytes: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&f, &bytes).unwrap();
        let h = hash_path(&f).unwrap();
        assert_eq!(h.hash, crate::records::hash_bytes(&bytes));
        assert_eq!((h.kind, h.bytes, h.files), ("file", bytes.len() as u64, 1));
        let foam = d.join("polyMesh");
        std::fs::create_dir_all(foam.join("sets")).unwrap();
        std::fs::write(foam.join("points"), b"p").unwrap();
        std::fs::write(foam.join("faces"), b"f").unwrap();
        std::fs::write(foam.join("sets").join("z"), b"z").unwrap();
        let a = hash_path(&foam).unwrap();
        let want = format!(
            "faces\0{}\npoints\0{}\nsets/z\0{}\n",
            crate::records::hash_bytes(b"f"),
            crate::records::hash_bytes(b"p"),
            crate::records::hash_bytes(b"z")
        );
        assert_eq!(a.hash, crate::records::hash_bytes(want.as_bytes()));
        assert_eq!((a.kind, a.bytes, a.files), ("folder", 3, 3));
        std::fs::write(foam.join("faces"), b"g").unwrap();
        assert_ne!(hash_path(&foam).unwrap().hash, a.hash);
        let e = hash_path_limited(&foam, 2, 10).unwrap_err().to_string();
        assert!(e.contains("max_bytes"), "{e}");
        let e = hash_path_limited(&foam, 10, 2).unwrap_err().to_string();
        assert!(e.contains("more than 2 files") && e.contains("max_files"), "{e}");
        assert!(matches!(hash_path(&d.join("none")), Err(PathHashError::Missing(_))));
        std::fs::remove_dir_all(&d).ok();
    }
}
