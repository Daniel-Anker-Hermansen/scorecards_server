use std::collections::HashMap;

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

#[derive(Serialize, Deserialize)]
pub struct Competitors {
    pub competitors: Vec<u64>,
    pub names: HashMap<u64, String>,
    pub delegates: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct PdfRequest {
    pub stages: u64,
    pub stations: u64,
    pub groups: Vec<Vec<u64>>,
    pub wcif: bool,
    pub event: String,
    pub round: u64,
    pub session: u64,
}
