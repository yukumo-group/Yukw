//! Compatibility layer for the AquesTalk engines, matching `aqtk-shim`.

mod embind;
mod error;
mod imports;
mod param;
mod preset;
mod synth;
mod voice;

pub use embind::{Embind, HostArg, HostValue, RegisterFunction, RegisterValueObjectField};
pub use error::error_message;
pub use imports::{IMPORT_MODULE, ImportBinding, Semantic, parse_wasm_imports};
pub use param::{
    AQ1_PRESET_COUNT, AQ2_PRESET_COUNT, AQ10_PRESET_COUNT, Aq1Preset, Aq2Preset, Aq10Preset,
    AqSynthParam, AqVersion,
};
pub use preset::{
    AQ1_VOICES, AQ2_PHONTS, AQ10_VOICES, Aq10Voice, aq1_voice_name, aq2_phont_name, aq10_voice,
};
pub use synth::{ERR_ARG, ERR_OPEN, ERR_VERSION, LinkedVoice, Session, find_voice};
pub use voice::{VoiceIo, WireVal};
