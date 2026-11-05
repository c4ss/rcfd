//! ADR 0190: the helper that reads the expected-hash tables (`tests/support/expected_hashes.rs`), and every table of the workspace.

use std::path::{Path, PathBuf};

#[path = "support/expected_hashes.rs"]
mod expected_hashes;

use expected_hashes::{Platform, Table};

const GOOD: &str = "# provenance line\n\
\n\
shared\tany\t0x00000000000000ff\n\
split\twindows\t0x0000000000000001\n\
split\tlinux-x86_64\t0x0000000000000002\n\
only windows\twindows\t0x0000000000000003\n\
image\tany\t8902a5bd06d0a563a8c8046330ccd2ffa63b6b09d05ab503d9be96683c1ea415\n";

fn good() -> Table {
    Table::parse("t", GOOD).unwrap()
}

/// GOOD with `row` appended: the error of the parse, which must name the table.
fn error_with(row: &str) -> String {
    let e = Table::parse("t", &format!("{GOOD}{row}\n")).unwrap_err();
    assert!(e.starts_with("t line "), "{e}");
    e
}

#[test]
fn rows_are_selected_per_platform() {
    let t = good();
    let w = Some(Platform::Windows);
    let l = Some(Platform::LinuxX86_64);
    assert_eq!(t.lookup("shared", w).unwrap(), Some("0x00000000000000ff"));
    assert_eq!(t.lookup("shared", l).unwrap(), Some("0x00000000000000ff"));
    assert_eq!(t.lookup("shared", None).unwrap(), Some("0x00000000000000ff"));
    assert_eq!(t.lookup("split", w).unwrap(), Some("0x0000000000000001"));
    assert_eq!(t.lookup("split", l).unwrap(), Some("0x0000000000000002"));
    assert_eq!(t.lookup("split", None).unwrap(), None);
    assert_eq!(t.lookup("only windows", l).unwrap(), None);
    assert!(t.has_key("only windows") && !t.has_key("nope"));
}

#[test]
fn the_current_platform_is_the_cfg_split_the_tests_had() {
    let want = if cfg!(windows) {
        Some(Platform::Windows)
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some(Platform::LinuxX86_64)
    } else {
        None
    };
    assert_eq!(Platform::current(), want);
    let t = good();
    let split = match want {
        Some(Platform::Windows) => Some(1),
        Some(_) => Some(2),
        None => None,
    };
    assert_eq!(t.hash("split"), split);
    assert_eq!(t.hash("shared"), Some(0xff));
    assert_eq!(t.expect_hash("shared"), 0xff);
    assert_eq!(
        t.expect_sha256("image"),
        "8902a5bd06d0a563a8c8046330ccd2ffa63b6b09d05ab503d9be96683c1ea415"
    );
}

#[test]
fn unknown_key_is_an_error_naming_it() {
    let t = good();
    let e = t.lookup("spilt", Some(Platform::Windows)).unwrap_err();
    assert!(e.contains("unknown key \"spilt\"") && e.contains("table t"), "{e}");
}

#[test]
#[should_panic(expected = "unknown key \"spilt\"")]
fn hash_panics_on_an_unknown_key() {
    good().hash("spilt");
}

#[test]
#[should_panic(expected = "unknown key \"spilt\"")]
fn expect_hash_panics_on_an_unknown_key() {
    good().expect_hash("spilt");
}

/// `expect_hash` refuses a key with no row here: the only row names a platform other than this one.
#[test]
#[should_panic(expected = "has no row for this platform")]
fn expect_hash_panics_without_a_row_for_this_platform() {
    let other = if cfg!(windows) { "linux-x86_64" } else { "windows" };
    Table::parse("t", &format!("k\t{other}\t0x0000000000000001\n"))
        .unwrap()
        .expect_hash("k");
}

#[test]
#[should_panic(expected = "is not a 64-bit hash")]
fn a_sha256_is_not_a_64_bit_hash() {
    good().hash("image");
}

#[test]
#[should_panic(expected = "is not a SHA-256")]
fn a_64_bit_hash_is_not_a_sha256() {
    good().expect_sha256("shared");
}

#[test]
#[should_panic(expected = "duplicate row")]
fn load_panics_on_a_duplicate_row() {
    Table::load("t", "k\tany\t0x0000000000000001\nk\tany\t0x0000000000000001\n");
}

/// A duplicate fails however its value compares. Negative control: GOOD itself parses and the same key on another platform is not
/// a duplicate, so the refusals below come from the row added, not from the rest of the table.
#[test]
fn duplicate_rows_fail_negative_control_the_same_table_without_them_parses() {
    assert!(Table::parse("t", GOOD).is_ok());
    assert!(Table::parse("t", &format!("{GOOD}only windows\tlinux-x86_64\t0x0000000000000004\n")).is_ok());
    for row in [
        "shared\tany\t0x00000000000000ff",
        "shared\tany\t0x00000000000000fe",
        "split\twindows\t0x0000000000000001",
        "split\tlinux-x86_64\t0x0000000000000009",
    ] {
        let e = error_with(row);
        assert!(e.contains("duplicate row"), "{row}: {e}");
    }
}

#[test]
fn any_beside_a_named_platform_fails() {
    for row in ["shared\twindows\t0x0000000000000001", "split\tany\t0x0000000000000001"] {
        let e = error_with(row);
        assert!(e.contains("`any` row beside"), "{row}: {e}");
    }
}

#[test]
fn malformed_rows_fail() {
    for (row, want) in [
        ("two columns\tany", "2 columns"),
        ("four\tany\t0x0000000000000001\textra", "4 columns"),
        ("k\tmacos\t0x0000000000000001", "unknown platform"),
        ("k\tWindows\t0x0000000000000001", "unknown platform"),
        ("k\tany\t0x1", "bad hash"),
        ("k\tany\t0x00000000000000FF", "bad hash"),
        ("k\tany\t0x00000000000000fg", "bad hash"),
        ("k\tany\t0000000000000001", "bad hash"),
        ("k\tany\t18446744073709551615", "bad hash"),
        ("k\tany\t", "bad hash"),
        ("\tany\t0x0000000000000001", "empty key"),
        (" k\tany\t0x0000000000000001", "outer spaces"),
    ] {
        let e = error_with(row);
        assert!(e.contains(want), "{row}: {e}");
    }
}

#[test]
fn line_endings_do_not_change_the_rows() {
    let crlf = GOOD.replace('\n', "\r\n");
    let t = Table::parse("t", &crlf).unwrap();
    assert_eq!(t.lookup("shared", None).unwrap(), Some("0x00000000000000ff"));
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Every table of the workspace parses, is named after a test file of its crate that reads it, and is a `.tsv`, which the fixture
/// walkers skip (`case_hashes_w48u` walks `tests/fixtures` of each crate for `.toml` and `.json`).
#[test]
fn every_table_of_the_workspace_parses_and_belongs_to_a_test_file() {
    let mut crates: Vec<PathBuf> = std::fs::read_dir(repo().join("crates"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    crates.sort();
    let mut tables = 0;
    for c in crates {
        let Ok(rd) = std::fs::read_dir(c.join("tests/fixtures/expected_hashes")) else {
            continue;
        };
        for e in rd {
            let p = e.unwrap().path();
            assert_eq!(p.extension().and_then(|x| x.to_str()), Some("tsv"), "{}", p.display());
            let stem = p.file_stem().unwrap().to_str().unwrap().to_string();
            let test = c.join("tests").join(format!("{stem}.rs"));
            assert!(test.is_file(), "{}: no test file {}", p.display(), test.display());
            let source = std::fs::read_to_string(&test).unwrap();
            assert!(
                source.contains(&format!("fixtures/expected_hashes/{stem}.tsv")),
                "{} does not read {}",
                test.display(),
                p.display()
            );
            let t = Table::parse(&stem, &std::fs::read_to_string(&p).unwrap()).unwrap_or_else(|e| panic!("{e}"));
            assert!(t.has_any_row(), "{}: no rows", p.display());
            tables += 1;
        }
    }
    println!("{tables} expected-hash tables parsed");
    assert!(tables > 0, "no table found: the walk is looking in the wrong place");
}
