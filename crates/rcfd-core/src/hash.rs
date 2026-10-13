//! Small deterministic, non-cryptographic hashing (FNV-1a, 64 bit) used for
//! content fingerprints of settings trees and meshes.

/// FNV-1a 64-bit offset basis.
pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Incremental FNV-1a hasher.
#[derive(Debug, Clone, Copy)]
pub struct Fnv64(u64);

impl Default for Fnv64 {
    fn default() -> Self {
        Fnv64(FNV_OFFSET)
    }
}

impl Fnv64 {
    /// New hasher.
    pub fn new() -> Self {
        Self::default()
    }
    /// Feed bytes.
    pub fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(FNV_PRIME);
        }
    }
    /// Feed a u64 in little-endian order.
    pub fn write_u64(&mut self, v: u64) {
        self.write(&v.to_le_bytes());
    }
    /// Feed an f64 by bit pattern.
    pub fn write_f64(&mut self, v: f64) {
        self.write_u64(v.to_bits());
    }
    /// Final hash value.
    pub fn finish(&self) -> u64 {
        self.0
    }
}

/// One-shot hash of a byte slice.
pub fn fnv64(bytes: &[u8]) -> u64 {
    let mut h = Fnv64::new();
    h.write(bytes);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_vectors() {
        assert_eq!(fnv64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv64(b"foobar"), 0x8594_4171_f739_67e8);
    }
}
