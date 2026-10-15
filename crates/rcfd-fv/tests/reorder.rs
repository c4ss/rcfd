//! F-236 registered checks R3 and R4 (ADR 0020, part A): reverse Cuthill-McKee on stretched meshes.

use rcfd_linalg::reorder::{bandwidth, inverse_permutation, rcm_order};
use rcfd_linalg::{LduAddressing, LduMatrix};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::mesh::Mesh;
use std::sync::Arc;

fn faces(m: &Mesh) -> (Vec<usize>, Vec<usize>) {
    let nf = m.n_internal_faces();
    (m.owner()[..nf].to_vec(), m.neighbour()[..nf].to_vec())
}

fn shuffled(n: usize) -> Vec<usize> {
    let mut p: Vec<usize> = (0..n).collect();
    let mut s: u64 = 0x9E3779B97F4A7C15;
    for i in (1..n).rev() {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = ((s >> 33) as usize) % (i + 1);
        p.swap(i, j);
    }
    p
}

fn check(dims: [usize; 3], bound: usize) {
    let m = box_mesh(
        dims,
        [dims[0] as f64, dims[1] as f64, dims[2] as f64],
        [Grading::Uniform; 3],
    )
    .unwrap();
    let n = m.n_cells();
    let (l, u) = faces(&m);
    let natural = bandwidth(&l, &u, None);
    let order = rcm_order(n, &l, &u);
    let rcm = bandwidth(&l, &u, Some(&order));
    println!("R3 {dims:?}: natural {natural}, rcm {rcm}, derived bound {bound}");
    assert!(rcm <= bound, "{dims:?}: rcm {rcm} > bound {bound}");
    assert!(rcm < natural, "{dims:?}: rcm {rcm} !< natural {natural}");
    // shuffled labels
    let label = shuffled(n);
    let (ls, us): (Vec<usize>, Vec<usize>) = l.iter().zip(&u).map(|(&a, &b)| (label[a], label[b])).unzip();
    let sh = bandwidth(&ls, &us, None);
    let o2 = rcm_order(n, &ls, &us);
    let rcm2 = bandwidth(&ls, &us, Some(&o2));
    println!("R3 {dims:?} shuffled: bandwidth {sh}, rcm {rcm2}");
    assert!(rcm2 <= bound, "{dims:?} shuffled: rcm {rcm2} > bound {bound}");
    assert!(
        rcm2 <= sh / 4,
        "{dims:?} shuffled: rcm {rcm2} > shuffled/4 = {}",
        sh / 4
    );
}

#[test]
fn r3_stretched_3d_mesh() {
    check([40, 8, 8], 2 * 8 * 8 - 1);
}

#[test]
fn r3_stretched_2d_mesh() {
    check([40, 6, 1], 2 * 6 - 1);
}

#[test]
fn r4_permutation_identity() {
    let m = box_mesh([10, 6, 3], [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap();
    let n = m.n_cells();
    let (l, u) = faces(&m);
    let addr = Arc::new(LduAddressing {
        n,
        lower: l.clone(),
        upper: u.clone(),
    });
    let mut a = LduMatrix::zeros(addr.clone());
    for f in 0..l.len() {
        a.lower[f] = -1.0 - 0.01 * (f % 7) as f64;
        a.upper[f] = -1.0 + 0.02 * (f % 5) as f64;
    }
    for i in 0..n {
        a.diag[i] = 6.5 + 0.1 * (i % 3) as f64;
    }
    let csr = a.to_csr();
    assert_eq!(
        csr.bandwidth(),
        l.iter().zip(&u).map(|(&p, &q)| p.abs_diff(q)).max().unwrap()
    );
    assert_eq!(csr.bandwidth(), bandwidth(&l, &u, None));
    let order = rcm_order(n, &l, &u);
    let b = csr.permute_symmetric(&order);
    assert_eq!(b.bandwidth(), bandwidth(&l, &u, Some(&order)));
    let x: Vec<f64> = (0..n).map(|i| ((i as f64) * 0.37).sin() + 0.1 * i as f64).collect();
    let px: Vec<f64> = order.iter().map(|&o| x[o]).collect();
    let mut ax = vec![0.0; n];
    csr.matvec(&x, &mut ax);
    let pax: Vec<f64> = order.iter().map(|&o| ax[o]).collect();
    let mut bpx = vec![0.0; n];
    b.matvec(&px, &mut bpx);
    let scale = pax.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let err = pax.iter().zip(&bpx).fold(0.0f64, |m, (p, q)| m.max((p - q).abs())) / scale;
    println!("R4 permutation identity relative error {err:e}");
    assert!(err <= 1e-12, "{err}");
    // the inverse permutation takes B back to A
    let back = b.permute_symmetric(&inverse_permutation(&order));
    assert_eq!(back, csr);
}
