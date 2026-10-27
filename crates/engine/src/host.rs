//! Emscripten libc imports the voice modules call. Binding stays in `embind`.

use crate::voice::VoiceIo;

const PAGE: u32 = 65_536;

/// Grow `voice` so it holds `requested` bytes. Returns 1 on success and 0 if growing failed.
pub fn resize_heap(io: &mut dyn VoiceIo, voice: u32, requested: i32) -> i32 {
    let requested = requested.max(0) as u32;
    let have = io.memory_pages(voice);
    if requested <= have.saturating_mul(PAGE) {
        return 1;
    }
    let need = requested.div_ceil(PAGE);
    let grow_by = need.saturating_sub(have);
    if io.grow(voice, grow_by) < 0 { 0 } else { 1 }
}

/// `emscripten_random`. Synthesis does not use the value.
pub fn random() -> f32 {
    0.0
}

/// WASI `random_get`. Fills `buf` with zeros.
pub fn random_get(io: &mut dyn VoiceIo, voice: u32, buf: i32, len: i32) -> i32 {
    let len = len.max(0) as u32;
    if len > 0 && buf != 0 {
        io.write(voice, buf as u32, &vec![0; len as usize]);
    }
    0
}

/// `__gmtime_js`. Writes the UTC `tm` fields emscripten reads. `1` when `time` or `tm` is unusable.
pub fn gmtime(io: &mut dyn VoiceIo, voice: u32, time: i64, tm: i32) -> i32 {
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
    io.write(voice, tm as u32, &fields);
    0
}

/// `__tzset_js`. Zero means UTC.
pub fn tzset(io: &mut dyn VoiceIo, voice: u32, ptrs: [i32; 4]) {
    for ptr in ptrs {
        if ptr != 0 {
            io.write(voice, ptr as u32, &0u32.to_le_bytes());
        }
    }
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
