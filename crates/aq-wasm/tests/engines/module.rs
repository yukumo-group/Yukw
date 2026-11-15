//! Module shape and static-link inventory.

use super::common::built;

#[test]
fn module_exports_one_memory_and_no_imports() {
    let Some(built) = built() else {
        return;
    };
    assert_eq!(
        built.module.imports().count(),
        0,
        "module must not import anything"
    );
    let memories = built
        .module
        .exports()
        .filter(|export| matches!(export.ty(), wasmtime::ExternType::Memory(_)))
        .count();
    assert_eq!(memories, 1, "module must export exactly one memory");
}

#[test]
fn static_link_reports_five_engines() {
    let Some(built) = built() else {
        return;
    };
    let mut store = wasmtime::Store::new(&built.engine, ());
    let instance =
        wasmtime::Instance::new(&mut store, &built.module, &[]).expect("instantiate aq-wasm");
    let engines = instance
        .get_typed_func::<(), u32>(&mut store, "aq_static_engines")
        .expect("aq_static_engines");
    assert_eq!(engines.call(&mut store, ()).unwrap(), 5);
}
