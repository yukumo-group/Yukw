use synth::{
    AQ1_PRESET_COUNT, AQ1_VOICES, AQ2_DISPLAY_NAMES, AQ2_PHONTS, AQ2_PRESET_COUNT,
    AQ10_DISPLAY_NAMES, AQ10_PRESET_COUNT, AQ10_VOICES, Aq1Preset, Aq2Preset, Aq10Preset,
    AqVersion, aq1_voice_name, aq2_phont_name, preset_display_name, preset_display_names,
};

#[test]
fn preset_zero_and_counts_match_the_header() {
    assert_eq!(Aq1Preset::F1 as i32, 0);
    assert_eq!(Aq1Preset::Jgr as i32, AQ1_PRESET_COUNT - 1);
    assert_eq!(Aq2Preset::AqYukkuri as i32, 0);
    assert_eq!(Aq2Preset::ArRm3 as i32, 15);
    assert_eq!(Aq2Preset::AqMomo1 as i32, AQ2_PRESET_COUNT - 1);
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
    assert_eq!(
        aq2_phont_name(Aq2Preset::AqMomo1 as i32),
        Some("aq_momo1.phont")
    );
    assert_eq!(aq2_phont_name(-1), None);
}

#[test]
fn preset_display_names_index_from_zero() {
    assert_eq!(preset_display_names(AqVersion::V1)[0], "f1");
    assert_eq!(
        preset_display_names(AqVersion::V1).len(),
        AQ1_PRESET_COUNT as usize
    );
    assert_eq!(preset_display_name(AqVersion::V1, 0), Some("f1"));
    assert_eq!(preset_display_name(AqVersion::V1, AQ1_PRESET_COUNT), None);

    assert_eq!(AQ2_DISPLAY_NAMES.len(), AQ2_PRESET_COUNT as usize);
    for (phont, display) in AQ2_PHONTS.iter().zip(AQ2_DISPLAY_NAMES.iter()) {
        assert_eq!(*phont, format!("{display}.phont"));
    }
    assert_eq!(preset_display_names(AqVersion::V2)[0], "aq_yukkuri");
    assert_eq!(
        preset_display_name(AqVersion::V2, Aq2Preset::AqMomo1 as i32),
        Some("aq_momo1")
    );

    assert_eq!(AQ10_DISPLAY_NAMES.len(), AQ10_PRESET_COUNT as usize);
    assert_eq!(preset_display_names(AqVersion::V10)[0], "custom");
    assert_eq!(
        preset_display_name(AqVersion::V10, Aq10Preset::R2 as i32),
        Some("r2")
    );
}
