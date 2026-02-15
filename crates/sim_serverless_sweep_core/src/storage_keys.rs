#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatasetKind {
    ShardMetrics,
    TripData,
    SnapshotCounts,
    ShardOutcomes,
    RunContext,
    EffectiveParameters,
}

impl DatasetKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ShardMetrics => "shard_metrics",
            Self::TripData => "trip_data",
            Self::SnapshotCounts => "snapshot_counts",
            Self::ShardOutcomes => "shard_outcomes",
            Self::RunContext => "run_context",
            Self::EffectiveParameters => "effective_parameters",
        }
    }

    fn uses_partition_suffix_keys(self) -> bool {
        matches!(
            self,
            Self::ShardOutcomes | Self::RunContext | Self::EffectiveParameters
        )
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
    let run_id_key = if dataset.uses_partition_suffix_keys() {
        "run_id_partition"
    } else {
        "run_id"
    };
    let status_key = if dataset.uses_partition_suffix_keys() {
        "status_partition"
    } else {
        "status"
    };
    format!(
        "{trimmed}/dataset={}/run_date={run_date}/{run_id_key}={run_id}/{status_key}={status}",
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
        "{}/shard_id_partition={shard_id}/part-0.parquet",
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
        "{}/shard_id_partition={shard_id}/part-0.parquet",
        partition_prefix(
            base_prefix,
            DatasetKind::ShardOutcomes,
            run_date,
            run_id,
            "failure",
        ),
    )
}

pub fn run_context_object_key(
    base_prefix: &str,
    run_date: &str,
    run_id: &str,
    status: &str,
) -> String {
    format!(
        "{}/part-0.parquet",
        partition_prefix(
            base_prefix,
            DatasetKind::RunContext,
            run_date,
            run_id,
            status,
        )
    )
}

pub fn effective_parameters_object_key(
    base_prefix: &str,
    run_date: &str,
    run_id: &str,
    status: &str,
    shard_id: usize,
    point_index: usize,
) -> String {
    format!(
        "{}/shard_id_partition={shard_id}/point_index_partition={point_index}/part-0.parquet",
        partition_prefix(
            base_prefix,
            DatasetKind::EffectiveParameters,
            run_date,
            run_id,
            status,
        )
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
            "outcomes/dataset=shard_outcomes/run_date=2026-02-14/run_id_partition=run-123/status_partition=failure/shard_id_partition=7/part-0.parquet"
        );
    }

    #[test]
    fn builds_success_outcome_key() {
        let key = success_outcome_object_key("outcomes", "2026-02-14", "run-123", 4);
        assert_eq!(
            key,
            "outcomes/dataset=shard_outcomes/run_date=2026-02-14/run_id_partition=run-123/status_partition=success/shard_id_partition=4/part-0.parquet"
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

    #[test]
    fn builds_run_context_key() {
        let key = run_context_object_key("outcomes", "2026-02-14", "run-123", "accepted");
        assert_eq!(
            key,
            "outcomes/dataset=run_context/run_date=2026-02-14/run_id_partition=run-123/status_partition=accepted/part-0.parquet"
        );
    }

    #[test]
    fn builds_effective_parameter_key_with_point_partition() {
        let key =
            effective_parameters_object_key("outcomes", "2026-02-14", "run-123", "success", 2, 11);
        assert_eq!(
            key,
            "outcomes/dataset=effective_parameters/run_date=2026-02-14/run_id_partition=run-123/status_partition=success/shard_id_partition=2/point_index_partition=11/part-0.parquet"
        );
    }
}
