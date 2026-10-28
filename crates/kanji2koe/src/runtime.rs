//! Wasm32 runtime. Dictionary code calls `host_*`. The host calls back through `voice_*`.

use std::cell::Cell;
use std::sync::OnceLock;

use embind::VoiceIo;
use koe::ToKoe;

use crate::{ERR_OPEN, ERR_SCRIPT, LinkedScript, Reader, Script, Session, find_script};

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

fn note_throw() {
    THREW.0.set(true);
    session().embind.cxa_throw(0, 0, 0, 0);
}

embind::export_host!(session, note_throw, Io);

fn linked_scripts() -> &'static [LinkedScript] {
    static SCRIPTS: OnceLock<Vec<LinkedScript>> = OnceLock::new();
    SCRIPTS.get_or_init(|| {
        let count = unsafe { voice_count() }.max(0);
        (0..count)
            .filter_map(|index| {
                let packed = unsafe { voice_info(index) } as u64;
                let script = Script::from_i32((packed >> 32) as i32)?;
                Some(LinkedScript { script })
            })
            .collect()
    })
}

fn ensure_init(script: u32) {
    if session().embind.is_inited(script) {
        return;
    }
    Io.init(script);
    session().embind.flush_with(script, &mut Io);
    session().embind.mark_inited(script);
}

fn read_text(ptr: u32, len: u32) -> String {
    if len == 0 || ptr == 0 {
        return String::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    String::from_utf8_lossy(bytes).into_owned()
}

static READINGS: SyncCell<Vec<(u32, Vec<u8>)>> = SyncCell(Cell::new(Vec::new()));

fn store_text(text: String) -> (u32, u32) {
    let mut bytes = text.into_bytes();
    let ptr = bytes.as_mut_ptr() as u32;
    let len = bytes.len() as u32;
    let mut readings = READINGS.0.take();
    readings.push((ptr, bytes));
    READINGS.0.set(readings);
    (ptr, len)
}

fn write_u32(ptr: u32, value: u32) {
    if ptr != 0 {
        unsafe { *(ptr as *mut u32) = value }
    }
}

/// Convert UTF-8 `text` with `script` ([`Script`] as `i32`).
/// On success returns 0 and writes the reading pointer and size. Otherwise, a negative code.
#[unsafe(no_mangle)]
pub extern "C" fn aqk_convert(
    script: i32,
    text: u32,
    text_len: u32,
    out_ptr: u32,
    out_len: u32,
) -> i32 {
    THREW.0.set(false);
    let Some(script) = Script::from_i32(script) else {
        return ERR_SCRIPT;
    };
    let text = read_text(text, text_len);
    let mut io = Io;
    let mut reader = Reader::new(session(), linked_scripts(), &mut io, script);
    let result = reader.to_koe(&text).map_err(|err| err.code);
    if THREW.0.replace(false) {
        return ERR_OPEN;
    }
    match result {
        Ok(reading) => {
            let (ptr, len) = store_text(reading);
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
pub extern "C" fn aqk_free(ptr: u32) {
    if ptr == 0 {
        return;
    }
    let mut readings = READINGS.0.take();
    readings.retain(|(stored, _)| *stored != ptr);
    READINGS.0.set(readings);
}

/// 1 when `name` was registered for `script`.
#[unsafe(no_mangle)]
pub extern "C" fn aqk_has_function(script: i32, name: u32, name_len: u32) -> i32 {
    let Some(script) = Script::from_i32(script) else {
        return 0;
    };
    let scripts = linked_scripts();
    let Some(module) = find_script(scripts, script) else {
        return 0;
    };
    ensure_init(module);
    let name = read_text(name, name_len);
    i32::from(session().embind.registered(module, &name))
}
