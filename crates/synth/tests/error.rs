use synth::{AqVersion, error_message};

#[test]
fn shared_and_engine_codes_use_the_shim_names() {
    assert_eq!(error_message(AqVersion::V1, 100), Some("AQ1_OTHER_ERROR"));
    assert_eq!(
        error_message(AqVersion::V10, 101),
        Some("AQ10_INSUFFICIENT_MEMORY")
    );
    assert_eq!(
        error_message(AqVersion::V1, 200),
        Some("AQ1_PHONETIC_TOO_LONG")
    );
    assert_eq!(
        error_message(AqVersion::V1, 202),
        Some("AQ1_PHONETIC_TOO_LONG")
    );
    assert_eq!(
        error_message(AqVersion::V2, 202),
        Some("AQ2_PHONETIC_BUFFER_OVERFLOW")
    );
    assert_eq!(
        error_message(AqVersion::V10, 120),
        Some("AQ10_PHONETIC_TOO_LONG")
    );
    assert_eq!(
        error_message(AqVersion::V2, 1000),
        Some("AQ2_INVALID_PHONT")
    );
    assert_eq!(
        error_message(AqVersion::V2, 1008),
        Some("AQ2_INVALID_PHONT")
    );
}

#[test]
fn negative_codes_match_the_positive_name() {
    assert_eq!(
        error_message(AqVersion::V10, -122),
        error_message(AqVersion::V10, 122)
    );
    assert_eq!(
        error_message(AqVersion::V10, -122),
        Some("AQ10_PHONETIC_BUFFER_OVERFLOW")
    );
}

#[test]
fn unknown_codes_and_cross_engine_codes_are_absent() {
    assert_eq!(error_message(AqVersion::V1, 0), None);
    assert_eq!(error_message(AqVersion::V1, 999), None);
    assert_eq!(error_message(AqVersion::V1, 120), None);
    assert_eq!(error_message(AqVersion::V10, 200), None);
    assert_eq!(error_message(AqVersion::V1, 1000), None);
    assert_eq!(error_message(AqVersion::V2, 1009), None);
}
