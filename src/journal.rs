use std::env;
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

pub fn journal_file(txid: &String, journal_dir: &PathBuf) -> PathBuf {
    let current_dir = env::current_dir().unwrap();
    let journal_dir = current_dir.join("journal");

    fs::create_dir_all(&journal_dir).unwrap();

    let file_name = format!("{}.log", txid);
    journal_dir.join(file_name)
}

pub fn have_been_journaled(txid: &String, journal_dir: &PathBuf) -> bool {
    let file = journal_file(txid, journal_dir);
    file.is_file() && file.exists()
}

pub fn journal_writer(journal_rx: Receiver<JournalEntry>, journal_dir: &PathBuf) {
    loop {
        let entry = journal_rx.recv().unwrap();

        let log_file = journal_file(&entry.txid, journal_dir);

        let mut f = File::options()
            .create(true)
            .append(true)
            .open(log_file)
            .unwrap();

        let time = humantime::format_rfc3339_seconds(entry.time);

        writeln!(f, "{} {:+.2}", time, entry.remaining_watt_hours);
    }
}
