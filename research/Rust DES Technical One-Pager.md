Technical One-Pager: Bolt-Sim-Rust

Project Goal: To build a high-fidelity, high-concurrency Discrete Event Simulation (DES) using Rust to model the emergent marketplace dynamics of the Bolt ecosystem, specifically testing the impacts of commission rates, multi-service switching, and matching algorithms on platform liquidity.

1. System Architecture
The simulation utilizes an Entity Component System (ECS) to manage 50,000+ agents with high data locality and minimal memory overhead.

Core Engine: Powered by bevy_ecs or hecs, storing agent data (Location, State, Wallet) in contiguous arrays to maximize CPU cache hits.


Geospatial Indexing: Uses Uber H3 (Resolution 9) for discrete hexagonal grid-based movement and efficient k-ring driver lookups.


Simulation Clock: A Discrete Event basis managed via a BinaryHeap of events (e.g., RequestInbound, TripArrival) stored as an ECS Resource.


Concurrency: Employs rayon to parallelize multiple simulation iterations (Monte Carlo experiments) across CPU threads, rather than parallelizing individual agents.


2. Deliverable Timeline: The "Crawl-Walk-Run" Sequence
Phase
Task Name
Deliverable
Commit Requirement
Validation/Test Strategy
Crawl
ECS & H3 Skeleton
Basic world grid with static drivers/riders.



spatial.rs compiles with H3O integration.


Unit test: Verify grid_disk lookups return correct neighboring cell IDs.


Crawl
The Event Heap
A functional SimulationClock that executes events in order.



Event_Request_Inbound triggers a state change.


Unit test: Ensure clock fast-forwards correctly between sparse events.


Walk
Deterministic Core
A "Batch Match" engine using FCFS or basic bipartite logic.


Successful DriverID to RiderID pairing vector.


Integration test: Verify 1:1 matching without double-booking drivers.
Walk
Agent State Machine
Implementation of Idle, EnRoute, and OnTrip states.


Transition logic based on trip completion.
State-machine testing: Ensure drivers cannot transition from OffDuty to OnTrip.
Run
Behavioral Layers
Logit-based driver acceptance and multi-service switching.



P(accept) logic integrated into dispatch.


Property-based testing: Verify $P(\text{accept})$ decreases as PickupDist increases.


Run
Stochastic Shocks
Poisson request spawning and "Rain" weather events.


Config-driven event injections (e.g., exp_pricing_shock.json).


Telemetry validation: Check if "Rain" results in 15% speed drop in output logs.



3. Requirement Mapping
Logit Acceptance Model: Addressed in sim_model/src/agents/ using the formula $P(\text{accept}) = \frac{1}{1 + e^{-(\beta_0 + \beta_1 \cdot \text{Profit} + \dots)}}$.


Multi-Service Fluidity: Drivers evaluate food_surge vs. ride_hailing earnings to toggle availability.


Wild Goose Chase (WGC): Modeled by tracking ETA degradation as utilization approaches 100%.


Marketplace Regulation: Dynamic pricing and commission levers (10%–30%) are swappable via PricingStrategy traits.


Shadow Pricing: Advanced matching logic accounts for the opportunity cost of moving drivers between H3 clusters.


4. Success Metrics (KPIs)
The simulation will emit structured binary data (Parquet/CSV) to be processed by Polars for the following metrics:

Throughput: Total completed trips per hour.


Gini Coefficient: Measurement of earnings distribution (fairness) among driver agents.


Match Rate: Percentage of inbound requests resulting in a booking.


Platform Liquidity: Ratio of "Idle" time vs. "On-Trip" time for the fleet.


Driver Churn: Rate of agents going OffDuty based on commission sensitivity and fatigue.


Next Step: Would you like me to generate the initial Rust Cargo.toml workspace structure and the sim_core module for the H3 spatial indexing?
Bolt-Sim-Rust: Technical Master Spec
1. Mathematical Implementation (The Physics)
To prevent the agent from defaulting to simplistic logic, the following functions must be implemented exactly as defined:
Driver Acceptance Model (Logit): Drivers evaluate trips based on profit and distance.
$$P(\text{accept}) = \frac{1}{1 + e^{-(\beta_0 + \beta_1 \cdot \text{Profit} + \beta_2 \cdot \text{PickupDist} + \beta_3 \cdot \text{Surge})}}$$
Coefficients: $\beta_0 = -2.0$, $\beta_1 = 1.5$, $\beta_2 = -0.8$, $\beta_3 = 2.0$.
Rider Conversion Model: Riders decide to book based on price elasticity.
$$P(\text{book}) = \text{BaseConv} \times \left( \frac{\text{RefPrice}}{\text{ActualPrice}} \right)^{\epsilon}$$
Elasticity ($\epsilon$): $1.5$ for Economy, $0.6$ for Premium.
Gini Coefficient (Fairness): Used to measure wealth inequality across the driver fleet.
$$G = \frac{\sum_{i=1}^{n} \sum_{j=1}^{n} |x_i - x_j|}{2n^2\bar{x}}$$

2. Core Architecture Sequence
The simulation must follow an Entity Component System (ECS) pattern to manage high agent counts (50,000+).
Layer
Component
Implementation Detail
Spatial
H3O Integration
Use H3 Resolution 9 (~0.1km²). All movement is discrete "hops" between cell indices.
Temporal
BinaryHeap DES
The clock is a PriorityQueue of events. It only moves when an event occurs.
Logic
Batch Matcher
Every $N$ seconds, solve the Bipartite Matching Problem to minimize Global ETA.
Market
Service Switch
Drivers toggle to Food Delivery if food_surge > ride_surge + hassle_cost.


3. Strategy Traits (Code Boilerplate)
Define these traits in crates/sim_model/src/strategies.rs to allow for algorithmic hot-swapping.
Rust
pub trait MatchingStrategy: Send + Sync {
    /// Solves the bipartite matching problem for a batch of requests.
    fn solve_match(
        &self, 
        requests: &[&Request], 
        drivers: &[&Driver], 
        geo_index: &GeoIndex
    ) -> Vec<(u64, u64)>; // Returns (RiderID, DriverID)
}

pub trait PricingStrategy: Send + Sync {
    /// Calculates dynamic fare and surge based on H3 cell supply/demand.
    fn calculate_quote(
        &self, 
        request: &Request, 
        supply_demand_ratio: f64
    ) -> (f64, f64); // Returns (Fare, SurgeMultiplier)
}



4. Telemetry & Analysis
The simulation will not log raw text. It must emit structured SimResult snapshots to be processed by Polars for high-performance analysis.
KPIs to Track: Throughput (trips/hr), Matching Success Rate (%), Platform Liquidity (Idle vs. On-Trip), and Driver Churn Rate.
Data Sink: Export to .parquet or .csv for post-simulation validation in Python/Jupyter.


To establish a high-performance ECS (Entity Component System) architecture in Rust, we must first define the Domain Model. This ensures that our "Systems" (the logic) and "Components" (the data) align with the actual marketplace physics of Bolt.
In ECS, Entities are the "things" (Drivers, Riders, Hexagons), Components are their "states" or "attributes," and Systems are the "rules" that transition them between states.

1. System-Level States (The Marketplace Environment)
The "System" is the world itself. In DDD terms, this is our Bounded Context for the Marketplace.
State
Definition
Impact on Agents
Undersupplied
Demand > Supply. High surge multipliers active.
Riders: High churn/low conversion. Drivers: High $P(\text{accept})$.
Saturated
Supply > Demand. Low or base pricing.
Drivers: High idle time, multi-service switching (Food) triggers.
WGC Triggered
"Wild Goose Chase" state. Avg. ETA > 10 mins.
System: Aggregate throughput drops; supply-demand feedback loop breaks.
Shifting
High movement between H3 cells (e.g., morning rush).
System: Rebalancing algorithms (Shadow Pricing) prioritize positioning.


2. Driver Agent: Lifecycle & Component States
In ECS, a Driver is an Entity. Their behavior is determined by which State Component is currently attached to them.
State (Component)
Domain Logic
ECS System Responsibility
Idle
Driver is in an H3 cell waiting for a match.
System::SupplyDemand (Calculates local density).
Evaluating
Received a MatchOffer. Running Logit Model.
System::Decision (Applies $\beta$ coefficients).
EnRoute
Accepted a trip; moving toward Rider H3 cell.
System::Movement (H3 grid-step interpolation).
OnTrip
Rider is in the car; moving toward Destination.
System::Revenue (Accruing fare/commission).
OffDuty
Target earnings met or fatigue threshold hit.
System::Lifecycle (Despawn or disable entity).


3. Rider Agent: Lifecycle & Component States
Riders are often short-lived entities in the ECS, spawning on a Poisson distribution and despawning after a trip or a "rejection" (churn).
State (Component)
Domain Logic
ECS System Responsibility
Requesting
Spawned. Looking for a quote/ETA.
System::Spawning (Poisson Distribution).
Browsing
Evaluating the price/surge quote vs. Elasticity.
System::Conversion (Price sensitivity check).
Waiting
Trip booked. Monitoring Driver ETA.
System::Patience (Chance of cancellation if ETA > $T$).
InTransit
Linked to a Driver entity via ActiveTrip component.
System::Movement (Syncs Rider pos to Driver pos).


4. Designing the ECS Layout (Data-Oriented)
To make this "Cursor-ready," we structure the Rust code to maximize cache hits. Instead of an Agent class, we use SoA (Structure of Arrays).
Rust
// --- Components (The "What") ---

#[derive(Component)]
struct Position(u64); // H3 Cell Index

#[derive(Component)]
struct Wallet { balance: f64, daily_target: f64 }

#[derive(Component)]
struct DriverState::Idle; // Marker component for queries

// --- Systems (The "How") ---

/// This system only iterates over IDLE drivers, making it extremely fast.
fn matching_system(
    mut commands: Commands,
    drivers: Query<(Entity, &Position), With<DriverState::Idle>>,
    requests: Query<(Entity, &Position), With<RiderState::Requesting>>,
) {
    // Logic: Cross-reference H3 cells and solve bipartite matching
}

/// This system applies the Logit Physics we defined.
fn driver_decision_system(
    mut drivers: Query<(&mut Wallet, &LogitWeights, &mut DriverState)>,
) {
    // Logic: P(accept) = 1 / (1 + exp(-...))
}


Why this works for the Agent:
Separation of Concerns: The agent won't try to move a driver while they are "Evaluating" because the MovementSystem will only query for entities with an EnRoute or OnTrip component.
Predictable Transitions: States are switched by adding/removing components (cmd.entity(id).remove::<Idle>().insert(EnRoute)), which is the standard, high-performance way to handle state machines in Rust ECS.
To ensure the simulation remains a reliable "Source of Truth" for the marketplace, the Rust engine must enforce several hard invariants. These are the non-negotiable rules of the world—if any are violated, the simulation's validity is compromised.
I have categorized these into Spatial, Temporal, and Economic invariants to help guide the ECS System implementation.

1. Spatial Invariants (H3 Geospatial Integrity)
These ensure the physical world of the simulation remains consistent.
Singular Occupancy: An agent (Driver or Rider) can exist in exactly one H3 cell at any given timestamp. In ECS terms, an entity cannot have two Position components.
Adjacency Continuity: A driver cannot "teleport." Movement between $T_n$ and $T_{n+1}$ must be to the current H3 cell or a direct neighbor (unless a "High-Speed" travel event is explicitly calculated over a long time jump).
Resolution Uniformity: All spatial calculations (matching, surge, supply density) must occur at Resolution 9. Mixing resolutions without explicit projection is a terminal error.

2. Temporal Invariants (Discrete Event Integrity)
The BinaryHeap simulation clock must prevent "causality loops."
Monotonicity: The global simulation clock ($T_{global}$) can never move backward. An event scheduled for $T_{100}$ cannot be executed if the clock is already at $T_{101}$.
Event Atomicity: No two events can modify the same agent's state simultaneously. The ECS must resolve state transitions in a strictly linear fashion within the EventHeap.
Future-Dating: All new events generated during a system tick must be scheduled for $T > T_{current}$.

3. Agent & State Invariants (Domain Logic)
These define the "legal" life cycles of drivers and riders.
Exclusive State Occupancy: A driver cannot be Idle and OnTrip at the same time. In ECS, these must be mutually exclusive marker components.
The Match Lock: Once a driver is "Offered" a trip (entering the Evaluating state), they are removed from the Idle pool. They cannot receive another offer until they either reject the current one or complete the trip.
Rider-Driver Pairing (1:1): An ActiveTrip entity must link exactly one DriverID to exactly one RiderID. A driver cannot have two passengers, and a passenger cannot be in two cars (unless modeling "Bolt Pool," which requires a separate set of invariants).

4. Economic Invariants (The "Physics" of Money)
These prevent the simulation from generating "free" value or impossible financial states.
Conservation of Value: Every unit of currency deducted from a Rider's Wallet must be accounted for as:
$$\text{RiderPayment} = \text{DriverEarnings} + \text{PlatformCommission} + \text{Taxes/Fees}$$
Non-Negative Wallets: Unless "Credit" is an explicit feature, a Rider cannot initiate a Requesting state if their Wallet balance is less than the BaseFare.
Commission Ceiling: Platform commission cannot exceed 100% (the simulation should likely cap this at 50% to maintain behavioral realism).

5. Marketplace Physics (WGC & Liquidity)
Search Radius Cap: The matching engine cannot look beyond a maximum $K$-ring (e.g., $K=15$ at Res 9). If no driver is found, the request must fail rather than matching a driver 50km away.
The WGC Limit: As system-wide utilization ($U$) approaches 1.0, the average ETA must increase non-linearly. If $U=1.0$ and ETAs remain low, the matching logic is likely "teleporting" drivers.

Implementation Note for Cursor/LLM:
To enforce these, use Rust's Type System.
Instead of a String for state, use an Enum.
Instead of a f64 for H3 indices, use the h3o::CellIndex type.
Use debug_assert! inside the ECS Systems to check for Singular Occupancy and Value Conservation during every tick.
