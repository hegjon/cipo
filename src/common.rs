use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Payment {
    pub address: String,
    pub txid: String,
    pub watt_hours: f64,
}