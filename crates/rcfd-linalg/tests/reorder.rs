//! F-236 registered checks R1 and R2 (ADR 0020, part A).

use rcfd_linalg::reorder::{bandwidth, inverse_permutation, rcm_order};

/// Fixed linear congruential shuffle of `0..n` (Fisher-Yates).
fn shuffled(n: usize) -> Vec<usize> {
    let mut p: Vec<usize> = (0..n).collect();
    let mut s: u64 = 0x2545F4914F6CDD1D;
    for i in (1..n).rev() {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = ((s >> 33) as usize) % (i + 1);
        p.swap(i, j);
    }
    p
}

fn is_permutation(o: &[usize], n: usize) -> bool {
    let mut seen = vec![false; n];
    o.len() == n
        && o.iter().all(|&i| {
            let ok = i < n && !seen[i];
            if ok {
                seen[i] = true;
            }
            ok
        })
}

#[test]
fn r1_order_is_a_permutation_and_deterministic() {
    // path
    let (l, u): (Vec<usize>, Vec<usize>) = (0..49).map(|i| (i, i + 1)).unzip();
    let o = rcm_order(50, &l, &u);
    assert!(is_permutation(&o, 50));
    assert_eq!(o, rcm_order(50, &l, &u));
    // two paths (0..4, 5..9) and an isolated node 10
    let (l, u): (Vec<usize>, Vec<usize>) = (0..4).map(|i| (i, i + 1)).chain((5..9).map(|i| (i, i + 1))).unzip();
    let o = rcm_order(11, &l, &u);
    assert!(is_permutation(&o, 11));
    assert_eq!(o, rcm_order(11, &l, &u));
    // 2D grid 7 x 5
    let (nx, ny) = (7, 5);
    let (mut l, mut u) = (Vec::new(), Vec::new());
    for j in 0..ny {
        for i in 0..nx {
            let c = i + nx * j;
            if i + 1 < nx {
                l.push(c);
                u.push(c + 1);
            }
            if j + 1 < ny {
                l.push(c);
                u.push(c + nx);
            }
        }
    }
    let o = rcm_order(nx * ny, &l, &u);
    assert!(is_permutation(&o, nx * ny));
    assert_eq!(o, rcm_order(nx * ny, &l, &u));
    let inv = inverse_permutation(&o);
    for (i, &old) in o.iter().enumerate() {
        assert_eq!(inv[old], i);
    }
}

#[test]
fn r2_shuffled_path_has_bandwidth_one() {
    let n = 200;
    let label = shuffled(n);
    let (l, u): (Vec<usize>, Vec<usize>) = (0..n - 1).map(|i| (label[i], label[i + 1])).unzip();
    let before = bandwidth(&l, &u, None);
    assert!(before >= 100, "positive control: shuffled bandwidth {before}");
    let o = rcm_order(n, &l, &u);
    assert_eq!(bandwidth(&l, &u, Some(&o)), 1);
}
