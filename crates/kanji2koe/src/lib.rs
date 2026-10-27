//! Reading converters.
//! Japanese Kanji2Koe (`vendor/aqk2k.wasm`) is the first script.
//! Another script, such as Chinese, can be linked beside it.

mod session;

#[cfg(not(target_arch = "wasm32"))]
mod link;
#[cfg(target_arch = "wasm32")]
mod runtime;

pub use session::{ERR_ARG, ERR_OPEN, ERR_SCRIPT, LinkedScript, Script, Session, find_script};

#[cfg(not(target_arch = "wasm32"))]
pub use link::link_vendor;
