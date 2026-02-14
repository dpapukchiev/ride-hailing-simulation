SELECT
  run_id_partition AS run_id,
  COUNT(DISTINCT shard_id_partition) AS observed_shards,
  :expected_shards AS expected_shards,
  COUNT(DISTINCT shard_id_partition) = :expected_shards AS has_full_coverage
FROM ride_sim_analytics.sweep_shard_outcomes
WHERE run_id_partition = ':run_id'
GROUP BY run_id_partition;
