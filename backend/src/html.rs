use common::{to_base_64, Competitors, RoundInfo};
use wca_oauth::Competition;

const VALIDATED: &str = include_str!("../../frontend/html_src/validated.html");
const ROUNDS: &str = include_str!("../../frontend/html_src/competition_rounds.html");
const GROUP: &str = include_str!("../../frontend/html_src/group.html");

pub fn root(competitions: Vec<Competition>) -> String {
	let inner = competitions
		.into_iter()
		.map(|competition| {
			format!(
				"<a class =  \"style_list\" href = \"{}\"><text>{}</text></a>",
				competition.id(),
				competition.name()
			)
		})
		.collect::<Vec<_>>()
		.join("\n");
	VALIDATED.replace("COMPETITIONS", &inner)
}

pub fn rounds(rounds: Vec<RoundInfo>, competition_id: &str, stations: u64) -> String {
	let inner = rounds
		.into_iter()
		.flat_map(|round| {
			let class_style = if round.groups_exist {
				"style_list groups_exist"
			} else {
				"style_list"
			};
			let data = format!(
				"<a class =  \"{}\" onclick = redirect(\"{}/{}/{}\")><text>{}</text></a>",
				class_style,
				competition_id,
				round.event,
				round.round_num,
				round.print_name()?
			);
			Some(data)
		})
		.collect::<Vec<_>>()
		.join("\n");
	ROUNDS
		.replace("ROUNDS", &inner)
		.replace("STATIONS", &stations.to_string())
}

pub fn group(competitors: Competitors) -> String {
	GROUP.replace("DATA", &to_base_64(&competitors))
}
