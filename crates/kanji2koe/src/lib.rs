//! Japanese Kanji2Koe backed by the statically linked AqKanji2Koe engine.
//! [`Reader`] implements [`koe::ToKoe`]. Another `*2koe` crate can implement
//! that same trait.
//!
//! The wasm module no longer carries the dictionary.  The host writes
//! `lookup.bin`, `aqdic.bin` and `aq_user.dic` into the module and calls
//! `aqk_load` before the first `aqk_convert`; see [`Session::load`].

mod session;

#[cfg(target_arch = "wasm32")]
mod runtime;

// Tests compile this too. `no_mangle` stays wasm-only so libc is left alone.
#[cfg(any(target_arch = "wasm32", test))]
#[path = "host_alloc.rs"]
mod host_alloc;

pub use session::{
    ERR_ARG, ERR_OPEN, ERR_SCRIPT, LinkedScript, Reader, Script, Session, find_script,
};
