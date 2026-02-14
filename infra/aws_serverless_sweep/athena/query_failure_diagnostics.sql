SELECT
  run_id_partition AS run_id,
  error_code,
  COUNT(*) AS occurrences
FROM ride_sim_analytics.sweep_shard_outcomes
WHERE run_id_partition = ':run_id'
  AND status_partition = 'failure'
GROUP BY run_id_partition, error_code
ORDER BY occurrences DESC;
