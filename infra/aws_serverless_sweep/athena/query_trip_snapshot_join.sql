SELECT
  m.run_id,
  m.shard_id,
  m.point_index,
  m.completed_trips,
  COUNT(t.trip_entity) AS trip_rows,
  MAX(s.trips_completed) AS peak_completed_in_snapshot
FROM ride_sim_analytics.sweep_shard_metrics m
LEFT JOIN ride_sim_analytics.sweep_trip_data t
  ON m.run_id = t.run_id
 AND m.shard_id = t.shard_id
 AND m.point_index = t.point_index
 AND m.run_date = t.run_date
LEFT JOIN ride_sim_analytics.sweep_snapshot_counts s
  ON m.run_id = s.run_id
 AND m.shard_id = s.shard_id
 AND m.point_index = s.point_index
 AND m.run_date = s.run_date
WHERE m.run_id = ':run_id'
  AND m.status = 'success'
GROUP BY m.run_id, m.shard_id, m.point_index, m.completed_trips
ORDER BY m.shard_id, m.point_index;
