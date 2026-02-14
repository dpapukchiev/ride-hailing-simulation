use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const ORCHESTRATION_SCHEMA_VERSION: &str = "v1";
pub const OUTCOME_RECORD_SCHEMA_VERSION: &str = "v1";
pub const MAX_DIMENSION_VALUES: usize = 10_000;
pub const MAX_TOTAL_PARAMETER_POINTS: usize = 200_000;
pub const DEFAULT_MAX_SHARDS: usize = 1_000;

pub type Dimensions = BTreeMap<String, Vec<Value>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunContext {
    pub run_id: String,
    pub schema_version: String,
    pub request_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShardAssignment {
    pub shard_id: usize,
    pub start_index: usize,
    pub end_index_exclusive: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SweepRequest {
    pub run_id: String,
    pub dimensions: Dimensions,
    pub shard_count: Option<usize>,
    pub shard_size: Option<usize>,
    #[serde(default = "default_max_shards")]
    pub max_shards: usize,
    #[serde(default)]
    pub seed: i64,
    #[serde(default)]
    pub failure_injection_shards: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NormalizedSweepRequest {
    pub run_id: String,
    pub dimensions: Dimensions,
    pub total_points: usize,
    pub shard_count: Option<usize>,
    pub shard_size: Option<usize>,
    pub max_shards: usize,
    pub seed: i64,
    pub failure_injection_shards: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChildShardPayload {
    pub run_id: String,
    pub dimensions: Dimensions,
    pub total_points: usize,
    pub shard_id: usize,
    pub start_index: usize,
    pub end_index_exclusive: usize,
    pub seed: i64,
    pub failure_injection_shards: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispatchRecord {
    pub shard_id: usize,
    pub status_code: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParentAcceptedResponse {
    pub run_id: String,
    pub total_points: usize,
    pub shards_dispatched: usize,
    pub dispatches: Vec<DispatchRecord>,
    pub status: String,
    pub schema_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutcomeError {
    pub error_code: String,
    pub error_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShardOutputMetadata {
    pub result_key: String,
    pub points_processed: usize,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShardOutcomeRecord {
    pub run_id: String,
    pub shard_id: usize,
    pub status: String,
    pub start_index: usize,
    pub end_index_exclusive: usize,
    pub event_time: String,
    pub record_schema: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_metadata: Option<ShardOutputMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<OutcomeError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    message: String,
}

impl ValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ValidationError {}

pub fn default_max_shards() -> usize {
    DEFAULT_MAX_SHARDS
}

pub fn normalize_request(payload: SweepRequest) -> Result<NormalizedSweepRequest, ValidationError> {
    let run_id = payload.run_id.trim().to_string();
    if run_id.is_empty() {
        return Err(ValidationError::new("run_id cannot be empty"));
    }

    if payload.dimensions.is_empty() {
        return Err(ValidationError::new("dimensions cannot be empty"));
    }

    let mut total_points = 1usize;
    for (name, values) in &payload.dimensions {
        if name.trim().is_empty() {
            return Err(ValidationError::new(
                "dimension names must be non-empty strings",
            ));
        }
        if values.is_empty() {
            return Err(ValidationError::new(format!(
                "Dimension '{name}' must be a non-empty list"
            )));
        }
        if values.len() > MAX_DIMENSION_VALUES {
            return Err(ValidationError::new(format!(
                "Dimension '{name}' exceeds MAX_DIMENSION_VALUES={MAX_DIMENSION_VALUES}"
            )));
        }
        total_points = total_points.saturating_mul(values.len());
        if total_points > MAX_TOTAL_PARAMETER_POINTS {
            return Err(ValidationError::new(format!(
                "Parameter space is too large for this deployment (>{MAX_TOTAL_PARAMETER_POINTS} points)"
            )));
        }
    }

    if payload.shard_count.is_none() && payload.shard_size.is_none() {
        return Err(ValidationError::new(
            "Either shard_count or shard_size is required",
        ));
    }

    if let Some(0) = payload.shard_count {
        return Err(ValidationError::new(
            "shard_count must be a positive integer",
        ));
    }

    if let Some(0) = payload.shard_size {
        return Err(ValidationError::new(
            "shard_size must be a positive integer",
        ));
    }

    if payload.max_shards == 0 {
        return Err(ValidationError::new(
            "max_shards must be a positive integer",
        ));
    }

    let mut failure_injection_shards = payload.failure_injection_shards;
    failure_injection_shards.sort_unstable();
    failure_injection_shards.dedup();

    Ok(NormalizedSweepRequest {
        run_id,
        dimensions: payload.dimensions,
        total_points,
        shard_count: payload.shard_count,
        shard_size: payload.shard_size,
        max_shards: payload.max_shards,
        seed: payload.seed,
        failure_injection_shards,
    })
}

pub fn request_fingerprint(request: &NormalizedSweepRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(stable_contract_json(request));
    format!("{:x}", hasher.finalize())
}

pub fn stable_contract_json(value: impl Serialize) -> String {
    serde_json::to_string(&value).expect("serialization of contract value should not fail")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_request_rejects_empty_run_id() {
        let request = SweepRequest {
            run_id: " ".to_string(),
            dimensions: BTreeMap::from([("num_riders".to_string(), vec![Value::from(100)])]),
            shard_count: Some(1),
            shard_size: None,
            max_shards: 10,
            seed: 0,
            failure_injection_shards: Vec::new(),
        };

        let error = normalize_request(request).expect_err("request should fail");
        assert_eq!(error.message(), "run_id cannot be empty");
    }

    #[test]
    fn normalize_request_sorts_and_deduplicates_failures() {
        let request = SweepRequest {
            run_id: "run-1".to_string(),
            dimensions: BTreeMap::from([
                (
                    "num_riders".to_string(),
                    vec![Value::from(100), Value::from(200)],
                ),
                ("num_drivers".to_string(), vec![Value::from(20)]),
            ]),
            shard_count: Some(2),
            shard_size: None,
            max_shards: 10,
            seed: 7,
            failure_injection_shards: vec![3, 1, 3],
        };

        let normalized = normalize_request(request).expect("request should pass");
        assert_eq!(normalized.total_points, 2);
        assert_eq!(normalized.failure_injection_shards, vec![1, 3]);
    }
}
