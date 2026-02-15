## Context

The deploy script is designed for correctness, but it behaves like a clean-room build each time. The most expensive repeated work is:

1. `docker run --pull=always ...` for toolchain verification and build.
2. `apt-get update && apt-get install ...` inside the build container every run.
3. `rm -rf target/...` before packaging, which discards Cargo incremental and dependency caches.

For local operator loops (small Terraform changes, env var updates, or reruns after failed apply), these costs dominate total deployment time.

## Goals / Non-Goals

**Goals**
- Make unchanged subsequent `deploy_local.sh` runs materially faster.
- Preserve reproducibility and safe rebuild behavior when inputs change.
- Keep operator UX simple and explicit.

**Non-Goals**
- Replacing Docker-based builds with host-only builds.
- Changing Lambda runtime architecture or packaging format.
- Adding remote build cache infrastructure.

## Decision

### Decision: Add cache-aware deploy stages with explicit rebuild controls

Implement a two-path flow:

- **Fast path (default for repeat runs)**
  - Reuse local Cargo target cache.
  - Reuse a prebuilt “builder” Docker image that already includes clang/cmake/perl/pkg-config.
  - Skip rebuilding runtime zip when tracked build inputs are unchanged.

- **Cold path (on first run or invalidation)**
  - Build/update builder image.
  - Rebuild and package runtime artifact.

### Cache invalidation model

The script stores a metadata file in `infra/aws_serverless_sweep/dist/` containing:
- target triple
- profile
- rust toolchain image tag
- hash of relevant Rust/TOML inputs (workspace manifests + serverless crates + xtask packaging code)

If metadata mismatches current inputs, the script rebuilds and refreshes metadata.

## Detailed design

1. **Builder image layering**
   - Add Dockerfile (or inline build recipe) that extends `rust:1-bullseye` with required native packages.
   - Tag locally (e.g., `ride-sweep-builder:<rust-tag>`).
   - Pull base image conditionally by policy (`SWEEP_DOCKER_PULL_POLICY=always|if-missing|never`).

2. **Artifact reuse gate**
   - Before invoking packaging, check:
     - runtime zip exists
     - metadata exists and matches current fingerprint
   - If both true and no force flag, skip package build and proceed to Terraform.

3. **Force rebuild semantics**
   - `--force-rebuild` CLI flag and/or `SWEEP_FORCE_REBUILD=1` bypasses cache checks.
   - Force mode rebuilds artifacts and rewrites metadata.

4. **Observability**
   - Log whether run is `cache-hit` or `cache-miss` and why (e.g., `changed Cargo.lock`).
   - Emit elapsed time per major stage.

## Alternatives considered

1. **Keep current script and rely on Docker layer cache only**
   - Reject: still does apt invocation and target cleanup each run.

2. **Use host Rust toolchain for incremental builds**
   - Reject: introduces host variance and weakens deployment reproducibility.

3. **Remote cache service (sccache/registry-backed layers)**
   - Reject for now: higher setup/ops burden than needed for local-loop gains.

## Risks / Trade-offs

- Stale artifact risk if fingerprint excludes relevant files.
  - Mitigation: conservative include set and `--force-rebuild` fallback.
- Local disk usage increases due to preserved target/build cache.
  - Mitigation: add `clean` guidance in docs.
- Pull policy set to `if-missing` may lag security patches.
  - Mitigation: document periodic explicit refresh.

## Migration Plan

1. Implement cache-aware script changes behind default behavior.
2. Add docs for new flags/env vars and troubleshooting.
3. Validate: first run (cold) and second run (hot) both deploy successfully.
4. Measure and document before/after timing samples.
