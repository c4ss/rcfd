//! The face coefficient of the pressure term that the transient Rhie-Chow carry-over is consistent with (ADR 0245 C,
//! opt-in `PimpleSettings::rhie_chow_step_consistent`).
//!
//! With `rAU = V / a_P` and the linear face interpolation `interp`, the default flux of a corrector is
//! `interp(HbyA) . S + (gamma / dt) K - gamma snGrad(p) |S|`, `gamma = interp(rAU)`, `K` the carried old-level defects.
//! Its converged face defect `d = phi - interp(u) . S` depends on the step size where `a_P / V` differs between the two
//! cells of a face (ADR 0073 R2, ADR 0245). Here the face coefficient is the harmonic face value
//! `Gamma_f = 1 / interp(1 / rAU)` and `[Gamma_f interp(x) - interp(rAU x)] . S` is added to the flux, with
//! `x = grad(p_lag) - b` of the pressure the corrector starts from (`b` a body acceleration). Converged, the defect then
//! obeys `(c0 d - c1 d^n + c2 d^{n-1}) / dt + a_f d = interp(x) . S - snGrad(p) |S|` with `a_f = interp(a_P / V) - c0 / dt`:
//! the step size appears only through the backward difference.

use crate::geometry::FvGeometry;
use crate::gradient::green_gauss;
use crate::pressure::{boundary_values_pbc, PressureBc};
use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::overset::{CellKind, Connectivity};

/// The harmonic face value of a positive cell field with the linear face weights; the cell value on a boundary face.
pub(crate) fn harmonic_face(geom: &FvGeometry, cells: &[f64]) -> Vec<f64> {
    let mesh = &geom.mesh;
    (0..mesh.n_faces())
        .map(|f| {
            if f < geom.n_internal {
                let w = geom.weights[f];
                1.0 / (w / cells[mesh.owner()[f]] + (1.0 - w) / cells[mesh.neighbour()[f]])
            } else {
                cells[mesh.owner()[f]]
            }
        })
        .collect()
}

/// Add `[gamma_f interp(x) - interp(q)] . S` to the flux of every internal face (`q` is `rAU x` of the cells that have a
/// momentum row).
pub(crate) fn add_commutator(geom: &FvGeometry, gamma: &[f64], q: &[Vec3], x: &[Vec3], phi_hby_a: &mut [f64]) {
    let mesh = &geom.mesh;
    let sf = mesh.face_area_vectors();
    for f in 0..geom.n_internal {
        let w = geom.weights[f];
        let (o, n) = (mesh.owner()[f], mesh.neighbour()[f]);
        let v = [0, 1, 2].map(|k| gamma[f] * (w * x[o][k] + (1.0 - w) * x[n][k]) - (w * q[o][k] + (1.0 - w) * q[n][k]));
        phi_hby_a[f] += vec3::dot(v, sf[f]);
    }
}

/// [`add_commutator`] for a step in which every cell has a momentum row: `x = grad(p_lag) - b` with the Green-Gauss
/// gradient of the corrector and `q = rAU x`.
pub(crate) fn add_lagged_gradient(
    geom: &FvGeometry,
    pbc: &PressureBc,
    gamma: &[f64],
    r_au: &[f64],
    p_lag: &[f64],
    body: Option<&[Vec3]>,
    phi_hby_a: &mut [f64],
) {
    let mut x = green_gauss(geom, p_lag, &boundary_values_pbc(geom, pbc, p_lag), 0);
    if let Some(b) = body {
        for (xc, bc) in x.iter_mut().zip(b) {
            *xc = vec3::sub(*xc, *bc);
        }
    }
    let q: Vec<Vec3> = x.iter().zip(r_au).map(|(xc, r)| vec3::scale(*xc, *r)).collect();
    add_commutator(geom, gamma, &q, &x, phi_hby_a);
}

/// The cell quantities of [`add_commutator`] in the overset step (ADR 0245 amendment 1, "The overset step"): `x = grad(p_lag)`
/// and `q = rAU x` of the calculated cells; at a fringe cell the donor interpolants of the calculated values (with the
/// stencil weights that give its `HbyA`), zero at a hole cell. With a trivial connectivity the cell values everywhere.
pub(crate) fn overset_lagged_gradient(
    geom: &FvGeometry,
    pbc: &PressureBc,
    p_lag: &[f64],
    r_au: &[f64],
    conn: &Connectivity,
    trivial: bool,
) -> (Vec<Vec3>, Vec<Vec3>) {
    let n = geom.n_cells();
    let mut x = green_gauss(geom, p_lag, &boundary_values_pbc(geom, pbc, p_lag), 0);
    let mut q: Vec<Vec3> = x.iter().zip(r_au).map(|(v, r)| vec3::scale(*v, *r)).collect();
    if !trivial {
        let (cx, cq) = (x.clone(), q.clone());
        for (c, stn) in &conn.stencils {
            let (mut vx, mut vq) = ([0.0; 3], [0.0; 3]);
            for (&k, &w) in stn.cells.iter().zip(&stn.weights) {
                for d in 0..3 {
                    vx[d] += w * cx[k][d];
                    vq[d] += w * cq[k][d];
                }
            }
            x[*c] = vx;
            q[*c] = vq;
        }
        for c in 0..n {
            if conn.kind[c] == CellKind::Hole {
                x[c] = [0.0; 3];
                q[c] = [0.0; 3];
            }
        }
    }
    (x, q)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{ScalarField, VectorField};
    use crate::pressure::{coupled_gamma, face_interpolate, pressure_bc};
    use rcfd_mesh::gen::{annulus_sector, box_2d, Grading};
    use rcfd_mesh::overset::{DonorInterpolation, Stencil};
    use std::sync::Arc;

    fn geometry(mesh: rcfd_mesh::Mesh) -> Arc<FvGeometry> {
        Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
    }

    fn pressure_of(x: Vec3) -> f64 {
        x[0] * x[0] + 3.0 * x[0] * x[1] - x[1] * x[1] * x[1]
    }

    fn max_abs(v: &[Vec3]) -> f64 {
        v.iter().flatten().fold(0.0f64, |a, b| a.max(b.abs()))
    }

    fn max_diff(a: &[Vec3], b: &[Vec3]) -> f64 {
        a.iter()
            .flatten()
            .zip(b.iter().flatten())
            .fold(0.0f64, |m, (x, y)| m.max((x - y).abs()))
    }

    /// ADR 0245 amendment 3 G1, G1n, G1t: the fringe branch of the overset opt-in against its formula (the guard of MC5).
    #[test]
    fn g1_overset_lagged_gradient_is_the_donor_interpolant_at_fringe_cells() {
        let g = geometry(box_2d(8, 8, 1.0, 1.0, false, false).unwrap());
        let n = g.n_cells();
        let u = VectorField::uniform("U", &g, [0.0; 3]);
        let p = ScalarField::from_fn("p", &g, pressure_of);
        let pbc = pressure_bc(&g, &u, &p);
        let r_au: Vec<f64> = (0..n).map(|c| 1.0 + 0.01 * c as f64).collect();
        let fringe = [
            (27usize, vec![18usize, 19, 26], vec![0.5, 0.3, 0.2]),
            (28, vec![20, 21, 29], vec![0.6, 0.25, 0.15]),
        ];
        let hole = 36usize;
        let mut kind = vec![CellKind::Calculated; n];
        for (c, _, _) in &fringe {
            kind[*c] = CellKind::Fringe;
        }
        kind[hole] = CellKind::Hole;
        let conn = Connectivity {
            kind,
            stencils: fringe
                .iter()
                .map(|(c, cells, weights)| {
                    (
                        *c,
                        Stencil {
                            cells: cells.clone(),
                            weights: weights.clone(),
                        },
                    )
                })
                .collect(),
            n_background_cells: n,
            interpolation: DonorInterpolation::Linear,
            fallbacks: Vec::new(),
        };
        // the formula, evaluated here
        let gp = green_gauss(&g, &p.values, &boundary_values_pbc(&g, &pbc, &p.values), 0);
        let rq: Vec<Vec3> = gp.iter().zip(&r_au).map(|(v, r)| vec3::scale(*v, *r)).collect();
        let (mut ex, mut eq) = (gp.clone(), rq.clone());
        for (c, cells, weights) in &fringe {
            let mut vx = [0.0; 3];
            let mut vq = [0.0; 3];
            for (&k, &w) in cells.iter().zip(weights) {
                for d in 0..3 {
                    vx[d] += w * gp[k][d];
                    vq[d] += w * r_au[k] * gp[k][d];
                }
            }
            ex[*c] = vx;
            eq[*c] = vq;
        }
        ex[hole] = [0.0; 3];
        eq[hole] = [0.0; 3];
        let (x, q) = overset_lagged_gradient(&g, &pbc, &p.values, &r_au, &conn, false);
        let (sx, sq) = (max_abs(&ex), max_abs(&eq));
        let (dx, dq) = (max_diff(&x, &ex), max_diff(&q, &eq));
        // G1n: at the fringe cells the donor interpolant is not the cell's own value
        let own_x = fringe
            .iter()
            .map(|(c, _, _)| max_diff(&[gp[*c]], &[ex[*c]]))
            .fold(0.0, f64::max);
        let own_q = fringe
            .iter()
            .map(|(c, _, _)| max_diff(&[rq[*c]], &[eq[*c]]))
            .fold(0.0, f64::max);
        println!(
            "G1: x {dx:.3e} of {sx:.3e}, q {dq:.3e} of {sq:.3e}; G1n: own against donor x {own_x:.3e}, q {own_q:.3e}"
        );
        assert!(own_x >= 1e-3 * sx && own_q >= 1e-3 * sq, "G1n: {own_x:e}, {own_q:e}");
        assert!(dx <= 1e-14 * sx && dq <= 1e-14 * sq, "G1: {dx:e}, {dq:e}");
        // G1t: a trivial connectivity gives the cell values, bit for bit
        let (xt, qt) = overset_lagged_gradient(&g, &pbc, &p.values, &r_au, &conn, true);
        let bits = |v: &[Vec3]| -> Vec<u64> { v.iter().flatten().map(|a| a.to_bits()).collect() };
        assert!(bits(&xt) == bits(&gp) && bits(&qt) == bits(&rq), "G1t");
    }

    /// ADR 0245 amendment 3 P0, P0n: on coupled (periodic and rotationally periodic) faces the opt-in's coefficient, coupled
    /// coefficient and flux are those of the default, bit for bit.
    #[test]
    fn p0_the_opt_in_is_bit_inert_on_coupled_faces() {
        let meshes = [
            ("periodic box", box_2d(8, 8, 1.0, 1.0, true, true).unwrap()),
            (
                "rotational sector",
                annulus_sector(8, 8, 0.5, 1.0, std::f64::consts::FRAC_PI_3, Grading::Uniform, 0.1).unwrap(),
            ),
        ];
        for (label, mesh) in meshes {
            let g = geometry(mesh);
            let (n, n_int) = (g.n_cells(), g.n_internal);
            let n_faces = g.mesh.n_faces();
            let u = VectorField::uniform("U", &g, [0.0; 3]);
            let p = ScalarField::from_fn("p", &g, pressure_of);
            let pbc = pressure_bc(&g, &u, &p);
            let n_coupled = pbc.coupled.iter().filter(|c| c.is_some()).count();
            assert!(n_coupled > 0, "{label}: no coupled face");
            let r_au: Vec<f64> = (0..n).map(|c| 0.02 * (1.0 + 0.3 * ((c * 7) % 11) as f64)).collect();
            let (h, l) = (harmonic_face(&g, &r_au), face_interpolate(&g, &r_au));
            let same_boundary = (n_int..n_faces).all(|f| h[f].to_bits() == l[f].to_bits());
            let (ch, cl) = (coupled_gamma(&g, &pbc, &h), coupled_gamma(&g, &pbc, &l));
            let same_coupled = (n_int..n_faces).all(|f| ch[f].to_bits() == cl[f].to_bits());
            let phi0: Vec<f64> = (0..n_faces).map(|f| 0.1 * ((f * 13) % 17) as f64 - 0.8).collect();
            let mut phi = phi0.clone();
            add_lagged_gradient(&g, &pbc, &h, &r_au, &p.values, None, &mut phi);
            let same_flux = (n_int..n_faces).all(|f| phi[f].to_bits() == phi0[f].to_bits());
            let changed_flux = (0..n_int).filter(|&f| phi[f] != phi0[f]).count();
            let inner = (0..n_int).map(|f| (h[f] - l[f]).abs() / l[f]).fold(0.0, f64::max);
            println!(
                "P0 {label}: {n_coupled} coupled faces; boundary coefficient bit-equal {same_boundary}, coupled coefficient \
                 bit-equal {same_coupled}, boundary flux bit-equal {same_flux}; internal faces with a changed flux \
                 {changed_flux} of {n_int}; P0n largest relative internal difference {inner:.3e}"
            );
            assert!(same_boundary && same_coupled && same_flux, "P0 {label}");
            assert!(inner >= 1e-6, "P0n {label}: {inner:e}");
        }
    }
}
