//! Link `vendor/aqk2k.wasm` and convert a short Japanese phrase.

use std::path::PathBuf;
use std::process::Command;

use kanji2koe::Script;

#[test]
fn nihongo_converts_to_kana() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.join("../..");
    let vendor = workspace.join("vendor");
    if !vendor.join("aqk2k.wasm").exists() {
        return;
    }
    let target = std::env::temp_dir().join("yukw-kanji2koe-target");
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "kanji2koe",
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
    let runtime =
        std::fs::read(target.join("wasm32-unknown-unknown/debug/kanji2koe.wasm")).unwrap();
    let linked = kanji2koe::link_vendor(&runtime, &vendor).unwrap();

    let mut config = wasmtime::Config::new();
    config.wasm_multi_memory(true);
    let engine = wasmtime::Engine::new(&config).unwrap();
    let module = wasmtime::Module::new(&engine, &linked).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
    let memory = instance.get_memory(&mut store, "memory").unwrap();

    let has = instance
        .get_typed_func::<(i32, u32, u32), i32>(&mut store, "aqk_has_function")
        .unwrap();
    let name_at = 1024u32;
    for name in [
        "convert",
        "verifyLicense",
        "issueBalanceToken",
        "verifyUsrLicKey",
    ] {
        memory
            .write(&mut store, name_at as usize, name.as_bytes())
            .unwrap();
        let found = has
            .call(
                &mut store,
                (Script::Japanese as i32, name_at, name.len() as u32),
            )
            .unwrap();
        assert_eq!(found, 1, "{name} was not registered");
    }

    let text = "日本語".as_bytes();
    let text_at = 65_536u32;
    memory.write(&mut store, text_at as usize, text).unwrap();
    let convert = instance
        .get_typed_func::<(i32, u32, u32, u32, u32), i32>(&mut store, "aqk_convert")
        .unwrap();
    let out_ptr = 90_000u32;
    let out_len = 90_004u32;
    let code = convert
        .call(
            &mut store,
            (
                Script::Japanese as i32,
                text_at,
                text.len() as u32,
                out_ptr,
                out_len,
            ),
        )
        .unwrap();
    assert_eq!(code, 0, "aqk_convert returned {code}");
    let mut header = [0u8; 8];
    memory
        .read(&mut store, out_ptr as usize, &mut header[..4])
        .unwrap();
    let ptr = u32::from_le_bytes(header[..4].try_into().unwrap());
    memory
        .read(&mut store, out_len as usize, &mut header[4..])
        .unwrap();
    let len = u32::from_le_bytes(header[4..].try_into().unwrap());
    assert!(len > 0, "empty reading");
    let mut bytes = vec![0u8; len as usize];
    memory.read(&mut store, ptr as usize, &mut bytes).unwrap();
    let reading = String::from_utf8_lossy(&bytes);
    assert!(
        reading
            .chars()
            .any(|ch| ('\u{3040}'..='\u{30ff}').contains(&ch)),
        "expected kana, got {reading}"
    );
}
