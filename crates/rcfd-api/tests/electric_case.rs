//! Electric potential and Joule heating through the settings tree (F-213, ADR 0035 checks H8, H9). The limits are the ones
//! registered in the ADR before this file was written.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::sim::Simulation;

fn build(text: &str) -> Simulation {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"))
}

fn errors(text: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => match Simulation::new(&c) {
            Err(e) => e,
            Ok(_) => vec![],
        },
    }
}

fn xmin_area(s: &Simulation) -> f64 {
    let sf = s.geom.mesh.face_area_vectors();
    s.geom
        .mesh
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| rcfd_core::vec3::norm(sf[f]))
        .sum()
}

fn has(errs: &[Diagnostic], path: &str) -> bool {
    errs.iter().any(|d| d.path == path)
}

/// Bar of length 1, ends fixed at 300 K, `k = 2` (H4) or 1 (H5), `rho cp = 1`, electric part supplied by the caller.
fn bar(n: usize, k: f64, electric: &str, left: &str, right: &str) -> String {
    format!(
        r#"
title = "joule bar"
[mesh]
generator = "box_2d"
nx = {n}
ny = 1
lx = 1.0
ly = 1.0
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = {k}
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 300.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
{left}
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
{right}
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simple"
convection = "upwind"
frozen_flow = true
tolerance_u = 1e-13
max_iterations = 200
[electric]
enabled = true
{electric}
"#
    )
}

fn parabola(n: usize) -> (f64, f64) {
    let text = bar(
        n,
        2.0,
        "conductivity = 100.0",
        "electric = \"potential\"\nelectric_potential = 2.0",
        "electric = \"potential\"\nelectric_potential = 0.0",
    );
    let mut s = build(&text);
    s.run(200);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let t = s.field("T").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let err = (0..t.len())
        .map(|i| (t[i] - (300.0 + 100.0 * cc[i][0] * (1.0 - cc[i][0]))).abs())
        .fold(0.0, f64::max)
        / 25.0;
    // current into the domain through xmin from the boundary current density
    let jd = s.boundary_field("electric_current_density").unwrap();
    let p = s.geom.mesh.patch("xmin").unwrap();
    let off = p.faces().next().unwrap() - s.geom.n_internal;
    let sf = s.geom.mesh.face_area_vectors();
    let i_in: f64 = p
        .faces()
        .zip(&jd[off..off + p.faces().count()])
        .map(|(f, j)| j * rcfd_core::vec3::norm(sf[f]))
        .sum();
    let i_exact = 200.0 * xmin_area(&s);
    let ei = (i_in - i_exact).abs() / i_exact;
    println!("H8 parabola N={n}: error {err:.4e}, current {i_in:.9} vs {i_exact} (rel {ei:.3e})");
    (err, ei)
}

#[test]
fn h8_joule_parabola_through_the_tree() {
    let r: Vec<_> = [20, 40, 80].iter().map(|&n| parabola(n)).collect();
    let (o1, o2) = ((r[0].0 / r[1].0).log2(), (r[1].0 / r[2].0).log2());
    println!("H8 orders {o1:.3} {o2:.3}");
    assert!(r[0].0 <= 1e-2);
    assert!(o1 >= 1.8 && o2 >= 1.8);
    assert!(r.iter().all(|x| x.1 <= 1e-9), "current {r:?}");
}

fn sigma_t(n: usize) -> (f64, f64) {
    let (a, rho0, j, k) = (0.1_f64, 1.0_f64, 4.0_f64, 1.0_f64);
    // the 2D mesh depth is part of the patch area: prescribe the total current J * A for the density J = 4 A/m^2
    let area = xmin_area(&build(&bar(
        n,
        k,
        "conductivity = 1.0",
        "electric = \"potential\"\nelectric_potential = 1.0",
        "electric = \"potential\"\nelectric_potential = 0.0",
    )));
    let text = bar(
        n,
        k,
        "conductivity = 1.0\ntemperature_coefficient = 0.1\nreference_temperature = 300.0",
        &format!("electric = \"current\"\nelectric_current = {}", j * area),
        "electric = \"potential\"\nelectric_potential = 0.0",
    );
    let mut s = build(&text);
    s.run(200);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let m = (a * rho0 * j * j / k).sqrt();
    let theta = |x: f64| (((m * (x - 0.5)).cos() / (m * 0.5).cos()) - 1.0) / a;
    let t = s.field("T").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let err = (0..t.len())
        .map(|i| (t[i] - 300.0 - theta(cc[i][0])).abs())
        .fold(0.0, f64::max)
        / theta(0.5);
    let v = 2.0 * rho0 * j * (m * 0.5).tan() / m;
    let bp = s.boundary_field("electric_potential").unwrap();
    let p = s.geom.mesh.patch("xmin").unwrap();
    let off = p.faces().next().unwrap() - s.geom.n_internal;
    let nb = p.faces().count();
    let vn = bp[off..off + nb].iter().sum::<f64>() / nb as f64;
    let ev = (vn - v).abs() / v;
    println!("H8 sigma(T) N={n}: error/peak {err:.4e}, voltage {vn:.6} vs {v:.6} (rel {ev:.3e})");
    (err, ev)
}

#[test]
fn h8_sigma_of_t_through_the_tree() {
    let r: Vec<_> = [20, 40, 80].iter().map(|&n| sigma_t(n)).collect();
    let (o1, o2) = ((r[0].0 / r[1].0).log2(), (r[1].0 / r[2].0).log2());
    println!("H8 sigma(T) orders {o1:.3} {o2:.3}");
    assert!(r[1].0 <= 5e-3 && r[1].1 <= 5e-3);
    assert!(o1 >= 1.8 && o2 >= 1.8);
}

#[test]
fn h9_located_errors() {
    let ok_l = "electric = \"potential\"\nelectric_potential = 2.0";
    let ok_r = "electric = \"potential\"\nelectric_potential = 0.0";
    let cases: Vec<(String, &str)> = vec![
        (bar(10, 1.0, "", ok_l, ok_r), "electric.conductivity"),
        (
            bar(10, 1.0, "conductivity = 1.0\ntemperature_coefficient = 0.1", ok_l, ok_r),
            "electric.reference_temperature",
        ),
        (
            bar(10, 1.0, "conductivity = 1.0", "electric = \"current\"\nelectric_current = 1.0", "electric = \"current\"\nelectric_current = 1.0"),
            "electric",
        ),
        (bar(10, 1.0, "conductivity = 1.0", "electric = \"potential\"", ok_r), "boundaries.xmin.electric_potential"),
        (bar(10, 1.0, "conductivity = 1.0", "electric = \"current\"", ok_r), "boundaries.xmin.electric_current"),
        (
            bar(10, 1.0, "conductivity = 1.0", "electric_potential = 1.0", ok_r),
            "boundaries.xmin.electric_potential",
        ),
        (
            bar(10, 1.0, "conductivity = 1.0", "electric = \"insulated\"\nelectric_current = 1.0", ok_r),
            "boundaries.xmin.electric_current",
        ),
        (
            bar(10, 1.0, "conductivity = 1.0", "electric = \"potential\"\nelectric_potential = 1.0\nelectric_current = 2.0", ok_r),
            "boundaries.xmin.electric_current",
        ),
        (
            bar(10, 1.0, "conductivity = 1.0", ok_l, ok_r).replace("frozen_flow = true", "frozen_flow = false"),
            "electric.enabled",
        ),
        (
            bar(10, 1.0, "conductivity = 1.0", ok_l, ok_r).replace("[energy]\nenabled = true", "[energy]\nenabled = false"),
            "electric.enabled",
        ),
        (
            format!(
                "{}\n[solids.s]\nzone = \"all\"\nconductivity = 1.0\ndensity = 1.0\nspecific_heat = 1.0\nelectrical_conductivity = -2.0\n",
                bar(10, 1.0, "conductivity = 1.0", ok_l, ok_r)
            ),
            "solids.s.electrical_conductivity",
        ),
        (
            bar(10, 1.0, "conductivity = 1.0", ok_l, ok_r).replace("[electric]\nenabled = true", "[electric]\nenabled = false"),
            "boundaries.xmin.electric",
        ),
    ];
    for (i, (text, path)) in cases.iter().enumerate() {
        let e = errors(text);
        assert!(has(&e, path), "case {i}: expected {path}, got {e:?}");
    }
    // the valid case builds
    assert!(errors(&bar(10, 1.0, "conductivity = 1.0", ok_l, ok_r)).is_empty());
}

#[test]
fn h9_nonpositive_resistivity_stops_the_run_with_a_located_message() {
    // a = -1 / K and T_ref = 300 K: 1 + a (T - T_ref) <= 0 above 301 K, reached by the Joule heating
    let text = bar(
        10,
        1.0,
        "conductivity = 1.0\ntemperature_coefficient = -1.0\nreference_temperature = 300.0",
        "electric = \"potential\"\nelectric_potential = 40.0",
        "electric = \"potential\"\nelectric_potential = 0.0",
    );
    let mut s = build(&text);
    s.run(200);
    let f = s.failure().expect("run must fail");
    println!("H9 failure message: {f}");
    assert!(f.contains("electric.temperature_coefficient"), "{f}");
}
