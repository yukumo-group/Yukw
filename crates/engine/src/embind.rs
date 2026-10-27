//! Embind host shared by every voice module.
//! Registrations follow the emscripten glue in `vendor/aqtk*.js`.
//! Pointers passed to these functions address that voice's memory.

use std::cell::RefCell;
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

/// Pointers for `__embind_register_function` in one voice's memory.
pub struct RegisterFunction {
    pub voice: u32,
    pub name: u32,
    pub arg_count: u32,
    pub arg_types: u32,
    pub signature: u32,
    pub invoker: u32,
    pub func: u32,
    pub is_async: u32,
    pub is_nonnull_return: u32,
}

/// Pointers for `__embind_register_value_object_field` in one voice's memory.
pub struct RegisterValueObjectField {
    pub voice: u32,
    pub struct_type: u32,
    pub field_name: u32,
    pub getter_type: u32,
    pub getter: u32,
    pub getter_context: u32,
    pub setter_type: u32,
    pub setter: u32,
    pub setter_context: u32,
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
    /// A typed-array copy. `memory_view` arguments become this.
    Bytes(Vec<u8>),
    /// A host constructor or global, such as `Uint8Array`.
    Global(String),
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
///
/// State sits behind a [`RefCell`] so a voice can call back into the host
/// while a synthesis call is still on the stack.
pub struct Embind {
    inner: RefCell<Inner>,
}

#[derive(Default)]
struct Inner {
    types: HashMap<(u32, u32), TypeInfo>,
    functions: Vec<(u32, BoundFn)>,
    pending_fns: Vec<(u32, PendingFn)>,
    pending_objects: HashMap<(u32, u32), PendingObject>,
    inited: Vec<u32>,
    threw: bool,
    slots: Vec<Option<EmSlot>>,
    objects: Vec<HashMap<String, EmValue>>,
    invokers: Vec<Invoker>,
    clock_ms: f64,
}

#[derive(Clone)]
struct Invoker {
    types: Vec<u32>,
    kind: u32,
}

impl Inner {
    fn new() -> Self {
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

    /// Field names of a value object, with each setter's type label.
    pub fn object_fields(&self, voice: u32, name: &str) -> Vec<String> {
        self.types
            .iter()
            .find_map(|((id, _), info)| {
                if *id != voice {
                    return None;
                }
                match &info.kind {
                    TypeKind::Object { name: found, data } if found == name => Some(
                        data.fields
                            .iter()
                            .map(|field| {
                                format!(
                                    "{}:{}",
                                    field.name,
                                    self.type_label(voice, field.setter_type)
                                )
                            })
                            .collect(),
                    ),
                    _ => None,
                }
            })
            .unwrap_or_default()
    }

    /// True when `name` is a registered function or value-object type.
    pub fn registered(&self, voice: u32, name: &str) -> bool {
        if self.function(voice, name).is_some() {
            return true;
        }
        self.types.iter().any(|((id, _), info)| {
            *id == voice
                && matches!(&info.kind, TypeKind::Object { name: found, .. } if found == name)
        })
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
        ctor: u32,
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
        args: RegisterValueObjectField,
        io: &mut dyn VoiceIo,
    ) {
        let RegisterValueObjectField {
            voice,
            struct_type,
            field_name,
            getter_type,
            getter,
            getter_context,
            setter_type,
            setter,
            setter_context,
        } = args;
        let name = read_cstr(io, voice, field_name);
        if let Some(object) = self.pending_objects.get_mut(&(voice, struct_type)) {
            object.fields.push(Field {
                name,
                getter_type,
                getter,
                getter_context,
                setter_type,
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

    pub fn register_function(&mut self, args: RegisterFunction, io: &mut dyn VoiceIo) {
        let RegisterFunction {
            voice,
            name: name_ptr,
            arg_count,
            arg_types: arg_types_addr,
            signature: signature_ptr,
            invoker,
            func,
            is_async: _,
            is_nonnull_return: _,
        } = args;
        let pending = PendingFn {
            name_ptr,
            arg_types_addr,
            arg_count,
            signature_ptr,
            invoker,
            func,
        };
        let type_ids = self.read_type_ids(io, voice, arg_count, arg_types_addr);
        if self.types_ready(voice, &type_ids) {
            self.bind_function(voice, type_ids, &pending, io);
        } else {
            self.pending_fns.push((voice, pending));
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
            return self.alloc_emval(EmValue::Global("globalThis".to_string()));
        }
        let name = read_cstr(io, voice, name_ptr);
        self.alloc_emval(EmValue::Global(name))
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

    pub fn emval_create_invoker(
        &mut self,
        voice: u32,
        arg_count: u32,
        arg_types: u32,
        kind: u32,
        io: &mut dyn VoiceIo,
    ) -> u32 {
        let mut types = Vec::with_capacity(arg_count as usize);
        for index in 0..arg_count {
            types.push(read_u32(io, voice, arg_types + index * 4));
        }
        let id = self.invokers.len() as u32;
        self.invokers.push(Invoker { types, kind });
        id
    }

    pub fn random(&mut self) -> f32 {
        0.0
    }

    pub fn set_clock(&mut self, ms: f64) {
        self.clock_ms = ms;
    }

    pub fn date_now(&mut self) -> f64 {
        self.clock_ms
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
                self.bind_function(voice, type_ids, &pending, io);
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
        arg_types: Vec<u32>,
        pending: &PendingFn,
        io: &mut dyn VoiceIo,
    ) {
        let mut name = read_cstr(io, voice, pending.name_ptr);
        if let Some(paren) = name.find('(') {
            name.truncate(paren);
            name = name.trim().to_string();
        }
        let signature = read_cstr(io, voice, pending.signature_ptr);
        self.functions.push((
            voice,
            BoundFn {
                name,
                arg_types,
                invoker: pending.invoker,
                func: pending.func,
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

    fn type_kind(&self, voice: u32, raw: u32) -> Option<TypeKind> {
        self.types.get(&(voice, raw)).map(|info| info.kind.clone())
    }

    fn slot_value(&self, handle: u32) -> EmValue {
        self.slots
            .get(handle as usize)
            .and_then(|slot| slot.as_ref())
            .map(|slot| slot.value.clone())
            .unwrap_or(EmValue::Undef)
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

impl Default for Embind {
    fn default() -> Self {
        Self::new()
    }
}

impl Embind {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(Inner::new()),
        }
    }

    pub fn threw(&self) -> bool {
        self.inner.borrow().threw()
    }

    pub fn clear_threw(&self) {
        self.inner.borrow_mut().clear_threw();
    }

    pub fn is_inited(&self, voice: u32) -> bool {
        self.inner.borrow().is_inited(voice)
    }

    pub fn mark_inited(&self, voice: u32) {
        self.inner.borrow_mut().mark_inited(voice);
    }

    pub fn function(&self, voice: u32, name: &str) -> Option<BoundFn> {
        self.inner.borrow().function(voice, name).cloned()
    }

    pub fn object_fields(&self, voice: u32, name: &str) -> Vec<String> {
        self.inner.borrow().object_fields(voice, name)
    }

    pub fn registered(&self, voice: u32, name: &str) -> bool {
        self.inner.borrow().registered(voice, name)
    }

    pub fn type_names(&self, voice: u32, func: &BoundFn) -> Vec<String> {
        self.inner.borrow().type_names(voice, func)
    }

    pub fn flush_with(&self, voice: u32, io: &mut dyn VoiceIo) {
        self.inner.borrow_mut().flush_with(voice, io);
    }

    pub fn register_void(&self, voice: u32, raw: u32, name: u32, io: &mut dyn VoiceIo) {
        self.inner.borrow_mut().register_void(voice, raw, name, io);
    }

    pub fn register_bool(
        &self,
        voice: u32,
        raw: u32,
        name: u32,
        yes: u32,
        no: u32,
        io: &mut dyn VoiceIo,
    ) {
        self.inner
            .borrow_mut()
            .register_bool(voice, raw, name, yes, no, io);
    }

    pub fn register_integer(
        &self,
        voice: u32,
        raw: u32,
        name: u32,
        size: u32,
        io: &mut dyn VoiceIo,
    ) {
        self.inner
            .borrow_mut()
            .register_integer(voice, raw, name, size, io);
    }

    pub fn register_bigint(
        &self,
        voice: u32,
        raw: u32,
        name: u32,
        size: u32,
        io: &mut dyn VoiceIo,
    ) {
        self.inner
            .borrow_mut()
            .register_bigint(voice, raw, name, size, io);
    }

    pub fn register_float(&self, voice: u32, raw: u32, name: u32, size: u32, io: &mut dyn VoiceIo) {
        self.inner
            .borrow_mut()
            .register_float(voice, raw, name, size, io);
    }

    pub fn register_std_string(&self, voice: u32, raw: u32, name: u32, io: &mut dyn VoiceIo) {
        self.inner
            .borrow_mut()
            .register_std_string(voice, raw, name, io);
    }

    pub fn register_std_wstring(
        &self,
        voice: u32,
        raw: u32,
        char_size: u32,
        name: u32,
        io: &mut dyn VoiceIo,
    ) {
        self.inner
            .borrow_mut()
            .register_std_wstring(voice, raw, char_size, name, io);
    }

    pub fn register_emval(&self, voice: u32, raw: u32) {
        self.inner.borrow_mut().register_emval(voice, raw);
    }

    pub fn register_memory_view(
        &self,
        voice: u32,
        raw: u32,
        data_type: u32,
        name: u32,
        io: &mut dyn VoiceIo,
    ) {
        self.inner
            .borrow_mut()
            .register_memory_view(voice, raw, data_type, name, io);
    }

    pub fn register_value_object(&self, voice: u32, raw: u32, name: u32, ctor: u32, dtor: u32) {
        self.inner
            .borrow_mut()
            .register_value_object(voice, raw, name, ctor, dtor);
    }

    pub fn register_value_object_field(
        &self,
        args: RegisterValueObjectField,
        io: &mut dyn VoiceIo,
    ) {
        self.inner
            .borrow_mut()
            .register_value_object_field(args, io);
    }

    pub fn finalize_value_object(&self, voice: u32, struct_type: u32, io: &mut dyn VoiceIo) {
        self.inner
            .borrow_mut()
            .finalize_value_object(voice, struct_type, io);
    }

    pub fn register_function(&self, args: RegisterFunction, io: &mut dyn VoiceIo) {
        self.inner.borrow_mut().register_function(args, io);
    }

    pub fn cxa_throw(&self, voice: u32, ptr: u32, ty: u32, destructor: u32) {
        self.inner
            .borrow_mut()
            .cxa_throw(voice, ptr, ty, destructor);
    }

    pub fn abort(&self) {
        self.inner.borrow_mut().abort();
    }

    pub fn resize_heap(&self, voice: u32, requested: u32, io: &mut dyn VoiceIo) -> i32 {
        self.inner.borrow_mut().resize_heap(voice, requested, io)
    }

    pub fn random(&self) -> f32 {
        self.inner.borrow_mut().random()
    }

    pub fn set_clock(&self, ms: f64) {
        self.inner.borrow_mut().set_clock(ms);
    }

    pub fn date_now(&self) -> f64 {
        self.inner.borrow_mut().date_now()
    }

    pub fn random_get(&self, voice: u32, buf: u32, len: u32, io: &mut dyn VoiceIo) -> i32 {
        self.inner.borrow_mut().random_get(voice, buf, len, io)
    }

    pub fn gmtime(&self, voice: u32, time: i64, tm_ptr: u32, io: &mut dyn VoiceIo) -> i32 {
        self.inner.borrow_mut().gmtime(voice, time, tm_ptr, io)
    }

    pub fn tzset(&self, voice: u32, a: u32, b: u32, c: u32, d: u32, io: &mut dyn VoiceIo) {
        self.inner.borrow_mut().tzset(voice, a, b, c, d, io);
    }

    pub fn emval_decref(&self, handle: u32) {
        self.inner.borrow_mut().emval_decref(handle);
    }

    pub fn emval_incref(&self, handle: u32) {
        self.inner.borrow_mut().emval_incref(handle);
    }

    pub fn emval_new_cstring(&self, voice: u32, ptr: u32, io: &mut dyn VoiceIo) -> u32 {
        self.inner.borrow_mut().emval_new_cstring(voice, ptr, io)
    }

    pub fn emval_new_object(&self) -> u32 {
        self.inner.borrow_mut().emval_new_object()
    }

    pub fn emval_get_global(&self, voice: u32, name: u32, io: &mut dyn VoiceIo) -> u32 {
        self.inner.borrow_mut().emval_get_global(voice, name, io)
    }

    pub fn emval_set_property(&self, object: u32, key: u32, value: u32) {
        self.inner
            .borrow_mut()
            .emval_set_property(object, key, value);
    }

    pub fn emval_run_destructors(&self, handle: u32) {
        self.inner.borrow_mut().emval_run_destructors(handle);
    }

    pub fn emval_create_invoker(
        &self,
        voice: u32,
        arg_count: u32,
        arg_types: u32,
        kind: u32,
        io: &mut dyn VoiceIo,
    ) -> u32 {
        self.inner
            .borrow_mut()
            .emval_create_invoker(voice, arg_count, arg_types, kind, io)
    }

    /// `args` follows the parameter types, not the return type.
    pub fn invoke(
        &self,
        voice: u32,
        func: &BoundFn,
        args: &[HostArg],
        io: &mut dyn VoiceIo,
    ) -> Result<HostValue, String> {
        if func.arg_types.is_empty() {
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
        let returned = io.call(voice, func.invoker, &wired);
        for ptr in free_list {
            io.free(voice, ptr);
        }
        for (dtor, ptr) in object_dtors {
            io.call(voice, dtor, &[WireVal::I32(ptr)]);
        }
        if self.threw() {
            return Err("voice raised".to_string());
        }
        self.wire_to_host(voice, func.arg_types[0], returned, io)
    }

    /// Run one invoker created by `__emval_create_invoker`. The return is the f64 wire value.
    pub fn emval_invoke(
        &self,
        voice: u32,
        caller: u32,
        handle: u32,
        method_name: u32,
        args: u32,
        io: &mut dyn VoiceIo,
    ) -> f64 {
        let Some(invoker) = self.inner.borrow().invokers.get(caller as usize).cloned() else {
            return 0.0;
        };
        let mut values = Vec::with_capacity(invoker.types.len().saturating_sub(1));
        for (index, raw) in invoker.types.iter().skip(1).enumerate() {
            let addr = args.saturating_add((index * 8) as u32);
            values.push(self.read_em_arg(voice, *raw, addr, io));
        }
        let value = match invoker.kind {
            1 => self.call_method(voice, handle, method_name, &values, io),
            2 => self.construct(handle, &values, voice, io),
            3 => values.into_iter().next_back().unwrap_or(EmValue::Undef),
            _ => EmValue::Undef,
        };
        let ret = invoker.types.first().copied().unwrap_or(0);
        self.write_return(voice, ret, value)
    }

    fn to_wire(
        &self,
        voice: u32,
        raw_type: u32,
        arg: &HostArg,
        io: &mut dyn VoiceIo,
        free_list: &mut Vec<u32>,
        object_dtors: &mut Vec<(u32, u32)>,
    ) -> Result<WireVal, String> {
        let kind = self
            .inner
            .borrow()
            .type_kind(voice, raw_type)
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
                self.alloc_string(voice, text.as_bytes(), io, free_list)
            }
            (TypeKind::WString { char_size }, HostArg::Text(text)) => {
                self.alloc_wstring(voice, text, char_size, io, free_list)
            }
            (TypeKind::String, HostArg::Bytes(bytes)) => {
                self.alloc_string(voice, bytes, io, free_list)
            }
            (TypeKind::Object { data, .. }, HostArg::Object(fields)) => {
                let ptr = io.call(voice, data.ctor, &[]).as_i32();
                for field in &data.fields {
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
                object_dtors.push((data.dtor, ptr));
                Ok(WireVal::I32(ptr))
            }
            (kind, arg) => Err(format!("cannot convert {arg:?} to {kind:?}")),
        }
    }

    fn alloc_wstring(
        &self,
        voice: u32,
        text: &str,
        char_size: u32,
        io: &mut dyn VoiceIo,
        free_list: &mut Vec<u32>,
    ) -> Result<WireVal, String> {
        let encoded: Vec<u8> = if char_size == 4 {
            text.chars()
                .flat_map(|ch| (ch as u32).to_le_bytes())
                .collect()
        } else {
            text.encode_utf16()
                .flat_map(|unit| unit.to_le_bytes())
                .collect()
        };
        let units = encoded.len() / char_size.max(1) as usize;
        let base = io.malloc(voice, (4 + encoded.len() + char_size as usize) as u32);
        if base == 0 {
            return Err("malloc failed".to_string());
        }
        write_u32(io, voice, base, units as u32);
        io.write(voice, base + 4, &encoded);
        io.write(
            voice,
            base + 4 + encoded.len() as u32,
            &vec![0; char_size as usize],
        );
        free_list.push(base);
        Ok(WireVal::I32(base))
    }

    fn alloc_string(
        &self,
        voice: u32,
        bytes: &[u8],
        io: &mut dyn VoiceIo,
        free_list: &mut Vec<u32>,
    ) -> Result<WireVal, String> {
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

    fn wire_to_host(
        &self,
        voice: u32,
        raw_type: u32,
        value: WireVal,
        io: &mut dyn VoiceIo,
    ) -> Result<HostValue, String> {
        let kind = self
            .inner
            .borrow()
            .type_kind(voice, raw_type)
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
                Ok(HostValue::Bytes(self.read_view(voice, handle, index, io)))
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
                    let value = self.wire_to_host(voice, field.getter_type, raw, io)?;
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
            TypeKind::Bigint { .. } => Ok(HostValue::I32(value.as_i32() as i32)),
            TypeKind::Emval => self.host_from_handle(value.as_i32()),
        }
    }

    fn host_from_handle(&self, handle: u32) -> Result<HostValue, String> {
        let value = self.inner.borrow().slot_value(handle);
        let host = match &value {
            EmValue::Object(id) => {
                let fields = self
                    .inner
                    .borrow()
                    .objects
                    .get(*id)
                    .cloned()
                    .unwrap_or_default();
                let code = fields.get("errorCode").map(number_i32).unwrap_or(0);
                if code != 0 {
                    HostValue::I32(if code > 0 { -code } else { code })
                } else if let Some(EmValue::Bytes(bytes)) = fields.get("data") {
                    HostValue::Bytes(bytes.clone())
                } else if let Some(EmValue::Str(text)) = fields.get("data") {
                    HostValue::Bytes(text.as_bytes().to_vec())
                } else {
                    HostValue::I32(handle as i32)
                }
            }
            EmValue::Bytes(bytes) => HostValue::Bytes(bytes.clone()),
            EmValue::Str(text) => HostValue::Text(text.clone()),
            EmValue::Number(number) => HostValue::I32(*number as i32),
            EmValue::Bool(value) => HostValue::I32(i32::from(*value)),
            _ => HostValue::I32(handle as i32),
        };
        self.emval_decref(handle);
        Ok(host)
    }

    fn read_em_arg(&self, voice: u32, raw: u32, addr: u32, io: &mut dyn VoiceIo) -> EmValue {
        let Some(kind) = self.inner.borrow().type_kind(voice, raw) else {
            return EmValue::Undef;
        };
        match kind {
            TypeKind::Bool { .. } => {
                let mut byte = [0];
                io.read(voice, addr, &mut byte);
                EmValue::Bool(byte[0] != 0)
            }
            TypeKind::Int { size } => EmValue::Number(read_number(io, voice, addr, size, true)),
            TypeKind::Float { size } => EmValue::Number(read_number(io, voice, addr, size, false)),
            TypeKind::MemoryView { index } => {
                EmValue::Bytes(self.read_view(voice, addr, index, io))
            }
            TypeKind::String | TypeKind::WString { .. } => {
                let ptr = read_u32(io, voice, addr);
                if ptr == 0 {
                    return EmValue::Str(String::new());
                }
                let len = read_u32(io, voice, ptr) as usize;
                let mut bytes = vec![0; len];
                if len > 0 {
                    io.read(voice, ptr + 4, &mut bytes);
                }
                io.free(voice, ptr);
                EmValue::Str(String::from_utf8_lossy(&bytes).into_owned())
            }
            TypeKind::Emval => {
                let handle = read_u32(io, voice, addr);
                let value = self.inner.borrow().slot_value(handle);
                self.emval_decref(handle);
                value
            }
            TypeKind::Void | TypeKind::Bigint { .. } | TypeKind::Object { .. } => EmValue::Undef,
        }
    }

    fn read_view(&self, voice: u32, addr: u32, index: u32, io: &mut dyn VoiceIo) -> Vec<u8> {
        let count = read_u32(io, voice, addr) as usize;
        let data = read_u32(io, voice, addr + 4);
        let elem = match index {
            2..=3 => 2,
            4..=6 => 4,
            7..=9 => 8,
            _ => 1,
        };
        let nbytes = count.saturating_mul(elem).min(32 * 1024 * 1024);
        let mut bytes = vec![0; nbytes];
        if nbytes > 0 && data != 0 {
            io.read(voice, data, &mut bytes);
        }
        bytes
    }

    fn write_return(&self, voice: u32, raw: u32, value: EmValue) -> f64 {
        let Some(kind) = self.inner.borrow().type_kind(voice, raw) else {
            return 0.0;
        };
        match kind {
            TypeKind::Void => 0.0,
            TypeKind::Emval => self.inner.borrow_mut().alloc_emval(value) as f64,
            TypeKind::Bool { yes, no } => {
                let on = match &value {
                    EmValue::Bool(bit) => *bit,
                    EmValue::Number(number) => *number != 0.0,
                    _ => false,
                };
                if on { yes as f64 } else { no as f64 }
            }
            TypeKind::Int { .. } | TypeKind::Float { .. } => match value {
                EmValue::Number(number) => number,
                EmValue::Bool(bit) => f64::from(u8::from(bit)),
                _ => 0.0,
            },
            _ => 0.0,
        }
    }

    fn construct(
        &self,
        handle: u32,
        args: &[EmValue],
        voice: u32,
        io: &mut dyn VoiceIo,
    ) -> EmValue {
        let ctor = self.inner.borrow().slot_value(handle);
        let EmValue::Global(name) = ctor else {
            return EmValue::Undef;
        };
        if !name.ends_with("Array") {
            return EmValue::Undef;
        }
        match args {
            [EmValue::Number(len)] => {
                EmValue::Bytes(vec![0; (*len as usize).min(32 * 1024 * 1024)])
            }
            [EmValue::Bytes(bytes)] => EmValue::Bytes(bytes.clone()),
            [_, EmValue::Number(offset), EmValue::Number(len)] => {
                let start = *offset as u32;
                let count = (*len as usize).min(32 * 1024 * 1024);
                let mut bytes = vec![0; count];
                if count > 0 {
                    io.read(voice, start, &mut bytes);
                }
                EmValue::Bytes(bytes)
            }
            _ => EmValue::Bytes(Vec::new()),
        }
    }

    fn call_method(
        &self,
        voice: u32,
        handle: u32,
        method_name: u32,
        args: &[EmValue],
        io: &mut dyn VoiceIo,
    ) -> EmValue {
        let name = read_cstr(io, voice, method_name);
        let target = self.inner.borrow().slot_value(handle);
        match (target, name.as_str(), args) {
            (EmValue::Bytes(bytes), "subarray", [EmValue::Number(start), EmValue::Number(end)]) => {
                let start = (*start as usize).min(bytes.len());
                let end = (*end as usize).min(bytes.len()).max(start);
                EmValue::Bytes(bytes[start..end].to_vec())
            }
            (EmValue::Bytes(bytes), "slice", [EmValue::Number(start), EmValue::Number(end)]) => {
                let start = (*start as usize).min(bytes.len());
                let end = (*end as usize).min(bytes.len()).max(start);
                EmValue::Bytes(bytes[start..end].to_vec())
            }
            (EmValue::Object(id), method, _) => self
                .inner
                .borrow()
                .objects
                .get(id)
                .and_then(|fields| fields.get(method).cloned())
                .unwrap_or(EmValue::Undef),
            _ => EmValue::Undef,
        }
    }
}

fn read_number(io: &mut dyn VoiceIo, voice: u32, addr: u32, size: u32, integer: bool) -> f64 {
    let mut buf = [0; 8];
    let take = (size as usize).clamp(1, 8);
    io.read(voice, addr, &mut buf[..take]);
    if !integer {
        return if size > 4 {
            f64::from_le_bytes(buf)
        } else {
            f32::from_le_bytes(buf[..4].try_into().unwrap()).into()
        };
    }
    match size {
        1 => buf[0] as i8 as f64,
        2 => i16::from_le_bytes([buf[0], buf[1]]) as f64,
        8 => i64::from_le_bytes(buf) as f64,
        _ => i32::from_le_bytes(buf[..4].try_into().unwrap()) as f64,
    }
}

fn number_i32(value: &EmValue) -> i32 {
    match value {
        EmValue::Number(number) => *number as i32,
        EmValue::Bool(bit) => i32::from(*bit),
        _ => 0,
    }
}
