//! Rigid transforms of a mesh's points (ADR 0155): a rotation about an axis parallel to z.
//!
//! The rotation turns x and y of every point about the axis through `origin`; z is copied unchanged, so the planes of a
//! 2D mesh (its `Empty` patches, normal to z) stay exactly where they were. Positive angles turn counter-clockwise seen
//! from +z (x to the right, y up). The topology is copied (point, face and cell order, owner, neighbour, patches with
//! their kinds and periodic data, zones) and `Mesh::from_topology` recomputes the derived geometry and validates it.
//! An angle that reduces to 0 modulo 360 degrees is no transform: the mesh comes back as it was, bit for bit.

use crate::mesh::Mesh;
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};

/// `deg` reduced to (-180, 180] degrees, exactly (the remainder and the one shift by 360 are exact in floating point);
/// `0.0` for every multiple of 360 (signed zero included).
pub fn reduce_deg(deg: f64) -> f64 {
    let mut a = deg % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a <= -180.0 {
        a += 360.0;
    }
    if a == 0.0 {
        0.0
    } else {
        a
    }
}

/// Cosine and sine of `deg` degrees, exact at the multiples of 90 degrees, else from `to_radians` and `sin_cos`.
pub fn cos_sin_deg(deg: f64) -> (f64, f64) {
    let a = reduce_deg(deg);
    if a == 0.0 {
        (1.0, 0.0)
    } else if a == 90.0 {
        (0.0, 1.0)
    } else if a == 180.0 {
        (-1.0, 0.0)
    } else if a == -90.0 {
        (0.0, -1.0)
    } else {
        let (s, c) = a.to_radians().sin_cos();
        (c, s)
    }
}

/// The rotation matrix about z by `deg` degrees (row-major): `[[c, -s, 0], [s, c, 0], [0, 0, 1]]`.
pub fn rotation_z_matrix(deg: f64) -> [[f64; 3]; 3] {
    let (c, s) = cos_sin_deg(deg);
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

/// Why rotating `mesh` about the z-parallel axis through `origin` would break a rotationally periodic pair, if it
/// would: a stored periodic rotation stays consistent only when its axis is parallel to z and passes through `origin`
/// (the two rotations then commute). Translational pairs never conflict (their shift is recomputed from the rotated
/// patches). Tolerance: `1e-12` of the larger of 1, `|origin|` and the pair's axis point.
pub fn periodic_conflict(mesh: &Mesh, origin: Vec3) -> Option<String> {
    for p in mesh.patches() {
        let Some(r) = &p.rotation else { continue };
        let a = vec3::unit(r.axis);
        let tol = 1e-12 * vec3::norm(origin).max(vec3::norm(r.origin)).max(1.0);
        let parallel = a[0].abs() <= 1e-12 && a[1].abs() <= 1e-12;
        let on_axis = (origin[0] - r.origin[0]).abs() <= tol && (origin[1] - r.origin[1]).abs() <= tol;
        if !(parallel && on_axis) {
            return Some(format!(
                "the mesh has the rotationally periodic pair '{}'/'{}' about the axis through ({}, {}, {}) along ({}, {}, {}); \
                 a rotation about the z-parallel axis through ({}, {}, {}) would move that axis and the pair would no longer \
                 match: put rotate_origin on that axis (x = {}, y = {}), or remove the rotation",
                p.name,
                p.periodic_with.as_deref().unwrap_or("?"),
                r.origin[0],
                r.origin[1],
                r.origin[2],
                a[0],
                a[1],
                a[2],
                origin[0],
                origin[1],
                origin[2],
                r.origin[0],
                r.origin[1]
            ));
        }
    }
    None
}

/// `mesh` rotated by `deg` degrees about the axis parallel to z through `origin` (counter-clockwise seen from +z). An
/// angle that reduces to 0 gives the mesh back unchanged; a rotation that would break a rotationally periodic pair is
/// refused ([`periodic_conflict`]).
pub fn rotate_z(mesh: &Mesh, deg: f64, origin: Vec3) -> Result<Mesh> {
    if reduce_deg(deg) == 0.0 {
        return Ok(mesh.clone());
    }
    if let Some(m) = periodic_conflict(mesh, origin) {
        return Err(Error::Mesh(m));
    }
    let (c, s) = cos_sin_deg(deg);
    let mut topo = mesh.topology().clone();
    for p in topo.points.iter_mut() {
        let (dx, dy) = (p[0] - origin[0], p[1] - origin[1]);
        *p = [origin[0] + (c * dx - s * dy), origin[1] + (s * dx + c * dy), p[2]];
    }
    Mesh::from_topology(topo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{self, Grading};

    const ANGLES: [f64; 7] = [-4.0, 2.0, 30.0, 90.0, -90.0, 180.0, 123.456];

    fn mat_mul_t(r: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x = (0..3).map(|k| r[k][i] * r[k][j]).sum();
            }
        }
        m
    }

    fn orthogonality_error(r: &[[f64; 3]; 3]) -> f64 {
        let m = mat_mul_t(r);
        let mut e: f64 = 0.0;
        for (i, row) in m.iter().enumerate() {
            for (j, x) in row.iter().enumerate() {
                e = e.max((x - if i == j { 1.0 } else { 0.0 }).abs());
            }
        }
        e
    }

    fn det(r: &[[f64; 3]; 3]) -> f64 {
        r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1]) - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
            + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0])
    }

    /// ADR 0155 U1: every registered angle gives an orthogonal matrix of determinant 1 with an exact z row and column;
    /// control: a column scaled by 1 + 1e-12 fails the orthogonality limit.
    #[test]
    fn u1_rotation_matrices_are_orthogonal_with_determinant_one() {
        let mut worst = (0.0f64, 0.0f64);
        for a in ANGLES {
            let r = rotation_z_matrix(a);
            let (o, d) = (orthogonality_error(&r), (det(&r) - 1.0).abs());
            worst = (worst.0.max(o), worst.1.max(d));
            assert!(o <= 1e-15 && d <= 1e-15, "{a}: {o:e} {d:e}");
            assert_eq!([r[2][0], r[2][1], r[2][2], r[0][2], r[1][2]], [0.0, 0.0, 1.0, 0.0, 0.0]);
            // the inverse is the transpose and the rotation by -a
            let back = rotation_z_matrix(-a);
            for i in 0..3 {
                for j in 0..3 {
                    assert!((back[i][j] - r[j][i]).abs() <= 1e-16, "{a}");
                }
            }
        }
        println!("U1: orthogonality {:.2e}, determinant {:.2e}", worst.0, worst.1);
        let mut bad = rotation_z_matrix(30.0);
        for row in bad.iter_mut() {
            row[0] *= 1.0 + 1e-12;
        }
        assert!(orthogonality_error(&bad) > 1e-15, "control");
    }

    #[test]
    fn reduced_angles_and_exact_quadrants() {
        assert_eq!(reduce_deg(360.0), 0.0);
        assert_eq!(reduce_deg(-360.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(reduce_deg(-0.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(reduce_deg(270.0), -90.0);
        assert_eq!(reduce_deg(-270.0), 90.0);
        assert_eq!(reduce_deg(-180.0), 180.0);
        assert_eq!(reduce_deg(190.0), -170.0);
        assert_eq!(reduce_deg(-4.0), -4.0);
        assert_eq!(cos_sin_deg(90.0), (0.0, 1.0));
        assert_eq!(cos_sin_deg(-270.0), (0.0, 1.0));
        assert_eq!(cos_sin_deg(180.0), (-1.0, 0.0));
        assert_eq!(cos_sin_deg(-90.0), (0.0, -1.0));
        let (c, s) = cos_sin_deg(-4.0);
        assert!((c - 4f64.to_radians().cos()).abs() <= 1e-16 && (s + 4f64.to_radians().sin()).abs() <= 1e-16);
    }

    /// U2 on generator meshes (the aerofoil mesh file is checked through the case in `rcfd-api`): a rotation and its
    /// inverse bring every point back within 1e-12 of its size; U4: the topology is copied.
    #[test]
    fn u2_round_trip_and_u4_topology_on_generator_meshes() {
        let meshes = [
            (
                "cavity_2d",
                gen::cavity_2d_rect(8, 8, 1.0, 1.0).unwrap(),
                [0.25, -0.5, 0.0],
            ),
            (
                "cylinder_ogrid",
                gen::cylinder_ogrid(32, 12, 0.5, 5.0, Grading::Expansion(10.0), 0.1).unwrap(),
                [0.25, -0.5, 0.0],
            ),
            (
                "box_2d periodic_x",
                gen::box_2d(6, 4, 2.0, 1.0, true, false).unwrap(),
                [0.25, -0.5, 0.0],
            ),
            (
                "annulus_3d_sector",
                gen::annulus_3d_sector(8, 4, 3, 0.5, 1.0, 0.3, 40f64.to_radians(), Grading::Uniform).unwrap(),
                [0.0, 0.0, 0.7],
            ),
        ];
        for (name, m, o) in &meshes {
            let mut worst: f64 = 0.0;
            for a in ANGLES {
                let r = rotate_z(m, a, *o).unwrap();
                assert_eq!(r.topology().face_points, m.topology().face_points, "{name}");
                assert_eq!(r.topology().owner, m.topology().owner, "{name}");
                assert_eq!(r.topology().neighbour, m.topology().neighbour, "{name}");
                assert_eq!(r.topology().patches, m.topology().patches, "{name}");
                assert_eq!(r.topology().cell_zones, m.topology().cell_zones, "{name}");
                let back = rotate_z(&r, -a, *o).unwrap();
                for (p, q) in back.points().iter().zip(m.points()) {
                    let d = vec3::norm(vec3::sub(*p, *q)) / vec3::norm(*q).max(1.0);
                    worst = worst.max(d);
                    assert!(d <= 1e-12, "{name} {a}: {d:e}");
                    assert_eq!(p[2].to_bits(), q[2].to_bits(), "{name} {a}: z");
                }
            }
            println!("U2 {name}: round trip within {worst:.2e}");
            let same = rotate_z(m, 360.0, *o).unwrap();
            assert_eq!(same.content_hash(), m.content_hash(), "{name}: 360");
        }
    }

    /// D5: an on-axis rotation of a rotationally periodic mesh is allowed, an off-axis one refused.
    #[test]
    fn periodic_rotation_refused_off_its_axis() {
        let m = gen::annulus_3d_sector(8, 4, 3, 0.5, 1.0, 0.3, 40f64.to_radians(), Grading::Uniform).unwrap();
        assert!(periodic_conflict(&m, [0.0, 0.0, 5.0]).is_none());
        let e = rotate_z(&m, 17.0, [1.0, 0.0, 0.0]).unwrap_err().to_string();
        assert!(
            e.contains("rotationally periodic pair") && e.contains("rotate_origin"),
            "{e}"
        );
        let b = gen::box_2d(6, 4, 2.0, 1.0, true, true).unwrap();
        assert!(periodic_conflict(&b, [3.0, -7.0, 0.0]).is_none());
    }
}
