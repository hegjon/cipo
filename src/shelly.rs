use crate::Device;

use attohttpc::{Error, Response};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Status {
    pub apower: f64,
    pub aenergy: Energy,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Energy {
    pub total: f64,
    pub by_minute: Vec<f32>,
    pub minute_ts: i64,
}

pub fn on(shelly: &Device) -> Result<Response, Error> {
    let url = format!(
        "http://{}/rpc/Switch.Set?id={}&on=true",
        shelly.host, shelly.switch
    );
    let resp = attohttpc::get(url).send()?;

    resp.error_for_status()
}

pub fn off(shelly: &Device) -> Result<Response, Error> {
    let url = format!(
        "http://{}/rpc/Switch.Set?id={}&on=false",
        shelly.host, shelly.switch
    );
    let resp = attohttpc::get(url).send()?;

    resp.error_for_status()
}

pub fn status(shelly: &Device) -> Result<Status, Error> {
    let url = format!(
        "http://{}/rpc/Switch.GetStatus?id={}",
        shelly.host, shelly.switch
    );

    let resp = attohttpc::get(url).send()?;
    let json: Status = resp.json()?;
    Ok(json)
}
