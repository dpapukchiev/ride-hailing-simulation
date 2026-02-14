//! Shared serverless sweep domain primitives.
//!
//! This crate owns deterministic orchestration behavior and request/response
//! contracts. It intentionally excludes AWS SDK and Lambda runtime concerns.
//! See `crates/sim_serverless_sweep_core/README.md` for ownership boundaries.

pub mod contract;
pub mod sharding;
pub mod storage_keys;
