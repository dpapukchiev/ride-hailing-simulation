use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::runtime::contract::{
    normalize_request, request_fingerprint, ChildShardPayload, DispatchRecord,
    ParentAcceptedResponse, RunContext, RunContextRecord, SweepRequest,
    ORCHESTRATION_SCHEMA_VERSION, RUN_CONTEXT_RECORD_SCHEMA_VERSION,
};
use crate::runtime::sharding::compute_shard_plan;
use crate::runtime::storage_keys::run_context_object_key;
use arrow::array::{ArrayRef, StringArray, UInt64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use chrono::Utc;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiGatewayResponse {
    #[serde(rename = "statusCode")]
    pub status_code: u16,
    pub headers: Value,
    pub body: String,
}

pub struct RunContextExportConfig<'a> {
    pub prefix: &'a str,
    pub persist_object: &'a dyn Fn(&str, &[u8]) -> Result<(), String>,
}

pub fn build_run_context(run_id: impl Into<String>, request_fingerprint: String) -> RunContext {
    RunContext {
        run_id: run_id.into(),
        schema_version: ORCHESTRATION_SCHEMA_VERSION.to_string(),
        request_fingerprint,
    }
}

pub fn handle_parent_event(
    event: Value,
    dispatch_target: Option<&str>,
    dispatch: &dyn Fn(&[u8]) -> Result<(), String>,
) -> ApiGatewayResponse {
    handle_parent_event_with_context_export(event, dispatch_target, dispatch, None)
}

pub fn handle_parent_event_with_context_export(
    event: Value,
    dispatch_target: Option<&str>,
    dispatch: &dyn Fn(&[u8]) -> Result<(), String>,
    run_context_export: Option<RunContextExportConfig<'_>>,
) -> ApiGatewayResponse {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        handle_parent_event_impl(event, dispatch_target, dispatch, run_context_export)
    }));

    match result {
        Ok(response) => response,
        Err(panic_payload) => error_response(
            500,
            json!({
                "error": "internal_error",
                "message": "Parent handler panicked while processing the request",
                "details": panic_payload_message(panic_payload),
            }),
        ),
    }
}

fn handle_parent_event_impl(
    event: Value,
    dispatch_target: Option<&str>,
    dispatch: &dyn Fn(&[u8]) -> Result<(), String>,
    run_context_export: Option<RunContextExportConfig<'_>>,
) -> ApiGatewayResponse {
    let payload = match normalize_apigw_event(event) {
        Ok(value) => value,
        Err(message) => return validation_error_response(&message),
    };

    let request = match serde_json::from_value::<SweepRequest>(payload) {
        Ok(value) => value,
        Err(error) => return validation_error_response(&format!("Malformed request: {error}")),
    };

    let normalized = match normalize_request(request) {
        Ok(value) => value,
        Err(error) => return validation_error_response(error.message()),
    };

    let dispatch_target = match dispatch_target {
        Some(value) if !value.trim().is_empty() => value,
        _ => {
            return error_response(
                500,
                json!({
                    "error": "misconfiguration",
                    "message": "SHARD_QUEUE_URL must be configured",
                }),
            );
        }
    };

    let request_fingerprint = request_fingerprint(&normalized);
    let run_context = build_run_context(normalized.run_id.clone(), request_fingerprint.clone());
    let run_date = Utc::now().format("%Y-%m-%d").to_string();
    let shard_plan = match compute_shard_plan(&normalized) {
        Ok(value) => value,
        Err(error) => return validation_error_response(error.message()),
    };

    let run_context_record = RunContextRecord {
        run_id: normalized.run_id.clone(),
        run_date: run_date.clone(),
        status: "accepted".to_string(),
        request_source: "api_gateway".to_string(),
        record_schema: RUN_CONTEXT_RECORD_SCHEMA_VERSION.to_string(),
        request_fingerprint: request_fingerprint.clone(),
        config_fingerprint: request_fingerprint,
        total_points: normalized.total_points,
        shard_count: shard_plan.len(),
        shard_strategy: if normalized.shard_count.is_some() {
            "explicit_shard_count".to_string()
        } else {
            "derived_from_shard_size".to_string()
        },
        max_shards: normalized.max_shards,
    };

    if let Some(export) = run_context_export {
        let run_context_key = run_context_object_key(
            export.prefix,
            &run_context_record.run_date,
            &run_context_record.run_id,
            &run_context_record.status,
        );
        let payload = match serialize_run_context_parquet(&run_context_record) {
            Ok(value) => value,
            Err(error) => {
                return error_response(
                    500,
                    json!({
                        "error": "run_context_serialization_error",
                        "message": error,
                    }),
                );
            }
        };

        if let Err(error) = (export.persist_object)(&run_context_key, &payload) {
            return error_response(
                502,
                json!({
                    "error": "run_context_persist_failed",
                    "message": error,
                    "run_context_key": run_context_key,
                }),
            );
        }
    }

    let mut dispatches = Vec::with_capacity(shard_plan.len());
    for assignment in shard_plan {
        let child_payload = ChildShardPayload {
            run_id: normalized.run_id.clone(),
            run_date: Some(run_date.clone()),
            dimensions: normalized.dimensions.clone(),
            total_points: normalized.total_points,
            shard_id: assignment.shard_id,
            start_index: assignment.start_index,
            end_index_exclusive: assignment.end_index_exclusive,
            seed: normalized.seed,
            failure_injection_shards: normalized.failure_injection_shards.clone(),
        };

        let bytes = match serde_json::to_vec(&child_payload) {
            Ok(value) => value,
            Err(error) => {
                return error_response(
                    500,
                    json!({
                        "error": "serialization_error",
                        "message": error.to_string(),
                    }),
                );
            }
        };

        let dispatch_result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatch(&bytes)));

        match dispatch_result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                return error_response(
                    502,
                    json!({
                        "error": "dispatch_failed",
                        "message": error,
                        "dispatch_target": dispatch_target,
                        "run_context": run_context,
                    }),
                );
            }
            Err(panic_payload) => {
                return error_response(
                    500,
                    json!({
                        "error": "dispatch_panicked",
                        "message": "Child dispatch panicked before completion",
                        "details": panic_payload_message(panic_payload),
                        "dispatch_target": dispatch_target,
                        "run_context": run_context,
                    }),
                );
            }
        }

        dispatches.push(DispatchRecord {
            shard_id: child_payload.shard_id,
            status_code: 202,
        });
    }

    let response = ParentAcceptedResponse {
        run_id: normalized.run_id,
        total_points: normalized.total_points,
        shards_dispatched: dispatches.len(),
        dispatches,
        status: "dispatch_submitted".to_string(),
        schema_version: ORCHESTRATION_SCHEMA_VERSION.to_string(),
    };
    success_response(202, response)
}

fn panic_payload_message(panic_payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = panic_payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = panic_payload.downcast_ref::<String>() {
        return message.clone();
    }
    "panic payload was not a string".to_string()
}

fn serialize_run_context_parquet(record: &RunContextRecord) -> Result<Vec<u8>, String> {
    let mut temp_path = std::env::temp_dir();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Failed to read clock for parquet export: {error}"))?
        .as_nanos();
    temp_path.push(format!("serverless-run-context-{timestamp}.parquet"));

    let schema = std::sync::Arc::new(Schema::new(vec![
        Field::new("run_id", DataType::Utf8, false),
        Field::new("run_date", DataType::Utf8, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("request_source", DataType::Utf8, false),
        Field::new("record_schema", DataType::Utf8, false),
        Field::new("request_fingerprint", DataType::Utf8, false),
        Field::new("config_fingerprint", DataType::Utf8, false),
        Field::new("total_points", DataType::UInt64, false),
        Field::new("shard_count", DataType::UInt64, false),
        Field::new("shard_strategy", DataType::Utf8, false),
        Field::new("max_shards", DataType::UInt64, false),
    ]));

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            std::sync::Arc::new(StringArray::from(vec![record.run_id.clone()])) as ArrayRef,
            std::sync::Arc::new(StringArray::from(vec![record.run_date.clone()])),
            std::sync::Arc::new(StringArray::from(vec![record.status.clone()])),
            std::sync::Arc::new(StringArray::from(vec![record.request_source.clone()])),
            std::sync::Arc::new(StringArray::from(vec![record.record_schema.clone()])),
            std::sync::Arc::new(StringArray::from(vec![record.request_fingerprint.clone()])),
            std::sync::Arc::new(StringArray::from(vec![record.config_fingerprint.clone()])),
            std::sync::Arc::new(UInt64Array::from(vec![record.total_points as u64])),
            std::sync::Arc::new(UInt64Array::from(vec![record.shard_count as u64])),
            std::sync::Arc::new(StringArray::from(vec![record.shard_strategy.clone()])),
            std::sync::Arc::new(UInt64Array::from(vec![record.max_shards as u64])),
        ],
    )
    .map_err(|error| format!("Failed to build run-context parquet record batch: {error}"))?;

    let file = fs::File::create(&temp_path)
        .map_err(|error| format!("Failed to create temporary parquet file: {error}"))?;
    let props = WriterProperties::builder().build();
    let mut writer =
        ArrowWriter::try_new(file, schema, Some(props)).map_err(|error| error.to_string())?;
    writer
        .write(&batch)
        .map_err(|error| format!("Failed to write run-context parquet batch: {error}"))?;
    writer
        .close()
        .map_err(|error| format!("Failed to close run-context parquet writer: {error}"))?;

    let bytes = fs::read(&temp_path)
        .map_err(|error| format!("Failed to read exported parquet file: {error}"))?;
    let _ = fs::remove_file(&temp_path);
    Ok(bytes)
}

fn normalize_apigw_event(event: Value) -> Result<Value, String> {
    let Some(object) = event.as_object() else {
        return Err("Request payload must be a JSON object".to_string());
    };

    let Some(body) = object.get("body") else {
        return Ok(event);
    };

    match body {
        Value::Null => Ok(json!({})),
        Value::Object(_) => Ok(body.clone()),
        Value::String(text) => {
            serde_json::from_str(text).map_err(|error| format!("Malformed JSON body: {error}"))
        }
        _ => Err("Request body must be a JSON object".to_string()),
    }
}

fn validation_error_response(message: &str) -> ApiGatewayResponse {
    error_response(
        400,
        json!({
            "error": "validation_error",
            "message": message,
        }),
    )
}

fn success_response(status_code: u16, payload: impl Serialize) -> ApiGatewayResponse {
    ApiGatewayResponse {
        status_code,
        headers: json!({"Content-Type": "application/json"}),
        body: serde_json::to_string(&payload).expect("response payload should serialize"),
    }
}

fn error_response(status_code: u16, payload: Value) -> ApiGatewayResponse {
    ApiGatewayResponse {
        status_code,
        headers: json!({"Content-Type": "application/json"}),
        body: payload.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn rejects_invalid_payload_without_dispatching() {
        let payloads: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        let run_context_writes: Arc<Mutex<HashMap<String, Vec<u8>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let payloads_for_dispatch = Arc::clone(&payloads);
        let writes_for_export = Arc::clone(&run_context_writes);
        let response = handle_parent_event(
            json!({"body": "{\"run_id\":\"missing-dimensions\"}"}),
            Some("arn:aws:lambda:example:child"),
            &move |payload| {
                payloads_for_dispatch
                    .lock()
                    .expect("poisoned mutex")
                    .push(payload.to_vec());
                Ok(())
            },
        );

        let _ = handle_parent_event_with_context_export(
            json!({"body": "{\"run_id\":\"missing-dimensions\"}"}),
            Some("arn:aws:lambda:example:child"),
            &|_payload| Ok(()),
            Some(RunContextExportConfig {
                prefix: "serverless-sweeps/outcomes",
                persist_object: &move |key, body| {
                    writes_for_export
                        .lock()
                        .expect("poisoned mutex")
                        .insert(key.to_string(), body.to_vec());
                    Ok(())
                },
            }),
        );

        assert_eq!(response.status_code, 400);
        assert!(payloads.lock().expect("poisoned mutex").is_empty());
        assert!(run_context_writes
            .lock()
            .expect("poisoned mutex")
            .is_empty());
    }

    #[test]
    fn accepted_run_persists_run_context_once() {
        let run_context_writes: Arc<Mutex<HashMap<String, Vec<u8>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let writes_for_export = Arc::clone(&run_context_writes);

        let response = handle_parent_event_with_context_export(
            json!({
                "body": {
                    "run_id": "context-run",
                    "dimensions": {
                        "commission_rate": [0.1, 0.2],
                        "num_drivers": [100]
                    },
                    "shard_count": 2,
                    "seed": 7
                }
            }),
            Some("arn:aws:lambda:example:child"),
            &|_payload| Ok(()),
            Some(RunContextExportConfig {
                prefix: "serverless-sweeps/outcomes",
                persist_object: &move |key, body| {
                    writes_for_export
                        .lock()
                        .expect("poisoned mutex")
                        .insert(key.to_string(), body.to_vec());
                    Ok(())
                },
            }),
        );

        assert_eq!(response.status_code, 202);
        let writes = run_context_writes.lock().expect("poisoned mutex");
        assert_eq!(writes.len(), 1);
        let key = writes
            .keys()
            .next()
            .expect("run-context key should exist")
            .clone();
        assert!(key.contains("dataset=run_context"));
        assert!(key.contains("run_id_partition=context-run"));
        assert!(key.contains("status_partition=accepted"));
    }

    #[test]
    fn dispatches_reproducible_child_payloads() {
        let payloads: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        let payloads_for_dispatch = Arc::clone(&payloads);
        let response = handle_parent_event(
            json!({
                "body": {
                    "run_id": "dispatch-run",
                    "dimensions": {
                        "commission_rate": [0.1, 0.2],
                        "num_drivers": [100, 200]
                    },
                    "shard_count": 2,
                    "seed": 7,
                    "failure_injection_shards": [1]
                }
            }),
            Some("arn:aws:lambda:example:child"),
            &move |payload| {
                payloads_for_dispatch
                    .lock()
                    .expect("poisoned mutex")
                    .push(payload.to_vec());
                Ok(())
            },
        );

        assert_eq!(response.status_code, 202);
        let payloads = payloads.lock().expect("poisoned mutex").clone();
        assert_eq!(payloads.len(), 2);

        let first: ChildShardPayload =
            serde_json::from_slice(&payloads[0]).expect("payload should parse");
        let second: ChildShardPayload =
            serde_json::from_slice(&payloads[1]).expect("payload should parse");

        assert_eq!(first.run_id, "dispatch-run");
        assert_eq!(first.run_date, second.run_date);
        assert!(first.run_date.is_some());
        assert_eq!(first.end_index_exclusive, second.start_index);
        assert_eq!(second.failure_injection_shards, vec![1]);
    }

    #[test]
    fn returns_detailed_error_when_dispatch_panics() {
        let response = handle_parent_event(
            json!({
                "body": {
                    "run_id": "panic-run",
                    "dimensions": {
                        "commission_rate": [0.1],
                        "num_drivers": [100]
                    },
                    "shard_count": 1,
                    "seed": 7
                }
            }),
            Some("arn:aws:lambda:example:child"),
            &|_payload| panic!("simulated dispatch panic"),
        );

        assert_eq!(response.status_code, 500);

        let body: Value = serde_json::from_str(&response.body).expect("response body should parse");
        assert_eq!(body["error"], "dispatch_panicked");
        assert_eq!(body["message"], "Child dispatch panicked before completion");
        assert_eq!(body["details"], "simulated dispatch panic");
    }
}
