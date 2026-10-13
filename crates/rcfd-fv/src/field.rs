//! Cell-centred fields with per-patch boundary conditions.

use crate::bc::{Bc, BcCtx, Empty, FaceCoeffs, Periodic, ZeroGradient};
use crate::geometry::FvGeometry;
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::PatchKind;

fn default_bcs(geom: &FvGeometry) -> Vec<Box<dyn Bc>> {
    geom.mesh
        .patches()
        .iter()
        .map(|p| -> Box<dyn Bc> {
            match p.kind {
                PatchKind::Empty => Box::new(Empty),
                PatchKind::Cyclic => Box::new(Periodic),
                _ => Box::new(ZeroGradient),
            }
        })
        .collect()
}

fn patch_index(geom: &FvGeometry, name: &str) -> Result<usize> {
    geom.mesh
        .patches()
        .iter()
        .position(|p| p.name == name)
        .ok_or_else(|| Error::PathNotFound(format!("patch {name}")))
}

/// Apply a row-major 3x3 matrix to a vector.
pub fn rotate(r: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| r[i][0] * v[0] + r[i][1] * v[1] + r[i][2] * v[2])
}

/// Evaluate boundary coefficients for all boundary faces (indexed `f - n_internal`).
fn eval_coeffs(
    geom: &FvGeometry,
    bcs: &[Box<dyn Bc>],
    cell: &dyn Fn(usize) -> Vec3,
    is_vector: bool,
    comp: usize,
) -> Vec<FaceCoeffs> {
    let mesh = &geom.mesh;
    let mut out = vec![
        FaceCoeffs {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 0.0
        };
        mesh.n_boundary_faces()
    ];
    for (pi, p) in mesh.patches().iter().enumerate() {
        for (local, f) in p.faces().enumerate() {
            let partner = if bcs[pi].is_coupled() {
                let g = geom.periodic_partner[pi]
                    .as_ref()
                    .expect("coupled patch without partner")[local];
                let mut q = cell(mesh.owner()[g]);
                if let (true, Some(r)) = (is_vector, &geom.periodic_rot[pi]) {
                    q = rotate(r, q);
                }
                Some((q, geom.dn[g - geom.n_internal]))
            } else {
                None
            };
            let ctx = BcCtx {
                normal: geom.normal(f),
                dn: geom.dn[f - geom.n_internal],
                local_face: local,
                cell_value: cell(mesh.owner()[f]),
                partner,
                is_vector,
            };
            out[f - geom.n_internal] = bcs[pi].coeffs(&ctx, comp);
        }
    }
    out
}

/// Cell-centred scalar field.
#[derive(Debug, Clone)]
pub struct ScalarField {
    /// Field name.
    pub name: String,
    /// Cell values.
    pub values: Vec<f64>,
    bcs: Vec<Box<dyn Bc>>,
}

impl ScalarField {
    /// Uniform field; patches default to zero gradient (empty/cyclic by kind).
    pub fn uniform(name: &str, geom: &FvGeometry, value: f64) -> Self {
        ScalarField {
            name: name.into(),
            values: vec![value; geom.n_cells()],
            bcs: default_bcs(geom),
        }
    }

    /// Field from a function of cell centres.
    pub fn from_fn(name: &str, geom: &FvGeometry, f: impl Fn(Vec3) -> f64) -> Self {
        let mut s = Self::uniform(name, geom, 0.0);
        s.values = geom.mesh.cell_centres().iter().map(|&c| f(c)).collect();
        s
    }

    /// Assign a boundary condition to a patch by name.
    pub fn set_bc(&mut self, geom: &FvGeometry, patch: &str, bc: Box<dyn Bc>) -> Result<()> {
        let i = patch_index(geom, patch)?;
        self.bcs[i] = bc;
        Ok(())
    }

    /// Condition on patch index `i`.
    pub fn bc(&self, i: usize) -> &dyn Bc {
        self.bcs[i].as_ref()
    }

    /// Implicit coefficients for every boundary face.
    pub fn boundary_coeffs(&self, geom: &FvGeometry) -> Vec<FaceCoeffs> {
        eval_coeffs(geom, &self.bcs, &|c| [self.values[c], 0.0, 0.0], false, 0)
    }

    /// Face values on boundary faces (index `f - n_internal`).
    pub fn boundary_values(&self, geom: &FvGeometry) -> Vec<f64> {
        let own = geom.mesh.owner();
        self.boundary_coeffs(geom)
            .iter()
            .enumerate()
            .map(|(i, c)| c.value(self.values[own[geom.n_internal + i]]))
            .collect()
    }
}

/// Cell-centred vector field.
#[derive(Debug, Clone)]
pub struct VectorField {
    /// Field name.
    pub name: String,
    /// Cell values.
    pub values: Vec<Vec3>,
    bcs: Vec<Box<dyn Bc>>,
}

impl VectorField {
    /// Uniform field.
    pub fn uniform(name: &str, geom: &FvGeometry, value: Vec3) -> Self {
        VectorField {
            name: name.into(),
            values: vec![value; geom.n_cells()],
            bcs: default_bcs(geom),
        }
    }

    /// Field from a function of cell centres.
    pub fn from_fn(name: &str, geom: &FvGeometry, f: impl Fn(Vec3) -> Vec3) -> Self {
        let mut s = Self::uniform(name, geom, [0.0; 3]);
        s.values = geom.mesh.cell_centres().iter().map(|&c| f(c)).collect();
        s
    }

    /// Assign a boundary condition to a patch by name.
    pub fn set_bc(&mut self, geom: &FvGeometry, patch: &str, bc: Box<dyn Bc>) -> Result<()> {
        let i = patch_index(geom, patch)?;
        self.bcs[i] = bc;
        Ok(())
    }

    /// Condition on patch index `i`.
    pub fn bc(&self, i: usize) -> &dyn Bc {
        self.bcs[i].as_ref()
    }

    /// Implicit coefficients of component `comp` for every boundary face.
    pub fn boundary_coeffs(&self, geom: &FvGeometry, comp: usize) -> Vec<FaceCoeffs> {
        eval_coeffs(geom, &self.bcs, &|c| self.values[c], true, comp)
    }

    /// Face vectors on boundary faces (index `f - n_internal`).
    pub fn boundary_values(&self, geom: &FvGeometry) -> Vec<Vec3> {
        let own = geom.mesh.owner();
        let per: Vec<Vec<FaceCoeffs>> = (0..3).map(|k| self.boundary_coeffs(geom, k)).collect();
        (0..geom.mesh.n_boundary_faces())
            .map(|i| {
                let u = self.values[own[geom.n_internal + i]];
                [per[0][i].value(u[0]), per[1][i].value(u[1]), per[2][i].value(u[2])]
            })
            .collect()
    }

    /// Copy of component `k` as a scalar field carrying the same conditions.
    pub fn component(&self, k: usize) -> ScalarField {
        ScalarField {
            name: format!("{}_{k}", self.name),
            values: self.values.iter().map(|v| v[k]).collect(),
            bcs: self.bcs.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bc::{FixedValue, Symmetry};
    use rcfd_mesh::gen::cavity_2d;
    use std::sync::Arc;

    #[test]
    fn scalar_boundary_values() {
        let g = FvGeometry::new(Arc::new(cavity_2d(4, 1.0).unwrap())).unwrap();
        let mut t = ScalarField::from_fn("T", &g, |c| 2.0 * c[0]);
        t.set_bc(&g, "lid", Box::new(FixedValue::scalar(9.0))).unwrap();
        let bv = t.boundary_values(&g);
        let lid = g.mesh.patch("lid").unwrap();
        for f in lid.faces() {
            assert_eq!(bv[f - g.n_internal], 9.0);
        }
        // default zero gradient on walls
        let w = g.mesh.patch("walls").unwrap();
        let f = w.start;
        assert_eq!(bv[f - g.n_internal], t.values[g.mesh.owner()[f]]);
        assert!(t.set_bc(&g, "nope", Box::new(ZeroGradient)).is_err());
        // default for the empty patch is Empty
        let pi = g.mesh.patches().iter().position(|p| p.name == "frontAndBack").unwrap();
        assert!(t.bc(pi).is_empty());
    }

    #[test]
    fn vector_symmetry_removes_normal_component() {
        let g = FvGeometry::new(Arc::new(cavity_2d(3, 1.0).unwrap())).unwrap();
        let mut u = VectorField::uniform("U", &g, [1.0, 2.0, 0.0]);
        u.set_bc(&g, "lid", Box::new(Symmetry)).unwrap();
        let bv = u.boundary_values(&g);
        for f in g.mesh.patch("lid").unwrap().faces() {
            let v = bv[f - g.n_internal];
            let n = g.normal(f);
            assert!((v[0] * n[0] + v[1] * n[1] + v[2] * n[2]).abs() < 1e-12);
            assert!((v[0] - 1.0).abs() < 1e-12); // tangential part kept (lid normal is +y)
        }
        assert_eq!(u.component(1).values.len(), 9);
    }

    #[test]
    fn periodic_default_couples_to_partner_cell() {
        use rcfd_mesh::gen::{box_mesh, Grading};
        let mut m = box_mesh([4, 1, 1], [4.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap();
        let mut minus = String::new();
        let mut plus = String::new();
        for p in m.patches() {
            let n = rcfd_core::vec3::unit(m.face_area_vectors()[p.start]);
            if n[0] < -0.9 {
                minus = p.name.clone();
            }
            if n[0] > 0.9 {
                plus = p.name.clone();
            }
        }
        m.set_periodic(&minus, &plus).unwrap();
        m.set_periodic(&plus, &minus).unwrap();
        let g = FvGeometry::new(Arc::new(m)).unwrap();
        let t = ScalarField::from_fn("T", &g, |c| c[0]);
        let bv = t.boundary_values(&g);
        // faces at x=0 see cells at x=0.5 and x=3.5 -> mean 2.0 (equal distances)
        let f = g.mesh.patch(&minus).unwrap().start;
        assert!((bv[f - g.n_internal] - 2.0).abs() < 1e-12, "{}", bv[f - g.n_internal]);
    }
}
