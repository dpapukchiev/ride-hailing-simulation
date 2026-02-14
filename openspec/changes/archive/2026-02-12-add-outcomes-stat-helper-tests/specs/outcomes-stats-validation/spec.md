## ADDED Requirements

### Requirement: Outcomes stats helpers compute deterministic distributions

The outcomes statistics helper functions SHALL return deterministic min/mean/max and percentile values for empty and non-empty datasets used by the outcomes UI.

#### Scenario: Empty timing input yields zeroed distribution

- **WHEN** timing distribution is computed for an empty trip list
- **THEN** min, mean, and max are all `0`
- **AND** percentile lookups return no value

#### Scenario: Non-empty timing input yields correct aggregate values

- **WHEN** timing distribution is computed for a non-empty trip list with known values
- **THEN** min and max reflect the smallest and largest values
- **AND** mean reflects integer average over all values
- **AND** percentile lookups follow nearest-rank index behavior used by the helper

#### Scenario: Invalid percentile input is rejected

- **WHEN** percentile is requested with a value greater than `100`
- **THEN** helper functions return no value

### Requirement: Outcomes fare percentile helper handles sorted float data consistently

The float percentile helper SHALL return deterministic values for sorted non-empty slices and no value for empty or out-of-range percentile requests.

#### Scenario: Sorted float input returns nearest-rank percentile

- **WHEN** a sorted float slice is provided with a valid percentile
- **THEN** the helper returns the value at the nearest-rank index

#### Scenario: Empty or out-of-range float percentile request returns no value

- **WHEN** the sorted float slice is empty or percentile is greater than `100`
- **THEN** the helper returns no value
