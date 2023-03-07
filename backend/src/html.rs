use wca_oauth::Competition;

const VALIDATED: &str = include_str!("../../frontend/html_src/validated.html");

pub fn validated(competitions: Vec<Competition>) -> String {
    let inner = competitions.into_iter()
        .map(|competition| format!("<a class =  \"style_list\" href = \"/{id}\"><text>{name}</text></a>",
            id = competition.id(),
            name = competition.name()))
        .collect::<Vec<_>>()
        .join("\n");
    VALIDATED.replace("COMPETITIONS", &inner)
}
