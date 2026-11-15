//! AquesTalk2 preset and phont synthesis through the static wasm module.

use super::common::{Session, assert_distinct, assert_riff, built};

#[test]
fn aq2_presets_produce_distinct_wavs() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    let mut wavs = Vec::new();
    for (preset, name) in synth::AQ2_PHONTS.iter().enumerate() {
        let wav = session
            .preset(2, preset as i32)
            .unwrap_or_else(|code| panic!("aq2 {name}: {code}"));
        assert_riff(&format!("aq2 {name}"), &wav);
        wavs.push((*name, wav));
    }
    assert_distinct(&wavs);
}

#[test]
fn aq2_unknown_preset_without_phont_fails() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    assert!(
        session.preset(2, synth::AQ2_PRESET_COUNT).is_err(),
        "an unknown aq2 preset without phont data must fail"
    );
}

#[test]
fn aq2_file_phont_matches_builtin_aq_rm() {
    let Some(built) = built() else {
        return;
    };
    let phont_path = built
        .workspace
        .join("../aquestalk-wasm/engines/aquestalk2/aqtk2-lnx/phont/aq_rm.phont");
    if !phont_path.exists() {
        eprintln!("skipping: aq_rm.phont is not in the aquestalk-wasm checkout");
        return;
    }
    let preset = synth::AQ2_PHONTS
        .iter()
        .position(|name| *name == "aq_rm.phont")
        .expect("aq_rm.phont is a builtin preset") as i32;
    let mut session = Session::new(built);
    let builtin = session.preset(2, preset).expect("builtin aq_rm");
    let file = std::fs::read(&phont_path).unwrap();
    let mut param = synth::AqSynthParam::new(synth::AqVersion::V2);
    param.preset = synth::AQ2_PRESET_COUNT;
    param.speed = 100;
    param.phont_data = Some(file);
    let custom = session
        .speak(&param)
        .expect("custom phont data overrides an unknown preset");
    assert_eq!(
        custom, builtin,
        "aq_rm.phont must match the inlined AqRm preset"
    );
}
