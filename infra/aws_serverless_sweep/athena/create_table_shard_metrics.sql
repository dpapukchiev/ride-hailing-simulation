CREATE EXTERNAL TABLE IF NOT EXISTS ride_sim_analytics.sweep_shard_metrics (
  total_riders bigint,
  completed_riders bigint,
  abandoned_quote_riders bigint,
  cancelled_riders bigint,
  conversion_rate double,
  platform_revenue double,
  driver_payouts double,
  total_fares_collected double,
  avg_time_to_match_ms double,
  median_time_to_match_ms double,
  p90_time_to_match_ms double,
  avg_time_to_pickup_ms double,
  median_time_to_pickup_ms double,
  p90_time_to_pickup_ms double,
  completed_trips bigint,
  riders_abandoned_price bigint,
  riders_abandoned_eta bigint,
  riders_abandoned_stochastic bigint
)
PARTITIONED BY (
  run_date string,
  run_id string,
  status string,
  shard_id string,
  point_index string
)
STORED AS PARQUET
LOCATION 's3://<results-bucket>/serverless-sweeps/outcomes/dataset=shard_metrics/'
TBLPROPERTIES ('projection.enabled'='false');
