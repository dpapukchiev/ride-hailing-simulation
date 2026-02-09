//! Parquet export: write simulation data to Parquet files for analysis.
//!
//! Provides functions to export:
//!
//! - Completed trips with full trip details
//! - All trips (including in-progress and cancelled)
//! - Time-series snapshot counts
//! - Agent position snapshots over time
//!
//! All exports use Arrow/Parquet format for efficient storage and compatibility
//! with data analysis tools (Pandas, Polars, etc.).

mod agent_positions;
mod completed_trips;
mod snapshot_counts;
mod trips;
mod utils;
mod validate;

pub use agent_positions::write_agent_positions_parquet;
pub use completed_trips::write_completed_trips_parquet;
pub use snapshot_counts::write_snapshot_counts_parquet;
pub use trips::write_trips_parquet;
pub use validate::validate_trip_timestamp_ordering;
