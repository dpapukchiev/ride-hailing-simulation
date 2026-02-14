#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatasetKind {
    ShardMetrics,
    TripData,
    SnapshotCounts,
    ShardOutcomes,
}

impl DatasetKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ShardMetrics => "shard_metrics",
            Self::TripData => "trip_data",
            Self::SnapshotCounts => "snapshot_counts",
            Self::ShardOutcomes => "shard_outcomes",
        }
    }
}

pub fn partition_prefix(
    base_prefix: &str,
    dataset: DatasetKind,
    run_date: &str,
    run_id: &str,
    status: &str,
) -> String {
    let trimmed = base_prefix.trim_matches('/');
    format!(
        "{trimmed}/dataset={}/run_date={run_date}/run_id={run_id}/status={status}",
        dataset.as_str(),
    )
}

pub fn metrics_object_key(
    base_prefix: &str,
    run_date: &str,
    run_id: &str,
    status: &str,
    shard_id: usize,
    point_index: usize,
) -> String {
    format!(
        "{}/shard_id={shard_id}/point_index={point_index}/part-0.parquet",
        partition_prefix(
            base_prefix,
            DatasetKind::ShardMetrics,
            run_date,
            run_id,
            status
        ),
    )
}

pub fn trip_data_object_key(
    base_prefix: &str,
    run_date: &str,
    run_id: &str,
    status: &str,
    shard_id: usize,
    point_index: usize,
) -> String {
    format!(
        "{}/shard_id={shard_id}/point_index={point_index}/part-0.parquet",
        partition_prefix(base_prefix, DatasetKind::TripData, run_date, run_id, status,),
    )
}

pub fn snapshot_counts_object_key(
    base_prefix: &str,
    run_date: &str,
    run_id: &str,
    status: &str,
    shard_id: usize,
    point_index: usize,
) -> String {
    format!(
        "{}/shard_id={shard_id}/point_index={point_index}/part-0.parquet",
        partition_prefix(
            base_prefix,
            DatasetKind::SnapshotCounts,
            run_date,
            run_id,
            status,
        ),
    )
}

pub fn success_outcome_object_key(
    base_prefix: &str,
    run_date: &str,
    run_id: &str,
    shard_id: usize,
) -> String {
    format!(
        "{}/shard_id={shard_id}/part-0.parquet",
        partition_prefix(
            base_prefix,
            DatasetKind::ShardOutcomes,
            run_date,
            run_id,
            "success",
        ),
    )
}

pub fn error_object_key(
    base_prefix: &str,
    run_date: &str,
    run_id: &str,
    shard_id: usize,
) -> String {
    format!(
        "{}/shard_id={shard_id}/part-0.parquet",
        partition_prefix(
            base_prefix,
            DatasetKind::ShardOutcomes,
            run_date,
            run_id,
            "failure",
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_metrics_key_with_expected_partitions() {
        let key = metrics_object_key(
            "serverless-sweeps/outcomes/",
            "2026-02-14",
            "run-123",
            "success",
            4,
            9,
        );

        assert_eq!(
            key,
            "serverless-sweeps/outcomes/dataset=shard_metrics/run_date=2026-02-14/run_id=run-123/status=success/shard_id=4/point_index=9/part-0.parquet"
        );
    }

    #[test]
    fn builds_failure_error_key() {
        let key = error_object_key("outcomes", "2026-02-14", "run-123", 7);
        assert_eq!(
            key,
            "outcomes/dataset=shard_outcomes/run_date=2026-02-14/run_id=run-123/status=failure/shard_id=7/part-0.parquet"
        );
    }

    #[test]
    fn builds_success_outcome_key() {
        let key = success_outcome_object_key("outcomes", "2026-02-14", "run-123", 4);
        assert_eq!(
            key,
            "outcomes/dataset=shard_outcomes/run_date=2026-02-14/run_id=run-123/status=success/shard_id=4/part-0.parquet"
        );
    }

    #[test]
    fn builds_trip_data_key_with_point_partition() {
        let key = trip_data_object_key("outcomes", "2026-02-14", "run-123", "success", 2, 11);
        assert_eq!(
            key,
            "outcomes/dataset=trip_data/run_date=2026-02-14/run_id=run-123/status=success/shard_id=2/point_index=11/part-0.parquet"
        );
    }

    #[test]
    fn builds_snapshot_counts_key_with_point_partition() {
        let key = snapshot_counts_object_key("outcomes", "2026-02-14", "run-123", "success", 2, 11);
        assert_eq!(
            key,
            "outcomes/dataset=snapshot_counts/run_date=2026-02-14/run_id=run-123/status=success/shard_id=2/point_index=11/part-0.parquet"
        );
    }
}
