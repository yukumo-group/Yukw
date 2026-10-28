//! Dispatch text onto the reading module that was linked for that script.

use std::cell::RefCell;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use embind::{BoundFn, Embind, HostArg, HostValue, VoiceIo};

pub const ERR_ARG: i32 = -1;
pub const ERR_SCRIPT: i32 = -2;
pub const ERR_OPEN: i32 = -3;

/// A reading dictionary. Japanese is `vendor/aqk2k.wasm`.
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

/// Embind registrations for the linked dictionaries.
pub struct Session {
    pub embind: Embind,
    /// Dictionaries whose `verifyLicense` has already completed.
    licensed: RefCell<Vec<u32>>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            embind: Embind::new(),
            licensed: RefCell::new(Vec::new()),
        }
    }

    /// Run a dictionary's initializer once. The initializer calls back into embind.
    pub fn init_script(&self, script: u32, io: &mut dyn VoiceIo) {
        if self.embind.is_inited(script) {
            return;
        }
        io.init(script);
        self.embind.flush_with(script, io);
        self.embind.mark_inited(script);
    }

    /// Convert `text` with the dictionary for `script`.
    /// An empty input is [`ERR_ARG`]. A missing dictionary or a throw is [`ERR_OPEN`].
    /// A negative code from the module is returned as-is.
    pub fn convert(
        &self,
        scripts: &[LinkedScript],
        io: &mut dyn VoiceIo,
        script: Script,
        text: &str,
    ) -> Result<String, i32> {
        if text.is_empty() {
            return Err(ERR_ARG);
        }
        self.embind.set_clock(LICENSE_CLOCK_MS);
        let module = find_script(scripts, script).ok_or(ERR_OPEN)?;
        self.init_script(module, io);
        if self.embind.threw() {
            self.embind.clear_threw();
            return Err(ERR_OPEN);
        }
        let balance = self.balance_token(module, io);
        let func = self.embind.function(module, "convert").ok_or(ERR_OPEN)?;
        let args = bind_strings(&self.embind, module, &func, &[text, &balance]).ok_or(ERR_OPEN)?;
        let value = self
            .embind
            .invoke(module, &func, &args, io)
            .map_err(|_| ERR_OPEN)?;
        value_to_text(value)
    }

    /// `verifyUsrLicKey`. False when the dictionary is missing or the key is rejected.
    pub fn user_key_valid(
        &self,
        scripts: &[LinkedScript],
        io: &mut dyn VoiceIo,
        script: Script,
        key: &str,
    ) -> bool {
        let Some(module) = find_script(scripts, script) else {
            return false;
        };
        self.embind.set_clock(LICENSE_CLOCK_MS);
        self.init_script(module, io);
        if self.embind.threw() {
            self.embind.clear_threw();
            return false;
        }
        let Some(func) = self.embind.function(module, "verifyUsrLicKey") else {
            return false;
        };
        let Some(args) = bind_strings(&self.embind, module, &func, &[key]) else {
            return false;
        };
        matches!(
            self.embind.invoke(module, &func, &args, io),
            Ok(HostValue::I32(value)) if value != 0
        )
    }

    fn apply_license(&self, module: u32, io: &mut dyn VoiceIo) -> bool {
        let Some(func) = self.embind.function(module, "verifyLicense") else {
            return true;
        };
        let jwt = license_jwt();
        let Some(args) = bind_strings(&self.embind, module, &func, &[&jwt]) else {
            return false;
        };
        let accepted = match self.embind.invoke(module, &func, &args, io) {
            Ok(HostValue::I32(value)) => value != 0,
            Ok(_) => true,
            Err(_) => false,
        };
        self.embind.clear_threw();
        accepted
    }

    /// `verifyLicense` runs once per dictionary. `convert` takes a new balance token each call.
    fn balance_token(&self, module: u32, io: &mut dyn VoiceIo) -> String {
        let fresh = !self.licensed.borrow().contains(&module);
        if fresh && self.apply_license(module, io) {
            self.licensed.borrow_mut().push(module);
        }
        let Some(func) = self.embind.function(module, "issueBalanceToken") else {
            return String::new();
        };
        let value = match self.embind.invoke(module, &func, &[], io) {
            Ok(value) => value,
            Err(_) => {
                self.embind.clear_threw();
                return String::new();
            }
        };
        match value {
            HostValue::Text(text) => text,
            HostValue::Bytes(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            _ => String::new(),
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// Same instant the voice engines present to `verifyLicense`.
const LICENSE_CLOCK_MS: f64 = 1_793_034_672_000.0;

/// A [`koe::ToKoe`] view of one linked dictionary.
pub struct Reader<'a> {
    session: &'a Session,
    scripts: &'a [LinkedScript],
    io: &'a mut dyn VoiceIo,
    script: Script,
}

impl<'a> Reader<'a> {
    pub fn new(
        session: &'a Session,
        scripts: &'a [LinkedScript],
        io: &'a mut dyn VoiceIo,
        script: Script,
    ) -> Self {
        Self {
            session,
            scripts,
            io,
            script,
        }
    }
}

impl koe::ToKoe for Reader<'_> {
    fn to_koe(&mut self, text: &str) -> Result<String, koe::Error> {
        self.session
            .convert(self.scripts, self.io, self.script, text)
            .map_err(koe::Error::new)
    }
}

pub fn find_script(scripts: &[LinkedScript], script: Script) -> Option<u32> {
    scripts
        .iter()
        .position(|module| module.script == script)
        .map(|index| index as u32)
}

fn license_jwt() -> String {
    let bytes = STANDARD
        .decode(env!("LICENSE_JWT_B64"))
        .expect("LICENSE_JWT_B64 is valid base64");
    String::from_utf8(bytes).expect("LICENSE_JWT_B64 decodes to UTF-8")
}

/// Fill string parameters in order. Extra candidates are left unused.
fn bind_strings(
    embind: &Embind,
    module: u32,
    func: &BoundFn,
    texts: &[&str],
) -> Option<Vec<HostArg>> {
    let mut labels = embind.type_names(module, func);
    if labels.is_empty() {
        return None;
    }
    labels.remove(0);
    let mut texts = texts.iter();
    let mut args = Vec::with_capacity(labels.len());
    for label in &labels {
        if label == "string" || label == "wstring" {
            args.push(HostArg::Text((*texts.next()?).to_string()));
        } else {
            return None;
        }
    }
    Some(args)
}

fn value_to_text(value: HostValue) -> Result<String, i32> {
    match value {
        HostValue::Text(text) => Ok(text),
        HostValue::Bytes(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        HostValue::I32(code) if code < 0 => Err(code),
        HostValue::I32(_) | HostValue::Void | HostValue::F64(_) => Err(ERR_OPEN),
    }
}
