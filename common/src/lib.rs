use std::collections::HashMap;

use base64::{engine::{GeneralPurpose, GeneralPurposeConfig}, alphabet::URL_SAFE, Engine};
use serde::{Serialize, Deserialize, de::DeserializeOwned};

#[derive(Serialize, Deserialize)]
pub struct Competitors {
    pub competition: String,
    pub competitors: Vec<u64>,
    pub names: HashMap<u64, String>,
    pub delegates: Vec<u64>,
    pub stages: u64,
    pub stations: u64,
    pub event: String,
    pub round: u64,
    pub seperate_stages: bool,
}

#[derive(Serialize, Deserialize)]
pub struct RoundInfo {
    pub event: String,
    pub round_num: u8, 
    pub groups_exist: bool,
    pub entered: u64,
    pub competitors: u64,
}

impl RoundInfo {
    pub fn human_readable_event_name(&self) -> Option<&'static str> {
        Some(match self.event.as_str() {
            "333" => "3x3",
            "222" => "2x2",
            "444" => "4x4",
            "555" => "5x5",
            "666" => "6x6",
            "777" => "7x7",
            "333oh" => "3x3 One-Handed",
            "333bf" => "3x3 Blindfolded",
            "clock" => "Clock",
            "pyram" => "Pyraminx",
            "minx" => "Megaminx",
            "skewb" => "Skewb",
            "sq1" => "Square-1",
            "444bf" => "4x4 Blindfolded",
            "555bf" => "5x5 Blindfolded",
            "333mbf" => "3x3 Multi-Blind",
            _ => None?,
        })
    }

    pub fn print_name(&self) -> Option<String> {
        Some(format!("{}, Round {}", self.human_readable_event_name()?, self.round_num))
    }
}

#[derive(Serialize, Deserialize)]
pub struct PdfRequest {
    pub competition: String,
    pub stages: u64,
    pub stations: u64,
    pub groups: Vec<Vec<u64>>,
    pub wcif: bool,
    pub event: String,
    pub round: u64,
    pub seperate_stages: bool,
}

pub fn to_base_64<T>(data: T) -> String where T: Serialize {
    let bytes = postcard::to_allocvec(&data).unwrap();
    let engine = GeneralPurpose::new(&URL_SAFE, GeneralPurposeConfig::new());
    engine.encode(bytes) 
}

pub fn from_base_64<T>(base64: &str) -> T where T: DeserializeOwned { 
    let engine = GeneralPurpose::new(&URL_SAFE, GeneralPurposeConfig::new());
    let bytes = engine.decode(base64).unwrap();
    postcard::from_bytes(&bytes).unwrap()
}
