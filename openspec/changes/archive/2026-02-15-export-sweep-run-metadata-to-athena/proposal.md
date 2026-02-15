## Why

API-triggered sweep runs currently persist outcome artifacts, but they do not export a first-class run context record or per-point effective simulation parameters in a join-friendly analytics layout. This makes post-run analysis brittle because analysts cannot reliably join outcomes back to the exact configuration that produced them.

## What Changes

- Add run-context metadata export for every API sweep run, including stable run identifiers, request provenance, sweep dimensions, shard plan metadata, and configuration fingerprints.
- Add per-point effective simulation parameter export to S3 in Athena-queryable format, keyed so each outcome row can be joined to its originating effective configuration.
- Extend analytics dataset contracts so outcomes, run context, and effective parameter records share deterministic join keys.
- Add/adjust Athena table definitions to query run context and per-point configuration alongside existing outcome datasets.
- Document data contract expectations so downstream analytics can build stable joins without reverse-engineering runtime payloads.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `serverless-rust-sweep-orchestration`: require durable run-context metadata export during API-triggered run creation.
- `serverless-rust-sweep-worker-execution`: require per-point effective simulation parameter export tied to shard execution outputs.
- `athena-run-analytics`: require Athena-readable schema/join contract spanning outcomes, run context, and effective parameter datasets.

## Impact

- Affected areas: serverless orchestration and worker runtime crates, S3 partition/object contracts, and Athena SQL definitions under `infra/aws_serverless_sweep/athena/`.
- API sweep behavior remains asynchronous, but accepted runs now produce additional analytics metadata artifacts.
- Analytics workflows gain deterministic joins between outcomes and configuration, reducing manual reconstruction and query fragility.
