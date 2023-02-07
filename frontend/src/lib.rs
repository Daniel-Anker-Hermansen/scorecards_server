use std::panic::set_hook;

use js_sys::Error;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{console::log_1, window, Response, ReadableStreamDefaultReader};

#[wasm_bindgen(module = "/src/js.js")]
extern "C" {
    pub fn array(v: &JsValue) -> Vec<u8>;
}

async fn get_array(url: &str) -> Result<Vec<u8>, Error> {
    let future = JsFuture::from(window()
        .ok_or(Error::new("no window"))?
        .fetch_with_str(url));
    let stream = Response::from(future.await?)
        .body()
        .ok_or(Error::new("no body"))?;
    let future = JsFuture::from(ReadableStreamDefaultReader::new(&stream)?.read());
    Ok(array(&future.await?))
}

#[wasm_bindgen(start)]
pub async fn main() -> Result<(), Error> {
    set_hook(Box::new(|p| log_1(&p.to_string().into())));
        
    let data = get_array("test").await?;
    let data: Vec<String> = postcard::from_bytes(&data)
        .map_err(|e| Error::new(&e.to_string()))?;
    log_1(&format!("{data:?}").into());

    Ok(())
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