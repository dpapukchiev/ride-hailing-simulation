use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::defaults::AppDefaults;

use super::model::{NamedPresetV1, PresetLibraryV1};
use super::store::load_library;
use super::*;

fn unique_test_path(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("sim_ui_preset_tests_{label}_{nanos}"))
}

#[test]
fn load_missing_file_returns_empty_library() {
    let path = unique_test_path("missing").join(PRESETS_FILE_NAME);
    let loaded = load_library(&path).expect("missing file should be treated as empty");
    assert_eq!(loaded.version, PRESET_FILE_VERSION);
    assert!(loaded.presets.is_empty());
    assert!(loaded.active_preset.is_none());
}

#[test]
fn malformed_json_is_reported_as_recoverable_error() {
    let path = unique_test_path("malformed").join(PRESETS_FILE_NAME);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("test directory should be creatable");
    }
    fs::write(&path, "{ definitely-not-json ").expect("test fixture should be written");

    let result = load_library(&path);
    assert!(matches!(result, Err(PresetStoreError::InvalidFormat(_))));
}

#[test]
fn save_and_load_round_trip() {
    let path = unique_test_path("round_trip").join(PRESETS_FILE_NAME);
    let defaults = AppDefaults::new();
    let scenario = ScenarioPresetV1::from_defaults(&defaults);

    save_autosave_preset(&path, &scenario).expect("save should succeed");
    let loaded = load_active_preset(&path)
        .expect("load should succeed")
        .expect("active preset should exist");
    assert_eq!(loaded, scenario);
}

#[test]
fn apply_to_defaults_normalizes_out_of_range_values() {
    let mut defaults = AppDefaults::new();
    let mut scenario = ScenarioPresetV1::from_defaults(&defaults);
    scenario.num_riders = 0;
    scenario.request_window_hours = 0;
    scenario.simulation_duration_hours = 500;
    scenario.min_trip_km = 300.0;
    scenario.max_trip_km = 0.0;
    scenario.rider_cancel_min_mins = 900;
    scenario.rider_cancel_max_mins = 0;
    scenario.commission_rate = -1.0;
    scenario.accept_probability = 2.5;
    scenario.driver_pickup_distance_penalty = 2.0;
    scenario.base_speed_kmh = 5.0;
    scenario.start_month = 99;
    scenario.start_minute = 99;
    scenario.osrm_endpoint = "   ".to_string();

    scenario.apply_to_defaults(&mut defaults);

    assert_eq!(defaults.num_riders, 1);
    assert_eq!(defaults.request_window_hours, 1);
    assert_eq!(defaults.simulation_duration_hours, 168);
    assert_eq!(defaults.min_trip_km, 100.0);
    assert_eq!(defaults.max_trip_km, 100.0);
    assert_eq!(defaults.rider_cancel_min_mins, 600);
    assert_eq!(defaults.rider_cancel_max_mins, 600);
    assert_eq!(defaults.commission_rate, 0.0);
    assert_eq!(defaults.accept_probability, 1.0);
    assert_eq!(defaults.driver_pickup_distance_penalty, 0.0);
    assert_eq!(defaults.base_speed_kmh, 10.0);
    assert_eq!(defaults.start_month, 12);
    assert_eq!(defaults.start_minute, 59);
    assert_eq!(defaults.osrm_endpoint, "http://localhost:5000");
}

#[test]
fn save_named_preset_returns_conflict_without_overwrite() {
    let path = unique_test_path("named_conflict").join(PRESETS_FILE_NAME);
    let defaults = AppDefaults::new();
    let scenario = ScenarioPresetV1::from_defaults(&defaults);

    let first = save_named_preset(&path, "morning", &scenario, false)
        .expect("initial named save should succeed");
    assert_eq!(first, SaveNamedPresetOutcome::Saved);

    let conflict = save_named_preset(&path, "morning", &scenario, false)
        .expect("duplicate save should return conflict");
    assert_eq!(conflict, SaveNamedPresetOutcome::Conflict);
}

#[test]
fn overwrite_replaces_payload_and_sets_active_name() {
    let path = unique_test_path("overwrite").join(PRESETS_FILE_NAME);
    let defaults = AppDefaults::new();
    let first = ScenarioPresetV1::from_defaults(&defaults);
    let mut second = first.clone();
    second.num_riders = first.num_riders.saturating_add(99);

    let first_result =
        save_named_preset(&path, "weekday", &first, false).expect("first save should succeed");
    assert_eq!(first_result, SaveNamedPresetOutcome::Saved);

    let overwrite =
        save_named_preset(&path, "weekday", &second, true).expect("overwrite save should succeed");
    assert_eq!(overwrite, SaveNamedPresetOutcome::Overwritten);

    let loaded = load_named_preset(&path, "weekday")
        .expect("load should succeed")
        .expect("saved preset should exist");
    assert_eq!(loaded, second);

    let metadata = list_named_presets(&path).expect("list should succeed");
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata[0].name, "weekday");
    assert!(metadata[0].is_active);
}

#[test]
fn delete_removes_named_preset_from_disk() {
    let path = unique_test_path("delete").join(PRESETS_FILE_NAME);
    let defaults = AppDefaults::new();
    let scenario = ScenarioPresetV1::from_defaults(&defaults);

    save_named_preset(&path, "to-delete", &scenario, false).expect("save should succeed");

    let result = delete_named_preset(&path, "to-delete").expect("delete should succeed");
    assert_eq!(result, DeleteNamedPresetOutcome::Deleted);

    let metadata = list_named_presets(&path).expect("list should succeed");
    assert!(metadata.is_empty());
}

#[test]
fn deleting_active_named_preset_clears_active_marker() {
    let path = unique_test_path("delete_active").join(PRESETS_FILE_NAME);
    let defaults = AppDefaults::new();
    let scenario = ScenarioPresetV1::from_defaults(&defaults);

    save_named_preset(&path, "active", &scenario, false).expect("save should succeed");
    let deleted = delete_named_preset(&path, "active").expect("delete should succeed");
    assert_eq!(deleted, DeleteNamedPresetOutcome::Deleted);

    let metadata = list_named_presets(&path).expect("list should succeed");
    assert!(metadata.is_empty());
    let loaded_active = load_active_preset(&path).expect("active load should succeed");
    assert!(loaded_active.is_none());
}

#[test]
fn import_with_unsupported_version_keeps_existing_library_unchanged() {
    let store_path = unique_test_path("import_invalid_version").join(PRESETS_FILE_NAME);
    let import_path = unique_test_path("import_invalid_version_src").join("library.json");
    let defaults = AppDefaults::new();
    let baseline = ScenarioPresetV1::from_defaults(&defaults);

    save_named_preset(&store_path, "baseline", &baseline, false)
        .expect("baseline save should succeed");

    if let Some(parent) = import_path.parent() {
        fs::create_dir_all(parent).expect("import fixture directory should be creatable");
    }
    fs::write(
        &import_path,
        r#"{
  "version": 99,
  "active_preset": null,
  "presets": []
}"#,
    )
    .expect("import fixture should be written");

    let result = import_library(&store_path, &import_path);
    assert!(matches!(result, Err(PresetStoreError::InvalidFormat(_))));

    let listed = list_named_presets(&store_path).expect("list should still succeed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "baseline");
    assert!(listed[0].is_active);
    let loaded = load_named_preset(&store_path, "baseline")
        .expect("load should succeed")
        .expect("baseline preset should still exist");
    assert_eq!(loaded, baseline);
}

#[test]
fn import_valid_library_replaces_existing_library() {
    let store_path = unique_test_path("import_valid_replace").join(PRESETS_FILE_NAME);
    let import_path = unique_test_path("import_valid_replace_src").join("library.json");
    let defaults = AppDefaults::new();
    let old_scenario = ScenarioPresetV1::from_defaults(&defaults);
    let mut imported_scenario = old_scenario.clone();
    imported_scenario.num_drivers = imported_scenario.num_drivers.saturating_add(77);

    save_named_preset(&store_path, "existing", &old_scenario, false)
        .expect("existing save should succeed");

    let import_library_payload = PresetLibraryV1 {
        version: PRESET_FILE_VERSION,
        active_preset: Some("imported".to_string()),
        presets: vec![NamedPresetV1 {
            name: "imported".to_string(),
            scenario: imported_scenario.clone(),
        }],
    };

    if let Some(parent) = import_path.parent() {
        fs::create_dir_all(parent).expect("import fixture directory should be creatable");
    }
    let serialized = serde_json::to_string_pretty(&import_library_payload)
        .expect("import payload should serialize");
    fs::write(&import_path, serialized).expect("import payload should be written");

    import_library(&store_path, &import_path).expect("import should succeed");

    let listed = list_named_presets(&store_path).expect("list should succeed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "imported");
    assert!(listed[0].is_active);
    let loaded = load_named_preset(&store_path, "imported")
        .expect("load should succeed")
        .expect("imported preset should exist");
    assert_eq!(loaded, imported_scenario);
}

#[test]
fn strict_import_validation_rejects_invalid_payloads_without_mutating_store() {
    let store_path = unique_test_path("strict_validation_store").join(PRESETS_FILE_NAME);
    let import_path = unique_test_path("strict_validation_import").join("library.json");
    let defaults = AppDefaults::new();
    let baseline_scenario = ScenarioPresetV1::from_defaults(&defaults);

    save_named_preset(&store_path, "baseline", &baseline_scenario, false)
        .expect("baseline save should succeed");

    let invalid_payloads = vec![
        (
            "unknown_field",
            r#"{
  "version": 1,
  "active_preset": null,
  "presets": [],
  "unexpected": true
}"#
            .to_string(),
        ),
        (
            "trimmed_name",
            serde_json::to_string_pretty(&PresetLibraryV1 {
                version: PRESET_FILE_VERSION,
                active_preset: Some(" baseline ".to_string()),
                presets: vec![NamedPresetV1 {
                    name: " baseline ".to_string(),
                    scenario: baseline_scenario.clone(),
                }],
            })
            .expect("payload should serialize"),
        ),
        (
            "duplicate_name",
            serde_json::to_string_pretty(&PresetLibraryV1 {
                version: PRESET_FILE_VERSION,
                active_preset: Some("dup".to_string()),
                presets: vec![
                    NamedPresetV1 {
                        name: "dup".to_string(),
                        scenario: baseline_scenario.clone(),
                    },
                    NamedPresetV1 {
                        name: "dup".to_string(),
                        scenario: baseline_scenario.clone(),
                    },
                ],
            })
            .expect("payload should serialize"),
        ),
        (
            "missing_active_reference",
            serde_json::to_string_pretty(&PresetLibraryV1 {
                version: PRESET_FILE_VERSION,
                active_preset: Some("missing".to_string()),
                presets: vec![NamedPresetV1 {
                    name: "present".to_string(),
                    scenario: baseline_scenario.clone(),
                }],
            })
            .expect("payload should serialize"),
        ),
    ];

    for (label, payload) in invalid_payloads {
        if let Some(parent) = import_path.parent() {
            fs::create_dir_all(parent).expect("import fixture directory should be creatable");
        }
        fs::write(&import_path, payload).expect("import fixture should be written");

        let result = import_library(&store_path, &import_path);
        assert!(
            matches!(result, Err(PresetStoreError::InvalidFormat(_))),
            "case '{label}' should reject invalid import"
        );

        let listed = list_named_presets(&store_path).expect("list should still succeed");
        assert_eq!(listed.len(), 1, "case '{label}' should keep preset count");
        assert_eq!(
            listed[0].name, "baseline",
            "case '{label}' should keep name"
        );
        assert!(
            listed[0].is_active,
            "case '{label}' should keep active marker"
        );

        let loaded = load_named_preset(&store_path, "baseline")
            .expect("load should succeed")
            .expect("baseline should still exist");
        assert_eq!(
            loaded, baseline_scenario,
            "case '{label}' should keep baseline data"
        );
    }
}

#[test]
fn export_import_round_trip_preserves_library_and_active_marker() {
    let source_store_path = unique_test_path("round_trip_source").join(PRESETS_FILE_NAME);
    let target_store_path = unique_test_path("round_trip_target").join(PRESETS_FILE_NAME);
    let transfer_path = unique_test_path("round_trip_transfer").join("library.json");
    let defaults = AppDefaults::new();

    let mut morning = ScenarioPresetV1::from_defaults(&defaults);
    morning.num_riders = morning.num_riders.saturating_add(10);
    let mut evening = ScenarioPresetV1::from_defaults(&defaults);
    evening.num_drivers = evening.num_drivers.saturating_add(20);

    save_named_preset(&source_store_path, "morning", &morning, false)
        .expect("morning save should succeed");
    save_named_preset(&source_store_path, "evening", &evening, false)
        .expect("evening save should succeed");
    let _ = load_named_preset(&source_store_path, "evening")
        .expect("load should succeed")
        .expect("evening preset should exist");

    export_library(&source_store_path, &transfer_path).expect("export should succeed");
    import_library(&target_store_path, &transfer_path).expect("import should succeed");

    let metadata = list_named_presets(&target_store_path).expect("list should succeed");
    assert_eq!(metadata.len(), 2);
    assert_eq!(metadata[0].name, "evening");
    assert!(metadata[0].is_active);
    assert_eq!(metadata[1].name, "morning");
    assert!(!metadata[1].is_active);

    let loaded_evening = load_named_preset(&target_store_path, "evening")
        .expect("load should succeed")
        .expect("evening preset should exist");
    assert_eq!(loaded_evening, evening);

    let loaded_morning = load_named_preset(&target_store_path, "morning")
        .expect("load should succeed")
        .expect("morning preset should exist");
    assert_eq!(loaded_morning, morning);
}
