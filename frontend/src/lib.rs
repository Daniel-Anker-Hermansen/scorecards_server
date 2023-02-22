use std::panic::set_hook;

use common::{CompetitionInfo, RoundInfo};
use js_sys::{Error, Object, Array, Uint8Array, JsString};

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{console::log_1, window, Response, ReadableStreamDefaultReader, Event, Document, Element};

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

#[wasm_bindgen]
pub async fn main(session: &str) -> Result<(), Error> {
    set_hook(Box::new(|p| log_1(&p.to_string().into())));
    set_session(session);
    let url = format!("{}/competitions", session);
    let data = get_array(&url).await?;
    let data: Vec<CompetitionInfo> = postcard::from_bytes(&data)
        .map_err(|e| Error::new(&e.to_string()))?;
    for c in data {
        append_competition(&c)?;
    }
    Ok(())
}

fn append_competition(competition: &CompetitionInfo) -> Result<(), Error> {
    let document = document();
    let closure = Closure::once(competition_on_click);
    let div = document.create_element("div")?;
        div.add_event_listener_with_callback("click", closure.into_js_value().unchecked_ref())?;
    let text = document.create_element("text")?;
    let main = document.get_element_by_id("main")
        .unwrap();
    div.set_id(&competition.id);
    text.set_text_content(Some(&competition.name));
    div.append_child(&text)?;
    main.append_child(&div)?;
    Ok(())
}

fn competition_on_click(event: Event) {
    let inner = async move { 
        let target = event.current_target()
            .unwrap();
        let div: Element = target.unchecked_into();
        log_1(&div.id().into());
        let url = format!("{}/{}/rounds", session(), div.id());
        let bytes = get_array(&url)
            .await
            .unwrap();
        let round_info: Vec<RoundInfo> = postcard::from_bytes(&bytes)
            .unwrap();
        for round in round_info {
            log_1(&round.name.into());
        }
        let main = document()
            .get_element_by_id("main")
            .unwrap();
        while let Some(child) = main.last_child() {
            main.remove_child(&child)
                .unwrap();
        }
    };
    spawn_local(inner);
}

fn document() -> Document {
    window()
        .unwrap()
        .document()
        .unwrap()
}

static mut SESSION: u64 = 0;

fn set_session(session: &str) {
    unsafe {
        SESSION = session.parse().unwrap()
    }
}

fn session() -> u64 {
    unsafe { SESSION }
}
