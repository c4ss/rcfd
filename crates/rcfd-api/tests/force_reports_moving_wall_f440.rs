//! F-440 (ADR 0342): the force, moment and force-coefficient reports of a moving-mesh case take the wall pressure of the
//! kept fixed-flux gradient (the face pressure of `rcfd_fv::sixdof::patch_force_moment_wall_gradient`) with the key
//! `wall_pressure` of the optional table `[dynamic_mesh.force_reports]`; without it every report is bit-identical.
//!
//! The limits, set-ups and mutations were registered in ADR 0342 before this file existed. The column is ADR 0269 AU1b's
//! (copied from `wall_pressure_case_f358.rs`, which is not edited), with five reports added.

use rcfd_api::case::Case;
use rcfd_api::sim::{ReportDef, ReportKind, Simulation};
use rcfd_core::vec3;
use rcfd_fv::sixdof::{patch_face_forces, patch_force_moment_wall_gradient};

const KEY: &str = "dynamic_mesh.force_reports.wall_pressure";
const PRESCRIBED: &str = "[dynamic_mesh.force_reports]\nwall_pressure = \"prescribed_gradient\"\n";
const STORED: &str = "[dynamic_mesh.force_reports]\nwall_pressure = \"boundary_value\"\n";
const NAMES: [&str; 5] = ["cf", "fo", "fp", "ft", "my"];

/// ADR 0269 AU1b's rigid column (the box starting at rest) with `xmin` at rest (`wall = "rest"`) or following, the five
/// reports of ADR 0342 and `table` at the end.
fn column(wall: &str, table: &str) -> String {
    let patches = if wall == "rest" {
        "rest_patches = [\"xmin\"]"
    } else {
        "moving_patches = [\"xmin\"]"
    };
    format!(
        r#"
[mesh]
generator = "box_3d"
nx = 20
ny = 2
nz = 2
lx = 1.0
ly = 0.2
lz = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[boundaries.zmin]
velocity = "zero_gradient"
[boundaries.zmax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.01
end_time = 1.0
momentum_rel_tol = 1e-13
momentum_max_iter = 5000
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-16
pressure_max_iter = 5000
[reports.fp]
type = "force"
patch = "xmin"
component = "pressure"
direction = [1.0, 0.0, 0.0]
[reports.ft]
type = "force"
patch = "xmin"
component = "total"
direction = [1.0, 0.0, 0.0]
[reports.my]
type = "moment"
patch = "xmin"
direction = [0.0, 1.0, 0.0]
origin = [0.0, 0.0, 0.0]
[reports.cf]
type = "force_coefficient"
patch = "xmin"
component = "pressure"
direction = [1.0, 0.0, 0.0]
u_ref = 2.0
a_ref = 0.04
[reports.fo]
type = "force"
patch = "xmax"
component = "total"
direction = [1.0, 0.0, 0.0]
[dynamic_mesh]
enabled = true
deformation = "rigid"
{patches}
displacement_expression_x = "0.01 [m] * (cos(2 * pi * t / 1 [s]) - 1)"
{table}
"#
    )
}

fn parse(text: &str) -> Case {
    let c = Case::parse_toml(text).unwrap_or_else(|d| panic!("{d:?}"));
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    c
}

/// The report values by name (sorted as [`NAMES`]).
fn reports(s: &Simulation) -> Vec<f64> {
    let mut v: Vec<(String, f64)> = s
        .report_values()
        .into_iter()
        .map(|(n, r)| (n, r.unwrap_or_else(|e| panic!("{e}")).value))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(v.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), NAMES);
    v.into_iter().map(|x| x.1).collect()
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|a| a.to_bits()).collect()
}

/// `u`, `p`, `phi` of the flow, as bits.
fn flow_bits(s: &Simulation) -> Vec<u64> {
    let f = s.pimple_solver().unwrap();
    let mut out: Vec<u64> = f.u.values.iter().flatten().map(|a| a.to_bits()).collect();
    out.extend(f.p.values.iter().map(|a| a.to_bits()));
    out.extend(f.phi.iter().map(|a| a.to_bits()));
    out
}

/// Every report evaluated by the formula of the tree before F-440 (`wall_force_and_moment` with `pressure_face_values`).
fn reports_by_the_old_formula(s: &Simulation, case: &Case) -> Vec<f64> {
    let pf = s.pressure_face_values();
    let mut out: Vec<(String, f64)> = ReportDef::from_case(case)
        .into_iter()
        .map(|r| {
            let (f, m) =
                rcfd_post::integrals::wall_force_and_moment(&s.geom, &r.patch, s.velocity(), &pf, 0.01, 1.0, r.origin)
                    .unwrap();
            let pick = if r.component == "pressure" {
                f.pressure
            } else {
                f.total()
            };
            let v = match r.kind {
                ReportKind::Force => vec3::dot(pick, vec3::unit(r.direction)),
                ReportKind::ForceCoefficient => {
                    rcfd_post::integrals::force_coefficient(pick, r.direction, 1.0, r.u_ref, r.a_ref)
                }
                ReportKind::Moment => vec3::dot(m, vec3::unit(r.direction)),
                _ => unreachable!(),
            };
            (r.name, v)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.into_iter().map(|x| x.1).collect()
}

/// The exact force on `xmin`, `-g_m L A`, from the kept vector.
fn exact_force(s: &Simulation) -> (f64, f64) {
    let g = &s.geom;
    let w = s.dynamic_wall_gradients();
    let (mut area, mut gsum, mut k) = (0.0, 0.0, 0.0);
    for f in g.mesh.patch("xmin").unwrap().faces() {
        area += vec3::norm(g.mesh.face_area_vectors()[f]);
        gsum += w[f - g.n_internal];
        k += 1.0;
    }
    let gm = gsum / k;
    (-gm * area, gm)
}

/// Deficits `[cf, fp, ft, my]` against their exact values (ADR 0342 "Registered checks").
fn deficits(s: &Simulation, r: &[f64]) -> [f64; 4] {
    let (fx, _) = exact_force(s);
    [
        1.0 - r[0] / (fx / (0.5 * 4.0 * 0.04)),
        1.0 - r[2] / fx,
        1.0 - r[3] / fx,
        1.0 - r[4] / (0.1 * fx),
    ]
}

/// 12 steps of a column; the worst deficits `[cf, fp, ft, my]` over steps 3 to 12 (largest in magnitude, or furthest
/// from `about`).
fn worst(wall: &str, table: &str, about: f64) -> [f64; 4] {
    let mut s = Simulation::new(&parse(&column(wall, table))).unwrap();
    let mut w = [about; 4];
    for n in 1..=12 {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        if n < 3 {
            continue;
        }
        let r = reports(&s);
        let d = deficits(&s, &r);
        println!(
            "{wall} {} step {n}: g {:.6e}; deficits cf {:.7e}, fp {:.7e}, ft {:.7e}, my {:.7e}",
            if table.is_empty() { "no table" } else { "key" },
            exact_force(&s).1,
            d[0],
            d[1],
            d[2],
            d[3]
        );
        for k in 0..4 {
            if (d[k] - about).abs() > (w[k] - about).abs() {
                w[k] = d[k];
            }
        }
    }
    w
}

/// ADR 0342 T0: the case without the table keeps its texts.
#[test]
fn t0_a_case_without_the_table_keeps_its_text() {
    let without = parse(&column("rest", ""));
    let with = parse(&column("rest", PRESCRIBED));
    let (tw, jw) = (without.to_toml(), without.to_json_string());
    println!(
        "T0: settings hash without {:016x}, with {:016x}",
        without.tree.content_hash(),
        with.tree.content_hash()
    );
    assert!(!tw.contains("force_reports") && !jw.contains("force_reports"));
    assert!(without.tree.to_json().pointer("/dynamic_mesh/force_reports").is_none());
    assert_eq!(without.get(KEY).unwrap(), "boundary_value");
    assert_eq!(with.get(KEY).unwrap(), "prescribed_gradient");
    assert!(with.to_toml().contains("wall_pressure = \"prescribed_gradient\""));
    assert!(with.to_json_string().contains("\"force_reports\""));
    assert_ne!(without.tree.content_hash(), with.tree.content_hash());
}

/// ADR 0342 Z0, Z1, Z2: without the key every report is the old formula bit for bit; a table with the default equals no
/// table; the key does not change the flow.
#[test]
fn z0_z1_z2_without_the_key_nothing_changes() {
    let case0 = parse(&column("rest", ""));
    let mut s0 = Simulation::new(&case0).unwrap();
    let mut s1 = Simulation::new(&parse(&column("rest", STORED))).unwrap();
    let mut s2 = Simulation::new(&parse(&column("rest", PRESCRIBED))).unwrap();
    let (mut z0, mut z1, mut z2) = (0, 0, 0);
    for n in 0..=12 {
        if n > 0 {
            for s in [&mut s0, &mut s1, &mut s2] {
                s.step();
                assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
            }
        }
        let r0 = reports(&s0);
        if bits(&r0) != bits(&reports_by_the_old_formula(&s0, &case0)) {
            z0 += 1;
            println!("Z0 differs at step {n}");
        }
        if bits(&r0) != bits(&reports(&s1)) || flow_bits(&s0) != flow_bits(&s1) {
            z1 += 1;
            println!("Z1 differs at step {n}");
        }
        if flow_bits(&s0) != flow_bits(&s2) {
            z2 += 1;
            println!("Z2 differs at step {n}");
        }
        if n == 0 {
            // before the first step the key keeps the stored values
            assert_eq!(bits(&r0), bits(&reports(&s2)), "Z1: step 0 with the key");
        }
    }
    println!("Z0 {z0}, Z1 {z1}, Z2 {z2} of 13 evaluations differ");
    assert_eq!((z0, z1, z2), (0, 0, 0));
}

/// ADR 0342 E1, E2, E3: the report's face pressure is the corrected evaluation's, bit for bit.
#[test]
fn e1_e2_e3_the_report_takes_the_corrected_face_pressure() {
    let mut s = Simulation::new(&parse(&column("rest", PRESCRIBED))).unwrap();
    let mut s0 = Simulation::new(&parse(&column("rest", ""))).unwrap();
    let wall = vec!["xmin".to_string()];
    let (mut e1, mut e2, mut e3) = (0usize, 0.0f64, 0usize);
    for n in 1..=12 {
        s.step();
        s0.step();
        assert!(s.failure().is_none() && s0.failure().is_none(), "step {n}");
        let kept = s.dynamic_wall_gradients().to_vec();
        let pr = s.report_pressure_face_values();
        let pd = s.pressure_face_values();
        let fl = s.pimple_solver().unwrap();
        let g = &s.geom;
        let ni = g.n_internal;
        let loads = patch_face_forces(g, &fl.u, &fl.p, 1.0, 0.0, &wall, Some(&kept)).unwrap();
        let faces: Vec<usize> = g.mesh.patch("xmin").unwrap().faces().collect();
        for (j, &f) in faces.iter().enumerate() {
            let sf = g.mesh.face_area_vectors()[f];
            for k in 0..3 {
                // a zero area component gives a signed zero on either side: compared as a value
                let same = if sf[k] == 0.0 {
                    loads[j][k] == 0.0
                } else {
                    (sf[k] * pr[f - ni]).to_bits() == loads[j][k].to_bits()
                };
                if !same {
                    e1 += 1;
                }
            }
        }
        for i in 0..pr.len() {
            if !faces.contains(&(ni + i)) && pr[i].to_bits() != pd[i].to_bits() {
                e1 += 1;
            }
        }
        let (fc, _) = patch_force_moment_wall_gradient(g, &fl.u, &fl.p, 1.0, 0.0, &wall, vec3::ZERO, &kept).unwrap();
        let r = reports(&s);
        let rel = (r[2] - fc[0]).abs() / fc[0].abs();
        e2 = e2.max(rel);
        if r[1].to_bits() != reports(&s0)[1].to_bits() {
            e3 += 1;
        }
        println!(
            "E step {n}: fp {:.9e}, corrected pressure force {:.9e}, relative {rel:.2e}",
            r[2], fc[0]
        );
    }
    println!("E1 {e1} differing values; E2 largest relative {e2:.2e}; E3 {e3} steps differ");
    assert_eq!(e1, 0);
    assert!(e2 <= 1e-14, "{e2}");
    assert_eq!(e3, 0);
}

/// ADR 0342 A1 (the acceptance), A1c, A2, A3, A4: the resting wall of AU1b.
#[test]
fn a1_a4_the_resting_wall_of_au1b() {
    let key = worst("rest", PRESCRIBED, 0.0);
    let def = worst("rest", "", 1.0 / 40.0);
    println!(
        "A rest, key: worst cf {:.3e}, fp {:.3e}, ft {:.3e}, my {:.3e}; no table: cf {:.7e}, fp {:.7e}, ft {:.7e}, my {:.7e}",
        key[0], key[1], key[2], key[3], def[0], def[1], def[2], def[3]
    );
    // A1 (acceptance) and its control A1c
    assert!(key[1].abs() <= 1e-4, "A1 {}", key[1]);
    assert!((def[1] - 2.500460e-2).abs() <= 1e-8 && def[1] > 1e-4, "A1c {}", def[1]);
    // A2
    assert!(key[2].abs() <= 1e-4, "A2 {}", key[2]);
    // A3
    assert!(key[3].abs() <= 1e-4, "A3 {}", key[3]);
    assert!((def[3] / (1.0 / 40.0) - 1.0).abs() <= 0.01, "A3 no table {}", def[3]);
    // A4
    assert!(key[0].abs() <= 1e-4, "A4 {}", key[0]);
    assert!((def[0] / (1.0 / 40.0) - 1.0).abs() <= 0.01, "A4 no table {}", def[0]);
}

/// ADR 0342 F1: the same column with `xmin` a following wall.
#[test]
fn f1_the_following_wall() {
    let key = worst("follow", PRESCRIBED, 0.0);
    let def = worst("follow", "", 1.0 / 40.0);
    println!(
        "F1 follow, key: worst cf {:.3e}, fp {:.3e}, ft {:.3e}, my {:.3e}; no table: fp {:.7e}",
        key[0], key[1], key[2], key[3], def[1]
    );
    for (k, v) in key.iter().enumerate() {
        assert!(v.abs() <= 1e-4, "F1 {k}: {v}");
    }
    assert!((def[1] / (1.0 / 40.0) - 1.0).abs() <= 0.01, "F1 no table {}", def[1]);
}

/// Diagnostic paths of a text: parse errors, then the static checks.
fn paths(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.iter().map(|x| x.path.clone()).collect(),
        Ok(c) => c.semantic_errors().iter().map(|x| x.path.clone()).collect(),
    }
}

/// ADR 0342 N: the static checks are located at the key.
#[test]
fn n_static_checks_are_located() {
    let bogus = column("rest", "[dynamic_mesh.force_reports]\nwall_pressure = \"bogus\"\n");
    let layering = column("follow", PRESCRIBED).replace("deformation = \"rigid\"", "deformation = \"layering\"");
    let comp = column("rest", PRESCRIBED).replace("algorithm = \"piso\"", "algorithm = \"compressible\"");
    let off = column("rest", PRESCRIBED).replace("enabled = true", "enabled = false");
    assert!(layering.contains("\"layering\"") && comp.contains("\"compressible\"") && off.contains("= false"));
    let pb = paths(&bogus);
    let pl = paths(&layering);
    let pc = paths(&comp);
    let pv = paths(&column("rest", PRESCRIBED));
    let po = paths(&off);
    println!("N: bogus {pb:?}; layering {pl:?}; compressible {pc:?}; valid {pv:?}; disabled {po:?}");
    assert_eq!(pb, vec![KEY.to_string()]);
    assert!(pl.iter().any(|p| p == KEY));
    assert!(pc.iter().any(|p| p == KEY));
    assert!(pv.is_empty());
    assert!(po.is_empty());
}
