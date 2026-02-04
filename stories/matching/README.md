## Matching

- Story: As a rider, I can be matched to an idle driver within a configurable
  H3 grid radius so nearby supply is used first.
  Status: Done

- Story: As a dispatcher, I can use a cost function (distance, ETA) to score
  pairings and select the best match instead of the first available driver.
  Status: Done

- Story: As a dispatcher, I can batch pending requests every N seconds and run
  a global matching pass to reduce total ETA.
  Status: Done (BatchMatchRun event, configurable interval; batch mode toggle in UI)

- Story: As a dispatcher, I can use a bipartite matching algorithm (e.g., Hungarian
  algorithm) to optimize driver-rider pairings globally when processing batches
  of waiting riders.
  Status: Done (HungarianMatching uses Kuhn–Munkres in find_batch_matches; default when batch enabled)

- Story: As a dispatcher, I can factor in opportunity cost when matching drivers,
  considering their potential earnings from other matches to maximize overall
  marketplace efficiency.
  Status: Backlog

- Story: As a dispatcher, I can weight driver-rider pairings by driver value
  (e.g., rating, earnings history, or other quality metrics) to prioritize
  better matches.
  Status: Backlog

- Story: As a dispatcher, I can minimize total ETA across all riders in a batch
  by solving a global optimization problem rather than matching riders sequentially.
  Status: Done (Hungarian algorithm minimizes total cost in each batch run)
