use std::{
	collections::{HashMap, HashSet},
	panic::set_hook,
	sync::{Mutex, OnceLock},
};

use common::{from_base_64, to_base_64, Competitors, PdfRequest};
use js_sys::Error;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{
	console::log_1, window, Document, Element, Event, HtmlElement, HtmlInputElement,
	HtmlTableElement, HtmlTableRowElement,
};

#[wasm_bindgen]
pub fn start(base_64: &str) {
	set_hook(Box::new(|p| log_1(&p.to_string().into())));
	let competitor_info: Competitors = from_base_64(base_64);
	let groups = make_groups(
		competitor_info.competitors,
		competitor_info.delegates,
		competitor_info.stages,
		competitor_info.stations,
	);
	let round_config = RoundConfig {
		competition: competitor_info.competition,
		stages: competitor_info.stages,
		stations: competitor_info.stations,
		groups,
		names: competitor_info.names,
		event: competitor_info.event,
		round: competitor_info.round,
	};
	ROUND_CONFIG.get_or_init(|| Mutex::new(round_config));
	redraw_round_config().unwrap();
}

fn make_groups(
	competitors: Vec<u64>,
	delegates: Vec<u64>,
	stages: u64,
	stations: u64,
) -> Vec<Vec<u64>> {
	let capacity = stages * stations;
	let no_of_groups = (competitors.len() as u64 - 1 + capacity) / capacity;
	let map: HashSet<_> = delegates.into_iter().collect();
	let mut competing_delegates: Vec<_> = competitors
		.iter()
		.filter(|id| map.contains(&id))
		.cloned()
		.collect();
	let mut competing_non_delegates: Vec<_> = competitors
		.iter()
		.filter(|id| !map.contains(&id))
		.cloned()
		.collect();
	let delegate_distribution = distribution(competing_delegates.len() as u64, no_of_groups);
	let competitor_distribution = distribution(competitors.len() as u64, no_of_groups);
	(0..no_of_groups)
		.map(|idx| {
			let no_of_delegates = delegate_distribution[idx as usize];
			let no_of_non_delegates = competitor_distribution[idx as usize] - no_of_delegates;
			competing_non_delegates
				.split_off(competing_non_delegates.len() - no_of_non_delegates as usize)
				.into_iter()
				.chain(
					competing_delegates
						.split_off(competing_delegates.len() - no_of_delegates as usize),
				)
				.collect()
		})
		.collect()
}

fn distribution(mut remaining: u64, no_of_groups: u64) -> Vec<u64> {
	(0..no_of_groups)
		.map(|group| {
			let per_group = remaining / (no_of_groups - group);
			remaining -= per_group;
			per_group
		})
		.collect()
}

#[derive(Clone)]
struct RoundConfig {
	competition: String,
	stages: u64,
	stations: u64,
	groups: Vec<Vec<u64>>,
	names: HashMap<u64, String>,
	event: String,
	round: u64,
}

fn move_competitor(event: Event) {
	let t = async move {
		let target: Element = event.current_target().unwrap().unchecked_into();
		let id = target.id();
		let numbers: Vec<_> = id.split("/").map(|z| z.parse().unwrap()).collect();
		let group = numbers[0] as _;
		let number = numbers[1] as _;
		let translation = numbers[2];
		get_round_config()
			.lock()
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
					left_button.add_event_listener_with_callback(
						"click",
						closure.into_js_value().unchecked_ref(),
					)?;
					let right_button = document().create_element("button")?;
					right_button.set_text_content(Some(">"));
					right_button.set_id(&format!("{}/{}/{}", group, number, 1));
					let closure = Closure::once(move_competitor);
					right_button.add_event_listener_with_callback(
						"click",
						closure.into_js_value().unchecked_ref(),
					)?;
					let text = document().create_element("text")?;
					text.set_text_content(Some(&format!("{} ({})", id, names[id])));
					if group != 0 {
						l_cell.append_child(&left_button)?;
					}
					cell.append_child(&text)?;
					if group != no_of_groups - 1 {
						r_cell.append_child(&right_button)?;
					}
				}
				None => (),
			}
		}
	}
	remove_all_children("main");
	let main = document().get_element_by_id("main").unwrap();
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

static ROUND_CONFIG: OnceLock<Mutex<RoundConfig>> = OnceLock::new();

fn get_round_config() -> &'static Mutex<RoundConfig> {
	ROUND_CONFIG.get().unwrap()
}

fn submit_on_click() {
	let t = async {
		let checkbox: HtmlInputElement = document()
			.get_element_by_id("checkbox")
			.unwrap()
			.unchecked_into();
		let rc = get_round_config();
		let round_config = rc.lock().unwrap();
		let pdf_request = PdfRequest {
			competition: round_config.competition.clone(),
			stages: round_config.stages,
			stations: round_config.stations,
			groups: round_config.submit().unwrap().clone(),
			wcif: checkbox.checked(),
			event: round_config.event.clone(),
			round: round_config.round,
		};
		let base64 = to_base_64(&pdf_request);
		let url = format!("/pdf?data={base64}");

		let element: HtmlElement = document().create_element("a").unwrap().unchecked_into();
		element.set_attribute("href", &url).unwrap();
		document()
			.get_element_by_id("main")
			.unwrap()
			.append_child(&element)
			.unwrap();
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
	let elem = document().get_element_by_id(id).unwrap();
	while let Some(child) = elem.last_child() {
		elem.remove_child(&child).unwrap();
	}
}

fn document() -> Document {
	window().unwrap().document().unwrap()
}
