use engine::host::{gmtime, resize_heap};
use engine::{VoiceIo, WireVal};

struct Mem {
    bytes: Vec<u8>,
    pages: u32,
    grew: u32,
}

impl VoiceIo for Mem {
    fn read(&mut self, _voice: u32, addr: u32, dst: &mut [u8]) {
        let start = addr as usize;
        dst.copy_from_slice(&self.bytes[start..start + dst.len()]);
    }

    fn write(&mut self, _voice: u32, addr: u32, src: &[u8]) {
        let start = addr as usize;
        let end = start + src.len();
        if end > self.bytes.len() {
            self.bytes.resize(end, 0);
        }
        self.bytes[start..end].copy_from_slice(src);
    }

    fn memory_pages(&mut self, _voice: u32) -> u32 {
        self.pages
    }

    fn grow(&mut self, _voice: u32, pages: u32) -> i32 {
        self.grew += pages;
        self.pages += pages;
        (self.pages - pages) as i32
    }

    fn malloc(&mut self, _voice: u32, _size: u32) -> u32 {
        0
    }

    fn free(&mut self, _voice: u32, _ptr: u32) {}

    fn call(&mut self, _voice: u32, _func: u32, _args: &[WireVal]) -> WireVal {
        WireVal::None
    }

    fn init(&mut self, _voice: u32) {}
}

fn i32_at(bytes: &[u8], addr: usize) -> i32 {
    i32::from_le_bytes(bytes[addr..addr + 4].try_into().unwrap())
}

#[test]
fn gmtime_writes_unix_epoch() {
    let mut mem = Mem {
        bytes: vec![0; 64],
        pages: 1,
        grew: 0,
    };
    assert_eq!(gmtime(&mut mem, 0, 0, 16), 0);
    assert_eq!(i32_at(&mem.bytes, 16), 0);
    assert_eq!(i32_at(&mem.bytes, 20), 0);
    assert_eq!(i32_at(&mem.bytes, 24), 0);
    assert_eq!(i32_at(&mem.bytes, 28), 1);
    assert_eq!(i32_at(&mem.bytes, 32), 0);
    assert_eq!(i32_at(&mem.bytes, 36), 70);
    assert_eq!(i32_at(&mem.bytes, 40), 4);
    assert_eq!(i32_at(&mem.bytes, 44), 0);
}

#[test]
fn gmtime_rejects_a_missing_pointer() {
    let mut mem = Mem {
        bytes: vec![0; 16],
        pages: 1,
        grew: 0,
    };
    assert_eq!(gmtime(&mut mem, 0, 0, 0), 1);
    assert_eq!(gmtime(&mut mem, 0, -1, 8), 1);
}

#[test]
fn resize_heap_grows_only_when_the_request_exceeds_the_heap() {
    let mut mem = Mem {
        bytes: Vec::new(),
        pages: 1,
        grew: 0,
    };
    assert_eq!(resize_heap(&mut mem, 0, 100), 1);
    assert_eq!(mem.grew, 0);
    assert_eq!(resize_heap(&mut mem, 0, 70_000), 1);
    assert_eq!(mem.grew, 1);
    assert_eq!(mem.pages, 2);
}
