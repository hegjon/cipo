use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(rename = "monero-rpc")]
    pub monero_rpc: HostPort,
    pub device: Vec<Device>,
    pub price: Price,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Price {
    #[serde(rename = "xmr-per-watt-hour")]
    pub xmr_per_watt_hour: f64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct HostPort {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Device {
    pub location: String,
    pub host: String,
    pub switch: u16,
    pub monero: String
}

pub fn load_from_file() -> Config {
    let content = fs::read_to_string("config.toml").unwrap();
    let config: Config = toml::from_str(&content).unwrap();

    config
}