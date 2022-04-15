use std::collections::HashSet;
use std::collections::HashMap;
use std::{thread, time};
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver};
use monero::util::amount::Amount;
use f64;
use serde::{Deserialize};

#[derive(Deserialize, Debug, Clone)]
struct Payment {
    watt_hours: f64,
}

#[derive(Deserialize, Debug, Clone)]
struct Status {
    apower: f64,
    aenergy: Energy,
}

#[derive(Deserialize, Debug, Clone)]
struct Energy {
    total: f64,
    by_minute: Vec<f32>,
    minute_ts: i64,
}

#[derive(Deserialize, Debug, Clone)]
struct MoneroResponse {
    result: MoneroResult,
}

#[derive(Deserialize, Debug, Clone)]
struct MoneroResult {

    #[serde(rename = "in")]
    transfers: Option<Vec<MoneroTransfer>>,

    pool: Option<Vec<MoneroTransfer>>
}

#[derive(Deserialize, Debug, Clone)]
struct MoneroTransfer {
    address: String,
    amount: u64,
    txid: String,
}

#[derive(Deserialize, Debug, Clone)]
struct Config {
    #[serde(rename = "monero-rpc")]
    monero_rpc: HostPort,
    device: Vec<Device>,
    price: Price,
}

#[derive(Deserialize, Debug, Clone)]
struct Price {
    #[serde(rename = "xmr-per-watt-hour")]
    xmr_per_watt_hour: f64,
}

#[derive(Deserialize, Debug, Clone)]
struct HostPort {
    host: String,
    port: u16,
}

#[derive(Deserialize, Debug, Clone)]
struct Device {
    location: String,
    host: String,
    switch: u16,
    monero: String
}

fn main() -> () {
    let config: Config = toml::from_str(r#"
        [price]
        xmr-per-watt-hour = 10.0

        [monero-rpc]
        host = 'localhost'
        port = 18083

        [[device]]
        location = 'Camping#1'
        host =  '10.40.4.96'
        switch = 3
        monero = '46vp22XJf4CWcAdhXrWTW3AbgWbjairqd2pHE3Z5tMzrfq8szv1Dt7g1Pw7qj4gE87gJDJopNno6tDRcGDn8zUNg72h7eQt'

        [[device]]
        location = 'Camping#2'
        host =  '10.40.4.96'
        switch = 2
        monero = '84aGHMyaHbRg1rcZ9mCByuEMkAMorEqe4UCK3GFgcgTkHxQ1kJEJq6pBbHgdX1wRsRhJaZ2vbrxdoFTR7JNw7m7kMj6C1sm'
    "#).unwrap();

    let (sender, receiver): (Sender<MoneroTransfer>, Receiver<MoneroTransfer>) = mpsc::channel();

    thread::spawn(move || {
        listen_for_monero_payments(sender, config.monero_rpc);
    });

    route_payments(receiver, config.device, &config.price);
}

fn route_payments(receiver: Receiver<MoneroTransfer>, devices: Vec<Device>, price: &Price) {
    let persistence = sled::open("/tmp/juice-me").expect("open");

    let mut router = HashMap::new();

    for device in devices {
        let (sender2, receiver2): (Sender<Payment>, Receiver<Payment>) = mpsc::channel();
        let address = device.monero.clone();
        router.insert(address, sender2);
        thread::spawn(move || {
            waiting_for_payment_per_device(receiver2, &device);
        });
    }

    loop {
        let moneroTransfer: MoneroTransfer = receiver.recv().unwrap();

        let key = moneroTransfer.txid.as_bytes();
        let hit = persistence.get(key).unwrap();


        match hit {
            Some(_already_paid) => {
                continue;
            }
            None => {                        
                match router.get(&moneroTransfer.address) {
                    Some(channel) => {
                        let amount = Amount::from_pico(moneroTransfer.amount);   
                        let payment = Payment {
                            watt_hours: price.xmr_per_watt_hour * amount.as_xmr(),
                        };
                
                        persistence.insert(key, "OK");
                        channel.send(payment);
                    },
                    None => println!("ERROR, missing device for address {}", &moneroTransfer.address),
                }
            }
        }                
    }
}

fn waiting_for_payment_per_device(receiver: Receiver<Payment>, device: &Device) {
    loop {
        let paid: Payment = receiver.recv().unwrap();

        match status(device) {
            Ok(s) => {
                println!("Payment received! Turing on power @{} for {:.3} Wh", device.location, paid.watt_hours);
                let end = s.aenergy.total + paid.watt_hours;
                got_paid(device, paid.clone(), end);
            },
            Err(_) => println!("Error while getting status"),
        }
    }
}

fn listen_for_monero_payments(sender: Sender<MoneroTransfer>, config: HostPort) -> Result<(), reqwest::Error> {
    let poll_delay = time::Duration::from_millis(1000);
    
    let mut old_transactions: HashSet<String> = HashSet::new();

    let body = r#"{"jsonrpc":"2.0","id":"0","method":"get_transfers","params":{"in":true,"pending":true,"pool":true}}"#;
    let body2 = r#"{"jsonrpc":"2.0","id":"1","method":"refresh","params":{"start_height":2598796}}"#;

    println!("Waiting for payments from Monero");
    loop {
        let client = reqwest::blocking::Client::new();

        let url = format!("http://{}:{}/json_rpc", config.host, config.port);
        let res2 = client.post(&url)
            .body(body2)
            .send()?;

        let res = client.post(&url)
            .body(body)
            .send()?;
                
        let response: MoneroResponse = res.json()?;

        match response.result.transfers {
            Some(t) => iterate_monero_transactions(&t, &mut old_transactions, &sender),
            None => (),
        }

        match response.result.pool {
            Some(t) => iterate_monero_transactions(&t, &mut old_transactions, &sender),
            None => (),
        }

        thread::sleep(poll_delay);     
    }
}

fn iterate_monero_transactions(transactions: &Vec<MoneroTransfer>, old_transactions: &mut HashSet<String>, sender: &Sender<MoneroTransfer>) {
    for t in transactions {   
        if old_transactions.contains(&t.txid) {
            continue;
        }   

        let amount = Amount::from_pico(t.amount);
        println!("Got Monero transaction with {} XMR", amount);
        let hash = t.txid.clone();

        sender.send(t.clone());
        old_transactions.insert(hash);
    }    
}

fn got_paid(device: &Device, paid: Payment, end: f64) -> std::io::Result<()> {
    let poll_delay = time::Duration::from_millis(1000);
    println!("Turing on @{}!", device.location);
    on(device);
    loop {
        match status(device) {
            Ok(s) => {
                println!("Current power @{} {:.1}W, total watt hour {:.3} Wh used, will end at {:.3} Wh", device.location, s.apower, s.aenergy.total, end);
                //write!(f, "{} {:.3}", 123445, end - s.aenergy.total);
                if s.aenergy.total > end {
                    println!("Session done at {:.3} Wh", s.aenergy.total);
                    break;
                }
            },
            Err(_) => println!("Error while getting status for {}", device.location),
        }   
        thread::sleep(poll_delay);
    }
    println!("Turing off @{}!", device.location);
    off(device);

    Ok(())
}

fn on(shelly: &Device) -> Result<(), reqwest::Error> {
    let url = format!("http://{}/rpc/Switch.Set?id={}&on=true", shelly.host, shelly.switch);
    reqwest::blocking::get(url)?;
    
    Ok(())
}

fn off(shelly: &Device) -> Result<(), reqwest::Error> {
    let url = format!("http://{}/rpc/Switch.Set?id={}&on=false", shelly.host, shelly.switch);
    reqwest::blocking::get(url)?;

    Ok(())
}

fn status(shelly: &Device) -> Result<Status, reqwest::Error> {
    let url = format!("http://{}/rpc/Switch.GetStatus?id={}", shelly.host, shelly.switch);

    let json: Status = reqwest::blocking::get(url)?.json()?;

    Ok(json)
}