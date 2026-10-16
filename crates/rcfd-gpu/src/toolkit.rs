//! Locating the CUDA toolkit libraries (NVRTC) at run time (ADR 0038 D4). Pure functions of an environment lookup and a
//! list of install roots, so they are testable without a GPU.

use std::path::{Path, PathBuf};

/// A directory that holds the NVRTC library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolkit {
    /// Directory containing the NVRTC library (to be put on the library search path).
    pub dll_dir: PathBuf,
    /// Where the directory came from (`RCFD_CUDA_BIN`, `CUDA_PATH`, `CUDA_PATH_V13_4`, `scan`).
    pub source: String,
}

fn is_nvrtc_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if cfg!(windows) {
        lower.starts_with("nvrtc64_") && lower.ends_with(".dll")
    } else {
        lower.starts_with("libnvrtc.so")
    }
}

/// Whether `dir` contains an NVRTC library.
pub fn has_nvrtc(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .any(|e| e.file_name().to_str().map(is_nvrtc_name).unwrap_or(false))
        })
        .unwrap_or(false)
}

/// Directories to try below one root (the root itself, then the usual library sub-directories).
fn candidates_below(root: &Path) -> Vec<PathBuf> {
    vec![
        root.to_path_buf(),
        root.join("bin").join("x64"),
        root.join("bin"),
        root.join("lib64"),
        root.join("lib").join("x64"),
    ]
}

/// Default install roots scanned when no variable names the toolkit: the newest version first.
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let base = if cfg!(windows) {
        PathBuf::from(r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA")
    } else {
        PathBuf::from("/usr/local")
    };
    if let Ok(rd) = std::fs::read_dir(&base) {
        let mut found: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with('v') || n.starts_with("cuda"))
                    .unwrap_or(false)
            })
            .collect();
        // lexicographic descending is good enough for v12.x / v13.x directory names of equal width
        found.sort();
        found.reverse();
        roots.extend(found);
    }
    roots
}

/// Locate the toolkit. `env` looks up an environment variable; `extra_roots` are scanned last. On failure the error
/// lists every directory that was searched.
pub fn locate_with(
    env: &dyn Fn(&str) -> Option<String>,
    env_names: &[String],
    extra_roots: &[PathBuf],
) -> Result<Toolkit, String> {
    let mut searched: Vec<String> = Vec::new();
    let try_root = |root: PathBuf, source: &str, searched: &mut Vec<String>| -> Option<Toolkit> {
        for c in candidates_below(&root) {
            if has_nvrtc(&c) {
                return Some(Toolkit {
                    dll_dir: c,
                    source: source.to_string(),
                });
            }
        }
        searched.push(format!("{} ({source})", root.display()));
        None
    };
    // 1 explicit directory
    if let Some(v) = env("RCFD_CUDA_BIN").filter(|v| !v.is_empty()) {
        if let Some(t) = try_root(PathBuf::from(v), "RCFD_CUDA_BIN", &mut searched) {
            return Ok(t);
        }
    }
    // 2 CUDA_PATH, then CUDA_PATH_V*
    let mut names: Vec<String> = vec!["CUDA_PATH".to_string()];
    let mut versioned: Vec<String> = env_names
        .iter()
        .filter(|n| n.starts_with("CUDA_PATH_V"))
        .cloned()
        .collect();
    versioned.sort();
    versioned.reverse();
    names.extend(versioned);
    for name in names {
        if let Some(v) = env(&name).filter(|v| !v.is_empty()) {
            if let Some(t) = try_root(PathBuf::from(v), &name, &mut searched) {
                return Ok(t);
            }
        }
    }
    // 3 scan of the default install location
    for r in extra_roots {
        if let Some(t) = try_root(r.clone(), "scan", &mut searched) {
            return Ok(t);
        }
    }
    Err(format!(
        "NVRTC library not found; searched: [{}]. Install the CUDA toolkit or set RCFD_CUDA_BIN to the directory that holds {}",
        searched.join(", "),
        if cfg!(windows) { "nvrtc64_*.dll" } else { "libnvrtc.so" }
    ))
}

/// Locate the toolkit from the real process environment and the default install roots.
pub fn locate() -> Result<Toolkit, String> {
    let names: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
    locate_with(&|k| std::env::var(k).ok(), &names, &default_roots())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_toolkit_error_lists_searched_places() {
        let e = locate_with(
            &|k| (k == "CUDA_PATH").then(|| "Z:/does/not/exist".to_string()),
            &[],
            &[],
        )
        .unwrap_err();
        assert!(e.contains("NVRTC library not found"), "{e}");
        assert!(e.contains("does/not/exist"), "{e}");
        assert!(e.contains("RCFD_CUDA_BIN"), "{e}");
    }

    #[test]
    fn finds_library_in_bin_x64_of_a_fake_root() {
        let root = std::env::temp_dir().join(format!("rcfd-gpu-fake-toolkit-{}", std::process::id()));
        let dir = root.join("bin").join("x64");
        std::fs::create_dir_all(&dir).unwrap();
        let lib = if cfg!(windows) {
            "nvrtc64_130_0.dll"
        } else {
            "libnvrtc.so.13"
        };
        std::fs::write(dir.join(lib), b"x").unwrap();
        let r = root.to_str().unwrap().to_string();
        let t = locate_with(&|k| (k == "CUDA_PATH").then(|| r.clone()), &[], &[]).unwrap();
        assert_eq!(t.dll_dir, dir);
        assert_eq!(t.source, "CUDA_PATH");
        let _ = std::fs::remove_dir_all(&root);
    }
}
