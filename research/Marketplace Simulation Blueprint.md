Bolt-Sim: Simulated World Specification
This document outlines the architectural and behavioral logic of the Bolt-Sim environment. It describes how individual agents interact within a synthetic geospatial grid to form a complex, emergent marketplace.
1. The Synthetic Environment
The world is mapped using the Uber H3 Indexing system at Resolution 9 (cells approx. 0.1km²).
Spatial Topology: Instead of continuous coordinates, all movement is discrete hops between H3 cells.
Temporal Resolution: The simulation operates on a Discrete Event basis. The clock only moves when an event (e.g., a new request or a driver arriving) occurs.
2. Primary Agents & Behaviors
A. The Driver Agent (Supply)
Drivers are profit-maximizing autonomous entities. Their life cycle is a loop of evaluation and execution.
States: Idle, EnRoutePickup, OnTrip, and OffDuty.
Decision Logic (The "Bolt" Edge):
Acceptance Probability: Calculated using a Logit model. A driver is more likely to accept a ride if the net_profit (Fare - Commission - Fuel) is high and the pickup_distance is low.
Multi-Service Switching: Drivers can toggle between "Ride-Hailing" and "Food Delivery". If the surge in the food market exceeds ride-hailing earnings plus a "hassle cost" threshold, the agent switches services.
Session Fatigue: Every driver has a daily_target_earnings. Once met, the probability of them going OffDuty increases exponentially.
B. The Rider Agent (Demand)
Riders are utility-maximizing entities that appear based on geospatial demand heatmaps.
States: WaitingForQuote, Converted (Booking), or Abandoned.
Conversion Logic:
Upon requesting a ride, the rider receives a Quote (Price + Estimated ETA).
Price Elasticity: If the price exceeds their max_price_willingness, they abandon the app.
Patience Threshold: If the ETA is too high, the rider "churns" and tries a different mode of transport (or a competitor app).
3. Marketplace Components
The Matching Engine (The Orchestrator)
The engine solves the Bipartite Matching Problem. Every  seconds, it gathers all pending requests and idle drivers within a radius and optimizes for:
Global Minimum ETA: Minimizing the total wait time for all riders in the batch.
Supply Balancing: In advanced scenarios, it may prioritize matching drivers who are moving toward "High Demand" zones.
The Dynamic Pricing Engine (The Regulator)
This component monitors the Supply/Demand ratio within specific H3 clusters.
Surge Multiplier: If Demand > Supply, the multiplier increases to discourage low-value demand and attract "Off-Duty" or "Delivery" drivers back to the ride-hailing pool.
Commission Lever: A core "Bolt" variable. The simulation tests how varying the platform take-rate (e.g., 15% vs 25%) affects long-term driver retention and market liquidity.
4. The Role of Randomness (Stochastic Variables)
To ensure the simulation reflects the "messiness" of the real world, several factors are determined by Monte Carlo sampling:
Request Spawning: Inbound requests follow a Poisson Distribution tailored to the time of day.
Travel Latency: Actual travel time is the Calculated_Time * Traffic_Noise_Factor. This simulates unexpected congestion.
Agent Heterogeneity: No two drivers are the same. Each is assigned a random commission_sensitivity and fatigue_threshold at spawn.
Cancellations: Even after a match, there is a small random probability that a rider or driver cancels the trip.
5. Outcome Influence Factors
The success of a specific strategy (e.g., "Batch Matching") is influenced by these environmental "Shocks":
Weather Events: Rain reduces average speed and increases demand spikes.
Density Variation: Algorithms that work in high-density London may fail in lower-density suburban markets.
Competitor Presence: The simulation can model a "Ghost Competitor" that steals supply if the Bolt platform’s commission becomes too uncompetitive.
6. Key Performance Indicators (KPIs)
The simulation outputs the following metrics to "prove" algorithmic effectiveness:
Throughput: Total completed trips per hour.
Gini Coefficient: The distribution of earnings among drivers (measuring fairness).
Matching Success Rate: Percentage of requests that resulted in a completed trip.
Platform Liquidity: The average time a driver spends "Idle" vs. "On-Trip".
