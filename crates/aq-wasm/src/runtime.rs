//! Wasm32 runtime. Voice code calls `host_*` with a voice id. The host calls
//! back through `voice_*`, which the linker implements with that voice's memory.

use std::cell::Cell;
use std::sync::OnceLock;

use engine::{
    AqSynthParam, AqVersion, ERR_OPEN, ERR_VERSION, LinkedVoice, Session, VoiceIo, WireVal,
    error_message, find_voice,
};

const PAGE: u32 = 65_536;

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

struct Io;

impl VoiceIo for Io {
    fn read(&mut self, voice: u32, addr: u32, dst: &mut [u8]) {
        if dst.is_empty() {
            return;
        }
        let mut buf = vec![0; dst.len()];
        unsafe {
            voice_read(
                voice as i32,
                addr as i32,
                buf.as_mut_ptr() as i32,
                dst.len() as i32,
            );
        }
        dst.copy_from_slice(&buf);
    }

    fn write(&mut self, voice: u32, addr: u32, src: &[u8]) {
        if src.is_empty() {
            return;
        }
        unsafe {
            voice_write(
                voice as i32,
                addr as i32,
                src.as_ptr() as i32,
                src.len() as i32,
            );
        }
    }

    fn memory_pages(&mut self, voice: u32) -> u32 {
        unsafe { voice_pages(voice as i32).max(0) as u32 }
    }

    fn grow(&mut self, voice: u32, pages: u32) -> i32 {
        unsafe { voice_grow(voice as i32, pages as i32) }
    }

    fn malloc(&mut self, voice: u32, size: u32) -> u32 {
        unsafe { voice_malloc(voice as i32, size as i32) as u32 }
    }

    fn free(&mut self, voice: u32, ptr: u32) {
        unsafe { voice_free(voice as i32, ptr as i32) }
    }

    fn call(&mut self, voice: u32, func: u32, args: &[WireVal]) -> WireVal {
        let mut bytes = vec![0; args.len() * 8];
        for (index, arg) in args.iter().enumerate() {
            let bits = match arg {
                WireVal::I32(value) => u64::from(*value),
                WireVal::I64(value) | WireVal::F64(value) => *value,
                WireVal::F32(value) => u64::from(*value),
                WireVal::None => 0,
            };
            bytes[index * 8..index * 8 + 8].copy_from_slice(&bits.to_le_bytes());
        }
        let argv = if bytes.is_empty() {
            0
        } else {
            bytes.as_ptr() as i32
        };
        let packed = unsafe { voice_call(voice as i32, func as i32, args.len() as i32, argv) };
        WireVal::I64(packed as u64)
    }

    fn init(&mut self, voice: u32) {
        unsafe { voice_init(voice as i32) }
    }
}

fn linked_voices() -> Vec<LinkedVoice> {
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
}

fn ensure_init(voice: u32) {
    if session().embind.is_inited(voice) {
        return;
    }
    Io.init(voice);
    session().embind.flush_with(voice, &mut Io);
    session().embind.mark_inited(voice);
}

fn resize_heap(voice: i32, requested: i32) -> i32 {
    let requested = requested.max(0) as u32;
    let have = unsafe { voice_pages(voice).max(0) as u32 };
    if requested <= have.saturating_mul(PAGE) {
        return 1;
    }
    let need = requested.div_ceil(PAGE);
    let grow_by = need.saturating_sub(have);
    if unsafe { voice_grow(voice, grow_by as i32) } < 0 {
        0
    } else {
        1
    }
}

fn note_throw() {
    THREW.0.set(true);
    session().embind.cxa_throw(0, 0, 0, 0);
}

fn with_embind<R>(body: impl FnOnce(&engine::Embind, &mut Io) -> R) -> R {
    body(&session().embind, &mut Io)
}

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
/// On success returns 0 and writes the wav pointer and size. Otherwise a negative code.
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

#[unsafe(no_mangle)]
pub extern "C" fn host____cxa_throw(_voice: i32, _ptr: i32, _ty: i32, _dtor: i32) {
    note_throw();
    core::arch::wasm32::unreachable()
}

#[unsafe(no_mangle)]
pub extern "C" fn host___abort_js(_voice: i32) {
    note_throw();
    core::arch::wasm32::unreachable()
}

#[unsafe(no_mangle)]
pub extern "C" fn host__emscripten_resize_heap(voice: i32, requested: i32) -> i32 {
    resize_heap(voice, requested)
}

#[unsafe(no_mangle)]
pub extern "C" fn host__emscripten_random(_voice: i32) -> f32 {
    0.0
}

#[unsafe(no_mangle)]
pub extern "C" fn host__emscripten_date_now(_voice: i32) -> f64 {
    session().embind.date_now()
}

#[unsafe(no_mangle)]
pub extern "C" fn host__random_get(voice: i32, buf: i32, len: i32) -> i32 {
    let len = len.max(0) as u32;
    if len > 0 && buf != 0 {
        Io.write(voice as u32, buf as u32, &vec![0; len as usize]);
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn host___gmtime_js(voice: i32, time: i64, tm: i32) -> i32 {
    if tm == 0 || time < 0 {
        return 1;
    }
    let secs = time as u64;
    let days = (secs / 86_400) as i64;
    let (year, month, mday, yday, wday) = civil_from_unix_days(days);
    let mut fields = [0u8; 32];
    for (index, value) in [
        (secs % 60) as i32,
        ((secs / 60) % 60) as i32,
        ((secs / 3_600) % 24) as i32,
        mday,
        month,
        year,
        wday,
        yday,
    ]
    .into_iter()
    .enumerate()
    {
        fields[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    Io.write(voice as u32, tm as u32, &fields);
    0
}

/// `(year - 1900, month 0-11, day of month, day of year, weekday)`.
fn civil_from_unix_days(days: i64) -> (i32, i32, i32, i32, i32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let mut year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let mday = (doy - (153 * mp + 2) / 5 + 1) as i32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as i32;
    if month <= 2 {
        year += 1;
    }
    let leaps_before = if month > 2 && is_leap(year) { 1 } else { 0 };
    let month_starts = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let yday = month_starts[(month - 1) as usize] + mday - 1 + leaps_before;
    let wday = ((days + 4).rem_euclid(7)) as i32;
    ((year - 1900) as i32, month - 1, mday, yday, wday)
}

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn host___tzset_js(voice: i32, a: i32, b: i32, c: i32, d: i32) {
    for ptr in [a, b, c, d] {
        if ptr != 0 {
            Io.write(voice as u32, ptr as u32, &0u32.to_le_bytes());
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn host___emval_decref(_voice: i32, handle: i32) {
    with_embind(|embind, _| embind.emval_decref(handle as u32));
}

#[unsafe(no_mangle)]
pub extern "C" fn host___emval_incref(_voice: i32, handle: i32) {
    with_embind(|embind, _| embind.emval_incref(handle as u32));
}

#[unsafe(no_mangle)]
pub extern "C" fn host___emval_new_cstring(voice: i32, ptr: i32) -> i32 {
    with_embind(|embind, io| embind.emval_new_cstring(voice as u32, ptr as u32, io) as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn host___emval_new_object(_voice: i32) -> i32 {
    with_embind(|embind, _| embind.emval_new_object() as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn host___emval_get_global(voice: i32, name: i32) -> i32 {
    with_embind(|embind, io| embind.emval_get_global(voice as u32, name as u32, io) as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn host___emval_set_property(_voice: i32, object: i32, key: i32, value: i32) {
    with_embind(|embind, _| {
        embind.emval_set_property(object as u32, key as u32, value as u32);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host___emval_run_destructors(_voice: i32, handle: i32) {
    with_embind(|embind, _| embind.emval_run_destructors(handle as u32));
}

#[unsafe(no_mangle)]
pub extern "C" fn host___emval_create_invoker(
    voice: i32,
    arg_count: i32,
    arg_types: i32,
    kind: i32,
) -> i32 {
    with_embind(|embind, io| {
        embind.emval_create_invoker(
            voice as u32,
            arg_count as u32,
            arg_types as u32,
            kind as u32,
            io,
        ) as i32
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn host___emval_invoke(
    voice: i32,
    caller: i32,
    handle: i32,
    method: i32,
    destructors: i32,
    args: i32,
) -> f64 {
    with_embind(|embind, io| {
        embind.emval_invoke(
            voice as u32,
            caller as u32,
            handle as u32,
            method as u32,
            destructors as u32,
            args as u32,
            io,
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_void(voice: i32, raw: i32, name: i32) {
    with_embind(|embind, io| embind.register_void(voice as u32, raw as u32, name as u32, io));
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_bool(voice: i32, raw: i32, name: i32, yes: i32, no: i32) {
    with_embind(|embind, io| {
        embind.register_bool(
            voice as u32,
            raw as u32,
            name as u32,
            yes as u32,
            no as u32,
            io,
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_integer(
    voice: i32,
    raw: i32,
    name: i32,
    size: i32,
    min: i32,
    max: i32,
) {
    with_embind(|embind, io| {
        embind.register_integer(
            voice as u32,
            raw as u32,
            name as u32,
            size as u32,
            min,
            max,
            io,
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_bigint(
    voice: i32,
    raw: i32,
    name: i32,
    size: i32,
    min: i64,
    max: i64,
) {
    with_embind(|embind, io| {
        embind.register_bigint(
            voice as u32,
            raw as u32,
            name as u32,
            size as u32,
            min,
            max,
            io,
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_float(voice: i32, raw: i32, name: i32, size: i32) {
    with_embind(|embind, io| {
        embind.register_float(voice as u32, raw as u32, name as u32, size as u32, io);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_std_string(voice: i32, raw: i32, name: i32) {
    with_embind(|embind, io| {
        embind.register_std_string(voice as u32, raw as u32, name as u32, io);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_std_wstring(
    voice: i32,
    raw: i32,
    char_size: i32,
    name: i32,
) {
    with_embind(|embind, io| {
        embind.register_std_wstring(voice as u32, raw as u32, char_size as u32, name as u32, io);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_emval(voice: i32, raw: i32) {
    with_embind(|embind, _| embind.register_emval(voice as u32, raw as u32));
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_memory_view(
    voice: i32,
    raw: i32,
    data_type: i32,
    name: i32,
) {
    with_embind(|embind, io| {
        embind.register_memory_view(voice as u32, raw as u32, data_type as u32, name as u32, io);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_value_object(
    voice: i32,
    raw: i32,
    name: i32,
    ctor_sig: i32,
    ctor: i32,
    dtor_sig: i32,
    dtor: i32,
) {
    with_embind(|embind, _| {
        embind.register_value_object(
            voice as u32,
            raw as u32,
            name as u32,
            ctor_sig as u32,
            ctor as u32,
            dtor_sig as u32,
            dtor as u32,
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_value_object_field(
    voice: i32,
    struct_type: i32,
    field_name: i32,
    getter_type: i32,
    getter_sig: i32,
    getter: i32,
    getter_context: i32,
    setter_type: i32,
    setter_sig: i32,
    setter: i32,
    setter_context: i32,
) {
    with_embind(|embind, io| {
        embind.register_value_object_field(
            voice as u32,
            struct_type as u32,
            field_name as u32,
            getter_type as u32,
            getter_sig as u32,
            getter as u32,
            getter_context as u32,
            setter_type as u32,
            setter_sig as u32,
            setter as u32,
            setter_context as u32,
            io,
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_finalize_value_object(voice: i32, struct_type: i32) {
    with_embind(|embind, io| {
        embind.finalize_value_object(voice as u32, struct_type as u32, io);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn host___embind_register_function(
    voice: i32,
    name: i32,
    arg_count: i32,
    arg_types: i32,
    signature: i32,
    invoker: i32,
    func: i32,
    is_async: i32,
    is_nonnull_return: i32,
) {
    with_embind(|embind, io| {
        embind.register_function(
            voice as u32,
            name as u32,
            arg_count as u32,
            arg_types as u32,
            signature as u32,
            invoker as u32,
            func as u32,
            is_async as u32,
            is_nonnull_return as u32,
            io,
        );
    });
}
