//! Link `target/wasm32-unknown-unknown/debug/aq_wasm.wasm` with `vendor/` voices.
//!
//! ```sh
//! cargo build -p aq-wasm --target wasm32-unknown-unknown --lib
//! cargo run -p aq-wasm --bin aq-link
//! ```

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.join("../..");
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let runtime_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| target.join("wasm32-unknown-unknown/debug/aq_wasm.wasm"));
    let runtime = std::fs::read(&runtime_path).unwrap_or_else(|err| {
        panic!(
            "read {}: {err}. Build the runtime with `cargo build -p aq-wasm --target wasm32-unknown-unknown --lib` first.",
            runtime_path.display()
        )
    });
    let vendor = workspace.join("vendor");
    let linked = aq_wasm::link_vendor(&runtime, &vendor).unwrap_or_else(|err| panic!("{err}"));
    let out = target.join("dist/yukw.wasm");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(&out, &linked).unwrap();
    println!("{}", out.display());
}
