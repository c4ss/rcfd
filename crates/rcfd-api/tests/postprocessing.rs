//! Derived fields and sampling through the case tree (ADR 0021, checks P1 to P8, P10, P11).

mod wiring_common;

use rcfd_api::case::Case;
use rcfd_api::postproc::{self, SampleKind, SampleSpec};
use rcfd_api::runctl::write_results;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_fv::gradient::least_squares;
use rcfd_mesh::PatchKind;
use rcfd_post::derived;
use serde_json::json;
use wiring_common::*;

const BCS: &str = "[boundaries.xmin]\nvelocity = \"fixed\"\n[boundaries.xmax]\nvelocity = \"fixed\"\n[boundaries.ymin]\nvelocity = \"fixed\"\n[boundaries.ymax]\nvelocity = \"fixed\"\n";

/// Linear velocity field on a 12 x 10 box, no iteration: u = x + 2y, v = 3x - y.
fn linear(extra: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"box_2d\"\nnx = 12\nny = 10\nlx = 4.0\nly = 1.0\n[physics]\nnu = 0.05\nrho = 1.2\n[initial]\nvelocity_expression_x = \"1 [1/s] * x + 2 [1/s] * y\"\nvelocity_expression_y = \"3 [1/s] * x - 1 [1/s] * y\"\n{BCS}[solver]\nalgorithm = \"simplec\"\nmax_iterations = 5\n{extra}"
    )
}

fn sim_of(text: &str) -> Simulation {
    let c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"))
}

fn export(sim: &Simulation, name: &str) -> String {
    let dir = tmpdir(name);
    std::fs::read_to_string(sim.export_vtu(&dir, "solution").unwrap()).unwrap()
}

/// Cells that own no face of a non-empty boundary patch.
fn interior_cells(sim: &Simulation) -> Vec<usize> {
    let m = &sim.geom.mesh;
    let mut touched = vec![false; m.n_cells()];
    for p in m.patches() {
        if p.kind != PatchKind::Empty {
            for f in p.faces() {
                touched[m.owner()[f]] = true;
            }
        }
    }
    (0..m.n_cells()).filter(|&c| !touched[c]).collect()
}

fn gradient_of(sim: &Simulation) -> Vec<[Vec3; 3]> {
    let ub = sim.velocity().boundary_values(&sim.geom);
    derived::velocity_gradient(&sim.geom, &sim.velocity().values, &ub)
}

#[test]
fn p1_vorticity_and_q_through_the_case_tree() {
    let sim = sim_of(&linear(
        "[postprocessing]\nderived = [\"vorticity\", \"q_criterion\"]\n",
    ));
    let text = export(&sim, "p1");
    let w = vtk_array(&text, "vorticity");
    let q = vtk_array(&text, "q_criterion");
    let g = gradient_of(&sim);
    let (wl, ql) = (derived::vorticity(&g), derived::q_criterion(&g));
    assert_eq!(w.len(), 3 * sim.geom.mesh.n_cells());
    let mut diff = 0;
    for c in 0..wl.len() {
        for k in 0..3 {
            diff += usize::from(w[3 * c + k] != wl[c][k]);
        }
        diff += usize::from(q[c] != ql[c]);
    }
    assert_eq!(diff, 0, "file values differ from the library output");
    let inner = interior_cells(&sim);
    assert!(inner.len() >= 40, "{} interior cells", inner.len());
    let mut worst: f64 = 0.0;
    for &c in &inner {
        worst = worst
            .max((w[3 * c + 2] - 1.0).abs())
            .max(w[3 * c].abs())
            .max(w[3 * c + 1].abs())
            .max((q[c] + 7.0).abs());
    }
    println!(
        "P1: {} interior cells, worst analytic error {worst:.3e} (limit 1e-12)",
        inner.len()
    );
    assert!(worst <= 1e-12, "{worst}");
}

#[test]
fn p2_incompressible_total_pressure_converts_kinematic_pressure() {
    let text = "[mesh]\ngenerator = \"box_2d\"\nnx = 6\nny = 4\nlx = 1.0\nly = 1.0\n[physics]\nnu = 0.05\nrho = 1.2\n[initial]\nvelocity = [2.0, 0.0, 0.0]\npressure = 3.0\n".to_string()
        + BCS
        + "[solver]\nalgorithm = \"simplec\"\nmax_iterations = 5\n[postprocessing]\nderived = [\"total_pressure\"]\n";
    let sim = sim_of(&text);
    let t = export(&sim, "p2");
    let pt = vtk_array(&t, "p_total");
    assert_eq!(pt.len(), 24);
    let mut worst: f64 = 0.0;
    for (c, v) in pt.iter().enumerate() {
        worst = worst.max((v - 6.0).abs() / 6.0);
        let lib = derived::total_pressure_incompressible(1.2 * sim.pressure().values[c], 1.2, sim.velocity().values[c]);
        assert_eq!(*v, lib);
    }
    println!("P2: worst relative error {worst:.3e} (limit 1e-12)");
    assert!(worst <= 1e-12);
}

const VD: &str = r#"
[mesh]
generator = "nozzle_2d"
nx = 20
ny = 4
lx = 2.0
ly = 0.1
throat_height = 0.05
exit_height = 0.1
[fluid]
density = "ideal_gas"
gas_constant = 287.0
[energy]
enabled = true
relax = 0.9
[initial]
velocity = [30.0, 0.0, 0.0]
temperature = 300.0
pressure = 100000.0
[boundaries.xmin]
inlet = "stagnation"
total_pressure_value = 101325.0
temperature_value = 300.0
velocity = "zero_gradient"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 95000.0
temperature = "zero_gradient"
[boundaries.ymin]
velocity = "symmetry"
[boundaries.ymax]
velocity = "symmetry"
[solver]
algorithm = "simplec"
max_iterations = 5
"#;

#[test]
fn p3_variable_density_mach_and_isentropic_total_pressure() {
    let mut sim = sim_of(&format!(
        "{VD}[postprocessing]\nderived = [\"mach\", \"total_pressure\"]\n"
    ));
    sim.run(3);
    let t = export(&sim, "p3");
    let m = vtk_array(&t, "mach");
    assert_eq!(m, sim.field("mach").unwrap());
    let pt = vtk_array(&t, "p_total");
    let pa = sim.field("p_abs").unwrap();
    let mut worst: f64 = 0.0;
    for c in 0..pt.len() {
        let want = derived::total_pressure_isentropic(pa[c], m[c], 1.4);
        worst = worst.max((pt[c] - want).abs() / want);
    }
    println!("P3: worst relative error {worst:.3e} (limit 1e-12)");
    assert!(worst <= 1e-12);
    assert!(pt.iter().zip(&pa).all(|(t, p)| t >= p), "total pressure below static");
}

const COUETTE: &str = "[mesh]\ngenerator = \"box_2d\"\nnx = 8\nny = 10\nlx = 2.0\nly = 0.5\n[physics]\nnu = 0.05\nrho = 1.2\n[initial]\nvelocity_expression_x = \"6 [1/s] * y\"\n[boundaries.xmin]\nvelocity = \"fixed\"\n[boundaries.xmax]\nvelocity = \"fixed\"\n[boundaries.ymin]\nvelocity = \"fixed\"\n[boundaries.ymax]\nvelocity = \"fixed\"\nvelocity_value = [3.0, 0.0, 0.0]\n[solver]\nalgorithm = \"simplec\"\nmax_iterations = 5\n";

#[test]
fn p4_wall_shear_and_y_plus_of_couette_flow() {
    let text = format!(
        "{COUETTE}[postprocessing]\nderived = [\"wall_shear\", \"wall_y_plus\"]\nwall_patches = [\"ymin\", \"ymax\"]\n"
    );
    let sim = sim_of(&text);
    let dir = tmpdir("p4");
    write_results(&sim, &dir).unwrap();
    let ub = sim.velocity().boundary_values(&sim.geom);
    let mut worst_tau: f64 = 0.0;
    let mut worst_yp: f64 = 0.0;
    let yp_exact = 0.025 * (0.3f64).sqrt() / 0.05;
    for (patch, sign) in [("ymin", 1.0), ("ymax", -1.0)] {
        let s = std::fs::read_to_string(dir.join(format!("wall_{patch}.vtp"))).unwrap();
        let tau = vtk_array(&s, "wall_shear");
        let yp = vtk_array(&s, "wall_y_plus");
        assert_eq!(yp.len(), 8, "{patch}");
        assert_eq!(tau.len(), 24);
        let lib_tau = derived::wall_shear(&sim.geom, patch, &sim.velocity().values, &ub, 1.2, 0.05).unwrap();
        let lib_yp = derived::y_plus(&sim.geom, patch, &lib_tau, 1.2, 0.05).unwrap();
        assert_eq!(yp, lib_yp);
        for (i, t) in lib_tau.iter().enumerate() {
            assert_eq!([tau[3 * i], tau[3 * i + 1], tau[3 * i + 2]], *t);
            let mag = (tau[3 * i].powi(2) + tau[3 * i + 1].powi(2) + tau[3 * i + 2].powi(2)).sqrt();
            worst_tau = worst_tau.max((mag - 0.36).abs() / 0.36);
            assert!(tau[3 * i] * sign > 0.0, "{patch} sign");
            worst_yp = worst_yp.max((yp[i] - yp_exact).abs() / yp_exact);
        }
    }
    println!("P4: worst |tau| relative error {worst_tau:.3e}, y+ {worst_yp:.3e} (limit 1e-12)");
    assert!(worst_tau <= 1e-12 && worst_yp <= 1e-12);
    // not requested: no wall files
    let sim = sim_of(COUETTE);
    let dir = tmpdir("p4b");
    write_results(&sim, &dir).unwrap();
    assert!(!dir.join("wall_ymin.vtp").exists());
    // default patches: every wall patch
    let sim = sim_of(&format!("{COUETTE}[postprocessing]\nderived = [\"wall_y_plus\"]\n"));
    let dir = tmpdir("p4c");
    write_results(&sim, &dir).unwrap();
    assert!(dir.join("wall_ymin.vtp").exists() && dir.join("wall_xmin.vtp").exists());
}

fn errs(text: &str) -> Vec<rcfd_api::case::Diagnostic> {
    Case::parse_toml(text).unwrap_err()
}

fn has(errs: &[rcfd_api::case::Diagnostic], path: &str) -> bool {
    errs.iter().any(|e| e.path == path && e.line.is_some())
}

#[test]
fn p5_located_restrictions() {
    let wall = "[postprocessing]\nderived = [\"wall_shear\"]\n";
    let laminar = format!("{COUETTE}{wall}");
    Case::parse_toml(&laminar).unwrap_or_else(|e| panic!("positive control: {e:?}"));
    let cases: Vec<(String, &str)> = vec![
        (
            format!("{COUETTE}[turbulence]\nmodel = \"k_epsilon\"\n{wall}"),
            "postprocessing.derived",
        ),
        (
            format!("{COUETTE}[fluid]\ndensity = \"ideal_gas\"\n{wall}"),
            "postprocessing.derived",
        ),
        (
            COUETTE.replace("simplec", "compressible") + wall,
            "postprocessing.derived",
        ),
        (
            format!("{COUETTE}[rheology]\nmodel = \"power_law\"\n{wall}"),
            "postprocessing.derived",
        ),
        (
            format!("{COUETTE}[postprocessing]\nderived = [\"mach\"]\n"),
            "postprocessing.derived",
        ),
        (
            format!("{COUETTE}[fluid]\ndensity = \"real_gas\"\n[postprocessing]\nderived = [\"total_pressure\"]\n"),
            "postprocessing.derived",
        ),
        (
            format!("{COUETTE}[postprocessing]\nderived = [\"helicity\"]\n"),
            "postprocessing.derived",
        ),
    ];
    for (text, path) in &cases {
        let e = errs(text);
        assert!(has(&e, path), "{path} not located in {e:?}\n{text}");
    }
    // mesh-dependent: unknown patch and a non-wall patch
    for (patch, what) in [("nowhere", "not a patch"), ("frontAndBack", "not a wall")] {
        let c = Case::parse_toml(&format!(
            "{COUETTE}[postprocessing]\nderived = [\"wall_shear\"]\nwall_patches = [\"{patch}\"]\n"
        ))
        .unwrap();
        let e = Simulation::new(&c).err().expect("refused");
        let d = e
            .iter()
            .find(|d| d.path == "postprocessing.wall_patches")
            .unwrap_or_else(|| panic!("{e:?}"));
        assert!(d.message.contains(what), "{}", d.message);
    }
}

fn zmid(sim: &Simulation) -> f64 {
    let m = &sim.geom.mesh;
    let z: Vec<f64> = m.points().iter().map(|p| p[2]).collect();
    0.5 * (z.iter().cloned().fold(f64::INFINITY, f64::min) + z.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
}

/// The library result for the velocity components of `sim` (the reference of the byte comparison).
fn library_csv(sim: &Simulation, set: rcfd_post::derived::SampleSet) -> String {
    let mut set = set;
    let ub = sim.velocity().boundary_values(&sim.geom);
    for (name, k) in [("ux", 0usize), ("uy", 1)] {
        let cells: Vec<f64> = sim.velocity().values.iter().map(|v| v[k]).collect();
        let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
        let g = least_squares(&sim.geom, &cells, &bv);
        set.add_field(&sim.geom, name, &cells, Some(&g));
    }
    set.to_csv()
}

#[test]
fn p6_line_sampling_matches_library_and_analytic_field() {
    let sim = sim_of(&linear(""));
    let z = zmid(&sim);
    let (p0, p1) = ([0.8, 0.3, z], [3.2, 0.7, z]);
    let spec = SampleSpec {
        kind: SampleKind::Line { p0, p1, points: 20 },
        fields: vec!["ux".into(), "uy".into()],
        gradient: true,
    };
    let csv = postproc::sample_csv(&sim, &spec).unwrap();
    let lib = library_csv(&sim, derived::sample_line(&sim.geom, p0, p1, 20).unwrap());
    assert_eq!(csv, lib);
    let mut lines = csv.lines();
    assert_eq!(lines.next().unwrap(), "s,x,y,z,ux,uy,inside");
    let rows: Vec<Vec<f64>> = lines
        .map(|l| l.split(',').map(|t| t.parse().unwrap()).collect())
        .collect();
    assert_eq!(rows.len(), 20);
    let mut worst: f64 = 0.0;
    for r in &rows {
        assert_eq!(r[6], 1.0);
        worst = worst
            .max((r[4] - (r[1] + 2.0 * r[2])).abs())
            .max((r[5] - (3.0 * r[1] - r[2])).abs());
    }
    println!("P6: worst analytic error {worst:.3e} (limit 1e-12)");
    assert!(worst <= 1e-12);

    // the same sample from the case tree is written at the end of a run and equals the direct result
    let text = linear(&format!(
        "[postprocessing.samples.s1]\ntype = \"line\"\np0 = [0.8, 0.3, {z:?}]\np1 = [3.2, 0.7, {z:?}]\npoints = 20\nfields = [\"ux\", \"uy\"]\n"
    ));
    let sim2 = sim_of(&text);
    let dir = tmpdir("p6");
    write_results(&sim2, &dir).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("sample_s1.csv")).unwrap(), lib);

    // without gradient: the cell value
    let flat = postproc::sample(
        &sim,
        &SampleSpec {
            gradient: false,
            ..spec.clone()
        },
    )
    .unwrap();
    for (i, c) in flat.cells.iter().enumerate() {
        assert_eq!(flat.values[0][i], sim.velocity().values[c.unwrap()][0]);
    }
}

#[test]
fn p7_plane_sampling_matches_library_and_analytic_field() {
    let sim = sim_of(&linear(""));
    let z = zmid(&sim);
    let spec = SampleSpec {
        kind: SampleKind::Plane {
            origin: [0.8, 0.3, z],
            e1: [1.0, 0.0, 0.0],
            e2: [0.0, 1.0, 0.0],
            extent1: 2.4,
            extent2: 0.4,
            n1: 6,
            n2: 4,
        },
        fields: vec!["ux".into(), "uy".into()],
        gradient: true,
    };
    let csv = postproc::sample_csv(&sim, &spec).unwrap();
    let lib = library_csv(
        &sim,
        derived::sample_plane(
            &sim.geom,
            [0.8, 0.3, z],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            2.4,
            0.4,
            6,
            4,
        )
        .unwrap(),
    );
    assert_eq!(csv, lib);
    let mut lines = csv.lines();
    assert_eq!(lines.next().unwrap(), "x,y,z,ux,uy,inside");
    let rows: Vec<Vec<f64>> = lines
        .map(|l| l.split(',').map(|t| t.parse().unwrap()).collect())
        .collect();
    assert_eq!(rows.len(), 24);
    let worst = rows
        .iter()
        .map(|r| {
            (r[3] - (r[0] + 2.0 * r[1]))
                .abs()
                .max((r[4] - (3.0 * r[0] - r[1])).abs())
        })
        .fold(0.0, f64::max);
    println!("P7: worst analytic error {worst:.3e} (limit 1e-12)");
    assert!(worst <= 1e-12);
}

#[test]
fn p8_points_outside_the_mesh_are_flagged() {
    let sim = sim_of(&linear(""));
    let z = zmid(&sim);
    let (p0, p1) = ([0.5, 0.5, z], [5.5, 0.5, z]);
    let spec = SampleSpec {
        kind: SampleKind::Line { p0, p1, points: 20 },
        fields: vec!["ux".into(), "uy".into()],
        gradient: true,
    };
    let csv = postproc::sample_csv(&sim, &spec).unwrap();
    let set = derived::sample_line(&sim.geom, p0, p1, 20).unwrap();
    let inside = set.n_inside();
    assert!(inside > 0 && inside < 20, "{inside}");
    assert_eq!(csv, library_csv(&sim, set));
    let outside_rows: Vec<&str> = csv.lines().skip(1).filter(|l| l.ends_with(",0")).collect();
    assert_eq!(outside_rows.len(), 20 - inside);
    assert!(outside_rows.iter().all(|l| l.contains(",,")), "empty value fields");
}

#[test]
fn p10_service_field_sample_equals_the_direct_csv() {
    let svc = Service::new(ServiceConfig::default());
    let text = linear("");
    let sim = sim_of(&text);
    let z = zmid(&sim);
    let id = svc.call("session.create", &json!({ "toml": text })).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    svc.call("run.initialize", &json!({"session": id})).unwrap();
    let r = svc
        .call(
            "field.sample",
            &json!({"session": id, "type": "line", "p0": [0.8, 0.3, z], "p1": [3.2, 0.7, z], "points": 20, "fields": ["ux", "uy"]}),
        )
        .unwrap();
    let spec = SampleSpec {
        kind: SampleKind::Line {
            p0: [0.8, 0.3, z],
            p1: [3.2, 0.7, z],
            points: 20,
        },
        fields: vec!["ux".into(), "uy".into()],
        gradient: true,
    };
    assert_eq!(r["csv"], json!(postproc::sample_csv(&sim, &spec).unwrap()));
    assert_eq!(r["n_samples"], 20);
    assert_eq!(r["n_inside"], 20);
    // an unknown field is a parameter error
    assert!(svc
        .call(
            "field.sample",
            &json!({"session": id, "p0": [0.0, 0.0, 0.0], "p1": [1.0, 0.0, 0.0], "fields": ["nope"]})
        )
        .is_err());
}

#[test]
fn p11_located_sample_errors() {
    let base = "[postprocessing.samples.s]\n";
    let ok =
        format!("{base}type = \"line\"\np0 = [0.5, 0.5, 0.0]\np1 = [1.5, 0.5, 0.0]\npoints = 5\nfields = [\"ux\"]\n");
    Case::parse_toml(&linear(&ok)).unwrap_or_else(|e| panic!("positive control: {e:?}"));
    let cases: Vec<(String, &str)> = vec![
        (
            ok.replace("points = 5", "points = 1"),
            "postprocessing.samples.s.points",
        ),
        (
            ok.replace("[1.5, 0.5, 0.0]", "[0.5, 0.5, 0.0]"),
            "postprocessing.samples.s.p1",
        ),
        (ok.replace("\"ux\"", "\"nope\""), "postprocessing.samples.s.fields"),
        (ok.replace("\"ux\"", "\"T\""), "postprocessing.samples.s.fields"),
        (
            format!("{base}type = \"plane\"\ne2 = [1.0, 1.0, 0.0]\n"),
            "postprocessing.samples.s.e2",
        ),
        (
            format!("{base}type = \"plane\"\nn1 = 0\n"),
            "postprocessing.samples.s.n1",
        ),
    ];
    for (text, path) in &cases {
        let e = errs(&linear(text));
        assert!(has(&e, path), "{path} not located in {e:?}");
    }
    // a field the engine does not have is refused when the simulation is built
    let c = Case::parse_toml(&linear(&ok.replace("\"ux\"", "\"k\""))).unwrap();
    let e = Simulation::new(&c).err().expect("refused");
    assert!(e.iter().any(|d| d.path == "postprocessing.samples.s.fields"), "{e:?}");
}
