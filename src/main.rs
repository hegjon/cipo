#[macro_use]
extern crate log;

use std::collections::HashMap;
use std::collections::HashSet;

use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use std::time::{Duration, SystemTime};

mod args;
mod common;
mod config;
mod journal;
mod monero;
mod shelly;

use crate::args::Args;
use crate::common::Payment;
use crate::config::{Config, Device, Price};
use crate::journal::{JournalEntry, JournalReader, JournalWriter};
use crate::monero::{Monero, MoneroTransfer};
use crate::shelly::Shelly;

fn main() -> () {
    env_logger::init();

    let Args {
        config_file,
        journal_dir,
    } = Args::parse();

    info!("Cipo is starting up");
    info!("Config file: {}", config_file);
    info!(
        "Journal dir: {}",
        String::from(journal_dir.to_string_lossy())
    );

    let config = match Config::from_file(&config_file) {
        Ok(config) => {
            debug!("Loaded config with {} devices", config.device.len());
            config
        }
        Err(err) => {
            error!("Could not load config file: {}", err);
            std::process::exit(1);
        }
    };

    let (journal_tx, journal_rx): (Sender<JournalEntry>, Receiver<JournalEntry>) = mpsc::channel();

    let (monero_tx, monero_rx): (Sender<MoneroTransfer>, Receiver<MoneroTransfer>) =
        mpsc::channel();

    let (journal_reader_tx, journal_reader_rx): (Sender<Payment>, Receiver<Payment>) =
        mpsc::channel();

    let journal_reader = JournalReader::new(journal_reader_tx, journal_dir.clone());
    match journal_reader.read() {
        Ok(()) => info!("Journal have been read successful"),
        Err(err) => panic!("Failed while loading from journal: {}", err),
    }

    let journal = JournalWriter::new(journal_rx, journal_dir);
    thread::spawn(move || {
        journal.start();
    });

    let monero = Monero {
        sender: monero_tx,
        config: config.monero_rpc,
    };
    thread::spawn(move || loop {
        match monero.listen_for_payments() {
            Ok(_) => error!("Pre-mature exit from Monero query"),
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
                    channel.send(credit.clone()).unwrap();
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

                channel.send(payment).unwrap();
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

fn waiting_for_payment_per_device(
    payment_rx: Receiver<Payment>,
    journal: Sender<JournalEntry>,
    device: &Device,
) {
    for payment in payment_rx {
        handle_payment(journal.clone(), device, payment);
    }
}

fn handle_payment(journal_tx: Sender<JournalEntry>, device: &Device, payment: Payment) -> () {
    journal_tx
        .send(JournalEntry {
            time: SystemTime::now(),
            address: payment.address.clone(),
            txid: payment.txid.clone(),
            remaining_watt_hours: payment.watt_hours,
        })
        .unwrap();

    let ten_seconds = Duration::from_secs(10);
    let mut start: Option<f64> = None;
    let shelly = Shelly::new(device.clone());

    loop {
        match shelly.status() {
            Ok(s) => {
                let total = s.aenergy.total;
                match start {
                    None => {
                        start = Some(total);
                        info!("{}: Turing on, meter at {:.2} Wh", device.location, total);
                        debug!("{}: Current txid: {}", device.location, payment.txid);
                        let result = shelly.on();
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
                            let result = shelly.on();
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
                            let result = shelly.off();

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
        thread::sleep(ten_seconds);
    }
}
