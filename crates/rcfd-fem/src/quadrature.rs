//! Quadrature rules of the element library (ADR 0273 D3). Declared by ADR 0271; filled by ADR 0273.
//!
//! Points are in reference coordinates: the reference tetrahedron `{x, y, z >= 0, x + y + z <= 1}` (volume 1/6) with
//! `(x, y, z) = (L1, L2, L3)`, or the cube `[-1, 1]^3`. Weights are absolute on the reference domain (they sum to 1/6 or 8).
//! Every rule is checked by the monomial test of ADR 0273 (check A4), never taken on trust.

use std::sync::OnceLock;

/// The reference domain of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Domain {
    /// The reference tetrahedron; the stated degree is the total degree.
    Tetrahedron,
    /// The cube `[-1, 1]^3`; the stated degree is the degree in each variable.
    Cube,
}

/// A quadrature rule on a reference domain.
#[derive(Debug, Clone, PartialEq)]
pub struct QuadratureRule {
    name: &'static str,
    domain: Domain,
    degree: u32,
    points: Vec<[f64; 3]>,
    weights: Vec<f64>,
}

impl QuadratureRule {
    /// Name of the rule (`tet_1`, `tet_4`, `tet_14`, `gauss_1`, `gauss_2`, `gauss_3`).
    pub fn name(&self) -> &'static str {
        self.name
    }
    /// Reference domain.
    pub fn domain(&self) -> Domain {
        self.domain
    }
    /// Stated degree: total degree on the tetrahedron, degree in each variable on the cube.
    pub fn degree(&self) -> u32 {
        self.degree
    }
    /// Points in reference coordinates.
    pub fn points(&self) -> &[[f64; 3]] {
        &self.points
    }
    /// Absolute weights on the reference domain.
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }
    /// Number of points.
    pub fn len(&self) -> usize {
        self.points.len()
    }
    /// Whether the rule has no point (never true for the rules of this module).
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// Barycentric `(L0, L1, L2, L3)` to reference coordinates `(L1, L2, L3)`.
fn bary(l: [f64; 4]) -> [f64; 3] {
    [l[1], l[2], l[3]]
}

/// The four points `(1 - 3a, a, a, a)` and its permutations.
fn orbit31(a: f64) -> [[f64; 3]; 4] {
    let b = 1.0 - 3.0 * a;
    [
        bary([b, a, a, a]),
        bary([a, b, a, a]),
        bary([a, a, b, a]),
        bary([a, a, a, b]),
    ]
}

/// The six points with two barycentric coordinates `b` and two `1/2 - b`.
fn orbit22(b: f64) -> [[f64; 3]; 6] {
    let c = 0.5 - b;
    [
        bary([b, b, c, c]),
        bary([b, c, b, c]),
        bary([b, c, c, b]),
        bary([c, b, b, c]),
        bary([c, b, c, b]),
        bary([c, c, b, b]),
    ]
}

/// One-point rule at the centroid of the tetrahedron, exact to total degree 1.
pub fn tet_1() -> &'static QuadratureRule {
    static R: OnceLock<QuadratureRule> = OnceLock::new();
    R.get_or_init(|| QuadratureRule {
        name: "tet_1",
        domain: Domain::Tetrahedron,
        degree: 1,
        points: vec![[0.25, 0.25, 0.25]],
        weights: vec![1.0 / 6.0],
    })
}

/// Four-point rule, exact to total degree 2: `a = (5 - sqrt 5)/20` (ADR 0273 arithmetic 3), weights 1/24.
pub fn tet_4() -> &'static QuadratureRule {
    static R: OnceLock<QuadratureRule> = OnceLock::new();
    R.get_or_init(|| QuadratureRule {
        name: "tet_4",
        domain: Domain::Tetrahedron,
        degree: 2,
        points: orbit31(0.138_196_601_125_010_5).to_vec(),
        weights: vec![1.0 / 24.0; 4],
    })
}

// The 14-point rule of ADR 0273 D3: constants solved by Newton's method on the six invariant moment equations
// (w49i-work/f372-scratch/tet14.py, final residual 2.8e-17), from start values recalled from memory.
const TET14_A1: f64 = 0.092_735_250_310_891_53;
const TET14_W1: f64 = 0.012_248_840_519_393_714;
const TET14_A2: f64 = 0.310_885_919_263_300_67;
const TET14_W2: f64 = 0.018_781_320_953_002_73;
const TET14_B: f64 = 0.045_503_704_125_648_88;
const TET14_W3: f64 = 0.007_091_003_462_846_812;

/// Fourteen-point rule with positive weights, exact to total degree 5 (the mass rule of Tet10).
pub fn tet_14() -> &'static QuadratureRule {
    static R: OnceLock<QuadratureRule> = OnceLock::new();
    R.get_or_init(|| {
        let mut points = Vec::with_capacity(14);
        let mut weights = Vec::with_capacity(14);
        points.extend(orbit31(TET14_A1));
        weights.extend([TET14_W1; 4]);
        points.extend(orbit31(TET14_A2));
        weights.extend([TET14_W2; 4]);
        points.extend(orbit22(TET14_B));
        weights.extend([TET14_W3; 6]);
        QuadratureRule {
            name: "tet_14",
            domain: Domain::Tetrahedron,
            degree: 5,
            points,
            weights,
        }
    })
}

/// Gauss-Legendre points and weights on [-1, 1] with `n` = 1, 2, 3 points (ADR 0273 arithmetic 2).
fn gauss_line(n: usize) -> (Vec<f64>, Vec<f64>) {
    match n {
        1 => (vec![0.0], vec![2.0]),
        2 => {
            let g = 0.577_350_269_189_625_8; // 1/sqrt(3)
            (vec![-g, g], vec![1.0, 1.0])
        }
        3 => {
            let g = 0.774_596_669_241_483_4; // sqrt(3/5)
            (vec![-g, 0.0, g], vec![5.0 / 9.0, 8.0 / 9.0, 5.0 / 9.0])
        }
        _ => unreachable!("Gauss rule with {n} points"),
    }
}

/// Tensor-product Gauss rule on the cube, `n` points per direction; points ordered with `xi` fastest.
fn gauss_cube(name: &'static str, n: usize) -> QuadratureRule {
    let (x, w) = gauss_line(n);
    let mut points = Vec::with_capacity(n * n * n);
    let mut weights = Vec::with_capacity(n * n * n);
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                points.push([x[i], x[j], x[k]]);
                weights.push(w[i] * w[j] * w[k]);
            }
        }
    }
    QuadratureRule {
        name,
        domain: Domain::Cube,
        degree: (2 * n - 1) as u32,
        points,
        weights,
    }
}

/// One-point Gauss rule on the cube, exact to degree 1 in each variable (reduced integration).
pub fn gauss_1() -> &'static QuadratureRule {
    static R: OnceLock<QuadratureRule> = OnceLock::new();
    R.get_or_init(|| gauss_cube("gauss_1", 1))
}

/// 2 x 2 x 2 Gauss rule on the cube, exact to degree 3 in each variable (Hex8 stiffness).
pub fn gauss_2() -> &'static QuadratureRule {
    static R: OnceLock<QuadratureRule> = OnceLock::new();
    R.get_or_init(|| gauss_cube("gauss_2", 2))
}

/// 3 x 3 x 3 Gauss rule on the cube, exact to degree 5 in each variable (Hex8 mass).
pub fn gauss_3() -> &'static QuadratureRule {
    static R: OnceLock<QuadratureRule> = OnceLock::new();
    R.get_or_init(|| gauss_cube("gauss_3", 3))
}

/// Every rule of this module, in a fixed order (for the checks).
pub fn all_rules() -> [&'static QuadratureRule; 6] {
    [tet_1(), tet_4(), tet_14(), gauss_1(), gauss_2(), gauss_3()]
}
