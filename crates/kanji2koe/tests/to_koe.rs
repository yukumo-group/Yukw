use kanji2koe::{ERR_ARG, ERR_OPEN, LinkedScript, Reader, Script, Session, find_script};
use koe::ToKoe;

#[test]
fn japanese_is_script_one() {
    assert_eq!(Script::from_i32(1), Some(Script::Japanese));
    assert_eq!(Script::from_i32(2), None);
    let linked = [LinkedScript {
        script: Script::Japanese,
    }];
    assert_eq!(find_script(&linked, Script::Japanese), Some(0));
}

#[test]
fn reader_reports_contract_errors() {
    let session = Session::new();
    let mut reader = Reader::new(&session, &[], Script::Japanese);
    assert_eq!(reader.to_koe("").unwrap_err().code, ERR_ARG);
    assert_eq!(reader.to_koe("あ").unwrap_err().code, ERR_OPEN);
}
