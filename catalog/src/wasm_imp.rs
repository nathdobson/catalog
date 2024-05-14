use std::sync::Once;

use js_sys::Uint8Array;
use js_sys::WebAssembly::Module;
use wasm_bindgen::JsCast;

pub mod reexport {
    pub use wasm_bindgen;
}

pub fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let module = wasm_bindgen::module()
            .dyn_into::<Module>()
            .expect("Should be a module");
        for x in Module::custom_sections(&module, "registry_ctors").iter() {
            let array = Uint8Array::new(&x).to_vec();
            let array = std::str::from_utf8(&array).unwrap();
            for element in array.split(" ") {
                if !element.is_empty() {
                    js_sys::eval(&format!("{}()", element)).unwrap();
                }
            }
        }
    });
}
