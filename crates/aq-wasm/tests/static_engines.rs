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
    let free = instance
        .get_typed_func::<u32, ()>(&mut store, "aq_free")
        .unwrap();

    let text = "こんにちは。".as_bytes();
    let text_at = 65_536u32;
    memory.write(&mut store, text_at as usize, text).unwrap();

    let speak = |store: &mut wasmtime::Store<()>,
                 version: i32,
                 preset: i32,
                 speed: i32,
                 base: i32,
                 volume: i32,
                 pitch: i32,
                 accent: i32,
                 lmd: i32,
                 fsc: i32,
                 phont: &[u8]|
     -> Result<Vec<u8>, i32> {
        let mut param = [0u8; 40];
        param[0..4].copy_from_slice(&version.to_le_bytes());
        param[4..8].copy_from_slice(&preset.to_le_bytes());
        param[8..12].copy_from_slice(&speed.to_le_bytes());
        param[16..20].copy_from_slice(&base.to_le_bytes());
        param[20..24].copy_from_slice(&volume.to_le_bytes());
        param[24..28].copy_from_slice(&pitch.to_le_bytes());
        param[28..32].copy_from_slice(&accent.to_le_bytes());
        param[32..36].copy_from_slice(&lmd.to_le_bytes());
        param[36..40].copy_from_slice(&fsc.to_le_bytes());
        let phont_at = 100_000u32;
        let phont_len = if phont.is_empty() {
            0
        } else {
            memory.write(&mut *store, phont_at as usize, phont).unwrap();
            param[12..16].copy_from_slice(&phont_at.to_le_bytes());
            phont.len() as u32
        };
        let param_at = 66_000u32;
        memory
            .write(&mut *store, param_at as usize, &param)
            .unwrap();
        let out_ptr = 90_000u32;
        let out_len = 90_004u32;
        let code = synthesize
            .call(
                &mut *store,
                (
                    text_at,
                    text.len() as u32,
                    param_at,
                    phont_len,
                    out_ptr,
                    out_len,
                ),
            )
            .unwrap();
        if code != 0 {
            return Err(code);
        }
        let mut fields = [0u8; 8];
        memory
            .read(&mut *store, out_ptr as usize, &mut fields[..4])
            .unwrap();
        memory
            .read(&mut *store, out_len as usize, &mut fields[4..])
            .unwrap();
        let ptr = u32::from_le_bytes(fields[..4].try_into().unwrap());
        let len = u32::from_le_bytes(fields[4..].try_into().unwrap());
        let mut bytes = vec![0u8; len as usize];
        memory.read(&mut *store, ptr as usize, &mut bytes).unwrap();
        free.call(&mut *store, ptr).unwrap();
        Ok(bytes)
    };

    let mut aq1 = Vec::new();
    for (preset, name) in synth::AQ1_VOICES.iter().enumerate() {
        let wav = speak(&mut store, 1, preset as i32, 100, 0, 0, 0, 0, 0, 0, &[])
            .unwrap_or_else(|code| panic!("aq1 {name}: {code}"));
        assert!(wav.starts_with(b"RIFF"), "aq1 {name}");
        aq1.push((*name, wav));
    }
    assert_distinct(&aq1);

    let mut aq2 = Vec::new();
    for (preset, name) in synth::AQ2_PHONTS.iter().enumerate() {
        let wav = speak(&mut store, 2, preset as i32, 100, 0, 0, 0, 0, 0, 0, &[])
            .unwrap_or_else(|code| panic!("aq2 {name}: {code}"));
        assert!(wav.starts_with(b"RIFF"), "aq2 {name}");
        aq2.push((*name, wav));
    }
    assert_distinct(&aq2);
    assert!(
        speak(
            &mut store,
            2,
            synth::AQ2_PRESET_COUNT,
            100,
            0,
            0,
            0,
            0,
            0,
            0,
            &[],
        )
        .is_err(),
        "an unknown aq2 preset without phont data must fail"
    );

    let phont_path =
        workspace.join("../aquestalk-wasm/engines/aquestalk2/aqtk2-lnx/phont/aq_rm.phont");
    if let Some((_, rm)) = aq2.iter().find(|(name, _)| *name == "aq_rm.phont") {
        if phont_path.exists() {
            let file = std::fs::read(&phont_path).unwrap();
            let custom = speak(
                &mut store,
                2,
                synth::AQ2_PRESET_COUNT,
                100,
                0,
                0,
                0,
                0,
                0,
                0,
                &file,
            )
            .expect("custom phont data overrides an unknown preset");
            assert_eq!(
                custom, *rm,
                "aq_rm.phont must match the inlined AqRm preset"
            );
        }
    }

    // AquesTalk10 bases are the three 基本素片: F1E, F2E, M1E.
    let mut aq10 = Vec::new();
    for (base, name) in [(0, "f1e"), (1, "f2e"), (2, "m1e")] {
        let wav = speak(
            &mut store,
            10,
            synth::Aq10Preset::Custom as i32,
            100,
            base,
            100,
            100,
            100,
            100,
            100,
            &[],
        )
        .unwrap_or_else(|code| panic!("aq10 {name}: {code}"));
        assert!(wav.starts_with(b"RIFF"), "aq10 {name}");
        aq10.push((name, wav));
    }
    assert_distinct(&aq10);
}

fn assert_distinct(wavs: &[(&str, Vec<u8>)]) {
    for (index, (name, wav)) in wavs.iter().enumerate() {
        for (other, other_wav) in wavs.iter().take(index) {
            assert_ne!(wav, other_wav, "{name} produced the same wav as {other}");
        }
    }
}
