//! `AqSynthParam` and the preset enums from `aqtk-shim/src/Aqsh.h`.
//! Discriminants match the C header. A zero `preset` is f1, aq_yukkuri, or
//! the aq10 custom voice, in that order.

/// AquesTalk engine. Values match `AqVersion`.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AqVersion {
    V1 = 1,
    V2 = 2,
    V10 = 10,
}

impl AqVersion {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::V1),
            2 => Some(Self::V2),
            10 => Some(Self::V10),
            _ => None,
        }
    }
}

/// AquesTalk1 voice libraries. 0 is [`Aq1Preset::F1`].
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aq1Preset {
    F1 = 0,
    F2,
    F3,
    M1,
    M2,
    R1,
    Dvd,
    Imd1,
    Jgr,
}

pub const AQ1_PRESET_COUNT: i32 = 9;

/// AquesTalk2 phont files. 0 is [`Aq2Preset::AqYukkuri`].
/// A non-empty phont path overrides this preset. `phont_data` overrides both.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aq2Preset {
    AqYukkuri = 0,
    AqDefo1,
    AqF1c,
    AqF3a,
    AqHuskey,
    AqM4b,
    AqMf1,
    AqRb2,
    AqRb3,
    AqRm,
    AqRobo,
    AqTeto1,
    ArF4,
    ArM5,
    ArMf2,
    ArRm3,
}

pub const AQ2_PRESET_COUNT: i32 = 16;

/// AquesTalk10 `gVoice_*` presets. 0 uses base/speed/volume/pitch/accent/lmd/fsc.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aq10Preset {
    Custom = 0,
    F1,
    F2,
    F3,
    M1,
    M2,
    R1,
    R2,
}

pub const AQ10_PRESET_COUNT: i32 = 8;

/// Parameters for one synthesis call. Fields follow `AqSynthParam` in `Aqsh.h`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AqSynthParam {
    pub version: AqVersion,
    pub preset: i32,
    pub speed: i32,
    pub phont: Option<String>,
    pub phont_data: Option<Vec<u8>>,
    pub base: i32,
    pub volume: i32,
    pub pitch: i32,
    pub accent: i32,
    pub lmd: i32,
    pub fsc: i32,
    pub dev_key: Option<String>,
    pub user_key: Option<String>,
}

impl AqSynthParam {
    /// `preset` 0 and every numeric field 0. Keys and phont data are absent.
    pub fn new(version: AqVersion) -> Self {
        Self {
            version,
            preset: 0,
            speed: 0,
            phont: None,
            phont_data: None,
            base: 0,
            volume: 0,
            pitch: 0,
            accent: 0,
            lmd: 0,
            fsc: 0,
            dev_key: None,
            user_key: None,
        }
    }
}
