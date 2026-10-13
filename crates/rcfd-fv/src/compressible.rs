//! Density-based explicit compressible Euler solver on polyhedral meshes:
//! cell-centred finite volumes, MUSCL reconstruction of primitive variables
//! with TVD limiters (Darwish-Moukalled r-ratio form), approximate Riemann
//! fluxes ([`RiemannSolver`]) and Shu-Osher SSP Runge-Kutta time stepping.
//!
//! References: Toro (2009); Shu and Osher, J. Comput. Phys. 77 (1988) for the
//! SSP-RK schemes; Darwish and Moukalled, Int. J. Heat Mass Transfer 46 (2003) 599-611 for
//! the unstructured TVD reconstruction.

use crate::convection::TvdLimiter;
use crate::geometry::FvGeometry;
pub use crate::riemann::{to_prim, Cons, Gas, Prim, RiemannSolver};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::PatchKind;

/// Boundary treatment of a patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompBc {
    /// Slip wall / symmetry plane (mirror ghost state).
    Slip,
    /// Zero-gradient outflow (ghost state equals the interior state; valid for
    /// supersonic outflow).
    Extrapolate,
    /// Fixed free-stream / inflow state (the Riemann problem against it is solved).
    Fixed(Prim),
}

/// Time integration scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RkScheme {
    /// Forward Euler (first order; for tests).
    Euler,
    /// Two-stage SSP-RK2 (Heun).
    Rk2,
    /// Three-stage SSP-RK3.
    Rk3,
}

/// Solver settings.
#[derive(Debug, Clone, Copy)]
pub struct CompSettings {
    /// Numerical flux.
    pub solver: RiemannSolver,
    /// Slope limiter; `None` gives first-order upwind-type fluxes.
    pub limiter: Option<TvdLimiter>,
    /// Time integrator.
    pub rk: RkScheme,
    /// CFL number.
    pub cfl: f64,
}

impl Default for CompSettings {
    fn default() -> Self {
        Self {
            solver: RiemannSolver::Hllc,
            limiter: Some(TvdLimiter::VanLeer),
            rk: RkScheme::Rk3,
            cfl: 0.4,
        }
    }
}

/// Explicit compressible solver state.
#[derive(Debug, Clone)]
pub struct CompressibleSolver {
    /// Mesh geometry.
    pub geom: FvGeometry,
    /// Gas model.
    pub gas: Gas,
    /// Settings.
    pub settings: CompSettings,
    /// Conserved variables per cell.
    pub u: Vec<Cons>,
    /// Boundary treatment per mesh patch (indexed like `mesh.patches()`).
    pub bcs: Vec<CompBc>,
    /// Inverse of the weighted least-squares moment matrix per cell.
    ls_inv: Vec<[[f64; 3]; 3]>,
    /// Physical time.
    pub time: f64,
    /// Number of steps taken.
    pub steps: usize,
}

impl CompressibleSolver {
    /// New solver with initial primitive state `init(x)`. Patches default to
    /// slip walls (empty patches are ignored).
    pub fn new(geom: FvGeometry, gas: Gas, settings: CompSettings, init: impl Fn(Vec3) -> Prim) -> Result<Self> {
        let cc = geom.mesh.cell_centres();
        let u: Vec<Cons> = cc.iter().map(|&x| init(x).to_cons(gas)).collect();
        for c in &u {
            let p = to_prim(gas, c);
            if !(p.rho > 0.0 && p.p > 0.0) {
                return Err(Error::Numerical(
                    "initial state has non-positive density or pressure".into(),
                ));
            }
        }
        let bcs = geom
            .mesh
            .patches()
            .iter()
            .map(|p| match p.kind {
                PatchKind::Outlet => CompBc::Extrapolate,
                _ => CompBc::Slip,
            })
            .collect();
        let ls_inv = ls_inverse(&geom);
        Ok(Self {
            geom,
            gas,
            settings,
            u,
            bcs,
            ls_inv,
            time: 0.0,
            steps: 0,
        })
    }

    /// Set the boundary treatment of a named patch.
    pub fn set_bc(&mut self, patch: &str, bc: CompBc) -> Result<()> {
        let i = self
            .geom
            .mesh
            .patches()
            .iter()
            .position(|p| p.name == patch)
            .ok_or_else(|| Error::PathNotFound(format!("patch {patch}")))?;
        self.bcs[i] = bc;
        Ok(())
    }

    /// Primitive state of every cell.
    pub fn primitives(&self) -> Vec<Prim> {
        self.u.iter().map(|c| to_prim(self.gas, c)).collect()
    }

    /// Largest stable explicit step for the current state (`cfl` times the
    /// cell-wise limit `V / (sum_f (|u.n| + a)|S| / 2)`).
    pub fn max_dt(&self) -> f64 {
        let mesh = &self.geom.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let prim = self.primitives();
        let mut sum = vec![0.0; mesh.n_cells()];
        for f in 0..mesh.n_faces() {
            if self.geom.empty[f] {
                continue;
            }
            let a = vec3::norm(sf[f]);
            let n = vec3::scale(sf[f], 1.0 / a);
            let lam = |p: &Prim| (vec3::dot(p.u, n).abs() + p.sound_speed(self.gas)) * a;
            sum[own[f]] += lam(&prim[own[f]]);
            if f < self.geom.n_internal {
                sum[nei[f]] += lam(&prim[nei[f]]);
            }
        }
        let vol = mesh.cell_volumes();
        (0..mesh.n_cells())
            .map(|c| 2.0 * vol[c] / sum[c])
            .fold(f64::INFINITY, f64::min)
            * self.settings.cfl
    }

    /// Ghost state across boundary face `f` of patch `pi`.
    fn ghost(&self, pi: usize, interior: &Prim, n: Vec3) -> Prim {
        match self.bcs[pi] {
            CompBc::Slip => interior.mirrored(n),
            CompBc::Extrapolate => *interior,
            CompBc::Fixed(p) => p,
        }
    }

    /// Weighted least-squares gradients of the five primitive variables.
    fn gradients(&self, prim: &[Prim], bface: &[Prim], var: &dyn Fn(&Prim, usize) -> f64) -> Vec<[Vec3; 5]> {
        let mesh = &self.geom.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let mut rhs = vec![[[0.0; 3]; 5]; mesh.n_cells()];
        let mut add = |c: usize, dr: Vec3, dphi: [f64; 5]| {
            let w = 1.0 / vec3::dot(dr, dr);
            for k in 0..5 {
                for i in 0..3 {
                    rhs[c][k][i] += w * dr[i] * dphi[k];
                }
            }
        };
        for f in 0..self.geom.n_internal {
            let (o, nb) = (own[f], nei[f]);
            let d = self.geom.delta[f];
            let dphi: [f64; 5] = std::array::from_fn(|k| var(&prim[nb], k) - var(&prim[o], k));
            add(o, d, dphi);
            add(nb, vec3::scale(d, -1.0), dphi.map(|x| -x));
        }
        for f in self.geom.n_internal..mesh.n_faces() {
            if !self.geom.empty[f] {
                let o = own[f];
                let b = &bface[f - self.geom.n_internal];
                add(
                    o,
                    self.geom.delta[f],
                    std::array::from_fn(|k| var(b, k) - var(&prim[o], k)),
                );
            }
        }
        rhs.iter()
            .zip(&self.ls_inv)
            .map(|(r, inv)| {
                std::array::from_fn(|k| [0, 1, 2].map(|i| (0..3).map(|j| inv[i][j] * r[k][j]).sum::<f64>()))
            })
            .collect()
    }

    /// Residual `-(1/V) sum_f F_f S_f` for the state `u`.
    fn residual(&self, u: &[Cons]) -> Result<Vec<Cons>> {
        let mesh = &self.geom.mesh;
        let n_cells = mesh.n_cells();
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let cf = mesh.face_centres();
        let cc = mesh.cell_centres();
        let n_int = self.geom.n_internal;
        let gas = self.gas;
        let prim: Vec<Prim> = u.iter().map(|c| to_prim(gas, c)).collect();
        if prim.iter().any(|p| !(p.rho > 0.0 && p.p > 0.0)) {
            return Err(Error::Numerical(
                "non-positive density or pressure in compressible solver".into(),
            ));
        }
        // boundary ghost states and face values
        let patch_of: Vec<usize> = {
            let mut v = vec![usize::MAX; mesh.n_faces()];
            for (pi, p) in mesh.patches().iter().enumerate() {
                for f in p.faces() {
                    v[f] = pi;
                }
            }
            v
        };
        let mut ghost = vec![prim[0]; mesh.n_faces() - n_int];
        let mut bface = vec![prim[0]; mesh.n_faces() - n_int];
        for f in n_int..mesh.n_faces() {
            if self.geom.empty[f] {
                continue;
            }
            let n = vec3::unit(sf[f]);
            let pi = patch_of[f];
            let g = self.ghost(pi, &prim[own[f]], n);
            ghost[f - n_int] = g;
            let pr = &prim[own[f]];
            bface[f - n_int] = match self.bcs[pi] {
                CompBc::Slip => Prim {
                    rho: pr.rho,
                    u: vec3::sub(pr.u, vec3::scale(n, vec3::dot(pr.u, n))),
                    p: pr.p,
                },
                CompBc::Extrapolate => *pr,
                CompBc::Fixed(p) => p,
            };
        }
        // reconstruction gradients of the 5 primitive variables
        let var = |p: &Prim, k: usize| -> f64 {
            match k {
                0 => p.rho,
                1 => p.u[0],
                2 => p.u[1],
                3 => p.u[2],
                _ => p.p,
            }
        };
        let grads: Option<Vec<[Vec3; 5]>> = self.settings.limiter.map(|_| self.gradients(&prim, &bface, &var));
        let mut res = vec![[0.0; 5]; n_cells];
        for f in 0..mesh.n_faces() {
            if self.geom.empty[f] {
                continue;
            }
            let a = vec3::norm(sf[f]);
            let n = vec3::scale(sf[f], 1.0 / a);
            let o = own[f];
            let flux = if f < n_int {
                let nb = nei[f];
                let (mut pl, mut pr) = (prim[o], prim[nb]);
                if let (Some(lim), Some(gr)) = (self.settings.limiter, grads.as_ref()) {
                    let d = self.geom.delta[f];
                    let _ = (cf, cc);
                    let mut left = [0.0; 5];
                    let mut right = [0.0; 5];
                    for k in 0..5 {
                        let (vp, vn) = (var(&prim[o], k), var(&prim[nb], k));
                        let dv = vn - vp;
                        let rp = 2.0 * vec3::dot(gr[o][k], d) / safe(dv) - 1.0;
                        let rn = 2.0 * vec3::dot(gr[nb][k], vec3::scale(d, -1.0)) / safe(-dv) - 1.0;
                        left[k] = vp + 0.5 * lim.psi(rp) * dv;
                        right[k] = vn - 0.5 * lim.psi(rn) * dv;
                    }
                    let l = Prim {
                        rho: left[0],
                        u: [left[1], left[2], left[3]],
                        p: left[4],
                    };
                    let r = Prim {
                        rho: right[0],
                        u: [right[1], right[2], right[3]],
                        p: right[4],
                    };
                    if l.rho > 0.0 && l.p > 0.0 && r.rho > 0.0 && r.p > 0.0 {
                        pl = l;
                        pr = r;
                    }
                }
                self.settings.solver.flux(gas, &pl, &pr, n)
            } else {
                self.settings.solver.flux(gas, &prim[o], &ghost[f - n_int], n)
            };
            for k in 0..5 {
                res[o][k] -= flux[k] * a;
                if f < n_int {
                    res[nei[f]][k] += flux[k] * a;
                }
            }
        }
        let vol = mesh.cell_volumes();
        for (r, v) in res.iter_mut().zip(vol) {
            for x in r.iter_mut() {
                *x /= v;
            }
        }
        Ok(res)
    }

    /// Advance one step of size `dt`.
    pub fn step(&mut self, dt: f64) -> Result<()> {
        let n = self.u.len();
        let axpy = |a: f64, x: &[Cons], b: f64, y: &[Cons], c: f64, r: &[Cons]| -> Vec<Cons> {
            (0..n)
                .map(|i| std::array::from_fn(|k| a * x[i][k] + b * y[i][k] + c * r[i][k]))
                .collect()
        };
        let u0 = self.u.clone();
        match self.settings.rk {
            RkScheme::Euler => {
                let r = self.residual(&u0)?;
                self.u = axpy(1.0, &u0, 0.0, &u0, dt, &r);
            }
            RkScheme::Rk2 => {
                let r0 = self.residual(&u0)?;
                let u1 = axpy(1.0, &u0, 0.0, &u0, dt, &r0);
                let r1 = self.residual(&u1)?;
                self.u = axpy(0.5, &u0, 0.5, &u1, 0.5 * dt, &r1);
            }
            RkScheme::Rk3 => {
                let r0 = self.residual(&u0)?;
                let u1 = axpy(1.0, &u0, 0.0, &u0, dt, &r0);
                let r1 = self.residual(&u1)?;
                let u2 = axpy(0.75, &u0, 0.25, &u1, 0.25 * dt, &r1);
                let r2 = self.residual(&u2)?;
                self.u = axpy(1.0 / 3.0, &u0, 2.0 / 3.0, &u2, 2.0 / 3.0 * dt, &r2);
            }
        }
        self.time += dt;
        self.steps += 1;
        Ok(())
    }

    /// Advance to physical time `t_end` with CFL-limited steps (last step clipped).
    pub fn run_to(&mut self, t_end: f64) -> Result<()> {
        while self.time < t_end - 1e-14 * t_end.abs().max(1.0) {
            let dt = self.max_dt().min(t_end - self.time);
            self.step(dt)?;
        }
        Ok(())
    }

    /// Advance with CFL-limited steps until the largest relative density
    /// change in a step falls below `tol` (steady state) or `max_steps` is
    /// reached. Returns `(steps, final_change)`.
    pub fn run_steady(&mut self, tol: f64, max_steps: usize) -> Result<(usize, f64)> {
        let mut change = f64::INFINITY;
        for i in 0..max_steps {
            let before: Vec<f64> = self.u.iter().map(|c| c[0]).collect();
            let dt = self.max_dt();
            self.step(dt)?;
            change = self
                .u
                .iter()
                .zip(&before)
                .map(|(c, b)| (c[0] - b).abs() / b)
                .fold(0.0, f64::max);
            if change < tol {
                return Ok((i + 1, change));
            }
        }
        Ok((max_steps, change))
    }

    /// Total conserved quantities `sum_c U_c V_c`.
    pub fn totals(&self) -> Cons {
        let vol = self.geom.mesh.cell_volumes();
        let mut t = [0.0; 5];
        for (c, v) in self.u.iter().zip(vol) {
            for k in 0..5 {
                t[k] += c[k] * v;
            }
        }
        t
    }
}

/// Inverse of the (masked) inverse-distance weighted least-squares matrix of each cell.
fn ls_inverse(geom: &FvGeometry) -> Vec<[[f64; 3]; 3]> {
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut mat = vec![[[0.0; 3]; 3]; mesh.n_cells()];
    let mut add = |c: usize, dr: Vec3| {
        let w = 1.0 / vec3::dot(dr, dr);
        for i in 0..3 {
            for j in 0..3 {
                mat[c][i][j] += w * dr[i] * dr[j];
            }
        }
    };
    for f in 0..geom.n_internal {
        add(own[f], geom.delta[f]);
        add(nei[f], geom.delta[f]);
    }
    for f in geom.n_internal..mesh.n_faces() {
        if !geom.empty[f] {
            add(own[f], geom.delta[f]);
        }
    }
    mat.into_iter()
        .map(|mut m| {
            for d in 0..3 {
                if !geom.active[d] {
                    for k in 0..3 {
                        m[d][k] = 0.0;
                        m[k][d] = 0.0;
                    }
                    m[d][d] = 1.0;
                }
            }
            let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
                - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
            if det.abs() < 1e-300 {
                return [[0.0; 3]; 3];
            }
            let mut inv = [[0.0; 3]; 3];
            for r in 0..3 {
                for c in 0..3 {
                    let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
                    let (c1, c2) = ((c + 1) % 3, (c + 2) % 3);
                    inv[r][c] = (m[c1][r1] * m[c2][r2] - m[c1][r2] * m[c2][r1]) / det;
                }
            }
            for d in 0..3 {
                if !geom.active[d] {
                    inv[d] = [0.0; 3];
                }
            }
            inv
        })
        .collect()
}

fn safe(x: f64) -> f64 {
    if x.abs() < 1e-300 {
        if x < 0.0 {
            -1e-300
        } else {
            1e-300
        }
    } else {
        x
    }
}

/// L1 norm `sum |a_c - b_c| V_c / sum V_c` of the difference between two cell fields.
pub fn l1_error(geom: &FvGeometry, a: &[f64], b: &[f64]) -> f64 {
    let vol = geom.mesh.cell_volumes();
    let num: f64 = a.iter().zip(b).zip(vol).map(|((x, y), v)| (x - y).abs() * v).sum();
    num / vol.iter().sum::<f64>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::riemann::{oblique_shock_beta, ExactRiemann};
    use rcfd_mesh::gen::box_2d;
    use std::sync::Arc;

    /// 1D shock tube on `[0,1]` with `n` cells (2D mesh, one cell tall; the
    /// y-walls act as slip walls and the front/back are empty).
    fn sod(n: usize, settings: CompSettings, t_end: f64) -> (CompressibleSolver, Vec<[f64; 3]>) {
        let mesh = Arc::new(box_2d(n, 1, 1.0, 1.0 / n as f64, false, false).unwrap());
        let geom = FvGeometry::new(mesh).unwrap();
        let gas = Gas::AIR;
        let left = Prim {
            rho: 1.0,
            u: [0.0; 3],
            p: 1.0,
        };
        let right = Prim {
            rho: 0.125,
            u: [0.0; 3],
            p: 0.1,
        };
        let mut s = CompressibleSolver::new(geom, gas, settings, |x| if x[0] < 0.5 { left } else { right }).unwrap();
        s.set_bc("xmin", CompBc::Fixed(left)).unwrap();
        s.set_bc("xmax", CompBc::Fixed(right)).unwrap();
        s.run_to(t_end).unwrap();
        let ex = ExactRiemann::new(gas, [1.0, 0.0, 1.0], [0.125, 0.0, 0.1]).unwrap();
        let exact: Vec<[f64; 3]> = s
            .geom
            .mesh
            .cell_centres()
            .iter()
            .map(|x| ex.sample((x[0] - 0.5) / t_end))
            .collect();
        (s, exact)
    }

    fn sod_errors(n: usize, settings: CompSettings) -> [f64; 3] {
        let (s, exact) = sod(n, settings, 0.2);
        let prim = s.primitives();
        let rho: Vec<f64> = prim.iter().map(|p| p.rho).collect();
        let u: Vec<f64> = prim.iter().map(|p| p.u[0]).collect();
        let p: Vec<f64> = prim.iter().map(|p| p.p).collect();
        let er: Vec<f64> = exact.iter().map(|e| e[0]).collect();
        let eu: Vec<f64> = exact.iter().map(|e| e[1]).collect();
        let ep: Vec<f64> = exact.iter().map(|e| e[2]).collect();
        [
            l1_error(&s.geom, &rho, &er),
            l1_error(&s.geom, &u, &eu),
            l1_error(&s.geom, &p, &ep),
        ]
    }

    #[test]
    fn sod_l1_convergence_hllc_muscl() {
        let st = CompSettings::default();
        let e: Vec<[f64; 3]> = [100, 200, 400].iter().map(|&n| sod_errors(n, st)).collect();
        for (n, e) in [100, 200, 400].iter().zip(&e) {
            println!(
                "Sod HLLC+vanLeer RK3 n={n}: L1(rho,u,p) = {:.4e} {:.4e} {:.4e}",
                e[0], e[1], e[2]
            );
        }
        let o1 = (e[0][0] / e[1][0]).log2();
        let o2 = (e[1][0] / e[2][0]).log2();
        println!("observed density L1 orders: {o1:.2} {o2:.2}");
        assert!(e[2][0] < 3.0e-3, "fine-grid density L1 {}", e[2][0]);
        assert!(o1 > 0.7 && o2 > 0.7, "orders {o1} {o2}");
    }

    #[test]
    fn sod_all_solvers_and_first_order() {
        let mut first = None;
        for (name, solver) in [
            ("rusanov", RiemannSolver::Rusanov),
            ("roe", RiemannSolver::Roe),
            ("hllc", RiemannSolver::Hllc),
            ("ausm+", RiemannSolver::AusmPlus),
        ] {
            let st = CompSettings {
                solver,
                ..CompSettings::default()
            };
            let e = sod_errors(200, st);
            println!("Sod n=200 {name} MUSCL: L1 rho = {:.4e}", e[0]);
            assert!(e[0] < 5.0e-3, "{name}: {}", e[0]);
            if solver == RiemannSolver::Hllc {
                first = Some(e[0]);
            }
        }
        let st1 = CompSettings {
            limiter: None,
            rk: RkScheme::Rk2,
            ..CompSettings::default()
        };
        let e1 = sod_errors(200, st1);
        println!("Sod n=200 hllc first order: L1 rho = {:.4e}", e1[0]);
        assert!(e1[0] > first.unwrap(), "MUSCL must beat first order");
    }

    /// Supersonic flow past a compression ramp: M = 2, theta = 10 degrees. The
    /// measured wave angle is compared with the theta-beta-M relation.
    fn wedge_run(nx: usize, ny: usize, settings: CompSettings, t_end: f64) -> (f64, f64, f64, f64) {
        use rcfd_mesh::gen::warp;
        let (lx, ly, x0, th) = (1.5, 1.0, 0.2, 10f64.to_radians());
        let base = box_2d(nx, ny, lx, ly, false, false).unwrap();
        let mesh = warp(&base, |p| {
            let rise = (p[0] - x0).max(0.0) * th.tan();
            [p[0], p[1] + rise * (1.0 - p[1] / ly), p[2]]
        })
        .unwrap();
        let geom = FvGeometry::new(Arc::new(mesh)).unwrap();
        let gas = Gas::AIR;
        let fs = Prim {
            rho: 1.0,
            u: [2.0, 0.0, 0.0], // a = sqrt(1.4 * (1/1.4) / 1) = 1
            p: 1.0 / 1.4,
        };
        let mut s = CompressibleSolver::new(geom, gas, settings, |_| fs).unwrap();
        s.set_bc("xmin", CompBc::Fixed(fs)).unwrap();
        s.set_bc("xmax", CompBc::Extrapolate).unwrap();
        s.set_bc("ymax", CompBc::Extrapolate).unwrap();
        s.set_bc("ymin", CompBc::Slip).unwrap();
        s.run_to(t_end).unwrap();
        // steadiness: one more unit of time changes the density by very little
        let before = s.u.iter().map(|c| c[0]).collect::<Vec<_>>();
        s.run_to(t_end + 0.5).unwrap();
        let drift =
            s.u.iter()
                .zip(&before)
                .map(|(c, b)| (c[0] - b).abs())
                .fold(0.0, f64::max);
        let prim = s.primitives();
        let cc = s.geom.mesh.cell_centres();
        let beta = oblique_shock_beta(1.4, 2.0, th).unwrap();
        let p2 = fs.p * (1.0 + 2.0 * 1.4 / 2.4 * (4.0 * beta.sin().powi(2) - 1.0));
        let mid = 0.5 * (fs.p + p2);
        // shock position per column: first crossing of the mid pressure scanning down from the top
        let mut cols: std::collections::BTreeMap<i64, Vec<(f64, f64)>> = Default::default();
        for (c, x) in cc.iter().enumerate() {
            cols.entry((x[0] * 1e9).round() as i64)
                .or_default()
                .push((x[1], prim[c].p));
        }
        let (mut sx, mut sy, mut sxx, mut sxy, mut n) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (k, mut col) in cols {
            let x = k as f64 * 1e-9;
            if !(0.5..=1.2).contains(&x) {
                continue;
            }
            col.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            for w in col.windows(2) {
                if w[0].1 < mid && w[1].1 >= mid {
                    let t = (mid - w[0].1) / (w[1].1 - w[0].1);
                    let y = w[0].0 + t * (w[1].0 - w[0].0);
                    sx += x;
                    sy += y;
                    sxx += x * x;
                    sxy += x * y;
                    n += 1.0;
                    break;
                }
            }
        }
        let slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
        let beta_meas = slope.atan();
        // post-shock plateau pressure sampled at x = 1.0 just above the wall
        let p_post = cc
            .iter()
            .enumerate()
            .filter(|(_, x)| (x[0] - 1.0).abs() < 0.5 * lx / nx as f64)
            .min_by(|a, b| a.1[1].partial_cmp(&b.1[1]).unwrap())
            .map(|(c, _)| prim[c].p)
            .unwrap();
        (beta_meas, beta, p_post / p2, drift)
    }

    #[test]
    #[ignore = "heavy: 200x70 cells, about 1 min in release"]
    fn oblique_shock_fine_grid() {
        let (bm, be, pr, drift) = wedge_run(200, 70, CompSettings::default(), 3.5);
        println!(
            "fine 200x70: beta {:.3} deg vs {:.3} deg, p2 ratio {pr:.4}, drift {drift:.1e}",
            bm.to_degrees(),
            be.to_degrees()
        );
        assert!((bm - be).abs() / be < 0.01 && (pr - 1.0).abs() < 0.02 && drift < 1e-4);
    }

    #[test]
    fn oblique_shock_angle_within_one_percent() {
        let (bm, be, pr, drift) = wedge_run(100, 35, CompSettings::default(), 3.0);
        println!(
            "wedge M=2 theta=10: beta measured {:.3} deg, theory {:.3} deg (err {:.3} %), p2/p2_theory {:.4}, steady drift {:.2e}",
            bm.to_degrees(),
            be.to_degrees(),
            100.0 * (bm - be).abs() / be,
            pr,
            drift
        );
        assert!(
            (bm - be).abs() / be < 0.01,
            "beta {} vs {}",
            bm.to_degrees(),
            be.to_degrees()
        );
        assert!((pr - 1.0).abs() < 0.02, "post-shock pressure ratio {pr}");
        assert!(drift < 1e-4, "not steady: {drift}");
    }

    #[test]
    fn sod_solution_is_monotone_and_conservative() {
        let (s, _) = sod(200, CompSettings::default(), 0.2);
        let prim = s.primitives();
        // total mass/energy: waves have not reached the ends, so totals are exactly the initial ones
        let vol = s.geom.mesh.cell_volumes()[0];
        let n = prim.len() as f64;
        let mass0 = (0.5 * 1.0 + 0.5 * 0.125) * n * vol;
        let e0 = (0.5 * 1.0 / 0.4 + 0.5 * 0.1 / 0.4) * n * vol;
        let t = s.totals();
        assert!((t[0] - mass0).abs() < 1e-12 * mass0, "mass {} vs {}", t[0], mass0);
        assert!((t[4] - e0).abs() < 1e-12 * e0, "energy");
        // overshoot of density/pressure beyond the initial extrema must be tiny
        let rmax = prim.iter().map(|p| p.rho).fold(0.0, f64::max);
        let rmin = prim.iter().map(|p| p.rho).fold(1e9, f64::min);
        assert!(rmax < 1.0 + 1e-3 && rmin > 0.125 - 1e-3, "rho range {rmin}..{rmax}");
        // transverse momentum stays zero
        assert!(s.u.iter().all(|c| c[2].abs() < 1e-13 && c[3].abs() < 1e-13));
    }
}
