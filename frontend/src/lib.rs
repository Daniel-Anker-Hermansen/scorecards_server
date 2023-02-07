use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
fn main() {
    let data = JsValue::from("Hi from wasm!");
    web_sys::console::log(&data.into())
}
