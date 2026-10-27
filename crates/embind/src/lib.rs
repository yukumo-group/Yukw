//! Embind host shared by the voice engines and the reading converters.
//! A module calls these imports while it registers types and while a host
//! call is still on the stack.

mod embind;
mod export;
pub mod host;
mod imports;
mod voice;

pub use embind::{BoundFn, Embind, HostArg, HostValue, RegisterFunction, RegisterValueObjectField};
pub use imports::{IMPORT_MODULE, ImportBinding, Semantic, parse_wasm_imports};
pub use voice::{VoiceIo, WireVal};
