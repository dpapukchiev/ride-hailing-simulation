use crate::health::{calculate_health_scores, HealthWeights};
use crate::metrics::SimulationResult;
use crate::parameters::ParameterSet;

pub(crate) fn find_best_index_by_health(
    results: &[SimulationResult],
    weights: &HealthWeights,
) -> Option<usize> {
    if results.is_empty() {
        return None;
    }

    let scores = calculate_health_scores(results, weights);
    let (best_idx, _best_score) = scores
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();

    Some(best_idx)
}

pub(crate) fn find_best_parameters_impl<'a>(
    results: &'a [SimulationResult],
    parameter_sets: &'a [ParameterSet],
    weights: &'a HealthWeights,
) -> Option<&'a ParameterSet> {
    if results.is_empty() || results.len() != parameter_sets.len() {
        return None;
    }

    let best_idx = find_best_index_by_health(results, weights)?;
    Some(&parameter_sets[best_idx])
}
