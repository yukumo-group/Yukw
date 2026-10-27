//! Wasm32 runtime. Voice code calls `host_*` with a voice id. The host calls
//! back through `voice_*`, which the linker implements with that voice's memory.

use std::cell::Cell;
use std::sync::OnceLock;

use embind::VoiceIo;
use engine::{
    AqSynthParam, AqVersion, ERR_OPEN, ERR_VERSION, LinkedVoice, Session, error_message, find_voice,
};

#[link(wasm_import_module = "voice")]
unsafe extern "C" {
    fn voice_read(voice: i32, addr: i32, ptr: i32, len: i32);
    fn voice_write(voice: i32, addr: i32, ptr: i32, len: i32);
    fn voice_grow(voice: i32, pages: i32) -> i32;
    fn voice_call(voice: i32, func: i32, argc: i32, argv: i32) -> i64;
    fn voice_init(voice: i32);
    fn voice_malloc(voice: i32, size: i32) -> i32;
    fn voice_free(voice: i32, ptr: i32);
    fn voice_info(voice: i32) -> i64;
    fn voice_count() -> i32;
    fn voice_pages(voice: i32) -> i32;
}

struct Shared(Session);
// The module is single-threaded. Embind uses a `RefCell` for host reentry.
unsafe impl Sync for Shared {}

static SESSION: OnceLock<Shared> = OnceLock::new();

struct SyncCell<T>(Cell<T>);
unsafe impl<T> Sync for SyncCell<T> {}

static THREW: SyncCell<bool> = SyncCell(Cell::new(false));

fn session() -> &'static Session {
    &SESSION.get_or_init(|| Shared(Session::new())).0
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../embind/src/voice_io.rs"
));

fn linked_voices() -> &'static [LinkedVoice] {
    static VOICES: OnceLock<Vec<LinkedVoice>> = OnceLock::new();
    VOICES.get_or_init(|| {
        let count = unsafe { voice_count() }.max(0);
        (0..count)
            .map(|index| {
                let packed = unsafe { voice_info(index) } as u64;
                let version = AqVersion::from_i32((packed >> 32) as i32).unwrap_or(AqVersion::V1);
                let preset = packed as u32 as i32;
                LinkedVoice {
                    version,
                    aq1_preset: (version == AqVersion::V1).then_some(preset),
                }
            })
            .collect()
    })
}

fn ensure_init(voice: u32) {
    if session().embind.is_inited(voice) {
        return;
    }
    Io.init(voice);
    session().embind.flush_with(voice, &mut Io);
    session().embind.mark_inited(voice);
}

fn note_throw() {
    THREW.0.set(true);
    session().embind.cxa_throw(0, 0, 0, 0);
}

embind::export_host!(session, note_throw, Io);

#[repr(C)]
struct ParamAbi {
    version: i32,
    preset: i32,
    speed: i32,
    phont: u32,
    phont_data: u32,
    base: i32,
    volume: i32,
    pitch: i32,
    accent: i32,
    lmd: i32,
    fsc: i32,
}

fn read_text(ptr: u32, len: u32) -> String {
    if len == 0 || ptr == 0 {
        return String::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    String::from_utf8_lossy(bytes).into_owned()
}

fn read_cstr(ptr: u32) -> Option<String> {
    if ptr == 0 {
        return None;
    }
    let mut len = 0usize;
    unsafe {
        while *(ptr as *const u8).add(len) != 0 && len < 1_048_576 {
            len += 1;
        }
        let bytes = std::slice::from_raw_parts(ptr as *const u8, len);
        Some(String::from_utf8_lossy(bytes).into_owned())
    }
}

fn read_param(ptr: u32, phont_len: u32) -> Option<AqSynthParam> {
    if ptr == 0 {
        return None;
    }
    let raw = unsafe { &*(ptr as *const ParamAbi) };
    let version = AqVersion::from_i32(raw.version)?;
    let mut param = AqSynthParam::new(version);
    param.preset = raw.preset;
    param.speed = raw.speed;
    param.base = raw.base;
    param.volume = raw.volume;
    param.pitch = raw.pitch;
    param.accent = raw.accent;
    param.lmd = raw.lmd;
    param.fsc = raw.fsc;
    param.phont = read_cstr(raw.phont).filter(|text| !text.is_empty());
    if raw.phont_data != 0 && phont_len > 0 {
        let bytes =
            unsafe { std::slice::from_raw_parts(raw.phont_data as *const u8, phont_len as usize) };
        param.phont_data = Some(bytes.to_vec());
    }
    Some(param)
}

static WAVS: SyncCell<Vec<(u32, Vec<u8>)>> = SyncCell(Cell::new(Vec::new()));

fn store_wav(bytes: Vec<u8>) -> (u32, u32) {
    let mut bytes = bytes;
    let ptr = bytes.as_mut_ptr() as u32;
    let len = bytes.len() as u32;
    let mut wavs = WAVS.0.take();
    wavs.push((ptr, bytes));
    WAVS.0.set(wavs);
    (ptr, len)
}

fn write_u32(ptr: u32, value: u32) {
    if ptr != 0 {
        unsafe { *(ptr as *mut u32) = value }
    }
}

/// Synthesize UTF-8 `text`. `param` points at [`ParamAbi`] in memory 0.
/// `phont_len` is the size of `phont_data` because the C struct has no length.
/// On success returns 0 and writes the wav pointer and size. Otherwise, a negative code.
#[unsafe(no_mangle)]
pub extern "C" fn aq_synthesize(
    text: u32,
    text_len: u32,
    param: u32,
    phont_len: u32,
    out_ptr: u32,
    out_len: u32,
) -> i32 {
    THREW.0.set(false);
    let Some(param) = read_param(param, phont_len) else {
        return ERR_VERSION;
    };
    let text = read_text(text, text_len);
    let voices = linked_voices();
    let Some(_) = find_voice(&voices, &param) else {
        return ERR_OPEN;
    };
    let result = session().synthesize(&voices, &mut Io, &param, &text);
    if THREW.0.replace(false) {
        return ERR_OPEN;
    }
    match result {
        Ok(bytes) => {
            let (ptr, len) = store_wav(bytes);
            write_u32(out_ptr, ptr);
            write_u32(out_len, len);
            0
        }
        Err(code) => {
            write_u32(out_ptr, 0);
            write_u32(out_len, 0);
            code
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn aq_free(ptr: u32) {
    if ptr == 0 {
        return;
    }
    let mut wavs = WAVS.0.take();
    wavs.retain(|(stored, _)| *stored != ptr);
    WAVS.0.set(wavs);
}

/// Wall time in milliseconds since the Unix epoch. `verifyLicense` compares a
/// ticket's `exp` with this clock. Synthesis also sets a license-valid clock.
#[unsafe(no_mangle)]
pub extern "C" fn aq_set_clock(ms: f64) {
    session().embind.set_clock(ms);
}

static ERR_BUF: SyncCell<[u8; 256]> = SyncCell(Cell::new([0; 256]));

#[unsafe(no_mangle)]
pub extern "C" fn aq_error_message(version: i32, code: i32) -> u32 {
    let Some(version) = AqVersion::from_i32(version) else {
        return 0;
    };
    let Some(text) = error_message(version, code) else {
        return 0;
    };
    let mut bytes = [0; 256];
    let take = text.len().min(bytes.len() - 1);
    bytes[..take].copy_from_slice(&text.as_bytes()[..take]);
    ERR_BUF.0.set(bytes);
    ERR_BUF.0.as_ptr() as u32
}

/// 1 when `name` was registered for the voice selected by `version` and `preset`.
#[unsafe(no_mangle)]
pub extern "C" fn aq_has_function(version: i32, preset: i32, name: u32, name_len: u32) -> i32 {
    let Some(version) = AqVersion::from_i32(version) else {
        return 0;
    };
    let mut param = AqSynthParam::new(version);
    param.preset = preset;
    let voices = linked_voices();
    let Some(voice) = find_voice(&voices, &param) else {
        return 0;
    };
    ensure_init(voice);
    let name = read_text(name, name_len);
    i32::from(session().embind.registered(voice, &name))
}
