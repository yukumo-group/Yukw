//! AquesTalk package. Native builds can rewrite and link voice modules.
//! The wasm32 build exports the runtime entry points.

#[cfg(not(target_arch = "wasm32"))]
mod link;

#[cfg(not(target_arch = "wasm32"))]
pub use link::{import_names, rewrite_imports};
