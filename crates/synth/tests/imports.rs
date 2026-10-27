use synth::{Semantic, parse_wasm_imports};

#[test]
fn fixture_object_keeps_field_order() {
    let mut js = String::from("var wasmImports = {\n");
    for (index, semantic) in Semantic::ALL.iter().enumerate() {
        if index > 0 {
            js.push(',');
        }
        js.push_str(&format!("f{index}: {}", semantic.js_name()));
    }
    js.push_str("\n};");
    let bindings = parse_wasm_imports(&js).unwrap();
    assert_eq!(bindings.len(), Semantic::ALL.len());
    assert_eq!(bindings[0].field, "f0");
    assert_eq!(bindings[0].semantic, Semantic::ALL[0]);
    assert_eq!(bindings[11].semantic.js_name(), "___cxa_throw");
}

#[test]
fn a_subset_of_the_glue_object_parses() {
    let js = "var wasmImports = { a: __emval_decref, b: __emval_new_object };";
    let bindings = parse_wasm_imports(js).unwrap();
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[1].semantic, Semantic::EmvalNewObject);
}

#[test]
fn vendor_glue_uses_the_same_semantic_set() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor");
    if !root.join("aqtk1-f1.js").is_file() {
        return;
    }
    for name in ["aqtk1-f1.js", "aqtk2.js", "aqtk10.js"] {
        let js = std::fs::read_to_string(root.join(name)).unwrap();
        let bindings = parse_wasm_imports(&js).unwrap_or_else(|err| panic!("{name}: {err}"));
        let mut names: Vec<_> = bindings
            .iter()
            .map(|binding| binding.semantic.js_name())
            .collect();
        names.sort_unstable();
        let mut expected: Vec<_> = Semantic::ALL.iter().map(|item| item.js_name()).collect();
        expected.sort_unstable();
        assert_eq!(names, expected, "{name}");
    }
}
