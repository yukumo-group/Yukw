//! Dispatch text onto the statically linked AqKanji2Koe engine.

use std::sync::Mutex;

pub const ERR_ARG: i32 = -1;
pub const ERR_SCRIPT: i32 = -2;
pub const ERR_OPEN: i32 = -3;

/// A reading dictionary. Japanese is the statically linked `libaqk2k.a`.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Japanese = 1,
}

impl Script {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Japanese),
            _ => None,
        }
    }
}

/// One dictionary linked into the package, in linker order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkedScript {
    pub script: Script,
}

/// Dictionary buffers kept alive for as long as the engine holds pointers to
/// them.  `aqk2k_create` borrows the system/user dictionaries and
/// `aqk2k_set_lookup` borrows the lookup table.
#[allow(dead_code)]
struct Dictionaries {
    lookup: Vec<u8>,
    system: Vec<u8>,
    user: Vec<u8>,
}

pub struct Session {
    #[allow(dead_code)]
    dictionaries: Mutex<Option<Dictionaries>>,
    #[cfg(target_arch = "wasm32")]
    handle: std::sync::atomic::AtomicU32,
}

impl Session {
    pub fn new() -> Self {
        Self {
            dictionaries: Mutex::new(None),
            #[cfg(target_arch = "wasm32")]
            handle: std::sync::atomic::AtomicU32::new(0),
        }
    }

    /// Load `lookup.bin`, `aqdic.bin` and `aq_user.dic` into the engine.
    ///
    /// The buffers are copied and kept for the lifetime of the session; the
    /// engine borrows them.  On non-wasm builds there is no engine, so this
    /// returns [`ERR_OPEN`].
    pub fn load(&self, lookup: &[u8], system: &[u8], user: &[u8]) -> Result<(), i32> {
        if lookup.is_empty() || system.is_empty() {
            return Err(ERR_ARG);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = user;
            Err(ERR_OPEN)
        }

        #[cfg(target_arch = "wasm32")]
        {
            let mut guard = self.dictionaries.lock().map_err(|_| ERR_OPEN)?;
            let dictionaries = Dictionaries {
                lookup: lookup.to_vec(),
                system: system.to_vec(),
                user: user.to_vec(),
            };
            let user_pointer = if dictionaries.user.is_empty() {
                std::ptr::null()
            } else {
                dictionaries.user.as_ptr()
            };
            unsafe {
                aqk2k_init();
                let rc = aqk2k_set_lookup(
                    dictionaries.lookup.as_ptr(),
                    dictionaries.lookup.len() as u32,
                );
                if rc != 0 {
                    return Err(rc);
                }
                let mut error = 0;
                let handle = aqk2k_create(dictionaries.system.as_ptr(), user_pointer, &mut error);
                if handle.is_null() {
                    return Err(if error == 0 { ERR_OPEN } else { error });
                }
                let previous = self
                    .handle
                    .swap(handle as u32, std::sync::atomic::Ordering::SeqCst);
                *guard = Some(dictionaries);
                if previous != 0 {
                    aqk2k_release(previous as *mut u8);
                }
            }
            Ok(())
        }
    }

    pub fn is_loaded(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            self.handle.load(std::sync::atomic::Ordering::SeqCst) != 0
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            false
        }
    }

    /// Convert `text` with the engine for `script`.
    /// Empty input is [`ERR_ARG`], a missing dictionary is [`ERR_OPEN`], and
    /// an engine error code is returned as-is.
    pub fn convert(&self, script: Script, text: &str) -> Result<String, i32> {
        if text.is_empty() {
            return Err(ERR_ARG);
        }
        if script != Script::Japanese {
            return Err(ERR_SCRIPT);
        }
        self.convert_japanese(text)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn convert_japanese(&self, _text: &str) -> Result<String, i32> {
        Err(ERR_OPEN)
    }

    #[cfg(target_arch = "wasm32")]
    fn convert_japanese(&self, text: &str) -> Result<String, i32> {
        let handle = self.handle.load(std::sync::atomic::Ordering::SeqCst);
        if handle == 0 {
            return Err(ERR_OPEN);
        }

        let mut input = text.as_bytes().to_vec();
        input.push(0);
        let mut capacity = (text.len() * 16 + 4096).max(4096);
        for _ in 0..6 {
            let mut output = vec![0u8; capacity];
            let code = unsafe {
                aqk2k_convert(
                    handle as *mut u8,
                    input.as_ptr(),
                    output.as_mut_ptr(),
                    capacity as i32,
                )
            };
            if code == 0 {
                let len = output
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(output.len());
                return Ok(String::from_utf8_lossy(&output[..len]).into_owned());
            }
            // 0x69: the output buffer is too small.
            if code != 0x69 {
                return Err(code);
            }
            capacity = capacity.saturating_mul(2);
        }
        Err(0x69)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// A [`koe::ToKoe`] view of one linked dictionary.
pub struct Reader<'a> {
    session: &'a Session,
    scripts: &'a [LinkedScript],
    script: Script,
}

impl<'a> Reader<'a> {
    pub fn new(session: &'a Session, scripts: &'a [LinkedScript], script: Script) -> Self {
        Self {
            session,
            scripts,
            script,
        }
    }
}

impl koe::ToKoe for Reader<'_> {
    fn to_koe(&mut self, text: &str) -> Result<String, koe::Error> {
        if text.is_empty() {
            return Err(koe::Error::new(ERR_ARG));
        }
        if find_script(self.scripts, self.script).is_none() {
            return Err(koe::Error::new(ERR_OPEN));
        }
        self.session
            .convert(self.script, text)
            .map_err(koe::Error::new)
    }
}

pub fn find_script(scripts: &[LinkedScript], script: Script) -> Option<u32> {
    scripts
        .iter()
        .position(|module| module.script == script)
        .map(|index| index as u32)
}

#[cfg(target_arch = "wasm32")]
unsafe extern "C" {
    fn aqk2k_init();
    fn aqk2k_set_lookup(data: *const u8, len: u32) -> i32;
    fn aqk2k_create(system: *const u8, user: *const u8, error: *mut i32) -> *mut u8;
    fn aqk2k_release(handle: *mut u8);
    fn aqk2k_convert(handle: *mut u8, kanji: *const u8, koe: *mut u8, capacity: i32) -> i32;
}
