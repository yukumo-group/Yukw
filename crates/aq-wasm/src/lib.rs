//! AquesTalk package. Native builds can rewrite and link voice modules.
//! The wasm32 build exports the runtime entry points.

#[cfg(not(target_arch = "wasm32"))]
mod link;
#[cfg(not(target_arch = "wasm32"))]
mod merge;

#[cfg(not(target_arch = "wasm32"))]
pub use link::{import_names, rewrite_imports};
#[cfg(not(target_arch = "wasm32"))]
pub use merge::{CatalogVoice, VOICE_CATALOG, VoiceModule, link, link_vendor, myriad_4_link};

#[cfg(all(target_arch = "wasm32", not(feature = "static-engines")))]
mod runtime;
#[cfg(all(target_arch = "wasm32", feature = "static-engines"))]
mod runtime_static;
