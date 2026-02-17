use serde::{Deserialize, Serialize};

use super::scenario::ScenarioPresetV1;
use super::PRESET_FILE_VERSION;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PresetLibraryV1 {
    pub(super) version: u32,
    pub(super) active_preset: Option<String>,
    pub(super) presets: Vec<NamedPresetV1>,
}

impl PresetLibraryV1 {
    pub(super) fn empty() -> Self {
        Self {
            version: PRESET_FILE_VERSION,
            active_preset: None,
            presets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NamedPresetV1 {
    pub(super) name: String,
    pub(super) scenario: ScenarioPresetV1,
}
