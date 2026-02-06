## Pricing

- Story: As a rider, I receive a quote with fare and ETA based on local supply
  and demand. I can accept (proceed to matching), reject and request another
  quote, or give up after a configurable number of rejections.
  Status: Done

- Story: As a marketplace operator, I can configure a commission rate so driver
  earnings and platform revenue can be studied.
  Status: Done

- Story: As a pricing engine, I can apply surge multipliers when demand exceeds
  supply within an H3 cluster.
  Status: Done

- Story: As a marketplace operator, I can configure base fare and per-kilometer
  rate to customize pricing formulas (fare = base_fare + distance_km × per_km_rate).
  Status: Done (configurable base_fare and per_km_rate in PricingConfig; adjustable in UI)

- Story: As a marketplace operator, I can calculate driver earnings (fare minus
  commission) separately from platform revenue, enabling analysis of driver
  economics.
  Status: Done (calculate_driver_earnings function computes fare × (1 - commission_rate); tracked per trip and accumulated in driver earnings)
