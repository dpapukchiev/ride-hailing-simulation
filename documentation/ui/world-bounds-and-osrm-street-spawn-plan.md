# World Bounds + OSRM Street Spawn Implementation Plan

This plan is split into sessions so you can implement one chunk at a time and
keep each session small, testable, and reviewable.

## How to use this plan across sessions
- At the start of each new session, read this file first and continue from the
  first session marked `Not started` or `In progress`.
- At the end of each session, update the status block in this file before
  ending work so the next agent can resume without extra context.
- Always include a short note with what changed, what remains, and any blockers.
- Do not mark a session as `Done` unless `./ci.sh check` ends with
  `✓ CI job 'check' passed.`

## Session status board

| Session | Title | Status | Last updated | Notes |
|---|---|---|---|---|
| 1 | Lock world-bounds behavior at spawn time | Not started | - | - |
| 2 | Add OSRM match-service spawn snap utility | Not started | - | - |
| 3 | Build spawn position resolver for routing mode | Not started | - | - |
| 4 | Add graceful fallback chain for OSRM spawn failures | Not started | - | - |
| 5 | Prevent out-of-bounds drift during movement (policy pass) | Not started | - | - |
| 6 | UI/Docs alignment | Not started | - | - |
| 7 | End-to-end validation and perf guardrails | Not started | - | - |

Status values: `Not started`, `In progress`, `Blocked`, `Done`.

## Session handoff template
Use this template in the `Notes` column (or directly below a session) when you
finish a session:

```
Status: Done | In progress | Blocked
Completed:
- <short bullet>
- <short bullet>
Remaining:
- <short bullet>
CI:
- Ran: ./ci.sh check
- Result: ✓ CI job 'check' passed.
Blockers:
- <none or blocker>
```

## Goals
- `Map size (km)` must bound the simulation world, not only the viewport.
- In `RoutingMode::Osrm`, rider and driver spawns should land on real streets
  (not lakes/parks/water).
- Preserve current behavior for non-OSRM runs unless explicitly changed.

## Session 1 - Lock world-bounds behavior at spawn time

### Tasks
- Add a shared bounds check helper in `sim_core` for H3 cells and lat/lng.
- Enforce bounds in `spawn_rider` and `spawn_driver` for all spawn sources:
  - uniform random
  - weighted hotspots
- If a weighted spawn cell is out of bounds, fall back to bounded random spawn.

### Suggested files
- `crates/sim_core/src/systems/spawner.rs`
- `crates/sim_core/src/spawner.rs`
- `crates/sim_core/src/scenario.rs` (if you move/share helper logic)

### Acceptance criteria
- No rider/driver spawn outside configured lat/lng bounds.
- Small map sizes still work with `SpawnMode::BerlinHotspots`.

### Verification
- Add/adjust unit tests for weighted spawns under tight bounds.
- Run `cargo test -p sim_core`.
- Run `./ci.sh check` and only close this session when it ends with
  `✓ CI job 'check' passed.`

---

## Session 2 - Add OSRM match-service spawn snap utility

### Tasks
- Add an OSRM helper that snaps candidate spawn points to roads using
  OSRM `match` service (`/match/v1/driving/...`).
- Use match options tailored for spawn snapping:
  - `gaps=ignore`
  - `tidy=true`
  - `radiuses=<meters>` per point (start with a conservative value like 15-30m)
  - `geometries=geojson` (if route geometry is needed for diagnostics)
- Parse and return structured output from `matchings` + `tracepoints`:
  - snapped coordinate from non-null tracepoint
  - `confidence` from selected matching
  - optional snapped distance/name metadata when available
- Add timeout + error handling so failures degrade gracefully.

### Suggested files
- `crates/sim_core/src/routing.rs` (or a new `routing/osrm_spawn.rs` module)

### Acceptance criteria
- Valid input returns a snapped road coordinate when OSRM is reachable.
- Matching selection is deterministic (pick highest-confidence matching, then
  stable tie-break by first non-null tracepoint).
- Failures return cleanly (no panic; caller can fallback).

### Verification
- Add isolated tests for response parsing and error paths.
- If integration tests are hard, add small parser-level unit tests.
- Run `./ci.sh check` and only close this session when it ends with
  `✓ CI job 'check' passed.`

---

## Session 3 - Build spawn position resolver for routing mode

### Tasks
- Introduce a spawn resolver abstraction (or function) that chooses spawn logic
  based on routing mode:
  - `H3Grid`: existing bounded behavior
  - `Osrm`: bounded candidate(s) -> `match` snap -> H3 cell conversion
- Ensure the snapped coordinate is written to `GeoPosition` at spawn time.
- For `match` inputs, generate a tiny synthetic trace (2-3 nearby points within
  bounds) so map matching has enough context while preserving spawn locality.
- Keep deterministic RNG for candidate selection; allow snapping to be
  nondeterministic only where OSRM response requires it.

### Suggested files
- `crates/sim_core/src/systems/spawner.rs`
- `crates/sim_core/src/scenario.rs` (wire config/resources)
- `crates/sim_core/src/ecs.rs` (only if component wiring needs updates)

### Acceptance criteria
- In `RoutingMode::Osrm`, new entities spawn near drivable roads.
- In `RoutingMode::H3Grid`, behavior stays unchanged.

### Verification
- Add tests for mode switching and fallback behavior.
- Run `cargo test -p sim_core`.
- Run `./ci.sh check` and only close this session when it ends with
  `✓ CI job 'check' passed.`

---

## Session 4 - Add graceful fallback chain for OSRM spawn failures

### Tasks
- Implement explicit fallback order for OSRM mode:
  1) `match` bounded candidate trace to road and require minimum confidence
  2) retry with adjusted `radiuses` and/or fresh bounded candidate trace(s)
  3) fallback to `nearest` snap (single-point) as compatibility path
  4) fallback to bounded random cell center
- Add rate limiting or simple retry cap to avoid blocking spawn throughput.
- Add observability counters/log fields for snap success/failure rate.

### Suggested files
- `crates/sim_core/src/systems/spawner.rs`
- `crates/sim_core/src/telemetry.rs` (if adding counters)

### Acceptance criteria
- Simulation never stalls due to OSRM spawn snapping failures.
- Spawns still respect bounds even when OSRM is down.

### Verification
- Add tests/mocks for failure paths.
- Run `cargo test -p sim_core`.
- Run `./ci.sh check` and only close this session when it ends with
  `✓ CI job 'check' passed.`

---

## Session 5 - Prevent out-of-bounds drift during movement (policy pass)

### Tasks
- Decide and implement movement-boundary policy:
  - soft policy: allow route geometry to leave bounds transiently
  - hard policy: clamp/re-route/terminate when step exits bounds
- Implement the chosen policy consistently in movement updates.

### Suggested files
- `crates/sim_core/src/systems/movement.rs`
- `crates/sim_core/src/scenario.rs` (policy config resource)

### Acceptance criteria
- Behavior near map edges is explicit and deterministic.
- No silent out-of-bounds movement if hard policy is selected.

### Verification
- Add boundary-focused movement tests.
- Run `cargo test -p sim_core`.
- Run `./ci.sh check` and only close this session when it ends with
  `✓ CI job 'check' passed.`

---

## Session 6 - UI/Docs alignment

### Tasks
- Update UI/docs to reflect new semantics:
  - map size bounds simulation world
  - OSRM mode uses road-snapped spawn positions
  - fallback behavior when OSRM is unavailable
- Clarify that this applies to both riders and drivers.

### Suggested files
- `documentation/ui/spec.md`
- `documentation/ui/driver-map.md`
- `documentation/core-sim/spec.md`

### Acceptance criteria
- Documentation matches implementation and edge cases.

### Verification
- Manual read-through + optional screenshot refresh.
- Run `./ci.sh check` and only close this session when it ends with
  `✓ CI job 'check' passed.`

---

## Session 7 - End-to-end validation and perf guardrails

### Tasks
- Add a scenario-level test or scripted check for:
  - small map size
  - OSRM mode on
  - all new spawns road-snapped and in bounds
- Measure spawn-time overhead from `match` calls (and fallback `nearest` calls).
- Add caching/batching if needed for acceptable UI runtime.

### Acceptance criteria
- End-to-end behavior is stable and reasonably fast.
- Regressions are covered by automated checks.

### Verification
- Run `cargo test -p sim_core` and `./ci.sh`.
- Require `./ci.sh check` success at minimum before session sign-off:
  `✓ CI job 'check' passed.`

---

## Implementation notes
- Keep spawn constraints in this order: `world bounds` first, `road snap` second.
- Never let road snapping expand effective world bounds.
- Reuse existing `GeoPosition` rendering path so UI benefits immediately.
- For `match`, treat `tracepoints` with null entries as outliers and skip them.
- Use `confidence` (0..1) threshold to reject weak matches and trigger fallback.
- If snapped distance is available, reject snaps beyond a max threshold to avoid
  unrealistic teleports to distant roads.
