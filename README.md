# Ride-Hailing Simulation

Rust-based discrete-event simulation scaffold with a minimal ECS-based agent model.

## Setup

This repo uses `mise` to manage the Rust toolchain via `.mise.toml`.

```sh
mise install
```

If you do not already have mise activated in your shell, run:

```sh
mise activate
```

## Run

```sh
cargo test -p sim_core
cargo run -p sim_core --example scenario_run
```
