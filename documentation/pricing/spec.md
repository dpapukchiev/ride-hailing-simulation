# Pricing

## `sim_core::pricing`

Configurable pricing system with commission support and surge pricing for marketplace analysis.

- **`PricingConfig`** (ECS Resource): Configurable pricing parameters (see [CONFIG.md](../../CONFIG.md#pricing-configuration) for defaults and formulas).
- **`calculate_trip_fare(pickup, dropoff)`**: Calculates base fare using default constants (backward compatibility). Formula: `fare = BASE_FARE + (distance_km * PER_KM_RATE)`. Note: This does not include surge pricing; surge is applied separately in `show_quote_system`.
- **`calculate_trip_fare_with_config(pickup, dropoff, config)`**: Calculates base fare using provided `PricingConfig`. Note: This does not include surge pricing; surge is applied separately in `show_quote_system`.
- **Surge pricing**: When `surge_enabled` is true, surge multipliers are calculated dynamically in `show_quote_system` based on local supply and demand (see [CONFIG.md](../../CONFIG.md#pricing-configuration) for formula).
- **`calculate_commission(fare, commission_rate)`**: Calculates commission amount (`fare * commission_rate`).
- **`calculate_driver_earnings(fare, commission_rate)`**: Calculates driver net earnings (`fare * (1 - commission_rate)`).
- **`calculate_platform_revenue(fare, commission_rate)`**: Calculates platform revenue (same as commission).

See [CONFIG.md](../../CONFIG.md#pricing-configuration) for detailed pricing formulas, defaults, and surge calculation logic.

## `sim_core::systems::show_quote`

System: `show_quote_system`

- Reacts to `CurrentEvent`.
- On `EventKind::ShowQuote` with subject `Rider(rider_entity)`:
  - Rider must be in `Browsing`. Reads `PricingConfig` from resources. Computes **base fare** via `calculate_trip_fare_with_config(pickup, dropoff, config)`. When `surge_enabled` and `surge_radius_k > 0`, calculates surge multiplier: counts demand (Browsing/Waiting riders) and supply (Idle drivers) in `grid_disk(pickup, surge_radius_k)`. If `demand > supply` and `supply > 0`: `multiplier = min(1.0 + (demand - supply) / supply, surge_max_multiplier)`. If `demand > supply` and `supply == 0`: `multiplier = surge_max_multiplier`. Otherwise: `multiplier = 1.0`. **Fare** = base fare × surge multiplier. **ETA** = nearest idle driver distance/speed, or default 300s. Inserts `RiderQuote { fare, eta_ms }` on the rider entity.
  - Schedules `QuoteDecision` 1 second from now for the same rider.
