#!/usr/bin/env bash
# Local CI script — mirrors .github/workflows/ci.yml so you can catch failures
# before pushing.
#
# Usage:
#   ./ci.sh              # runs the "check" job (fmt + clippy + tests)
#   ./ci.sh check        # same as above
#   ./ci.sh examples     # builds & runs example scenarios
#   ./ci.sh bench        # runs benchmarks
#   ./ci.sh all          # runs check + examples + bench
set -euo pipefail

# ── helpers ──────────────────────────────────────────────────────────────────

step() {
    echo ""
    echo "═══ $1 ═══"
}

# ── jobs ─────────────────────────────────────────────────────────────────────

job_check() {
    step "Cloud secret scan"
    ./scripts/check_no_cloud_secrets.sh

    step "Check formatting"
    cargo fmt --all -- --check

    step "Clippy"
    cargo clippy --all-targets --all-features -- -D warnings

    step "Test sim_core"
    cargo test -p sim_core

    step "Test sim_experiments"
    cargo test -p sim_experiments
}

job_examples() {
    step "Run scenario_run (500 riders, 100 drivers)"
    cargo run -p sim_core --example scenario_run --release

    step "Run scenario_run_large (10K riders, 7K drivers)"
    cargo run -p sim_core --example scenario_run_large --release
}

job_bench() {
    step "Run benchmarks"
    cargo bench --package sim_core --bench performance
}

# ── main ─────────────────────────────────────────────────────────────────────

JOB="${1:-check}"

case "$JOB" in
    check)
        job_check
        ;;
    examples)
        job_examples
        ;;
    bench)
        job_bench
        ;;
    all)
        job_check
        job_examples
        job_bench
        ;;
    *)
        echo "Unknown job: $JOB"
        echo "Usage: $0 {check|examples|bench|all}"
        exit 1
        ;;
esac

echo ""
echo "✓ CI job '$JOB' passed."
