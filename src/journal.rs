use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use std::sync::mpsc::Receiver;

use std::time::SystemTime;

pub struct JournalEntry {
    pub txid: String,
    pub time: SystemTime,
    pub remaining_watt_hours: f64,
}

pub struct JournalWriter {
    receiver: Receiver<JournalEntry>,
    journal_dir: PathBuf,
}

impl JournalWriter {
    pub fn new(receiver: Receiver<JournalEntry>, journal_dir: PathBuf) -> Self {
        JournalWriter {
            receiver,
            journal_dir,
        }
    }

    pub fn start(&self) -> () {
        while let Ok(entry) = self.receiver.recv() {
            self.handle_message(entry);
        }
    }

    fn handle_message(&self, entry: JournalEntry) {
        let log_file = journal_file(&entry.txid, &self.journal_dir);

        let mut f = File::options()
            .create(true)
            .append(true)
            .open(log_file)
            .unwrap();

        let time = humantime::format_rfc3339_seconds(entry.time);

        writeln!(f, "{} {:+.2}", time, entry.remaining_watt_hours);
    }
}

fn journal_file(txid: &String, journal_dir: &PathBuf) -> PathBuf {
    fs::create_dir_all(&journal_dir).unwrap();

    let file_name = format!("{}.log", txid);
    journal_dir.join(file_name)
}

pub fn have_been_journaled(txid: &String, journal_dir: &PathBuf) -> bool {
    let file = journal_file(txid, journal_dir);
    file.is_file() && file.exists()
}
