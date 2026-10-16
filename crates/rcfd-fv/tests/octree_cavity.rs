//! F-234 (ADR 0047 check O9): lid-driven cavity on a mesh produced by the octree fill (`rcfd_mesh::octree`).
//!
//! The fill is `Region::InteriorOf(box surface)` over the unit-square slab with `40 x 40 x 1` root cells and no refinement, so the
//! mesh has no hanging nodes and no cut cell (a refined single-layer 2-D slab is impossible with cubic cells); the test checks the
//! whole fill path (classification, face emission, merged patches) in a flow solve. Limit: absolute error divided by the lid speed
//! at the tabulated Ghia et al. (1982) centreline rows at most 0.03, with linear interpolation between the bracketing cell values.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::octree::{fill, FillSpec, Region};
use rcfd_mesh::surface::box_surface;
use rcfd_mesh::PatchKind;
use std::path::Path;
use std::sync::Arc;

fn read_csv(name: &str) -> Vec<Vec<f64>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../cases/cavity/reference")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(|v| v.trim().parse().unwrap()).collect())
        .collect()
}

fn cavity_mesh(n: usize) -> rcfd_mesh::Mesh {
    let h = 1.0 / n as f64;
    let surf = box_surface([0.0; 3], [1.0, 1.0, h], [2, 2, 1]);
    let wall = |s: &str| (s.to_string(), PatchKind::Wall);
    let (m, rep) = fill(&FillSpec {
        lo: [0.0; 3],
        hi: [1.0, 1.0, h],
        roots: [n, n, 1],
        max_level: 0,
        band: 3.0,
        margin: 0.0,
        region: Region::InteriorOf(&surf),
        layers: None,
        box_patches: [
            wall("walls"),
            wall("walls"),
            wall("walls"),
            wall("lid"),
            ("frontAndBack".into(), PatchKind::Empty),
            ("frontAndBack".into(), PatchKind::Empty),
        ],
        body_patch: wall("body"),
    })
    .unwrap();
    assert_eq!(rep.n_core, n * n);
    assert_eq!(rep.n_hex, n * n);
    m
}

/// Velocity component `comp` on the centreline (`x = 0.5` for comp 0, `y = 0.5` for comp 1) at the coordinate `at` along it:
/// the two columns bracketing the line are averaged with linear weights, then linear interpolation between cell centres, the
/// wall values (0 at 0; the lid speed for u at 1, 0 otherwise) closing the line.
fn centreline(solver: &SimpleSolver, comp: usize, at: f64) -> f64 {
    let g = &solver.geom;
    let n = (g.n_cells() as f64).sqrt().round() as usize;
    let cc = g.mesh.cell_centres();
    let (vary, fixed) = if comp == 0 { (1, 0) } else { (0, 1) };
    let h = 1.0 / n as f64;
    let mut line = vec![0.0; n];
    let mut count = vec![0.0; n];
    for c in 0..g.n_cells() {
        let along = (cc[c][vary] / h).floor() as usize;
        let w = (1.0 - (cc[c][fixed] - 0.5).abs() / h).max(0.0);
        if w > 0.0 {
            line[along] += w * solver.u.values[c][comp];
            count[along] += w;
        }
    }
    let mut xs = vec![0.0];
    let mut ys = vec![0.0];
    for i in 0..n {
        xs.push((i as f64 + 0.5) * h);
        ys.push(line[i] / count[i]);
    }
    xs.push(1.0);
    ys.push(if comp == 0 { 1.0 } else { 0.0 });
    let j = xs.partition_point(|&v| v < at).clamp(1, xs.len() - 1);
    let t = (at - xs[j - 1]) / (xs[j] - xs[j - 1]);
    ys[j - 1] + t * (ys[j] - ys[j - 1])
}

#[test]
fn o9_lid_driven_cavity_on_the_generated_mesh_matches_ghia() {
    let n = 40;
    let mesh = Arc::new(cavity_mesh(n));
    assert_eq!(mesh.n_cells(), 1600);
    assert!((mesh.total_volume() - 1.0 / n as f64).abs() < 1e-12);
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let re = 100.0;
    let mut s = SimpleSettings::new(1.0 / re, Coupling::Simplec);
    s.convection = ConvScheme::LinearUpwind;
    let mut solver = SimpleSolver::new(geom, u, p, s);
    let sum = solver.run(3000, 1e-7);
    println!("O9: {} iterations, converged {}", sum.iterations, sum.converged);
    assert!(sum.converged);
    let u_ref = read_csv("ghia1982_u_centerline.csv");
    let v_ref = read_csv("ghia1982_v_centerline_Re100.csv");
    let eu = u_ref
        .iter()
        .map(|r| (centreline(&solver, 0, r[0]) - r[1]).abs())
        .fold(0.0, f64::max);
    let ev = v_ref
        .iter()
        .map(|r| (centreline(&solver, 1, r[0]) - r[1]).abs())
        .fold(0.0, f64::max);
    println!("O9: max |u - Ghia| = {eu:.5}, max |v - Ghia| = {ev:.5} (lid speed 1, limit 0.03)");
    assert!(eu <= 0.03, "{eu}");
    assert!(ev <= 0.03, "{ev}");
}
