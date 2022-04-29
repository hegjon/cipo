use serde::Deserialize;
use std::fs;
use std::io;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(rename = "monero-rpc")]
    pub monero_rpc: HostPort,
    pub device: Vec<Device>,
    pub price: Price,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Price {
    #[serde(rename = "xmr-per-kwh")]
    pub xmr_per_kwh: f64,
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
    pub monero: String,
}

impl Config {
    pub fn from_file(config_file: &String) -> io::Result<Self> {
        let content = fs::read_to_string(config_file)?;
        let config: Config = toml::from_str(&content)?;

        Ok(config)
    }
}
