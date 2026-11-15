use synth::AqVersion;

#[test]
fn version_values_match_the_header() {
    assert_eq!(AqVersion::V1 as i32, 1);
    assert_eq!(AqVersion::V2 as i32, 2);
    assert_eq!(AqVersion::V10 as i32, 10);
    assert_eq!(AqVersion::from_i32(1), Some(AqVersion::V1));
    assert_eq!(AqVersion::from_i32(0), None);
}
