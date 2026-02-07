//! Example: Parameter sweep for pricing and supply/demand analysis.
//!
//! This example demonstrates how to:
//! 1. Select a pre-defined parameter space
//! 2. Run multiple simulations in parallel
//! 3. Calculate health scores
//! 4. Find optimal parameter combinations
//! 5. Export results to Parquet/JSON
//!
//! To use a different parameter space, change the function call in main().

use sim_core::scenario::MatchingAlgorithmType;
use sim_experiments::{
    export_to_csv, 
    // export_to_json, export_to_parquet, 
    find_best_parameters, find_best_result_index,
    HealthWeights, run_parallel_experiments,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting parameter sweep experiment...");

    // Select which parameter space to use:
    // - comprehensive_space(): Full exploration of all dimensions
    // - pricing_focused_space(): Pricing analysis with fixed supply/demand
    // - matching_focused_space(): Matching algorithm comparison
    // - supply_demand_space(): Supply/demand analysis
    // - minimal_space(): Quick testing
    let space = sim_experiments::parameter_spaces::surge_pricing_space();

    println!("Generating parameter sets...");
    let parameter_sets = space.generate();
    println!("Generated {} parameter combinations (invalid combinations filtered out)", parameter_sets.len());

    // Run experiments in parallel (uses all available CPU cores by default)
    println!("Running simulations in parallel...");
    let results = run_parallel_experiments(parameter_sets.clone(), None);
    println!("Completed {} simulations", results.len());

    // Calculate health scores
    println!("Calculating health scores...");
    let weights = HealthWeights::default();
    let best_idx = find_best_result_index(&results, &weights)
        .expect("No results to analyze");

    println!("\n=== Best Configuration ===");
    let best_result = &results[best_idx];
    println!("Conversion rate: {:.2}%", best_result.conversion_rate * 100.0);
    println!("Platform revenue: ${:.2}", best_result.platform_revenue);
    println!("Driver payouts: ${:.2}", best_result.driver_payouts);
    println!("Avg time to match: {:.1}s", best_result.avg_time_to_match_ms / 1000.0);
    println!("Avg time to pickup: {:.1}s", best_result.avg_time_to_pickup_ms / 1000.0);
    println!("Abandoned riders: {}", best_result.abandoned_quote_riders);

    if let Some(best_params) = find_best_parameters(&results, &parameter_sets, &weights) {
        println!("\n=== Best Parameters ===");
        if let Some(pricing) = &best_params.params.pricing_config {
            println!("Commission rate: {:.1}%", pricing.commission_rate * 100.0);
            println!("Base fare: ${:.2}", pricing.base_fare);
            println!("Per km rate: ${:.2}", pricing.per_km_rate);
            println!("Surge enabled: {}", pricing.surge_enabled);
        }
        println!("Number of drivers: {}", best_params.params.num_drivers);
        println!("Number of riders: {}", best_params.params.num_riders);
        println!("Match radius: {}", best_params.params.match_radius);
        if let Some(alg_type) = &best_params.params.matching_algorithm_type {
            let alg_name = match alg_type {
                MatchingAlgorithmType::Simple => "Simple",
                MatchingAlgorithmType::CostBased => "Cost-based",
                MatchingAlgorithmType::Hungarian => "Hungarian",
            };
            println!("Matching algorithm: {}", alg_name);
        }
        if let Some(batch_enabled) = best_params.params.batch_matching_enabled {
            println!("Batch matching enabled: {}", batch_enabled);
        }
        if let Some(batch_interval) = best_params.params.batch_interval_secs {
            println!("Batch interval: {}s", batch_interval);
        }
        if let Some(eta_weight) = best_params.params.eta_weight {
            println!("ETA weight: {:.2}", eta_weight);
        }
    }

    // Export results
    println!("\nExporting results...");
    // export_to_json(&results, "experiment_results.json")?;
    // println!("Exported to experiment_results.json");

    // export_to_parquet(&results, "experiment_results.parquet")?;
    // println!("Exported to experiment_results.parquet");

    export_to_csv(&results, &parameter_sets, "experiment_results.csv")?;
    println!("Exported to experiment_results.csv");

    println!("\nExperiment complete!");

    Ok(())
}
