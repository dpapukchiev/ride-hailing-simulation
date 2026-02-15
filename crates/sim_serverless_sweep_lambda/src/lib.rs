//! AWS-oriented adapters and handlers for serverless sweep execution.
//!
//! This crate owns runtime integration details (Lambda handlers, queue dispatch,
//! and storage adapters) and exposes a single runtime module boundary for
//! contract, sharding, and storage key primitives.
//! See `crates/sim_serverless_sweep_lambda/README.md` for ownership boundaries.

pub mod adapters;
pub mod handlers;
pub mod runtime;
