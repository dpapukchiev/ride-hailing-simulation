use crate::contract::{NormalizedSweepRequest, ShardAssignment, ValidationError};

pub fn compute_shard_plan(
    request: &NormalizedSweepRequest,
) -> Result<Vec<ShardAssignment>, ValidationError> {
    let total_points = request.total_points;
    let shard_count = match (request.shard_count, request.shard_size) {
        (Some(count), _) => count.min(total_points),
        (None, Some(size)) => total_points.div_ceil(size),
        (None, None) => {
            return Err(ValidationError::new(
                "Either shard_count or shard_size is required",
            ));
        }
    };

    if shard_count == 0 {
        return Err(ValidationError::new("No shards to process"));
    }

    if shard_count > request.max_shards {
        return Err(ValidationError::new(format!(
            "Computed shard count {shard_count} exceeds max_shards={}",
            request.max_shards
        )));
    }

    let base_size = total_points / shard_count;
    let remainder = total_points % shard_count;

    let mut assignments = Vec::with_capacity(shard_count);
    let mut cursor = 0usize;

    for shard_id in 0..shard_count {
        let current_size = base_size + usize::from(shard_id < remainder);
        let start_index = cursor;
        let end_index_exclusive = cursor + current_size;
        assignments.push(ShardAssignment {
            shard_id,
            start_index,
            end_index_exclusive,
        });
        cursor = end_index_exclusive;
    }

    validate_assignments(total_points, &assignments)?;
    Ok(assignments)
}

fn validate_assignments(
    total_points: usize,
    assignments: &[ShardAssignment],
) -> Result<(), ValidationError> {
    if assignments.is_empty() {
        return Err(ValidationError::new("No shards to process"));
    }

    if assignments[0].start_index != 0
        || assignments[assignments.len() - 1].end_index_exclusive != total_points
    {
        return Err(ValidationError::new(
            "Shard boundaries do not cover full parameter space",
        ));
    }

    for idx in 1..assignments.len() {
        if assignments[idx - 1].end_index_exclusive != assignments[idx].start_index {
            return Err(ValidationError::new(
                "Shard boundaries overlap or leave gaps",
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use crate::contract::{normalize_request, SweepRequest};

    use super::*;

    #[test]
    fn compute_shard_plan_is_deterministic_for_identical_input() {
        let request = SweepRequest {
            run_id: "deterministic-001".to_string(),
            dimensions: BTreeMap::from([
                (
                    "num_riders".to_string(),
                    vec![Value::from(100), Value::from(200)],
                ),
                (
                    "commission_rate".to_string(),
                    vec![Value::from(0.1), Value::from(0.2), Value::from(0.3)],
                ),
            ]),
            shard_count: Some(4),
            shard_size: None,
            max_shards: 10,
            seed: 42,
            failure_injection_shards: Vec::new(),
        };

        let normalized = normalize_request(request).expect("request should pass");
        let plan_a = compute_shard_plan(&normalized).expect("plan should pass");
        let plan_b = compute_shard_plan(&normalized).expect("plan should pass");

        assert_eq!(plan_a, plan_b);
        assert_eq!(plan_a[0].start_index, 0);
        assert_eq!(
            plan_a[plan_a.len() - 1].end_index_exclusive,
            normalized.total_points
        );
    }

    #[test]
    fn compute_shard_plan_rejects_excessive_shards() {
        let request = SweepRequest {
            run_id: "too-many-shards".to_string(),
            dimensions: BTreeMap::from([(
                "num_riders".to_string(),
                vec![
                    Value::from(10),
                    Value::from(20),
                    Value::from(30),
                    Value::from(40),
                    Value::from(50),
                ],
            )]),
            shard_count: None,
            shard_size: Some(1),
            max_shards: 2,
            seed: 0,
            failure_injection_shards: Vec::new(),
        };

        let normalized = normalize_request(request).expect("request should pass");
        let error = compute_shard_plan(&normalized).expect_err("plan should fail");
        assert_eq!(
            error.message(),
            format!(
                "Computed shard count {} exceeds max_shards=2",
                normalized.total_points
            )
        );
    }
}
