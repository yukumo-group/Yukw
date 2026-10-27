//! Dispatch `AqSynthParam` onto the voice module that was linked for it.

use std::collections::HashMap;

use crate::embind::{BoundFn, Embind, HostArg, HostValue};
use crate::param::{AqSynthParam, AqVersion};
use crate::preset::{Aq10Voice, aq10_voice};
use crate::voice::VoiceIo;

pub const ERR_ARG: i32 = -1;
pub const ERR_VERSION: i32 = -2;
pub const ERR_OPEN: i32 = -3;

/// One voice linked into the package, in linker order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkedVoice {
    pub version: AqVersion,
    /// Set when this module is one AquesTalk1 voice library.
    pub aq1_preset: Option<i32>,
}

#[derive(Clone, Debug, Default)]
struct KeyPair {
    dev: Option<String>,
    usr: Option<String>,
}

/// Keys from `Engine::set_keys` plus the embind registrations.
pub struct Session {
    pub embind: Embind,
    keys: HashMap<i32, KeyPair>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            embind: Embind::new(),
            keys: HashMap::new(),
        }
    }

    pub fn set_keys(&mut self, version: AqVersion, dev: Option<String>, usr: Option<String>) {
        self.keys.insert(version as i32, KeyPair { dev, usr });
    }

    /// Run a voice's initializer once. The initializer calls back into embind.
    pub fn init_voice(&mut self, voice: u32, io: &mut dyn VoiceIo) {
        if self.embind.is_inited(voice) {
            return;
        }
        io.init(voice);
        self.embind.flush_with(voice, io);
        self.embind.mark_inited(voice);
    }

    pub fn synthesize(
        &mut self,
        voices: &[LinkedVoice],
        io: &mut dyn VoiceIo,
        param: &AqSynthParam,
        text: &str,
    ) -> Result<Vec<u8>, i32> {
        if text.is_empty() {
            return Err(ERR_ARG);
        }
        let voice = find_voice(voices, param).ok_or(ERR_OPEN)?;
        self.init_voice(voice, io);
        if self.embind.threw() {
            self.embind.clear_threw();
            return Err(ERR_OPEN);
        }
        self.apply_license(voice, param, io);
        let func = self
            .embind
            .function(voice, "synthe")
            .cloned()
            .ok_or(ERR_OPEN)?;
        let voice_row = if param.version == AqVersion::V10 {
            aq10_voice(param)
        } else {
            None
        };
        let args = bind_args(&self.embind, voice, &func, text, param, voice_row.as_ref())
            .ok_or(ERR_OPEN)?;
        let value = self
            .embind
            .invoke(voice, &func, &args, io)
            .map_err(|_| ERR_OPEN)?;
        value_to_wav(value)
    }

    fn apply_license(&mut self, voice: u32, param: &AqSynthParam, io: &mut dyn VoiceIo) {
        let stored = self
            .keys
            .get(&(param.version as i32))
            .cloned()
            .unwrap_or_default();
        let dev = pick(&param.dev_key, stored.dev.as_deref());
        let usr = pick(&param.user_key, stored.usr.as_deref());
        let Some(func) = self.embind.function(voice, "verifyLicense").cloned() else {
            return;
        };
        for key in [dev, usr].into_iter().flatten() {
            if key.is_empty() {
                continue;
            }
            let args = bind_key(&self.embind, voice, &func, key);
            if let Some(args) = args {
                let _ = self.embind.invoke(voice, &func, &args, io);
                self.embind.clear_threw();
            }
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

fn pick<'a>(param_key: &'a Option<String>, stored: Option<&'a str>) -> Option<&'a str> {
    match param_key {
        Some(key) if !key.is_empty() => Some(key.as_str()),
        _ => stored.filter(|key| !key.is_empty()),
    }
}

fn find_voice(voices: &[LinkedVoice], param: &AqSynthParam) -> Option<u32> {
    voices
        .iter()
        .position(|voice| match param.version {
            AqVersion::V1 => {
                voice.version == AqVersion::V1 && voice.aq1_preset == Some(param.preset)
            }
            version => voice.version == version,
        })
        .map(|index| index as u32)
}

fn bind_args(
    embind: &Embind,
    voice: u32,
    func: &BoundFn,
    text: &str,
    param: &AqSynthParam,
    row: Option<&Aq10Voice>,
) -> Option<Vec<HostArg>> {
    let mut labels = embind.type_names(voice, func);
    if labels.is_empty() {
        return None;
    }
    labels.remove(0);
    let int_count = labels
        .iter()
        .filter(|label| label.starts_with("int"))
        .count();
    let mut ints = int_args(param, row, int_count);
    let mut used_text = false;
    let mut args = Vec::with_capacity(labels.len());
    for label in &labels {
        if label == "string" || label == "wstring" {
            if !used_text {
                args.push(HostArg::Text(text.to_string()));
                used_text = true;
            } else if let Some(phont) = param.phont.as_deref().filter(|phont| !phont.is_empty()) {
                args.push(HostArg::Text(phont.to_string()));
            } else {
                return None;
            }
        } else if label.starts_with("int") {
            args.push(HostArg::I32(ints.next()?));
        } else if label == "float4" || label == "float8" {
            args.push(HostArg::F64(param.speed as f64));
        } else if let Some(row) = row {
            args.push(object_arg(row));
        } else if param.phont_data.is_some() && label.starts_with("view") {
            args.push(HostArg::Bytes(param.phont_data.clone().unwrap_or_default()));
        } else {
            return None;
        }
    }
    Some(args)
}

fn int_args(
    param: &AqSynthParam,
    row: Option<&Aq10Voice>,
    count: usize,
) -> impl Iterator<Item = i32> {
    let values = if let Some(row) = row.filter(|_| count >= 7) {
        vec![
            row.bas, row.spd, row.vol, row.pit, row.acc, row.lmd, row.fsc,
        ]
    } else if let Some(row) = row.filter(|_| count == 1) {
        vec![row.spd]
    } else {
        vec![param.speed]
    };
    values.into_iter()
}

fn object_arg(row: &Aq10Voice) -> HostArg {
    HostArg::Object(vec![
        ("bas".to_string(), HostArg::I32(row.bas)),
        ("spd".to_string(), HostArg::I32(row.spd)),
        ("vol".to_string(), HostArg::I32(row.vol)),
        ("pit".to_string(), HostArg::I32(row.pit)),
        ("acc".to_string(), HostArg::I32(row.acc)),
        ("lmd".to_string(), HostArg::I32(row.lmd)),
        ("fsc".to_string(), HostArg::I32(row.fsc)),
    ])
}

fn bind_key(embind: &Embind, voice: u32, func: &BoundFn, key: &str) -> Option<Vec<HostArg>> {
    let labels = embind.type_names(voice, func);
    let params = labels.get(1..)?;
    if params.len() == 1 && (params[0] == "string" || params[0] == "wstring") {
        Some(vec![HostArg::Text(key.to_string())])
    } else {
        None
    }
}

fn value_to_wav(value: HostValue) -> Result<Vec<u8>, i32> {
    match value {
        HostValue::Bytes(bytes) if !bytes.is_empty() => Ok(bytes),
        HostValue::Text(text) if !text.is_empty() => Ok(text.into_bytes()),
        HostValue::I32(code) if code < 0 => Err(code),
        HostValue::I32(0) | HostValue::Void | HostValue::Text(_) | HostValue::Bytes(_) => {
            Err(ERR_OPEN)
        }
        HostValue::I32(_) => Err(ERR_OPEN),
        HostValue::F64(_) => Err(ERR_OPEN),
    }
}
