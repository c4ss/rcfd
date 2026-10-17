//! Wave 27C (ADR 0051) registered checks R3-R6: one-way structural load export.
use rcfd_io::nastran::Deck;
use rcfd_mesh::gen::{annulus_3d, box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_post::remap::{RemapMethod, Remapper};
use rcfd_post::structural::*;

const U: [Grading; 3] = [Grading::Uniform; 3];

type Case = (Mesh, Vec<f64>, ShellSurface, Vec<Vec<(usize, f64)>>);

fn plane_case(nu: usize, nv: usize) -> Case {
    let m = box_mesh([8, 8, 1], [1.0; 3], U).unwrap();
    let rects = patch_rects_plane(&m, "zmax", [0, 1]).unwrap();
    let p: Vec<f64> = m
        .patch("zmax")
        .unwrap()
        .faces()
        .map(|f| {
            let c = m.face_centres()[f];
            1.0 + 2.0 * c[0] + 3.0 * c[1]
        })
        .collect();
    let s = shell_plane(
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0],
        [0.0, 1.0],
        nu,
        nv,
        true,
    )
    .unwrap();
    let w = overlap_weights(&rects, &s.rects, None).unwrap();
    (m, p, s, w)
}

fn tube_case(nth_f: usize, nz_f: usize, nth_s: usize, nz_s: usize, uniform: Option<f64>) -> Case {
    let m = annulus_3d(nth_f, 1, nz_f, 0.02, 0.05, 1.0, 0.0, Grading::Uniform).unwrap();
    let rects = patch_rects_cylinder(&m, "outer").unwrap();
    let p: Vec<f64> = m
        .patch("outer")
        .unwrap()
        .faces()
        .map(|f| {
            let c = m.face_centres()[f];
            let th = c[1].atan2(c[0]);
            uniform.unwrap_or(1e5 * (1.0 + 0.5 * th.cos() + 0.3 * c[2]))
        })
        .collect();
    let s = shell_tube(0.05, 0.0, 1.0, nth_s, nz_s).unwrap();
    let w = overlap_weights(&rects, &s.rects, Some(2.0 * std::f64::consts::PI)).unwrap();
    (m, p, s, w)
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn r3_uniform_pressure_is_exact() {
    let p0 = 3.7e5;
    let (m, _, _, _) = plane_case(5, 7);
    let n = m.patch("zmax").unwrap().size;
    let rects = patch_rects_plane(&m, "zmax", [0, 1]).unwrap();
    let mut worst = 0.0f64;
    for (nu, nv) in [(5, 7), (13, 11), (8, 8)] {
        let s = shell_plane(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0],
            [0.0, 1.0],
            nu,
            nv,
            true,
        )
        .unwrap();
        let w = overlap_weights(&rects, &s.rects, None).unwrap();
        for v in map_face_field(&w, &vec![p0; n]).unwrap() {
            worst = worst.max((v - p0).abs() / p0);
        }
    }
    let (_, pt, _, wt) = tube_case(96, 24, 64, 20, Some(p0));
    for v in map_face_field(&wt, &pt).unwrap() {
        worst = worst.max((v - p0).abs() / p0);
    }
    eprintln!(
        "W27C R3 uniform pressure worst relative deviation {worst:.3e} (limit 4 eps = {:.3e})",
        4.0 * f64::EPSILON
    );
    assert!(worst <= 4.0 * f64::EPSILON);
}

#[test]
fn r4_force_and_moment_preserved() {
    for (nu, nv) in [(5, 7), (13, 11)] {
        let (m, p, s, w) = plane_case(nu, nv);
        let pe = map_face_field(&w, &p).unwrap();
        let deck = shell_deck(&s, &pe, 1).unwrap();
        // through the text form: write, parse, sum
        let back = Deck::parse(&deck.to_string_checked().unwrap()).unwrap();
        let (f_f, m_f) = fluid_force_moment(&m, "zmax", &p, [0.0; 3]).unwrap();
        let (f_d, m_d) = deck_force_moment(&back, 1, [0.0; 3]).unwrap();
        let big_s: f64 = p
            .iter()
            .zip(m.patch("zmax").unwrap().faces())
            .map(|(v, f)| v.abs() * m.face_area(f))
            .sum();
        let ef = dist(f_f, f_d) / big_s;
        let em = dist(m_f, m_d) / big_s;
        eprintln!("W27C R4 plane {nu}x{nv}: force {ef:.3e} (limit 1e-12), moment {em:.3e} (limit 5e-3)");
        assert!(ef <= 1e-12 && em <= 5e-3);
    }
    let (m, p, s, w) = tube_case(96, 24, 64, 20, None);
    let pe = map_face_field(&w, &p).unwrap();
    let deck = shell_deck(&s, &pe, 3).unwrap();
    let back = Deck::parse(&deck.to_string_checked().unwrap()).unwrap();
    let about = [0.0, 0.0, 0.5];
    let (f_f, m_f) = fluid_force_moment(&m, "outer", &p, about).unwrap();
    let (f_d, m_d) = deck_force_moment(&back, 3, about).unwrap();
    let big_s: f64 = p
        .iter()
        .zip(m.patch("outer").unwrap().faces())
        .map(|(v, f)| v.abs() * m.face_area(f))
        .sum();
    let ef = dist(f_f, f_d) / big_s;
    let em = dist(m_f, m_d) / (big_s * 0.05);
    eprintln!("W27C R4 tube 96x24 -> 64x20: |dF|/S {ef:.3e} (limit 1e-3), |dM|/(S R) {em:.3e} (limit 5e-3); F_fluid {f_f:?} F_deck {f_d:?}");
    assert!(ef <= 1e-3 && em <= 5e-3);
}

fn lin(x: [f64; 3]) -> f64 {
    1.0 + 2.0 * x[0] - x[1] + 3.0 * x[2]
}

#[test]
fn r5_linear_temperature_exact_on_structural_nodes() {
    let fluid = box_mesh([4, 4, 4], [1.0; 3], U).unwrap();
    let t: Vec<f64> = fluid.cell_centres().iter().map(|&c| lin(c)).collect();
    for dims in [[8, 8, 8], [6, 6, 6], [5, 7, 3]] {
        let st = box_mesh(dims, [1.0; 3], U).unwrap();
        for method in [RemapMethod::LeastSquares, RemapMethod::ConservativeOverlap] {
            let nodal = temperature_at_nodes(&fluid, &t, &st, method).unwrap();
            let worst = st
                .points()
                .iter()
                .zip(&nodal)
                .map(|(x, v)| (lin(*x) - v).abs())
                .fold(0.0f64, f64::max);
            eprintln!("W27C R5 {dims:?} {method:?}: max nodal error {worst:.3e} (limit 1e-11)");
            assert!(worst <= 1e-11, "{dims:?} {method:?}");
        }
        let r = Remapper::new(&fluid, &st, RemapMethod::ConservativeOverlap).unwrap();
        let tc = r.scalar(&t).unwrap();
        let i_src: f64 = t.iter().zip(fluid.cell_volumes()).map(|(a, v)| a * v).sum();
        let i_tgt: f64 = tc.iter().zip(st.cell_volumes()).map(|(a, v)| a * v).sum();
        eprintln!(
            "W27C R5 {dims:?}: integral of T conserved to {:.3e} (limit 1e-12)",
            (i_src - i_tgt).abs() / i_src
        );
        assert!((i_src - i_tgt).abs() / i_src <= 1e-12);
        // deck round trip
        let nodal = temperature_at_nodes(&fluid, &t, &st, RemapMethod::LeastSquares).unwrap();
        let deck = temperature_deck(&st, &nodal, 2).unwrap();
        let back = Deck::parse(&deck.to_string_checked().unwrap()).unwrap();
        assert_eq!(back.temps, deck.temps);
    }
}

#[test]
fn r6_located_errors() {
    let (m, p, s, w) = plane_case(5, 7);
    let rects = patch_rects_plane(&m, "zmax", [0, 1]).unwrap();
    assert!(fluid_force_moment(&m, "nowhere", &p, [0.0; 3])
        .unwrap_err()
        .contains("nowhere"));
    assert!(fluid_force_moment(&m, "zmax", &p[..3], [0.0; 3])
        .unwrap_err()
        .contains("zmax"));
    let mut bad = p.clone();
    bad[4] = f64::NAN;
    assert!(map_face_field(&w, &bad).unwrap_err().contains("non-finite"));
    assert!(map_face_field(&w, &p[..10]).unwrap_err().contains("source face"));
    let degenerate = vec![ParamRect {
        u: [0.5, 0.5],
        v: [0.0, 1.0],
    }];
    assert!(overlap_weights(&rects, &degenerate, None)
        .unwrap_err()
        .contains("element 0"));
    let far = vec![ParamRect {
        u: [5.0, 6.0],
        v: [5.0, 6.0],
    }];
    assert!(overlap_weights(&rects, &far, None)
        .unwrap_err()
        .contains("zero total weight"));
    let partly = vec![ParamRect {
        u: [0.5, 1.5],
        v: [0.0, 1.0],
    }];
    assert!(overlap_weights(&rects, &partly, None)
        .unwrap_err()
        .contains("covered only"));
    assert!(shell_plane(
        [0.0; 3],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 1.0],
        [0.0, 1.0],
        0,
        3,
        false
    )
    .unwrap_err()
    .contains("2 grid lines"));
    assert!(shell_deck(&s, &[1.0], 1).unwrap_err().contains("element pressures"));
    // structural element referencing a missing node
    let mut deck = shell_deck(&s, &vec![1.0; s.quads.len()], 1).unwrap();
    deck.elements[0].nodes[2] = 99999;
    assert!(deck
        .to_string_checked()
        .unwrap_err()
        .to_string()
        .contains("missing grid 99999"));
    let fluid = box_mesh([2, 2, 2], [1.0; 3], U).unwrap();
    assert!(
        temperature_at_nodes(&fluid, &[1.0; 3], &fluid, RemapMethod::LeastSquares)
            .unwrap_err()
            .contains("fluid cells")
    );
}
