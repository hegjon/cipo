use std::f64;
use std::fs;
use std::io;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use std::sync::mpsc::{Sender, Receiver};

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
        JournalWriter {
            rx,
            journal_dir,
        }
    }

    pub fn start(&self) -> () {
        for entry in &self.rx {
            self.write(entry);
        }
    }

    fn write(&self, entry: JournalEntry) {
        let log_file = journal_file(&entry, &self.journal_dir);

        let mut f = File::options()
            .create(true)
            .append(true)
            .open(log_file)
            .unwrap();

        let time = humantime::format_rfc3339_seconds(entry.time);

        writeln!(f, "{} {:+.2}", time, entry.remaining_watt_hours);
    }
}

impl JournalReader {
    pub fn new(tx: Sender<Payment>, journal_dir: PathBuf) -> Self {
        JournalReader {
            tx,
            journal_dir,
        }
    }

    pub fn read(&self) -> io::Result<()> {
        info!("Scanning journal for unfinished deliveries");
        for entry in fs::read_dir(&self.journal_dir)? {
            let address = entry?;
            let path = address.path();
            debug!("Loading journal for address {:?}", address);
            for entry2 in fs::read_dir(path)? {
                let txid = entry2?;
                debug!("Loading txid {:?}", txid);

                let line = last_line(&txid.path());
                let remaining_watt_hours: f64 = match line.split_once(' ') {
                    Some((time, watt_hours)) => watt_hours.parse().unwrap(),
                    None => -0.1
                };

                let credit = Payment {
                    address: address.file_name().into_string().unwrap(),
                    txid: txid.file_name().into_string().unwrap().trim_end_matches(".log").to_string(),
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

fn last_line(file: &PathBuf) -> String {
    let content = fs::read_to_string(file);

    match content {
        Ok(lines) => match lines.lines().last() {
            Some(last) => last.to_owned(),
            None => "2000-04-20T20:50:47Z -0.01".to_owned()
        },
        Err(err) => "2000-04-20T20:50:47Z -0.01".to_owned()
    }
}
