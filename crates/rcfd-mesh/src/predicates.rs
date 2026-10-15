//! Exact geometric predicates (ADR 0030 part C).
//!
//! `orient3d` and `insphere` return the exact sign of the determinant for any finite `f64` input (barring over- and
//! underflow). A floating-point evaluation with a conservative error bound decides almost all cases; the rest are
//! evaluated with non-overlapping floating-point expansions (error-free `two_sum` / `two_product`, as in the published
//! adaptive-precision arithmetic of Shewchuk, 1997; written here from the description, from memory, unverified).

use rcfd_core::vec3::Vec3;

fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    let bv = x - a;
    let av = x - bv;
    (x, (a - av) + (b - bv))
}

fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let x = a * b;
    (x, a.mul_add(b, -x))
}

/// A real number held exactly as the unevaluated sum of non-overlapping doubles (increasing magnitude).
#[derive(Clone, Debug)]
struct Exp(Vec<f64>);

impl Exp {
    fn grow(&mut self, b: f64) {
        let mut q = b;
        let mut out = Vec::with_capacity(self.0.len() + 1);
        for &e in &self.0 {
            let (s, h) = two_sum(q, e);
            q = s;
            if h != 0.0 {
                out.push(h);
            }
        }
        if q != 0.0 {
            out.push(q);
        }
        self.0 = out;
    }

    fn add(&self, o: &Exp) -> Exp {
        let mut r = self.clone();
        for &x in &o.0 {
            r.grow(x);
        }
        r
    }

    fn neg(&self) -> Exp {
        Exp(self.0.iter().map(|x| -x).collect())
    }

    fn sub(&self, o: &Exp) -> Exp {
        self.add(&o.neg())
    }

    fn mul(&self, o: &Exp) -> Exp {
        let mut r = Exp(vec![]);
        for &f in &o.0 {
            for &e in &self.0 {
                let (hi, lo) = two_prod(e, f);
                r.grow(lo);
                r.grow(hi);
            }
        }
        r
    }

    fn sign(&self) -> i32 {
        match self.0.last() {
            None => 0,
            Some(&x) if x > 0.0 => 1,
            _ => -1,
        }
    }
}

fn diff(a: f64, b: f64) -> Exp {
    let (x, y) = two_sum(a, -b);
    let mut e = Exp(vec![]);
    e.grow(y);
    e.grow(x);
    e
}

fn det3(m: &[[Exp; 3]; 3]) -> Exp {
    let t = |a: usize, b: usize, c: usize, d: usize| m[a][b].mul(&m[c][d]);
    let c0 = m[0][0].mul(&t(1, 1, 2, 2).sub(&t(1, 2, 2, 1)));
    let c1 = m[0][1].mul(&t(1, 0, 2, 2).sub(&t(1, 2, 2, 0)));
    let c2 = m[0][2].mul(&t(1, 0, 2, 1).sub(&t(1, 1, 2, 0)));
    c0.sub(&c1).add(&c2)
}

/// Sign of `dot(cross(b - a, c - a), d - a)`: +1 when `d` lies on the side of plane `abc` its normal points to.
pub fn orient3d(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> i32 {
    let (bx, by, bz) = (b[0] - a[0], b[1] - a[1], b[2] - a[2]);
    let (cx, cy, cz) = (c[0] - a[0], c[1] - a[1], c[2] - a[2]);
    let (dx, dy, dz) = (d[0] - a[0], d[1] - a[1], d[2] - a[2]);
    let det = bx * (cy * dz - cz * dy) - by * (cx * dz - cz * dx) + bz * (cx * dy - cy * dx);
    let perm = bx.abs() * ((cy * dz).abs() + (cz * dy).abs())
        + by.abs() * ((cx * dz).abs() + (cz * dx).abs())
        + bz.abs() * ((cx * dy).abs() + (cy * dx).abs());
    if det.abs() > 1e-13 * perm {
        return if det > 0.0 { 1 } else { -1 };
    }
    let r = |p: Vec3| [diff(p[0], a[0]), diff(p[1], a[1]), diff(p[2], a[2])];
    det3(&[r(b), r(c), r(d)]).sign()
}

/// Positive when `e` lies strictly inside the circumsphere of `abcd` (which must satisfy `orient3d(a,b,c,d) > 0`),
/// negative when outside, zero when exactly on it.
pub fn insphere(a: Vec3, b: Vec3, c: Vec3, d: Vec3, e: Vec3) -> i32 {
    let p = [a, b, c, d];
    let mut v = [[0.0f64; 4]; 4];
    let mut av = [[0.0f64; 4]; 4];
    for i in 0..4 {
        let (x, y, z) = (p[i][0] - e[0], p[i][1] - e[1], p[i][2] - e[2]);
        v[i] = [x, y, z, x * x + y * y + z * z];
        av[i] = [x.abs(), y.abs(), z.abs(), x * x + y * y + z * z];
    }
    let d3 = |m: &[[f64; 4]; 4], r: [usize; 3]| -> f64 {
        m[r[0]][0] * (m[r[1]][1] * m[r[2]][2] - m[r[1]][2] * m[r[2]][1])
            - m[r[0]][1] * (m[r[1]][0] * m[r[2]][2] - m[r[1]][2] * m[r[2]][0])
            + m[r[0]][2] * (m[r[1]][0] * m[r[2]][1] - m[r[1]][1] * m[r[2]][0])
    };
    let d3p = |m: &[[f64; 4]; 4], r: [usize; 3]| -> f64 {
        m[r[0]][0] * (m[r[1]][1] * m[r[2]][2] + m[r[1]][2] * m[r[2]][1])
            + m[r[0]][1] * (m[r[1]][0] * m[r[2]][2] + m[r[1]][2] * m[r[2]][0])
            + m[r[0]][2] * (m[r[1]][0] * m[r[2]][1] + m[r[1]][1] * m[r[2]][0])
    };
    // expansion along the last column: det = sum_i (-1)^(i+3) w_i det3(rows != i)
    let rows = [[1usize, 2, 3], [0, 2, 3], [0, 1, 3], [0, 1, 2]];
    let mut det = 0.0;
    let mut perm = 0.0;
    for i in 0..4 {
        let sgn = if (i + 3) % 2 == 0 { 1.0 } else { -1.0 };
        det += sgn * v[i][3] * d3(&v, rows[i]);
        perm += av[i][3] * d3p(&av, rows[i]);
    }
    // det(M) < 0 means inside for positively oriented abcd
    if det.abs() > 1e-12 * perm {
        return if det < 0.0 { 1 } else { -1 };
    }
    let mut ex: Vec<[Exp; 4]> = Vec::new();
    for q in p.iter() {
        let (x, y, z) = (diff(q[0], e[0]), diff(q[1], e[1]), diff(q[2], e[2]));
        let w = x.mul(&x).add(&y.mul(&y)).add(&z.mul(&z));
        ex.push([x, y, z, w]);
    }
    let mut total = Exp(vec![]);
    for i in 0..4 {
        let r = rows[i];
        let m3 = [
            [ex[r[0]][0].clone(), ex[r[0]][1].clone(), ex[r[0]][2].clone()],
            [ex[r[1]][0].clone(), ex[r[1]][1].clone(), ex[r[1]][2].clone()],
            [ex[r[2]][0].clone(), ex[r[2]][1].clone(), ex[r[2]][2].clone()],
        ];
        let term = ex[i][3].mul(&det3(&m3));
        total = if (i + 3) % 2 == 0 {
            total.add(&term)
        } else {
            total.sub(&term)
        };
    }
    -total.sign()
}
