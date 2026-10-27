//! Access to one voice module's memory and table.
//! The linked module implements this with per-voice shims. Tests use a fake.

/// One wasm value passed across `voice_call`.
/// Floats are stored as their IEEE bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireVal {
    I32(u32),
    I64(u64),
    F32(u32),
    F64(u64),
    None,
}

impl WireVal {
    pub fn as_i32(self) -> u32 {
        match self {
            WireVal::I32(value) => value,
            WireVal::I64(value) => value as u32,
            WireVal::None => 0,
            WireVal::F32(bits) => bits,
            WireVal::F64(bits) => bits as u32,
        }
    }
}

/// Operations the embind host needs from the voice that is currently calling in.
pub trait VoiceIo {
    fn read(&mut self, voice: u32, addr: u32, dst: &mut [u8]);
    fn write(&mut self, voice: u32, addr: u32, src: &[u8]);
    fn memory_pages(&mut self, voice: u32) -> u32;
    /// Grow by `pages`. Returns the previous page count, or -1 if it failed.
    fn grow(&mut self, voice: u32, pages: u32) -> i32;
    fn malloc(&mut self, voice: u32, size: u32) -> u32;
    fn free(&mut self, voice: u32, ptr: u32);
    fn call(&mut self, voice: u32, func: u32, args: &[WireVal]) -> WireVal;
    fn init(&mut self, voice: u32);
}

pub(crate) fn read_u32(io: &mut dyn VoiceIo, voice: u32, addr: u32) -> u32 {
    let mut bytes = [0; 4];
    io.read(voice, addr, &mut bytes);
    u32::from_le_bytes(bytes)
}

pub(crate) fn write_u32(io: &mut dyn VoiceIo, voice: u32, addr: u32, value: u32) {
    io.write(voice, addr, &value.to_le_bytes());
}

pub(crate) fn read_cstr(io: &mut dyn VoiceIo, voice: u32, mut addr: u32) -> String {
    if addr == 0 {
        return String::new();
    }
    let limit = io.memory_pages(voice).saturating_mul(65_536);
    let mut bytes = Vec::new();
    let mut buf = [0u8; 256];
    while bytes.len() < 1_048_576 {
        let room = if addr < limit {
            ((limit - addr) as usize)
                .min(buf.len())
                .min(1_048_576 - bytes.len())
        } else {
            1
        };
        io.read(voice, addr, &mut buf[..room]);
        match buf[..room].iter().position(|byte| *byte == 0) {
            Some(end) => {
                bytes.extend_from_slice(&buf[..end]);
                break;
            }
            None => {
                bytes.extend_from_slice(&buf[..room]);
                addr = addr.saturating_add(room as u32);
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
