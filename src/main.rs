use std::sync::mpsc::RecvError;
use std::collections::HashSet;
use std::{thread, time};
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver};
use monero::util::amount::Amount;


use serde::{Deserialize};

#[derive(Debug, Clone)]
struct Payment {
    transaction_hash: String,
    watt_hours: f32,
}

#[derive(Deserialize, Debug, Clone)]
struct Status {
    apower: f32,
    aenergy: Energy,
}

#[derive(Deserialize, Debug, Clone)]
struct Energy {
    total: f32,
    by_minute: Vec<f32>,
    minute_ts: i64,
}

#[derive(Deserialize, Debug, Clone)]
struct Transaction {
    entryType: String,
    transactionHash: String,
    value: i64,
    blockIndex: i64,
    confirmations: i64,
}

#[derive(Deserialize, Debug, Clone)]
struct MoneroResponse {
    result: MoneroResult,
}

#[derive(Deserialize, Debug, Clone)]
struct MoneroResult {

    #[serde(rename = "in")]
    transfers: Vec<MoneroTransfer>,

    pool: Vec<MoneroTransfer>
}

#[derive(Deserialize, Debug, Clone)]
struct MoneroTransfer {
    amount: u64,
    #[serde(rename = "txid")]
    tx_hash: String, 
}

#[derive(Deserialize)]
struct Config {
    ip: String,
    monero: MoneroConfig,
}

#[derive(Deserialize)]
struct MoneroConfig {
    host: String,
    port: u16,
}


fn main() -> () {
    let config: Config = toml::from_str(r#"
        ip = '127.0.0.1'

        [monero]
        host = 'localhost'
        port = 18083

        [device.0]
        type = 'shelly'
        host =  '10.40.4.96'
        switch = 3
        monero = '46vp22XJf4CWcAdhXrWTW3AbgWbjairqd2pHE3Z5tMzrfq8szv1Dt7g1Pw7qj4gE87gJDJopNno6tDRcGDn8zUNg72h7eQt'

        [device.1]
        type = 'shelly'
        host =  '10.40.4.96'
        switch = 2
        monero = '84aGHMyaHbRg1rcZ9mCByuEMkAMorEqe4UCK3GFgcgTkHxQ1kJEJq6pBbHgdX1wRsRhJaZ2vbrxdoFTR7JNw7m7kMj6C1sm'
    "#).unwrap();


    let (sender, receiver): (Sender<Payment>, Receiver<Payment>) = mpsc::channel();
    let sender2 = sender.clone();

    thread::spawn(move || {
        //listen_for_blockcore_payments(sender);
    });

    thread::spawn(move || {
        listen_for_monero_payments(sender2, config.monero);
    });

    waiting_for_payments(receiver);
}

fn waiting_for_payments(receiver: Receiver<Payment>) {
    let tree = sled::open("/tmp/juice-me").expect("open");

    loop {
        let payment: Result<Payment, RecvError> = receiver.recv();

        match payment {
            Err(err) => {
                println!("Error while receiving payment: {}", err);
                thread::sleep_ms(1000);
                continue;
            }
            Ok(paid) => {
                let key = paid.transaction_hash.as_bytes();
                let cache = tree.get(key).unwrap();
        
                match cache {
                    Some(_already_paid) => {
                        continue;
                    }
                    None => {
                        println!("Payment received! Turing on power for {:.3} Wh", paid.watt_hours);
                    }
                }
        
                
                match status() {
                    Ok(s) => {
                        let end = s.aenergy.total + paid.watt_hours;
                        got_paid(paid.clone(), end);
                        tree.insert(key, "OK");
                    },
                    Err(_) => println!("Error while getting status"),
                }
            }
        } 
    }
}

/*
fn listen_for_blockcore_payments(sender: Sender<Transaction>) -> Result<(), reqwest::Error> {
    println!("Waiting for payments from blockcore");
    let poll_delay = time::Duration::from_millis(1000);
    let address = "qepVBqAgJ6xzv1TLpJS3mAnPycGfUab9pC";
    let mut offset = 0;    
    loop {
        let url = format!("https://tstrax.indexer.blockcore.net/api/query/address/{}/transactions?offset={}&limit=1", address, offset);
        let transactions: Vec<Transaction> = reqwest::blocking::get(url)?.json()?;

        match transactions.first() {
            Some(t) => {
                println!("Got transaction with {:.8} value", t.value as f64 / 10_0000_000 as f64);
                sender.send(t.clone());
                offset = offset + 1;        
            },
            None => thread::sleep(poll_delay),
        }
    }
}

fn listen_for_electrum_payments(sender: Sender<Transaction>) {
    let client = Client::new("tcp://10.40.4.2:50001").unwrap();
    let res = client.server_features();
    println!("{:#?}", res);

}
*/

fn listen_for_monero_payments(sender: Sender<Payment>, config: MoneroConfig) -> Result<(), reqwest::Error> {
    let host = config.host;
    let port = config.port;
    let poll_delay = time::Duration::from_millis(1000);
    
    let mut old_transactions: HashSet<String> = HashSet::new();

    let body = r#"{"jsonrpc":"2.0","id":"0","method":"get_transfers","params":{"in":true,"pending":true,"pool":true}}"#;
    let body2 = r#"{"jsonrpc":"2.0","id":"1","method":"refresh","params":{"start_height":2598796}}"#;

    println!("Waiting for payments from Monero");
    loop {
        let client = reqwest::blocking::Client::new();

        let url = format!("http://{}:{}/json_rpc", host, port);
        let res2 = client.post(&url)
            .body(body2)
            .send()?;

        let res = client.post(&url)
            .body(body)
            .send()?;
                
        let response: MoneroResponse = res.json()?;

        for t in response.result.transfers {   
            if old_transactions.contains(&t.tx_hash) {
                continue;
            }   

            let amount = Amount::from_pico(t.amount);
            let hash = t.tx_hash.clone();  
            println!("Got Monero transaction with {} XMR", amount);
            let paid = Payment {
                transaction_hash: t.tx_hash,
                watt_hours: 10 as f32 * amount.as_xmr() as f32,
            };

            sender.send(paid);
            old_transactions.insert(hash);
        }

        for t in response.result.pool {   
            if old_transactions.contains(&t.tx_hash) {
                continue;
            }   

            let amount = Amount::from_pico(t.amount);
            let hash = t.tx_hash.clone();  
            println!("Got Monero transaction with {} XMR", amount);
            let paid = Payment {
                transaction_hash: t.tx_hash,
                watt_hours: 10 as f32 * amount.as_xmr() as f32,
            };

            sender.send(paid);
            old_transactions.insert(hash);
        }

        thread::sleep(poll_delay);     
    }
}

fn got_paid(paid: Payment, end: f32) -> std::io::Result<()> {
    let poll_delay = time::Duration::from_millis(1000);
/*
    let root = "/home/jonny/tmp/juice_me/";
    fs::create_dir_all(root).unwrap();
    let full_path = Path::new(root).join(paid.transaction_hash);
    let mut f = File::options().append(true).open(full_path)?;
    println!("Turing on!");

*/
    on();
//    write!(f, "{} {}", 123445, "open");
    loop {
        match status() {
            Ok(s) => {
                println!("Current power {:.1}W, total watt hour {:.3} Wh used, will end at {:.3} Wh", s.apower, s.aenergy.total, end);
                //write!(f, "{} {:.3}", 123445, end - s.aenergy.total);
                if s.aenergy.total > end {
                    println!("Session done at {:.3} Wh", s.aenergy.total);
                    break;
                }
            },
            Err(_) => println!("Error while getting status"),
        }   
        thread::sleep(poll_delay);
    }
    println!("Turing off!");
    off();
//    write!(f, "{} {}", 223445, "close");

    Ok(())
}

fn on() -> Result<(), reqwest::Error> {
    let url = "http://10.40.4.96/rpc/Switch.Set?id=3&on=true";
    reqwest::blocking::get(url)?;
    
    Ok(())
}

fn off() -> Result<(), reqwest::Error> {
    let url = "http://10.40.4.96/rpc/Switch.Set?id=3&on=false";
    reqwest::blocking::get(url)?;

    Ok(())
}

fn status() -> Result<Status, reqwest::Error> {
    let url = "http://10.40.4.96/rpc/Switch.GetStatus?id=3";

    let json: Status = reqwest::blocking::get(url)?.json()?;

    Ok(json)
}