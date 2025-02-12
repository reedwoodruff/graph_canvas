use uuid::Uuid;
use wasm_bindgen::prelude::*;

pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}
