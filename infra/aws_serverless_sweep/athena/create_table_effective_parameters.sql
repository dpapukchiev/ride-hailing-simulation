CREATE EXTERNAL TABLE IF NOT EXISTS ride_sim_analytics.sweep_effective_parameters (
  run_id string,
  shard_id bigint,
  point_index bigint,
  status string,
  record_schema string,
  parameter_fingerprint string,
  effective_parameters_json string
)
PARTITIONED BY (
  run_date string,
  run_id_partition string,
  status_partition string,
  shard_id_partition string,
  point_index_partition string
)
STORED AS PARQUET
LOCATION 's3://<results-bucket>/serverless-sweeps/outcomes/dataset=effective_parameters/'
TBLPROPERTIES ('projection.enabled'='false');
