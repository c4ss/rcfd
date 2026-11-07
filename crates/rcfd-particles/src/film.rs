//! Wall film fed by impinging parcels (F-253, ADR 0192).
//!
//! A thin liquid film lives on the faces of wall patches. Each face holds a film mass. The film moves along the wall with
//! the depth-averaged velocity of a steady laminar film of the face's own thickness under the tangential part of gravity
//! (lubrication closure, ADR 0192 M1, M2), and mass moves between faces by a donor-cell flux across their shared edges,
//! integrated by explicit Euler sub-steps under a Courant limit on the kinematic wave (M4 to M6). Mass that leaves through
//! an edge no other film face shares is booked as drained. Parcels that reach a film face are absorbed into it (M8).
//!
//! Two opt-in gas drives (F-292, ADR 0206), switched on by [`Film::set_gas`]: gas buoyancy drives the film by
//! `(rho_l - rho_g) g_t` in place of `rho_l g_t` (G1), and gas shear adds the Couette part `h tau / (2 mu_l)` of a surface
//! stress `tau` (G2), with `tau` the carrier's one-sided laminar wall stress at each film face (G4). Without them the film
//! is the film of ADR 0192, bit for bit.

use crate::{err, Result};
use rcfd_core::vec3::{add, cross, dot, norm, scale, sub, Vec3};
use rcfd_mesh::{Mesh, PatchKind};
use std::collections::HashMap;
use std::fmt::Write as _;

/// Courant number of the kinematic wave in one film sub-step (ADR 0192 M6).
pub const C_FILM: f64 = 0.5;

/// Depth-averaged velocity `rho h^2 g_t / (3 mu)` of a steady laminar film of thickness `h` under the tangential gravity
/// `g_t` (ADR 0192 M1, M2).
pub fn film_velocity(rho: f64, mu: f64, h: f64, g_t: Vec3) -> Vec3 {
    scale(g_t, rho * h * h / (3.0 * mu))
}

/// Nusselt thickness `(3 mu Gamma / (rho^2 |g_t|))^(1/3)` of a film carrying the mass flow `gamma` per unit width
/// (ADR 0192 M1).
pub fn nusselt_thickness(rho: f64, mu: f64, gamma: f64, g_t: f64) -> f64 {
    (3.0 * mu * gamma / (rho * rho * g_t)).cbrt()
}

/// Thickness `(3 mu Gamma / (rho_l (rho_l - rho_g) |g_t|))^(1/3)` of a film under gravity with gas buoyancy, carrying the
/// mass flow `gamma` per unit width (ADR 0206 G1).
pub fn buoyant_thickness(rho_l: f64, rho_g: f64, mu: f64, gamma: f64, g_t: f64) -> f64 {
    (3.0 * mu * gamma / (rho_l * (rho_l - rho_g) * g_t)).cbrt()
}

/// Depth-averaged velocity `h tau / (2 mu)` of a steady laminar film of thickness `h` under the surface shear stress `tau`
/// (the Couette part, ADR 0206 G2).
pub fn shear_film_velocity(mu: f64, h: f64, tau: Vec3) -> Vec3 {
    scale(tau, h / (2.0 * mu))
}

/// Thickness `(2 mu Gamma / (rho |tau|))^(1/2)` of a film driven by the surface shear `tau` alone, carrying the mass flow
/// `gamma` per unit width (ADR 0206 G2).
pub fn shear_thickness(rho: f64, mu: f64, gamma: f64, tau: f64) -> f64 {
    (2.0 * mu * gamma / (rho * tau)).sqrt()
}

/// The gas drives of the film (ADR 0206); both off (the default) is the film of ADR 0192.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FilmGas {
    /// Drive the film by `(rho_l - rho_g) g_t`, with `rho_g` the gas density of the step (G1).
    pub buoyancy: bool,
    /// Add `h tau / (2 mu_l)` to the film velocity, with `tau` the gas shear of each face (G2, G4).
    pub shear: bool,
}

/// What a film is made of and where it lies.
#[derive(Debug, Clone, PartialEq)]
pub struct FilmSpec {
    /// Wall patches that carry the film.
    pub patches: Vec<String>,
    /// Liquid density [kg/m^3] (the parcel density).
    pub density: f64,
    /// Liquid dynamic viscosity [Pa s].
    pub viscosity: f64,
    /// Gravity [m/s^2].
    pub gravity: Vec3,
}

/// An edge of the film: face `a` and, for an inner edge, face `b`, with the in-plane unit normals pointing out of each.
#[derive(Debug, Clone)]
struct Edge {
    a: usize,
    b: Option<usize>,
    len: f64,
    m_a: Vec3,
    m_b: Vec3,
}

/// The wall film.
#[derive(Debug, Clone)]
pub struct Film {
    names: Vec<String>,
    faces: Vec<usize>,
    patch: Vec<usize>,
    local: HashMap<usize, usize>,
    area: Vec<f64>,
    centre: Vec<Vec3>,
    normal: Vec<Vec3>,
    g_t: Vec<Vec3>,
    edges: Vec<Edge>,
    rho: f64,
    mu: f64,
    mass: Vec<f64>,
    drained: f64,
    received: f64,
    min_mass: f64,
    substeps: usize,
    /// Owner cell and normal distance from its centre of each face (for the gas shear, ADR 0206 G4).
    owner: Vec<usize>,
    dn: Vec<f64>,
    gas: FilmGas,
    rho_g: f64,
    tau: Vec<Vec3>,
    opposed: u64,
}

fn unit(v: Vec3) -> Vec3 {
    scale(v, 1.0 / norm(v))
}

impl Film {
    /// Build the film on the faces of the named wall patches of `mesh`. Refuses non-positive properties, an empty or
    /// repeated patch list, an unknown patch, a patch that is not a wall and an edge held by more than two film faces.
    pub fn new(mesh: &Mesh, spec: &FilmSpec) -> Result<Film> {
        if !(spec.density.is_finite() && spec.density > 0.0) {
            return err("wall film: the liquid density must be positive");
        }
        if !(spec.viscosity.is_finite() && spec.viscosity > 0.0) {
            return err("wall film: the liquid viscosity must be positive");
        }
        if !spec.gravity.iter().all(|x| x.is_finite()) {
            return err("wall film: gravity must be finite");
        }
        if spec.patches.is_empty() {
            return err("wall film: no patch given");
        }
        let mut faces = Vec::new();
        let mut patch = Vec::new();
        for (k, name) in spec.patches.iter().enumerate() {
            if spec.patches[..k].contains(name) {
                return err(format!("wall film: patch '{name}' is listed twice"));
            }
            let Some(p) = mesh.patch(name) else {
                return err(format!("wall film: unknown boundary patch '{name}'"));
            };
            if p.kind != PatchKind::Wall {
                return err(format!("wall film: patch '{name}' is not a wall"));
            }
            for f in p.faces() {
                faces.push(f);
                patch.push(k);
            }
        }
        if faces.is_empty() {
            return err("wall film: the patches have no faces");
        }
        let n = faces.len();
        let mut local = HashMap::with_capacity(n);
        let mut area = Vec::with_capacity(n);
        let mut centre = Vec::with_capacity(n);
        let mut normal = Vec::with_capacity(n);
        let mut g_t = Vec::with_capacity(n);
        let mut owner = Vec::with_capacity(n);
        let mut dn = Vec::with_capacity(n);
        for (k, &f) in faces.iter().enumerate() {
            local.insert(f, k);
            let nv = unit(mesh.face_area_vectors()[f]);
            area.push(mesh.face_area(f));
            centre.push(mesh.face_centres()[f]);
            normal.push(nv);
            g_t.push(sub(spec.gravity, scale(nv, dot(spec.gravity, nv))));
            // the normal distance of the owner centre as `FvGeometry::dn` computes it (ADR 0206 G4)
            let (s, c) = (mesh.face_area_vectors()[f], mesh.owner()[f]);
            owner.push(c);
            dn.push(dot(s, sub(mesh.face_centres()[f], mesh.cell_centres()[c])).abs() / norm(s));
        }
        // Edges in order of first appearance: (face, start point, end point) of each holder.
        let pts = mesh.points();
        let mut index: HashMap<(usize, usize), usize> = HashMap::new();
        let mut holders: Vec<Vec<(usize, usize, usize)>> = Vec::new();
        for (k, &f) in faces.iter().enumerate() {
            let ids = mesh.face_points(f);
            for i in 0..ids.len() {
                let (p, q) = (ids[i], ids[(i + 1) % ids.len()]);
                let key = (p.min(q), p.max(q));
                let e = *index.entry(key).or_insert_with(|| {
                    holders.push(Vec::new());
                    holders.len() - 1
                });
                holders[e].push((k, p, q));
            }
        }
        let out_normal = |k: usize, p: usize, q: usize| -> Vec3 {
            let e = sub(pts[q], pts[p]);
            let m = unit(cross(e, normal[k]));
            let mid = scale(rcfd_core::vec3::add(pts[p], pts[q]), 0.5);
            if dot(m, sub(mid, centre[k])) < 0.0 {
                scale(m, -1.0)
            } else {
                m
            }
        };
        let mut edges = Vec::with_capacity(holders.len());
        for h in &holders {
            let (a, p, q) = h[0];
            let len = norm(sub(pts[q], pts[p]));
            match h.len() {
                1 => edges.push(Edge {
                    a,
                    b: None,
                    len,
                    m_a: out_normal(a, p, q),
                    m_b: [0.0; 3],
                }),
                2 if h[1].0 != a => {
                    let (b, pb, qb) = h[1];
                    edges.push(Edge {
                        a,
                        b: Some(b),
                        len,
                        m_a: out_normal(a, p, q),
                        m_b: out_normal(b, pb, qb),
                    })
                }
                _ => {
                    return err(format!(
                        "wall film: the edge between points {p} and {q} is held by {} film face sides (more than two faces, or one face twice)",
                        h.len()
                    ))
                }
            }
        }
        Ok(Film {
            names: spec.patches.clone(),
            faces,
            patch,
            local,
            area,
            centre,
            normal,
            g_t,
            edges,
            rho: spec.density,
            mu: spec.viscosity,
            mass: vec![0.0; n],
            drained: 0.0,
            received: 0.0,
            min_mass: 0.0,
            substeps: 0,
            owner,
            dn,
            gas: FilmGas::default(),
            rho_g: 0.0,
            tau: vec![[0.0; 3]; n],
            opposed: 0,
        })
    }

    /// Switch the gas drives (ADR 0206); [`FilmGas::default`] is the film of ADR 0192.
    pub fn set_gas(&mut self, gas: FilmGas) {
        self.gas = gas;
    }

    /// The gas drives.
    pub fn gas(&self) -> FilmGas {
        self.gas
    }

    /// Set the gas density `rho_g` and, with the shear on, the shear `tau` of every film face (one vector per face, in
    /// face order; ignored with the shear off). Refuses a density that is not finite or is negative, a density not below
    /// the liquid's with the buoyancy on, and with the shear on a list of the wrong length or a value that is not finite.
    pub fn set_gas_state(&mut self, rho_g: f64, tau: &[Vec3]) -> Result<()> {
        if !(rho_g.is_finite() && rho_g >= 0.0) {
            return err("wall film: the gas density must be finite and not negative");
        }
        if self.gas.buoyancy && rho_g >= self.rho {
            return err(format!(
                "wall film: gas buoyancy needs a gas density below the liquid density ({rho_g} against {})",
                self.rho
            ));
        }
        if self.gas.shear {
            if tau.len() != self.len() {
                return err(format!(
                    "wall film: {} gas shear vectors for {} film faces",
                    tau.len(),
                    self.len()
                ));
            }
            if !tau.iter().all(|t| t.iter().all(|x| x.is_finite())) {
                return err("wall film: the gas shear must be finite");
            }
            self.tau.copy_from_slice(tau);
        }
        self.rho_g = rho_g;
        Ok(())
    }

    /// The carrier's wall shear on every film face, `mu_g (u_P - (u_P . n) n) / d` with `u_P` the velocity of the owner
    /// cell and `d` the normal distance of its centre from the face (ADR 0206 G4: the one-sided laminar stress of a
    /// no-slip wall at rest). `u` has one velocity per mesh cell.
    pub fn carrier_shear(&self, u: &[Vec3], mu_g: f64) -> Vec<Vec3> {
        (0..self.len())
            .map(|k| {
                let n = self.normal[k];
                let v = scale(u[self.owner[k]], mu_g / self.dn[k]);
                sub(v, scale(n, dot(v, n)))
            })
            .collect()
    }

    /// Gas density of the last [`Film::set_gas_state`] [kg/m^3] (0 before any).
    pub fn gas_density(&self) -> f64 {
        self.rho_g
    }

    /// Gas shear of film face `k` [Pa] (zero with the shear off).
    pub fn shear(&self, k: usize) -> Vec3 {
        self.tau[k]
    }

    /// Largest face gas shear [Pa].
    pub fn max_shear(&self) -> f64 {
        self.tau.iter().map(|t| norm(*t)).fold(0.0, f64::max)
    }

    /// Face sub-steps so far in which the gravity and shear parts of the velocity had a negative dot product (a
    /// counter-current film, which ADR 0206 G5 does not deliver).
    pub fn opposed(&self) -> u64 {
        self.opposed
    }

    /// Gravity part and shear part of the velocity of face `k` with the gas drives (ADR 0206 G1, G2).
    fn velocity_parts(&self, k: usize) -> (Vec3, Vec3) {
        let h = self.thickness(k);
        let drive = if self.gas.buoyancy {
            self.rho - self.rho_g
        } else {
            self.rho
        };
        let g = film_velocity(drive, self.mu, h, self.g_t[k]);
        let s = if self.gas.shear {
            shear_film_velocity(self.mu, h, self.tau[k])
        } else {
            [0.0; 3]
        };
        (g, s)
    }

    /// Number of film faces.
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    /// True when the film has no faces (never, once built).
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Global mesh face of each film face.
    pub fn faces(&self) -> &[usize] {
        &self.faces
    }

    /// Film face of a global mesh face, if it is one.
    pub fn local(&self, face: usize) -> Option<usize> {
        self.local.get(&face).copied()
    }

    /// Patch name of film face `k`.
    pub fn patch_name(&self, k: usize) -> &str {
        &self.names[self.patch[k]]
    }

    /// Area of film face `k` [m^2].
    pub fn area(&self, k: usize) -> f64 {
        self.area[k]
    }

    /// Centre of film face `k`.
    pub fn centre(&self, k: usize) -> Vec3 {
        self.centre[k]
    }

    /// Unit normal of film face `k` (out of the fluid domain).
    pub fn normal(&self, k: usize) -> Vec3 {
        self.normal[k]
    }

    /// Tangential gravity of film face `k`.
    pub fn g_t(&self, k: usize) -> Vec3 {
        self.g_t[k]
    }

    /// Film mass of each face [kg].
    pub fn masses(&self) -> &[f64] {
        &self.mass
    }

    /// Film thickness of face `k` [m].
    pub fn thickness(&self, k: usize) -> f64 {
        self.mass[k] / (self.rho * self.area[k])
    }

    /// Depth-averaged film velocity of face `k` [m/s]: ADR 0192 M2, with the gas drives of ADR 0206 G1 and G2 when set.
    pub fn velocity(&self, k: usize) -> Vec3 {
        if self.gas == FilmGas::default() {
            return film_velocity(self.rho, self.mu, self.thickness(k), self.g_t[k]);
        }
        let (g, s) = self.velocity_parts(k);
        add(g, s)
    }

    /// Film Reynolds number `4 Gamma / mu` of face `k`, with `Gamma = rho h |u|` (ADR 0192 M9).
    pub fn reynolds(&self, k: usize) -> f64 {
        4.0 * self.rho * self.thickness(k) * norm(self.velocity(k)) / self.mu
    }

    /// Largest face thickness [m].
    pub fn max_thickness(&self) -> f64 {
        (0..self.len()).map(|k| self.thickness(k)).fold(0.0, f64::max)
    }

    /// Largest face Reynolds number.
    pub fn max_reynolds(&self) -> f64 {
        (0..self.len()).map(|k| self.reynolds(k)).fold(0.0, f64::max)
    }

    /// Mass on the wall [kg].
    pub fn held(&self) -> f64 {
        self.mass.iter().sum()
    }

    /// Mass that left the film through its open edges [kg].
    pub fn drained(&self) -> f64 {
        self.drained
    }

    /// Mass the film received (absorbed parcels and [`Film::add_mass`]) [kg].
    pub fn received(&self) -> f64 {
        self.received
    }

    /// Smallest face mass after any sub-step so far [kg].
    pub fn min_mass(&self) -> f64 {
        self.min_mass
    }

    /// Sub-steps taken so far.
    pub fn substeps(&self) -> usize {
        self.substeps
    }

    /// Liquid density [kg/m^3].
    pub fn density(&self) -> f64 {
        self.rho
    }

    /// Liquid viscosity [Pa s].
    pub fn viscosity(&self) -> f64 {
        self.mu
    }

    /// Add mass `m` to film face `k` (a source; counted as received).
    pub fn add_mass(&mut self, k: usize, m: f64) {
        self.mass[k] += m;
        self.received += m;
    }

    /// Absorb the mass `m` of a parcel that reached the global mesh face `face`; false when it is not a film face.
    pub(crate) fn absorb(&mut self, face: usize, m: f64) -> bool {
        match self.local(face) {
            Some(k) => {
                self.add_mass(k, m);
                true
            }
            None => false,
        }
    }

    /// Advance the film over `dt` (ADR 0192 M5, M6).
    pub fn advance(&mut self, dt: f64) {
        if dt.is_nan() || dt <= 0.0 {
            return;
        }
        let n = self.len();
        let mut u = vec![[0.0; 3]; n];
        let mut rate = vec![0.0; n];
        let mut coef = vec![[0.0; 2]; self.edges.len()];
        let mut delta = vec![0.0; n];
        let mut rem = dt;
        while rem > 0.0 {
            for (k, uk) in u.iter_mut().enumerate() {
                *uk = self.velocity(k);
            }
            if self.gas.shear {
                // ADR 0206 G5: count the faces whose gravity and shear parts oppose (not delivered, reported)
                let opposed = (0..n)
                    .filter(|&k| {
                        let (g, s) = self.velocity_parts(k);
                        dot(g, s) < 0.0
                    })
                    .count();
                self.opposed += opposed as u64;
            }
            rate.iter_mut().for_each(|r| *r = 0.0);
            for (e, c) in self.edges.iter().zip(coef.iter_mut()) {
                c[0] = e.len * dot(u[e.a], e.m_a).max(0.0) / self.area[e.a];
                rate[e.a] += c[0];
                if let Some(b) = e.b {
                    c[1] = e.len * dot(u[b], e.m_b).max(0.0) / self.area[b];
                    rate[b] += c[1];
                }
            }
            let rmax = rate.iter().copied().fold(0.0, f64::max);
            let mut h = if rmax > 0.0 {
                (C_FILM / (3.0 * rmax)).min(rem)
            } else {
                rem
            };
            if rem - h <= 1e-12 * rem {
                h = rem;
            }
            delta.iter_mut().for_each(|d| *d = 0.0);
            for (e, c) in self.edges.iter().zip(coef.iter()) {
                let ta = self.mass[e.a] * c[0] * h;
                delta[e.a] -= ta;
                match e.b {
                    Some(b) => {
                        delta[b] += ta;
                        let tb = self.mass[b] * c[1] * h;
                        delta[b] -= tb;
                        delta[e.a] += tb;
                    }
                    None => self.drained += ta,
                }
            }
            for (m, d) in self.mass.iter_mut().zip(&delta) {
                *m += d;
                self.min_mass = self.min_mass.min(*m);
            }
            self.substeps += 1;
            rem -= h;
        }
    }

    /// One row per film face: `face,patch,x,y,z,area,thickness,mass,ux,uy,uz` (shortest round-trip numbers), with the gas
    /// shear on also `tx,ty,tz` (ADR 0206 S1).
    pub fn csv(&self) -> String {
        let mut s = String::from("face,patch,x,y,z,area,thickness,mass,ux,uy,uz");
        s.push_str(if self.gas.shear { ",tx,ty,tz\n" } else { "\n" });
        for k in 0..self.len() {
            let (c, u) = (self.centre[k], self.velocity(k));
            let _ = write!(
                s,
                "{},{},{},{},{},{},{},{},{},{},{}",
                self.faces[k],
                self.patch_name(k),
                c[0],
                c[1],
                c[2],
                self.area[k],
                self.thickness(k),
                self.mass[k],
                u[0],
                u[1],
                u[2]
            );
            if self.gas.shear {
                let t = self.tau[k];
                let _ = write!(s, ",{},{},{}", t[0], t[1], t[2]);
            }
            s.push('\n');
        }
        s
    }
}
