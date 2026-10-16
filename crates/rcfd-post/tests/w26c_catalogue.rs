//! F-238 registered checks (ADR 0048). Limits are the registered ones; measured values are printed with `--nocapture`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_post::compare::{difference, difference_vector};
use rcfd_post::remap::RemapMethod;
use rcfd_post::stats::*;
use rcfd_post::surfaces::*;
use std::f64::consts::PI;
use std::sync::Arc;

fn cube(n: usize) -> Mesh {
    box_mesh([n; 3], [1.0; 3], [Grading::Uniform; 3]).unwrap()
}
fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}
fn order(e_coarse: f64, e_fine: f64) -> f64 {
    (e_coarse / e_fine).log2()
}
const C: Vec3 = [0.5, 0.5, 0.5];
const R: f64 = 0.3;

fn cell_field(m: &Mesh, f: impl Fn(Vec3) -> f64) -> Vec<f64> {
    m.cell_centres().iter().map(|&c| f(c)).collect()
}

#[test]
fn s1_sphere_area() {
    let mut e = Vec::new();
    for n in [8, 16, 32] {
        let s = cut_sphere(&cube(n), C, R).unwrap();
        e.push(rel(s.area(), 4.0 * PI * R * R));
        eprintln!("S1 n={n} rel err {:e}", e.last().unwrap());
    }
    eprintln!("S1 orders {} {}", order(e[0], e[1]), order(e[1], e[2]));
    assert!(e[2] <= 5e-3);
    assert!(order(e[1], e[2]) >= 1.5);
}

#[test]
fn s2_sphere_flux_and_sign() {
    let mut e = Vec::new();
    for n in [8, 16, 32] {
        let m = cube(n);
        let s = cut_sphere(&m, C, R).unwrap();
        let fv: Vec<Vec3> = m.cell_centres().iter().map(|&c| vec3::sub(c, C)).collect();
        let fl = s.flux(&s.facet_vector(&m, &fv).unwrap()).unwrap();
        e.push(rel(fl, 4.0 * PI * R * R * R));
        eprintln!("S2 n={n} rel err {:e}", e.last().unwrap());
        if n == 16 {
            // reversed g flips the orientation
            let q = Quadric::sphere(C, R).unwrap();
            let rev = cut_implicit(&m, |p| -q.eval(p)).unwrap();
            let fr = rev.flux(&rev.facet_vector(&m, &fv).unwrap()).unwrap();
            eprintln!("S2 reversed/forward {:e}", (fr + fl).abs() / fl.abs());
            assert!((fr + fl).abs() <= 1e-12 * fl.abs());
        }
    }
    eprintln!("S2 orders {} {}", order(e[0], e[1]), order(e[1], e[2]));
    assert!(e[2] <= 5e-3);
    assert!(order(e[1], e[2]) >= 1.5);
}

#[test]
fn s3_sphere_mean_and_std() {
    let std_exact = R / 3f64.sqrt();
    let mut e = Vec::new();
    for n in [16, 32] {
        let m = cube(n);
        let s = cut_sphere(&m, C, R).unwrap();
        let f = cell_field(&m, |p| 2.0 + (p[2] - C[2]));
        let st = s.stats(&s.facet_scalar(&m, &f).unwrap()).unwrap();
        e.push(rel(st.std, std_exact));
        eprintln!(
            "S3 n={n} mean err {:e} std rel {:e}",
            (st.mean - 2.0).abs(),
            e.last().unwrap()
        );
        if n == 32 {
            assert!((st.mean - 2.0).abs() <= 2e-3);
            assert!(e[1] <= 5e-3);
            let f2 = cell_field(&m, |p| 1e9 + 2.0 + (p[2] - C[2]));
            let st2 = s.stats(&s.facet_scalar(&m, &f2).unwrap()).unwrap();
            eprintln!("S3 offset 1e9 std rel {:e}", rel(st2.std, std_exact));
            assert!(rel(st2.std, std_exact) <= 5e-3);
        }
    }
    eprintln!("S3 order {}", order(e[0], e[1]));
    assert!(order(e[0], e[1]) >= 1.5);
}

fn box_faces(m: &Mesh, f: impl Fn(Vec3) -> f64) -> (Vec<f64>, Vec<f64>) {
    let ni = m.n_internal_faces();
    let mut v = Vec::new();
    let mut w = Vec::new();
    for p in m.patches() {
        for fc in p.faces() {
            v.push(f(m.face_centres()[fc]));
            w.push(vec3::norm(m.face_area_vectors()[fc]));
            assert!(fc >= ni);
        }
    }
    (v, w)
}

/// Original version. Its auxiliary (not registered) `patch_stats` expectation 0.4375 is a set-up defect: the mean of the
/// face-centre y over a uniform 8x8 patch is 0.5. Kept as a record, corrected in `b1_b3_box_area_flux_mean_std_corrected`.
#[test]
#[ignore = "set-up defect in an auxiliary assertion (expected 0.4375, correct 0.5), see the corrected test"]
fn b1_b3_box_area_flux_mean_std() {
    let m = cube(8);
    let (_, w) = box_faces(&m, |_| 0.0);
    let area: f64 = w.iter().sum();
    assert!((area - 6.0).abs() <= 1e-12);
    // flux of F=(x,y,z)
    let ni = m.n_internal_faces();
    let fvec: Vec<Vec3> = (ni..m.n_faces()).map(|f| m.face_centres()[f]).collect();
    let flux: f64 = m
        .patches()
        .iter()
        .map(|p| normal_flux(&m, &p.name, &fvec).unwrap())
        .sum();
    eprintln!("B2 flux {flux}");
    assert!((flux - 3.0).abs() <= 1e-12);
    let (v, w) = box_faces(&m, |p| p[0]);
    let st = weighted_stats(&v, &w).unwrap();
    assert!((st.mean - 0.5).abs() <= 1e-12);
    let exact = 5f64.sqrt() / 6.0;
    let mut e = Vec::new();
    for n in [8, 16] {
        let m = cube(n);
        let (v, w) = box_faces(&m, |p| p[0]);
        let s = weighted_stats(&v, &w).unwrap();
        e.push(rel(s.std, exact));
        eprintln!("B3 n={n} std rel {:e}", e.last().unwrap());
    }
    eprintln!("B3 order {}", order(e[0], e[1]));
    assert!(e[1] <= 5e-3);
    assert!(order(e[0], e[1]) >= 1.9);
    // patch_stats of one patch
    let ni = m.n_internal_faces();
    let fv: Vec<f64> = (ni..m.n_faces()).map(|f| m.face_centres()[f][1]).collect();
    let ps = patch_stats(&m, "xmax", &fv).unwrap();
    assert!((ps.mean - 0.4375).abs() < 1e-12 && ps.count == 64);
    assert!((ps.min - 1.0 / 16.0).abs() < 1e-14 && (ps.max - 15.0 / 16.0).abs() < 1e-14);
}

#[test]
fn b1_b3_box_area_flux_mean_std_corrected() {
    let m = cube(8);
    let (_, w) = box_faces(&m, |_| 0.0);
    let area: f64 = w.iter().sum();
    assert!((area - 6.0).abs() <= 1e-12);
    // flux of F=(x,y,z)
    let ni = m.n_internal_faces();
    let fvec: Vec<Vec3> = (ni..m.n_faces()).map(|f| m.face_centres()[f]).collect();
    let flux: f64 = m
        .patches()
        .iter()
        .map(|p| normal_flux(&m, &p.name, &fvec).unwrap())
        .sum();
    eprintln!("B2 flux {flux}");
    assert!((flux - 3.0).abs() <= 1e-12);
    let (v, w) = box_faces(&m, |p| p[0]);
    let st = weighted_stats(&v, &w).unwrap();
    assert!((st.mean - 0.5).abs() <= 1e-12);
    let exact = 5f64.sqrt() / 6.0;
    let mut e = Vec::new();
    for n in [8, 16] {
        let m = cube(n);
        let (v, w) = box_faces(&m, |p| p[0]);
        let s = weighted_stats(&v, &w).unwrap();
        e.push(rel(s.std, exact));
        eprintln!("B3 n={n} std rel {:e}", e.last().unwrap());
    }
    eprintln!("B3 order {}", order(e[0], e[1]));
    assert!(e[1] <= 5e-3);
    assert!(order(e[0], e[1]) >= 1.9);
    // patch_stats of one patch
    let ni = m.n_internal_faces();
    let fv: Vec<f64> = (ni..m.n_faces()).map(|f| m.face_centres()[f][1]).collect();
    let ps = patch_stats(&m, "xmax", &fv).unwrap();
    assert!((ps.mean - 0.5).abs() < 1e-12 && ps.count == 64);
    assert!((ps.min - 1.0 / 16.0).abs() < 1e-14 && (ps.max - 15.0 / 16.0).abs() < 1e-14);
}

fn uniform_phi(m: &Mesh, u: Vec3) -> Vec<f64> {
    m.face_area_vectors().iter().map(|s| vec3::dot(*s, u)).collect()
}

#[test]
fn b4_b5_fluxes_and_weighted_average() {
    let m = cube(8);
    let ni = m.n_internal_faces();
    let phi = uniform_phi(&m, [1.0, 0.0, 0.0]);
    let q: Vec<f64> = (ni..m.n_faces()).map(|f| m.face_centres()[f][1]).collect();
    let out: f64 = m.patch("xmax").unwrap().faces().map(|f| phi[f]).sum();
    let inn: f64 = m.patch("xmin").unwrap().faces().map(|f| phi[f]).sum();
    assert!((out - 1.0).abs() <= 1e-12 && (inn + 1.0).abs() <= 1e-12);
    for p in ["xmax", "xmin"] {
        let a = mass_weighted_average(&m, p, &phi, &q).unwrap();
        assert!((a - 0.5).abs() <= 1e-12, "{p} {a}");
    }
    let phi = uniform_phi(&m, [2.0, 3.0, 5.0]);
    let u = vec![[2.0, 3.0, 5.0]; m.n_faces() - ni];
    let v = vector_flux(&m, "xmax", &phi, &u).unwrap();
    assert!((v[0] - 4.0).abs() <= 1e-12 && (v[1] - 6.0).abs() <= 1e-12 && (v[2] - 10.0).abs() <= 1e-12);
    let vin = vector_flux(&m, "xmin", &phi, &u).unwrap();
    assert!((vin[0] + 4.0).abs() <= 1e-12);
    // closed box: net transport of a uniform vector is zero
    let tot = m.patches().iter().fold([0.0; 3], |s, p| {
        vec3::add(s, vector_flux(&m, &p.name, &phi, &u).unwrap())
    });
    assert!(vec3::norm(tot) <= 1e-12);
    // net-zero flux is an error
    let zero = vec![0.0; m.n_faces()];
    assert!(mass_weighted_average(&m, "xmax", &zero, &q).is_err());
}

#[test]
fn b6_force_projection() {
    let (a, p) = project_force([3.0, 4.0, 0.0], [0.0, 1.0, 0.0]).unwrap();
    assert!((a - 4.0).abs() <= 1e-14 && (p[0] - 3.0).abs() <= 1e-14 && p[1].abs() <= 1e-14);
    let (a, p) = project_force([3.0, 4.0, 0.0], [3.0, 4.0, 0.0]).unwrap();
    assert!((a - 5.0).abs() <= 1e-14 && vec3::norm(p) <= 1e-14);
    assert!((project_moment([0.0, 0.0, 7.0], [0.0, 0.0, 2.0]).unwrap() - 7.0).abs() <= 1e-14);
}

#[test]
fn w1_weighted_stats() {
    let mut v = Vec::new();
    for _ in 0..1000 {
        for k in 0..7 {
            v.push(1e9 + k as f64);
        }
    }
    let s = weighted_stats(&v, &vec![1.0; v.len()]).unwrap();
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    let naive = v.iter().map(|x| x * x).sum::<f64>() / n - mean * mean;
    eprintln!(
        "W1 variance {} (error {:e}); naive formula {} ",
        s.variance,
        (s.variance - 4.0).abs(),
        naive
    );
    assert!((s.variance - 4.0).abs() <= 1e-9);
    let s = weighted_stats(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]).unwrap();
    assert!((s.mean - 7.0 / 3.0).abs() <= 1e-14 && (s.variance - 5.0 / 9.0).abs() <= 1e-14);
    assert!(s.min == 1.0 && s.max == 3.0 && s.count == 3);
}

#[test]
fn q1_plane_cuts_exact() {
    for n in [4, 8] {
        let m = cube(n);
        let cases: [(Vec3, Vec3, f64); 3] = [
            ([0.37, 0.5, 0.5], [1.0, 0.0, 0.0], 1.0),
            ([0.5, 0.5, 0.5], [1.0, 1.0, 0.0], 2f64.sqrt()),
            ([0.5, 0.5, 0.5], [1.0, 1.0, 1.0], 3.0 * 3f64.sqrt() / 4.0),
        ];
        for (p, nrm, area) in cases {
            let pt = if nrm == [1.0, 1.0, 1.0] { [0.5, 0.5, 0.5] } else { p };
            let s = cut_plane(&m, pt, nrm).unwrap();
            let a = s.area();
            let un = vec3::unit(nrm);
            let mind = s
                .area_vectors()
                .iter()
                .map(|v| vec3::dot(vec3::unit(*v), un))
                .fold(f64::INFINITY, f64::min);
            eprintln!(
                "Q1 n={n} nrm={nrm:?} area err {:e} min dot {:e}",
                (a - area).abs(),
                1.0 - mind
            );
            assert!((a - area).abs() <= 1e-12, "{a} vs {area}");
            assert!(mind >= 1.0 - 1e-12);
        }
    }
}

#[test]
fn q2_cylinder_cone_quadric() {
    let mut e = Vec::new();
    for n in [16, 32] {
        let s = cut_cylinder(&cube(n), C, [0.0, 0.0, 1.0], R).unwrap();
        e.push(rel(s.area(), 2.0 * PI * R));
        eprintln!("Q2 cylinder n={n} rel {:e}", e.last().unwrap());
    }
    assert!(e[1] <= 5e-3);
    assert!(order(e[0], e[1]) >= 1.5);
    let a = 20f64.to_radians();
    let s = cut_cone(&cube(32), [0.5, 0.5, 0.0], [0.0, 0.0, 1.0], a).unwrap();
    let ex = PI * a.tan() / a.cos();
    eprintln!("Q2 cone rel {:e}", rel(s.area(), ex));
    assert!(rel(s.area(), ex) <= 1e-2);
    // ellipsoid flux
    let m = cube(32);
    let (sa, sb, sc) = (0.35, 0.25, 0.3);
    let amat = [
        [1.0 / (sa * sa), 0.0, 0.0],
        [0.0, 1.0 / (sb * sb), 0.0],
        [0.0, 0.0, 1.0 / (sc * sc)],
    ];
    let b = [
        -2.0 * C[0] * amat[0][0],
        -2.0 * C[1] * amat[1][1],
        -2.0 * C[2] * amat[2][2],
    ];
    let c = C[0] * C[0] * amat[0][0] + C[1] * C[1] * amat[1][1] + C[2] * C[2] * amat[2][2] - 1.0;
    let q = Quadric::new(amat, b, c).unwrap();
    let s = cut_quadric(&m, &q).unwrap();
    let fv: Vec<Vec3> = m.cell_centres().iter().map(|&p| vec3::sub(p, C)).collect();
    let fl = s.flux(&s.facet_vector(&m, &fv).unwrap()).unwrap();
    eprintln!("Q2 ellipsoid flux rel {:e}", rel(fl, 4.0 * PI * sa * sb * sc));
    assert!(rel(fl, 4.0 * PI * sa * sb * sc) <= 5e-3);
    // general-quadric sphere equals cut_sphere
    let id = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let qs = Quadric::new(id, [-1.0, -1.0, -1.0], 0.75 - R * R).unwrap();
    let a1 = cut_quadric(&cube(16), &qs).unwrap().area();
    let a2 = cut_sphere(&cube(16), C, R).unwrap().area();
    assert!(rel(a1, a2) <= 1e-14);
}

#[test]
fn o1_offset() {
    let m = cube(8);
    let s = cut_plane(&m, [0.4, 0.5, 0.5], [1.0, 0.0, 0.0]).unwrap();
    let o = s.offset(0.1).unwrap();
    let worst = o.points.iter().map(|p| (p[0] - 0.5).abs()).fold(0.0, f64::max);
    eprintln!("O1 plane worst {worst:e}");
    assert!(worst <= 1e-14);
    let s = cut_sphere(&cube(32), C, R).unwrap();
    let o = s.offset(0.05).unwrap();
    eprintln!("O1 sphere rel {:e}", rel(o.area(), 4.0 * PI * 0.35 * 0.35));
    assert!(rel(o.area(), 4.0 * PI * 0.35 * 0.35) <= 1e-2);
}

#[test]
fn r1_rake_and_point_cloud() {
    let m = Arc::new(cube(8));
    let g = FvGeometry::new(m.clone()).unwrap();
    let (p0, p1) = ([0.1, 0.2, 0.3], [0.9, 0.8, 0.7]);
    let r = PointCloud::rake(p0, p1, 11).unwrap();
    assert_eq!(r.points[0], p0);
    assert_eq!(r.points[10], p1);
    let h = vec3::scale(vec3::sub(p1, p0), 0.1);
    for i in 0..11 {
        let e = vec3::add(p0, vec3::scale(h, i as f64));
        assert!(vec3::norm(vec3::sub(r.points[i], e)) <= 1e-14);
    }
    let f = cell_field(&m, |p| 1.0 + 2.0 * p[0] - p[1] + 3.0 * p[2]);
    let (v, ins) = r.sample(&g, &f).unwrap();
    for (i, p) in r.points.iter().enumerate() {
        assert!(ins[i]);
        assert!((v[i] - (1.0 + 2.0 * p[0] - p[1] + 3.0 * p[2])).abs() <= 1e-12, "{i}");
    }
    let out = PointCloud {
        points: vec![[1.5, 0.5, 0.5]],
    };
    let (v, ins) = out.sample(&g, &f).unwrap();
    assert!(v[0].is_nan() && !ins[0]);
}

#[test]
fn t1_instance_transforms() {
    let m = cube(16);
    let s = cut_sphere(&m, C, R).unwrap();
    let a = s.area();
    let copies = s.rotational_copies(4, [0.0, 0.0, 1.0], C).unwrap();
    let all = Surface::merged(&copies);
    eprintln!("T1 area rel {:e}", rel(all.area(), 4.0 * a));
    assert!(rel(all.area(), 4.0 * a) <= 1e-12);
    let dist = |p: Vec3| ((p[0] - C[0]).powi(2) + (p[1] - C[1]).powi(2)).sqrt();
    let t = Transform::rotation([0.0, 0.0, 1.0], PI / 2.0, C).unwrap();
    let rs = s.transformed(&t);
    let worst = s
        .points
        .iter()
        .zip(&rs.points)
        .map(|(a, b)| (dist(*a) - dist(*b)).abs())
        .fold(0.0, f64::max);
    eprintln!("T1 axis distance worst {worst:e}");
    assert!(worst <= 1e-14);
    for i in 0..3 {
        for j in 0..3 {
            let d = vec3::dot(t.rot[i], t.rot[j]) - if i == j { 1.0 } else { 0.0 };
            assert!(d.abs() <= 1e-15);
        }
    }
    let tv = [0.25, -0.5, 1.0];
    let tr = s.transformed(&Transform::translation(tv).unwrap());
    for (a, b) in s.points.iter().zip(&tr.points) {
        assert_eq!(*b, vec3::add(*a, tv));
    }
}

#[test]
fn c1_iso_clip() {
    let m = cube(8);
    let s = cut_plane(&m, [0.5, 0.5, 0.5], [1.0, 0.0, 0.0]).unwrap();
    assert!(
        (s.area() - 1.0).abs() <= 1e-12,
        "face on the plane emitted once: {}",
        s.area()
    );
    let y = cell_field(&m, |p| p[1]);
    let v = s.vertex_scalar(&m, &y).unwrap();
    let (hi, vh) = s.clip_by_values(&v, 0.3, true).unwrap();
    let (lo, _) = s.clip_by_values(&v, 0.3, false).unwrap();
    eprintln!("C1 {} {}", hi.area(), lo.area());
    assert!((hi.area() - 0.7).abs() <= 1e-12);
    assert!((lo.area() - 0.3).abs() <= 1e-12);
    assert!((hi.area() + lo.area() - 1.0).abs() <= 1e-12);
    assert!(vh.iter().all(|x| *x >= 0.3 - 1e-12));
    let (full, _) = s.clip_by_values(&v, -1.0, true).unwrap();
    let (none, _) = s.clip_by_values(&v, 2.0, true).unwrap();
    assert!((full.area() - 1.0).abs() <= 1e-12);
    assert_eq!(none.n_tris(), 0);
}

#[test]
fn d1_identical_solutions_exactly_zero() {
    let m1 = cube(8);
    let m2 = cube(8);
    let f = cell_field(&m1, |p| (3.0 * p[0]).sin() + p[1] * p[2]);
    for m in RemapMethod::ALL {
        let d = difference(&m1, &f, &m1, &f, m).unwrap();
        assert!(d.values.iter().all(|x| x.to_bits() == 0.0f64.to_bits()), "{m:?}");
        assert_eq!(d.max_abs, 0.0);
        let d = difference(&m1, &f, &m2, &f, m).unwrap();
        assert!(
            d.values.iter().all(|x| x.to_bits() == 0.0f64.to_bits()),
            "{m:?} separate mesh"
        );
    }
}

#[test]
fn d1_uniform_offsets_across_meshes() {
    let (a, b) = (3.25, 1.5);
    let mut worst: f64 = 0.0;
    for (na, nb) in [(8, 12), (12, 8), (8, 5), (5, 8), (12, 5), (5, 12)] {
        let (ma, mb) = (cube(na), cube(nb));
        let d = difference(
            &ma,
            &vec![a; ma.n_cells()],
            &mb,
            &vec![b; mb.n_cells()],
            RemapMethod::LeastSquares,
        )
        .unwrap();
        let w = d
            .values
            .iter()
            .map(|x| (x - (b - a)).abs() / (b - a).abs())
            .fold(0.0, f64::max);
        eprintln!("D1 uniform {na}->{nb} worst rel {:e} ({:.1} eps)", w, w / f64::EPSILON);
        worst = worst.max(w);
    }
    assert!(worst <= 64.0 * f64::EPSILON);
}

#[test]
fn d1_linear_plus_offset_across_meshes() {
    let lin = |p: Vec3| 1.0 + 2.0 * p[0] - p[1] + 3.0 * p[2];
    for method in [RemapMethod::LeastSquares, RemapMethod::ConservativeOverlap] {
        for (na, nb) in [(8, 12), (12, 8)] {
            let (ma, mb) = (cube(na), cube(nb));
            let fa = cell_field(&ma, |p| lin(p) + 7.0);
            let fb = cell_field(&mb, |p| lin(p) + 2.5);
            let d = difference(&ma, &fa, &mb, &fb, method).unwrap();
            let w = d.values.iter().map(|x| (x - (2.5 - 7.0)).abs()).fold(0.0, f64::max);
            eprintln!("D1 linear {method:?} {na}->{nb} worst {w:e}");
            assert!(w <= 1e-11, "{method:?} {na}->{nb}: {w:e}");
        }
    }
    // vector
    let (ma, mb) = (cube(8), cube(12));
    let va: Vec<Vec3> = ma
        .cell_centres()
        .iter()
        .map(|p| [p[0] + 1.0, 2.0 * p[1], p[2] - 3.0])
        .collect();
    let vb: Vec<Vec3> = mb
        .cell_centres()
        .iter()
        .map(|p| [p[0] + 4.0, 2.0 * p[1], p[2] - 1.0])
        .collect();
    let d = difference_vector(&ma, &va, &mb, &vb, RemapMethod::LeastSquares).unwrap();
    let off = [3.0, 0.0, 2.0];
    for k in 0..3 {
        let w = d[k].values.iter().map(|x| (x - off[k]).abs()).fold(0.0, f64::max);
        assert!(w <= 1e-11, "component {k}: {w:e}");
    }
}

fn msg<T: std::fmt::Debug>(r: rcfd_core::Result<T>) -> String {
    format!("{}", r.expect_err("expected an error"))
}

#[test]
fn bad_inputs_are_located_errors() {
    let m = cube(4);
    assert!(msg(weighted_stats(&[1.0, 2.0], &[1.0, -1.0])).contains("weights[1]"));
    assert!(msg(weighted_stats(&[1.0, 2.0], &[1.0])).contains("values has 2"));
    assert!(msg(weighted_stats(&[], &[])).contains("values is empty"));
    assert!(msg(weighted_stats(&[f64::NAN], &[1.0])).contains("values[0]"));
    assert!(msg(weighted_stats(&[1.0], &[0.0])).contains("sum to zero"));
    assert!(msg(normal_flux(&m, "nope", &[])).contains("nope"));
    assert!(msg(patch_stats(&m, "nope", &[])).contains("nope"));
    assert!(msg(project_force([1.0, 0.0, 0.0], [0.0; 3])).contains("dir"));
    assert!(msg(Quadric::sphere(C, 0.0)).contains("radius"));
    assert!(msg(Quadric::plane(C, [0.0; 3])).contains("normal"));
    assert!(msg(Quadric::cone(C, [0.0, 0.0, 1.0], 2.0)).contains("half_angle"));
    assert!(msg(Quadric::cone(C, [0.0, 0.0, 1.0], 0.0)).contains("half_angle"));
    assert!(msg(Quadric::cylinder(C, [0.0; 3], 1.0)).contains("axis"));
    assert!(msg(PointCloud::rake(C, [0.9; 3], 1)).contains("n = 1"));
    assert!(msg(PointCloud::rake(C, C, 5)).contains("coincide"));
    let s = cut_plane(&m, C, [1.0, 0.0, 0.0]).unwrap();
    assert!(msg(s.clip_by_values(&[0.0], 0.0, true)).contains("vertex_values"));
    assert!(msg(s.rotational_copies(0, [0.0, 0.0, 1.0], C)).contains("n_copies"));
    assert!(msg(Transform::rotation([0.0; 3], 1.0, C)).contains("axis"));
    let f = vec![1.0; m.n_cells()];
    let mut g = f.clone();
    g[3] = f64::NAN;
    assert!(msg(difference(&m, &f, &m, &g, RemapMethod::Nearest)).contains("values[3]"));
    assert!(msg(difference(&m, &f[..5], &m, &f, RemapMethod::Nearest)).contains("ref_values"));
    assert!(msg(s.facet_scalar(&m, &f[..2])).contains("cell_values"));
}
