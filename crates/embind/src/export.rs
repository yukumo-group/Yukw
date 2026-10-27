//! Wasm32 `host_*` exports.
//! `$session` returns the owner of an [`Embind`](crate::Embind).
//! `$throw` records a trap. `$io` is the caller's unit struct that implements [`VoiceIo`](crate::VoiceIo).

#[macro_export]
macro_rules! export_host {
    ($session:path, $throw:path, $io:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn host____cxa_throw(_voice: i32, _ptr: i32, _ty: i32, _dtor: i32) {
            $throw();
            core::arch::wasm32::unreachable()
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___abort_js(_voice: i32) {
            $throw();
            core::arch::wasm32::unreachable()
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host__emscripten_resize_heap(voice: i32, requested: i32) -> i32 {
            $crate::host::resize_heap(&mut $io, voice as u32, requested)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host__emscripten_random(_voice: i32) -> f32 {
            $crate::host::random()
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host__emscripten_date_now(_voice: i32) -> f64 {
            $session().embind.date_now()
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host__random_get(voice: i32, buf: i32, len: i32) -> i32 {
            $crate::host::random_get(&mut $io, voice as u32, buf, len)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___gmtime_js(voice: i32, time: i64, tm: i32) -> i32 {
            $crate::host::gmtime(&mut $io, voice as u32, time, tm)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___tzset_js(voice: i32, a: i32, b: i32, c: i32, d: i32) {
            $crate::host::tzset(&mut $io, voice as u32, [a, b, c, d]);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___emval_decref(_voice: i32, handle: i32) {
            $session().embind.emval_decref(handle as u32);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___emval_incref(_voice: i32, handle: i32) {
            $session().embind.emval_incref(handle as u32);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___emval_new_cstring(voice: i32, ptr: i32) -> i32 {
            $session()
                .embind
                .emval_new_cstring(voice as u32, ptr as u32, &mut $io) as i32
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___emval_new_object(_voice: i32) -> i32 {
            $session().embind.emval_new_object() as i32
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___emval_get_global(voice: i32, name: i32) -> i32 {
            $session()
                .embind
                .emval_get_global(voice as u32, name as u32, &mut $io) as i32
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___emval_set_property(
            _voice: i32,
            object: i32,
            key: i32,
            value: i32,
        ) {
            $session()
                .embind
                .emval_set_property(object as u32, key as u32, value as u32);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___emval_run_destructors(_voice: i32, handle: i32) {
            $session().embind.emval_run_destructors(handle as u32);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___emval_create_invoker(
            voice: i32,
            arg_count: i32,
            arg_types: i32,
            kind: i32,
        ) -> i32 {
            $session().embind.emval_create_invoker(
                voice as u32,
                arg_count as u32,
                arg_types as u32,
                kind as u32,
                &mut $io,
            ) as i32
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___emval_invoke(
            voice: i32,
            caller: i32,
            handle: i32,
            method: i32,
            _destructors: i32,
            args: i32,
        ) -> f64 {
            $session().embind.emval_invoke(
                voice as u32,
                caller as u32,
                handle as u32,
                method as u32,
                args as u32,
                &mut $io,
            )
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_void(voice: i32, raw: i32, name: i32) {
            $session()
                .embind
                .register_void(voice as u32, raw as u32, name as u32, &mut $io);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_bool(
            voice: i32,
            raw: i32,
            name: i32,
            yes: i32,
            no: i32,
        ) {
            $session().embind.register_bool(
                voice as u32,
                raw as u32,
                name as u32,
                yes as u32,
                no as u32,
                &mut $io,
            );
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_integer(
            voice: i32,
            raw: i32,
            name: i32,
            size: i32,
            _min: i32,
            _max: i32,
        ) {
            $session().embind.register_integer(
                voice as u32,
                raw as u32,
                name as u32,
                size as u32,
                &mut $io,
            );
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_bigint(
            voice: i32,
            raw: i32,
            name: i32,
            size: i32,
            _min: i64,
            _max: i64,
        ) {
            $session().embind.register_bigint(
                voice as u32,
                raw as u32,
                name as u32,
                size as u32,
                &mut $io,
            );
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_float(voice: i32, raw: i32, name: i32, size: i32) {
            $session().embind.register_float(
                voice as u32,
                raw as u32,
                name as u32,
                size as u32,
                &mut $io,
            );
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_std_string(voice: i32, raw: i32, name: i32) {
            $session()
                .embind
                .register_std_string(voice as u32, raw as u32, name as u32, &mut $io);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_std_wstring(
            voice: i32,
            raw: i32,
            char_size: i32,
            name: i32,
        ) {
            $session().embind.register_std_wstring(
                voice as u32,
                raw as u32,
                char_size as u32,
                name as u32,
                &mut $io,
            );
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_emval(voice: i32, raw: i32) {
            $session().embind.register_emval(voice as u32, raw as u32);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_memory_view(
            voice: i32,
            raw: i32,
            data_type: i32,
            name: i32,
        ) {
            $session().embind.register_memory_view(
                voice as u32,
                raw as u32,
                data_type as u32,
                name as u32,
                &mut $io,
            );
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_value_object(
            voice: i32,
            raw: i32,
            name: i32,
            _ctor_sig: i32,
            ctor: i32,
            _dtor_sig: i32,
            dtor: i32,
        ) {
            $session().embind.register_value_object(
                voice as u32,
                raw as u32,
                name as u32,
                ctor as u32,
                dtor as u32,
            );
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_value_object_field(
            voice: i32,
            struct_type: i32,
            field_name: i32,
            getter_type: i32,
            _getter_sig: i32,
            getter: i32,
            getter_context: i32,
            setter_type: i32,
            _setter_sig: i32,
            setter: i32,
            setter_context: i32,
        ) {
            $session().embind.register_value_object_field(
                $crate::RegisterValueObjectField {
                    voice: voice as u32,
                    struct_type: struct_type as u32,
                    field_name: field_name as u32,
                    getter_type: getter_type as u32,
                    getter: getter as u32,
                    getter_context: getter_context as u32,
                    setter_type: setter_type as u32,
                    setter: setter as u32,
                    setter_context: setter_context as u32,
                },
                &mut $io,
            );
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_finalize_value_object(voice: i32, struct_type: i32) {
            $session()
                .embind
                .finalize_value_object(voice as u32, struct_type as u32, &mut $io);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn host___embind_register_function(
            voice: i32,
            name: i32,
            arg_count: i32,
            arg_types: i32,
            signature: i32,
            invoker: i32,
            func: i32,
            is_async: i32,
            is_nonnull_return: i32,
        ) {
            $session().embind.register_function(
                $crate::RegisterFunction {
                    voice: voice as u32,
                    name: name as u32,
                    arg_count: arg_count as u32,
                    arg_types: arg_types as u32,
                    signature: signature as u32,
                    invoker: invoker as u32,
                    func: func as u32,
                    is_async: is_async as u32,
                    is_nonnull_return: is_nonnull_return as u32,
                },
                &mut $io,
            );
        }
    };
}
