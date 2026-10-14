# ADR 0005: Variable density in the pressure-based kernel

Status: accepted (design and pre-registered checks written before any validation run).

## Context

The SIMPLE/SIMPLEC and PISO/PIMPLE kernels (`rcfd-fv` `simple.rs`, `pimple.rs`, `pressure.rs`) are kinematic: the
pressure is `p/rho`, the face flux is volumetric, the viscosity is `nu`, and `energy.rs` adds a Boussinesq body force.
F-050 therefore stopped at the Boussinesq cavity, and F-061 (pressure-based compressible extension, choked flow) is
blocked. The density-based Euler solver (`compressible.rs`, F-060) is inviscid and explicit and cannot do natural
convection.

## Decision

### 1. A separate solver, the constant-density code is not touched

Variable density is a new module `rcfd-fv::varden` (solver) with its own pressure assembly. The bodies of
`SimpleSolver::iterate`, `solve_pressure`, `pressure_bc`, `corrected_flux`, `BuoyantSimple` and all PISO code are not
modified (not even refactored), so every existing case keeps its floating-point operation order. Evidence: a
fixed-iteration hash of `u, p, T, phi` bits for SIMPLE, SIMPLEC, coupled, PISO, heated cavity (steady and transient),
packed bed, rotating channel, Sod, wedge and k-epsilon channel is taken on the clean tree before any edit and compared
after every commit. Shared building blocks are only reused, not changed: `convection` (takes any flux, here the mass
flux), `laplacian` (per-face `gamma`), `face_interpolate`, `boundary_hby_a`, `FvMatrix`.

The case file selects the solver with `fluid.density = "constant" | "ideal_gas"` (default `constant`; every existing
file is unchanged). `physics.rho` stays the constant density of the old path; the new path uses `fluid.*`.

### 2. Equations (steady, laminar, SIMPLE/SIMPLEC only)

Unknowns: velocity `u`, pressure `p` (Pa), temperature `T`, density `rho`, mass flux `m_f = rho_f u_f . S_f` (kg/s).
Properties: `mu(T)` (dynamic), `k(T)`, constant `cp`, gas constant `R`, `gamma = cp / (cp - R)`.

* Momentum: `div(m u) - Sp(div m) u - div(mu grad u) = -grad p + f`, `f = (rho - rho_ref) g`. The `Sp(div m)` term
  (diagonal `-div m`) removes the continuity error of an unconverged mass flux, so the matrix stays an M-matrix.
  Same implicit relaxation, `H`, `HbyA = H/a_P`, `rAU = V/a_P` (SIMPLEC `rAtU = V/(a_P - H1)`) as the incompressible
  kernel; the body force enters like the old balanced-force `BodyForce` with `f_f . S` at faces and the wall pressure
  extrapolated with the gradient that balances `f` (fixed-flux condition), now in force-per-volume units.
* Pressure equation (mass): face volumetric flux `Q_f = (HbyA + rAU f)_f . S` (+ SIMPLEC correction),
  `m_f = rho_f Q_f - rho_f^lin rAtU_f |S| snGrad p`, `div m = 0`. `rho_f` is a deferred-correction interpolation
  (upwind implicit, `linear_upwind`/central explicit, `fluid.density_scheme`).
* Transonic (compressible mode only): `rho_f` in the first term is linearised in the face-upwinded pressure,
  `rho_up(p') = psi_up p'_up`, `psi = 1/(R T)`: with `phid_f = psi_up Q_f` the equation is
  `div(phid p') - div(rho rAtU grad p') = -div(m*) + div(phid p*_up)`, `m*_f = rho_f Q_f` (as in the transonic
  `rhoSimpleFoam` formulation; the converged `m` is `rho_f Q_f` regardless of the linearisation). The matrix is
  non-symmetric (BiCGStab/ILU through `FvMatrix::solve`). This gives the characteristic upwinding needed for M >= 1.
  Faces with prescribed normal velocity (walls, slip walls, velocity inlets) carry no `phid`.
* Equation of state: ideal gas, `rho = P/(R T)`; two modes (`fluid.mode`):
  * `compressible` (default): `P = p` the solved absolute pressure; energy in total-enthalpy form
    `div(m h0) - Sp(div m) h0 - div((k/cp) grad h0) = -div((k/cp) grad(|u|^2/2))`, `T = (h0 - |u|^2/2)/cp`
    (the diffusion of the kinetic energy is moved to the right side explicitly; viscous dissipation is not modelled).
    Isentropic inviscid flow keeps `h0` constant exactly.
  * `low_mach`: `P = P_th(t)` spatially uniform, `p` is the dynamic pressure and does not enter the EOS, so the
    pressure equation is the incompressible-like one (no `phid`); energy in temperature form
    `cp [div(m T) - Sp(div m) T] - div(k grad T) = S` (no pressure work or kinetic energy). Closed domain (no
    Dirichlet pressure boundary): `P_th = M R / int(1/T) dV` from the initial mass `M` (Le Quere et al. 2005, eq. 5);
    open domain: `P_th = fluid.reference_pressure`. The body force uses `rho_ref = M / V` (closed) or
    `rho(P_th, T_ref)` (open) so that the large hydrostatic part is removed and the O(h) wall imbalance of F-031/F-050
    only acts on the density perturbation. (Compressible mode keeps the full `rho g`.)
* Closed compressible domain: after the pressure solve the level is shifted so that `int rho dV = M` (the pinned
  `p_ref` only removes the singularity; its value does not set the mass).
* Under-relaxation: `alpha_u`, `alpha_p` as before, `fluid.relax_density` on `rho`, `energy.relax` on `h0`/`T`.
* Boundary conditions: velocity as before (fixed, zero gradient, symmetry; symmetry on inclined faces is recognised as
  a prescribed-zero-normal-flux face inside `varden`, the shared `pressure_bc` is not changed); pressure fixed /
  zero gradient; new velocity-inlet alternative `boundaries.<p>.inlet = "stagnation"` with `total_pressure_value` and
  `temperature_value` = total temperature (compressible mode, subsonic inflow): the velocity is zero-gradient, the
  static pressure follows the isentropic relation `p = p0 (1 + (gamma-1)/2 M^2)^(-gamma/(gamma-1))` from the owner
  cell Mach number each iteration, `h0 = cp T0`.
* Boundary density `rho_b = P_b/(R T_b)` from the boundary values of `p` (or `P_th`) and `T`.

### 3. Unsupported combinations (located diagnostics)

With `fluid.density = "ideal_gas"`: `piso`/`pimple`/`coupled`/`compressible` algorithms, turbulence, `rheology`,
`properties` (temperature laws are given as `fluid.viscosity`/`conductivity` laws instead), `scalars`/`species`,
`sources`, `udf` momentum/viscosity, `solver.bulk_velocity`, periodic meshes, `statistics`, `energy.enabled = false`
(the EOS needs `T`), `physics.body_force` (use `physics.gravity`), stagnation inlets in `low_mach` mode.
`fluid.*` keys with `density = "constant"` are errors.

### 4. Constant-density limit

With uniform `T` the ideal-gas density is constant, the new solver reduces to the kinematic equations scaled by
`rho`; check (d) compares it with the old kernel. The old path itself is bit-identical (hash comparison).

## Pre-registered validation checks (fixed before any validation run)

Air: `R = 287`, `gamma = 1.4`, `cp = gamma R/(gamma-1)`. Tolerances are stated now and are not changed after a run;
a miss is recorded as a miss in the roadmap with the measured numbers.

**(a1) Subsonic nozzle (F-061, first half).** 2-D planar converging-diverging duct, half-height
`h(x) = h_t + (h_in - h_t)(1 + cos(2 pi x/L))/2` (two cosine halves, throat at `L/2`), `h_in = h_e = 2 h_t`,
`h_t = 0.05 m`, `L = 40 h_t`, slip adiabatic walls, `mu = 1.8e-5`. Stagnation inlet `p0 = 101325`, `T0 = 300`; outlet
static pressure from the isentropic relation at `A_e/A* = 2 A_t/A*` with the throat Mach number 0.3
(`A_t/A* = 2.0351`), i.e. inlet and exit Mach about 0.145. 200 x 10 cells, `linear_upwind`. Compared with the
quasi-1-D area-Mach relations (inverted by bisection) at the 10 columns `i = 10, 30, ..., 190`, volume-weighted
column means: Mach within 1.5 %, `p/p0` within 0.5 %, `T/T0` within 0.3 %; total mass flow within 1.0 % of
`A* p0 sqrt(gamma/(R T0)) (2/(gamma+1))^((gamma+1)/(2(gamma-1)))`; inlet and outlet mass flow equal to 1e-6.
The maximum wall slope is `pi/40 = 0.08`, so 2-D effects are of order slope^2 < 1 %.

**(a2) Choked nozzle (F-061, second half).** Same family with `h_in = 2 h_t`, `h_e = 1.5 h_t` (exit area ratio 1.5,
supersonic exit `M_e = 1.85`), outlet static pressure equal to the isentropic value of that exit Mach number,
240 x 12 cells. Mass flow within 1.0 % of the choked-flow formula above with `A* = A_t`; the 10 column means
(`i = 12, 36, ..., 228`) of Mach within 3 % and of `p/p0` within 2 % of the area-Mach relation (subsonic branch before
the throat column, supersonic after it; the throat column itself is excluded). If this does not converge or is out of
tolerance F-061 is recorded as partial with the numbers; the roadmap criterion (choked mass flow within 1 %) is kept.

**(b) Hydrostatic ideal-gas column (compressible mode).** Closed column, 1 x 40 cells (`box_2d`), height
`H = 8000 m` (density ratio ~0.4), `g = 9.81`, uniform initial `p = 101325`, `T = 300` (isothermal, `T` fixed top and
bottom; case b1) and case b2: `T = 300` at the bottom, 240 at the top (steady conduction, `k` constant); mass equal
to the initial mass. Analytic: b1 `rho = rho_b exp(-g y/(R T))`; b2 `p = p_b (T/T_b)^(g/(R a))`, `a = 60/H`
(`p_b` from the mass by quadrature). Limits: maximum relative density error at cell centres 1e-3 (b1) and 2e-3 (b2),
maximum spurious speed 1e-6 m/s, mass conserved to 1e-10.

**(c) Differentially heated cavity with large temperature difference (low-Mach mode).** Le Quere, Weisman, Paillere,
Vierendeels, Dick, Becker, Braack and Locke, ESAIM: M2AN 39 (2005) 609-616, DOI 10.1051/m2an:2005027 (Table 1, values
verified from the author's postprint, see `docs/reference-data.md`): `T0 = 600 K`, `epsilon = 0.6`, `P0 = 101325 Pa`,
`Pr = 0.71`, `Ra = 1e6` (sets `L`), `Ra = Pr g^2 rho0^2 (Th-Tc) L^3/(T0 mu0^2)`, no-slip adiabatic top and bottom.
T1 constant properties `mu = mu0 = mu(T0)` of the Sutherland law below, `k = mu cp/Pr` (the Nusselt number depends
on `Ra`, `Pr`, `epsilon` only, the value of `mu0` just sets `L`); T2 Sutherland `mu = 1.68e-5 (T/273)^1.5 (273+110.5)/(T+110.5)`, `k = mu cp/Pr`. Mesh 96 x 96,
`box_2d` grading 8, `linear_upwind` momentum and energy, SIMPLEC. Nu on both walls (`L/(k0 dT) * mean(k dT/dx)`,
`k0 = k(T0)`) within 1.0 % of 8.85978 (T1) and 8.6866 (T2); `P/P0` within 0.3 % of 0.856338 (T1) and 0.924487 (T2); hot
and cold wall heat flow equal to 1e-6 relative.

**(d) Constant-density limit.** d1: hashes of the existing paths identical before/after (every commit). d2: 20 x 20
lid-driven cavity, Re = 100, `linear_upwind`, SIMPLEC, air at 293 K isothermal in `low_mach` mode with `nu = mu/rho =
0.01`: max |u - u_old kernel| / U_lid <= 1e-4 for both velocity components; the same in `compressible` mode (Mach
~3e-3) <= 1e-3. d3: Boussinesq limit, 24 x 24 graded 4 cavity, `Ra = 1e4`, `Pr = 0.71`, air at 300 K, `dT = 6 K`
(`epsilon = 0.01`, `beta = 1/T0`), `low_mach`: Nu within 1 % of the old Boussinesq kernel on the same mesh and settings.

## Consequences

* Variable density and the compressible extension live next to, not inside, the incompressible kernel; code between
  them is duplicated (about 300 lines of the SIMPLE iteration), which is the price of bit-identical old results.
* The first version is steady only. Transient variable-density PISO/PIMPLE, real-gas EOS (the EOS is a small
  function pair `rho(P, T)`, `psi(T)` so RK/PR from `rcfd-core::fluids` can follow), viscous heating, radiation and
  temperature-dependent `cp` are not included.
