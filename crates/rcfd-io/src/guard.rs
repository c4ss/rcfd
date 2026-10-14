//! Limits and small helpers shared by the interchange-format parsers (ADR 0011, rules H1-H5).
//!
//! Every parser checks sizes read from a header against the bytes that remain, with checked
//! arithmetic, before reserving memory; hard caps bound what a single file may declare.

use rcfd_core::{Error, Result};

/// Largest file a parser accepts (bytes).
pub const MAX_FILE_BYTES: usize = 1 << 30;
/// Largest number of triangles, nodes, elements or grid points a file may declare.
pub const MAX_ITEMS: usize = 50_000_000;
/// Largest number of blocks / zones in one file.
pub const MAX_BLOCKS: usize = 4096;

/// Reject a file that is larger than [`MAX_FILE_BYTES`].
pub fn check_file_size(len: usize, what: &str) -> Result<()> {
    if len > MAX_FILE_BYTES {
        return Err(Error::Io(format!(
            "{what}: file of {len} bytes exceeds the {MAX_FILE_BYTES} byte limit"
        )));
    }
    Ok(())
}

/// `a * b` as `usize`, or an error naming the quantity (no wrap-around from hostile counts).
pub fn mul(a: usize, b: usize, what: &str) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| Error::Io(format!("{what}: size {a} x {b} overflows")))
}

/// Check a declared item count against the cap and the bytes that remain (`min_bytes` per item).
pub fn check_count(count: usize, min_bytes: usize, remaining: usize, what: &str) -> Result<()> {
    if count > MAX_ITEMS {
        return Err(Error::Io(format!(
            "{what}: {count} items exceeds the limit of {MAX_ITEMS}"
        )));
    }
    let need = mul(count, min_bytes, what)?;
    if need > remaining {
        return Err(Error::Io(format!(
            "{what}: header declares {count} items needing at least {need} bytes but only {remaining} remain"
        )));
    }
    Ok(())
}

/// Finite check with a location text.
pub fn finite(v: f64, loc: impl FnOnce() -> String) -> Result<f64> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(Error::Io(format!("{}: non-finite number", loc())))
    }
}

/// Little deterministic xorshift64 used by the fuzz-style tests.
pub struct XorShift(pub u64);

impl XorShift {
    /// Next pseudo-random value.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Run `parse` on every prefix of `valid` and on `n` deterministic mutations of it (ADR 0011, H1).
/// The parse result is ignored; the point is that nothing panics.
pub fn fuzz_no_panic(valid: &[u8], n: usize, mut parse: impl FnMut(&[u8])) {
    for len in 0..=valid.len() {
        parse(&valid[..len]);
    }
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    for _ in 0..n {
        let mut b = valid.to_vec();
        if b.is_empty() {
            break;
        }
        let bursts = 1 + (rng.next_u64() % 4) as usize;
        for _ in 0..bursts {
            let pos = (rng.next_u64() as usize) % b.len();
            match rng.next_u64() % 3 {
                0 => b[pos] = rng.next_u64() as u8,
                1 => b[pos] ^= 1 << (rng.next_u64() % 8),
                _ => {
                    let cut = (pos + 1 + (rng.next_u64() as usize % 8)).min(b.len());
                    b.drain(pos..cut);
                    if b.is_empty() {
                        break;
                    }
                }
            }
        }
        parse(&b);
    }
}
