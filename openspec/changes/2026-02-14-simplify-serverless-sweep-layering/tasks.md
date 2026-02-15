## 1. Orchestration Simplification

- [ ] 1.1 Add SQS queue resources, queue policy, and Lambda event-source mapping in `infra/aws_serverless_sweep/terraform`.
- [ ] 1.2 Update runtime IAM: remove parent->child `lambda:InvokeFunction`; add scoped `sqs:SendMessage` and worker consume permissions.
- [ ] 1.3 Keep API Gateway integration but route requests to the unified runtime handler.

## 2. Runtime Layer Consolidation

- [ ] 2.1 Implement a single Lambda entrypoint that routes API vs SQS events to dedicated handler modules.
- [ ] 2.2 Consolidate contract normalization and shard planning with runtime handler code under one crate/module ownership model.
- [ ] 2.3 Remove obsolete child-invoker abstractions and duplicate payload translation layers.

## 3. Worker and Persistence Parity

- [ ] 3.1 Execute shard messages from SQS with deterministic `run_id` / `shard_id` semantics.
- [ ] 3.2 Preserve Parquet output compatibility for shard outcomes and per-point artifacts.
- [ ] 3.3 Enforce idempotent writes for retry safety and retain explicit failure outcome records.

## 4. Validation and Rollout

- [ ] 4.1 Add tests for event-shape routing, enqueue behavior, and SQS worker execution.
- [ ] 4.2 Run an end-to-end sandbox sweep and compare outputs/queries against current architecture.
- [ ] 4.3 Update runbook/deploy docs to describe the simplified architecture and troubleshooting flow.
