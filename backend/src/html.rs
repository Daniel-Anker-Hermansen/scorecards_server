use common::{RoundInfo,Competitors, to_base_64};
use wca_oauth::Competition;

const VALIDATED: &str = include_str!("../../frontend/html_src/validated.html");
const ROUNDS: &str = include_str!("../../frontend/html_src/competition_rounds.html");
const GROUP: &str = include_str!("../../frontend/html_src/group.html");

pub fn validated(competitions: Vec<Competition>) -> String {
    let inner = competitions.into_iter()
        .map(|competition| format!("<a class =  \"style_list\" href = \"/{id}\"><text>{name}</text></a>",
            id = competition.id(),
            name = competition.name()))
        .collect::<Vec<_>>()
        .join("\n");
    VALIDATED.replace("COMPETITIONS", &inner)
}

pub fn rounds(rounds: Vec<RoundInfo>, competition_id: &str, stations: u64) -> String {
    let inner = rounds.into_iter()
        .flat_map(|round| 
            {
                let class_style = if round.groups_exist {
                    "style_list groups_exist"
                } else {
                    "style_list"
                };
                Some(format!("<a class =  \"{class_style}\" onclick = redirect(\"/{competition_id}/{event}/{round}\")><text>{name} ({entered}/{competitors})</text></a>",
            event = round.event,
            round = round.round_num,
            name = round.print_name()?,
	    entered = round.entered,
	    competitors = round.competitors))})
        .collect::<Vec<_>>()
        .join("\n");
    ROUNDS.replace("ROUNDS", &inner).replace("STATIONS", &stations.to_string())
}

pub fn group(competitors: Competitors, groups_exist: bool) -> String {
    let intermediate = if groups_exist { GROUP.replace("ERROR", "Warning: This round already has groups patched. Make sure that you chose the correct group.")} else { GROUP.replace("ERROR", "")};
    intermediate.replace("DATA", &to_base_64(&competitors))
}
