SELECT
  m.run_id,
  COUNT(*) AS joined_points,
  MIN(rc.request_source) AS request_source,
  MIN(ep.parameter_fingerprint) AS example_parameter_fingerprint
FROM ride_sim_analytics.sweep_shard_metrics m
JOIN ride_sim_analytics.sweep_effective_parameters ep
  ON m.run_id = ep.run_id
 AND CAST(m.shard_id AS bigint) = ep.shard_id
 AND CAST(m.point_index AS bigint) = ep.point_index
 AND m.run_date = ep.run_date
JOIN ride_sim_analytics.sweep_run_context rc
  ON m.run_id = rc.run_id
 AND m.run_date = rc.run_date
WHERE m.run_id = ':run_id'
  AND m.status = 'success'
GROUP BY m.run_id;
