use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct CompetitionInfo {
    pub name: String,
    pub id: String,
}

#[derive(Serialize, Deserialize)]
pub struct RoundInfo {
    pub name: String,
    pub previous_is_done: bool,
}
