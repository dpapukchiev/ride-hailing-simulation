//! Example: Parameter sweep for pricing and supply/demand analysis.
//!
//! This example demonstrates how to:
//! 1. Define a parameter space (grid search)
//! 2. Run multiple simulations in parallel
//! 3. Calculate health scores
//! 4. Find optimal parameter combinations
//! 5. Export results to Parquet/JSON

use sim_experiments::{
    export_to_json, export_to_parquet, find_best_parameters, find_best_result_index,
    HealthWeights, ParameterSpace, run_parallel_experiments,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting parameter sweep experiment...");

    // Define parameter space: explore commission rates and supply/demand balance
    // You can also vary simulation start time and duration
    let space = ParameterSpace::grid()
        .commission_rate(vec![0.0, 0.1, 0.2, 0.3]) // 0%, 10%, 20%, 30% commission
        .num_drivers(vec![50, 100, 150])           // Low, medium, high supply
        .num_riders(vec![300, 500, 700])           // Low, medium, high demand
        // Optional: vary simulation start time (epoch_ms) and duration
        // .epoch_ms(vec![Some(1700000000000), Some(1700086400000)]) // Different start times
        // .simulation_duration_hours(vec![Some(4), Some(8), Some(12)]); // Different durations
        ;

    println!("Generating parameter sets...");
    let parameter_sets = space.generate();
    println!("Generated {} parameter combinations", parameter_sets.len());

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
    }

    // Export results
    println!("\nExporting results...");
    export_to_json(&results, "experiment_results.json")?;
    println!("Exported to experiment_results.json");

    export_to_parquet(&results, "experiment_results.parquet")?;
    println!("Exported to experiment_results.parquet");

    println!("\nExperiment complete!");

    Ok(())
}
