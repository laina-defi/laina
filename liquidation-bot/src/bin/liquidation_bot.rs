use core::time;
use log::{error, info, warn};
use std::thread;

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

#[tokio::main]
async fn main() -> Result<(), BotError> {
    // let connection = &mut establish_connection();
    env_logger::init();

    info!("This is an info message");
    warn!("This is a warning message");
    error!("This is an error message");

    let mut last_ledger = 862846;

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
                        "CDNSUOZJTE5UU4WWTUFLXCUGSZXXH7F7FQFHLX743CA2XRLMYOXSWNM4"
                    ]
                }
            ]
        }
    });

    let client = reqwest::Client::new();

    let response = client.post(url).json(&json_data).send().await?;
    println!("Status Code: {}", "application/json");

    let response_body = response.text().await?;
    println!("Response body: \n {}", response_body);

    let parsed: Value = serde_json::from_str(&response_body)?;
    let latest_ledger = parsed["result"]["latestLedger"]
        .as_i64()
        .unwrap_or(start_ledger as i64) as i32;

    Ok(latest_ledger)
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
