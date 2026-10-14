//! Probes, volume and surface integrals, force/moment coefficients and flux
//! reports (F-071).
//!
//! Conventions: all fields are kinematic (pressure divided by density); the
//! `rho` arguments convert to dynamic quantities. Face area vectors point out
//! of the fluid domain, so
//!
//! * the force of the fluid on a wall is `F = rho * sum(p_f S_f)` (pressure)
//!   `- rho nu sum((grad U + grad U^T) . n |S|)` (viscous: the full stress; the
//!   wall-normal derivative comes from the boundary condition, the tangential
//!   parts from the cell gradient);
//! * a flux is positive when it leaves the domain.

use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_fv::field::{ScalarField, VectorField};
use rcfd_fv::FvGeometry;

fn patch_faces(geom: &FvGeometry, patch: &str) -> Result<std::ops::Range<usize>> {
    geom.mesh
        .patch(patch)
        .map(|p| p.faces())
        .ok_or_else(|| Error::PathNotFound(format!("patch {patch}")))
}

/// Integral of a cell field over the whole domain (`sum v_c V_c`).
pub fn volume_integral(geom: &FvGeometry, values: &[f64]) -> f64 {
    geom.mesh.cell_volumes().iter().zip(values).map(|(v, x)| v * x).sum()
}

/// Volume-weighted average of a cell field.
pub fn volume_average(geom: &FvGeometry, values: &[f64]) -> f64 {
    volume_integral(geom, values) / geom.mesh.total_volume()
}

/// Area of a patch.
pub fn patch_area(geom: &FvGeometry, patch: &str) -> Result<f64> {
    let sf = geom.mesh.face_area_vectors();
    Ok(patch_faces(geom, patch)?.map(|f| vec3::norm(sf[f])).sum())
}

/// Integral of face values over a patch (`face_values` indexed `f - n_internal`).
pub fn patch_integral(geom: &FvGeometry, patch: &str, face_values: &[f64]) -> Result<f64> {
    let sf = geom.mesh.face_area_vectors();
    Ok(patch_faces(geom, patch)?
        .map(|f| face_values[f - geom.n_internal] * vec3::norm(sf[f]))
        .sum())
}

/// Area-weighted average of face values over a patch.
pub fn patch_average(geom: &FvGeometry, patch: &str, face_values: &[f64]) -> Result<f64> {
    Ok(patch_integral(geom, patch, face_values)? / patch_area(geom, patch)?)
}

/// Net outward volumetric flux through a patch, from the face flux `phi`.
pub fn volume_flux(geom: &FvGeometry, patch: &str, phi: &[f64]) -> Result<f64> {
    Ok(patch_faces(geom, patch)?.map(|f| phi[f]).sum())
}

/// Net outward mass flux through a patch (`rho * flux`).
pub fn mass_flux(geom: &FvGeometry, patch: &str, phi: &[f64], rho: f64) -> Result<f64> {
    Ok(rho * volume_flux(geom, patch, phi)?)
}

/// Net outward flux of a transported scalar `integral(phi * T)` through a patch
/// (face scalar values indexed `f - n_internal`).
pub fn scalar_flux(geom: &FvGeometry, patch: &str, phi: &[f64], face_values: &[f64]) -> Result<f64> {
    Ok(patch_faces(geom, patch)?
        .map(|f| phi[f] * face_values[f - geom.n_internal])
        .sum())
}

/// Heat flow leaving the domain through a patch by conduction,
/// `-k dT/dn |S|` summed over faces, with `dT/dn` the outward normal gradient
/// from the temperature boundary condition. `k` is the conductivity [W/(m K)].
pub fn conductive_heat_flow(geom: &FvGeometry, patch: &str, t: &ScalarField, k: f64) -> Result<f64> {
    let coeffs = t.boundary_coeffs(geom);
    let own = geom.mesh.owner();
    let sf = geom.mesh.face_area_vectors();
    Ok(patch_faces(geom, patch)?
        .map(|f| {
            let i = f - geom.n_internal;
            -k * coeffs[i].gradient(t.values[own[f]]) * vec3::norm(sf[f])
        })
        .sum())
}

/// Pressure and viscous parts of the force of the fluid on a patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatchForce {
    /// Pressure force.
    pub pressure: Vec3,
    /// Viscous (wall shear) force.
    pub viscous: Vec3,
}

impl PatchForce {
    /// Sum of both parts.
    pub fn total(&self) -> Vec3 {
        vec3::add(self.pressure, self.viscous)
    }
}

/// Force of the fluid on `patch` and its moment about `origin` (dynamic units).
///
/// `p_face` are the pressure face values (indexed `f - n_internal`, kinematic).
pub fn wall_force_and_moment(
    geom: &FvGeometry,
    patch: &str,
    u: &VectorField,
    p_face: &[f64],
    nu: f64,
    rho: f64,
    origin: Vec3,
) -> Result<(PatchForce, Vec3)> {
    let mesh = &geom.mesh;
    let (sf, cf, own) = (mesh.face_area_vectors(), mesh.face_centres(), mesh.owner());
    let coeffs: Vec<Vec<_>> = (0..3).map(|k| u.boundary_coeffs(geom, k)).collect();
    let mut pf = vec3::ZERO;
    let mut vf = vec3::ZERO;
    let mut moment = vec3::ZERO;
    // Cell gradients of the components (their tangential parts give the transpose term of the stress).
    let grads: Vec<Vec<Vec3>> = if nu == 0.0 {
        vec![]
    } else {
        (0..3)
            .map(|k| {
                let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
                let bvals: Vec<f64> = (0..mesh.n_boundary_faces())
                    .map(|i| coeffs[k][i].value(u.values[own[geom.n_internal + i]][k]))
                    .collect();
                rcfd_fv::gradient::least_squares(geom, &cells, &bvals)
            })
            .collect()
    };
    for f in patch_faces(geom, patch)? {
        let i = f - geom.n_internal;
        let fp = vec3::scale(sf[f], rho * p_face[i]);
        let area = vec3::norm(sf[f]);
        let c = own[f];
        let dn = [0, 1, 2].map(|k| coeffs[k][i].gradient(u.values[c][k]));
        // t = nu (grad u + grad u^T) . n with the normal derivative of the wall-face gradient fixed by the boundary
        // condition: (grad u . n)_i = dn_i and ((grad u)^T . n)_i = sum_j n_j d_i u_j.
        let mut tr = dn;
        if !grads.is_empty() {
            let n = vec3::unit(sf[f]);
            for (j, nj) in n.iter().enumerate() {
                let gj = grads[j][c];
                // d_i u_j on the face: cell gradient with its normal part replaced by the wall derivative
                let corr = dn[j] - vec3::dot(gj, n);
                for (i2, t) in tr.iter_mut().enumerate() {
                    *t += nj * (gj[i2] + corr * n[i2]);
                }
            }
        }
        let fv = tr.map(|t| -rho * nu * t * area);
        pf = vec3::add(pf, fp);
        vf = vec3::add(vf, fv);
        moment = vec3::add(moment, vec3::cross(vec3::sub(cf[f], origin), vec3::add(fp, fv)));
    }
    Ok((
        PatchForce {
            pressure: pf,
            viscous: vf,
        },
        moment,
    ))
}

/// Force coefficient `F . dir / (0.5 rho u_ref^2 a_ref)`.
pub fn force_coefficient(force: Vec3, dir: Vec3, rho: f64, u_ref: f64, a_ref: f64) -> f64 {
    vec3::dot(force, vec3::unit(dir)) / (0.5 * rho * u_ref * u_ref * a_ref)
}

/// Moment coefficient `M . axis / (0.5 rho u_ref^2 a_ref l_ref)`.
pub fn moment_coefficient(moment: Vec3, axis: Vec3, rho: f64, u_ref: f64, a_ref: f64, l_ref: f64) -> f64 {
    vec3::dot(moment, vec3::unit(axis)) / (0.5 * rho * u_ref * u_ref * a_ref * l_ref)
}

/// Fixed monitoring point resolved to the containing (nearest-centre) cell.
#[derive(Debug, Clone, Copy)]
pub struct Probe {
    /// Requested location.
    pub point: Vec3,
    /// Cell holding the point.
    pub cell: usize,
}

impl Probe {
    /// Locate the cell whose centre is nearest to `point`.
    pub fn new(geom: &FvGeometry, point: Vec3) -> Self {
        let cc = geom.mesh.cell_centres();
        let cell = (0..cc.len())
            .min_by(|&a, &b| {
                let d = |i: usize| {
                    let r = vec3::sub(cc[i], point);
                    vec3::dot(r, r)
                };
                d(a).total_cmp(&d(b))
            })
            .unwrap_or(0);
        Probe { point, cell }
    }

    /// Cell value, linearly reconstructed with the cell gradient when given.
    pub fn sample(&self, geom: &FvGeometry, values: &[f64], grad: Option<&[Vec3]>) -> f64 {
        let v = values[self.cell];
        match grad {
            Some(g) => v + vec3::dot(g[self.cell], vec3::sub(self.point, geom.mesh.cell_centres()[self.cell])),
            None => v,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_fv::gradient::least_squares;
    use rcfd_fv::FixedValue;
    use rcfd_mesh::gen::{box_mesh, channel, Grading};
    use std::sync::Arc;

    fn geom_box() -> FvGeometry {
        FvGeometry::new(Arc::new(
            box_mesh([8, 4, 2], [2.0, 1.0, 0.5], [Grading::Uniform; 3]).unwrap(),
        ))
        .unwrap()
    }

    fn all_patch_faces(g: &FvGeometry) -> Vec<(String, std::ops::Range<usize>)> {
        g.mesh.patches().iter().map(|p| (p.name.clone(), p.faces())).collect()
    }

    #[test]
    fn volume_integrals_of_polynomials() {
        let g = geom_box();
        let cc = g.mesh.cell_centres();
        let one = vec![1.0; g.n_cells()];
        assert!((volume_integral(&g, &one) - 1.0).abs() < 1e-12);
        let x: Vec<f64> = cc.iter().map(|c| c[0]).collect();
        // midpoint rule is exact for linear fields: int x dV = 1 * 0.5 * 2 = 1.0 ... (L^2/2 * 1 * 0.5)
        assert!((volume_integral(&g, &x) - 2.0 * 0.5 * 1.0).abs() < 1e-12);
        assert!((volume_average(&g, &x) - 1.0).abs() < 1e-12);
        let xy: Vec<f64> = cc.iter().map(|c| c[0] * c[1]).collect();
        // exact: (2^2/2) * (1/2) * 0.5 = 0.5 ; cell-wise products of independent axes are exact for the midpoint rule
        assert!((volume_integral(&g, &xy) - 0.5).abs() < 1e-12);
        let x2: Vec<f64> = cc.iter().map(|c| c[0] * c[0]).collect();
        let exact = 8.0 / 3.0 * 0.5;
        let err = (volume_integral(&g, &x2) - exact).abs() / exact;
        assert!(err < 5e-3 && err > 0.0, "{err}"); // second-order midpoint error
    }

    #[test]
    fn patch_areas_and_averages() {
        let g = geom_box();
        assert!((patch_area(&g, "xmin").unwrap() - 0.5).abs() < 1e-12);
        assert!((patch_area(&g, "zmax").unwrap() - 2.0).abs() < 1e-12);
        let n = g.mesh.n_boundary_faces();
        let cf = g.mesh.face_centres();
        let vals: Vec<f64> = (0..n).map(|i| cf[g.n_internal + i][1]).collect();
        // average of y over the xmax face (y in [0,1]) is 0.5
        assert!((patch_average(&g, "xmax", &vals).unwrap() - 0.5).abs() < 1e-12);
        assert!(patch_area(&g, "nope").is_err());
    }

    #[test]
    fn uniform_pressure_gives_no_net_force_on_a_closed_surface() {
        let g = geom_box();
        let n = g.mesh.n_boundary_faces();
        let p = vec![3.5; n];
        let u = VectorField::uniform("U", &g, [0.0; 3]);
        let mut total = vec3::ZERO;
        for (name, _) in all_patch_faces(&g) {
            let (f, _) = wall_force_and_moment(&g, &name, &u, &p, 0.0, 1.0, vec3::ZERO).unwrap();
            total = vec3::add(total, f.total());
        }
        assert!(vec3::norm(total) < 1e-12, "{total:?}");
    }

    #[test]
    fn hydrostatic_pressure_force_equals_the_fluid_weight() {
        // p = rho g (H - y): the fluid pushes outward on the container walls and the net force is
        // sum(p S) = integral(grad p) dV = -rho g V in y (the weight of the fluid, carried by the walls).
        let g = geom_box();
        let (grav, rho) = (9.81, 1000.0);
        let n = g.mesh.n_boundary_faces();
        let cf = g.mesh.face_centres();
        let p: Vec<f64> = (0..n).map(|i| grav * (1.0 - cf[g.n_internal + i][1])).collect();
        let u = VectorField::uniform("U", &g, [0.0; 3]);
        let mut total = vec3::ZERO;
        let mut mom = vec3::ZERO;
        for (name, _) in all_patch_faces(&g) {
            let (f, m) = wall_force_and_moment(&g, &name, &u, &p, 0.0, rho, [1.0, 0.5, 0.25]).unwrap();
            total = vec3::add(total, f.total());
            mom = vec3::add(mom, m);
        }
        let weight = rho * grav * 1.0; // V = 1
        assert!((total[1] + weight).abs() / weight < 1e-12, "{total:?}");
        assert!(total[0].abs() < 1e-9 && total[2].abs() < 1e-9);
        // the resultant acts through the centroid, so the moment about the centroid vanishes
        assert!(vec3::norm(mom) / weight < 1e-10, "{mom:?}");
    }

    #[test]
    fn couette_wall_shear_force_and_coefficient() {
        let mesh = channel([6, 10, 1], [3.0, 0.5, 1.0], [Grading::Uniform; 3], true).unwrap();
        let g = FvGeometry::new(Arc::new(mesh)).unwrap();
        let (h, uw, nu, rho) = (0.5, 2.0, 1e-3, 1.2);
        let mut u = VectorField::from_fn("U", &g, |c| [uw * c[1] / h, 0.0, 0.0]);
        u.set_bc(&g, "walls", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        // walls patch holds both walls; split by sign of the normal via the per-face check below
        let pf = vec![0.0; g.mesh.n_boundary_faces()];
        let (f, _) = wall_force_and_moment(&g, "walls", &u, &pf, nu, rho, vec3::ZERO).unwrap();
        // bottom wall at rest: force +mu U/H A in +x ; top wall also at rest here (u=0 BC), so shear on top
        // has the opposite sign for this linear field and the total over both walls is the
        // difference of two equal and opposite contributions of magnitude rho nu U/H A
        // (bottom: u_P > 0 -> force on wall in +x ; top: u_P is larger -> force bigger)
        let a = 3.0 * 1.0 / 1.0; // area of one wall: L * depth, depth = 1
        let bottom = rho * nu * (uw * (0.025 / h)) / 0.025 * a; // gradient (u_P - 0)/dn with dn = dy/2
        let top = rho * nu * (uw * (1.0 - 0.025 / h)) / 0.025 * a;
        assert!(
            (f.viscous[0] - (bottom + top)).abs() / (bottom + top) < 1e-10,
            "{:?}",
            f.viscous
        );
        assert!(f.viscous[1].abs() < 1e-12);
        let cd = force_coefficient(f.viscous, [1.0, 0.0, 0.0], rho, uw, a);
        assert!((cd - f.viscous[0] / (0.5 * rho * uw * uw * a)).abs() < 1e-14);
    }

    /// Field `f(x)` on the box with exact fixed face values on every patch.
    fn exact_field(g: &FvGeometry, f: impl Fn(Vec3) -> Vec3) -> VectorField {
        let mut u = VectorField::from_fn("U", g, &f);
        let cf = g.mesh.face_centres();
        for (name, faces) in all_patch_faces(g) {
            let vals: Vec<Vec3> = faces.map(|fc| f(cf[fc])).collect();
            u.set_bc(g, &name, Box::new(FixedValue(rcfd_fv::bc::Profile::PerFace(vals))))
                .unwrap();
        }
        u
    }

    #[test]
    fn rigid_rotation_has_no_viscous_force_or_moment() {
        // u = Omega x r has zero strain rate: nu (grad u + grad u^T) = 0, although nu du/dn does not vanish.
        let g = geom_box();
        let u = exact_field(&g, |x| [-2.0 * x[1], 2.0 * x[0], 0.0]);
        let pf = vec![0.0; g.mesh.n_boundary_faces()];
        let (mut fsum, mut msum) = (vec3::ZERO, vec3::ZERO);
        for (name, _) in all_patch_faces(&g) {
            let (f, m) = wall_force_and_moment(&g, &name, &u, &pf, 0.1, 1.0, [0.3, 0.2, 0.1]).unwrap();
            assert!(vec3::norm(f.viscous) < 1e-12, "{name}: {:?}", f.viscous);
            fsum = vec3::add(fsum, f.viscous);
            msum = vec3::add(msum, m);
        }
        assert!(
            vec3::norm(fsum) < 1e-12 && vec3::norm(msum) < 1e-12,
            "{fsum:?} {msum:?}"
        );
    }

    #[test]
    fn simple_shear_traction_includes_the_transpose_term() {
        // u = (gamma y, 0, 0): sigma_xy = sigma_yx = nu gamma. On xmax (outward normal +x) the fluid pulls the wall
        // with -nu gamma A in y; nu du/dn alone (normal derivative of u_x) would give zero.
        let g = geom_box();
        let (gamma, nu, rho) = (3.0, 0.02, 1.5);
        let u = exact_field(&g, |x| [gamma * x[1], 0.0, 0.0]);
        let pf = vec![0.0; g.mesh.n_boundary_faces()];
        let (f, _) = wall_force_and_moment(&g, "xmax", &u, &pf, nu, rho, vec3::ZERO).unwrap();
        let area = 1.0 * 0.5;
        assert!(
            (f.viscous[1] + rho * nu * gamma * area).abs() < 1e-12,
            "{:?}",
            f.viscous
        );
        assert!(f.viscous[0].abs() < 1e-12 && f.viscous[2].abs() < 1e-12);
        // on ymax (outward normal +y) the shear is the usual -nu gamma A in x
        let (f, _) = wall_force_and_moment(&g, "ymax", &u, &pf, nu, rho, vec3::ZERO).unwrap();
        let area = 2.0 * 0.5;
        assert!(
            (f.viscous[0] + rho * nu * gamma * area).abs() < 1e-12,
            "{:?}",
            f.viscous
        );
        assert!(f.viscous[1].abs() < 1e-12);
    }

    #[test]
    fn mass_flux_and_scalar_flux_through_a_channel() {
        let mesh = channel([6, 4, 1], [3.0, 1.0, 0.2], [Grading::Uniform; 3], true).unwrap();
        let g = FvGeometry::new(Arc::new(mesh)).unwrap();
        let mut u = VectorField::uniform("U", &g, [1.5, 0.0, 0.0]);
        u.set_bc(&g, "inlet", Box::new(FixedValue::vector([1.5, 0.0, 0.0])))
            .unwrap();
        let bu = u.boundary_values(&g);
        let phi = rcfd_fv::convection::face_flux(&g, &u.values, &bu);
        let q_in = volume_flux(&g, "inlet", &phi).unwrap();
        let q_out = volume_flux(&g, "outlet", &phi).unwrap();
        assert!((q_in + 1.5 * 1.0 * 0.2).abs() < 1e-12, "{q_in}");
        assert!((q_in + q_out).abs() < 1e-12);
        assert!((mass_flux(&g, "outlet", &phi, 2.0).unwrap() - 2.0 * 0.3).abs() < 1e-12);
        let tf = vec![10.0; g.mesh.n_boundary_faces()];
        assert!((scalar_flux(&g, "outlet", &phi, &tf).unwrap() - 3.0).abs() < 1e-12);
    }

    #[test]
    fn conductive_heat_flow_matches_fourier_law() {
        let mesh = channel([4, 8, 1], [1.0, 1.0, 0.1], [Grading::Uniform; 3], true).unwrap();
        let g = FvGeometry::new(Arc::new(mesh)).unwrap();
        // linear temperature T = 300 + 100 y, hot top wall fixed at 400, cold bottom at 300
        let mut t = ScalarField::from_fn("T", &g, |c| 300.0 + 100.0 * c[1]);
        t.set_bc(&g, "walls", Box::new(FixedValue::scalar(350.0))).unwrap();
        // fixed 350 on both walls with cell value 300+100y: gradient_n = (350 - T_P)/dn
        let q = conductive_heat_flow(&g, "walls", &t, 0.5).unwrap();
        let dn = 0.0625;
        let expect = -0.5 * ((350.0 - (300.0 + 100.0 * dn)) / dn + (350.0 - (300.0 + 100.0 * (1.0 - dn))) / dn) * 0.1;
        assert!((q - expect).abs() < 1e-9 * expect.abs().max(1.0), "{q} {expect}");
    }

    #[test]
    fn probe_samples_with_gradient_reconstruction() {
        let g = geom_box();
        let f = |c: Vec3| 1.0 + 2.0 * c[0] - 3.0 * c[1];
        let vals: Vec<f64> = g.mesh.cell_centres().iter().map(|&c| f(c)).collect();
        let bv: Vec<f64> = (0..g.mesh.n_boundary_faces())
            .map(|i| f(g.mesh.face_centres()[g.n_internal + i]))
            .collect();
        let grad = least_squares(&g, &vals, &bv);
        let probe = Probe::new(&g, [0.93, 0.41, 0.2]);
        let nearest = probe.sample(&g, &vals, None);
        let recon = probe.sample(&g, &vals, Some(&grad));
        assert!((recon - f([0.93, 0.41, 0.2])).abs() < 1e-9);
        assert!((nearest - f([0.93, 0.41, 0.2])).abs() > 1e-3);
    }
}
