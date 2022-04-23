#[macro_use]
extern crate log;

use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;

use std::env;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use std::time::{Duration, SystemTime};

mod common;
mod config;
mod journal;
mod shelly;

use crate::common::Payment;
use crate::config::{Config, Device, HostPort, Price};
use crate::journal::{JournalEntry, JournalReader, JournalWriter};

#[derive(Deserialize, Debug, Clone)]
struct MoneroResponse {
    result: MoneroResult,
}

#[derive(Deserialize, Debug, Clone)]
struct MoneroResult {
    #[serde(rename = "in")]
    transfers: Option<Vec<MoneroTransfer>>,

    pool: Option<Vec<MoneroTransfer>>,
}

#[derive(Deserialize, Debug, Clone)]
struct MoneroTransfer {
    address: String,
    amount: u64,
    txid: String,
}

use clap::{Arg, Command};

fn main() -> () {
    env_logger::init();

    let matches = Command::new("Cipo")
        .version("0.1.2")
        .author("Jonny Heggheim <jonny@hegghe.im>")
        .about("Crypto in, power out")
        .arg(
            Arg::new("config")
                .short('f')
                .long("config")
                .takes_value(true)
                .default_value("/etc/cipo.toml")
                .help("Config file"),
        )
        .arg(
            Arg::new("journal")
                .short('j')
                .long("journal")
                .takes_value(true)
                .help("Journal directory, default is ${CWD}/journal"),
        )
        .get_matches();

    let config_file = matches.value_of("config").unwrap_or("/etc/cipo.toml");
    let journal_dir = match matches.value_of("journal") {
        Some(dir) => PathBuf::from(dir),
        None => {
            let current_dir = env::current_dir().unwrap();
            let journal_dir = current_dir.join("journal");
            journal_dir
        }
    };

    info!("Cipo is starting up");
    info!("Config file: {}", config_file);
    info!(
        "Journal dir: {}",
        String::from(journal_dir.to_string_lossy())
    );

    let config: Config = config::load_from_file(&config_file.to_string());

    let (journal_tx, journal_rx): (Sender<JournalEntry>, Receiver<JournalEntry>) = mpsc::channel();

    let (monero_tx, monero_rx): (Sender<MoneroTransfer>, Receiver<MoneroTransfer>) =
        mpsc::channel();

    let (journal_reader_tx, journal_reader_rx): (Sender<Payment>, Receiver<Payment>) =
        mpsc::channel();
    let reader = JournalReader::new(journal_reader_tx, journal_dir.clone());

    reader.read();

    let journal = JournalWriter::new(journal_rx, journal_dir);
    thread::spawn(move || {
        journal.start();
    });

    thread::spawn(move || loop {
        match listen_for_monero_payments(monero_tx.clone(), config.monero_rpc.clone()) {
            Ok(_) => error!("Premature return from Monero query"),
            Err(err) => error!("Error while query Monero wallet: {}", err),
        }

        thread::sleep(Duration::from_secs(10));
    });

    route_payments(
        monero_rx,
        journal_tx,
        config.device,
        &config.price,
        journal_reader_rx,
    );
}

fn route_payments(
    monero_rx: Receiver<MoneroTransfer>,
    journal: Sender<JournalEntry>,
    devices: Vec<Device>,
    price: &Price,
    journal_reader: Receiver<Payment>,
) {
    let mut router = HashMap::new();
    let mut processed_transactions: HashSet<String> = HashSet::new();

    for device in devices {
        let (sender2, receiver2): (Sender<Payment>, Receiver<Payment>) = mpsc::channel();
        let address = device.monero.clone();
        let journal = journal.clone();
        router.insert(address, sender2);
        thread::spawn(move || {
            waiting_for_payment_per_device(receiver2, journal, &device);
        });
    }

    for credit in journal_reader.try_iter() {
        if processed_transactions.contains(&credit.txid) {
            continue;
        }

        match router.get(&credit.address) {
            Some(channel) => {
                if credit.watt_hours > 0.0 {
                    info!("Got credit of {} for {} Wh", credit.txid, credit.watt_hours);
                    channel.send(credit.clone());
                }
                processed_transactions.insert(credit.txid.clone());
            }
            None => error!("missing device for address"),
        }
    }

    for transfer in monero_rx {
        if processed_transactions.contains(&transfer.txid) {
            continue;
        }

        match router.get(&transfer.address) {
            Some(channel) => {
                let payment = Payment {
                    watt_hours: calculate_watt_hours(price.xmr_per_kwh, transfer.amount),
                    address: transfer.address.clone(),
                    txid: transfer.txid.clone(),
                };

                channel.send(payment);
                processed_transactions.insert(transfer.txid.clone());
            }
            None => error!("missing device for address {}", &transfer.address),
        }
    }
}

fn calculate_watt_hours(xmr_per_kwh: f64, picomonero: u64) -> f64 {
    let xmr: f64 = picomonero as f64 / 1_000_000_000_000.0;

    (xmr / xmr_per_kwh) * 1000.0
}

#[cfg(test)]
mod tests {
    use super::calculate_watt_hours;

    #[test]
    fn test_one_xmr() {
        let actual = calculate_watt_hours(1.0, 1_000_000_000_000);
        let expected = 1000.0;

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_two_xmr() {
        let actual = calculate_watt_hours(1.0, 2_000_000_000_000);
        let expected = 2000.0;

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_half_price() {
        let actual = calculate_watt_hours(0.5, 1_000_000_000_000);
        let expected = 2000.0;

        assert_eq!(actual, expected);
    }
}

fn listen_for_monero_payments(
    sender: Sender<MoneroTransfer>,
    config: HostPort,
) -> Result<(), attohttpc::Error> {
    let poll_delay = Duration::from_secs(1);

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
        attohttpc::post(&url).json(&refresh)?.send()?;

        let res = attohttpc::post(&url).json(&get_transfers)?.send()?;

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

fn iterate_monero_transactions(
    transactions: &Vec<MoneroTransfer>,
    old_transactions: &mut HashSet<String>,
    sender: &Sender<MoneroTransfer>,
) {
    for t in transactions {
        if old_transactions.contains(&t.txid) {
            continue;
        }

        let xmr = t.amount as f64 / 1_000_000_000_000.0;

        info!("Received {:0.12} XMR to {}", xmr, t.address);
        let hash = t.txid.clone();

        sender.send(t.clone());
        old_transactions.insert(hash);
    }
}

fn waiting_for_payment_per_device(
    payment_rx: Receiver<Payment>,
    journal: Sender<JournalEntry>,
    device: &Device,
) {
    for payment in payment_rx {
        deliver_electricity(journal.clone(), device, payment);
    }
}

fn deliver_electricity(journal_tx: Sender<JournalEntry>, device: &Device, payment: Payment) -> () {
    journal_tx
        .send(JournalEntry {
            time: SystemTime::now(),
            address: payment.address.clone(),
            txid: payment.txid.clone(),
            remaining_watt_hours: payment.watt_hours,
        })
        .unwrap();

    let poll_delay = Duration::from_secs(10);
    let mut start: Option<f64> = None;

    loop {
        match shelly::status(device) {
            Ok(s) => {
                let total = s.aenergy.total;
                match start {
                    None => {
                        start = Some(total);
                        info!("{}: Turing on, meter at {:.2} Wh", device.location, total);
                        debug!("{}: Current txid: {}", device.location, payment.txid);
                        let result = shelly::on(device);
                        match result {
                            Ok(_) => debug!("{}: Device turned on", device.location),
                            Err(err) => error!(
                                "{}: Error while turning device on: {}",
                                device.location, err
                            ),
                        }
                    }
                    Some(start) => {
                        let end = start + payment.watt_hours;

                        journal_tx
                            .send(JournalEntry {
                                time: SystemTime::now(),
                                address: payment.address.clone(),
                                txid: payment.txid.clone(),
                                remaining_watt_hours: end - total,
                            })
                            .unwrap();

                        debug!(
                            "{}: Current load {:.1} W, meter at {:.3} Wh, will end at {:.3} Wh",
                            device.location, s.apower, total, end
                        );

                        if total < end {
                            let result = shelly::on(device);
                            match result {
                                Ok(_) => {
                                    debug!("{}: Make sure device is turned on", device.location)
                                }
                                Err(err) => error!(
                                    "{}: Error while turning device on: {}",
                                    device.location, err
                                ),
                            }
                        } else {
                            info!("{}: Turing off, meter at {:.2} Wh", device.location, total);
                            debug!("{}: Current txid: {}", device.location, payment.txid);
                            let result = shelly::off(device);

                            match result {
                                Err(err) => error!(
                                    "{}: Error while turning device off: {}",
                                    device.location, err
                                ),
                                Ok(_) => {
                                    debug!("{}: Device turned off", device.location);
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => error!(
                "{}: Error while getting status for: {}",
                device.location, err
            ),
        }
        thread::sleep(poll_delay);
    }
}
