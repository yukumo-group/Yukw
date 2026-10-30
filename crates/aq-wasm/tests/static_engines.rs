//! Build `aq-wasm` with the relocatable AquesTalk archives and synthesize.
//!
//! Skipped unless the archives from `aquestalk-wasm/scripts/build-reloc.sh`
//! are available (set `AQTK_RELOC_DIR` or keep the two checkouts side by side).
//! The wasm module is built once and shared by every test.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn reloc_dir(workspace: &Path) -> Option<PathBuf> {
    let dir = std::env::var_os("AQTK_RELOC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("../aquestalk-wasm/dist/reloc"));
    dir.join("libaq1.a").exists().then_some(dir)
}

struct Built {
    engine: wasmtime::Engine,
    module: wasmtime::Module,
    workspace: PathBuf,
}

fn built() -> Option<&'static Built> {
    static BUILT: OnceLock<Option<Built>> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let workspace = manifest.join("../..");
            let Some(reloc) = reloc_dir(&workspace) else {
                eprintln!("skipping: relocatable archives are not built");
                return None;
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

            let bytes =
                std::fs::read(target.join("wasm32-unknown-unknown/debug/aq_wasm.wasm")).unwrap();
            let engine = wasmtime::Engine::default();
            let module = wasmtime::Module::new(&engine, &bytes).unwrap();
            Some(Built {
                engine,
                module,
                workspace,
            })
        })
        .as_ref()
}

struct Session {
    store: wasmtime::Store<()>,
    memory: wasmtime::Memory,
    synthesize: wasmtime::TypedFunc<(u32, u32, u32, u32, u32, u32), i32>,
    free: wasmtime::TypedFunc<u32, ()>,
    text_at: u32,
    text_len: u32,
}

impl Session {
    fn new(built: &Built) -> Self {
        let mut store = wasmtime::Store::new(&built.engine, ());
        let instance =
            wasmtime::Instance::new(&mut store, &built.module, &[]).expect("instantiate aq-wasm");
        let memory = instance
            .get_memory(&mut store, "memory")
            .expect("memory export");
        let synthesize = instance
            .get_typed_func(&mut store, "aq_synthesize")
            .expect("aq_synthesize");
        let free = instance
            .get_typed_func(&mut store, "aq_free")
            .expect("aq_free");

        let text = "こんにちは。".as_bytes();
        let text_at = 65_536u32;
        memory
            .write(&mut store, text_at as usize, text)
            .expect("write text");
        Self {
            store,
            memory,
            synthesize,
            free,
            text_at,
            text_len: text.len() as u32,
        }
    }

    fn speak(
        &mut self,
        version: i32,
        preset: i32,
        speed: i32,
        base: i32,
        volume: i32,
        pitch: i32,
        accent: i32,
        lmd: i32,
        fsc: i32,
        phont: &[u8],
    ) -> Result<Vec<u8>, i32> {
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
            self.memory
                .write(&mut self.store, phont_at as usize, phont)
                .expect("write phont");
            param[12..16].copy_from_slice(&phont_at.to_le_bytes());
            phont.len() as u32
        };
        let param_at = 66_000u32;
        self.memory
            .write(&mut self.store, param_at as usize, &param)
            .expect("write param");
        let out_ptr = 90_000u32;
        let out_len = 90_004u32;
        let code = self
            .synthesize
            .call(
                &mut self.store,
                (
                    self.text_at,
                    self.text_len,
                    param_at,
                    phont_len,
                    out_ptr,
                    out_len,
                ),
            )
            .expect("aq_synthesize");
        if code != 0 {
            return Err(code);
        }
        let mut fields = [0u8; 8];
        self.memory
            .read(&mut self.store, out_ptr as usize, &mut fields[..4])
            .expect("read wav pointer");
        self.memory
            .read(&mut self.store, out_len as usize, &mut fields[4..])
            .expect("read wav length");
        let ptr = u32::from_le_bytes(fields[..4].try_into().unwrap());
        let len = u32::from_le_bytes(fields[4..].try_into().unwrap());
        let mut bytes = vec![0u8; len as usize];
        self.memory
            .read(&mut self.store, ptr as usize, &mut bytes)
            .expect("read wav");
        self.free.call(&mut self.store, ptr).expect("aq_free");
        Ok(bytes)
    }

    /// Preset voice at speed 100, with the numeric fields left at zero.
    fn preset(&mut self, version: i32, preset: i32) -> Result<Vec<u8>, i32> {
        self.speak(version, preset, 100, 0, 0, 0, 0, 0, 0, &[])
    }
}

fn sample_rate(wav: &[u8]) -> u32 {
    u32::from_le_bytes(wav[24..28].try_into().unwrap())
}

fn playback_secs(wav: &[u8]) -> f64 {
    let rate = sample_rate(wav);
    assert!(rate >= 8_000, "sample rate {rate}");
    (wav.len() - 44) as f64 / (f64::from(rate) * 2.0)
}

fn assert_distinct(wavs: &[(&str, Vec<u8>)]) {
    for (index, (name, wav)) in wavs.iter().enumerate() {
        for (other, other_wav) in wavs.iter().take(index) {
            assert_ne!(wav, other_wav, "{name} produced the same wav as {other}");
        }
    }
}

fn assert_riff(label: &str, wav: &[u8]) {
    assert!(wav.starts_with(b"RIFF"), "{label}");
}

#[test]
fn module_exports_one_memory_and_no_imports() {
    let Some(built) = built() else {
        return;
    };
    assert_eq!(
        built.module.imports().count(),
        0,
        "module must not import anything"
    );
    let memories = built
        .module
        .exports()
        .filter(|export| matches!(export.ty(), wasmtime::ExternType::Memory(_)))
        .count();
    assert_eq!(memories, 1, "module must export exactly one memory");
}

#[test]
fn static_link_reports_five_engines() {
    let Some(built) = built() else {
        return;
    };
    let mut store = wasmtime::Store::new(&built.engine, ());
    let instance =
        wasmtime::Instance::new(&mut store, &built.module, &[]).expect("instantiate aq-wasm");
    let engines = instance
        .get_typed_func::<(), u32>(&mut store, "aq_static_engines")
        .expect("aq_static_engines");
    assert_eq!(engines.call(&mut store, ()).unwrap(), 5);
}

#[test]
fn aq1_voices_produce_distinct_wavs() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    let mut wavs = Vec::new();
    for (preset, name) in synth::AQ1_VOICES.iter().enumerate() {
        let wav = session
            .preset(1, preset as i32)
            .unwrap_or_else(|code| panic!("aq1 {name}: {code}"));
        assert_riff(&format!("aq1 {name}"), &wav);
        wavs.push((*name, wav));
    }
    assert_distinct(&wavs);
}

#[test]
fn aq2_presets_produce_distinct_wavs() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    let mut wavs = Vec::new();
    for (preset, name) in synth::AQ2_PHONTS.iter().enumerate() {
        let wav = session
            .preset(2, preset as i32)
            .unwrap_or_else(|code| panic!("aq2 {name}: {code}"));
        assert_riff(&format!("aq2 {name}"), &wav);
        wavs.push((*name, wav));
    }
    assert_distinct(&wavs);
}

#[test]
fn aq2_unknown_preset_without_phont_fails() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    assert!(
        session.preset(2, synth::AQ2_PRESET_COUNT).is_err(),
        "an unknown aq2 preset without phont data must fail"
    );
}

#[test]
fn aq2_file_phont_matches_builtin_aq_rm() {
    let Some(built) = built() else {
        return;
    };
    let phont_path = built
        .workspace
        .join("../aquestalk-wasm/engines/aquestalk2/aqtk2-lnx/phont/aq_rm.phont");
    if !phont_path.exists() {
        eprintln!("skipping: aq_rm.phont is not in the aquestalk-wasm checkout");
        return;
    }
    let preset = synth::AQ2_PHONTS
        .iter()
        .position(|name| *name == "aq_rm.phont")
        .expect("aq_rm.phont is a builtin preset") as i32;
    let mut session = Session::new(built);
    let builtin = session.preset(2, preset).expect("builtin aq_rm");
    let file = std::fs::read(&phont_path).unwrap();
    let custom = session
        .speak(2, synth::AQ2_PRESET_COUNT, 100, 0, 0, 0, 0, 0, 0, &file)
        .expect("custom phont data overrides an unknown preset");
    assert_eq!(
        custom, builtin,
        "aq_rm.phont must match the inlined AqRm preset"
    );
}

#[test]
fn aq10_bases_produce_distinct_wavs() {
    let Some(built) = built() else {
        return;
    };
    // AquesTalk10 bases are the three 基本素片: F1E, F2E, M1E.
    let mut session = Session::new(built);
    let mut wavs = Vec::new();
    for (base, name) in [(0, "f1e"), (1, "f2e"), (2, "m1e")] {
        let wav = session
            .speak(
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
        assert_riff(&format!("aq10 {name}"), &wav);
        wavs.push((name, wav));
    }
    assert_distinct(&wavs);
}

#[test]
fn aq10_named_presets_produce_distinct_wavs() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    let mut wavs = Vec::new();
    for (preset, name) in [
        (synth::Aq10Preset::F1 as i32, "f1"),
        (synth::Aq10Preset::F2 as i32, "f2"),
        (synth::Aq10Preset::M1 as i32, "m1"),
    ] {
        let wav = session
            .preset(10, preset)
            .unwrap_or_else(|code| panic!("aq10 preset {name}: {code}"));
        assert_riff(&format!("aq10 preset {name}"), &wav);
        wavs.push((name, wav));
    }
    assert_distinct(&wavs);
}

#[test]
fn aq10_fsc_100_is_16_khz() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    let wav = session
        .speak(
            10,
            synth::Aq10Preset::Custom as i32,
            100,
            0,
            100,
            100,
            100,
            100,
            100,
            &[],
        )
        .expect("aq10 f1e");
    assert_eq!(sample_rate(&wav), 16_000, "aq10 fsc 100 is 16 kHz");
}

#[test]
fn aq10_speed_100_matches_aq1_duration() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    let aq1 = session.preset(1, 0).expect("aq1 f1");
    let aq10 = session
        .speak(
            10,
            synth::Aq10Preset::Custom as i32,
            100,
            0,
            100,
            100,
            100,
            100,
            100,
            &[],
        )
        .expect("aq10 f1e");
    let aq1_secs = playback_secs(&aq1);
    let aq10_secs = playback_secs(&aq10);
    let ratio = aq10_secs / aq1_secs;
    assert!(
        (0.6..=1.5).contains(&ratio),
        "aq10 {aq10_secs:.2}s vs aq1 {aq1_secs:.2}s (ratio {ratio:.2}); speed 100 is the 50-300 scale on both"
    );
}
