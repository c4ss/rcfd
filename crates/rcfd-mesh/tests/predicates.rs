//! ADR 0030 T9: exact predicates against an i128 reference on lattice points (many exact degeneracies).

use rcfd_mesh::predicates::{insphere, orient3d};

struct Lcg(u64);
impl Lcg {
    fn next(&mut self, m: i64) -> i64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as i64) % m
    }
}

type P = [i64; 3];

fn sub(a: P, b: P) -> [i128; 3] {
    [(a[0] - b[0]) as i128, (a[1] - b[1]) as i128, (a[2] - b[2]) as i128]
}

fn det3(r: [[i128; 3]; 3]) -> i128 {
    r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1]) - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
        + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0])
}

fn ref_orient(a: P, b: P, c: P, d: P) -> i32 {
    det3([sub(b, a), sub(c, a), sub(d, a)]).signum() as i32
}

fn ref_insphere(a: P, b: P, c: P, d: P, e: P) -> i32 {
    let rows: Vec<[i128; 4]> = [a, b, c, d]
        .iter()
        .map(|p| {
            let v = sub(*p, e);
            [v[0], v[1], v[2], v[0] * v[0] + v[1] * v[1] + v[2] * v[2]]
        })
        .collect();
    let mut det = 0i128;
    for i in 0..4 {
        let o: Vec<usize> = (0..4).filter(|&k| k != i).collect();
        let m = [
            [rows[o[0]][0], rows[o[0]][1], rows[o[0]][2]],
            [rows[o[1]][0], rows[o[1]][1], rows[o[1]][2]],
            [rows[o[2]][0], rows[o[2]][1], rows[o[2]][2]],
        ];
        let s = if (i + 3) % 2 == 0 { 1 } else { -1 };
        det += s * rows[i][3] * det3(m);
    }
    -(det.signum() as i32)
}

fn f(p: P, s: f64) -> [f64; 3] {
    [p[0] as f64 * s, p[1] as f64 * s, p[2] as f64 * s]
}

#[test]
fn t9_exact_predicates() {
    let mut rng = Lcg(12345);
    let s = 2f64.powi(-30);
    let mut zero_o = 0;
    let mut zero_i = 0;
    let mut n_i = 0;
    for case in 0..4000 {
        let m = if case % 2 == 0 { 4 } else { 4096 };
        let mut pt = || [rng.next(m), rng.next(m), rng.next(m)];
        let (a, b, c, d, e) = (pt(), pt(), pt(), pt(), pt());
        let ro = ref_orient(a, b, c, d);
        assert_eq!(orient3d(f(a, s), f(b, s), f(c, s), f(d, s)), ro);
        if ro == 0 {
            zero_o += 1;
            continue;
        }
        let (b, c) = if ro < 0 { (c, b) } else { (b, c) };
        let ri = ref_insphere(a, b, c, d, e);
        assert_eq!(insphere(f(a, s), f(b, s), f(c, s), f(d, s), f(e, s)), ri, "case {case}");
        n_i += 1;
        if ri == 0 {
            zero_i += 1;
        }
    }
    assert!(
        zero_o > 50 && zero_i > 5 && n_i > 1000,
        "degenerate cases exercised: {zero_o} {zero_i} {n_i}"
    );
}
