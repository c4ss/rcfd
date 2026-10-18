//! Small seeded random number generator (xoshiro256** seeded through SplitMix64), so that every study is reproducible
//! bit for bit on every platform (ADR 0067). Not cryptographic.

/// xoshiro256** generator.
#[derive(Debug, Clone)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    /// Generator for `seed`.
    pub fn new(seed: u64) -> Self {
        let mut st = seed;
        let s = [
            splitmix64(&mut st),
            splitmix64(&mut st),
            splitmix64(&mut st),
            splitmix64(&mut st),
        ];
        Rng { s }
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let r = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        r
    }

    /// Uniform in [0, 1) with 53 random bits.
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.uniform()
    }

    /// Uniform integer in [0, n) (n > 0; negligible modulo bias for n far below 2^64).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream_and_uniform_in_range() {
        let (mut a, mut b) = (Rng::new(42), Rng::new(42));
        let mut c = Rng::new(43);
        let mut differ = false;
        for _ in 0..1000 {
            let (x, y) = (a.uniform(), b.uniform());
            assert_eq!(x.to_bits(), y.to_bits());
            assert!((0.0..1.0).contains(&x));
            differ |= c.uniform().to_bits() != x.to_bits();
        }
        assert!(differ);
        let mean: f64 = (0..20000).map(|_| a.uniform()).sum::<f64>() / 20000.0;
        assert!((mean - 0.5).abs() < 0.01, "{mean}");
    }
}
