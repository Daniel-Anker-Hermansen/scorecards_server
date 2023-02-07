use std::panic::set_hook;

use wasm_bindgen::prelude::*;
use web_sys::{console::log_1, window, Response, ReadableStreamDefaultReader};

#[wasm_bindgen(module = "/src/js.js")]
extern "C" {
    pub fn array(v: &JsValue) -> Vec<u8>;
}

#[wasm_bindgen(start)]
pub fn main() {
    set_hook(Box::new(|p| log_1(&p.to_string().into())));

    let str = "Hi mom from wasm!";
    web_sys::console::log_1(&str.into());
    let closure = Closure::<dyn FnMut(JsValue)>::new(|v| {
        let closure = Closure::<dyn FnMut(JsValue)>::new(|v| {
            let array = array(&v);
            let data: Vec<String> = postcard::from_bytes(&array).unwrap();
            log_1(&format!("{data:?}").into());
        });

        let response = Response::from(v);
        let stream = response.body().unwrap();
        let readable = ReadableStreamDefaultReader::new(&stream).unwrap();
        readable.read()
            .then(&closure)
            .constructor();
        closure.forget();
    });

    window().unwrap().fetch_with_str("test")
        .then(&closure)
        .constructor();
    closure.forget();

    //append_paragraph("hi");
}

#[allow(unused)]
fn append_paragraph(data: &str) {
    let document = web_sys::window().unwrap()
        .document().unwrap();
    let element = document.create_element("p").unwrap();
    element.set_text_content(Some(data));
    document.get_element_by_id("main").unwrap()
        .append_child(&element).unwrap();
}
