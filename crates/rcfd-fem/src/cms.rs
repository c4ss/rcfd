//! Static condensation and component mode synthesis. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR
//! 0469 (F-430, design stub ADR 0333).
//!
//! A part is a [`SolidModel`] with its constraints, supports and loads, and a node set, its interface. Its unknowns are those
//! of the transformation `x = T q` of ADR 0296 (zero constraint values only): `K_q = T^T K T`, `M_q = T^T M T`, `f_q = T^T f`.
//! The boundary unknowns `b` are the independent unknowns of the interface nodes (nodes ascending, components ascending; a
//! fixed one is left out, a dependent of a constraint refused); every other independent unknown is interior `i`.
//!
//! Static condensation (Guyan, AIAA J. 3 (1965) 380, cited from memory): `Psi = -K_ii^{-1} K_ib`, `K_hat = K_bb + K_bi Psi`,
//! `f_hat = f_b + Psi^T f_i`, and the exact recovery `u_i = Psi u_b + K_ii^{-1} f_i` (block elimination).
//!
//! Fixed-interface component mode synthesis (Craig and Bampton, AIAA J. 6 (1968) 1313, cited from memory): the lowest `m`
//! M-normalised modes `Phi` of `K_ii phi = omega^2 M_ii phi` and `(u_b, u_i) = T_cb (u_b, eta)`, `T_cb = [[I, 0], [Psi, Phi]]`,
//! give `K_r = [[K_hat, 0], [0, Lambda]]` and `M_r = [[M_hat, M_bq], [M_bq^T, Phi^T M_ii Phi]]` with `M_hat = M_bb + M_bi Psi +
//! Psi^T M_ib + Psi^T M_ii Psi` and `M_bq = (M_bi + Psi^T M_ii) Phi`; the reduced load is `T_cb^T f_q`. `K_hat` and `M_r` are
//! symmetrised after the products (the asymmetry before is reported). The reduction is a Rayleigh-Ritz projection (ADR 0469).
//!
//! [`Assembly`] joins reduced parts at boundary unknowns of equal position and component; [`ReducedPart::write`] and
//! [`ReducedPart::read`] store a part as text with the bits of every float.
//!
//! Options of ADR 0474 (F-552), both off by default (the reduction is then that of ADR 0469, bit for bit):
//! [`CmsOptions::cutoff`] keeps every fixed-interface mode with `omega_k < omega_c`; [`CmsOptions::residual_vectors`] adds
//! the static residual vector of the part's interior load, `K_ii^{-1} f_i` M-orthogonalised (twice, ADR 0464) against the
//! kept modes and M-normalised, as a column `[0; R]` of the transformation: `T = [[I, 0, 0], [Psi, Phi, R]]`. Its blocks are
//! `K_qR = Phi^T K_ii R`, `K_RR = R^T K_ii R` (the coupling with the constraint modes is zero exactly), `M_bR = M_bi R +
//! Psi^T M_ii R`, `M_qR = Phi^T M_ii R`, `M_RR = R^T M_ii R`, and the load `R^T f_i`; with it the static solution of an
//! interior load lies in the range of `T` and the reduced statics are exact.

use crate::assemble::stiffness_with_materials;
use crate::error::{FemError, Result, SetKind};
use crate::loads::nodal_load_vector;
use crate::mass::consistent_with;
use crate::model::{Constraint, SolidModel};
use crate::mpc::{add_supports, refuse_nonzero_values, transformation, Transformation};
use rcfd_core::vec3::Vec3;
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen::{
    generalized_eigen_dense, lowest_eigenpairs, DenseEigen, ResidualTest, Shift, SubspaceOptions,
};
use rcfd_linalg::skyline::SkylineLdlt;
use rcfd_linalg::CsrMatrix;
use std::path::Path;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// Interior size up to which the fixed-interface modes come from the dense eigen-solution (the bound of
/// [`crate::harmonic::ModalBasis::all_modes`]); above it, from [`lowest_eigenpairs`].
pub const DENSE_LIMIT: usize = 600;

/// Two boundary unknowns are one unknown of an [`Assembly`] when their positions agree within this fraction of the largest
/// extent of the interfaces and their components are equal.
pub const MATCH_TOLERANCE: f64 = 1e-9;

/// The first line of a part file.
pub const FILE_HEADER: &str = "rcfd-cms-part 1";

/// The first line of the file of a part with residual vectors (ADR 0474: four sizes and a section `residual`).
pub const FILE_HEADER_2: &str = "rcfd-cms-part 2";

/// The first number of fixed-interface pairs asked of the subspace solver to find the modes below a cutoff (doubled until
/// one at or above it is found, ADR 0474).
pub const CUTOFF_FIRST_PAIRS: usize = 8;

fn invalid(key: &'static str, reason: String) -> FemError {
    FemError::InvalidValue { key, reason }
}

/// Options of [`reduce`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CmsOptions {
    /// Fixed-interface modes kept (`0`: static condensation).
    pub modes: usize,
    /// Worker threads of the assembly of `K` and `M` (`None`: the available parallelism); the result does not depend on it.
    pub threads: Option<usize>,
    /// Interior size up to which the eigen-solution is dense ([`DENSE_LIMIT`]).
    pub dense_limit: usize,
    /// Add the static residual vector of the part's interior load (ADR 0474; default `false`).
    pub residual_vectors: bool,
    /// Keep every fixed-interface mode with `omega_k < cutoff` (rad/s) instead of a mode count (ADR 0474; default `None`).
    pub cutoff: Option<f64>,
}

impl CmsOptions {
    /// `modes` fixed-interface modes, the default threads and dense limit.
    pub fn new(modes: usize) -> CmsOptions {
        CmsOptions {
            modes,
            threads: None,
            dense_limit: DENSE_LIMIT,
            residual_vectors: false,
            cutoff: None,
        }
    }
    /// The same options with a thread count.
    pub fn threads(mut self, threads: usize) -> CmsOptions {
        self.threads = Some(threads);
        self
    }
    /// The same options with another dense limit.
    pub fn dense_limit(mut self, limit: usize) -> CmsOptions {
        self.dense_limit = limit;
        self
    }
    /// The same options with (or without) the static residual vector of the interior load (ADR 0474).
    pub fn residual_vectors(mut self, on: bool) -> CmsOptions {
        self.residual_vectors = on;
        self
    }
    /// The same options keeping every fixed-interface mode with `omega_k < omega` (rad/s; ADR 0474). The mode count must
    /// then be 0.
    pub fn cutoff(mut self, omega: f64) -> CmsOptions {
        self.cutoff = Some(omega);
        self
    }
}

/// One boundary unknown of a part: its node, component and the node's position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryUnknown {
    /// Node of the part's mesh.
    pub node: usize,
    /// Component of the node.
    pub component: usize,
    /// Position of the node.
    pub position: Vec3,
}

/// A part reduced to its interface (ADR 0469). Matrices are dense and row-major; the reduced unknowns are the boundary
/// unknowns followed by the modal coordinates and then the residual-vector coordinates (ADR 0474).
#[derive(Debug, Clone, PartialEq)]
pub struct ReducedPart {
    boundary: Vec<BoundaryUnknown>,
    boundary_q: Vec<usize>,
    interior_q: Vec<usize>,
    n_mesh: usize,
    n_q: usize,
    modes: usize,
    stiffness: Vec<f64>,
    mass: Vec<f64>,
    load: Vec<f64>,
    static_interior: Vec<f64>,
    eigenvalues: Vec<f64>,
    omitted: Option<f64>,
    psi: Vec<f64>,
    phi: Vec<f64>,
    asymmetry: f64,
    n_residual: usize,
    residual: Vec<f64>,
}

/// Static condensation of `model` to the interface node set `interface` ([`reduce`] with no mode).
pub fn condense(model: &SolidModel, interface: &str, threads: Option<usize>) -> Result<ReducedPart> {
    let mut o = CmsOptions::new(0);
    o.threads = threads;
    reduce(model, interface, &o)
}

/// Rows `rows` and columns `cols` of `a` as a CSR matrix (`cols` ascending).
fn sub_csr(a: &CsrMatrix, rows: &[usize], col_of: &[usize], ncols: usize) -> CsrMatrix {
    let mut row_ptr = Vec::with_capacity(rows.len() + 1);
    let mut col = Vec::new();
    let mut val = Vec::new();
    row_ptr.push(0);
    for &r in rows {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            let c = col_of[a.col[p]];
            if c != usize::MAX {
                col.push(c);
                val.push(a.val[p]);
            }
        }
        row_ptr.push(col.len());
    }
    CsrMatrix::rect(rows.len(), ncols, row_ptr, col, val)
}

/// Rows `rows` of `a`, the columns mapped by `col_of`, as a dense row-major block.
fn dense_block(a: &CsrMatrix, rows: &[usize], col_of: &[usize], ncols: usize) -> Vec<f64> {
    let mut d = vec![0.0; rows.len() * ncols];
    for (ri, &r) in rows.iter().enumerate() {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            let c = col_of[a.col[p]];
            if c != usize::MAX {
                d[ri * ncols + c] += a.val[p];
            }
        }
    }
    d
}

/// `A X` for a CSR `a` (`n x k`) and a dense row-major `x` (`k x nc`).
fn csr_dense(a: &CsrMatrix, x: &[f64], nc: usize) -> Vec<f64> {
    let mut y = vec![0.0; a.n * nc];
    for r in 0..a.n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            let v = a.val[p];
            let c = a.col[p];
            for j in 0..nc {
                y[r * nc + j] += v * x[c * nc + j];
            }
        }
    }
    y
}

/// `A^T B` for dense row-major `a` (`k x n`) and `b` (`k x m`).
fn at_b(a: &[f64], b: &[f64], k: usize, n: usize, m: usize) -> Vec<f64> {
    let mut y = vec![0.0; n * m];
    for r in 0..k {
        for i in 0..n {
            let v = a[r * n + i];
            if v != 0.0 {
                for j in 0..m {
                    y[i * m + j] += v * b[r * m + j];
                }
            }
        }
    }
    y
}

/// `A B` for dense row-major `a` (`n x k`) and `b` (`k x m`).
fn a_b(a: &[f64], b: &[f64], n: usize, k: usize, m: usize) -> Vec<f64> {
    let mut y = vec![0.0; n * m];
    for i in 0..n {
        for r in 0..k {
            let v = a[i * k + r];
            if v != 0.0 {
                for j in 0..m {
                    y[i * m + j] += v * b[r * m + j];
                }
            }
        }
    }
    y
}

/// The largest `|a_ij - a_ji|` over the largest `|a|` (0 for a zero matrix).
pub fn relative_asymmetry(a: &[f64], n: usize) -> f64 {
    let big = a.iter().fold(0.0f64, |s, v| s.max(v.abs()));
    if big == 0.0 {
        return 0.0;
    }
    let mut d = 0.0f64;
    for i in 0..n {
        for j in i + 1..n {
            d = d.max((a[i * n + j] - a[j * n + i]).abs());
        }
    }
    d / big
}

fn symmetrise(a: &mut [f64], n: usize) {
    for i in 0..n {
        for j in i + 1..n {
            let v = 0.5 * (a[i * n + j] + a[j * n + i]);
            a[i * n + j] = v;
            a[j * n + i] = v;
        }
    }
}

/// Dense Cholesky `A = L L^T` (row-major lower `L`), `None` when `A` is not positive definite.
fn cholesky(a: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n];
    for j in 0..n {
        let mut s = a[j * n + j];
        for k in 0..j {
            s -= l[j * n + k] * l[j * n + k];
        }
        if s.is_nan() || s <= 0.0 {
            return None;
        }
        let d = s.sqrt();
        l[j * n + j] = d;
        for i in j + 1..n {
            let mut v = a[i * n + j];
            for k in 0..j {
                v -= l[i * n + k] * l[j * n + k];
            }
            l[i * n + j] = v / d;
        }
    }
    Some(l)
}

fn cholesky_solve(l: &[f64], n: usize, b: &[f64]) -> Vec<f64> {
    let mut y = b.to_vec();
    for i in 0..n {
        let mut s = y[i];
        for k in 0..i {
            s -= l[i * n + k] * y[k];
        }
        y[i] = s / l[i * n + i];
    }
    for i in (0..n).rev() {
        let mut s = y[i];
        for k in i + 1..n {
            s -= l[k * n + i] * y[k];
        }
        y[i] = s / l[i * n + i];
    }
    y
}

/// The entry checks of a reduction: the model, zero constraint values, no cyclic constraint.
fn check_model(model: &SolidModel) -> Result<()> {
    model.validate()?;
    model.check_supported_with_materials()?;
    if let Some(i) = model
        .constraints()
        .iter()
        .position(|c| matches!(c, Constraint::Cyclic { .. }))
    {
        return Err(FemError::NotYetSupported {
            item: "constraint",
            variant: "Cyclic",
            location: format!("constraint {i}, a substructure reduction"),
            package: "a follow-up of ADR 0469",
        });
    }
    refuse_nonzero_values(model, "a substructure reduction")
}

/// The boundary unknowns of `interface` on the transformation `tr` (ADR 0469).
fn boundary_unknowns(
    model: &SolidModel,
    tr: &Transformation,
    interface: &str,
) -> Result<(Vec<BoundaryUnknown>, Vec<usize>)> {
    let mesh = model.mesh();
    let nodes = mesh.node_set(interface).ok_or_else(|| FemError::UnknownSet {
        kind: SetKind::Node,
        name: interface.to_string(),
    })?;
    if nodes.is_empty() {
        return Err(invalid(
            "interface",
            format!("node set '{interface}' has no nodes: an interface needs at least one"),
        ));
    }
    let mut sorted = nodes.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let dofs = model.dof_map();
    let mut q_of = vec![usize::MAX; tr.n_unknowns()];
    for (a, &x) in tr.independent().iter().enumerate() {
        q_of[x] = a;
    }
    let t = tr.t();
    let mut out = Vec::new();
    let mut qs = Vec::new();
    for &node in &sorted {
        for component in 0..dofs.components(node) {
            let Some(eq) = dofs.equation(node, component) else {
                continue;
            };
            if q_of[eq] != usize::MAX {
                out.push(BoundaryUnknown {
                    node,
                    component,
                    position: mesh.nodes()[node],
                });
                qs.push(q_of[eq]);
            } else if t.row_ptr[eq + 1] > t.row_ptr[eq] {
                return Err(invalid(
                    "interface",
                    format!(
                        "node {node} component {component} of '{interface}' is the dependent of a constraint, not an \
                         unknown of the part"
                    ),
                ));
            }
        }
    }
    if qs.is_empty() {
        return Err(invalid(
            "interface",
            format!("node set '{interface}' has no free unknown (every component is fixed)"),
        ));
    }
    Ok((out, qs))
}

/// The subspace options of the fixed-interface modes (ADR 0469).
fn subspace_options() -> SubspaceOptions {
    SubspaceOptions {
        shift: Shift::Value(0.0),
        tolerance: 1e-12,
        residual_test: ResidualTest::Backward,
        ..SubspaceOptions::default()
    }
}

/// Every fixed-interface pair of the interior (dense).
fn dense_interior(kii: &CsrMatrix, mii: &CsrMatrix, ni: usize) -> Result<DenseEigen> {
    let ident: Vec<usize> = (0..ni).collect();
    let dk = dense_block(kii, &ident, &ident, ni);
    let dm = dense_block(mii, &ident, &ident, ni);
    generalized_eigen_dense(&dk, &dm, ni).map_err(|e| FemError::Solver {
        solver: "generalized_eigen_dense",
        reason: format!("fixed-interface modes: {e}"),
    })
}

/// Kept eigenvalues, kept vectors and the first omitted eigenvalue.
type Modes = (Vec<f64>, Vec<Vec<f64>>, Option<f64>);

/// The fixed-interface modes with eigenvalue below `c2` (ADR 0474): from the dense solution when `ni <= dense_limit`,
/// otherwise from the subspace solver with [`CUTOFF_FIRST_PAIRS`] + 1 pairs, the count doubled until a pair at or above
/// `c2` is found (the dense solution once `k + 1 >= ni`).
fn modes_below(kii: &CsrMatrix, mii: &CsrMatrix, ni: usize, c2: f64, dense_limit: usize) -> Result<Modes> {
    fn split(values: &[f64], vectors: &[Vec<f64>], c2: f64) -> Modes {
        let k = values.iter().take_while(|&&l| l < c2).count();
        (values[..k].to_vec(), vectors[..k].to_vec(), values.get(k).copied())
    }
    let mut k = CUTOFF_FIRST_PAIRS;
    loop {
        if ni <= dense_limit || k + 1 >= ni {
            let e = dense_interior(kii, mii, ni)?;
            return Ok(split(&e.values, &e.vectors, c2));
        }
        let e = lowest_eigenpairs(kii, mii, k + 1, &subspace_options()).map_err(|e| FemError::Solver {
            solver: "lowest_eigenpairs",
            reason: format!("fixed-interface modes: {e}"),
        })?;
        if e.values[k] >= c2 {
            return Ok(split(&e.values, &e.vectors, c2));
        }
        k *= 2;
    }
}

/// The reduction of `model` to the interface node set `interface` with `options.modes` fixed-interface modes (ADR 0469):
/// static condensation for `modes = 0`, the Craig-Bampton reduction otherwise; with [`CmsOptions::cutoff`] the modes below
/// it, with [`CmsOptions::residual_vectors`] the residual vector of the interior load (ADR 0474). Refuses an unknown or
/// empty interface, an interface without a free unknown, `modes` above the interior size, an interior that the interface
/// and the constraints do not hold, a non-zero constraint value, a cyclic constraint, a cutoff that is not finite and
/// positive, and a cutoff with a mode count.
pub fn reduce(model: &SolidModel, interface: &str, options: &CmsOptions) -> Result<ReducedPart> {
    check_model(model)?;
    let cutoff2 = match options.cutoff {
        None => None,
        Some(w) => {
            if !w.is_finite() || w <= 0.0 {
                return Err(invalid(
                    "cutoff",
                    format!("the cutoff {w:e} rad/s must be finite and positive"),
                ));
            }
            if options.modes != 0 {
                return Err(invalid(
                    "cutoff",
                    format!(
                        "{} fixed-interface modes and a cutoff {w:e} rad/s asked: give a mode count or a cutoff",
                        options.modes
                    ),
                ));
            }
            Some(w * w)
        }
    };
    let tr = transformation(model, &[])?;
    let (boundary, boundary_q) = boundary_unknowns(model, &tr, interface)?;
    let n_mesh = tr.n_mesh();
    let nn = tr.n_unknowns();
    let mut k = stiffness_with_materials(model, options.threads)?;
    add_supports(&mut k, model)?;
    let m = consistent_with(model, options.threads)?;
    let mut f = nodal_load_vector(model, &[])?;
    f.resize(nn, 0.0);
    let (kq, fq) = tr.reduce(&k, &f)?;
    let (mq, _) = tr.reduce(&m, &vec![0.0; nn])?;
    let n_q = kq.n;
    let mut is_b = vec![false; n_q];
    for &q in &boundary_q {
        is_b[q] = true;
    }
    let interior_q: Vec<usize> = (0..n_q).filter(|&q| !is_b[q]).collect();
    let nb = boundary_q.len();
    let ni = interior_q.len();
    let nm = options.modes;
    if ni == 0 {
        return Err(invalid(
            "interface",
            format!("node set '{interface}' takes every unknown of the part: no interior is left"),
        ));
    }
    if nm > ni {
        return Err(invalid(
            "modes",
            format!("{nm} fixed-interface modes asked of an interior of {ni} unknowns (at most {ni})"),
        ));
    }
    // Column maps of the boundary and the interior.
    let mut b_col = vec![usize::MAX; n_q];
    let mut i_col = vec![usize::MAX; n_q];
    for (a, &q) in boundary_q.iter().enumerate() {
        b_col[q] = a;
    }
    for (a, &q) in interior_q.iter().enumerate() {
        i_col[q] = a;
    }
    let kii = sub_csr(&kq, &interior_q, &i_col, ni);
    let mii = sub_csr(&mq, &interior_q, &i_col, ni);
    let kib = dense_block(&kq, &interior_q, &b_col, nb);
    let kbi = dense_block(&kq, &boundary_q, &i_col, ni);
    let kbb = dense_block(&kq, &boundary_q, &b_col, nb);
    let mib = dense_block(&mq, &interior_q, &b_col, nb);
    let mbi = dense_block(&mq, &boundary_q, &i_col, ni);
    let mbb = dense_block(&mq, &boundary_q, &b_col, nb);
    let fac = SkylineLdlt::factor(&kii).map_err(|e| FemError::Solver {
        solver: "skyline-ldlt",
        reason: format!("interior stiffness of the part: {e}"),
    })?;
    if let Some((r, d)) = fac.pivots().iter().enumerate().find(|(_, d)| d.is_nan() || **d <= 0.0) {
        return Err(invalid(
            "interface",
            format!(
                "the interior stiffness is not positive definite (pivot {r}: {d:e}): '{interface}' and the constraints do \
                 not hold the interior"
            ),
        ));
    }
    // Constraint modes Psi = -K_ii^{-1} K_ib, column by column.
    let mut psi = vec![0.0; ni * nb];
    let mut rhs = vec![0.0; ni];
    for j in 0..nb {
        for r in 0..ni {
            rhs[r] = -kib[r * nb + j];
        }
        let x = fac.solve(&rhs);
        for r in 0..ni {
            psi[r * nb + j] = x[r];
        }
    }
    let fb: Vec<f64> = boundary_q.iter().map(|&q| fq[q]).collect();
    let fi: Vec<f64> = interior_q.iter().map(|&q| fq[q]).collect();
    let static_interior = fac.solve(&fi);
    // Fixed-interface modes.
    let (eigenvalues, phi_cols, omitted) = if let Some(c2) = cutoff2 {
        modes_below(&kii, &mii, ni, c2, options.dense_limit)?
    } else if ni <= options.dense_limit || nm + 1 >= ni {
        let e = dense_interior(&kii, &mii, ni)?;
        let omitted = e.values.get(nm).copied();
        (e.values[..nm].to_vec(), e.vectors[..nm].to_vec(), omitted)
    } else {
        let e = lowest_eigenpairs(&kii, &mii, nm + 1, &subspace_options()).map_err(|e| FemError::Solver {
            solver: "lowest_eigenpairs",
            reason: format!("fixed-interface modes: {e}"),
        })?;
        (e.values[..nm].to_vec(), e.vectors[..nm].to_vec(), Some(e.values[nm]))
    };
    let nm = eigenvalues.len();
    let mut phi = vec![0.0; ni * nm];
    for (c, v) in phi_cols.iter().enumerate() {
        for r in 0..ni {
            phi[r * nm + c] = v[r];
        }
    }
    // The residual vector of the interior load (ADR 0474), M-orthonormalised against the kept modes.
    let mut residual_cols: Vec<Vec<f64>> = Vec::new();
    if options.residual_vectors {
        let mut r = static_interior.clone();
        let mut y = vec![0.0; ni];
        let basis: Vec<&[f64]> = phi_cols.iter().map(Vec::as_slice).collect();
        if crate::damped::m_orthonormalise(&mii, &mut r, &basis, &mut y) {
            residual_cols.push(r);
        }
    }
    let np = residual_cols.len();
    let mut residual = vec![0.0; ni * np];
    for (c, v) in residual_cols.iter().enumerate() {
        for r in 0..ni {
            residual[r * np + c] = v[r];
        }
    }
    // Reduced matrices.
    let nr = nb + nm + np;
    let kbi_psi = a_b(&kbi, &psi, nb, ni, nb);
    let mut khat = kbb.clone();
    for (a, v) in khat.iter_mut().zip(&kbi_psi) {
        *a += v;
    }
    let mbi_psi = a_b(&mbi, &psi, nb, ni, nb);
    let psit_mib = at_b(&psi, &mib, ni, nb, nb);
    let mii_psi = csr_dense(&mii, &psi, nb);
    let psit_mii_psi = at_b(&psi, &mii_psi, ni, nb, nb);
    let mut mhat = mbb.clone();
    for a in 0..nb * nb {
        mhat[a] += mbi_psi[a] + psit_mib[a] + psit_mii_psi[a];
    }
    // M_bq = M_bi Phi + Psi^T (M_ii Phi); M_qq = Phi^T M_ii Phi.
    let mii_phi = csr_dense(&mii, &phi, nm);
    let mbi_phi = a_b(&mbi, &phi, nb, ni, nm);
    let psit_mii_phi = at_b(&psi, &mii_phi, ni, nb, nm);
    let mqq = at_b(&phi, &mii_phi, ni, nm, nm);
    let mut kr = vec![0.0; nr * nr];
    let mut mr = vec![0.0; nr * nr];
    for i in 0..nb {
        for j in 0..nb {
            kr[i * nr + j] = khat[i * nb + j];
            mr[i * nr + j] = mhat[i * nb + j];
        }
        for c in 0..nm {
            let v = mbi_phi[i * nm + c] + psit_mii_phi[i * nm + c];
            mr[i * nr + nb + c] = v;
            mr[(nb + c) * nr + i] = v;
        }
    }
    for c in 0..nm {
        kr[(nb + c) * nr + nb + c] = eigenvalues[c];
        for d in 0..nm {
            mr[(nb + c) * nr + nb + d] = mqq[c * nm + d];
        }
    }
    if np > 0 {
        // K_qR = Phi^T K_ii R, K_RR = R^T K_ii R (K_bR = 0); M_bR = M_bi R + Psi^T M_ii R, M_qR, M_RR.
        let kii_r = csr_dense(&kii, &residual, np);
        let mii_r = csr_dense(&mii, &residual, np);
        let mbi_r = a_b(&mbi, &residual, nb, ni, np);
        let psit_mii_r = at_b(&psi, &mii_r, ni, nb, np);
        let phit_kii_r = at_b(&phi, &kii_r, ni, nm, np);
        let phit_mii_r = at_b(&phi, &mii_r, ni, nm, np);
        let rt_kii_r = at_b(&residual, &kii_r, ni, np, np);
        let rt_mii_r = at_b(&residual, &mii_r, ni, np, np);
        let o = nb + nm;
        for c in 0..np {
            for i in 0..nb {
                let v = mbi_r[i * np + c] + psit_mii_r[i * np + c];
                mr[i * nr + o + c] = v;
                mr[(o + c) * nr + i] = v;
            }
            for q in 0..nm {
                let (k, m) = (phit_kii_r[q * np + c], phit_mii_r[q * np + c]);
                kr[(nb + q) * nr + o + c] = k;
                kr[(o + c) * nr + nb + q] = k;
                mr[(nb + q) * nr + o + c] = m;
                mr[(o + c) * nr + nb + q] = m;
            }
            for d in 0..np {
                kr[(o + c) * nr + o + d] = rt_kii_r[c * np + d];
                mr[(o + c) * nr + o + d] = rt_mii_r[c * np + d];
            }
        }
    }
    let asymmetry = relative_asymmetry(&kr, nr).max(relative_asymmetry(&mr, nr));
    symmetrise(&mut kr, nr);
    symmetrise(&mut mr, nr);
    // Reduced load T_cb^T f_q.
    let psit_fi = at_b(&psi, &fi, ni, nb, 1);
    let phit_fi = at_b(&phi, &fi, ni, nm, 1);
    let mut load = fb;
    for (a, v) in load.iter_mut().zip(&psit_fi) {
        *a += v;
    }
    load.extend_from_slice(&phit_fi);
    if np > 0 {
        load.extend(at_b(&residual, &fi, ni, np, 1));
    }
    Ok(ReducedPart {
        boundary,
        boundary_q,
        interior_q,
        n_mesh,
        n_q,
        modes: nm,
        stiffness: kr,
        mass: mr,
        load,
        static_interior,
        eigenvalues,
        omitted,
        psi,
        phi,
        asymmetry,
        n_residual: np,
        residual,
    })
}

impl ReducedPart {
    /// The boundary unknowns, in the order of the reduced unknowns.
    pub fn boundary(&self) -> &[BoundaryUnknown] {
        &self.boundary
    }
    /// Number of boundary unknowns `n_b`.
    pub fn n_boundary(&self) -> usize {
        self.boundary.len()
    }
    /// Number of interior unknowns `n_i`.
    pub fn n_interior(&self) -> usize {
        self.interior_q.len()
    }
    /// Fixed-interface modes kept `m`.
    pub fn modes(&self) -> usize {
        self.modes
    }
    /// Residual vectors kept `p` (ADR 0474; 0 without the option).
    pub fn residual_vectors(&self) -> usize {
        self.n_residual
    }
    /// Order of the reduced matrices, `n_b + m + p`.
    pub fn n_reduced(&self) -> usize {
        self.boundary.len() + self.modes + self.n_residual
    }
    /// `K_r`, row-major.
    pub fn stiffness(&self) -> &[f64] {
        &self.stiffness
    }
    /// `M_r`, row-major.
    pub fn mass(&self) -> &[f64] {
        &self.mass
    }
    /// The reduced load `T_cb^T f_q` of the model's loads.
    pub fn load(&self) -> &[f64] {
        &self.load
    }
    /// `K_ii^{-1} f_i`, the interior response to the interior load with the boundary fixed (of the static recovery).
    pub fn static_interior(&self) -> &[f64] {
        &self.static_interior
    }
    /// The kept fixed-interface eigenvalues `omega_k^2`, ascending.
    pub fn fixed_interface_eigenvalues(&self) -> &[f64] {
        &self.eigenvalues
    }
    /// The first omitted fixed-interface eigenvalue (`None` when every mode is kept).
    pub fn omitted_eigenvalue(&self) -> Option<f64> {
        self.omitted
    }
    /// `Psi`, `n_i x n_b`, row-major.
    pub fn psi(&self) -> &[f64] {
        &self.psi
    }
    /// `Phi`, `n_i x m`, row-major.
    pub fn phi(&self) -> &[f64] {
        &self.phi
    }
    /// `R`, the M_ii-orthonormal residual vectors, `n_i x p`, row-major (ADR 0474).
    pub fn residual(&self) -> &[f64] {
        &self.residual
    }
    /// The relative asymmetry of `K_r` and `M_r` before their symmetrisation (the larger of the two).
    pub fn asymmetry(&self) -> f64 {
        self.asymmetry
    }
    /// The eigen-solution of the part alone, `K_r x = lambda M_r x` (dense).
    pub fn eigen(&self) -> Result<DenseEigen> {
        generalized_eigen_dense(&self.stiffness, &self.mass, self.n_reduced()).map_err(|e| FemError::Solver {
            solver: "generalized_eigen_dense",
            reason: format!("reduced part: {e}"),
        })
    }

    /// The condensed static solution `K_hat u_b = f_hat` of the part alone (the boundary unknowns).
    pub fn solve_static(&self) -> Result<Vec<f64>> {
        let nb = self.n_boundary();
        let nr = self.n_reduced();
        let mut khat = vec![0.0; nb * nb];
        for i in 0..nb {
            khat[i * nb..(i + 1) * nb].copy_from_slice(&self.stiffness[i * nr..i * nr + nb]);
        }
        let l = cholesky(&khat, nb).ok_or_else(|| {
            invalid(
                "interface",
                "the condensed stiffness is not positive definite: the part is not held by its constraints".into(),
            )
        })?;
        Ok(cholesky_solve(&l, nb, &self.load[..nb]))
    }

    /// Checks that `model` is the model of this part (its unknown counts and boundary positions); returns its transformation.
    fn transformation_of(&self, model: &SolidModel) -> Result<Transformation> {
        let tr = transformation(model, &[])?;
        let nq = tr.independent().len();
        let mesh = model.mesh();
        let same = tr.n_mesh() == self.n_mesh
            && nq == self.n_q
            && self
                .boundary
                .iter()
                .all(|b| b.node < mesh.nodes().len() && mesh.nodes()[b.node] == b.position);
        if !same {
            return Err(invalid(
                "model",
                format!(
                    "the model does not match the reduced part ({} mesh and {} reduced unknowns, or the boundary positions \
                     differ)",
                    self.n_mesh, self.n_q
                ),
            ));
        }
        Ok(tr)
    }

    fn expand_with(&self, model: &SolidModel, ub: &[f64], interior: &[f64]) -> Result<Vec<f64>> {
        let tr = self.transformation_of(model)?;
        let nb = self.n_boundary();
        let mut q = vec![0.0; self.n_q];
        for (a, &qq) in self.boundary_q.iter().enumerate() {
            q[qq] = ub[a];
        }
        let psi_ub = a_b(&self.psi, ub, self.n_interior(), nb, 1);
        for (a, &qq) in self.interior_q.iter().enumerate() {
            q[qq] = psi_ub[a] + interior[a];
        }
        let x = tr.expand(&q);
        Ok(x[..self.n_mesh].to_vec())
    }

    /// The static recovery of the mesh displacements from the boundary displacements `u_b`: `u_i = Psi u_b + K_ii^{-1} f_i`
    /// (exact), then `x = T q`.
    pub fn recover_static(&self, model: &SolidModel, ub: &[f64]) -> Result<Vec<f64>> {
        if ub.len() != self.n_boundary() {
            return Err(FemError::FieldLength {
                key: "boundary displacements",
                expected: self.n_boundary(),
                found: ub.len(),
            });
        }
        self.expand_with(model, ub, &self.static_interior)
    }

    /// The mesh displacements of reduced coordinates `(u_b, eta, rho)`: `u_i = Psi u_b + Phi eta + R rho`, then `x = T q`.
    pub fn expand(&self, model: &SolidModel, coordinates: &[f64]) -> Result<Vec<f64>> {
        if coordinates.len() != self.n_reduced() {
            return Err(FemError::FieldLength {
                key: "reduced coordinates",
                expected: self.n_reduced(),
                found: coordinates.len(),
            });
        }
        let nb = self.n_boundary();
        let ni = self.n_interior();
        let o = nb + self.modes;
        let mut interior = a_b(&self.phi, &coordinates[nb..o], ni, self.modes, 1);
        if self.n_residual > 0 {
            let r_rho = a_b(&self.residual, &coordinates[o..], ni, self.n_residual, 1);
            for (a, v) in interior.iter_mut().zip(&r_rho) {
                *a += v;
            }
        }
        self.expand_with(model, &coordinates[..nb], &interior)
    }

    /// The part as text (ADR 0469): header [`FILE_HEADER`], then one line per section, every float as the 16 hexadecimal
    /// digits of its bits. A part with residual vectors (ADR 0474) has header [`FILE_HEADER_2`], four sizes (the fourth the
    /// count of residual vectors) and a section `residual` after `phi`.
    pub fn to_text(&self) -> String {
        fn ints(out: &mut String, name: &str, v: &[usize]) {
            out.push_str(name);
            out.push(' ');
            out.push_str(&v.len().to_string());
            for x in v {
                out.push(' ');
                out.push_str(&x.to_string());
            }
            out.push('\n');
        }
        fn floats(out: &mut String, name: &str, v: &[f64]) {
            out.push_str(name);
            out.push(' ');
            out.push_str(&v.len().to_string());
            for x in v {
                out.push(' ');
                out.push_str(&format!("{:016x}", x.to_bits()));
            }
            out.push('\n');
        }
        let v2 = self.n_residual > 0;
        let mut s = String::new();
        s.push_str(if v2 { FILE_HEADER_2 } else { FILE_HEADER });
        s.push('\n');
        if v2 {
            ints(&mut s, "sizes", &[self.n_mesh, self.n_q, self.modes, self.n_residual]);
        } else {
            ints(&mut s, "sizes", &[self.n_mesh, self.n_q, self.modes]);
        }
        ints(
            &mut s,
            "nodes",
            &self.boundary.iter().map(|b| b.node).collect::<Vec<_>>(),
        );
        ints(
            &mut s,
            "components",
            &self.boundary.iter().map(|b| b.component).collect::<Vec<_>>(),
        );
        floats(
            &mut s,
            "positions",
            &self.boundary.iter().flat_map(|b| b.position).collect::<Vec<_>>(),
        );
        ints(&mut s, "boundary_q", &self.boundary_q);
        ints(&mut s, "interior_q", &self.interior_q);
        floats(&mut s, "stiffness", &self.stiffness);
        floats(&mut s, "mass", &self.mass);
        floats(&mut s, "load", &self.load);
        floats(&mut s, "static_interior", &self.static_interior);
        floats(&mut s, "eigenvalues", &self.eigenvalues);
        floats(&mut s, "omitted", &self.omitted.into_iter().collect::<Vec<_>>());
        floats(&mut s, "psi", &self.psi);
        floats(&mut s, "phi", &self.phi);
        if v2 {
            floats(&mut s, "residual", &self.residual);
        }
        floats(&mut s, "asymmetry", &[self.asymmetry]);
        s.push_str("end\n");
        s
    }

    /// The part from the text of [`ReducedPart::to_text`] (either header); refuses another header, a missing, short or long
    /// section and a malformed number.
    pub fn from_text(text: &str) -> Result<ReducedPart> {
        let bad = |reason: String| invalid("part file", reason);
        let mut lines = text.split('\n');
        let v2 = match lines.next() {
            Some(FILE_HEADER) => false,
            Some(FILE_HEADER_2) => true,
            _ => {
                return Err(bad(format!(
                    "the first line is neither '{FILE_HEADER}' nor '{FILE_HEADER_2}'"
                )))
            }
        };
        let mut section = |name: &str| -> Result<Vec<String>> {
            let line = lines
                .next()
                .ok_or_else(|| bad(format!("section '{name}' is missing")))?;
            let mut it = line.split(' ');
            if it.next() != Some(name) {
                return Err(bad(format!("section '{name}' expected")));
            }
            let count: usize = it
                .next()
                .and_then(|c| c.parse().ok())
                .ok_or_else(|| bad(format!("section '{name}' has no count")))?;
            let v: Vec<String> = it.map(|s| s.to_string()).collect();
            if v.len() != count {
                return Err(bad(format!(
                    "section '{name}' has {} values, {count} expected",
                    v.len()
                )));
            }
            Ok(v)
        };
        let to_ints = |name: &str, v: Vec<String>| -> Result<Vec<usize>> {
            v.iter()
                .map(|s| {
                    s.parse::<usize>()
                        .map_err(|_| bad(format!("section '{name}': '{s}' is not an integer")))
                })
                .collect()
        };
        let to_floats = |name: &str, v: Vec<String>| -> Result<Vec<f64>> {
            v.iter()
                .map(|s| {
                    if s.len() != 16 {
                        return Err(bad(format!("section '{name}': '{s}' is not 16 hexadecimal digits")));
                    }
                    u64::from_str_radix(s, 16)
                        .map(f64::from_bits)
                        .map_err(|_| bad(format!("section '{name}': '{s}' is not hexadecimal")))
                })
                .collect()
        };
        let sizes = to_ints("sizes", section("sizes")?)?;
        let n_sizes = if v2 { 4 } else { 3 };
        if sizes.len() != n_sizes {
            return Err(bad(format!("section 'sizes' must have {n_sizes} values")));
        }
        let n_residual = if v2 { sizes[3] } else { 0 };
        if v2 && n_residual == 0 {
            return Err(bad(format!(
                "a '{FILE_HEADER_2}' file has at least one residual vector"
            )));
        }
        let nodes = to_ints("nodes", section("nodes")?)?;
        let components = to_ints("components", section("components")?)?;
        let positions = to_floats("positions", section("positions")?)?;
        let boundary_q = to_ints("boundary_q", section("boundary_q")?)?;
        let interior_q = to_ints("interior_q", section("interior_q")?)?;
        let stiffness = to_floats("stiffness", section("stiffness")?)?;
        let mass = to_floats("mass", section("mass")?)?;
        let load = to_floats("load", section("load")?)?;
        let static_interior = to_floats("static_interior", section("static_interior")?)?;
        let eigenvalues = to_floats("eigenvalues", section("eigenvalues")?)?;
        let omitted = to_floats("omitted", section("omitted")?)?;
        let psi = to_floats("psi", section("psi")?)?;
        let phi = to_floats("phi", section("phi")?)?;
        let residual = if v2 {
            to_floats("residual", section("residual")?)?
        } else {
            Vec::new()
        };
        let asymmetry = to_floats("asymmetry", section("asymmetry")?)?;
        if lines.next() != Some("end") {
            return Err(bad("the line 'end' is missing".into()));
        }
        let (n_mesh, n_q, modes) = (sizes[0], sizes[1], sizes[2]);
        let nb = nodes.len();
        let ni = interior_q.len();
        let nr = nb + modes + n_residual;
        let consistent = components.len() == nb
            && positions.len() == 3 * nb
            && boundary_q.len() == nb
            && nb + ni == n_q
            && modes <= ni
            && stiffness.len() == nr * nr
            && mass.len() == nr * nr
            && load.len() == nr
            && static_interior.len() == ni
            && eigenvalues.len() == modes
            && omitted.len() <= 1
            && psi.len() == ni * nb
            && phi.len() == ni * modes
            && residual.len() == ni * n_residual
            && asymmetry.len() == 1
            && boundary_q.iter().chain(&interior_q).all(|&q| q < n_q);
        if !consistent {
            return Err(bad("the section sizes do not agree with each other".into()));
        }
        let boundary = (0..nb)
            .map(|a| BoundaryUnknown {
                node: nodes[a],
                component: components[a],
                position: [positions[3 * a], positions[3 * a + 1], positions[3 * a + 2]],
            })
            .collect();
        Ok(ReducedPart {
            boundary,
            boundary_q,
            interior_q,
            n_mesh,
            n_q,
            modes,
            stiffness,
            mass,
            load,
            static_interior,
            eigenvalues,
            omitted: omitted.first().copied(),
            psi,
            phi,
            asymmetry: asymmetry[0],
            n_residual,
            residual,
        })
    }

    /// Writes the part to `path` ([`ReducedPart::to_text`]).
    pub fn write(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_text()).map_err(|e| invalid("part file", format!("{}: {e}", path.display())))
    }

    /// Reads a part written by [`ReducedPart::write`].
    pub fn read(path: &Path) -> Result<ReducedPart> {
        let text =
            std::fs::read_to_string(path).map_err(|e| invalid("part file", format!("{}: {e}", path.display())))?;
        ReducedPart::from_text(&text)
    }
}

/// Reduced parts joined at their boundary unknowns (ADR 0469): the unknowns are the distinct boundary unknowns (matched by
/// position within [`MATCH_TOLERANCE`] of the largest interface extent and by component), in the order met, then the interior
/// coordinates of each part in order (its modal coordinates, then its residual-vector coordinates, ADR 0474).
#[derive(Debug, Clone, PartialEq)]
pub struct Assembly {
    parts: Vec<ReducedPart>,
    unknowns: Vec<(Vec3, usize)>,
    maps: Vec<Vec<usize>>,
    n: usize,
    stiffness: Vec<f64>,
    mass: Vec<f64>,
    load: Vec<f64>,
}

impl Assembly {
    /// Joins `parts`. Refuses no part, and two boundary unknowns of one part that match one unknown of the assembly.
    pub fn new(parts: Vec<ReducedPart>) -> Result<Assembly> {
        if parts.is_empty() {
            return Err(invalid("parts", "an assembly needs at least one part".into()));
        }
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in &parts {
            for b in &p.boundary {
                for c in 0..3 {
                    lo[c] = lo[c].min(b.position[c]);
                    hi[c] = hi[c].max(b.position[c]);
                }
            }
        }
        let extent = (0..3).map(|c| hi[c] - lo[c]).fold(0.0f64, f64::max);
        let tol = MATCH_TOLERANCE * extent.max(f64::MIN_POSITIVE);
        let mut unknowns: Vec<(Vec3, usize)> = Vec::new();
        let mut maps = Vec::with_capacity(parts.len());
        for (pi, p) in parts.iter().enumerate() {
            let mut map = Vec::with_capacity(p.n_reduced());
            for b in &p.boundary {
                let found = unknowns
                    .iter()
                    .position(|(x, c)| *c == b.component && (0..3).all(|k| (x[k] - b.position[k]).abs() <= tol));
                let g = match found {
                    Some(g) => {
                        if map.contains(&g) {
                            return Err(invalid(
                                "parts",
                                format!(
                                    "part {pi}: two boundary unknowns match one unknown of the assembly (node {} \
                                     component {})",
                                    b.node, b.component
                                ),
                            ));
                        }
                        g
                    }
                    None => {
                        unknowns.push((b.position, b.component));
                        unknowns.len() - 1
                    }
                };
                map.push(g);
            }
            maps.push(map);
        }
        let nbg = unknowns.len();
        let mut n = nbg;
        for (p, map) in parts.iter().zip(maps.iter_mut()) {
            for _ in p.n_boundary()..p.n_reduced() {
                map.push(n);
                n += 1;
            }
        }
        let mut stiffness = vec![0.0; n * n];
        let mut mass = vec![0.0; n * n];
        let mut load = vec![0.0; n];
        for (p, map) in parts.iter().zip(&maps) {
            let nr = p.n_reduced();
            for a in 0..nr {
                load[map[a]] += p.load[a];
                for c in 0..nr {
                    stiffness[map[a] * n + map[c]] += p.stiffness[a * nr + c];
                    mass[map[a] * n + map[c]] += p.mass[a * nr + c];
                }
            }
        }
        Ok(Assembly {
            parts,
            unknowns,
            maps,
            n,
            stiffness,
            mass,
            load,
        })
    }
    /// The parts.
    pub fn parts(&self) -> &[ReducedPart] {
        &self.parts
    }
    /// The number of unknowns.
    pub fn n_unknowns(&self) -> usize {
        self.n
    }
    /// The boundary unknowns of the assembly (position, component).
    pub fn boundary_unknowns(&self) -> &[(Vec3, usize)] {
        &self.unknowns
    }
    /// The assembly unknown of each reduced unknown of part `p`.
    pub fn map(&self, p: usize) -> &[usize] {
        &self.maps[p]
    }
    /// The stiffness, row-major.
    pub fn stiffness(&self) -> &[f64] {
        &self.stiffness
    }
    /// The mass, row-major.
    pub fn mass(&self) -> &[f64] {
        &self.mass
    }
    /// The load (the sum of the parts' reduced loads).
    pub fn load(&self) -> &[f64] {
        &self.load
    }
    /// The dense eigen-solution `K x = lambda M x`.
    pub fn eigen(&self) -> Result<DenseEigen> {
        generalized_eigen_dense(&self.stiffness, &self.mass, self.n).map_err(|e| FemError::Solver {
            solver: "generalized_eigen_dense",
            reason: format!("assembly: {e}"),
        })
    }
    /// The coordinates of part `p` in an assembly vector `x`.
    pub fn part_coordinates(&self, p: usize, x: &[f64]) -> Vec<f64> {
        self.maps[p].iter().map(|&g| x[g]).collect()
    }
    /// The steady harmonic response `((1 + i eta) K - omega^2 M) X = F` of the assembly's load, as `(Re X, Im X)` (dense
    /// Gaussian elimination with partial pivoting). Refuses a non-finite or negative input and a singular matrix.
    pub fn harmonic(&self, omega: f64, loss: f64) -> Result<(Vec<f64>, Vec<f64>)> {
        if !omega.is_finite() || omega < 0.0 || !loss.is_finite() || loss < 0.0 {
            return Err(invalid(
                "harmonic",
                format!("omega {omega:e} and loss factor {loss:e} must be finite and not negative"),
            ));
        }
        let n = self.n;
        let w2 = omega * omega;
        let mut a: Vec<C64> = (0..n * n)
            .map(|p| C64::new(self.stiffness[p] - w2 * self.mass[p], loss * self.stiffness[p]))
            .collect();
        let mut b: Vec<C64> = self.load.iter().map(|&v| C64::from_real(v)).collect();
        let scale = a.iter().fold(0.0f64, |s, v| s.max(v.abs()));
        for col in 0..n {
            let mut piv = col;
            for r in col + 1..n {
                if a[r * n + col].abs() > a[piv * n + col].abs() {
                    piv = r;
                }
            }
            if a[piv * n + col].abs() <= f64::EPSILON * scale {
                return Err(invalid(
                    "harmonic",
                    format!("the matrix of the assembly is singular at omega {omega:e} (column {col})"),
                ));
            }
            if piv != col {
                for c in 0..n {
                    a.swap(col * n + c, piv * n + c);
                }
                b.swap(col, piv);
            }
            let d = a[col * n + col];
            for r in col + 1..n {
                let factor = a[r * n + col] / d;
                if factor.abs() != 0.0 {
                    for c in col..n {
                        let v = a[col * n + c];
                        a[r * n + c] -= factor * v;
                    }
                    let v = b[col];
                    b[r] -= factor * v;
                }
            }
        }
        let mut x = vec![C64::ZERO; n];
        for r in (0..n).rev() {
            let mut s = b[r];
            for c in r + 1..n {
                s -= a[r * n + c] * x[c];
            }
            x[r] = s / a[r * n + r];
        }
        Ok((x.iter().map(|v| v.re).collect(), x.iter().map(|v| v.im).collect()))
    }
}
