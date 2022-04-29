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

pub struct Shelly {
    host: String,
    switch: u16,
}

impl Shelly {
    pub fn new(device: Device) -> Self {
        Shelly {
            host: device.host,
            switch: device.switch,
        }
    }

    pub fn on(&self) -> Result<Response, Error> {
        let url = format!(
            "http://{}/rpc/Switch.Set?id={}&on=true",
            self.host, self.switch
        );
        let resp = attohttpc::get(url).send()?;

        resp.error_for_status()
    }

    pub fn off(&self) -> Result<Response, Error> {
        let url = format!(
            "http://{}/rpc/Switch.Set?id={}&on=false",
            self.host, self.switch
        );
        let resp = attohttpc::get(url).send()?;

        resp.error_for_status()
    }

    pub fn status(&self) -> Result<Status, Error> {
        let url = format!(
            "http://{}/rpc/Switch.GetStatus?id={}",
            self.host, self.switch
        );

        let resp = attohttpc::get(url).send()?;
        let json: Status = resp.json()?;
        Ok(json)
    }
}
