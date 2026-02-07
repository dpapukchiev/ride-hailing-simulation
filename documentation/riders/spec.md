# Riders

## ECS Components

- `RiderState`: `Browsing`, `Waiting`, `InTransit`, `Completed`, `Cancelled`
- `Rider` component: `{ state, matched_driver, assigned_trip: Option<Entity>, destination: Option<CellIndex>, requested_at: Option<u64>, quote_rejections: u32, accepted_fare: Option<f64>, last_rejection_reason: Option<RiderAbandonmentReason> }`
  - `assigned_trip`: backlink to the active Trip entity. Set when a Trip is spawned, cleared on trip completion/cancellation. Enables O(1) trip lookup.
  - `destination`: requested dropoff cell. Must be set; riders without a destination will be rejected by the driver decision system.
  - `requested_at`: simulation time (ms) when the rider was spawned (set by spawner when spawning).
  - `quote_rejections`: number of times this rider has rejected a quote; used for give-up after `max_quote_rejections`.
  - `accepted_fare`: fare the rider accepted when transitioning to Waiting; used for driver earnings and trip completion.
  - `last_rejection_reason`: tracks the reason for the most recent quote rejection (`QuotePriceTooHigh`, `QuoteEtaTooLong`, `QuoteStochasticRejection`); used to record abandonment reason when rider gives up.
- `RiderQuote` component (optional, attached while viewing a quote): `{ fare: f64, eta_ms: u64 }` — current quote shown to the rider (for UI/telemetry).

## `sim_core::systems::quote_decision`

System: `quote_decision_system`

- Reacts to `CurrentEvent`.
- On `EventKind::QuoteDecision` with subject `Rider(rider_entity)`:
  - Rider must be in `Browsing` with a `RiderQuote` component.
  - If `quote.fare > max_willingness_to_pay`: sets `rider.last_rejection_reason = QuotePriceTooHigh` and schedules `QuoteRejected`.
  - Else if `quote.eta_ms > max_acceptable_eta_ms`: sets `rider.last_rejection_reason = QuoteEtaTooLong` and schedules `QuoteRejected`.
  - Else: stochastically accepts/rejects based on `accept_probability`; if rejected, sets `rider.last_rejection_reason = QuoteStochasticRejection` and schedules `QuoteRejected`; if accepted, schedules `QuoteAccepted`.
  - Rider must be in `Browsing` with `RiderQuote`. If quote fare > `max_willingness_to_pay` or quote eta_ms > `max_acceptable_eta_ms`, schedules `QuoteRejected`. Otherwise samples accept/reject using `RiderQuoteConfig::accept_probability` (seed + rider entity ID for reproducibility).
  - If accept: schedules `QuoteAccepted` at current time. If reject: schedules `QuoteRejected` at current time.

## `sim_core::systems::quote_accepted`

System: `quote_accepted_system`

- Reacts to `CurrentEvent`.
- On `EventKind::QuoteAccepted` with subject `Rider(rider_entity)`:
  - Rider: `Browsing` → `Waiting`; sets `rider.accepted_fare = Some(quote.fare)`; removes `RiderQuote` component.
  - If batch matching is **disabled**, schedules `TryMatch` 1 second from now for the same rider.
  - Samples cancellation time from uniform distribution between `min_wait_secs` and `max_wait_secs` in `RiderCancelConfig` (using seed + rider entity ID for reproducibility with variety), then schedules `RiderCancel` at that sampled time.

## `sim_core::systems::quote_rejected`

System: `quote_rejected_system`

- Reacts to `CurrentEvent`.
- On `EventKind::QuoteRejected` with subject `Rider(rider_entity)`:
  - Rider must be in `Browsing`. Increments `rider.quote_rejections`.
  - If `quote_rejections < max_quote_rejections`: schedules `ShowQuote` at `now + re_quote_delay_secs` (rider requests another quote).
  - Else: rider gives up — state set to `Cancelled`, entity despawned, `SimTelemetry::riders_abandoned_quote_total` incremented, and the appropriate breakdown counter is incremented based on `rider.last_rejection_reason` (`riders_abandoned_price`, `riders_abandoned_eta`, or `riders_abandoned_stochastic`).

## `sim_core::systems::rider_cancel`

System: `rider_cancel_system`

- Reacts to `CurrentEvent`.
- On `EventKind::RiderCancel` with subject `Rider(rider_entity)`:
  - Handles both rider-initiated timeout cancels (scheduled by `quote_accepted_system`) and
    ETA-triggered cancels (delegated by `pickup_eta_updated_system` at delta 0). Uses
    `rider.assigned_trip` for O(1) trip lookup (no full trip scan).
  - Rider must be in `Waiting`. Sets rider state to `Cancelled`, increments `SimTelemetry::riders_cancelled_total` and `SimTelemetry::riders_cancelled_pickup_timeout`, despawns rider entity. If rider has a matched driver, cancels the associated trip and resets driver state.
  - If the rider is still `Waiting`:
    - Rider: `Waiting` → `Cancelled`, clears `matched_driver`, then the rider entity is despawned
    - If a matched driver exists, clears `matched_rider` and returns the driver to `Idle`
    - If an `EnRoute` trip exists for that rider, marks it `Cancelled`

## `sim_core::systems::pickup_eta_updated`

System: `pickup_eta_updated_system`

- Reacts to `CurrentEvent`.
- On `EventKind::PickupEtaUpdated` with subject `Trip(trip_entity)`:
  - Pure patience check — read-only queries, no direct state mutations.
  - If the trip is `EnRoute` and the rider is still `Waiting`, compares projected pickup time
    (`now + trip.pickup_eta_ms`) to the rider's wait window (`RiderCancelConfig`).
  - If the projected pickup exceeds the wait deadline (after min wait), schedules a
    `RiderCancel` event at delta 0 with `EventSubject::Rider(rider_entity)` so that
    `rider_cancel_system` handles all cancellation mutations.
  - **Cancelled/Completed**: no-op.
- ETA in ms: derived from haversine distance and a stochastic speed sample
  (default 20–60 km/h), with a 1 second minimum (`ONE_SEC_MS`).

See [CONFIG.md](../../CONFIG.md#movement--speed) for speed sampling formulas and movement time calculations.

This is a deterministic, FCFS-style placeholder. No distance or cost logic
is implemented yet beyond H3 grid distance and simple stochastic speeds.
