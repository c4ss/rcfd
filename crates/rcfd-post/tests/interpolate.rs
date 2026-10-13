//! F-115 building blocks: the gradient-corrected nearest-cell transfer reproduces a linear field
//! exactly inside the source cell-centre hull (check fixed up front: 1e-12), never creates new
//! extrema, and the conservative mode matches the volume average.

use rcfd_mesh::gen::box_2d;
use rcfd_post::interpolate::{InterpMode, Interpolator};

#[test]
fn linear_field_is_reproduced_inside_the_hull() {
    let src = box_2d(8, 8, 1.0, 1.0, false, false).unwrap();
    let tgt = box_2d(20, 14, 1.0, 1.0, false, false).unwrap();
    let f = |c: [f64; 3]| 2.0 * c[0] + 3.0 * c[1] + 1.0;
    let vals: Vec<f64> = src.cell_centres().iter().map(|&c| f(c)).collect();
    let ip = Interpolator::new(&src, &tgt).unwrap();
    let out = ip.scalar(&vals, InterpMode::NonConservative).unwrap();
    let (lo, hi) = (
        vals.iter().cloned().fold(f64::INFINITY, f64::min),
        vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    let mut checked = 0;
    for (c, v) in tgt.cell_centres().iter().zip(&out) {
        assert!(*v >= lo - 1e-12 && *v <= hi + 1e-12, "new extremum {v}");
        if c[0] > 1.0 / 16.0 && c[0] < 15.0 / 16.0 && c[1] > 1.0 / 16.0 && c[1] < 15.0 / 16.0 {
            // the nearest source cell has all neighbours, so the gradient is exact
            assert!((v - f(*c)).abs() < 1e-12, "{} vs {}", v, f(*c));
            checked += 1;
        }
    }
    assert!(checked > 150, "{checked}");
}

#[test]
fn conservative_matches_average_and_vector_works() {
    let src = box_2d(10, 10, 1.0, 1.0, false, false).unwrap();
    let tgt = box_2d(7, 13, 1.0, 1.0, false, false).unwrap();
    let vals: Vec<f64> = src
        .cell_centres()
        .iter()
        .map(|c| (3.0 * c[0]).sin() + c[1] * c[1])
        .collect();
    let ip = Interpolator::new(&src, &tgt).unwrap();
    let avg = |m: &rcfd_mesh::Mesh, v: &[f64]| {
        let w = m.cell_volumes();
        v.iter().zip(w).map(|(a, b)| a * b).sum::<f64>() / w.iter().sum::<f64>()
    };
    let c = ip.scalar(&vals, InterpMode::Conservative).unwrap();
    assert!((avg(&tgt, &c) - avg(&src, &vals)).abs() < 1e-13);
    let n = ip.scalar(&vals, InterpMode::NonConservative).unwrap();
    assert!(
        (avg(&tgt, &n) - avg(&src, &vals)).abs() > 1e-6,
        "test would not discriminate the modes"
    );
    let vv: Vec<[f64; 3]> = vals.iter().map(|&x| [x, -x, 0.0]).collect();
    let o = ip.vector(&vv, InterpMode::NonConservative).unwrap();
    assert!(o.iter().zip(&n).all(|(a, b)| a[0] == *b && a[1] == -*b && a[2] == 0.0));
    assert!(ip.scalar(&vals[1..], InterpMode::Conservative).is_err());
    assert_eq!(InterpMode::parse("Non-Conservative"), Some(InterpMode::NonConservative));
    assert_eq!(InterpMode::parse("conservative"), Some(InterpMode::Conservative));
    assert_eq!(InterpMode::parse("x"), None);
}
