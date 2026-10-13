//! Tiny 3-vector helpers on `[f64; 3]` (no external linear algebra crate).

/// 3-vector alias.
pub type Vec3 = [f64; 3];

/// Zero vector.
pub const ZERO: Vec3 = [0.0; 3];

/// a + b
#[inline]
pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
/// a - b
#[inline]
pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
/// s * a
#[inline]
pub fn scale(a: Vec3, s: f64) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
/// a . b
#[inline]
pub fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
/// a x b
#[inline]
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
/// Euclidean norm.
#[inline]
pub fn norm(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}
/// Unit vector (returns zero vector for a zero input).
#[inline]
pub fn unit(a: Vec3) -> Vec3 {
    let n = norm(a);
    if n > 0.0 {
        scale(a, 1.0 / n)
    } else {
        ZERO
    }
}
/// a += s * b
#[inline]
pub fn axpy(a: &mut Vec3, s: f64, b: Vec3) {
    a[0] += s * b[0];
    a[1] += s * b[1];
    a[2] += s * b[2];
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basics() {
        assert_eq!(cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
        assert_eq!(dot([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), 32.0);
        assert!((norm([3.0, 4.0, 0.0]) - 5.0).abs() < 1e-15);
        assert_eq!(unit(ZERO), ZERO);
        let mut a = [1.0, 1.0, 1.0];
        axpy(&mut a, 2.0, [1.0, 0.0, -1.0]);
        assert_eq!(a, [3.0, 1.0, -1.0]);
    }
}
