use uuid::Uuid;
use wasm_bindgen::prelude::*;

pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}
