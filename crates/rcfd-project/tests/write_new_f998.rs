//! ADR 0834 (F-998): `atomic::write_new` as a plain write. W1: readers of a file being created never see a partial
//! file (Windows); N3: its negative control, a plain write without share mode 0, which they do see partially; W2: the
//! bytes written equal, no temp file left; W3: an existing file is refused and left unchanged.

use rcfd_project::{atomic, ProjectError};
use std::path::{Path, PathBuf};

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-f998-write-new-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 % 251 + 1) as u8).collect()
}

#[cfg(windows)]
mod readers {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Registered: 20 files of 8 MiB, 3 readers.
    const FILES: usize = 20;
    const LEN: usize = 8 << 20;
    const READERS: usize = 3;

    #[derive(Debug, Default)]
    #[allow(dead_code)]
    pub struct Counts {
        pub full: u64,
        pub partial: u64,
        pub sharing: u64,
        pub absent: u64,
        pub other: u64,
    }

    /// The readers `fs::read` the file being created while `write` makes `FILES` new files one after another.
    pub fn hammer(tag: &str, write: fn(&Path, &[u8]) -> std::io::Result<()>) -> Counts {
        let d = dir(tag);
        let bytes = Arc::new(pattern(LEN));
        let cur = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let c: Arc<[AtomicU64; 5]> = Arc::new(Default::default());
        let readers: Vec<_> = (0..READERS)
            .map(|_| {
                let (d, bytes, cur, stop, c) = (d.clone(), bytes.clone(), cur.clone(), stop.clone(), c.clone());
                std::thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        let p = d.join(format!("f-{}.bin", cur.load(Ordering::Relaxed)));
                        let k = match std::fs::read(&p) {
                            Ok(b) if b == *bytes => 0,
                            Ok(_) => 1,
                            Err(e) if e.raw_os_error() == Some(32) => 2,
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 3,
                            Err(_) => 4,
                        };
                        c[k].fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect();
        for k in 0..FILES {
            cur.store(k, Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_millis(2));
            write(&d.join(format!("f-{k}.bin")), &bytes).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        stop.store(true, Ordering::Relaxed);
        for r in readers {
            r.join().unwrap();
        }
        let _ = std::fs::remove_dir_all(&d);
        let l = |i: usize| c[i].load(Ordering::Relaxed);
        let counts = Counts {
            full: l(0),
            partial: l(1),
            sharing: l(2),
            absent: l(3),
            other: l(4),
        };
        eprintln!("{tag}: {counts:?}");
        counts
    }
}

/// ADR 0834 W1: no partial read of a file `write_new` is creating; the readers met the write (≥ 5 sharing violations).
#[cfg(windows)]
#[test]
fn w1_readers_never_see_a_partial_new_file() {
    let c = readers::hammer("w1", |p, b| atomic::write_new(p, b).map_err(std::io::Error::other));
    assert_eq!(c.partial, 0, "W1: partial reads: {c:?}");
    assert!(c.sharing >= 5, "W1: the readers did not meet the write: {c:?}");
}

/// ADR 0834 N3: a plain write without share mode 0 is read partially.
#[cfg(windows)]
#[test]
fn n3_negative_control_a_shared_plain_write_is_read_partially() {
    fn naive(p: &Path, b: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(p)?;
        f.write_all(b)?;
        f.sync_all()
    }
    let c = readers::hammer("n3", naive);
    assert!(c.partial >= 1, "N3: no partial read: {c:?}");
}

/// ADR 0834 W2: the bytes written equal; no temp file left.
#[test]
fn w2_the_bytes_written_equal_and_no_temp_file_is_left() {
    let d = dir("w2");
    for n in [0, 1, 4095, (1 << 20) + 3] {
        let p = d.join(format!("n-{n}.bin"));
        let b = pattern(n);
        atomic::write_new(&p, &b).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b, "W2: bytes of {n}");
    }
    let names: Vec<String> = std::fs::read_dir(&d)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !names.iter().any(|n| atomic::is_temp_name(n)),
        "W2: temp files: {names:?}"
    );
    assert_eq!(names.len(), 4, "W2: files: {names:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// ADR 0834 W3: an existing file is refused with `Exists` and left unchanged.
#[test]
fn w3_an_existing_file_is_refused_and_unchanged() {
    let d = dir("w3");
    let p = d.join("case.toml");
    std::fs::write(&p, b"old bytes\n").unwrap();
    let e = atomic::write_new(&p, &pattern(5000)).unwrap_err();
    assert!(matches!(e, ProjectError::Exists { .. }), "W3: {e}");
    assert_eq!(
        std::fs::read(&p).unwrap(),
        b"old bytes\n",
        "W3: the existing file changed"
    );
    let _ = std::fs::remove_dir_all(&d);
}
