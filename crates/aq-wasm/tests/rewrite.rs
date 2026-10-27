use aq_wasm::{import_names, rewrite_imports};
use synth::{ImportBinding, Semantic};
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
    Instruction, Module, TypeSection, ValType,
};

fn voice_with_import(module: &str, field: &str) -> Vec<u8> {
    let mut types = TypeSection::new();
    types.ty().function([], [ValType::I32]);
    let mut imports = ImportSection::new();
    imports.import(module, field, EntityType::Function(0));
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut exports = ExportSection::new();
    exports.export("get", ExportKind::Func, 1);
    let mut body = Function::new([]);
    body.instruction(&Instruction::Call(0));
    body.instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&body);
    let mut module = Module::new();
    module.section(&types);
    module.section(&imports);
    module.section(&functions);
    module.section(&exports);
    module.section(&code);
    module.finish()
}

#[test]
fn different_letters_become_the_same_import() {
    let left = voice_with_import("a", "l");
    let right = voice_with_import("a", "m");
    let left = rewrite_imports(
        &left,
        &[ImportBinding {
            field: "l".into(),
            semantic: Semantic::CxaThrow,
        }],
    )
    .unwrap();
    let right = rewrite_imports(
        &right,
        &[ImportBinding {
            field: "m".into(),
            semantic: Semantic::CxaThrow,
        }],
    )
    .unwrap();
    assert_eq!(
        import_names(&left).unwrap(),
        vec![("env".into(), "___cxa_throw".into())]
    );
    assert_eq!(import_names(&left).unwrap(), import_names(&right).unwrap());
}
