//! AquesTalk1 preset synthesis through the static wasm module.

use super::common::{Session, assert_distinct, assert_riff, built};

#[test]
fn aq1_voices_produce_distinct_wavs() {
    let Some(built) = built() else {
        return;
    };
    let mut session = Session::new(built);
    let mut wavs = Vec::new();
    for (preset, name) in synth::AQ1_VOICES.iter().enumerate() {
        let wav = session
            .preset(1, preset as i32)
            .unwrap_or_else(|code| panic!("aq1 {name}: {code}"));
        assert_riff(&format!("aq1 {name}"), &wav);
        wavs.push((*name, wav));
    }
    assert_distinct(&wavs);
}
