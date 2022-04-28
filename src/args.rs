use clap::{Arg, Command};
use serde::Deserialize;
use std::env;
use std::path::PathBuf;

#[derive(Deserialize, Debug, Clone)]
pub struct Args {
    pub config_file: String,
    pub journal_dir: PathBuf,
}

impl Args {
    pub fn parse() -> Self {
        let matches = Command::new("Cipo")
            .version("0.1.6")
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

        Args {
            config_file: String::from(config_file),
            journal_dir,
        }
    }
}
