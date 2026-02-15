use aws_sdk_s3::primitives::ByteStream;
use chrono::Utc;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde_json::{json, Value};
use sim_serverless_sweep_lambda::adapters::object_store::OutcomeStore;
use sim_serverless_sweep_lambda::handlers::child::{
    handle_child_payload_with_sim_runtime, ChildHandlerConfig,
};
use sim_serverless_sweep_lambda::handlers::parent::{handle_parent_event, ApiGatewayResponse};
use sim_serverless_sweep_lambda::runtime::contract::ChildShardPayload;

struct S3OutcomeStore {
    bucket: String,
    s3_client: aws_sdk_s3::Client,
}

impl OutcomeStore for S3OutcomeStore {
    fn write_object(&self, key: &str, body: &[u8]) -> Result<(), String> {
        let bucket = self.bucket.clone();
        let object_key = key.to_string();
        let body_bytes = body.to_vec();
        let client = self.s3_client.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                client
                    .put_object()
                    .bucket(bucket)
                    .key(object_key)
                    .body(ByteStream::from(body_bytes))
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|error| format!("failed to write object to s3: {error}"))
            })
        })
    }
}

#[derive(Clone)]
struct RuntimeDependencies {
    queue_url: String,
    bucket: String,
    prefix: String,
    s3_client: aws_sdk_s3::Client,
    sqs_client: aws_sdk_sqs::Client,
}

async fn handle_request(event: LambdaEvent<Value>) -> Result<Value, Error> {
    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let deps = RuntimeDependencies {
        queue_url: std::env::var("SHARD_QUEUE_URL")
            .map_err(|_| Error::from("SHARD_QUEUE_URL must be configured"))?,
        bucket: std::env::var("SWEEP_RESULTS_BUCKET")
            .map_err(|_| Error::from("SWEEP_RESULTS_BUCKET must be configured"))?,
        prefix: std::env::var("SWEEP_RESULTS_PREFIX")
            .unwrap_or_else(|_| "serverless-sweeps/outcomes".to_string()),
        s3_client: aws_sdk_s3::Client::new(&aws_config),
        sqs_client: aws_sdk_sqs::Client::new(&aws_config),
    };

    if is_sqs_event(&event.payload) {
        handle_sqs_event(&event.payload, &deps)?;
        Ok(json!({ "status": "ok" }))
    } else {
        let sqs_client = deps.sqs_client.clone();
        let queue_url = deps.queue_url.clone();
        let response: ApiGatewayResponse =
            handle_parent_event(event.payload, Some(&deps.queue_url), &move |payload| {
                let body = String::from_utf8(payload.to_vec())
                    .map_err(|error| format!("invalid UTF-8 shard payload: {error}"))?;
                let client = sqs_client.clone();
                let target_queue_url = queue_url.clone();
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async move {
                        client
                            .send_message()
                            .queue_url(target_queue_url)
                            .message_body(body)
                            .send()
                            .await
                            .map(|_| ())
                            .map_err(|error| format!("failed to enqueue shard message: {error}"))
                    })
                })
            });
        serde_json::to_value(response)
            .map_err(|error| Error::from(format!("failed to serialize api response: {error}")))
    }
}

fn is_sqs_event(event: &Value) -> bool {
    event
        .get("Records")
        .and_then(Value::as_array)
        .map(|records| {
            !records.is_empty()
                && records.iter().all(|record| {
                    record
                        .get("eventSource")
                        .and_then(Value::as_str)
                        .map(|source| source == "aws:sqs")
                        .unwrap_or(false)
                })
        })
        .unwrap_or(false)
}

fn handle_sqs_event(event: &Value, deps: &RuntimeDependencies) -> Result<(), Error> {
    let payloads = decode_sqs_payloads(event)?;

    let now = Utc::now();
    let fallback_run_date = now.format("%Y-%m-%d").to_string();
    let event_time = now.to_rfc3339();

    let outcome_store = S3OutcomeStore {
        bucket: deps.bucket.clone(),
        s3_client: deps.s3_client.clone(),
    };

    for payload in payloads {
        let config = ChildHandlerConfig {
            bucket: deps.bucket.clone(),
            prefix: deps.prefix.clone(),
            run_date: resolve_run_date(&payload, &fallback_run_date),
            event_time: event_time.clone(),
        };
        handle_child_payload_with_sim_runtime(&payload, &config, &outcome_store)
            .map_err(|error| Error::from(error.message))?;
    }

    Ok(())
}

fn resolve_run_date(payload: &ChildShardPayload, fallback_run_date: &str) -> String {
    payload
        .run_date
        .clone()
        .unwrap_or_else(|| fallback_run_date.to_string())
}

fn decode_sqs_payloads(event: &Value) -> Result<Vec<ChildShardPayload>, Error> {
    let records = event
        .get("Records")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::from("SQS event must include Records array"))?;

    let mut payloads = Vec::with_capacity(records.len());
    for record in records {
        let body = record
            .get("body")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::from("SQS record body must be a string"))?;
        let payload: ChildShardPayload = serde_json::from_str(body)
            .map_err(|error| Error::from(format!("invalid child shard payload: {error}")))?;
        payloads.push(payload);
    }

    Ok(payloads)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(handle_request)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_sqs_event_shape() {
        let event = json!({
            "Records": [
                {"eventSource": "aws:sqs", "body": "{}"}
            ]
        });
        assert!(is_sqs_event(&event));
    }

    #[test]
    fn rejects_non_sqs_records() {
        let event = json!({
            "Records": [
                {"eventSource": "aws:s3", "body": "{}"}
            ]
        });
        assert!(!is_sqs_event(&event));
    }

    #[test]
    fn rejects_record_without_body_string() {
        let event = json!({
            "Records": [
                {"eventSource": "aws:sqs", "body": 42}
            ]
        });

        let error = decode_sqs_payloads(&event).expect_err("non-string body should fail");
        assert!(error
            .to_string()
            .contains("SQS record body must be a string"));
    }

    #[test]
    fn rejects_invalid_child_payload_json() {
        let event = json!({
            "Records": [
                {"eventSource": "aws:sqs", "body": "{\"run_id\":\"x\"}"}
            ]
        });

        let error = decode_sqs_payloads(&event).expect_err("invalid child payload should fail");
        assert!(error.to_string().contains("invalid child shard payload"));
    }

    #[test]
    fn run_date_prefers_payload_value() {
        let payload = ChildShardPayload {
            run_id: "run-123".to_string(),
            run_date: Some("2026-02-14".to_string()),
            dimensions: std::collections::BTreeMap::new(),
            total_points: 1,
            shard_id: 0,
            start_index: 0,
            end_index_exclusive: 1,
            seed: 0,
            failure_injection_shards: Vec::new(),
        };

        let resolved = resolve_run_date(&payload, "2026-02-15");
        assert_eq!(resolved, "2026-02-14");
    }

    #[test]
    fn run_date_falls_back_when_payload_missing() {
        let payload = ChildShardPayload {
            run_id: "run-123".to_string(),
            run_date: None,
            dimensions: std::collections::BTreeMap::new(),
            total_points: 1,
            shard_id: 0,
            start_index: 0,
            end_index_exclusive: 1,
            seed: 0,
            failure_injection_shards: Vec::new(),
        };

        let resolved = resolve_run_date(&payload, "2026-02-15");
        assert_eq!(resolved, "2026-02-15");
    }
}
