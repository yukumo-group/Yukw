//! AquesTalk10 base/preset synthesis through the static wasm module.

use super::common::{
    Session, aq10_custom, assert_distinct, assert_riff, built, playback_secs, sample_rate,
};

#[test]
fn aq10_bases_produce_distinct_wavs() {
    let Some(built) = built() else {
        return;
    };
    // AquesTalk10 bases are the three 基本素片: F1E, F2E, M1E.
    let mut session = Session::new(built);
    let mut wavs = Vec::new();
    for (base, name) in [(0, "f1e"), (1, "f2e"), (2, "m1e")] {
        let wav = session
            .speak(&aq10_custom(base))
            .unwrap_or_else(|code| panic!("aq10 {name}: {code}"));
        assert_riff(&format!("aq10 {name}"), &wav);
        wavs.push((name, wav));
    }
    assert_distinct(&wavs);
}

#[test]
fn aq10_named_presets_produce_distinct_wavs() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    let mut wavs = Vec::new();
    for (preset, name) in [
        (synth::Aq10Preset::F1 as i32, "f1"),
        (synth::Aq10Preset::F2 as i32, "f2"),
        (synth::Aq10Preset::M1 as i32, "m1"),
    ] {
        let wav = session
            .preset(10, preset)
            .unwrap_or_else(|code| panic!("aq10 preset {name}: {code}"));
        assert_riff(&format!("aq10 preset {name}"), &wav);
        wavs.push((name, wav));
    }
    assert_distinct(&wavs);
}

#[test]
fn aq10_fsc_100_is_16_khz() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    let wav = session.speak(&aq10_custom(0)).expect("aq10 f1e");
    assert_eq!(sample_rate(&wav), 16_000, "aq10 fsc 100 is 16 kHz");
}

#[test]
fn aq10_speed_100_matches_aq1_duration() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    let aq1 = session.preset(1, 0).expect("aq1 f1");
    let aq10 = session.speak(&aq10_custom(0)).expect("aq10 f1e");
    let aq1_secs = playback_secs(&aq1);
    let aq10_secs = playback_secs(&aq10);
    let ratio = aq10_secs / aq1_secs;
    assert!(
        (0.6..=1.5).contains(&ratio),
        "aq10 {aq10_secs:.2}s vs aq1 {aq1_secs:.2}s (ratio {ratio:.2}); speed 100 is the 50-300 scale on both"
    );
}
