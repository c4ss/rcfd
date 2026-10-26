//! Run service of the Studio (layer L2 of `docs/ui-restart-plan.md`, ADR 0123).
//!
//! One detached `rcfd-runner` process per project folder owns the runs: a queue in `runs/`, up to
//! `max_runs` concurrent `rcfd run` child processes under a thread and memory budget, pause, resume,
//! cancel and crash recovery. Clients ([`client::Client`]) and the runner talk through files only.

pub mod client;
pub mod records;
pub mod runner;
pub mod store;
pub mod sys;
