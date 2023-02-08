use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct CompetitionInfo {
    pub name: String,
    pub id: String,
}