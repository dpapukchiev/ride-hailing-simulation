mod model;
mod scenario;
mod store;

#[cfg(test)]
mod tests;

use std::fmt;

pub(crate) const PRESETS_FILE_NAME: &str = "sim_ui_presets.json";
pub(super) const PRESET_FILE_VERSION: u32 = 1;
pub(super) const AUTOSAVE_PRESET_NAME: &str = "autosave";

#[derive(Debug)]
pub(crate) enum PresetStoreError {
    Io(String),
    InvalidFormat(String),
}

impl fmt::Display for PresetStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PresetStoreError::Io(message) => write!(f, "{message}"),
            PresetStoreError::InvalidFormat(message) => write!(f, "{message}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PresetMetadata {
    pub(crate) name: String,
    pub(crate) is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SaveNamedPresetOutcome {
    Saved,
    Overwritten,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeleteNamedPresetOutcome {
    Deleted,
    NotFound,
}

pub(crate) use scenario::ScenarioPresetV1;
pub(crate) use store::{
    delete_named_preset, export_library, import_library, list_named_presets, load_active_preset,
    load_named_preset, presets_file_path, save_autosave_preset, save_named_preset,
};
