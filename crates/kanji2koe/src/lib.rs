//! Japanese Kanji2Koe (`vendor/aqk2k.wasm`).
//! [`Reader`] implements [`koe::ToKoe`]. Another `*2koe` crate can implement that same trait.

mod session;

#[cfg(not(target_arch = "wasm32"))]
mod link;
#[cfg(target_arch = "wasm32")]
mod runtime;

pub use session::{
    ERR_ARG, ERR_OPEN, ERR_SCRIPT, LinkedScript, Reader, Script, Session, find_script,
};

#[cfg(not(target_arch = "wasm32"))]
pub use link::link_vendor;
