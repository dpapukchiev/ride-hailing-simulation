# Coverage-aware matching and repositioning

## Flow

1. Build waiting-demand and idle-supply maps by H3 zone.
2. For each rider, evaluate feasible drivers with a composite score:
   - `pickup_time_cost`
   - `reposition_cost`
   - `imbalance_penalty`
   - `hotspot_bonus`
3. Assign greedily in demand-priority order.
4. On control-interval ticks, compute target idle supply (`uniform + hotspot uplift`) and move only surplus idle drivers to deficits under cooldown/distance/cap constraints.

## Objective intuition

- Pickup-time and reposition costs keep assignments local.
- Imbalance penalty protects thin zones from being emptied.
- Hotspot bonus increases the chance of nearby drivers in high-demand cells.

## Failure fallback

If policy selection cannot produce a match (or returns no candidates), systems fall back to the existing matching algorithm resource (cost-based/Hungarian/simple as configured), preserving robustness.

## Metrics interpretation

Track:
- mean pickup ETA
- p95 pickup ETA
- `% requests with pickup ETA <= X minutes`
- idle reposition distance per cycle
- per-zone supply/demand ratio

Healthy behavior is lower mean/p95 ETA without collapsing per-zone supply in low-demand areas.
