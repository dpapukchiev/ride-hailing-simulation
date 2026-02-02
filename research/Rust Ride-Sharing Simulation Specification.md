This Initial System Specification (ISS) outlines the architecture for "Bolt-Sim-Rust", a high-fidelity Discrete Event Simulation (DES) designed to model marketplace dynamics.
1. Executive Architecture Strategy
To achieve high concurrency and data locality, we will utilize an Entity Component System (ECS) architecture rather than a pure Actor model.
Why ECS? In a marketplace with 50,000+ agents, an Actor model (each agent as a Tokio task) incurs significant memory overhead and context-switching costs. An ECS (using bevy_ecs or hecs) stores agent data in contiguous arrays (Columnar storage), maximizing CPU cache hits during bulk updates (e.g., "Move all drivers").
The DES Core: The "Simulation Clock" will not be a simple loop. It will be a Resource in the ECS containing a BinaryHeap of Events. Systems will query for the next event, fast-forward the global clock, and execute logic.
Parallelism: We will use rayon to parallelize the experiments (running 100 simulations of different cities simultaneously) rather than parallelizing the agents (which is handled by ECS internal scheduling).
2. Project Structure (Workspace Layout)
The repository will be structured as a Cargo Workspace to separate concerns between the core engine, business logic, and analysis tools.

Plaintext


bolt-sim-rust/
├── Cargo.toml                  # Workspace definition
├── configs/
│   ├── base_market.toml        # Static topology (London/Lagos coords)
│   └── exp_pricing_shock.json  # Permutation definitions
├── crates/
│   ├── sim_core/               # The "Physics" Engine (ECS, Time, H3)
│   │   ├── src/lib.rs
│   │   └── src/spatial.rs      # H3O integration
│   ├── sim_model/              # The "Bolt" Logic (Agents, Pricing, Matching)
│   │   ├── src/agents/         # Driver/Rider Component definitions
│   │   └── src/strategies/     # Traits for Matching/Pricing
│   ├── sim_runner/             # Orchestrator (Rayon + Config Generation)
│   └── sim_telemetry/          # Data Sinks (Parquet/CSV via Polars)
└── analysis/                   # Python/Jupyter notebooks for post-sim valid.


3. Configuration & Permutation Schema
We need a "Generator" system that explodes a configuration file into hundreds of distinct simulation runs. We will use a custom ScenarioManifest struct.
File: configs/exp_pricing_shock.json

JSON


{
  "experiment_name": "commission_sensitivity_analysis_v1",
  "base_config_path": "./configs/london_core.toml",
  "iterations_per_config": 50,
  "permutations": {
    "marketplace": {
      // The Generator will create a distinct SimConfig for every step in this range
      "commission_rate": { "start": 0.10, "end": 0.30, "step": 0.05 },
      "surge_dampening_factor": { "start": 0.5, "end": 1.0, "step": 0.1 }
    },
    "drivers": {
      "supply_count": { "values": [1000, 2000, 5000] }
    }
  }
}


Rust Implementation Strategy:
Use serde to deserialize this generic structure. A ConfigGenerator iterator will yield flattened SimConfig structs that are passed to the sim_runner.
4. Core Traits: The Strategy Pattern
To scientifically test "What-If" scenarios, we must decouple the mechanism (matching riders to drivers) from the policy (the algorithm used).

Rust


// crates/sim_model/src/strategies.rs

use crate::components::{Driver, Rider, Request};
use sim_core::spatial::GeoIndex;

/// The "Brain" of the simulation. Swappable at runtime.
pub trait MatchingStrategy: Send + Sync {
    /// Given a batch of requests and available drivers, return pairings.
    /// This runs inside the ECS system "batch_match_system".
    fn solve_match(
        &self, 
        requests: &[&Request], 
        drivers: &[&Driver], 
        geo_index: &GeoIndex
    ) -> Vec<(u64, u64)>; // Vector of (RiderID, DriverID)
}

pub trait PricingStrategy: Send + Sync {
    /// Calculates the fare and surge multiplier for a specific geospatial cell.
    fn calculate_quote(
        &self, 
        request: &Request, 
        supply_demand_ratio: f64
    ) -> (f64, f64); // (FarePrice, SurgeMultiplier)
}


5. Telemetry & The SimResult
We will not log text. We will emit structured binary data (or CSV) for high-throughput analysis.

Rust


// crates/sim_telemetry/src/lib.rs

#[derive(Debug, Serialize, Clone)] // derive Serde for easy CSV export
pub struct SimResult {
    // Metadata
    pub run_id: String,
    pub tick_count: u64,
    
    // Config Snapshot
    pub commission_rate: f32,
    pub strategy_name: String,

    // KPIs (The "Proof")
    pub avg_eta_seconds: f64,
    pub match_rate_percent: f64,
    pub driver_gini_coefficient: f64, // Wealth inequality metric
    pub platform_revenue: f64,
    pub driver_churn_rate: f64,
}


6. Mathematical Appendix: "Educated Assumptions"
Since we lack proprietary data, we define these "Physics laws" for the simulation to ensure agent behavior is statistically plausible.
A. Driver Acceptance Model (The Logit Function)
A driver does not accept every ride. They weigh Profit vs. Effort.

$$P(\text{accept}) = \frac{1}{1 + e^{-(\beta_0 + \beta_1 \cdot \text{Profit} + \beta_2 \cdot \text{PickupDist} + \beta_3 \cdot \text{Surge})}}$$
Implementation: In the DriverDecisionSystem, we calculate $P(\text{accept})$ and roll rand::random::<f64>().
Default Coefficients:
$\beta_0 = -2.0$ (Base reluctance)
$\beta_1 = 1.5$ (High sensitivity to profit/min)
$\beta_2 = -0.8$ (Strong aversion to long pickups > 3km)
$\beta_3 = 2.0$ (Strong attraction to surge badges)
B. Rider Conversion Model
Does the rider book the trip after seeing the price?

$$P(\text{book}) = \text{BaseConv} \times \left( \frac{\text{RefPrice}}{\text{ActualPrice}} \right)^{\epsilon}$$
$\epsilon$ (Price Elasticity): Set to $1.5$ for "Economy" rides (highly elastic) and $0.6$ for "Premium" (inelastic).
C. H3 Spatial Indexing Strategy
Resolution: We will use H3 Resolution 9 (~0.1 km² hexagons) for city-center precise matching.
K-Ring Lookups: To find drivers, we query h3o::grid_disk(origin, k=3) to scan neighboring cells efficiently.
7. Reasoning for Rust Patterns
Safety in Parallelism: By using rayon to parallelize entire simulation runs (SIMD - Single Instruction Multiple Data approach to experiments), we avoid the complexity of managing thread-safety within a single simulation. Each thread owns one World instance completely.
Data-Oriented Design (ECS): Agents in ride-sharing sims are mostly "Data Blobs" (Location, State, Wallet). ECS allows us to iterate query<(&mut Location, &Velocity)> linearly in memory, which is crucial when simulating 50,000 drivers at 100ms distinct time steps.
H3O & Integer Math: h3o uses u64 integers for cell indices. Comparisons and hash map lookups on u64 are orders of magnitude faster than geospatial point-in-polygon operations.
This video on Lightning Fast Data Analysis in Rust demonstrates how to utilize Polars for the high-performance telemetry analysis required in the final step of this architecture.

