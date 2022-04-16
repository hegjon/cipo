use crate::Device;

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

pub fn on(shelly: &Device) -> Result<(), attohttpc::Error> {
    let url = format!(
        "http://{}/rpc/Switch.Set?id={}&on=true",
        shelly.host, shelly.switch
    );
    attohttpc::get(url).send();

    Ok(())
}

pub fn off(shelly: &Device) -> Result<(), attohttpc::Error> {
    let url = format!(
        "http://{}/rpc/Switch.Set?id={}&on=false",
        shelly.host, shelly.switch
    );
    attohttpc::get(url).send();

    Ok(())
}

pub fn status(shelly: &Device) -> Result<Status, attohttpc::Error> {
    let url = format!(
        "http://{}/rpc/Switch.GetStatus?id={}",
        shelly.host, shelly.switch
    );

    let response = attohttpc::get(url).send();

    match response {
        Ok(r) => {
            let json: Status = r.json().unwrap();
            return Ok(json);
        }
        Err(e) => {
            return Err(e);
        }
    }
}
