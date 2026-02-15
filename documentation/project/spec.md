# Project

## Workspace Layout

```text
README.md
Cargo.toml
infra/
  aws_serverless_sweep/
    README.md
    deploy_local.sh
    terraform/
      main.tf
      variables.tf
  osrm/
    docker-compose.yml
    setup.sh
documentation/
  README.md
  core-sim/
  drivers/
  experiments/
  matching/
  pricing/
  riders/
  telemetry/
  testing/
  ui/
  project/
crates/
  sim_core/
    Cargo.toml
    src/
      clock.rs
      ecs.rs
      lib.rs
      runner.rs
      scenario/            # scenario modules (build, params, mod)
      spatial.rs
      telemetry.rs
      pricing.rs
      profiling.rs
      routing.rs
      traffic.rs
      systems/
        mod.rs
        show_quote.rs
        quote_decision.rs
        quote_accepted.rs
        quote_rejected.rs
        matching.rs
        match_accepted.rs
        driver_decision.rs
        movement.rs
        rider_cancel.rs
        trip_started.rs
        trip_completed.rs
        driver_offduty.rs
        telemetry_snapshot.rs
    benches/
      performance.rs
      README.md
    tests/
      load_tests.rs
    examples/
      scenario_run.rs
      scenario_run_large.rs
  sim_experiments/
    Cargo.toml
    src/
      lib.rs
      parameters.rs
      parameter_spaces.rs
      runner.rs
      metrics.rs
      health.rs
      export.rs
    examples/
      parameter_sweep.rs
  sim_serverless_sweep_core/
    Cargo.toml
    src/
      lib.rs
      contract.rs
      sharding.rs
      storage_keys.rs
  sim_serverless_sweep_lambda/
    Cargo.toml
    src/
      lib.rs
      handlers/
        mod.rs
        parent.rs
        child.rs
  sim_ui/
    Cargo.toml
    src/
      main.rs
      app.rs
      ui/
        mod.rs
        controls/          # control panels split by concern
        rendering.rs
        utils.rs
        constants.rs
xtask/
  Cargo.toml
  src/
    main.rs
```

## Dependencies

`crates/sim_core/Cargo.toml`:

- `h3o = "0.8"` for H3 spatial indexing (stable toolchain compatible).
- `bevy_ecs = "0.13"` for ECS world, components, and systems.
- `rand = "0.8"` for scenario randomisation (positions, request times, destinations).
- `arrow` + `parquet` for Parquet export of completed trips and snapshots.
- `pathfinding = "4.14"` for Hungarian matching algorithm (Kuhn-Munkres).
- `lru = "0.12"` for distance calculation caching.

`crates/sim_ui/Cargo.toml`:

- `eframe` + `egui_plot` for the native visualization UI.
- `bevy_ecs` + `h3o` for shared types and map projection.
- Feature `osrm` (default): enables OSRM routing backend selection in the UI.

## Tooling

- `mise` is used for toolchain management via `.mise.toml`.
- Rust toolchain: `stable`.
- `README.md` includes setup and run commands.
- **`xtask`**: Cross-platform task runner (see [Task Runner](#task-runner-xtask) section below). Provides a single CLI entrypoint for running simulations, experiments, benchmarks, CI checks, and more. Defined in the `xtask` workspace member and invoked with `cargo run -p xtask -- ...`.
- **CI**: GitHub Actions workflow (`.github/workflows/ci.yml`) runs three parallel jobs on every push/PR to `main`:
  - `check`: formatting (`cargo fmt --check`), linting (`cargo clippy -D warnings`), and tests (`sim_core` + `sim_experiments`)
  - `examples`: runs both `scenario_run` and `scenario_run_large` in release mode
  - `bench`: runs Criterion benchmarks (push to `main` only)

## Task Runner (`xtask`)

A cross-platform CLI task runner built with `clap`. The `xtask` crate is a
workspace member. Run `cargo run -p xtask -- --help` to list all commands.

| Command | Description |
|---|---|
| `cargo run -p xtask -- ui` | Launch the simulation GUI |
| `cargo run -p xtask -- run` | Run the standard scenario (500 riders, 100 drivers, release) |
| `cargo run -p xtask -- run-large` | Run the large scenario (10K riders, 7K drivers, release) |
| `cargo run -p xtask -- sweep` | Run a parameter sweep experiment |
| `cargo run -p xtask -- route-export` | Export a precomputed route table (`--sample-count`, `--output`) |
| `cargo run -p xtask -- bench` | Run Criterion benchmarks |
| `cargo run -p xtask -- bench-compare` | Stash changes, create baseline, restore, compare benchmarks |
| `cargo run -p xtask -- ci [check\|examples\|bench\|all]` | Run CI checks (default: `check`) |
| `cargo run -p xtask -- load-test` | Run load tests (ignored tests in sim_core) |
| `cargo run -p xtask -- serverless-package` | Build and package Rust Lambda artifacts for Terraform (`parent.zip`, `child.zip`) |

**Dependencies** (`xtask/Cargo.toml`): `clap = "4"` (with `derive` feature).

## Local CI

A `ci.sh` script at the repository root mirrors the GitHub Actions workflow
(`.github/workflows/ci.yml`) so you can catch failures before pushing.
The same checks are also available cross-platform via `cargo run -p xtask -- ci`.

```bash
./ci.sh              # runs "check" job: fmt + clippy + tests (default)
./ci.sh check        # same as above
./ci.sh examples     # builds & runs example scenarios (release mode)
./ci.sh bench        # runs benchmarks
./ci.sh all          # runs check + examples + bench

# Or cross-platform via xtask:
cargo run -p xtask -- ci           # same as ./ci.sh check
cargo run -p xtask -- ci examples  # same as ./ci.sh examples
cargo run -p xtask -- ci all       # same as ./ci.sh all
```

The CI workflow delegates to `ci.sh`, so local and remote checks stay in
sync. On Windows, use `cargo run -p xtask -- ci` instead of the shell script.
