//! AquesTalk parameter tables and error messages shared by the wasm runtime.

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

/// Empty input or an unusable parameter block.
pub const ERR_ARG: i32 = -1;
/// Unknown `AqVersion`.
pub const ERR_VERSION: i32 = -2;
/// The engine rejected the call or no voice matched.
pub const ERR_OPEN: i32 = -3;
