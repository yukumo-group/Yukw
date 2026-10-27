use embind::{Semantic, parse_wasm_imports};
use kanji2koe::{LinkedScript, Script, find_script};

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
fn aqk2k_glue_imports_a_subset_of_embind() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor");
    let path = root.join("aqk2k.js");
    if !path.is_file() {
        return;
    }
    let js = std::fs::read_to_string(path).unwrap();
    let bindings = parse_wasm_imports(&js).unwrap();
    let mut names: Vec<_> = bindings
        .iter()
        .map(|binding| binding.semantic.js_name())
        .collect();
    names.sort_unstable();
    assert!(names.len() < Semantic::ALL.len());
    for required in [
        "__embind_register_function",
        "__embind_register_std_string",
        "__embind_register_std_wstring",
        "__emval_new_object",
        "__emval_set_property",
        "__emval_new_cstring",
        "___cxa_throw",
        "_emscripten_date_now",
        "_emscripten_resize_heap",
    ] {
        assert!(names.contains(&required), "missing {required}");
    }
}
