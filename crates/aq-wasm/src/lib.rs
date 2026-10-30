//! AquesTalk package for `wasm32`: the relocatable engines from
//! `aquestalk-wasm` are linked into this module and exposed through
//! `aq_synthesize`.  Non-wasm builds are empty; linking is done by `build.rs`.

#[cfg(target_arch = "wasm32")]
mod runtime;

#[cfg(target_arch = "wasm32")]
#[path = "host_alloc.rs"]
mod host_alloc;
