use aws_sdk_s3::primitives::ByteStream;
use chrono::Utc;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use sim_serverless_sweep_core::contract::ChildShardPayload;
use sim_serverless_sweep_lambda::adapters::object_store::OutcomeStore;
use sim_serverless_sweep_lambda::handlers::child::{
    handle_child_payload_with_sim_runtime, ChildHandlerConfig, ChildSuccessResponse,
};

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

    fn delete_object(&self, key: &str) -> Result<(), String> {
        let bucket = self.bucket.clone();
        let object_key = key.to_string();
        let client = self.s3_client.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                client
                    .delete_object()
                    .bucket(bucket)
                    .key(object_key)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|error| format!("failed to delete object from s3: {error}"))
            })
        })
    }
}

async fn handle_request(
    event: LambdaEvent<serde_json::Value>,
) -> Result<ChildSuccessResponse, Error> {
    let payload: ChildShardPayload = serde_json::from_value(event.payload)
        .map_err(|error| Error::from(format!("invalid child payload: {error}")))?;

    let bucket = std::env::var("SWEEP_RESULTS_BUCKET")
        .map_err(|_| Error::from("SWEEP_RESULTS_BUCKET must be configured"))?;
    let prefix = std::env::var("SWEEP_RESULTS_PREFIX")
        .unwrap_or_else(|_| "serverless-sweeps/outcomes".to_string());

    let now = Utc::now();
    let config = ChildHandlerConfig {
        bucket: bucket.clone(),
        prefix,
        run_date: now.format("%Y-%m-%d").to_string(),
        event_time: now.to_rfc3339(),
    };

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let outcome_store = S3OutcomeStore {
        bucket,
        s3_client: aws_sdk_s3::Client::new(&aws_config),
    };

    handle_child_payload_with_sim_runtime(&payload, &config, &outcome_store)
        .map_err(|error| Error::from(error.message))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(handle_request)).await
}
