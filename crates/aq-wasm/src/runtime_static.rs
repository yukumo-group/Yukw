//! Wasm32 runtime for the statically linked AquesTalk engines.
//!
//! With the `static-engines` Cargo feature, `build.rs` links the relocatable
//! archives produced by `aquestalk-wasm` (`libaq1.a` … `libaqusr.a`) plus the
//! host helpers into this module.  Engine calls are direct C calls: there is
//! no embind host, no `voice_*` import, no per-voice memory and no
//! multi-memory module.
//!
//! Only synthesis (AquesTalk1/2/10) is driven from here.  AqKanji2Koe and
//! AqUsrDic are linked and exported as `aqk2k_*` / `aqusr_*` for the host to
//! call directly.

use std::cell::Cell;

use synth::{AQ1_VOICES, AqSynthParam, AqVersion, aq10_voice, error_message};

const ERR_ARG: i32 = -1;
const ERR_VERSION: i32 = -2;
const ERR_OPEN: i32 = -3;

#[path = "host_alloc.rs"]
mod host_alloc;

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

unsafe extern "C" {
    fn aq1_select_voice(name: *const u8) -> i32;
    fn aq1_synthe_utf8(text: *const u8, speed: i32, size: *mut i32) -> *mut u8;
    fn aq1_free_wave(wave: *mut u8);

    fn aq2_synthe_utf8(text: *const u8, speed: i32, size: *mut i32, phont: *const u8) -> *mut u8;
    fn aq2_free_wave(wave: *mut u8);

    fn aq10_synthe_utf8_default(
        text: *const u8,
        base: i32,
        speed: i32,
        volume: i32,
        pitch: i32,
        accent: i32,
        lmd: i32,
        fsc: i32,
        size: *mut i32,
    ) -> *mut u8;
    fn aq10_free_wave(wave: *mut u8);
}

struct SyncCell<T>(Cell<T>);
unsafe impl<T> Sync for SyncCell<T> {}

static WAVS: SyncCell<Vec<(u32, Vec<u8>)>> = SyncCell(Cell::new(Vec::new()));
static ERR_BUF: SyncCell<[u8; 256]> = SyncCell(Cell::new([0; 256]));

fn read_text(pointer: u32, len: u32) -> String {
    if len == 0 || pointer == 0 {
        return String::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer as *const u8, len as usize) };
    String::from_utf8_lossy(bytes).into_owned()
}

fn read_cstr(pointer: u32) -> Option<String> {
    if pointer == 0 {
        return None;
    }
    let mut len = 0usize;
    unsafe {
        while *(pointer as *const u8).add(len) != 0 && len < 1_048_576 {
            len += 1;
        }
        let bytes = std::slice::from_raw_parts(pointer as *const u8, len);
        Some(String::from_utf8_lossy(bytes).into_owned())
    }
}

fn read_param(pointer: u32, phont_len: u32) -> Option<AqSynthParam> {
    if pointer == 0 {
        return None;
    }
    let raw = unsafe { &*(pointer as *const ParamAbi) };
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

fn store_wav(bytes: Vec<u8>) -> (u32, u32) {
    let mut bytes = bytes;
    let pointer = bytes.as_mut_ptr() as u32;
    let len = bytes.len() as u32;
    let mut wavs = WAVS.0.take();
    wavs.push((pointer, bytes));
    WAVS.0.set(wavs);
    (pointer, len)
}

fn write_u32(pointer: u32, value: u32) {
    if pointer != 0 {
        unsafe { *(pointer as *mut u32) = value }
    }
}

fn synthesize(param: &AqSynthParam, text: &str) -> Result<Vec<u8>, i32> {
    if text.is_empty() {
        return Err(ERR_ARG);
    }
    let mut ctext = text.as_bytes().to_vec();
    ctext.push(0);
    let mut size: i32 = 0;

    let pointer = unsafe {
        match param.version {
            AqVersion::V1 => {
                let index = param.preset;
                if !(0..AQ1_VOICES.len() as i32).contains(&index) {
                    return Err(ERR_OPEN);
                }
                let mut name = AQ1_VOICES[index as usize].as_bytes().to_vec();
                name.push(0);
                if aq1_select_voice(name.as_ptr()) != 0 {
                    return Err(ERR_OPEN);
                }
                aq1_synthe_utf8(ctext.as_ptr(), param.speed, &mut size)
            }
            AqVersion::V2 => {
                let phont = param
                    .phont_data
                    .as_deref()
                    .filter(|bytes| !bytes.is_empty())
                    .map_or(std::ptr::null(), |bytes| bytes.as_ptr());
                aq2_synthe_utf8(ctext.as_ptr(), param.speed, &mut size, phont)
            }
            AqVersion::V10 => {
                let voice = aq10_voice(param).ok_or(ERR_OPEN)?;
                aq10_synthe_utf8_default(
                    ctext.as_ptr(),
                    voice.bas,
                    voice.spd,
                    voice.vol,
                    voice.pit,
                    voice.acc,
                    voice.lmd,
                    voice.fsc,
                    &mut size,
                )
            }
        }
    };

    if pointer.is_null() || size <= 0 {
        return Err(ERR_OPEN);
    }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, size as usize) }.to_vec();
    unsafe {
        match param.version {
            AqVersion::V1 => aq1_free_wave(pointer),
            AqVersion::V2 => aq2_free_wave(pointer),
            AqVersion::V10 => aq10_free_wave(pointer),
        }
    }
    Ok(bytes)
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
    let Some(param) = read_param(param, phont_len) else {
        return ERR_VERSION;
    };
    let text = read_text(text, text_len);
    match synthesize(&param, &text) {
        Ok(bytes) => {
            let (pointer, len) = store_wav(bytes);
            write_u32(out_ptr, pointer);
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
pub extern "C" fn aq_free(pointer: u32) {
    if pointer == 0 {
        return;
    }
    let mut wavs = WAVS.0.take();
    wavs.retain(|(stored, _)| *stored != pointer);
    WAVS.0.set(wavs);
}

/// Kept for API parity with the embind runtime.  The static engines verify
/// their own license state, so the host clock is unused.
#[unsafe(no_mangle)]
pub extern "C" fn aq_set_clock(_ms: f64) {}

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

/// 1 when the statically linked engine accepts `version`/`preset`.
#[unsafe(no_mangle)]
pub extern "C" fn aq_has_function(version: i32, preset: i32, name: u32, name_len: u32) -> i32 {
    let Some(version) = AqVersion::from_i32(version) else {
        return 0;
    };
    let valid = match version {
        AqVersion::V1 => (0..AQ1_VOICES.len() as i32).contains(&preset),
        AqVersion::V2 => true,
        AqVersion::V10 => (0..synth::AQ10_PRESET_COUNT).contains(&preset),
    };
    if !valid {
        return 0;
    }
    match read_text(name, name_len).as_str() {
        "synthe" | "verifyLicense" | "issueBalanceToken" => 1,
        _ => 0,
    }
}

/// Marker: number of engines linked into this module (1/2/10/k2k/k2kusrdic).
#[unsafe(no_mangle)]
pub extern "C" fn aq_static_engines() -> u32 {
    5
}
