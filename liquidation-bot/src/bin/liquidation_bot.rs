use base64::engine::general_purpose::STANDARD as base64_engine;
use base64::Engine;
use core::time;
use log::{error, info, warn};
use std::thread;
use stellar_strkey;
use stellar_xdr::curr::{AccountId, Limits, PublicKey, ReadXdr, ScAddress, ScVal, ScVec, Uint256};

// use self::models::*;
// use diesel::prelude::*;
// use liquidation_bot::*;
use reqwest;
use serde_json::{json, Value};

const SLEEP_TIME_SECONDS: u64 = 10;

#[derive(Debug)]
#[allow(dead_code)]
enum BotError {
    Request(reqwest::Error),
    Parse(serde_json::Error),
    StrKey(stellar_strkey::DecodeError),
}

impl From<reqwest::Error> for BotError {
    fn from(err: reqwest::Error) -> Self {
        BotError::Request(err)
    }
}

impl From<serde_json::Error> for BotError {
    fn from(err: serde_json::Error) -> Self {
        BotError::Parse(err)
    }
}

impl From<stellar_strkey::DecodeError> for BotError {
    fn from(err: stellar_strkey::DecodeError) -> Self {
        BotError::StrKey(err)
    }
}

#[tokio::main]
async fn main() -> Result<(), BotError> {
    // let connection = &mut establish_connection();
    env_logger::init();

    info!("This is an info message");
    warn!("This is a warning message");
    error!("This is an error message");

    let mut last_ledger = 958657;

    loop {
        last_ledger = get_new_loans(last_ledger).await?;
        // get_prices();
        // find_liquidateable(connection);
        // attempt_liquidating();

        info!("Sleeping for {SLEEP_TIME_SECONDS} seconds.");
        thread::sleep(time::Duration::from_secs(SLEEP_TIME_SECONDS))
    }
}

async fn get_new_loans(start_ledger: i32) -> Result<i32, BotError> {
    // TODO: fetch loans from Loan Manager
    // TODO: push new loans to the DB.
    info!("Fetching new loans from Loan Manager.");
    let url = "http://localhost:8000/soroban/rpc";

    let json_data = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getEvents",
        "params": {
            "startLedger": start_ledger,
            "filters": [
                {
                    "type": "contract",
                    "contractIds": [
                        "CD5QTKZZCIBF2LRGKWXB4KCRFDZAOBBLSS443FX3UODCATS2N27DWZMF"
                    ]
                }
            ]
        }
    });

    let client = reqwest::Client::new();

    let response = client.post(url).json(&json_data).send().await?;

    let response_body = response.text().await?;

    let parsed: Value = serde_json::from_str(&response_body)?;
    println!("Fetching events...");
    if let Some(events) = parsed["result"]["events"].as_array() {
        for event in events {
            let contract_id = event["contractId"].as_str().unwrap_or_default();
            let in_success = event["inSuccessfulContractCall"].as_bool().unwrap_or(false);
            let ledger = event["ledger"].as_u64().unwrap_or_default();

            let topics: Vec<String> = event["topic"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|t| t.as_str().and_then(decode_topic))
                .collect();
            let raw_value = event["value"].as_str().unwrap_or_default();

            let decoded_value = base64_engine.decode(raw_value).ok().and_then(|bytes| {
                ScVal::from_xdr(
                    bytes,
                    Limits {
                        depth: 64,
                        len: 10000,
                    },
                )
                .ok()
            });

            println!("--- Event ---");
            println!("Contract ID: {}", contract_id);
            println!("In Successful Call: {}", in_success);
            println!("Ledger: {}", ledger);
            println!("Topics: {:?}", topics);
            if let Some(val) = decoded_value {
                match unpack_scval(&val) {
                    Ok(public_key_string) => {
                        println!("Extracted public key: {}", public_key_string);
                        // if contract_id = in list of allowed AND ->
                        if in_success {
                            match topics.as_slice() {
                                [a, b] if a == "Loan" && b == "created" => {
                                    println!("Loan created!")
                                }
                                [a, b] if a == "Loan" && b == "updated" => {
                                    println!("Loan updated!")
                                }
                                [a, b] if a == "Loan" && b == "deleted" => {
                                    println!("Loan deleted!")
                                }
                                _ => println!("Unknown topic: {:?}", topics),
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to unpack value: {:#?}", e);
                    }
                }
            } else {
                println!("Decoded Value: <failed to decode>");
            }
        }
    }
    let latest_ledger = parsed["result"]["latestLedger"]
        .as_i64()
        .unwrap_or(start_ledger as i64) as i32;

    Ok(latest_ledger)
}

fn decode_topic(val: &str) -> Option<String> {
    let decoded = base64_engine.decode(val).ok()?;
    let scval = ScVal::from_xdr(
        decoded,
        Limits {
            depth: 64,
            len: 10000,
        },
    )
    .ok()?;
    if let ScVal::Symbol(sym) = scval {
        Some(sym.to_string())
    } else {
        None
    }
}

fn unpack_scval(val: &ScVal) -> Result<String, BotError> {
    match val {
        ScVal::Vec(Some(ScVec(vec))) => {
            println!("It's a vector with {} items", vec.len());
            for (i, item) in vec.iter().enumerate() {
                match item {
                    ScVal::Symbol(symbol) => {
                        println!("Item {} is a Symbol: {}", i, symbol.to_string());
                    }
                    ScVal::Address(addr) => match addr {
                        ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(
                            Uint256(bytes),
                        ))) => {
                            let publickey_string = stellar_strkey::ed25519::PublicKey::to_string(
                                &stellar_strkey::ed25519::PublicKey::from_payload(bytes)?,
                            );
                            println!("Item {} is an Address: {:#?}", i, publickey_string);
                            return Ok(publickey_string);
                        }
                        _ => println!("Item {} is another Address type", i),
                    },
                    other => {
                        println!("Item {} is something else: {:?}", i, other);
                    }
                }
            }
            Ok(String::new())
        }
        other => {
            println!("Not a vector, it's: {:?}", other);
            Ok(String::new())
        }
    }
}

// fn get_prices() {
//     // TODO: fetch and return token prices from CoinGecko
//     info!("Getting prices from CoinGecko.")
// }
//
// fn find_liquidateable(connection: &mut PgConnection /*prices: Prices*/) {
//     use self::schema::loans::dsl::*;
//
//     let results = loans
//         .limit(5)
//         .select(Loan::as_select())
//         .load(connection)
//         .expect("Error loading loans");
//
//     info!("Displaying {} loans.", results.len());
//     for loan in results {
//         info!("{}", loan.id);
//     }
//
//     // TODO: calculate the health of each loan and return the unhealthy ones
// }
//
// fn attempt_liquidating(/* unhealthy_loans: Vec<Loan> */) {
//     // TODO: attempt to liquidate unhealthy loans
//     // TODO: update the loan in DB with the new values
// }
