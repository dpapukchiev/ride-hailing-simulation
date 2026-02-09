use super::*;

#[test]
fn test_grid_search_single_parameter() {
    let space = ParameterSpace::grid().commission_rate(vec![0.0, 0.1, 0.2]);
    let sets = space.generate();
    assert_eq!(sets.len(), 3);
}

#[test]
fn test_grid_search_multiple_parameters() {
    let space = ParameterSpace::grid()
        .commission_rate(vec![0.0, 0.1])
        .num_drivers(vec![50, 100]);
    let sets = space.generate();
    assert_eq!(sets.len(), 4);
}

#[test]
fn test_random_sampling() {
    let space = ParameterSpace::grid()
        .commission_rate(vec![0.0, 0.1, 0.2, 0.3])
        .num_drivers(vec![50, 100, 150]);
    let sets = space.sample_random(10, 42);
    assert_eq!(sets.len(), 10);
}

#[test]
fn test_epoch_ms_and_duration() {
    let space = ParameterSpace::grid()
        .epoch_ms(vec![Some(1700000000000), Some(1700086400000)])
        .simulation_duration_hours(vec![Some(4), Some(8)]);
    let sets = space.generate();
    assert_eq!(sets.len(), 4);

    let epoch1 = Some(1700000000000);
    let epoch2 = Some(1700086400000);
    let mut found_epoch1_dur4 = false;
    let mut found_epoch1_dur8 = false;
    let mut found_epoch2_dur4 = false;
    let mut found_epoch2_dur8 = false;

    let request_window = sets[0].scenario_params().request_window_ms;

    for set in &sets {
        let params = set.scenario_params();
        let duration_hours = params
            .simulation_end_time_ms
            .map(|end_time| (end_time - request_window) / (60 * 60 * 1000));

        match (params.epoch_ms, duration_hours) {
            (e, Some(4)) if e == epoch1 => found_epoch1_dur4 = true,
            (e, Some(8)) if e == epoch1 => found_epoch1_dur8 = true,
            (e, Some(4)) if e == epoch2 => found_epoch2_dur4 = true,
            (e, Some(8)) if e == epoch2 => found_epoch2_dur8 = true,
            _ => {}
        }

        if let Some(dur) = duration_hours {
            let expected_end = request_window + dur * 60 * 60 * 1000;
            assert_eq!(params.simulation_end_time_ms, Some(expected_end));
        }
    }

    assert!(
        found_epoch1_dur4,
        "Missing combination: epoch1 + duration 4"
    );
    assert!(
        found_epoch1_dur8,
        "Missing combination: epoch1 + duration 8"
    );
    assert!(
        found_epoch2_dur4,
        "Missing combination: epoch2 + duration 4"
    );
    assert!(
        found_epoch2_dur8,
        "Missing combination: epoch2 + duration 8"
    );
}

#[test]
fn test_invalid_combinations_filtered() {
    let space = ParameterSpace::grid()
        .matching_algorithm_type(vec![
            MatchingAlgorithmType::Simple,
            MatchingAlgorithmType::Hungarian,
        ])
        .batch_matching_enabled(vec![false, true]);

    let sets = space.generate();
    assert_eq!(sets.len(), 3);

    for set in &sets {
        if set.params.matching_algorithm_type == Some(MatchingAlgorithmType::Hungarian) {
            assert_eq!(
                set.params.batch_matching_enabled,
                Some(true),
                "Hungarian matching must have batch matching enabled"
            );
        }
    }
}
