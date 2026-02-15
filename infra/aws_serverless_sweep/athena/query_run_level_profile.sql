WITH run_outcomes AS (
  SELECT
    run_id_partition AS run_id,
    status_partition,
    points_processed,
    from_iso8601_timestamp(event_time) AS event_ts
  FROM ride_sim_analytics.sweep_shard_outcomes
  WHERE run_id_partition = ':run_id'
),
run_context AS (
  SELECT
    run_id_partition AS run_id,
    MAX(total_points) AS total_points,
    MAX(shard_count) AS expected_shards
  FROM ride_sim_analytics.sweep_run_context
  WHERE run_id_partition = ':run_id'
    AND status_partition = 'accepted'
  GROUP BY run_id_partition
)
SELECT
  o.run_id,
  COUNT(*) AS shard_attempts,
  SUM(CASE WHEN o.status_partition = 'success' THEN 1 ELSE 0 END) AS successful_shards,
  SUM(CASE WHEN o.status_partition = 'failure' THEN 1 ELSE 0 END) AS failed_shards,
  CAST(SUM(CASE WHEN o.status_partition = 'failure' THEN 1 ELSE 0 END) AS DOUBLE)
    / NULLIF(COUNT(*), 0) AS failure_rate,
  SUM(CASE WHEN o.status_partition = 'success' THEN COALESCE(o.points_processed, 0) ELSE 0 END) AS successful_points,
  CAST(
    date_diff(
      'second',
      MIN(CASE WHEN o.status_partition = 'success' THEN o.event_ts END),
      MAX(CASE WHEN o.status_partition = 'success' THEN o.event_ts END)
    ) AS bigint
  ) AS run_window_seconds,
  CASE
    WHEN SUM(CASE WHEN o.status_partition = 'success' THEN 1 ELSE 0 END) = 0 THEN NULL
    ELSE CAST(
      SUM(CASE WHEN o.status_partition = 'success' THEN COALESCE(o.points_processed, 0) ELSE 0 END)
      AS DOUBLE
    )
      / GREATEST(
        CAST(
          date_diff(
            'second',
            MIN(CASE WHEN o.status_partition = 'success' THEN o.event_ts END),
            MAX(CASE WHEN o.status_partition = 'success' THEN o.event_ts END)
          ) AS DOUBLE
        ),
        1.0
      )
  END AS points_per_second,
  MAX(c.total_points) AS expected_points,
  MAX(c.expected_shards) AS expected_shards
FROM run_outcomes o
LEFT JOIN run_context c ON o.run_id = c.run_id
GROUP BY o.run_id;
