CREATE EXTERNAL TABLE IF NOT EXISTS ride_sim_analytics.sweep_trip_data (
  trip_entity bigint,
  rider_entity bigint,
  driver_entity bigint,
  state tinyint,
  pickup_cell bigint,
  dropoff_cell bigint,
  pickup_distance_km_at_accept double,
  requested_at bigint,
  matched_at bigint,
  pickup_at bigint,
  dropoff_at bigint,
  cancelled_at bigint
)
PARTITIONED BY (
  run_date string,
  run_id string,
  status string,
  shard_id string,
  point_index string
)
STORED AS PARQUET
LOCATION 's3://<results-bucket>/serverless-sweeps/outcomes/dataset=trip_data/'
TBLPROPERTIES ('projection.enabled'='false');
