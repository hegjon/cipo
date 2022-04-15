use std::env;
use std::path::PathBuf;
use std::fs::File;
use std::io::Write;

use std::sync::mpsc::Receiver;

use std::time::SystemTime;


pub struct JournalEntry {
    pub txid: String,
    pub time: SystemTime,
    pub remaining_watt_hours: f64,
}

pub fn journal_file(txid: &String) -> PathBuf {
    let current_dir = env::current_dir().unwrap();
    let log_file = current_dir.join("journal").join(txid.to_owned() + ".log");

    log_file
}

pub fn have_been_journaled(txid: &String) -> bool {
    let file = journal_file(txid);
    file.is_file() && file.exists()
}

pub fn journal_writer(journal_rx: Receiver<JournalEntry>) {
    loop {
        let entry = journal_rx.recv().unwrap();

        let log_file = journal_file(&entry.txid);

        let mut f = File::options().create(true).append(true).open(log_file).unwrap();

        let time = humantime::format_rfc3339(entry.time);
        
        writeln!(f, "{} {:+.3}", time, entry.remaining_watt_hours);
    }
}