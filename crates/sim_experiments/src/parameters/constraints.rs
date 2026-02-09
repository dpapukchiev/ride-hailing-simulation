use super::combinations::ParameterCombination;
use sim_core::scenario::MatchingAlgorithmType;

pub(super) fn is_valid_matching_config(
    matching_algorithm_type: MatchingAlgorithmType,
    batch_matching_enabled: bool,
) -> bool {
    matching_algorithm_type != MatchingAlgorithmType::Hungarian || batch_matching_enabled
}

/// Returns false for invalid combinations that should be discarded.
pub(super) fn is_valid_combination(combo: &ParameterCombination) -> bool {
    is_valid_matching_config(combo.matching_algorithm_type, combo.batch_matching_enabled)
}
