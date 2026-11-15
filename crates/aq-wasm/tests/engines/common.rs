//! Shared harness for aq-wasm static-engine integration tests.
//!
//! Skipped unless the archives from `aquestalk-wasm/scripts/build-reloc.sh`
//! are available (set `AQTK_RELOC_DIR` or keep the two checkouts side by side).
//! The wasm module is built once and shared by every test.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub fn reloc_dir(workspace: &Path) -> Option<PathBuf> {
    let dir = std::env::var_os("AQTK_RELOC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("../aquestalk-wasm/dist/reloc"));
    dir.join("libaq1.a").exists().then_some(dir)
}

pub struct Built {
    pub engine: wasmtime::Engine,
    pub module: wasmtime::Module,
    pub workspace: PathBuf,
}

pub fn built() -> Option<&'static Built> {
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

pub struct Session {
    store: wasmtime::Store<()>,
    memory: wasmtime::Memory,
    synthesize: wasmtime::TypedFunc<(u32, u32, u32, u32, u32, u32), i32>,
    free: wasmtime::TypedFunc<u32, ()>,
    text_at: u32,
    text_len: u32,
}

impl Session {
    pub fn new(built: &Built) -> Self {
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

    pub fn speak(&mut self, param: &synth::AqSynthParam) -> Result<Vec<u8>, i32> {
        let mut abi = [0u8; 40];
        abi[0..4].copy_from_slice(&(param.version as i32).to_le_bytes());
        abi[4..8].copy_from_slice(&param.preset.to_le_bytes());
        abi[8..12].copy_from_slice(&param.speed.to_le_bytes());
        abi[16..20].copy_from_slice(&param.base.to_le_bytes());
        abi[20..24].copy_from_slice(&param.volume.to_le_bytes());
        abi[24..28].copy_from_slice(&param.pitch.to_le_bytes());
        abi[28..32].copy_from_slice(&param.accent.to_le_bytes());
        abi[32..36].copy_from_slice(&param.lmd.to_le_bytes());
        abi[36..40].copy_from_slice(&param.fsc.to_le_bytes());
        let phont = param.phont_data.as_deref().unwrap_or(&[]);
        let phont_at = 100_000u32;
        let phont_len = if phont.is_empty() {
            0
        } else {
            self.memory
                .write(&mut self.store, phont_at as usize, phont)
                .expect("write phont");
            abi[12..16].copy_from_slice(&phont_at.to_le_bytes());
            phont.len() as u32
        };
        let param_at = 66_000u32;
        self.memory
            .write(&mut self.store, param_at as usize, &abi)
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
    pub fn preset(&mut self, version: i32, preset: i32) -> Result<Vec<u8>, i32> {
        let mut param =
            synth::AqSynthParam::new(synth::AqVersion::from_i32(version).expect("version"));
        param.preset = preset;
        param.speed = 100;
        self.speak(&param)
    }
}

/// AquesTalk10 custom voice: every knob at 100, `base` selects F1E, F2E, or M1E.
pub fn aq10_custom(base: i32) -> synth::AqSynthParam {
    synth::AqSynthParam {
        version: synth::AqVersion::V10,
        preset: synth::Aq10Preset::Custom as i32,
        speed: 100,
        phont_data: None,
        base,
        volume: 100,
        pitch: 100,
        accent: 100,
        lmd: 100,
        fsc: 100,
    }
}

pub fn sample_rate(wav: &[u8]) -> u32 {
    u32::from_le_bytes(wav[24..28].try_into().unwrap())
}

pub fn playback_secs(wav: &[u8]) -> f64 {
    let rate = sample_rate(wav);
    assert!(rate >= 8_000, "sample rate {rate}");
    (wav.len() - 44) as f64 / (f64::from(rate) * 2.0)
}

pub fn assert_distinct(wavs: &[(&str, Vec<u8>)]) {
    for (index, (name, wav)) in wavs.iter().enumerate() {
        for (other, other_wav) in wavs.iter().take(index) {
            assert_ne!(wav, other_wav, "{name} produced the same wav as {other}");
        }
    }
}

pub fn assert_riff(label: &str, wav: &[u8]) {
    assert!(wav.starts_with(b"RIFF"), "{label}");
}
