CREATE EXTERNAL TABLE IF NOT EXISTS ride_sim_analytics.sweep_snapshot_counts (
  timestamp_ms bigint,
  riders_browsing bigint,
  riders_waiting bigint,
  riders_in_transit bigint,
  riders_completed bigint,
  riders_cancelled bigint,
  drivers_idle bigint,
  drivers_evaluating bigint,
  drivers_en_route bigint,
  drivers_on_trip bigint,
  drivers_off_duty bigint,
  trips_en_route bigint,
  trips_on_trip bigint,
  trips_completed bigint,
  trips_cancelled bigint
)
PARTITIONED BY (
  run_date string,
  run_id string,
  status string,
  shard_id string,
  point_index string
)
STORED AS PARQUET
LOCATION 's3://<results-bucket>/serverless-sweeps/outcomes/dataset=snapshot_counts/'
TBLPROPERTIES ('projection.enabled'='false');
