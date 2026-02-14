CREATE EXTERNAL TABLE IF NOT EXISTS ride_sim_analytics.sweep_shard_outcomes (
  run_id string,
  shard_id bigint,
  status string,
  start_index bigint,
  end_index_exclusive bigint,
  event_time string,
  record_schema string,
  result_key string,
  points_processed bigint,
  format string,
  error_code string,
  error_message string
)
PARTITIONED BY (
  run_date string,
  run_id_partition string,
  status_partition string,
  shard_id_partition string
)
STORED AS PARQUET
LOCATION 's3://<results-bucket>/serverless-sweeps/outcomes/dataset=shard_outcomes/'
TBLPROPERTIES ('projection.enabled'='false');
