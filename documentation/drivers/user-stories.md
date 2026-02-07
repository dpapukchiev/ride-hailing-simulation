## Drivers

- Story: As a driver, I progress through Idle → Evaluating → EnRoute → OnTrip
  and return to Idle after completion.
  Status: Done

- Story: As a driver, my acceptance decision follows a logit model based on
  profit, pickup distance, and surge.
  Status: Done (logit model considers fare/profit, pickup distance, trip distance, earnings progress, and fatigue; fare includes surge when applied)

- Story: As a driver, I go OffDuty after hitting a daily earnings target or
  fatigue threshold.
  Status: Done

- Story: As a simulation engine, I periodically check all active drivers
  (every 5 minutes) for earnings targets and fatigue thresholds, including
  drivers in EnRoute or OnTrip, so limits are enforced even during back-to-back
  trips.
  Status: Done (CheckDriverOffDuty event scheduled every 5 minutes; checks all active drivers and transitions to OffDuty if thresholds exceeded)

- Story: As a driver, I can switch between ride-hailing and food delivery based
  on relative earnings.
  Status: Backlog
