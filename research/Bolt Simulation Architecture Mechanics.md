This is a comprehensive architectural feasibility analysis and technical specification for building a high-fidelity Discrete Event Simulation (DES) of the Bolt.eu marketplace.
1. Step-Back Prompting: The Physics of the Bolt Marketplace
Before architecting the simulator, we must characterize the unique physical and economic forces inherent to Bolt’s operating model, which differs slightly from US-centric competitors (Uber/Lyft).
The "Wild Goose Chase" (WGC) Threshold:
Concept: In high-density markets (e.g., London, Lagos), a critical tipping point exists. If utilization nears 100%, pickup times (ETAs) degrade non-linearly because the nearest driver is increasingly far away. This leads to a feedback loop: Longer pickups → Lower Driver Utilization (time driving empty) → Higher Prices → Lower Demand.
Bolt Specifics: Bolt operates heavily in highly price-sensitive markets (Eastern Europe, Africa). Their matching engine is likely tuned to avoid WGC by aggressively capping search radii or batching matches to maximize aggregate throughput rather than individual rider speed.
The "Frugal" Commission Lever:
Bolt publicly emphasizes a "low commission" model (often ~15-20% vs competitors' ~25%). In a simulation, this is not just a financial variable; it is a Driver Retention Force.
Simulation Implication: Driver agents must have a churn_probability function that is inversely correlated with take_rate. Lower commissions increase the "stickiness" of supply during demand troughs.
Multi-Modal Fluidity:
Unlike pure-play ride-hailing, Bolt drivers often toggle between "Rides" and "Food".
Simulation Implication: Supply is not fixed. It is a fluid resource pool that oscillates between two queues (people vs. food) based on earnings_per_hour heuristics.
2. Chain of Thought: Simulation Logic Design
We will design the logic flow for a single "tick" or event cycle in the simulation.
Step 1: State Definition (The "World")
The simulation cannot rely on simple arrays. We need a Spatial Index (QuadTree or H3 Hexagons) to query agents efficiently.
Global State:
Grid_Hex_ID: { active_drivers: [], active_requests: [], surge_multiplier: 1.2x }
Global_Queue: Priority Queue of events sorted by timestamp.
Agent State (Driver):
Status: IDLE | EN_ROUTE_PICKUP | ON_TRIP | SWITCHING_SERVICE
Wallet: accumulated_earnings, daily_target (Stopping condition).
Step 2: Discrete Event Taxonomy
We need specific events to model Bolt's operational complexity:
Event_Request_Inbound: Rider opens app. Logic: Check local supply → Calculate Surge → Rider decides (Conversion Model).
Event_Batch_Process: Every $N$ seconds (e.g., 5s), the system pauses to solve the bipartite matching problem for all pending requests in a geo-shard.
Event_Service_Switch: A driver evaluates food_demand vs ride_demand. If food_surge > ride_surge + switching_cost, the driver toggles availability.
Event_Shock_External: Inject a "Rain" event. Logic: Drop driver_speed by 15%, Increase rider_demand by 40%.
Step 3: Probabilistic Agent Logic
The "Bolt Driver" Agent:
Acceptance Probability: $P(accept) = \frac{1}{1 + e^{-(fare - \text{threshold})}}$.
Commission Sensitivity: Bolt drivers are modeled as "net-income maximizers." If a competitor lowers prices, the Bolt agent stays only if (fare * (1 - bolt_commission)) > (competitor_fare * (1 - comp_commission)).
Step 4: Algorithmic Interfacing
The Matching Engine should not be "First-Come-First-Served" (FCFS). It must be Global Optimization within a time window.
Use a Bipartite Matching algorithm (e.g., Kuhn-Munkres or Min-Cost Max-Flow) to minimize total fleet ETA, not just individual rider wait time.
3. Master Spec File: Research Findings
A. Bolt Marketplace Mechanics
Revenue Model: Hybrid Commission/Subscription.
Standard: % fee per ride.


Bolt Drive/Fleet: Fixed daily/weekly lease costs (depreciation).
Supply Dynamics:
Elasticity: High. Drivers in Bolt markets (e.g., Poland, South Africa) are often multi-app users.
Switching Cost: Low latency. A driver can drop a food delivery and immediately accept a ride passenger if the vehicle qualifies (cleaning/seats).
Demand Dynamics:
Price Sensitivity: High. Bolt users often price-check. Simulator must include a "Cross-App Comparison" probability where a Rider Agent abandons the queue if Price > Competitor_Price.
B. Simulation Stack Recommendations
Framework
Recommendation
Use Case
Rust (Custom)
Highest
Performance Critical. Rust’s memory safety and zero-cost abstractions allow for millions of agent interactions per second without Garbage Collection pauses (which kill simulations). Ideal for the core Matching Engine.
MATSim (Java)
Medium
Transport Fidelity. Excellent for traffic flow and multi-modal routing, but "heavy" for rapid iterative market-testing. Harder to customize for "Gig Economy" logic (dynamic pricing).
SimPy (Python)
Low
Prototyping Only. Good for logic validation, but Python's GIL will bottleneck at >5,000 concurrent agents.

C. Matching & Pricing Taxonomy
Batch Matching (Windowed Bipartite):
Accumulate requests for $t=5$ seconds. Build a cost matrix (Distance + Driver Value). Solve for max cardinality matching. Reduces "Wild Goose Chases."
Shadow Pricing (Dual Primal):
Instead of just "Supply/Demand" ratios, calculate the "Shadow Price" of a driver in a specific H3 hexagon. If a driver moves from Hex A to Hex B, how much system value is lost? Charge the rider that "opportunity cost."
Value-Based Dispatching:
Prioritize matching "High Retention Risk" drivers with "High Value" rides to prevent churn (Logic: if driver_churn_risk > 0.8: boost_match_score(driver)).
D. Agent Behavioral Matrix (JSON Schema)
This schema defines the configuration for a "Bolt-like" driver agent in the simulation.

JSON


{
  "agent_type": "driver_partner",
  "attributes": {
    "vehicle_class": ["bolt_lite", "green", "food_courier"],
    "multi_service_capable": true,
    "home_base_geo": "52.5200,13.4050"
  },
  "behavioral_parameters": {
    "commission_sensitivity": 0.85, 
    "earnings_target_daily": 150.00,
    "fatigue_threshold_hours": 10,
    "churn_probability_function": "logistic",
    "acceptance_logic": {
      "min_profitability_per_km": 0.45,
      "long_pickup_aversion": 1.5,
      "destination_filter_bias": 0.3
    }
  },
  "state_machine_triggers": {
    "switch_to_food_delivery": {
      "condition": "ride_demand_rolling_avg < 0.2 AND food_surge_multiplier > 1.3",
      "cooldown_minutes": 30
    },
    "go_offline": {
      "condition": "earnings >= daily_target OR shift_duration > fatigue_threshold"
    }
  }
}



