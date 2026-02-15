CREATE EXTERNAL TABLE IF NOT EXISTS ride_sim_analytics.sweep_run_context (
  run_id string,
  request_source string,
  record_schema string,
  request_fingerprint string,
  config_fingerprint string,
  total_points bigint,
  shard_count bigint,
  shard_strategy string,
  max_shards bigint
)
PARTITIONED BY (
  run_date string,
  run_id_partition string,
  status_partition string
)
STORED AS PARQUET
LOCATION 's3://<results-bucket>/serverless-sweeps/outcomes/dataset=run_context/'
TBLPROPERTIES ('projection.enabled'='false');
