//! Helpers for snapping spawn candidates to roads via OSRM's `/match` service.
//!
//! This module wraps a blocking HTTP client for OSRM and exposes a deterministic
//! selection strategy so the simulation can land riders and drivers on drivable
//! streets without leaking details of the HTTP response.

mod client;
mod error;
mod parser;
mod radius;
mod response;
mod selection;

pub use client::OsrmSpawnClient;
pub use error::OsrmSpawnError;
pub(crate) use radius::radiuses_for_attempt;
pub use response::{OsrmNearestMatch, OsrmSpawnMatch};

#[cfg(test)]
mod tests;
