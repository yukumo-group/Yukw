use engine::{Embind, HostArg, HostValue, VoiceIo, WireVal};

struct Fake {
    mem: Vec<u8>,
    calls: Vec<(u32, Vec<WireVal>)>,
    frees: Vec<u32>,
    next: u32,
}

impl Fake {
    fn new() -> Self {
        Self {
            mem: vec![0; 4096],
            calls: Vec::new(),
            frees: Vec::new(),
            next: 1000,
        }
    }

    fn put(&mut self, addr: usize, text: &str) {
        let bytes = text.as_bytes();
        self.mem[addr..addr + bytes.len()].copy_from_slice(bytes);
        self.mem[addr + bytes.len()] = 0;
    }
}

impl VoiceIo for Fake {
    fn read(&mut self, _voice: u32, addr: u32, dst: &mut [u8]) {
        let start = addr as usize;
        dst.copy_from_slice(&self.mem[start..start + dst.len()]);
    }

    fn write(&mut self, _voice: u32, addr: u32, src: &[u8]) {
        let start = addr as usize;
        let end = start + src.len();
        if end > self.mem.len() {
            self.mem.resize(end, 0);
        }
        self.mem[start..end].copy_from_slice(src);
    }

    fn memory_pages(&mut self, _voice: u32) -> u32 {
        1
    }

    fn grow(&mut self, _voice: u32, _pages: u32) -> i32 {
        1
    }

    fn malloc(&mut self, _voice: u32, size: u32) -> u32 {
        let ptr = self.next;
        self.next += size.max(1);
        let end = self.next as usize;
        if end > self.mem.len() {
            self.mem.resize(end, 0);
        }
        ptr
    }

    fn free(&mut self, _voice: u32, ptr: u32) {
        self.frees.push(ptr);
    }

    fn call(&mut self, _voice: u32, func: u32, args: &[WireVal]) -> WireVal {
        self.calls.push((func, args.to_vec()));
        WireVal::I32(0)
    }

    fn init(&mut self, _voice: u32) {}
}

#[test]
fn registration_records_synthe_and_call_wires_the_string() {
    let mut io = Fake::new();
    io.put(0, "void");
    io.put(8, "std::string");
    io.put(32, "synthe");
    io.put(48, "vii");
    io.mem[64..68].copy_from_slice(&1u32.to_le_bytes());
    io.mem[68..72].copy_from_slice(&2u32.to_le_bytes());

    let embind = Embind::new();
    embind.register_void(0, 1, 0, &mut io);
    embind.register_std_string(0, 2, 8, &mut io);
    embind.register_function(0, 32, 2, 64, 48, 10, 11, 0, 0, &mut io);

    let func = embind.function(0, "synthe").unwrap().clone();
    assert_eq!(
        embind.type_names(0, &func),
        vec!["void".to_string(), "string".to_string()]
    );
    let value = embind
        .invoke(0, &func, &[HostArg::Text("hi".to_string())], &mut io)
        .unwrap();
    assert_eq!(value, HostValue::Void);
    let (invoker, args) = &io.calls[0];
    assert_eq!(*invoker, 10);
    assert_eq!(args[0], WireVal::I32(11));
    let ptr = args[1].as_i32();
    assert_eq!(&io.mem[ptr as usize..ptr as usize + 4], &2u32.to_le_bytes());
    assert_eq!(&io.mem[ptr as usize + 4..ptr as usize + 6], b"hi");
    assert!(io.frees.contains(&ptr));
}
