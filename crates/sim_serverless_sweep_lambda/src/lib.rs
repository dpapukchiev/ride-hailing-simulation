//! AWS-oriented adapters and handlers for serverless sweep execution.
//!
//! This crate owns integration details (Lambda handlers, invoke clients, and
//! storage adapters). Domain validation and shard planning stay in
//! `sim_serverless_sweep_core`.
//! See `crates/sim_serverless_sweep_lambda/README.md` for ownership boundaries.

pub mod adapters;
pub mod handlers;
