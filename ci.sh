#!/usr/bin/env bash
# Local CI script — mirrors .github/workflows/ci.yml so you can catch failures
# before pushing.
#
# Usage:
#   ./ci.sh              # runs the "check" job (fmt + clippy + tests)
#   ./ci.sh check        # same as above
#   ./ci.sh render-diagrams  # renders Mermaid diagrams
#   ./ci.sh examples     # builds & runs example scenarios
#   ./ci.sh bench        # runs benchmarks
#   ./ci.sh all          # runs check + examples + bench
set -euo pipefail

# ── helpers ──────────────────────────────────────────────────────────────────

step() {
    echo ""
    echo "═══ $1 ═══"
}

run_parallel() {
    local pids=()
    local names=()
    local logs=()
    local statuses=()
    local idx=0
    local logs_dir

    logs_dir="$(mktemp -d)"

    while (($#)); do
        local name="$1"
        local cmd="$2"
        local log_file
        shift 2

        echo "→ $name"
        log_file="$logs_dir/task_${idx}.log"
        bash -lc "$cmd" >"$log_file" 2>&1 &
        pids+=("$!")
        names+=("$name")
        logs+=("$log_file")
        idx=$((idx + 1))
    done

    local failed=0
    for i in "${!pids[@]}"; do
        if wait "${pids[$i]}"; then
            statuses+=("0")
        else
            local status=$?
            statuses+=("$status")
            failed=1
        fi
    done

    echo ""
    echo "Parallel summary:"
    for i in "${!names[@]}"; do
        if [[ "${statuses[$i]}" == "0" ]]; then
            echo "✓ ${names[$i]}"
        else
            echo "✗ ${names[$i]} (exit ${statuses[$i]})"
        fi
    done

    if ((failed)); then
        echo ""
        echo "Failed step logs:"
        for i in "${!names[@]}"; do
            if [[ "${statuses[$i]}" != "0" ]]; then
                echo ""
                echo "--- ${names[$i]} ---"
                cat "${logs[$i]}"
            fi
        done
        rm -rf "$logs_dir"
        exit 1
    fi

    rm -rf "$logs_dir"
}

# ── jobs ─────────────────────────────────────────────────────────────────────

job_check() {
    step "Check job (max parallel)"
    run_parallel \
        "Cloud secret scan" "bash ./scripts/check_no_cloud_secrets.sh" \
        "Diagram artifact check" "bash ./scripts/check_rendered_diagrams.sh" \
        "Check formatting" "cargo fmt --all -- --check" \
        "Clippy" "cargo clippy --all-targets --all-features -- -D warnings" \
        "Test sim_core" "cargo test -p sim_core" \
        "Test sim_experiments" "cargo test -p sim_experiments"
}

job_render_diagrams() {
    step "Render diagrams"
    python scripts/render_diagrams.py --puppeteer-config scripts/puppeteer-config-ci.json
}

job_examples() {
    step "Run examples (parallel)"
    run_parallel \
        "scenario_run (500 riders, 100 drivers)" "cargo run -p sim_core --example scenario_run --release" \
        "scenario_run_large (10K riders, 7K drivers)" "cargo run -p sim_core --example scenario_run_large --release"
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
    render-diagrams)
        job_render_diagrams
        ;;
    examples)
        job_examples
        ;;
    bench)
        job_bench
        ;;
    all)
        step "Run all jobs (max parallel)"
        run_parallel \
            "check" "bash ./ci.sh check" \
            "examples" "bash ./ci.sh examples" \
            "bench" "bash ./ci.sh bench"
        ;;
    *)
        echo "Unknown job: $JOB"
        echo "Usage: $0 {check|render-diagrams|examples|bench|all}"
        exit 1
        ;;
esac

echo ""
echo "✓ CI job '$JOB' passed."
