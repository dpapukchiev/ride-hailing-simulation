# sim_core Test Extraction + Context Reduction Refactor Plan

This plan is split into sessions so multiple agents can run in parallel with low
merge conflict risk while reducing code+test context in `sim_core`.

See `AGENTS.md` for repo constraints and command expectations before starting any
session.

## How to use this plan across sessions
- At the start of each new session, read this file first and continue from the
  first session marked `Not started` whose dependencies are complete.
- Run only the tasks listed for your session and file scope.
- At the end of each session, update the status board and add a handoff note.
- Do not mark a session as `Done` unless `./ci.sh check` ends with
  `✓ CI job 'check' passed.`

## Session status board

| Session | Lane | Title | Depends on | Status | Last updated | Notes |
|---|---|---|---|---|---|---|
| 0 | Shared | Baseline inventory + guardrails | - | Done | 2026-02-08 | Handed off with documentation updates and CI verified |
| 1 | Shared | Create reusable testkit + fixtures | 0 | Done | 2026-02-08 | See handoff below |
| 2A | A (Systems) | Extract end-to-end/system scenario tests | 1 | Not started | - | - |
| 2B | B (Spawn/Routing) | Extract spawn/routing integration scenarios | 1 | Not started | - | - |
| 2C | C (Core Utils) | Extract utility-heavy module tests | 1 | Not started | - | - |
| 3A | A (Driver lifecycle) | Split driver lifecycle tests from source files | 2A | Not started | - | - |
| 3B | B (Rider/quote/matching) | Split quote + matching tests from source files | 2A | Not started | - | - |
| 3C | C (Routing internals) | Split OSRM/routing internals tests | 2B | Not started | - | - |
| 4 | Shared | Refactor large scenario/spawner setup builders | 3A, 3B, 3C | Not started | - | - |
| 5 | Shared | CI/docs/test command alignment | 4 | Not started | - | - |
| 6 | Shared | Final validation + metrics signoff | 5 | Not started | - | - |

### Session 0 handoff note
Status: Done
Completed:
- Recorded the baseline inline test inventory table and clarified each extraction destination.
- Locked in naming conventions and lane ownership boundaries for future sessions.
Remaining:
- Kick off Session 1 when helpers are ready.
Verification:
- Ran: `./ci.sh check`
- Result: ✓ CI job 'check' passed.
Blockers:
- none.

### Session 1 handoff note
Status: Done
Completed:
- Added the reusable `tests/support` modules (world builder, entity fixtures, schedule runner) and marked them `#![allow(dead_code)]` until broader adoption, keeping the helpers deterministic via explicit seeds.
- Pointed `tests/load_tests.rs` at `TestWorldBuilder` so the ignored load scenarios compile through the shared fixture and the helpers stay exercised.
Remaining:
- Begin Session 2A by extracting the heavy scenario tests from `crates/sim_core/src/systems/mod.rs` into `crates/sim_core/tests/integration_system_end_to_end_tests.rs` while reusing the new helpers.
Verification:
- Ran: `cargo test -p sim_core`
- Ran: `./ci.sh check`
- Result: ✓ CI job 'check' passed.
Blockers:
- none.

Status values: `Not started`, `In progress`, `Blocked`, `Done`.

## Session handoff template
Use this template in the `Notes` column (or directly below the session):

```
Status: Done | In progress | Blocked
Completed:
- <short bullet>
- <short bullet>
Remaining:
- <short bullet>
Verification:
- Ran: <targeted tests>
- Ran: ./ci.sh check
- Result: ✓ CI job 'check' passed.
Blockers:
- <none or blocker>
```

## Goals
- Reduce per-file context in `crates/sim_core/src` by moving long/complex tests
  and scenario-heavy test setups out of production files.
- Keep fast unit checks near code only when they are short and private-behavior
  critical.
- Make refactors safe for parallel agent execution using strict file boundaries.
- Keep behavior unchanged.

## Non-negotiable rules (all sessions)
- Preserve semantics; no product behavior changes unless a test was incorrect.
- Keep tiny local unit tests in-place if extraction would force unnecessary API
  visibility changes.
- Move scenario-style or fixture-heavy tests to `crates/sim_core/tests/`.
- Prefer helper reuse over duplicating large world setup blocks.
- Each session must run targeted tests for touched areas, then `./ci.sh check`.
- If you add temporary debug helpers, remove them before session close.

## Baseline hotspots (current)
Use this baseline to prioritize extraction first where it buys the most context:

- Largest source files:
  - `crates/sim_core/src/scenario.rs` (~834 lines)
  - `crates/sim_core/src/systems/spawner.rs` (~726 lines)
  - `crates/sim_core/src/telemetry_export.rs` (~673 lines)
  - `crates/sim_core/src/spawner.rs` (~543 lines)
  - `crates/sim_core/src/systems/driver_decision.rs` (~471 lines)
- Largest inline test blocks (approx):
  - `crates/sim_core/src/systems/driver_decision.rs` (~305 lines)
  - `crates/sim_core/src/systems/mod.rs` (~244 lines)
  - `crates/sim_core/src/telemetry_export.rs` (~178 lines)
  - `crates/sim_core/src/systems/driver_offduty.rs` (~162 lines)
  - `crates/sim_core/src/systems/rider_cancel.rs` (~161 lines)
  - `crates/sim_core/src/systems/movement.rs` (~145 lines)

Target outcome by Session 6:
- At least 50% reduction in inline test lines in top-10 largest source files.
- End-to-end scenario tests live under `crates/sim_core/tests/`.
- No new production module exceeds its pre-refactor size.

## Pre-filled move map (source -> destination)
This table is intentionally pre-filled so agents can start immediately without
extra discovery work.

| Source file | Approx inline test size | Move type | Planned destination |
|---|---:|---|---|
| `crates/sim_core/src/systems/mod.rs` | 244 | full extraction | `crates/sim_core/tests/integration_system_end_to_end_tests.rs` |
| `crates/sim_core/src/systems/driver_decision.rs` | 305 | full extraction | `crates/sim_core/tests/system_driver_decision_tests.rs` |
| `crates/sim_core/src/systems/driver_offduty.rs` | 162 | full extraction | `crates/sim_core/tests/system_driver_offduty_tests.rs` |
| `crates/sim_core/src/systems/trip_started.rs` | 104 | full extraction | `crates/sim_core/tests/system_trip_lifecycle_tests.rs` |
| `crates/sim_core/src/systems/trip_completed.rs` | 113 | full extraction | `crates/sim_core/tests/system_trip_lifecycle_tests.rs` |
| `crates/sim_core/src/systems/rider_cancel.rs` | 161 | full extraction | `crates/sim_core/tests/system_rider_cancel_tests.rs` |
| `crates/sim_core/src/systems/show_quote.rs` | 62 | full extraction | `crates/sim_core/tests/system_quote_flow_tests.rs` |
| `crates/sim_core/src/systems/quote_decision.rs` | 130 | full extraction | `crates/sim_core/tests/system_quote_flow_tests.rs` |
| `crates/sim_core/src/systems/quote_rejected.rs` | 131 | full extraction | `crates/sim_core/tests/system_quote_flow_tests.rs` |
| `crates/sim_core/src/systems/quote_accepted.rs` | 86 | full extraction | `crates/sim_core/tests/system_quote_flow_tests.rs` |
| `crates/sim_core/src/systems/matching.rs` | 95 | full extraction | `crates/sim_core/tests/system_matching_flow_tests.rs` |
| `crates/sim_core/src/systems/match_accepted.rs` | 49 | full extraction | `crates/sim_core/tests/system_matching_flow_tests.rs` |
| `crates/sim_core/src/systems/match_rejected.rs` | 99 | full extraction | `crates/sim_core/tests/system_matching_flow_tests.rs` |
| `crates/sim_core/src/systems/movement.rs` | 145 | split extraction | `crates/sim_core/tests/system_movement_routing_tests.rs` |
| `crates/sim_core/src/scenario.rs` | 89 | full extraction | `crates/sim_core/tests/integration_scenario_builder_tests.rs` |
| `crates/sim_core/src/systems/spawner.rs` | 37 | selective extraction | `crates/sim_core/tests/integration_spawn_flow_tests.rs` |
| `crates/sim_core/src/routing.rs` | 49 | split extraction | `crates/sim_core/tests/integration_routing_provider_tests.rs` |
| `crates/sim_core/src/routing/osrm_spawn.rs` | 118 | split extraction | `crates/sim_core/tests/integration_osrm_spawn_fallback_tests.rs` |
| `crates/sim_core/src/telemetry_export.rs` | 178 | split extraction | `crates/sim_core/tests/integration_telemetry_export_tests.rs` |
| `crates/sim_core/src/clock.rs` | 44 | selective extraction | `crates/sim_core/tests/integration_clock_schedule_tests.rs` |
| `crates/sim_core/src/traffic.rs` | 70 | selective extraction | `crates/sim_core/tests/integration_traffic_profile_tests.rs` |
| `crates/sim_core/src/pricing.rs` | 89 | keep local by default | keep inline unless Session 6 shows low ROI |
| `crates/sim_core/src/distributions.rs` | 31 | keep local by default | keep inline unless helper reuse is needed |
| `crates/sim_core/src/spatial.rs` | 18 | keep local | keep inline |
| `crates/sim_core/src/test_helpers.rs` | 26 | keep local | keep inline |

---

## Session 0 - Baseline inventory + guardrails

### Tasks
- Capture baseline test layout in a small table in this file:
  - source file
  - inline test line estimate
  - planned destination test file
- Freeze naming convention for extracted tests:
  - `crates/sim_core/tests/system_<area>_tests.rs`
  - `crates/sim_core/tests/integration_<area>_tests.rs`
- Define ownership map for lanes A/B/C to avoid file overlap.

### Baseline inventory snapshot
| Source file | Inline test line estimate | Planned destination |
|---|---:|---|
| `crates/sim_core/src/systems/mod.rs` | 244 | `crates/sim_core/tests/integration_system_end_to_end_tests.rs` |
| `crates/sim_core/src/systems/driver_decision.rs` | 305 | `crates/sim_core/tests/system_driver_decision_tests.rs` |
| `crates/sim_core/src/systems/driver_offduty.rs` | 162 | `crates/sim_core/tests/system_driver_offduty_tests.rs` |
| `crates/sim_core/src/scenario.rs` | 89 | `crates/sim_core/tests/integration_scenario_builder_tests.rs` |
| `crates/sim_core/src/telemetry_export.rs` | 178 | `crates/sim_core/tests/integration_telemetry_export_tests.rs` |

### Naming convention (frozen)
Extracted test files follow these established patterns:
- `crates/sim_core/tests/system_<area>_tests.rs` for driver/rider/matching lifecycle scenarios.
- `crates/sim_core/tests/integration_<area>_tests.rs` for routing, telemetry, scenario builder, and spawn flows.

### Lane ownership map (frozen)
| Lane | Scope highlights |
|---|---|
| A | `systems/mod.rs`, driver lifecycle systems (`driver_*`, `trip_*`), `system_driver_*` and `system_trip_*` tests |
| B | Quote/matching/rider cancel systems + scenario/spawner sources; `system_quote_*`, `system_matching_*`, and spawn integration tests |
| C | Routing, OSRM, movement, telemetry, clock, and traffic sources; integration routing/telemetry/clock/traffic tests |

### Allowed files
- `documentation/testing/test-context-reduction-refactor-plan.md`

### Acceptance criteria
- Baseline + naming + ownership map are written and agreed.
- Every later session has a destination file path pre-declared.

### Verification
- No code changes required.

---

## Session 1 - Create reusable testkit + fixtures

### Tasks
- Add reusable test setup helpers under `crates/sim_core/tests/support/`:
  - world bootstrap helper (clock, telemetry, traffic, pricing, spatial index)
  - common seeded cell helpers
  - standard rider/driver spawn helper builders
  - schedule runner helper for single-event and full-run patterns
- Keep helpers integration-test-facing (public within test crate via `mod support`).
- Avoid coupling testkit to private module internals.

### Suggested files
- `crates/sim_core/tests/support/mod.rs`
- `crates/sim_core/tests/support/world.rs`
- `crates/sim_core/tests/support/entities.rs`
- `crates/sim_core/tests/support/schedule.rs`

### Acceptance criteria
- At least one existing integration test (`load_tests`) can compile with support
  imports (no behavior change required).
- New helpers are deterministic with explicit seed defaults.

### Verification
- `cargo test -p sim_core --test load_tests -- --ignored --nocapture` (optional)
- `cargo test -p sim_core`
- `./ci.sh check`

---

## Session 2A - Extract end-to-end/system scenario tests (Lane A)

### Scope
Move the heavy `#[cfg(test)]` scenario tests out of `systems/mod.rs`.

### Tasks
- Extract end-to-end tests from `crates/sim_core/src/systems/mod.rs` into:
  - `crates/sim_core/tests/integration_system_end_to_end_tests.rs`
- Reuse Session 1 support helpers to remove duplicated world setup.
- Keep assertions equivalent (trip completion, driver final state, telemetry
  ordering checks).
- Leave only minimal in-source smoke checks if still useful.

### Allowed files
- `crates/sim_core/src/systems/mod.rs`
- `crates/sim_core/tests/integration_system_end_to_end_tests.rs`
- `crates/sim_core/tests/support/**/*`

### Acceptance criteria
- `systems/mod.rs` no longer contains full scenario-style setup tests.
- Extracted tests pass under `cargo test -p sim_core --test integration_system_end_to_end_tests`.

### Verification
- `cargo test -p sim_core --test integration_system_end_to_end_tests`
- `cargo test -p sim_core`
- `./ci.sh check`

---

## Session 2B - Extract spawn/routing integration scenarios (Lane B)

### Scope
Move scenario-like tests from spawn/routing files that are large or fixture-heavy.

### Tasks
- Review and extract scenario-heavy tests from:
  - `crates/sim_core/src/scenario.rs`
  - `crates/sim_core/src/systems/spawner.rs`
  - `crates/sim_core/src/routing/osrm_spawn.rs` (only scenario-style tests)
- Create destination files:
  - `crates/sim_core/tests/integration_scenario_builder_tests.rs`
  - `crates/sim_core/tests/integration_spawn_flow_tests.rs`
  - `crates/sim_core/tests/integration_osrm_spawn_fallback_tests.rs`
- Keep parser/selection micro-tests in-source when private internals are tested.

### Allowed files
- `crates/sim_core/src/scenario.rs`
- `crates/sim_core/src/systems/spawner.rs`
- `crates/sim_core/src/routing/osrm_spawn.rs`
- `crates/sim_core/tests/integration_scenario_builder_tests.rs`
- `crates/sim_core/tests/integration_spawn_flow_tests.rs`
- `crates/sim_core/tests/integration_osrm_spawn_fallback_tests.rs`
- `crates/sim_core/tests/support/**/*`

### Acceptance criteria
- Scenario-scale setup tests are moved out of source modules.
- Spawn bounds and OSRM fallback behavior stay covered.

### Verification
- `cargo test -p sim_core --test integration_scenario_builder_tests`
- `cargo test -p sim_core --test integration_spawn_flow_tests`
- `cargo test -p sim_core --features osrm --test integration_osrm_spawn_fallback_tests`
- `cargo test -p sim_core`
- `./ci.sh check`

---

## Session 2C - Extract utility-heavy module tests (Lane C)

### Scope
Move longer functional tests from utility modules where extraction reduces noise.

### Tasks
- Extract heavier tests from:
  - `crates/sim_core/src/telemetry_export.rs`
  - `crates/sim_core/src/traffic.rs`
  - `crates/sim_core/src/clock.rs`
- Create files:
  - `crates/sim_core/tests/integration_telemetry_export_tests.rs`
  - `crates/sim_core/tests/integration_traffic_profile_tests.rs`
  - `crates/sim_core/tests/integration_clock_schedule_tests.rs`
- Keep tiny pure-function checks local only if extraction causes awkward API
  exposure.

### Allowed files
- `crates/sim_core/src/telemetry_export.rs`
- `crates/sim_core/src/traffic.rs`
- `crates/sim_core/src/clock.rs`
- `crates/sim_core/tests/integration_telemetry_export_tests.rs`
- `crates/sim_core/tests/integration_traffic_profile_tests.rs`
- `crates/sim_core/tests/integration_clock_schedule_tests.rs`
- `crates/sim_core/tests/support/**/*`

### Acceptance criteria
- Telemetry timestamp ordering validation remains fully covered.
- Utility modules are slimmer and focused on production logic.

### Verification
- `cargo test -p sim_core --test integration_telemetry_export_tests`
- `cargo test -p sim_core --test integration_traffic_profile_tests`
- `cargo test -p sim_core --test integration_clock_schedule_tests`
- `cargo test -p sim_core`
- `./ci.sh check`

---

## Session 3A - Split driver lifecycle tests from source files (Lane A)

### Scope
Refactor driver lifecycle tests into focused integration files with shared setup.

### Tasks
- Extract tests from:
  - `crates/sim_core/src/systems/driver_decision.rs`
  - `crates/sim_core/src/systems/driver_offduty.rs`
  - `crates/sim_core/src/systems/trip_started.rs`
  - `crates/sim_core/src/systems/trip_completed.rs`
- Create files:
  - `crates/sim_core/tests/system_driver_decision_tests.rs`
  - `crates/sim_core/tests/system_driver_offduty_tests.rs`
  - `crates/sim_core/tests/system_trip_lifecycle_tests.rs`
- Keep deterministic RNG assertions and event scheduling assertions intact.

### Allowed files
- `crates/sim_core/src/systems/driver_decision.rs`
- `crates/sim_core/src/systems/driver_offduty.rs`
- `crates/sim_core/src/systems/trip_started.rs`
- `crates/sim_core/src/systems/trip_completed.rs`
- `crates/sim_core/tests/system_driver_decision_tests.rs`
- `crates/sim_core/tests/system_driver_offduty_tests.rs`
- `crates/sim_core/tests/system_trip_lifecycle_tests.rs`
- `crates/sim_core/tests/support/**/*`

### Acceptance criteria
- Driver lifecycle source files mostly contain production logic.
- Event-sequencing and state-transition coverage preserved.

### Verification
- `cargo test -p sim_core --test system_driver_decision_tests`
- `cargo test -p sim_core --test system_driver_offduty_tests`
- `cargo test -p sim_core --test system_trip_lifecycle_tests`
- `cargo test -p sim_core`
- `./ci.sh check`

---

## Session 3B - Split quote + matching tests from source files (Lane B)

### Scope
Move rider quote/matching system tests into dedicated files.

### Tasks
- Extract tests from:
  - `crates/sim_core/src/systems/quote_decision.rs`
  - `crates/sim_core/src/systems/quote_rejected.rs`
  - `crates/sim_core/src/systems/quote_accepted.rs`
  - `crates/sim_core/src/systems/matching.rs`
  - `crates/sim_core/src/systems/match_accepted.rs`
  - `crates/sim_core/src/systems/match_rejected.rs`
  - `crates/sim_core/src/systems/show_quote.rs`
  - `crates/sim_core/src/systems/rider_cancel.rs`
- Create files:
  - `crates/sim_core/tests/system_quote_flow_tests.rs`
  - `crates/sim_core/tests/system_matching_flow_tests.rs`
  - `crates/sim_core/tests/system_rider_cancel_tests.rs`

### Allowed files
- `crates/sim_core/src/systems/quote_decision.rs`
- `crates/sim_core/src/systems/quote_rejected.rs`
- `crates/sim_core/src/systems/quote_accepted.rs`
- `crates/sim_core/src/systems/matching.rs`
- `crates/sim_core/src/systems/match_accepted.rs`
- `crates/sim_core/src/systems/match_rejected.rs`
- `crates/sim_core/src/systems/show_quote.rs`
- `crates/sim_core/src/systems/rider_cancel.rs`
- `crates/sim_core/tests/system_quote_flow_tests.rs`
- `crates/sim_core/tests/system_matching_flow_tests.rs`
- `crates/sim_core/tests/system_rider_cancel_tests.rs`
- `crates/sim_core/tests/support/**/*`

### Acceptance criteria
- Quote/match/rider cancel behavior remains fully covered.
- Source modules are shorter and easier to read.

### Verification
- `cargo test -p sim_core --test system_quote_flow_tests`
- `cargo test -p sim_core --test system_matching_flow_tests`
- `cargo test -p sim_core --test system_rider_cancel_tests`
- `cargo test -p sim_core`
- `./ci.sh check`

---

## Session 3C - Split OSRM/routing internals tests (Lane C)

### Scope
Move routing tests that do not need private access; keep parser microtests local.

### Tasks
- Extract suitable tests from:
  - `crates/sim_core/src/routing.rs`
  - `crates/sim_core/src/routing/osrm_spawn.rs`
  - `crates/sim_core/src/systems/movement.rs`
- Create files:
  - `crates/sim_core/tests/system_movement_routing_tests.rs`
  - `crates/sim_core/tests/integration_routing_provider_tests.rs`
- For tests that require private helper access, either:
  - keep in-source, or
  - promote helper visibility with `pub(crate)` only when justified.

### Allowed files
- `crates/sim_core/src/routing.rs`
- `crates/sim_core/src/routing/osrm_spawn.rs`
- `crates/sim_core/src/systems/movement.rs`
- `crates/sim_core/tests/system_movement_routing_tests.rs`
- `crates/sim_core/tests/integration_routing_provider_tests.rs`
- `crates/sim_core/tests/support/**/*`

### Acceptance criteria
- Movement/routing behavior is still validated with deterministic seeds.
- No unnecessary public API exposure for testing only.

### Verification
- `cargo test -p sim_core --test system_movement_routing_tests`
- `cargo test -p sim_core --test integration_routing_provider_tests`
- `cargo test -p sim_core --features osrm`
- `./ci.sh check`

---

## Session 4 - Refactor large scenario/spawner setup builders

### Scope
Now that heavy tests moved out, reduce production-file size by extracting setup
submodules from large files.

### Tasks
- Split `crates/sim_core/src/scenario.rs` into focused modules:
  - scenario params/config types
  - world resource wiring
  - spawner builder wiring
  - optional OSRM wiring
- Split `crates/sim_core/src/spawner.rs` and/or
  `crates/sim_core/src/systems/spawner.rs` into focused modules where practical
  (candidate generation, bounds guards, spawn resolver usage).
- Keep public API paths stable (`sim_core::scenario::*`, etc.).

### Suggested files
- `crates/sim_core/src/scenario/mod.rs`
- `crates/sim_core/src/scenario/params.rs`
- `crates/sim_core/src/scenario/build.rs`
- `crates/sim_core/src/scenario/spawner_wiring.rs`
- `crates/sim_core/src/spawner/mod.rs`
- `crates/sim_core/src/spawner/bounds.rs`
- `crates/sim_core/src/spawner/resolver.rs`

### Acceptance criteria
- No behavior change in scenario construction or spawn scheduling.
- Refactored modules are smaller and easier to load independently.

### Verification
- `cargo test -p sim_core`
- `cargo run -p sim_core --example scenario_run`
- `./ci.sh check`

---

## Session 5 - CI/docs/test command alignment

### Tasks
- Update test docs with new layout and commands:
  - `documentation/testing/spec.md`
- Add a concise mapping table (module -> new test file).
- Ensure contributors can run targeted tests per area quickly.

### Suggested files
- `documentation/testing/spec.md`

### Acceptance criteria
- Documentation reflects actual test locations.
- New contributors can run focused tests without reading source internals.

### Verification
- Manual read-through
- `./ci.sh check`

---

## Session 6 - Final validation + metrics signoff

### Tasks
- Recompute file-size and inline-test metrics and append to this plan.
- Compare with baseline hotspots and verify reduction target.
- Run full verification suite and capture pass/fail summary.

### Acceptance criteria
- All prior sessions marked `Done`.
- Inline test reduction target met, or gap + follow-up plan documented.
- CI check passes.

### Verification
- `cargo test -p sim_core`
- `cargo test -p sim_core --features osrm`
- `cargo test --workspace`
- `./ci.sh check`

---

## Parallel execution map

Use these disjoint boundaries so 2A/2B/2C and 3A/3B/3C can run at the same time.

- Lane A owns:
  - `crates/sim_core/src/systems/mod.rs`
  - driver lifecycle system files
  - `crates/sim_core/tests/system_driver_*`
  - `crates/sim_core/tests/system_trip_*`
- Lane B owns:
  - quote/match/rider-cancel system files
  - scenario/spawner integration test files
  - `crates/sim_core/tests/system_quote_*`
  - `crates/sim_core/tests/system_matching_*`
- Lane C owns:
  - routing/osrm/movement utility files
  - telemetry_export/clock/traffic extraction
  - `crates/sim_core/tests/integration_*routing*`
  - `crates/sim_core/tests/integration_*telemetry*`

Conflict rule:
- If two lanes must touch the same file, pause and create a tiny pre-session that
  lands pure mechanical moves first, then rebase the two lanes.

---

## Agent task card template (copy/paste)

```text
See AGENTS.md for repo constraints and command expectations.
Task: <session id + title>
Allowed files: <list from this plan>
Avoid files: everything else
Constraints:
- Preserve behavior; no production logic drift
- Use shared test support helpers where available
- Keep refactor scoped to this lane
Acceptance:
- Run targeted tests for touched files
- Run `cargo test -p sim_core`
- Run `./ci.sh check` and require: ✓ CI job 'check' passed.
Output:
- Brief summary
- Files changed
- Risks/follow-ups
```

## Notes for reviewers
- Prefer reviewing extracted tests first, then production diffs.
- Watch for accidental API widening done only for tests.
- If a moved test became flaky, keep old inline version until deterministic fix is
  merged in same session.
