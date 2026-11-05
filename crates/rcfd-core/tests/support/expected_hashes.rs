//! The registered expected hashes of a test file, read from its table (ADR 0190). A test includes this file with `#[path]` (no
//! crate depends on another for it) and the table with `include_str!`:
//!
//! ```text
//! #[path = "../../rcfd-core/tests/support/expected_hashes.rs"]
//! mod expected_hashes;
//!
//! fn hash_table() -> expected_hashes::Table {
//!     expected_hashes::Table::load("my_test", include_str!("fixtures/expected_hashes/my_test.tsv"))
//! }
//! ```
//!
//! A table is UTF-8 text, one row per line, three tab-separated columns: key, platform, hash. Lines that start with `#` and
//! blank lines are ignored; the `#` lines carry the provenance of the values. Platform is `any`, `windows` or `linux-x86_64`. A
//! hash is `0x` and 16 lowercase hex digits (a 64-bit content or bit hash) or 64 lowercase hex digits (a SHA-256). A key has
//! either one `any` row or at most one row per named platform: `any` beside a named platform is refused, and so is a duplicate.
//! Everything here fails loudly: a test can never pass because a row was skipped or a key was misspelt.

#![allow(dead_code)] // each test file uses part of it

/// The platform column of a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Any,
    Windows,
    LinuxX86_64,
}

impl Platform {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "any" => Some(Self::Any),
            "windows" => Some(Self::Windows),
            "linux-x86_64" => Some(Self::LinuxX86_64),
            _ => None,
        }
    }

    /// The named platform this test runs on; `None` on any other (only `any` rows apply there).
    pub fn current() -> Option<Self> {
        if cfg!(windows) {
            Some(Self::Windows)
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Self::LinuxX86_64)
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct Row {
    key: String,
    platform: Platform,
    hash: String,
}

/// The rows of one table.
#[derive(Debug)]
pub struct Table {
    name: String,
    rows: Vec<Row>,
}

fn is_lower_hex(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn valid_hash(h: &str) -> bool {
    let bits64 = h.strip_prefix("0x").is_some_and(|d| d.len() == 16 && is_lower_hex(d));
    bits64 || (h.len() == 64 && is_lower_hex(h))
}

impl Table {
    /// Parse a table's text; `name` names it in messages. Refuses a malformed line and a duplicate or conflicting row.
    pub fn parse(name: &str, text: &str) -> Result<Self, String> {
        let mut rows: Vec<Row> = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let n = i + 1;
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            let [key, platform, hash] = cols[..] else {
                return Err(format!(
                    "{name} line {n}: {} columns, want key, platform, hash: {line:?}",
                    cols.len()
                ));
            };
            if key.is_empty() || key != key.trim() {
                return Err(format!("{name} line {n}: empty key or key with outer spaces: {key:?}"));
            }
            let platform = Platform::parse(platform)
                .ok_or_else(|| format!("{name} line {n}: unknown platform {platform:?} of key {key:?}"))?;
            if !valid_hash(hash) {
                return Err(format!("{name} line {n}: bad hash {hash:?} of key {key:?}"));
            }
            for r in rows.iter().filter(|r| r.key == key) {
                if r.platform == platform {
                    return Err(format!(
                        "{name} line {n}: duplicate row for key {key:?}, platform {platform:?}"
                    ));
                }
                if r.platform == Platform::Any || platform == Platform::Any {
                    return Err(format!(
                        "{name} line {n}: key {key:?} has an `any` row beside a platform row"
                    ));
                }
            }
            rows.push(Row {
                key: key.to_string(),
                platform,
                hash: hash.to_string(),
            });
        }
        Ok(Self {
            name: name.to_string(),
            rows,
        })
    }

    /// `parse`, panicking on a bad table.
    pub fn load(name: &str, text: &str) -> Self {
        Self::parse(name, text).unwrap_or_else(|e| panic!("expected-hash table: {e}"))
    }

    /// True if the table holds at least one row.
    pub fn has_any_row(&self) -> bool {
        !self.rows.is_empty()
    }

    /// True if the table has any row for `key`, whatever its platform.
    pub fn has_key(&self, key: &str) -> bool {
        self.rows.iter().any(|r| r.key == key)
    }

    /// The hash text of `key` on `platform` (`None`: a platform the table does not name). `Err`: the key is unknown. `Ok(None)`: the
    /// key has no row that applies there.
    pub fn lookup(&self, key: &str, platform: Option<Platform>) -> Result<Option<&str>, String> {
        if !self.has_key(key) {
            return Err(format!("expected-hash table {}: unknown key {key:?}", self.name));
        }
        Ok(self
            .rows
            .iter()
            .find(|r| r.key == key && (r.platform == Platform::Any || Some(r.platform) == platform))
            .map(|r| r.hash.as_str()))
    }

    fn on_this_platform(&self, key: &str) -> Option<&str> {
        self.lookup(key, Platform::current()).unwrap_or_else(|e| panic!("{e}"))
    }

    fn as_u64(&self, key: &str, hash: &str) -> u64 {
        hash.strip_prefix("0x")
            .and_then(|d| u64::from_str_radix(d, 16).ok())
            .unwrap_or_else(|| {
                panic!(
                    "expected-hash table {}: key {key:?}: {hash:?} is not a 64-bit hash",
                    self.name
                )
            })
    }

    /// The 64-bit hash of `key` on this platform; `None` where the key has no row for this platform (the test prints and skips).
    /// Panics on an unknown key.
    pub fn hash(&self, key: &str) -> Option<u64> {
        self.on_this_platform(key).map(|h| self.as_u64(key, h))
    }

    /// The 64-bit hash of `key`, which must have a row for this platform. Panics otherwise.
    pub fn expect_hash(&self, key: &str) -> u64 {
        self.hash(key).unwrap_or_else(|| self.no_row(key))
    }

    /// The SHA-256 (64 hex digits) of `key`, which must have a row for this platform. Panics otherwise.
    pub fn expect_sha256(&self, key: &str) -> &str {
        let h = self.on_this_platform(key).unwrap_or_else(|| self.no_row(key));
        if h.starts_with("0x") {
            panic!("expected-hash table {}: key {key:?}: {h:?} is not a SHA-256", self.name);
        }
        h
    }

    fn no_row(&self, key: &str) -> ! {
        panic!(
            "expected-hash table {}: key {key:?} has no row for this platform ({:?})",
            self.name,
            Platform::current()
        )
    }
}
