## Experiments

- Story: As a researcher, I can run multiple simulations in parallel with varying parameters to explore parameter space efficiently.
  Status: Done

- Story: As a researcher, I can define parameter spaces (grid search, random sampling) to systematically explore configurations without manually creating each parameter set.
  Status: Done

- Story: As a researcher, I can extract comprehensive metrics (conversion, revenue, driver payouts, timing) from simulation results for analysis.
  Status: Done

- Story: As a researcher, I can calculate marketplace health scores using weighted combinations of normalized metrics to evaluate overall marketplace performance.
  Status: Done

- Story: As a researcher, I can export experiment results to Parquet/JSON for external analysis in data science tools.
  Status: Done

- Story: As a researcher, I can find optimal parameter combinations based on health scores to identify the best marketplace configurations.
  Status: Done

- Story: As a researcher, I can deploy the parameter sweep experiment to AWS
  Lambda for serverless POC runs, without requiring a separate Docker container
  for OSRM (using H3 grid routing instead).
  Status: Backlog
