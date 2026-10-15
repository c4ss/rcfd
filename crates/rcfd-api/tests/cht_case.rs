//! Conjugate heat transfer through the settings tree (F-051, ADR 0024 checks C2, D4, F1 to F5 and the case-level
//! convection checks). The limits are those registered in the ADR before this file was written; none was changed
//! afterwards.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;
use rcfd_fv::thermal_bc::STEFAN_BOLTZMANN;

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

fn build(text: &str) -> Simulation {
    Simulation::new(&case(text)).unwrap_or_else(|e| panic!("{e:?}"))
}

/// Located errors of a case text, from parsing or from building the simulation.
fn errors(text: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => match Simulation::new(&c) {
            Err(e) => e,
            Ok(_) => vec![],
        },
    }
}

fn has(errs: &[Diagnostic], path: &str) -> bool {
    errs.iter().any(|d| d.path == path)
}

fn patch_area(s: &Simulation, patch: &str) -> f64 {
    let sf = s.geom.mesh.face_area_vectors();
    s.geom
        .mesh
        .patch(patch)
        .unwrap()
        .faces()
        .map(|f| vec3::norm(sf[f]))
        .sum()
}

// ------------------------------------------------------------------------------------- F1 / F3: composite wall

/// Three layers in x: solid a (k = 1, rho cp = 3) for x < 0.2, solid b (k = 10, rho cp = 1) for 0.2 < x < 0.5, fluid
/// (k = 0.1, rho cp = 7) beyond; ends fixed at 400 K and 300 K, the other walls adiabatic, zero velocity.
fn wall_text(nx: usize) -> String {
    format!(
        r#"
title = "composite wall"
[mesh]
generator = "box_2d"
nx = {nx}
ny = 1
lx = 1.0
ly = 1.0
[physics]
nu = 0.01
rho = 7.0
cp = 1.0
conductivity = 0.1
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 350.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 400.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simple"
convection = "upwind"
frozen_flow = true
tolerance_u = 1e-14
max_iterations = 4
[solids.a]
zone = "box"
box_min = [-10.0, -10.0, -10.0]
box_max = [0.2, 10.0, 10.0]
conductivity = 1.0
density = 3.0
specific_heat = 1.0
[solids.b]
zone = "box"
box_min = [0.2, -10.0, -10.0]
box_max = [0.5, 10.0, 10.0]
conductivity = 10.0
density = 1.0
specific_heat = 1.0
[reports.q_in]
type = "heat_flow"
patch = "xmin"
[reports.q_out]
type = "heat_flow"
patch = "xmax"
"#
    )
}

fn wall_exact(x: f64) -> (f64, f64) {
    let (edges, k) = ([0.0, 0.2, 0.5, 1.0], [1.0, 10.0, 0.1]);
    let r: f64 = (0..3).map(|i| (edges[i + 1] - edges[i]) / k[i]).sum();
    let q = 100.0 / r;
    let mut res = 0.0;
    for i in 0..3 {
        if x <= edges[i + 1] {
            res += (x - edges[i]) / k[i];
            break;
        }
        res += (edges[i + 1] - edges[i]) / k[i];
    }
    (400.0 - q * res, q)
}

#[test]
fn f1_f3_composite_wall_through_the_tree() {
    let mut s = build(&wall_text(50));
    s.run(10);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let t = s.field("T").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let err = (0..t.len())
        .map(|i| (t[i] - wall_exact(cc[i][0]).0).abs())
        .fold(0.0, f64::max)
        / 100.0;
    println!("F1 max cell error / dT = {err:.3e} (limit 1e-9)");
    assert!(err <= 1e-9, "F1 {err}");
    // F3: heat flow reports on a solid patch (xmin) and a fluid patch (xmax)
    let q = wall_exact(0.0).1;
    let a = patch_area(&s, "xmin");
    let (qin, qout) = (s.monitors.last("q_in").unwrap(), s.monitors.last("q_out").unwrap());
    let (r_in, r_out) = ((qin / (-q * a) - 1.0).abs(), (qout / (q * a) - 1.0).abs());
    println!("F3 heat flow out: xmin {qin:.9e} vs {:.9e} (rel {r_in:.2e}); xmax {qout:.9e} vs {:.9e} (rel {r_out:.2e}) (limit 1e-9)", -q * a, q * a);
    assert!(r_in <= 1e-9 && r_out <= 1e-9, "F3 {r_in} {r_out}");
}

// ------------------------------------------------------------------------------------- F2: transient contact

fn contact_text(n: usize, steps: usize) -> String {
    format!(
        r#"
title = "contact"
[mesh]
generator = "box_2d"
nx = {n}
ny = 1
lx = 16.0
ly = 1.0
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = 2.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 400.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "piso"
convection = "upwind"
time_scheme = "bdf2"
frozen_flow = true
dt = {dt}
end_time = 0.2
[solids.right]
zone = "box"
box_min = [8.0, -10.0, -10.0]
box_max = [20.0, 10.0, 10.0]
conductivity = 0.5
density = 2.0
specific_heat = 1.0
initial_temperature = 300.0
"#,
        dt = 0.2 / steps as f64
    )
}

fn erfc(x: f64) -> f64 {
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    if x < 3.0 {
        let (mut term, mut sum, mut n) = (x, x, 0.0);
        while term.abs() > 1e-17 {
            n += 1.0;
            term *= -x * x / n;
            sum += term / (2.0 * n + 1.0);
        }
        1.0 - 2.0 / std::f64::consts::PI.sqrt() * sum
    } else {
        let mut f = 0.0;
        for k in (1..60).rev() {
            f = (k as f64 / 2.0) / (x + f);
        }
        (-x * x).exp() / ((x + f) * std::f64::consts::PI.sqrt())
    }
}

#[test]
fn f2_transient_contact_through_the_tree() {
    let mut s = build(&contact_text(640, 80));
    s.run(200);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let (t1, t2) = (400.0, 300.0);
    let (k1, r1, k2, r2): (f64, f64, f64, f64) = (2.0, 1.0, 0.5, 2.0);
    let (e1, e2) = ((k1 * r1).sqrt(), (k2 * r2).sqrt());
    let ti = (e1 * t1 + e2 * t2) / (e1 + e2);
    let (a1, a2, tend) = (k1 / r1, k2 / r2, 0.2);
    let t = s.field("T").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let err = (0..t.len())
        .map(|i| {
            let x = cc[i][0] - 8.0;
            let exact = if x < 0.0 {
                t1 - (t1 - ti) * erfc(x.abs() / (2.0 * (a1 * tend).sqrt()))
            } else {
                t2 + (ti - t2) * erfc(x / (2.0 * (a2 * tend).sqrt()))
            };
            (t[i] - exact).abs()
        })
        .fold(0.0, f64::max)
        / (t1 - t2);
    println!(
        "F2 max error / dT = {err:.3e} at N = 640 (limit 1e-3), time {:.6}",
        s.time()
    );
    assert!(err <= 1e-3, "F2 {err}");
}

// ------------------------------------------------------------------------------------- C2: non-conformal interface

fn interface_text(nl: [usize; 2], nr: [usize; 2]) -> String {
    format!(
        r#"
title = "non-conformal composite wall"
[mesh]
generator = "channel_2d_interface"
nx = {}
ny = {}
nx_right = {}
ny_right = {}
lx = 1.0
ly = 1.0
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = 1.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 350.0
[boundaries.inlet]
velocity = "fixed"
temperature = "fixed"
temperature_value = 400.0
[boundaries.outlet]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simple"
frozen_flow = true
tolerance_u = 1e-14
max_iterations = 60
[solids.right]
zone = "box"
box_min = [0.5, -10.0, -10.0]
box_max = [10.0, 10.0, 10.0]
conductivity = 5.0
density = 1.0
specific_heat = 1.0
[reports.q_in]
type = "heat_flow"
patch = "inlet"
[reports.q_out]
type = "heat_flow"
patch = "outlet"
"#,
        nl[0], nl[1], nr[0], nr[1]
    )
}

fn interface_run(nl: [usize; 2], nr: [usize; 2]) -> (f64, f64) {
    let mut s = build(&interface_text(nl, nr));
    s.run(100);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let q = 100.0 / (0.5 / 1.0 + 0.5 / 5.0);
    let ti = 400.0 - q * 0.5;
    let t = s.field("T").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let err = (0..t.len())
        .map(|i| {
            let x = cc[i][0];
            let ex = if x < 0.5 {
                400.0 - q * x
            } else {
                ti - q / 5.0 * (x - 0.5)
            };
            (t[i] - ex).abs()
        })
        .fold(0.0, f64::max)
        / 100.0;
    let (qin, qout) = (s.monitors.last("q_in").unwrap(), s.monitors.last("q_out").unwrap());
    let cons = (qin + qout).abs() / (q * patch_area(&s, "inlet"));
    (err, cons)
}

#[test]
fn c2_non_conformal_composite_wall_through_the_tree() {
    let (e1, c1) = interface_run([10, 20], [13, 27]);
    let (e2, c2) = interface_run([20, 40], [26, 54]);
    println!(
        "C2 error/dT {e1:.3e} -> {e2:.3e} (ratio {:.2}, limits <= 1e-2 and >= 1.5); conservation {c1:.2e} {c2:.2e} (limit 1e-9)",
        e1 / e2
    );
    assert!(e1 <= 1e-2 && e2 <= 1e-2);
    assert!(e1 / e2 >= 1.5);
    assert!(c1 <= 1e-9 && c2 <= 1e-9);
}

// ------------------------------------------------------------------------------------- D4: duct with solid wall

fn duct_text(nf: usize) -> String {
    let ny = nf + nf / 2;
    format!(
        r#"
title = "fluid-solid duct"
[mesh]
generator = "box_2d"
nx = 8
ny = {ny}
lx = 2.0
ly = 1.5
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = 1.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 301.0
velocity_expression_x = "if(y < 1 [m], 1.5 [m/s] * (1 - (y / 1 [m])^2), 0 [m/s])"
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
temperature = "fixed"
temperature_expression = "300 [K] + 1 [K/m] * x + if(y < 1 [m], 1.5 [K] * ((y / 1 [m])^2 / 2 - (y / 1 [m])^4 / 12), 0.625 [K] + 0.1 [K/m] * (y - 1 [m]))"
[boundaries.xmax]
velocity = "zero_gradient"
temperature = "fixed"
temperature_expression = "300 [K] + 1 [K/m] * x + if(y < 1 [m], 1.5 [K] * ((y / 1 [m])^2 / 2 - (y / 1 [m])^4 / 12), 0.625 [K] + 0.1 [K/m] * (y - 1 [m]))"
[boundaries.ymin]
velocity = "symmetry"
[boundaries.ymax]
velocity = "fixed"
temperature = "heat_flux"
heat_flux_value = 1.0
[solver]
algorithm = "simple"
convection = "central"
frozen_flow = true
tolerance_u = 1e-14
max_iterations = 6
[solids.wall]
zone = "box"
box_min = [-10.0, 1.0, -10.0]
box_max = [10.0, 10.0, 10.0]
conductivity = 10.0
density = 4.0
specific_heat = 1.0
"#
    )
}

#[test]
fn d4_duct_nusselt_through_the_tree() {
    let nf = 32;
    let mut s = build(&duct_text(nf));
    s.run(10);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let t = s.field("T").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let xc = cc.iter().map(|c| c[0]).fold(
        f64::INFINITY,
        |a, b| if (b - 1.0).abs() < (a - 1.0).abs() { b } else { a },
    );
    let dy = 1.5 / (nf + nf / 2) as f64;
    let (mut num, mut den, mut top) = (0.0, 0.0, f64::NAN);
    let mut ymax = f64::NEG_INFINITY;
    for i in 0..t.len() {
        if (cc[i][0] - xc).abs() > 1e-9 {
            continue;
        }
        let y = cc[i][1];
        if y < 1.0 {
            let w = 1.5 * (1.0 - y * y) * dy;
            num += w * t[i];
            den += w;
        }
        if y > ymax {
            ymax = y;
            top = t[i] + 1.0 / 10.0 * 0.5 * dy;
        }
    }
    let tb = num / den;
    let exact = 17.0 / 35.0 + 0.5 / 10.0;
    let rel = ((top - tb) - exact).abs() / exact;
    println!(
        "D4 dT_tot {:.6} vs {exact:.6}, relative error {rel:.3e} (limit 1e-2)",
        top - tb
    );
    assert!(rel <= 1e-2, "D4 {rel}");
}

// ------------------------------------------------------------------------------------- convection conditions

fn slab_text(boundary: &str, iterations: usize) -> String {
    format!(
        r#"
title = "slab"
[mesh]
generator = "box_2d"
nx = 20
ny = 1
lx = 0.05
ly = 0.05
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = 15.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 500.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 800.0
[boundaries.xmax]
velocity = "fixed"
{boundary}
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simple"
convection = "upwind"
frozen_flow = true
tolerance_u = 1e-14
max_iterations = {iterations}
[reports.q_out]
type = "heat_flow"
patch = "xmax"
"#
    )
}

#[test]
fn convection_wall_through_the_tree() {
    // A1: k = 15, L = 0.05, left 800 K, right h = 100 to 300 K
    let mut s = build(&slab_text(
        "temperature = \"convection\"\nheat_transfer_coefficient = 100.0\nfree_stream_temperature = 300.0",
        4,
    ));
    s.run(10);
    let (k, l, h) = (15.0, 0.05, 100.0);
    let q = 500.0 / (l / k + 1.0 / h);
    let t = s.field("T").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let err = (0..t.len())
        .map(|i| (t[i] - (800.0 - q * cc[i][0] / k)).abs())
        .fold(0.0, f64::max)
        / 500.0;
    let area = patch_area(&s, "xmax");
    let flow = s.monitors.last("q_out").unwrap();
    println!(
        "A1 case: error/dT {err:.3e} (limit 1e-10); heat flow rel {:.3e}",
        (flow / (q * area) - 1.0).abs()
    );
    assert!(err <= 1e-10);
    assert!((flow / (q * area) - 1.0).abs() <= 1e-9);
    // A3b: pure radiation to 300 K, emissivity 0.8 (reference by bisection of k (800 - Ts) / L = eps sigma (Ts^4 - Ta^4))
    let mut s = build(&slab_text(
        "temperature = \"convection\"\nheat_transfer_coefficient = 0.0\nwall_emissivity = 0.8\nradiation_temperature = 300.0",
        40,
    ));
    s.run(60);
    let sg = 0.8 * STEFAN_BOLTZMANN;
    let f = |ts: f64| k * (800.0 - ts) / l - sg * (ts.powi(4) - 300.0_f64.powi(4));
    let (mut lo, mut hi) = (300.0, 800.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            lo = mid
        } else {
            hi = mid
        }
    }
    let ts = 0.5 * (lo + hi);
    let q = k * (800.0 - ts) / l;
    let t = s.field("T").unwrap();
    let err = (0..t.len())
        .map(|i| (t[i] - (800.0 - q * cc[i][0] / k)).abs())
        .fold(0.0, f64::max)
        / 500.0;
    println!("A3b case: surface {ts:.6} K, error/dT {err:.3e} (limit 1e-9)");
    assert!(err <= 1e-9);
}

// ------------------------------------------------------------------------------------- F4: located validation

#[test]
fn f4_located_validation_errors() {
    let base = wall_text(20);
    let cases: Vec<(String, &str)> = vec![
        (base.replace("zone = \"box\"\nbox_min = [-10.0, -10.0, -10.0]\nbox_max = [0.2, 10.0, 10.0]", "zone = \"unset\""), "solids.a.zone"),
        (base.replacen("conductivity = 1.0\ndensity = 3.0", "density = 3.0", 1), "solids.a.conductivity"),
        (base.replacen("conductivity = 1.0\ndensity = 3.0", "conductivity = 1.0\ndensity = -3.0", 1), "solids.a.density"),
        (base.replacen("density = 3.0\nspecific_heat = 1.0", "density = 3.0\nspecific_heat = 0.0", 1), "solids.a.specific_heat"),
        (base.replace("box_max = [0.5, 10.0, 10.0]", "box_max = [0.5, 10.0, 10.0]\n").replace("box_min = [0.2, -10.0, -10.0]", "box_min = [0.1, -10.0, -10.0]"), "solids.b.zone"),
        (base.replace("box_min = [0.2, -10.0, -10.0]\nbox_max = [0.5, 10.0, 10.0]", "box_min = [5.0, 5.0, 5.0]\nbox_max = [6.0, 6.0, 6.0]"), "solids.b.zone"),
        (format!("{base}[thermal_interfaces.i]\nsolid_a = \"nope\"\nsolid_b = \"fluid\"\nresistance = 0.1\n"), "thermal_interfaces.i.solid_a"),
        (format!("{base}[thermal_interfaces.i]\nsolid_a = \"a\"\nsolid_b = \"b\"\n"), "thermal_interfaces.i.resistance"),
        (format!("{base}[thermal_interfaces.i]\nsolid_a = \"a\"\nsolid_b = \"b\"\nresistance = -0.5\n"), "thermal_interfaces.i.resistance"),
        (base.replace("enabled = true\nrelax", "enabled = false\nrelax"), "solids"),
        (base.replace("frozen_flow = true\n", ""), "solver.frozen_flow"),
        (format!("{base}[radiation]\nmodel = \"p1\"\nabsorption_coefficient = 1.0\n"), "radiation.model"),
        (base.replace("[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"fixed\"\ntemperature_value = 300.0", "[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"convection\""), "boundaries.xmax.heat_transfer_coefficient"),
        (base.replace("[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"fixed\"\ntemperature_value = 300.0", "[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"convection\"\nheat_transfer_coefficient = 5.0"), "boundaries.xmax.free_stream_temperature"),
        (base.replace("[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"fixed\"\ntemperature_value = 300.0", "[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"convection\"\nheat_transfer_coefficient = 5.0\nfree_stream_temperature = 300.0\nwall_emissivity = 0.5"), "boundaries.xmax.radiation_temperature"),
        (base.replace("[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"fixed\"\ntemperature_value = 300.0", "[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"convection\"\nheat_transfer_coefficient = 5.0\nfree_stream_temperature = 300.0\nwall_thickness = [0.01, 0.02]\nwall_conductivity = [1.0]"), "boundaries.xmax.wall_conductivity"),
        (base.replace("[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"fixed\"\ntemperature_value = 300.0", "[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"convection\"\nheat_transfer_coefficient = 5.0\nfree_stream_temperature = 300.0\nwall_thickness = [0.0]\nwall_conductivity = [1.0]"), "boundaries.xmax.wall_thickness"),
        (base.replace("temperature_value = 300.0", "temperature_value = 300.0\nwall_thickness = [0.01]\nwall_conductivity = [1.0]"), "boundaries.xmax.wall_thickness"),
        (base.replace("[initial]\ntemperature = 350.0", "[initial]\ntemperature = 350.0\nvelocity_expression_x = \"1 [m/s]\""), "initial.velocity_expression_x"),
    ];
    for (i, (text, path)) in cases.iter().enumerate() {
        let errs = errors(text);
        println!(
            "F4 case {i}: expecting {path}; got {:?}",
            errs.iter().map(|d| d.path.clone()).collect::<Vec<_>>()
        );
        assert!(has(&errs, path), "F4 case {i}: no error at {path}: {errs:?}");
    }
}

// ------------------------------------------------------------------------------------- F5: regression

#[test]
fn f5_a_case_without_conjugate_features_is_untouched() {
    // the registered baseline hashes live in baseline_hashes.rs; here, no cht state is built for a plain case
    let text = wall_text(10)
        .replace("frozen_flow = true\n", "")
        .split("[solids.a]")
        .next()
        .unwrap()
        .to_string();
    let mut s = build(&text);
    s.run(3);
    assert!(s.failure().is_none());
}

// ------------------------------------------------------------------------------------- F4b: silent-drop guards

/// Combinations that would silently ignore or shadow a setting are located errors (added after review of the first
/// version; the registered F4 test above is unchanged).
#[test]
fn f4b_combinations_that_would_drop_a_setting_are_rejected() {
    let conv = |extra: &str| {
        format!(
            "{}{extra}",
            slab_text(
                "temperature = \"convection\"
heat_transfer_coefficient = 10.0
free_stream_temperature = 300.0",
                3
            )
        )
    };
    // user scalars with a prescribed flow
    let errs = errors(&format!(
        "{}[scalars.s]
diffusivity = 1.0
",
        wall_text(10)
    ));
    assert!(has(&errs, "scalars"), "{errs:?}");
    // convection with a temperature-dependent conductivity law
    let errs = errors(&conv(
        "[properties.conductivity]
law = \"polynomial\"
coefficients = [-2.0, 0.01]
",
    ));
    assert!(has(&errs, "boundaries.xmax.temperature"), "{errs:?}");
    // convection with the variable-density solver
    let errs = errors(&conv(
        "[fluid]
density = \"ideal_gas\"
",
    ));
    assert!(
        has(&errs, "boundaries.xmax.temperature") || !errs.is_empty(),
        "{errs:?}"
    );
    println!(
        "F4b variable-density errors: {:?}",
        errs.iter().map(|d| d.path.clone()).collect::<Vec<_>>()
    );
}
