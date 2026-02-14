use aws_sdk_lambda::types::InvocationType;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde_json::Value;
use sim_serverless_sweep_lambda::adapters::invoke::ChildInvoker;
use sim_serverless_sweep_lambda::handlers::parent::{handle_parent_event, ApiGatewayResponse};

struct AwsLambdaChildInvoker {
    lambda_client: aws_sdk_lambda::Client,
    child_lambda_arn: String,
}

struct NoopInvoker;

impl ChildInvoker for NoopInvoker {
    fn invoke_child_async(&self, _payload: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

impl ChildInvoker for AwsLambdaChildInvoker {
    fn invoke_child_async(&self, payload: &[u8]) -> Result<(), String> {
        let request_payload = payload.to_vec();
        let client = self.lambda_client.clone();
        let function_name = self.child_lambda_arn.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                client
                    .invoke()
                    .function_name(function_name)
                    .invocation_type(InvocationType::Event)
                    .set_payload(Some(request_payload.into()))
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|error| format!("failed to invoke child lambda: {error}"))
            })
        })
    }
}

async fn handle_request(event: LambdaEvent<Value>) -> Result<ApiGatewayResponse, Error> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let lambda_client = aws_sdk_lambda::Client::new(&config);

    let child_lambda_arn = std::env::var("CHILD_LAMBDA_ARN").ok();
    let invoker = child_lambda_arn.as_ref().map(|arn| AwsLambdaChildInvoker {
        lambda_client,
        child_lambda_arn: arn.clone(),
    });
    let noop_invoker = NoopInvoker;

    let response = handle_parent_event(
        event.payload,
        child_lambda_arn.as_deref(),
        invoker
            .as_ref()
            .map(|value| value as &dyn ChildInvoker)
            .unwrap_or(&noop_invoker),
    );
    Ok(response)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    lambda_runtime::run(service_fn(handle_request)).await
}
