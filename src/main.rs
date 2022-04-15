#[macro_use]
extern crate log;

use std::collections::HashSet;
use std::collections::HashMap;
use std::{thread, time};
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver};
use monero::util::amount::Amount;
use f64;
use serde::{Deserialize,};
use serde_json::json;

use std::time::SystemTime;

struct JournalEntry {
    txid: String,
    time: SystemTime,
    remaining_watt_hours: f64,
}

#[derive(Deserialize, Debug, Clone)]
struct Payment {
    watt_hours: f64,
    txid: String,
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
    env_logger::init();

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

    let (journalTx, journal_rx): (Sender<JournalEntry>, Receiver<JournalEntry>) = mpsc::channel();

    let (sender, receiver): (Sender<MoneroTransfer>, Receiver<MoneroTransfer>) = mpsc::channel();

    thread::spawn(move || {
        listen_for_monero_payments(sender, config.monero_rpc);
    });
    thread::spawn(move || {
        journal_writer(journal_rx);
    });

    route_payments(receiver, journalTx, config.device, &config.price);
}

use std::env;
use std::path::PathBuf;
use std::fs::File;
use std::io::Write;

fn journal_file(txid: &String) -> PathBuf {
    let current_dir = env::current_dir().unwrap();
    let log_file = current_dir.join("journal").join(txid.to_owned() + ".log");

    log_file
}

fn have_been_journaled(txid: &String) -> bool {
    let file = journal_file(txid);
    file.is_file() && file.exists()
}

fn journal_writer(journal_rx: Receiver<JournalEntry>) {
    loop {
        let entry = journal_rx.recv().unwrap();

        let log_file = journal_file(&entry.txid);

        let mut f = File::options().create(true).append(true).open(log_file).unwrap();

        let time = humantime::format_rfc3339(entry.time);
        
        writeln!(f, "{} {:+.3}", time, entry.remaining_watt_hours);
    }
}


fn route_payments(receiver: Receiver<MoneroTransfer>, journal: Sender<JournalEntry>, devices: Vec<Device>, price: &Price) {
    let mut router = HashMap::new();

    for device in devices {
        let (sender2, receiver2): (Sender<Payment>, Receiver<Payment>) = mpsc::channel();
        let address = device.monero.clone();
        let journal = journal.clone();
        router.insert(address, sender2);
        thread::spawn(move || {
            waiting_for_payment_per_device(receiver2, journal, &device);
        });
    }

    loop {
        let transfer: MoneroTransfer = receiver.recv().unwrap();

        if have_been_journaled(&transfer.txid) {
            //already delivered electricity
            continue;
        }

        match router.get(&transfer.address) {
            Some(channel) => {
                let amount = Amount::from_pico(transfer.amount);   
                let payment = Payment {
                    watt_hours: price.xmr_per_watt_hour * amount.as_xmr(),
                    txid: transfer.txid.clone(),
                };
        
                channel.send(payment);
            },
            None => error!("missing device for address {}", &transfer.address),
        }                
    }
}

fn waiting_for_payment_per_device(receiver: Receiver<Payment>, journal: Sender<JournalEntry>, device: &Device) {
    loop {
        let payment: Payment = receiver.recv().unwrap();
        deliver_electricity(journal.clone(), device, payment);
    }
}

fn listen_for_monero_payments(sender: Sender<MoneroTransfer>, config: HostPort) -> Result<(), attohttpc::Error> {
    let poll_delay = time::Duration::from_millis(1000);
    
    let mut old_transactions: HashSet<String> = HashSet::new();

    let url = format!("http://{}:{}/json_rpc", config.host, config.port);
    let refresh = json!({
        "jsonrpc": "2.0",
        "id": "0",
        "method": "refresh",
        "params": {"start_height": 2598796}
    });

    let get_transfers = json!({
        "jsonrpc": "2.0",
        "id": "1",
        "method": "get_transfers",
        "params": {"in":true,"pending":true,"pool":true}
    });
    
    info!("Waiting for payments from Monero");
    loop {
        attohttpc::post(&url)
             .json(&refresh)?
             .send().unwrap();

        let res = attohttpc::post(&url)
            .json(&get_transfers)?
            .send()
            .unwrap();
                
        let response: MoneroResponse = res.json().unwrap();

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
        info!("Received {}", amount);
        let hash = t.txid.clone();

        sender.send(t.clone());
        old_transactions.insert(hash);
    }    
}

fn deliver_electricity(journal: Sender<JournalEntry>, device: &Device, paid: Payment) -> std::io::Result<()> {
    journal.send(JournalEntry {
        time: SystemTime::now(),
        txid: paid.txid.clone(),
        remaining_watt_hours: paid.watt_hours,
    }).unwrap();

    let mut start: Option<f64> = None;

    let poll_delay = time::Duration::from_millis(5000);
    loop {
        match status(device) {
            Ok(s) => {
                match start {
                    None => {
                        start = Some(s.aenergy.total);
                        info!("Turing on @{}!", device.location);
                        on(device);
                    }
                    Some(start) => {
                        let end = start + paid.watt_hours;
                        let current = s.aenergy.total;

                        journal.send(JournalEntry {
                            time: SystemTime::now(),
                            txid: paid.txid.clone(),
                            remaining_watt_hours: end - current,
                        }).unwrap();                        

                        debug!("Current power @{} {:.1}W, total watt hour {:.3} Wh used, will end at {:.3} Wh", device.location, s.apower, current, end);

                        if current < end {
                            on(device);
                        } else {
                            info!("Session done at {:.3} Wh", current);
                            info!("Turing off @{}!", device.location);
                            off(device);
                        
                            return Ok(());
                        }        
                    }
                }
            },
            Err(_) => error!("Error while getting status for {}", device.location),
        }   
        thread::sleep(poll_delay);
    }
}

fn on(shelly: &Device) -> Result<(), attohttpc::Error> {
    let url = format!("http://{}/rpc/Switch.Set?id={}&on=true", shelly.host, shelly.switch);
    attohttpc::get(url).send();
    
    Ok(())
}

fn off(shelly: &Device) -> Result<(), attohttpc::Error> {
    let url = format!("http://{}/rpc/Switch.Set?id={}&on=false", shelly.host, shelly.switch);
    attohttpc::get(url).send();

    Ok(())
}

fn status(shelly: &Device) -> Result<Status, attohttpc::Error> {
    let url = format!("http://{}/rpc/Switch.GetStatus?id={}", shelly.host, shelly.switch);

    let response = attohttpc::get(url).send();
    
    match response {
        Ok(r) => {
            let json: Status = r.json().unwrap();
            return Ok(json);
        }
        Err(e) => {
            return Err(e);
        },
    }
}