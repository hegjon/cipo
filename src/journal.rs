use std::f64;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::PathBuf;

use std::sync::mpsc::{Receiver, Sender};

use std::time::SystemTime;

use crate::Payment;

pub struct JournalEntry {
    pub address: String,
    pub txid: String,
    pub time: SystemTime,
    pub remaining_watt_hours: f64,
}

pub struct JournalWriter {
    rx: Receiver<JournalEntry>,
    journal_dir: PathBuf,
}

pub struct JournalReader {
    pub tx: Sender<Payment>,
    pub journal_dir: PathBuf,
}

impl JournalWriter {
    pub fn new(rx: Receiver<JournalEntry>, journal_dir: PathBuf) -> Self {
        JournalWriter { rx, journal_dir }
    }

    pub fn start(&self) -> () {
        for entry in &self.rx {
            let result = self.write(entry);

            if let Err(err) = result {
                panic!("Error while writing to journal: {}", err)
            }
        }
    }

    fn write(&self, entry: JournalEntry) -> io::Result<()> {
        let log_file = journal_file(&entry, &self.journal_dir);

        let mut f = File::options().create(true).append(true).open(log_file)?;

        let time = humantime::format_rfc3339_seconds(entry.time);

        writeln!(f, "{} {:+.2}", time, entry.remaining_watt_hours)
    }
}

impl JournalReader {
    pub fn new(tx: Sender<Payment>, journal_dir: PathBuf) -> Self {
        JournalReader { tx, journal_dir }
    }

    pub fn read(&self) -> io::Result<()> {
        info!("Scanning journal for unfinished deliveries");
        for entry in fs::read_dir(&self.journal_dir)? {
            let address = entry?;
            let address2 = address.file_name().into_string().unwrap();

            debug!("Loading journal for address {}", address2);
            for entry2 in fs::read_dir(address.path())? {
                let txid = entry2?;
                let txid2 = txid
                    .file_name()
                    .into_string()
                    .unwrap()
                    .trim_end_matches(".log")
                    .to_string();

                debug!("Loading txid {}", txid2);

                let line = last_line(&txid.path());
                let remaining_watt_hours: f64 = match line?.split_once(' ') {
                    Some((_time, watt_hours)) => watt_hours.parse().unwrap(),
                    None => -0.1,
                };

                let credit = Payment {
                    address: address2.clone(),
                    txid: txid2,
                    watt_hours: remaining_watt_hours,
                };

                if remaining_watt_hours > 0.0 {
                    debug!("Unfinished delivery for txid {} Wh", remaining_watt_hours);
                }
                self.tx.send(credit).unwrap();
            }
        }
        info!("Journal is done");
        Ok(())
    }
}

fn journal_file(entry: &JournalEntry, journal_dir: &PathBuf) -> PathBuf {
    let file_name = format!("{}.log", entry.txid);
    let path = journal_dir.join(&entry.address);
    fs::create_dir_all(&path).unwrap();

    path.join(file_name)
}

fn last_line(file: &PathBuf) -> io::Result<String> {
    let content = fs::read_to_string(file)?;

    match content.lines().last() {
        Some(last) => Ok(last.to_owned()),
        None => {
            let msg = format!("Corrupt journal, {} have no lines!", file.to_string_lossy());
            let error = io::Error::new(io::ErrorKind::Other, msg);

            Err(error)
        }
    }
}
