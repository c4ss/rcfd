//! Design-loop infrastructure (ADR 0067, wave 32C): operating-map automation, design-of-experiments samplers, gradient-free
//! optimisers, robust and Pareto objectives, a persistent resumable evaluation store and the service tools that expose them.
//!
//! The numerics (`rng`, `sampling`, `pareto`, `optim`) do not depend on the solver. `study` ties them to an
//! [`study::Evaluator`]; `case_eval` is the evaluator that runs rcfd cases; `opmap` is the chained operating-point driver;
//! `tools` holds the `design.*` service methods. `adjoint` gives shape sensitivities of laminar SIMPLE/SIMPLEC solutions with
//! respect to free-form-deformation parameters (ADR 0091) for the gradient method `optim::projected_gradient`.

pub mod adjoint;
pub mod case_eval;
pub mod fan_rom_eval;
pub mod opmap;
pub mod optim;
pub mod pareto;
pub mod rng;
pub mod sampling;
pub mod store;
pub mod study;
pub mod tools;
