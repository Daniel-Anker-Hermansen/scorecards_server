use std::{panic::set_hook, sync::{Arc, Mutex}, collections::{HashSet, HashMap}};

use base64::{engine::{GeneralPurpose, GeneralPurposeConfig}, alphabet::URL_SAFE, Engine};
use common::{CompetitionInfo, RoundInfo, Competitors, PdfRequest};
use js_sys::{Error, Object, Array, Uint8Array, JsString};

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{console::log_1, window, Response, ReadableStreamDefaultReader, Event, Document, Element, HtmlInputElement, HtmlTableElement, HtmlTableRowElement, HtmlElement};

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
    div.set_class_name("style_list");
    text.set_text_content(Some(&competition.name));
    div.append_child(&text)?;
    main.append_child(&div)?;
    Ok(())
}

fn append_round(round: &RoundInfo, competition_name: &str) -> Result<(), Error> {
    let document = document();
    let closure = Closure::once(round_on_click);
    let div = document.create_element("div")?;
    div.add_event_listener_with_callback("click", closure.into_js_value().unchecked_ref())?;
    let text = document.create_element("text")?;
    let main = document.get_element_by_id("main")
        .unwrap();
    div.set_id(&format!("{}/{}", competition_name, round.name));
    div.set_class_name("style_list");
    text.set_text_content(Some(&round.name));
    div.append_child(&text)?;
    main.append_child(&div)?;
    Ok(())
}

fn append_input(text: &str, id: &str, default: &str) -> Result<(), Error> {
    let document = document();
    let input: HtmlInputElement = document.create_element("input")?.dyn_into().unwrap();
    input.set_id(id);
    input.set_default_value(default);
    let div = document.create_element("div")?;
    let txt = document.create_element("text")?;
    txt.set_text_content(Some(text));
    div.append_child(&txt)?;
    div.append_child(&input)?;
    let main = document.get_element_by_id("main")
        .unwrap();
    main.append_child(&div)?;
    Ok(())
}

fn competition_on_click(event: Event) {
    let inner = async move { 
        let target = event.current_target()
            .unwrap();
        let div: Element = target.unchecked_into();
        let url = format!("{}/{}/rounds", session(), div.id());
        let bytes = get_array(&url)
            .await
            .unwrap();
        let round_info: Vec<RoundInfo> = postcard::from_bytes(&bytes)
            .unwrap();
        remove_all_children("main");
        append_input("Number of stages: ", "stages", "1").unwrap();
        append_input("Number of stations per stage: ", "stations", "10").unwrap();
        for round in round_info {
            append_round(&round, &div.id())
                .unwrap();
        }
    };
    spawn_local(inner);
}

fn round_on_click(event: Event) {
    let inner = async move {
        let target = event.current_target()
            .unwrap();
        let div: Element = target.unchecked_into();
        let stages: HtmlInputElement = document().get_element_by_id("stages").unwrap().unchecked_into();
        let stages: u64 = stages.value().parse().unwrap();
        let stations: HtmlInputElement = document().get_element_by_id("stations").unwrap().unchecked_into();
        let stations: u64 = stations.value().parse().unwrap();
        let url = format!("{}/{}/competitors", session(), div.id());
        let bytes = get_array(&url)
            .await
            .unwrap();
        let result: Competitors = postcard::from_bytes(&bytes)
            .unwrap();
        let groups = make_groups(result.competitors, result.delegates, stages, stations);
        let id = div.id();
        let mut iter = id.split("-")
            .flat_map(|t| t.split("/"));
        let event = iter.nth(1).unwrap().to_owned();
        let round = iter.next().unwrap()[1..].parse().unwrap();
        let round_config = RoundConfig { stages, stations, groups, names: result.names, event, round };
        unsafe {
            ROUND_CONFIG = Some(Arc::new(Mutex::new(round_config)));
        }
        redraw_round_config().unwrap();
    };
    spawn_local(inner);
}

fn make_groups(competitors: Vec<u64>, delegates: Vec<u64>, stages: u64, stations: u64) -> Vec<Vec<u64>> {
    let capacity = stages * stations;
    let no_of_groups = (competitors.len() as u64 - 1 + capacity) / capacity;
    let map: HashSet<_> = delegates.into_iter().collect();
    let mut competing_delegates: Vec<_> = competitors.iter().filter(|id| map.contains(&id)).cloned().collect();  
    let mut competing_non_delegates: Vec<_> = competitors.iter().filter(|id| !map.contains(&id)).cloned().collect(); 
    let mut remaining_delegates = competing_delegates.len() as u64;
    let delegate_distribution: Vec<_> = (0..no_of_groups).map(|group| {
            let per_group = remaining_delegates / (no_of_groups - group);
            let rem = remaining_delegates % (no_of_groups - group);
            let res = per_group + if rem > 0 { 1 } else { 0 };
            remaining_delegates -= res;
            res
        }).collect();
    let mut remaining_competitors = competitors.len() as u64;
    let competitor_distribution: Vec<_> = (0..no_of_groups).map(|group| {
            let per_group = remaining_competitors / (no_of_groups - group);
            let rem = remaining_competitors % (no_of_groups - group);
            let res = per_group + if rem > 0 { 1 } else { 0 };
            remaining_competitors -= res;
            res
        }).collect();
    (0..no_of_groups).map(|idx| {
            let no_of_delegates = delegate_distribution[idx as usize];
            let no_of_non_delegates = competitor_distribution[idx as usize] - no_of_delegates;
            competing_non_delegates.split_off(competing_non_delegates.len() - no_of_non_delegates as usize)
                .into_iter()
                .chain(competing_delegates.split_off(competing_delegates.len() - no_of_delegates as usize))
                .collect()
        }).collect()
}

#[derive(Clone)]
struct RoundConfig {
    stages: u64,
    stations: u64,
    groups: Vec<Vec<u64>>,
    names: HashMap<u64, String>,
    event: String,
    round: u64,
}

fn move_competitor(event: Event) {
    let t = async move {
        let target: Element = event.current_target()
            .unwrap()
            .unchecked_into();
        let id = target.id();
        let numbers: Vec<_> = id.split("/").map(|z| z.parse().unwrap()).collect();
        let group = numbers[0] as _;
        let number = numbers[1] as _;
        let translation = numbers[2];
        get_round_config().lock()
            .unwrap()
            .move_competitor(group, number, translation);
        redraw_round_config().unwrap();
    };
    spawn_local(t);

}

fn redraw_round_config() -> Result<(), Error> {
    let table: HtmlTableElement = document().create_element("table").unwrap().unchecked_into();
    let rc = get_round_config();
    let lock = rc.lock().unwrap();
    let groups = &lock.groups;
    let names = &lock.names;
    let no_of_rows = groups.iter().map(|group| group.len()).max().unwrap_or(0);
    for _ in 0..no_of_rows {
        table.insert_row()?;
    }
    let no_of_groups = groups.len();
    let rows = table.rows();
    for number in 0..no_of_rows {
        let item: HtmlTableRowElement = rows.item(number as u32).unwrap().unchecked_into();
        for group in 0..no_of_groups {
            let l_cell = item.insert_cell().unwrap();
            let cell = item.insert_cell().unwrap();
            let r_cell = item.insert_cell().unwrap();
            match groups[group].get(number) {
                Some(id) => {
                    let left_button = document().create_element("button")?;
                    left_button.set_text_content(Some("<"));
                    left_button.set_id(&format!("{}/{}/{}", group, number, -1));
                    let closure = Closure::once(move_competitor);
                    left_button.add_event_listener_with_callback("click", closure.into_js_value().unchecked_ref())?;
                    let right_button = document().create_element("button")?;
                    right_button.set_text_content(Some(">"));
                    right_button.set_id(&format!("{}/{}/{}", group, number, 1));
                    let closure = Closure::once(move_competitor);
                    right_button.add_event_listener_with_callback("click", closure.into_js_value().unchecked_ref())?;
                    let text = document().create_element("text")?;
                    text.set_text_content(Some(&format!("{} ({})", id, names[id])));
                    if group != 0 {
                        l_cell.append_child(&left_button)?;
                    }
                    cell.append_child(&text)?;
                    if group != no_of_groups - 1 {
                        r_cell.append_child(&right_button)?;
                    }

                },
                None => (),
            } 
        }
    }
    remove_all_children("main");
    let main = document().get_element_by_id("main")
        .unwrap();
    main.append_child(&table)?;
    let submit = document().create_element("button")?;
    submit.set_text_content(Some("Submit!"));
    let closure = Closure::once(submit_on_click);
    submit.add_event_listener_with_callback("click", &closure.into_js_value().unchecked_ref())?;
    let document = document();
    let input: HtmlInputElement = document.create_element("input")?.dyn_into().unwrap();
    input.set_id("checkbox");
    input.set_type("checkbox");
    let div = document.create_element("div")?;
    let txt = document.create_element("text")?;
    txt.set_text_content(Some("Do you want to patch to wcif?"));
    div.append_child(&txt)?;
    div.append_child(&input)?;
    main.append_child(&div)?;
    main.append_child(&submit)?;
    Ok(())
}

static mut ROUND_CONFIG: Option<Arc<Mutex<RoundConfig>>> = None;

fn get_round_config() -> Arc<Mutex<RoundConfig>> { 
    unsafe {
        ROUND_CONFIG.clone().unwrap()
    }
}

fn submit_on_click() {
    let t = async {
        let checkbox: HtmlInputElement = document().get_element_by_id("checkbox")
            .unwrap()
            .unchecked_into();
        let rc = get_round_config();
        let round_config = rc.lock().unwrap();
        let pdf_request = PdfRequest {
            stages: round_config.stages,
            stations: round_config.stations,
            groups: round_config.submit().unwrap().clone(),
            wcif: checkbox.checked(),
            event: round_config.event.clone(),
            round: round_config.round,
            session: session(),
        };
        let data = postcard::to_allocvec(&pdf_request).unwrap();
        let base64 = GeneralPurpose::new(&URL_SAFE, GeneralPurposeConfig::new()).encode(data);
        let url = format!("submit?data={base64}");

        let element: HtmlElement = document().create_element("a").unwrap().unchecked_into();
        element.set_attribute("href", &url).unwrap();
        document().get_element_by_id("main")
            .unwrap()
            .append_child(&element).unwrap();
        element.click();
    };
    spawn_local(t);
}



impl RoundConfig {
    fn move_competitor(&mut self, group: usize, number: usize, translation: isize) {
        let id = self.groups[group].remove(number);
        self.groups[(group as isize + translation) as usize].push(id);
    }

    fn submit(&self) -> Result<&Vec<Vec<u64>>, String> {
        self.groups
            .iter()
            .position(|group| group.len() as u64 > self.stages * self.stations)
            .map(|group| Err(format!("Group {group} has too many competitors")))
            .unwrap_or(Ok(&self.groups))
    }
}

fn remove_all_children(id: &str) {
    let elem = document()
        .get_element_by_id(id)
        .unwrap();
    while let Some(child) = elem.last_child() {
        elem.remove_child(&child)
            .unwrap();
    }
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
