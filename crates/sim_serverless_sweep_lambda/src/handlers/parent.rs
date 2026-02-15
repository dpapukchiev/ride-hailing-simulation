use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sim_serverless_sweep_core::contract::{
    normalize_request, request_fingerprint, ChildShardPayload, DispatchRecord,
    ParentAcceptedResponse, RunContext, SweepRequest, ORCHESTRATION_SCHEMA_VERSION,
};
use sim_serverless_sweep_core::sharding::compute_shard_plan;

use crate::adapters::invoke::ChildInvoker;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiGatewayResponse {
    #[serde(rename = "statusCode")]
    pub status_code: u16,
    pub headers: Value,
    pub body: String,
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
    invoker: &dyn ChildInvoker,
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

    let run_context =
        build_run_context(normalized.run_id.clone(), request_fingerprint(&normalized));
    let shard_plan = match compute_shard_plan(&normalized) {
        Ok(value) => value,
        Err(error) => return validation_error_response(error.message()),
    };

    let mut dispatches = Vec::with_capacity(shard_plan.len());
    for assignment in shard_plan {
        let child_payload = ChildShardPayload {
            run_id: normalized.run_id.clone(),
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

        if let Err(error) = invoker.invoke_child_async(&bytes) {
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
    use std::sync::Mutex;

    use super::*;

    struct CapturingInvoker {
        payloads: Mutex<Vec<Vec<u8>>>,
    }

    impl CapturingInvoker {
        fn new() -> Self {
            Self {
                payloads: Mutex::new(Vec::new()),
            }
        }

        fn payloads(&self) -> Vec<Vec<u8>> {
            self.payloads.lock().expect("poisoned mutex").clone()
        }
    }

    impl ChildInvoker for CapturingInvoker {
        fn invoke_child_async(&self, payload: &[u8]) -> Result<(), String> {
            self.payloads
                .lock()
                .expect("poisoned mutex")
                .push(payload.to_vec());
            Ok(())
        }
    }

    #[test]
    fn rejects_invalid_payload_without_dispatching() {
        let invoker = CapturingInvoker::new();
        let response = handle_parent_event(
            json!({"body": "{\"run_id\":\"missing-dimensions\"}"}),
            Some("arn:aws:lambda:example:child"),
            &invoker,
        );

        assert_eq!(response.status_code, 400);
        assert!(invoker.payloads().is_empty());
    }

    #[test]
    fn dispatches_reproducible_child_payloads() {
        let invoker = CapturingInvoker::new();
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
            &invoker,
        );

        assert_eq!(response.status_code, 202);
        let payloads = invoker.payloads();
        assert_eq!(payloads.len(), 2);

        let first: ChildShardPayload =
            serde_json::from_slice(&payloads[0]).expect("payload should parse");
        let second: ChildShardPayload =
            serde_json::from_slice(&payloads[1]).expect("payload should parse");

        assert_eq!(first.run_id, "dispatch-run");
        assert_eq!(first.end_index_exclusive, second.start_index);
        assert_eq!(second.failure_injection_shards, vec![1]);
    }
}
