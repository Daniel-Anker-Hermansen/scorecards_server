use std::panic::set_hook;

use js_sys::{Error, Object, Array, Uint8Array, JsString};

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{console::log_1, window, Response, ReadableStreamDefaultReader};

async fn get_array(url: &str) -> Result<Vec<u8>, Error> {
    let future = JsFuture::from(window()
        .ok_or(Error::new("no window"))?
        .fetch_with_str(url));
    let stream = Response::from(future.await?)
        .body()
        .ok_or(Error::new("no body"))?;
    let future = JsFuture::from(ReadableStreamDefaultReader::new(&stream)?.read());
    let object: Object = future.await?.into();
    let array = Object::entries(&object);
    let inner: Array = array.find(&mut |val, _, _| {
        let array: Array = val.into();
        let key: JsString = array.at(0).into();
        let string = key.to_string();
        string == "value"
    }).into();
    let fina: Uint8Array = inner.at(1).into();
    Ok(fina.to_vec())
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
