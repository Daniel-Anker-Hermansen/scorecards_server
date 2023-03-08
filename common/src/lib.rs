use std::collections::HashMap;

use base64::{engine::{GeneralPurpose, GeneralPurposeConfig}, alphabet::URL_SAFE, Engine};
use serde::{Serialize, Deserialize, de::DeserializeOwned};

#[derive(Serialize, Deserialize)]
pub struct Competitors {
    pub competitors: Vec<u64>,
    pub names: HashMap<u64, String>,
    pub delegates: Vec<u64>,
    pub stages: u64,
    pub stations: u64,
    pub event: String,
    pub round: u64,
}

#[derive(Serialize, Deserialize)]
pub struct RoundInfo {
    pub event: String,
    pub round_num: u8, 
}

impl RoundInfo {
    pub fn print_name(&self) -> String {
        format!("{}, Round {}", self.event, self.round_num)
    }
}

#[derive(Serialize, Deserialize)]
pub struct PdfRequest {
    pub stages: u64,
    pub stations: u64,
    pub groups: Vec<Vec<u64>>,
    pub wcif: bool,
    pub event: String,
    pub round: u64,
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
