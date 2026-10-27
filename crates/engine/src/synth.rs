//! Dispatch `AqSynthParam` onto the voice module that was linked for it.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

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

/// Embind registrations for the linked voices.
pub struct Session {
    pub embind: Embind,
}

impl Session {
    pub fn new() -> Self {
        Self {
            embind: Embind::new(),
        }
    }

    /// Run a voice's initializer once. The initializer calls back into embind.
    pub fn init_voice(&self, voice: u32, io: &mut dyn VoiceIo) {
        if self.embind.is_inited(voice) {
            return;
        }
        io.init(voice);
        self.embind.flush_with(voice, io);
        self.embind.mark_inited(voice);
    }

    pub fn synthesize(
        &self,
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
        let balance = self.balance_token(voice, io);
        let func = self.embind.function(voice, "synthe").ok_or(ERR_OPEN)?;
        let voice_row = if param.version == AqVersion::V10 {
            aq10_voice(param)
        } else {
            None
        };
        let args = bind_args(
            &self.embind,
            voice,
            &func,
            text,
            param,
            voice_row.as_ref(),
            &balance,
        )
        .ok_or(ERR_OPEN)?;
        let value = self
            .embind
            .invoke(voice, &func, &args, io)
            .map_err(|_| ERR_OPEN)?;
        value_to_wav(value)
    }

    fn apply_license(&self, voice: u32, io: &mut dyn VoiceIo) {
        let Some(func) = self.embind.function(voice, "verifyLicense") else {
            return;
        };
        let jwt = license_jwt();
        let Some(args) = bind_key(&self.embind, voice, &func, &jwt) else {
            return;
        };
        let _ = self.embind.invoke(voice, &func, &args, io);
        self.embind.clear_threw();
    }

    /// `verifyLicense` takes the JWT. `synthe` takes the balance token that call issues.
    fn balance_token(&self, voice: u32, io: &mut dyn VoiceIo) -> String {
        self.apply_license(voice, io);
        let Some(func) = self.embind.function(voice, "issueBalanceToken") else {
            return String::new();
        };
        let value = match self.embind.invoke(voice, &func, &[], io) {
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

fn license_jwt() -> String {
    let bytes = STANDARD
        .decode(env!("LICENSE_JWT_B64"))
        .expect("LICENSE_JWT_B64 is valid base64");
    String::from_utf8(bytes).expect("LICENSE_JWT_B64 decodes to UTF-8")
}

pub fn find_voice(voices: &[LinkedVoice], param: &AqSynthParam) -> Option<u32> {
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
    balance: &str,
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
    let mut used_phont = false;
    let mut used_key = false;
    let mut args = Vec::with_capacity(labels.len());
    for label in &labels {
        if label == "string" || label == "wstring" {
            if !used_text {
                args.push(HostArg::Text(text.to_string()));
                used_text = true;
            } else if !used_phont
                && let Some(phont) = param.phont.as_deref().filter(|phont| !phont.is_empty())
            {
                args.push(HostArg::Text(phont.to_string()));
                used_phont = true;
            } else if !used_key {
                args.push(HostArg::Text(balance.to_string()));
                used_key = true;
            } else {
                args.push(HostArg::Text(String::new()));
            }
        } else if label == "bool" {
            args.push(HostArg::I32(0));
        } else if label.starts_with("int") {
            args.push(HostArg::I32(ints.next()?));
        } else if label == "float4" || label == "float8" {
            args.push(HostArg::F64(param.speed as f64));
        } else if label.starts_with("view") {
            let Some(bytes) = param.phont_data.clone() else {
                return None;
            };
            args.push(HostArg::Bytes(bytes));
        } else if let Some(object) = object_arg(embind, voice, label, param, row) {
            args.push(object);
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

fn object_arg(
    embind: &Embind,
    voice: u32,
    label: &str,
    param: &AqSynthParam,
    row: Option<&Aq10Voice>,
) -> Option<HostArg> {
    let specs = embind.object_fields(voice, label);
    if specs.is_empty() {
        return None;
    }
    let mut fields = Vec::with_capacity(specs.len());
    for spec in specs {
        let (name, _) = spec.split_once(':')?;
        fields.push((
            name.to_string(),
            HostArg::I32(field_value(name, param, row)?),
        ));
    }
    Some(HostArg::Object(fields))
}

fn field_value(name: &str, param: &AqSynthParam, row: Option<&Aq10Voice>) -> Option<i32> {
    let picked = |value: i32| Some(value);
    match name {
        "bas" => picked(row.map(|row| row.bas).unwrap_or(param.base)),
        "spd" => picked(row.map(|row| row.spd).unwrap_or(or_100(param.speed))),
        "vol" => picked(row.map(|row| row.vol).unwrap_or(or_100(param.volume))),
        "pit" => picked(row.map(|row| row.pit).unwrap_or(or_100(param.pitch))),
        "acc" => picked(row.map(|row| row.acc).unwrap_or(or_100(param.accent))),
        "lmd" => picked(row.map(|row| row.lmd).unwrap_or(or_100(param.lmd))),
        "fsc" => picked(row.map(|row| row.fsc).unwrap_or(or_100(param.fsc))),
        "monotone" => Some(0),
        _ => None,
    }
}

fn or_100(value: i32) -> i32 {
    if value == 0 { 100 } else { value }
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
