//! ADR 0738 (F-809): iso-surfaces, streamlines and the Studio's cut and arrows as the engine computes them.
//!
//! - I1 a cell field linear in y on a box (uniform, and graded along x): the iso-surface is the plane `cut_plane` gives,
//!   to 1e-12 relative; I2 the distance to a point on 16^3, 32^3, 64^3: the area of the sphere converges with order in
//!   [1.8, 2.2]; I3 `cut_implicit` after the split into `cut_values` equals the base commit's (its text is copied here as
//!   `base_cut_implicit`) bit for bit.
//! - S1 uniform flow: straight lines to 1e-12, `left_domain`, and the cell of every point is `locate_cell`'s; S2 solid
//!   body rotation with a speed that varies with the radius: one revolution's radius error falls with the mesh with order
//!   in [1.8, 2.2] on a graded mesh (linear interpolation over tets; at least 1.8 on a uniform one, where the symmetric
//!   means superconverge, about 3.2), and the phase error with the analytic velocity falls with order in [3.8, 4.2] in the
//!   step (RK4, step fractions 0.4, 0.2, 0.1); S3 the stops `stagnant`, `max_steps` and `max_length`.
//! - T1 to T3 as the engine's rules: the 2D verdict, the cut (every kept point on the kept side, whole triangles bit for
//!   bit, the kept area exact on a box), the arrows (one per lattice bin, on the front face of a 2D case) and their cut;
//!   G1 the GLB passes `validate_glb` and reads back to the `f32` of what was written.

use rcfd_core::vec3::{self, Vec3};
use rcfd_core::Result;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_post::iso::{area, iso_surface};
use rcfd_post::picture::{
    add_triangles, clip_segments, clip_surface, mesh_layers, read_primitives, store_surface, vector_glyphs, Attribute,
    Cut, GlbBuilder,
};
use rcfd_post::streamlines::{trace, trace_with, Directions, Stop, TraceOptions};
use rcfd_post::surfaces::{cut_implicit, cut_plane, Surface, CULL_RATIO};
use std::collections::HashMap;
use std::sync::Arc;

const U: Grading = Grading::Uniform;

/// The base commit's `cut_implicit` (aee95d4a), copied unchanged but for its name and error constructor.
fn base_cut_implicit(mesh: &Mesh, g: impl Fn(Vec3) -> f64) -> Result<Surface> {
    let np = mesh.n_points();
    let nf = mesh.n_faces();
    let nc = mesh.n_cells();
    let pos = |id: usize| -> Vec3 {
        if id < np {
            mesh.points()[id]
        } else if id < np + nf {
            mesh.face_centres()[id - np]
        } else {
            mesh.cell_centres()[id - np - nf]
        }
    };
    let gv: Vec<f64> = (0..np + nf + nc).map(|i| g(pos(i))).collect();
    if gv.iter().any(|x| !x.is_finite()) {
        return Err(rcfd_core::Error::Other(
            "cut_implicit: the implicit function is not finite at a mesh location".into(),
        ));
    }
    let mut surf = Surface::default();
    let mut vmap: HashMap<(usize, usize), usize> = HashMap::new();
    for c in 0..nc {
        let cid = np + nf + c;
        for &f in mesh.cell_faces(c) {
            let fp = mesh.face_points(f);
            for k in 0..fp.len() {
                let tet = [cid, np + f, fp[k], fp[(k + 1) % fp.len()]];
                let ids: Vec<usize> = (0..4).collect();
                let posi: Vec<usize> = ids.iter().copied().filter(|&i| gv[tet[i]] >= 0.0).collect();
                let negi: Vec<usize> = ids.iter().copied().filter(|&i| gv[tet[i]] < 0.0).collect();
                if posi.is_empty() || negi.is_empty() {
                    continue;
                }
                let mut vertex = |a: usize, b: usize, surf: &mut Surface| -> usize {
                    // a positive (g >= 0), b negative
                    let (ia, ib) = (tet[a], tet[b]);
                    let key = if gv[ia] == 0.0 {
                        (ia, ia)
                    } else {
                        (ia.min(ib), ia.max(ib))
                    };
                    *vmap.entry(key).or_insert_with(|| {
                        let p = if gv[ia] == 0.0 {
                            pos(ia)
                        } else {
                            let t = gv[ia] / (gv[ia] - gv[ib]);
                            vec3::add(pos(ia), vec3::scale(vec3::sub(pos(ib), pos(ia)), t))
                        };
                        surf.points.push(p);
                        surf.vertex_cell.push(Some(c));
                        surf.points.len() - 1
                    })
                };
                let mut polys: Vec<[usize; 3]> = Vec::new();
                match (posi.len(), negi.len()) {
                    (1, 3) => {
                        let a = posi[0];
                        let v: Vec<usize> = negi.iter().map(|&b| vertex(a, b, &mut surf)).collect();
                        polys.push([v[0], v[1], v[2]]);
                    }
                    (3, 1) => {
                        let b = negi[0];
                        let v: Vec<usize> = posi.iter().map(|&a| vertex(a, b, &mut surf)).collect();
                        polys.push([v[0], v[1], v[2]]);
                    }
                    _ => {
                        // two positive, two negative: quad around the four crossing edges
                        let (a0, a1, b0, b1) = (posi[0], posi[1], negi[0], negi[1]);
                        let q = [
                            vertex(a0, b0, &mut surf),
                            vertex(a0, b1, &mut surf),
                            vertex(a1, b1, &mut surf),
                            vertex(a1, b0, &mut surf),
                        ];
                        polys.push([q[0], q[1], q[2]]);
                        polys.push([q[0], q[2], q[3]]);
                    }
                }
                let mp = posi.iter().fold([0.0; 3], |s, &i| vec3::add(s, pos(tet[i])));
                let mn = negi.iter().fold([0.0; 3], |s, &i| vec3::add(s, pos(tet[i])));
                let up = vec3::sub(
                    vec3::scale(mp, 1.0 / posi.len() as f64),
                    vec3::scale(mn, 1.0 / negi.len() as f64),
                );
                for mut t in polys {
                    let n = vec3::cross(
                        vec3::sub(surf.points[t[1]], surf.points[t[0]]),
                        vec3::sub(surf.points[t[2]], surf.points[t[0]]),
                    );
                    // numerically degenerate facets (crossing within rounding of a tet vertex) have a noise normal and
                    // no area: drop them (area / (longest tet edge)^2 below CULL_RATIO)
                    let e2 = (0..4)
                        .flat_map(|a| (a + 1..4).map(move |b| (a, b)))
                        .map(|(a, b)| {
                            let d = vec3::sub(pos(tet[a]), pos(tet[b]));
                            vec3::dot(d, d)
                        })
                        .fold(0.0, f64::max);
                    if vec3::norm(n) <= CULL_RATIO * e2 {
                        continue;
                    }
                    if vec3::dot(n, up) < 0.0 {
                        t.swap(1, 2);
                    }
                    surf.tris.push(t);
                    surf.tri_cell.push(Some(c));
                }
            }
        }
    }
    Ok(surf)
}

fn same(a: &Surface, b: &Surface) -> bool {
    a.tris == b.tris
        && a.tri_cell == b.tri_cell
        && a.vertex_cell == b.vertex_cell
        && a.points.len() == b.points.len()
        && a.points
            .iter()
            .zip(&b.points)
            .all(|(p, q)| (0..3).all(|k| p[k].to_bits() == q[k].to_bits()))
}

#[test]
fn i3_cut_implicit_is_the_base_commits_bit_for_bit() {
    let meshes = [
        box_mesh(
            [7, 5, 4],
            [1.0, 0.8, 0.6],
            [Grading::Expansion(2.5), U, Grading::Symmetric(1.7)],
        )
        .unwrap(),
        box_mesh([12, 12, 12], [1.0, 1.0, 1.0], [U, U, U]).unwrap(),
        rcfd_mesh::gen::cavity_2d(20, 1.0).unwrap(),
    ];
    let funcs: [&dyn Fn(Vec3) -> f64; 4] = [
        &|p| p[0] - 0.4,
        &|p| vec3::norm(vec3::sub(p, [0.5, 0.4, 0.3])) - 0.27,
        &|p| (3.0 * p[0]).sin() + p[1] * p[1] - 0.5,
        // a level through mesh points exactly (the g = 0 vertex case)
        &|p| p[1] - 0.5,
    ];
    let mut n = 0;
    for m in &meshes {
        for g in funcs {
            let a = cut_implicit(m, g).unwrap();
            let b = base_cut_implicit(m, g).unwrap();
            assert!(same(&a, &b), "I3: cut_implicit differs from the base commit's");
            n += a.tris.len();
        }
    }
    eprintln!("I3: {} cuts, {n} triangles, bit for bit", meshes.len() * funcs.len());
}

fn linear_y_case(m: &Mesh, level_y: f64) -> (f64, f64, usize) {
    let v: Vec<f64> = m.cell_centres().iter().map(|c| 2.0 * c[1] + 1.0).collect();
    let iso = iso_surface(m, &v, 2.0 * level_y + 1.0, None).unwrap();
    let plane = cut_plane(m, [0.0, level_y, 0.0], [0.0, 1.0, 0.0]).unwrap();
    let worst = iso
        .surface
        .points
        .iter()
        .map(|p| (p[1] - level_y).abs())
        .fold(0.0, f64::max);
    let (ai, ap) = (area(&iso.surface), area(&plane));
    assert_eq!(iso.surface.tris.len(), plane.tris.len());
    ((ai - ap).abs() / ap, worst / level_y, iso.surface.tris.len())
}

#[test]
fn i1_a_linear_field_gives_the_plane() {
    for (name, g) in [("uniform", U), ("graded x", Grading::Expansion(4.0))] {
        let m = box_mesh([9, 12, 7], [1.3, 1.2, 0.7], [g, U, U]).unwrap();
        let (da, dy, n) = linear_y_case(&m, 0.537);
        assert!(da <= 1e-12 && dy <= 1e-12, "I1 {name}: area {da:e}, y {dy:e}");
        eprintln!("I1 {name}: {n} triangles, area relative {da:e}, y relative {dy:e}");
    }
}

#[test]
fn i2_the_sphere_converges_at_second_order() {
    let (c, r) = ([0.5, 0.5, 0.5], 0.3);
    let exact = 4.0 * std::f64::consts::PI * r * r;
    let mut errs = Vec::new();
    for n in [16, 32, 64] {
        let m = box_mesh([n, n, n], [1.0; 3], [U, U, U]).unwrap();
        let v: Vec<f64> = m.cell_centres().iter().map(|p| vec3::norm(vec3::sub(*p, c))).collect();
        let s = iso_surface(&m, &v, r, Some(&v)).unwrap();
        errs.push((area(&s.surface) - exact).abs() / exact);
        assert!(
            s.colour.iter().all(|x| (x - r).abs() < 1e-9),
            "the colour by the iso field is the level"
        );
    }
    let o1 = (errs[0] / errs[1]).log2();
    let o2 = (errs[1] / errs[2]).log2();
    eprintln!("I2: area errors {errs:?}, orders {o1:.3} {o2:.3}");
    assert!((1.8..=2.2).contains(&o2), "I2: observed order {o2}");
}

fn geom(m: &Mesh) -> rcfd_fv::FvGeometry {
    rcfd_fv::FvGeometry::new(Arc::new(m.clone())).unwrap()
}

#[test]
fn s1_uniform_flow_gives_straight_lines() {
    let m = box_mesh([10, 5, 5], [2.0, 1.0, 1.0], [U, U, U]).unwrap();
    let n = m.n_cells();
    let (ux, uy, uz) = (vec![1.0; n], vec![0.0; n], vec![0.0; n]);
    let seeds = [[0.02, 0.43, 0.57], [0.33, 0.11, 0.89], [1.01, 0.5, 0.31]];
    let lines = trace(&m, [&ux, &uy, &uz], &seeds, &TraceOptions::default()).unwrap();
    let g = geom(&m);
    let mut pts = 0;
    for (l, s) in lines.iter().zip(&seeds) {
        assert_eq!(l.stop, Stop::LeftDomain);
        assert!(l.points.len() > 5);
        let last = l.points.last().unwrap();
        assert!(
            last[0] > 2.0 - 0.25 * 0.2 - 1e-12,
            "the last point is within a step of the outlet: {last:?}"
        );
        for (p, &c) in l.points.iter().zip(&l.cells) {
            assert!(
                (p[1] - s[1]).abs() <= 1e-12 && (p[2] - s[2]).abs() <= 1e-12,
                "S1: {p:?} off the line"
            );
            assert_eq!(
                Some(c),
                rcfd_post::derived::locate_cell(&g, *p),
                "S1: the cell of {p:?}"
            );
            pts += 1;
        }
        for w in l.points.windows(2) {
            assert!(w[1][0] > w[0][0]);
        }
    }
    eprintln!("S1: {} lines, {pts} points straight, left_domain", lines.len());
}

fn rotation(p: Vec3) -> Vec3 {
    let (x, y) = (p[0] - 1.0, p[1] - 1.0);
    let w = 1.0 + x * x + y * y;
    [-w * y, w * x, 0.0]
}

/// The largest relative radius error over one revolution of the stored (interpolated) rotation, traced with a fine step.
fn revolution_error(m: &Mesh) -> f64 {
    let r0 = 0.5;
    let seeds = [[1.0 + r0, 1.0, 0.05]];
    let o = TraceOptions {
        max_length: Some(2.0 * std::f64::consts::PI * r0),
        max_steps: 1_000_000,
        step_fraction: 0.05,
        ..Default::default()
    };
    let u: Vec<Vec3> = m.cell_centres().iter().map(|c| rotation(*c)).collect();
    let comp = |k: usize| u.iter().map(|v| v[k]).collect::<Vec<f64>>();
    let (a, b, c) = (comp(0), comp(1), comp(2));
    let l = &trace(m, [&a, &b, &c], &seeds, &o).unwrap()[0];
    assert_eq!(l.stop, Stop::MaxLength, "one revolution");
    l.points
        .iter()
        .map(|p| ((p[0] - 1.0).hypot(p[1] - 1.0) - r0).abs() / r0)
        .fold(0.0, f64::max)
}

/// The angle error after the steps of one revolution of the analytic rotation (each step `h` of arc length on the circle
/// of radius `r0` turns it by `h / r0`): RK4's global error.
fn phase_error(m: &Mesh, fraction: f64) -> f64 {
    let r0 = 0.5;
    let h = fraction * rcfd_mesh::adapt::cell_lengths(m, [true; 3])[0];
    let steps = (2.0 * std::f64::consts::PI * r0 / h).round() as usize;
    let o = TraceOptions {
        max_steps: steps,
        step_fraction: fraction,
        ..Default::default()
    };
    let l = &trace_with(m, &|_, p| rotation(p), &[[1.0 + r0, 1.0, 0.05]], &o).unwrap()[0];
    assert_eq!((l.stop, l.points.len()), (Stop::MaxSteps, steps + 1));
    let end = l.points.last().unwrap();
    let got = (end[1] - 1.0).atan2(end[0] - 1.0);
    let want = steps as f64 * h / r0;
    let d = (got - want).rem_euclid(2.0 * std::f64::consts::PI);
    d.min(2.0 * std::f64::consts::PI - d)
}

#[test]
fn s2_solid_body_rotation_closes_with_the_orders() {
    // linear over tets: the radius error falls with the mesh at second order at least (a fine step, so the integration
    // error is far below it); on a uniform mesh the errors of opposite sides of the orbit cancel and it falls faster
    let mut errs = Vec::new();
    let mut graded = Vec::new();
    for n in [16, 32, 64] {
        let m = box_mesh([n, n, 1], [2.0, 2.0, 0.1], [U, U, U]).unwrap();
        errs.push(revolution_error(&m));
        let g = Grading::Expansion(3.0);
        let m = box_mesh([n, n, 1], [2.0, 2.0, 0.1], [g, g, U]).unwrap();
        graded.push(revolution_error(&m));
    }
    let om = (errs[1] / errs[2]).log2();
    let og = (graded[1] / graded[2]).log2();
    // RK4 on the analytic velocity: the phase error falls with the step at fourth order
    let m = box_mesh([32, 32, 1], [2.0, 2.0, 0.1], [U, U, U]).unwrap();
    let steps: Vec<f64> = [0.4, 0.2, 0.1].iter().map(|&f| phase_error(&m, f)).collect();
    let os = (steps[1] / steps[2]).log2();
    eprintln!(
        "S2: radius errors uniform {errs:?} order {om:.3}, graded {graded:?} order {og:.3}; RK4 phase errors {steps:?} order {os:.3}"
    );
    assert!(
        (1.8..=2.2).contains(&og),
        "S2: interpolation order on the graded mesh {og}"
    );
    assert!(om >= 1.8, "S2: interpolation order on the uniform mesh {om}");
    assert!((3.8..=4.2).contains(&os), "S2: RK4 order {os}");
}

#[test]
fn s3_the_named_stops() {
    let m = box_mesh([20, 4, 4], [2.0, 1.0, 1.0], [U, U, U]).unwrap();
    let n = m.n_cells();
    let ux: Vec<f64> = m
        .cell_centres()
        .iter()
        .map(|c| if c[0] < 1.0 { 1.0 } else { 0.0 })
        .collect();
    let z = vec![0.0; n];
    let seeds = [[0.05, 0.5, 0.5]];
    let l = &trace(&m, [&ux, &z, &z], &seeds, &TraceOptions::default()).unwrap()[0];
    assert_eq!(l.stop, Stop::Stagnant);
    let end = l.points.last().unwrap()[0];
    assert!(end > 0.9 && end < 1.3, "{end}");
    let uniform = vec![1.0; n];
    let o = TraceOptions {
        max_steps: 5,
        ..Default::default()
    };
    let l = &trace(&m, [&uniform, &z, &z], &seeds, &o).unwrap()[0];
    assert_eq!((l.stop, l.points.len()), (Stop::MaxSteps, 6));
    let o = TraceOptions {
        max_length: Some(0.3),
        ..Default::default()
    };
    let l = &trace(&m, [&uniform, &z, &z], &seeds, &o).unwrap()[0];
    assert_eq!(l.stop, Stop::MaxLength);
    assert!((l.length() - 0.3).abs() < 1e-12, "{}", l.length());
    let o = TraceOptions {
        direction: Directions::Both,
        ..Default::default()
    };
    let both = trace(&m, [&uniform, &z, &z], &seeds, &o).unwrap();
    assert_eq!(both.len(), 2);
    assert_eq!(both[0].stop, Stop::LeftDomain);
    assert!(both[0].points.len() <= 3, "backward from near the inlet");
    // a seed outside the mesh
    let l = &trace(&m, [&uniform, &z, &z], &[[5.0, 0.5, 0.5]], &TraceOptions::default()).unwrap()[0];
    assert_eq!((l.stop, l.points.len()), (Stop::LeftDomain, 0));
    eprintln!("S3: stagnant, max_steps, max_length, both directions and a seed outside stop as named");
}

#[test]
fn t1_the_2d_verdict() {
    let c = rcfd_mesh::gen::cavity_2d(10, 1.0).unwrap();
    let l = mesh_layers(&store_surface(&c).unwrap(), c.n_cells());
    assert!(l.two_d);
    assert_eq!(l.thin, 2);
    let b = box_mesh([4, 4, 4], [1.0; 3], [U, U, U]).unwrap();
    assert!(!mesh_layers(&store_surface(&b).unwrap(), b.n_cells()).two_d);
    let b2 = box_mesh([4, 4, 2], [1.0, 1.0, 0.1], [U, U, U]).unwrap();
    let l2 = mesh_layers(&store_surface(&b2).unwrap(), b2.n_cells());
    assert!(!l2.two_d && l2.thin == 2, "two layers is not 2D");
}

fn tri_area(c: &[[f32; 3]]) -> f64 {
    let p = |i: usize| c[i].map(|x| x as f64);
    0.5 * vec3::norm(vec3::cross(vec3::sub(p(1), p(0)), vec3::sub(p(2), p(0))))
}

#[test]
fn t2_the_cut_keeps_the_side_exactly() {
    let m = box_mesh([7, 6, 5], [1.0, 0.9, 0.8], [Grading::Expansion(2.0), U, U]).unwrap();
    let s = store_surface(&m).unwrap();
    let whole = clip_surface(&s, None);
    assert_eq!(whole.len(), s.n_triangles());
    for (axis, at, below) in [(0, 0.37f32, true), (1, 0.5, false), (2, 0.1234, true)] {
        let cut = Cut { axis, at, below };
        let t = clip_surface(&s, Some(&cut));
        assert!(t.corners.iter().all(|p| cut.keeps(*p)), "every corner kept");
        // whole triangles keep their corners bit for bit
        let mut whole_kept = 0;
        for (i, &src) in t.source.iter().enumerate() {
            let src = src as usize;
            let q = [s.corner(src, 0), s.corner(src, 1), s.corner(src, 2)];
            if q.iter().all(|p| cut.keeps(*p)) {
                assert_eq!(&t.corners[3 * i..3 * i + 3], &q);
                whole_kept += 1;
            }
            assert_eq!(t.cells[i], s.cells[src]);
        }
        // the kept area of a box: the box's surface below (or above) the plane
        let l = [1.0f64, 0.9, 0.8];
        let h = if below { at as f64 } else { l[axis] - at as f64 };
        let (a, b) = match axis {
            0 => (l[1], l[2]),
            1 => (l[0], l[2]),
            _ => (l[0], l[1]),
        };
        let want = a * b + 2.0 * h * (a + b);
        let got: f64 = t.corners.chunks_exact(3).map(tri_area).sum();
        assert!((got - want).abs() < 1e-6 * want, "T2 area {got} want {want}");
        eprintln!(
            "T2: axis {axis} at {at}: {} triangles ({whole_kept} whole), area {got:.7} of {want:.7}",
            t.len()
        );
    }
}

#[test]
fn t3_the_arrows_and_their_cut() {
    let m = box_mesh([30, 30, 30], [1.0; 3], [U, U, U]).unwrap();
    let s = store_surface(&m).unwrap();
    let n = m.n_cells();
    let cc = m.cell_centres();
    let comps: Vec<Vec<f32>> = (0..3)
        .map(|k| cc.iter().map(|c| (c[(k + 1) % 3] - 0.5) as f32).collect())
        .collect();
    let l = mesh_layers(&s, n);
    let g = vector_glyphs(&s, [Some(&comps[0]), Some(&comps[1]), Some(&comps[2])], &l, n);
    // one arrow per lattice bin of the surface
    let mut bins: HashMap<[i64; 3], usize> = HashMap::new();
    for &t in &g.triangles {
        let c = [0, 1, 2].map(|k| (0..3).map(|i| s.corner(t as usize, i)[k] as f64).sum::<f64>() / 3.0);
        *bins.entry(c.map(|x| (x * 24.0).floor().max(0.0) as i64)).or_default() += 1;
    }
    assert!(bins.values().all(|&k| k == 1));
    assert!(
        g.triangles.len() > 2000 && g.triangles.len() <= 6 * 25 * 25,
        "{}",
        g.triangles.len()
    );
    assert_eq!(g.segments.len(), 3 * g.triangles.len());
    // the longest arrow is 0.8 of a lattice step
    let longest = g
        .segments
        .chunks(3)
        .map(|a| vec3::norm(vec3::sub(a[0][1], a[0][0])))
        .fold(0.0, f64::max);
    assert!(
        longest <= 0.8 / 24.0 * (1.0 + 1e-12) && longest > 0.7 / 24.0,
        "{longest}"
    );
    let cut = Cut {
        axis: 0,
        at: 0.5,
        below: true,
    };
    let seg = clip_segments(&g, Some(&cut));
    assert!(seg
        .points
        .iter()
        .all(|p| cut.keeps([p[0], p[1], p[2]]) && cut.keeps([p[3], p[4], p[5]])));
    assert!(seg.points.len() < g.segments.len() && seg.points.len() > g.segments.len() / 3);
    // a 2D case: the arrows on the front face only (z = max), in-plane
    let c2 = rcfd_mesh::gen::cavity_2d(16, 1.0).unwrap();
    let s2 = store_surface(&c2).unwrap();
    let l2 = mesh_layers(&s2, c2.n_cells());
    let u2: Vec<f32> = c2.cell_centres().iter().map(|c| c[1] as f32).collect();
    let g2 = vector_glyphs(&s2, [Some(&u2), Some(&u2), None], &l2, c2.n_cells());
    // two triangles per front cell, at most one arrow per bin of the 24 x 24 lattice
    assert!(
        g2.triangles.len() > 16 * 16 && g2.triangles.len() <= 24 * 24,
        "{}",
        g2.triangles.len()
    );
    assert!(g2.segments.iter().flatten().all(|p| p[2] == l2.max[2]));
    eprintln!(
        "T3: {} arrows on the box, {} strokes kept of {} by the cut; {} on the 2D cavity",
        g.triangles.len(),
        seg.points.len(),
        g.segments.len(),
        g2.triangles.len()
    );
}

#[test]
fn g1_the_glb_passes_and_reads_back() {
    let m = box_mesh([5, 4, 3], [1.0; 3], [U, U, U]).unwrap();
    let s = store_surface(&m).unwrap();
    let t = clip_surface(
        &s,
        Some(&Cut {
            axis: 1,
            at: 0.41,
            below: true,
        }),
    );
    let vals: Vec<f32> = t.cells.iter().map(|&c| c as f32 * 0.5).collect();
    let mut b = GlbBuilder::new();
    add_triangles(&mut b, &t, Some(&vals), serde_json::Value::Null);
    let bytes = b.finish("test", serde_json::json!({"k": 1})).unwrap();
    let (doc, prims) = read_primitives(&bytes).unwrap();
    assert_eq!(doc["asset"]["extras"]["k"], 1);
    let p = &prims[0];
    assert_eq!(p.mode, 4);
    let Attribute::F32(pos, 3) = &p.attributes["POSITION"] else {
        panic!("POSITION")
    };
    assert_eq!(pos.len(), 9 * t.len());
    assert!(pos.chunks_exact(3).zip(&t.corners).all(|(a, b)| a == b));
    let Attribute::U32(cells) = &p.attributes["_CELL"] else {
        panic!("_CELL")
    };
    assert!(cells.chunks(3).zip(&t.cells).all(|(a, &c)| a == [c, c, c]));
    let Attribute::F32(v, 1) = &p.attributes["_VALUE"] else {
        panic!("_VALUE")
    };
    assert!(v.chunks(3).zip(&vals).all(|(a, &x)| a == [x, x, x]));
    // an empty picture is a valid file too
    let empty = GlbBuilder::new().finish("empty", serde_json::json!({})).unwrap();
    assert!(read_primitives(&empty).unwrap().1.is_empty());
    eprintln!("G1: {} bytes, {} triangles read back", bytes.len(), t.len());
}
