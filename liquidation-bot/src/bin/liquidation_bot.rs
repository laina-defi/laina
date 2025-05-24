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
    dotenv().ok();
    let loan_manager_id =
        env::var("CONTRACT_ID_LOAN_MANAGER").expect("CONTRACT_ID_LOAN_MANAGER must be set in .env");
    info!("Fetching new loans from Loan Manager.");
    let url = "http://localhost:8000/soroban/rpc";
    let client = stellar_rpc_client::Client::new(url)?;

    let start = EventStart::Ledger(ledger);
    let event_type = Some(EventType::Contract);
    let contract_ids = vec![loan_manager_id];
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
    dotenv().ok();
    let url =
        env::var("PUBLIC_SOROBAN_RPC_URL").expect("PUBLIC_SOROBAN_RPC_URL must be set in .env");
    let server =
        soroban_client::Server::new(&url, Options::default()).expect("Cannot create server");

    // Load secret and derive public
    let secret_str =
        env::var("SOROBAN_SECRET_KEY").expect("SOROBAN_SECRET_KEY must be set in .env");
    let source_keypair = Keypair::from_secret(&secret_str).unwrap(); // Expected
    let source_public = source_keypair.public_key();

    let ledger_data = server.get_latest_ledger().await?;
    let source_account = Rc::new(RefCell::new(
        Account::new(&source_public, &ledger_data.sequence.to_string())
            .map_err(|e| anyhow::anyhow!("Account::new failed: {}", e))?,
    ));

    // Build operation
    let loan_manager_id =
        env::var("CONTRACT_ID_LOAN_MANAGER").expect("CONTRACT_ID_LOAN_MANAGER must be set in .env");
    let method = "get_loan";
    let loan_owner = &loan[1];
    println!("Loan Owner: {}", loan_owner);

    let args = vec![Address::to_sc_val(
        &Address::from_string(loan_owner)
            .map_err(|e| anyhow::anyhow!("Account::from_string failed: {}", e))?,
    )
    .map_err(|e| anyhow::anyhow!("Address::to_sc_val failed: {}", e))?];

    let read_loan_op = Operation::new()
        .invoke_contract(&loan_manager_id, method, args, None)
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

#[cfg(test)]
mod tests {
    use super::*;
    use dotenvy::dotenv;
    use std::env;

    fn setup_test_db() -> PgConnection {
        dotenv().ok();
        let test_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        PgConnection::establish(&test_url).expect("Failed to connect to test DB")
    }

    #[tokio::test]
    async fn test_save_loan_inserts_and_updates() {
        let mut conn = setup_test_db();

        use crate::schema::loans::dsl::*;
        diesel::delete(loans.filter(borrower.eq("TEST_BORROWER")))
            .execute(&mut conn)
            .unwrap();

        let test_loan = Loan {
            id: 0,
            borrower: "TEST_BORROWER".into(),
            borrowed_amount: 100,
            borrowed_from: "SourceA".into(),
            collateral_amount: 50,
            collateral_from: "SourceB".into(),
            unpaid_interest: 10,
        };

        save_loan(&mut conn, test_loan.clone()).unwrap();

        let updated_loan = Loan {
            borrowed_amount: 120,
            unpaid_interest: 5,
            ..test_loan
        };

        save_loan(&mut conn, updated_loan).unwrap();

        let saved = loans
            .filter(borrower.eq("TEST_BORROWER"))
            .first::<Loan>(&mut conn)
            .unwrap();

        assert_eq!(saved.borrowed_amount, 120);
        assert_eq!(saved.unpaid_interest, 5);
    }
}
