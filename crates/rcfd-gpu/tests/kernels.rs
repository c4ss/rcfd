//! ADR 0038 checks D1 (discovery), K1 (SpMV) and K2 (reductions, axpy). Skip visibly without a device.
#![cfg(feature = "cuda")]

mod common;
use common::*;
use rcfd_gpu::{ops, DeviceStatus, DeviceSystem};
use rcfd_linalg::{CsrMatrix, LinearOptions};

#[test]
fn d1_discovery_reports_device_and_toolkit() {
    if rcfd_gpu::skip_without_device("d1_discovery") {
        return;
    }
    match rcfd_gpu::probe() {
        DeviceStatus::Available(i) => {
            println!("D1: {}", rcfd_gpu::probe().describe());
            println!(
                "D1: init (toolkit + context + NVRTC compile) {:.3} s",
                rcfd_gpu::init_seconds().unwrap()
            );
            assert!(!i.name.is_empty());
            assert!(
                i.compute_capability.0 >= 7,
                "compute capability {:?}",
                i.compute_capability
            );
            assert!(i.total_mem > 0);
            // CUDA_PATH is deliberately not required: the toolkit directory is reported
            println!("D1: toolkit dir `{}` found via {}", i.toolkit_dir, i.toolkit_source);
        }
        other => panic!("{other:?}"),
    }
}

fn spmv_case(name: &str, a: &CsrMatrix) -> f64 {
    let mut g = Lcg(99);
    let x: Vec<f64> = (0..a.n).map(|_| g.next_f64() * 2.0 - 1.0).collect();
    let yc = matvec_ref(a, &x);
    let mut yg = vec![0.0; a.n];
    let mut sys = DeviceSystem::new(a, &LinearOptions::default()).unwrap();
    sys.matvec(&x, &mut yg).unwrap();
    let d = rel2(&yg, &yc);
    let bitwise = yg.iter().zip(&yc).all(|(p, q)| p.to_bits() == q.to_bits());
    println!(
        "K1 {name}: n={} nnz={} rel2 diff {d:.3e} bitwise_equal={bitwise}",
        a.n,
        a.nnz()
    );
    d
}

#[test]
fn k1_spmv_matches_cpu() {
    if rcfd_gpu::skip_without_device("k1_spmv") {
        return;
    }
    let one = CsrMatrix::from_triplets(1, vec![(0, 0, 3.5)]);
    let mats: Vec<(&str, CsrMatrix)> = vec![
        ("poisson3d_20", grid_matrix(20, 20, 20, 0.0)),
        ("poisson2d_300", grid_matrix(300, 300, 1, 0.0)),
        ("convdiff2d_100", grid_matrix(100, 100, 1, 2.0)),
        ("irregular_50000", random_irregular(50_000, 7)),
        ("1x1", one),
    ];
    let mut worst = 0.0f64;
    for (name, a) in &mats {
        worst = worst.max(spmv_case(name, a));
    }
    println!("K1 max rel2 diff over the set: {worst:.3e} (limit 1e-13)");
    assert!(worst <= 1e-13, "{worst:e}");
}

fn data(kind: u8, n: usize, seed: u64) -> (Vec<f64>, Vec<f64>) {
    let mut g = Lcg(seed);
    let mut gen = |_: usize| -> f64 {
        match kind {
            0 => g.next_f64() * 2.0 - 1.0,
            1 => g.next_f64() + 0.1,
            _ => {
                let m = 10f64.powf(g.next_f64() * 12.0 - 6.0);
                if g.next_f64() < 0.5 {
                    -m
                } else {
                    m
                }
            }
        }
    };
    let a: Vec<f64> = (0..n).map(&mut gen).collect();
    let b: Vec<f64> = (0..n).map(&mut gen).collect();
    (a, b)
}

#[test]
fn k2_reductions_within_compensated_bound() {
    if rcfd_gpu::skip_without_device("k2_reductions") {
        return;
    }
    let sizes = [1usize, 2, 255, 256, 257, 1000, 65_537, 1_000_003, 4_194_304];
    let mut worst_dot = 0.0f64;
    let mut worst_norm = 0.0f64;
    let mut worst_sum = 0.0f64;
    let mut worst_axpy = 0.0f64;
    for kind in 0..3u8 {
        for &n in &sizes {
            let (a, b) = data(kind, n, 1000 + n as u64 + kind as u64);
            let refd = neumaier(a.iter().zip(&b).map(|(x, y)| x * y));
            let scale: f64 = a.iter().zip(&b).map(|(x, y)| (x * y).abs()).sum();
            let gd = ops::dot(&a, &b).unwrap();
            let e = (gd - refd).abs() / scale;
            worst_dot = worst_dot.max(e);
            assert!(e <= 1e-14, "dot kind {kind} n {n}: {e:e}");
            let gd2 = ops::dot(&a, &b).unwrap();
            assert_eq!(
                gd.to_bits(),
                gd2.to_bits(),
                "dot not reproducible run to run, kind {kind} n {n}"
            );
            // norm
            let rn = neumaier(a.iter().map(|x| x * x)).sqrt();
            let gn = ops::norm2(&a).unwrap();
            let en = (gn - rn).abs() / rn;
            worst_norm = worst_norm.max(en);
            assert!(en <= 1e-14, "norm kind {kind} n {n}: {en:e}");
            // sum
            let rs = neumaier(a.iter().copied());
            let sscale: f64 = a.iter().map(|x| x.abs()).sum();
            let gs = ops::sum(&a).unwrap();
            let es = (gs - rs).abs() / sscale;
            worst_sum = worst_sum.max(es);
            assert!(es <= 1e-14, "sum kind {kind} n {n}: {es:e}");
            // axpy componentwise
            let mut yg = b.clone();
            ops::axpy(0.37, &a, &mut yg).unwrap();
            let mut yc = b.clone();
            for (yi, xi) in yc.iter_mut().zip(&a) {
                *yi += 0.37 * xi;
            }
            let ea = yg
                .iter()
                .zip(&yc)
                .map(|(p, q)| if *q == 0.0 { (p - q).abs() } else { ((p - q) / q).abs() })
                .fold(0.0, f64::max);
            worst_axpy = worst_axpy.max(ea);
            assert!(ea <= 2.5e-16, "axpy kind {kind} n {n}: {ea:e}");
        }
    }
    println!(
        "K2 worst: dot {worst_dot:.3e} (limit 1e-14 sum|xy|), norm2 {worst_norm:.3e} (1e-14), sum {worst_sum:.3e} (1e-14), axpy {worst_axpy:.3e} (2.5e-16)"
    );
}
