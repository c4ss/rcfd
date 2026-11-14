//! Reduced parts as superelements in an ordinary model (ADR 0536, F-553).
//!
//! A [`ReducedPart`] of ADR 0469 is attached to a host [`SolidModel`] at its interface nodes: each boundary unknown is the
//! unknown of the host node at the same position (within [`MATCH_TOLERANCE`] of the host's largest extent) and of the same
//! component, which must be an independent unknown of the host transformation `x = T q` (ADR 0296). The unknowns of the
//! combined model are `z = (q, eta_1, eta_2, ..)`: the host's independent unknowns, then the generalised coordinates of each
//! part (its modal and residual-vector coordinates, in its reduced order). With the selection `r_p = S_p z`, `K = K_q (+) sum
//! S_p^T K_r S_p`, `M` alike, `f = f_q (+) sum S_p^T f_r`: the energies of the host and of the parts added (the direct
//! stiffness method on the projected part).
//!
//! Statics: `K z = f` (envelope `L D L^T`); the part's interior by the exact static recovery of ADR 0469. Modal analysis:
//! the lowest pairs of `(K, M)` by the subspace iteration. Harmonic: `(K + i D - omega^2 M) X = F` with the hysteretic
//! damping `D = eta_h K_q (+) sum eta_p S_p^T K_r S_p` (one loss factor for the host, one per part). Rotations of beam and
//! shell interface nodes are joined like any other component; a part component that the host node lacks is refused.

use super::{check_model, invalid, ReducedPart, MATCH_TOLERANCE};
use crate::assemble::stiffness_with_materials;
use crate::error::{FemError, Result};
use crate::harmonic::HarmonicLoad;
use crate::loads::nodal_load_vector;
use crate::mass::consistent_with;
use crate::modal::ModalOptions;
use crate::model::SolidModel;
use crate::mpc::{add_supports, transformation, Transformation};
use rcfd_linalg::complex::C64;
use rcfd_linalg::cskyline::{ComplexCsr, ComplexSkylineLdlt};
use rcfd_linalg::eigen::{lowest_eigenpairs, Shift, SubspaceOptions};
use rcfd_linalg::skyline::SkylineLdlt;
use rcfd_linalg::CsrMatrix;

/// One attached part: the part and the combined index of each of its reduced unknowns.
#[derive(Debug, Clone, PartialEq)]
struct Attached {
    part: ReducedPart,
    map: Vec<usize>,
}

/// A host model with reduced parts attached at its nodes (ADR 0536).
#[derive(Debug, Clone, PartialEq)]
pub struct SuperelementModel {
    host: SolidModel,
    tr: Transformation,
    tolerance: f64,
    parts: Vec<Attached>,
}

/// The matrices and load of a combined model over its unknowns `z`.
#[derive(Debug, Clone, PartialEq)]
pub struct SuperMatrices {
    /// `K = K_q (+) sum S_p^T K_r S_p`.
    pub stiffness: CsrMatrix,
    /// `M = M_q (+) sum S_p^T M_r S_p`.
    pub mass: CsrMatrix,
    /// `f = f_q (+) sum S_p^T f_r`.
    pub load: Vec<f64>,
}

/// A static solution of a combined model.
#[derive(Debug, Clone, PartialEq)]
pub struct SuperStatic {
    /// The combined unknowns `z`.
    pub coordinates: Vec<f64>,
    /// The host's mesh displacements `x = T q` (the mesh unknowns only).
    pub host_displacement: Vec<f64>,
}

/// The lowest modes of a combined model.
#[derive(Debug, Clone, PartialEq)]
pub struct SuperModes {
    /// Eigenvalues `omega^2`, ascending.
    pub eigenvalues: Vec<f64>,
    /// M-orthonormal mode vectors over the combined unknowns.
    pub vectors: Vec<Vec<f64>>,
}

/// Options of [`SuperelementModel::harmonic`].
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct SuperHarmonicOptions {
    /// Loss factor of the host (every element and elastic support) and default of every part.
    pub loss: f64,
    /// Loss factors of given parts (part index, loss factor), replacing the default.
    pub part_loss: Vec<(usize, f64)>,
    /// Worker threads of the host assembly (`None`: the available parallelism); the result does not depend on it.
    pub threads: Option<usize>,
}

impl SuperHarmonicOptions {
    /// No damping, the default threads.
    pub fn new() -> SuperHarmonicOptions {
        SuperHarmonicOptions::default()
    }
    /// The same options with the loss factor of the host (and the default of the parts).
    pub fn loss(mut self, eta: f64) -> SuperHarmonicOptions {
        self.loss = eta;
        self
    }
    /// The same options with the loss factor of part `p`.
    pub fn part_loss(mut self, p: usize, eta: f64) -> SuperHarmonicOptions {
        self.part_loss.push((p, eta));
        self
    }
    /// The same options with a thread count.
    pub fn threads(mut self, threads: usize) -> SuperHarmonicOptions {
        self.threads = Some(threads);
        self
    }
}

/// A harmonic load of a combined model: a load over the host's mesh equations and loads over the reduced unknowns of parts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SuperLoad {
    host: Option<HarmonicLoad>,
    parts: Vec<(usize, HarmonicLoad)>,
}

impl SuperLoad {
    /// No load.
    pub fn new() -> SuperLoad {
        SuperLoad::default()
    }
    /// The same load with a load over the host's mesh equations.
    pub fn host(mut self, load: HarmonicLoad) -> SuperLoad {
        self.host = Some(load);
        self
    }
    /// The same load with a load over the reduced unknowns of part `p` (for example [`ReducedPart::reduce_load`]).
    pub fn part(mut self, p: usize, load: HarmonicLoad) -> SuperLoad {
        self.parts.push((p, load));
        self
    }
}

/// The complex response over the combined unknowns.
#[derive(Debug, Clone, PartialEq)]
pub struct SuperResponse {
    /// The circular frequency, rad/s.
    pub omega: f64,
    /// The real part.
    pub re: Vec<f64>,
    /// The imaginary part.
    pub im: Vec<f64>,
}

/// The direct harmonic solver of a combined model: `K`, `D`, `M` assembled once, one complex factorisation per frequency.
#[derive(Debug, Clone, PartialEq)]
pub struct SuperHarmonic {
    stiffness: CsrMatrix,
    damping: CsrMatrix,
    mass: CsrMatrix,
    tr: Transformation,
    maps: Vec<Vec<usize>>,
    n: usize,
}

fn finite_nonneg(key: &'static str, v: f64) -> Result<()> {
    if !(v.is_finite() && v >= 0.0) {
        return Err(invalid(key, format!("{v:e} (must be finite and >= 0)")));
    }
    Ok(())
}

impl SuperelementModel {
    /// A host without parts. The host is checked as a part is (ADR 0469): valid, supported, no cyclic constraint, zero
    /// constraint values only.
    pub fn new(host: SolidModel) -> Result<SuperelementModel> {
        check_model(&host)?;
        let tr = transformation(&host, &[])?;
        let nodes = host.mesh().nodes();
        let mut extent = 0.0f64;
        for axis in 0..3 {
            let lo = nodes.iter().fold(f64::INFINITY, |a, p| a.min(p[axis]));
            let hi = nodes.iter().fold(f64::NEG_INFINITY, |a, p| a.max(p[axis]));
            if hi >= lo {
                extent = extent.max(hi - lo);
            }
        }
        let tolerance = MATCH_TOLERANCE * if extent > 0.0 { extent } else { 1.0 };
        Ok(SuperelementModel {
            host,
            tr,
            tolerance,
            parts: Vec::new(),
        })
    }

    /// Attaches `part` at the host nodes of its boundary positions; returns its index. Refused: a boundary node with no host
    /// node within the tolerance or with two, a component the host node lacks, a host unknown that is fixed or the dependent of
    /// a constraint, two boundary unknowns on one host unknown.
    pub fn add_part(&mut self, part: ReducedPart) -> Result<usize> {
        let nodes = self.host.mesh().nodes();
        let dofs = self.host.dof_map();
        let mut q_of = vec![usize::MAX; self.tr.n_unknowns()];
        for (a, &x) in self.tr.independent().iter().enumerate() {
            q_of[x] = a;
        }
        let p = self.parts.len();
        let mut map = Vec::with_capacity(part.n_reduced());
        let mut used = std::collections::HashSet::new();
        let mut last: Option<(usize, usize)> = None;
        for b in part.boundary() {
            let host_node = match last {
                Some((pn, hn)) if pn == b.node => hn,
                _ => {
                    let near: Vec<usize> = (0..nodes.len())
                        .filter(|&h| (0..3).all(|c| (nodes[h][c] - b.position[c]).abs() <= self.tolerance))
                        .collect();
                    if near.len() != 1 {
                        return Err(invalid(
                            "superelement",
                            format!(
                                "part {p}: node {} at {:?} has {} host nodes within {:e} (exactly one needed)",
                                b.node,
                                b.position,
                                near.len(),
                                self.tolerance
                            ),
                        ));
                    }
                    last = Some((b.node, near[0]));
                    near[0]
                }
            };
            let eq = dofs.equation(host_node, b.component).ok_or_else(|| {
                invalid(
                    "superelement",
                    format!(
                        "part {p}: component {} of node {} has no unknown on host node {host_node} ({} components)",
                        b.component,
                        b.node,
                        dofs.components(host_node)
                    ),
                )
            })?;
            let q = q_of[eq];
            if q == usize::MAX {
                return Err(invalid(
                    "superelement",
                    format!(
                        "part {p}: host node {host_node} component {} is fixed or the dependent of a constraint, not an \
                         independent unknown of the host",
                        b.component
                    ),
                ));
            }
            if !used.insert(q) {
                return Err(invalid(
                    "superelement",
                    format!(
                        "part {p}: two boundary unknowns on host node {host_node} component {}",
                        b.component
                    ),
                ));
            }
            map.push(q);
        }
        let offset = self.n_unknowns();
        map.extend(offset..offset + part.n_reduced() - part.n_boundary());
        self.parts.push(Attached { part, map });
        Ok(p)
    }

    /// The host model.
    pub fn host(&self) -> &SolidModel {
        &self.host
    }
    /// The host transformation.
    pub fn transformation(&self) -> &Transformation {
        &self.tr
    }
    /// The number of attached parts.
    pub fn n_parts(&self) -> usize {
        self.parts.len()
    }
    /// Part `p`.
    pub fn part(&self, p: usize) -> &ReducedPart {
        &self.parts[p].part
    }
    /// The host's independent unknowns `n_q` (the first combined unknowns).
    pub fn n_host_unknowns(&self) -> usize {
        self.tr.independent().len()
    }
    /// The number of combined unknowns.
    pub fn n_unknowns(&self) -> usize {
        self.n_host_unknowns()
            + self
                .parts
                .iter()
                .map(|a| a.part.n_reduced() - a.part.n_boundary())
                .sum::<usize>()
    }
    /// The first combined unknown of the generalised coordinates of part `p`.
    pub fn part_offset(&self, p: usize) -> usize {
        let a = &self.parts[p];
        a.map.get(a.part.n_boundary()).copied().unwrap_or_else(|| {
            self.n_host_unknowns()
                + self.parts[..p]
                    .iter()
                    .map(|a| a.part.n_reduced() - a.part.n_boundary())
                    .sum::<usize>()
        })
    }
    /// The combined index of every reduced unknown of part `p`.
    pub fn part_map(&self, p: usize) -> &[usize] {
        &self.parts[p].map
    }

    /// `K_q` (with the elastic supports), `M_q` and `f_q` of the host.
    fn host_matrices(&self, threads: Option<usize>) -> Result<(CsrMatrix, CsrMatrix, Vec<f64>)> {
        let mut k = stiffness_with_materials(&self.host, threads)?;
        add_supports(&mut k, &self.host)?;
        let m = consistent_with(&self.host, threads)?;
        let mut f = nodal_load_vector(&self.host, &[])?;
        f.resize(self.tr.n_unknowns(), 0.0);
        let (kq, fq) = self.tr.reduce(&k, &f)?;
        let zero = vec![0.0; self.tr.n_unknowns()];
        let (mq, _) = self.tr.reduce(&m, &zero)?;
        Ok((kq, mq, fq))
    }

    /// `w_h A_q (+) sum w_p S_p^T A_p S_p` for a host matrix and the parts' dense matrices of one kind.
    fn combine(
        &self,
        host: &CsrMatrix,
        host_weight: f64,
        part: impl Fn(usize, &ReducedPart) -> (&[f64], f64),
    ) -> CsrMatrix {
        let mut t = Vec::with_capacity(host.nnz());
        if host_weight != 0.0 {
            for r in 0..host.n {
                for k in host.row_ptr[r]..host.row_ptr[r + 1] {
                    t.push((r, host.col[k], host_weight * host.val[k]));
                }
            }
        }
        for (p, a) in self.parts.iter().enumerate() {
            let (mat, w) = part(p, &a.part);
            if w == 0.0 {
                continue;
            }
            let nr = a.part.n_reduced();
            for i in 0..nr {
                for j in 0..nr {
                    let v = mat[i * nr + j];
                    if v != 0.0 {
                        t.push((a.map[i], a.map[j], w * v));
                    }
                }
            }
        }
        CsrMatrix::from_triplets(self.n_unknowns(), t)
    }

    /// `f = f_q (+) sum S_p^T f_r`.
    fn combined_load(&self, fq: &[f64]) -> Vec<f64> {
        let mut f = vec![0.0; self.n_unknowns()];
        f[..fq.len()].copy_from_slice(fq);
        for a in &self.parts {
            for (i, v) in a.part.load().iter().enumerate() {
                f[a.map[i]] += v;
            }
        }
        f
    }

    /// The combined `K`, `M` and `f` (ADR 0536).
    pub fn matrices(&self, threads: Option<usize>) -> Result<SuperMatrices> {
        let (kq, mq, fq) = self.host_matrices(threads)?;
        Ok(SuperMatrices {
            stiffness: self.combine(&kq, 1.0, |_, p| (p.stiffness(), 1.0)),
            mass: self.combine(&mq, 1.0, |_, p| (p.mass(), 1.0)),
            load: self.combined_load(&fq),
        })
    }

    /// The static solution `K z = f` (envelope `L D L^T`, positive pivots).
    pub fn solve_static(&self, threads: Option<usize>) -> Result<SuperStatic> {
        let m = self.matrices(threads)?;
        let ldlt = SkylineLdlt::factor(&m.stiffness).map_err(|e| FemError::Solver {
            solver: "skyline-ldlt",
            reason: format!("combined model of {} parts: {e}", self.parts.len()),
        })?;
        let z = ldlt.solve(&m.load);
        let host_displacement = self.host_displacement(&z);
        Ok(SuperStatic {
            coordinates: z,
            host_displacement,
        })
    }

    /// The lowest `options.modes` modes of `(K, M)` by the subspace iteration (the options of the modal analysis on the
    /// transformation; shift `Shift::Auto` unless given).
    pub fn modal_analysis(&self, options: &ModalOptions) -> Result<SuperModes> {
        let m = self.matrices(options.threads)?;
        let n = m.stiffness.n;
        if options.modes == 0 || options.modes >= n {
            return Err(invalid(
                "modes",
                format!(
                    "{} modes requested of a combined model with {n} unknowns (at least 1 and fewer than the unknowns)",
                    options.modes
                ),
            ));
        }
        for (a, d) in m.mass.diag().iter().enumerate() {
            if d.is_nan() || *d <= 0.0 {
                return Err(invalid(
                    "density",
                    format!("combined unknown {a} has no mass (diagonal {d:e})"),
                ));
            }
        }
        let opts = SubspaceOptions {
            shift: options.shift.unwrap_or(Shift::Auto),
            tolerance: options.tolerance,
            residual_test: options.residual_test,
            max_iterations: options.max_iterations,
            subspace_size: options.subspace_size,
            factorisation: options.factorisation,
            ..SubspaceOptions::default()
        };
        let eig = lowest_eigenpairs(&m.stiffness, &m.mass, options.modes, &opts).map_err(|e| FemError::Solver {
            solver: "lowest_eigenpairs",
            reason: e.to_string(),
        })?;
        Ok(SuperModes {
            eigenvalues: eig.values,
            vectors: eig.vectors,
        })
    }

    /// The direct harmonic solver with hysteretic damping `D = eta_h K_q (+) sum eta_p S_p^T K_r S_p`.
    pub fn harmonic(&self, options: &SuperHarmonicOptions) -> Result<SuperHarmonic> {
        finite_nonneg("loss", options.loss)?;
        let mut eta = vec![options.loss; self.parts.len()];
        for &(p, e) in &options.part_loss {
            if p >= self.parts.len() {
                return Err(invalid("part_loss", format!("part {p} of {}", self.parts.len())));
            }
            finite_nonneg("part_loss", e)?;
            eta[p] = e;
        }
        let (kq, mq, _) = self.host_matrices(options.threads)?;
        Ok(SuperHarmonic {
            stiffness: self.combine(&kq, 1.0, |_, p| (p.stiffness(), 1.0)),
            damping: self.combine(&kq, options.loss, |i, p| (p.stiffness(), eta[i])),
            mass: self.combine(&mq, 1.0, |_, p| (p.mass(), 1.0)),
            tr: self.tr.clone(),
            maps: self.parts.iter().map(|a| a.map.clone()).collect(),
            n: self.n_unknowns(),
        })
    }

    /// The host's mesh displacements of combined unknowns `z`: `x = T q`, the mesh unknowns.
    pub fn host_displacement(&self, z: &[f64]) -> Vec<f64> {
        let x = self.tr.expand(&z[..self.n_host_unknowns()]);
        x[..self.tr.n_mesh()].to_vec()
    }

    /// The reduced coordinates `(u_b, eta, ..)` of part `p` in combined unknowns `z`.
    pub fn part_coordinates(&self, p: usize, z: &[f64]) -> Vec<f64> {
        self.parts[p].map.iter().map(|&i| z[i]).collect()
    }

    /// The static recovery of part `p` (its model `part_model`) from a static solution `z`: `u_i = Psi u_b + K_ii^{-1} f_i`
    /// (ADR 0469), the mesh displacements of the part.
    pub fn recover_part_static(&self, p: usize, part_model: &SolidModel, z: &[f64]) -> Result<Vec<f64>> {
        let c = self.part_coordinates(p, z);
        let part = &self.parts[p].part;
        part.recover_static(part_model, &c[..part.n_boundary()])
    }

    /// The mesh displacements of part `p` of combined unknowns `z` (a mode or a response part): `u_i = Psi u_b + Phi eta + ..`.
    pub fn expand_part(&self, p: usize, part_model: &SolidModel, z: &[f64]) -> Result<Vec<f64>> {
        self.parts[p].part.expand(part_model, &self.part_coordinates(p, z))
    }
}

impl SuperHarmonic {
    /// The combined stiffness `K`.
    pub fn stiffness(&self) -> &CsrMatrix {
        &self.stiffness
    }
    /// The damping `D`.
    pub fn damping(&self) -> &CsrMatrix {
        &self.damping
    }
    /// The combined mass `M`.
    pub fn mass(&self) -> &CsrMatrix {
        &self.mass
    }

    /// The complex load over the combined unknowns.
    fn load(&self, load: &SuperLoad) -> Result<(Vec<f64>, Vec<f64>)> {
        let mut re = vec![0.0; self.n];
        let mut im = vec![0.0; self.n];
        if let Some(h) = &load.host {
            let n_mesh = self.tr.n_mesh();
            if h.re().len() != n_mesh {
                return Err(FemError::FieldLength {
                    key: "host harmonic load",
                    expected: n_mesh,
                    found: h.re().len(),
                });
            }
            for (part, out) in [(h.re(), &mut re), (h.im(), &mut im)] {
                let mut v = part.to_vec();
                v.resize(self.tr.n_unknowns(), 0.0);
                let q = self.tr.transpose_times(&v);
                for (o, x) in out.iter_mut().zip(&q) {
                    *o += x;
                }
            }
        }
        for (p, l) in &load.parts {
            let map = self
                .maps
                .get(*p)
                .ok_or_else(|| invalid("part load", format!("part {p} of {}", self.maps.len())))?;
            if l.re().len() != map.len() {
                return Err(FemError::FieldLength {
                    key: "part harmonic load",
                    expected: map.len(),
                    found: l.re().len(),
                });
            }
            for (i, &z) in map.iter().enumerate() {
                re[z] += l.re()[i];
                im[z] += l.im()[i];
            }
        }
        Ok((re, im))
    }

    /// The response `(K + i D - omega^2 M) X = F` at `omega` (rad/s).
    pub fn response(&self, load: &SuperLoad, omega: f64) -> Result<SuperResponse> {
        finite_nonneg("omega", omega)?;
        let (fre, fim) = self.load(load)?;
        let w2 = omega * omega;
        let mut t = Vec::with_capacity(self.stiffness.nnz() + self.mass.nnz());
        for (a, s) in [(&self.stiffness, 1.0), (&self.mass, -w2)] {
            for r in 0..a.n {
                for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                    t.push((r, a.col[k], s * a.val[k]));
                }
            }
        }
        let re = CsrMatrix::from_triplets(self.n, t);
        let z = ComplexCsr::from_parts(&re, &self.damping).map_err(|e| FemError::Solver {
            solver: "complex-skyline-ldlt",
            reason: e.to_string(),
        })?;
        let f = ComplexSkylineLdlt::factor(&z).map_err(|e| FemError::Solver {
            solver: "complex-skyline-ldlt",
            reason: format!("omega {omega:e}: {e}"),
        })?;
        let b: Vec<C64> = fre.iter().zip(&fim).map(|(&r, &i)| C64::new(r, i)).collect();
        let x = f.solve(&b);
        Ok(SuperResponse {
            omega,
            re: x.iter().map(|c| c.re).collect(),
            im: x.iter().map(|c| c.im).collect(),
        })
    }
}
