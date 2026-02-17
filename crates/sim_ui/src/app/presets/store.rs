use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::model::{NamedPresetV1, PresetLibraryV1};
use super::{
    DeleteNamedPresetOutcome, PresetMetadata, PresetStoreError, SaveNamedPresetOutcome,
    ScenarioPresetV1, AUTOSAVE_PRESET_NAME, PRESETS_FILE_NAME, PRESET_FILE_VERSION,
};

pub(crate) fn presets_file_path() -> Result<PathBuf, PresetStoreError> {
    let cwd = std::env::current_dir().map_err(|error| {
        PresetStoreError::Io(format!("failed to read current directory: {error}"))
    })?;
    Ok(cwd.join(PRESETS_FILE_NAME))
}

pub(crate) fn load_active_preset(
    path: &Path,
) -> Result<Option<ScenarioPresetV1>, PresetStoreError> {
    let library = load_library(path)?;
    let Some(active_name) = library.active_preset.as_ref() else {
        return Ok(None);
    };
    Ok(library
        .presets
        .iter()
        .find(|preset| preset.name == *active_name)
        .map(|preset| preset.scenario.clone()))
}

pub(crate) fn save_autosave_preset(
    path: &Path,
    scenario: &ScenarioPresetV1,
) -> Result<(), PresetStoreError> {
    let mut library = match load_library(path) {
        Ok(library) => library,
        Err(PresetStoreError::InvalidFormat(_)) => PresetLibraryV1::empty(),
        Err(error) => return Err(error),
    };

    if let Some(existing) = library
        .presets
        .iter_mut()
        .find(|preset| preset.name == AUTOSAVE_PRESET_NAME)
    {
        existing.scenario = scenario.clone();
    } else {
        library.presets.push(NamedPresetV1 {
            name: AUTOSAVE_PRESET_NAME.to_string(),
            scenario: scenario.clone(),
        });
    }
    library.active_preset = Some(AUTOSAVE_PRESET_NAME.to_string());

    save_library_atomic(path, &library)
}

pub(crate) fn list_named_presets(path: &Path) -> Result<Vec<PresetMetadata>, PresetStoreError> {
    let library = load_library(path)?;
    let active_name = library.active_preset.as_ref();
    let mut presets = library
        .presets
        .into_iter()
        .map(|preset| PresetMetadata {
            is_active: active_name
                .map(|active| active == &preset.name)
                .unwrap_or(false),
            name: preset.name,
        })
        .collect::<Vec<_>>();
    presets.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(presets)
}

pub(crate) fn save_named_preset(
    path: &Path,
    name: &str,
    scenario: &ScenarioPresetV1,
    overwrite: bool,
) -> Result<SaveNamedPresetOutcome, PresetStoreError> {
    let mut library = match load_library(path) {
        Ok(library) => library,
        Err(PresetStoreError::InvalidFormat(_)) => PresetLibraryV1::empty(),
        Err(error) => return Err(error),
    };

    if let Some(existing) = library
        .presets
        .iter_mut()
        .find(|preset| preset.name == name)
    {
        if !overwrite {
            return Ok(SaveNamedPresetOutcome::Conflict);
        }
        existing.scenario = scenario.clone();
        library.active_preset = Some(name.to_string());
        save_library_atomic(path, &library)?;
        return Ok(SaveNamedPresetOutcome::Overwritten);
    }

    library.presets.push(NamedPresetV1 {
        name: name.to_string(),
        scenario: scenario.clone(),
    });
    library.active_preset = Some(name.to_string());
    save_library_atomic(path, &library)?;
    Ok(SaveNamedPresetOutcome::Saved)
}

pub(crate) fn load_named_preset(
    path: &Path,
    name: &str,
) -> Result<Option<ScenarioPresetV1>, PresetStoreError> {
    let mut library = load_library(path)?;
    let scenario = library
        .presets
        .iter()
        .find(|preset| preset.name == name)
        .map(|preset| preset.scenario.clone());

    if scenario.is_some() {
        library.active_preset = Some(name.to_string());
        save_library_atomic(path, &library)?;
    }

    Ok(scenario)
}

pub(crate) fn delete_named_preset(
    path: &Path,
    name: &str,
) -> Result<DeleteNamedPresetOutcome, PresetStoreError> {
    let mut library = load_library(path)?;
    let initial_len = library.presets.len();
    library.presets.retain(|preset| preset.name != name);

    if library.presets.len() == initial_len {
        return Ok(DeleteNamedPresetOutcome::NotFound);
    }

    if library
        .active_preset
        .as_ref()
        .map(|active| active == name)
        .unwrap_or(false)
    {
        library.active_preset = None;
    }

    save_library_atomic(path, &library)?;
    Ok(DeleteNamedPresetOutcome::Deleted)
}

pub(crate) fn export_library(path: &Path, export_path: &Path) -> Result<(), PresetStoreError> {
    let library = load_library(path)?;
    save_library_atomic(export_path, &library)
}

pub(crate) fn import_library(path: &Path, import_path: &Path) -> Result<(), PresetStoreError> {
    let import_contents = fs::read_to_string(import_path).map_err(|error| {
        PresetStoreError::Io(format!(
            "failed to read import file '{}': {error}",
            import_path.display()
        ))
    })?;

    let import_library: PresetLibraryV1 =
        serde_json::from_str(&import_contents).map_err(|error| {
            PresetStoreError::InvalidFormat(format!(
                "invalid import file '{}': {error}",
                import_path.display()
            ))
        })?;

    let candidate = validate_import(import_library, import_path)?;
    save_library_atomic(path, &candidate)
}

fn validate_import(
    library: PresetLibraryV1,
    import_path: &Path,
) -> Result<PresetLibraryV1, PresetStoreError> {
    if library.version != PRESET_FILE_VERSION {
        return Err(PresetStoreError::InvalidFormat(format!(
            "unsupported import file version {} in '{}'",
            library.version,
            import_path.display()
        )));
    }

    let mut names = HashSet::with_capacity(library.presets.len());
    for preset in &library.presets {
        if preset.name.trim().is_empty() {
            return Err(PresetStoreError::InvalidFormat(format!(
                "preset names must not be empty in '{}'",
                import_path.display()
            )));
        }
        if preset.name.trim() != preset.name {
            return Err(PresetStoreError::InvalidFormat(format!(
                "preset names must not have surrounding whitespace in '{}'",
                import_path.display()
            )));
        }
        if !names.insert(preset.name.clone()) {
            return Err(PresetStoreError::InvalidFormat(format!(
                "duplicate preset name '{}' in '{}'",
                preset.name,
                import_path.display()
            )));
        }
    }

    if let Some(active_name) = library.active_preset.as_ref() {
        if active_name.trim().is_empty() {
            return Err(PresetStoreError::InvalidFormat(format!(
                "active preset must not be empty in '{}'",
                import_path.display()
            )));
        }
        if active_name.trim() != active_name {
            return Err(PresetStoreError::InvalidFormat(format!(
                "active preset must not have surrounding whitespace in '{}'",
                import_path.display()
            )));
        }
        if !names.contains(active_name) {
            return Err(PresetStoreError::InvalidFormat(format!(
                "active preset '{}' not found in import file '{}'",
                active_name,
                import_path.display()
            )));
        }
    }

    Ok(library)
}

pub(super) fn load_library(path: &Path) -> Result<PresetLibraryV1, PresetStoreError> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PresetLibraryV1::empty())
        }
        Err(error) => {
            return Err(PresetStoreError::Io(format!(
                "failed to read presets file '{}': {error}",
                path.display()
            )))
        }
    };

    let library: PresetLibraryV1 = serde_json::from_str(&contents).map_err(|error| {
        PresetStoreError::InvalidFormat(format!(
            "invalid preset file '{}': {error}",
            path.display()
        ))
    })?;

    if library.version != PRESET_FILE_VERSION {
        return Err(PresetStoreError::InvalidFormat(format!(
            "unsupported preset file version {} in '{}'",
            library.version,
            path.display()
        )));
    }

    Ok(library)
}

fn save_library_atomic(path: &Path, library: &PresetLibraryV1) -> Result<(), PresetStoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PresetStoreError::Io(format!(
                "failed to create presets directory '{}': {error}",
                parent.display()
            ))
        })?;
    }

    let serialized = serde_json::to_string_pretty(library).map_err(|error| {
        PresetStoreError::Io(format!("failed to serialize presets to json: {error}"))
    })?;

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let temp_path = path.with_extension(format!("json.tmp.{nanos}"));
    let mut temp_file = File::create(&temp_path).map_err(|error| {
        PresetStoreError::Io(format!(
            "failed to create temp presets file '{}': {error}",
            temp_path.display()
        ))
    })?;
    temp_file
        .write_all(serialized.as_bytes())
        .map_err(|error| {
            PresetStoreError::Io(format!(
                "failed to write temp presets file '{}': {error}",
                temp_path.display()
            ))
        })?;
    temp_file.sync_all().map_err(|error| {
        PresetStoreError::Io(format!(
            "failed to flush temp presets file '{}': {error}",
            temp_path.display()
        ))
    })?;

    replace_file(&temp_path, path)?;
    Ok(())
}

fn replace_file(temp_path: &Path, target_path: &Path) -> Result<(), PresetStoreError> {
    match fs::rename(temp_path, target_path) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            if target_path.exists() {
                fs::remove_file(target_path).map_err(|remove_error| {
                    let _ = fs::remove_file(temp_path);
                    PresetStoreError::Io(format!(
                        "failed to replace presets file '{}': {first_error}; remove failed: {remove_error}",
                        target_path.display()
                    ))
                })?;
                fs::rename(temp_path, target_path).map_err(|rename_error| {
                    let _ = fs::remove_file(temp_path);
                    PresetStoreError::Io(format!(
                        "failed to move temp presets file '{}' to '{}': {rename_error}",
                        temp_path.display(),
                        target_path.display()
                    ))
                })
            } else {
                let _ = fs::remove_file(temp_path);
                Err(PresetStoreError::Io(format!(
                    "failed to move temp presets file '{}' to '{}': {first_error}",
                    temp_path.display(),
                    target_path.display()
                )))
            }
        }
    }
}
