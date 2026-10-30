//! Wasm32 runtime. The host loads the dictionary with `aqk_load`; conversion
//! then calls the statically linked AqKanji2Koe engine directly.

use std::cell::Cell;
use std::sync::OnceLock;

use koe::ToKoe;

use crate::{ERR_SCRIPT, LinkedScript, Reader, Script, Session, find_script};

#[path = "host_alloc.rs"]
mod host_alloc;

struct Shared(Session);
// The module is single-threaded.
unsafe impl Sync for Shared {}

static SESSION: OnceLock<Shared> = OnceLock::new();

struct SyncCell<T>(Cell<T>);
unsafe impl<T> Sync for SyncCell<T> {}

fn session() -> &'static Session {
    &SESSION.get_or_init(|| Shared(Session::new())).0
}

fn read_bytes(pointer: u32, len: u32) -> Vec<u8> {
    if len == 0 || pointer == 0 {
        return Vec::new();
    }
    unsafe { std::slice::from_raw_parts(pointer as *const u8, len as usize) }.to_vec()
}

fn read_text(pointer: u32, len: u32) -> String {
    String::from_utf8_lossy(&read_bytes(pointer, len)).into_owned()
}

static READINGS: SyncCell<Vec<(u32, Vec<u8>)>> = SyncCell(Cell::new(Vec::new()));

fn store_text(text: String) -> (u32, u32) {
    let mut bytes = text.into_bytes();
    let pointer = bytes.as_mut_ptr() as u32;
    let len = bytes.len() as u32;
    let mut readings = READINGS.0.take();
    readings.push((pointer, bytes));
    READINGS.0.set(readings);
    (pointer, len)
}

fn write_u32(pointer: u32, value: u32) {
    if pointer != 0 {
        unsafe { *(pointer as *mut u32) = value }
    }
}

/// Load `lookup.bin`, `aqdic.bin` and `aq_user.dic` (pointer/length pairs).
/// Returns 0 on success, or the engine/argument error code.
#[unsafe(no_mangle)]
pub extern "C" fn aqk_load(
    lookup: u32,
    lookup_len: u32,
    system: u32,
    system_len: u32,
    user: u32,
    user_len: u32,
) -> i32 {
    let lookup = read_bytes(lookup, lookup_len);
    let system = read_bytes(system, system_len);
    let user = read_bytes(user, user_len);
    match session().load(&lookup, &system, &user) {
        Ok(()) => 0,
        Err(code) => code,
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
    let Some(script) = Script::from_i32(script) else {
        return ERR_SCRIPT;
    };
    let text = read_text(text, text_len);
    let scripts = [LinkedScript { script }];
    let mut reader = Reader::new(session(), &scripts, script);
    match reader.to_koe(&text) {
        Ok(reading) => {
            let (pointer, len) = store_text(reading);
            write_u32(out_ptr, pointer);
            write_u32(out_len, len);
            0
        }
        Err(error) => {
            write_u32(out_ptr, 0);
            write_u32(out_len, 0);
            error.code
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn aqk_free(pointer: u32) {
    if pointer == 0 {
        return;
    }
    let mut readings = READINGS.0.take();
    readings.retain(|(stored, _)| *stored != pointer);
    READINGS.0.set(readings);
}

/// 1 when the engine can run `name` for `script`.
#[unsafe(no_mangle)]
pub extern "C" fn aqk_has_function(script: i32, name: u32, name_len: u32) -> i32 {
    let Some(script) = Script::from_i32(script) else {
        return 0;
    };
    let scripts = [LinkedScript { script }];
    if find_script(&scripts, script).is_none() || !session().is_loaded() {
        return 0;
    }
    match read_text(name, name_len).as_str() {
        "convert" => 1,
        _ => 0,
    }
}
