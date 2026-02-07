# UI

## `sim_ui`

Native UI that runs the scenario in-process (`cargo run -p sim_ui`),
renders riders/drivers on a map with icons and state-based colors, and charts for
active trips, completed trips, waiting riders, idle drivers, cancelled riders, abandoned (quote), and cancelled trips. The UI starts paused, allows
scenario parameter edits before start, shows sim/wall-clock datetimes, overlays
a metric grid for scale, and includes a live trip table with all trips (all states) showing pickup distance at
driver acceptance (km), pickup-to-dropoff distance (km), and full timestamp columns (requested, matched, started, completed, cancelled),
with timestamps shown as simulation datetimes sorted by last updated time (descending, most recent first). Controls include
**Start** button (enabled only before simulation starts), **Step** button (advances 1 event), **Step 100** button (advances 100 events),
**Run/Pause** toggle (auto-advances simulation at configured clock speed), **Run to end** button (runs until event queue is empty or simulation end time is reached),
and **Reset** button (resets simulation with current parameters). Match radius, trip length, and map size inputs are
configured in kilometers and converted to H3 cell distances (resolution 9, ~0.24 km per cell);
the map size defines the scenario bounds used for spawning and destination sampling, so it is
only editable before the simulation starts, and the grid overlay adapts to the map size. Rider
cancellation wait windows (min/max minutes) are configurable before start.
**Simulation start time** is configurable via year, month, day, hour, and minute inputs (UTC);
defaults to 2026-02-03 06:30:00 UTC but can be set to any datetime via inputs or a **"Now"** button that sets it to current wall-clock time.
This start time is used as the simulation epoch, affecting the time-of-day patterns applied to spawn rates (rush hours, day/night variations).
A real-time clock speed selector (10x, 20x, 50x, 100x, 200x, 400x, 1000x, 2000x) controls simulation playback speed. Riders in `InTransit` state
are hidden from the map (they are with the driver). Drivers in `OnTrip` state display "D(R)" instead
of "D" to indicate they have a rider on board. The UI differentiates between riders waiting for a
match (yellow/orange) and riders waiting for pickup (darker orange/red) based on whether `matched_driver`
is set, making it easy to see which riders have a driver assigned and are waiting for pickup versus
those still searching for a match. **Driver earnings and fatigue information** can be displayed on driver
labels in compact format: `D[50/200][3/8h]` shows earnings (current/target) and fatigue (current hours/max hours).
A toggle checkbox "Driver stats (earnings/fatigue)" controls whether this information is displayed; when disabled,
drivers show only "D" or "D(R)" without the earnings and fatigue brackets. The font size is 8.5pt monospace
for compact display. **Matching algorithm** can be changed at any time (even while simulation is running) via a dropdown
selector; changes take effect immediately for new matching attempts (riders already waiting continue with their current
matching attempts, but new `TryMatch` events will use the updated algorithm). The metrics chart includes an **Abandoned (quote)** series for riders who gave up after rejecting too many quotes. The Run outcomes section displays breakdowns of abandonment reasons (price too high, ETA too long, stochastic rejection) and pickup cancellation reasons (timeout) with counts and percentages.

## Collapsible Sections

The UI is organized into collapsible sections:

- **Scenario parameters**: Organized in an eight-column layout:
  - **Supply (Drivers)**: Initial count, spawn count, spread (hours)
  - **Demand (Riders)**: Initial count, spawn count, spread (hours), cancel wait (min/max minutes)
  - **Pricing**: Base fare, per km rate, commission rate (displayed as percentage), surge pricing (checkbox, surge radius k, max multiplier)
  - **Rider quote**: Max willingness to pay ($), max ETA (min), accept probability (%), max quote rejections
  - **Matching**: Matching algorithm (Simple, Cost-based, or Hungarian (batch)), batch matching checkbox (default on), batch interval (s), match radius (km)
  - **Map & Trips**: Map size (km), trip length range (km, min-max)
  - **Routing & Traffic**: Routing backend (H3 Grid or OSRM with configurable endpoint), traffic profile (None or Berlin), congestion zones checkbox, dynamic congestion checkbox, spawn location weighting (Uniform or Berlin Hotspots), optional base speed override (km/h)
  - **Timing**: Simulation start time (year/month/day/hour/minute UTC with "Now" button), sim duration (hours; simulation stops when clock reaches this time), seed (optional)
  All parameters except matching algorithm are only editable before simulation starts. Platform revenue is displayed in the Run outcomes section.
- **Run outcomes**: Shows outcome counters (riders completed, riders cancelled with pickup timeout breakdown, abandoned quote with breakdown by reason: price too high, ETA too long, stochastic rejection, trips completed, total resolved, conversion %, platform revenue, total rider pay, avg fare),
  current state breakdowns (riders now: browsing/waiting/in transit, drivers now: idle/evaluating/en route/on trip/off duty, trips now: en route/on trip, fare distribution: to riders (total/min/avg/max/p50/p90) and to drivers (total/min/avg/max/p50/p90) from completed trips),
  and timing distributions for completed trips (time to match, time to pickup, trip duration) with min, max, average, and percentiles (p50, p90, p95, p99), plus surge impact distribution (additional cost due to surge pricing) with min, max, average, and percentiles (p50, p90, p95, p99).
- **Fleet**: Shows driver utilization metrics (busy %, total drivers, active drivers), state breakdown with percentages,
  earnings metrics (sum daily earnings/targets, targets met, off duty count, average earnings/target per driver, earnings distribution with percentiles,
  earnings/target ratio distribution with percentiles), and fatigue metrics (drivers at fatigue limit, session duration min/avg/max,
  fatigue threshold min/avg/max, drivers with fatigue data count).

## Trip Table

The trip table displays all trips (all states: EnRoute, OnTrip, Completed, Cancelled) with columns: Trip entity ID, Rider entity ID, Driver entity ID,
State, Pickup km (at driver acceptance), Distance km (pickup to dropoff), Requested (simulation datetime), Matched (simulation datetime),
Started (simulation datetime, if applicable), Completed (simulation datetime, if applicable), Cancelled (simulation datetime, if applicable).
The UI scales to 80% (pixels_per_point = 0.8) for better screen fit and includes toggle checkboxes for showing/hiding riders, drivers, driver stats, and grid overlay.
