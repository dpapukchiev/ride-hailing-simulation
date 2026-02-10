use crate::metrics::SimulationResult;

pub(crate) fn export_to_json_impl(
    results: &[SimulationResult],
    file: std::fs::File,
) -> Result<(), Box<dyn std::error::Error>> {
    serde_json::to_writer_pretty(file, results)?;
    Ok(())
}
