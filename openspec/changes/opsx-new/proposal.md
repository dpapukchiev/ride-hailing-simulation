## Why

`infra/aws_serverless_sweep/deploy_local.sh` currently performs a cold build path on every invocation: it always pulls the Docker image, re-installs OS build dependencies, and wipes target build directories before packaging. Even when no Rust code has changed, this causes repeated deploys to take several minutes.

## What Changes

- Introduce an incremental mode for `deploy_local.sh` that reuses previously built artifacts and Docker-layer state on subsequent runs.
- Replace destructive clean steps with cache-aware invalidation tied to code/toolchain changes.
- Make Docker image pulling configurable (`always` vs `if-missing`) so operators can trade freshness for speed.
- Persist build metadata (toolchain, target, profile, dependency hash) to decide when a rebuild is required.
- Add explicit `--force-rebuild` / `SWEEP_FORCE_REBUILD=1` escape hatch for deterministic full rebuilds.
- Update deployment docs with expected hot-path timing improvements and cache troubleshooting guidance.

## Capabilities

### Modified Capabilities
- `public-repo-secure-cloud-deployment`: local deploy flow supports fast repeat deployments via safe cache reuse.

## Impact

- Primary files: `infra/aws_serverless_sweep/deploy_local.sh` and related serverless deployment docs.
- No infrastructure topology or IAM model changes.
- Improves developer/operator iteration speed while preserving correctness through cache invalidation rules.
