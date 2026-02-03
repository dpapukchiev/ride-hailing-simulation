## Core Simulation

- Story: As a simulation runner, I can advance time by popping the next event so
  the simulation is deterministic and efficient.
  Status: Done

- Story: As a scenario author, I can spawn riders and drivers with randomized
  H3 positions and request times for reproducible runs. I can configure initial
  counts (spawned immediately at simulation start) separately from total counts
  (includes scheduled spawns over time).
  Status: Done

- Story: As a trip lifecycle engine, I can create trips, move drivers to pickup,
  start trips, and complete trips with timestamps for KPIs.
  Status: Done

- Story: As a scenario author, I can configure spawners with time-of-day and
  day-of-week distributions that vary spawn rates by hour (0-23) and day of week
  (Monday-Sunday), creating realistic demand patterns with rush hours (7-9 AM,
  5-7 PM) and day/night variations. The simulation epoch maps simulation time 0
  to a real-world datetime, enabling time-aware spawn rate calculations.
  Status: Done

- Story: As a simulation engine, I can use stochastic speed sampling (default
  20-60 km/h for city driving) with seeded RNG for reproducible movement times
  between H3 cells. Each movement step samples a new speed, creating realistic
  variability in trip durations.
  Status: Done

- Story: As a simulation runner, I can map simulation time to real-world datetime
  via an epoch (real-world ms corresponding to simulation time 0), enabling
  time-of-day aware distributions and datetime display in the UI.
  Status: Done

- Story: As a simulation operator, I can change the matching algorithm dynamically
  during simulation execution (even while running), and changes take effect
  immediately for new matching attempts.
  Status: Done

- Story: As a researcher, I can generate scenario permutations from a config
  manifest to run experiment batches.
  Status: Backlog
