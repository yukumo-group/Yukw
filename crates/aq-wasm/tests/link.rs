//! Two voice memories answer the same import with their own bytes.

use aq_wasm::{VoiceModule, link};

#[test]
fn same_import_reads_each_voice_memory() {
    let runtime = wat::parse_str(
        r#"
        (module
          (type (func (param i32 i32 i32 i32)))
          (import "voice" "voice_read" (func $read (type 0)))
          (func (export "host_answer") (param i32) (result i32)
            local.get 0
            i32.const 0
            i32.const 16
            i32.const 4
            call $read
            i32.const 16
            i32.load)
          (memory (export "memory") 1))
        "#,
    )
    .unwrap();
    let voice = |byte: &str| {
        wat::parse_str(format!(
            r#"
            (module
              (func (import "env" "answer") (result i32))
              (func (export "get") (result i32)
                call 0)
              (memory 1)
              (data (i32.const 0) "{byte}"))
            "#
        ))
        .unwrap()
    };
    let first = voice("\\01\\02\\03\\04");
    let second = voice("\\05\\06\\07\\08");
    let linked = link(
        &runtime,
        &[
            VoiceModule {
                wasm: &first,
                version: 1,
                preset: 0,
            },
            VoiceModule {
                wasm: &second,
                version: 2,
                preset: -1,
            },
        ],
    )
    .unwrap();
    assert!(aq_wasm::import_names(&linked).unwrap().is_empty());

    let mut config = wasmtime::Config::new();
    config.wasm_multi_memory(true);
    let engine = wasmtime::Engine::new(&config).unwrap();
    let module = wasmtime::Module::new(&engine, &linked).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
    let get0 = instance
        .get_typed_func::<(), i32>(&mut store, "v0_get")
        .unwrap();
    let get1 = instance
        .get_typed_func::<(), i32>(&mut store, "v1_get")
        .unwrap();
    assert_eq!(get0.call(&mut store, ()).unwrap(), 0x0403_0201);
    assert_eq!(get1.call(&mut store, ()).unwrap(), 0x0807_0605);
}
