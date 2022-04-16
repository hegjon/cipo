#[macro_use]
extern crate log;

use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use std::time::{Duration, SystemTime};

mod config;
mod journal;
mod shelly;

use crate::config::{Config, Device, HostPort, Price};
use crate::journal::JournalEntry;

#[derive(Deserialize, Debug, Clone)]
struct Payment {
    watt_hours: f64,
    txid: String,
}

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

fn main() -> () {
    env_logger::init();

    info!("Cipo is starting up");

    let config: Config = config::load_from_file();

    let (journalTx, journal_rx): (Sender<JournalEntry>, Receiver<JournalEntry>) = mpsc::channel();

    let (sender, receiver): (Sender<MoneroTransfer>, Receiver<MoneroTransfer>) = mpsc::channel();

    thread::spawn(move || {
        listen_for_monero_payments(sender, config.monero_rpc);
    });
    thread::spawn(move || {
        journal::journal_writer(journal_rx);
    });

    route_payments(receiver, journalTx, config.device, &config.price);
}

fn route_payments(
    receiver: Receiver<MoneroTransfer>,
    journal: Sender<JournalEntry>,
    devices: Vec<Device>,
    price: &Price,
) {
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

        if journal::have_been_journaled(&transfer.txid) {
            //already delivered electricity
            continue;
        }

        match router.get(&transfer.address) {
            Some(channel) => {
                let payment = Payment {
                    watt_hours: calculate_watt_hours(price.xmr_per_kwh, transfer.amount),
                    txid: transfer.txid.clone(),
                };

                channel.send(payment);
            }
            None => error!("missing device for address {}", &transfer.address),
        }
    }
}

fn calculate_watt_hours(xmr_per_kwh: f64, picomonero: u64) -> f64 {
    let xmr: f64 = picomonero as f64 / 1000000000000.0;

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

fn waiting_for_payment_per_device(
    receiver: Receiver<Payment>,
    journal: Sender<JournalEntry>,
    device: &Device,
) {
    loop {
        let payment: Payment = receiver.recv().unwrap();
        deliver_electricity(journal.clone(), device, payment);
    }
}

fn listen_for_monero_payments(
    sender: Sender<MoneroTransfer>,
    config: HostPort,
) -> Result<(), attohttpc::Error> {
    let poll_delay = Duration::from_millis(1000);

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
        attohttpc::post(&url).json(&refresh)?.send().unwrap();

        let res = attohttpc::post(&url).json(&get_transfers)?.send().unwrap();

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

fn iterate_monero_transactions(
    transactions: &Vec<MoneroTransfer>,
    old_transactions: &mut HashSet<String>,
    sender: &Sender<MoneroTransfer>,
) {
    for t in transactions {
        if old_transactions.contains(&t.txid) {
            continue;
        }

        let xmr = t.amount as f64 / 1000000000000.0;

        info!("Received {:0.12} XMR to {}", xmr, t.address);
        let hash = t.txid.clone();

        sender.send(t.clone());
        old_transactions.insert(hash);
    }
}

fn deliver_electricity(
    journal: Sender<JournalEntry>,
    device: &Device,
    paid: Payment,
) -> std::io::Result<()> {
    journal
        .send(JournalEntry {
            time: SystemTime::now(),
            txid: paid.txid.clone(),
            remaining_watt_hours: paid.watt_hours,
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
                        shelly::on(device);
                    }
                    Some(start) => {
                        let end = start + paid.watt_hours;

                        journal
                            .send(JournalEntry {
                                time: SystemTime::now(),
                                txid: paid.txid.clone(),
                                remaining_watt_hours: end - total,
                            })
                            .unwrap();

                        debug!(
                            "{}: Load {:.1} W, meter at {:.3} Wh, will end at {:.3} Wh",
                            device.location, s.apower, total, end
                        );

                        if total < end {
                            shelly::on(device);
                        } else {
                            info!("{}: Turing off, meter at {:.2} Wh", device.location, total);
                            shelly::off(device);

                            return Ok(());
                        }
                    }
                }
            }
            Err(_) => error!("Error while getting status for {}", device.location),
        }
        thread::sleep(poll_delay);
    }
}