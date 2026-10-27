use synth::{
    AQ1_PRESET_COUNT, AQ1_VOICES, AQ2_PHONTS, AQ2_PRESET_COUNT, AQ10_PRESET_COUNT, AQ10_VOICES,
    Aq1Preset, Aq2Preset, Aq10Preset, Aq10Voice, AqSynthParam, AqVersion, aq1_voice_name,
    aq2_phont_name, aq10_voice,
};

#[test]
fn version_values_match_the_header() {
    assert_eq!(AqVersion::V1 as i32, 1);
    assert_eq!(AqVersion::V2 as i32, 2);
    assert_eq!(AqVersion::V10 as i32, 10);
    assert_eq!(AqVersion::from_i32(1), Some(AqVersion::V1));
    assert_eq!(AqVersion::from_i32(0), None);
}

#[test]
fn preset_zero_and_counts_match_the_header() {
    assert_eq!(Aq1Preset::F1 as i32, 0);
    assert_eq!(Aq1Preset::Jgr as i32, AQ1_PRESET_COUNT - 1);
    assert_eq!(Aq2Preset::AqYukkuri as i32, 0);
    assert_eq!(Aq2Preset::ArRm3 as i32, AQ2_PRESET_COUNT - 1);
    assert_eq!(Aq10Preset::Custom as i32, 0);
    assert_eq!(Aq10Preset::R2 as i32, AQ10_PRESET_COUNT - 1);
    assert_eq!(AQ1_VOICES.len(), AQ1_PRESET_COUNT as usize);
    assert_eq!(AQ2_PHONTS.len(), AQ2_PRESET_COUNT as usize);
    assert_eq!(AQ10_VOICES.len(), (AQ10_PRESET_COUNT - 1) as usize);
}

#[test]
fn voice_names_follow_preset_order() {
    assert_eq!(aq1_voice_name(Aq1Preset::F1 as i32), Some("f1"));
    assert_eq!(aq1_voice_name(Aq1Preset::Imd1 as i32), Some("imd1"));
    assert_eq!(aq1_voice_name(Aq1Preset::Jgr as i32), Some("jgr"));
    assert_eq!(aq1_voice_name(AQ1_PRESET_COUNT), None);
    assert_eq!(
        aq2_phont_name(Aq2Preset::AqYukkuri as i32),
        Some("aq_yukkuri.phont")
    );
    assert_eq!(
        aq2_phont_name(Aq2Preset::ArRm3 as i32),
        Some("ar_rm3.phont")
    );
    assert_eq!(aq2_phont_name(-1), None);
}

#[test]
fn aq10_table_matches_aqtk10_cpp() {
    assert_eq!(
        AQ10_VOICES[0],
        Aq10Voice {
            bas: 0,
            spd: 100,
            vol: 100,
            pit: 100,
            acc: 100,
            lmd: 100,
            fsc: 100
        }
    );
    assert_eq!(
        AQ10_VOICES[2],
        Aq10Voice {
            bas: 0,
            spd: 80,
            vol: 100,
            pit: 100,
            acc: 100,
            lmd: 61,
            fsc: 148
        }
    );
    assert_eq!(
        AQ10_VOICES[6],
        Aq10Voice {
            bas: 1,
            spd: 70,
            vol: 100,
            pit: 50,
            acc: 50,
            lmd: 50,
            fsc: 180
        }
    );
}

#[test]
fn aq10_nonzero_preset_ignores_param_speed() {
    let mut param = AqSynthParam::new(AqVersion::V10);
    param.preset = Aq10Preset::F2 as i32;
    param.speed = 1;
    let voice = aq10_voice(&param).unwrap();
    assert_eq!(voice.bas, 1);
    assert_eq!(voice.spd, 100);
    assert_eq!(voice.pit, 77);
    assert_eq!(voice.acc, 150);
}

#[test]
fn aq10_custom_reads_param_fields() {
    let mut param = AqSynthParam::new(AqVersion::V10);
    param.preset = Aq10Preset::Custom as i32;
    param.base = 2;
    param.speed = 90;
    param.volume = 80;
    param.pitch = 40;
    param.accent = 50;
    param.lmd = 60;
    param.fsc = 70;
    assert_eq!(
        aq10_voice(&param),
        Some(Aq10Voice {
            bas: 2,
            spd: 90,
            vol: 80,
            pit: 40,
            acc: 50,
            lmd: 60,
            fsc: 70
        })
    );
    param.preset = AQ10_PRESET_COUNT;
    assert_eq!(aq10_voice(&param), None);
}
