use embind::{VoiceIo, WireVal};
use kanji2koe::{ERR_ARG, ERR_OPEN, Reader, Script, Session};
use koe::ToKoe;

struct Nop;

impl VoiceIo for Nop {
    fn read(&mut self, _voice: u32, _addr: u32, dst: &mut [u8]) {
        dst.fill(0);
    }
    fn write(&mut self, _voice: u32, _addr: u32, _src: &[u8]) {}
    fn memory_pages(&mut self, _voice: u32) -> u32 {
        1
    }
    fn grow(&mut self, _voice: u32, _pages: u32) -> i32 {
        0
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

#[test]
fn reader_reports_contract_errors() {
    let session = Session::new();
    let mut io = Nop;
    let mut reader = Reader::new(&session, &[], &mut io, Script::Japanese);
    assert_eq!(reader.to_koe("").unwrap_err().code, ERR_ARG);
    assert_eq!(reader.to_koe("あ").unwrap_err().code, ERR_OPEN);
}
