//! F-289 (ADR 0202), library part: `cht::conductive_face_flows_opt` gives the face flows of the operator the energy equation
//! was solved with (`diffusion::laplacian_opt` with the same gradient options), and without options it is the unchanged
//! `conductive_face_flows` bit for bit.
//!
//! Set-up K2: ADR 0016's skewed mesh K at n = 8, two materials with a contact resistance, (rho cp)_ref = 2, fixed values on
//! every boundary from a smooth function and cell values with local extrema. E-lib: per cell, the net flow / (rho cp)_ref
//! equals `A T - b` of the operator to 1e-12 of the largest cell sum of |flow| / (rho cp)_ref; E-lib-T: the unchanged
//! formula misses that by more than 1e-9 for every option set; D1: `None` and the default options give the bits of
//! `conductive_face_flows`, and without a gradient the options do nothing.

use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::cht::{conductive_face_flows, conductive_face_flows_opt, contact_faces, ChtMaterials};
use rcfd_fv::diffusion::{laplacian_opt, BoundaryGradient, GradientOptions};
use rcfd_fv::gradient::{least_squares, Limiter};
use rcfd_fv::{FvGeometry, ScalarField};
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use std::sync::Arc;

/// E-lib limit, relative to the largest cell sum of |flow| / (rho cp)_ref.
const IDENTITY: f64 = 1e-12;
/// E-lib-T: the unchanged formula must miss the operator by more than this (same scale).
const TEETH: f64 = 1e-9;
/// Reference capacity of K2.
const RHO_CP: f64 = 2.0;

/// The option sets of ADR 0202 (those of ADR 0190): name and library options.
fn sets() -> Vec<(&'static str, GradientOptions)> {
    let d = GradientOptions::default();
    vec![
        (
            "O1",
            GradientOptions {
                face_correction: true,
                ..d
            },
        ),
        (
            "O2",
            GradientOptions {
                boundary: BoundaryGradient::Corrected,
                ..d
            },
        ),
        (
            "O3",
            GradientOptions {
                boundary: BoundaryGradient::Dropped,
                ..d
            },
        ),
        (
            "O4",
            GradientOptions {
                secondary_limiter: Some(Limiter::BarthJespersen),
                ..d
            },
        ),
        (
            "O5",
            GradientOptions {
                secondary_limiter: Some(Limiter::Venkatakrishnan(5.0)),
                ..d
            },
        ),
        (
            "O6",
            GradientOptions {
                secondary_limiter: Some(Limiter::Venkatakrishnan(0.5)),
                ..d
            },
        ),
        ("C2", c2()),
    ]
}

/// ADR 0016's option set C2: face correction, corrected boundary gradient, Venkatakrishnan with K = 5.
fn c2() -> GradientOptions {
    GradientOptions {
        face_correction: true,
        secondary_limiter: Some(Limiter::Venkatakrishnan(5.0)),
        boundary: BoundaryGradient::Corrected,
    }
}

/// Set-up K2: the geometry, the field, the face conductivity and `gamma = k / (rho cp)_ref`.
struct K2 {
    geom: Arc<FvGeometry>,
    t: ScalarField,
    k: Vec<f64>,
    gamma: Vec<f64>,
}

fn smooth(p: [f64; 3]) -> f64 {
    1.0 + (1.3 * p[0] + 0.4).sin() * (1.1 * p[1]).cos() * (0.9 * p[2] + 0.2).cos()
}

fn k2() -> K2 {
    let m = warp(&box_mesh([8; 3], [1.0; 3], [Grading::Uniform; 3]).unwrap(), |p| {
        [p[0] + 0.7 * p[1] + 0.3 * p[2], p[1] + 0.3 * p[2], p[2]]
    })
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(m)).unwrap());
    let cc = geom.mesh.cell_centres().to_vec();
    let label: Vec<usize> = cc.iter().map(|c| usize::from(c[0] >= 0.8)).collect();
    let n_b = label.iter().filter(|&&l| l == 1).count();
    assert!(
        n_b > 0 && n_b < cc.len(),
        "K2 set-up: both materials have cells ({n_b} of {})",
        cc.len()
    );
    let contacts = contact_faces(&geom, &label, 0, 1, 0.05);
    assert!(!contacts.is_empty(), "K2 set-up: the materials share faces");
    let mats = ChtMaterials {
        rho_cp_ref: RHO_CP,
        k_cell: label.iter().map(|&l| if l == 0 { 1.0 } else { 10.0 }).collect(),
        rho_cp_cell: vec![RHO_CP; cc.len()],
        contacts,
    };
    let mut t = ScalarField::uniform("T", &geom, 1.0);
    for p in geom.mesh.patches() {
        let vals = p
            .faces()
            .map(|f| [smooth(geom.mesh.face_centres()[f]), 0.0, 0.0])
            .collect();
        t.set_bc(&geom, &p.name, Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
    }
    for (c, v) in t.values.iter_mut().enumerate() {
        *v = smooth(cc[c]) + if c % 2 == 0 { 0.05 } else { -0.05 };
    }
    K2 {
        k: mats.face_conductivity(&geom),
        gamma: mats.gamma(&geom),
        geom,
        t,
    }
}

/// Net flow out of every cell divided by (rho cp)_ref, and the largest cell sum of |flow| / (rho cp)_ref.
fn net(geom: &FvGeometry, flows: &[f64]) -> (Vec<f64>, f64) {
    let (own, nei) = (geom.mesh.owner(), geom.mesh.neighbour());
    let n = geom.n_cells();
    let (mut out, mut abs) = (vec![0.0; n], vec![0.0; n]);
    for (f, q) in flows.iter().enumerate() {
        if f >= geom.n_internal && geom.empty[f] {
            continue;
        }
        out[own[f]] += q / RHO_CP;
        abs[own[f]] += q.abs() / RHO_CP;
        if f < geom.n_internal {
            out[nei[f]] -= q / RHO_CP;
            abs[nei[f]] += q.abs() / RHO_CP;
        }
    }
    (out, abs.iter().fold(0.0, |a, b| a.max(*b)))
}

/// `A T - b` of the solved diffusion operator with the options `opts`, at the field's values.
fn operator(s: &K2, opts: Option<GradientOptions>) -> Vec<f64> {
    let bc = s.t.boundary_coeffs(&s.geom);
    let bvals = s.t.boundary_values(&s.geom);
    let grad = least_squares(&s.geom, &s.t.values, &bvals);
    let m = laplacian_opt(&s.geom, &s.gamma, &bc, Some(&grad), &s.t.values, &bvals, opts);
    let mut at = vec![0.0; s.t.values.len()];
    m.a.matvec(&s.t.values, &mut at);
    at.iter().zip(&m.b).map(|(a, b)| a - b).collect()
}

/// Largest per-cell difference between the flows' net outflow and the operator, relative to the flows' scale.
fn identity_error(s: &K2, flows: &[f64], op: &[f64]) -> (f64, f64) {
    let (n, scale) = net(&s.geom, flows);
    let e = n.iter().zip(op).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    (e / scale, scale)
}

/// E-lib and E-lib-T: for the default and every option set the new flows are the operator's; the unchanged formula is not.
#[test]
fn e_lib_flows_are_the_solved_operator_for_every_option_set() {
    let s = k2();
    let q = rcfd_mesh::quality::check_mesh(&s.geom.mesh);
    println!(
        "K2: {} cells, max non-orthogonality {:.2} degrees",
        s.geom.n_cells(),
        q.max_non_orthogonality
    );
    let old = conductive_face_flows(&s.geom, &s.t, &s.k, true);
    let (e0, scale) = identity_error(&s, &old, &operator(&s, None));
    println!("E-lib default: {e0:.3e} of S = {scale:.4e} (limit {IDENTITY:e})");
    assert!(e0 <= IDENTITY, "E-lib default {e0}");
    for (name, o) in sets() {
        let op = operator(&s, Some(o));
        let new = conductive_face_flows_opt(&s.geom, &s.t, &s.k, true, Some(o));
        let (e, _) = identity_error(&s, &new, &op);
        let (teeth, _) = identity_error(&s, &old, &op);
        println!(
            "E-lib {name}: {e:.3e} (limit {IDENTITY:e}); E-lib-T unchanged formula {teeth:.3e} (limit > {TEETH:e})"
        );
        assert!(e <= IDENTITY, "E-lib {name}: {e}");
        assert!(teeth > TEETH, "E-lib-T {name}: {teeth}");
    }
}

/// D1: `None` and `Some(GradientOptions::default())` give the bits of `conductive_face_flows`, with and without a
/// gradient; without a gradient C2 does nothing.
#[test]
fn d1_no_options_and_default_options_are_the_unchanged_formula_bit_for_bit() {
    let s = k2();
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<u64>>();
    for non_orth in [true, false] {
        let old = bits(&conductive_face_flows(&s.geom, &s.t, &s.k, non_orth));
        for (what, o) in [("None", None), ("default", Some(GradientOptions::default()))] {
            let new = bits(&conductive_face_flows_opt(&s.geom, &s.t, &s.k, non_orth, o));
            let differ = old.iter().zip(&new).filter(|(a, b)| a != b).count();
            println!(
                "D1 {what}, non_orthogonal = {non_orth}: {differ} of {} faces differ in a bit (limit 0)",
                old.len()
            );
            assert_eq!(differ, 0, "D1 {what} {non_orth}");
        }
    }
    let old = bits(&conductive_face_flows(&s.geom, &s.t, &s.k, false));
    let new = bits(&conductive_face_flows_opt(&s.geom, &s.t, &s.k, false, Some(c2())));
    let differ = old.iter().zip(&new).filter(|(a, b)| a != b).count();
    println!("D1 C2 without a gradient: {differ} faces differ in a bit (limit 0)");
    assert_eq!(differ, 0, "D1 C2 without a gradient");
    // control: with a gradient C2 changes the flows, so the comparisons above can fail
    let c2_flows = conductive_face_flows_opt(&s.geom, &s.t, &s.k, true, Some(c2()));
    let d = conductive_face_flows(&s.geom, &s.t, &s.k, true)
        .iter()
        .zip(&c2_flows)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!("D1 control: C2 with a gradient changes a face flow by {d:.3e}");
    assert!(d > 0.0, "D1 control");
}
