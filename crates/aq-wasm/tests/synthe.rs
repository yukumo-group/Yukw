//! Link the real voice modules, use the built-in test license, and synthesize.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn yukkuri_wav_starts_with_riff() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.join("../..");
    let vendor = workspace.join("vendor");
    if !vendor.join("aqtk1-f1.wasm").exists() {
        return;
    }
    let target = std::env::temp_dir().join("yukw-aq-wasm-target");
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "aq-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "--lib",
            "--offline",
        ])
        .current_dir(&workspace)
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .expect("cargo build");
    assert!(status.success(), "wasm32 runtime failed to build");
    let runtime = std::fs::read(target.join("wasm32-unknown-unknown/debug/aq_wasm.wasm")).unwrap();
    let linked = aq_wasm::link_vendor(&runtime, &vendor).unwrap();

    let mut config = wasmtime::Config::new();
    config.wasm_multi_memory(true);
    let engine = wasmtime::Engine::new(&config).unwrap();
    let module = wasmtime::Module::new(&engine, &linked).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
    let memory = instance.get_memory(&mut store, "memory").unwrap();

    let has = instance
        .get_typed_func::<(i32, i32, u32, u32), i32>(&mut store, "aq_has_function")
        .unwrap();
    let name_at = 1024u32;
    for (version, preset, name) in [
        (1, 0, "synthe"),
        (10, 0, "synthe"),
        (10, 0, "getPresetVoices"),
        (10, 0, "AqtkVoice"),
    ] {
        if version == 10 && !vendor.join("aqtk10.wasm").exists() {
            continue;
        }
        memory
            .write(&mut store, name_at as usize, name.as_bytes())
            .unwrap();
        let found = has
            .call(&mut store, (version, preset, name_at, name.len() as u32))
            .unwrap();
        assert_eq!(found, 1, "{name} was not registered for version {version}");
    }

    let text = "ゆっくりしていってね".as_bytes();
    let text_at = 65_536u32;
    memory.write(&mut store, text_at as usize, text).unwrap();
    let mut param = [0u8; 44];
    param[0..4].copy_from_slice(&1i32.to_le_bytes());
    param[8..12].copy_from_slice(&100i32.to_le_bytes());
    let param_at = 66_000u32;
    memory.write(&mut store, param_at as usize, &param).unwrap();
    let synthesize = instance
        .get_typed_func::<(u32, u32, u32, u32, u32, u32), i32>(&mut store, "aq_synthesize")
        .unwrap();
    let out_ptr = 90_000u32;
    let out_len = 90_004u32;
    let code = synthesize
        .call(
            &mut store,
            (text_at, text.len() as u32, param_at, 0, out_ptr, out_len),
        )
        .unwrap();
    assert_eq!(code, 0, "aq_synthesize returned {code}");
    let mut header = [0u8; 8];
    memory
        .read(&mut store, out_ptr as usize, &mut header[..4])
        .unwrap();
    let ptr = u32::from_le_bytes(header[..4].try_into().unwrap());
    memory
        .read(&mut store, out_len as usize, &mut header[4..])
        .unwrap();
    let len = u32::from_le_bytes(header[4..].try_into().unwrap());
    assert!(len >= 4, "wav length {len}");
    let mut magic = [0u8; 4];
    memory.read(&mut store, ptr as usize, &mut magic).unwrap();
    assert_eq!(&magic, b"RIFF");
}
