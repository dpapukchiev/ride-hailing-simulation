use bevy_ecs::prelude::World;
use sim_core::pricing::PricingConfig;
use sim_core::runner::{initialize_simulation, run_until_empty, simulation_schedule};
use sim_core::scenario::{MatchingAlgorithmType, ScenarioParams};
use sim_core::telemetry::{SimSnapshots, SimTelemetry};
use sim_core::telemetry_export::{write_snapshot_counts_parquet, write_trips_parquet};
use sim_core::traffic::TrafficProfileKind;
use sim_experiments::metrics::extract_metrics;
use sim_experiments::{ParameterSet, SimulationResult};
use sim_serverless_sweep_core::contract::ChildShardPayload;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::handlers::child::{ChildExecutionSummary, ShardExecutor};

#[derive(Debug, Default, Clone, Copy)]
pub struct SimExperimentsShardExecutor;

impl ShardExecutor for SimExperimentsShardExecutor {
    fn execute_shard(&self, payload: &ChildShardPayload) -> Result<ChildExecutionSummary, String> {
        if payload.failure_injection_shards.contains(&payload.shard_id) {
            return Err("Injected shard failure for verification".to_string());
        }

        if payload.end_index_exclusive > payload.total_points {
            return Err("end_index_exclusive exceeds total_points".to_string());
        }

        let mut points_processed = 0usize;
        let mut point_results =
            Vec::with_capacity(payload.end_index_exclusive - payload.start_index);
        for point_index in payload.start_index..payload.end_index_exclusive {
            let parameter_set = parameter_set_for_index(payload, point_index)?;
            let (metrics, trip_data_parquet, snapshot_counts_parquet) =
                run_single_simulation_with_exports(
                    &parameter_set,
                    &payload.run_id,
                    payload.shard_id,
                    point_index,
                )?;
            point_results.push(crate::handlers::child::ShardPointResult {
                point_index,
                metrics,
                trip_data_parquet,
                snapshot_counts_parquet,
            });
            points_processed += 1;
        }

        Ok(ChildExecutionSummary {
            points_processed,
            point_results,
        })
    }
}

fn run_single_simulation_with_exports(
    param_set: &ParameterSet,
    run_id: &str,
    shard_id: usize,
    point_index: usize,
) -> Result<(SimulationResult, Vec<u8>, Vec<u8>), String> {
    let mut world = World::new();
    let mut params = param_set.scenario_params();

    if params.simulation_end_time_ms.is_none() {
        let request_window_ms = params.request_window_ms;
        let end_time_ms = request_window_ms.saturating_add(2 * 60 * 60 * 1000);
        params.simulation_end_time_ms = Some(end_time_ms);
    }

    sim_core::scenario::build_scenario(&mut world, params);
    initialize_simulation(&mut world);

    let mut schedule = simulation_schedule();
    let _steps = run_until_empty(&mut world, &mut schedule, 2_000_000);

    let metrics = extract_metrics(&mut world);
    world
        .get_resource::<SimTelemetry>()
        .ok_or_else(|| "SimTelemetry resource not found".to_string())?;
    let snapshots = world
        .get_resource::<SimSnapshots>()
        .ok_or_else(|| "SimSnapshots resource not found".to_string())?;

    let trip_data_parquet = serialize_to_parquet_bytes(
        |path| write_trips_parquet(path, snapshots),
        run_id,
        shard_id,
        point_index,
        "trip-data",
    )?;
    let snapshot_counts_parquet = serialize_to_parquet_bytes(
        |path| write_snapshot_counts_parquet(path, snapshots),
        run_id,
        shard_id,
        point_index,
        "snapshot-counts",
    )?;

    Ok((metrics, trip_data_parquet, snapshot_counts_parquet))
}

fn serialize_to_parquet_bytes<F>(
    write_fn: F,
    run_id: &str,
    shard_id: usize,
    point_index: usize,
    suffix: &str,
) -> Result<Vec<u8>, String>
where
    F: FnOnce(&std::path::Path) -> Result<(), Box<dyn std::error::Error>>,
{
    let mut temp_path = std::env::temp_dir();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Failed to read clock for parquet export: {error}"))?
        .as_nanos();
    temp_path.push(format!(
        "serverless-shard-{run_id}-{shard_id}-{point_index}-{suffix}-{timestamp}.parquet"
    ));

    write_fn(&temp_path).map_err(|error| format!("Parquet export failed: {error}"))?;
    let bytes = fs::read(&temp_path)
        .map_err(|error| format!("Failed to read exported parquet file: {error}"))?;
    let _ = fs::remove_file(&temp_path);
    Ok(bytes)
}

fn parameter_set_for_index(
    payload: &ChildShardPayload,
    index: usize,
) -> Result<ParameterSet, String> {
    let mut params = ScenarioParams::default();
    let dims: Vec<(&str, &Vec<serde_json::Value>)> = payload
        .dimensions
        .iter()
        .map(|(name, values)| (name.as_str(), values))
        .collect();

    let mut remainder = index;
    for (name, values) in dims.iter().rev() {
        let radix = values.len();
        if radix == 0 {
            return Err(format!("Dimension '{name}' cannot be empty"));
        }
        let value_idx = remainder % radix;
        remainder /= radix;
        apply_dimension(&mut params, name, &values[value_idx])?;
    }

    if remainder != 0 {
        return Err("Index exceeds dimension product".to_string());
    }

    let seed = payload.seed as u64 ^ (index as u64);
    Ok(ParameterSet::new(
        params,
        format!("{}-shard-{}", payload.run_id, payload.shard_id),
        index,
        seed,
    ))
}

fn apply_dimension(
    params: &mut ScenarioParams,
    name: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    match name {
        "num_riders" => params.num_riders = as_usize(value, name)?,
        "num_drivers" => params.num_drivers = as_usize(value, name)?,
        "match_radius" => params.match_radius = as_u32(value, name)?,
        "commission_rate" => {
            let pricing = params
                .pricing_config
                .get_or_insert_with(PricingConfig::default);
            pricing.commission_rate = as_f64(value, name)?;
        }
        "base_fare" => {
            let pricing = params
                .pricing_config
                .get_or_insert_with(PricingConfig::default);
            pricing.base_fare = as_f64(value, name)?;
        }
        "per_km_rate" => {
            let pricing = params
                .pricing_config
                .get_or_insert_with(PricingConfig::default);
            pricing.per_km_rate = as_f64(value, name)?;
        }
        "surge_enabled" => {
            let pricing = params
                .pricing_config
                .get_or_insert_with(PricingConfig::default);
            pricing.surge_enabled = as_bool(value, name)?;
        }
        "surge_radius_k" => {
            let pricing = params
                .pricing_config
                .get_or_insert_with(PricingConfig::default);
            pricing.surge_radius_k = as_u32(value, name)?;
        }
        "surge_max_multiplier" => {
            let pricing = params
                .pricing_config
                .get_or_insert_with(PricingConfig::default);
            pricing.surge_max_multiplier = as_f64(value, name)?;
        }
        "epoch_ms" => {
            params.epoch_ms = match value {
                serde_json::Value::Null => None,
                _ => Some(as_i64(value, name)?),
            }
        }
        "simulation_duration_hours" => {
            params.simulation_end_time_ms = match value {
                serde_json::Value::Null => None,
                _ => {
                    let hours = as_u64(value, name)?;
                    Some(hours.saturating_mul(60 * 60 * 1000))
                }
            }
        }
        "matching_algorithm_type" => {
            params.matching_algorithm_type = Some(parse_matching_algorithm(value)?);
        }
        "batch_matching_enabled" => params.batch_matching_enabled = Some(as_bool(value, name)?),
        "batch_interval_secs" => params.batch_interval_secs = Some(as_u64(value, name)?),
        "eta_weight" => params.eta_weight = Some(as_f64(value, name)?),
        "traffic_profile" => params.traffic_profile = parse_traffic_profile(value)?,
        "dynamic_congestion_enabled" => params.dynamic_congestion_enabled = as_bool(value, name)?,
        "base_speed_kmh" => {
            params.base_speed_kmh = match value {
                serde_json::Value::Null => None,
                _ => Some(as_f64(value, name)?),
            }
        }
        _ => return Err(format!("Unsupported dimension '{name}'")),
    }

    Ok(())
}

fn as_u64(value: &serde_json::Value, name: &str) -> Result<u64, String> {
    value
        .as_u64()
        .ok_or_else(|| format!("Dimension '{name}' must be an unsigned integer"))
}

fn as_usize(value: &serde_json::Value, name: &str) -> Result<usize, String> {
    let parsed = as_u64(value, name)?;
    usize::try_from(parsed).map_err(|_| format!("Dimension '{name}' is too large"))
}

fn as_u32(value: &serde_json::Value, name: &str) -> Result<u32, String> {
    let parsed = as_u64(value, name)?;
    u32::try_from(parsed).map_err(|_| format!("Dimension '{name}' is too large"))
}

fn as_i64(value: &serde_json::Value, name: &str) -> Result<i64, String> {
    value
        .as_i64()
        .ok_or_else(|| format!("Dimension '{name}' must be a signed integer"))
}

fn as_f64(value: &serde_json::Value, name: &str) -> Result<f64, String> {
    value
        .as_f64()
        .ok_or_else(|| format!("Dimension '{name}' must be numeric"))
}

fn as_bool(value: &serde_json::Value, name: &str) -> Result<bool, String> {
    value
        .as_bool()
        .ok_or_else(|| format!("Dimension '{name}' must be boolean"))
}

fn parse_matching_algorithm(value: &serde_json::Value) -> Result<MatchingAlgorithmType, String> {
    let Some(raw) = value.as_str() else {
        return Err("Dimension 'matching_algorithm_type' must be a string".to_string());
    };

    match raw.to_ascii_lowercase().as_str() {
        "simple" => Ok(MatchingAlgorithmType::Simple),
        "costbased" | "cost_based" | "cost-based" => Ok(MatchingAlgorithmType::CostBased),
        "hungarian" => Ok(MatchingAlgorithmType::Hungarian),
        _ => Err(format!(
            "Unsupported matching_algorithm_type '{raw}' (expected simple, cost_based, or hungarian)"
        )),
    }
}

fn parse_traffic_profile(value: &serde_json::Value) -> Result<TrafficProfileKind, String> {
    let Some(raw) = value.as_str() else {
        return Err("Dimension 'traffic_profile' must be a string".to_string());
    };

    match raw.to_ascii_lowercase().as_str() {
        "none" => Ok(TrafficProfileKind::None),
        "berlin" => Ok(TrafficProfileKind::Berlin),
        _ => Err(format!(
            "Unsupported traffic_profile '{raw}' (expected none or berlin)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;
    use sim_serverless_sweep_core::contract::ChildShardPayload;

    use super::*;

    fn sample_payload() -> ChildShardPayload {
        ChildShardPayload {
            run_id: "run-456".to_string(),
            dimensions: BTreeMap::from([
                (
                    "commission_rate".to_string(),
                    vec![Value::from(0.1), Value::from(0.2)],
                ),
                ("num_drivers".to_string(), vec![Value::from(2)]),
                ("num_riders".to_string(), vec![Value::from(4)]),
            ]),
            total_points: 2,
            shard_id: 0,
            start_index: 0,
            end_index_exclusive: 1,
            seed: 1,
            failure_injection_shards: vec![],
        }
    }

    #[test]
    fn executes_only_requested_shard_bounds() {
        let payload = sample_payload();
        let executor = SimExperimentsShardExecutor;
        let summary = executor
            .execute_shard(&payload)
            .expect("shard execution should succeed");

        assert_eq!(summary.points_processed, 1);
    }

    #[test]
    fn rejects_unsupported_dimension_name() {
        let mut payload = sample_payload();
        payload.dimensions.insert(
            "unknown_dimension".to_string(),
            vec![serde_json::Value::from(1)],
        );
        payload.total_points = 2;

        let executor = SimExperimentsShardExecutor;
        let error = executor
            .execute_shard(&payload)
            .expect_err("unsupported dimension should fail");

        assert!(error.contains("Unsupported dimension 'unknown_dimension'"));
    }
}
