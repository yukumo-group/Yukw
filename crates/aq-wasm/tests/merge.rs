//! Vendor voices link against a stub host runtime.

use std::path::PathBuf;

use aq_wasm::{VoiceModule, link};
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction, MemorySection,
    MemoryType, Module, TypeSection, ValType,
};
use wasmparser::{
    CompositeInnerType, Import, Imports, Parser, Payload, TypeRef, ValType as ParserVal,
};

#[test]
fn vendor_voices_link_against_a_stub_runtime() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor");
    for name in ["aqtk1-f1", "aqtk2", "aqtk10"] {
        let wasm_path = root.join(format!("{name}.wasm"));
        let js_path = root.join(format!("{name}.js"));
        if !wasm_path.exists() || !js_path.exists() {
            continue;
        }
        let wasm = std::fs::read(&wasm_path).unwrap();
        let js = std::fs::read_to_string(&js_path).unwrap();
        let bindings = engine::parse_wasm_imports(&js).unwrap();
        let rewritten = aq_wasm::rewrite_imports(&wasm, &bindings).unwrap();
        let runtime = stub_runtime(&rewritten).unwrap();
        link(
            &runtime,
            &[VoiceModule {
                wasm: &rewritten,
                version: 1,
                preset: 0,
            }],
        )
        .unwrap_or_else(|err| panic!("{name}: {err}"));
    }
}

fn stub_runtime(voice: &[u8]) -> Result<Vec<u8>, String> {
    let imports = func_imports(voice)?;
    let mut types = TypeSection::new();
    let mut functions = FunctionSection::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    for (index, import) in imports.iter().enumerate() {
        let mut params = vec![ValType::I32];
        params.extend(import.params.iter().copied());
        let type_index = types.len();
        types.ty().function(params, import.results.iter().copied());
        functions.function(type_index);
        let mut body = Function::new([]);
        match import.results.as_slice() {
            [] => {}
            [ValType::I32] => {
                body.instruction(&Instruction::I32Const(0));
            }
            [ValType::I64] => {
                body.instruction(&Instruction::I64Const(0));
            }
            [ValType::F32] => {
                body.instruction(&Instruction::F32Const(0f32.into()));
            }
            [ValType::F64] => {
                body.instruction(&Instruction::F64Const(0f64.into()));
            }
            _ => return Err(format!("{} has an unsupported result", import.field)),
        }
        body.instruction(&Instruction::End);
        code.function(&body);
        exports.export(
            &format!("host_{}", import.field),
            ExportKind::Func,
            index as u32,
        );
    }
    exports.export("memory", ExportKind::Memory, 0);
    let mut module = Module::new();
    module.section(&types);
    module.section(&functions);
    module.section(&memories);
    module.section(&exports);
    module.section(&code);
    Ok(module.finish())
}

struct FuncImport {
    field: String,
    params: Vec<ValType>,
    results: Vec<ValType>,
}

fn func_imports(bytes: &[u8]) -> Result<Vec<FuncImport>, String> {
    let mut type_sigs = Vec::new();
    let mut imports = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(|err| err.to_string())?;
        match payload {
            Payload::TypeSection(section) => {
                for group in section {
                    let group = group.map_err(|err| err.to_string())?;
                    for sub in group.types() {
                        let CompositeInnerType::Func(func) = &sub.composite_type.inner else {
                            return Err("only function types can be linked".into());
                        };
                        type_sigs
                            .push((convert_vals(func.params())?, convert_vals(func.results())?));
                    }
                }
            }
            Payload::ImportSection(section) => {
                for group in section {
                    let group = group.map_err(|err| err.to_string())?;
                    match group {
                        Imports::Single(_, import) => {
                            push_import(&mut imports, &type_sigs, import)?
                        }
                        Imports::Compact1 { items, .. } => {
                            for item in items {
                                let item = item.map_err(|err| err.to_string())?;
                                push_import(
                                    &mut imports,
                                    &type_sigs,
                                    Import {
                                        module: "",
                                        name: item.name,
                                        ty: item.ty,
                                    },
                                )?;
                            }
                        }
                        Imports::Compact2 { ty, names, .. } => {
                            for name in names {
                                let name = name.map_err(|err| err.to_string())?;
                                push_import(
                                    &mut imports,
                                    &type_sigs,
                                    Import {
                                        module: "",
                                        name,
                                        ty,
                                    },
                                )?;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(imports)
}

fn push_import(
    imports: &mut Vec<FuncImport>,
    type_sigs: &[(Vec<ValType>, Vec<ValType>)],
    import: Import<'_>,
) -> Result<(), String> {
    let (TypeRef::Func(type_index) | TypeRef::FuncExact(type_index)) = import.ty else {
        return Err(format!(
            "import {}.{} is not a function",
            import.module, import.name
        ));
    };
    let (params, results) = type_sigs
        .get(type_index as usize)
        .ok_or_else(|| format!("type {type_index} is missing"))?;
    imports.push(FuncImport {
        field: import.name.to_string(),
        params: params.clone(),
        results: results.clone(),
    });
    Ok(())
}

fn convert_vals(types: &[ParserVal]) -> Result<Vec<ValType>, String> {
    types
        .iter()
        .map(|ty| match ty {
            ParserVal::I32 => Ok(ValType::I32),
            ParserVal::I64 => Ok(ValType::I64),
            ParserVal::F32 => Ok(ValType::F32),
            ParserVal::F64 => Ok(ValType::F64),
            other => Err(format!("unsupported valtype {other:?}")),
        })
        .collect()
}
