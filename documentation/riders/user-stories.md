## Riders

- Story: As a rider, I move through Browsing → Waiting → InTransit
  and complete or cancel the trip.
  Status: Done

- Story: As a rider, I cancel if the projected pickup time exceeds my wait
  window. The system continuously updates pickup ETAs as drivers move, and I
  cancel if the projected pickup time (now + ETA) exceeds my wait deadline.
  Status: Done

- Story: As a rider, I decide to book based on price elasticity and my maximum
  willingness to pay.
  Status: Done (max_willingness_to_pay threshold in RiderQuoteConfig; quote rejected if fare exceeds threshold)

- Story: As a rider, I can reject a quote and request another, or give up after
  N rejections (abandoned-quote telemetry).
  Status: Done

- Story: As a rider, I abandon the request if the ETA is too high even before
  matching.
  Status: Done (max_acceptable_eta_ms threshold in RiderQuoteConfig; quote rejected if ETA exceeds threshold before matching)

- Story: As a rider waiting for pickup, I see continuously updated pickup ETAs
  as my driver moves, and I cancel if the projected pickup time exceeds my wait
  window.
  Status: Done (pickup_eta_updated_system updates ETA on each driver movement step; rider cancels if projected pickup time exceeds wait deadline)
