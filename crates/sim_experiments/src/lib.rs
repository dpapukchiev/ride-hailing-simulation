//! Parallel experimentation framework for ride-hailing simulation parameter sweeps.
//!
//! This crate enables running multiple simulations in parallel with varying parameters,
//! extracting comprehensive metrics, and calculating marketplace health scores to analyze
//! how pricing changes and supply/demand balance affect marketplace outcomes.
//!
//! # Quick Start
//!
//! ```no_run
//! use sim_experiments::{ParameterSpace, run_parallel_experiments, HealthWeights, find_best_result_index};
//!
//! // Define parameter space (grid search)
//! let space = ParameterSpace::grid()
//!     .commission_rate(vec![0.0, 0.1, 0.2, 0.3])
//!     .num_drivers(vec![50, 100, 150])
//!     .num_riders(vec![300, 500, 700]);
//!
//! // Generate parameter sets
//! let parameter_sets = space.generate();
//!
//! // Run experiments in parallel
//! let results = run_parallel_experiments(parameter_sets, None);
//!
//! // Calculate health scores and find best result
//! let weights = HealthWeights::default();
//! let best_idx = find_best_result_index(&results, &weights).unwrap();
//! ```
//!
//! # Architecture
//!
//! The crate is organized into several modules:
//!
//! - [`parameters`]: Parameter variation framework (grid search, random sampling)
//! - [`runner`]: Parallel simulation execution using rayon
//! - [`metrics`]: Metrics extraction from simulation results
//! - [`health`]: Marketplace health score calculation
//! - [`export`]: Result export to Parquet/JSON
//!
//! # Scaling to Multiple Machines
//!
//! See the [README.md](../README.md) for information on scaling experiments across
//! multiple machines using a coordinator/worker architecture.

pub mod export;
pub mod health;
pub mod metrics;
pub mod parameter_spaces;
pub mod parameters;
pub mod runner;

pub use export::{export_to_csv, export_to_json, export_to_parquet, find_best_parameters, find_best_result_index};
pub use health::{calculate_health_scores, HealthWeights};
pub use metrics::SimulationResult;
pub use parameters::{ParameterSet, ParameterSpace};
pub use runner::run_parallel_experiments;
