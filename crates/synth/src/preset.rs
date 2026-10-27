//! Voice tables copied from `aqtk-shim`. Order matches the preset enums.

use crate::param::{
    AQ1_PRESET_COUNT, AQ2_PRESET_COUNT, AQ10_PRESET_COUNT, Aq10Preset, AqSynthParam,
};

/// AquesTalk1 library stem, same order as `Aq1Preset`.
pub const AQ1_VOICES: [&str; AQ1_PRESET_COUNT as usize] =
    ["f1", "f2", "f3", "m1", "m2", "r1", "dvd", "imd1", "jgr"];

/// AquesTalk2 phont file staged in `phont/`, same order as `Aq2Preset`.
pub const AQ2_PHONTS: [&str; AQ2_PRESET_COUNT as usize] = [
    "aq_yukkuri.phont",
    "aq_defo1.phont",
    "aq_f1c.phont",
    "aq_f3a.phont",
    "aq_huskey.phont",
    "aq_m4b.phont",
    "aq_mf1.phont",
    "aq_rb2.phont",
    "aq_rb3.phont",
    "aq_rm.phont",
    "aq_robo.phont",
    "aq_teto1.phont",
    "ar_f4.phont",
    "ar_m5.phont",
    "ar_mf2.phont",
    "ar_rm3.phont",
];

/// One AquesTalk10 voice row: bas, spd, vol, pit, acc, lmd, fsc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aq10Voice {
    pub bas: i32,
    pub spd: i32,
    pub vol: i32,
    pub pit: i32,
    pub acc: i32,
    pub lmd: i32,
    pub fsc: i32,
}

/// Preset rows from `aqtk10.cpp`. Index 0 is `AQ10_F1`, not custom.
pub const AQ10_VOICES: [Aq10Voice; (AQ10_PRESET_COUNT - 1) as usize] = [
    Aq10Voice {
        bas: 0,
        spd: 100,
        vol: 100,
        pit: 100,
        acc: 100,
        lmd: 100,
        fsc: 100,
    },
    Aq10Voice {
        bas: 1,
        spd: 100,
        vol: 100,
        pit: 77,
        acc: 150,
        lmd: 100,
        fsc: 100,
    },
    Aq10Voice {
        bas: 0,
        spd: 80,
        vol: 100,
        pit: 100,
        acc: 100,
        lmd: 61,
        fsc: 148,
    },
    Aq10Voice {
        bas: 2,
        spd: 100,
        vol: 100,
        pit: 30,
        acc: 100,
        lmd: 100,
        fsc: 100,
    },
    Aq10Voice {
        bas: 2,
        spd: 105,
        vol: 100,
        pit: 45,
        acc: 130,
        lmd: 120,
        fsc: 100,
    },
    Aq10Voice {
        bas: 2,
        spd: 100,
        vol: 100,
        pit: 30,
        acc: 20,
        lmd: 190,
        fsc: 100,
    },
    Aq10Voice {
        bas: 1,
        spd: 70,
        vol: 100,
        pit: 50,
        acc: 50,
        lmd: 50,
        fsc: 180,
    },
];

/// Resolve an aq10 preset the way `voice_from_param` does.
/// A non-zero preset replaces speed and the other voice fields.
/// Custom reads those fields from `param`. An unknown preset returns `None`.
pub fn aq10_voice(param: &AqSynthParam) -> Option<Aq10Voice> {
    if param.preset == Aq10Preset::Custom as i32 {
        return Some(Aq10Voice {
            bas: param.base,
            spd: param.speed,
            vol: param.volume,
            pit: param.pitch,
            acc: param.accent,
            lmd: param.lmd,
            fsc: param.fsc,
        });
    }
    let index = param.preset - (Aq10Preset::F1 as i32);
    AQ10_VOICES.get(index as usize).copied()
}

pub fn aq1_voice_name(preset: i32) -> Option<&'static str> {
    AQ1_VOICES.get(preset as usize).copied()
}

pub fn aq2_phont_name(preset: i32) -> Option<&'static str> {
    AQ2_PHONTS.get(preset as usize).copied()
}
