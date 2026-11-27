//! ADR 0830 (F-978): the replace of a cooperative edit against a file held open by another process (Windows). U1: a
//! reader that shares delete (std's `File::open`, every rcfd reader) does not refuse the replace, even with one attempt.
//! U0 and U2 of ADR 0830 belong to its fix F1, which was not made (the arms did not find the cause).

#![cfg(windows)]

mod common;

use rcfd_project::atomic::{self, Retry};
use std::path::PathBuf;

const REL: &str = "cases/new.toml";

fn made(tag: &str) -> (PathBuf, PathBuf) {
    let root = common::scratch(tag);
    std::fs::create_dir_all(root.join("cases")).unwrap();
    let path = root.join(REL);
    atomic::write_new(&path, b"old").unwrap();
    (root, path)
}

/// ADR 0830 U1.
#[test]
fn u1_a_reader_that_shares_delete_does_not_refuse_the_replace() {
    let (_root, path) = made("f978-u1");
    let reader = std::fs::File::open(&path).unwrap();
    let one = Retry {
        attempts: 1,
        ..atomic::DEFAULT_RETRY
    };
    let r = atomic::atomic_write_with(&path, b"new", &one);
    drop(reader);
    eprintln!("U1: {r:?}");
    assert!(r.is_ok(), "U1: {r:?}");
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
}
