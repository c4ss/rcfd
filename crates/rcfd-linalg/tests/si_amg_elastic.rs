//! ADR 0305 (F-402): the local QR with its column-drop rule (Q1) and the rigid-body modes (Q2) of the elasticity multigrid.

use rcfd_linalg::amg_elastic::{rigid_body_modes, tentative_block, ElasticAmgSettings};

/// Rows of all components `comps` of each point.
fn rows(points: &[[f64; 3]], comps: &[usize]) -> (Vec<[f64; 3]>, Vec<usize>) {
    let mut p = Vec::new();
    let mut c = Vec::new();
    for x in points {
        for &k in comps {
            p.push(*x);
            c.push(k);
        }
    }
    (p, c)
}

fn row_major(modes: &[Vec<f64>]) -> (Vec<f64>, usize, usize) {
    let (k, m) = (modes.len(), modes[0].len());
    let mut b = vec![0.0; m * k];
    for (j, col) in modes.iter().enumerate() {
        for i in 0..m {
            b[i * k + j] = col[i];
        }
    }
    (b, m, k)
}

/// `|B - Q Q^T B|_F / |B|_F`.
fn defect(b: &[f64], m: usize, k: usize, q: &[f64], ka: usize) -> f64 {
    let mut num = 0.0;
    let mut den = 0.0;
    for c in 0..k {
        let coef: Vec<f64> = (0..ka)
            .map(|j| (0..m).map(|i| q[i * ka + j] * b[i * k + c]).sum())
            .collect();
        for i in 0..m {
            let proj: f64 = (0..ka).map(|j| q[i * ka + j] * coef[j]).sum();
            num += (b[i * k + c] - proj).powi(2);
            den += b[i * k + c].powi(2);
        }
    }
    (num / den).sqrt()
}

#[test]
fn q1_local_qr_drop_rule() {
    let tol = ElasticAmgSettings::default().drop_tolerance;
    let p0 = [1.0, 2.0, 3.0];
    let p1 = [1.3, 2.1, 2.9];
    let p2 = [
        p0[0] + 2.0 * (p1[0] - p0[0]),
        p0[1] + 2.0 * (p1[1] - p0[1]),
        p0[2] + 2.0 * (p1[2] - p0[2]),
    ];
    let p3 = [0.7, 2.5, 3.2];
    type Case = (&'static str, Vec<[f64; 3]>, Vec<usize>, usize);
    let cases: Vec<Case> = vec![
        ("one node", vec![p0], vec![0, 1, 2], 3),
        ("two nodes", vec![p0, p1], vec![0, 1, 2], 5),
        ("three collinear", vec![p0, p1, p2], vec![0, 1, 2], 5),
        ("three non-collinear", vec![p0, p1, p3], vec![0, 1, 2], 6),
        ("one node, components (0, 1)", vec![p0], vec![0, 1], 2),
    ];
    let mut ok = true;
    for (tag, pts, comps, want) in cases {
        let (p, c) = rows(&pts, &comps);
        let modes = rigid_body_modes(&p, &c);
        let (b, m, k) = row_major(&modes);
        let (q, ka) = tentative_block(&b, m, k, tol);
        let d = defect(&b, m, k, &q, ka);
        // orthonormality of Q
        let mut orth = 0.0f64;
        for i in 0..ka {
            for j in 0..ka {
                let s: f64 = (0..m).map(|r| q[r * ka + i] * q[r * ka + j]).sum();
                orth = orth.max((s - if i == j { 1.0 } else { 0.0 }).abs());
            }
        }
        println!("Q1 {tag}: kept {ka} (expected {want}), defect {d:.3e}, |Q^T Q - I| {orth:.3e}");
        ok &= ka == want && d <= 1e-12;
    }
    assert!(ok, "Q1");
}

#[test]
fn q2_rigid_body_modes_are_rigid() {
    let pts = [[0.1, 0.2, 0.3], [1.4, 0.1, 0.2], [0.3, 1.2, 0.1], [0.2, 0.4, 1.7]];
    let (p, c) = rows(&pts, &[0, 1, 2]);
    let modes = rigid_body_modes(&p, &c);
    let mut worst = 0.0f64;
    for col in &modes {
        let un = col.iter().map(|v| v * v).sum::<f64>().sqrt();
        for i in 0..4 {
            for j in 0..i {
                let du: Vec<f64> = (0..3).map(|d| col[3 * i + d] - col[3 * j + d]).collect();
                let dx: Vec<f64> = (0..3).map(|d| pts[i][d] - pts[j][d]).collect();
                let lx = dx.iter().map(|v| v * v).sum::<f64>().sqrt();
                let s: f64 = du.iter().zip(&dx).map(|(a, b)| a * b).sum();
                worst = worst.max(s.abs() / (un * lx));
            }
        }
    }
    // Gram matrix and its Cholesky pivots
    let k = modes.len();
    let mut g = vec![0.0; k * k];
    for a in 0..k {
        for b in 0..k {
            g[a * k + b] = modes[a].iter().zip(&modes[b]).map(|(x, y)| x * y).sum();
        }
    }
    let mut l = vec![0.0; k * k];
    let mut piv = Vec::new();
    for i in 0..k {
        for j in 0..=i {
            let mut s = g[i * k + j];
            for t in 0..j {
                s -= l[i * k + t] * l[j * k + t];
            }
            if i == j {
                piv.push(s);
                l[i * k + i] = s.max(0.0).sqrt();
            } else {
                l[i * k + j] = s / l[j * k + j];
            }
        }
    }
    let pmax = piv.iter().cloned().fold(f64::MIN, f64::max);
    let pmin = piv.iter().cloned().fold(f64::MAX, f64::min);
    println!(
        "Q2 worst relative stretch {worst:.3e} (limit 1e-14); Gram pivots min / max {:.3e}",
        pmin / pmax
    );
    assert!(worst <= 1e-14, "Q2 rigid");
    assert!(pmin > 1e-6 * pmax, "Q2 rank 6");
}
