use core::time;
use log::{error, info, warn};
use std::thread;

use self::models::*;
use anyhow::{Error, Result};
use base64::engine::general_purpose::STANDARD as base64_engine;
use base64::Engine;
use diesel::prelude::*;
use liquidation_bot::*;
use stellar_rpc_client::{self, Event, EventStart, EventType, GetEventsResponse};
use stellar_xdr::curr::{Limits, ReadXdr, ScVal};

const SLEEP_TIME_SECONDS: u64 = 10;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // let connection = &mut establish_connection();
    env_logger::init();

    info!("This is an info message");
    warn!("This is a warning message");
    error!("This is an error message");

    let mut ledger = 993344;

    loop {
        let GetEventsResponse {
            events,
            latest_ledger: new_ledger,
        } = fetch_events(ledger).await?;
        ledger = new_ledger;
        find_loans_from_events(events).await?;
        // get_prices();
        // find_liquidateable(connection);
        // attempt_liquidating();

        info!("Sleeping for {SLEEP_TIME_SECONDS} seconds.");
        thread::sleep(time::Duration::from_secs(SLEEP_TIME_SECONDS))
    }
}

async fn fetch_events(ledger: u32) -> Result<GetEventsResponse, Error> {
    // TODO: fetch loans from Loan Manager
    // TODO: push new loans to the DB.
    info!("Fetching new loans from Loan Manager.");
    let url = "http://localhost:8000/soroban/rpc";
    let client = stellar_rpc_client::Client::new(url)?;

    let start = EventStart::Ledger(ledger);
    let event_type = Some(EventType::Contract);
    let contract_ids = vec!["CANE56WP4UAMFTIIFM4IAYWY226RKKLBYBIU6QBOZDMFRNT2COZ6CHC5".to_string()];
    let topics_slice = &[];
    let limit = Some(100);
    let events_response = client
        .get_events(start, event_type, &contract_ids, topics_slice, limit)
        .await?;
    println!("{:?}", events_response.events);
    Ok(events_response)
}

async fn find_loans_from_events(events: Vec<Event>) -> Result<(), Error> {
    for event in events {
        match decode_topic(event.topic.clone()) {
            Ok(topics) => {
                if topics.iter().any(|t| t.to_lowercase() == "loan") {
                    match decode_value(event.value.clone()) {
                        Ok(value) => {
                            println!("Loan event: {:#?}", value);
                            fetch_loan_to_db(value).await?;
                        }
                        Err(e) => eprintln!("Failed to decode value: {e}"),
                    }
                } else {
                    println!("Not a loan event, skipping.");
                }
            }
            Err(e) => {
                eprintln!("Failed to decode topic: {e}");
                continue;
            }
        }
    }
    Ok(())
}

async fn fetch_loan_to_db(loan: Vec<String>) -> Result<(), Error> {
    Ok(())
}

fn decode_topic(topic: Vec<String>) -> Result<Vec<String>, Error> {
    let mut decoded_topics = Vec::new();

    for string in topic {
        let decoded = base64_engine.decode(string)?;
        let scval = ScVal::from_xdr(
            decoded,
            Limits {
                depth: 64,
                len: 10000,
            },
        )?;

        if let ScVal::Symbol(symbol) = scval {
            decoded_topics.push(symbol.to_string());
        }
    }
    Ok(decoded_topics)
}

fn decode_value(value: String) -> Result<Vec<String>, Error> {
    let decoded = base64_engine.decode(value)?;
    let scval = ScVal::from_xdr(
        decoded,
        Limits {
            depth: 64,
            len: 10000,
        },
    )?;

    let vec = match scval {
        ScVal::Vec(Some(v)) => v,
        _ => return Err(anyhow::anyhow!("Expected ScVal::Vec")),
    };

    let mut result_parts = Vec::new();

    for item in vec.iter() {
        match item {
            ScVal::Symbol(symbol) => result_parts.push(symbol.to_string()),
            ScVal::Address(address) => result_parts.push(address.to_string()),
            other => result_parts.push(format!("{:?}", other)),
        }
    }

    Ok(result_parts)
}
//
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
