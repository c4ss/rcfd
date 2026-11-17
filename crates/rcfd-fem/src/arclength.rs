//! Arc-length continuation through limit points. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0480
//! (the registration of ADR 0327, F-424).
//!
//! [`trace`] follows the equilibrium path `f_int(u) + contributions(u) = lambda f` of the Newton driver of
//! [`crate::nonlinear`] with the load factor `lambda` as an unknown, under the spherical constraint of Crisfield (Comput.
//! Struct. 13 (1981) 55; the idea of the load factor as an unknown from Riks, Int. J. Solids Struct. 15 (1979) 529; both cited
//! from memory, ADR 0480 D4): `|Du|^2 + c Dlambda^2 = Ds^2` on the free equations, `Du, Dlambda` the increment from the last
//! converged point, `c` the load scale. The iteration is the driver's own attempt with a per-iteration correction of the
//! load factor (no second Newton loop): at iteration `k` the driver solves `K_T du_r = R` and `K_T du_f = f` with one
//! indefinite factor of the reduced tangent, and the correction `d lambda` is the root of the quadratic of the constraint
//! whose new increment is closer in angle to the reference (at `k = 0` the previous converged increment: the predictor; at
//! `k >= 1` the increment so far). A failed attempt halves `Ds`; after a converged point `Ds` doubles back towards the given
//! length. By default the load direction is that of the dead loads: a contribution's dependence on the load factor is in the
//! residual, not in `du_f`. With [`ArcLengthOptions::contribution_load`] (ADR 0560 D3) the load direction is `f - sum_c d
//! g_c / d lambda` at the iterate ([`crate::nonlinear::Contribution::add_load_factor_derivative`]), so that a reference
//! load of follower pressure only is traced; with [`crate::nonlinear::NewtonOptions::unsymmetric`] the correction factors
//! the full tangent by the unsymmetric envelope factor (ADR 0560 D4).
//!
//! Shell4 and Shell3 take the corotated element of ADR 0490 (ADR 0522, F-585), their rotation unknowns corrected
//! additively as every other unknown.
//!
//! [`trace_from`] (ADR 0726, F-797) is the opt-in start from a given converged state and increment ([`PathStart`]) with the
//! prescribed values of the model and of the list scaled by the load factor: the load direction of the correction is then
//! `f_F - K_Fp v` (`v` the prescribed values at load 1) and the prescribed equations follow the load factor after every
//! iteration ([`crate::nonlinear`]'s controlled iteration, ADR 0726 D1). [`trace`] is unchanged: it starts unloaded and
//! refuses non-zero prescribed values.

use crate::constraint::{check_rigid_body_motions, prescribed_equations, Prescribed};
use crate::element::ElementState;
use crate::error::{FemError, Result};
use crate::loads::{nodal_load_vector, NodalForceAt};
use crate::model::SolidModel;
use crate::nonlinear::{
    controlled_attempt, initial_states, load_direction, Contribution, LoadFactorControl, NewtonOptions,
};

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// Options of [`trace`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ArcLengthOptions {
    /// The arc length `Ds` of a point, in the norm of the constraint.
    pub length: f64,
    /// The load scale `c` of the constraint; `None`: `|K_0^-1 f|^2` from the first factorisation.
    pub load_scale: Option<f64>,
    /// The largest number of points.
    pub max_points: usize,
    /// Halvings of `Ds` allowed per point.
    pub max_halvings: usize,
    /// The iteration cap, the tolerances and the threads of each attempt (`steps` and `max_halvings` are not used).
    pub newton: NewtonOptions,
    /// The load direction with the contributions' derivatives with the load factor (ADR 0560 D3); off by default (the dead
    /// loads only).
    pub contribution_load: bool,
}

impl ArcLengthOptions {
    /// Points of arc length `length`, the default load scale, at most 200 points, 5 halvings, the driver's defaults.
    pub fn new(length: f64) -> ArcLengthOptions {
        ArcLengthOptions {
            length,
            load_scale: None,
            max_points: 200,
            max_halvings: 5,
            newton: NewtonOptions::new(1),
            contribution_load: false,
        }
    }
    /// The same options with a given load scale.
    pub fn load_scale(mut self, c: f64) -> ArcLengthOptions {
        self.load_scale = Some(c);
        self
    }
    /// The same options with another cap on the number of points.
    pub fn max_points(mut self, n: usize) -> ArcLengthOptions {
        self.max_points = n;
        self
    }
    /// The same options with other Newton options.
    pub fn newton(mut self, newton: NewtonOptions) -> ArcLengthOptions {
        self.newton = newton;
        self
    }
    /// The same options with the load direction of the contributions switched on or off (ADR 0560 D3).
    pub fn contribution_load(mut self, on: bool) -> ArcLengthOptions {
        self.contribution_load = on;
        self
    }

    fn check(&self) -> Result<()> {
        if !(self.length > 0.0 && self.length.is_finite()) {
            return Err(FemError::InvalidValue {
                key: "arc length",
                reason: format!("length {:e}: must be positive and finite", self.length),
            });
        }
        if let Some(c) = self.load_scale {
            if !(c > 0.0 && c.is_finite()) {
                return Err(FemError::InvalidValue {
                    key: "arc length",
                    reason: format!("load scale {c:e}: must be positive and finite"),
                });
            }
        }
        if self.max_points == 0 {
            return Err(FemError::InvalidValue {
                key: "arc length",
                reason: "0 points: at least 1".to_string(),
            });
        }
        // ADR 0542 D2: the controlled correction keeps the indefinite envelope factor and its inertia.
        if self.newton.factor != crate::solve::DirectFactor::Envelope {
            return Err(FemError::InvalidValue {
                key: "solver",
                reason: format!(
                    "{:?}: the arc-length correction factors by Envelope only (ADR 0542)",
                    self.newton.factor
                ),
            });
        }
        // ADR 0607 D1: the controlled correction is not given the line search.
        if self.newton.line_search {
            return Err(FemError::InvalidValue {
                key: "line_search",
                reason: "the arc-length correction is not given the line search (ADR 0607 D1)".to_string(),
            });
        }
        // ADR 0688 D3: nor the damping of the predictor.
        if self.newton.predictor_damping {
            return Err(FemError::InvalidValue {
                key: "predictor_damping",
                reason: "the arc-length correction is not given the damping of the predictor (ADR 0688 D3)".to_string(),
            });
        }
        // ADR 0694 D2, D3: nor the indefinite tangent (the correction factors it already) or the backtracking.
        if self.newton.indefinite_tangent {
            return Err(FemError::InvalidValue {
                key: "indefinite_tangent",
                reason: "the arc-length correction factors the indefinite tangent already (ADR 0694 D2)".to_string(),
            });
        }
        if self.newton.predictor_backtracking {
            return Err(FemError::InvalidValue {
                key: "predictor_backtracking",
                reason: "the arc-length correction is not given the backtracking of the predictor (ADR 0694 D3)"
                    .to_string(),
            });
        }
        // ADR 0719 D1: nor the retry of the load-stepping driver.
        self.newton.refuse_indefinite_retry("the arc-length trace")?;
        self.newton.check()
    }
}

/// One converged point of the path.
#[derive(Debug, Clone, PartialEq)]
pub struct PathPoint {
    /// The load factor.
    pub load_factor: f64,
    /// The displacement, one value per equation of the model's `DofMap`.
    pub displacement: Vec<f64>,
    /// The arc length `Ds` of this point (after its halvings).
    pub length: f64,
    /// Halvings of `Ds` before this point converged.
    pub halvings: usize,
    /// Negative pivots of the last factorisation of the tangent of the point's attempt.
    pub negative_pivots: usize,
    /// The relative residuals of the iterations, the last one at convergence.
    pub residuals: Vec<f64>,
    /// The energies of the corrections.
    pub energies: Vec<f64>,
}

/// The result of [`trace`].
#[derive(Debug, Clone, PartialEq)]
pub struct ArcLengthPath {
    /// The converged points in order (the unloaded start not included).
    pub points: Vec<PathPoint>,
    /// The load scale `c` used.
    pub load_scale: f64,
    /// The committed element states at the last point.
    pub states: Vec<ElementState>,
    /// Whether the stop criterion ended the trace (`false`: the cap on the points did).
    pub stopped: bool,
}

/// The given converged start of [`trace_from`] (ADR 0726 D2).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PathStart {
    /// The load factor of the start.
    pub load_factor: f64,
    /// The displacement, one value per equation of the model's `DofMap`; each prescribed equation must carry
    /// `load_factor` times its value at load 1.
    pub displacement: Vec<f64>,
    /// The committed element states, one per element.
    pub states: Vec<ElementState>,
    /// The increment `(Du, Dlambda)` that led to the start (`Du` one value per equation, only the free equations read): it
    /// selects the root of the first correction as the previous converged increment does; `None`: the root of the larger
    /// `Dlambda`, as from the unloaded start.
    pub increment: Option<(Vec<f64>, f64)>,
}

impl PathStart {
    /// The start at `load_factor` with `displacement` and the committed `states`, without an increment.
    pub fn new(load_factor: f64, displacement: Vec<f64>, states: Vec<ElementState>) -> PathStart {
        PathStart {
            load_factor,
            displacement,
            states,
            increment: None,
        }
    }
    /// The unloaded start of `model`: zero displacement, load factor 0, the initial element states.
    pub fn unloaded(model: &SolidModel) -> Result<PathStart> {
        Ok(PathStart::new(
            0.0,
            vec![0.0; model.dof_map().n_equations()],
            initial_states(model)?,
        ))
    }
    /// The same start with the increment `(du, dlambda)` that led to it.
    pub fn increment(mut self, du: Vec<f64>, dlambda: f64) -> PathStart {
        self.increment = Some((du, dlambda));
        self
    }
}

/// The spherical constraint of one attempt (ADR 0480 D4).
struct Spherical {
    ds: f64,
    c: Option<f64>,
    /// The increment of the attempt so far (free equations) and of the load factor.
    du: Vec<f64>,
    dl: f64,
    /// The load factor of the attempt (the same additions as the driver's).
    lambda: f64,
    /// The converged increment of the previous point.
    previous: Option<(Vec<f64>, f64)>,
    negative: usize,
    /// The load direction of the contributions (ADR 0560 D3).
    contribution_load: bool,
    /// The prescribed values scaled by the load factor with their load column (ADR 0726 D1).
    prescribed_load: bool,
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

impl LoadFactorControl for Spherical {
    fn correct(
        &mut self,
        k: usize,
        du_r: &[f64],
        du_f: &[f64],
        negative_pivots: usize,
    ) -> std::result::Result<f64, String> {
        self.negative = negative_pivots;
        if self.du.is_empty() {
            self.du = vec![0.0; du_r.len()];
        }
        let c = *self.c.get_or_insert_with(|| dot(du_f, du_f));
        if !(c > 0.0 && c.is_finite()) {
            return Err(format!("load scale {c:e}: the load gives no displacement"));
        }
        let b: Vec<f64> = self.du.iter().zip(du_r).map(|(x, y)| x + y).collect();
        let a1 = dot(du_f, du_f) + c;
        let a2 = 2.0 * (dot(du_f, &b) + c * self.dl);
        let a3 = dot(&b, &b) + c * self.dl * self.dl - self.ds * self.ds;
        let disc = a2 * a2 - 4.0 * a1 * a3;
        if disc.is_nan() || disc < 0.0 {
            return Err(format!(
                "the arc-length constraint has no real root at iteration {k} (discriminant {disc:e})"
            ));
        }
        let sq = disc.sqrt();
        // The two roots without cancellation.
        let q = -0.5 * (a2 + if a2 >= 0.0 { sq } else { -sq });
        let roots = if q != 0.0 { [q / a1, a3 / q] } else { [0.0, 0.0] };
        let (reference, rl): (&[f64], f64) = if k == 0 {
            match &self.previous {
                Some((p, pl)) => (p, *pl),
                None => (&[], 1.0),
            }
        } else {
            (&self.du, self.dl)
        };
        let score = |dl: f64| -> f64 {
            if reference.is_empty() {
                return dl;
            }
            let mut s = 0.0;
            for ((r, bi), fi) in reference.iter().zip(&b).zip(du_f) {
                s += r * (bi + dl * fi);
            }
            s + c * rl * (self.dl + dl)
        };
        let dl = if score(roots[0]) >= score(roots[1]) {
            roots[0]
        } else {
            roots[1]
        };
        for ((x, bi), fi) in self.du.iter_mut().zip(&b).zip(du_f) {
            *x = bi + dl * fi;
        }
        self.dl += dl;
        self.lambda += dl;
        Ok(dl)
    }
    fn contribution_load(&self) -> bool {
        self.contribution_load
    }
    fn prescribed_load(&self) -> bool {
        self.prescribed_load
    }
}

/// Follows the equilibrium path of `model` under the load factor times its loads and `forces` (ADR 0480 D4), from the
/// unloaded state, with the contributions at the current load factor, until `stop` returns `true` for a converged point or
/// `options.max_points` points have converged. Every prescribed value must be zero (key `"arc length"`), the load must be
/// non-zero on the free equations; the model refusals are those of [`crate::nonlinear::solve`]. A point whose attempts fail
/// `options.max_halvings + 1` times is an error (`FemError::Solver { solver: "arc-length" }`) naming the point and the cause.
pub fn trace(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    options: &ArcLengthOptions,
    contributions: &[&dyn Contribution],
    stop: &mut dyn FnMut(&PathPoint) -> bool,
) -> Result<ArcLengthPath> {
    model.validate()?;
    // ADR 0522 D4: Shell4 and Shell3 take the corotated element of ADR 0490.
    model.check_supported_nonlinear_shells()?;
    crate::nonlinear::check_driver_model(model)?;
    options.check()?;
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let f = nodal_load_vector(model, forces)?;
    let fixed = prescribed_equations(model, prescribed)?;
    if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
        let (node, component) = dofs.node_component(eq).expect("existing equation");
        return Err(FemError::InvalidValue {
            key: "arc length",
            reason: format!(
                "prescribed value {v:e} at node {node} component {component}: arc-length continuation takes zero values \
                 only (ADR 0480 D4)"
            ),
        });
    }
    check_rigid_body_motions(model, &fixed)?;
    let mut is_fixed = vec![false; n];
    for &(eq, _) in &fixed {
        is_fixed[eq] = true;
    }
    if options.contribution_load {
        // ADR 0560 D3: the load direction at the unloaded start, with the contributions.
        let f0 = load_direction(&f, contributions, &vec![0.0; n], 0.0).map_err(|reason| FemError::Solver {
            solver: "arc-length",
            reason,
        })?;
        if (0..n).all(|i| is_fixed[i] || f0[i] == 0.0) {
            return Err(FemError::InvalidValue {
                key: "arc length",
                reason: "the load and the contributions' load-factor derivatives are zero on the free equations: no \
                         load direction to follow (ADR 0560 D3)"
                    .to_string(),
            });
        }
    } else if (0..n).all(|i| is_fixed[i] || f[i] == 0.0) {
        return Err(FemError::InvalidValue {
            key: "arc length",
            reason: "the load is zero on the free equations: no load direction to follow (ADR 0480 D4)".to_string(),
        });
    }
    run(
        model,
        &f,
        &fixed,
        options,
        contributions,
        (vec![0.0; n], 0.0, initial_states(model)?),
        None,
        false,
        stop,
    )
}

/// The trace of [`trace`] from the given converged `start` (ADR 0726 D2), the prescribed values (the model's and
/// `prescribed`) being their values at load 1 scaled by the load factor, with their load column `f_F - K_Fp v` in the
/// controlled correction and the prescribed equations following the load factor after every iteration (ADR 0726 D1). The
/// options, points, halvings, stop and errors are those of [`trace`]. Refused, besides the refusals of [`trace`] other than
/// its refusal of non-zero prescribed values (key `"arc-length start"`): a displacement or increment of another length
/// (`FemError::FieldLength`), a non-finite start or increment, a number of element states other than the number of
/// elements, an increment zero on every free equation with `Dlambda = 0`, a prescribed equation whose start value is not
/// `load_factor` times its value; and (key `"arc length"`) a load zero on every free equation with every prescribed value
/// zero.
pub fn trace_from(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    options: &ArcLengthOptions,
    contributions: &[&dyn Contribution],
    start: &PathStart,
    stop: &mut dyn FnMut(&PathPoint) -> bool,
) -> Result<ArcLengthPath> {
    model.validate()?;
    // ADR 0522 D4: Shell4 and Shell3 take the corotated element of ADR 0490.
    model.check_supported_nonlinear_shells()?;
    crate::nonlinear::check_driver_model(model)?;
    options.check()?;
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let f = nodal_load_vector(model, forces)?;
    let fixed = prescribed_equations(model, prescribed)?;
    let key = "arc-length start";
    let lengths = std::iter::once(start.displacement.len()).chain(start.increment.iter().map(|(du, _)| du.len()));
    for found in lengths {
        if found != n {
            return Err(FemError::FieldLength {
                key,
                expected: n,
                found,
            });
        }
    }
    let invalid = |reason: String| FemError::InvalidValue { key, reason };
    let lambda0 = start.load_factor;
    if !lambda0.is_finite() || start.displacement.iter().any(|x| !x.is_finite()) {
        return Err(invalid(format!("the start (load factor {lambda0:e}) is not finite")));
    }
    let n_elements = model.mesh().n_elements();
    if start.states.len() != n_elements {
        return Err(invalid(format!(
            "{} element states for {n_elements} elements",
            start.states.len()
        )));
    }
    let mut is_fixed = vec![false; n];
    for &(eq, _) in &fixed {
        is_fixed[eq] = true;
    }
    let previous = match &start.increment {
        None => None,
        Some((du, dl)) => {
            if !dl.is_finite() || du.iter().any(|x| !x.is_finite()) {
                return Err(invalid("the start increment is not finite".to_string()));
            }
            let free: Vec<f64> = (0..n).filter(|&i| !is_fixed[i]).map(|i| du[i]).collect();
            if *dl == 0.0 && free.iter().all(|&x| x == 0.0) {
                return Err(invalid("the start increment is zero".to_string()));
            }
            Some((free, *dl))
        }
    };
    if let Some(&(eq, v)) = fixed.iter().find(|&&(eq, v)| start.displacement[eq] != lambda0 * v) {
        let (node, component) = dofs.node_component(eq).expect("existing equation");
        return Err(invalid(format!(
            "the start has {:e} at node {node} component {component}, prescribed {v:e} at load 1: it must carry the load \
             factor {lambda0:e} times it (ADR 0726 D2)",
            start.displacement[eq]
        )));
    }
    check_rigid_body_motions(model, &fixed)?;
    let any_prescribed = fixed.iter().any(|&(_, v)| v != 0.0);
    if options.contribution_load {
        // ADR 0560 D3: the load direction at the start, with the contributions.
        let f0 =
            load_direction(&f, contributions, &start.displacement, lambda0).map_err(|reason| FemError::Solver {
                solver: "arc-length",
                reason,
            })?;
        if !any_prescribed && (0..n).all(|i| is_fixed[i] || f0[i] == 0.0) {
            return Err(FemError::InvalidValue {
                key: "arc length",
                reason: "the load, the contributions' load-factor derivatives and every prescribed value are zero: no \
                         load direction to follow (ADR 0726 D2)"
                    .to_string(),
            });
        }
    } else if !any_prescribed && (0..n).all(|i| is_fixed[i] || f[i] == 0.0) {
        return Err(FemError::InvalidValue {
            key: "arc length",
            reason: "the load is zero on the free equations and every prescribed value is zero: no load direction to \
                     follow (ADR 0726 D2)"
                .to_string(),
        });
    }
    run(
        model,
        &f,
        &fixed,
        options,
        contributions,
        (start.displacement.clone(), lambda0, start.states.clone()),
        previous,
        true,
        stop,
    )
}

/// The loop of [`trace`] and [`trace_from`] from the converged `(u, lambda, states)` with the previous increment (on the
/// free equations, ascending) and the prescribed values scaled by the load factor or not (ADR 0726 D1).
#[allow(clippy::too_many_arguments)]
fn run(
    model: &SolidModel,
    f: &[f64],
    fixed: &[(usize, f64)],
    options: &ArcLengthOptions,
    contributions: &[&dyn Contribution],
    from: (Vec<f64>, f64, Vec<ElementState>),
    previous: Option<(Vec<f64>, f64)>,
    prescribed_load: bool,
    stop: &mut dyn FnMut(&PathPoint) -> bool,
) -> Result<ArcLengthPath> {
    let (mut u, mut lambda, mut states) = from;
    let mut previous = previous;
    let mut c = options.load_scale;
    let mut ds = options.length;
    let mut points = Vec::new();
    while points.len() < options.max_points {
        let mut halvings = 0;
        loop {
            let mut control = Spherical {
                ds,
                c,
                du: Vec::new(),
                dl: 0.0,
                lambda,
                previous: previous.clone(),
                negative: 0,
                contribution_load: options.contribution_load,
                prescribed_load,
            };
            match controlled_attempt(
                model,
                f,
                fixed,
                &options.newton,
                contributions,
                &u,
                lambda,
                &states,
                &mut control,
            ) {
                Ok(a) => {
                    c = control.c;
                    u = a.u;
                    states = a.states;
                    lambda = control.lambda;
                    previous = Some((control.du, control.dl));
                    let point = PathPoint {
                        load_factor: lambda,
                        displacement: u.clone(),
                        length: ds,
                        halvings,
                        negative_pivots: control.negative,
                        residuals: a.residuals,
                        energies: a.energies,
                    };
                    let done = stop(&point);
                    points.push(point);
                    if done {
                        return Ok(ArcLengthPath {
                            points,
                            load_scale: c.expect("set by the first attempt"),
                            states,
                            stopped: true,
                        });
                    }
                    ds = (2.0 * ds).min(options.length);
                    break;
                }
                Err(cause) => {
                    if control.c.is_some() {
                        c = control.c;
                    }
                    if halvings == options.max_halvings {
                        return Err(FemError::Solver {
                            solver: "arc-length",
                            reason: format!(
                                "point {} (load factor {lambda:e}, arc length {ds:e}) after {halvings} halvings: {cause}",
                                points.len() + 1
                            ),
                        });
                    }
                    halvings += 1;
                    ds *= 0.5;
                }
            }
        }
    }
    Ok(ArcLengthPath {
        points,
        load_scale: c.unwrap_or(0.0),
        states,
        stopped: false,
    })
}
