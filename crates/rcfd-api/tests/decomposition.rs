//! Case-tree partitioning (ADR 0021, checks D1 to D11): the `decomposition` section against the library calls it
//! wraps, on a 400-cell box and a 576-cell curved O-grid.

mod wiring_common;

use rcfd_api::case::Case;
use rcfd_api::decomp;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_linalg::reorder::{bandwidth, rcm_order};
use rcfd_mesh::partition::{
    edge_cut, smooth_partition, weighted_imbalance, Bisection, BisectionRule, Decomposition, GraphPartitioner,
    Partitioner, SectorKind, Sectors,
};
use rcfd_mesh::Mesh;
use serde_json::json;
use wiring_common::*;

const Q1: &str = "[mesh]\ngenerator = \"box_2d\"\nnx = 40\nny = 10\nlx = 4.0\nly = 1.0\n";
const Q2: &str = "[mesh]\ngenerator = \"cylinder_ogrid\"\nnx = 48\nny = 12\nr_in = 1.0\nr_out = 5.0\ngrading = 4.0\n";
const Q3: &str = "[mesh]\ngenerator = \"box_2d\"\nnx = 40\nny = 4\nlx = 4.0\nly = 0.4\n";
const PARTS: [usize; 5] = [2, 3, 5, 8, 16];

fn case(mesh: &str, decomposition: &str) -> Case {
    Case::parse_toml(&format!("{mesh}[decomposition]\n{decomposition}")).unwrap_or_else(|e| panic!("{e:?}"))
}

fn centre(mesh: &Mesh) -> [f64; 3] {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in mesh.points() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    [0, 1, 2].map(|k| 0.5 * (lo[k] + hi[k]))
}

/// A strategy as case-tree text and as the library object it must equal.
struct Strategy {
    name: &'static str,
    toml: String,
    lib: Box<dyn Partitioner>,
}

fn strategies(mesh: &Mesh, weights: Option<Vec<f64>>) -> Vec<Strategy> {
    let c = centre(mesh);
    let w = weights;
    let bis = |name: &'static str, rule: &str, r: BisectionRule| Strategy {
        name,
        toml: format!("method = \"bisection\"\nrule = \"{rule}\"\n"),
        lib: Box::new(Bisection {
            rule: r,
            weights: w.clone(),
        }),
    };
    let sec = |name: &'static str, kind: &str, k: SectorKind| {
        Strategy {
        name,
        toml: format!(
            "method = \"sectors\"\nsector_kind = \"{kind}\"\norigin = [{:?}, {:?}, {:?}]\naxis = [0.0, 0.0, 1.0]\nreference = [1.0, 0.0, 0.0]\n",
            c[0], c[1], c[2]
        ),
        lib: Box::new(Sectors {
            kind: k,
            origin: c,
            axis: [0.0, 0.0, 1.0],
            reference: [1.0, 0.0, 0.0],
            weights: w.clone(),
        }),
    }
    };
    vec![
        bis("longest", "longest", BisectionRule::Longest),
        bis("cycle", "cycle", BisectionRule::Cycle),
        bis("x", "x", BisectionRule::Fixed(0)),
        bis("y", "y", BisectionRule::Fixed(1)),
        bis("z", "z", BisectionRule::Fixed(2)),
        Strategy {
            name: "inertial",
            toml: "method = \"inertial\"\n".into(),
            lib: Box::new(Bisection {
                rule: BisectionRule::Inertial,
                weights: w.clone(),
            }),
        },
        sec("radial", "radial", SectorKind::Radial),
        sec("circumferential", "circumferential", SectorKind::Circumferential),
        Strategy {
            name: "graph",
            toml: "method = \"graph\"\n".into(),
            lib: Box::new(GraphPartitioner {
                weights: w,
                tolerance: 0.03,
                max_passes: 20,
            }),
        },
    ]
}

fn independent_cut(mesh: &Mesh, part: &[usize]) -> usize {
    let mut n = 0;
    for f in 0..mesh.n_internal_faces() {
        if part[mesh.owner()[f]] != part[mesh.neighbour()[f]] {
            n += 1;
        }
    }
    n
}

#[test]
fn d1_to_d4_case_tree_equals_the_library_covers_balances_and_cuts() {
    let mut worst: f64 = 0.0;
    let mut cases = 0;
    for mesh_toml in [Q1, Q2] {
        let mesh = build_mesh(&Case::parse_toml(mesh_toml).unwrap()).unwrap();
        let strats = strategies(&mesh, None);
        for &n in &PARTS {
            let mut cuts = std::collections::BTreeMap::new();
            for s in &strats {
                let c = case(mesh_toml, &format!("{}parts = {n}\n", s.toml));
                let r = decomp::run(&c, &mesh).unwrap_or_else(|e| panic!("{} {n}: {e:?}", s.name));
                // D1: equals the direct library call
                let direct = s.lib.partition(&mesh, n).unwrap();
                let diff = r.part.iter().zip(&direct).filter(|(a, b)| a != b).count();
                assert_eq!(diff, 0, "{} n={n}: {diff} differences", s.name);
                // D2: cover
                assert_eq!(r.part.len(), mesh.n_cells());
                assert!(r.part.iter().all(|&p| p < n));
                assert!(r.part_cells.iter().all(|&c| c > 0), "{} n={n}", s.name);
                assert_eq!(r.part_cells.iter().sum::<usize>(), mesh.n_cells());
                Decomposition::from_assignment(&mesh, n, r.part.clone()).unwrap();
                // D3: balance
                assert_eq!(r.imbalance.to_bits(), weighted_imbalance(&r.part, n, None).to_bits());
                assert!(r.imbalance <= 0.05, "{} n={n}: imbalance {}", s.name, r.imbalance);
                worst = worst.max(r.imbalance);
                // D4: cut
                assert_eq!(r.edge_cut, independent_cut(&mesh, &r.part));
                assert_eq!(r.edge_cut, edge_cut(&mesh, &r.part));
                cuts.insert(s.name, r.edge_cut);
                cases += 1;
            }
            assert!(
                cuts["graph"] <= cuts["longest"],
                "n={n}: graph cut {} > bisection cut {}",
                cuts["graph"],
                cuts["longest"]
            );
        }
    }
    println!("D1-D4: {cases} cases, worst unit-weight imbalance {worst:.4} (limit 0.05)");
}

fn half_weights(mesh: &Mesh) -> (Vec<f64>, f64) {
    let mid = centre(mesh)[0];
    (
        mesh.cell_centres()
            .iter()
            .map(|c| if c[0] >= mid { 4.0 } else { 1.0 })
            .collect(),
        mid,
    )
}

#[test]
fn d5_weights_through_the_case_tree() {
    let mut worst: f64 = 0.0;
    for mesh_toml in [Q1, Q2] {
        let mesh = build_mesh(&Case::parse_toml(mesh_toml).unwrap()).unwrap();
        let (w, mid) = half_weights(&mesh);
        let weights_toml = format!(
            "[decomposition.weights]\nmode = \"box\"\nbox_min = [{mid:?}, -1000.0, -1000.0]\nbox_max = [1000.0, 1000.0, 1000.0]\ninside = 4.0\noutside = 1.0\n"
        );
        let strats = strategies(&mesh, Some(w.clone()));
        let mut unweighted_bad = false;
        for &n in &[2usize, 4, 8, 16] {
            for s in strats
                .iter()
                .filter(|s| matches!(s.name, "longest" | "inertial" | "graph"))
            {
                // the table [decomposition.weights] must follow the plain keys of [decomposition]
                let c = Case::parse_toml(&format!(
                    "{mesh_toml}[decomposition]\n{}parts = {n}\n{weights_toml}",
                    s.toml
                ))
                .unwrap_or_else(|e| panic!("{e:?}"));
                let r = decomp::run(&c, &mesh).unwrap();
                let direct = s.lib.partition(&mesh, n).unwrap();
                assert_eq!(r.part, direct, "{} n={n}", s.name);
                assert!(
                    r.imbalance <= 0.05,
                    "{} n={n}: weighted imbalance {}",
                    s.name,
                    r.imbalance
                );
                assert_eq!(
                    r.imbalance.to_bits(),
                    weighted_imbalance(&r.part, n, Some(&w)).to_bits()
                );
                worst = worst.max(r.imbalance);
            }
            // positive control: the same weights ignored
            let unw = Bisection::new(BisectionRule::Longest).partition(&mesh, n).unwrap();
            if weighted_imbalance(&unw, n, Some(&w)) > 0.05 {
                unweighted_bad = true;
            }
        }
        assert!(
            unweighted_bad,
            "positive control: an unweighted partition should be unbalanced under the weights"
        );
    }
    println!("D5: worst weighted imbalance {worst:.4} (limit 0.05)");
}

#[test]
fn d6_smoothing_equals_partition_then_smooth() {
    for mesh_toml in [Q1, Q2] {
        let mesh = build_mesh(&Case::parse_toml(mesh_toml).unwrap()).unwrap();
        for &n in &[3usize, 8] {
            let plain = decomp::run(
                &case(mesh_toml, &format!("method = \"inertial\"\nparts = {n}\n")),
                &mesh,
            )
            .unwrap();
            let sm = decomp::run(
                &case(
                    mesh_toml,
                    &format!("method = \"inertial\"\nparts = {n}\nsmooth = true\n"),
                ),
                &mesh,
            )
            .unwrap();
            let mut direct = Bisection::new(BisectionRule::Inertial).partition(&mesh, n).unwrap();
            let rep = smooth_partition(&mesh, &mut direct, n, None, 0.03, 20).unwrap();
            assert_eq!(sm.part, direct);
            assert_eq!(sm.smoothed_moves, Some(rep.moved));
            assert!(sm.edge_cut <= plain.edge_cut);
            assert!(sm.part.iter().all(|&p| p < n) && sm.part_cells.iter().sum::<usize>() == mesh.n_cells());
        }
    }
}

fn flow_case_on(mesh: &str, extra: &str) -> Case {
    Case::parse_toml(&format!(
        "{mesh}[physics]\nnu = 0.05\n[boundaries.xmin]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.xmax]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\n[boundaries.ymin]\nvelocity = \"fixed\"\n[boundaries.ymax]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"simplec\"\nmax_iterations = 5\n{extra}"
    ))
    .unwrap_or_else(|e| panic!("{e:?}"))
}

fn flow_case(extra: &str) -> Case {
    flow_case_on(Q1, extra)
}

#[test]
fn d7_reorder_reports_library_bandwidths_and_changes_nothing() {
    let mesh = build_mesh(&Case::parse_toml(Q3).unwrap()).unwrap();
    let c = case(Q3, "method = \"graph\"\nparts = 4\nreorder = true\n");
    let r = decomp::run(&c, &mesh).unwrap();
    let nf = mesh.n_internal_faces();
    let (lo, up) = (&mesh.owner()[..nf], mesh.neighbour());
    let order = rcm_order(mesh.n_cells(), lo, up);
    let b = r.bandwidth.expect("bandwidth reported");
    assert_eq!(b.natural, bandwidth(lo, up, None));
    assert_eq!(b.rcm, bandwidth(lo, up, Some(&order)));
    println!("D7: natural {} rcm {} (bound 7)", b.natural, b.rcm);
    assert!(b.rcm <= 7, "rcm bandwidth {}", b.rcm);
    // not requested: absent
    let r2 = decomp::run(&case(Q3, "method = \"graph\"\nparts = 4\n"), &mesh).unwrap();
    assert!(r2.bandwidth.is_none());

    // a simulation of a case with reorder = true is bit-identical to one without
    let run = |extra: &str| {
        let c = flow_case_on(Q3, extra);
        let mut s = Simulation::new(&c).unwrap();
        s.run(5);
        (s.field("ux").unwrap(), s.field("p").unwrap())
    };
    let a = run("");
    let b = run("[decomposition]\nmethod = \"graph\"\nparts = 4\nreorder = true\nsmooth = true\n");
    assert!(a.0.iter().zip(&b.0).all(|(x, y)| x.to_bits() == y.to_bits()));
    assert!(a.1.iter().zip(&b.1).all(|(x, y)| x.to_bits() == y.to_bits()));
}

#[test]
fn d8_partition_field_in_the_vtu_and_default_files_unchanged() {
    let dir = tmpdir("d8");
    let c = flow_case("[decomposition]\nmethod = \"bisection\"\nparts = 7\nwrite_field = true\n");
    let sim = Simulation::new(&c).unwrap();
    let p = sim.export_vtu(&dir, "with").unwrap();
    let text = std::fs::read_to_string(p).unwrap();
    let arr = vtk_array(&text, "partition");
    let rep = decomp::run(&c, &sim.geom.mesh).unwrap();
    assert_eq!(arr.len(), 400);
    assert_eq!(arr.iter().zip(&rep.part).filter(|(a, b)| **a != **b as f64).count(), 0);

    // default run: no partition array, byte-identical with explicit inert sections
    let plain = Simulation::new(&flow_case("")).unwrap();
    let inert = Simulation::new(&flow_case("[decomposition]\nmethod = \"none\"\n[postprocessing]\n")).unwrap();
    let a = std::fs::read(plain.export_vtu(&dir, "plain").unwrap()).unwrap();
    let b = std::fs::read(inert.export_vtu(&dir, "inert").unwrap()).unwrap();
    assert_eq!(a, b);
    let t = String::from_utf8(a).unwrap();
    assert!(!has_array(&t, "partition") && !has_array(&t, "vorticity") && !has_array(&t, "q_criterion"));
}

#[test]
fn d10_located_validation_errors() {
    let bad = |decomposition: &str| -> Vec<rcfd_api::case::Diagnostic> {
        Case::parse_toml(&format!("{Q1}[decomposition]\n{decomposition}")).unwrap_err()
    };
    let cases: [(&str, &str); 8] = [
        ("parts = 0\n", "decomposition.parts"),
        ("method = \"none\"\nparts = 4\n", "decomposition.method"),
        (
            "method = \"sectors\"\nparts = 4\naxis = [0.0, 0.0, 0.0]\n",
            "decomposition.axis",
        ),
        (
            "method = \"sectors\"\nparts = 4\nreference = [0.0, 0.0, 1.0]\n",
            "decomposition.reference",
        ),
        (
            "method = \"graph\"\nparts = 4\n[decomposition.weights]\nmode = \"box\"\nbox_min = [1.0, 0.0, 0.0]\nbox_max = [0.0, 1.0, 1.0]\n",
            "decomposition.weights.box_max",
        ),
        (
            "method = \"graph\"\nparts = 4\n[decomposition.weights]\nmode = \"box\"\ninside = 0.0\n",
            "decomposition.weights.inside",
        ),
        (
            "method = \"graph\"\nparts = 4\ntolerance = -1.0\n",
            "decomposition.tolerance",
        ),
        ("method = \"metis\"\nparts = 4\n", "decomposition.method"),
    ];
    for (text, path) in cases {
        let errs = bad(text);
        let e = errs
            .iter()
            .find(|e| e.path == path)
            .unwrap_or_else(|| panic!("{path}: not in {errs:?}"));
        assert!(e.line.is_some(), "{path}: no line number: {e:?}");
    }
    // write_field without a method
    let errs = bad("write_field = true\n");
    assert!(errs.iter().any(|e| e.path == "decomposition.write_field"), "{errs:?}");
    // parts larger than the cell count: from the library wrapper and from Simulation::new
    let c = case(Q1, "method = \"graph\"\nparts = 401\n");
    let mesh = build_mesh(&c).unwrap();
    let e = decomp::run(&c, &mesh).unwrap_err();
    assert_eq!(e[0].path, "decomposition.parts");
    let c = flow_case("[decomposition]\nmethod = \"graph\"\nparts = 401\n");
    let e = Simulation::new(&c).err().expect("Simulation::new refuses");
    assert!(e.iter().any(|d| d.path == "decomposition.parts"), "{e:?}");
    // positive control
    case(Q1, "method = \"graph\"\nparts = 400\n");
}

#[test]
fn d11_service_method_returns_the_library_report() {
    let svc = Service::new(ServiceConfig::default());
    let toml = format!("{Q1}[decomposition]\nmethod = \"graph\"\nparts = 6\nreorder = true\n");
    let id = svc.call("session.create", &json!({ "toml": toml })).unwrap()["session"]
        .as_str()
        .unwrap()
        .to_string();
    let r = svc.call("mesh.partition", &json!({"session": id})).unwrap();
    let c = Case::parse_toml(&toml).unwrap();
    let mesh = build_mesh(&c).unwrap();
    let want = decomp::report_json(&decomp::run(&c, &mesh).unwrap(), false);
    assert_eq!(r, want);
    // override and per-cell vector
    let r = svc
        .call(
            "mesh.partition",
            &json!({"session": id, "method": "bisection", "parts": 4, "with_parts": true}),
        )
        .unwrap();
    assert_eq!(r["n_parts"], 4);
    assert_eq!(r["part"].as_array().unwrap().len(), 400);
    // invalid override is a validation error
    assert!(svc.call("mesh.partition", &json!({"session": id, "parts": 0})).is_err());
}
