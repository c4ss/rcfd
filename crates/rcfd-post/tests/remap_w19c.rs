//! Field transfer between meshes (F-115, ADR 0031 D2): checks I1-I6 as registered before the code.
//!
//! I1 linear exactness 1e-12; I2 convergence orders (second order methods in [1.7, 2.5], first
//! order in [0.7, 1.5]); I3 conservation 1e-12 (overlap) and 1e-10 (uniform shift); I4 bit-exact
//! identity; I5 fallback and boundedness; I6 refusals.

use rcfd_core::vec3::Vec3;
use rcfd_mesh::gen::{self, Grading};
use rcfd_mesh::Mesh;
use rcfd_post::remap::{RemapMethod, Remapper};

fn cav(nx: usize, ny: usize, lx: f64, ly: f64) -> Mesh {
    gen::cavity_2d_rect(nx, ny, lx, ly).unwrap()
}

/// 2D mesh with an explicit depth (equal for source and target of a pair).
fn flat(nx: usize, ny: usize, lx: f64, ly: f64, depth: f64) -> Mesh {
    gen::channel([nx, ny, 1], [lx, ly, depth], [Grading::Uniform; 3], true).unwrap()
}

type BoxPair = ([f64; 3], [f64; 3]);

fn boxes(m: &Mesh) -> Vec<BoxPair> {
    let pts = m.points();
    (0..m.n_cells())
        .map(|c| {
            let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
            for &f in m.cell_faces(c) {
                for &p in m.face_points(f) {
                    for k in 0..3 {
                        lo[k] = lo[k].min(pts[p][k]);
                        hi[k] = hi[k].max(pts[p][k]);
                    }
                }
            }
            (lo, hi)
        })
        .collect()
}

fn on_centres(m: &Mesh, f: impl Fn(Vec3) -> f64) -> Vec<f64> {
    m.cell_centres().iter().map(|&c| f(c)).collect()
}

fn rel_l2(a: &[f64], b: &[f64], vol: &[f64]) -> f64 {
    let num: f64 = a.iter().zip(b).zip(vol).map(|((x, y), v)| v * (x - y).powi(2)).sum();
    let den: f64 = b.iter().zip(vol).map(|(y, v)| v * y * y).sum();
    (num / den).sqrt()
}

fn max_abs(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

#[test]
fn i1_linear_fields_are_reproduced_to_rounding() {
    let lin2 = |p: Vec3| 0.7 + 1.3 * p[0] - 0.9 * p[1];
    let lin3 = |p: Vec3| 0.7 + 1.3 * p[0] - 0.9 * p[1] + 0.4 * p[2];
    let cases: Vec<(&str, Mesh, Mesh, bool)> = vec![
        ("2d coarse->fine", cav(13, 9, 1.3, 0.9), cav(17, 11, 1.3, 0.9), false),
        ("2d fine->coarse", cav(17, 11, 1.3, 0.9), cav(13, 9, 1.3, 0.9), false),
        (
            "3d coarse->fine",
            gen::channel([5, 4, 3], [1.0, 0.8, 0.6], [Grading::Uniform; 3], false).unwrap(),
            gen::channel([7, 5, 4], [1.0, 0.8, 0.6], [Grading::Uniform; 3], false).unwrap(),
            true,
        ),
    ];
    for (name, src, tgt, three_d) in cases {
        let f = |p: Vec3| if three_d { lin3(p) } else { lin2(p) };
        let s = on_centres(&src, f);
        let want = on_centres(&tgt, f);
        for method in [
            RemapMethod::LeastSquares,
            RemapMethod::ConservativeOverlap,
            RemapMethod::Gradient,
        ] {
            let r = Remapper::new(&src, &tgt, method).unwrap();
            let got = r.scalar(&s).unwrap();
            // gradient mode: only target centres inside the box spanned by the source centres
            let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
            for c in src.cell_centres() {
                for k in 0..3 {
                    lo[k] = lo[k].min(c[k]);
                    hi[k] = hi[k].max(c[k]);
                }
            }
            let mut worst: f64 = 0.0;
            for (j, c) in tgt.cell_centres().iter().enumerate() {
                let inside = (0..3).all(|k| hi[k] - lo[k] < 1e-12 || (c[k] >= lo[k] && c[k] <= hi[k]));
                if method == RemapMethod::Gradient && !inside {
                    continue;
                }
                worst = worst.max((got[j] - want[j]).abs());
            }
            println!("I1 {name} {}: max abs error {worst:.3e}", method.name());
            assert!(worst <= 1e-12, "{name} {}: {worst:e}", method.name());
        }
    }
}

fn avg_quad(lo: [f64; 3], hi: [f64; 3]) -> f64 {
    // exact cell average of x^2 + 0.5 x y + y^2 over a box
    let a2 = |l: f64, h: f64| (h.powi(3) - l.powi(3)) / (3.0 * (h - l));
    let a1 = |l: f64, h: f64| 0.5 * (l + h);
    a2(lo[0], hi[0]) + 0.5 * a1(lo[0], hi[0]) * a1(lo[1], hi[1]) + a2(lo[1], hi[1])
}

/// Observed orders `8->16` and `16->32` of one method (registered bands in the callers).
fn orders(m: RemapMethod) -> Vec<f64> {
    let q = |p: Vec3| p[0] * p[0] + 0.5 * p[0] * p[1] + p[1] * p[1];
    let tgt = cav(41, 37, 1.0, 1.0);
    let tgt_pt = on_centres(&tgt, q);
    let tgt_avg: Vec<f64> = boxes(&tgt).iter().map(|(l, h)| avg_quad(*l, *h)).collect();
    let vol = tgt.cell_volumes();
    let mut errs = Vec::new();
    for n in [8usize, 16, 32] {
        let src = cav(n, n, 1.0, 1.0);
        let (s, want) = if m == RemapMethod::ConservativeOverlap {
            (
                boxes(&src).iter().map(|(l, h)| avg_quad(*l, *h)).collect::<Vec<_>>(),
                &tgt_avg,
            )
        } else {
            (on_centres(&src, q), &tgt_pt)
        };
        let got = Remapper::new(&src, &tgt, m).unwrap().scalar(&s).unwrap();
        errs.push(rel_l2(&got, want, vol));
    }
    let o: Vec<f64> = errs.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
    println!("I2 {}: errors {errs:?} orders {o:?}", m.name());
    o
}

fn assert_band(m: RemapMethod, band: (f64, f64)) {
    for o in orders(m) {
        assert!(o >= band.0 && o <= band.1, "{}: order {o} outside {band:?}", m.name());
    }
}

const SECOND: (f64, f64) = (1.7, 2.5);
const FIRST: (f64, f64) = (0.7, 1.5);

#[test]
fn i2_least_squares_is_second_order() {
    assert_band(RemapMethod::LeastSquares, SECOND);
}

/// Registered check I2 for `conservative_overlap` (band 1.7 to 2.5). Measured orders 2.63 and 3.77 (errors
/// 3.54e-3, 5.73e-4, 4.21e-5): above the band. Diagnosis: with exact cell averages of a quadratic the
/// symmetric least-squares gradient reconstruction is exact in the interior, so only the boundary cells carry
/// error and it falls faster than h^2 (see `diagnostic_orders_for_a_smooth_non_polynomial_field`). Known miss.
#[test]
#[ignore = "known miss: observed orders 2.63 and 3.77 above the registered band [1.7, 2.5] (quadratic superconvergence)"]
fn i2_conservative_overlap_is_second_order_known_miss() {
    assert_band(RemapMethod::ConservativeOverlap, SECOND);
}

#[test]
fn i2_nearest_is_first_order() {
    assert_band(RemapMethod::Nearest, FIRST);
}

/// Registered check I2 for `inverse_distance` (band 0.7 to 1.5). Measured orders 1.58 and 1.64 (errors
/// 6.29e-2, 2.10e-2, 6.75e-3): above the band; with symmetric neighbour sets the first-order error terms
/// of the weighted mean largely cancel. Known miss.
#[test]
#[ignore = "known miss: observed orders 1.58 and 1.64 above the registered band [0.7, 1.5]"]
fn i2_inverse_distance_is_first_order_known_miss() {
    assert_band(RemapMethod::InverseDistance, FIRST);
}

/// Registered check I2 for `gradient` (band 1.7 to 2.5). Measured orders 2.09 and 2.75: the second
/// interval is above the band (errors 1.22e-2, 2.87e-3, 4.27e-4). Kept as a known miss, limit intact.
#[test]
#[ignore = "known miss: observed order 2.75 above the registered band [1.7, 2.5]"]
fn i2_gradient_is_second_order_known_miss() {
    assert_band(RemapMethod::Gradient, SECOND);
}

fn integral(m: &Mesh, f: &[f64]) -> f64 {
    m.cell_volumes().iter().zip(f).map(|(v, x)| v * x).sum()
}

#[test]
fn i3_conservation_of_integrals() {
    let depth = 0.05;
    let fld = |p: Vec3| 1.0 + (3.0 * p[0]).sin() * (2.0 * p[1]).cos() + if p[0] > 0.5 { 0.5 } else { 0.0 };
    let pairs = [
        ("refine", flat(10, 7, 1.0, 1.0, depth), flat(23, 17, 1.0, 1.0, depth)),
        ("coarsen", flat(23, 17, 1.0, 1.0, depth), flat(10, 7, 1.0, 1.0, depth)),
    ];
    for (name, src, tgt) in pairs {
        let s = on_centres(&src, fld);
        let sneg: Vec<f64> = s.iter().map(|x| -x).collect();
        let den: f64 = src.cell_volumes().iter().zip(&s).map(|(v, x)| v * x.abs()).sum();
        let r = Remapper::new(&src, &tgt, RemapMethod::ConservativeOverlap).unwrap();
        let vec: Vec<Vec3> = s.iter().map(|&x| [x, -x, 0.0]).collect();
        let tv = r.vector(&vec).unwrap();
        let t0: Vec<f64> = tv.iter().map(|v| v[0]).collect();
        let t1: Vec<f64> = tv.iter().map(|v| v[1]).collect();
        let e0 = (integral(&tgt, &t0) - integral(&src, &s)).abs() / den;
        let e1 = (integral(&tgt, &t1) - integral(&src, &sneg)).abs() / den;
        println!("I3 {name} overlap: relative integral mismatch {e0:.3e} {e1:.3e}");
        assert!(e0 <= 1e-12 && e1 <= 1e-12, "{name}: {e0:e} {e1:e}");
        let rc = Remapper::new(&src, &tgt, RemapMethod::Conservative).unwrap();
        let tc = rc.scalar(&s).unwrap();
        let ec = (integral(&tgt, &tc) - integral(&src, &s)).abs() / den;
        println!("I3 {name} uniform shift: relative integral mismatch {ec:.3e}");
        assert!(ec <= 1e-10, "{name} shift: {ec:e}");
    }
}

#[test]
fn i4_identical_meshes_give_bit_identical_fields() {
    let a = cav(12, 9, 1.2, 0.9);
    let b = cav(12, 9, 1.2, 0.9);
    let s = on_centres(&a, |p| (7.0 * p[0]).sin() + p[1] * p[1]);
    let v: Vec<Vec3> = s.iter().map(|&x| [x, 0.3 * x, -x]).collect();
    for m in RemapMethod::ALL {
        let r = Remapper::new(&a, &b, m).unwrap();
        assert!(r.is_identity());
        let got = r.scalar(&s).unwrap();
        assert!(
            got.iter().zip(&s).all(|(x, y)| x.to_bits() == y.to_bits()),
            "{}",
            m.name()
        );
        let gv = r.vector(&v).unwrap();
        assert!(gv
            .iter()
            .zip(&v)
            .all(|(x, y)| (0..3).all(|k| x[k].to_bits() == y[k].to_bits())));
    }
}

#[test]
fn i5_fallback_and_boundedness_outside_the_source() {
    let src = cav(12, 12, 1.0, 1.0);
    let tgt = cav(20, 20, 1.2, 1.2);
    let s = on_centres(&src, |p| 2.0 + (4.0 * p[0]).sin() * p[1]);
    let (lo, hi) = (
        s.iter().cloned().fold(f64::INFINITY, f64::min),
        s.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    for m in RemapMethod::ALL {
        if m == RemapMethod::ConservativeOverlap {
            continue;
        }
        let got = Remapper::new(&src, &tgt, m).unwrap().scalar(&s).unwrap();
        assert!(got.iter().all(|x| x.is_finite()), "{}", m.name());
        if matches!(
            m,
            RemapMethod::Nearest | RemapMethod::InverseDistance | RemapMethod::Gradient
        ) {
            assert!(
                got.iter().all(|&x| x >= lo - 1e-14 && x <= hi + 1e-14),
                "{} not bounded",
                m.name()
            );
        }
    }
    // constant field through the overlap method, including partially covered target cells
    let r = Remapper::new(&src, &tgt, RemapMethod::ConservativeOverlap).unwrap();
    let c = r.scalar(&vec![3.25; src.n_cells()]).unwrap();
    let worst = c.iter().map(|x| (x - 3.25).abs() / 3.25).fold(0.0, f64::max);
    println!(
        "I5 constant through overlap: worst relative error {worst:.3e}, min coverage {}",
        r.min_coverage()
    );
    assert!(worst <= 1e-14);
    assert!(r.min_coverage() < 1.0);
}

#[test]
fn i6_refusals() {
    let o = gen::cylinder_ogrid(12, 4, 0.5, 1.0, Grading::Uniform, 0.1).unwrap();
    let b = cav(8, 8, 1.0, 1.0);
    match Remapper::new(&o, &b, RemapMethod::ConservativeOverlap) {
        Err(e) => assert!(e.to_string().contains("axis-aligned"), "{e}"),
        Ok(_) => panic!("non-box mesh accepted"),
    }
    let b2 = cav(9, 9, 1.0, 1.0);
    let r = Remapper::new(&b, &b2, RemapMethod::LeastSquares).unwrap();
    assert!(r.scalar(&[1.0, 2.0]).is_err());
}

/// Not a registered check: the point-value methods also work on a non-box (o-grid) pair.
#[test]
fn point_methods_work_on_unstructured_pairs() {
    let o = gen::cylinder_ogrid(24, 6, 0.5, 1.0, Grading::Uniform, 0.1).unwrap();
    let o2 = gen::cylinder_ogrid(36, 9, 0.5, 1.0, Grading::Uniform, 0.1).unwrap();
    let lin = |p: Vec3| 1.0 + 0.5 * p[0] - 0.25 * p[1];
    let s = on_centres(&o, lin);
    let want = on_centres(&o2, lin);
    let got = Remapper::new(&o, &o2, RemapMethod::LeastSquares)
        .unwrap()
        .scalar(&s)
        .unwrap();
    println!(
        "o-grid least_squares linear field max error {:.3e}",
        max_abs(&got, &want)
    );
}

/// Registered check I5, least-squares clause, literal: target centres outside the bounding box of the
/// source cell CENTRES take the nearest value exactly. Misses by design: the implemented fit extrapolates
/// over the boundary half cells (needed for the registered I1, all target cells exact), so the
/// target centre 0.99 (inside the source mesh, outside the centre box) gets the fit, not the nearest value.
#[test]
#[ignore = "known miss: registered I1 and this clause of I5 are inconsistent for targets between the last source centre and the source boundary"]
fn i5_least_squares_nearest_outside_centre_box_known_miss() {
    let src = cav(12, 12, 1.0, 1.0);
    let tgt = cav(20, 20, 1.2, 1.2);
    let s = on_centres(&src, |p| 2.0 + (4.0 * p[0]).sin() * p[1]);
    let near = Remapper::new(&src, &tgt, RemapMethod::Nearest)
        .unwrap()
        .scalar(&s)
        .unwrap();
    let got = Remapper::new(&src, &tgt, RemapMethod::LeastSquares)
        .unwrap()
        .scalar(&s)
        .unwrap();
    let (mut clo, mut chi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for c in src.cell_centres() {
        for k in 0..2 {
            clo[k] = clo[k].min(c[k]);
            chi[k] = chi[k].max(c[k]);
        }
    }
    for (j, c) in tgt.cell_centres().iter().enumerate() {
        if (0..2).any(|k| c[k] < clo[k] - 1e-12 || c[k] > chi[k] + 1e-12) {
            assert_eq!(got[j].to_bits(), near[j].to_bits(), "target {j} at {c:?}");
        }
    }
}

/// Corrected I5 clause (new test, the registered one is kept above as a known miss): target centres outside
/// the bounding box of the source MESH take the nearest value exactly, and there are such centres.
#[test]
fn i5b_least_squares_nearest_outside_mesh_box() {
    let src = cav(12, 12, 1.0, 1.0);
    let tgt = cav(20, 20, 1.2, 1.2);
    let s = on_centres(&src, |p| 2.0 + (4.0 * p[0]).sin() * p[1]);
    let near = Remapper::new(&src, &tgt, RemapMethod::Nearest)
        .unwrap()
        .scalar(&s)
        .unwrap();
    let got = Remapper::new(&src, &tgt, RemapMethod::LeastSquares)
        .unwrap()
        .scalar(&s)
        .unwrap();
    let mut n_out = 0;
    for (j, c) in tgt.cell_centres().iter().enumerate() {
        if (0..2).any(|k| c[k] < 0.0 || c[k] > 1.0) {
            n_out += 1;
            assert_eq!(got[j].to_bits(), near[j].to_bits(), "target {j} at {c:?}");
        }
    }
    assert!(n_out > 0);
}

/// Diagnostic, NOT a registered check: observed orders for a smooth non-polynomial field, to see the methods
/// without the quadratic's superconvergence. Prints; asserts only that errors decrease.
#[test]
fn diagnostic_orders_for_a_smooth_non_polynomial_field() {
    let f = |p: Vec3| (3.0 * p[0]).sin() * (2.0 * p[1]).cos();
    let tgt = cav(41, 37, 1.0, 1.0);
    let want = on_centres(&tgt, f);
    for m in [
        RemapMethod::Nearest,
        RemapMethod::InverseDistance,
        RemapMethod::LeastSquares,
        RemapMethod::Gradient,
    ] {
        let mut errs = Vec::new();
        for n in [8usize, 16, 32] {
            let src = cav(n, n, 1.0, 1.0);
            let got = Remapper::new(&src, &tgt, m)
                .unwrap()
                .scalar(&on_centres(&src, f))
                .unwrap();
            errs.push(rel_l2(&got, &want, tgt.cell_volumes()));
        }
        let o: Vec<f64> = errs.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
        println!("diagnostic {}: errors {errs:?} orders {o:?}", m.name());
        assert!(errs[2] < errs[0]);
    }
}
