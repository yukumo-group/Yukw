// Included by each wasm32 runtime. `voice_*` is declared in that runtime.
struct Io;

impl embind::VoiceIo for Io {
    fn read(&mut self, voice: u32, addr: u32, dst: &mut [u8]) {
        if dst.is_empty() {
            return;
        }
        unsafe {
            voice_read(
                voice as i32,
                addr as i32,
                dst.as_mut_ptr() as i32,
                dst.len() as i32,
            );
        }
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

    fn call(&mut self, voice: u32, func: u32, args: &[embind::WireVal]) -> embind::WireVal {
        let mut bytes = vec![0; args.len() * 8];
        for (index, arg) in args.iter().enumerate() {
            let bits = match arg {
                embind::WireVal::I32(value) => u64::from(*value),
                embind::WireVal::I64(value) | embind::WireVal::F64(value) => *value,
                embind::WireVal::F32(value) => u64::from(*value),
                embind::WireVal::None => 0,
            };
            bytes[index * 8..index * 8 + 8].copy_from_slice(&bits.to_le_bytes());
        }
        let argv = if bytes.is_empty() {
            0
        } else {
            bytes.as_ptr() as i32
        };
        let packed = unsafe { voice_call(voice as i32, func as i32, args.len() as i32, argv) };
        embind::WireVal::I64(packed as u64)
    }

    fn init(&mut self, voice: u32) {
        unsafe { voice_init(voice as i32) }
    }
}
