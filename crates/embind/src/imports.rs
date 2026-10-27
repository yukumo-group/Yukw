//! Canonical embind import names. Vendor modules use different minified field
//! names for this set; the linker rewrites them to `env.<js_name>`.
//! A module may import a subset. Voice glue imports every name.

/// One host function the voice modules import.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Semantic {
    EmvalDecref,
    EmbindRegisterMemoryView,
    EmvalSetProperty,
    EmvalNewCstring,
    EmbindRegisterInteger,
    EmvalRunDestructors,
    EmvalInvoke,
    EmvalCreateInvoker,
    EmbindRegisterValueObjectField,
    EmbindRegisterStdWstring,
    EmbindRegisterFunction,
    CxaThrow,
    EmbindRegisterFloat,
    EmbindRegisterBigint,
    EmvalIncref,
    EmvalGetGlobal,
    EmscriptenRandom,
    EmscriptenResizeHeap,
    AbortJs,
    GmtimeJs,
    TzsetJs,
    RandomGet,
    EmscriptenDateNow,
    EmbindRegisterEmval,
    EmbindRegisterStdString,
    EmbindRegisterBool,
    EmbindRegisterVoid,
    EmbindFinalizeValueObject,
    EmbindRegisterValueObject,
    EmvalNewObject,
}

impl Semantic {
    pub const ALL: [Semantic; 30] = [
        Semantic::EmvalDecref,
        Semantic::EmbindRegisterMemoryView,
        Semantic::EmvalSetProperty,
        Semantic::EmvalNewCstring,
        Semantic::EmbindRegisterInteger,
        Semantic::EmvalRunDestructors,
        Semantic::EmvalInvoke,
        Semantic::EmvalCreateInvoker,
        Semantic::EmbindRegisterValueObjectField,
        Semantic::EmbindRegisterStdWstring,
        Semantic::EmbindRegisterFunction,
        Semantic::CxaThrow,
        Semantic::EmbindRegisterFloat,
        Semantic::EmbindRegisterBigint,
        Semantic::EmvalIncref,
        Semantic::EmvalGetGlobal,
        Semantic::EmscriptenRandom,
        Semantic::EmscriptenResizeHeap,
        Semantic::AbortJs,
        Semantic::GmtimeJs,
        Semantic::TzsetJs,
        Semantic::RandomGet,
        Semantic::EmscriptenDateNow,
        Semantic::EmbindRegisterEmval,
        Semantic::EmbindRegisterStdString,
        Semantic::EmbindRegisterBool,
        Semantic::EmbindRegisterVoid,
        Semantic::EmbindFinalizeValueObject,
        Semantic::EmbindRegisterValueObject,
        Semantic::EmvalNewObject,
    ];

    /// Name as it appears in the emscripten glue, including underscores.
    pub const fn js_name(self) -> &'static str {
        match self {
            Semantic::EmvalDecref => "__emval_decref",
            Semantic::EmbindRegisterMemoryView => "__embind_register_memory_view",
            Semantic::EmvalSetProperty => "__emval_set_property",
            Semantic::EmvalNewCstring => "__emval_new_cstring",
            Semantic::EmbindRegisterInteger => "__embind_register_integer",
            Semantic::EmvalRunDestructors => "__emval_run_destructors",
            Semantic::EmvalInvoke => "__emval_invoke",
            Semantic::EmvalCreateInvoker => "__emval_create_invoker",
            Semantic::EmbindRegisterValueObjectField => "__embind_register_value_object_field",
            Semantic::EmbindRegisterStdWstring => "__embind_register_std_wstring",
            Semantic::EmbindRegisterFunction => "__embind_register_function",
            Semantic::CxaThrow => "___cxa_throw",
            Semantic::EmbindRegisterFloat => "__embind_register_float",
            Semantic::EmbindRegisterBigint => "__embind_register_bigint",
            Semantic::EmvalIncref => "__emval_incref",
            Semantic::EmvalGetGlobal => "__emval_get_global",
            Semantic::EmscriptenRandom => "_emscripten_random",
            Semantic::EmscriptenResizeHeap => "_emscripten_resize_heap",
            Semantic::AbortJs => "__abort_js",
            Semantic::GmtimeJs => "__gmtime_js",
            Semantic::TzsetJs => "__tzset_js",
            Semantic::RandomGet => "_random_get",
            Semantic::EmscriptenDateNow => "_emscripten_date_now",
            Semantic::EmbindRegisterEmval => "__embind_register_emval",
            Semantic::EmbindRegisterStdString => "__embind_register_std_string",
            Semantic::EmbindRegisterBool => "__embind_register_bool",
            Semantic::EmbindRegisterVoid => "__embind_register_void",
            Semantic::EmbindFinalizeValueObject => "__embind_finalize_value_object",
            Semantic::EmbindRegisterValueObject => "__embind_register_value_object",
            Semantic::EmvalNewObject => "__emval_new_object",
        }
    }

    pub fn from_js_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|item| item.js_name() == name)
    }

    /// Export the runtime provides for this import: `host_` plus the glue name.
    pub fn host_export(self) -> String {
        format!("host_{}", self.js_name())
    }
}

/// Import module used after rewriting voice wasm.
pub const IMPORT_MODULE: &str = "env";

/// One binding from a minified import field to its semantic function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportBinding {
    pub field: String,
    pub semantic: Semantic,
}

/// Read `var wasmImports = { field: semantic, ... }`.
/// Field names stay in source order, which is the wasm import order.
pub fn parse_wasm_imports(js: &str) -> Result<Vec<ImportBinding>, String> {
    let marker = "var wasmImports";
    let start = js
        .find(marker)
        .ok_or_else(|| "missing var wasmImports".to_string())?;
    let brace = js[start..]
        .find('{')
        .ok_or_else(|| "wasmImports has no object body".to_string())?
        + start;
    let mut bindings = Vec::new();
    let mut chars = js[brace + 1..].chars().peekable();
    let mut closed = false;
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() || ch == ',' {
            continue;
        }
        if ch == '}' {
            closed = true;
            break;
        }
        if ch == '/' {
            // The glue object has no comments. Reject anything else that looks like one.
            return Err("unexpected comment in wasmImports".to_string());
        }
        let mut field = String::new();
        field.push(ch);
        while let Some(next) = chars.peek() {
            if next.is_ascii_alphanumeric() || *next == '_' {
                field.push(chars.next().unwrap());
            } else {
                break;
            }
        }
        while matches!(chars.peek(), Some(c) if c.is_whitespace() || *c == ':') {
            let c = chars.next().unwrap();
            if c == ':' {
                break;
            }
        }
        while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
            chars.next();
        }
        let mut semantic_name = String::new();
        while let Some(next) = chars.peek() {
            if next.is_ascii_alphanumeric() || *next == '_' {
                semantic_name.push(chars.next().unwrap());
            } else {
                break;
            }
        }
        let semantic = Semantic::from_js_name(&semantic_name)
            .ok_or_else(|| format!("unknown import {semantic_name}"))?;
        bindings.push(ImportBinding { field, semantic });
    }
    if !closed {
        return Err("wasmImports object is not closed".to_string());
    }
    if bindings.is_empty() {
        return Err("wasmImports object is empty".to_string());
    }
    Ok(bindings)
}
