//! Link a runtime and rewritten voice modules into one multi-memory wasm.
//!
//! The runtime keeps memory 0 and table 0. Each voice keeps its own memory and
//! table. Voice `env.*` imports become shims that call `host_*` with a fixed
//! voice id. The runtime's `voice.*` imports become functions that branch on
//! that id.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use engine::IMPORT_MODULE;
use wasm_encoder::reencode::{Error as ReencodeError, Reencode};
use wasm_encoder::{
    BlockType, CodeSection, DataCountSection, DataSection, ElementSection, ExportKind,
    ExportSection, Function, FunctionSection, GlobalSection, Instruction, MemArg, MemorySection,
    Module, StartSection, TableSection, TypeSection, ValType,
};
use wasmparser::{
    CompositeInnerType, ConstExpr, ElementItems, ElementKind, ExternalKind, Import, Imports,
    Operator, Parser, Payload, TypeRef, ValType as ParserVal,
};

const VOICE_MODULE: &str = "voice";

/// A voice file the linker looks for under `vendor/`, in link order.
pub struct CatalogVoice {
    pub file: &'static str,
    pub version: i32,
    pub preset: i32,
}

/// Present voice modules, skipping `aqtk1-imd` because vendor has no such file.
/// `jgr` keeps preset 8 so a missing `imd1` does not renumber it.
pub const VOICE_CATALOG: &[CatalogVoice] = &[
    CatalogVoice {
        file: "aqtk1-f1",
        version: 1,
        preset: 0,
    },
    CatalogVoice {
        file: "aqtk1-f2",
        version: 1,
        preset: 1,
    },
    CatalogVoice {
        file: "aqtk1-f3",
        version: 1,
        preset: 2,
    },
    CatalogVoice {
        file: "aqtk1-m1",
        version: 1,
        preset: 3,
    },
    CatalogVoice {
        file: "aqtk1-m2",
        version: 1,
        preset: 4,
    },
    CatalogVoice {
        file: "aqtk1-r1",
        version: 1,
        preset: 5,
    },
    CatalogVoice {
        file: "aqtk1-dvd",
        version: 1,
        preset: 6,
    },
    CatalogVoice {
        file: "aqtk1-jgr",
        version: 1,
        preset: 8,
    },
    CatalogVoice {
        file: "aqtk2",
        version: 2,
        preset: -1,
    },
    CatalogVoice {
        file: "aqtk10",
        version: 10,
        preset: -1,
    },
];

/// A voice wasm whose imports already use `env.<semantic>`.
pub struct VoiceModule<'a> {
    pub wasm: &'a [u8],
    /// Engine version packed into `voice_info`.
    pub version: i32,
    /// Aq1 preset packed into `voice_info`, or `-1` for other engines.
    pub preset: i32,
}

/// Merge `runtime` with `voices`. The result imports nothing.
pub fn link(runtime: &[u8], voices: &[VoiceModule<'_>]) -> Result<Vec<u8>, String> {
    let runtime_info = scan(runtime)?;
    if runtime_info.memories != 1 {
        return Err("runtime must define one memory".into());
    }
    let mut voice_infos = Vec::with_capacity(voices.len());
    for voice in voices {
        let info = scan(voice.wasm)?;
        if info.memories != 1 {
            return Err("a voice module must define one memory".into());
        }
        if info.tables > 1 {
            return Err("a voice module must define at most one table".into());
        }
        voice_infos.push(info);
    }

    let mut places = Vec::with_capacity(1 + voices.len());
    let mut cursor = Cursor::default();
    places.push(cursor.place(&runtime_info));
    for info in &voice_infos {
        places.push(cursor.place(info));
    }
    let shim_base = cursor.defined;
    let shim_count: u32 = voice_infos
        .iter()
        .map(|info| info.func_imports.len() as u32)
        .sum();
    let generated_base = shim_base + shim_count;

    fill_runtime_targets(&mut places[0], &runtime_info, generated_base)?;
    let mut shim_at = shim_base;
    for (place, info) in places[1..].iter_mut().zip(&voice_infos) {
        place.import_targets = (0..info.func_imports.len())
            .map(|index| {
                let target = shim_at + index as u32;
                target
            })
            .collect();
        shim_at += info.func_imports.len() as u32;
    }

    let mut hosts = HashMap::new();
    for export in &runtime_info.exports {
        if matches!(export.kind, ExternalKind::Func | ExternalKind::FuncExact) {
            hosts.insert(export.name.clone(), places[0].map_func(export.index)?);
        }
    }

    let mut shims = Vec::new();
    for (voice_id, (place, info)) in places[1..].iter().zip(&voice_infos).enumerate() {
        for import in &info.func_imports {
            if import.module != IMPORT_MODULE {
                return Err(format!(
                    "voice import {}.{} was not rewritten onto {IMPORT_MODULE}",
                    import.module, import.field
                ));
            }
            let host_name = format!("host_{}", import.field);
            let host = *hosts.get(&host_name).ok_or_else(|| {
                format!("runtime does not export {host_name} for {}", import.field)
            })?;
            let sig = info.sig(import.type_index)?;
            shims.push(Shim {
                host,
                voice: voice_id as i32,
                params: sig.params.len() as u32,
                type_index: place.type_base + import.type_index,
            });
        }
    }

    let mut voice_meta = Vec::new();
    for ((place, info), module) in places[1..].iter().zip(&voice_infos).zip(voices) {
        voice_meta.push(voice_runtime(place, info, module)?);
    }

    let mut out = Out::default();
    append(&mut out, &places[0], runtime)?;
    for (place, module) in places[1..].iter().zip(voices) {
        append(&mut out, place, module.wasm)?;
    }
    for shim in &shims {
        out.functions.function(shim.type_index);
        out.code.function(&shim_body(shim));
    }
    let imported_ops = runtime_op_types(&runtime_info, &places[0]);
    emit_voice_ops(&mut out, &imported_ops, &voice_meta)?;

    let mut exports = ExportSection::new();
    let mut seen = HashMap::<String, ()>::new();
    push_exports(&mut exports, &mut seen, &runtime_info, &places[0], None)?;
    for (voice_id, (info, place)) in voice_infos.iter().zip(&places[1..]).enumerate() {
        push_exports(&mut exports, &mut seen, info, place, Some(voice_id as u32))?;
    }

    let mut module = Module::new();
    module.section(&out.types);
    module.section(&out.functions);
    if !out.tables.is_empty() {
        module.section(&out.tables);
    }
    module.section(&out.memories);
    if !out.globals.is_empty() {
        module.section(&out.globals);
    }
    module.section(&exports);
    if let Some(start) = runtime_info.start {
        module.section(&StartSection {
            function_index: places[0].map_func(start)?,
        });
    }
    if !out.elements.is_empty() {
        module.section(&out.elements);
    }
    if out.data_count > 0 {
        module.section(&DataCountSection {
            count: out.data_count,
        });
    }
    module.section(&out.code);
    if out.data_count > 0 {
        module.section(&out.data);
    }
    let bytes = module.finish();
    wasmparser::validate(&bytes).map_err(|err| err.to_string())?;
    Ok(bytes)
}

pub(crate) fn myriad_4_link(wasm: &[u8]) -> Result<Vec<u8>, String> {
    let v = xl(env!("M4L_FIND_B64"))?;
    let t = xl(env!("M4L_REPLACE_B64"))?;
    let positions = wasm
        .windows(32)
        .enumerate()
        .filter_map(|(index, window)| (window == v).then_some(index))
        .collect::<Vec<_>>();
    match positions.as_slice() {
        [offset] => {
            let mut rew = wasm.to_vec();
            rew[*offset..*offset + 32].copy_from_slice(&t);
            Ok(rew)
        }
        [] => Err("m4l0".into()),
        positions => Err(format!("m4l{}", positions.len())),
    }
}

fn xl(encoded: &str) -> Result<[u8; 32], String> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|err| format!("invalid base64: {err}"))?;
    bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| format!("xl must be 32 bytes, got {}", bytes.len()))
}

/// Rewrite every catalog voice that exists in `vendor` and link it with `runtime`.
pub fn link_vendor(runtime: &[u8], vendor: &Path) -> Result<Vec<u8>, String> {
    let mut owned = Vec::new();
    for voice in VOICE_CATALOG {
        let wasm_path = vendor.join(format!("{}.wasm", voice.file));
        let js_path = vendor.join(format!("{}.js", voice.file));
        if !wasm_path.exists() {
            continue;
        }
        let wasm = std::fs::read(&wasm_path).map_err(|err| err.to_string())?;
        let js = std::fs::read_to_string(&js_path).map_err(|err| err.to_string())?;
        let bindings = engine::parse_wasm_imports(&js)?;
        let rewritten = crate::rewrite_imports(&wasm, &bindings)?;
        let rewritten = myriad_4_link(&rewritten)?;
        owned.push((rewritten, voice.version, voice.preset));
    }
    let modules = owned
        .iter()
        .map(|(bytes, version, preset)| VoiceModule {
            wasm: bytes,
            version: *version,
            preset: *preset,
        })
        .collect::<Vec<_>>();
    link(runtime, &modules)
}

struct Shim {
    host: u32,
    voice: i32,
    params: u32,
    type_index: u32,
}

struct CallSlot {
    type_index: u32,
    params: Vec<ValType>,
    results: Vec<ValType>,
}

struct VoiceRuntime {
    memory: u32,
    table: Option<u32>,
    init: Vec<u32>,
    malloc: Option<u32>,
    free: Option<u32>,
    version: i32,
    preset: i32,
    slots: Vec<Option<CallSlot>>,
}

struct Sig {
    params: Vec<ValType>,
    results: Vec<ValType>,
}

struct FuncImport {
    module: String,
    field: String,
    type_index: u32,
}

struct ExportRec {
    name: String,
    kind: ExternalKind,
    index: u32,
}

struct ElemInit {
    offset: u32,
    funcs: Vec<u32>,
}

struct ModInfo {
    func_imports: Vec<FuncImport>,
    defined_types: Vec<u32>,
    type_sigs: Vec<Sig>,
    memories: u32,
    tables: u32,
    globals: u32,
    data: u32,
    elements: u32,
    exports: Vec<ExportRec>,
    start: Option<u32>,
    elems: Vec<ElemInit>,
}

impl ModInfo {
    fn sig(&self, index: u32) -> Result<&Sig, String> {
        self.type_sigs
            .get(index as usize)
            .ok_or_else(|| format!("type {index} is missing"))
    }

    fn func_type(&self, func: u32) -> Result<u32, String> {
        if (func as usize) < self.func_imports.len() {
            Ok(self.func_imports[func as usize].type_index)
        } else {
            self.defined_types
                .get(func as usize - self.func_imports.len())
                .copied()
                .ok_or_else(|| format!("function {func} has no type"))
        }
    }
}

#[derive(Clone)]
struct Place {
    type_base: u32,
    defined_base: u32,
    memory_base: u32,
    table_base: u32,
    global_base: u32,
    data_base: u32,
    element_base: u32,
    import_targets: Vec<u32>,
}

impl Place {
    fn map_func(&self, func: u32) -> Result<u32, String> {
        if (func as usize) < self.import_targets.len() {
            Ok(self.import_targets[func as usize])
        } else {
            Ok(self.defined_base + func - self.import_targets.len() as u32)
        }
    }
}

#[derive(Default)]
struct Cursor {
    types: u32,
    defined: u32,
    memories: u32,
    tables: u32,
    globals: u32,
    data: u32,
    elements: u32,
}

impl Cursor {
    fn place(&mut self, info: &ModInfo) -> Place {
        let place = Place {
            type_base: self.types,
            defined_base: self.defined,
            memory_base: self.memories,
            table_base: self.tables,
            global_base: self.globals,
            data_base: self.data,
            element_base: self.elements,
            import_targets: Vec::new(),
        };
        self.types += info.type_sigs.len() as u32;
        self.defined += info.defined_types.len() as u32;
        self.memories += info.memories;
        self.tables += info.tables;
        self.globals += info.globals;
        self.data += info.data;
        self.elements += info.elements;
        place
    }
}

fn fill_runtime_targets(
    place: &mut Place,
    info: &ModInfo,
    generated_base: u32,
) -> Result<(), String> {
    for import in &info.func_imports {
        if import.module != VOICE_MODULE {
            return Err(format!(
                "runtime import {}.{} is not a {VOICE_MODULE} import",
                import.module, import.field
            ));
        }
        let slot = voice_op_index(&import.field)?;
        place.import_targets.push(generated_base + slot);
    }
    Ok(())
}

fn voice_op_index(name: &str) -> Result<u32, String> {
    let index = match name {
        "voice_read" => 0,
        "voice_write" => 1,
        "voice_grow" => 2,
        "voice_call" => 3,
        "voice_init" => 4,
        "voice_malloc" => 5,
        "voice_free" => 6,
        "voice_info" => 7,
        "voice_count" => 8,
        "voice_pages" => 9,
        other => return Err(format!("unknown voice import {other}")),
    };
    Ok(index)
}

fn voice_runtime(
    place: &Place,
    info: &ModInfo,
    module: &VoiceModule<'_>,
) -> Result<VoiceRuntime, String> {
    let mut init = Vec::new();
    if let Some(start) = info.start {
        init.push(place.map_func(start)?);
    }
    let mut malloc = None;
    let mut free = None;
    for export in &info.exports {
        if !matches!(export.kind, ExternalKind::Func | ExternalKind::FuncExact) {
            continue;
        }
        let func = place.map_func(export.index)?;
        match export.name.as_str() {
            "F" if !init.contains(&func) => init.push(func),
            "H" => malloc = Some(func),
            "I" => free = Some(func),
            _ => {}
        }
    }
    let mut slots: Vec<Option<CallSlot>> = Vec::new();
    for elem in &info.elems {
        for (offset, func) in elem.funcs.iter().enumerate() {
            let index = elem.offset as usize + offset;
            if slots.len() <= index {
                slots.resize_with(index + 1, || None);
            }
            let ty = info.func_type(*func)?;
            let sig = info.sig(ty)?;
            slots[index] = Some(CallSlot {
                type_index: place.type_base + ty,
                params: sig.params.clone(),
                results: sig.results.clone(),
            });
        }
    }
    Ok(VoiceRuntime {
        memory: place.memory_base,
        table: (info.tables == 1).then_some(place.table_base),
        init,
        malloc,
        free,
        version: module.version,
        preset: module.preset,
        slots,
    })
}

#[derive(Default)]
struct Out {
    types: TypeSection,
    functions: FunctionSection,
    tables: TableSection,
    memories: MemorySection,
    globals: GlobalSection,
    elements: ElementSection,
    code: CodeSection,
    data: DataSection,
    data_count: u32,
}

struct Remap {
    place: Place,
}

impl Reencode for Remap {
    type Error = String;

    fn function_index(&mut self, func: u32) -> Result<u32, ReencodeError<String>> {
        self.place.map_func(func).map_err(ReencodeError::UserError)
    }

    fn type_index(&mut self, ty: u32) -> Result<u32, ReencodeError<String>> {
        Ok(self.place.type_base + ty)
    }

    fn memory_index(&mut self, memory: u32) -> Result<u32, ReencodeError<String>> {
        Ok(self.place.memory_base + memory)
    }

    fn table_index(&mut self, table: u32) -> Result<u32, ReencodeError<String>> {
        Ok(self.place.table_base + table)
    }

    fn global_index(&mut self, global: u32) -> Result<u32, ReencodeError<String>> {
        Ok(self.place.global_base + global)
    }

    fn data_index(&mut self, data: u32) -> Result<u32, ReencodeError<String>> {
        Ok(self.place.data_base + data)
    }

    fn element_index(&mut self, element: u32) -> Result<u32, ReencodeError<String>> {
        Ok(self.place.element_base + element)
    }
}

fn append(out: &mut Out, place: &Place, bytes: &[u8]) -> Result<(), String> {
    let mut remap = Remap {
        place: place.clone(),
    };
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(|err| err.to_string())?;
        let result = match payload {
            Payload::Version { .. }
            | Payload::ImportSection(_)
            | Payload::ExportSection(_)
            | Payload::StartSection { .. }
            | Payload::DataCountSection { .. }
            | Payload::CustomSection(_)
            | Payload::CodeSectionStart { .. }
            | Payload::End(_) => Ok(()),
            Payload::TypeSection(section) => remap
                .parse_type_section(&mut out.types, section)
                .map_err(|err| err.to_string()),
            Payload::FunctionSection(section) => remap
                .parse_function_section(&mut out.functions, section)
                .map_err(|err| err.to_string()),
            Payload::TableSection(section) => remap
                .parse_table_section(&mut out.tables, section)
                .map_err(|err| err.to_string()),
            Payload::MemorySection(section) => remap
                .parse_memory_section(&mut out.memories, section)
                .map_err(|err| err.to_string()),
            Payload::GlobalSection(section) => remap
                .parse_global_section(&mut out.globals, section)
                .map_err(|err| err.to_string()),
            Payload::ElementSection(section) => remap
                .parse_element_section(&mut out.elements, section)
                .map_err(|err| err.to_string()),
            Payload::DataSection(section) => {
                let count = section.count();
                remap
                    .parse_data_section(&mut out.data, section)
                    .map_err(|err| err.to_string())?;
                out.data_count += count;
                Ok(())
            }
            Payload::CodeSectionEntry(body) => remap
                .parse_function_body(&mut out.code, body)
                .map_err(|err| err.to_string()),
            Payload::TagSection(_) => Err("tag sections are not linked".into()),
            Payload::UnknownSection { id, .. } => Err(format!("unknown section {id}")),
            other => Err(format!("unsupported section {other:?}")),
        };
        result?;
    }
    Ok(())
}

fn push_exports(
    exports: &mut ExportSection,
    seen: &mut HashMap<String, ()>,
    info: &ModInfo,
    place: &Place,
    voice: Option<u32>,
) -> Result<(), String> {
    for export in &info.exports {
        let (name, kind, index) = match export.kind {
            ExternalKind::Func | ExternalKind::FuncExact => {
                if voice.is_none() && export.name.starts_with("host_") {
                    continue;
                }
                let name = match voice {
                    Some(id) => format!("v{id}_{}", export.name),
                    None => export.name.clone(),
                };
                (name, ExportKind::Func, place.map_func(export.index)?)
            }
            ExternalKind::Memory if voice.is_none() => (
                export.name.clone(),
                ExportKind::Memory,
                place.memory_base + export.index,
            ),
            ExternalKind::Table if voice.is_none() => (
                export.name.clone(),
                ExportKind::Table,
                place.table_base + export.index,
            ),
            ExternalKind::Global if voice.is_none() => (
                export.name.clone(),
                ExportKind::Global,
                place.global_base + export.index,
            ),
            _ => continue,
        };
        if seen.insert(name.clone(), ()).is_some() {
            return Err(format!("duplicate export {name}"));
        }
        exports.export(&name, kind, index);
    }
    Ok(())
}

fn runtime_op_types(info: &ModInfo, place: &Place) -> HashMap<String, u32> {
    info.func_imports
        .iter()
        .map(|import| (import.field.clone(), place.type_base + import.type_index))
        .collect()
}

fn emit_voice_ops(
    out: &mut Out,
    imported: &HashMap<String, u32>,
    voices: &[VoiceRuntime],
) -> Result<(), String> {
    let read = op_type(out, imported, "voice_read", &[ValType::I32; 4], &[])?;
    let write = op_type(out, imported, "voice_write", &[ValType::I32; 4], &[])?;
    let grow = op_type(
        out,
        imported,
        "voice_grow",
        &[ValType::I32, ValType::I32],
        &[ValType::I32],
    )?;
    let call = op_type(
        out,
        imported,
        "voice_call",
        &[ValType::I32; 4],
        &[ValType::I64],
    )?;
    let init = op_type(out, imported, "voice_init", &[ValType::I32], &[])?;
    let malloc = op_type(
        out,
        imported,
        "voice_malloc",
        &[ValType::I32, ValType::I32],
        &[ValType::I32],
    )?;
    let free = op_type(
        out,
        imported,
        "voice_free",
        &[ValType::I32, ValType::I32],
        &[],
    )?;
    let info = op_type(
        out,
        imported,
        "voice_info",
        &[ValType::I32],
        &[ValType::I64],
    )?;
    let count = op_type(out, imported, "voice_count", &[], &[ValType::I32])?;
    let pages = op_type(
        out,
        imported,
        "voice_pages",
        &[ValType::I32],
        &[ValType::I32],
    )?;

    out.functions.function(read);
    out.code.function(&copy_fn(voices, true)?);
    out.functions.function(write);
    out.code.function(&copy_fn(voices, false)?);
    out.functions.function(grow);
    out.code.function(&grow_fn(voices));
    out.functions.function(call);
    out.code.function(&call_fn(voices)?);
    out.functions.function(init);
    out.code.function(&init_fn(voices));
    out.functions.function(malloc);
    out.code.function(&malloc_fn(voices));
    out.functions.function(free);
    out.code.function(&free_fn(voices));
    out.functions.function(info);
    out.code.function(&info_fn(voices));
    out.functions.function(count);
    out.code.function(&count_fn(voices.len() as i32));
    out.functions.function(pages);
    out.code.function(&pages_fn(voices));
    Ok(())
}

fn count_fn(voices: i32) -> Function {
    let mut func = Function::new([]);
    op(&mut func, Instruction::I32Const(voices));
    op(&mut func, Instruction::End);
    func
}

fn pages_fn(voices: &[VoiceRuntime]) -> Function {
    let mut func = Function::new([]);
    emit_switch(&mut func, voices.len() as u32, 0, |func, index| {
        op(func, Instruction::MemorySize(voices[index as usize].memory));
    });
    op(&mut func, Instruction::End);
    func
}

fn op_type(
    out: &mut Out,
    imported: &HashMap<String, u32>,
    name: &str,
    params: &[ValType],
    results: &[ValType],
) -> Result<u32, String> {
    if let Some(index) = imported.get(name) {
        return Ok(*index);
    }
    let index = out.types.len();
    out.types
        .ty()
        .function(params.iter().copied(), results.iter().copied());
    Ok(index)
}

fn shim_body(shim: &Shim) -> Function {
    let mut func = Function::new([]);
    op(&mut func, Instruction::I32Const(shim.voice));
    for index in 0..shim.params {
        op(&mut func, Instruction::LocalGet(index));
    }
    op(&mut func, Instruction::Call(shim.host));
    op(&mut func, Instruction::End);
    func
}

fn copy_fn(voices: &[VoiceRuntime], read: bool) -> Result<Function, String> {
    let mut func = Function::new([(1, ValType::I32)]);
    emit_switch(&mut func, voices.len() as u32, 0, |func, index| {
        let voice = &voices[index as usize];
        if read {
            emit_copy(func, voice.memory, 1, 0, 2);
        } else {
            emit_copy(func, 0, 2, voice.memory, 1);
        }
    });
    op(&mut func, Instruction::End);
    Ok(func)
}

fn emit_copy(func: &mut Function, src_mem: u32, src_base: u32, dst_mem: u32, dst_base: u32) {
    op(func, Instruction::Loop(BlockType::Empty));
    op(func, Instruction::LocalGet(4));
    op(func, Instruction::LocalGet(3));
    op(func, Instruction::I32GeU);
    op(func, Instruction::If(BlockType::Empty));
    op(func, Instruction::Return);
    op(func, Instruction::End);
    op(func, Instruction::LocalGet(dst_base));
    op(func, Instruction::LocalGet(4));
    op(func, Instruction::I32Add);
    op(func, Instruction::LocalGet(src_base));
    op(func, Instruction::LocalGet(4));
    op(func, Instruction::I32Add);
    op(
        func,
        Instruction::I32Load8U(MemArg {
            offset: 0,
            align: 0,
            memory_index: src_mem,
        }),
    );
    op(
        func,
        Instruction::I32Store8(MemArg {
            offset: 0,
            align: 0,
            memory_index: dst_mem,
        }),
    );
    op(func, Instruction::LocalGet(4));
    op(func, Instruction::I32Const(1));
    op(func, Instruction::I32Add);
    op(func, Instruction::LocalSet(4));
    op(func, Instruction::Br(0));
    op(func, Instruction::End);
}

fn grow_fn(voices: &[VoiceRuntime]) -> Function {
    let mut func = Function::new([]);
    emit_switch(&mut func, voices.len() as u32, 0, |func, index| {
        op(func, Instruction::LocalGet(1));
        op(func, Instruction::MemoryGrow(voices[index as usize].memory));
    });
    op(&mut func, Instruction::End);
    func
}

fn init_fn(voices: &[VoiceRuntime]) -> Function {
    let mut func = Function::new([]);
    emit_switch(&mut func, voices.len() as u32, 0, |func, index| {
        for func_index in &voices[index as usize].init {
            op(func, Instruction::Call(*func_index));
        }
    });
    op(&mut func, Instruction::End);
    func
}

fn malloc_fn(voices: &[VoiceRuntime]) -> Function {
    let mut func = Function::new([]);
    emit_switch(
        &mut func,
        voices.len() as u32,
        0,
        |func, index| match voices[index as usize].malloc {
            Some(target) => {
                op(func, Instruction::LocalGet(1));
                op(func, Instruction::Call(target));
            }
            None => op(func, Instruction::Unreachable),
        },
    );
    op(&mut func, Instruction::End);
    func
}

fn free_fn(voices: &[VoiceRuntime]) -> Function {
    let mut func = Function::new([]);
    emit_switch(
        &mut func,
        voices.len() as u32,
        0,
        |func, index| match voices[index as usize].free {
            Some(target) => {
                op(func, Instruction::LocalGet(1));
                op(func, Instruction::Call(target));
            }
            None => op(func, Instruction::Unreachable),
        },
    );
    op(&mut func, Instruction::End);
    func
}

fn info_fn(voices: &[VoiceRuntime]) -> Function {
    let mut func = Function::new([]);
    emit_switch(&mut func, voices.len() as u32, 0, |func, index| {
        let voice = &voices[index as usize];
        let packed = ((voice.version as u32 as u64) << 32) | (voice.preset as u32 as u64);
        op(func, Instruction::I64Const(packed as i64));
    });
    op(&mut func, Instruction::End);
    func
}

fn call_fn(voices: &[VoiceRuntime]) -> Result<Function, String> {
    let mut func = Function::new([]);
    emit_switch(&mut func, voices.len() as u32, 0, |func, index| {
        let voice = &voices[index as usize];
        let Some(table) = voice.table else {
            op(func, Instruction::Unreachable);
            return;
        };
        emit_switch(
            func,
            voice.slots.len() as u32,
            1,
            |func, slot_index| match &voice.slots[slot_index as usize] {
                Some(slot) => emit_indirect(func, slot, table),
                None => op(func, Instruction::Unreachable),
            },
        );
    });
    op(&mut func, Instruction::End);
    Ok(func)
}

fn emit_indirect(func: &mut Function, slot: &CallSlot, table: u32) {
    for (index, param) in slot.params.iter().enumerate() {
        op(func, Instruction::LocalGet(3));
        op(
            func,
            Instruction::I64Load(MemArg {
                offset: (index * 8) as u64,
                align: 3,
                memory_index: 0,
            }),
        );
        match param {
            ValType::I32 => op(func, Instruction::I32WrapI64),
            ValType::I64 => {}
            ValType::F32 => {
                op(func, Instruction::I32WrapI64);
                op(func, Instruction::F32ReinterpretI32);
            }
            ValType::F64 => op(func, Instruction::F64ReinterpretI64),
            _ => op(func, Instruction::Unreachable),
        }
    }
    op(func, Instruction::LocalGet(1));
    op(
        func,
        Instruction::CallIndirect {
            type_index: slot.type_index,
            table_index: table,
        },
    );
    match slot.results.as_slice() {
        [] => op(func, Instruction::I64Const(0)),
        [ValType::I32] => op(func, Instruction::I64ExtendI32U),
        [ValType::I64] => {}
        [ValType::F32] => {
            op(func, Instruction::I32ReinterpretF32);
            op(func, Instruction::I64ExtendI32U);
        }
        [ValType::F64] => op(func, Instruction::I64ReinterpretF64),
        _ => op(func, Instruction::Unreachable),
    }
}

fn emit_switch(
    func: &mut Function,
    arms: u32,
    selector: u32,
    mut arm: impl FnMut(&mut Function, u32),
) {
    for _ in 0..=arms {
        op(func, Instruction::Block(BlockType::Empty));
    }
    op(func, Instruction::LocalGet(selector));
    let labels: Vec<u32> = (0..arms).collect();
    op(func, Instruction::BrTable(Cow::Owned(labels), arms));
    for index in 0..arms {
        op(func, Instruction::End);
        arm(func, index);
        op(func, Instruction::Return);
    }
    op(func, Instruction::End);
    op(func, Instruction::Unreachable);
}

fn op(func: &mut Function, instruction: Instruction<'static>) {
    func.instruction(&instruction);
}

fn scan(bytes: &[u8]) -> Result<ModInfo, String> {
    let mut info = ModInfo {
        func_imports: Vec::new(),
        defined_types: Vec::new(),
        type_sigs: Vec::new(),
        memories: 0,
        tables: 0,
        globals: 0,
        data: 0,
        elements: 0,
        exports: Vec::new(),
        start: None,
        elems: Vec::new(),
    };
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
                        info.type_sigs.push(Sig {
                            params: convert_vals(func.params())?,
                            results: convert_vals(func.results())?,
                        });
                    }
                }
            }
            Payload::ImportSection(section) => {
                for group in section {
                    let group = group.map_err(|err| err.to_string())?;
                    match group {
                        Imports::Single(_, import) => push_import(&mut info, import)?,
                        Imports::Compact1 { module, items } => {
                            for item in items {
                                let item = item.map_err(|err| err.to_string())?;
                                push_import(
                                    &mut info,
                                    Import {
                                        module,
                                        name: item.name,
                                        ty: item.ty,
                                    },
                                )?;
                            }
                        }
                        Imports::Compact2 { module, ty, names } => {
                            for name in names {
                                let name = name.map_err(|err| err.to_string())?;
                                push_import(
                                    &mut info,
                                    Import {
                                        module,
                                        name,
                                        ty: ty.clone(),
                                    },
                                )?;
                            }
                        }
                    }
                }
            }
            Payload::FunctionSection(section) => {
                for ty in section {
                    info.defined_types.push(ty.map_err(|err| err.to_string())?);
                }
            }
            Payload::TableSection(section) => info.tables = section.count(),
            Payload::MemorySection(section) => info.memories = section.count(),
            Payload::GlobalSection(section) => info.globals = section.count(),
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export.map_err(|err| err.to_string())?;
                    info.exports.push(ExportRec {
                        name: export.name.to_string(),
                        kind: export.kind,
                        index: export.index,
                    });
                }
            }
            Payload::StartSection { func, .. } => info.start = Some(func),
            Payload::ElementSection(section) => {
                info.elements = section.count();
                for element in section {
                    let element = element.map_err(|err| err.to_string())?;
                    let ElementKind::Active {
                        table_index,
                        offset_expr,
                    } = &element.kind
                    else {
                        continue;
                    };
                    if table_index.unwrap_or(0) != 0 {
                        return Err("voice element segments must target table 0".into());
                    }
                    let ElementItems::Functions(funcs) = element.items else {
                        return Err("element items must be function indices".into());
                    };
                    let mut indices = Vec::new();
                    for func in funcs {
                        indices.push(func.map_err(|err| err.to_string())?);
                    }
                    info.elems.push(ElemInit {
                        offset: const_i32(offset_expr)?,
                        funcs: indices,
                    });
                }
            }
            Payload::DataSection(section) => info.data = section.count(),
            Payload::DataCountSection { count, .. } => {
                if info.data == 0 {
                    info.data = count;
                }
            }
            _ => {}
        }
    }
    Ok(info)
}

fn push_import(info: &mut ModInfo, import: Import<'_>) -> Result<(), String> {
    let (TypeRef::Func(type_index) | TypeRef::FuncExact(type_index)) = import.ty else {
        return Err(format!(
            "import {}.{} is not a function",
            import.module, import.name
        ));
    };
    info.func_imports.push(FuncImport {
        module: import.module.to_string(),
        field: import.name.to_string(),
        type_index,
    });
    Ok(())
}

fn convert_vals(types: &[ParserVal]) -> Result<Vec<ValType>, String> {
    types
        .iter()
        .copied()
        .map(|ty| match ty {
            ParserVal::I32 => Ok(ValType::I32),
            ParserVal::I64 => Ok(ValType::I64),
            ParserVal::F32 => Ok(ValType::F32),
            ParserVal::F64 => Ok(ValType::F64),
            other => Err(format!("unsupported value type {other:?}")),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use wasm_encoder::MemoryType;

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
            let rewritten = crate::rewrite_imports(&wasm, &bindings).unwrap();
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
        let info = scan(voice)?;
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
        for (index, import) in info.func_imports.iter().enumerate() {
            let sig = info.sig(import.type_index)?;
            let mut params = vec![ValType::I32];
            params.extend(sig.params.iter().copied());
            let type_index = types.len();
            types.ty().function(params, sig.results.iter().copied());
            functions.function(type_index);
            let mut body = Function::new([]);
            match sig.results.as_slice() {
                [] => {}
                [ValType::I32] => op(&mut body, Instruction::I32Const(0)),
                [ValType::I64] => op(&mut body, Instruction::I64Const(0)),
                [ValType::F32] => op(&mut body, Instruction::F32Const(0f32.into())),
                [ValType::F64] => op(&mut body, Instruction::F64Const(0f64.into())),
                _ => return Err(format!("{} has an unsupported result", import.field)),
            }
            op(&mut body, Instruction::End);
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
}

fn const_i32(expr: &ConstExpr<'_>) -> Result<u32, String> {
    let mut reader = expr.get_operators_reader();
    match reader.read().map_err(|err| err.to_string())? {
        Operator::I32Const { value } => Ok(value as u32),
        other => Err(format!("element offset must be i32.const, got {other:?}")),
    }
}
