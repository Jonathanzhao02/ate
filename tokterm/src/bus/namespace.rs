use wasmer::imports;
use wasmer::ImportObject;
use wasmer::{Function, Store};

use super::thread::WasmBusThread;
use super::syscalls::raw;

/// Combines a state generating function with the import list for the WASM bus
pub(super) fn generate_import_object_wasm_bus(store: &Store, env: WasmBusThread) -> ImportObject {
    imports! {
        "wasm-bus" => {
            "drop" => Function::new_native_with_env(store, env.clone(), raw::wasm_bus_drop),
            "rand" => Function::new_native_with_env(store, env.clone(), raw::wasm_bus_rand),
            "recv" => Function::new_native_with_env(store, env.clone(), raw::wasm_bus_recv),
            "error" => Function::new_native_with_env(store, env.clone(), raw::wasm_bus_error),
            "reply" => Function::new_native_with_env(store, env.clone(), raw::wasm_bus_reply),
            "call" => Function::new_native_with_env(store, env.clone(), raw::wasm_bus_call),
            "recv_recursive" => Function::new_native_with_env(store, env.clone(), raw::wasm_bus_recv_recursive),
            "yield_and_wait" => Function::new_native_with_env(store, env.clone(), raw::wasm_bus_yield_and_wait),
            "thread_id" => Function::new_native_with_env(store, env.clone(), raw::wasm_bus_thread_id),
        }
    }
}
