//! Build `aq-wasm` with the relocatable AquesTalk archives and synthesize.
//!
//! Skipped unless the archives from `aquestalk-wasm/scripts/build-reloc.sh`
//! are available (set `AQTK_RELOC_DIR` or keep the two checkouts side by side).

use std::path::{Path, PathBuf};
use std::process::Command;

fn reloc_dir(workspace: &Path) -> Option<PathBuf> {
    let dir = std::env::var_os("AQTK_RELOC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("../aquestalk-wasm/dist/reloc"));
    dir.join("libaq1.a").exists().then_some(dir)
}

#[test]
fn static_engines_link_into_one_single_memory_module() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.join("../..");
    let Some(reloc) = reloc_dir(&workspace) else {
        eprintln!("skipping: relocatable archives are not built");
        return;
    };

    let target = std::env::temp_dir().join("yukw-aq-wasm-static-target");
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "aq-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "--lib",
            "--features",
            "static-engines",
            "--offline",
        ])
        .current_dir(&workspace)
        .env("CARGO_TARGET_DIR", &target)
        .env("AQTK_RELOC_DIR", &reloc)
        .status()
        .expect("cargo build");
    assert!(status.success(), "static-engines wasm32 build failed");

    let bytes = std::fs::read(target.join("wasm32-unknown-unknown/debug/aq_wasm.wasm")).unwrap();

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &bytes).unwrap();

    // Single memory, no imports: this is the browser-compatible shape.
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
    let engines = instance
        .get_typed_func::<(), u32>(&mut store, "aq_static_engines")
        .unwrap();
    assert_eq!(engines.call(&mut store, ()).unwrap(), 5);

    let synthesize = instance
        .get_typed_func::<(u32, u32, u32, u32, u32, u32), i32>(&mut store, "aq_synthesize")
        .unwrap();

    let text = "こんにちは。".as_bytes();
    let text_at = 65_536u32;
    memory.write(&mut store, text_at as usize, text).unwrap();

    for (version, preset) in [(1i32, 0i32), (1, 2), (2, 0), (10, 1), (10, 3)] {
        let mut param = [0u8; 44];
        param[0..4].copy_from_slice(&version.to_le_bytes());
        param[4..8].copy_from_slice(&preset.to_le_bytes());
        param[8..12].copy_from_slice(&100i32.to_le_bytes());
        let param_at = 66_000u32;
        memory.write(&mut store, param_at as usize, &param).unwrap();

        let out_ptr = 90_000u32;
        let out_len = 90_004u32;
        let code = synthesize
            .call(
                &mut store,
                (text_at, text.len() as u32, param_at, 0, out_ptr, out_len),
            )
            .unwrap();
        assert_eq!(
            code, 0,
            "aq_synthesize(version {version}, preset {preset}) -> {code}"
        );

        let mut fields = [0u8; 8];
        memory
            .read(&mut store, out_ptr as usize, &mut fields[..4])
            .unwrap();
        memory
            .read(&mut store, out_len as usize, &mut fields[4..])
            .unwrap();
        let ptr = u32::from_le_bytes(fields[..4].try_into().unwrap());
        let len = u32::from_le_bytes(fields[4..].try_into().unwrap());
        assert!(len >= 4, "wav length {len}");

        let mut magic = [0u8; 4];
        memory.read(&mut store, ptr as usize, &mut magic).unwrap();
        assert_eq!(&magic, b"RIFF", "version {version} preset {preset}");
    }
}
