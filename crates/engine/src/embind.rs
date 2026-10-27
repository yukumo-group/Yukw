//! Embind host shared by every voice module.
//! Registrations follow the emscripten glue in `vendor/aqtk*.js`.
//! Pointers passed to these functions address that voice's memory.

use std::collections::HashMap;

use crate::voice::{VoiceIo, WireVal, read_cstr, read_u32, write_u32};

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum TypeKind {
    Void,
    Bool { yes: u32, no: u32 },
    Int { size: u32 },
    Float { size: u32 },
    Bigint { size: u32 },
    String,
    WString { char_size: u32 },
    Emval,
    MemoryView { index: u32 },
    Object { name: String, data: ObjectData },
}

#[derive(Clone, Debug)]
struct ObjectData {
    ctor: u32,
    dtor: u32,
    fields: Vec<Field>,
}

#[derive(Clone, Debug)]
struct Field {
    name: String,
    getter_type: u32,
    getter: u32,
    getter_context: u32,
    setter_type: u32,
    setter: u32,
    setter_context: u32,
}

#[derive(Clone, Debug)]
struct TypeInfo {
    kind: TypeKind,
}

#[derive(Clone, Debug)]
pub struct BoundFn {
    pub name: String,
    pub arg_types: Vec<u32>,
    pub invoker: u32,
    pub func: u32,
    pub signature: String,
}

struct PendingFn {
    name_ptr: u32,
    arg_types_addr: u32,
    arg_count: u32,
    signature_ptr: u32,
    invoker: u32,
    func: u32,
}

struct PendingObject {
    name_ptr: u32,
    ctor: u32,
    dtor: u32,
    fields: Vec<Field>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum EmValue {
    Undef,
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Object(usize),
}

struct EmSlot {
    value: EmValue,
    refs: u32,
}

/// A value the host passes into a registered function.
#[derive(Clone, Debug, PartialEq)]
pub enum HostArg {
    I32(i32),
    F64(f64),
    Text(String),
    Bytes(Vec<u8>),
    Object(Vec<(String, HostArg)>),
}

/// A value returned from a registered function.
#[derive(Clone, Debug, PartialEq)]
pub enum HostValue {
    Void,
    I32(i32),
    F64(f64),
    Text(String),
    Bytes(Vec<u8>),
}

/// Registrations and emval handles for every voice in the linked module.
#[derive(Default)]
pub struct Embind {
    types: HashMap<(u32, u32), TypeInfo>,
    functions: Vec<(u32, BoundFn)>,
    pending_fns: Vec<(u32, PendingFn)>,
    pending_objects: HashMap<(u32, u32), PendingObject>,
    inited: Vec<u32>,
    threw: bool,
    slots: Vec<Option<EmSlot>>,
    objects: Vec<HashMap<String, EmValue>>,
    invokers: Vec<(u32, u32, u32)>,
}

impl Embind {
    pub fn new() -> Self {
        let mut slots = Vec::new();
        // Match the handles the glue treats as fixed: 2 undefined, 4 null, 6 true, 8 false.
        for _ in 0..10 {
            slots.push(None);
        }
        slots[2] = Some(EmSlot {
            value: EmValue::Undef,
            refs: 1,
        });
        slots[4] = Some(EmSlot {
            value: EmValue::Null,
            refs: 1,
        });
        slots[6] = Some(EmSlot {
            value: EmValue::Bool(true),
            refs: 1,
        });
        slots[8] = Some(EmSlot {
            value: EmValue::Bool(false),
            refs: 1,
        });
        Self {
            slots,
            ..Self::default()
        }
    }

    pub fn threw(&self) -> bool {
        self.threw
    }

    pub fn clear_threw(&mut self) {
        self.threw = false;
    }

    pub fn is_inited(&self, voice: u32) -> bool {
        self.inited.contains(&voice)
    }

    pub fn mark_inited(&mut self, voice: u32) {
        if !self.is_inited(voice) {
            self.inited.push(voice);
        }
    }

    pub fn functions(&self, voice: u32) -> Vec<&BoundFn> {
        self.functions
            .iter()
            .filter(|(id, _)| *id == voice)
            .map(|(_, func)| func)
            .collect()
    }

    pub fn function(&self, voice: u32, name: &str) -> Option<&BoundFn> {
        self.functions(voice)
            .into_iter()
            .find(|func| func.name == name)
    }

    pub fn type_names(&self, voice: u32, func: &BoundFn) -> Vec<String> {
        func.arg_types
            .iter()
            .map(|raw| self.type_label(voice, *raw))
            .collect()
    }

    pub fn register_void(
        &mut self,
        voice: u32,
        raw_type: u32,
        name_ptr: u32,
        io: &mut dyn VoiceIo,
    ) {
        let _name = read_cstr(io, voice, name_ptr);
        self.insert_type(voice, raw_type, TypeKind::Void);
    }

    pub fn register_bool(
        &mut self,
        voice: u32,
        raw_type: u32,
        name_ptr: u32,
        yes: u32,
        no: u32,
        io: &mut dyn VoiceIo,
    ) {
        let _name = read_cstr(io, voice, name_ptr);
        self.insert_type(voice, raw_type, TypeKind::Bool { yes, no });
    }

    pub fn register_integer(
        &mut self,
        voice: u32,
        raw_type: u32,
        name_ptr: u32,
        size: u32,
        _min: i32,
        _max: i32,
        io: &mut dyn VoiceIo,
    ) {
        let _name = read_cstr(io, voice, name_ptr);
        self.insert_type(voice, raw_type, TypeKind::Int { size });
    }

    pub fn register_bigint(
        &mut self,
        voice: u32,
        raw_type: u32,
        name_ptr: u32,
        size: u32,
        _min: i64,
        _max: i64,
        io: &mut dyn VoiceIo,
    ) {
        let _name = read_cstr(io, voice, name_ptr);
        self.insert_type(voice, raw_type, TypeKind::Bigint { size });
    }

    pub fn register_float(
        &mut self,
        voice: u32,
        raw_type: u32,
        name_ptr: u32,
        size: u32,
        io: &mut dyn VoiceIo,
    ) {
        let _name = read_cstr(io, voice, name_ptr);
        self.insert_type(voice, raw_type, TypeKind::Float { size });
    }

    pub fn register_std_string(
        &mut self,
        voice: u32,
        raw_type: u32,
        name_ptr: u32,
        io: &mut dyn VoiceIo,
    ) {
        let _name = read_cstr(io, voice, name_ptr);
        self.insert_type(voice, raw_type, TypeKind::String);
    }

    pub fn register_std_wstring(
        &mut self,
        voice: u32,
        raw_type: u32,
        char_size: u32,
        name_ptr: u32,
        io: &mut dyn VoiceIo,
    ) {
        let _name = read_cstr(io, voice, name_ptr);
        self.insert_type(voice, raw_type, TypeKind::WString { char_size });
    }

    pub fn register_emval(&mut self, voice: u32, raw_type: u32) {
        self.insert_type(voice, raw_type, TypeKind::Emval);
    }

    pub fn register_memory_view(
        &mut self,
        voice: u32,
        raw_type: u32,
        data_type_index: u32,
        name_ptr: u32,
        io: &mut dyn VoiceIo,
    ) {
        let _name = read_cstr(io, voice, name_ptr);
        self.insert_type(
            voice,
            raw_type,
            TypeKind::MemoryView {
                index: data_type_index,
            },
        );
    }

    pub fn register_value_object(
        &mut self,
        voice: u32,
        raw_type: u32,
        name_ptr: u32,
        _ctor_sig: u32,
        ctor: u32,
        _dtor_sig: u32,
        dtor: u32,
    ) {
        self.pending_objects.insert(
            (voice, raw_type),
            PendingObject {
                name_ptr,
                ctor,
                dtor,
                fields: Vec::new(),
            },
        );
    }

    pub fn register_value_object_field(
        &mut self,
        voice: u32,
        struct_type: u32,
        field_name_ptr: u32,
        getter_return_type: u32,
        _getter_sig: u32,
        getter: u32,
        getter_context: u32,
        setter_argument_type: u32,
        _setter_sig: u32,
        setter: u32,
        setter_context: u32,
        io: &mut dyn VoiceIo,
    ) {
        let name = read_cstr(io, voice, field_name_ptr);
        if let Some(object) = self.pending_objects.get_mut(&(voice, struct_type)) {
            object.fields.push(Field {
                name,
                getter_type: getter_return_type,
                getter,
                getter_context,
                setter_type: setter_argument_type,
                setter,
                setter_context,
            });
        }
    }

    pub fn finalize_value_object(&mut self, voice: u32, struct_type: u32, io: &mut dyn VoiceIo) {
        let Some(pending) = self.pending_objects.remove(&(voice, struct_type)) else {
            return;
        };
        let name = read_cstr(io, voice, pending.name_ptr);
        self.insert_type(
            voice,
            struct_type,
            TypeKind::Object {
                name,
                data: ObjectData {
                    ctor: pending.ctor,
                    dtor: pending.dtor,
                    fields: pending.fields,
                },
            },
        );
    }

    pub fn register_function(
        &mut self,
        voice: u32,
        name_ptr: u32,
        arg_count: u32,
        arg_types_addr: u32,
        signature_ptr: u32,
        invoker: u32,
        func: u32,
        _is_async: u32,
        _is_nonnull_return: u32,
        io: &mut dyn VoiceIo,
    ) {
        let type_ids = self.read_type_ids(io, voice, arg_count, arg_types_addr);
        if self.types_ready(voice, &type_ids) {
            self.bind_function(voice, name_ptr, type_ids, signature_ptr, invoker, func, io);
        } else {
            self.pending_fns.push((
                voice,
                PendingFn {
                    name_ptr,
                    arg_types_addr,
                    arg_count,
                    signature_ptr,
                    invoker,
                    func,
                },
            ));
        }
    }

    pub fn resize_heap(&mut self, voice: u32, requested: u32, io: &mut dyn VoiceIo) -> i32 {
        let page = 65_536u32;
        let have = io.memory_pages(voice);
        let have_bytes = have.saturating_mul(page);
        if requested <= have_bytes {
            return 1;
        }
        let need = requested.div_ceil(page);
        let grow_by = need.saturating_sub(have);
        if io.grow(voice, grow_by) < 0 { 0 } else { 1 }
    }

    pub fn cxa_throw(&mut self, _voice: u32, _ptr: u32, _type: u32, _destructor: u32) {
        self.threw = true;
    }

    pub fn abort(&mut self) {
        self.threw = true;
    }

    pub fn emval_decref(&mut self, handle: u32) {
        if handle > 9
            && let Some(Some(slot)) = self.slots.get_mut(handle as usize)
        {
            slot.refs = slot.refs.saturating_sub(1);
            if slot.refs == 0 {
                self.slots[handle as usize] = None;
            }
        }
    }

    pub fn emval_incref(&mut self, handle: u32) {
        if handle > 9
            && let Some(Some(slot)) = self.slots.get_mut(handle as usize)
        {
            slot.refs = slot.refs.saturating_add(1);
        }
    }

    pub fn emval_new_cstring(&mut self, voice: u32, ptr: u32, io: &mut dyn VoiceIo) -> u32 {
        let text = read_cstr(io, voice, ptr);
        self.alloc_emval(EmValue::Str(text))
    }

    pub fn emval_new_object(&mut self) -> u32 {
        let id = self.objects.len();
        self.objects.push(HashMap::new());
        self.alloc_emval(EmValue::Object(id))
    }

    pub fn emval_get_global(&mut self, voice: u32, name_ptr: u32, io: &mut dyn VoiceIo) -> u32 {
        if name_ptr == 0 {
            let id = self.objects.len();
            self.objects.push(HashMap::new());
            return self.alloc_emval(EmValue::Object(id));
        }
        let name = read_cstr(io, voice, name_ptr);
        self.alloc_emval(EmValue::Str(name))
    }

    pub fn emval_set_property(&mut self, object: u32, key: u32, value: u32) {
        let key_text = self.emval_string(key);
        let stored = self
            .slots
            .get(value as usize)
            .and_then(|slot| slot.as_ref())
            .map(|slot| slot.value.clone());
        let Some(key_text) = key_text else {
            return;
        };
        let Some(stored) = stored else {
            return;
        };
        let EmValue::Object(id) = self
            .slots
            .get(object as usize)
            .and_then(|slot| slot.as_ref())
            .map(|slot| slot.value.clone())
            .unwrap_or(EmValue::Undef)
        else {
            return;
        };
        if let Some(object) = self.objects.get_mut(id) {
            object.insert(key_text, stored);
        }
    }

    pub fn emval_run_destructors(&mut self, handle: u32) {
        self.emval_decref(handle);
    }

    pub fn emval_create_invoker(&mut self, arg_count: u32, arg_types: u32, kind: u32) -> u32 {
        let id = self.invokers.len() as u32;
        self.invokers.push((arg_count, arg_types, kind));
        id
    }

    pub fn emval_invoke(
        &mut self,
        _caller: u32,
        _handle: u32,
        _method: u32,
        _destructors: u32,
        _args: u32,
    ) -> f64 {
        0.0
    }

    pub fn random(&mut self) -> f32 {
        0.0
    }

    pub fn date_now(&mut self) -> f64 {
        0.0
    }

    pub fn random_get(&mut self, voice: u32, buf: u32, len: u32, io: &mut dyn VoiceIo) -> i32 {
        if len > 0 && buf != 0 {
            io.write(voice, buf, &vec![0; len as usize]);
        }
        0
    }

    pub fn gmtime(&mut self, voice: u32, _time: i64, tm_ptr: u32, io: &mut dyn VoiceIo) -> i32 {
        if tm_ptr != 0 {
            io.write(voice, tm_ptr, &[0; 36]);
        }
        0
    }

    pub fn tzset(&mut self, voice: u32, a: u32, b: u32, c: u32, d: u32, io: &mut dyn VoiceIo) {
        // The glue writes timezone offsets through these pointers. Zero means UTC.
        for ptr in [a, b, c, d] {
            if ptr != 0 {
                write_u32(io, voice, ptr, 0);
            }
        }
    }

    /// Call a registered function. `args` follow the parameter types, not the return type.
    pub fn invoke(
        &mut self,
        voice: u32,
        func: &BoundFn,
        args: &[HostArg],
        io: &mut dyn VoiceIo,
    ) -> Result<HostValue, String> {
        if func.arg_types.len() < 1 {
            return Err("function has no return type".to_string());
        }
        let params = &func.arg_types[1..];
        if params.len() != args.len() {
            return Err(format!(
                "expected {} args, got {}",
                params.len(),
                args.len()
            ));
        }
        let mut wired = Vec::with_capacity(params.len() + 1);
        wired.push(WireVal::I32(func.func));
        let mut free_list = Vec::new();
        let mut object_dtors = Vec::new();
        for (raw_type, arg) in params.iter().zip(args) {
            wired.push(self.to_wire(
                voice,
                *raw_type,
                arg,
                io,
                &mut free_list,
                &mut object_dtors,
            )?);
        }
        let invoker = func.invoker;
        let ret_type = func.arg_types[0];
        let returned = io.call(voice, invoker, &wired);
        for ptr in free_list {
            io.free(voice, ptr);
        }
        for (dtor, ptr) in object_dtors {
            io.call(voice, dtor, &[WireVal::I32(ptr)]);
        }
        if self.threw {
            return Err("voice raised".to_string());
        }
        self.from_wire(voice, ret_type, returned, io)
    }

    fn insert_type(&mut self, voice: u32, raw_type: u32, kind: TypeKind) {
        self.types.insert((voice, raw_type), TypeInfo { kind });
    }

    pub fn flush_with(&mut self, voice: u32, io: &mut dyn VoiceIo) {
        let pending = std::mem::take(&mut self.pending_fns);
        let mut still = Vec::new();
        for (id, pending) in pending {
            if id != voice {
                still.push((id, pending));
                continue;
            }
            let type_ids = self.read_type_ids(io, voice, pending.arg_count, pending.arg_types_addr);
            if self.types_ready(voice, &type_ids) {
                self.bind_function(
                    voice,
                    pending.name_ptr,
                    type_ids,
                    pending.signature_ptr,
                    pending.invoker,
                    pending.func,
                    io,
                );
            } else {
                still.push((id, pending));
            }
        }
        self.pending_fns = still;
    }

    fn types_ready(&self, voice: u32, ids: &[u32]) -> bool {
        ids.iter().all(|id| self.types.contains_key(&(voice, *id)))
    }

    fn read_type_ids(&self, io: &mut dyn VoiceIo, voice: u32, count: u32, addr: u32) -> Vec<u32> {
        let mut ids = Vec::with_capacity(count as usize);
        for index in 0..count {
            ids.push(read_u32(io, voice, addr + index * 4));
        }
        ids
    }

    fn bind_function(
        &mut self,
        voice: u32,
        name_ptr: u32,
        arg_types: Vec<u32>,
        signature_ptr: u32,
        invoker: u32,
        func: u32,
        io: &mut dyn VoiceIo,
    ) {
        let mut name = read_cstr(io, voice, name_ptr);
        if let Some(paren) = name.find('(') {
            name.truncate(paren);
            name = name.trim().to_string();
        }
        let signature = read_cstr(io, voice, signature_ptr);
        self.functions.push((
            voice,
            BoundFn {
                name,
                arg_types,
                invoker,
                func,
                signature,
            },
        ));
    }

    fn type_label(&self, voice: u32, raw: u32) -> String {
        match self.types.get(&(voice, raw)).map(|info| &info.kind) {
            Some(TypeKind::Void) => "void".to_string(),
            Some(TypeKind::Bool { .. }) => "bool".to_string(),
            Some(TypeKind::Int { size }) => format!("int{size}"),
            Some(TypeKind::Float { size }) => format!("float{size}"),
            Some(TypeKind::Bigint { size }) => format!("bigint{size}"),
            Some(TypeKind::String) => "string".to_string(),
            Some(TypeKind::WString { .. }) => "wstring".to_string(),
            Some(TypeKind::Emval) => "emval".to_string(),
            Some(TypeKind::MemoryView { index }) => format!("view{index}"),
            Some(TypeKind::Object { name, .. }) => name.clone(),
            None => format!("type{raw}"),
        }
    }

    fn to_wire(
        &mut self,
        voice: u32,
        raw_type: u32,
        arg: &HostArg,
        io: &mut dyn VoiceIo,
        free_list: &mut Vec<u32>,
        object_dtors: &mut Vec<(u32, u32)>,
    ) -> Result<WireVal, String> {
        let kind = self
            .types
            .get(&(voice, raw_type))
            .map(|info| info.kind.clone())
            .ok_or_else(|| format!("unknown type {raw_type}"))?;
        match (kind, arg) {
            (TypeKind::Void, _) => Ok(WireVal::None),
            (TypeKind::Bool { yes, no }, HostArg::I32(value)) => {
                Ok(WireVal::I32(if *value != 0 { yes } else { no }))
            }
            (TypeKind::Int { size }, HostArg::I32(value)) => {
                if size > 4 {
                    Ok(WireVal::I64(*value as u64))
                } else {
                    Ok(WireVal::I32(*value as u32))
                }
            }
            (TypeKind::Float { size }, HostArg::F64(value)) => {
                if size > 4 {
                    Ok(WireVal::F64(value.to_bits()))
                } else {
                    Ok(WireVal::F32((*value as f32).to_bits()))
                }
            }
            (TypeKind::String, HostArg::Text(text)) => {
                let bytes = text.as_bytes();
                let base = io.malloc(voice, (4 + bytes.len() + 1) as u32);
                if base == 0 {
                    return Err("malloc failed".to_string());
                }
                write_u32(io, voice, base, bytes.len() as u32);
                io.write(voice, base + 4, bytes);
                io.write(voice, base + 4 + bytes.len() as u32, &[0]);
                free_list.push(base);
                Ok(WireVal::I32(base))
            }
            (TypeKind::String, HostArg::Bytes(bytes)) => {
                let base = io.malloc(voice, (4 + bytes.len() + 1) as u32);
                if base == 0 {
                    return Err("malloc failed".to_string());
                }
                write_u32(io, voice, base, bytes.len() as u32);
                io.write(voice, base + 4, bytes);
                io.write(voice, base + 4 + bytes.len() as u32, &[0]);
                free_list.push(base);
                Ok(WireVal::I32(base))
            }
            (TypeKind::Object { data, .. }, HostArg::Object(fields)) => {
                let ctor = data.ctor;
                let dtor = data.dtor;
                let field_defs = data.fields.clone();
                let ptr = io.call(voice, ctor, &[]).as_i32();
                for field in &field_defs {
                    let value = fields
                        .iter()
                        .find(|(name, _)| name == &field.name)
                        .map(|(_, value)| value)
                        .ok_or_else(|| format!("missing field {}", field.name))?;
                    let wired =
                        self.to_wire(voice, field.setter_type, value, io, free_list, object_dtors)?;
                    io.call(
                        voice,
                        field.setter,
                        &[WireVal::I32(field.setter_context), WireVal::I32(ptr), wired],
                    );
                }
                object_dtors.push((dtor, ptr));
                Ok(WireVal::I32(ptr))
            }
            (kind, arg) => Err(format!("cannot convert {arg:?} to {kind:?}")),
        }
    }

    fn from_wire(
        &mut self,
        voice: u32,
        raw_type: u32,
        value: WireVal,
        io: &mut dyn VoiceIo,
    ) -> Result<HostValue, String> {
        let kind = self
            .types
            .get(&(voice, raw_type))
            .map(|info| info.kind.clone())
            .ok_or_else(|| format!("unknown return type {raw_type}"))?;
        match kind {
            TypeKind::Void => Ok(HostValue::Void),
            TypeKind::Bool { .. } | TypeKind::Int { .. } => {
                Ok(HostValue::I32(value.as_i32() as i32))
            }
            TypeKind::Float { size } => {
                let number = if size > 4 {
                    f64::from_bits(match value {
                        WireVal::F64(bits) => bits,
                        WireVal::I64(bits) => bits,
                        other => other.as_i32() as u64,
                    })
                } else {
                    f32::from_bits(value.as_i32()).into()
                };
                Ok(HostValue::F64(number))
            }
            TypeKind::String | TypeKind::WString { .. } => {
                let ptr = value.as_i32();
                if ptr == 0 {
                    return Ok(HostValue::Text(String::new()));
                }
                let len = read_u32(io, voice, ptr) as usize;
                let mut bytes = vec![0; len];
                if len > 0 {
                    io.read(voice, ptr + 4, &mut bytes);
                }
                io.free(voice, ptr);
                if matches!(kind, TypeKind::String) {
                    Ok(HostValue::Bytes(bytes))
                } else {
                    Ok(HostValue::Text(
                        String::from_utf8_lossy(&bytes).into_owned(),
                    ))
                }
            }
            TypeKind::MemoryView { index } => {
                let handle = value.as_i32();
                if handle == 0 {
                    return Ok(HostValue::Bytes(Vec::new()));
                }
                let size = read_u32(io, voice, handle) as usize;
                let data = read_u32(io, voice, handle + 4);
                let elem = match index {
                    2 | 3 => 2,
                    4 | 5 | 6 => 4,
                    7 | 8 | 9 => 8,
                    _ => 1,
                };
                let mut bytes = vec![0; size.saturating_mul(elem)];
                if !bytes.is_empty() && data != 0 {
                    io.read(voice, data, &mut bytes);
                }
                Ok(HostValue::Bytes(bytes))
            }
            TypeKind::Object { data, .. } => {
                let ptr = value.as_i32();
                let mut text = String::new();
                for field in &data.fields {
                    let raw = io.call(
                        voice,
                        field.getter,
                        &[WireVal::I32(field.getter_context), WireVal::I32(ptr)],
                    );
                    let value = self.from_wire(voice, field.getter_type, raw, io)?;
                    if !text.is_empty() {
                        text.push(',');
                    }
                    text.push_str(&field.name);
                    text.push('=');
                    text.push_str(&format!("{value:?}"));
                }
                io.call(voice, data.dtor, &[WireVal::I32(ptr)]);
                Ok(HostValue::Text(text))
            }
            TypeKind::Bigint { .. } | TypeKind::Emval => Ok(HostValue::I32(value.as_i32() as i32)),
        }
    }

    fn alloc_emval(&mut self, value: EmValue) -> u32 {
        self.slots.push(Some(EmSlot { value, refs: 1 }));
        (self.slots.len() - 1) as u32
    }

    fn emval_string(&self, handle: u32) -> Option<String> {
        match &self.slots.get(handle as usize)?.as_ref()?.value {
            EmValue::Str(text) => Some(text.clone()),
            EmValue::Number(value) => Some(value.to_string()),
            _ => None,
        }
    }
}
