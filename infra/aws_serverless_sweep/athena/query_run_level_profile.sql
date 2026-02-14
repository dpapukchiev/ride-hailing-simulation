SELECT
  run_id_partition AS run_id,
  COUNT(*) AS shard_attempts,
  SUM(CASE WHEN status_partition = 'success' THEN 1 ELSE 0 END) AS successful_shards,
  SUM(CASE WHEN status_partition = 'failure' THEN 1 ELSE 0 END) AS failed_shards,
  CAST(SUM(CASE WHEN status_partition = 'failure' THEN 1 ELSE 0 END) AS DOUBLE)
    / NULLIF(COUNT(*), 0) AS failure_rate
FROM ride_sim_analytics.sweep_shard_outcomes
WHERE run_id_partition = ':run_id'
GROUP BY run_id_partition;
