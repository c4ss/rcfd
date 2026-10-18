//! Design-of-experiments samplers on the unit cube (ADR 0067): full factorial, Latin hypercube (McKay, Beckman and Conover
//! 1979) with a seeded generator, and the unscrambled Sobol sequence (Sobol 1967; Gray-code construction of Antonov and
//! Saleev 1979) with direction numbers of Joe and Kuo (2008) for dimensions 1 to 10.

use super::rng::Rng;

/// Largest dimension of [`sobol`].
pub const SOBOL_MAX_DIM: usize = 10;

/// Full factorial design: `levels[k]` equally spaced levels in dimension k, endpoints included (a single level is 0.5).
/// Row-major: the first dimension varies slowest.
pub fn full_factorial(levels: &[usize]) -> Vec<Vec<f64>> {
    if levels.is_empty() || levels.contains(&0) {
        return Vec::new();
    }
    let total: usize = levels.iter().product();
    let coord = |l: usize, n: usize| -> f64 {
        if n == 1 {
            0.5
        } else if l + 1 == n {
            1.0
        } else {
            l as f64 / (n - 1) as f64
        }
    };
    (0..total)
        .map(|i| {
            let mut rest = i;
            let mut idx = vec![0usize; levels.len()];
            for k in (0..levels.len()).rev() {
                idx[k] = rest % levels[k];
                rest /= levels[k];
            }
            idx.iter().zip(levels).map(|(&l, &n)| coord(l, n)).collect()
        })
        .collect()
}

/// Latin hypercube sample of `n` points in `d` dimensions: in every dimension each stratum `[k/n, (k+1)/n)` holds exactly
/// one point, positioned uniformly inside it.
pub fn latin_hypercube(n: usize, d: usize, seed: u64) -> Vec<Vec<f64>> {
    let mut rng = Rng::new(seed);
    let mut pts = vec![vec![0.0; d]; n];
    for k in 0..d {
        let mut perm: Vec<usize> = (0..n).collect();
        rng.shuffle(&mut perm);
        for (i, p) in pts.iter_mut().enumerate() {
            let u = rng.uniform();
            let x = (perm[i] as f64 + u) / n as f64;
            // guard the open upper end against rounding
            p[k] = if x >= 1.0 {
                f64::from_bits(1.0f64.to_bits() - 1)
            } else {
                x
            };
        }
    }
    pts
}

/// (degree s, coefficient integer a, initial direction numbers m) of dimensions 2 to 10 (dimension 1 is the van der Corput
/// sequence, all m = 1).
const JOE_KUO: [(u32, u32, [u32; 5]); 9] = [
    (1, 0, [1, 0, 0, 0, 0]),
    (2, 1, [1, 3, 0, 0, 0]),
    (3, 1, [1, 3, 1, 0, 0]),
    (3, 2, [1, 1, 1, 0, 0]),
    (4, 1, [1, 1, 3, 3, 0]),
    (4, 4, [1, 3, 5, 13, 0]),
    (5, 2, [1, 1, 5, 5, 17]),
    (5, 4, [1, 1, 5, 5, 5]),
    (5, 7, [1, 1, 7, 11, 19]),
];

/// Direction numbers `V[1..=32]` of dimension `dim` (0-based) as 32-bit fixed-point fractions.
fn direction_numbers(dim: usize) -> [u32; 33] {
    let mut m = [0u32; 33];
    if dim == 0 {
        for mi in m.iter_mut().skip(1) {
            *mi = 1;
        }
    } else {
        let (s, a, init) = JOE_KUO[dim - 1];
        let s = s as usize;
        for i in 1..=32usize {
            if i <= s {
                m[i] = init[i - 1];
            } else {
                let mut v = m[i - s] ^ (m[i - s] << s);
                for k in 1..s {
                    if (a >> (s - 1 - k)) & 1 == 1 {
                        v ^= m[i - k] << k;
                    }
                }
                m[i] = v;
            }
        }
    }
    let mut v = [0u32; 33];
    for i in 1..=32usize {
        v[i] = m[i] << (32 - i);
    }
    v
}

/// The first `n` points of the unscrambled Sobol sequence in `d` dimensions (the first point is the origin).
pub fn sobol(n: usize, d: usize) -> Result<Vec<Vec<f64>>, String> {
    if d == 0 || d > SOBOL_MAX_DIM {
        return Err(format!(
            "the Sobol sampler supports 1 to {SOBOL_MAX_DIM} dimensions (got {d}); use latin_hypercube for more"
        ));
    }
    if n as u64 > 1u64 << 32 {
        return Err("the Sobol sampler is limited to 2^32 points".into());
    }
    let dirs: Vec<[u32; 33]> = (0..d).map(direction_numbers).collect();
    let mut x = vec![0u32; d];
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if i > 0 {
            // index of the lowest zero bit of i - 1
            let c = (!(i as u64 - 1)).trailing_zeros() as usize;
            for k in 0..d {
                x[k] ^= dirs[k][c + 1];
            }
        }
        out.push(x.iter().map(|&v| v as f64 / 4_294_967_296.0).collect());
    }
    Ok(out)
}

/// Scale unit-cube points to the box `bounds` (lo, hi).
pub fn scale(points: &[Vec<f64>], bounds: &[(f64, f64)]) -> Vec<Vec<f64>> {
    points
        .iter()
        .map(|p| p.iter().zip(bounds).map(|(&u, &(lo, hi))| lo + (hi - lo) * u).collect())
        .collect()
}

/// L2-star discrepancy of a point set in the unit cube (Warnock's formula).
pub fn l2_star_discrepancy(points: &[Vec<f64>]) -> f64 {
    let n = points.len() as f64;
    let d = points.first().map_or(0, Vec::len);
    let t1 = 3f64.powi(-(d as i32));
    let t2: f64 = points
        .iter()
        .map(|p| p.iter().map(|x| 1.0 - x * x).product::<f64>())
        .sum::<f64>()
        * 2f64.powi(1 - d as i32)
        / n;
    let mut t3 = 0.0;
    for a in points {
        for b in points {
            t3 += a.iter().zip(b).map(|(x, y)| 1.0 - x.max(*y)).product::<f64>();
        }
    }
    (t1 - t2 + t3 / (n * n)).max(0.0).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d1_factorial_count_endpoints_and_order() {
        let p = full_factorial(&[3, 2, 4]);
        assert_eq!(p.len(), 24);
        assert_eq!(p[0], vec![0.0, 0.0, 0.0]);
        assert_eq!(p[1], vec![0.0, 0.0, 1.0 / 3.0]);
        assert_eq!(p[3], vec![0.0, 0.0, 1.0]);
        assert_eq!(p[4], vec![0.0, 1.0, 0.0]);
        assert_eq!(p[23], vec![1.0, 1.0, 1.0]);
        assert_eq!(full_factorial(&[1, 2])[1], vec![0.5, 1.0]);
        assert!(full_factorial(&[3, 0]).is_empty());
    }

    #[test]
    fn d2_latin_hypercube_hits_every_stratum_once() {
        for seed in 1..=10u64 {
            let (n, d) = (20, 5);
            let p = latin_hypercube(n, d, seed);
            for k in 0..d {
                let mut hit = vec![0usize; n];
                for q in &p {
                    assert!((0.0..1.0).contains(&q[k]));
                    hit[(q[k] * n as f64).floor() as usize] += 1;
                }
                assert!(hit.iter().all(|&h| h == 1), "seed {seed} dim {k}: {hit:?}");
            }
            let again = latin_hypercube(n, d, seed);
            assert_eq!(p, again);
        }
        assert_ne!(latin_hypercube(20, 5, 1), latin_hypercube(20, 5, 2));
    }

    #[test]
    fn d3_sobol_known_points_and_nets() {
        let p = sobol(8, 2).unwrap();
        let d1: Vec<f64> = p.iter().map(|x| x[0]).collect();
        let d2: Vec<f64> = p.iter().map(|x| x[1]).collect();
        assert_eq!(d1, vec![0.0, 0.5, 0.75, 0.25, 0.375, 0.875, 0.625, 0.125]);
        assert_eq!(d2, vec![0.0, 0.5, 0.25, 0.75, 0.375, 0.875, 0.125, 0.625]);
        for m in 3..=8u32 {
            let n = 1usize << m;
            let p = sobol(n, SOBOL_MAX_DIM).unwrap();
            for k in 0..SOBOL_MAX_DIM {
                let mut hit = vec![0usize; n];
                for q in &p {
                    hit[(q[k] * n as f64).floor() as usize] += 1;
                }
                assert!(hit.iter().all(|&h| h == 1), "m {m} dim {k}");
            }
            // (0, m, 2)-net in dimensions 1 and 2
            for a in 0..=m {
                let b = m - a;
                let (na, nb) = (1usize << a, 1usize << b);
                let mut hit = vec![0usize; na * nb];
                for q in &p {
                    let i = (q[0] * na as f64).floor() as usize;
                    let j = (q[1] * nb as f64).floor() as usize;
                    hit[i * nb + j] += 1;
                }
                assert!(hit.iter().all(|&h| h == 1), "m {m} a {a}");
            }
        }
        assert!(sobol(4, 11).is_err());
    }

    #[test]
    fn d3_sobol_discrepancy_beats_random() {
        let s = l2_star_discrepancy(&sobol(256, 4).unwrap());
        let mean: f64 = (1..=20u64)
            .map(|seed| {
                let mut r = Rng::new(seed);
                let pts: Vec<Vec<f64>> = (0..256).map(|_| (0..4).map(|_| r.uniform()).collect()).collect();
                l2_star_discrepancy(&pts)
            })
            .sum::<f64>()
            / 20.0;
        eprintln!("L2-star discrepancy: sobol {s:.5}, random mean {mean:.5}");
        assert!(s <= 0.5 * mean, "{s} vs {mean}");
    }

    #[test]
    fn scale_maps_to_bounds() {
        let p = scale(&[vec![0.0, 1.0], vec![0.5, 0.5]], &[(-1.0, 1.0), (10.0, 20.0)]);
        assert_eq!(p, vec![vec![-1.0, 20.0], vec![0.0, 15.0]]);
    }
}
