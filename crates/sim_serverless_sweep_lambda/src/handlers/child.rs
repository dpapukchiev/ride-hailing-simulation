use std::fs;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::runtime::contract::{
    ChildShardPayload, EffectiveParameterRecord, OutcomeError, ShardOutcomeRecord,
    ShardOutputMetadata, EFFECTIVE_PARAMETER_RECORD_SCHEMA_VERSION, OUTCOME_RECORD_SCHEMA_VERSION,
};
use crate::runtime::storage_keys::{
    effective_parameters_object_key, error_object_key, metrics_object_key,
    snapshot_counts_object_key, success_outcome_object_key, trip_data_object_key,
};
use arrow::array::{ArrayRef, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sim_experiments::{export_to_parquet, SimulationResult};

use crate::adapters::object_store::OutcomeStore;
use crate::adapters::shard_execution::SimExperimentsShardExecutor;

#[derive(Debug, Clone)]
pub struct ShardPointResult {
    pub point_index: usize,
    pub metrics: SimulationResult,
    pub trip_data_parquet: Vec<u8>,
    pub snapshot_counts_parquet: Vec<u8>,
    pub effective_parameters_json: String,
    pub parameter_fingerprint: String,
}

pub trait ShardExecutor {
    fn execute_shard(
        &self,
        payload: &ChildShardPayload,
        on_point_result: &mut dyn FnMut(ShardPointResult) -> Result<(), String>,
    ) -> Result<usize, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildHandlerConfig {
    pub bucket: String,
    pub prefix: String,
    pub run_date: String,
    pub event_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChildSuccessResponse {
    pub status: String,
    pub shard_id: usize,
    pub outcome_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildHandlerError {
    pub message: String,
    pub failure_key: Option<String>,
}

pub fn handle_child_payload(
    payload: &ChildShardPayload,
    config: &ChildHandlerConfig,
    executor: &impl ShardExecutor,
    outcome_store: &impl OutcomeStore,
) -> Result<ChildSuccessResponse, ChildHandlerError> {
    let started_at = Instant::now();
    log_child_info(
        "shard_started",
        json!({
            "run_id": payload.run_id.clone(),
            "shard_id": payload.shard_id,
            "start_index": payload.start_index,
            "end_index_exclusive": payload.end_index_exclusive,
            "planned_points": payload.end_index_exclusive.saturating_sub(payload.start_index),
        }),
    );

    if payload.start_index >= payload.end_index_exclusive {
        return Err(ChildHandlerError {
            message: "Invalid shard bounds".to_string(),
            failure_key: None,
        });
    }

    if payload.shard_id >= payload.total_points {
        return Err(ChildHandlerError {
            message: "shard_id must be less than total_points".to_string(),
            failure_key: None,
        });
    }

    match write_success(payload, config, executor, outcome_store) {
        Ok((response, points_processed)) => {
            let elapsed_ms = started_at.elapsed().as_millis();
            let points_per_second = if elapsed_ms == 0 {
                points_processed as f64
            } else {
                (points_processed as f64) / ((elapsed_ms as f64) / 1_000.0)
            };
            log_child_info(
                "shard_completed",
                json!({
                    "run_id": payload.run_id.clone(),
                    "shard_id": payload.shard_id,
                    "points_processed": points_processed,
                    "duration_ms": elapsed_ms,
                    "points_per_second": points_per_second,
                    "outcome_key": response.outcome_key.clone(),
                }),
            );
            Ok(response)
        }
        Err(success_error) => {
            let error_message = success_error.message;
            write_failure(payload, config, &error_message, outcome_store)?;
            log_child_error(
                "shard_failed",
                json!({
                    "run_id": payload.run_id.clone(),
                    "shard_id": payload.shard_id,
                    "duration_ms": started_at.elapsed().as_millis(),
                    "error": error_message.clone(),
                }),
            );
            Err(ChildHandlerError {
                message: error_message,
                failure_key: Some(error_object_key(
                    &config.prefix,
                    &config.run_date,
                    &payload.run_id,
                    payload.shard_id,
                )),
            })
        }
    }
}

pub fn handle_child_payload_with_sim_runtime(
    payload: &ChildShardPayload,
    config: &ChildHandlerConfig,
    outcome_store: &impl OutcomeStore,
) -> Result<ChildSuccessResponse, ChildHandlerError> {
    handle_child_payload(payload, config, &SimExperimentsShardExecutor, outcome_store)
}

fn write_success(
    payload: &ChildShardPayload,
    config: &ChildHandlerConfig,
    executor: &impl ShardExecutor,
    outcome_store: &impl OutcomeStore,
) -> Result<(ChildSuccessResponse, usize), ChildHandlerError> {
    let mut first_metrics_key: Option<String> = None;

    let mut on_point_result = |point_result: ShardPointResult| -> Result<(), String> {
        let metrics_key = metrics_object_key(
            &config.prefix,
            &config.run_date,
            &payload.run_id,
            "success",
            payload.shard_id,
            point_result.point_index,
        );
        let trip_data_key = trip_data_object_key(
            &config.prefix,
            &config.run_date,
            &payload.run_id,
            "success",
            payload.shard_id,
            point_result.point_index,
        );
        let snapshot_counts_key = snapshot_counts_object_key(
            &config.prefix,
            &config.run_date,
            &payload.run_id,
            "success",
            payload.shard_id,
            point_result.point_index,
        );
        let effective_parameters_key = effective_parameters_object_key(
            &config.prefix,
            &config.run_date,
            &payload.run_id,
            "success",
            payload.shard_id,
            point_result.point_index,
        );

        let parquet_body = serialize_metrics_parquet(
            std::slice::from_ref(&point_result.metrics),
            &payload.run_id,
            payload.shard_id,
        )
        .map_err(|error| format!("Failed to serialize shard metrics to parquet: {error}"))?;

        outcome_store
            .write_object(&metrics_key, &parquet_body)
            .map_err(|error| format!("Failed to persist shard metrics artifact: {error}"))?;

        outcome_store
            .write_object(&trip_data_key, &point_result.trip_data_parquet)
            .map_err(|error| format!("Failed to persist trip data artifact: {error}"))?;

        outcome_store
            .write_object(&snapshot_counts_key, &point_result.snapshot_counts_parquet)
            .map_err(|error| format!("Failed to persist snapshot counts artifact: {error}"))?;

        let effective_parameters_record = EffectiveParameterRecord {
            run_id: payload.run_id.clone(),
            shard_id: payload.shard_id,
            point_index: point_result.point_index,
            status: "success".to_string(),
            record_schema: EFFECTIVE_PARAMETER_RECORD_SCHEMA_VERSION.to_string(),
            parameter_fingerprint: point_result.parameter_fingerprint,
            effective_parameters_json: point_result.effective_parameters_json,
        };
        let effective_parameters_body = serialize_effective_parameters_parquet(
            &effective_parameters_record,
        )
        .map_err(|error| format!("Failed to serialize effective-parameter parquet: {error}"))?;
        outcome_store
            .write_object(&effective_parameters_key, &effective_parameters_body)
            .map_err(|error| format!("Failed to persist effective-parameter artifact: {error}"))?;

        if first_metrics_key.is_none() {
            first_metrics_key = Some(metrics_key);
        }

        Ok(())
    };

    let points_processed = executor
        .execute_shard(payload, &mut on_point_result)
        .map_err(|error| ChildHandlerError {
            message: error,
            failure_key: None,
        })?;

    let metrics_prefix = first_metrics_key.unwrap_or_else(|| {
        metrics_object_key(
            &config.prefix,
            &config.run_date,
            &payload.run_id,
            "success",
            payload.shard_id,
            payload.start_index,
        )
    });

    let outcome_key = success_outcome_object_key(
        &config.prefix,
        &config.run_date,
        &payload.run_id,
        payload.shard_id,
    );

    let record = ShardOutcomeRecord {
        run_id: payload.run_id.clone(),
        shard_id: payload.shard_id,
        status: "success".to_string(),
        start_index: payload.start_index,
        end_index_exclusive: payload.end_index_exclusive,
        event_time: config.event_time.clone(),
        record_schema: OUTCOME_RECORD_SCHEMA_VERSION.to_string(),
        output_metadata: Some(ShardOutputMetadata {
            result_key: metrics_prefix,
            points_processed,
            format: "parquet".to_string(),
        }),
        error: None,
    };

    let body = serialize_outcome_parquet(&record).map_err(|error| ChildHandlerError {
        message: format!("Failed to serialize success outcome parquet: {error}"),
        failure_key: None,
    })?;

    outcome_store
        .write_object(&outcome_key, &body)
        .map_err(|error| ChildHandlerError {
            message: format!("Failed to persist success outcome: {error}"),
            failure_key: None,
        })?;

    Ok((
        ChildSuccessResponse {
            status: "ok".to_string(),
            shard_id: payload.shard_id,
            outcome_key,
        },
        points_processed,
    ))
}

fn log_child_info(event: &str, details: serde_json::Value) {
    eprintln!(
        "{}",
        json!({
            "component": "child_handler",
            "event": event,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "details": details,
        })
    );
}

fn log_child_error(event: &str, details: serde_json::Value) {
    eprintln!(
        "{}",
        json!({
            "component": "child_handler",
            "level": "error",
            "event": event,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "details": details,
        })
    );
}

fn serialize_metrics_parquet(
    results: &[SimulationResult],
    run_id: &str,
    shard_id: usize,
) -> Result<Vec<u8>, String> {
    if results.is_empty() {
        return Err("Shard metrics cannot be empty for a successful shard".to_string());
    }

    let mut temp_path = std::env::temp_dir();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Failed to read clock for parquet export: {error}"))?
        .as_nanos();
    temp_path.push(format!(
        "serverless-shard-{run_id}-{shard_id}-{timestamp}.parquet"
    ));

    export_to_parquet(results, &temp_path)
        .map_err(|error| format!("Parquet export failed: {error}"))?;
    let bytes = fs::read(&temp_path)
        .map_err(|error| format!("Failed to read exported parquet file: {error}"))?;
    let _ = fs::remove_file(&temp_path);
    Ok(bytes)
}

fn serialize_outcome_parquet(record: &ShardOutcomeRecord) -> Result<Vec<u8>, String> {
    let mut temp_path = std::env::temp_dir();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Failed to read clock for parquet export: {error}"))?
        .as_nanos();
    temp_path.push(format!("serverless-outcome-{timestamp}.parquet"));

    let schema = std::sync::Arc::new(Schema::new(vec![
        Field::new("run_id", DataType::Utf8, false),
        Field::new("shard_id", DataType::UInt64, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("start_index", DataType::UInt64, false),
        Field::new("end_index_exclusive", DataType::UInt64, false),
        Field::new("event_time", DataType::Utf8, false),
        Field::new("record_schema", DataType::Utf8, false),
        Field::new("result_key", DataType::Utf8, true),
        Field::new("points_processed", DataType::UInt64, true),
        Field::new("format", DataType::Utf8, true),
        Field::new("error_code", DataType::Utf8, true),
        Field::new("error_message", DataType::Utf8, true),
    ]));

    let points_processed = record
        .output_metadata
        .as_ref()
        .and_then(|meta| u64::try_from(meta.points_processed).ok());

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            std::sync::Arc::new(StringArray::from(vec![record.run_id.clone()])) as ArrayRef,
            std::sync::Arc::new(UInt64Array::from(vec![record.shard_id as u64])),
            std::sync::Arc::new(StringArray::from(vec![record.status.clone()])),
            std::sync::Arc::new(UInt64Array::from(vec![record.start_index as u64])),
            std::sync::Arc::new(UInt64Array::from(vec![record.end_index_exclusive as u64])),
            std::sync::Arc::new(StringArray::from(vec![record.event_time.clone()])),
            std::sync::Arc::new(StringArray::from(vec![record.record_schema.clone()])),
            std::sync::Arc::new(StringArray::from(vec![record
                .output_metadata
                .as_ref()
                .map(|meta| meta.result_key.clone())])),
            std::sync::Arc::new(UInt64Array::from(vec![points_processed])),
            std::sync::Arc::new(StringArray::from(vec![record
                .output_metadata
                .as_ref()
                .map(|meta| meta.format.clone())])),
            std::sync::Arc::new(StringArray::from(vec![record
                .error
                .as_ref()
                .map(|error| error.error_code.clone())])),
            std::sync::Arc::new(StringArray::from(vec![record
                .error
                .as_ref()
                .map(|error| error.error_message.clone())])),
        ],
    )
    .map_err(|error| format!("Failed to build outcome parquet record batch: {error}"))?;

    let file = fs::File::create(&temp_path)
        .map_err(|error| format!("Failed to create temporary parquet file: {error}"))?;
    let props = WriterProperties::builder().build();
    let mut writer =
        ArrowWriter::try_new(file, schema, Some(props)).map_err(|error| error.to_string())?;
    writer
        .write(&batch)
        .map_err(|error| format!("Failed to write outcome parquet batch: {error}"))?;
    writer
        .close()
        .map_err(|error| format!("Failed to close outcome parquet writer: {error}"))?;

    let bytes = fs::read(&temp_path)
        .map_err(|error| format!("Failed to read exported parquet file: {error}"))?;
    let _ = fs::remove_file(&temp_path);
    Ok(bytes)
}

fn serialize_effective_parameters_parquet(
    record: &EffectiveParameterRecord,
) -> Result<Vec<u8>, String> {
    let mut temp_path = std::env::temp_dir();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Failed to read clock for parquet export: {error}"))?
        .as_nanos();
    temp_path.push(format!(
        "serverless-effective-parameters-{timestamp}.parquet"
    ));

    let schema = std::sync::Arc::new(Schema::new(vec![
        Field::new("run_id", DataType::Utf8, false),
        Field::new("shard_id", DataType::UInt64, false),
        Field::new("point_index", DataType::UInt64, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("record_schema", DataType::Utf8, false),
        Field::new("parameter_fingerprint", DataType::Utf8, false),
        Field::new("effective_parameters_json", DataType::Utf8, false),
    ]));

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            std::sync::Arc::new(StringArray::from(vec![record.run_id.clone()])) as ArrayRef,
            std::sync::Arc::new(UInt64Array::from(vec![record.shard_id as u64])),
            std::sync::Arc::new(UInt64Array::from(vec![record.point_index as u64])),
            std::sync::Arc::new(StringArray::from(vec![record.status.clone()])),
            std::sync::Arc::new(StringArray::from(vec![record.record_schema.clone()])),
            std::sync::Arc::new(StringArray::from(vec![record
                .parameter_fingerprint
                .clone()])),
            std::sync::Arc::new(StringArray::from(vec![record
                .effective_parameters_json
                .clone()])),
        ],
    )
    .map_err(|error| {
        format!("Failed to build effective-parameter parquet record batch: {error}")
    })?;

    let file = fs::File::create(&temp_path)
        .map_err(|error| format!("Failed to create temporary parquet file: {error}"))?;
    let props = WriterProperties::builder().build();
    let mut writer =
        ArrowWriter::try_new(file, schema, Some(props)).map_err(|error| error.to_string())?;
    writer
        .write(&batch)
        .map_err(|error| format!("Failed to write effective-parameter parquet batch: {error}"))?;
    writer
        .close()
        .map_err(|error| format!("Failed to close effective-parameter parquet writer: {error}"))?;

    let bytes = fs::read(&temp_path)
        .map_err(|error| format!("Failed to read exported parquet file: {error}"))?;
    let _ = fs::remove_file(&temp_path);
    Ok(bytes)
}

fn write_failure(
    payload: &ChildShardPayload,
    config: &ChildHandlerConfig,
    error_message: &str,
    outcome_store: &impl OutcomeStore,
) -> Result<(), ChildHandlerError> {
    let key = error_object_key(
        &config.prefix,
        &config.run_date,
        &payload.run_id,
        payload.shard_id,
    );

    let record = ShardOutcomeRecord {
        run_id: payload.run_id.clone(),
        shard_id: payload.shard_id,
        status: "failure".to_string(),
        start_index: payload.start_index,
        end_index_exclusive: payload.end_index_exclusive,
        event_time: config.event_time.clone(),
        record_schema: OUTCOME_RECORD_SCHEMA_VERSION.to_string(),
        output_metadata: None,
        error: Some(OutcomeError {
            error_code: "runtime_error".to_string(),
            error_message: error_message.to_string(),
        }),
    };

    let body = serialize_outcome_parquet(&record).map_err(|error| ChildHandlerError {
        message: format!("Failed to serialize failure outcome parquet: {error}"),
        failure_key: Some(key.clone()),
    })?;

    outcome_store
        .write_object(&key, &body)
        .map_err(|error| ChildHandlerError {
            message: format!("Failed to persist failure outcome: {error}"),
            failure_key: Some(key),
        })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::sync::Mutex;

    use serde_json::Value;

    use super::*;

    struct RecordingStore {
        writes: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl RecordingStore {
        fn new() -> Self {
            Self {
                writes: Mutex::new(HashMap::new()),
            }
        }

        fn keys(&self) -> Vec<String> {
            self.writes
                .lock()
                .expect("poisoned mutex")
                .keys()
                .cloned()
                .collect()
        }

        fn body(&self, key: &str) -> Option<Vec<u8>> {
            self.writes
                .lock()
                .expect("poisoned mutex")
                .get(key)
                .cloned()
        }

        fn seed_object(&self, key: &str, body: &[u8]) {
            self.writes
                .lock()
                .expect("poisoned mutex")
                .insert(key.to_string(), body.to_vec());
        }
    }

    impl OutcomeStore for RecordingStore {
        fn write_object(&self, key: &str, body: &[u8]) -> Result<(), String> {
            self.writes
                .lock()
                .expect("poisoned mutex")
                .insert(key.to_string(), body.to_vec());
            Ok(())
        }
    }

    struct PassExecutor;

    impl ShardExecutor for PassExecutor {
        fn execute_shard(
            &self,
            payload: &ChildShardPayload,
            on_point_result: &mut dyn FnMut(ShardPointResult) -> Result<(), String>,
        ) -> Result<usize, String> {
            let mut points_processed = 0usize;
            for point_index in payload.start_index..payload.end_index_exclusive {
                on_point_result(ShardPointResult {
                    point_index,
                    metrics: sample_simulation_result(),
                    trip_data_parquet: b"PAR1-trip".to_vec(),
                    snapshot_counts_parquet: b"PAR1-snap".to_vec(),
                    effective_parameters_json: format!("{{\"point_index\":{point_index}}}"),
                    parameter_fingerprint: format!("fingerprint-{point_index}"),
                })?;
                points_processed += 1;
            }

            Ok(points_processed)
        }
    }

    struct FailingExecutor;

    impl ShardExecutor for FailingExecutor {
        fn execute_shard(
            &self,
            _payload: &ChildShardPayload,
            _on_point_result: &mut dyn FnMut(ShardPointResult) -> Result<(), String>,
        ) -> Result<usize, String> {
            Err("Injected shard failure for verification".to_string())
        }
    }

    struct SelectiveFailStore {
        writes: Mutex<HashMap<String, Vec<u8>>>,
        denied_suffix: &'static str,
    }

    impl SelectiveFailStore {
        fn new(denied_suffix: &'static str) -> Self {
            Self {
                writes: Mutex::new(HashMap::new()),
                denied_suffix,
            }
        }

        fn keys(&self) -> Vec<String> {
            self.writes
                .lock()
                .expect("poisoned mutex")
                .keys()
                .cloned()
                .collect()
        }

        fn body(&self, key: &str) -> Option<Vec<u8>> {
            self.writes
                .lock()
                .expect("poisoned mutex")
                .get(key)
                .cloned()
        }
    }

    impl OutcomeStore for SelectiveFailStore {
        fn write_object(&self, key: &str, body: &[u8]) -> Result<(), String> {
            if key.ends_with(self.denied_suffix) {
                return Err(format!("simulated write failure for key: {key}"));
            }

            self.writes
                .lock()
                .expect("poisoned mutex")
                .insert(key.to_string(), body.to_vec());
            Ok(())
        }
    }

    fn sample_simulation_result() -> SimulationResult {
        SimulationResult {
            total_riders: 100,
            total_drivers: 20,
            completed_riders: 80,
            abandoned_quote_riders: 10,
            cancelled_riders: 10,
            conversion_rate: 0.8,
            platform_revenue: 1000.0,
            driver_payouts: 5000.0,
            total_fares_collected: 6000.0,
            avg_time_to_match_ms: 1000.0,
            median_time_to_match_ms: 1000.0,
            p90_time_to_match_ms: 2000.0,
            avg_time_to_pickup_ms: 5000.0,
            median_time_to_pickup_ms: 5000.0,
            p90_time_to_pickup_ms: 10000.0,
            completed_trips: 80,
            riders_abandoned_price: 5,
            riders_abandoned_eta: 3,
            riders_abandoned_stochastic: 2,
        }
    }

    fn sample_payload() -> ChildShardPayload {
        ChildShardPayload {
            run_id: "run-123".to_string(),
            run_date: Some("2026-02-14".to_string()),
            dimensions: BTreeMap::from([
                (
                    "commission_rate".to_string(),
                    vec![Value::from(0.1), Value::from(0.2)],
                ),
                (
                    "num_drivers".to_string(),
                    vec![Value::from(100), Value::from(200)],
                ),
            ]),
            total_points: 4,
            shard_id: 1,
            start_index: 2,
            end_index_exclusive: 4,
            seed: 42,
            failure_injection_shards: Vec::new(),
        }
    }

    fn sample_config() -> ChildHandlerConfig {
        ChildHandlerConfig {
            bucket: "local-bucket".to_string(),
            prefix: "serverless-sweeps/outcomes".to_string(),
            run_date: "2026-02-14".to_string(),
            event_time: "2026-02-14T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn child_writes_success_outcome_envelope() {
        let store = RecordingStore::new();
        let response =
            handle_child_payload(&sample_payload(), &sample_config(), &PassExecutor, &store)
                .expect("child should succeed");

        assert_eq!(response.status, "ok");
        assert_eq!(store.keys().len(), 9);
        assert!(store
            .keys()
            .iter()
            .any(|key| key.contains("status=success") && key.ends_with(".parquet")));
        assert!(store
            .keys()
            .iter()
            .any(|key| key.contains("dataset=shard_outcomes") && key.ends_with(".parquet")));
        assert!(store
            .keys()
            .iter()
            .any(|key| key.contains("dataset=trip_data") && key.ends_with(".parquet")));
        assert!(store
            .keys()
            .iter()
            .any(|key| key.contains("dataset=snapshot_counts") && key.ends_with(".parquet")));
        assert!(store
            .keys()
            .iter()
            .any(|key| key.contains("dataset=effective_parameters") && key.ends_with(".parquet")));

        let parquet_key = store
            .keys()
            .into_iter()
            .find(|key| key.contains("dataset=shard_metrics"))
            .expect("parquet artifact key should exist");
        let parquet = store
            .body(&parquet_key)
            .expect("parquet body should exist for key");
        assert!(parquet.starts_with(b"PAR1"));
    }

    #[test]
    fn child_writes_failure_outcome_envelope() {
        let store = RecordingStore::new();
        let error = handle_child_payload(
            &sample_payload(),
            &sample_config(),
            &FailingExecutor,
            &store,
        )
        .expect_err("child should fail");

        assert!(error
            .failure_key
            .expect("failure key should exist")
            .contains("status_partition=failure"));
        assert_eq!(store.keys().len(), 1);
        assert!(store.keys()[0].contains("dataset=shard_outcomes"));

        let failure_key = store.keys()[0].clone();
        let failure_record = store
            .body(&failure_key)
            .expect("failure body should exist for key");
        assert!(failure_record.starts_with(b"PAR1"));
    }

    #[test]
    fn child_failure_does_not_overwrite_existing_success_outputs() {
        let store = RecordingStore::new();
        let payload = sample_payload();
        let config = sample_config();

        let success_metrics_key = metrics_object_key(
            &config.prefix,
            &config.run_date,
            &payload.run_id,
            "success",
            payload.shard_id,
            payload.start_index,
        );
        let success_outcome_key = success_outcome_object_key(
            &config.prefix,
            &config.run_date,
            &payload.run_id,
            payload.shard_id,
        );

        let existing_metrics = b"existing-success-metrics".to_vec();
        let existing_outcome = b"existing-success-outcome".to_vec();
        store.seed_object(&success_metrics_key, &existing_metrics);
        store.seed_object(&success_outcome_key, &existing_outcome);

        let error = handle_child_payload(&payload, &config, &FailingExecutor, &store)
            .expect_err("child should fail");

        let failure_key = error.failure_key.expect("failure key should exist");
        assert!(failure_key.contains("dataset=shard_outcomes"));

        assert_eq!(
            store
                .body(&success_metrics_key)
                .expect("existing metrics should remain"),
            existing_metrics
        );
        assert_eq!(
            store
                .body(&success_outcome_key)
                .expect("existing success outcome should remain"),
            existing_outcome
        );
    }

    #[test]
    fn child_persists_failure_record_when_success_outcome_write_fails() {
        let store = SelectiveFailStore::new("dataset=shard_outcomes/run_date=2026-02-14/run_id_partition=run-123/status_partition=success/shard_id_partition=1/part-0.parquet");
        let payload = sample_payload();
        let config = sample_config();

        let error = handle_child_payload(&payload, &config, &PassExecutor, &store)
            .expect_err("child should return failure when success outcome write fails");

        assert!(error.message.contains("Failed to persist success outcome"));
        let failure_key = error.failure_key.expect("failure key should exist");
        assert!(failure_key.contains("dataset=shard_outcomes"));

        assert!(store
            .keys()
            .iter()
            .any(|key| key.contains("status_partition=success") && key.ends_with(".parquet")));
        assert!(store.keys().iter().any(|key| key == &failure_key));

        let failure_record = store
            .body(&failure_key)
            .expect("failure body should exist for key");
        assert!(failure_record.starts_with(b"PAR1"));
    }

    #[test]
    fn effective_parameter_keys_match_metric_join_keys() {
        let store = RecordingStore::new();
        let payload = sample_payload();
        let config = sample_config();
        handle_child_payload(&payload, &config, &PassExecutor, &store)
            .expect("child should succeed");

        let mut metric_suffixes = Vec::new();
        let mut effective_suffixes = Vec::new();
        for key in store.keys() {
            if key.contains("dataset=shard_metrics") {
                metric_suffixes.push(join_identity(&key));
            }
            if key.contains("dataset=effective_parameters") {
                effective_suffixes.push(join_identity(&key));
            }
        }

        metric_suffixes.sort();
        effective_suffixes.sort();
        assert_eq!(metric_suffixes, effective_suffixes);
    }

    fn join_identity(key: &str) -> String {
        let run_date = partition_value(key, &["run_date="]);
        let run_id = partition_value(key, &["run_id=", "run_id_partition="]);
        let status = partition_value(key, &["status=", "status_partition="]);
        let shard_id = partition_value(key, &["shard_id=", "shard_id_partition="]);
        let point_index = partition_value(key, &["point_index=", "point_index_partition="]);

        format!(
            "run_date={run_date}/run_id={run_id}/status={status}/shard_id={shard_id}/point_index={point_index}"
        )
    }

    fn partition_value<'a>(key: &'a str, prefixes: &[&str]) -> &'a str {
        key.split('/')
            .find_map(|segment| {
                prefixes
                    .iter()
                    .find_map(|prefix| segment.strip_prefix(prefix))
            })
            .expect("expected partition segment in key")
    }
}
