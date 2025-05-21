use core::time;
use dotenvy::dotenv;
use log::info;
use std::{cell::RefCell, env, rc::Rc, thread};

use self::models::*;
use anyhow::{Error, Result};
use diesel::prelude::*;
use liquidation_bot::utils::{decode_loan_from_simulate_response, decode_topic, decode_value};
use liquidation_bot::*;
use soroban_client::{
    account::{Account, AccountBehavior},
    address::{Address, AddressTrait},
    keypair::{Keypair, KeypairBehavior},
    network::{NetworkPassphrase, Networks},
    operation::Operation,
    transaction::{TransactionBehavior, TransactionBuilder, TransactionBuilderBehavior},
    Options,
};
use stellar_rpc_client::{self, Event, EventStart, EventType, GetEventsResponse};

const SLEEP_TIME_SECONDS: u64 = 10;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let connection = &mut establish_connection();
    env_logger::init();

    // info!("This is an info message");
    // warn!("This is a warning message");
    // error!("This is an error message");

    let mut ledger = 993790;

    loop {
        let GetEventsResponse {
            events,
            latest_ledger: new_ledger,
        } = fetch_events(ledger).await?;
        ledger = new_ledger;
        find_loans_from_events(events, connection).await?;
        // get_prices();
        // find_liquidateable(connection);
        // attempt_liquidating();

        info!("Sleeping for {SLEEP_TIME_SECONDS} seconds.");
        thread::sleep(time::Duration::from_secs(SLEEP_TIME_SECONDS))
    }
}

async fn fetch_events(ledger: u32) -> Result<GetEventsResponse, Error> {
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

async fn find_loans_from_events(
    events: Vec<Event>,
    connection: &mut PgConnection,
) -> Result<(), Error> {
    for event in events {
        match decode_topic(event.topic.clone()) {
            Ok(topics) => {
                if topics.iter().any(|t| t.to_lowercase() == "loan") {
                    match decode_value(event.value.clone()) {
                        Ok(value) => {
                            println!("Loan event: {:#?}", value);
                            fetch_loan_to_db(value, connection).await?;
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

async fn fetch_loan_to_db(loan: Vec<String>, connection: &mut PgConnection) -> Result<(), Error> {
    let url = "https://soroban-testnet.stellar.org";
    let server =
        soroban_client::Server::new(url, Options::default()).expect("Cannot create server");

    // Load secret and derive public
    dotenv().ok();
    let secret_str =
        env::var("SOROBAN_SECRET_KEY").expect("SOROBAN_SECRET_KEY must be set in .env");
    let source_keypair = Keypair::from_secret(&secret_str).unwrap(); // Expected
    let source_public = source_keypair.public_key();

    let ledger_data = server.get_latest_ledger().await?;
    let source_account = Rc::new(RefCell::new(
        Account::new(&source_public, &ledger_data.sequence.to_string()).unwrap(),
    ));

    // Build operation
    let contract_id_str = "CANE56WP4UAMFTIIFM4IAYWY226RKKLBYBIU6QBOZDMFRNT2COZ6CHC5";
    let method = "get_loan";
    let loan_owner = &loan[1];
    println!("Loan Owner: {}", loan_owner);
    let args = vec![Address::to_sc_val(&Address::from_string(loan_owner).unwrap()).unwrap()];
    let read_loan_op = Operation::new()
        .invoke_contract(contract_id_str, method, args, None)
        .expect("Cannot create invoke_contract operation");

    // Build the transaction
    let mut builder = TransactionBuilder::new(source_account.clone(), Networks::testnet(), None);
    builder.fee(1000u32);
    builder.add_operation(read_loan_op);

    let mut tx = builder.build();
    tx.sign(&[source_keypair.clone()]);

    // Simulate transaction and handle response
    let response = server.simulate_transaction(tx, None).await?;

    let loan = decode_loan_from_simulate_response(response.to_result().unwrap())?;

    save_loan(connection, loan)?;

    Ok(())
}

pub fn save_loan(connection: &mut PgConnection, loan: Loan) -> Result<(), diesel::result::Error> {
    use crate::schema::loans::dsl::*;

    let existing = loans
        .filter(borrower.eq(&loan.borrower))
        .first::<Loan>(connection)
        .optional()?;

    if let Some(existing_loan) = existing {
        diesel::update(loans.filter(id.eq(existing_loan.id)))
            .set((
                borrowed_amount.eq(loan.borrowed_amount),
                borrowed_from.eq(loan.borrowed_from),
                collateral_amount.eq(loan.collateral_amount),
                collateral_from.eq(loan.collateral_from),
                unpaid_interest.eq(loan.unpaid_interest),
            ))
            .execute(connection)?;
    } else {
        diesel::insert_into(loans)
            .values(&loan)
            .execute(connection)?;
    }
    Ok(())
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
