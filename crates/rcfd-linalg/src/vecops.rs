//! Deterministic parallel vector kernels.

use rayon::prelude::*;

/// Fixed chunk length for reductions and parallel loops.
pub const CHUNK: usize = 4096;

/// Dot product with a thread-count independent summation order.
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    debug_assert_eq!(a.len(), b.len());
    let partial: Vec<f64> = a
        .par_chunks(CHUNK)
        .zip(b.par_chunks(CHUNK))
        .map(|(x, y)| x.iter().zip(y).map(|(p, q)| p * q).sum::<f64>())
        .collect();
    partial.iter().sum()
}

/// Euclidean norm.
pub fn norm2(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// Sum of absolute values.
pub fn norm1(a: &[f64]) -> f64 {
    let partial: Vec<f64> = a
        .par_chunks(CHUNK)
        .map(|x| x.iter().map(|v| v.abs()).sum::<f64>())
        .collect();
    partial.iter().sum()
}

/// Sum of all entries.
pub fn sum(a: &[f64]) -> f64 {
    let partial: Vec<f64> = a.par_chunks(CHUNK).map(|x| x.iter().sum::<f64>()).collect();
    partial.iter().sum()
}

/// `y += alpha * x`
pub fn axpy(alpha: f64, x: &[f64], y: &mut [f64]) {
    y.par_chunks_mut(CHUNK).zip(x.par_chunks(CHUNK)).for_each(|(yc, xc)| {
        for (yi, xi) in yc.iter_mut().zip(xc) {
            *yi += alpha * xi;
        }
    });
}

/// `y = x + beta * y`
pub fn xpby(x: &[f64], beta: f64, y: &mut [f64]) {
    y.par_chunks_mut(CHUNK).zip(x.par_chunks(CHUNK)).for_each(|(yc, xc)| {
        for (yi, xi) in yc.iter_mut().zip(xc) {
            *yi = xi + beta * *yi;
        }
    });
}

/// `y *= alpha`
pub fn scale(alpha: f64, y: &mut [f64]) {
    y.par_chunks_mut(CHUNK)
        .for_each(|c| c.iter_mut().for_each(|v| *v *= alpha));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reductions_are_bit_identical_across_thread_counts() {
        let n = 100_003;
        let a: Vec<f64> = (0..n)
            .map(|i| ((i * 7919) % 1013) as f64 * 1e-3 + 1e-9 * i as f64)
            .collect();
        let b: Vec<f64> = (0..n as u64)
            .map(|i| ((i * 104_729) % 997) as f64 * 1e-2 - 3.0)
            .collect();
        let reference = dot(&a, &b);
        for threads in [1usize, 2, 3, 8] {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
            let d = pool.install(|| dot(&a, &b));
            assert_eq!(d.to_bits(), reference.to_bits(), "{threads} threads");
            let s = pool.install(|| sum(&a));
            assert_eq!(s.to_bits(), sum(&a).to_bits());
        }
    }

    #[test]
    fn basic_kernels() {
        let x = vec![1.0, 2.0, 3.0];
        let mut y = vec![1.0, 1.0, 1.0];
        axpy(2.0, &x, &mut y);
        assert_eq!(y, vec![3.0, 5.0, 7.0]);
        xpby(&x, 0.5, &mut y);
        assert_eq!(y, vec![2.5, 4.5, 6.5]);
        scale(2.0, &mut y);
        assert_eq!(y, vec![5.0, 9.0, 13.0]);
        assert_eq!(dot(&x, &x), 14.0);
        assert!((norm2(&x) - 14f64.sqrt()).abs() < 1e-15);
        assert_eq!(norm1(&[-1.0, 2.0]), 3.0);
    }
}
