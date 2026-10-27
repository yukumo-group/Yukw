//! Rewrite `vendor/aqk2k.wasm` and link it with the reading runtime.

use std::path::Path;

use aq_wasm::{VoiceModule, rewrite_imports};
use embind::parse_wasm_imports;

use crate::Script;

struct CatalogEntry {
    file: &'static str,
    script: Script,
    /// `__wasm_call_ctors` in this emscripten build.
    init_export: &'static str,
    malloc_export: &'static str,
    free_export: &'static str,
}

/// Japanese now. A Chinese module would be another entry with its own export letters.
const CATALOG: &[CatalogEntry] = &[CatalogEntry {
    file: "aqk2k",
    script: Script::Japanese,
    init_export: "y",
    malloc_export: "A",
    free_export: "B",
}];

/// Rewrite every catalog module that exists in `vendor` and link it with `runtime`.
pub fn link_vendor(runtime: &[u8], vendor: &Path) -> Result<Vec<u8>, String> {
    let mut owned = Vec::new();
    for entry in CATALOG {
        let wasm_path = vendor.join(format!("{}.wasm", entry.file));
        let js_path = vendor.join(format!("{}.js", entry.file));
        if !wasm_path.exists() {
            continue;
        }
        let wasm = std::fs::read(&wasm_path).map_err(|err| err.to_string())?;
        let js = std::fs::read_to_string(&js_path).map_err(|err| err.to_string())?;
        let bindings = parse_wasm_imports(&js)?;
        let rewritten = rewrite_imports(&wasm, &bindings)?;
        // The dictionary carries the same embedded marker as the voice modules.
        let rewritten = aq_wasm::myriad_4_link(&rewritten)?;
        owned.push((rewritten, entry));
    }
    if owned.is_empty() {
        return Err("no reading modules in vendor".into());
    }
    let modules = owned
        .iter()
        .map(|(bytes, entry)| {
            let mut module = VoiceModule::new(bytes, entry.script as i32, 0);
            module.init_export = entry.init_export;
            module.malloc_export = entry.malloc_export;
            module.free_export = entry.free_export;
            module
        })
        .collect::<Vec<_>>();
    aq_wasm::link(runtime, &modules)
}
