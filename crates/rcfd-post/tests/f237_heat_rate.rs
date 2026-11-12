//! F-237 (ADR 0244) registered checks H1 to H5, H7, NC1, NC2: heat-rate loads on a tube and a plane, exported as `RCFDQ`
//! cards, re-imported, and compared with the fluid-side heat rate.
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::{FvGeometry, ScalarField};
use rcfd_io::nastran::{parse_with_heat_flux, write_with_heat_flux};
use rcfd_mesh::gen::{annulus_3d, box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_post::integrals::conductive_heat_flow;
use rcfd_post::structural::*;
use std::f64::consts::PI;
use std::sync::Arc;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

fn tube_mesh() -> Mesh {
    annulus_3d(96, 1, 24, 0.02, 0.05, 1.0, 0.0, Grading::Uniform).unwrap()
}

fn tube_flux(m: &Mesh, uniform: Option<f64>) -> Vec<f64> {
    m.patch("outer")
        .unwrap()
        .faces()
        .map(|f| {
            let c = m.face_centres()[f];
            let th = c[1].atan2(c[0]);
            uniform.unwrap_or(2e4 * (1.0 + 0.5 * th.cos() + 0.3 * c[2]))
        })
        .collect()
}

struct Mapped {
    consistent: Vec<f64>,
    conservative: Vec<f64>,
}

fn map_both(m: &Mesh, patch: &str, src: &[ParamRect], s: &ShellSurface, q: &[f64], period: Option<f64>) -> Mapped {
    let w = overlap_weights(src, &s.rects, period).unwrap();
    let consistent = map_face_field(&w, q).unwrap();
    let conservative =
        map_heat_flux_conservative(&w, src, &patch_areas(m, patch).unwrap(), &s.rects, &shell_areas(s), q).unwrap();
    Mapped {
        consistent,
        conservative,
    }
}

/// Write the shell deck with pressure set `p_sid` and heat set `q_sid`, parse it back, return the imported heat rate.
fn round_trip_heat(s: &ShellSurface, q_elem: &[f64], q_sid: i64) -> f64 {
    let deck = shell_deck(s, &vec![0.0; s.quads.len()], 1).unwrap();
    let heat = shell_heat_flux(s, q_elem, q_sid).unwrap();
    let (back, hb) = parse_with_heat_flux(&write_with_heat_flux(&deck, &heat).unwrap()).unwrap();
    deck_heat_rate(&back, &hb, q_sid).unwrap()
}

#[test]
fn h1_h2_h7_tube_heat_rate_and_force_after_round_trip() {
    let m = tube_mesh();
    let q = tube_flux(&m, None);
    let src = patch_rects_cylinder(&m, "outer").unwrap();
    let s = shell_tube(0.05, 0.0, 1.0, 64, 20).unwrap();
    let mp = map_both(&m, "outer", &src, &s, &q, Some(2.0 * PI));
    let q_fluid = fluid_heat_rate(&m, "outer", &q).unwrap();

    // H1 and H7 through ONE deck: pressure loads (set 3) and heat-flux loads (set 4)
    let p: Vec<f64> = m
        .patch("outer")
        .unwrap()
        .faces()
        .map(|f| {
            let c = m.face_centres()[f];
            1e5 * (1.0 + 0.5 * c[1].atan2(c[0]).cos() + 0.3 * c[2])
        })
        .collect();
    let w = overlap_weights(&src, &s.rects, Some(2.0 * PI)).unwrap();
    let pe = map_face_field(&w, &p).unwrap();
    let deck = shell_deck(&s, &pe, 3).unwrap();
    let heat = shell_heat_flux(&s, &mp.consistent, 4).unwrap();
    let (back, hb) = parse_with_heat_flux(&write_with_heat_flux(&deck, &heat).unwrap()).unwrap();
    let q_deck = deck_heat_rate(&back, &hb, 4).unwrap();
    let e1 = rel(q_deck, q_fluid);
    let about = [0.0, 0.0, 0.5];
    let (f_f, _) = fluid_force_moment(&m, "outer", &p, about).unwrap();
    let (f_d, _) = deck_force_moment(&back, 3, about).unwrap();
    let big_s: f64 = p
        .iter()
        .zip(m.patch("outer").unwrap().faces())
        .map(|(v, f)| v.abs() * m.face_area(f))
        .sum();
    let df = ((f_f[0] - f_d[0]).powi(2) + (f_f[1] - f_d[1]).powi(2) + (f_f[2] - f_d[2]).powi(2)).sqrt() / big_s;

    // H2 conservative
    let q_deck2 = round_trip_heat(&s, &mp.conservative, 5);
    let e2 = rel(q_deck2, q_fluid);
    eprintln!(
        "F237 H1 tube consistent heat rate rel {e1:.4e} (limit 1e-3; fluid {q_fluid:.10e} W, deck {q_deck:.10e} W); \
         H2 conservative rel {e2:.3e} (limit 1e-12); H7 |dF|/S {df:.3e} (limit 1e-3)"
    );
    assert!(e1 <= 1e-3, "H1");
    assert!(e2 <= 1e-12, "H2");
    assert!(df <= 1e-3, "H7");
}

#[test]
fn h3_uniform_heat_flux() {
    let q0 = 1.7e4;
    let m = tube_mesh();
    let q = tube_flux(&m, Some(q0));
    let src = patch_rects_cylinder(&m, "outer").unwrap();
    let s = shell_tube(0.05, 0.0, 1.0, 64, 20).unwrap();
    let mp = map_both(&m, "outer", &src, &s, &q, Some(2.0 * PI));
    let w1 = mp.consistent.iter().fold(0.0f64, |a, v| a.max((v / q0 - 1.0).abs()));
    let w2 = mp.conservative.iter().fold(0.0f64, |a, v| a.max((v / q0 - 1.0).abs()));
    eprintln!(
        "F237 H3 uniform consistent worst {w1:.3e} (limit 4 eps = {:.3e}); H3b conservative worst {w2:.4e} (limit 1e-3)",
        4.0 * f64::EPSILON
    );
    assert!(w1 <= 4.0 * f64::EPSILON, "H3");
    assert!(w2 <= 1e-3, "H3b");
}

#[test]
fn h4_conservation_against_the_solver_heat_flow() {
    let m = tube_mesh();
    let g = FvGeometry::new(Arc::new(m.clone())).unwrap();
    let mut t = ScalarField::from_fn("T", &g, |c| 350.0 + 10.0 * c[1].atan2(c[0]).cos());
    let cf = g.mesh.face_centres();
    let vals: Vec<[f64; 3]> = g
        .mesh
        .patch("outer")
        .unwrap()
        .faces()
        .map(|f| {
            let c = cf[f];
            [
                300.0 + 20.0 * (1.0 + 0.5 * c[1].atan2(c[0]).cos() + 0.3 * c[2]),
                0.0,
                0.0,
            ]
        })
        .collect();
    t.set_bc(&g, "outer", Box::new(FixedValue(Profile::PerFace(vals))))
        .unwrap();
    let k = 0.6;
    let solver = conductive_heat_flow(&g, "outer", &t, k).unwrap();
    let q = wall_heat_flux(&g, "outer", &t, k).unwrap();
    let ea = rel(fluid_heat_rate(&m, "outer", &q).unwrap(), solver);
    let src = patch_rects_cylinder(&m, "outer").unwrap();
    let s = shell_tube(0.05, 0.0, 1.0, 64, 20).unwrap();
    let mp = map_both(&m, "outer", &src, &s, &q, Some(2.0 * PI));
    let eb = rel(round_trip_heat(&s, &mp.conservative, 2), solver);
    let ec = rel(round_trip_heat(&s, &mp.consistent, 2), solver);
    eprintln!(
        "F237 H4 solver heat flow {solver:.10e} W: (a) per-face sum rel {ea:.3e} (limit 1e-12), \
         (b) conservative round trip {eb:.3e} (limit 1e-12), (c) consistent round trip {ec:.4e} (limit 1e-3)"
    );
    assert!(ea <= 1e-12, "H4a");
    assert!(eb <= 1e-12, "H4b");
    assert!(ec <= 1e-3, "H4c");
}

#[test]
fn h5_plane_heat_rate() {
    let m = box_mesh([8, 8, 1], [1.0; 3], [Grading::Uniform; 3]).unwrap();
    let q: Vec<f64> = m
        .patch("zmax")
        .unwrap()
        .faces()
        .map(|f| {
            let c = m.face_centres()[f];
            1.0 + 2.0 * c[0] + 3.0 * c[1]
        })
        .collect();
    let src = patch_rects_plane(&m, "zmax", [0, 1]).unwrap();
    let q_fluid = fluid_heat_rate(&m, "zmax", &q).unwrap();
    let mut worst = 0.0f64;
    for (nu, nv) in [(5, 7), (13, 11)] {
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
        let mp = map_both(&m, "zmax", &src, &s, &q, None);
        let a = rel(round_trip_heat(&s, &mp.consistent, 1), q_fluid);
        let b = rel(round_trip_heat(&s, &mp.conservative, 1), q_fluid);
        eprintln!("F237 H5 plane {nu}x{nv}: consistent {a:.3e}, conservative {b:.3e} (limit 1e-12)");
        worst = worst.max(a).max(b);
    }
    assert!(worst <= 1e-12, "H5");
}

#[test]
fn nc1_wrong_radius_shell_misses() {
    let m = tube_mesh();
    let q = tube_flux(&m, None);
    let src = patch_rects_cylinder(&m, "outer").unwrap();
    let s = shell_tube(0.045, 0.0, 1.0, 64, 20).unwrap();
    let mp = map_both(&m, "outer", &src, &s, &q, Some(2.0 * PI));
    let e = rel(
        round_trip_heat(&s, &mp.consistent, 1),
        fluid_heat_rate(&m, "outer", &q).unwrap(),
    );
    eprintln!("F237 NC1 wrong radius 0.045: consistent heat rate rel {e:.4e} (must exceed 1e-3)");
    assert!(e > 1e-3, "NC1");
}

#[test]
fn nc2_partial_coverage_breaks_conservation() {
    let m = tube_mesh();
    let q = tube_flux(&m, None);
    let src = patch_rects_cylinder(&m, "outer").unwrap();
    let s = shell_tube(0.05, 0.0, 0.5, 64, 10).unwrap();
    let mp = map_both(&m, "outer", &src, &s, &q, Some(2.0 * PI));
    let e = rel(
        round_trip_heat(&s, &mp.conservative, 1),
        fluid_heat_rate(&m, "outer", &q).unwrap(),
    );
    eprintln!("F237 NC2 half-length shell: conservative heat rate rel {e:.4e} (must exceed 1e-3)");
    assert!(e > 1e-3, "NC2");
}
