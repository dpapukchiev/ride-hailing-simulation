//! Time-of-day and day-of-week patterns for spawn rate multipliers.
//!
//! This module defines realistic demand and supply patterns that vary by hour of day
//! and day of week. Patterns are used to configure TimeOfDayDistribution instances.

/// Default weekday pattern for rider demand (Monday-Thursday, Sunday).
/// Multipliers for each hour (0-23) representing demand relative to base rate.
pub const RIDER_WEEKDAY_PATTERN: [f64; 24] = [
    0.4, // 12 AM - 1 AM
    0.4, // 1 AM - 2 AM
    0.4, // 2 AM - 3 AM
    0.4, // 3 AM - 4 AM
    0.4, // 4 AM - 5 AM
    0.4, // 5 AM - 6 AM
    0.4, // 6 AM - 7 AM
    2.5, // 7 AM - 8 AM (morning rush)
    3.0, // 8 AM - 9 AM (morning rush peak)
    2.0, // 9 AM - 10 AM (morning rush)
    1.2, // 10 AM - 11 AM
    1.2, // 11 AM - 12 PM
    1.2, // 12 PM - 1 PM
    1.2, // 1 PM - 2 PM
    1.2, // 2 PM - 3 PM
    1.2, // 3 PM - 4 PM
    1.2, // 4 PM - 5 PM
    2.8, // 5 PM - 6 PM (evening rush)
    3.2, // 6 PM - 7 PM (evening rush peak)
    2.5, // 7 PM - 8 PM (evening rush)
    1.5, // 8 PM - 9 PM
    1.5, // 9 PM - 10 PM
    1.5, // 10 PM - 11 PM
    1.5, // 11 PM - 12 AM
];

/// Friday and Saturday pattern for rider demand.
/// Higher demand in the evenings compared to weekdays.
pub const RIDER_WEEKEND_PATTERN: [f64; 24] = [
    2.0, // 12 AM - 1 AM
    2.5, // 1 AM - 2 AM
    3.0, // 2 AM - 3 AM
    2.8, // 3 AM - 4 AM
    2.2, // 4 AM - 5 AM
    1.8, // 5 AM - 6 AM
    1.5, // 6 AM - 7 AM
    1.2, // 7 AM - 8 AM
    1.0, // 8 AM - 9 AM
    0.8, // 9 AM - 10 AM
    0.6, // 10 AM - 11 AM
    0.5, // 11 AM - 12 PM
    0.4, // 12 PM - 1 PM
    0.4, // 1 PM - 2 PM
    0.4, // 2 PM - 3 PM
    0.4, // 3 PM - 4 PM
    0.5, // 4 PM - 5 PM
    2.8, // 5 PM - 6 PM (evening rush)
    3.5, // 6 PM - 7 PM (evening rush peak)
    3.0, // 7 PM - 8 PM (evening rush)
    2.5, // 8 PM - 9 PM
    2.0, // 9 PM - 10 PM
    1.8, // 10 PM - 11 PM
    1.5, // 11 PM - 12 AM
];

/// Default weekday pattern for driver supply (Monday-Thursday, Sunday).
/// Multipliers for each hour (0-23) representing supply relative to base rate.
/// Supply is more consistent than demand but still varies with rush hours.
pub const DRIVER_WEEKDAY_PATTERN: [f64; 24] = [
    0.6, // 12 AM - 1 AM
    0.6, // 1 AM - 2 AM
    0.6, // 2 AM - 3 AM
    0.6, // 3 AM - 4 AM
    0.6, // 4 AM - 5 AM
    0.6, // 5 AM - 6 AM
    1.0, // 6 AM - 7 AM
    1.5, // 7 AM - 8 AM (morning rush)
    1.8, // 8 AM - 9 AM (morning rush peak)
    1.6, // 9 AM - 10 AM (morning rush)
    1.2, // 10 AM - 11 AM
    1.2, // 11 AM - 12 PM
    1.2, // 12 PM - 1 PM
    1.2, // 1 PM - 2 PM
    1.2, // 2 PM - 3 PM
    1.2, // 3 PM - 4 PM
    1.2, // 4 PM - 5 PM
    1.7, // 5 PM - 6 PM (evening rush)
    2.0, // 6 PM - 7 PM (evening rush peak)
    1.8, // 7 PM - 8 PM (evening rush)
    1.3, // 8 PM - 9 PM
    1.3, // 9 PM - 10 PM
    1.3, // 10 PM - 11 PM
    1.3, // 11 PM - 12 AM
];

/// Friday and Saturday pattern for driver supply.
/// Higher supply in the evenings to match increased demand.
pub const DRIVER_WEEKEND_PATTERN: [f64; 24] = [
    0.7, // 12 AM - 1 AM
    0.8, // 1 AM - 2 AM
    0.9, // 2 AM - 3 AM
    1.0, // 3 AM - 4 AM
    1.1, // 4 AM - 5 AM
    1.2, // 5 AM - 6 AM
    1.3, // 6 AM - 7 AM
    1.5, // 7 AM - 8 AM
    1.6, // 8 AM - 9 AM
    1.4, // 9 AM - 10 AM
    1.2, // 10 AM - 11 AM
    1.0, // 11 AM - 12 PM
    0.9, // 12 PM - 1 PM
    0.8, // 1 PM - 2 PM
    0.7, // 2 PM - 3 PM
    0.7, // 3 PM - 4 PM
    0.8, // 4 PM - 5 PM
    1.7, // 5 PM - 6 PM (evening rush)
    2.2, // 6 PM - 7 PM (evening rush peak)
    2.0, // 7 PM - 8 PM (evening rush)
    1.8, // 8 PM - 9 PM
    1.6, // 9 PM - 10 PM
    1.4, // 10 PM - 11 PM
    1.2, // 11 PM - 12 AM
];

/// Apply rider demand patterns to a TimeOfDayDistribution.
/// Uses weekday pattern for Monday-Thursday and Sunday, weekend pattern for Friday-Saturday.
pub fn apply_rider_patterns(
    mut dist: crate::distributions::TimeOfDayDistribution,
) -> crate::distributions::TimeOfDayDistribution {
    // Apply weekday pattern to Monday (0), Tuesday (1), Wednesday (2), Thursday (3), Sunday (6)
    for &day in &[0, 1, 2, 3, 6] {
        dist = dist.set_day_multipliers(day, RIDER_WEEKDAY_PATTERN);
    }

    // Apply weekend pattern to Friday (4) and Saturday (5)
    dist = dist.set_day_multipliers(4, RIDER_WEEKEND_PATTERN);
    dist = dist.set_day_multipliers(5, RIDER_WEEKEND_PATTERN);

    dist
}

/// Apply driver supply patterns to a TimeOfDayDistribution.
/// Uses weekday pattern for Monday-Thursday and Sunday, weekend pattern for Friday-Saturday.
pub fn apply_driver_patterns(
    mut dist: crate::distributions::TimeOfDayDistribution,
) -> crate::distributions::TimeOfDayDistribution {
    // Apply weekday pattern to Monday (0), Tuesday (1), Wednesday (2), Thursday (3), Sunday (6)
    for &day in &[0, 1, 2, 3, 6] {
        dist = dist.set_day_multipliers(day, DRIVER_WEEKDAY_PATTERN);
    }

    // Apply weekend pattern to Friday (4) and Saturday (5)
    dist = dist.set_day_multipliers(4, DRIVER_WEEKEND_PATTERN);
    dist = dist.set_day_multipliers(5, DRIVER_WEEKEND_PATTERN);

    dist
}
