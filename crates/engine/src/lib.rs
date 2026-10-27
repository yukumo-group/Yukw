//! Compatibility layer for the AquesTalk engines, matching `aqtk-shim`.

mod error;
mod param;
mod preset;

pub use error::error_message;
pub use param::{
    AQ1_PRESET_COUNT, AQ2_PRESET_COUNT, AQ10_PRESET_COUNT, Aq1Preset, Aq2Preset, Aq10Preset,
    AqSynthParam, AqVersion,
};
pub use preset::{
    AQ1_VOICES, AQ2_PHONTS, AQ10_VOICES, Aq10Voice, aq1_voice_name, aq2_phont_name, aq10_voice,
};
