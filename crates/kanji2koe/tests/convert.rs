//! Build the static `kanji2koe` module, load a dictionary, and convert text.
//!
//! Skipped unless the relocatable archives and the AqKanji2Koe dictionary
//! files are available (`AQTK_RELOC_DIR`, `AQK2K_DIC_DIR`, or sibling
//! `aquestalk-wasm` checkout).

use std::path::{Path, PathBuf};
use std::process::Command;

fn reloc_dir(workspace: &Path) -> Option<PathBuf> {
    let dir = std::env::var_os("AQTK_RELOC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("../aquestalk-wasm/dist/reloc"));
    dir.join("libaqk2k.a").exists().then_some(dir)
}

fn dictionary_dir(workspace: &Path) -> Option<PathBuf> {
    let dir = std::env::var_os("AQK2K_DIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("../aquestalk-wasm/engines/aqk2k/aqk2k_lnx/aq_dic"));
    (dir.join("lookup.bin").exists() && dir.join("aqdic.bin").exists()).then_some(dir)
}

#[test]
fn static_engine_converts_japanese_to_kana() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.join("../..");
    let (Some(reloc), Some(dictionary)) = (reloc_dir(&workspace), dictionary_dir(&workspace))
    else {
        eprintln!("skipping: relocatable archives or dictionaries are not available");
        return;
    };

    let target = std::env::temp_dir().join("yukw-kanji2koe-static-target");
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
        .env("AQTK_RELOC_DIR", &reloc)
        .status()
        .expect("cargo build");
    assert!(status.success(), "kanji2koe wasm32 build failed");

    let bytes = std::fs::read(target.join("wasm32-unknown-unknown/debug/kanji2koe.wasm")).unwrap();

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &bytes).unwrap();
    assert_eq!(
        module.imports().count(),
        0,
        "module must not import anything"
    );
    let memories = module
        .exports()
        .filter(|export| matches!(export.ty(), wasmtime::ExternType::Memory(_)))
        .count();
    assert_eq!(memories, 1, "module must export exactly one memory");

    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
    let memory = instance.get_memory(&mut store, "memory").unwrap();
    let malloc = instance
        .get_typed_func::<u32, u32>(&mut store, "malloc")
        .unwrap();
    let load = instance
        .get_typed_func::<(u32, u32, u32, u32, u32, u32), i32>(&mut store, "aqk_load")
        .unwrap();

    fn put(
        store: &mut wasmtime::Store<()>,
        memory: &wasmtime::Memory,
        malloc: &wasmtime::TypedFunc<u32, u32>,
        bytes: &[u8],
    ) -> u32 {
        let pointer = malloc.call(&mut *store, bytes.len() as u32).unwrap();
        memory.write(&mut *store, pointer as usize, bytes).unwrap();
        pointer
    }

    let lookup = std::fs::read(dictionary.join("lookup.bin")).unwrap();
    let system = std::fs::read(dictionary.join("aqdic.bin")).unwrap();
    let user = std::fs::read(dictionary.join("aq_user.dic")).unwrap_or_default();
    let lookup_at = put(&mut store, &memory, &malloc, &lookup);
    let system_at = put(&mut store, &memory, &malloc, &system);
    let user_at = put(&mut store, &memory, &malloc, &user);

    let code = load
        .call(
            &mut store,
            (
                lookup_at,
                lookup.len() as u32,
                system_at,
                system.len() as u32,
                user_at,
                user.len() as u32,
            ),
        )
        .unwrap();
    assert_eq!(code, 0, "aqk_load returned {code}");

    let text = "日本語".as_bytes();
    let text_at = put(&mut store, &memory, &malloc, text);
    let convert = instance
        .get_typed_func::<(i32, u32, u32, u32, u32), i32>(&mut store, "aqk_convert")
        .unwrap();
    let out_ptr = malloc.call(&mut store, 4).unwrap();
    let out_len = malloc.call(&mut store, 4).unwrap();
    let code = convert
        .call(
            &mut store,
            (
                kanji2koe::Script::Japanese as i32,
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
    memory
        .read(&mut store, out_len as usize, &mut header[4..])
        .unwrap();
    let pointer = u32::from_le_bytes(header[..4].try_into().unwrap());
    let len = u32::from_le_bytes(header[4..].try_into().unwrap());
    assert!(len > 0, "empty reading");

    let mut bytes = vec![0u8; len as usize];
    memory
        .read(&mut store, pointer as usize, &mut bytes)
        .unwrap();
    let reading = String::from_utf8_lossy(&bytes);
    assert!(
        reading
            .chars()
            .any(|ch| ('\u{3040}'..='\u{30ff}').contains(&ch)),
        "expected kana, got {reading}"
    );
}
