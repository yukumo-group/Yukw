use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use aq_wasm::myriad_4_link;

#[test]
fn myriad_4_link_replaces_every_copy() {
    let find = xl(env!("M4L_FIND_B64"));
    let replace = xl(env!("M4L_REPLACE_B64"));
    let mut wasm = vec![7u8; 4];
    wasm.extend_from_slice(&find);
    wasm.extend_from_slice(&[9, 9]);
    wasm.extend_from_slice(&find);
    let out = myriad_4_link(&wasm).unwrap();
    assert_eq!(out.len(), wasm.len());
    assert_eq!(&out[4..36], &replace);
    assert_eq!(&out[38..70], &replace);
    assert!(myriad_4_link(&[1, 2, 3]).is_err());
}

fn xl(encoded: &str) -> [u8; 32] {
    STANDARD.decode(encoded).unwrap().try_into().unwrap()
}
