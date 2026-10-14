//! Registers, statistics and adaption marks on analytic cases (ADR 0011 section 3, criteria R1-R5, R8-R10).

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_post::registers::{face_jump_indicator, mark_by_jump, stats, stats_component, Register};

fn cube(n: usize) -> Mesh {
    box_mesh([n; 3], [1.0; 3], [Grading::Uniform; 3]).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

fn xs(m: &Mesh) -> Vec<f64> {
    m.cell_centres().iter().map(|c| c[0]).collect()
}

fn central_box(m: &Mesh) -> Register {
    Register::in_box(m, [0.25; 3], [0.75; 3]).unwrap()
}

#[test]
fn r1_box() {
    let m = cube(20);
    let r = central_box(&m);
    assert_eq!(r.count(), 1000);
    let e = rel(r.volume(&m), 0.125);
    println!("R1 box volume rel err {e:.2e}");
    assert!(e <= 1e-12);
}

#[test]
fn r2_threshold_and_stats() {
    let m = cube(20);
    let x = xs(&m);
    let r = Register::threshold(&m, &x, 0.5, f64::INFINITY).unwrap();
    assert_eq!(r.count(), 4000);
    assert!(rel(r.volume(&m), 0.5) <= 1e-12);
    let s = stats(&m, &r, &x).unwrap().unwrap();
    let all = stats(&m, &Register::all(&m), &x).unwrap().unwrap();
    println!(
        "R2 mean {:.15} (0.75) integral(all) {:.15} (0.5) min {:.15} max {:.15}",
        s.mean, all.integral, s.min, s.max
    );
    assert!(rel(s.mean, 0.75) <= 1e-12);
    assert!(rel(all.integral, 0.5) <= 1e-12);
    assert!(rel(s.min, 0.525) <= 1e-12);
    assert!(rel(s.max, 0.975) <= 1e-12);
    assert_eq!(s.count, 4000);
    // limits are inclusive: the smallest value of the region is selected by lo = hi = min (the centre coordinates of
    // different cells of one column are not bit identical, so only a lower bound on the count is asserted)
    let r2 = Register::threshold(&m, &x, s.min, s.min).unwrap();
    assert!(r2.count() >= 1);
}

#[test]
fn r3_sphere_by_centre_function() {
    let m = cube(40);
    let r = Register::from_centre_fn(&m, |c| {
        (c[0] - 0.5).powi(2) + (c[1] - 0.5).powi(2) + (c[2] - 0.5).powi(2) <= 0.3 * 0.3
    });
    let exact = 4.0 / 3.0 * std::f64::consts::PI * 0.3f64.powi(3);
    let e = rel(r.volume(&m), exact);
    println!(
        "R3 sphere volume {:.6} exact {:.6} rel err {:.3e} (limit 3e-2)",
        r.volume(&m),
        exact,
        e
    );
    assert!(e <= 0.03);
}

#[test]
fn r4_boundary_adjacent() {
    let m = cube(20);
    let l1 = Register::boundary_adjacent(&m, &["xmin"], 1).unwrap();
    let l2 = Register::boundary_adjacent(&m, &["xmin"], 2).unwrap();
    let all6 = Register::boundary_adjacent(&m, &["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"], 1).unwrap();
    assert_eq!((l1.count(), l2.count(), all6.count()), (400, 800, 2168));
    let a = l1.interface_area(&m);
    println!("R4 interface area {a:.15} (1.0)");
    assert!(rel(a, 1.0) <= 1e-12);
}

#[test]
fn r5_algebra() {
    let m = cube(20);
    let a = central_box(&m);
    let x = xs(&m);
    let b = Register::threshold(&m, &x, 0.5, f64::INFINITY).unwrap();
    let (u, i) = (a.union(&b).unwrap(), a.intersection(&b).unwrap());
    assert_eq!(u.count() + i.count(), a.count() + b.count());
    assert_eq!(a.complement().count(), 8000 - a.count());
    assert_eq!(a.difference(&b).unwrap().count(), a.count() - i.count());
    // the registered grow/shrink numbers (1728 and 1000) hold for point-neighbour dilation; the face-neighbour
    // dilation named in ADR 0011 gives 1000 + 6 * 100 = 1600 (see `r5_face_grow_known_miss`)
    let g = a.grow_points(&m, 1);
    assert_eq!(g.count(), 1728);
    assert_eq!(g.shrink_points(&m, 1).count(), 1000);
    assert_eq!(g.shrink_points(&m, 1), a);
    let gf = a.grow(&m, 1);
    assert_eq!(gf.count(), 1600);
    assert_eq!(gf.shrink(&m, 1), a);
    // empty and full registers are handled
    assert!(a.difference(&a).unwrap().is_empty());
    assert_eq!(a.difference(&a).unwrap().grow(&m, 3).count(), 0);
}

#[test]
fn r6_patch_field_changes_only_the_register() {
    let m = cube(20);
    let r = central_box(&m);
    let base: Vec<f64> = (0..m.n_cells()).map(|i| (i as f64) * 0.1 + 0.37).collect();
    let out = r.patch_field(&base, 42.0).unwrap();
    let changed = out
        .iter()
        .zip(&base)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    assert_eq!(changed, 1000);
    for (c, ((o, b), &m)) in out.iter().zip(&base).zip(r.mask()).enumerate() {
        if m {
            assert_eq!(*o, 42.0, "cell {c}");
        } else {
            assert_eq!(o.to_bits(), b.to_bits(), "cell {c}");
        }
    }
}

#[test]
fn r8_adaption_marks() {
    let m = cube(20);
    let step: Vec<f64> = xs(&m).iter().map(|&x| if x < 0.5 { 0.0 } else { 1.0 }).collect();
    let mk = mark_by_jump(&m, &step, 0.5, None).unwrap();
    println!("R8 step marks {} volume fraction {:.6}", mk.count, mk.volume_fraction);
    assert_eq!(mk.count, 800);
    assert!(rel(mk.volume_fraction, 0.1) <= 1e-12);
    // marked cells are exactly the two columns i = 9, 10 (centres 0.475 and 0.525)
    let x = xs(&m);
    for (c, &flag) in mk.cells.iter().enumerate() {
        assert_eq!(flag, (x[c] - 0.5).abs() < 0.03, "cell {c}");
    }
    let lin = xs(&m);
    assert_eq!(mark_by_jump(&m, &lin, 0.5, None).unwrap().count, 0);
    // restricted to a register: only register cells are marked
    let reg = Register::in_box(&m, [0.0, 0.0, 0.0], [1.0, 0.5, 1.0]).unwrap();
    let mr = mark_by_jump(&m, &step, 0.5, Some(&reg)).unwrap();
    assert_eq!(mr.count, 400);
    assert!(mr.cells.iter().zip(reg.mask()).all(|(&a, &b)| !a || b));
    // the indicator itself: largest jump is 1 at the step, 0.05 for the linear field
    let ind = face_jump_indicator(&m, &lin).unwrap();
    assert!(ind.iter().all(|&i| (i - 0.05).abs() < 1e-12));
}

#[test]
fn r9_vector_and_empty_stats() {
    let m = cube(20);
    let v: Vec<[f64; 3]> = m.cell_centres().iter().map(|c| [c[0], 2.0 * c[1], -1.0]).collect();
    let r = central_box(&m);
    let sx = stats_component(&m, &r, &v, 0).unwrap().unwrap();
    let sy = stats_component(&m, &r, &v, 1).unwrap().unwrap();
    assert!(rel(sx.mean, 0.5) <= 1e-12);
    assert!(rel(sy.mean, 1.0) <= 1e-12);
    let none = Register::in_box(&m, [5.0; 3], [6.0; 3]).unwrap();
    assert!(stats_component(&m, &none, &v, 0).unwrap().is_none());
    assert!(stats(&m, &none, &xs(&m)).unwrap().is_none());
    assert!(stats_component(&m, &r, &v, 3).is_err());
}

#[test]
fn r10_errors_are_located() {
    let m = cube(4);
    let e = |r: rcfd_core::Result<Register>| r.unwrap_err().to_string();
    assert!(e(Register::in_box(&m, [0.5; 3], [0.5; 3])).contains("box_max"));
    assert!(e(Register::in_box(&m, [f64::NAN; 3], [1.0; 3])).contains("box_min"));
    assert!(e(Register::boundary_adjacent(&m, &["xmin"], 0)).contains("layers"));
    let u = e(Register::boundary_adjacent(&m, &["nope"], 1));
    assert!(u.contains("patches") && u.contains("nope") && u.contains("xmin"), "{u}");
    assert!(e(Register::boundary_adjacent(&m, &[], 1)).contains("patches"));
    assert!(e(Register::cell_zone(&m, "nozone")).contains("nozone"));
    let short = vec![0.0; 3];
    assert!(e(Register::threshold(&m, &short, 0.0, 1.0)).contains("3 values for 64 cells"));
    let mut vals = vec![0.5; 64];
    vals[17] = f64::NAN;
    let n = e(Register::threshold(&m, &vals, 0.0, 1.0));
    assert!(n.contains("cell 17"), "{n}");
    assert!(stats(&m, &Register::all(&m), &short).is_err());
    assert!(mark_by_jump(&m, &vec![0.0; 64], 0.0, None).is_err());
    assert!(mark_by_jump(&m, &short, 0.5, None).is_err());
    let other = Register::from_mask(vec![true; 5]);
    assert!(Register::all(&m).union(&other).is_err());
}

/// The literal wording of ADR 0011 R5 (face-neighbour dilation of the 10^3 block gives 12^3 = 1728 cells) is wrong
/// arithmetic for face neighbours: the dilation adds six 10x10 slabs (1600). Kept as a recorded miss; the 1728 / 1000
/// numbers are met by point-neighbour dilation (`r5_algebra`).
#[test]
#[ignore = "known miss: face-neighbour grow gives 1600, not 1728 (ADR 0011 R5 wording)"]
fn r5_face_grow_known_miss() {
    let m = cube(20);
    let a = central_box(&m);
    assert_eq!(a.grow(&m, 1).count(), 1728);
}
