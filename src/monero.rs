use std::thread;

use std::collections::HashSet;

use std::sync::mpsc::Sender;

use serde::Deserialize;
use serde_json::json;

use crate::config::HostPort;

use std::time::Duration;

#[derive(Deserialize, Debug, Clone)]
pub struct MoneroResponse {
    pub result: MoneroResult,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MoneroResult {
    #[serde(rename = "in")]
    pub transfers: Option<Vec<MoneroTransfer>>,

    pub pool: Option<Vec<MoneroTransfer>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MoneroTransfer {
    pub address: String,
    pub amount: u64,
    pub txid: String,
}

pub struct Monero {
    pub sender: Sender<MoneroTransfer>,
    pub config: HostPort,
}

impl Monero {
    pub fn listen_for_payments(&self) -> Result<(), attohttpc::Error> {
        let one_second = Duration::from_secs(1);

        let mut old_transactions: HashSet<String> = HashSet::new();

        let url = format!("http://{}:{}/json_rpc", self.config.host, self.config.port);
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
                Some(t) => self.iterate_monero_transactions(&t, &mut old_transactions),
                None => (),
            }

            match response.result.pool {
                Some(t) => self.iterate_monero_transactions(&t, &mut old_transactions),
                None => (),
            }

            thread::sleep(one_second);
        }
    }

    fn iterate_monero_transactions(
        &self,
        transactions: &Vec<MoneroTransfer>,
        old_transactions: &mut HashSet<String>,
    ) {
        for t in transactions {
            if old_transactions.contains(&t.txid) {
                continue;
            }

            let xmr = t.amount as f64 / 1_000_000_000_000.0;

            info!("Received {:0.12} XMR to {}", xmr, t.address);
            let hash = t.txid.clone();

            self.sender.send(t.clone()).unwrap();
            old_transactions.insert(hash);
        }
    }
}
